#!/usr/bin/env bash
# The three reproductions in hunt.md, with plain rustc (RUSTC, or the pinned
# nightly). Each prints "same" or "DIFFER".
set -u
here=$(cd "$(dirname "$0")" && pwd)
rustc=${RUSTC:-$(rustc +nightly-2026-10-06 --print sysroot)/bin/rustc}
d=$(mktemp -d)
trap 'rm -rf "$d"' EXIT
rc() { "$rustc" --edition 2024 "$@" 2> "$d/err" || { cat "$d/err"; exit 1; }; }

echo -n "threads-rpitit, 8 builds with -Zthreads=8, distinct .rmeta files: "
for i in 1 2 3 4 5 6 7 8; do
  rc --crate-type lib --emit=metadata -Zthreads=8 "$here/threads-rpitit/lib.rs" -o "$d/t$i.rmeta"
done
sha256sum "$d"/t*.rmeta | cut -d' ' -f1 | sort -u | wc -l

echo -n "p6-generics, incremental rebuild after a comment vs a clean build: "
cp "$here/p6-generics/lib.rs" "$d/lib.rs"
rc --crate-type lib --emit=metadata -Cincremental="$d/i1" "$d/lib.rs" -o "$d/inc.rmeta"
{ echo "// a comment"; cat "$here/p6-generics/lib.rs"; } > "$d/lib.rs"
rc --crate-type lib --emit=metadata -Cincremental="$d/i1" "$d/lib.rs" -o "$d/inc.rmeta"
rc --crate-type lib --emit=metadata -Cincremental="$d/i2" "$d/lib.rs" -o "$d/clean.rmeta"
cmp -s "$d/inc.rmeta" "$d/clean.rmeta" && echo same || echo DIFFER

echo -n "p6-literals, incremental rebuild after editing b vs a clean build: "
mkdir -p "$d/o1" "$d/o2"
cp "$here/p6-literals/before.rs" "$d/lib.rs"
rc --crate-type lib --emit=metadata,link -Cincremental="$d/i3" --out-dir "$d/o1" "$d/lib.rs"
cp "$here/p6-literals/after.rs" "$d/lib.rs"
rc --crate-type lib --emit=metadata,link -Cincremental="$d/i3" --out-dir "$d/o1" "$d/lib.rs"
rc --crate-type lib --emit=metadata,link -Cincremental="$d/i4" --out-dir "$d/o2" "$d/lib.rs"
cmp -s "$d/o1/liblib.rmeta" "$d/o2/liblib.rmeta" && echo same || echo DIFFER
