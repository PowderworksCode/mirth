#!/usr/bin/env bash
# Run any command with MIRTH_OUT set, folding the coverage logs of the instrumented compiler's
# processes into $WORK/cov-suites/<name>/union.txt as they finish (`mirth-lab coverage-compact`),
# where rustc/coverage-report.sh picks them up (COV_SUITES: another directory instead).
#
#   WORK=~/mirth-work rustc/coverage-run.sh <name> <command...>
#
# For example, the UI tests through incremental rebuilds:
#   rustc/coverage-run.sh ui-fuzz target/release/mirth-lab ui-fuzz --rustc $COV_RUSTC \
#     --tests $MIRTH_RUST/tests/ui --list ~/mirth-work/ui-cov/all-runnable.json \
#     --work ~/mirth-work/ui-fuzz-cov --edits 3 --jobs 6
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/mirth-work}
name=$1; shift
out=${COV_SUITES:-$work/cov-suites}/$name
mkdir -p "$out/logs"
rm -f "$out/done"
# The compactor is a mirth-lab subcommand: build it first (a no-op when up to date).
(cd "$here/.." && cargo build --release -q --offline -p mirth-lab) || exit 1
"$here/../target/release/mirth-lab" coverage-compact --logs "$out/logs" --out "$out" --until "$out/done" > "$out/compact.log" 2>&1 &
compactor=$!
# A compiler with docs/hunt/coverage-dims.patch also writes which MIR passes changed a body and
# which locks were contended; others ignore the variables.
MIRTH_OUT=$out/logs RUSTC_PASS_EFFECT=$out/logs RUSTC_LOCK_CONTENTION=$out/logs "$@" > "$out/run.log" 2>&1
status=$?
touch "$out/done"
wait "$compactor"
echo "$name: exit $status; $(wc -l < "$out/union.txt") sites"
