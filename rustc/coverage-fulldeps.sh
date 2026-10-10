#!/usr/bin/env bash
# The ui-fulldeps tests compiletest skips at stage 1 (`//@ ignore-stage1`: the stable MIR
# tests among them), run by hand on the coverage-instrumented compiler: compiled with the
# stage 0 compiler against the stage 0 sysroot (where bootstrap puts the stage 1 compiler's
# crates) and mirth's runtime, then run, recording coverage as rustc/coverage-suites.sh does.
#
#   MIRTH_RUST=<rust checkout> BUILD_DIR=<instrumented build dir> rustc/coverage-fulldeps.sh <work dir>
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
host=x86_64-unknown-linux-gnu
out=$1/ui-fulldeps-stage1
mkdir -p "$out/logs" "$out/bin"
rm -f "$out/done"
# The compactor is a mirth-lab subcommand: build it first (a no-op when up to date).
(cd "$here/.." && cargo build --release -q --offline -p mirth-lab) || exit 1
"$here/../target/release/mirth-lab" coverage-compact --logs "$out/logs" --out "$out" --until "$out/done" > "$out/compact.log" 2>&1 &
compactor=$!
pass=0; fail=0
for t in $(grep -l '^//@ ignore-stage1' -r "$MIRTH_RUST/tests/ui-fulldeps" --include=*.rs | grep -v /auxiliary/ | sort); do
  name=$(basename "$t" .rs)
  edition=$(sed -n 's,^//@ edition: *\([0-9]*\).*,\1,p' "$t" | head -1)
  flags=$(sed -n 's,^//@ compile-flags: *,,p' "$t" | tr '\n' ' ')
  if "$MIRTH_RUST/stage0/bin/rustc" "$t" --sysroot "$BUILD_DIR/$host/stage0-sysroot" \
       -L "dependency=$BUILD_DIR/mirth-runtime" --edition "${edition:-2015}" -Crpath $flags \
       -o "$out/bin/$name" > "$out/bin/$name.compile" 2>&1 \
     && grep -q '^//@ run-pass' "$t"; then
    (cd "$(dirname "$t")" && MIRTH_OUT="$out/logs" timeout 300 "$out/bin/$name" > "$out/bin/$name.run" 2>&1) && pass=$((pass + 1)) || fail=$((fail + 1))
  elif [ -f "$out/bin/$name" ]; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
  fi
done
touch "$out/done"
wait "$compactor"
echo "ui-fulldeps (stage 1 by hand): $pass passed, $fail failed; $(wc -l < "$out/union.txt") sites"
