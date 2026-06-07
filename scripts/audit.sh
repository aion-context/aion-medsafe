#!/bin/sh
# AION-MEDSAFE full system integrity audit.
#
# Runs the six checks from .claude/skills/audit/SKILL.md as one command:
#   registry health, provenance chains, data inventory, code quality,
#   security, and tests. Exits non-zero if any check FAILs.
#
#   ./scripts/audit.sh   (or: make audit)

set -u

root="$(git rev-parse --show-toplevel 2>/dev/null || (cd "$(dirname "$0")/.." && pwd))"
cd "$root"

BIN="system/target/release/aion-medsafe"
fails=0
note() { printf '  %s\n' "$1"; }
fail() { fails=$((fails + 1)); printf '  FAIL: %s\n' "$1"; }

if [ ! -x "$BIN" ]; then
	echo "Building release binary..."
	( cd system && cargo build --release --quiet ) || { echo "build failed"; exit 1; }
fi

echo "============================================================"
echo "AION-MEDSAFE Audit Report   $(git rev-parse --short HEAD 2>/dev/null)"
echo "============================================================"

# 1. Registry health -----------------------------------------------------------
echo "[1] Registry"
reg="system/.aion/medsafe.registry.json"
if [ -f "$reg" ] && python3 -c "import json,sys; d=json.load(open('$reg')); sys.exit(0 if d.get('version')==1 and d.get('authors') else 1)" 2>/dev/null; then
	authors=$(python3 -c "import json; print(len(json.load(open('$reg'))['authors']))")
	note "OK — v1, ${authors} author(s) registered"
else
	fail "registry missing/invalid ($reg)"
fi

# 2. Provenance chain integrity ------------------------------------------------
echo "[2] Provenance"
verified=0
for f in system/provenance/*.aion system/policy/*.aion system/decisions/*.aion system/release/*.aion; do
	[ -f "$f" ] || continue
	if ( cd system && ./target/release/aion-medsafe provenance --manifest "${f#system/}" 2>/dev/null ) | grep -q "Valid: true"; then
		verified=$((verified + 1))
	else
		fail "manifest not valid: $f"
	fi
done
note "OK — ${verified} manifest(s) verified"

# 3. Data inventory (informational) --------------------------------------------
echo "[3] Data"
raw=$(find pipeline/data/raw -type f 2>/dev/null | wc -l | tr -d ' ')
note "raw source files: ${raw}"
for f in pipeline/data/normalized/*.ndjson; do
	[ -f "$f" ] && note "$(printf '%12s' "$(wc -l < "$f")") records  $(basename "$f")"
done

# 4. Code quality --------------------------------------------------------------
echo "[4] Code Quality"
if ( cd system && cargo clippy --quiet -- -D warnings ) 2>/dev/null; then
	note "OK — clippy -D warnings clean"
else
	fail "clippy reported warnings/errors"
fi
if ( cd system && cargo fmt --check ) 2>/dev/null; then note "OK — rustfmt clean"; else fail "rustfmt would reformat"; fi
if python3 -m py_compile pipeline/src/aion_medsafe_pipeline/*.py 2>/dev/null; then
	note "OK — python compiles"
else
	fail "python py_compile failed"
fi

# 5. Security ------------------------------------------------------------------
echo "[5] Security"
hits=$(grep -rnE 'PRIVATE KEY|BEGIN RSA|sk_live|api_key[[:space:]]*=' \
	--include='*.py' --include='*.rs' --include='*.toml' --include='*.json' . \
	2>/dev/null | grep -v target | grep -v '\.venv' | grep -v '\.git/')
if [ -z "$hits" ]; then note "OK — no hard-coded secrets in source"; else fail "possible secret(s): $hits"; fi

# 6. Tests ---------------------------------------------------------------------
echo "[6] Tests"
if ( cd system && cargo test --quiet ) >/dev/null 2>&1; then note "OK — Rust tests pass"; else fail "Rust tests failed"; fi
if ( cd pipeline && PYTHONPATH=src python3 -m pytest tests/ -q ) >/dev/null 2>&1; then
	note "OK — Python tests pass"
else
	fail "Python tests failed"
fi

echo "============================================================"
if [ "$fails" -eq 0 ]; then
	echo "Overall: PASS"
	exit 0
else
	echo "Overall: FAIL (${fails} check(s) failed)"
	exit 1
fi
