#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Runs the local verification loop for this repository.
#
#   app: fmt -> check -> test -> clippy -> (optional) cargo audit
#   poc: fmt -> check -> test -> clippy
#
# The repository has no root Cargo.toml: `app/` and `poc/` are independent
# Cargo projects. The shared CI workflow (Seduce0068/workflows rust-gates@v1)
# covers `app/` and runs the `poc/` half through `extra_gates`; this script
# reproduces both halves locally.
#
# Usage:
#   bash scripts/check.sh            # full loop (app + poc)
#   bash scripts/check.sh --fast     # skip `cargo audit` (no network / RustSec DB)
#   bash scripts/check.sh --app      # app/ only
#   bash scripts/check.sh --poc      # poc/ only
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

run_app=true
run_poc=true
run_audit=true
selected_scope=false

for argument in "$@"; do
  case "${argument}" in
    --fast)
      # cargo audit needs the RustSec database; skip it when offline.
      run_audit=false
      ;;
    --app)
      if [ "${selected_scope}" = true ]; then
        echo "error: --app and --poc are mutually exclusive" >&2
        exit 2
      fi
      selected_scope=true
      run_poc=false
      ;;
    --poc)
      if [ "${selected_scope}" = true ]; then
        echo "error: --app and --poc are mutually exclusive" >&2
        exit 2
      fi
      selected_scope=true
      run_app=false
      ;;
    -h | --help)
      sed -n '3,18p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "error: unknown argument: ${argument}" >&2
      exit 2
      ;;
  esac
done

if [ "${run_app}" = true ]; then
  echo '==> app: cargo fmt --all -- --check'
  (cd app && cargo fmt --all -- --check)

  echo '==> app: cargo check --all-targets --locked'
  (cd app && cargo check --all-targets --locked)

  echo '==> app: cargo test --all-targets --locked'
  (cd app && cargo test --all-targets --locked)

  echo '==> app: cargo clippy --all-targets --locked -- -D warnings'
  (cd app && cargo clippy --all-targets --locked -- -D warnings)
fi

if [ "${run_poc}" = true ]; then
  echo '==> poc: cargo fmt --all -- --check'
  (cd poc && cargo fmt --all -- --check)

  echo '==> poc: cargo check --all-targets --locked'
  (cd poc && cargo check --all-targets --locked)

  echo '==> poc: cargo test --all-targets --locked'
  (cd poc && cargo test --all-targets --locked)

  echo '==> poc: cargo clippy --all-targets --locked -- -D warnings'
  (cd poc && cargo clippy --all-targets --locked -- -D warnings)
fi

if [ "${run_app}" = true ] && [ "${run_audit}" = true ]; then
  if command -v cargo-audit >/dev/null 2>&1; then
    echo '==> app: cargo audit'
    (cd app && cargo audit)
  else
    echo 'cargo-audit is not installed; skipping the advisory check (CI runs it).'
  fi
fi

echo 'All checks passed.'
