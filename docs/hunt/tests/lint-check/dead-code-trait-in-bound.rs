// warning: trait `Bar` is never used. It is used: the impl below applies only where
// `[u8; N]: Bar<..>` holds, and `main` calls its `foo`. Removing `Bar` (and its impl) breaks
// the call.
trait Bar<T> {}
impl<T> Bar<T> for [u8; 7] {}

struct Foo<const N: usize>;
impl<const N: usize> Foo<N>
where
    [u8; N]: Bar<[(); N]>,
{
    fn foo() {}
}

fn main() {
    Foo::<7>::foo();
}
