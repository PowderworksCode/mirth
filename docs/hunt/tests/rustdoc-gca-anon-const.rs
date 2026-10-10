#![feature(generic_const_items, gca_min_const_items)]
#![feature(adt_const_params, const_param_ty_trait, generic_const_parameter_types)]
#![allow(incomplete_features)]
use std::gca;
use std::marker::{ConstParamTy, ConstParamTy_};
trait Owner {
    #[rustc_always_gca]
    const Q<T: ConstParamTy_>: Maybe<T>;
}
impl Owner for () {
    const Q<T: ConstParamTy_>: Maybe<T> = gca!(Maybe::Nothing::<T>);
}
fn take2<O: Owner<Q<()> = { Maybe::Just::<()>(()) }>>(_: O) {}
fn main() {}
#[derive(PartialEq, Eq, ConstParamTy)]
enum Maybe<T> { Nothing, Just(T) }
