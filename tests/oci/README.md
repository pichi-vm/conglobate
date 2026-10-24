# OCI tool interoperability

`run.py` migrates the original ORAS graph-copy assertion from the exploratory VM
pipeline fixture. It creates synthetic snapshot/verity blobs and a committed
pass-through recipe, publishes through the real CLI, then requires ORAS to copy
exactly the referenced manifest/config/layer graph with identical digests/bytes.
The inventory remains a separate conglobate artifact file, outside ORAS's graph.

No filesystem mount, command step, VM, download or initial-namespace privilege is
used by this test. A private unprivileged user namespace maps only the caller's
UID to satisfy the CLI's root check for the pass-through path. It is not evidence
that privileged image builds can run rootlessly. The runner must permit ordinary
unprivileged user namespaces. CI uses checksum-pinned ORAS 1.3.5 on amd64.

```sh
python3 tests/oci/run.py /path/to/new/result \
  --binary target/debug/conglobate --oras /path/to/oras
```
