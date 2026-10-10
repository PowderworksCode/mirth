#![feature(type_alias_impl_trait)]

pub trait Baz {}
impl Baz for u32 {}

pub type Qux = impl Baz;

// warning: function `assign` is never used; removing it gives "unconstrained opaque type"
#[define_opaque(Qux)]
fn assign() -> Qux {
    3
}

pub fn take(_: Option<Qux>) {}

fn main() {}
