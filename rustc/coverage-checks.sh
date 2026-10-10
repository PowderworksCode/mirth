#!/usr/bin/env bash
# Coverage of the compiler under the checks of docs/checks.md: each check that takes a --rustc
# runs once with the instrumented compiler (build-blk), through coverage-run.sh, as its own
# suite in $WORK/cov-suites/check-<name>/. rustc/coverage-report.sh then counts them with the
# other suites. Checks that need a whole toolchain (rustdoc-diff, instr-check, debug-check,
# xlink, release-diff) are not here: the instrumented build has no rustdoc or profiler.
#
#   rustc/coverage-checks.sh            JOBS=3; ONLY="lint-check abi-diff" for a subset
set -u
here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/mirth-work}
R=${COV_RUSTC:-$work/build-blk/host/stage1/bin/rustc}
rust=$work/rust
T=$rust/tests/ui
jobs=${JOBS:-3}
scratch=$work/cov-checks

suite() { # name args...
  local name=$1; shift
  if [ -n "${ONLY:-}" ] && [[ " $ONLY " != *" $name "* ]]; then return; fi
  [ -e "$work/cov-suites/check-$name/done" ] && return
  echo "$(date +%T) start $name"
  "$here/coverage-run.sh" "check-$name" "$here/../target/release/mirth-lab" "$@"
}

suite lint-check lint-check --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/lint-check"
suite solver-diff solver-diff --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/solver-diff"
suite repro-diff repro-diff --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/repro-diff"
suite suggest-diff suggest-diff --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/suggest-diff"
suite rewrite-diff rewrite-diff --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/rewrite-diff"
suite diag-check diag-check --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/diag-check"
suite gate-check gate-check --rustc "$R" --rust "$rust" --jobs "$jobs" --work "$scratch/gate-check"
suite opt-diff opt-diff --rustc "$R" --tests "$T" --jobs "$jobs" --work "$scratch/opt-diff"
suite gate-mutate gate-mutate --rustc "$R" --rust "$rust" --count 20000 --reduce-budget 0 --jobs "$jobs" --work "$scratch/gate-mutate"
suite abi-diff abi-diff --rustc "$R" --rust "$rust" --all --count 300 --seed 1 --jobs "$jobs" --work "$scratch/abi-diff"
suite scale-check scale-check --rustc "$R" --jobs 4 --work "$scratch/scale-check"
"$here/coverage-report.sh"
