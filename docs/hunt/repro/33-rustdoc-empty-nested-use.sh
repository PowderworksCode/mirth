#!/usr/bin/env bash
# Finding 33: rustdoc panics ("called `Option::unwrap()` on a `None` value" in
# src/librustdoc/clean/mod.rs) on an empty nested `use` group such as `use {{}};`, which rustc
# accepts. `cargo doc` crashes on a crate that `cargo build` compiles. A nightly regression:
# nightly-2026-09-25 is fine, nightly-2026-09-26 panics, and it is still there on
# nightly-2026-10-06. The range contains rust-lang/rust#161349 ("Unflatten `use` statements in
# HIR"), which introduced the code that panics (not confirmed by a build with it reverted).
#
# Found by mirth's rustdoc consistency check (`mirth-lab rustdoc-diff`), which runs rustdoc on
# every UI test rustc accepts; three import tests (imports/duplicate-empty-imports.rs and
# imports/empty-import-prefix-pass{,-2015}.rs) panic. Not found in a public crate.
#
# Needs: rustup, cargo. Installs the two nightlies if missing.
set -euo pipefail
GOOD=nightly-2026-09-25
BAD=${BAD:-nightly-2026-10-06}
for tc in $GOOD $BAD; do rustup toolchain install --profile minimal "$tc" >/dev/null 2>&1; done

dir=$(mktemp -d)
cd "$dir"
cargo +$BAD init -q --lib --edition 2021 --name emptyuse

cat > src/lib.rs <<'EOF'
use {{}};

/// Documented so that rustdoc has something to render.
pub fn f() {}
EOF

for tc in $GOOD $BAD; do
  rm -rf target
  echo "== $tc"
  printf '  cargo build: '; cargo +"$tc" build -q 2>/dev/null && echo ok || echo FAILED
  printf '  cargo doc:   '
  if cargo +"$tc" doc -q 2> doc-$tc.txt; then echo ok; else echo FAILED; grep -E "panicked at|unwrap" doc-$tc.txt | head -2 | sed 's/^/    /'; fi
done

if grep -q "panicked" doc-$BAD.txt && ! grep -q "panicked" doc-$GOOD.txt; then
  echo "REPRODUCED: cargo doc panics on $BAD (not on $GOOD) while cargo build succeeds."
else
  echo "NOT REPRODUCED"
fi
echo "(work directory: $dir)"
