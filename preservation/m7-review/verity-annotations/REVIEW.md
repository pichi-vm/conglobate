# Shared fixed verity parameters

Accepted simplification-008 remained present at fe545ca. A private associated
constructor now produces the five fixed verity annotation fields used by fresh
bootstrap and no-base firmware assembly. It deliberately has no root/parent
argument: those values still belong to the independent assembly paths.

Reviewed the diff against all three ancestry anchors:

- Firmware assembly takes the selected base hash when a base exists; only the
  no-base case starts from zero. The annotation constructor is used only when
  no base exists and scutes are present. Every scute parent is still checked.
- Bootstrap requires nonempty scutes and a zero first parent, checks each link,
  then sets the final hash after the loop.
- Derived assembly retains its existing parent selection and validation. It was
  not rewritten to treat a missing inherited root as a zero root.

131 library tests and strict all-target Clippy passed. A current-source native
amd64 disposable VM completed two-root bootstrap, remount checks, sealing and
export. Its maintained verifier checked the OCI descriptor graph, formatter
receipt, absence of inherited tool components and canonical CBOR inventory.
The emitted manifest contains the original five parameter values plus its final
root hash. All 168 compared source files match the preserved VM snapshot.

VM evidence and artifact: sibling verity-annotations-bootstrap-v1/. No fresh
firmware-replacement VM, target distro boot, ARM or confidential-platform claim.
Large VM assets are hashed and locally retained, not remotely durable.
This is a focused review, not a complete adherence pass. M4 remains 0/3.
