#![feature(min_specialization)]
fn main() {}
trait SpecFromIter<T> {
    fn f(&self);
}
impl<'a, T: 'a, I: Iterator<Item = &'a T>> SpecFromIter<T> for I {
    default fn f(&self) {}
}
impl<'a, T> SpecFromIter<T> for std::slice::Iter<'a, T> {
    fn f(&self) {}
}
