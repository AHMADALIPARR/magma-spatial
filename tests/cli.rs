// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque

//! Command-line contract tests: exit codes and diagnostics are part
//! of the tool's interface and are pinned here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_magma-spatial"))
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("magma_cli_{}_{}", std::process::id(), tag));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_source(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn usage_error_exits_2() {
    let out = bin().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("usage: magma-spatial compile <source.bf> <output.v>"));

    let out = bin().arg("frobnicate").arg("a").arg("b").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn missing_source_exits_1() {
    let dir = scratch("missing");
    let out = bin()
        .arg("compile")
        .arg(dir.join("nope.bf"))
        .arg(dir.join("out.v"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .starts_with("cannot read source:"));
}

#[test]
fn loops_are_rejected() {
    let dir = scratch("loops");
    let src = write_source(&dir, "l.bf", "++[>+<-]");
    let out = bin()
        .arg("compile")
        .arg(&src)
        .arg(dir.join("out.v"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(out.stderr).unwrap().trim(),
        "dynamic loop at character 2: feedback synthesis not implemented"
    );
}

#[test]
fn input_is_rejected() {
    let dir = scratch("input");
    let src = write_source(&dir, "i.bf", ",.");
    let out = bin()
        .arg("compile")
        .arg(&src)
        .arg(dir.join("out.v"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(out.stderr).unwrap().trim(),
        "input at character 0: streaming input synthesis not implemented"
    );
}

#[test]
fn resource_guard_is_enforced() {
    let dir = scratch("guard");
    let src = write_source(&dir, "g.bf", &">".repeat(4096));
    let out = bin()
        .arg("compile")
        .arg(&src)
        .arg(dir.join("out.v"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(out.stderr).unwrap().trim(),
        "more than 4096 addressed cells: resource guard"
    );
}

#[test]
fn parallel_example_compiles_with_status_line() {
    let dir = scratch("ok");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = bin()
        .arg("compile")
        .arg(manifest.join("examples/parallel.bf"))
        .arg(dir.join("parallel.v"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "compiled: 3 cells, 10 writes, 3 output taps, no instruction pointer"
    );
    let v = fs::read_to_string(dir.join("parallel.v")).unwrap();
    assert!(v.contains("assign cell_out_2 = cell_in_2 + 16'd65534;"));
    assert!(v.contains("assign print_0 = cell_in_2 + 16'd65534;"));
}
