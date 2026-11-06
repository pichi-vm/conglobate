#!/bin/bash
set -eu
status=0
for attempt in $(seq 1 100); do
  echo "LOOP_ATTEMPT $attempt uptime=$(cat /proc/uptime)"
  "$CONGLOBATE_TEST_BIN" --ignored --exact storage::partitioned::tests::root_views_keep_disk_offsets_in_one_snapshot --nocapture --test-threads=1 || { status=$?; break; }
done
printf '%s\n' "$status" > "$LOOP_EVIDENCE/test-status"
exit "$status"
