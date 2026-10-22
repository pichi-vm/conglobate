// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=policy/labels.bpf.c");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?);
    libbpf_cargo::SkeletonBuilder::new()
        .source("policy/labels.bpf.c")
        .obj(output.join("labels.bpf.o"))
        .clang_args(["-Wall", "-Werror"])
        .build()?;
    Ok(())
}
