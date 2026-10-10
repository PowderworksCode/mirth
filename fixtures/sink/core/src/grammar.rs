//! Stable syntax the rest of the sink does not use, found by measuring it against Ur's Rust
//! grammar (`mirth-lab grammar-coverage`): one function per construct or small group.

use core::cmp::Ordering;
use core::fmt::Debug;

/// Bit operators, shifts, `>=`, and every compound assignment.
pub fn bits(a: u32, b: u32) -> u32 {
    let mut x = (a & b) | (a ^ b) | (a << 2) | (b >> 1);
    x += 1;
    x -= 1;
    x *= 3;
    x /= 3;
    x %= 1 << 20;
    x &= !0;
    x |= 1;
    x ^= 0;
    x <<= 1;
    x >>= 1;
    if x >= a { x } else { a }
}

/// `continue`, a labelled `continue`, `false`, and an empty statement.
#[allow(redundant_semicolons)]
pub fn odd_sum(v: &[u32]) -> u32 {
    let mut sum = 0;
    let done = false;
    'outer: for &x in v {
        ;
        if x % 2 == 0 {
            continue;
        }
        for _ in 0..1 {
            if x > 100 {
                continue 'outer;
            }
        }
        sum += x;
    }
    if done { 0 } else { sum }
}

/// An open range, destructuring assignment with `_`, and `&raw const`.
pub fn ranges(v: &[u8]) -> (usize, u8) {
    let tail = &v[1..];
    let (first, _) = (v[0], ());
    let mut a;
    let b;
    (a, b) = (first, tail.len());
    _ = a;
    let p = &raw const a;
    // SAFETY: `p` points to `a`, a live local.
    (b, unsafe { *p })
}

/// Patterns: a leading `|`, negative literals and bounds, open ranges, paths, constants as
/// bounds, `ref`, `ref mut`, explicit struct fields, and a macro.
pub const LOW: i32 = -10;
pub const HIGH: i32 = 10;

macro_rules! zero_pat {
    () => {
        0
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub x: i32,
    pub y: i32,
}

pub fn patterns(n: i32, o: Ordering, p: Pt) -> &'static str {
    let mut q = p;
    let Pt { x: ref mut qx, y: ref qy } = q;
    *qx += *qy;
    let _ = q;
    match (n, o) {
        | (-1, _) => "minus one",
        (zero_pat!(), Ordering::Less) => "zero, less",
        (LOW..=HIGH, Ordering::Equal) => "small, equal",
        (..=-11, _) => "very negative",
        (100.., _) => "large",
        (-5..=-2, _) => "slightly negative",
        _ => match p {
            Pt { x: 0, y: yy } if yy > 0 => "on the y axis",
            Pt { .. } => "elsewhere",
        },
    }
}

/// Raw pointers, `fn` pointers with qualifiers, `!`, and a variadic foreign function.
pub type Callback = unsafe extern "C" fn(i32) -> i32;
pub type Formatter = fn(&str) -> String;
pub type Printf = unsafe extern "C" fn(*const u8, ...) -> i32;

unsafe extern "C" {
    pub safe fn abs(x: i32) -> i32;
    pub fn printf(format: *const u8, ...) -> i32;
}

pub extern "C" fn plus_one(x: i32) -> i32 {
    x + 1
}

pub fn pointers(v: &mut [i32; 2]) -> i32 {
    let p: *mut i32 = v.as_mut_ptr();
    let c: *const i32 = p;
    let f: Callback = plus_one;
    let g: Formatter = |s| s.to_uppercase();
    // SAFETY: both pointers are into `v`; `plus_one` has no preconditions.
    unsafe {
        *p.add(1) = 5;
        f(*c) + *p.add(1) + abs(-1) + g("x").len() as i32
    }
}

pub fn never_returns(msg: &str) -> ! {
    panic!("{msg}")
}

/// Qualified paths, associated type bounds, generic associated types with arguments in a
/// bound, a negative const argument, a macro in type position, and precise capturing.
pub trait Container {
    type Item<'a>
    where
        Self: 'a;
    fn first<'a>(&'a self) -> Option<Self::Item<'a>>;
}

impl Container for Vec<u8> {
    type Item<'a> = &'a u8;
    fn first<'a>(&'a self) -> Option<&'a u8> {
        <[u8]>::first(self)
    }
}

pub fn first_of<C>(c: &C) -> bool
where
    for<'a> C: Container<Item<'a> = &'a u8>,
{
    c.first().is_some()
}

pub fn cloned_items<I: Iterator<Item: Clone + Debug>>(it: I) -> usize {
    it.map(|x| x.clone()).count()
}

pub fn default_of<T: Default>() -> T {
    <T as Default>::default()
}

#[derive(Debug)]
pub struct Offset<const N: i32>;

impl<const N: i32> Offset<N> {
    pub const VALUE: i32 = N;
}

pub fn negative_const() -> i32 {
    Offset::<-3>::VALUE + Offset::<{ 2 + 1 }>::VALUE
}

macro_rules! byte_type {
    () => {
        u8
    };
}

pub fn macro_type(x: byte_type!()) -> u16 {
    x as u16
}

pub fn captured<'a>(v: &'a [u8]) -> impl Iterator<Item = u8> + use<'a> {
    v.iter().copied()
}

/// Raw identifiers for every reserved keyword.
pub fn reserved() -> u32 {
    let r#abstract = 1;
    let r#become = 1;
    let r#box = 1;
    let r#do = 1;
    let r#final = 1;
    let r#macro = 1;
    let r#override = 1;
    let r#priv = 1;
    let r#typeof = 1;
    let r#unsized = 1;
    let r#virtual = 1;
    let r#yield = 1;
    let r#try = 1;
    let r#gen = 1;
    r#abstract + r#become + r#box + r#do + r#final + r#macro + r#override + r#priv + r#typeof
        + r#unsized + r#virtual + r#yield + r#try + r#gen
}
