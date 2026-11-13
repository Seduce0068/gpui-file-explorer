// SPDX-License-Identifier: AGPL-3.0-or-later
//! Link workaround for Linux hosts without `libxkbcommon-x11-dev`.
//!
//! This PoC uses `gpui-kit`, which enables the X11 feature of
//! `gpui-pre-platform` unconditionally, so `gpui-pre-linux` links
//! `-lxkbcommon-x11`. Distributions ship the runtime library
//! (`libxkbcommon-x11.so.0`) separately from the development files (the
//! unversioned `libxkbcommon-x11.so` symlink and pkg-config file).
//!
//! `scripts/dev-setup.sh` creates `native-libs/libxkbcommon-x11.so` as a
//! symlink to the installed runtime library when the development files are
//! missing. This build script adds that directory to the linker search path
//! with an absolute path derived from `CARGO_MANIFEST_DIR`, so the build works
//! regardless of the current working directory.
//!
//! On hosts where the development package is installed the symlink is absent
//! and this script does nothing.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = match std::env::var_os("CARGO_MANIFEST_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => return,
    };

    let native_libs = manifest_dir.join("native-libs");
    if !native_libs.is_dir() {
        return;
    }

    let shim = native_libs.join("libxkbcommon-x11.so");
    if !shim.exists() {
        return;
    }

    println!("cargo:rerun-if-changed={}", shim.display());
    if let Some(path) = native_libs.to_str() {
        println!("cargo:rustc-link-search=native={path}");
    }
}
