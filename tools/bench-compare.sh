#!/usr/bin/env bash
# Compare Criterion results against the committed baseline; fail on a >10% regression.
#
#   tools/bench-compare.sh          compare against the baseline
#   tools/bench-compare.sh --save   record the current run as the new baseline
#
# There are no benches until T-003 adds the pitch detector, so this exits cleanly until there is
# something to measure. It is wired up now so that the first bench lands with its gate already in
# place rather than needing one retrofitted.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

BASELINE="rust/crates/dsp/benches/baseline.json"

if ! compgen -G "rust/crates/dsp/benches/*.rs" >/dev/null 2>&1; then
    echo "No benches yet - the DSP arrives in T-003 (docs/adr/0005)."
    exit 0
fi

if [[ "${1:-}" == "--save" ]]; then
    echo "TODO(T-003): record Criterion estimates into $BASELINE"
    exit 1
fi

echo "TODO(T-003): compare target/criterion against $BASELINE, fail on >10% regression"
exit 1
