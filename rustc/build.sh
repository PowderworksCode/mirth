#!/usr/bin/env bash
# Build a stage 1 rustc with mirth-watch instrumenting it, as rmeta.toml says,
# and the standard library for it.
#
# Bootstrap's rustc shim runs RUSTC_WRAPPER_REAL as `<wrapper> <rustc> <args…>`,
# the shape Cargo gives a rustc-wrapper. Each instrumented crate's site table
# lands in $MIRTH_RUST/build/mirth-sites; `mirth report` reads them to name
# what the logs record.
#
# Cargo does not know the wrapper or the configuration changed, so after
# changing either, rebuild the crates in scope:
#
#   rustc/build.sh --again
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
. "$here/pins.env"
: "${MIRTH_RUST:?set MIRTH_RUST to the checkout setup.sh made}"
watch=${MIRTH_WATCH:-$here/rmeta.toml}
jobs=${JOBS:-8}
# A separate build directory keeps an instrumented compiler apart from a plain one.
build=${BUILD_DIR:-$MIRTH_RUST/build}
host=$(rustc +"$TOOLCHAIN" -vV | sed -n 's/^host: //p')

cargo +"$TOOLCHAIN" build --release --manifest-path "$repo/Cargo.toml" -p mirth-watch
out=$repo/target/release
# rustc on its own embeds the runtime's full metadata in the rlib, so it can
# be injected as one file. (Cargo would put it in a separate .rmeta.)
rustc +"$TOOLCHAIN" --edition 2024 --crate-type rlib --crate-name mirth_runtime -O \
  "$repo/crates/mirth-runtime/src/lib.rs" --out-dir "$out"

if [ "${1:-}" = --again ]; then
  # Forget the fingerprints of the crates in scope, so Cargo compiles them
  # again through the wrapper.
  for krate in $(sed -n 's/^crates *= *\[\(.*\)\]/\1/p' "$watch" | tr -d '" ' | tr ',' ' '); do
    rm -rf "$build/$host/stage1-rustc/$host/release/build/$krate"/*/fingerprint
  done
fi

# Every crate downstream of an instrumented one needs the runtime: instrumenting
# tempfile makes most of the compiler depend on it, and so does rustdoc, which
# bootstrap compiles without the wrapper. Bootstrap passes RUSTFLAGS_BOOTSTRAP or
# RUSTFLAGS_NOT_BOOTSTRAP depending on the stage, and at this commit the compiler
# and rustdoc get the second, so both are set. They must be the same in every
# x.py run (suites.sh sets them too), or Cargo recompiles the compiler, and
# without the wrapper.
export RUSTFLAGS_BOOTSTRAP="-L dependency=$out"
export RUSTFLAGS_NOT_BOOTSTRAP="-L dependency=$out"

cd "$MIRTH_RUST"
env RUSTC_WRAPPER_REAL="$out/mirth-watch" \
    MIRTH_RUNTIME="$out/libmirth_runtime.rlib" \
    MIRTH_WATCH="$watch" \
    MIRTH_SITES="$build/mirth-sites" \
    ./x.py build --build-dir "$build" --stage 1 compiler/rustc -j "$jobs"
./x.py build --build-dir "$build" --stage 1 library -j "$jobs"
echo "instrumented rustc: $build/$host/stage1/bin/rustc"
echo "sites: $build/mirth-sites"
