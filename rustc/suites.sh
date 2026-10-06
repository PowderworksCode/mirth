#!/usr/bin/env bash
# Run rustc's own tests that concern crate metadata against the stage 1
# compiler, and summarize: each suite's result line and every failed test.
#
#   rustc/suites.sh [log]
set -uo pipefail
: "${MIRTH_RUST:?set MIRTH_RUST to the checkout build.sh built}"
log=${1:-$MIRTH_RUST/build/suites.log}
# tests/run-make needs rustdoc, which bootstrap at this commit cannot build
# with the pinned nightly's Cargo: its per-crate build directories hide the
# compiler crates rustdoc links. The suites below do not need rustdoc.
paths=(tests/incremental tests/ui/deprecation tests/ui/crate-loading tests/ui/rmeta
       tests/ui/extern tests/ui/cross-crate)

here=$(cd "$(dirname "$0")" && pwd)
. "$here/pins.env"
host=$(rustc +"$TOOLCHAIN" -vV | sed -n 's/^host: //p')
sysroot_lib=$MIRTH_RUST/build/$host/stage0-sysroot/lib/rustlib/$host/lib
cp "$here/../target/release/libmirth_runtime.rlib" "$sysroot_lib/"

cd "$MIRTH_RUST"
./x.py test --stage 1 --no-fail-fast --force-rerun "${paths[@]}" > "$log" 2>&1
status=$?
echo "existing tests (tests/incremental; tests/ui/{deprecation,crate-loading,rmeta,extern,cross-crate}):"
grep -E "^test result:" "$log" | sed 's/; finished in.*//' | sed 's/^/  /'
failed=$(sed -n '/^failures:$/,/^test result/p' "$log" | grep -E '^    \[' | sed 's/^ *//' | sort -u)
if [ -n "$failed" ]; then
  echo "  failed:"
  echo "$failed" | sed 's/^/    /'
fi
if grep -q "Compiling rustc_metadata" "$log"; then
  echo "  WARNING: the test run recompiled rustc_metadata without mirth-watch;"
  echo "  run rustc/build.sh --again before recording anything"
fi
grep -q "^test result:" "$log" || { echo "  no results: see $log"; tail -5 "$log" | sed 's/^/    /'; }
exit $status
