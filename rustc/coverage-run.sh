#!/usr/bin/env bash
# Run any command with MIRTH_OUT set, folding the coverage logs of the instrumented compiler's
# processes into $WORK/cov-suites/<name>/union.txt as they finish (rustc/coverage-compact.py),
# where rustc/coverage-report.sh picks them up.
#
#   WORK=~/mirth-work rustc/coverage-run.sh <name> <command...>
#
# For example, the UI tests through incremental rebuilds:
#   rustc/coverage-run.sh ui-fuzz python3 rustc/ui-fuzz.py --rustc $COV_RUSTC \
#     --tests $MIRTH_RUST/tests/ui --list ~/mirth-work/ui-cov/all-runnable.json \
#     --work ~/mirth-work/ui-fuzz-cov --edits 3 --jobs 6
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/mirth-work}
name=$1; shift
out=$work/cov-suites/$name
mkdir -p "$out/logs"
rm -f "$out/done"
python3 "$here/coverage-compact.py" --logs "$out/logs" --out "$out" --until "$out/done" > "$out/compact.log" 2>&1 &
compactor=$!
MIRTH_OUT=$out/logs "$@" > "$out/run.log" 2>&1
status=$?
touch "$out/done"
wait "$compactor"
echo "$name: exit $status; $(wc -l < "$out/union.txt") sites"
