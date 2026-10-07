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

echo -n "stale-source, incremental rebuild after a comment at the end vs a clean build: "
mkdir -p "$d/s1" "$d/s2"
printf 'pub fn f(a: u32) -> u32 { a }\n' > "$d/dep.rs"
rc --crate-type lib --crate-name dep --emit=metadata,link -Cincremental="$d/i5" --out-dir "$d/s1" "$d/dep.rs"
cp "$d/s1/libdep.rmeta" "$d/before.rmeta"
printf '// a comment at the end\n' >> "$d/dep.rs"
rc --crate-type lib --crate-name dep --emit=metadata,link -Cincremental="$d/i5" --out-dir "$d/s1" "$d/dep.rs"
rc --crate-type lib --crate-name dep --emit=metadata,link -Cincremental="$d/i6" --out-dir "$d/s2" "$d/dep.rs"
if cmp -s "$d/s1/libdep.rmeta" "$d/s2/libdep.rmeta"; then echo same
elif cmp -s "$d/s1/libdep.rmeta" "$d/before.rmeta"; then echo "DIFFER (the previous session's metadata, republished)"
else echo DIFFER; fi
