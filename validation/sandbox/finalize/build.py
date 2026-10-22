#!/usr/bin/env python3
"""Build standalone baseline/experimental fuse2fs fixtures; install nothing."""
import argparse
import difflib
import hashlib
import json
import os
import pathlib
import shutil
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", type=pathlib.Path, required=True)
parser.add_argument("--fuse-headers", type=pathlib.Path, required=True)
parser.add_argument("--fuse-config", type=pathlib.Path, required=True)
parser.add_argument("--fuse-library", type=pathlib.Path, required=True)
parser.add_argument("--out", type=pathlib.Path, required=True)
args = parser.parse_args()
out = args.out.resolve()
out.mkdir()
source = out / "source"
shutil.copytree(args.source, source)
code = source / "misc/fuse2fs.c"
before = code.read_text()
assert hashlib.sha256(code.read_bytes()).hexdigest() == (
    "a00756d5e7b74c936c8574b18acc24d2584b2bb0fc0b6e69887d07b655be4555")
build = out / "build"
build.mkdir()
headers = out / "fuse-headers"
shutil.copytree(args.fuse_headers, headers)
shutil.copy2(args.fuse_config, headers / "libfuse_config.h")
library = args.fuse_library.resolve()
env = os.environ | {
    "fuse3_CFLAGS": f"-I{headers}",
    "fuse3_LIBS": str(library),
}


def run(command, log):
    with (out / log).open("wb") as stream:
        subprocess.run(command, cwd=build, env=env, stdout=stream,
                       stderr=subprocess.STDOUT, check=True)


run([str(source / "configure"), "--disable-nls", "--disable-elf-shlibs",
     "--disable-fsck", "--disable-e2initrd-helper", "--enable-fuse2fs"], "configure.log")
run(["make", "-j8", "libs"], "libs.log")
run(["make", "-C", "misc", "-j8", "fuse2fs"], "baseline.log")
binary = build / "misc/fuse2fs"
shutil.copy2(binary, out / "baseline")
run(["gcc", "-I" + str(build / "lib"), "-I" + str(source / "lib"),
     "-c", str(HERE / "close-fault.c"), "-o", str(build / "close-fault.o")],
    "wrapper.log")
binary.unlink()
run(["make", "-C", "misc", "fuse2fs",
     f"LDFLAGS=-Wl,--wrap=ext2fs_close {build / 'close-fault.o'}"],
    "baseline-close.log")
shutil.copy2(binary, out / "baseline-close")


def replace(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


after = replace(before, "#include <pthread.h>", "#include <pthread.h>\n#include <stdatomic.h>")
after = replace(after, "\tchar *lockfile;\n};", "\tchar *lockfile;\n\tatomic_int io_failed;\n};")
after = replace(after, "\tmemset(&fctx, 0, sizeof(fctx));",
                "\tmemset(&fctx, 0, sizeof(fctx));\n\tatomic_init(&fctx.io_failed, 0);")
after = replace(after, "\tif (!is_err)\n\t\treturn ret;\n",
                "\tif (!is_err)\n\t\treturn ret;\n\n\tatomic_store(&ff->io_failed, 1);\n")
after = replace(after, '\t\tif (err)\n\t\t\tcom_err(argv[0], err, "while closing fs");',
                '\t\tif (err) {\n\t\t\tcom_err(argv[0], err, "while closing fs");\n'
                '\t\t\tatomic_store(&fctx.io_failed, 1);\n\t\t}')
after = replace(after, "\tfuse_opt_free_args(&args);\n\treturn ret;",
                "\tfuse_opt_free_args(&args);\n\tif (atomic_load(&fctx.io_failed))\n"
                "\t\tret |= 32;\n\treturn ret;")
code.write_text(after)
(out / "finalization.patch").write_text("".join(difflib.unified_diff(
    before.splitlines(True), after.splitlines(True),
    fromfile="a/misc/fuse2fs.c", tofile="b/misc/fuse2fs.c")))
run(["make", "-C", "misc", "-j8", "fuse2fs"], "patched.log")
shutil.copy2(binary, out / "patched")
binary.unlink()
run(["make", "-C", "misc", "fuse2fs",
     f"LDFLAGS=-Wl,--wrap=ext2fs_close {build / 'close-fault.o'}"],
    "patched-close.log")
shutil.copy2(binary, out / "patched-close")
inputs = {"source_file_before": hashlib.sha256(before.encode()).hexdigest(),
          "files": {}, "source_files": {}, "header_files": {}}
for directory, key in ((source, "source_files"), (headers, "header_files")):
    for path in sorted(directory.rglob("*")):
        if path.is_file():
            inputs[key][str(path.relative_to(directory))] = hashlib.sha256(path.read_bytes()).hexdigest()
inputs["compiler_version"] = subprocess.check_output(["gcc", "--version"], text=True)
for path in [out / n for n in ("baseline", "baseline-close", "patched", "patched-close",
                               "finalization.patch")] + [library, HERE / "close-fault.c", HERE / "build.py"]:
    inputs["files"][str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
(out / "build-inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
print(f"Built baseline and experimental workers in {out}; installed nothing.")
