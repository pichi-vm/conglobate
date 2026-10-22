#!/usr/bin/env python3
"""Exhaustive finite publication arbitration model, not a filesystem test.

The negative model assumes a cancellation acknowledgment promises no output.
It does not claim today's signal handler implements such an acknowledgment.
"""
from dataclasses import dataclass, replace
import json


@dataclass(frozen=True)
class State:
    phase: str = 'building'
    eligible: bool = False
    destination: str = 'absent'
    cancellation: str = 'none'
    synced: bool = False
    returned: str = 'none'


TERMINAL = {'cancelled', 'failed', 'collision', 'returned', 'unknown', 'visible-error'}


def steps(s, reserved):
    if s.phase in TERMINAL:
        return
    if s.cancellation == 'none':
        if s.phase in ('building', 'prepared') or (not reserved and s.phase == 'checked'):
            # The broken check-then-rename variant permits the already checked
            # commit to proceed even after promising cancellation.
            phase = s.phase if s.phase == 'checked' else 'cancelled'
            yield 'cancel-accepted', replace(s, phase=phase, cancellation='accepted')
        else:
            yield 'cancel-pending', replace(s, cancellation='pending')
    yield 'publisher-dies', replace(s, phase='unknown', returned='unknown')
    if s.phase == 'building':
        yield 'completion-barrier-passes', replace(s, phase='prepared', eligible=True)
        yield 'completion-barrier-fails', replace(s, phase='failed', returned='failed')
    elif s.phase == 'prepared':
        yield 'reserve-commit' if reserved else 'check-cancel', replace(
            s, phase='reserved' if reserved else 'checked')
    elif s.phase in ('reserved', 'checked'):
        yield 'rename-noreplace-succeeds', replace(s, phase='visible', destination='ours')
        yield 'foreign-destination-wins', replace(s, phase='collision', destination='foreign', returned='collision')
        yield 'rename-fails-without-output', replace(s, phase='failed', returned='failed')
    elif s.phase == 'visible':
        yield 'parent-sync-succeeds', replace(s, phase='synced', synced=True)
        yield 'parent-sync-fails', replace(s, phase='visible-error', returned='visible-error')
    elif s.phase == 'synced':
        yield 'return-committed', replace(s, phase='returned', returned='committed')


def violation(s):
    if s.cancellation == 'accepted' and s.destination == 'ours':
        return 'accepted cancellation followed by visible output'
    if s.destination == 'ours' and not s.eligible:
        return 'publication without completion eligibility'
    if s.returned == 'committed' and (s.destination != 'ours' or not s.synced):
        return 'success without visible output and completed sync'
    if s.returned == 'failed' and s.destination == 'ours':
        return 'visible output misreported as no-output failure'
    return None


def explore(reserved):
    pending = [(State(), [])]
    visited, failures = set(), []
    transitions = 0
    while pending:
        state, trace = pending.pop()
        if state in visited:
            continue
        visited.add(state)
        if problem := violation(state):
            failures.append({'problem': problem, 'trace': trace})
        for event, next_state in steps(state, reserved):
            transitions += 1
            pending.append((next_state, trace + [event]))
    return {'states': len(visited), 'transitions': transitions, 'violations': failures,
            'terminal_outcomes': sorted({s.returned for s in visited if s.phase in TERMINAL})}


negative, candidate = explore(False), explore(True)
assert any(v['problem'] == 'accepted cancellation followed by visible output'
           for v in negative['violations'])
assert not candidate['violations']
assert {'committed', 'unknown', 'collision', 'failed', 'visible-error'} <= set(candidate['terminal_outcomes'])
print(json.dumps({'scope': 'finite abstract arbitration only; no runtime or recovery proof',
                  'negative_check_then_rename': negative, 'candidate_reservation': candidate}, indent=2))
