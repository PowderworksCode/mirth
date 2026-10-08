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

echo -n "inlined-alloc, -Zmir-opt-level=3, a generic fn duplicated, incremental vs clean: "
mkdir -p "$d/ia"
echo 'pub fn f<T>(v: &[T]) -> Option<&[T]> { v.get(..3) }' > "$d/ia/lib.rs"
iaf="--edition 2021 --crate-type lib --crate-name x --emit=metadata,link -Zmir-opt-level=3"
"$rustc" $iaf -Cincremental="$d/i13" --out-dir "$d/ia/o1" "$d/ia/lib.rs" 2> /dev/null
echo 'pub fn g<T>(v: &[T]) -> Option<&[T]> { v.get(..3) }' >> "$d/ia/lib.rs"
"$rustc" $iaf -Cincremental="$d/i13" --out-dir "$d/ia/o1" "$d/ia/lib.rs" 2> /dev/null
"$rustc" $iaf -Cincremental="$d/i14" --out-dir "$d/ia/o2" "$d/ia/lib.rs" 2> /dev/null
cmp -s "$d/ia/o1/libx.rmeta" "$d/ia/o2/libx.rmeta" && echo same || echo DIFFER

echo -n "thinlto-order, -Clto=thin -g with incremental, 30 clean builds of a binary, distinct object sets: "
mkdir -p "$d/to"
cat > "$d/to/up.rs" <<'RS'
pub struct Matrix<const R: usize, const C: usize>(pub [[u32; C]; R]);
impl<const R: usize, const C: usize> Matrix<R, C> {
    pub fn transpose(&self) -> Matrix<C, R> {
        let mut out = [[0; R]; C];
        for i in 0..R { for j in 0..C { out[j][i] = self.0[i][j]; } }
        Matrix(out)
    }
}
#[inline(always)]
pub fn always_inline(v: u32) -> u32 { v.rotate_left(3) }
RS
cat > "$d/to/bin.rs" <<'RS'
use up::Matrix;
pub fn spin(m: &Matrix<2, 3>) -> Matrix<3, 2> { m.transpose() }
fn main() { let m = Matrix([[1,2,3],[4,5,6]]); println!("{}", spin(&m).0[2][1] + up::always_inline(std::env::args().count() as u32)); }
RS
"$rustc" --edition 2021 --crate-type rlib "$d/to/up.rs" -o "$d/to/libup.rlib"
for i in $(seq 1 30); do
  rm -rf "$d/to/o" "$d/to/inc"; mkdir "$d/to/o"
  "$rustc" --edition 2021 -g -Clto=thin -Cincremental="$d/to/inc" -Csave-temps --extern up="$d/to/libup.rlib" \
    -o "$d/to/o/bin" "$d/to/bin.rs" 2> /dev/null
  cat $(ls "$d"/to/o/*.rcgu.o | grep -v 'no-opt\|thin-lto' | sort) | sha256sum
done | sort -u | wc -l

echo -n "no-prepopulate-link, a dylib with -Cno-prepopulate-passes -Zshare-generics=no -Zthinlto=yes: "
mkdir -p "$d/np"
echo 'pub fn f(a: &mut u8, b: &mut u8) { core::mem::swap(a, b) }' > "$d/np/a.rs"
if "$rustc" --edition 2021 --crate-type dylib -Ccodegen-units=16 -Cno-prepopulate-passes -Zshare-generics=no \
    -Zthinlto=yes --out-dir "$d/np" "$d/np/a.rs" > "$d/np/log" 2>&1; then echo links
else echo "fails: $(grep -o 'undefined hidden symbol: [^ ]*' "$d/np/log" | head -1)"; fi

echo -n "print-type-sizes-ice, a session with -Zprint-type-sizes, an edit, a session without: "
mkdir -p "$d/pt"
cat > "$d/pt/lib.rs" <<'RS'
pub async fn inner() -> u32 { 1 }
pub async fn outer() -> u32 { let s = String::from("x"); inner().await + s.len() as u32 }
pub fn make() -> impl std::future::Future<Output = u32> { outer() }
RS
"$rustc" --edition 2021 --crate-type lib -Cincremental="$d/pt/inc" -Zprint-type-sizes --out-dir "$d/pt" "$d/pt/lib.rs" > /dev/null 2>&1
echo 'pub fn g() {}' >> "$d/pt/lib.rs"
if "$rustc" --edition 2021 --crate-type lib -Cincremental="$d/pt/inc" --out-dir "$d/pt" "$d/pt/lib.rs" > "$d/pt/log" 2>&1
then echo builds; else echo "ICE: $(grep -o 'trimmed_def_paths. called[^.]*' "$d/pt/log")"; fi

echo -n "rwpi-segfault, -g -Crelocation-model=rwpi on a static mut: "
echo 'pub static mut M: u32 = 0;' > "$d/rw.rs"
"$rustc" --crate-type lib -g -Crelocation-model=rwpi "$d/rw.rs" -o "$d/rw.rlib" > /dev/null 2>&1
rc=$?; if [ $rc = 0 ]; then echo builds; else echo "exit $rc"; fi
