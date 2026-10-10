#![feature(adt_const_params, unsized_const_params, generic_const_exprs)]
#![feature(inherent_associated_types)]
#![feature(gca_const_items, gca_min_const_items)]
use std::gca;
struct Foo<const A: usize>;
impl<const A: usize> Foo<A> {
    const SIZE: usize = { todo!() };
    fn to_bytes() -> [u8; gca!(Self::SIZE)] {
    }
}
