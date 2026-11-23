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
# app/Cargo.toml, core/src, core/Cargo.toml or the workspace-root Cargo.lock
# (i.e. files newer than the binary are detected with `find -newer`).
# --verify only compares an existing staging tree against the current sources
# (read-only); it never builds, copies or removes anything. It also compares
# the staged DEBIAN/control with the control metadata inside the existing .deb.
# --dry-run only resolves paths and prints the plan; no directory is created,
# nothing is deleted, copied or built.
# The workspace-profile output normally lands in the root target/; the explicit
# `--target-dir "${app_dir}/target"` on `cargo build` keeps the binary at the
# path consumed below (app/target/release/).
set -euo pipefail

usage() {
  cat <<'EOF'
使い方: packaging/linux/build-deb.sh [オプション]

オプション:
  -o, --output-dir DIR  出力先（既定: packaging/linux/build）
      --skip-build      release バイナリを再ビルドせず既存のものを使う
                        （app/src・build.rs・native-libs/・Cargo.toml・
                         core/src・core/Cargo.toml・ルート Cargo.lock より
                         古い場合は拒否し再ビルドを促す）
      --verify          既存ステージのドキュメント/.desktop/アイコンと
                         DEBIAN/control を現行ソース／既存 .deb の
                         メタデータと非破壊で照合
                         （ビルド・コピー・削除なし、出力先も作らない）
      --dry-run         解決済みパスと実行計画のみ表示
                         （削除・コピー・ビルドせず、出力先も作らない）
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
verify=0
dry_run=0

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
    --verify)
      verify=1
      ;;
    --dry-run)
      dry_run=1
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
# `cargo build` は workspace 化で既定の target/ がルートへ移ったため、
# 下のビルドで --target-dir "${app_dir}/target" を明示してここへ出力する。
binary="${app_dir}/target/release/${pkg}"

# Guard: never delete anything outside the chosen output directory. Normalize
# to an absolute path first so relative inputs (`.`, `..`) cannot bypass the
# check, and reject root/home/repo-root/its parents outright: the staging path
# would otherwise pass the prefix check while `rm -rf` targets a broad
# location.
if [ -z "${out_dir}" ]; then
  die "出力ディレクトリが空です"
fi
# Read-only modes must not create the output directory (AV23). A missing
# directory in --verify is reported as a missing stage below, and --dry-run
# resolves the path lexically without touching the filesystem.
if [ "${verify}" -eq 1 ] && [ ! -d "${out_dir}" ]; then
  die "ステージがありません: ${out_dir}/${pkg}_${deb_version}_${arch}（--verify は既存ステージの照合のみ行います）"
fi
if [ "${verify}" -eq 0 ] && [ "${dry_run}" -eq 0 ]; then
  mkdir -p -- "${out_dir}"
fi
if [ -d "${out_dir}" ]; then
  out_dir_abs="$(cd -- "${out_dir}" 2>/dev/null && pwd -P)"
elif command -v realpath >/dev/null 2>&1; then
  out_dir_abs="$(realpath -m -- "${out_dir}")"
else
  out_dir_abs="${out_dir}"
fi
[ -n "${out_dir_abs}" ] || die "出力ディレクトリを解決できません: ${out_dir}"
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

# Source → staged-path pairs shared by the build-time gate and --verify.
# The old gate only compared copies made in the same run, so a stale committed
# staging tree (.desktop / icons / NOTICE-app.md / ...) went unnoticed.
# .desktop and every hicolor icon are now covered too.
doc_pairs=(
  "${repo_root}/app/README.md|${stage}/usr/share/doc/${pkg}/README.md"
  "${repo_root}/packaging/linux/copyright|${stage}/usr/share/doc/${pkg}/copyright"
  "${repo_root}/LICENSE|${stage}/usr/share/doc/${pkg}/LICENSE"
  "${repo_root}/NOTICE.md|${stage}/usr/share/doc/${pkg}/NOTICE.md"
  "${repo_root}/app/NOTICE.md|${stage}/usr/share/doc/${pkg}/NOTICE-app.md"
  "${repo_root}/app/NOTICE.md|${stage}/usr/share/doc/${pkg}/app/NOTICE.md"
  "${repo_root}/packaging/linux/${pkg}.desktop|${stage}/usr/share/applications/${pkg}.desktop"
)
shopt -s nullglob
for icon in "${repo_root}/packaging/linux/icons/hicolor"/*/apps/*; do
  rel="${icon#"${repo_root}/packaging/linux/icons/"}"
  doc_pairs+=("${icon}|${stage}/usr/share/icons/${rel}")
done
shopt -u nullglob

verify_staged_docs() {
  [ -d "${stage}" ] ||
    die "ステージがありません: ${stage}（--verify は既存ステージの照合のみ行います）"
  local pair src installed mismatches=0
  for pair in "${doc_pairs[@]}"; do
    src="${pair%%|*}"
    installed="${pair#*|}"
    if [ ! -f "${installed}" ]; then
      echo "照合 NG (欠落): ${installed}" >&2
      mismatches=$((mismatches + 1))
    elif ! cmp -s -- "${src}" "${installed}"; then
      echo "照合 NG (不一致): ${installed} (source: ${src})" >&2
      mismatches=$((mismatches + 1))
    else
      echo "照合 OK: ${installed}"
    fi
  done
  if [ "${mismatches}" -ne 0 ]; then
    die "${mismatches} 件の不一致があります（ビルド・コピー・削除は行っていません）"
  fi
  echo "すべてのステージング済みファイルが現行ソースと一致しました: ${stage}"
}

# Also compare the staged control file with the metadata embedded in the
# existing .deb, so a committed package built from an older control goes
# unnoticed by the byte-for-byte source/stage check alone.
verify_control() {
  [ -f "${stage}/DEBIAN/control" ] ||
    die "ステージの DEBIAN/control がありません: ${stage}/DEBIAN/control"
  grep -qx "Package: ${pkg}" "${stage}/DEBIAN/control" ||
    die "control の Package が不正です: ${stage}/DEBIAN/control"
  grep -qx "Version: ${deb_version}" "${stage}/DEBIAN/control" ||
    die "control の Version が不正です: ${stage}/DEBIAN/control"
  grep -qx "Architecture: ${arch}" "${stage}/DEBIAN/control" ||
    die "control の Architecture が不正です: ${stage}/DEBIAN/control"
  [ -f "${deb}" ] ||
    die "既存 .deb がありません: ${deb}（--verify は既存 .deb との照合も行います）"
  if cmp -s -- "${stage}/DEBIAN/control" \
      <(dpkg-deb --ctrl-tarfile "${deb}" | tar -xO ./control 2>/dev/null); then
    echo "照合 OK: ${stage}/DEBIAN/control == ${deb} の control"
  else
    echo "照合 NG (不一致): ${stage}/DEBIAN/control vs ${deb} の control" >&2
    die "DEBIAN/control と既存 .deb のメタデータが一致しません"
  fi
}

# Dry-run: print the resolved paths and the exact plan, but never create a
# directory, delete a file, copy anything or invoke cargo/dpkg-deb.
if [ "${dry_run}" -eq 1 ]; then
  echo "ドライラン: 削除・コピー・ビルドは行いません"
  echo "  repo root : ${repo_root}"
  echo "  output dir: ${out_dir}"
  echo "  stage     : ${stage}"
  echo "  deb       : ${deb}"
  echo "  binary    : ${binary}"
  echo "  version   : ${deb_version} (arch: ${arch})"
  echo "実行予定:"
  echo "  1. ステージを削除: ${stage}"
  echo "  2. 既存 DEB を削除: ${deb}"
  if [ "${skip_build}" -eq 1 ]; then
    echo "  3. release ビルドはスキップし、既存バイナリのバージョン/鮮度を検証"
  else
    echo "  3. cargo build --release --locked → ${binary}"
  fi
  echo "  4. バイナリ/.desktop/アイコン/ドキュメントを ${stage} へ配置"
  echo "  5. ステージング済みファイルと現行ソースを照合"
  echo "  6. DEBIAN/control を生成して ${deb} をビルド"
  echo "  7. --verify でステージと .deb のメタデータを照合"
  printf '  照合対象 (%d 件):\n' "${#doc_pairs[@]}"
  for pair in "${doc_pairs[@]}"; do
    echo "    ${pair%%|*} -> ${pair#*|}"
  done
  exit 0
fi

# Read-only mode: check an existing staging tree and stop before any destructive
# step (stage removal, .deb removal, cargo build, dpkg-deb).
if [ "${verify}" -eq 1 ]; then
  verify_staged_docs
  verify_control
  exit 0
fi

rm -rf -- "${stage}"
# AU07: a failed build must not leave a previous .deb at the documented path.
# ${deb} is ${stage}.deb inside the already-validated output directory.
case "${deb}" in
  "${out_dir}"/*.deb) rm -f -- "${deb}" ;;
  *) die "安全でない出力先: ${deb}" ;;
esac
mkdir -p "${stage}/DEBIAN" "${stage}/usr/bin" "${stage}/usr/share/applications"

if [ "${skip_build}" -eq 0 ]; then
  echo "release ビルド中…"
  cargo build --release --locked --manifest-path "${app_dir}/Cargo.toml" \
    --target-dir "${app_dir}/target"
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
# package it. core/src and core/Cargo.toml must be included: since the core
# extraction they are linked into the binary just as much as app/src is.
if [ "${skip_build}" -eq 1 ]; then
  # app/Cargo.lock は workspace 化前の残骸で未使用（正本はルート Cargo.lock）。
  # 指示により削除はしないが、個別ビルド等で更新されていたら警告する。
  if [ -f "${app_dir}/Cargo.lock" ] && [ "${app_dir}/Cargo.lock" -nt "${repo_root}/Cargo.lock" ]; then
    echo "警告: ${app_dir}/Cargo.lock は workspace では未使用です（正本: ${repo_root}/Cargo.lock）。" >&2
  fi
  # AU06: core/src and core/Cargo.toml are part of the linked binary as well;
  # omitting them let a core-only change ship with an old binary under
  # --skip-build. Only pass existing paths so `find` does not abort early.
  freshness_paths=()
  for path in \
    "${app_dir}/src" \
    "${app_dir}/build.rs" \
    "${app_dir}/native-libs" \
    "${app_dir}/Cargo.toml" \
    "${repo_root}/Cargo.lock" \
    "${repo_root}/core/src" \
    "${repo_root}/core/Cargo.toml"; do
    if [ -e "${path}" ]; then
      freshness_paths+=("${path}")
    fi
  done
  [ "${#freshness_paths[@]}" -gt 0 ] || die "鮮度チェック対象が見つかりません"
  # `-print -quit` stops at the first match; `|| true` keeps `set -e` from
  # aborting on a transient find error before our own error message is shown.
  stale_source="$(find "${freshness_paths[@]}" \
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

# Gate: every staged file in doc_pairs (docs, .desktop and hicolor icons) must
# match the current repository source byte-for-byte. Earlier packages under
# packaging/linux/build/ shipped a NOTICE-app.md without the tracing /
# tracing-subscriber entries and no LICENSE. Here the files were just copied
# from those sources, so a mismatch means the packaging layout regressed.
# To check an already-existing staging tree use `--verify`, which never builds,
# copies or deletes. Abort before dpkg-deb; a stale .deb was already removed at
# the start of this run, so a failure here leaves no package at the README path.
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
# AU14: under `set -o pipefail`, `head` may close the pipe early and make this
# pipeline fail with SIGPIPE even though dpkg-deb succeeded.
dpkg-deb --contents "${deb}" | sed -n '1,20p'
echo
echo "展開して --version を確認する例:"
echo "  dpkg-deb -x '${deb}' /tmp/gpui-fe-deb && /tmp/gpui-fe-deb/usr/bin/${pkg} --version"
