# Finding 16: with cached derive expansions and the HIR crate hash, a rebuild gets a different crate hash

Facts so far; the report itself is for a person to write (rust-lang/rust's LLM policy).
Not root-caused.

**Repro (on `fixtures/sink`).** Build the workspace with
`-Zcache-proc-macros=yes -Zmetadata-crate-hash=no` and `CARGO_INCREMENTAL=1`, add an item to
`core/src/errors.rs` (`pub static FUZZ_STATIC_0: [u8; 3] = [0 as u8, 1, 2];`), build again,
then build clean at the same path.

**Expected.** `libsink_mid.rmeta` the same in both.

**Actual.** It differs in 16 bytes at offset 25, the crate hash (SVH) in the metadata header;
every table is the same size (`-Zmeta-stats`). `sink-mid` derives with `sink-macros` and
depends on `sink-core`, where the edit is.

**Conditions.** Both options are needed (delta minimization of a fuzzer row's 130 options).
`-Zcache-proc-macros` describes itself as "potentially unsound"; `-Zmetadata-crate-hash=no`
reverts to computing the crate hash from the HIR. Since the crate hash is what dependents
compare, a wrong one can make them reuse or reject the wrong things.

**How mirth found it.** The fuzzer under walk configurations (`mirth-lab flag-fuzz`), first
edit of a row; excluded from the models since (a known bug without a stopgap).
