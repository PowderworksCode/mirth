# Warnings from inline assembly disappear when an incremental rebuild reuses the codegen unit

<!-- Draft issue for rust-lang/rust. Seen on 1.60.0 through nightly-2026-10-06. -->

The assembler's warnings about an `asm!` block are reported while LLVM compiles the codegen
unit that contains it. When an incremental rebuild reuses that unit's object code, LLVM does
not compile it, and the warning is not shown. Unlike warnings from queries, which incremental
compilation stores and shows again, warnings from LLVM are not stored. A clean build of the
same source shows the warning.

### Reproduction

```rust
// main.rs
mod m;
fn main() {
    m::f();
}
```

```rust
// m.rs
pub fn f() {
    unsafe { std::arch::asm!(".warning \"from the assembler\"") }
}
```

```sh
rustc --edition 2021 -C codegen-units=4 -C incremental=incr       -o rebuilt main.rs   # the warning
echo "// a comment at the end" >> m.rs
rustc --edition 2021 -C codegen-units=4 -C incremental=incr       -o rebuilt main.rs   # nothing
rustc --edition 2021 -C codegen-units=4 -C incremental=clean-incr -o clean   main.rs   # the warning
```

The first build and the clean build print:

```text
warning: from the assembler
 --> m.rs:2:31
  |
2 |     unsafe { std::arch::asm!(".warning \"from the assembler\"") }
  |                               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

The rebuild prints nothing. I expected the rebuild to print the same warning as the clean
build, as it does for warnings from rustc itself.

The same happens on 1.60.0, 1.65.0, 1.70.0, 1.75.0 and 1.90.0, and at `-C opt-level=2`.

### Why

Diagnostics emitted inside a query are saved as side effects and replayed when the query's
result is reused. Diagnostics from the LLVM backend are emitted through the codegen
coordinator's `SharedEmitter` while a module is compiled, and a work product (the `.o`, and
the `.bc` for ThinLTO) records nothing of them. A reused unit is never compiled, so its
warnings are lost, for as long as the unit stays reused.

Inline assembly is the case found here; any other warning LLVM reports while compiling a
module would be lost the same way.

### Possible fixes

Record the diagnostics emitted while compiling a module with its work product and emit them
again when the work product is reused, or treat a unit whose compilation warned as not
reusable.

### How it was found

[mirth](https://github.com/PowderworksCode/mirth) checks reused codegen units against a fresh
codegen of the same unit. The only differences on its test workspace were `srcloc` cookies on
inline assembly, which led to looking at what inline-assembly diagnostics do across a reuse.
