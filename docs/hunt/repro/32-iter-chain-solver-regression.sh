#!/usr/bin/env bash
# Finding 32: a chain of iterator adapters long enough to exceed the recursion limit (128 here)
# is rejected by every toolchain. Under the new trait solver since nightly-2026-08-04 (nightly's
# default solver since then) the rejection takes ~10x the CPU time and ~4x the memory, and
# reports ~75 errors (mostly E0320, "overflow while computing layout") and ~200 overflow
# warnings, where older compilers report one E0275 and stop. Programs below the limit are not
# slower (the new solver is faster there). Bisected over nightlies to 2026-08-03 (good) ..
# 2026-08-04 (bad); the only trait-solver PR in that range is rust-lang/rust#160254 (not
# confirmed by a build with it reverted).
#
# Found by mirth's scaling check (`mirth-lab scale-check`, the `iter-chain` shape), which compiles
# size-parameterized programs; it timed out at N=200. Not found in a public crate (the program
# is generated).
#
# Needs: rustup, cargo, GNU time (/usr/bin/time). Installs the toolchains if missing.
set -euo pipefail
GOOD=nightly-2026-08-03
BAD=nightly-2026-08-04
for tc in $GOOD $BAD; do rustup toolchain install --profile minimal "$tc" >/dev/null 2>&1; done

dir=$(mktemp -d)
cd "$dir"
cargo +$BAD init -q --bin --edition 2021 --name chain

chain() { # n -> src/main.rs with n .map() calls
  {
    printf 'fn main() {\n    let s: u64 = (0u64..10)\n'
    for ((i = 0; i < $1; i++)); do printf '        .map(|x| x.wrapping_add(%d))\n' "$i"; done
    printf '        .sum();\n    println!("{}", s);\n}\n'
  } > src/main.rs
}

build() { # toolchain -> "user-seconds max-rss-MB exit errors overflow-warnings"
  rm -rf target
  local status=0
  RUSTFLAGS="-Znext-solver=globally" /usr/bin/time -o time.txt -f "%U %M" \
    cargo +"$1" build -q 2> stderr.txt || status=$?
  read -r t m < <(tail -1 time.txt)
  echo "$t $((m / 1024)) $status $(grep -cE '^error(\[|:)' stderr.txt) $(grep -c '^warning: overflow' stderr.txt)"
}

report() { printf '  %-20s %7s s CPU %6s MB  exit %3s  %3s error lines  %3s overflow warnings\n' "$@"; }

echo "== 126 maps (under the limit: compiles on both)"
chain 126
read -r t m s e w < <(build $GOOD); report $GOOD "$t" "$m" "$s" "$e" "$w"
read -r t m s e w < <(build $BAD); report $BAD "$t" "$m" "$s" "$e" "$w"

echo "== 200 maps (over the limit: rejected by both)"
chain 200
read -r gt gm gs ge gw < <(build $GOOD); report $GOOD "$gt" "$gm" "$gs" "$ge" "$gw"
read -r bt bm bs be bw < <(build $BAD); report $BAD "$bt" "$bm" "$bs" "$be" "$bw"
echo "  first errors on $BAD:"; grep -E '^error\[' stderr.txt | sort | uniq -c | sort -rn | head -3

# Expect (this machine, idle): 126 maps ~2 s on both. 200 maps: GOOD ~5 s, 2 error lines
# (E0275 and "aborting"); BAD ~40 s, ~2.4 GB, ~75 error lines, ~200 overflow warnings.
if awk -v g="$gt" -v b="$bt" 'BEGIN { exit !(b > 4 * g) }' && [ "$be" -gt $((4 * ge)) ]; then
  echo "REPRODUCED: rejecting the 200-map chain takes $(awk -v g="$gt" -v b="$bt" 'BEGIN { printf "%.1f", b / g }')x the CPU and $be error lines instead of $ge on $BAD."
else
  echo "NOT REPRODUCED"
fi
echo "(work directory: $dir)"
