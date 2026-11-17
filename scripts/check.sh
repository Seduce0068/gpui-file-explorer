#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Runs the local verification loop for this repository.
#
#   app: fmt -> check -> test -> clippy -> (optional) cargo audit / cargo deny / cargo vet
#   poc: fmt -> check -> test -> clippy -> (optional) cargo deny (advisories/bans/sources)
#
# The repository has no root Cargo.toml: `app/` and `poc/` are independent
# Cargo projects. The shared CI workflow (Seduce0068/workflows rust-gates@v1)
# covers `app/` and runs the `poc/` half through `extra_gates`; this script
# reproduces both halves locally.
#
# Supply-chain gates:
#   - cargo audit: RustSec advisory check for app/ and poc/
#   - cargo deny check: full check for app/; for poc/ the licenses check is
#     omitted because its Cargo.toml has no license field (out of scope here)
#     - see deny.toml
#   - cargo vet check: only when <project>/supply-chain/config.toml exists
#     (run `cargo vet init` in that project to opt in)
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

usage() {
	cat <<'EOF'
Runs the local verification loop for this repository.

  app: fmt -> check -> test -> clippy -> (optional) cargo audit / cargo deny / cargo vet
  poc: fmt -> check -> test -> clippy -> (optional) cargo deny (advisories/bans/sources)

The repository has no root Cargo.toml: `app/` and `poc/` are independent
Cargo projects. The shared CI workflow (Seduce0068/workflows rust-gates@v1)
covers `app/` and runs the `poc/` half through `extra_gates`; this script
reproduces both halves locally.

Supply-chain gates:
  - cargo audit: RustSec advisory check for app/ and poc/
  - cargo deny check: full check for app/; for poc/ the licenses check is
    omitted because its Cargo.toml has no license field (out of scope here)
    - see deny.toml
  - cargo vet check: only when <project>/supply-chain/config.toml exists
    (run `cargo vet init` in that project to opt in)

Usage:
  bash scripts/check.sh            # full loop (app + poc + supply-chain)
  bash scripts/check.sh --fast     # skip audit/vet and deny advisories;
                                   # deny still runs offline for licenses/bans/sources
  bash scripts/check.sh --app      # app/ only
  bash scripts/check.sh --poc      # poc/ only
EOF
}

run_app=true
run_poc=true
run_audit=true
run_deny=true
run_vet=true
fast_mode=false
selected_scope=false

for argument in "$@"; do
	case "${argument}" in
	--fast)
		# audit / vet / deny advisories need the RustSec DB or imported audits.
		# Keep the offline portion of deny (licenses / bans / sources) enabled.
		run_audit=false
		run_vet=false
		fast_mode=true
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
		usage
		exit 0
		;;
	*)
		echo "error: unknown argument: ${argument}" >&2
		exit 2
		;;
	esac
done

run_audit_check() {
	local project_dir="$1"
	if ! command -v cargo-audit >/dev/null 2>&1; then
		echo "cargo-audit is not installed; skipping the advisory check (CI runs it)."
		return
	fi
	echo "==> ${project_dir}: cargo audit"
	(cd "${project_dir}" && cargo audit)
}

run_deny_check() {
	local project_dir="$1"
	local project="$2"
	if ! command -v cargo-deny >/dev/null 2>&1; then
		echo "cargo-deny is not installed; skipping the deny check (CI runs it)."
		return
	fi

	if [ "${fast_mode}" = true ]; then
		if [ "${project}" = "app" ]; then
			echo "==> ${project_dir}: cargo deny check (offline: licenses/bans/sources)"
			(cd "${project_dir}" && cargo deny --offline check licenses bans sources)
		else
			echo "==> ${project_dir}: cargo deny check (offline: bans/sources)"
			(cd "${project_dir}" && cargo deny --offline check bans sources)
		fi
		return
	fi

	if [ "${project}" = "app" ]; then
		echo "==> ${project_dir}: cargo deny check"
		(cd "${project_dir}" && cargo deny check)
	else
		echo "==> ${project_dir}: cargo deny check (advisories/bans/sources; licenses skipped, see deny.toml)"
		(cd "${project_dir}" && cargo deny check advisories bans sources)
	fi
}

run_vet_check() {
	local project_dir="$1"
	if ! command -v cargo-vet >/dev/null 2>&1; then
		echo "cargo-vet is not installed; skipping."
		return
	fi
	if [ ! -f "${project_dir}/supply-chain/config.toml" ]; then
		echo "==> ${project_dir}: cargo vet skipped (no supply-chain/config.toml; run 'cargo vet init' there to enable)."
		return
	fi
	echo "==> ${project_dir}: cargo vet check"
	(cd "${project_dir}" && cargo vet check)
}

if [ "${run_app}" = true ]; then
	echo '==> app: cargo fmt --all -- --check'
	(cd app && cargo fmt --all -- --check)

	echo '==> app: cargo check --all-targets --locked'
	(cd app && cargo check --all-targets --locked)

	echo '==> app: cargo test --all-targets --locked'
	(cd app && cargo test --all-targets --locked)

	echo '==> app: cargo clippy --all-targets --locked -- -D warnings'
	(cd app && cargo clippy --all-targets --locked -- -D warnings)

	if [ "${run_audit}" = true ]; then
		run_audit_check app
	fi
	if [ "${run_deny}" = true ]; then
		run_deny_check app app
	fi
	if [ "${run_vet}" = true ]; then
		run_vet_check app
	fi
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

	if [ "${run_audit}" = true ]; then
		run_audit_check poc
	fi
	if [ "${run_deny}" = true ]; then
		run_deny_check poc poc
	fi
	if [ "${run_vet}" = true ]; then
		run_vet_check poc
	fi
fi

echo 'All checks passed.'
