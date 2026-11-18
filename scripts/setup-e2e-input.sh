#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Install synthetic-input tools into a user-space directory WITHOUT sudo.
#
# Why: this machine has no passwordless sudo and /dev/uinput is root:root 0600,
# so ydotool cannot be used. wtype needs a compositor that implements
# zwp_virtual_keyboard_v1 (KWin on this host does not). xdotool via Xwayland
# XTEST needs DISPLAY plus the "Control input devices" portal approval, which
# is pending on this host. All three tools install and link fine; it is the
# delivery to a Wayland-native app that fails here.
#
# Usage:
#   bash scripts/setup-e2e-input.sh
#
# Result: tools are unpacked under $E2E_TOOLS_DIR (default ~/.local/e2e-tools).
# The script verifies each binary with ldd, runs version smoke tests, and
# prints the PATH export plus a probe guide that separates install checks from
# input-delivery checks (the latter do NOT prove delivery on this host).
set -euo pipefail

e2e_dir="${E2E_TOOLS_DIR:-${HOME}/.local/e2e-tools}"
bin_dir="${e2e_dir}/bin"
lib_dir="${e2e_dir}/lib"
tmp_dir="${e2e_dir}/.deb"

mkdir -p "${bin_dir}" "${lib_dir}" "${tmp_dir}"

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
  source="$(find "${tmp_dir}/root" -type f -name "${binary}" -perm -u+x | head -n1)"
  if [ -z "${source}" ]; then
    echo "error: ${binary} not found in ${deb}" >&2
    return 1
  fi
  install -m755 "${source}" "${bin_dir}/${binary}"
  echo "${binary}: installed to ${bin_dir}/${binary}"
}

# xdotool links against libxdo.so.3 from the separate libxdo3 package.
# Only when ldd reports a missing library, extract libxdo3 into a private
# directory (no sudo, no system changes) so the checks below are meaningful.
rescue_libxdo3() {
  local xdotool="${bin_dir}/xdotool"
  [ -x "${xdotool}" ] || return 0
  local ldd_out
  ldd_out="$(ldd "${xdotool}" 2>&1 || true)"
  case "${ldd_out}" in
  *'not found'*) ;;
  *) return 0 ;;
  esac
  echo "xdotool: unresolved shared library detected; extracting libxdo3 privately (no sudo)..."
  if ! (cd "${tmp_dir}" && apt-get download libxdo3); then
    echo "warning: could not download libxdo3; ldd will report the missing dependency below" >&2
    return 0
  fi
  local deb
  deb="$(find "${tmp_dir}" -maxdepth 1 -name 'libxdo3_*.deb' | head -n1)"
  if [ -z "${deb}" ]; then
    echo "warning: libxdo3 .deb not found after download" >&2
    return 0
  fi
  dpkg-deb -x "${deb}" "${tmp_dir}/root"
  # cp -a preserves the libxdo.so.3 -> libxdo.so.3.x.y symlink.
  find "${tmp_dir}/root" -name 'libxdo.so*' \( -type f -o -type l \) \
    -exec cp -a -t "${lib_dir}/" {} + 2>/dev/null || true
  echo "libxdo3: extracted to ${lib_dir}"
}

verify_linkage() {
  local path="$1"
  local name="${path##*/}"
  local ldd_out
  ldd_out="$(ldd "${path}" 2>&1 || true)"
  if printf '%s\n' "${ldd_out}" | grep -q 'not found'; then
    echo "ldd WARN : ${name} -> unresolved dependencies:"
    printf '%s\n' "${ldd_out}" | grep 'not found' | sed 's/^/           /'
    return 1
  fi
  echo "ldd OK   : ${name} (all shared libraries resolved)"
}

# Run a binary invocation and report its exit code without aborting on it.
# A WARN here is diagnostic only; ldd is the authoritative install check.
smoke() {
  local label="$1"
  shift
  local out rc
  if out="$("$@" 2>&1)"; then
    rc=0
  else
    rc=$?
  fi
  if [ "${rc}" -eq 0 ]; then
    echo "smoke OK   : ${label} (exit 0)"
  else
    echo "smoke WARN : ${label} (exit ${rc})"
  fi
  if [ -n "${out}" ]; then
    printf '             %s\n' "${out%%$'\n'*}"
  fi
}

install_deb wtype wtype
install_deb xdotool xdotool
install_deb ydotool ydotool

rescue_libxdo3

# If a private lib dir was populated, make it visible to ldd/smoke tests.
if [ -n "$(find "${lib_dir}" -maxdepth 1 -name 'lib*.so*' -print -quit 2>/dev/null)" ]; then
  export LD_LIBRARY_PATH="${lib_dir}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
fi

echo
echo "=== [A] Binary-install / link check (expected: PASS) ==="
link_warn=0
for binary in wtype xdotool ydotool; do
  if [ -x "${bin_dir}/${binary}" ]; then
    verify_linkage "${bin_dir}/${binary}" || link_warn=1
  else
    echo "ldd SKIP   : ${binary} (not installed in ${bin_dir})"
  fi
done

echo
echo "=== Smoke tests (binary invocations, NOT input delivery) ==="
if [ -x "${bin_dir}/xdotool" ]; then
  smoke "xdotool --version" "${bin_dir}/xdotool" --version
fi
if [ -x "${bin_dir}/wtype" ]; then
  smoke "wtype --version (best effort)" "${bin_dir}/wtype" --version
fi
if [ -x "${bin_dir}/ydotool" ]; then
  smoke "ydotool help" "${bin_dir}/ydotool" help
fi

echo
echo "=== Shell setup ==="
echo "  export PATH=\"${bin_dir}:\${PATH}\""
if [ -n "${LD_LIBRARY_PATH:-}" ]; then
  echo "  export LD_LIBRARY_PATH=\"${LD_LIBRARY_PATH}\""
fi

echo
echo "=== Probe guide — two different purposes (do not confuse them) ==="
echo
echo "[A] Binary-installed / link check — expected to PASS (install succeeded):"
echo "  xdotool --version        -> exit 0, 'xdotool version ...'"
echo "  wtype --version          -> this 0.4 build: exit 1 'Missing argument to --version'"
echo "                              (no version flag; ldd OK is the authoritative check)"
echo "  ydotool help             -> exit 0, command list"
echo "  ldd '${bin_dir}/xdotool' | grep 'not found'   -> expected: no output"
echo
echo "[B] Input-delivery check — expected: does NOT prove delivery to the Wayland app:"
echo "  xdotool getactivewindow  -> here: exit 0 + window ID (Xwayland query works),"
echo "                              BUT that only proves the X11/Xwayland side; XTEST"
echo "                              keys still never reach the Wayland-native gpui app"
echo "                              ('Control input devices' portal unapproved)"
echo "  wtype -k a               -> needs zwp_virtual_keyboard_v1; KWin does not support it"
echo "  ydotool key F1           -> needs /dev/uinput (root:root 0600, sudo-less here)"
echo "  => Failure here is EXPECTED. Use GPUI_FE_DEBUG_* hooks + Portal screenshots"
echo "     for local UI-state verification (details in app/README.md)."

cat <<'EOF'

NOTE: ydotool needs /dev/uinput access. This script never changes system
permissions and never runs root commands. To enable it manually (root, optional):
  install -d /etc/udev/rules.d
  printf '%s\n' 'KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess"' > /etc/udev/rules.d/99-uinput.rules
  udevadm control --reload
  udevadm trigger --subsystem-match=misc
Then log out/in to the Wayland session and re-run: ydotool key F1
EOF

if [ "${link_warn}" -ne 0 ]; then
  echo "ERROR: unresolved shared libraries remain (see ldd WARN above)." >&2
  exit 1
fi
