#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Install synthetic-input tools into a user-space directory WITHOUT sudo.
#
# Why: this machine has no passwordless sudo and /dev/uinput is root:root 0600,
# so ydotool cannot be used. wtype needs a compositor that implements
# zwp_virtual_keyboard_v1 (KWin on this host does not). xdotool via Xwayland
# XTEST works when DISPLAY points at an Xwayland server with the XTEST ext.
#
# Usage:
#   bash scripts/setup-e2e-input.sh
#
# Result: tools are unpacked under $E2E_TOOLS_DIR (default ~/.local/e2e-tools).
# Print the PATH export line to add to your shell.
set -euo pipefail

e2e_dir="${E2E_TOOLS_DIR:-${HOME}/.local/e2e-tools}"
bin_dir="${e2e_dir}/bin"
tmp_dir="${e2e_dir}/.deb"

mkdir -p "${bin_dir}" "${tmp_dir}"

install_deb() {
  local package="$1"
  local binary="$2"
  if command -v "${binary}" >/dev/null 2>&1; then
    echo "${binary}: already on PATH"
    return
  fi
  if [ -x "${bin_dir}/${binary}" ]; then
    echo "${binary}: already installed in ${bin_dir}"
    return
  fi
  echo "Downloading ${package} (no sudo)..."
  (cd "${tmp_dir}" && apt-get download "${package}")
  local deb
  deb="$(find "${tmp_dir}" -maxdepth 1 -name "${package}_*.deb" | head -n1)"
  if [ -z "${deb}" ]; then
    echo "error: could not download ${package}" >&2
    return 1
  fi
  dpkg-deb -x "${deb}" "${tmp_dir}/root"
  local source
  source="$(find "${tmp_dir}/root" -type f -name "${binary}" | head -n1)"
  if [ -z "${source}" ]; then
    echo "error: ${binary} not found in ${deb}" >&2
    return 1
  fi
  install -m755 "${source}" "${bin_dir}/${binary}"
  echo "${binary}: installed to ${bin_dir}/${binary}"
}

install_deb wtype wtype
install_deb xdotool xdotool

echo
echo "Add this to your shell (or prefix E2E commands with it):"
echo "  export PATH=\"${bin_dir}:\${PATH}\""
echo
echo "Probe availability:"
echo "  DISPLAY=${DISPLAY:-<unset>} xdotool getactivewindow"
echo "  wtype -k a"
