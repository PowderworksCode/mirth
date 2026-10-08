#!/bin/bash
# Regenerate docs/hunt/{verify-reuse,report-untracked}.patch from ~/mirth-work/rust, which has
# applied: verify-reuse, the three fixes, report-untracked, and the stopgaps.
set -e
# Stopgaps for findings 9-12, each made from its own files (applied after report-untracked).
STOPGAPS="thinlto-order-stopgap print-type-sizes-trimmed-stopgap no-prepopulate-thinlto-stopgap rwpi-stopgap"
H=$HOME/mirth-work/patches; RUST=$HOME/mirth-work/rust; T=$CLAUDE_JOB_DIR/tmp/regen
cd $RUST; git worktree remove --force $T 2>/dev/null || true; git worktree prune
V="compiler/rustc_codegen_llvm/src/back/llvm_backend.rs compiler/rustc_codegen_llvm/src/base.rs compiler/rustc_codegen_llvm/src/llvm/ffi.rs compiler/rustc_codegen_ssa/src/base.rs compiler/rustc_codegen_ssa/src/traits/backend.rs compiler/rustc_incremental/src/persist/save.rs compiler/rustc_metadata/src/rmeta/encoder.rs compiler/rustc_middle/src/hooks.rs compiler/rustc_middle/src/query/on_disk_cache.rs compiler/rustc_query_impl/src/incremental.rs compiler/rustc_query_impl/src/lib.rs"
# 1. the reuse check alone, relative to the pinned commit
git worktree add -q --detach $T HEAD; cd $T
git apply $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
for f in $V; do cp $RUST/$f $T/$f; done
python3 - $T $V <<'PY'
import sys,re,pathlib
T=sys.argv[1]
for f in sys.argv[2:]:
    p=pathlib.Path(T)/f; s=p.read_text()
    s=re.sub(r'\(\*([\w\.\(\)]+?)\.(cg|unstable_opts)\.read_(\w+)\(\)\)', r'\1.\2.\3', s)
    out=[]
    for l in s.split('\n'):
        if 'untracked::untracked_read(' in l or 'declare_untracked_input(' in l:
            if out and out[-1].strip().startswith('// Its length, line table and content hash are read'): out.pop()
            continue
        out.append(l)
    p.write_text('\n'.join(out))
PY
git apply -R $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
git diff HEAD > $H/verify-reuse.patch
# 2. the report patch, relative to the check and the fixes
git checkout -q HEAD -- .; git apply $H/verify-reuse.patch
git apply $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
git add -A
cd $RUST; for f in $(git diff --name-only) compiler/rustc_data_structures/src/untracked.rs; do cp $f $T/$f; done
cd $T; git apply -R $H/debuginfo-checksum-stopgap.patch; git apply -R $H/threads-def-order-stopgap.patch
for p in $STOPGAPS; do git apply -R $H/$p.patch; done
git add -N compiler/rustc_data_structures/src/untracked.rs
git diff > $H/report-untracked.patch
# 3. check the stack reproduces the tree
git checkout -q HEAD -- .; git reset -q; rm -f compiler/rustc_data_structures/src/untracked.rs
git apply $H/verify-reuse.patch
git apply $H/generics-index-map.patch $H/alloc-dedup-on-decode.patch $H/metadata-source-files.patch
git apply $H/report-untracked.patch; git apply $H/debuginfo-checksum-stopgap.patch $H/threads-def-order-stopgap.patch
for p in $STOPGAPS; do git apply $H/$p.patch; done
n=0; cd $RUST; for f in $(git diff --name-only) compiler/rustc_data_structures/src/untracked.rs; do cmp -s $f $T/$f || { echo "differs: $f"; n=$((n+1)); }; done
echo "$n files differ"; wc -l $H/verify-reuse.patch $H/report-untracked.patch | head -2
git worktree remove --force $T
