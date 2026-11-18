#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# gen-perf-fixture.sh - create an empty-file directory fixture for measuring
# the cost of model::read_entries (see DESIGN.md section 10.5).
#
# Safety contract: the script only ever writes inside an explicitly provided
# temp root. OUTPUT_DIR must resolve to a strict descendant of --tmp-root, and
# the root itself may not be "/" . Cleanup is marker-gated: an existing
# directory is removed only when it carries our marker file.

set -euo pipefail

readonly MARKER=".perf-fixture-marker"
readonly NAME_PREFIX="entry_"

usage() {
    cat <<'EOF'
Usage:
  gen-perf-fixture.sh [--tmp-root DIR] [--cleanup] [COUNT [OUTPUT_DIR]]

Arguments:
  COUNT       Number of empty files (default: 1000, max: 2000000).
  OUTPUT_DIR  Destination directory (default: TMP_ROOT/gpui-fe-perf-COUNT).
              Must be a strict descendant of --tmp-root.

Options:
  --tmp-root DIR  Required. User-provided temp directory that bounds all
                  writes (e.g. /tmp or the result of mktemp -d).
  --cleanup       Remove OUTPUT_DIR instead of creating it. Only removes a
                  directory that contains our marker file.
  -h, --help      Show this help.
EOF
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

# Canonicalise a path that may not exist yet, without requiring realpath(1).
canonicalise() {
    local path=$1
    local parent base resolved
    if [[ -d $path ]]; then
        (cd -- "$path" && pwd -P)
        return
    fi
    parent=$(dirname -- "$path")
    base=$(basename -- "$path")
    [[ -d $parent ]] || die "parent directory does not exist: $parent"
    resolved=$(cd -- "$parent" && pwd -P)
    if [[ $base == / || $base == . || $base == .. ]]; then
        printf '%s\n' "$resolved"
    else
        printf '%s/%s\n' "$resolved" "$base"
    fi
}

count=1000
count_set=0
out_dir=""
cleanup=0
tmp_root=""

while (($# > 0)); do
    case $1 in
        --tmp-root)
            [[ $# -ge 2 ]] || die "--tmp-root needs a value"
            tmp_root=$2
            shift 2
            ;;
        --cleanup)
            cleanup=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            ;;
        -*)
            die "unknown option: $1"
            ;;
        *)
            if [[ $count_set -eq 0 ]]; then
                count=$1
                count_set=1
            elif [[ -z $out_dir ]]; then
                out_dir=$1
            else
                die "too many positional arguments"
            fi
            shift
            ;;
    esac
done

[[ -n $tmp_root ]] || die "--tmp-root is required (refusing to guess a write location)"
[[ $tmp_root != / ]] || die "--tmp-root must not be /"
[[ -d $tmp_root ]] || die "temp root is not a directory: $tmp_root"
[[ -w $tmp_root ]] || die "temp root is not writable: $tmp_root"

[[ $count =~ ^[0-9]+$ ]] || die "COUNT must be a decimal integer: $count"
[[ $count -ge 1 ]] || die "COUNT must be at least 1"
[[ $count -le 2000000 ]] || die "COUNT above safety cap 2000000: $count"

tmp_root_abs=$(canonicalise "$tmp_root")
if [[ -z $out_dir ]]; then
    out_dir="$tmp_root_abs/gpui-fe-perf-$count"
fi
out_dir_abs=$(canonicalise "$out_dir")

case $out_dir_abs in
    "$tmp_root_abs"/*) ;;
    *) die "OUTPUT_DIR must be strictly inside --tmp-root: $out_dir_abs" ;;
esac

if [[ $cleanup -eq 1 ]]; then
    [[ -e $out_dir_abs ]] || die "nothing to clean: $out_dir_abs"
    [[ -f $out_dir_abs/$MARKER ]] || die "refusing to remove a directory without our marker: $out_dir_abs"
    [[ $(basename -- "$out_dir_abs") == gpui-fe-perf-* ]] || \
        die "refusing to remove a directory with an unexpected name: $out_dir_abs"
    rm -rf -- "$out_dir_abs"
    printf 'removed fixture: %s\n' "$out_dir_abs"
    exit 0
fi

# Clean up a partially created fixture if generation fails midway.
partial=0
cleanup_partial() {
    if [[ $partial -eq 1 && -f $out_dir_abs/$MARKER ]]; then
        rm -rf -- "$out_dir_abs"
    fi
}
trap cleanup_partial ERR

mkdir -p -- "$out_dir_abs"
partial=1

{
    printf 'fixture=gpui-file-explorer/read_entries\n'
    printf 'count=%s\n' "$count"
    printf 'created_at=%s\n' "$(date +%Y-%m-%dT%H:%M:%S%:z)"
} > "$out_dir_abs/$MARKER"

(
    cd -- "$out_dir_abs"
    seq -f "${NAME_PREFIX}%06g" 1 "$count" | xargs -r -d '\n' -n 500 touch --
)

actual=$(find "$out_dir_abs" -maxdepth 1 -type f -name "${NAME_PREFIX}*" | wc -l)
[[ $actual -eq $count ]] || die "expected $count files, found $actual"

trap - ERR
printf 'created %s empty files in %s\n' "$actual" "$out_dir_abs"
