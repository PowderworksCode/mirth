#!/usr/bin/env bash
# The reproductions in hunt.md, with plain rustc (RUSTC, or the pinned
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

echo -n "stale-debuginfo, embedded source after a comment at the end, incremental vs clean: "
mkdir -p "$d/e1" "$d/e2"
printf 'pub fn f(x: u32) -> u32 {\n    x ^ 7\n}\n' > "$d/e.rs"
ef="--crate-type lib --crate-name e -Cdebuginfo=2 -Zdwarf-version=5 -Zembed-source=yes -Cembed-bitcode=no"
rc $ef -Cincremental="$d/i7" --out-dir "$d/e1" "$d/e.rs"
echo "// added after the first build" >> "$d/e.rs"
rc $ef -Cincremental="$d/i7" --out-dir "$d/e1" "$d/e.rs"
rc $ef -Cincremental="$d/i8" --out-dir "$d/e2" "$d/e.rs"
n1=$(cd "$d/e1" && ar p libe.rlib | grep -a -c "added after the first build")
n2=$(cd "$d/e2" && ar p libe.rlib | grep -a -c "added after the first build")
if [ "$n1" = "$n2" ]; then echo same; else echo "DIFFER (the rebuilt object embeds the file as it was)"; fi

echo -n "asm-warning, a warning from inline assembly after a comment at the end, incremental vs clean: "
mkdir -p "$d/w"
printf 'mod m;\nfn main() {\n    m::f();\n}\n' > "$d/w/main.rs"
printf 'pub fn f() {\n    unsafe { std::arch::asm!(".warning \\"from the assembler\\"") }\n}\n' > "$d/w/m.rs"
wf="--crate-type bin -Ccodegen-units=4"
"$rustc" --edition 2024 $wf -Cincremental="$d/i9" -o "$d/w/a" "$d/w/main.rs" 2> /dev/null
echo "// a comment at the end" >> "$d/w/m.rs"
n1=$("$rustc" --edition 2024 $wf -Cincremental="$d/i9" -o "$d/w/a" "$d/w/main.rs" 2>&1 | grep -c "from the assembler")
n2=$("$rustc" --edition 2024 $wf -Cincremental="$d/i10" -o "$d/w/b" "$d/w/main.rs" 2>&1 | grep -c "from the assembler")
if [ "$n1" = "$n2" ]; then echo same; else echo "DIFFER (the rebuild shows no warning)"; fi

echo -n "no-leak-check, a session with -Zno-leak-check then one without, vs a clean build: "
mkdir -p "$d/nl"
cat > "$d/nl/lib.rs" <<'RS'
fn foo(x: for<'a, 'b> fn(&'a u8, &'b u8) -> &'a u8, y: for<'a> fn(&'a u8, &'a u8) -> &'a u8) {
    let z = match 22 {
        0 => y,
        _ => x,
    };
}
RS
"$rustc" --crate-type lib -Cincremental="$d/i11" -Zno-leak-check --out-dir "$d/nl" "$d/nl/lib.rs" 2> /dev/null
r1=$("$rustc" --crate-type lib -Cincremental="$d/i11" --out-dir "$d/nl" "$d/nl/lib.rs" > /dev/null 2>&1; echo $?)
r2=$("$rustc" --crate-type lib -Cincremental="$d/i12" --out-dir "$d/nl" "$d/nl/lib.rs" > /dev/null 2>&1; echo $?)
if [ "$r1" = "$r2" ]; then echo same; else echo "DIFFER (the rebuild exits $r1, a clean build $r2)"; fi

echo -n "inlined-alloc, -Zmir-opt-level=3 with debuginfo, a duplicated generic fn, incremental vs clean: "
mkdir -p "$d/ia"
printf 'pub fn first_n<const N: usize>(v: &[u8]) -> Option<[u8; N]> {\n    v.get(..N)?.try_into().ok()\n}\n' > "$d/ia/lib.rs"
iaf="--edition 2021 --crate-type lib --crate-name x --emit=metadata,link -Zmir-opt-level=3 -Cdebuginfo=2"
"$rustc" $iaf -Cincremental="$d/i13" --out-dir "$d/ia/o1" "$d/ia/lib.rs" 2> /dev/null
printf 'pub fn first_m<const N: usize>(v: &[u8]) -> Option<[u8; N]> {\n    v.get(..N)?.try_into().ok()\n}\n' >> "$d/ia/lib.rs"
"$rustc" $iaf -Cincremental="$d/i13" --out-dir "$d/ia/o1" "$d/ia/lib.rs" 2> /dev/null
"$rustc" $iaf -Cincremental="$d/i14" --out-dir "$d/ia/o2" "$d/ia/lib.rs" 2> /dev/null
cmp -s "$d/ia/o1/libx.rmeta" "$d/ia/o2/libx.rmeta" && echo same || echo DIFFER
