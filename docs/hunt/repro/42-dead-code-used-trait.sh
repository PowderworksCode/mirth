#!/usr/bin/env bash
# Finding 42: the warn-by-default `dead_code` lint says "trait `X` is never used" about a trait the
# program needs: one that appears only in the where-clause, or in a projection in the self type,
# of an impl whose methods are called. Deleting the trait, as the warning invites, breaks the
# build. On stable since at least 1.80.0; a UI test (const-generics/issues/issue-69654-run-pass.rs)
# even blesses the warning.
#
# Found by mirth's lint-oracle check (`mirth-lab lint-check`), which deletes what `dead_code`
# flags and recompiles; three UI tests failed that way. Not found in a public crate.
#
# Needs: rustup, cargo.
set -euo pipefail
TOOLCHAIN=${TOOLCHAIN:-stable}

dir=$(mktemp -d)
cd "$dir"
cargo +"$TOOLCHAIN" init -q --bin --edition 2021 --name used

cat > src/main.rs <<'EOF'
// Case 1: the trait appears only in a projection in an impl's self type.
trait Mirror {
    type Me;
}
impl<T> Mirror for T {
    type Me = T;
}

struct Pair<A, B>(A, B);
impl<A> Pair<A, <A as Mirror>::Me> {
    fn make(a: A) -> u32 {
        let _ = a;
        1
    }
}

// Case 2: the trait appears only in the where-clause of an impl.
trait Bar<T> {}
impl<T> Bar<T> for [u8; 7] {}

struct Arr<const N: usize>;
impl<const N: usize> Arr<N>
where
    [u8; N]: Bar<[(); N]>,
{
    fn get() -> u32 {
        2
    }
}

fn main() {
    println!("{}", <Pair<u32, u32>>::make(22) + Arr::<7>::get());
}
EOF

echo "== cargo +$TOOLCHAIN run (expect: prints 3, and dead_code warns that both traits are never used)"
cargo +"$TOOLCHAIN" run -q 2> warnings.txt
grep -E "^warning: trait" warnings.txt || true

echo "== delete the two traits and their impls, as the warnings suggest, and build again"
sed -i '/^trait Mirror {/,/^}/d; /^impl<T> Mirror for T {/,/^}/d; /^trait Bar<T> {}/d; /^impl<T> Bar<T> for \[u8; 7\] {}/d' src/main.rs
if cargo +"$TOOLCHAIN" build -q 2> errors.txt; then
  echo "builds"
else
  grep -E "^error" errors.txt | sort -u
fi

if grep -q "trait \`Mirror\` is never used" warnings.txt && grep -q "trait \`Bar\` is never used" warnings.txt \
   && grep -q "^error" errors.txt; then
  echo "REPRODUCED: dead_code calls both traits never used, and removing them breaks the build."
else
  echo "NOT REPRODUCED"
fi
echo "(work directory: $dir)"
