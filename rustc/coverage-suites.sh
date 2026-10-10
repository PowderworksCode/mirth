#!/usr/bin/env bash
# Run rustc's own test suites through compiletest with the coverage-instrumented compiler
# (rustc/build.sh with MIRTH_WATCH=rustc/coverage.toml and BUILD_DIR), recording which of the
# compiler's functions each rustc process reaches. Logs are folded as they finish
# (`mirth-lab coverage-compact`), so the disk holds only the union and what each test added.
#
#   MIRTH_RUST=<rust checkout> BUILD_DIR=<instrumented build dir> \
#     rustc/coverage-suites.sh <work dir> <name> <x test paths...> [-- <x test options>]
#
# Writes <work dir>/<name>/{union.txt,added.jsonl,x.log}. Read with `mirth-lab coverage`
# --union <work dir>/*/union.txt.
set -uo pipefail
: "${MIRTH_RUST:?set MIRTH_RUST}"
: "${BUILD_DIR:?set BUILD_DIR to the instrumented build directory}"
here=$(cd "$(dirname "$0")" && pwd)
work=$1; name=$2; shift 2
paths=(); options=()
while [ $# -gt 0 ]; do
  if [ "$1" = -- ]; then shift; options=("$@"); break; fi
  paths+=("$1"); shift
done
out=$work/$name
mkdir -p "$out/logs"
rm -f "$out/done"

# The same as build.sh's, or Cargo rebuilds the compiler without the instrumentation.
export RUSTFLAGS_BOOTSTRAP="-L dependency=$BUILD_DIR/mirth-runtime"
export RUSTFLAGS_NOT_BOOTSTRAP="$RUSTFLAGS_BOOTSTRAP"

# The compactor is a mirth-lab subcommand: build it first (a no-op when up to date).
(cd "$here/.." && cargo build --release -q --offline -p mirth-lab) || exit 1
"$here/../target/release/mirth-lab" coverage-compact --logs "$out/logs" --out "$out" --until "$out/done" > "$out/compact.log" 2>&1 &
compactor=$!

# A test compile running more than five minutes is killed and named.
(
  while sleep 20; do
    ps -eo pid=,etimes=,args= | awk -v sys="$BUILD_DIR/" '$3 ~ /stage[12]\/bin\/rustc$/ && index($3, sys) == 1 && $2 > 300 {print}' |
      while read -r pid _ rustc rest; do
        echo "$rest" | grep -o 'tests/[^ ]*\.rs' | head -1 >> "$out/hung"
        kill "$pid"
      done
  done
) &
watchdog=$!

cd "$MIRTH_RUST"
if [ -n "${WRAPPED:-}" ]; then
  # WRAPPED=1: what the run compiles goes through mirth-watch with the coverage configuration
  # too, as rustc/build.sh does: for the compiler crates' own unit tests, whose test binaries
  # are compiled here.
  MIRTH_OUT="$out/logs" RUSTC_WRAPPER_REAL="$here/../target/release/mirth-watch" \
    MIRTH_RUNTIME="$BUILD_DIR/mirth-runtime/libmirth_runtime.rlib" MIRTH_WATCH="$here/coverage.toml" \
    MIRTH_SITES="$out/sites" \
    ./x.py test --build-dir "$BUILD_DIR" --stage 1 --no-fail-fast --force-rerun "${options[@]}" "${paths[@]}" \
    > "$out/x.log" 2>&1
  status=$?
else
# --keep-stage: never rebuild the compiler here, which would build it without the wrapper.
MIRTH_OUT="$out/logs" ./x.py test --build-dir "$BUILD_DIR" --stage 1 --keep-stage 0 --keep-stage 1 \
  --no-fail-fast --force-rerun "${options[@]}" "${paths[@]}" > "$out/x.log" 2>&1
status=$?
fi
if [ -z "${WRAPPED:-}" ] && grep -q '^ *Compiling rustc_' "$out/x.log"; then
  echo "$name: the compiler was rebuilt, without the instrumentation; results are not coverage" >&2
  status=99
fi
kill "$watchdog" 2>/dev/null
touch "$out/done"
wait "$compactor"
echo "$name: x test exit $status; $(wc -l < "$out/union.txt") sites; $(grep -E '^test result:' "$out/x.log" | sed 's/; finished in.*//' | tr '\n' ' ')"
