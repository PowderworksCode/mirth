#![warn(unused_lifetimes)]

struct Inherent;

// The binder keeps the bound from being global, so it is not checked here. Removing the
// "unused" `'a` as suggested gives E0277: `Inherent: Clone` is not satisfied.
fn do_it()
where
    for<'a> Inherent: Clone,
{
}

fn main() {}
