# Control namespace ownership without PID 1 coupling

Replaced InitMountNamespace with ControlMountNamespace. The owner pins the
acquisition thread's mount namespace and compares namespace inode/device and
nsfs type before control operations. Attached-mount ID validation remains.
Removed PID 1 pidfd polling, same-PID-namespace checks and the requirement that
conglobate share init's mount namespace. The dedicated-VM environment still owns
independent crash shutdown; this is not a substitute for that contract.

No command namespace, capability, process-drain or publication rule changes.
The former implementation is preserved under pre-init-namespace-removal.

Disposable native Fedora 7.2.10 validation passes:
- runtime-private-namespace: success, operation failure and retained cleanup
  failure all work after entering a private supervisor mount namespace;
- runtime-changed-namespace: changing namespace after acquisition rejects close
  and retry before removing the attachment; VM disposal cleans abandoned views;
- contained: launch/join failures and descendant/helper-death drain still pass.

Each guest archive's 152 source files match the implementation tested here.
Rootless 227 tests/46 doctests, strict root/lifecycle Clippy and formatting pass.
CI now includes both new namespace cases. Hosted results on this new commit are
pending; earlier 1523bc1 rootless and lifecycle runs are green but do not qualify
this change. No full adherence gate advances. Large guest assets remain local
with hashes, not durable M8 archives. Tmpfs mount ownership remains undecided.
