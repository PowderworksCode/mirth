#!/usr/bin/env bash
# Fetch the pinned rustc commit into $MIRTH_RUST and configure bootstrap to
# build it quickly, with the pinned nightly as stage 0.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
. "$here/pins.env"
: "${MIRTH_RUST:?set MIRTH_RUST to a directory for the rustc checkout}"

host=$(rustc +"$TOOLCHAIN" -vV | sed -n 's/^host: //p')
toolchain=$(rustc +"$TOOLCHAIN" --print sysroot)

# Stage 0 is a copy of the pinned nightly without its rustc-dev component.
# Bootstrap copies stage 0's libraries into the sysroot it builds the
# compiler against, and prebuilt compiler crates there would be picked over
# the ones being built. mirth-watch still uses the toolchain's own rustc-dev.
stage0_root=$MIRTH_RUST/stage0
if [ ! -x "$stage0_root/bin/rustc" ]; then
  rm -rf "$stage0_root"
  cp -a "$toolchain" "$stage0_root"
  sed -n 's/^file://p' "$stage0_root"/lib/rustlib/manifest-rustc-dev-* |
    while IFS= read -r file; do rm -f "$stage0_root/$file"; done
  rm -f "$stage0_root"/lib/rustlib/manifest-rustc-dev-*
fi
stage0=$stage0_root/bin

if [ ! -d "$MIRTH_RUST/.git" ]; then
  git init -q "$MIRTH_RUST"
  git -C "$MIRTH_RUST" remote add origin https://github.com/rust-lang/rust
fi
git -C "$MIRTH_RUST" fetch -q --depth 1 origin "$RUST_COMMIT"
git -C "$MIRTH_RUST" checkout -q -f "$RUST_COMMIT"

cat > "$MIRTH_RUST/bootstrap.toml" <<TOML
change-id = "ignore"

[build]
build = "$host"
rustc = "$stage0/rustc"
cargo = "$stage0/cargo"
docs = false
extended = false
tools = []

[llvm]
download-ci-llvm = true

[rust]
channel = "nightly"
debug = false
optimize = true
incremental = false
debug-assertions = false
overflow-checks = false
codegen-units = 16
deny-warnings = false
TOML
echo "rustc $RUST_COMMIT ready at $MIRTH_RUST"
