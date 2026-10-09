#!/usr/bin/env bash
# Combined function coverage of the compiler: every folded run under $WORK/cov-suites, the sink
# and option-configuration logs, against the call graph's denominator. Writes
# $WORK/coverage-report.txt, $WORK/gaps.md (what can run and did not, by crate and file) and
# $WORK/gaps.json. Extra arguments go to callgraph.py (`--why <function>`, `--unreachable <crate>`).
#
#   WORK=~/mirth-work rustc/coverage-report.sh [--why <function> ...]
#
# Runs of programs outside the compiler that link it (ui-fulldeps, the compiler crates' unit
# tests: directory names containing `fulldeps` or `compiler-unit`) are `--external`: what they
# ran is a root of the call graph, since their own mains call it.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/mirth-work}
runs=()
for f in "$work"/cov-suites/*/union.txt; do
  case $f in
    *fulldeps*|*compiler-unit*) runs+=(--external "$f") ;;
    *) runs+=(--hit "$f") ;;
  esac
done
logs=()
for d in "$work/cov-sink" "$work/cov-flags"; do
  [ -d "$d" ] && logs+=(--logs "$d")
done
python3 "$here/callgraph.py" --graph "$work/build-cg/mirth-sites" --sites "$work/build-cov/mirth-sites" \
  "${runs[@]}" "${logs[@]}" --gaps "$work/gaps.md" --json "$work/gaps.json" "$@" \
  > "$work/coverage-report.txt"
sed -n 1,5p "$work/coverage-report.txt"
