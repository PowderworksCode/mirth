#!/usr/bin/env bash
# Combined function (and block) coverage of the compiler: every folded run under $WORK/cov-suites, the sink
# and option-configuration logs, against the call graph's denominator. Writes
# $WORK/coverage-report.txt, $WORK/gaps.md (what can run and did not, by crate and file) and
# $WORK/gaps.json; with a compiler built with `[coverage] blocks`, also $WORK/gaps-blocks.md (the
# blocks that never ran in functions that did). Extra arguments go to callgraph.py (`--why <function>`, `--unreachable <crate>`).
#
#   WORK=~/mirth-work rustc/coverage-report.sh [--why <function> ...]
#
# Another corpus or compiler: COV_SUITES (the folded runs), COV_LOGS (directories of raw logs,
# space-separated), COV_BUILD (the coverage build directory, for its site tables) and COV_OUT
# (where the report and the gap lists go). Runs of a compiler built with `[coverage] blocks`
# belong in their own COV_SUITES: a run without block sites would count every block as unhit.
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
python3 "$here/callgraph.py" --graph "$work/build-cg/mirth-sites" --sites "$build/mirth-sites" \
  "${runs[@]}" "${logs[@]}" --gaps "$report/gaps.md" --block-gaps "$report/gaps-blocks.md" --json "$report/gaps.json" "$@" \
  > "$report/coverage-report.txt"
sed -n '1,5p;/^blocks:/p' "$report/coverage-report.txt"
