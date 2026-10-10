//! Debugger round trip (check 20): gdb, with the toolchain's Rust pretty-printers, must show the
//! values a program defines, and its printers must terminate.
//!
//! Each seed generates a program that builds known values of std and user types (integers of
//! every width, floats with special and random bit patterns, chars, strings with escapes and
//! non-ASCII, arrays, slices, Vec, VecDeque with wrap-around, HashMap/BTreeMap/HashSet/BTreeSet,
//! Option/Result and niche-optimized enums, Box, Rc/Arc with known counts, Weak, RefCell with live
//! borrows, Cell, tuples, generated structs, tuple structs, unit structs, enums with data and with
//! explicit discriminants, unions, references, PhantomData, large vectors and Rc cycles), stops
//! at a breakpoint and prints each local (and `*v` for boxes and references) under a timeout.
//! The output is parsed into a tree and compared with what the generator built: a wrong value,
//! a missing field, a wrong length, count or variant, a printer exception, an error, a gdb crash
//! or a timeout is a finding. At -Copt-level=1 and 2, `<optimized out>` is accepted anywhere,
//! a wrong value never is.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::Serialize;

use mirth_lab::rustc::{Exit, run_command};

#[derive(clap::Args, Debug)]
pub struct Args {
    /// A rustup toolchain with rust-gdb (its pretty-printers are the ones checked).
    #[arg(long, default_value = "nightly-2026-10-06")]
    toolchain: String,
    #[arg(long)]
    work: PathBuf,
    /// Seeds `lo..hi`.
    #[arg(long, default_value = "0..1000")]
    seeds: String,
    #[arg(long, default_value = "0,1,2")]
    opt: String,
    /// Locals per program.
    #[arg(long, default_value_t = 12)]
    locals: usize,
    /// Seconds per gdb run.
    #[arg(long, default_value_t = 60)]
    timeout: u64,
    #[arg(long, default_value_t = 4)]
    jobs: usize,
}

// ---- what the generator built ----

#[derive(Clone, Debug)]
enum M {
    Int(String),
    Float { bits: u64, f32: bool },
    Bool(bool),
    Char(u32),
    Str(String),
    Unit,
    /// An address (a reference, a Box): any pointer.
    Ptr,
    /// Only termination is checked (Weak, Rc cycles).
    Any,
    Tuple(Vec<M>),
    Array(Vec<M>),
    Seq { kind: &'static str, items: Vec<M>, ordered: bool },
    Map { kind: &'static str, pairs: Vec<(M, M)>, ordered: bool },
    Struct { name: String, fields: Vec<(String, M)> },
    TupleStruct { name: String, items: Vec<M> },
    /// A unit struct, unit variant or PhantomData: the last path segment, without generics.
    Named(String),
    Variant { name: String, payload: Payload },
    Rc { kind: &'static str, strong: u64, weak: u64, value: Box<M> },
    RefCell { borrow: i64, value: Box<M> },
    Cell(Box<M>),
    /// A live `Ref`/`RefMut`: the RefCell's borrow flag and the borrowed value.
    Guard { borrow: i64, value: Box<M> },
}

#[derive(Clone, Debug)]
enum Payload {
    Tuple(Vec<M>),
    Struct(Vec<(String, M)>),
}

impl M {
    fn kind(&self) -> String {
        match self {
            M::Int(_) => "int".into(),
            M::Float { f32: true, .. } => "f32".into(),
            M::Float { .. } => "f64".into(),
            M::Bool(_) => "bool".into(),
            M::Char(_) => "char".into(),
            M::Str(_) => "str".into(),
            M::Unit => "()".into(),
            M::Ptr => "ptr".into(),
            M::Any => "any".into(),
            M::Tuple(_) => "tuple".into(),
            M::Array(_) => "array".into(),
            M::Seq { kind: "&[", .. } => "slice".into(),
            M::Seq { kind, .. } | M::Map { kind, .. } => kind.to_lowercase(),
            M::Struct { .. } => "struct".into(),
            M::TupleStruct { .. } => "tuple-struct".into(),
            M::Named(_) => "unit".into(),
            M::Variant { .. } => "variant".into(),
            M::Rc { kind, .. } => kind.to_lowercase(),
            M::RefCell { .. } => "refcell".into(),
            M::Cell(_) => "cell".into(),
            M::Guard { borrow, .. } if *borrow < 0 => "refmut".into(),
            M::Guard { .. } => "ref".into(),
        }
    }
}

// ---- generating programs ----

#[derive(Clone, Debug)]
enum Ty {
    Int { bits: u32, signed: bool, size: bool },
    F32,
    F64,
    Bool,
    Char,
    Unit,
    Str,
    String,
    Array(Box<Ty>, usize),
    Slice(Box<Ty>),
    Vec(Box<Ty>),
    BigVec(usize),
    Deque(Box<Ty>),
    HashMap(Box<Ty>, Box<Ty>),
    BTreeMap(Box<Ty>, Box<Ty>),
    HashSet(Box<Ty>),
    BTreeSet(Box<Ty>),
    Option(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    Box(Box<Ty>),
    Rc(Box<Ty>),
    Arc(Box<Ty>),
    Weak(Box<Ty>),
    RefCell(Box<Ty>),
    Cell(Box<Ty>),
    Ref(Box<Ty>),
    Tuple(Vec<Ty>),
    Struct(usize),
    TupleStruct(usize),
    UnitStruct(usize),
    Enum(usize),
    Union(usize),
    NonZero { bits: u32, signed: bool },
    Phantom(Box<Ty>),
    OsString,
    BoxStr,
    BoxSlice(Box<Ty>),
    PathBuf,
    /// `Rc<str>` (true) or `Arc<[T]>`: unsized values behind a counted pointer.
    RcStr,
    ArcSlice(Box<Ty>),
}

enum Shape {
    Unit,
    Tuple(Vec<Ty>),
    Struct(Vec<(String, Ty)>),
}

struct EnumDef {
    /// `#[repr(..)]` and explicit discriminants (fieldless enums only).
    repr: Option<(&'static str, Vec<i64>)>,
    variants: Vec<Shape>,
}

struct Gen {
    rng: StdRng,
    structs: Vec<Vec<(String, Ty)>>,
    tuple_structs: Vec<Vec<Ty>>,
    unit_structs: usize,
    enums: Vec<EnumDef>,
    unions: Vec<Vec<(String, Ty)>>,
    /// Statements before the current local: the targets of references and slices.
    aux: Vec<String>,
    names: usize,
    /// The model of the last Box's or reference's pointee (for `print *v`).
    pointee: Option<M>,
}

const INT_BITS: [u32; 5] = [8, 16, 32, 64, 128];
const CHARS: &[char] = &[
    'a', 'Z', '0', ' ', '"', '\\', '\'', '\n', '\t', '\0', 'é', 'ß', '中', '🦀', '\u{7f}', '\u{80}', '\u{fffd}', '\u{10ffff}', '\u{1b}',
];

impl Gen {
    fn new(seed: u64) -> Gen {
        Gen { rng: StdRng::seed_from_u64(seed), structs: vec![], tuple_structs: vec![], unit_structs: 0, enums: vec![], unions: vec![], aux: vec![], names: 0, pointee: None }
    }

    fn chance(&mut self, p: f64) -> bool {
        self.rng.random_bool(p)
    }

    fn below(&mut self, n: usize) -> usize {
        self.rng.random_range(0..n)
    }

    fn int_ty(&mut self) -> Ty {
        let size = self.chance(0.1);
        Ty::Int { bits: if size { 64 } else { INT_BITS[self.below(5)] }, signed: self.chance(0.5), size }
    }

    fn key_ty(&mut self) -> Ty {
        match self.below(4) {
            0 | 1 => self.int_ty(),
            2 => Ty::String,
            _ => Ty::Char,
        }
    }

    /// A type; `def` for a field of a type definition (no references there).
    fn ty(&mut self, depth: u32, def: bool) -> Ty {
        let leaf = depth >= 3 || self.chance(0.35);
        if leaf {
            return match self.below(12) {
                0..=3 => self.int_ty(),
                4 => Ty::F32,
                5 => Ty::F64,
                6 => Ty::Bool,
                7 => Ty::Char,
                8 => Ty::Str,
                9 => Ty::String,
                10 => self.unit_struct(),
                11 if self.chance(0.2) => Ty::Unit,
                11 if self.chance(0.3) => [Ty::OsString, Ty::BoxStr, Ty::PathBuf, Ty::RcStr][self.below(4)].clone(),
                _ => Ty::NonZero { bits: INT_BITS[self.below(5)], signed: self.chance(0.5) },
            };
        }
        let d = depth + 1;
        match self.below(30) {
            0 => Ty::Array(Box::new(self.ty(d, def)), self.below(5)),
            1 if !def => Ty::Slice(Box::new(self.ty(d, def))),
            2 | 3 => Ty::Vec(Box::new(self.ty(d, def))),
            4 => Ty::Deque(Box::new(self.ty(d, def))),
            5 => Ty::HashMap(Box::new(self.key_ty()), Box::new(self.ty(d, def))),
            6 => Ty::BTreeMap(Box::new(self.key_ty()), Box::new(self.ty(d, def))),
            7 => Ty::HashSet(Box::new(self.key_ty())),
            8 => Ty::BTreeSet(Box::new(self.key_ty())),
            9..=11 => Ty::Option(Box::new(self.ty(d, def))),
            12 => Ty::Result(Box::new(self.ty(d, def)), Box::new(self.ty(d, def))),
            13 => Ty::Box(Box::new(self.ty(d, def))),
            14 => Ty::Rc(Box::new(self.ty(d, def))),
            15 => Ty::Arc(Box::new(self.ty(d, def))),
            16 => Ty::Weak(Box::new(self.ty(d, def))),
            17 => Ty::RefCell(Box::new(self.ty(d, def))),
            18 => Ty::Cell(Box::new(self.ty(d, def))),
            19 | 20 if !def => Ty::Ref(Box::new(self.ty(d, def))),
            21 => Ty::Tuple((0..self.below(4)).map(|_| self.ty(d, def)).collect()),
            22 | 23 => {
                let fields = (0..1 + self.below(4)).map(|i| (format!("f{i}"), self.ty(d, true))).collect();
                self.structs.push(fields);
                Ty::Struct(self.structs.len() - 1)
            }
            24 => {
                let items = (0..1 + self.below(3)).map(|_| self.ty(d, true)).collect();
                self.tuple_structs.push(items);
                Ty::TupleStruct(self.tuple_structs.len() - 1)
            }
            25 | 26 => self.enum_ty(d),
            27 => {
                let n = 1 + self.below(3);
                let fields = (0..=n)
                    .map(|i| {
                        let t = if self.chance(0.7) { self.int_ty() } else if self.chance(0.5) { Ty::F32 } else { Ty::F64 };
                        (format!("f{i}"), t)
                    })
                    .collect();
                self.unions.push(fields);
                Ty::Union(self.unions.len() - 1)
            }
            28 if self.chance(0.5) => Ty::Phantom(Box::new(self.int_ty())),
            28 if self.chance(0.5) => Ty::BoxSlice(Box::new(self.ty(d, def))),
            28 => Ty::ArcSlice(Box::new(self.ty(d, def))),
            _ => Ty::BigVec(200 + self.below(2800)),
        }
    }

    fn unit_struct(&mut self) -> Ty {
        self.unit_structs += 1;
        Ty::UnitStruct(self.unit_structs - 1)
    }

    fn enum_ty(&mut self, d: u32) -> Ty {
        let n = 1 + self.below(4);
        let def = if self.chance(0.2) {
            let reprs = ["u8", "i8", "u16", "i16", "u32", "i32", "i64", "u64"];
            let repr = reprs[self.below(reprs.len())];
            let (lo, hi): (i64, i64) = match repr {
                "u8" => (0, 255),
                "i8" => (-128, 127),
                "u16" => (0, 65535),
                "i16" => (-32768, 32767),
                "u32" => (0, u32::MAX as i64),
                "i32" => (i32::MIN as i64, i32::MAX as i64),
                "u64" => (0, i64::MAX),
                _ => (i64::MIN, i64::MAX),
            };
            let mut ds: Vec<i64> = Vec::new();
            while ds.len() < n {
                let v = if self.chance(0.3) { [lo, hi][self.below(2)] } else { self.rng.random_range(lo..=hi) };
                if !ds.contains(&v) {
                    ds.push(v);
                }
            }
            EnumDef { repr: Some((repr, ds)), variants: (0..n).map(|_| Shape::Unit).collect() }
        } else {
            let variants = (0..n)
                .map(|_| match self.below(3) {
                    0 => Shape::Unit,
                    1 => Shape::Tuple((0..1 + self.below(3)).map(|_| self.ty(d, true)).collect()),
                    _ => Shape::Struct((0..1 + self.below(3)).map(|i| (format!("x{i}"), self.ty(d, true))).collect()),
                })
                .collect();
            EnumDef { repr: None, variants }
        };
        self.enums.push(def);
        Ty::Enum(self.enums.len() - 1)
    }

    fn ty_str(&self, t: &Ty) -> String {
        match t {
            Ty::Int { bits, signed, size } => {
                if *size {
                    if *signed { "isize".into() } else { "usize".into() }
                } else {
                    format!("{}{bits}", if *signed { "i" } else { "u" })
                }
            }
            Ty::F32 => "f32".into(),
            Ty::F64 => "f64".into(),
            Ty::Bool => "bool".into(),
            Ty::Char => "char".into(),
            Ty::Unit => "()".into(),
            Ty::Str => "&'static str".into(),
            Ty::String => "String".into(),
            Ty::Array(e, n) => format!("[{}; {n}]", self.ty_str(e)),
            Ty::Slice(e) => format!("&[{}]", self.ty_str(e)),
            Ty::Vec(e) => format!("Vec<{}>", self.ty_str(e)),
            Ty::BigVec(_) => "Vec<u32>".into(),
            Ty::Deque(e) => format!("VecDeque<{}>", self.ty_str(e)),
            Ty::HashMap(k, v) => format!("HashMap<{}, {}>", self.ty_str(k), self.ty_str(v)),
            Ty::BTreeMap(k, v) => format!("BTreeMap<{}, {}>", self.ty_str(k), self.ty_str(v)),
            Ty::HashSet(k) => format!("HashSet<{}>", self.ty_str(k)),
            Ty::BTreeSet(k) => format!("BTreeSet<{}>", self.ty_str(k)),
            Ty::Option(e) => format!("Option<{}>", self.ty_str(e)),
            Ty::Result(a, b) => format!("Result<{}, {}>", self.ty_str(a), self.ty_str(b)),
            Ty::Box(e) => format!("Box<{}>", self.ty_str(e)),
            Ty::Rc(e) => format!("Rc<{}>", self.ty_str(e)),
            Ty::Arc(e) => format!("Arc<{}>", self.ty_str(e)),
            Ty::Weak(e) => format!("Weak<{}>", self.ty_str(e)),
            Ty::RefCell(e) => format!("RefCell<{}>", self.ty_str(e)),
            Ty::Cell(e) => format!("Cell<{}>", self.ty_str(e)),
            Ty::Ref(e) => format!("&{}", self.ty_str(e)),
            Ty::Tuple(items) if items.len() == 1 => format!("({},)", self.ty_str(&items[0])),
            Ty::Tuple(items) => format!("({})", items.iter().map(|t| self.ty_str(t)).collect::<Vec<_>>().join(", ")),
            Ty::Struct(i) => format!("S{i}"),
            Ty::TupleStruct(i) => format!("T{i}"),
            Ty::UnitStruct(i) => format!("Z{i}"),
            Ty::Enum(i) => format!("E{i}"),
            Ty::Union(i) => format!("U{i}"),
            Ty::NonZero { bits, signed } => format!("NonZero<{}{bits}>", if *signed { "i" } else { "u" }),
            Ty::Phantom(e) => format!("PhantomData<{}>", self.ty_str(e)),
            Ty::OsString => "std::ffi::OsString".into(),
            Ty::BoxStr => "Box<str>".into(),
            Ty::BoxSlice(e) => format!("Box<[{}]>", self.ty_str(e)),
            Ty::PathBuf => "std::path::PathBuf".into(),
            Ty::RcStr => "Rc<str>".into(),
            Ty::ArcSlice(e) => format!("Arc<[{}]>", self.ty_str(e)),
        }
    }

    fn int_value(&mut self, bits: u32, signed: bool) -> i128 {
        // As two's complement in i128 for signed, raw for unsigned (u128 max is handled by the caller).
        let (lo, hi): (i128, i128) = if signed {
            if bits == 128 { (i128::MIN, i128::MAX) } else { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) }
        } else if bits == 128 {
            (0, i128::MAX)
        } else {
            (0, (1i128 << bits) - 1)
        };
        match self.below(6) {
            0 => lo,
            1 => hi,
            2 => 0,
            3 => 1.clamp(lo, hi),
            _ => self.rng.random_range(lo..=hi),
        }
    }

    fn string(&mut self) -> String {
        let n = if self.chance(0.1) { 0 } else { self.below(14) };
        (0..n)
            .map(|_| {
                if self.chance(0.6) {
                    (b'a' + self.below(26) as u8) as char
                } else {
                    CHARS[self.below(CHARS.len())]
                }
            })
            .collect()
    }

    fn float_bits(&mut self, f32: bool) -> u64 {
        if f32 {
            let specials = [0u32, 0x8000_0000, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000, 0x7f80_0001, 1, 0x7f7f_ffff, 0x3fc0_0000, 0xc010_0000, 0x3dcc_cccd];
            (if self.chance(0.5) { specials[self.below(specials.len())] } else { self.rng.random() }) as u64
        } else {
            let specials = [0u64, 1 << 63, 0x7ff0 << 48, 0xfff0 << 48, 0x7ff8 << 48, (0x7ff0 << 48) | 1, 1, 0x7fef_ffff_ffff_ffff, 0x3ff8 << 48, 0x3fb9_9999_9999_999a];
            if self.chance(0.5) { specials[self.below(specials.len())] } else { self.rng.random() }
        }
    }

    fn aux_local(&mut self, ty: &str, expr: &str) -> String {
        let name = format!("r{}", self.names);
        self.names += 1;
        self.aux.push(format!("    let {name}: {ty} = {expr};"));
        name
    }

    /// A value of the type: (Rust expression, model).
    fn value(&mut self, t: &Ty) -> (String, M) {
        match t {
            Ty::Int { bits, signed, size } => {
                let ts = self.ty_str(t);
                if !signed && *bits == 128 && self.chance(0.2) {
                    return (format!("{}{ts}", u128::MAX), M::Int(u128::MAX.to_string()));
                }
                let v = self.int_value(*bits, *signed);
                let _ = size;
                (format!("{v}{ts}"), M::Int(v.to_string()))
            }
            Ty::F32 | Ty::F64 => {
                let f32 = matches!(t, Ty::F32);
                let bits = self.float_bits(f32);
                let e = if f32 { format!("f32::from_bits({bits:#x})") } else { format!("f64::from_bits({bits:#x})") };
                (e, M::Float { bits, f32 })
            }
            Ty::Bool => {
                let b = self.chance(0.5);
                (b.to_string(), M::Bool(b))
            }
            Ty::Char => {
                let c = if self.chance(0.7) {
                    CHARS[self.below(CHARS.len())]
                } else {
                    loop {
                        if let Some(c) = char::from_u32(self.rng.random_range(0..0x11_0000)) {
                            break c;
                        }
                    }
                };
                (format!("'\\u{{{:x}}}'", c as u32), M::Char(c as u32))
            }
            Ty::Unit => ("()".into(), M::Unit),
            Ty::Str => {
                let s = self.string();
                (format!("{s:?}"), M::Str(s))
            }
            Ty::String => {
                let s = self.string();
                (format!("String::from({s:?})"), M::Str(s))
            }
            Ty::Array(e, n) => {
                let (es, ms): (Vec<String>, Vec<M>) = (0..*n).map(|_| self.value(e)).unzip();
                (format!("[{}]", es.join(", ")), M::Array(ms))
            }
            Ty::Slice(e) => {
                let n = self.below(5);
                let (es, ms): (Vec<String>, Vec<M>) = (0..n).map(|_| self.value(e)).unzip();
                let ty = format!("[{}; {n}]", self.ty_str(e));
                let r = self.aux_local(&ty, &format!("[{}]", es.join(", ")));
                (format!("&{r}[..]"), M::Seq { kind: "&[", items: ms, ordered: true })
            }
            Ty::Vec(e) => {
                let n = if self.chance(0.1) { 10 + self.below(30) } else { self.below(5) };
                let (es, ms): (Vec<String>, Vec<M>) = (0..n).map(|_| self.value(e)).unzip();
                (format!("vec![{}]", es.join(", ")), M::Seq { kind: "Vec", items: ms, ordered: true })
            }
            Ty::BigVec(n) => {
                let k = 2_654_435_761u32;
                let items = (0..*n as u32).map(|i| M::Int(i.wrapping_mul(k).to_string())).collect();
                (format!("(0..{n}u32).map(|i| i.wrapping_mul({k})).collect::<Vec<u32>>()"), M::Seq { kind: "Vec", items, ordered: true })
            }
            Ty::Deque(e) => {
                let (n1, k, n2) = (self.below(6), 0, self.below(5));
                let k = if n1 > 0 { self.below(n1 + 1) } else { k };
                let (a, am): (Vec<String>, Vec<M>) = (0..n1).map(|_| self.value(e)).unzip();
                let (b, bm): (Vec<String>, Vec<M>) = (0..n2).map(|_| self.value(e)).unzip();
                let ts = self.ty_str(e);
                let cap = 1 + self.below(6);
                let expr = format!(
                    "{{ let mut d: VecDeque<{ts}> = VecDeque::with_capacity({cap}); let a: [{ts}; {n1}] = [{}]; for x in a {{ d.push_back(x); }} for _ in 0..{k} {{ d.pop_front(); }} let b: [{ts}; {n2}] = [{}]; for x in b {{ d.push_back(x); }} d }}",
                    a.join(", "),
                    b.join(", ")
                );
                let items = am.into_iter().skip(k).chain(bm).collect();
                (expr, M::Seq { kind: "VecDeque", items, ordered: true })
            }
            Ty::HashMap(k, v) | Ty::BTreeMap(k, v) => {
                let hash = matches!(t, Ty::HashMap(..));
                let n = if self.chance(0.1) { 10 + self.below(20) } else { self.below(5) };
                let mut pairs: Vec<(String, String, M, M)> = Vec::new();
                for _ in 0..n * 2 {
                    if pairs.len() >= n {
                        break;
                    }
                    let (ke, km) = self.value(k);
                    if pairs.iter().any(|p| key_eq(&p.2, &km)) {
                        continue;
                    }
                    let (ve, vm) = self.value(v);
                    pairs.push((ke, ve, km, vm));
                }
                let ts = self.ty_str(t);
                let lits: Vec<String> = pairs.iter().map(|p| format!("({}, {})", p.0, p.1)).collect();
                let mut ms: Vec<(M, M)> = pairs.into_iter().map(|p| (p.2, p.3)).collect();
                if !hash {
                    ms.sort_by(|a, b| key_cmp(&a.0, &b.0));
                }
                let kind = if hash { "HashMap" } else { "BTreeMap" };
                (format!("<{ts}>::from([{}])", lits.join(", ")), M::Map { kind, pairs: ms, ordered: !hash })
            }
            Ty::HashSet(k) | Ty::BTreeSet(k) => {
                let hash = matches!(t, Ty::HashSet(..));
                let n = self.below(6);
                let mut items: Vec<(String, M)> = Vec::new();
                for _ in 0..n * 2 {
                    if items.len() >= n {
                        break;
                    }
                    let (ke, km) = self.value(k);
                    if !items.iter().any(|p| key_eq(&p.1, &km)) {
                        items.push((ke, km));
                    }
                }
                let ts = self.ty_str(t);
                let lits: Vec<String> = items.iter().map(|p| p.0.clone()).collect();
                let mut ms: Vec<M> = items.into_iter().map(|p| p.1).collect();
                if !hash {
                    ms.sort_by(key_cmp);
                }
                let kind = if hash { "HashSet" } else { "BTreeSet" };
                (format!("<{ts}>::from([{}])", lits.join(", ")), M::Seq { kind, items: ms, ordered: !hash })
            }
            Ty::Option(e) => {
                if self.chance(0.25) {
                    ("None".into(), M::Variant { name: "None".into(), payload: Payload::Tuple(vec![]) })
                } else {
                    let (x, m) = self.value(e);
                    (format!("Some({x})"), M::Variant { name: "Some".into(), payload: Payload::Tuple(vec![m]) })
                }
            }
            Ty::Result(a, b) => {
                let ok = self.chance(0.5);
                let (x, m) = self.value(if ok { a } else { b });
                let name = if ok { "Ok" } else { "Err" };
                (format!("{name}({x})"), M::Variant { name: name.into(), payload: Payload::Tuple(vec![m]) })
            }
            Ty::Box(e) => {
                let (x, inner) = self.value(e);
                self.pointee = Some(inner);
                (format!("Box::new({x})"), M::Ptr)
            }
            Ty::Rc(e) | Ty::Arc(e) => {
                let kind = if matches!(t, Ty::Rc(_)) { "Rc" } else { "Arc" };
                let (x, m) = self.value(e);
                let (s, w) = (self.below(3) as u64, self.below(3) as u64);
                let expr = format!(
                    "{{ let r = {kind}::new({x}); for _ in 0..{s} {{ std::mem::forget(r.clone()); }} for _ in 0..{w} {{ std::mem::forget({kind}::downgrade(&r)); }} r }}"
                );
                (expr, M::Rc { kind, strong: 1 + s, weak: w, value: Box::new(m) })
            }
            Ty::Weak(e) => {
                if self.chance(0.3) {
                    ("Weak::new()".into(), M::Any)
                } else {
                    let (x, _) = self.value(e);
                    let ty = format!("Rc<{}>", self.ty_str(e));
                    let r = self.aux_local(&ty, &format!("Rc::new({x})"));
                    (format!("Rc::downgrade(&{r})"), M::Any)
                }
            }
            Ty::RefCell(e) => {
                let (x, m) = self.value(e);
                (format!("RefCell::new({x})"), M::RefCell { borrow: 0, value: Box::new(m) })
            }
            Ty::Cell(e) => {
                let (x, m) = self.value(e);
                (format!("Cell::new({x})"), M::Cell(Box::new(m)))
            }
            Ty::Ref(e) => {
                let (x, inner) = self.value(e);
                self.pointee = Some(inner);
                let ty = self.ty_str(e);
                let r = self.aux_local(&ty, &x);
                (format!("&{r}"), M::Ptr)
            }
            Ty::Tuple(items) => {
                let (es, ms): (Vec<String>, Vec<M>) = items.iter().map(|t| self.value(t)).unzip();
                if es.is_empty() {
                    return ("()".into(), M::Unit);
                }
                let e = if es.len() == 1 { format!("({},)", es[0]) } else { format!("({})", es.join(", ")) };
                (e, M::Tuple(ms))
            }
            Ty::Struct(i) => {
                let fields = self.structs[*i].clone();
                let (es, ms): (Vec<String>, Vec<(String, M)>) = fields
                    .iter()
                    .map(|(n, t)| {
                        let (e, m) = self.value(t);
                        (format!("{n}: {e}"), (n.clone(), m))
                    })
                    .unzip();
                (format!("S{i} {{ {} }}", es.join(", ")), M::Struct { name: format!("S{i}"), fields: ms })
            }
            Ty::TupleStruct(i) => {
                let items = self.tuple_structs[*i].clone();
                let (es, ms): (Vec<String>, Vec<M>) = items.iter().map(|t| self.value(t)).unzip();
                (format!("T{i}({})", es.join(", ")), M::TupleStruct { name: format!("T{i}"), items: ms })
            }
            Ty::UnitStruct(i) => (format!("Z{i}"), M::Named(format!("Z{i}"))),
            Ty::Enum(i) => {
                let n = self.enums[*i].variants.len();
                let v = self.below(n);
                let shape = match &self.enums[*i].variants[v] {
                    Shape::Unit => None,
                    Shape::Tuple(ts) => Some((true, ts.iter().map(|t| (String::new(), t.clone())).collect::<Vec<_>>())),
                    Shape::Struct(fs) => Some((false, fs.clone())),
                };
                let name = format!("V{v}");
                match shape {
                    None => (format!("E{i}::{name}"), M::Named(name)),
                    Some((true, ts)) => {
                        let (es, ms): (Vec<String>, Vec<M>) = ts.iter().map(|(_, t)| self.value(t)).unzip();
                        (format!("E{i}::{name}({})", es.join(", ")), M::Variant { name, payload: Payload::Tuple(ms) })
                    }
                    Some((false, fs)) => {
                        let (es, ms): (Vec<String>, Vec<(String, M)>) = fs
                            .iter()
                            .map(|(n, t)| {
                                let (e, m) = self.value(t);
                                (format!("{n}: {e}"), (n.clone(), m))
                            })
                            .unzip();
                        (format!("E{i}::{name} {{ {} }}", es.join(", ")), M::Variant { name, payload: Payload::Struct(ms) })
                    }
                }
            }
            Ty::Union(i) => {
                let fields = self.unions[*i].clone();
                let w = self.below(fields.len());
                let (fname, fty) = &fields[w];
                let (e, _) = self.value(fty);
                // The written field's bytes, little-endian; the others read what fits in them.
                let raw: u128 = match fty {
                    Ty::F32 | Ty::F64 => {
                        let b = e.trim_start_matches("f32::from_bits(").trim_start_matches("f64::from_bits(").trim_end_matches(')');
                        u128::from_str_radix(b.trim_start_matches("0x"), 16).unwrap_or(0)
                    }
                    _ => {
                        let digits: String = e.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
                        digits.parse::<i128>().map(|v| v as u128).or_else(|_| digits.parse::<u128>()).unwrap_or(0)
                    }
                };
                let wsize = ty_bytes(fty);
                let ms = fields
                    .iter()
                    .map(|(n, t)| {
                        let size = ty_bytes(t);
                        let m = if size > wsize {
                            M::Any
                        } else {
                            let mask = if size == 16 { u128::MAX } else { (1u128 << (size * 8)) - 1 };
                            let bits = raw & mask;
                            match t {
                                Ty::F32 => M::Float { bits: bits as u64, f32: true },
                                Ty::F64 => M::Float { bits: bits as u64, f32: false },
                                Ty::Int { signed: true, .. } => {
                                    let shift = 128 - size * 8;
                                    M::Int((((bits << shift) as i128) >> shift).to_string())
                                }
                                _ => M::Int(bits.to_string()),
                            }
                        };
                        (n.clone(), m)
                    })
                    .collect();
                (format!("U{i} {{ {fname}: {e} }}"), M::Struct { name: format!("U{i}"), fields: ms })
            }
            Ty::NonZero { bits, signed } => {
                let mut v = self.int_value(*bits, *signed);
                if v == 0 {
                    v = 1;
                }
                let ts = format!("{}{bits}", if *signed { "i" } else { "u" });
                (format!("NonZero::<{ts}>::new({v}{ts}).unwrap()"), M::Int(v.to_string()))
            }
            Ty::Phantom(e) => (format!("PhantomData::<{}>", self.ty_str(e)), M::Named("PhantomData".into())),
            Ty::OsString | Ty::BoxStr | Ty::PathBuf | Ty::RcStr => {
                let s = self.string();
                match t {
                    Ty::OsString => (format!("std::ffi::OsString::from({s:?})"), M::Str(s)),
                    Ty::BoxStr => (format!("Box::<str>::from({s:?})"), M::Str(s)),
                    // No printer for PathBuf: its OsString field is printed by its own printer.
                    Ty::PathBuf => (format!("std::path::PathBuf::from({s:?})"), M::Struct { name: "PathBuf".into(), fields: vec![("inner".into(), M::Str(s))] }),
                    // The Rc printer shows an unsized value as its address.
                    _ => (format!("Rc::<str>::from({s:?})"), M::Rc { kind: "Rc", strong: 1, weak: 0, value: Box::new(M::Ptr) }),
                }
            }
            Ty::BoxSlice(e) | Ty::ArcSlice(e) => {
                let n = self.below(5);
                let (es, ms): (Vec<String>, Vec<M>) = (0..n).map(|_| self.value(e)).unzip();
                let ts = self.ty_str(e);
                if matches!(t, Ty::BoxSlice(_)) {
                    (format!("Vec::<{ts}>::from([{}]).into_boxed_slice()", es.join(", ")), M::Seq { kind: "alloc::boxed::Box<[", items: ms, ordered: true })
                } else {
                    (format!("Arc::<[{ts}]>::from(Vec::<{ts}>::from([{}]))", es.join(", ")), M::Rc { kind: "Arc", strong: 1, weak: 0, value: Box::new(M::Ptr) })
                }
            }
        }
    }

    fn defs(&self) -> String {
        let mut s = String::new();
        for (i, fs) in self.structs.iter().enumerate() {
            let f: Vec<String> = fs.iter().map(|(n, t)| format!("{n}: {}", self.ty_str(t))).collect();
            let _ = writeln!(s, "struct S{i} {{ {} }}", f.join(", "));
        }
        for (i, ts) in self.tuple_structs.iter().enumerate() {
            let f: Vec<String> = ts.iter().map(|t| self.ty_str(t)).collect();
            let _ = writeln!(s, "struct T{i}({});", f.join(", "));
        }
        for i in 0..self.unit_structs {
            let _ = writeln!(s, "struct Z{i};");
        }
        for (i, e) in self.enums.iter().enumerate() {
            let vs: Vec<String> = e
                .variants
                .iter()
                .enumerate()
                .map(|(v, shape)| match (shape, &e.repr) {
                    (Shape::Unit, Some((_, ds))) => format!("V{v} = {}", ds[v]),
                    (Shape::Unit, None) => format!("V{v}"),
                    (Shape::Tuple(ts), _) => format!("V{v}({})", ts.iter().map(|t| self.ty_str(t)).collect::<Vec<_>>().join(", ")),
                    (Shape::Struct(fs), _) => {
                        format!("V{v} {{ {} }}", fs.iter().map(|(n, t)| format!("{n}: {}", self.ty_str(t))).collect::<Vec<_>>().join(", "))
                    }
                })
                .collect();
            let repr = e.repr.as_ref().map(|(r, _)| format!("#[repr({r})] ")).unwrap_or_default();
            let _ = writeln!(s, "{repr}enum E{i} {{ {} }}", vs.join(", "));
        }
        for (i, fs) in self.unions.iter().enumerate() {
            let f: Vec<String> = fs.iter().map(|(n, t)| format!("{n}: {}", self.ty_str(t))).collect();
            let _ = writeln!(s, "#[derive(Clone, Copy)] union U{i} {{ {} }}", f.join(", "));
        }
        s
    }
}

fn ty_bytes(t: &Ty) -> u32 {
    match t {
        Ty::Int { bits, .. } => bits / 8,
        Ty::F32 => 4,
        _ => 8,
    }
}

fn key_eq(a: &M, b: &M) -> bool {
    key_cmp(a, b) == std::cmp::Ordering::Equal
}

fn key_cmp(a: &M, b: &M) -> std::cmp::Ordering {
    match (a, b) {
        (M::Int(x), M::Int(y)) => match (x.parse::<i128>(), y.parse::<i128>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => x.parse::<u128>().unwrap_or(0).cmp(&y.parse::<u128>().unwrap_or(0)),
        },
        (M::Str(x), M::Str(y)) => x.as_bytes().cmp(y.as_bytes()),
        (M::Char(x), M::Char(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

struct Program {
    source: String,
    /// (gdb expression, model)
    checks: Vec<(String, M)>,
}

fn program(seed: u64, locals: usize) -> Program {
    let mut g = Gen::new(seed);
    let mut body = Vec::new();
    let mut keep = Vec::new();
    let mut checks = Vec::new();
    for i in 0..locals {
        let ty = g.ty(0, false);
        let (expr, m) = g.value(&ty);
        body.append(&mut g.aux);
        let ts = g.ty_str(&ty);
        body.push(format!("    let v{i}: {ts} = {expr};"));
        keep.push(format!("&v{i}"));
        let mut m = m;
        // A live borrow of a top-level RefCell: the printer shows the borrow flag.
        if let M::RefCell { value, .. } = &m
            && g.chance(0.4)
        {
            let n = if g.chance(0.3) { -1 } else { 1 + g.below(2) as i64 };
            for k in 0..n.max(1) {
                let guard = if n < 0 { "borrow_mut" } else { "borrow" };
                body.push(format!("    let b{i}_{k} = v{i}.{guard}();"));
                keep.push(format!("&b{i}_{k}"));
                checks.push((format!("b{i}_{k}"), M::Guard { borrow: n, value: value.clone() }));
            }
            m = M::RefCell { borrow: n, value: value.clone() };
        }
        checks.push((format!("v{i}"), m));
        // The pointee of a top-level Box or reference.
        let pointee = g.pointee.take();
        if let (Ty::Box(_) | Ty::Ref(_), Some(p)) = (&ty, pointee) {
            checks.push((format!("*v{i}"), p));
        }
    }
    // An Rc cycle: printing must terminate.
    let cycle = g.chance(0.2);
    let mut defs = g.defs();
    if cycle {
        defs.push_str("struct Node { val: u32, next: RefCell<Option<Rc<Node>>> }\n");
        body.push("    let ca = Rc::new(Node { val: 1, next: RefCell::new(None) });".into());
        body.push("    let cb = Rc::new(Node { val: 2, next: RefCell::new(Some(ca.clone())) });".into());
        body.push("    *ca.next.borrow_mut() = Some(cb.clone());".into());
        keep.push("&ca".into());
        checks.push(("ca".into(), M::Any));
    }
    let source = format!(
        "#![allow(dead_code, unused, non_camel_case_types, overflowing_literals, unpredictable_function_pointer_comparisons)]\n\
         use std::collections::*;\nuse std::rc::{{Rc, Weak}};\nuse std::sync::Arc;\nuse std::cell::{{Cell, RefCell}};\nuse std::num::NonZero;\nuse std::marker::PhantomData;\n\
         {defs}\n#[inline(never)]\nfn bp() {{ std::hint::black_box(()); }}\n\nfn main() {{\n{}\n    bp();\n    std::hint::black_box(({},));\n}}\n",
        body.join("\n"),
        keep.join(", ")
    );
    Program { source, checks }
}

// ---- reading gdb's output ----

#[derive(Clone, Debug)]
enum G {
    Num(String),
    Char(u32),
    Str(Vec<u8>),
    Ptr,
    Opt,
    Err(String),
    Ident(String),
    Tuple(Vec<G>),
    Array(Vec<Item>),
    Named { name: String, fields: Option<Vec<(String, G)>>, items: Option<Vec<G>> },
    Coll { name: String, attrs: Vec<(String, String)>, items: Vec<Item> },
    /// `{...}`: cut off at the depth limit.
    Trunc,
}

#[derive(Clone, Debug)]
enum Item {
    Plain(G),
    Field(String, G),
    Key(G, G),
    More,
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> u8 {
        *self.s.get(self.i).unwrap_or(&0)
    }
    fn starts(&self, t: &str) -> bool {
        self.s[self.i.min(self.s.len())..].starts_with(t.as_bytes())
    }
    fn ws(&mut self) {
        while self.peek() == b' ' {
            self.i += 1;
        }
    }
    fn rest(&self) -> String {
        String::from_utf8_lossy(&self.s[self.i.min(self.s.len())..]).chars().take(60).collect()
    }
    /// Skip to the bracket closing the one at the cursor.
    fn skip_balanced(&mut self, open: u8, close: u8) {
        let mut depth = 0;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
        }
    }

    fn name(&mut self) -> String {
        let start = self.i;
        loop {
            let c = self.peek();
            if c.is_ascii_alphanumeric() || c == b'_' || c == b':' {
                self.i += 1;
            } else if c == b'&' {
                self.i += 1;
                if self.starts("mut ") {
                    self.i += 4;
                }
            } else if c == b'<' && self.i > start {
                self.skip_balanced(b'<', b'>');
            } else if c == b'[' && (self.i == start || self.s[self.i - 1] == b'&') {
                self.skip_balanced(b'[', b']');
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
    }

    fn string(&mut self) -> Result<Vec<u8>, String> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let c = self.peek();
            if self.i >= self.s.len() {
                return Err("unterminated string".into());
            }
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek();
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'a' => out.push(7),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'v' => out.push(11),
                        b'e' => out.push(27),
                        b'0'..=b'7' => {
                            let mut v = (e - b'0') as u32;
                            for _ in 0..2 {
                                if (b'0'..=b'7').contains(&self.peek()) {
                                    v = v * 8 + (self.peek() - b'0') as u32;
                                    self.i += 1;
                                }
                            }
                            out.push(v as u8);
                        }
                        b'x' => {
                            let st = self.i;
                            while self.peek().is_ascii_hexdigit() {
                                self.i += 1;
                            }
                            let v = u32::from_str_radix(std::str::from_utf8(&self.s[st..self.i]).unwrap_or("0"), 16).unwrap_or(0);
                            out.push(v as u8);
                        }
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
        Ok(out)
    }

    fn list(&mut self, close: u8) -> Result<Vec<G>, String> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            self.ws();
            if self.peek() == close {
                self.i += 1;
                return Ok(out);
            }
            out.push(self.value()?);
            self.ws();
            match self.peek() {
                b',' => self.i += 1,
                c if c == close => {}
                _ => return Err(format!("in list: {}", self.rest())),
            }
        }
    }

    fn items(&mut self, close: u8) -> Result<Vec<Item>, String> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            self.ws();
            if self.peek() == close {
                self.i += 1;
                return Ok(out);
            }
            if self.starts("...") {
                self.i += 3;
                out.push(Item::More);
            } else {
                // `name = v` or `name: v`
                let save = self.i;
                let mut field = None;
                if self.peek().is_ascii_alphabetic() || self.peek() == b'_' || self.peek() == b'*' {
                    let st = self.i;
                    self.i += (self.peek() == b'*') as usize;
                    while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
                        self.i += 1;
                    }
                    let n = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                    // `value = v` in the printers' own output; `Cell = {…}` is a value.
                    if self.starts(" = ") && n.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c == '*') {
                        self.i += 3;
                        field = Some(n);
                    } else if self.starts(": ") {
                        self.i += 2;
                        field = Some(n);
                    } else {
                        self.i = save;
                    }
                }
                match field {
                    Some(n) => out.push(Item::Field(n, self.value()?)),
                    None => {
                        let v = self.value()?;
                        self.ws();
                        // `[k] = v`
                        if self.starts("= ")
                            && let G::Array(mut k) = v.clone()
                            && k.len() == 1
                            && let Item::Plain(k) = k.remove(0)
                        {
                            self.i += 2;
                            out.push(Item::Key(k, self.value()?));
                        } else {
                            out.push(Item::Plain(v));
                        }
                    }
                }
            }
            self.ws();
            match self.peek() {
                b',' => self.i += 1,
                c if c == close => {}
                _ => return Err(format!("in items: {}", self.rest())),
            }
        }
    }

    fn value(&mut self) -> Result<G, String> {
        self.ws();
        let c = self.peek();
        match c {
            b'(' => {
                // A cast before an address: `(*mut T) 0x…`, `(&T) 0x…`.
                if self.starts("(*mut ") || self.starts("(*const ") || self.starts("(&") {
                    let save = self.i;
                    self.skip_balanced(b'(', b')');
                    if self.starts(" 0x") {
                        return self.value();
                    }
                    self.i = save;
                }
                Ok(G::Tuple(self.list(b')')?))
            }
            b'[' => Ok(G::Array(self.items(b']')?)),
            b'{' => {
                if self.starts("{...}") {
                    self.i += 5;
                    return Ok(G::Trunc);
                }
                Ok(G::Coll { name: String::new(), attrs: vec![], items: self.items(b'}')? })
            }
            b'"' => {
                let s = self.string()?;
                if self.starts("...") {
                    self.i += 3;
                }
                Ok(G::Str(s))
            }
            b'<' => {
                let st = self.i;
                self.skip_balanced(b'<', b'>');
                let t = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                Ok(if t == "<optimized out>" { G::Opt } else { G::Err(t) })
            }
            b'0' if self.starts("0x") => {
                self.i += 2;
                while self.peek().is_ascii_hexdigit() {
                    self.i += 1;
                }
                self.ws();
                // `0x… <symbol>`, `0x… "C string"`, `0x… b"bytes"` (a `&u8`).
                if self.peek() == b'<' {
                    self.skip_balanced(b'<', b'>');
                } else if self.peek() == b'"' {
                    self.string()?;
                } else if self.starts("b\"") {
                    self.i += 1;
                    self.string()?;
                }
                Ok(G::Ptr)
            }
            b'-' if matches!(self.s.get(self.i + 1), Some(b'i' | b'n')) => {
                self.i += 1;
                let n = self.name();
                if self.peek() == b'(' {
                    self.skip_balanced(b'(', b')');
                }
                Ok(G::Ident(format!("-{n}")))
            }
            b'-' | b'0'..=b'9' => {
                let st = self.i;
                self.i += 1;
                while self.peek().is_ascii_digit() || matches!(self.peek(), b'.' | b'e') || (matches!(self.peek(), b'+' | b'-') && self.s[self.i - 1] == b'e') {
                    self.i += 1;
                }
                let n = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                if self.starts(" '") {
                    self.i += 2;
                    while self.i < self.s.len() && self.peek() != b'\'' {
                        if self.peek() == b'\\' {
                            self.i += 1;
                        }
                        self.i += 1;
                    }
                    self.i += 1;
                    return Ok(G::Char(n.parse().unwrap_or(u32::MAX)));
                }
                Ok(G::Num(n))
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c == b'&' => {
                let n = self.name();
                if (n == "nan" || n == "inf") && self.peek() == b'(' {
                    self.skip_balanced(b'(', b')');
                    return Ok(G::Ident(n));
                }
                if self.peek() == b'(' {
                    let save = self.i;
                    self.i += 1;
                    let st = self.i;
                    while self.peek().is_ascii_lowercase() || self.peek() == b'_' {
                        self.i += 1;
                    }
                    if self.peek() == b'=' && self.i > st {
                        self.i = save;
                        self.skip_balanced(b'(', b')');
                        let inner = String::from_utf8_lossy(&self.s[save + 1..self.i - 1]).into_owned();
                        let attrs = inner
                            .split(", ")
                            .filter_map(|kv| kv.split_once('='))
                            .map(|(k, v)| (k.to_owned(), v.to_owned()))
                            .collect();
                        let items = if self.starts(" = {") {
                            self.i += 3;
                            self.items(b'}')?
                        } else {
                            vec![]
                        };
                        return Ok(G::Coll { name: n, attrs, items });
                    }
                    self.i = save;
                    return Ok(G::Named { name: n, fields: None, items: Some(self.list(b')')?) });
                }
                if self.starts(" = {") {
                    self.i += 3;
                    return Ok(G::Coll { name: n, attrs: vec![], items: self.items(b'}')? });
                }
                if self.starts(" [") {
                    self.i += 1;
                    return Ok(G::Coll { name: n, attrs: vec![], items: self.items(b']')? });
                }
                // `Box<[()]> 0x1`: gdb's own rendering of a slice of zero-sized elements.
                if self.starts(" 0x") {
                    return self.value();
                }
                if self.starts(" (") {
                    self.i += 1;
                    return Ok(G::Named { name: n, fields: None, items: Some(self.list(b')')?) });
                }
                if self.starts(" {") || self.peek() == b'{' {
                    self.ws();
                    let items = self.items(b'}')?;
                    let fields = items
                        .into_iter()
                        .map(|it| match it {
                            Item::Field(n, v) => (n, v),
                            Item::Plain(v) => (String::new(), v),
                            Item::Key(_, v) => (String::new(), v),
                            Item::More => ("...".into(), G::Trunc),
                        })
                        .collect();
                    return Ok(G::Named { name: n, fields: Some(fields), items: None });
                }
                Ok(G::Ident(n))
            }
            _ => Err(format!("unexpected: {}", self.rest())),
        }
    }
}

fn parse(text: &str) -> Result<G, String> {
    let mut p = P { s: text.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i < p.s.len() {
        return Err(format!("trailing: {}", p.rest()));
    }
    Ok(v)
}

/// The last `::` segment outside generic arguments, without its own generic arguments.
fn last_segment(path: &str) -> &str {
    let b = path.as_bytes();
    let (mut depth, mut start) = (0, 0);
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'<' => depth += 1,
            b'>' => depth -= 1,
            b':' if depth == 0 && b.get(i + 1) == Some(&b':') => {
                start = i + 2;
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    let seg = &path[start..];
    seg.split('<').next().unwrap_or(seg)
}

// ---- comparing ----

struct Cmp<'a> {
    opt: bool,
    out: &'a mut Vec<Mismatch>,
}

/// What is already understood: gdb's own behaviour (expected) and recorded findings.
fn known(m: &Mismatch) -> Option<&'static str> {
    // Elements made only of unit values (no scalar, string, pointer or collection inside).
    let zst = |items: &str| {
        !["Int(", "Float", "Bool(", "Char(", "Str(", "Ptr", "Seq", "Map", "Rc {", "RefCell", "Guard", "Any"].iter().any(|k| items.contains(k))
    };
    let zst_array = (m.expected.starts_with("Array([") && zst(&m.expected))
        || (m.expected.starts_with("Seq { kind: \"alloc::boxed::Box<[\"") && zst(m.expected.trim_start_matches("Seq { kind: \"alloc::boxed::Box<[\"")));
    if m.class.contains("integers of more than 8 bytes") || m.got.contains("integers of more than 8 bytes") {
        // gdb 15 cannot read an enum discriminant wider than 8 bytes; rustc widens the tag to the
        // alignment of the first field, so Option<i128>, Option<NonZero<u128>>, … have a u128 tag.
        Some("expected: gdb 15 cannot read a 128-bit enum discriminant")
    } else if m.class.starts_with("gdb error: Cannot access memory at address 0x0 at") && m.opt {
        // An optimized-out Box or reference to an enum: gdb reads the pointee to resolve the
        // variant (at address 0) instead of printing <optimized out>, as it does for a struct.
        Some("expected: gdb reads through an optimized-out pointer to an enum")
    } else if m.class == "unparsed output" && m.got.contains("(null)") && zst_array {
        // An empty Box<[T]> of a zero-sized enum: `(null){(null): <error reading variable: …>`.
        Some("expected: gdb prints an array of zero-sized elements as an address")
    } else if m.class.starts_with("shape at") && m.got == "Ptr" && zst_array {
        // gdb prints an array whose element size is 0 like a pointer to its first element.
        Some("expected: gdb prints an array of zero-sized elements as an address")
    } else if m.class.ends_with("alloc::boxed::box<[") && (m.got.starts_with("Named {") || m.got.starts_with("Err(")) {
        // gdb takes a struct whose last field is zero-sized (PhantomData, `alloc: Global` in Rc,
        // Arc, Weak, BTreeMap) for an unsized one, so a slice of them prints as one struct with
        // the length applied to that field; Box<[T]> has no Rust printer (`&[T]` has one).
        Some("expected: gdb takes a struct ending in a zero-sized field for an unsized one (Box<[T]>)")
    } else if m.class.contains("ZeroDivisionError") || m.class.contains("Cannot perform pointer math on incomplete type") {
        Some("finding 33: VecDeque, Vec and slice printers on zero-sized elements")
    } else if m.class.contains("Attempt to take contents of a non-pointer value") && (m.class.ends_with(" at ref") || m.class.ends_with(" at refmut")) {
        Some("finding 34: the Ref/RefMut printer fails on every guard")
    } else if (m.class.contains("btreemap") || m.class.contains("btreeset")) && m.got == "Tuple([])" {
        Some("finding 33: BTreeMap/BTreeSet printer shows () for every zero-sized key or value")
    } else {
        None
    }
}

#[derive(Serialize, Clone, Debug)]
struct Mismatch {
    /// What, at which kind path: groups mismatches into classes.
    class: String,
    path: String,
    expected: String,
    got: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    known: Option<&'static str>,
    /// Seen at -Copt-level above 0.
    #[serde(skip)]
    opt: bool,
}

fn short<T: std::fmt::Debug>(v: &T) -> String {
    let s = format!("{v:?}");
    if s.chars().count() > 160 { format!("{}…", s.chars().take(160).collect::<String>()) } else { s }
}

impl Cmp<'_> {
    fn bad(&mut self, what: &str, kinds: &str, path: &str, m: &M, g: &G) {
        self.out.push(Mismatch { known: None, opt: false, class: format!("{what} at {kinds}"), path: path.into(), expected: short(m), got: short(g) });
    }

    fn eq(&mut self, m: &M, g: &G) -> bool {
        let mut v = Vec::new();
        Cmp { opt: self.opt, out: &mut v }.cmp(m, g, "", "");
        v.is_empty()
    }

    fn cmp(&mut self, m: &M, g: &G, path: &str, kinds: &str) {
        let kinds = if kinds.is_empty() { m.kind() } else { format!("{kinds}>{}", m.kind()) };
        match g {
            G::Opt if self.opt => return,
            G::Opt => return self.bad("optimized out at O0", &kinds, path, m, g),
            G::Trunc => return,
            G::Err(e) => return self.bad(&format!("error {}", e.split(':').next().unwrap_or("")), &kinds, path, m, g),
            _ => {}
        }
        match (m, g) {
            (M::Any, _) => {}
            (M::Int(a), G::Num(b)) if a == b => {}
            (M::Int(a), G::Char(b)) if a.parse::<u32>().ok() == Some(*b) => {}
            (M::Int(_), _) => self.bad("value", &kinds, path, m, g),
            (M::Float { bits, f32 }, G::Num(t) | G::Ident(t)) => {
                let want = if *f32 { f32::from_bits(*bits as u32) as f64 } else { f64::from_bits(*bits) };
                let ok = if want.is_nan() {
                    t.trim_start_matches('-').starts_with("nan")
                } else if want.is_infinite() {
                    t == if want > 0.0 { "inf" } else { "-inf" }
                } else {
                    match t.parse::<f64>() {
                        Ok(got) if *f32 => (got as f32) == (want as f32) && (got as f32).is_sign_negative() == (want as f32).is_sign_negative(),
                        Ok(got) => got == want && got.is_sign_negative() == want.is_sign_negative(),
                        Err(_) => false,
                    }
                };
                if !ok {
                    self.bad("value", &kinds, path, m, g)
                }
            }
            (M::Bool(b), G::Ident(t)) if t == if *b { "true" } else { "false" } => {}
            (M::Char(c), G::Char(n)) if c == n => {}
            (M::Str(s), G::Str(b)) if s.as_bytes() == b.as_slice() => {}
            (M::Unit, G::Tuple(v)) if v.is_empty() => {}
            (M::Ptr, G::Ptr) => {}
            (M::Tuple(ms), G::Tuple(gs)) => self.elems(ms, gs, path, &kinds),
            (M::Array(ms), G::Array(gs)) => self.items(ms, gs, true, path, &kinds),
            (M::Seq { kind, items, ordered }, G::Coll { name, attrs, items: gs }) if name.starts_with(kind) || name.ends_with(kind) => {
                self.size(items.len(), attrs, path, &kinds, m, g);
                self.items(items, gs, *ordered, path, &kinds)
            }
            (M::Map { kind, pairs, ordered }, G::Coll { name, attrs, items: gs }) if name.ends_with(kind) => {
                self.size(pairs.len(), attrs, path, &kinds, m, g);
                let got: Vec<(&G, &G)> = gs.iter().filter_map(|it| if let Item::Key(k, v) = it { Some((k, v)) } else { None }).collect();
                let truncated = gs.iter().any(|it| matches!(it, Item::More));
                if got.len() != pairs.len() && !truncated {
                    return self.bad("entries", &kinds, path, m, g);
                }
                if *ordered {
                    for (i, ((mk, mv), (gk, gv))) in pairs.iter().zip(&got).enumerate() {
                        self.cmp(mk, gk, &format!("{path}.key[{i}]"), &kinds);
                        self.cmp(mv, gv, &format!("{path}[{i}]"), &kinds);
                    }
                } else {
                    let mut used = vec![false; got.len()];
                    for (mk, mv) in pairs {
                        match (0..got.len()).find(|&j| !used[j] && self.eq(mk, got[j].0)) {
                            Some(j) => {
                                used[j] = true;
                                self.cmp(mv, got[j].1, &format!("{path}[{}]", short(mk)), &kinds);
                            }
                            None if !truncated => return self.bad("missing key", &kinds, path, mk, g),
                            None => {}
                        }
                    }
                }
            }
            (M::Struct { name, fields }, G::Named { name: gn, fields: Some(gf), .. }) if last_segment(gn) == name => {
                self.fields(fields, gf, path, &kinds, m, g)
            }
            (M::TupleStruct { name, items }, G::Named { name: gn, items: Some(gi), .. }) if last_segment(gn) == name => self.elems(items, gi, path, &kinds),
            (M::Named(n), G::Ident(t)) if last_segment(t) == n => {}
            (M::Variant { name, payload: Payload::Tuple(ms) }, G::Ident(t)) if ms.is_empty() && last_segment(t) == name => {}
            (M::Variant { name, payload: Payload::Tuple(ms) }, G::Named { name: gn, items: Some(gi), .. }) if last_segment(gn) == name => {
                self.elems(ms, gi, path, &kinds)
            }
            (M::Variant { name, payload: Payload::Struct(fs) }, G::Named { name: gn, fields: Some(gf), .. }) if last_segment(gn) == name => {
                self.fields(fs, gf, path, &kinds, m, g)
            }
            (M::Rc { kind, strong, weak, value }, G::Coll { name, attrs, items }) if name == kind => {
                let attr = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
                if attr("strong") != Some(strong.to_string()) || attr("weak") != Some(weak.to_string()) {
                    self.bad("counts", &kinds, path, m, g);
                }
                self.field_value(value, items, path, &kinds, m, g)
            }
            (M::RefCell { borrow, value }, G::Coll { name, attrs, items }) if name == "RefCell" => {
                let flag = if *borrow < 0 { ("borrow_mut", (-borrow).to_string()) } else { ("borrow", borrow.to_string()) };
                if attrs.iter().find(|(a, _)| a == flag.0).map(|(_, v)| v.clone()) != Some(flag.1) {
                    self.bad("borrow flag", &kinds, path, m, g);
                }
                self.field_value(value, items, path, &kinds, m, g)
            }
            (M::Cell(value), G::Coll { name, items, .. }) if name == "Cell" => self.field_value(value, items, path, &kinds, m, g),
            (M::Guard { borrow, value }, G::Coll { name, attrs, items }) if name == "Ref" => {
                let flag = if *borrow < 0 { ("borrow_mut", (-borrow).to_string()) } else { ("borrow", borrow.to_string()) };
                if attrs.iter().find(|(a, _)| a == flag.0).map(|(_, v)| v.clone()) != Some(flag.1) {
                    self.bad("borrow flag", &kinds, path, m, g);
                }
                match items.iter().find_map(|it| if let Item::Field(n, v) = it { (n == "*value").then_some(v) } else { None }) {
                    Some(v) => self.cmp(value, v, &format!("{path}.value"), &kinds),
                    None => self.bad("no value field", &kinds, path, m, g),
                }
            }
            _ => self.bad("shape", &kinds, path, m, g),
        }
    }

    fn size(&mut self, n: usize, attrs: &[(String, String)], path: &str, kinds: &str, m: &M, g: &G) {
        if let Some((_, v)) = attrs.iter().find(|(a, _)| a == "size")
            && v != &n.to_string()
        {
            self.bad("size", kinds, path, m, g);
        }
    }

    fn field_value(&mut self, value: &M, items: &[Item], path: &str, kinds: &str, m: &M, g: &G) {
        match items.iter().find_map(|it| if let Item::Field(n, v) = it { (n == "value").then_some(v) } else { None }) {
            Some(v) => self.cmp(value, v, &format!("{path}.value"), kinds),
            None => self.bad("no value field", kinds, path, m, g),
        }
    }

    fn elems(&mut self, ms: &[M], gs: &[G], path: &str, kinds: &str) {
        if ms.len() != gs.len() {
            self.out.push(Mismatch { known: None, opt: false, class: format!("length at {kinds}"), path: path.into(), expected: short(&ms), got: short(&gs) });
            return;
        }
        for (i, (m, g)) in ms.iter().zip(gs).enumerate() {
            self.cmp(m, g, &format!("{path}.{i}"), kinds);
        }
    }

    fn items(&mut self, ms: &[M], gs: &[Item], ordered: bool, path: &str, kinds: &str) {
        let truncated = gs.iter().any(|it| matches!(it, Item::More));
        let plain: Vec<&G> = gs.iter().filter_map(|it| if let Item::Plain(g) = it { Some(g) } else { None }).collect();
        if plain.len() != ms.len() && !(truncated && plain.len() < ms.len()) {
            self.out.push(Mismatch { known: None, opt: false, class: format!("length at {kinds}"), path: path.into(), expected: format!("{} items", ms.len()), got: short(&gs) });
            return;
        }
        if ordered {
            for (i, (m, g)) in ms.iter().zip(plain).enumerate() {
                self.cmp(m, g, &format!("{path}[{i}]"), kinds);
            }
        } else {
            let mut used = vec![false; plain.len()];
            for m in ms {
                match (0..plain.len()).find(|&j| !used[j] && self.eq(m, plain[j])) {
                    Some(j) => used[j] = true,
                    None if !truncated => {
                        self.out.push(Mismatch { known: None, opt: false, class: format!("missing element at {kinds}"), path: path.into(), expected: short(m), got: short(&gs) });
                        return;
                    }
                    None => {}
                }
            }
        }
    }

    fn fields(&mut self, ms: &[(String, M)], gf: &[(String, G)], path: &str, kinds: &str, m: &M, g: &G) {
        for (n, fm) in ms {
            match gf.iter().find(|(gn, _)| gn == n) {
                Some((_, fg)) => self.cmp(fm, fg, &format!("{path}.{n}"), kinds),
                None if gf.iter().any(|(gn, _)| gn == "...") => {}
                None => self.bad(&format!("missing field {n}"), kinds, path, m, g),
            }
        }
    }
}

// ---- running ----

/// An error message without the names and numbers that vary between programs.
fn normalize_message(m: &str) -> String {
    let m = m.trim();
    let mut out = String::new();
    let mut quoted = false;
    for c in m.chars() {
        if c == '"' {
            quoted = !quoted;
            if quoted {
                out.push_str("\"…\"");
            }
        } else if !quoted {
            out.push(c);
        }
    }
    out.chars().take(120).collect()
}

#[derive(Serialize, Default)]
struct OptResult {
    status: String,
    mismatches: Vec<Mismatch>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    printer_errors: Vec<String>,
}

#[derive(Serialize)]
struct SeedResult {
    seed: u64,
    opts: BTreeMap<u32, OptResult>,
}

fn gdb_commands(checks: &[(String, M)]) -> Vec<String> {
    let mut c: Vec<String> = [
        "set pagination off", "set print pretty off", "set width 0", "set height 0", "set confirm off",
        "set print elements 20000", "set print repeats unlimited", "set print max-depth 100", "break m::bp", "run", "up",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for (e, _) in checks {
        c.push(format!("echo @@{e}\\n"));
        c.push(format!("print {e}"));
    }
    for s in ["echo @@locals\\n", "info locals", "echo @@end\\n", "kill", "quit"] {
        c.push(s.into());
    }
    c
}

fn run_one(args: &Args, tc: &Path, seed: u64) -> SeedResult {
    let prog = program(seed, args.locals);
    let mut res = SeedResult { seed, opts: BTreeMap::new() };
    let dir = match tempfile::tempdir_in(&args.work) {
        Ok(d) => d,
        Err(_) => return res,
    };
    let _ = std::fs::write(dir.path().join("m.rs"), &prog.source);
    let commands = gdb_commands(&prog.checks);
    let _ = std::fs::write(dir.path().join("cmds"), commands.join("\n") + "\n");
    let path_env = format!("{}:{}", tc.join("bin").display(), std::env::var("PATH").unwrap_or_default());
    for opt in args.opt.split(',').filter_map(|o| o.parse::<u32>().ok()) {
        let mut r = OptResult::default();
        let bin = format!("m{opt}");
        let mut cc = Command::new(tc.join("bin/rustc"));
        cc.args(["--edition", "2021", "-g", &format!("-Copt-level={opt}"), "m.rs", "-o", &bin]).current_dir(dir.path());
        match run_command(cc, Duration::from_secs(300)) {
            Ok(d) if d.exit == Exit::Code(0) => {}
            Ok(d) => {
                r.status = "compile-error".into();
                r.printer_errors.push(d.stderr_text().lines().filter(|l| l.starts_with("error")).take(3).collect::<Vec<_>>().join(" | "));
                res.opts.insert(opt, r);
                continue;
            }
            Err(e) => {
                r.status = format!("harness: {e}");
                res.opts.insert(opt, r);
                continue;
            }
        }
        // One -ex per command: an error in a sourced file would abort the rest. stderr into
        // stdout, so a printer's exception lands in the section of the value it broke.
        let mut g = Command::new("sh");
        g.args(["-c", "exec \"$0\" \"$@\" 2>&1"]).arg(tc.join("bin/rust-gdb")).args(["-batch", "-nx", "-iex", "set debuginfod enabled off"]);
        for c in &commands {
            g.args(["-ex", c]);
        }
        g.arg(&bin)
            .current_dir(dir.path())
            .env("PATH", &path_env)
            .env("TERM", "dumb");
        let done = match run_command(g, Duration::from_secs(args.timeout)) {
            Ok(d) => d,
            Err(e) => {
                r.status = format!("harness: {e}");
                res.opts.insert(opt, r);
                continue;
            }
        };
        let text = format!("{}\n{}", done.stdout_text(), done.stderr_text());
        let _ = std::fs::write(dir.path().join(format!("gdb-O{opt}.txt")), &text);
        r.status = match done.exit {
            Exit::Timeout => "timeout".into(),
            Exit::Signal(s) => format!("gdb killed by signal {s}"),
            _ => "ok".into(),
        };
        if text.contains("internal-error") || text.contains("Fatal signal") {
            r.status = "gdb crash".into();
        }
        // Sections between markers.
        let mut sections: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut cur = String::new();
        for line in done.stdout_text().lines() {
            if let Some(m) = line.strip_prefix("@@") {
                cur = m.to_owned();
            } else if !cur.is_empty() {
                sections.entry(cur.clone()).or_default().push(line.to_owned());
            }
        }
        // `info locals` prints every local again: only that it terminates is checked (its
        // exceptions repeat the ones of each value's own `print`).
        let opt_on = opt > 0;
        let mut seen = 0;
        for (e, m) in &prog.checks {
            let Some(lines) = sections.get(e) else { continue };
            seen += 1;
            let Some(line) = lines.iter().find(|l| l.starts_with('$')) else {
                let err = lines.iter().find(|l| !l.trim().is_empty()).map_or(String::new(), |l| normalize_message(l));
                if opt_on && err == "value has been optimized out" {
                    continue;
                }
                r.mismatches.push(Mismatch { known: None, opt: false, class: format!("gdb error: {err} at {}", m.kind()), path: e.clone(), expected: short(m), got: lines.join(" | ").chars().take(300).collect() });
                continue;
            };
            // A printer's exception is printed inside the value it was printing.
            if line.contains("Python Exception") {
                let mut msgs: Vec<String> = line.split("Python Exception ").skip(1).map(normalize_message).collect();
                msgs.sort();
                msgs.dedup();
                for msg in msgs {
                    r.mismatches.push(Mismatch { known: None, opt: false, class: format!("printer exception {msg} at {}", m.kind()), path: e.clone(), expected: short(m), got: line.chars().take(300).collect() });
                }
                continue;
            }
            let value = line.split_once(" = ").map_or("", |x| x.1);
            match parse(value) {
                Ok(g) => {
                    let mut out = Vec::new();
                    Cmp { opt: opt_on, out: &mut out }.cmp(m, &g, e, "");
                    r.mismatches.extend(out);
                }
                Err(err) => r.mismatches.push(Mismatch { known: None, opt: false, class: "unparsed output".into(), path: e.clone(), expected: short(m), got: format!("{err}: {}", value.chars().take(200).collect::<String>()) }),
            }
        }
        if seen == 0 && r.status == "ok" {
            r.status = "no output".into();
        }
        res.opts.insert(opt, r);
    }
    for (opt, r) in res.opts.iter_mut() {
        for m in &mut r.mismatches {
            m.opt = *opt > 0;
            m.known = known(m);
        }
    }
    let bad = res.opts.values().any(|r| r.mismatches.iter().any(|m| m.known.is_none()) || !r.printer_errors.is_empty() || !matches!(r.status.as_str(), "ok" | "compile-error"));
    if bad {
        let f = args.work.join("findings").join(format!("seed-{seed}"));
        let _ = std::fs::create_dir_all(&f);
        for e in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.ends_with(".rs") || n.ends_with(".txt") || n == "cmds" {
                let _ = std::fs::copy(e.path(), f.join(&n));
            }
        }
        let _ = std::fs::write(f.join("finding.json"), serde_json::to_string_pretty(&res).unwrap_or_default());
    }
    res
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let (lo, hi) = args.seeds.split_once("..").ok_or_else(|| anyhow::anyhow!("--seeds takes lo..hi"))?;
    let (lo, hi): (u64, u64) = (lo.parse()?, hi.parse()?);
    let home = std::env::var("HOME")?;
    let tc = PathBuf::from(home).join(format!(".rustup/toolchains/{}-x86_64-unknown-linux-gnu", args.toolchain));
    anyhow::ensure!(tc.join("bin/rust-gdb").exists(), "no rust-gdb in {}", tc.display());
    let seeds: Vec<u64> = (lo..hi).collect();
    let out = Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(args.work.join("results.jsonl"))?);
    let done = AtomicUsize::new(0);
    let classes: Mutex<BTreeMap<String, (usize, u64)>> = Mutex::new(BTreeMap::new());
    let statuses: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    pool.install(|| {
        seeds.par_iter().for_each(|&seed| {
            let r = run_one(&args, &tc, seed);
            {
                let mut c = classes.lock().unwrap();
                let mut s = statuses.lock().unwrap();
                for (opt, o) in &r.opts {
                    *s.entry(format!("O{opt} {}", o.status)).or_default() += 1;
                    for m in &o.mismatches {
                        let key = match m.known {
                            Some(k) => format!("[{k}]"),
                            None => format!("O{opt} {}", m.class),
                        };
                        let e = c.entry(key).or_insert((0, seed));
                        e.0 += 1;
                    }
                    for p in &o.printer_errors {
                        if o.status != "compile-error" {
                            let key: String = p.split(':').take(2).collect::<Vec<_>>().join(":");
                            let e = c.entry(format!("O{opt} printer error {key}")).or_insert((0, seed));
                            e.0 += 1;
                        }
                    }
                }
            }
            if let Ok(mut f) = out.lock() {
                use std::io::Write;
                let _ = writeln!(f, "{}", serde_json::to_string(&r).unwrap_or_default());
            }
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 100 == 0 {
                println!("{n}/{} done", seeds.len());
            }
        })
    });
    println!("statuses: {:?}", statuses.into_inner().unwrap());
    let classes = classes.into_inner().unwrap();
    let mut sorted: Vec<_> = classes.iter().collect();
    sorted.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    for (class, (n, seed)) in sorted {
        println!("{n:6}  {class}  (first: seed {seed})");
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gdb_values() {
        for s in [
            "Vec(size=1) = {p::S {a: 1, b: -2, c: (true, 120 'x')}}",
            "HashMap(size=1) = {[1] = true}",
            "core::option::Option<&u32>::Some(0x555555560ccc)",
            "(*mut p::E) 0x5555555d3f30",
            "Rc(strong=1, weak=1) = {value = p::E::B(9), strong = 1, weak = 1}",
            "&[i32](size=2) = {4, 5}",
            "Vec(size=0)",
            "Cell = {value = 6}",
            "\"hi\\\"there\"",
            "-nan(0x400000)",
            "[1, 2, 3]",
        ] {
            assert!(parse(s).is_ok(), "{s}: {:?}", parse(s));
        }
        assert_eq!(last_segment("core::option::Option<core::num::nonzero::NonZero<u32>>::Some"), "Some");
        assert_eq!(last_segment("core::marker::PhantomData<u8>"), "PhantomData");
    }

    #[test]
    fn programs_are_deterministic() {
        assert_eq!(program(7, 12).source, program(7, 12).source);
    }
}
