#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Build a Debian package for gpui-file-explorer without root privileges.
#
# The package layout follows the usual FHS locations:
#   /usr/bin/gpui-file-explorer
#   /usr/share/applications/gpui-file-explorer.desktop
#   /usr/share/icons/hicolor/<size>/apps/gpui-file-explorer.png
#   /usr/share/doc/gpui-file-explorer/{README.md,copyright,NOTICE.md,NOTICE-app.md}
#
# `dpkg-deb --root-owner-group` sets root ownership inside the archive, so the
# script must run as a normal user. Nothing is installed on the host.
#
# Runtime dependencies are derived from `ldd` + `dpkg -S`, with `libvulkan1`
# added explicitly because wgpu loads the Vulkan loader with dlopen.
set -euo pipefail

usage() {
  cat <<'EOF'
使い方: packaging/linux/build-deb.sh [オプション]

オプション:
  -o, --output-dir DIR  出力先（既定: packaging/linux/build）
      --skip-build      release バイナリを再ビルドせず既存のものを使う
  -h, --help            このヘルプを表示

root では実行できません（dpkg-deb --root-owner-group が所有権を設定します）。
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
app_dir="${repo_root}/app"
pkg="gpui-file-explorer"
out_dir="${repo_root}/packaging/linux/build"
skip_build=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    -o | --output-dir)
      [ "$#" -ge 2 ] || die "$1 には引数が必要です"
      out_dir="$2"
      shift
      ;;
    --skip-build)
      skip_build=1
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      die "不明なオプション: $1"
      ;;
  esac
  shift
done

[ "$(id -u)" -ne 0 ] || die "root では実行しないでください（通常ユーザーで実行してください）"
command -v dpkg-deb >/dev/null 2>&1 || die "dpkg-deb が見つかりません（Debian/Ubuntu 系が必要です）"
command -v dpkg >/dev/null 2>&1 || die "dpkg が見つかりません"

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${app_dir}/Cargo.toml" | head -n1)"
[ -n "${version}" ] || die "app/Cargo.toml からバージョンを取得できません"
arch="$(dpkg --print-architecture)"
deb_version="${version}-1"
stage="${out_dir}/${pkg}_${deb_version}_${arch}"
deb="${stage}.deb"
binary="${app_dir}/target/release/${pkg}"

# Guard: never delete anything outside the chosen output directory.
case "${stage}" in
  "${out_dir}"/*) ;;
  *) die "安全でないステージングパス: ${stage}" ;;
esac

mkdir -p "${out_dir}"
rm -rf -- "${stage}"
mkdir -p "${stage}/DEBIAN" "${stage}/usr/bin" "${stage}/usr/share/applications"

if [ "${skip_build}" -eq 0 ]; then
  echo "release ビルド中…"
  cargo build --release --manifest-path "${app_dir}/Cargo.toml"
fi
[ -x "${binary}" ] || die "release バイナリがありません: ${binary}（--skip-build を外して再実行してください）"

# Verify the existing binary actually matches this source version before it is
# packaged. Without this, --skip-build can silently ship a stale binary.
if ! binary_version="$("${binary}" --version 2>/dev/null)"; then
  die "release バイナリの --version が失敗しました: ${binary}"
fi
case "${binary_version}" in
  "${pkg} ${version}") ;;
  *) die "release バイナリのバージョン不一致: ${binary_version}（expected: ${pkg} ${version}）" ;;
esac

echo "パッケージを構成中…"
install -Dm755 "${binary}" "${stage}/usr/bin/${pkg}"
install -Dm644 "${repo_root}/packaging/linux/${pkg}.desktop" \
  "${stage}/usr/share/applications/${pkg}.desktop"

shopt -s nullglob
for icon in "${repo_root}/packaging/linux/icons/hicolor"/*/apps/*; do
  rel="${icon#"${repo_root}/packaging/linux/icons/"}"
  install -Dm644 "${icon}" "${stage}/usr/share/icons/${rel}"
done
shopt -u nullglob

doc_dir="${stage}/usr/share/doc/${pkg}"
install -Dm644 "${repo_root}/app/README.md" "${doc_dir}/README.md"
install -Dm644 "${repo_root}/LICENSE" "${doc_dir}/copyright"
install -Dm644 "${repo_root}/NOTICE.md" "${doc_dir}/NOTICE.md"
install -Dm644 "${repo_root}/app/NOTICE.md" "${doc_dir}/NOTICE-app.md"

# Runtime dependencies from the ELF headers plus the dlopen'ed Vulkan loader.
compute_depends() {
  local elf="$1" lib pkg_name
  local -a found=()
  while IFS= read -r lib; do
    [ -n "${lib}" ] || continue
    # Only absolute paths from the dynamic loader are plausible library names.
    # Reject relative paths, symlinks with spaces already split, and anything
    # outside the usual library directories before asking dpkg about it.
    case "${lib}" in
      /lib/*|/usr/lib/*|/lib64/*|/usr/lib64/*) ;;
      *) continue ;;
    esac
    [ -e "${lib}" ] || continue
    # `dpkg -S` can print localized diversion lines first (e.g. for
    # ld-linux); only accept lines that start with a package name.
    pkg_name="$(dpkg -S "${lib}" 2>/dev/null \
      | grep -E '^[a-z0-9][a-z0-9+.-]*(:[a-z0-9-]+)?: ' \
      | head -n1 | cut -d: -f1 || true)"
    case "${pkg_name}" in
      ''|*[!a-z0-9+.-]*) continue ;;
    esac
    found+=("${pkg_name}")
  done < <(ldd "${elf}" 2>/dev/null | awk '/=> \// {print $3} /^\t\/.*\.so/ {print $1}')
  found+=("libvulkan1")
  printf '%s\n' "${found[@]}" | sort -u | paste -sd, - | sed 's/,/, /g'
}

depends="$(compute_depends "${binary}")"
[ -n "${depends}" ] || die "依存パッケージを決定できませんでした"
installed_size="$(du -sk "${stage}" | cut -f1)"

cat >"${stage}/DEBIAN/control" <<EOF
Package: ${pkg}
Version: ${deb_version}
Section: utils
Priority: optional
Architecture: ${arch}
Maintainer: koppe <keitaonodera0903@gmail.com>
Installed-Size: ${installed_size}
Depends: ${depends}
Recommends: libfontconfig1
Homepage: https://github.com/Seduce0068/gpui-file-explorer
Description: GPUI-based cross-platform file explorer
 A fast file explorer prototype built with GPUI (Zed's UI framework)
 using gpui-kit. Linux/Wayland-first, with asynchronous directory loading,
 file watching, safe cut/copy/paste and inline rename.
EOF

echo "DEB を作成中…"
dpkg-deb --root-owner-group --build "${stage}" "${deb}" >/dev/null

echo
echo "作成しました: ${deb}"
dpkg-deb --info "${deb}"
echo
echo "内容（先頭 20 行）:"
dpkg-deb --contents "${deb}" | head -n 20
echo
echo "展開して --version を確認する例:"
echo "  dpkg-deb -x '${deb}' /tmp/gpui-fe-deb && /tmp/gpui-fe-deb/usr/bin/${pkg} --version"
