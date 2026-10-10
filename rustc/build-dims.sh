#!/usr/bin/env bash
# The compiler for coverage beyond blocks (docs/coverage-plan.md): rustc/build.sh with
# rustc/coverage-dims.toml into its own build directory, from its own source tree, then std with
# the profiler runtime (as docs/coverage-handoff.md does for build-cov).
#
# The source tree is a worktree of the pinned rustc with the campaign's patches
# (docs/hunt/*.patch, as rustc-verify12) and the two coverage patches
# (docs/hunt/coverage-dims.patch: RUSTC_PASS_EFFECT, RUSTC_LOCK_CONTENTION). Its large submodules
# are links into the main checkout's, and bootstrap is told not to manage submodules.
#
#   DIMS_RUST=~/mirth-work/rust-cov2 BUILD_DIR=~/mirth-work/build-cov2 rustc/build-dims.sh [--again]
#
# --again after changing mirth-watch or the configuration: the compiler crates are compiled again
# through the wrapper (Cargo does not know either changed).
set -euxo pipefail
here=$(cd "$(dirname "$0")" && pwd)
W=${WORK:-$HOME/mirth-work}
export MIRTH_RUST=${DIMS_RUST:-$W/rust-cov2}
build=${BUILD_DIR:-$W/build-cov2}
MIRTH_WATCH=$here/coverage-dims.toml BUILD_DIR=$build JOBS=${JOBS:-8} "$here/build.sh" "$@"
cd "$MIRTH_RUST"
RUSTFLAGS_BOOTSTRAP="-L dependency=$build/mirth-runtime" \
RUSTFLAGS_NOT_BOOTSTRAP="-L dependency=$build/mirth-runtime" \
RUSTC_WRAPPER_REAL=$here/../target/release/mirth-watch \
MIRTH_RUNTIME=$build/mirth-runtime/libmirth_runtime.rlib \
MIRTH_WATCH=$here/coverage-dims.toml MIRTH_SITES=$build/mirth-sites \
./x.py build --build-dir "$build" --stage 1 compiler/rustc library --set build.profiler=true -j "${JOBS:-8}"
echo "dims compiler: $build/host/stage1/bin/rustc"
