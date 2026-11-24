#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Runs the local verification loop for this repository.
#
#   app: fmt -> check -> test -> clippy -> release(check/test) -> (optional) cargo audit / cargo deny / cargo vet
#   poc: fmt -> check -> test -> clippy -> (optional) cargo deny
#   pwa: node --test pwa/main.test.js (SKIPPED when node:test is unavailable;
#        the pure-helper subset still runs without the gitignored pwa/pkg/)
#   deb: packaging/linux/build-deb.sh --verify (read-only stage/.deb check;
#        SKIPPED when no built .deb is present)
#   android: python3 stdlib static gate (AndroidManifest.xml structure + pinned
#        Gradle versions incl. settings.gradle.kts and Compose compiler; SKIPPED
#        when the scaffold is absent)
#
# The repository is a virtual Cargo workspace with members core, app, poc
# and win32 (root Cargo.toml). The shared CI workflow
# (Seduce0068/workflows rust-gates@v1) covers `app/` and runs the other
# members through `extra_gates`; this script reproduces those gates locally.
# Cross-target checks (core/wasm32-unknown-unknown, win32/x86_64-pc-windows-gnu)
# are `cargo check` only (no linker) and require the rustup targets installed.
#
# Release verification (app/ only):
#   Debug builds always enable `debug_assertions`, so `#[cfg(not(debug_assertions))]`
#   branches are never compiled by the debug steps above. The full loop therefore
#   also runs check/test with `--release` for app/. Those two steps already compile
#   the non-debug_assertions branches. (clippy --release -D warnings is omitted:
#   it currently fails on pre-existing release-only warnings in app/src/main.rs.)
#   Pass --no-release (or --fast, which implies it) to skip this separate build.
#
# Supply-chain gates:
#   - cargo audit: RustSec advisory check for app/ and poc/
#   - cargo deny check: full check for app/ and poc/ (both declare license)
#     - see deny.toml
#   - cargo vet check: only when <project>/supply-chain/config.toml exists
#     (run `cargo vet init` in that project to opt in)
#
# Missing tools (cargo audit/deny/vet, node/node:test, python3,
# dpkg-deb/dpkg/realpath/tar) and build artifacts (pwa/pkg/, built .deb,
# android/ scaffold):
#   - default: reported as `SKIPPED: ...` before the final line
#   - --strict: the same condition is a hard failure (non-zero exit status)
# Artifact availability is checked at the FRONT of the gate list so a clean
# checkout is not surprised by a .deb failure only after every code gate ran.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

usage() {
	cat <<'EOF'
Runs the local verification loop for this repository.

  app: fmt -> check -> test -> clippy -> release(check/test) -> (optional) cargo audit / cargo deny / cargo vet
  poc: fmt -> check -> test -> clippy -> (optional) cargo deny
  pwa: node --test pwa/main.test.js (pure helpers + service worker registration)
  deb:  packaging/linux/build-deb.sh --verify (read-only; needs an existing stage/.deb)
  android: python3 stdlib static gate for the tracked scaffold (no build)

The repository is a virtual Cargo workspace with members core, app, poc
and win32 (root Cargo.toml). The shared CI workflow
(Seduce0068/workflows rust-gates@v1) covers `app/` and runs the other
members through `extra_gates`; this script reproduces those gates locally.
Cross-target checks: core (wasm32-unknown-unknown) and win32
(x86_64-pc-windows-gnu) via `cargo check`; install with `rustup target add`.

Release verification (app/ only):
  Compiles `#[cfg(not(debug_assertions))]` branches that debug builds never see.
  Enabled by default; disabled by --no-release (and implied by --fast).

Supply-chain gates:
  - cargo audit: RustSec advisory check for app/ and poc/
  - cargo deny check: full check for app/ and poc/ (both declare a license)
    - see deny.toml
  - cargo vet check: only when <project>/supply-chain/config.toml exists
    (run `cargo vet init` in that project to opt in)

Additional artifact gates (independent of --app/--poc; SKIPPED with a notice
when their gitignored build inputs are absent, hard failure under --strict):
  - pwa: node --test pwa/main.test.js (pure helpers run even without pwa/pkg/)
  - deb: packaging/linux/build-deb.sh --verify (needs a built .deb)
  - android: python3 stdlib static gate (tracked scaffold; no build)

Usage:
  bash scripts/check.sh             # full loop (app + release + poc + supply-chain)
  bash scripts/check.sh --fast      # skip audit/vet and deny advisories; implies
                                    # --no-release; deny still runs offline for
                                    # licenses/bans/sources
  bash scripts/check.sh --app       # app/ + core/win32 + cross-target checks
                                    # (includes release verification)
  bash scripts/check.sh --poc       # poc/ + core/win32 + cross-target checks (app/ omitted)
  bash scripts/check.sh --no-release # skip the app/ release verification
  bash scripts/check.sh --strict    # fail if a needed tool (incl. node:test /
                                    # python3, dpkg-deb/dpkg/realpath/tar) or
                                    # artifact (.deb, android/ scaffold) is missing
EOF
}

run_app=true
run_poc=true
run_audit=true
run_deny=true
run_vet=true
run_release=true
fast_mode=false
strict_mode=false
selected_scope=false
skipped_tools=()
skipped_artifacts=()
strict_failures=()

for argument in "$@"; do
	case "${argument}" in
	--fast)
		# audit / vet / deny advisories need the RustSec DB or imported audits.
		# A release build is a separate, expensive compilation, so --fast skips
		# release verification as well (documented as implied --no-release).
		run_audit=false
		run_vet=false
		run_release=false
		fast_mode=true
		;;
	--no-release)
		# Explicitly skip the app/ release verification.
		run_release=false
		;;
	--strict)
		# A missing cargo audit/deny/vet/node/dpkg-deb becomes a hard failure.
		strict_mode=true
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

# Record a missing supply-chain tool once and, in --strict mode, remember it
# so the final status can fail instead of claiming that everything passed.
record_missing_tool() {
	local tool="$1"
	local seen_tool
	if [ "${#skipped_tools[@]}" -gt 0 ]; then
		for seen_tool in "${skipped_tools[@]}"; do
			if [ "${seen_tool}" = "${tool}" ]; then
				return 0
			fi
		done
	fi
	skipped_tools+=("${tool}")
	if [ "${strict_mode}" = true ]; then
		strict_failures+=("${tool}")
	fi
}

# Record a missing build artifact once (gitignored build output absent in a
# clean checkout). Reported as SKIPPED by default and as a hard failure under
# --strict, exactly like a missing tool, but tracked separately so the summary
# can say "not present" instead of "not installed".
record_missing_artifact() {
	local artifact="$1"
	local seen_artifact
	if [ "${#skipped_artifacts[@]}" -gt 0 ]; then
		for seen_artifact in "${skipped_artifacts[@]}"; do
			if [ "${seen_artifact}" = "${artifact}" ]; then
				return 0
			fi
		done
	fi
	skipped_artifacts+=("${artifact}")
	if [ "${strict_mode}" = true ]; then
		strict_failures+=("${artifact}")
	fi
}

# --- Artifact availability pre-check (front of the gate list) -------------
# PWA and DEB gates consume gitignored build outputs (pwa/pkg/ and
# packaging/linux/build/). Detect them once up front so a clean checkout is
# told immediately that the DEB gate is SKIPPED, instead of paying for every
# code gate first and only then failing in build-deb.sh --verify (BH06/BH07).
pwa_snippet_dir="${repo_root}/pwa/pkg/snippets"
deb_build_dir="${repo_root}/packaging/linux/build"

deb_artifact_ready=false
if compgen -G "${deb_build_dir}/*.deb" >/dev/null 2>&1; then
	deb_artifact_ready=true
else
	echo "SKIPPED: deb --verify (no built .deb in packaging/linux/build/; run packaging/linux/build-deb.sh)."
	record_missing_artifact deb-artifact
fi

# The wasm/snippet assertions in pwa/main.test.js read the gitignored pwa/pkg/
# tree. main.test.js guards those assertions, so the pure helpers still run;
# report the reduced coverage here as well (BH07).
if [ ! -d "${pwa_snippet_dir}" ]; then
	echo "SKIPPED: PWA wasm/snippet assertions (no pwa/pkg/snippets/; pure helpers still run)."
	record_missing_artifact pwa-wasm-artifact
fi

# BI16: the Android scaffold is tracked, but it is gated like the other
# artifact checks (SKIPPED with a notice, hard failure under --strict) so a
# partially checked-out tree does not silently lose this static verification.
android_dir="${repo_root}/android"
android_scaffold_files=(
	"${android_dir}/settings.gradle.kts"
	"${android_dir}/app/src/main/AndroidManifest.xml"
	"${android_dir}/build.gradle.kts"
	"${android_dir}/app/build.gradle.kts"
)
android_scaffold_ready=true
for android_file in "${android_scaffold_files[@]}"; do
	if [ ! -f "${android_file}" ]; then
		android_scaffold_ready=false
		break
	fi
done
if [ "${android_scaffold_ready}" != true ]; then
	echo "SKIPPED: android scaffold static gate (android/ scaffold files missing)."
	record_missing_artifact android-scaffold
fi

run_audit_check() {
	local project_dir="$1"
	if ! command -v cargo-audit >/dev/null 2>&1; then
		# BI09: CI runs cargo audit only for app/ (rust-gates audit: true);
		# poc/ has no cargo-audit step, so a fixed "CI runs it" claim is wrong.
		echo "cargo-audit is not installed; skipping the advisory check for ${project_dir}."
		record_missing_tool cargo-audit
		return
	fi
	echo "==> ${project_dir}: cargo audit"
	(cd "${project_dir}" && cargo audit)
}

run_deny_check() {
	local project_dir="$1"
	if ! command -v cargo-deny >/dev/null 2>&1; then
		echo "cargo-deny is not installed; skipping the deny check (CI runs it)."
		record_missing_tool cargo-deny
		return
	fi

	if [ "${fast_mode}" = true ]; then
		# Both projects declare a license, so --fast keeps the offline
		# licenses/bans/sources set identical for app/ and poc/.
		echo "==> ${project_dir}: cargo deny check (offline: licenses/bans/sources)"
		(cd "${project_dir}" && cargo deny --offline check licenses bans sources)
		return
	fi

	echo "==> ${project_dir}: cargo deny check"
	(cd "${project_dir}" && cargo deny check)
}

run_vet_check() {
	local project_dir="$1"
	if ! command -v cargo-vet >/dev/null 2>&1; then
		echo "cargo-vet is not installed; skipping."
		record_missing_tool cargo-vet
		return
	fi
	if [ ! -f "${project_dir}/supply-chain/config.toml" ]; then
		echo "==> ${project_dir}: cargo vet skipped (no supply-chain/config.toml; run 'cargo vet init' there to enable)."
		return
	fi
	echo "==> ${project_dir}: cargo vet check"
	(cd "${project_dir}" && cargo vet check)
}

run_pwa_test() {
	if ! command -v node >/dev/null 2>&1; then
		# The CI workflow intentionally has no PWA step, so do not claim that
		# CI covers this gate when node is absent (BH08).
		echo "node is not installed; skipping the PWA test suite."
		record_missing_tool node
		return
	fi
	# `node --test` / node:test needs Node 18+. An older Node would otherwise
	# fail the gate; treat the missing capability like a missing tool (BH15).
	if ! node -e "require('node:test')" >/dev/null 2>&1; then
		echo "node lacks node:test support; skipping the PWA test suite."
		record_missing_tool node:test
		return
	fi
	echo '==> pwa: node --test pwa/main.test.js'
	node --test pwa/main.test.js
}

# BI16: static gate for the Android scaffold. No Gradle/SDK is needed: the
# manifest must be well-formed and structurally launchable, and the toolchain /
# dependency versions (including settings.gradle.kts and the Compose compiler
# plugin) must stay pinned. CI has no Android step, so a missing python3 is
# reported as SKIPPED only.
run_android_scaffold_gate() {
	if [ "${android_scaffold_ready}" != true ]; then
		# Already reported as SKIPPED by the front-of-list pre-check.
		return
	fi
	if ! command -v python3 >/dev/null 2>&1; then
		echo "python3 is not installed; skipping the Android scaffold static gate."
		record_missing_tool python3
		return
	fi
	echo '==> android: static scaffold gate (python3: XML + pinned versions)'
	python3 - "${android_dir}" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

root = Path(sys.argv[1])
errors = []

ANDROID_NS = "{http://schemas.android.com/apk/res/android}"
NAME = ANDROID_NS + "name"


def has_active_line(source, needle):
    """A pinned value that only appears in a // comment is not a declaration (BJ18)."""
    return any(
        needle in line and not line.lstrip().startswith("//")
        for line in source.splitlines()
    )


# BJ19: ET.parse alone only proves well-formedness. Also verify the structure
# that actually defines a launchable launcher activity.
manifest = root / "app/src/main/AndroidManifest.xml"
manifest_root = None
try:
    manifest_root = ET.parse(manifest).getroot()
except (ET.ParseError, OSError) as exc:
    errors.append(f"AndroidManifest.xml が well-formed ではありません: {exc}")

if manifest_root is not None:
    if manifest_root.tag != "manifest":
        errors.append(
            f"AndroidManifest.xml のルート要素が manifest ではありません: {manifest_root.tag}"
        )
    application = manifest_root.find("application")
    if application is None:
        errors.append("AndroidManifest.xml に <application> がありません")
    else:
        launcher_activities = []
        for activity in application.findall("activity"):
            for intent_filter in activity.findall("intent-filter"):
                actions = [node.get(NAME) for node in intent_filter.findall("action")]
                categories = [node.get(NAME) for node in intent_filter.findall("category")]
                if ("android.intent.action.MAIN" in actions
                        and "android.intent.category.LAUNCHER" in categories):
                    launcher_activities.append(activity.get(NAME) or "(name なし)")
        if not launcher_activities:
            errors.append(
                "AndroidManifest.xml に MAIN/LAUNCHER intent-filter を持つ <activity> がありません"
            )

root_gradle = (root / "build.gradle.kts").read_text(encoding="utf-8")
app_gradle = (root / "app/build.gradle.kts").read_text(encoding="utf-8")
settings_gradle = (root / "settings.gradle.kts").read_text(encoding="utf-8")

pinned = {
    "build.gradle.kts": (
        root_gradle,
        (
            'id("com.android.application") version "8.6.1"',
            'id("org.jetbrains.kotlin.android") version "2.0.21"',
            # BJ03: the Compose compiler plugin is pinned as well so a silent
            # version bump cannot escape the gate.
            'id("org.jetbrains.kotlin.plugin.compose") version "2.0.21"',
        ),
    ),
    "app/build.gradle.kts": (
        app_gradle,
        (
            'platform("androidx.compose:compose-bom:2024.09.02")',
            'implementation("androidx.activity:activity-compose:1.9.3")',
        ),
    ),
    # BJ13: settings.gradle.kts is part of the scaffold; include(":app") must
    # stay active or the app module silently disappears from the build.
    "settings.gradle.kts": (
        settings_gradle,
        ('include(":app")',),
    ),
}
for name, (source, needles) in pinned.items():
    for needle in needles:
        if not has_active_line(source, needle):
            errors.append(f"{name} に固定バージョンがありません: {needle}")

# BJ03: Kotlin Android and the Compose compiler plugin must use the same
# version; a mismatch fails Gradle configuration even when both are pinned.
kotlin_android = re.search(
    r'id\("org\.jetbrains\.kotlin\.android"\)\s+version\s+"([^"]+)"', root_gradle
)
kotlin_compose = re.search(
    r'id\("org\.jetbrains\.kotlin\.plugin\.compose"\)\s+version\s+"([^"]+)"', root_gradle
)
if (kotlin_android and kotlin_compose
        and kotlin_android.group(1) != kotlin_compose.group(1)):
    errors.append(
        "Kotlin Android と Compose compiler のバージョンが不一致です: "
        f"{kotlin_android.group(1)} != {kotlin_compose.group(1)}"
    )

if errors:
    for error in errors:
        print(error, file=sys.stderr)
    sys.exit(1)
print("照合 OK: android scaffold (manifest structure + pinned versions)")
PY
}

run_deb_verify() {
	if [ "${deb_artifact_ready}" != true ]; then
		# Already reported as SKIPPED by the front-of-list pre-check.
		return
	fi
	# build-deb.sh --verify shells out to all four commands (dpkg-deb for the
	# archive, dpkg for the architecture, realpath for NOTICE links, tar for
	# the embedded control/payload). Check every one of them, not just
	# dpkg-deb, so a missing tool is reported as SKIPPED instead of crashing
	# halfway through verification (BH14).
	local missing=0
	local tool
	for tool in dpkg-deb dpkg realpath tar; do
		if ! command -v "${tool}" >/dev/null 2>&1; then
			echo "${tool} is not installed; skipping the .deb verification."
			record_missing_tool "${tool}"
			missing=1
		fi
	done
	if [ "${missing}" -ne 0 ]; then
		return
	fi
	echo '==> deb: packaging/linux/build-deb.sh --verify'
	bash packaging/linux/build-deb.sh --verify
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

	if [ "${run_release}" = true ]; then
		echo '==> app: cargo check --release --all-targets --locked'
		(cd app && cargo check --release --all-targets --locked)

		echo '==> app: cargo test --release --all-targets --locked'
		(cd app && cargo test --release --all-targets --locked)
	fi

	if [ "${run_audit}" = true ]; then
		run_audit_check app
	fi
	if [ "${run_deny}" = true ]; then
		run_deny_check app
	fi
	if [ "${run_vet}" = true ]; then
		run_vet_check app
	fi
fi

# core / win32 は workspace member だが、app/poc の個別ゲートでは
# それぞれの #[cfg(test)] が実行されないため明示的に回す。
# 以前はこのブロックが run_poc 内にあり、--app 指定時に黙ってスキップされていた。
# app / poc のどちらかが有効なら常に実行する（CI の extra_gates と同じ範囲）。
run_member_checks() {
	for member in core win32; do
		echo "==> ${member}: cargo test --all-targets --locked"
		(cd "${member}" && cargo test --all-targets --locked)

		if [ "${member}" = "win32" ]; then
			# win32 は FFI 境界なので unsafe ブロックの SAFETY 注記を必須化する。
			echo "==> ${member}: cargo clippy --all-targets --locked -- -D warnings -D clippy::undocumented_unsafe_blocks"
			(cd "${member}" && cargo clippy --all-targets --locked -- -D warnings -D clippy::undocumented_unsafe_blocks)
		else
			echo "==> ${member}: cargo clippy --all-targets --locked -- -D warnings"
			(cd "${member}" && cargo clippy --all-targets --locked -- -D warnings)
		fi
	done

	# クロスコンパイル ゲート（CI の extra_gates と同一コマンド）。
	# `cargo check` はリンカを要さず、Windows GNU / wasm32 ターゲットを
	# rustup target add 済みなら Linux ホストでも実行できる。
	echo '==> core: cargo check --target wasm32-unknown-unknown --lib --locked'
	cargo check -p gpui-file-explorer-core --target wasm32-unknown-unknown --lib --locked

	echo '==> win32: cargo check --target x86_64-pc-windows-gnu --locked'
	cargo check -p gfe-win32 --target x86_64-pc-windows-gnu --locked
}

if [ "${run_app}" = true ] || [ "${run_poc}" = true ]; then
	run_member_checks
else
	echo 'SKIPPED: core/win32 member tests and cross-target checks (no scope selected)'
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
		run_deny_check poc
	fi
	if [ "${run_vet}" = true ]; then
		run_vet_check poc
	fi
fi

# PWA / DEB は scope（--app / --poc）に依存しない配布成果物のゲート。
# 両方とも読み取り専用で高速なので常に実行する。
run_pwa_test
run_deb_verify
run_android_scaffold_gate

if [ "${#skipped_tools[@]}" -gt 0 ]; then
	echo
	echo 'Skipped checks (tool not installed):'
	for skipped_tool in "${skipped_tools[@]}"; do
		echo "SKIPPED: ${skipped_tool} not installed"
	done
fi

if [ "${#skipped_artifacts[@]}" -gt 0 ]; then
	echo
	echo 'Skipped checks (build artifact not present):'
	for skipped_artifact in "${skipped_artifacts[@]}"; do
		echo "SKIPPED: ${skipped_artifact} not present"
	done
fi

if [ "${#strict_failures[@]}" -gt 0 ]; then
	echo "error: --strict: required tool(s)/artifact(s) not available: ${strict_failures[*]}" >&2
	exit 1
fi

echo 'All checks passed.'
