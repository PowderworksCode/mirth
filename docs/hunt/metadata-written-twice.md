# Metadata records written twice

Facts for findings 56 and 57. Found by the in-compiler invariant "each metadata record is
written once" (property 15 of [`properties.md`](../properties.md)), checked by
[`check-invariants.patch`](check-invariants.patch) under `RUSTC_CHECK_INVARIANTS`: a table entry
set a second time is reported (`rustc-invariant: metadata-written-once: ...`).

In both cases the second write encodes the same value again at a new position and points the
table entry at it. The first copy stays in the file, unreferenced. Nothing reads the wrong
value; the cost is bytes, and the invariant no longer holds, so a real double write (one that
changes the value) would not stand out.

## 56. The crate root's module children

`encode_def_ids` (`compiler/rustc_metadata/src/rmeta/encoder.rs`) starts with
`self.encode_info_for_mod(CRATE_DEF_ID)`, and its loop over `tcx.iter_local_def_id()` then
reaches the crate root again, whose `def_kind` is `DefKind::Mod`:

```rust
    fn encode_def_ids(&mut self) {
        self.encode_info_for_mod(CRATE_DEF_ID);
        ...
        for local_id in tcx.iter_local_def_id() {
            ...
            if let DefKind::Mod = def_kind {
                self.encode_info_for_mod(local_id);
            }
```

So `module_children_non_reexports`, `module_children_reexports` and `ambig_module_children`
are encoded twice for entry 0. An empty array is the table's default and is not written, so
only crates whose root has children in those tables are affected: every library with a public
item. `pub fn f() {}` alone, as a lib, reports both tables.

UI sweep: 310 of the 18,624 standalone tests (`module_children_non_reexports`), 306
(`module_children_reexports`), 1 (`ambig_module_children`, `entry-point/imported_main_conflict_lib.rs`).

## 57. Coroutine layouts

`encode_mir` records `mir_coroutine_witnesses` inside `if encode_opt { ... }` and again,
unconditionally, at the end of the loop body:

```rust
            if encode_opt {
                ...
                if self.tcx.is_coroutine(def_id.to_def_id())
                    && let Some(witnesses) = tcx.mir_coroutine_witnesses(def_id)
                {
                    record_some_lazy!(self.tables.mir_coroutine_witnesses[def_id.to_def_id()] <- witnesses);
                }
            }
            ...
            if self.tcx.is_coroutine(def_id.to_def_id())
                && let Some(witnesses) = tcx.mir_coroutine_witnesses(def_id)
            {
                record_some_lazy!(self.tables.mir_coroutine_witnesses[def_id.to_def_id()] <- witnesses);
            }
```

Every coroutine (async fn, async block, `gen` block) whose optimized MIR is encoded has its
`CoroutineLayout` written twice. UI sweep: 71 tests.

## Size

A compiler with both double writes removed (crate root skipped in the loop, the first
coroutine write deleted), against the same compiler with them, at the pin:

| crate | `.rmeta` with | without | saved |
|---|---:|---:|---:|
| `std` | 8,002,280 | 8,000,116 | 2,164 |
| `core` | 68,750,338 | 68,748,954 | 1,384 |
| `alloc` | 8,768,651 | 8,768,552 | 99 |
| a 4-line async library (`-O`) | 16,501 | 16,275 | 226 |

With the change, the invariant reports nothing for either case (6 reports to 0 for the async
library, 2 to 0 for `pub fn f() {}`).

## Versions

Both double writes are in `encoder.rs` at 1.80.0, 1.90.0 and 1.98.0 (source read through the
GitHub API; the local checkout is shallow) and at the pin. Searches of rust-lang/rust issues
and PRs on 2026-10-10 found nothing about either.

## Also reported, and intended

Proc-macro crates (6 UI tests): `encode_def_path_table` writes each proc macro's `def_keys`
entry, and `encode_proc_macros` then writes it again with `DefPathData::MacroNs(name)`. The
second write is deliberate (it changes the value); the first `DefKey` stays in the file.
