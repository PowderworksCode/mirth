#!/usr/bin/env bash
# The suites that measure coverage beyond blocks with the compiler rustc/build-dims.sh builds,
# each into $COV_SUITES/<name> (default $WORK/cov-dims), then the reports: `mirth-lab callgraph`
# (functions, blocks, arms; --arm-gaps, --config) and `mirth-lab coverage-dims` (keyed engine
# paths, incremental transitions, feature gates consulted, type kinds, call pairs, MIR pass
# effect, lock contention).
#
#   rustc/coverage-dims-suites.sh       JOBS=4; ONLY="ui incremental" for a subset
#
# ui: every standalone UI test compiled once (diag-check); incremental: tests/incremental through
# compiletest (dep-graph transitions across sessions); threads: every UI test again under
# -Zthreads=8 (repro-diff's threads variant: contention, the parallel front end); opt: run-pass
# tests at -Copt-level=2 and -Copt-level=3 -Zmir-opt-level=4 (opt-diff: MIR passes at higher
# levels).
set -u
here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/mirth-work}
build=${BUILD_DIR:-$work/build-cov2}
R=$build/host/stage1/bin/rustc
rust=${DIMS_RUST:-$work/rust-cov2}
T=$work/rust/tests/ui
jobs=${JOBS:-4}
export COV_SUITES=${COV_SUITES:-$work/cov-dims}
scratch=$work/cov-dims-scratch
lab=$here/../target/release/mirth-lab

want() { [ -z "${ONLY:-}" ] || [[ " $ONLY " == *" $1 "* ]]; }
done_already() { [ -e "$COV_SUITES/$1/done" ]; }

if want ui && ! done_already ui; then
  "$here/coverage-run.sh" ui "$lab" diag-check --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/ui"
fi
if want incremental && ! done_already incremental; then
  RUST_TEST_THREADS=$jobs MIRTH_RUST=$rust BUILD_DIR=$build \
    "$here/coverage-suites.sh" "$COV_SUITES" incremental tests/incremental
fi
if want threads && ! done_already threads; then
  "$here/coverage-run.sh" threads "$lab" repro-diff --rustc "$R" --tests "$T" --variants threads --jobs "$jobs" --work "$scratch/threads"
fi
if want opt && ! done_already opt; then
  "$here/coverage-run.sh" opt "$lab" opt-diff --rustc "$R" --tests "$T" --configs O2,O3-mir4 --jobs "$jobs" --work "$scratch/opt"
fi

suites=()
for d in "$COV_SUITES"/*/; do [ -s "$d/union.txt" ] && suites+=("$d"); done
hits=(); dims=()
for d in "${suites[@]}"; do hits+=(--hit "$d/union.txt"); dims+=(--suite "$d"); done
"$lab" callgraph --graph "$work/build-cg/mirth-sites" --sites "$build/mirth-sites" "${hits[@]}" \
  --block-gaps "$COV_SUITES/gaps-blocks.md" --arm-gaps "$COV_SUITES/gaps-arms.md" --config "$COV_SUITES/config-arms.md" \
  > "$COV_SUITES/coverage-report.txt"
"$lab" coverage-dims --sites "$build/mirth-sites" --graph "$work/build-cg/mirth-sites" "${dims[@]}" --out "$COV_SUITES/dims.md"
grep -E '^(coverage|blocks|arms):' "$COV_SUITES/coverage-report.txt"
