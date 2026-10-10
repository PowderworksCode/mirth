#!/bin/bash
# Every check of docs/checks.md, one after another, into $WORK/big-<date>/<check>/, from a frozen
# copy of mirth-lab (rebuilding during the run does not affect it). Rerunning with the same
# directory skips checks already done. Summary: <dir>/summary.txt.
#
#   rustc/big-run.sh [<dir>]     JOBS=14 by default; ONLY="diag-check gate-check" for a subset
set -u
here=$(cd "$(dirname "$0")" && pwd)
work=${WORK:-$HOME/mirth-work}
dir=${1:-$work/big-$(date +%F)}
jobs=${JOBS:-14}
pin=nightly-2026-10-06
rustc=$work/campaign/rustc/bin/rustc
checked=$work/build-da/host/stage1/bin/rustc
rust=$work/rust
tests=$rust/tests/ui
mkdir -p "$dir"
(cd "$here/.." && cargo build --release -q --offline -p mirth-lab) || exit 1
lab=$dir/mirth-lab
[ -x "$lab" ] || cp "$here/../target/release/mirth-lab" "$lab"

run() { # name args...
  local name=$1; shift
  if [ -n "${ONLY:-}" ] && [[ " $ONLY " != *" $name "* ]]; then return; fi
  [ -e "$dir/$name/DONE" ] && return
  mkdir -p "$dir/$name"
  echo "$(date +%T) start $name"
  local t0=$SECONDS
  "$lab" "$@" > "$dir/$name/run.log" 2>&1
  echo "exit $? after $((SECONDS - t0))s" > "$dir/$name/DONE"
  echo "$(date +%T) done $name: $(cat "$dir/$name/DONE"), $(ls "$dir/$name/findings" 2>/dev/null | wc -l) findings"
}

sweep=(--tests "$tests" --jobs "$jobs")
run diag-check diag-check --rustc "$rustc" "${sweep[@]}" --work "$dir/diag-check"
run gate-check gate-check --rustc "$rustc" --rust "$rust" --jobs "$jobs" --work "$dir/gate-check"
run solver-diff solver-diff --rustc "$rustc" "${sweep[@]}" --work "$dir/solver-diff"
run crash-diff crash-diff --rustc "$rustc" --checked "$checked" --known "$here/crash-known.txt" "${sweep[@]}" --work "$dir/crash-diff"
run repro-diff repro-diff --rustc "$rustc" "${sweep[@]}" --work "$dir/repro-diff"
run suggest-diff suggest-diff --rustc "$rustc" "${sweep[@]}" --work "$dir/suggest-diff"
run opt-diff opt-diff --rustc "$rustc" --cranelift "$(rustup +$pin which rustc)" "${sweep[@]}" --work "$dir/opt-diff"
run rewrite-diff rewrite-diff --rustc "$rustc" "${sweep[@]}" --work "$dir/rewrite-diff"
run miri-diff miri-diff --rustc "$rustc" --miri-toolchain "$pin" "${sweep[@]}" --work "$dir/miri-diff"
run rustdoc-diff rustdoc-diff --toolchain "$pin" "${sweep[@]}" --work "$dir/rustdoc-diff"
run instr-check instr-check --toolchain "$pin" "${sweep[@]}" --work "$dir/instr-check"
for s in 1 2 3 4 5 6 7 8 9 10; do
  run abi-diff-$s abi-diff --rustc "$rustc" --rust "$rust" --all --count 300 --seed $s --jobs "$jobs" --work "$dir/abi-diff-$s"
done
run scale-check scale-check --rustc "$rustc" --jobs 4 --work "$dir/scale-check"
run release-diff release-diff --corpus "$HOME/proofhouse-repos/rust" --old nightly-2026-07-18 --new "$pin" --jobs 4 --work "$dir/release-diff"
run xlink xlink --toolchain "$pin" --jobs 8 --work "$dir/xlink"

{
  echo "big run $(date +%F) in $dir"
  for d in "$dir"/*/; do
    n=$(basename "$d")
    [ -e "$d/DONE" ] || continue
    printf '%-14s %-22s %4s findings   %s\n' "$n" "$(cat "$d/DONE")" "$(ls "$d/findings" 2>/dev/null | wc -l)" "$(tail -1 "$d/run.log" | cut -c1-120)"
  done
} > "$dir/summary.txt"
cat "$dir/summary.txt"
