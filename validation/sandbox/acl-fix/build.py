#!/usr/bin/env python3
"""Extend the finalization prototype with experimental ACL corrections."""
import argparse
import difflib
import hashlib
import json
import pathlib
import shutil
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--worker-build", required=True, type=pathlib.Path,
                    help="disposable directory produced by finalize/build.py")
args = parser.parse_args()
out = args.worker_build.resolve()
code = out / "source/misc/fuse2fs.c"
before = code.read_text()
assert "probe_acl_mode" not in before, "use a fresh finalization build"


def replace(text, old, new, count=1):
    assert text.count(old) == count, old
    return text.replace(old, new)


after = replace(before, "#include <stdatomic.h>",
                "#include <stdatomic.h>\n#include <limits.h>\n#include <sys/acl.h>\n"
                "#include <acl/libacl.h>\n#include <linux/posix_acl_xattr.h>")
start = after.index("static int propagate_default_acls(")
end = after.index("static int op_mknod(", start)
after = after[:start] + (HERE / "acl.inc").read_text() + "\n\n" + after[end:]
after = replace(after, "\t\tconn->want |= FUSE_CAP_POSIX_ACL;",
                "\t\tconn->want |= FUSE_CAP_POSIX_ACL | FUSE_CAP_DONT_MASK;")
after = replace(after, "\tret = get_req_groups(ff, &gids, &nr_gids);",
                "\tif (ctxt->gid == gid)\n\t\treturn 1;\n\n"
                "\tret = get_req_groups(ff, &gids, &nr_gids);")
# chmod must update the ACL class permissions after writing its inode mode.
start = after.index("static int op_chmod(")
end = after.index("static int op_chown(", start)
part = after[start:end]
part = replace(part, "\nout:\n", "\tret = probe_acl_chmod(ff, ino, mode);\n\nout:\n")
after = after[:start] + part + after[end:]
# setxattr must reject malformed ACL before storing, then reconcile inode mode.
start = after.index("static int op_setxattr(")
end = after.index("static int op_removexattr(", start)
part = after[start:end]
part = replace(part, "\tint ret = 0;", "\tint ret = 0;\n\tmode_t acl_mode = 0;")
part = replace(part, "\terr = ext2fs_xattr_set(h, key, value, len);",
               "\tif (!strcmp(key, XATTR_NAME_POSIX_ACL_ACCESS) ||\n"
               "\t    !strcmp(key, XATTR_NAME_POSIX_ACL_DEFAULT)) {\n"
               "\t\tret = probe_acl_mode(value, len, &acl_mode);\n"
               "\t\tif (ret)\n\t\t\tgoto out2;\n\t}\n"
               "\tif (!strcmp(key, XATTR_NAME_POSIX_ACL_ACCESS) &&\n"
               "\t    len == sizeof(struct posix_acl_xattr_header) +\n"
               "\t           3 * sizeof(struct posix_acl_xattr_entry)) {\n"
               "\t\terr = ext2fs_xattr_remove(h, key);\n"
               "\t\tif (err == EXT2_ET_EA_KEY_NOT_FOUND)\n\t\t\terr = 0;\n"
               "\t} else\n\t\terr = ext2fs_xattr_set(h, key, value, len);")
part = replace(part, "\tret = update_ctime(fs, ino, NULL);",
               "\tif (!strcmp(key, XATTR_NAME_POSIX_ACL_ACCESS))\n"
               "\t\tret = probe_acl_inode_mode(ff, ino, acl_mode, 1);\n"
               "\telse\n\t\tret = update_ctime(fs, ino, NULL);")
after = after[:start] + part + after[end:]
# mkdir originally skips ACL propagation on filesystems without metadata_csum.
start = after.index("static int op_mkdir(")
end = after.index("static int ", start + 12)
part = after[start:end]
part = replace(part, "\tret = propagate_default_acls(ff, parent, child);\n"
               "\tif (ret)\n\t\tgoto out3;\n", "")
part = replace(part, "\t/* Rewrite the directory block checksum",
               "\tret = propagate_default_acls(ff, parent, child);\n"
               "\tif (ret)\n\t\tgoto out2;\n\n\t/* Rewrite the directory block checksum")
after = after[:start] + part + after[end:]
code.write_text(after)
(out / "acl.patch").write_text("".join(difflib.unified_diff(
    before.splitlines(True), after.splitlines(True),
    fromfile="a/misc/fuse2fs.c", tofile="b/misc/fuse2fs.c")))
with (out / "acl-build.log").open("wb") as stream:
    subprocess.run(["make", "-C", str(out / "build/misc"), "-j8", "fuse2fs",
                    "LIBFUSE=-l:libfuse3.so.4 -lacl"],
                   stdout=stream, stderr=subprocess.STDOUT, check=True)
shutil.copy2(out / "build/misc/fuse2fs", out / "acl-patched")
inputs = {}
for path in (code, out / "acl-patched", out / "acl.patch", HERE / "acl.inc", HERE / "build.py"):
    inputs[str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
(out / "acl-build-inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
print(f"Experimental ACL worker: {out}/acl-patched")
