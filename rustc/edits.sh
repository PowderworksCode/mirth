#!/usr/bin/env bash
# Apply each edit in rustc/edits to the compiler, rebuild, check a fixture
# and run rustc's own metadata-related tests, and keep what both said in
# docs/edits/<edit>.txt. `none` is the compiler without edits, run first as
# the baseline; it is rebuilt without edits at the end.
#
#   rustc/edits.sh [fixture] [edit…]
#
# SUITES=0 skips rustc's own tests.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
: "${MIRTH_RUST:?set MIRTH_RUST to the checkout build.sh built}"
fixture=${1:-chain}
shift || true
edits=("$@")
if [ ${#edits[@]} -eq 0 ]; then
  edits=(none)
  for patch in "$here"/edits/*.patch; do edits+=("$(basename "$patch" .patch)"); done
fi
mkdir -p "$repo/docs/edits"

restore() {
  git -C "$MIRTH_RUST" checkout -q -- compiler library
}
trap restore EXIT

for edit in "${edits[@]}"; do
  echo "== $edit"
  restore
  [ "$edit" = none ] || git -C "$MIRTH_RUST" apply "$here/edits/$edit.patch"
  out=$repo/docs/edits/$edit.txt
  {
    echo "# $edit"
    echo
    if ! "$here/build.sh" > "$MIRTH_RUST/build/edit-$edit.log" 2>&1; then
      echo "the compiler did not build; see build/edit-$edit.log"
      continue
    fi
    "$here/check.sh" "$fixture"
    echo "check.sh exit $?"
    echo
    if [ "${SUITES:-1}" != 0 ]; then
      "$here/suites.sh" "$MIRTH_RUST/build/suites-$edit.log"
      echo "suites.sh exit $?"
    fi
  } > "$out" 2>&1
  tail -12 "$out"
done

if [ "${edits[-1]}" != none ]; then
  echo "== without edits"
  restore
  "$here/build.sh" > "$MIRTH_RUST/build/edit-restore.log" 2>&1 && echo "compiler restored"
fi
