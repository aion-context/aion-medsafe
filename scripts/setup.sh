#!/bin/sh
# AION-MEDSAFE developer bootstrap.
# Run once after cloning to activate the version-controlled git hooks.
#
#   ./scripts/setup.sh
#
# core.hooksPath is a per-clone local git setting (it lives in .git/config and
# is not itself version-controlled), so each contributor must run this once.

set -eu

# Resolve repo root so this works regardless of the caller's cwd.
root="$(git rev-parse --show-toplevel)"
cd "$root"

git config core.hooksPath scripts/hooks
chmod +x scripts/hooks/* 2>/dev/null || true

echo "setup: core.hooksPath -> $(git config --get core.hooksPath)"
echo "setup: pre-commit hook active. Done."
