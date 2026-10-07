# Incremental rebuilds reuse object code whose debuginfo has the previous checksum (and embedded source) of an edited file

<!-- Draft issue for rust-lang/rust. Since 1.44 (#69718, checksums); -Zembed-source since #126985. -->

A codegen unit's debuginfo names each source file with a checksum of its contents
(`DIFile`, added in #69718 so that "a debugger can verify that the source code matches the
executable"), and with `-Zembed-source` the contents themselves. Both are read straight from
the session's source map (`file_metadata` in `rustc_codegen_llvm/src/debuginfo/metadata.rs`),
which nothing tracks. When an edit leaves a codegen unit green (a comment added at the end
of a file, for instance), the incremental rebuild reuses the unit's object code, and with
it the previous session's checksum and source. A clean build of the same source has the
new ones.

### Reproduction

With `-Zembed-source`, the stale file is visible directly:

```sh
printf 'pub fn f(x: u32) -> u32 {\n    x ^ 7\n}\n' > lib.rs
F="--crate-type lib -Cdebuginfo=2 -Zdwarf-version=5 -Zembed-source=yes -Cembed-bitcode=no"
rustc $F -C incremental=incr  --out-dir rebuilt lib.rs
echo "// added after the first build" >> lib.rs
rustc $F -C incremental=incr  --out-dir rebuilt lib.rs
rustc $F -C incremental=clean --out-dir clean   lib.rs
for d in rebuilt clean; do (mkdir $d/x && cd $d/x && ar x ../liblib.rlib && llvm-dwarfdump --debug-line *.o | grep source:); done
```

On `nightly-2026-10-06` the rebuilt object embeds `lib.rs` as it was before the edit:

```text
rebuilt:  source: "pub fn f(x: u32) -> u32 {\n    x ^ 7\n}\n"
clean:    source: "pub fn f(x: u32) -> u32 {\n    x ^ 7\n}\n// added after the first build\n"
```

Without `-Zembed-source` only the checksum is stale. It is in the LLVM IR, so it shows in the
bitcode `rustc` embeds in rlib objects by default: the same steps without
`-Zdwarf-version=5 -Zembed-source=yes -Cembed-bitcode=no` give objects whose embedded
bitcode, disassembled, differs only in the MD5 of `lib.rs` (the old one in the rebuilt
object, the current one in the clean build) and in the module hash computed from it. (DWARF line tables on Linux carry no MD5s, because the compile unit's own file entry
has none and LLVM emits them for all files or none.)

It also changes executables built with optimizations, through ThinLTO: the `.llvm.<hash>`
suffix of a promoted symbol is a hash of its module's bitcode, checksum included. With this
`main.rs`:

```rust
mod a {
    #[inline(never)]
    pub fn get(v: &[u32], i: usize) -> u32 {
        v[i] * 3
    }
}

mod b {
    pub fn run(v: &[u32]) -> u32 {
        crate::a::get(v, 1) + v[0]
    }
}

fn main() {
    let v: Vec<u32> = std::env::args().map(|a| a.len() as u32).collect();
    println!("{}", b::run(&v));
}
```

```sh
F="--edition 2021 -Copt-level=2 -Cdebuginfo=2 -Ccodegen-units=4"
rustc $F -C incremental=incr  -o rebuilt main.rs
echo "// a comment at the end" >> main.rs
rustc $F -C incremental=incr  -o rebuilt main.rs
rustc $F -C incremental=clean -o clean   main.rs
diff <(nm rebuilt | awk '{print $3}' | sort) <(nm clean | awk '{print $3}' | sort)
```

```text
< anon.a976517f1c32298f1e683e92c2a4747a.0.llvm.15415811769069898958
< anon.a976517f1c32298f1e683e92c2a4747a.1.llvm.15415811769069898958
---
> anon.a976517f1c32298f1e683e92c2a4747a.0.llvm.4724874333102837038
> anon.a976517f1c32298f1e683e92c2a4747a.1.llvm.4724874333102837038
```

The code, data and debug sections are identical; only those names differ. Two clean builds
agree, and with `-Cdebuginfo=0` the rebuild agrees with the clean build.

I expected the rebuilt objects to equal the clean build's, as they do when the same edit
is made with debuginfo off.

The Cranelift backend reads the same fields (`debuginfo/line_info.rs` in
`rustc_codegen_cranelift`) and gives the same result: with `-Zcodegen-backend=cranelift` added
to the first reproduction, the rebuilt object embeds the old `lib.rs` and the clean one the
current file.

### Consequences

- With `-Zembed-source`, a debugger that shows embedded source shows a file that no longer
  exists. With several codegen units, one binary can embed different versions of the same
  file, depending on which units were reused.
- The checksum exists so a debugger can tell whether the source on disk matches the binary.
  A reused unit claims the old file, so a debugger that checks it would reject the current
  file for code built from it. CodeView (MSVC targets) carries these checksums
  (#113707 made them SHA256); I have not tried this on Windows.
- Incremental builds are not reproducible against clean builds once ThinLTO runs (any
  `opt-level` above 0 with more than one codegen unit), which is the default for a profile
  with optimizations and incremental compilation on.

### Since when

Checksums came with #69718 (1.44). Bitcode embedded in rlibs by default came in 1.45;
from 1.45 on, the reproduction above without `-Zembed-source` gives a rebuilt object with the
old MD5 (checked on 1.44.0, 1.45.0, 1.46.0, 1.47.0, 1.55.0, 1.90.0 and the nightly). The
executable-level difference depends on what ThinLTO promotes; it appears on 1.47.0, 1.51.0,
1.60.0 to 1.90.0 and the nightly, and not on 1.55.0. `-Zembed-source` came with #126985.

### Possible fixes

The codegen unit's dependency node would have to depend on the contents of each file its
debuginfo names. That makes every unit with code from a file red whenever the file changes,
comments included, which costs codegen reuse; with `-Zembed-source` there is no way around
it. For the checksum alone, a cheaper option might be to leave it out of debuginfo in
incremental sessions, or to fix it up when a unit is reused, but either changes what
incremental builds emit. A query fingerprinting one source file's contents, read by
`file_metadata`, would make the dependency explicit, as the candidate fix for the same
problem in reused metadata does ([report](issue-stale-metadata-reuse.md)).

### A test

`tests/run-make/incr-debuginfo-embedded-source` (in mirth at
`docs/hunt/tests/incr-debuginfo-embedded-source/rmake.rs`) builds a binary with
`-Zembed-source`, adds a comment at the end of `main.rs`, rebuilds incrementally and builds
clean, and checks that both embed the edited file. On `ea137335b` it fails:

```text
the incremental rebuild embeds a different main.rs from a clean build: ["fn main() {\n    println!(\"{}\", 7);\n}\n"]
```

### How it was found

[mirth](https://github.com/PowderworksCode/mirth) fuzzes edits on a five-crate workspace
and compares every incremental rebuild's artifacts with a clean build's. At `-Copt-level=2`
a third of the rebuilds gave binaries that differed only in `.llvm.<hash>` suffixes; the
modules behind them had pre-LTO bitcode that differed only in a `DIFile` checksum.
