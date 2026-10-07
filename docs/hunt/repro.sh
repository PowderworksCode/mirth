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

echo -n "p6-expansions, incremental rebuild of user after a new variant vs a clean build: "
x=$here/p6-expansions
mkdir -p "$d/pm" "$d/c1" "$d/c2" "$d/o1" "$d/o2"
rc --crate-type proc-macro --extern proc_macro --crate-name wide_derive "$x/derive.rs" --out-dir "$d/pm"
rc --crate-type lib --crate-name wide_core "$x/core-before.rs" --out-dir "$d/c1"
rc --crate-type lib --crate-name wide_core "$x/core-after.rs" --out-dir "$d/c2"
user() {
  cp "$1" "$d/user.rs"
  rc --crate-type lib --crate-name wide_user "$d/user.rs" --emit=dep-info,metadata,link \
    --extern wide_core="$2/libwide_core.rlib" --extern wide_derive="$d/pm/libwide_derive.so" \
    -Cincremental="$3" --out-dir "$4"
}
user "$x/user-before.rs" "$d/c1" "$d/i3" "$d/o1"
user "$x/user-after.rs" "$d/c2" "$d/i3" "$d/o1"
user "$x/user-after.rs" "$d/c2" "$d/i4" "$d/o2"
cmp -s "$d/o1/libwide_user.rmeta" "$d/o2/libwide_user.rmeta" && echo same || echo DIFFER
