struct Foo { foo: i32 }
fn test(f: Foo) -> i32 { let g = Foo { foo: 4, ..f }; g.foo } // `f` reported unused
fn main() { test(Foo { foo: 1 }); }
