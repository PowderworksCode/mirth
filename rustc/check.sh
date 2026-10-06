#!/usr/bin/env bash
# Check a fixture with the instrumented compiler:
#
#   P1 P2 P4 P7  `mirth check` on a recorded clean build
#   the list     everything each process did with metadata, P3 among it,
#                against tests/rmeta/<fixture>.txt
#   P5           a second clean build gives the same .rmeta bytes
#   P6           an incremental rebuild after fixtures/<fixture>/edit gives the
#                same .rmeta bytes as a clean build of the edited source
#
#   rustc/check.sh <fixture> [--bless]
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
. "$here/pins.env"
: "${MIRTH_RUST:?set MIRTH_RUST to the checkout build.sh built}"
fixture=${1:?a fixture name}
shift
work=${MIRTH_WORK:-$MIRTH_RUST/build/mirth-work}/$fixture
host=$(rustc +"$TOOLCHAIN" -vV | sed -n 's/^host: //p')
rustc=$MIRTH_RUST/build/$host/stage1/bin/rustc
sites=$MIRTH_RUST/build/mirth-sites
cargo +"$TOOLCHAIN" build -q --release --manifest-path "$repo/Cargo.toml" -p mirth-cli
mirth=$repo/target/release/mirth
failed=0

# Every build is incremental, as Cargo's debug profile is by default: extern
# queries record their dependencies only when incremental compilation is on.
#
# Every build happens in $work/run and is moved aside afterwards: Cargo
# derives each crate's identity from its path, so builds in two directories
# would differ for that reason alone.
fresh() {
  rm -rf "$work/run"
  mkdir -p "$work/run"
  cp -R "$repo/fixtures/$fixture" "$work/run/src"
  rm -f "$work/run/src/edit"
}

build() {
  CARGO_INCREMENTAL=1 "$mirth" record --rustc "$rustc" --out "$work/$1.record" -- \
    cargo +"$TOOLCHAIN" build --manifest-path "$work/run/src/Cargo.toml" --target-dir "$work/run/target"
}

keep() {
  rm -rf "$work/$1"
  mv "$work/run" "$work/$1"
}

rmetas() {
  # The published .rmeta files: not the incremental cache's copies, nor any
  # left in a temporary directory (P7 reports those).
  (cd "$work/$1/target" && find . -path ./debug/incremental -prune -o -name '*.rmeta' -type f \
    -not -path '*/rmeta??????/*' -print | sort | xargs sha256sum)
}

roots=(--root "$work/run/target=target" --root "$work/run/src=."
       --root "$MIRTH_RUST/build/$host/stage1=<sysroot>")

rm -rf "$work"
mkdir -p "$work"

echo "== clean build"
fresh
build a || exit 1
"$mirth" report --sites "$sites" --out "$work/a.record" "${roots[@]}" \
  --expect "$repo/tests/rmeta/$fixture.txt" "$@" || failed=1
"$mirth" check --sites "$sites" --out "$work/a.record" --target-dir "$work/run/target" \
  --allow "$here/allow.txt" "${roots[@]}" || failed=1
keep a

echo "== P5: a second clean build"
fresh
build b || exit 1
keep b
if diff <(rmetas a) <(rmetas b) > "$work/p5.diff"; then
  echo "P5 holds: $(rmetas a | wc -l) .rmeta files identical"
else
  echo "P5 broken: .rmeta files differ between two clean builds"
  cat "$work/p5.diff"
  failed=1
fi

if [ -x "$repo/fixtures/$fixture/edit" ]; then
  echo "== P6: an incremental rebuild after an edit"
  fresh
  build inc-before || exit 1
  (cd "$work/run/src" && "$repo/fixtures/$fixture/edit")
  build inc || exit 1
  keep inc
  fresh
  (cd "$work/run/src" && "$repo/fixtures/$fixture/edit")
  build clean || exit 1
  keep clean
  if diff <(rmetas inc) <(rmetas clean) > "$work/p6.diff"; then
    echo "P6 holds: incremental and clean .rmeta files identical"
  else
    echo "P6 broken: the incremental rebuild's .rmeta files differ from a clean build's"
    cat "$work/p6.diff"
    failed=1
  fi
fi
exit $failed
