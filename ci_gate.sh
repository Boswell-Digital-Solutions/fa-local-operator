#!/usr/bin/env bash
# FA Local CI gate — execution bridge v1 contract participation
#
# Wires FA Local into the forge-contract-core gate runner and runs the
# local Rust test suite (cargo test). Both must pass.
#
# The contract-core gate validates: schemas, fixture corpus, validator
# correctness, compatibility notes, and forbidden patterns — using the
# shared contract library that FA Local will consume as an execution
# consumer in execution bridge v1.
#
# The local cargo test suite validates: bounded execution correctness,
# policy/capability admission, route decisions, and coordination
# behavior across all declared state machine transitions.
#
# Exit codes:
#   0 — all gates pass
#   1 — one or more gates failed
#
# Usage (from the FA Local root):
#   bash ci_gate.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Locate forge-contract-core relative to ecosystem root.
# Allow CONTRACT_CORE_PATH to point at the live checkout when repo naming differs.
ECOSYSTEM_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CONTRACT_CORE_PATH="${CONTRACT_CORE_PATH:-$ECOSYSTEM_ROOT/contracts/forge-contract-core}"
if [[ ! -d "$CONTRACT_CORE_PATH" && -d "$ECOSYSTEM_ROOT/contracts/forge_contract_core" ]]; then
    CONTRACT_CORE_PATH="$ECOSYSTEM_ROOT/contracts/forge_contract_core"
fi
REPORT_DIR="$SCRIPT_DIR/reports"

if [[ ! -d "$CONTRACT_CORE_PATH" ]]; then
    echo "ERROR: forge-contract-core not found at $CONTRACT_CORE_PATH"
    exit 1
fi

echo "FA Local CI gate — execution bridge v1 contract participation"
echo "  contract core path: $CONTRACT_CORE_PATH"
echo ""

# Use the contract-core venv if available, otherwise system python
PYTHON="$CONTRACT_CORE_PATH/.venv/bin/python"
if [[ ! -x "$PYTHON" ]]; then
    PYTHON="python3"
fi

echo "  python: $PYTHON"
echo ""

mkdir -p "$REPORT_DIR"

# ── Gate 1: forge-contract-core canonical gate runner ─────────────────────────
echo "=== Gate 1: forge-contract-core canonical gate ==="
GATE_REPORT="$REPORT_DIR/contract_core_gate_$(date +%Y%m%d_%H%M%S).json"
PYTHONPATH="$CONTRACT_CORE_PATH" "$PYTHON" -m forge_contract_core.gates.run_all \
    --repo "fa-local-operator" \
    --report-out "$GATE_REPORT"

echo ""

# ── Gate 2: local Rust test suite ─────────────────────────────────────────────
echo "=== Gate 2: local cargo test suite ==="
cd "$SCRIPT_DIR"

if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found in PATH"
    exit 1
fi

cargo test 2>&1

echo ""
echo "FA Local CI gate: PASSED"
echo "  contract core gate report: $GATE_REPORT"
