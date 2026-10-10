//! Nightly-only syntax: the parts of Ur's Rust grammar no stable construct reaches
//! (`mirth-lab grammar-coverage`). One module per feature, each with a function `main` checks.
#![feature(
    auto_traits,
    builtin_syntax,
    const_block_items,
    const_trait_impl,
    core_intrinsics,
    contracts_internals,
    coroutine_trait,
    coroutines,
    decl_macro,
    deref_patterns,
    ergonomic_clones,
    explicit_tail_calls,
    field_projections,
    gca_min_const_items,
    final_associated_functions,
    fn_delegation,
    gen_blocks,
    guard_patterns,
    move_expr,
    mut_restriction,
    negative_impls,
    never_patterns,
    pin_ergonomics,
    postfix_match,
    return_type_notation,
    specialization,
    stmt_expr_attributes,
    trait_alias,
    try_blocks,
    try_blocks_heterogeneous,
    unsafe_binders,
    yeet_expr,
    yield_expr
)]
#![allow(incomplete_features, internal_features, dead_code)]

pub mod exprs {
    use core::ops::{Coroutine, CoroutineState};
    use core::pin::Pin;

    /// An attribute on an expression.
    pub fn attributed() -> u32 {
        let x = #[allow(unused_parens)] (1 + 2);
        x
    }

    /// `become`: a guaranteed tail call.
    pub fn count(n: u32, acc: u32) -> u32 {
        if n == 0 { acc } else { become count(n - 1, acc + 1) }
    }

    #[repr(C)]
    pub struct Pair {
        pub a: u8,
        pub b: u32,
    }

    /// `builtin #` syntax: `offset_of` and `type_ascribe`.
    pub fn builtins() -> usize {
        let off = const { builtin # offset_of(Pair, b) };
        let five = builtin # type_ascribe(5, usize);
        off + five
    }

    /// A `gen` block and `.yield`.
    pub fn generated() -> u32 {
        let g = gen {
            yield 1u32;
            2u32.yield;
            yield 3;
        };
        g.sum()
    }

    /// A coroutine with `yield`.
    pub fn coroutine() -> u32 {
        let mut c = #[coroutine]
        || {
            yield 1u32;
            10u32
        };
        let mut total = 0;
        loop {
            match Pin::new(&mut c).resume(()) {
                CoroutineState::Yielded(x) => total += x,
                CoroutineState::Complete(x) => break total + x,
            }
        }
    }

    /// `&pin mut`, postfix `.match`, `.use`.
    pub fn postfix(x: u8) -> u8 {
        let mut v = 5u8;
        let p: Pin<&mut u8> = &pin mut v;
        let w = *p;
        let rc = std::rc::Rc::new(x);
        let copy = rc.use;
        let m = (*copy).match {
            0 => 1,
            n => n + w,
        };
        m
    }

    /// `try` blocks, a typed one, and `do yeet`.
    pub fn tries(a: Option<u8>, b: u8) -> (Option<u8>, Option<u8>, Option<u8>) {
        let plain: Option<u8> = try { a? + b };
        let typed = try bikeshed Option<u8> { a? * 2 };
        (plain, typed, yeets(b))
    }

    /// `move(...)` inside a closure: capture this expression's value by move.
    pub fn moved(v: Vec<u32>) -> usize {
        let c = || {
            let t = &move(v);
            t.len()
        };
        c()
    }

    /// A function with contracts, in their internal syntax.
    pub fn contracted(x: i32) -> i32
        contract_requires { x > 0 }
        contract_ensures { move |ret| *ret > x }
    {
        x + 1
    }

    fn yeets(b: u8) -> Option<u8> {
        if b == 0 {
            do yeet;
        }
        Some(b)
    }

    /// An unsafe binder: wrapping a reference and unwrapping it.
    pub fn binders(x: &u32) -> u32 {
        let b: unsafe<'a> &'a u32 = unsafe { builtin # wrap_binder(x) };
        // SAFETY: `x` outlives this use.
        let r: &u32 = unsafe { builtin # unwrap_binder(b) };
        *r
    }
}

pub mod items {
    // An anonymous `const` block at item level.
    const {
        assert!(1 + 1 == 2);
    }

    /// Specialization: a `default fn` overridden for one type.
    pub trait Size {
        fn size(&self) -> u8;
    }

    impl<T> Size for T {
        default fn size(&self) -> u8 {
            1
        }
    }

    impl Size for u64 {
        fn size(&self) -> u8 {
            8
        }
    }

    /// A `final` method.
    pub trait Named {
        final fn name(&self) -> &'static str {
            "named"
        }
    }

    impl Named for u8 {}

    /// Delegation with `reuse`: one function, a list, and a glob.
    pub mod to_reuse {
        pub fn add(a: u32, b: u32) -> u32 {
            a + b
        }
        pub fn mul(a: u32, b: u32) -> u32 {
            a * b
        }
    }

    pub reuse to_reuse::add;
    pub reuse to_reuse::{mul as times};

    pub trait Greet {
        fn hello(&self) -> u8;
        fn bye(&self) -> u8;
    }

    pub struct Inner;

    impl Greet for Inner {
        fn hello(&self) -> u8 {
            1
        }
        fn bye(&self) -> u8 {
            2
        }
    }

    pub struct Outer(pub Inner);

    impl Greet for Outer {
        reuse Greet::* { &self.0 }
    }

    pub struct Wrapped(pub Inner);

    reuse impl Greet for Wrapped { &self.0 }

    /// Macros 2.0.
    pub macro double($e:expr) {
        $e * 2
    }

    /// A trait alias and an auto trait with a negative impl.
    pub trait Both = Clone + Default;

    pub fn make<T: Both>() -> (T, T) {
        let t = T::default();
        (t.clone(), t)
    }

    pub auto trait Plain {}

    pub struct NotPlain;

    impl !Plain for NotPlain {}

    pub fn plain<T: Plain>(_: &T) -> bool {
        true
    }

    /// A field with a mutability restriction.
    pub struct Restricted {
        pub mut(crate) count: u32,
    }

    pub fn restricted(count: u32) -> Restricted {
        Restricted { count }
    }

    pub fn bump(r: &mut Restricted) -> u32 {
        r.count += 1;
        r.count
    }
}

pub mod consts {
    /// A `const` trait and a `~const` bound.
    pub const trait Weight {
        fn weight(&self) -> u8;
    }

    const impl Weight for u8 {
        fn weight(&self) -> u8 {
            *self * 2
        }
    }

    pub const fn weigh<T: ~const Weight>(t: &T) -> u8 {
        t.weight()
    }

    pub const HEAVY: u8 = weigh(&4u8);
}

pub mod patterns {
    /// Deref patterns, guard patterns and never patterns.
    pub fn deref(b: Box<u32>) -> u32 {
        match b {
            builtin # deref(0) => 0,
            builtin # deref(n) => n + 1,
        }
    }

    pub fn guarded(o: Option<u32>) -> u32 {
        match o {
            Some(x if x > 3) => 4,
            Some(_) => 1,
            None => 2,
        }
    }

    pub fn never(r: Result<u32, !>) -> u32 {
        match r {
            Ok(x) => x,
            Err(!),
        }
    }
}

pub mod types {
    use core::pin::Pin;

    /// Return-type notation in a bound and in a path.
    pub trait Fetch {
        async fn fetch(&self) -> u32;
    }

    pub fn needs_send<T: Fetch<fetch(..): Send>>(_: &T) -> bool {
        true
    }

    pub fn needs_send_path<T: Fetch>(_: &T) -> bool
    where
        T::fetch(..): Send,
    {
        true
    }

    pub struct Local;

    impl Fetch for Local {
        async fn fetch(&self) -> u32 {
            7
        }
    }

    /// A pinned reference type and a `const` block as a generic argument.
    pub fn pinned(p: &pin mut u32) -> u32 {
        *p
    }

    pub fn takes(p: Pin<&mut u32>) -> u32 {
        pinned(p)
    }

    pub fn konst<const N: usize>() -> usize {
        N
    }

    pub fn const_block_arg() -> usize {
        konst::<const { 2 + 2 }>()
    }

    /// `field_of` in type position.
    pub struct Pair {
        pub a: u8,
        pub b: u32,
    }

    pub type FieldB = builtin # field_of(Pair, b);
}
