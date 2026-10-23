# Remaining lifecycle abstraction dispositions

These are reconsidered suggestions, not defects silently marked implemented.
Original review records remain unchanged; this assessment updates current status.
No complete M4 adherence pass is claimed.

## simplification-003: decline shared path-construction abstraction

MountPoint, volume Point, RuntimeMounts and DomainTarget each construct a proc-FD
path from their own retained parent and leaf. Inspection confirms materially
different validation and cleanup: protected tmpfs attachment validation, owned
volume directory removal, ordered runtime unmounts, and namespace-verified cgroup
removal. A shared owner would erase these distinctions. A borrowed helper only
sharing the one expression would add a type or generic utility method without
strengthening the parent lifetime, path confinement or cleanup guarantee. Keep
the local one-line methods. This revises the earlier accepted deduplication
suggestion on benefit/complexity grounds, not a claim that deduplication occurred.

## simplification-013: decline generic public teardown wrapper

Ready and Cancelling start fresh fixed-budget teardown in distinct Phase variants.
Running also has teardown_until (existing deadline) and teardown_after_command
(AwaitingCleanup before cancellation). The private Teardown::run already shares
the ownership transitions, failure reconstruction and original deadline. A generic
public method would require a new externally nameable sealed state trait or a
private bound on public API, plus three conversions to the private Phase enum.
That adds abstraction around three short wrappers and does not reduce the core
transition logic. Retain explicit typed entrypoints and the common private engine.
No behavior, deadline, recovery owner or API is changed by this disposition.

## dependencies-008: fixture diagnostic implemented

Binary policy installation was already removed. The two trusted fixture compiler
paths now share an exception wrapper identifying clang/BPF, C compiler, the
current libbpf headers and shared library prerequisites. Stderr stays attached;
exception chaining preserves FileNotFoundError/OSError or CalledProcessError.
Successful compilation behavior and compiler arguments are unchanged. The fixed
local prerequisites remain a portability issue for M7 VM CI; this diagnostic does
not claim to make that runner portable or add binary dependencies.

Validation: successful /bin/true, failing /bin/false and an explicitly nonexistent
compiler invocation verify success/error behavior and preserved causes. Python
syntax compilation passes. No privileged operation or VM run is needed for this
error-message-only change; prior VM results are not described as new validation.
