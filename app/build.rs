// SPDX-License-Identifier: AGPL-3.0-or-later
//! Link workaround for Linux hosts without `libxkbcommon-x11-dev`.
//!
//! `gpui-kit` enables the X11 feature of `gpui-pre-platform` unconditionally, so
//! `gpui-pre-linux` links `-lxkbcommon-x11`. Distributions ship the runtime
//! library (`libxkbcommon-x11.so.0`) in a separate package from the development
//! files (the unversioned `libxkbcommon-x11.so` symlink and pkg-config file).
//!
//! When the development files are missing, `scripts/dev-setup.sh` creates
//! `native-libs/libxkbcommon-x11.so` as a symlink to the installed runtime
//! library. This build script adds that directory to the linker search path
//! using an absolute path derived from `CARGO_MANIFEST_DIR`, so the build works
//! regardless of the current working directory (a relative `-L native=...` in
//! `.cargo/config.toml` would be resolved against the caller's CWD).
//!
//! On hosts where the development package is installed the symlink is absent
//! and the build script does nothing; the default linker search path applies.

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

    // Only help when the local shim is actually present; otherwise let the
    // default search path resolve `-lxkbcommon-x11`.
    let shim = native_libs.join("libxkbcommon-x11.so");
    if !shim.exists() {
        return;
    }

    println!("cargo:rerun-if-changed={}", shim.display());
    if let Some(path) = native_libs.to_str() {
        println!("cargo:rustc-link-search=native={path}");
    }
}
