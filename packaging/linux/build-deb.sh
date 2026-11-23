#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Build a Debian package for gpui-file-explorer without root privileges.
#
# The package layout follows the usual FHS locations:
#   /usr/bin/gpui-file-explorer
#   /usr/share/applications/gpui-file-explorer.desktop
#   /usr/share/icons/hicolor/<size>/apps/gpui-file-explorer.png
#   /usr/share/doc/gpui-file-explorer/{README.md,copyright,LICENSE,NOTICE.md,NOTICE-app.md,app/NOTICE.md}
#
# `dpkg-deb --root-owner-group` sets root ownership inside the archive, so the
# script must run as a normal user. Nothing is installed on the host.
#
# Runtime dependencies are derived from `ldd` + `dpkg -S`, with `libvulkan1`
# added explicitly because wgpu loads the Vulkan loader with dlopen.
#
# --skip-build reuses the existing release binary. To avoid shipping a stale
# binary that still reports the same version, the script additionally rejects a
# binary that is older than any of app/src, app/build.rs, app/native-libs/,
# app/Cargo.toml or app/Cargo.lock (i.e. files newer than the binary are
# detected with `find -newer`).
set -euo pipefail

usage() {
  cat <<'EOF'
使い方: packaging/linux/build-deb.sh [オプション]

オプション:
  -o, --output-dir DIR  出力先（既定: packaging/linux/build）
      --skip-build      release バイナリを再ビルドせず既存のものを使う
                        （app/src・build.rs・native-libs/・Cargo.toml・Cargo.lock より古い場合は拒否し再ビルドを促す）
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

# Guard: never delete anything outside the chosen output directory. Normalize
# to an absolute path first so relative inputs (`.`, `..`) cannot bypass the
# check, and reject root/home/repo-root/its parents outright: the staging path
# would otherwise pass the prefix check while `rm -rf` targets a broad
# location.
if [ -z "${out_dir}" ]; then
  die "出力ディレクトリが空です"
fi
mkdir -p -- "${out_dir}"
if ! out_dir_abs="$(cd -- "${out_dir}" 2>/dev/null && pwd -P)"; then
  die "出力ディレクトリを解決できません: ${out_dir}"
fi
repo_root_abs="$(cd -- "${repo_root}" && pwd -P)"
parent_of_repo="$(dirname -- "${repo_root_abs}")"
case "${out_dir_abs}" in
  "/" | "${repo_root_abs}" | "${parent_of_repo}" | "${HOME}" | "${HOME}/")
    die "安全でない出力ディレクトリ: ${out_dir}" ;;
esac
out_dir="${out_dir_abs}"
stage="${out_dir}/${pkg}_${deb_version}_${arch}"
deb="${stage}.deb"
case "${stage}" in
  "${out_dir}"/*) ;;
  *) die "安全でないステージングパス: ${stage}" ;;
esac

rm -rf -- "${stage}"
mkdir -p "${stage}/DEBIAN" "${stage}/usr/bin" "${stage}/usr/share/applications"

if [ "${skip_build}" -eq 0 ]; then
  echo "release ビルド中…"
  cargo build --release --locked --manifest-path "${app_dir}/Cargo.toml"
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

# With --skip-build the binary is not rebuilt, so the version check above alone
# cannot catch a stale binary from an earlier build of the same version. Detect
# any source/build/native-lib/Cargo file newer than the binary and refuse to
# package it: build.rs links native-libs/, so both affect the linked binary.
if [ "${skip_build}" -eq 1 ]; then
  # `-print -quit` stops at the first match; `|| true` keeps `set -e` from
  # aborting on a missing search path before our own error message is shown.
  stale_source="$(find "${app_dir}/src" "${app_dir}/build.rs" "${app_dir}/native-libs" \
    "${app_dir}/Cargo.toml" "${app_dir}/Cargo.lock" \
    -newer "${binary}" -print -quit || true)"
  if [ -n "${stale_source}" ]; then
    die "release バイナリがソースより古いです: ${stale_source}（--skip-build を外して再ビルドしてください）"
  fi
fi

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
install -Dm644 "${repo_root}/packaging/linux/copyright" "${doc_dir}/copyright"
install -Dm644 "${repo_root}/LICENSE" "${doc_dir}/LICENSE"
install -Dm644 "${repo_root}/NOTICE.md" "${doc_dir}/NOTICE.md"
install -Dm644 "${repo_root}/app/NOTICE.md" "${doc_dir}/NOTICE-app.md"
# NOTICE.md links to `app/NOTICE.md`; install the same file at that relative
# path too so the link resolves inside the package. The repository-side link
# in NOTICE.md is left unchanged.
install -Dm644 "${repo_root}/app/NOTICE.md" "${doc_dir}/app/NOTICE.md"

# Gate: every staged doc file must match the current repository source
# byte-for-byte. Earlier packages under packaging/linux/build/ shipped a
# NOTICE-app.md without the tracing / tracing-subscriber entries and no
# LICENSE; this script copies the current sources, so a mismatch here means
# the packaging layout regressed (or a stale staging tree was reused).
# Abort before dpkg-deb; replacing an existing .deb is the caller's decision.
doc_pairs=(
  "${repo_root}/app/README.md|${doc_dir}/README.md"
  "${repo_root}/packaging/linux/copyright|${doc_dir}/copyright"
  "${repo_root}/LICENSE|${doc_dir}/LICENSE"
  "${repo_root}/NOTICE.md|${doc_dir}/NOTICE.md"
  "${repo_root}/app/NOTICE.md|${doc_dir}/NOTICE-app.md"
  "${repo_root}/app/NOTICE.md|${doc_dir}/app/NOTICE.md"
)
for pair in "${doc_pairs[@]}"; do
  src="${pair%%|*}"
  installed="${pair#*|}"
  cmp -s -- "${src}" "${installed}" ||
    die "doc が現行ソースと一致しません: ${installed} (source: ${src})"
done

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
