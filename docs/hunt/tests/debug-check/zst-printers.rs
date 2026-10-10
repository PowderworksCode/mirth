// Finding 33: the gdb pretty-printers on collections of zero-sized elements.
//   rustc --edition 2021 -g zst-printers.rs && rust-gdb -batch -ex 'break zst_printers::bp' -ex run -ex up \
//     -ex 'print v' -ex 'print d' -ex 'print s' -ex 'print m' -ex 'print t' ./zst-printers
use std::collections::{BTreeMap, BTreeSet, VecDeque};

struct Z;
enum One { Only }

#[inline(never)]
fn bp() {
    std::hint::black_box(());
}

fn main() {
    let v: Vec<Z> = vec![Z, Z];
    let d: VecDeque<()> = [(), ()].into_iter().collect();
    let s: &[Z] = &[Z, Z, Z];
    let m: BTreeMap<u8, One> = BTreeMap::from([(1, One::Only)]);
    let t: BTreeSet<Z> = BTreeSet::new();
    bp();
    std::hint::black_box((&v, &d, &s, &m, &t));
}
