#!/bin/bash
# Regenerate docs/hunt/{verify-reuse,report-untracked,check-invariants}.patch from
# ~/mirth-work/rust, which has applied, in order: verify-reuse, the three fixes,
# report-untracked, the stopgaps, and check-invariants.
set -e
# Stopgaps for findings 9-12, each made from its own files (applied after report-untracked).
STOPGAPS="thinlto-order-stopgap print-type-sizes-trimmed-stopgap no-prepopulate-thinlto-stopgap rwpi-stopgap upstream-alloc-reference alloc-canonical-metadata-stopgap compiletest-solver-pin"
H=$HOME/mirth-work/patches; RUST=$HOME/mirth-work/rust; T=$CLAUDE_JOB_DIR/tmp/regen
# Files the patches add (untracked in the tree).
NEW_REPORT="compiler/rustc_data_structures/src/untracked.rs"
NEW_INVARIANTS="compiler/rustc_data_structures/src/invariants.rs compiler/rustc_query_impl/src/invariants.rs"
cd $RUST; git worktree remove --force $T 2>/dev/null || true; git worktree prune
V="compiler/rustc_codegen_llvm/src/back/llvm_backend.rs compiler/rustc_codegen_llvm/src/base.rs compiler/rustc_codegen_llvm/src/llvm/ffi.rs compiler/rustc_codegen_ssa/src/base.rs compiler/rustc_codegen_ssa/src/traits/backend.rs compiler/rustc_incremental/src/persist/save.rs compiler/rustc_metadata/src/rmeta/encoder.rs compiler/rustc_middle/src/hooks.rs compiler/rustc_middle/src/query/on_disk_cache.rs compiler/rustc_query_impl/src/incremental.rs compiler/rustc_query_impl/src/lib.rs"
# 1. the reuse check alone, relative to the pinned commit
git worktree add -q --detach $T HEAD; cd $T
git apply $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
for f in $V; do
  cp $RUST/$f $T/$f
  # Take check-invariants out of the copy (it is the last patch of the stack).
  git apply -R --include=$f $H/check-invariants.patch
  # Take report-untracked out: its option reads, and its declared and reported reads.
  sed -E -i 's/\(\*([[:alnum:]_.()]+)\.(cg|unstable_opts)\.read_([[:alnum:]_]+)\(\)\)/\1.\2.\3/g' $f
  sed -E -i '/\/\/ Its length, line table and content hash are read/{N;/untracked::untracked_read\(|declare_untracked_input\(/d}' $f
  sed -E -i '/untracked::untracked_read\(|declare_untracked_input\(/d' $f
done
git apply -R $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
git diff HEAD > $H/verify-reuse.patch
# 2. the report patch, relative to the check and the fixes
git checkout -q HEAD -- .; git apply $H/verify-reuse.patch
git apply $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
git add -A
cd $RUST; for f in $(git diff --name-only) $NEW_REPORT $NEW_INVARIANTS; do cp $f $T/$f; done
cd $T; git apply -R $H/check-invariants.patch
git apply -R $H/debuginfo-checksum-stopgap.patch; git apply -R $H/threads-def-order-stopgap.patch
for p in $STOPGAPS; do git apply -R $H/$p.patch; done
git add -N $NEW_REPORT
git diff > $H/report-untracked.patch
# 3. the invariants patch, relative to everything before it
git checkout -q HEAD -- .; git reset -q; rm -f $NEW_REPORT $NEW_INVARIANTS
git apply $H/verify-reuse.patch
git apply $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
git apply $H/report-untracked.patch; git apply $H/debuginfo-checksum-stopgap.patch $H/threads-def-order-stopgap.patch
for p in $STOPGAPS; do git apply $H/$p.patch; done
git add -A
cd $RUST; for f in $(git diff --name-only) $NEW_INVARIANTS; do cp $f $T/$f; done
cd $T; git add -N $NEW_INVARIANTS; git diff > $H/check-invariants.patch
# 4. check the stack reproduces the tree
git checkout -q HEAD -- .; git reset -q; rm -f $NEW_REPORT $NEW_INVARIANTS
git apply $H/verify-reuse.patch
git apply $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
git apply $H/report-untracked.patch; git apply $H/debuginfo-checksum-stopgap.patch $H/threads-def-order-stopgap.patch
for p in $STOPGAPS; do git apply $H/$p.patch; done
git apply $H/check-invariants.patch
n=0; cd $RUST; for f in $(git diff --name-only) $NEW_REPORT $NEW_INVARIANTS; do cmp -s $f $T/$f || { echo "differs: $f"; n=$((n+1)); }; done
echo "$n files differ"; wc -l $H/verify-reuse.patch $H/report-untracked.patch $H/check-invariants.patch | head -3
git worktree remove --force $T
