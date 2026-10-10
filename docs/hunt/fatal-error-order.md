# Finding 18: after a fatal error, an incremental rebuild reports fewer errors than a clean build

Facts for a report; the report itself is for a person to write (rust-lang/rust's LLM policy).
Diagnostics only: both builds fail.

**Repro.** `tests/ui/pin-ergonomics/pinned-drop-sugar-no-core.rs` from rustc's suite: build it
with `-Cincremental=inc --emit=metadata`, rename its `#[lang = "legacy_receiver"]` to
`#[lang = "legacy_receiver~"]`, build again incrementally, and build with a fresh incremental
directory.

**Expected.** The same errors.

**Actual.** The clean build reports `requires \`legacy_receiver\` lang_item` once per function
that needs it (four times on the local compiler, plus the unknown lang item); the rebuild
reports it once. Stock nightly-2026-10-06: 2 errors against 5.

**Cause, as far as followed.** `TyCtxt::require_lang_item` reports a missing lang item with
`emit_fatal`, which unwinds out of the query. In a clean build, each body is checked inside a
loop that catches the unwinding per body and continues, so every body reports it. In the
rebuild, a query reached earlier (while checking what can be reused) hits the missing item first,
outside that loop, and the first fatal error ends the session. Any fatal error reached in a
different order would do the same.

**How mirth found it.** `mirth-lab ui-fuzz` over rustc's UI tests, after an edit renamed the lang
item. Such differences (the rebuild's diagnostics a subset of the clean build's, each missing
line repeating a message the rebuild has) are labelled known since.
