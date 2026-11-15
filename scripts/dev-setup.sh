#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Local build setup for Linux hosts that are missing the X11 xkbcommon
# development files (Kubuntu 2026.04 is one: it ships libxkbcommon-x11-0 but
# not libxkbcommon-x11-dev).
#
# `gpui-kit` enables the X11 feature of `gpui-pre-platform` unconditionally, so
# the final link needs `-lxkbcommon-x11`. Without the development package the
# unversioned `libxkbcommon-x11.so` symlink is absent and the link fails.
#
# This script creates `<crate>/native-libs/libxkbcommon-x11.so` pointing at the
# installed runtime library for every crate in this repository (app/ and poc/).
# Each crate's build.rs adds that directory to the linker search path with an
# absolute path. No system files are modified and no root privileges are
# required.
#
# Preferred alternative (system-wide, needs sudo):
#   sudo apt install libxkbcommon-x11-dev
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"

runtime_lib=""
if ! pkg-config --exists xkbcommon-x11 2>/dev/null; then
  for candidate in \
    /usr/lib/x86_64-linux-gnu/libxkbcommon-x11.so.0 \
    /usr/lib/libxkbcommon-x11.so.0 \
    /usr/lib64/libxkbcommon-x11.so.0 \
    /usr/lib/aarch64-linux-gnu/libxkbcommon-x11.so.0; do
    if [ -e "${candidate}" ]; then
      runtime_lib="${candidate}"
      break
    fi
  done
  if [ -z "${runtime_lib}" ]; then
    runtime_lib="$(ldconfig -p 2>/dev/null | awk '/libxkbcommon-x11\.so\.0/ {print $NF; exit}')"
  fi
fi

if pkg-config --exists xkbcommon-x11 2>/dev/null; then
  echo "xkbcommon-x11 development files found via pkg-config."
  echo "Removing local shims (not needed on this host)."
  for crate in app poc; do
    rm -f "${repo_root}/${crate}/native-libs/libxkbcommon-x11.so"
    rmdir "${repo_root}/${crate}/native-libs" 2>/dev/null || true
  done
  exit 0
fi

if [ -z "${runtime_lib}" ] || [ ! -e "${runtime_lib}" ]; then
  echo "error: could not find libxkbcommon-x11.so.0 on this host." >&2
  echo "Install the development package instead: sudo apt install libxkbcommon-x11-dev" >&2
  exit 1
fi

for crate in app poc; do
  native_libs="${repo_root}/${crate}/native-libs"
  mkdir -p "${native_libs}"
  ln -sfn "${runtime_lib}" "${native_libs}/libxkbcommon-x11.so"
  echo "Created ${native_libs}/libxkbcommon-x11.so -> ${runtime_lib}"
done
echo "Each crate's build.rs adds its native-libs directory to the linker search path."
