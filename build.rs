//! Locate a prebuilt ALE install and compile the C shim against it.
//!
//! When `ALE_ROOT` is unset, or does not contain the header and static library
//! this crate needs, the build **succeeds** and emits no `ale_linked` cfg. The
//! crate then compiles to a surface whose every constructor reports
//! [`AleError::NotLinked`], so a downstream test suite can skip cleanly instead
//! of failing on a machine without a C++ toolchain, CMake, and zlib
//!
//! Deliberately no `cc` dependency: this drives `$CXX` (default `c++`) and
//! `$AR` (default `ar`) directly, so the crate has no dependencies at all.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(ale_linked)");
    println!("cargo::rerun-if-env-changed=ALE_ROOT");
    println!("cargo::rerun-if-env-changed=CXX");
    println!("cargo::rerun-if-env-changed=AR");
    println!("cargo::rerun-if-changed=csrc/ale_shim.cpp");
    println!("cargo::rerun-if-changed=csrc/ale_shim.h");

    let Some(root) = ale_root() else {
        println!(
            "cargo::warning=ALE_ROOT is unset or incomplete; ale-rs built unlinked \
             (every constructor will report NotLinked)"
        );
        return;
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let object = out_dir.join("ale_shim.o");
    let archive = out_dir.join("libale_shim.a");

    let compiler = env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    run(
        Command::new(&compiler)
            .args(["-std=c++17", "-O2", "-fPIC", "-c"])
            .arg("csrc/ale_shim.cpp")
            .arg("-Icsrc")
            .arg(format!("-I{}", root.join("include").display()))
            .arg("-o")
            .arg(&object),
        &compiler,
    );

    let archiver = env::var("AR").unwrap_or_else(|_| "ar".to_string());
    // `rcs` rather than an append: a stale archive from an earlier build would
    // otherwise keep an old object alongside the new one.
    let _ = std::fs::remove_file(&archive);
    run(
        Command::new(&archiver)
            .arg("rcs")
            .arg(&archive)
            .arg(&object),
        &archiver,
    );

    println!("cargo::rustc-link-search=native={}", out_dir.display());
    println!("cargo::rustc-link-lib=static=ale_shim");
    println!(
        "cargo::rustc-link-search=native={}",
        root.join("lib").display()
    );
    println!("cargo::rustc-link-lib=static=ale");
    println!("cargo::rustc-link-lib=dylib=z");
    println!("cargo::rustc-link-lib=dylib={}", cxx_runtime());
    println!("cargo::rustc-cfg=ale_linked");
}

/// `Some(root)` only when the root actually carries both artifacts the link
/// line needs. A path that exists but holds neither is treated exactly like an
/// unset variable, so a half-finished CMake install degrades to "skip" instead
/// of to a link error at the end of a long build.
fn ale_root() -> Option<PathBuf> {
    let root = PathBuf::from(env::var_os("ALE_ROOT")?);
    let header = root.join("include/ale/ale_interface.hpp");
    let library = root.join("lib/libale.a");
    if header.is_file() && library.is_file() {
        Some(root)
    } else {
        None
    }
}

fn cxx_runtime() -> &'static str {
    // libc++ on Apple and the BSDs, libstdc++ elsewhere. Keyed on the target
    // Cargo is building for, not the host.
    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos" | "ios" | "freebsd" | "openbsd") => "c++",
        _ => "stdc++",
    }
}

fn run(command: &mut Command, program: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to invoke {program}: {error}"));
    assert!(status.success(), "{program} failed with {status}");
}
