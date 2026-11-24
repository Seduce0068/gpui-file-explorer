#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Runs the local verification loop for this repository.
#
#   app: fmt -> check -> test -> clippy -> release(check/test) -> (optional) cargo audit / cargo deny / cargo vet
#   poc: fmt -> check -> test -> clippy -> (optional) cargo deny
#   pwa: node --check pwa/sw.js + node --input-type=module --check < pwa/main.js
#        + PRECACHE asset existence
#        (parsed from sw.js) + node --test pwa/main.test.js
#        (node --check runs whenever node exists; the test suite is SKIPPED
#        when node:test is unavailable, and the pure-helper subset still runs
#        without the gitignored pwa/pkg/)
#   deb: packaging/linux/build-deb.sh --verify (read-only stage/.deb check;
#        SKIPPED when no built .deb is present)
#   android: python3 stdlib static gate (AndroidManifest.xml structure,
#        @style reference resolution, launcher Activity class resolution and
#        pinned Gradle versions incl. settings.gradle.kts and Compose compiler;
#        SKIPPED when the scaffold is absent)
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
# dpkg-deb/dpkg/realpath/tar/stat, desktop-file-validate) and build artifacts
# (pwa/pkg/, built .deb, android/ scaffold):
#   - default: reported as `SKIPPED: ...` before the final line
#   - --strict: the same condition is a hard failure (non-zero exit status)
# Artifact availability is checked at the FRONT of the gate list so a clean
# checkout is not surprised by a .deb failure only after every code gate ran.
# The .desktop validator is a TOOL gate, not an artifact gate: the .desktop
# file is tracked, so it runs unconditionally even when no .deb was built.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

usage() {
	cat <<'EOF'
Runs the local verification loop for this repository.

  app: fmt -> check -> test -> clippy -> release(check/test) -> (optional) cargo audit / cargo deny / cargo vet
  poc: fmt -> check -> test -> clippy -> (optional) cargo deny
  pwa: node --check pwa/sw.js + pwa/main.js + PRECACHE asset existence + node --test pwa/main.test.js
  deb:  packaging/linux/build-deb.sh --verify (read-only; needs an existing stage/.deb)
  android: python3 stdlib static gate for the tracked scaffold (XML/style/Activity resolution + pinned versions; no build)

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
  - pwa: node --check pwa/sw.js + pwa/main.js + PRECACHE asset existence + node --test pwa/main.test.js
         (pure helpers run even without pwa/pkg/)
  - deb: packaging/linux/build-deb.sh --verify (needs a built .deb)
  - android: python3 stdlib static gate (@style / launcher Activity resolution,
             tracked scaffold; no build)
  - desktop: desktop-file-validate on the tracked .desktop file (tool gate;
    runs even when no .deb is present)

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
                                    # python3, dpkg-deb/dpkg/realpath/tar/stat, desktop-file-validate) or
                                    # artifact (.deb, pwa/pkg/snippets/, android/ scaffold) is missing
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

# Only the generated-hash equality check in pwa/main.test.js (sw.js PRECACHE
# vs. the actual pwa/pkg/snippets/<hash>/ directory) is guarded on the
# gitignored tree. The snippet presence/format assertions run unconditionally;
# do not describe this as skipping all wasm/snippet tests (BH07/BL15).
if [ ! -d "${pwa_snippet_dir}" ]; then
	echo "SKIPPED: PWA snippet generated-hash match against pwa/pkg/snippets/ (no generated tree; snippet format and all other tests still run)."
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
	# BM04: main.test.js only reads sw.js as a *string*, so a syntax error in
	# the service worker would still pass `node --test`. `node --check` parses
	# the script without executing it and works even where node:test is absent,
	# so run it immediately after the node presence check (before the capability
	# gate below) to keep the syntax gate independent of node:test support.
	# BN12: main.js is the other half of the PWA runtime and can fail to parse
	# just as silently when node:test is unavailable; check both files.
	echo '==> pwa: node --check pwa/sw.js'
	node --check pwa/sw.js
	# BO02: index.html loads main.js via <script type="module"> (ESM = strict),
	# but a plain `node --check` parses it as CommonJS/sloppy. Parsing the same
	# source with --input-type=module catches strict-only syntax errors that
	# would otherwise only appear in the browser. sw.js stays classic.
	echo '==> pwa: node --input-type=module --check < pwa/main.js'
	node --input-type=module --check < pwa/main.js
	# BM15/BN04: the PRECACHE string assertions never prove the enumerated
	# assets are actually on disk, and a hand-maintained list silently drifts
	# from sw.js (the generated snippet inline0.js was missing entirely).
	# Parse the PRECACHE array itself and require every entry to exist. Entries
	# under gitignored pwa/pkg/ are only checked when that tree is present;
	# the five tracked shell files are always checked. BO03: comments are
	# masked, both quote styles are accepted, absolute/.. paths are rejected
	# and the required shell asset set must be present.
	if ! command -v python3 >/dev/null 2>&1; then
		echo "python3 is not installed; skipping the PRECACHE asset existence gate."
		record_missing_tool python3
	else
		python3 - <<'PY'
import re
import sys
from pathlib import Path

source = Path("pwa/sw.js").read_text(encoding="utf-8")


def strip_js_comments(text):
    """Mask // and block comments while preserving quoted string literals."""
    out = []
    index = 0
    quote = None
    while index < len(text):
        char = text[index]
        following = text[index + 1] if index + 1 < len(text) else ""
        if quote is not None:
            out.append(char)
            if char == "\\" and following:
                out.append(following)
                index += 2
                continue
            if char == quote:
                quote = None
            index += 1
            continue
        if char in ("'", '"', "`"):
            quote = char
            out.append(char)
            index += 1
            continue
        if char == "/" and following == "/":
            while index < len(text) and text[index] != "\n":
                out.append(" ")
                index += 1
            continue
        if char == "/" and following == "*":
            out.append("  ")
            index += 2
            while index < len(text) and not (
                text[index] == "*" and text[index + 1 : index + 2] == "/"
            ):
                out.append("\n" if text[index] == "\n" else " ")
                index += 1
            if index < len(text):
                out.append("  ")
                index += 2
            continue
        out.append(char)
        index += 1
    return "".join(out)


cleaned = strip_js_comments(source)
start = cleaned.find("const PRECACHE = [")
if start < 0:
    sys.exit("error: pwa/sw.js に const PRECACHE 配列がありません")
end = cleaned.find("];", start)
if end < 0:
    sys.exit("error: pwa/sw.js の PRECACHE 配列が閉じられていません")
block = cleaned[start + len("const PRECACHE = [") : end]
# BO03: quoted entries may use either quote style; comments are already masked.
entries = [match.group(2) for match in re.finditer(r"""(['"])(.*?)\1""", block, re.S)]
if not entries:
    sys.exit("error: pwa/sw.js の PRECACHE 配列が空です")

required = {
    "./",
    "./index.html",
    "./manifest.webmanifest",
    "./main.js",
    "./style.css",
    "./pkg/gfe_core.js",
    "./pkg/gfe_core_bg.wasm",
}
missing_required = sorted(required - set(entries))
if missing_required:
    for entry in missing_required:
        print(f"error: PRECACHE に必須アセットがありません: {entry}", file=sys.stderr)
    sys.exit(1)

pwa_dir = Path("pwa")
pkg_generated = (pwa_dir / "pkg").is_dir()
missing = []
checked = 0
for entry in entries:
    # BO03: reject absolute paths, parent traversal, escapes and control chars
    # before touching the filesystem; every literal must resolve inside pwa/.
    if not entry.startswith("./"):
        sys.exit(f"error: PRECACHE エントリは ./ 相対である必要があります: {entry}")
    relative = entry[2:]
    if any(segment == ".." for segment in re.split(r"[/\\]", relative)):
        sys.exit(f"error: PRECACHE エントリに .. は使用できません: {entry}")
    if "\\" in entry or any(ord(char) < 0x20 for char in entry):
        sys.exit(f"error: PRECACHE エントリに不正な文字があります: {entry!r}")
    target = pwa_dir / relative
    # pwa/pkg/ は gitignore 済みの生成物。clean checkout では存在しないため
    # その場合のみ存在照合をスキップする（追跡対象のシェルは常に照合）。
    if not pkg_generated and relative.startswith("pkg/"):
        continue
    checked += 1
    if not target.exists():
        missing.append(entry)
if missing:
    for entry in missing:
        print(f"error: PRECACHE アセットがありません: pwa/{entry[2:]}", file=sys.stderr)
    sys.exit(1)
print(f"照合 OK: pwa PRECACHE アセットの存在 ({checked}/{len(entries)} 件)")
PY
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


def strip_comments(source):
    """Mask comments and Kotlin raw strings, preserving newlines.

    Kotlin block comments nest, so a line comment marker inside a block comment
    (or a block marker inside a line comment) must not change the state. Regular
    string literals are copied verbatim: pinned version strings such as
    'id("...") version "8.6.1"' must remain searchable (BJ18/BK04). Raw strings
    (\\\"\\\"\\\") are masked instead: their content is data (test fixtures, docs),
    so a version-like token appearing only there is not an active declaration
    (BL16). Inline block comments are masked with one space per character and
    normalized by has_active_line, so they cannot split a needle (BL17).
    """
    out = []
    i = 0
    size = len(source)
    block_depth = 0
    in_line_comment = False
    in_raw_string = False
    quote = None
    while i < size:
        char = source[i]
        following = source[i + 1] if i + 1 < size else ""
        third = source[i + 2] if i + 2 < size else ""
        if in_raw_string:
            if char == '"' and following == '"' and third == '"':
                out.append("   ")
                in_raw_string = False
                i += 3
                continue
            out.append("\n" if char == "\n" else " ")
            i += 1
            continue
        if in_line_comment:
            if char == "\n":
                in_line_comment = False
                out.append("\n")
            else:
                out.append(" ")
            i += 1
            continue
        if block_depth:
            if char == "/" and following == "*":
                out.append("  ")
                block_depth += 1
                i += 2
                continue
            if char == "*" and following == "/":
                out.append("  ")
                block_depth -= 1
                i += 2
                continue
            out.append("\n" if char == "\n" else " ")
            i += 1
            continue
        if quote is not None:
            out.append(char)
            if char == "\\" and following:
                out.append(following)
                i += 2
                continue
            if char == quote:
                quote = None
            i += 1
            continue
        # BL16: a Kotlin raw string starts with exactly three quotes. Handle it
        # before the comment/quote branches so `//` and `/*` inside it are data.
        if char == '"' and following == '"' and third == '"':
            in_raw_string = True
            out.append("   ")
            i += 3
            continue
        if char == "/" and following == "/":
            in_line_comment = True
            out.append("  ")
            i += 2
            continue
        if char == "/" and following == "*":
            block_depth = 1
            out.append("  ")
            i += 2
            continue
        if char in ('"', "'"):
            quote = char
        out.append(char)
        i += 1
    return "".join(out)


def has_active_line(source, needle):
    """A pinned value that only appears in a comment is not a declaration (BJ18).

    Comment masking replaces each comment character with a space, which can
    break a needle that had an inline comment in the middle; collapse runs of
    spaces before matching (BL17). Newlines are kept by strip_comments, so the
    per-line search still cannot join two separate declarations.
    """
    masked = re.sub(r' +', ' ', strip_comments(source))
    return any(needle in line for line in masked.splitlines())


# BJ19: ET.parse alone only proves well-formedness. Also verify the structure
# that actually defines a launchable launcher activity.
manifest = root / "app/src/main/AndroidManifest.xml"
manifest_root = None
launcher_activities = []
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
                    launcher_activities.append(activity)
        if not launcher_activities:
            errors.append(
                "AndroidManifest.xml に MAIN/LAUNCHER intent-filter を持つ <activity> がありません"
            )
        else:
            # BK11: Android 12 (targetSdk 31+) requires an explicit
            # android:exported on every component with an intent-filter. Without
            # it the launcher entry is either rejected at install time or the
            # export becomes ambiguous, so require a literal true/false.
            for activity in launcher_activities:
                name = activity.get(NAME) or "(name なし)"
                exported = activity.get(ANDROID_NS + "exported")
                if exported not in ("true", "false"):
                    errors.append(
                        f"launcher activity {name} に android:exported の明示指定（true/false）がありません"
                    )

# BM16/BN15: ET.parse alone does not prove that every @style/... reference in
# the manifest or the value resources resolves. Collect the references from
# all attribute values and require a matching <style name="..."> definition
# in res/values*/*.xml (qualifier directories such as values-night/ and
# values-v29/ are included). Framework styles (@android:style/...) are not
# matched by the @style/ pattern and are intentionally out of scope.
# BO10: keep definitions/references keyed by (resource qualifier, name). An
# unqualified reference (from the manifest or values/) must resolve to a style
# defined in the unqualified values/ directory; a values-night/ definition must
# not silently satisfy it. Framework styles (@android:style/...) stay out of scope.
STYLE_REF_RE = re.compile(r"@style/([A-Za-z0-9_.]+)")
style_refs = set()
style_defs = set()


def qualifier_of(directory_name):
    """values -> '', values-night -> 'night', values-v29 -> 'v29'."""
    return directory_name[len("values") :].lstrip("-")


def collect_style_refs(element, qualifier=""):
    # BO10: references may live in element text/tail (e.g. an <item> body), not
    # only in attributes; recurse through children with the same qualifier.
    for value in [element.text, element.tail, *element.attrib.values()]:
        if value:
            for name in STYLE_REF_RE.findall(value):
                style_refs.add((qualifier, name))
    for child in element:
        collect_style_refs(child, qualifier)


if manifest_root is not None:
    collect_style_refs(manifest_root)

# BN15: scan res/values*/*.xml, not only res/values/*.xml. A style may be
# defined (or referenced) in a qualified resource directory, and limiting the
# scan to the unqualified directory produced false failures on such scaffolds.
res_dir = root / "app/src/main/res"
values_files = sorted(
    candidate for candidate in res_dir.glob("values*/*.xml") if candidate.is_file()
)
for values_file in values_files:
    try:
        values_root = ET.parse(values_file).getroot()
    except (ET.ParseError, OSError) as exc:
        errors.append(f"{values_file.name} が well-formed ではありません: {exc}")
        continue
    qualifier = qualifier_of(values_file.parent.name)
    collect_style_refs(values_root, qualifier)
    for style_node in values_root.iter("style"):
        style_name = style_node.get("name")
        if style_name:
            style_defs.add((qualifier, style_name))

for qualifier, style_ref in sorted(style_refs):
    if (qualifier, style_ref) in style_defs:
        continue
    # A qualified reference may fall back to the default values/ definition,
    # but an unqualified reference may never rely on a qualified-only style.
    if qualifier and ("", style_ref) in style_defs:
        continue
    where = f"values-{qualifier}/" if qualifier else "values/"
    errors.append(
        f"@style/{style_ref} に対応する <style name=\"{style_ref}\"> が "
        f"res/{where} にありません"
    )

root_gradle = (root / "build.gradle.kts").read_text(encoding="utf-8")
app_gradle = (root / "app/build.gradle.kts").read_text(encoding="utf-8")
settings_gradle = (root / "settings.gradle.kts").read_text(encoding="utf-8")

# BM17/BN14: a launcher android:name that starts with '.' is only a relative
# class name. Resolve it against namespace first (that is the compile-time
# package of the Kotlin sources), then fall back to applicationId; both
# candidates are accepted so a project where they differ is not rejected.
# Require the matching Kotlin source under src/main/{java,kotlin}/ and a
# package declaration equal to the resolved class's package. Without this a
# renamed/moved Activity still passes the manifest structure gate but cannot
# be launched.
def gradle_string(source, key):
    match = re.search(r"\b" + re.escape(key) + r'\s*=\s*"([^"]+)"', source)
    return match.group(1) if match else None


namespace = gradle_string(app_gradle, "namespace")
application_id = gradle_string(app_gradle, "applicationId")
package_candidates = [value for value in (namespace, application_id) if value]
if launcher_activities and not package_candidates:
    errors.append("app/build.gradle.kts から namespace / applicationId を取得できません")
for activity in launcher_activities:
    activity_name = activity.get(NAME)
    if not activity_name:
        continue  # 名前欠落は manifest 構造ゲート側で既に報告済み
    if not package_candidates:
        break
    resolved = None
    primary_relative_kt = None
    mismatch_report = None
    for index, package_name in enumerate(package_candidates):
        if activity_name.startswith("."):
            fqcn = package_name + activity_name
        elif "." in activity_name:
            fqcn = activity_name
        else:
            fqcn = package_name + "." + activity_name
        relative_kt = fqcn.replace(".", "/") + ".kt"
        if index == 0:
            primary_relative_kt = relative_kt
        kt_candidates = (
            root / "app/src/main/java" / relative_kt,
            root / "app/src/main/kotlin" / relative_kt,
        )
        kt_path = next((candidate for candidate in kt_candidates if candidate.is_file()), None)
        if kt_path is None:
            continue
        package_decl = re.search(
            r"^\s*package\s+([A-Za-z0-9_.]+)",
            kt_path.read_text(encoding="utf-8"),
            re.MULTILINE,
        )
        expected_pkg = fqcn.rsplit(".", 1)[0]
        if not package_decl or package_decl.group(1) != expected_pkg:
            if mismatch_report is None:
                mismatch_report = (
                    f"{kt_path.relative_to(root)} の package 宣言が {expected_pkg} ではありません"
                )
            continue
        resolved = (kt_path, expected_pkg)
        break
    if resolved is None:
        if mismatch_report is not None:
            errors.append(mismatch_report)
        else:
            errors.append(
                f"launcher activity {activity_name} に対応する Kotlin ファイルがありません: "
                f"app/src/main/java/{primary_relative_kt}"
            )
        continue
    kt_path, expected_pkg = resolved
    # 解決済み候補の package 宣言は上で検証済み。ここでは採用した候補を
    # 報告するだけで、一致しない候補を理由に失敗させない。

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

# BK12/BL11: validate the tracked .desktop file as a standalone TOOL gate.
# It used to live inside run_deb_verify, whose early return on a missing .deb
# meant a clean checkout never ran it at all. The .desktop is tracked source,
# not a build artifact, so it must be checked unconditionally; only a missing
# validator is SKIPPED (hard failure under --strict), like the other tools.
run_desktop_file_validate() {
	# BN13: the Exec/TryExec first-token check used to live only in
	# build-deb.sh --verify, so a clean checkout never ran it. The .desktop is
	# tracked source: verify it here with bash, independently of any .deb.
	# BO12: inspect all Exec/TryExec lines and reject duplicate keys.
	local desktop="packaging/linux/gpui-file-explorer.desktop"
	local pkg line program exec_count=0 try_count=0
	pkg="$(basename -- "${desktop}" .desktop)"
	# BO12: validate *every* Exec / TryExec line (the old grep only inspected
	# the first one) and reject duplicate keys, which must stay unique in a
	# desktop entry.
	while IFS= read -r line || [ -n "${line}" ]; do
		case "${line}" in
			Exec=*)
				exec_count=$((exec_count + 1))
				program="${line#Exec=}"
				program="${program%%[[:space:]]*}"
				if [ "${program}" != "${pkg}" ]; then
					echo "error: ${desktop} の Exec 先頭トークンが ${pkg} ではありません: ${program}" >&2
					exit 1
				fi
				;;
			TryExec=*)
				try_count=$((try_count + 1))
				program="${line#TryExec=}"
				program="${program%%[[:space:]]*}"
				if [ "${program}" != "${pkg}" ]; then
					echo "error: ${desktop} の TryExec 先頭トークンが ${pkg} ではありません: ${program}" >&2
					exit 1
				fi
				;;
		esac
	done < "${desktop}"
	if [ "${exec_count}" -eq 0 ] || [ "${try_count}" -eq 0 ]; then
		echo "error: ${desktop} に Exec / TryExec がありません" >&2
		exit 1
	fi
	if [ "${exec_count}" -gt 1 ] || [ "${try_count}" -gt 1 ]; then
		echo "error: ${desktop} に Exec / TryExec の重複があります: Exec=${exec_count} TryExec=${try_count}" >&2
		exit 1
	fi
	echo "照合 OK: ${desktop} の Exec / TryExec == ${pkg} (重複なし)"
	if command -v desktop-file-validate >/dev/null 2>&1; then
		echo '==> desktop: desktop-file-validate packaging/linux/gpui-file-explorer.desktop'
		desktop-file-validate packaging/linux/gpui-file-explorer.desktop
	else
		echo "desktop-file-validate is not installed; skipping the .desktop validation."
		record_missing_tool desktop-file-validate
	fi
}

run_deb_verify() {
	if [ "${deb_artifact_ready}" != true ]; then
		# Already reported as SKIPPED by the front-of-list pre-check.
		return
	fi
	# build-deb.sh --verify shells out to all five commands (dpkg-deb for the
	# archive, dpkg for the architecture, realpath for NOTICE links, tar for
	# the embedded control/payload and control tarfile maintainer scripts,
	# stat for the 0755 maintainer-script mode).
	# Check every one of them, not just dpkg-deb, so a missing tool is reported
	# as SKIPPED instead of crashing halfway through verification (BH14/BJ14).
	local missing=0
	local tool
	for tool in dpkg-deb dpkg realpath tar stat; do
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
run_desktop_file_validate
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
