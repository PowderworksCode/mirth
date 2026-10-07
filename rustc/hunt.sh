#!/usr/bin/env bash
# Look for bugs in the unmodified compiler with a fixture, beyond check.sh:
#
#   threads  REPEAT clean builds with -Zthreads=8 give the same .rmeta bytes
#   edits    for each script in fixtures/<fixture>/edits, an incremental
#            rebuild after it gives the same .rmeta bytes as a clean build,
#            with -Zincremental-verify-ich, single-threaded and with
#            -Zthreads=8; and the rebuild's record says which crates
#            encoded their metadata again
#
#   rustc/hunt.sh <fixture>
#
# REPEAT sets the number of threaded builds (8); HUNT_EDITS=0 skips the edits.
#
# Nothing here is blessed: every finding is printed, and the exit status is
# 1 if there was one.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
. "$here/pins.env"
: "${MIRTH_RUST:?set MIRTH_RUST to the checkout build.sh built}"
fixture=${1:?a fixture name}
repeat=${REPEAT:-8}
work=${MIRTH_WORK:-$MIRTH_RUST/build/mirth-work}/hunt-$fixture
host=$(rustc +"$TOOLCHAIN" -vV | sed -n 's/^host: //p')
rustc=$MIRTH_RUST/build/$host/stage1/bin/rustc
sites=$MIRTH_RUST/build/mirth-sites
cargo +"$TOOLCHAIN" build -q --release --manifest-path "$repo/Cargo.toml" -p mirth-cli
mirth=$repo/target/release/mirth
found=0

# As in check.sh: every build is incremental and happens in $work/run.
fresh() {
  rm -rf "$work/run"
  mkdir -p "$work/run"
  cp -R "$repo/fixtures/$fixture" "$work/run/src"
  rm -rf "$work/run/src/edit" "$work/run/src/edits"
}

build() {
  CARGO_INCREMENTAL=1 RUSTFLAGS="$flags" "$mirth" record --rustc "$rustc" --out "$work/$1.record" -- \
    cargo +"$TOOLCHAIN" build -q --manifest-path "$work/run/src/Cargo.toml" --target-dir "$work/run/target" \
    > "$work/$1.log" 2>&1 || { echo "the build failed:"; tail -30 "$work/$1.log"; return 1; }
}

keep() {
  rm -rf "$work/$1"
  mv "$work/run" "$work/$1"
}

rmetas() {
  (cd "$work/$1/target" && find . -path ./debug/incremental -prune -o -name '*.rmeta' -type f \
    -not -path '*/rmeta??????/*' -print | sort | xargs sha256sum)
}

roots=(--root "$work/run/target=target" --root "$work/run/src=."
       --root "$MIRTH_RUST/build/$host/stage1=<sysroot>")

# The crates whose rebuild encoded their metadata again, rather than reusing
# the incremental cache's copy.
encoded() {
  "$mirth" report --sites "$sites" --out "$work/$1.record" "${roots[@]}" 2> /dev/null |
    awk '/^== / {crate = $2} /encode-to .*full\.rmeta/ {print crate}' | sort -u | tr '\n' ' '
}

rm -rf "$work"
mkdir -p "$work"

echo "== $repeat clean builds with -Zthreads=8"
flags=-Zthreads=8
for i in $(seq 1 "$repeat"); do
  fresh
  build "t$i" || exit 1
  keep "t$i"
  if [ "$i" -gt 1 ] && ! diff <(rmetas t1) <(rmetas "t$i") > "$work/t$i.diff"; then
    echo "build $i differs from build 1:"
    cat "$work/t$i.diff"
    found=1
  fi
done
[ $found = 0 ] && echo "all $repeat identical: $(rmetas t1 | wc -l) .rmeta files"

[ "${HUNT_EDITS:-1}" = 0 ] && exit $found
for edit in "$repo/fixtures/$fixture/edits"/*; do
  [ -x "$edit" ] || continue
  name=$(basename "$edit")
  for flags in "-Zincremental-verify-ich" "-Zincremental-verify-ich -Zthreads=8"; do
    echo "== $name ($flags)"
    fresh
    build before || { found=1; continue; }
    (cd "$work/run/src" && "$edit")
    build inc || { found=1; continue; }
    echo "encoded again: $(encoded inc)"
    keep inc
    fresh
    (cd "$work/run/src" && "$edit")
    build clean || { found=1; continue; }
    keep clean
    if diff <(rmetas inc) <(rmetas clean) > "$work/$name.diff"; then
      echo "P6 holds"
    else
      echo "P6 broken: the incremental rebuild's .rmeta files differ from a clean build's"
      cat "$work/$name.diff"
      found=1
    fi
  done
done
exit $found
