// warning: trait `Mirror` is never used. It is used: `<A as Mirror>::Me` is in the self type
// of the impl that `main` calls into.
trait Mirror {
    type Me;
}
impl<T> Mirror for T {
    type Me = T;
}

struct Foo<A, B>(A, B);
impl<A> Foo<A, <A as Mirror>::Me> {
    fn m(_: A) {}
}

fn main() {
    <Foo<u32, u32>>::m(22);
}
