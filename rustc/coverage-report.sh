#!/usr/bin/env bash
# Combined function (and block) coverage of the compiler: every folded run under $WORK/cov-suites, the sink
# and option-configuration logs, against the call graph's denominator. Writes
# $WORK/coverage-report.txt, $WORK/gaps.md (what can run and did not, by crate and file) and
# $WORK/gaps.json; with a compiler built with `[coverage] blocks`, also $WORK/gaps-blocks.md (the
# blocks that never ran in functions that did). Extra arguments go to `mirth-lab callgraph` (`--why <function>`, `--unreachable <crate>`).
#
#   WORK=~/mirth-work rustc/coverage-report.sh [--why <function> ...]
#
# Another corpus or compiler: COV_SUITES (the folded runs), COV_LOGS (directories of raw logs,
# space-separated), COV_BUILD (the coverage build directory, for its site tables) and COV_OUT
# (where the report and the gap lists go). Runs of a compiler built with `[coverage] blocks`
# belong in their own COV_SUITES: a run without block sites would count every block as unhit.
#
# Then the dimensions beyond functions and blocks (docs/coverage-plan.md), from the block build's
# site tables (COV_BLOCKS, default build-blk; meaningful when COV_SUITES and COV_LOGS are the block
# build's runs, as in the cov-blk report) and the rust checkout it was built from (MIRTH_RUST):
# coverage-static (arms, configuration branches, gates, delayed bugs, keyed-site and table
# denominators), rmeta-coverage, and, when their corpus passes have been collected
# ($WORK/cov-static/{diags,ir}: `mirth-lab diag-coverage --collect`, `codegen-coverage --collect`),
# diag-coverage and codegen-coverage. Each writes coverage-<name>.txt and its gaps-*.md next to
# the others.
#
# Runs of programs outside the compiler that link it (ui-fulldeps, the compiler crates' unit
# tests: directory names containing `fulldeps` or `compiler-unit`) are `--external`: what they
# ran is a root of the call graph, since their own mains call it.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/mirth-work}
suites=${COV_SUITES:-$work/cov-suites}
build=${COV_BUILD:-$work/build-cov}
report=${COV_OUT:-$work}
runs=()
shopt -s nullglob
for f in "$suites"/*/union.txt; do
  case $f in
    *fulldeps*|*compiler-unit*) runs+=(--external "$f") ;;
    *) runs+=(--hit "$f") ;;
  esac
done
logs=()
dirs=${COV_LOGS-$work/cov-sink $work/cov-flags}
for d in $dirs; do
  [ -d "$d" ] && logs+=(--logs "$d")
done
(cd "$here/.." && cargo build --release -q --offline -p mirth-lab) || exit 1
"$here/../target/release/mirth-lab" callgraph --graph "$work/build-cg/mirth-sites" --sites "$build/mirth-sites" \
  "${runs[@]}" "${logs[@]}" --gaps "$report/gaps.md" --block-gaps "$report/gaps-blocks.md" --json "$report/gaps.json" "$@" \
  > "$report/coverage-report.txt"
sed -n '1,5p;/^blocks:/p' "$report/coverage-report.txt"
lab="$here/../target/release/mirth-lab"
rust=${MIRTH_RUST:-$work/rust}
blocks=${COV_BLOCKS:-$work/build-blk}
common=(--rust "$rust" --sites "$blocks/mirth-sites" --suites "$suites" --gaps "$report/gaps.json")
for d in $dirs; do
  [ -d "$d" ] && common+=(--logs "$d")
done
"$lab" coverage-static "${common[@]}" --out "$report" --json "$report/coverage-static.json" > "$report/coverage-static.txt" || true
"$lab" rmeta-coverage --rust "$rust" --out "$report" > "$report/coverage-rmeta.txt" || true
collected=$work/cov-static
if [ -s "$collected/diags/results.jsonl" ]; then
  "$lab" diag-coverage --rust "$rust" --tests "$rust/tests/ui" --work "$collected/diags" --out "$report" > "$report/coverage-diagnostics.txt" || true
fi
if [ -s "$collected/ir/results.jsonl" ]; then
  abi=$(ls -d "$work"/big-*/abi-diff-1 2>/dev/null | tail -1)
  "$lab" codegen-coverage "${common[@]}" --tests "$rust/tests/ui" --work "$collected/ir" ${abi:+--abi "$abi"} --out "$report" > "$report/coverage-codegen.txt" || true
fi
for f in static rmeta diagnostics codegen; do
  [ -s "$report/coverage-$f.txt" ] && grep -E "^(branch arms|  in reachable|configuration|feature gates|delayed-bug|keyed|metadata tables|error codes|lints|diagnostic messages|subdiagnostic|LLVM intrinsics|Rust intrinsics)" "$report/coverage-$f.txt"
done
true
