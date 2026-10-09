#!/usr/bin/env bash
# Run rustc's own tests that concern crate metadata against the stage 1
# compiler, and summarize: each suite's result line and every failed test.
#
#   rustc/suites.sh [log]
set -uo pipefail
: "${MIRTH_RUST:?set MIRTH_RUST to the checkout build.sh built}"
log=${1:-$MIRTH_RUST/build/suites.log}
run_make=(
  artifact-incr-cache artifact-incr-cache-no-obj crate-hash-metadata-flag crate-loading
  crate-loading-crate-depends-on-itself crate-loading-multiple-candidates dirty-incr-due-to-hard-link
  embed-metadata emit emit-named-files emit-path-unhashed emit-to-stdout extern-diff-internal-name
  extern-flag-disambiguates extern-flag-fun extern-flag-pathless extern-flag-rename-transitive
  extern-multiple-copies extern-multiple-copies2 extern-overrides-distribution
  extra-filename-with-temp-outputs inaccessible-temp-dir incremental-debugger-visualizer
  incremental-finalize-fail incremental-session-fail incremental-session-gc incr-foreign-head-span
  incr-prev-body-beyond-eof incr-test-moved-file incr-unstable-fingerprint-def-ident-span ls-metadata
  metadata-dep-info metadata-flag-frobs-symbols metadata-only-crate-no-ice metadata-stub-incremental-reuse
  multiple-emits notify-all-emit-artifacts prefer-rlib rlib-chain rmeta-preferred
  rmeta-unrelated-search-path-files symlinked-extern symlinked-rlib
)
paths=(tests/incremental tests/ui/deprecation tests/ui/crate-loading tests/ui/rmeta
       tests/ui/extern tests/ui/cross-crate)
for test in "${run_make[@]}"; do paths+=("tests/run-make/$test"); done

here=$(cd "$(dirname "$0")" && pwd)
# The same as build.sh's: rustdoc, built for run-make, links compiler crates
# that depend on the runtime.
export RUSTFLAGS_BOOTSTRAP="-L dependency=${BUILD_DIR:-$MIRTH_RUST/build}/mirth-runtime"
export RUSTFLAGS_NOT_BOOTSTRAP="$RUSTFLAGS_BOOTSTRAP"

# A test compile that runs for more than five minutes is killed, and named.
hung=$log.hung
: > "$hung"
(
  while sleep 20; do
    ps -eo pid=,etimes=,args= | awk -v sys="$MIRTH_RUST/build/" '$3 ~ /stage1\/bin\/rustc$/ && index($3, sys) == 1 && $2 > 300 {print}' |
      while read -r pid _ rustc rest; do
        # run-make compiles in its output directory, with relative paths.
        name=$(echo "$rest" | grep -o 'tests/[^ ]*\.rs' | head -1)
        [ -n "$name" ] || name=tests/$(readlink "/proc/$pid/cwd" | grep -o 'run-make/[^/]*' | head -1)
        echo "$name" >> "$hung"
        kill "$pid"
      done
  done
) &
watchdog=$!

cd "$MIRTH_RUST"
./x.py test --stage 1 --no-fail-fast --force-rerun "${paths[@]}" > "$log" 2>&1
status=$?
kill "$watchdog" 2>/dev/null
echo "existing tests (tests/incremental; tests/ui/{deprecation,crate-loading,rmeta,extern,cross-crate};"
echo "${#run_make[@]} metadata-related tests/run-make):"
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
if [ -s "$hung" ]; then
  echo "  hung for over five minutes, and killed:"
  sort -u "$hung" | sed 's/^/    /'
fi
grep -q "^test result:" "$log" || { echo "  no results: see $log"; tail -5 "$log" | sed 's/^/    /'; }
exit $status
