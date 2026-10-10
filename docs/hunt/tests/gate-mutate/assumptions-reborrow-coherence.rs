#![feature(reborrow)]
use std::marker::{CoerceShared, Reborrow};
struct FieldMut<'a, T> {
    value: &'a mut T,
}
struct FieldRef<'a, T> {
    value: &'a T,
}
impl<'a, T> CoerceShared<FieldRef<'a, T>> for FieldMut<'a, T> {}
struct Source<'a> {
    field: FieldMut<'a, &'a ()>,
}
struct Target<'a> {
    field: FieldRef<'a, &'static ()>,
}
impl<'a> CoerceShared<Target<'a>> for Source<'a> {}
