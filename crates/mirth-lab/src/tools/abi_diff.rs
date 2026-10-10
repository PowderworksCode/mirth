//! ABI differential: rustc's `extern "C"` must lower a signature the way clang lowers the same
//! C signature, on every target both support.
//!
//! Generates random C signatures (bool, integers of each width, float, double, pointers,
//! `__int128` on 64-bit targets, and repr(C) structs, unions and arrays of them, nested, packed
//! or over-aligned), writes each as a C function (clang, the target's LLVM triple, CPU and
//! features) and a Rust `#[no_mangle] extern "C" fn` (rustc against minicore, no sysroot), and
//! compares the two LLVM IR signatures parameter by parameter after first-class aggregates are
//! flattened. A difference in register class, extension, inreg/byval/sret, byval alignment,
//! parameter count or calling convention is a finding; representation-only differences are
//! notes. Verified equivalences per architecture, known bugs (#163911, findings 19 and 20) and
//! two differences this host cannot decide are labelled.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::sync::LazyLock;

use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long)]
    rustc: PathBuf,
    /// The rust checkout (for tests/auxiliary/minicore.rs).
    #[arg(long)]
    rust: PathBuf,
    #[arg(long)]
    work: PathBuf,
    /// Comma-separated targets (default: the main tier 1 and 2 targets).
    #[arg(long)]
    targets: Option<String>,
    /// Every target rustc knows (the non-main ones still show representation differences).
    #[arg(long)]
    all: bool,
    #[arg(long, default_value_t = 200)]
    count: usize,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[arg(long, default_value_t = 8)]
    jobs: usize,
    #[arg(long, default_value = "clang")]
    clang: String,
}

const MAIN: &[&str] = &[
    "x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu",
    "x86_64-apple-darwin", "i686-unknown-linux-gnu", "i686-pc-windows-msvc", "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin", "aarch64-pc-windows-msvc", "aarch64-unknown-linux-musl", "armv7-unknown-linux-gnueabihf",
    "arm-unknown-linux-gnueabi", "thumbv7em-none-eabihf", "riscv64gc-unknown-linux-gnu", "riscv32imac-unknown-none-elf",
    "loongarch64-unknown-linux-gnu", "powerpc64le-unknown-linux-gnu", "s390x-unknown-linux-gnu", "wasm32-unknown-unknown",
    "wasm32-wasip1",
];

// ---- generation ----

/// splitmix64: a small deterministic generator, so a seed names a program.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn float(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
}

const SCALARS: &[(&str, &str)] = &[
    ("_Bool", "bool"), ("signed char", "i8"), ("unsigned char", "u8"), ("short", "i16"), ("unsigned short", "u16"),
    ("int", "i32"), ("unsigned int", "u32"), ("long long", "i64"), ("unsigned long long", "u64"), ("float", "f32"),
    ("double", "f64"), ("void*", "*mut u8"),
];
const WIDE: &[(&str, &str)] = &[("__int128", "i128"), ("unsigned __int128", "u128")];

#[derive(Clone)]
enum Field {
    Scalar(String, String),
    Array(String, String, usize),
    Named(String),
}

struct Aggregate {
    name: String,
    union: bool,
    packed: bool,
    align: Option<u32>,
    fields: Vec<Field>,
}

struct Gen {
    rng: Rng,
    wide: bool,
    aggregates: Vec<Aggregate>,
    names: usize,
}

impl Gen {
    fn scalar(&mut self) -> (String, String) {
        let pool: Vec<(&str, &str)> = SCALARS.iter().chain(if self.wide { WIDE } else { &[] }).copied().collect();
        let (c, r) = *self.rng.pick(&pool);
        (c.into(), r.into())
    }
    fn field(&mut self, depth: u32) -> Field {
        let r = self.rng.float();
        if depth < 2 && r < 0.15 {
            return Field::Named(self.aggregate(depth + 1));
        }
        let (c, rs) = self.scalar();
        if r < 0.25 {
            let n = *self.rng.pick(&[1, 2, 3, 4, 8]);
            return Field::Array(c, rs, n);
        }
        Field::Scalar(c, rs)
    }
    fn aggregate(&mut self, depth: u32) -> String {
        let name = format!("S{}", self.names);
        self.names += 1;
        let union = self.rng.float() < 0.12;
        let packed = !union && self.rng.float() < 0.08;
        // Rust rejects packed with align, and a packed type holding an over-aligned one.
        let align = if packed { None } else { *self.rng.pick(&[None, None, None, None, None, None, None, None, None, Some(16), Some(32)]) };
        let n = 1 + self.rng.below(5);
        let fields = (0..n).map(|_| self.field(if packed { 2 } else { depth })).collect();
        self.aggregates.push(Aggregate { name: name.clone(), union, packed, align, fields });
        name
    }
    /// (C type, Rust type)
    fn ty(&mut self) -> (String, String) {
        if self.rng.float() < 0.4 {
            let n = self.aggregate(0);
            (n.clone(), n)
        } else {
            self.scalar()
        }
    }
}

fn program(wide: bool, seed: u64, count: usize) -> (String, String, Vec<String>) {
    let mut g = Gen { rng: Rng(seed), wide, aggregates: Vec::new(), names: 0 };
    let mut fns = Vec::new();
    for k in 0..count {
        let n = g.rng.below(9);
        let params: Vec<(String, String)> = (0..n).map(|_| g.ty()).collect();
        let ret = if g.rng.float() < 0.15 { None } else { Some(g.ty()) };
        fns.push((format!("f{k}"), params, ret));
    }
    let mut c = vec!["#include <stdbool.h>".to_owned()];
    let mut rs: Vec<String> = [
        "#![feature(no_core)]", "#![no_core]", "#![crate_type = \"lib\"]",
        "#![allow(improper_ctypes_definitions, unused, non_snake_case)]", "extern crate minicore;", "use minicore::*;",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for a in &g.aggregates {
        let kw = if a.union { "union" } else { "struct" };
        let cf: Vec<String> = a.fields.iter().enumerate().map(|(i, f)| match f {
            Field::Scalar(ct, _) => format!("{ct} f{i};"),
            Field::Array(ct, _, n) => format!("{ct} f{i}[{n}];"),
            Field::Named(n) => format!("{n} f{i};"),
        }).collect();
        let rf: Vec<String> = a.fields.iter().enumerate().map(|(i, f)| match f {
            Field::Scalar(_, rt) => format!("pub f{i}: {rt},"),
            Field::Array(_, rt, n) => format!("pub f{i}: [{rt}; {n}],"),
            Field::Named(n) => format!("pub f{i}: {n},"),
        }).collect();
        let cattr = format!("{}{}", if a.packed { " __attribute__((packed))" } else { "" }, a.align.map(|x| format!(" __attribute__((aligned({x})))")).unwrap_or_default());
        c.push(format!("typedef {kw} {} {{ {} }}{cattr} {};", a.name, cf.join(" "), a.name));
        let repr = format!("C{}{}", if a.packed { ", packed" } else { "" }, a.align.map(|x| format!(", align({x})")).unwrap_or_default());
        rs.push(format!("#[repr({repr})] pub {kw} {} {{ {} }}", a.name, rf.join(" ")));
        // Union fields must be Copy; minicore has no derive.
        rs.push(format!("impl Copy for {} {{}}", a.name));
    }
    let mut names = Vec::new();
    for (name, params, ret) in &fns {
        let cp = if params.is_empty() { "void".into() } else { params.iter().enumerate().map(|(i, t)| format!("{} a{i}", t.0)).collect::<Vec<_>>().join(", ") };
        c.push(format!("{} {name}({cp}) {{ for (;;); }}", ret.as_ref().map_or("void", |r| r.0.as_str())));
        let rp = params.iter().enumerate().map(|(i, t)| format!("a{i}: {}", t.1)).collect::<Vec<_>>().join(", ");
        let rr = ret.as_ref().map(|r| format!(" -> {}", r.1)).unwrap_or_default();
        rs.push(format!("#[no_mangle] pub extern \"C\" fn {name}({rp}){rr} {{ loop {{}} }}"));
        names.push(name.clone());
    }
    (c.join("\n") + "\n", rs.join("\n") + "\n", names)
}

// ---- reading LLVM IR signatures ----

#[derive(Clone, Debug, PartialEq)]
struct Param {
    ty: String,
    attrs: Vec<String>,
    noundef: bool,
}

#[derive(Clone, Debug)]
struct Sig {
    cc: Vec<String>,
    ret: Vec<String>,
    ret_attrs: Vec<String>,
    params: Vec<Param>,
}

static DEFINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^define\s+(.*?)@(\w+)\((.*)\)(.*)\{\s*$").unwrap());
static DROP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(noundef|nonnull|noalias|nocapture|readonly|readnone|writeonly|writable|dead_on_unwind|captures\([^)]*\)|dereferenceable(_or_null)?\(\d+\)|initializes\([^)]*\)|range\([^)]*\)|nofpclass\([^)]*\)|immarg|returned|local_unnamed_addr|unnamed_addr|dso_local|dso_preemptable|hidden|protected|internal|private|nounwind|noinline|optnone|!\w+ !\d+|#\d+)\b").unwrap()
});
static NAMED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(%[\w.]+) = type (.*)$").unwrap());
static CC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\w+cc|cc \d+)\b").unwrap());
static RET_TY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\{[^{}]*\}|<\{[^{}]*\}>|\[[^\]]*\]|<[^>]*>|[\w.%*]+)\s*$").unwrap());
static PARAM_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+%[\w.]+$").unwrap());
static PARAM_TY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\{[^}]*\}|\[[^\]]*\]|<[^>]*>|[\w.%*]+)(.*)$").unwrap());
static ABI_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(zeroext|signext|inreg|byval\([^)]*\)|sret\([^)]*\)|byref\([^)]*\)|align \d+|inalloca\([^)]*\))").unwrap()
});
static ONE_ELEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[1 x (.*)\]$").unwrap());
static ARRAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[(\d+) x (i\d+|float|double)\]$").unwrap());
static INT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^i(\d+)$").unwrap());
static INT_ARRAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[\d+ x i\d+\]$").unwrap());

fn split_top(s: &str) -> Vec<String> {
    let (mut out, mut depth, mut cur) = (Vec::new(), 0i32, String::new());
    for ch in s.chars() {
        match ch {
            '(' | '{' | '[' | '<' => depth += 1,
            ')' | '}' | ']' | '>' => depth -= 1,
            _ => {}
        }
        if ch == ',' && depth == 0 {
            out.push(cur.trim().to_owned());
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_owned());
    }
    out
}

fn param(p: &str) -> (String, Vec<String>, bool) {
    let p = PARAM_NAME.replace(p.trim(), "").into_owned();
    let noundef = p.contains("noundef");
    let (ty, attrs) = match PARAM_TY.captures(&p) {
        Some(c) => (c[1].to_owned(), c[2].to_owned()),
        None => (p.clone(), String::new()),
    };
    let attrs = DROP.replace_all(&attrs, "");
    let mut kept: Vec<String> = ABI_ATTR
        .find_iter(&attrs)
        .map(|m| {
            let a = m.as_str();
            if ["byval", "sret", "byref", "inalloca"].iter().any(|k| a.starts_with(k)) { a.split('(').next().unwrap().to_owned() } else { a.to_owned() }
        })
        .collect();
    // An `align` on a plain pointer is a hint, not ABI; on byval and sret it is ABI.
    if !kept.iter().any(|k| k == "byval" || k == "sret" || k == "byref") {
        kept.retain(|k| !k.starts_with("align"));
    }
    kept.sort();
    (ty, kept, noundef)
}

fn flatten(ty: &str, named: &BTreeMap<String, String>) -> Vec<String> {
    let mut ty = ty.trim().to_owned();
    if let Some(body) = named.get(&ty) {
        ty = body.clone();
    }
    if ty.starts_with("<{") && ty.ends_with("}>") {
        ty = ty[1..ty.len() - 1].to_owned();
    }
    if ty.starts_with('{') && ty.ends_with('}') {
        return split_top(&ty[1..ty.len() - 1]).iter().flat_map(|p| flatten(p, named)).collect();
    }
    if let Some(c) = ONE_ELEM.captures(&ty) {
        return flatten(&c[1], named);
    }
    vec![ty]
}

fn signatures(ll: &str) -> BTreeMap<String, Sig> {
    let named: BTreeMap<String, String> = NAMED.captures_iter(ll).map(|c| (c[1].to_owned(), c[2].trim().to_owned())).collect();
    let mut out = BTreeMap::new();
    for line in ll.lines().filter(|l| l.starts_with("define")) {
        let Some(c) = DEFINE.captures(line) else { continue };
        let cc: Vec<String> = CC.find_iter(&c[1]).map(|m| m.as_str().to_owned()).collect();
        let head = DROP.replace_all(&c[1], "").trim().to_owned();
        let (ret_ty, prefix) = match RET_TY.find(&head) {
            Some(m) => (m.as_str().trim().to_owned(), head[..m.start()].to_owned()),
            None => ("void".to_owned(), String::new()),
        };
        let mut ret_attrs: Vec<String> = prefix.split_whitespace().filter(|a| ["zeroext", "signext", "inreg"].contains(a)).map(str::to_owned).collect();
        ret_attrs.sort();
        let params = split_top(&c[3])
            .iter()
            .flat_map(|p| {
                let (ty, attrs, noundef) = param(p);
                flatten(&ty, &named).into_iter().map(move |t| Param { ty: t, attrs: attrs.clone(), noundef })
            })
            .collect();
        out.insert(c[2].to_owned(), Sig { cc, ret: flatten(&ret_ty, &named), ret_attrs, params });
    }
    out
}

// ---- comparing ----

fn klass(ty: &str) -> String {
    match ty {
        "float" | "double" | "half" | "bfloat" | "fp128" | "x86_fp80" | "ppc_fp128" => "fp".into(),
        "void" => "void".into(),
        _ if ty.starts_with('<') => "vec".into(),
        _ if ty.starts_with('[') => {
            let inner = ty.trim_start_matches('[').trim_end_matches(']');
            match inner.split_once(" x ") {
                Some((n, e)) => format!("[{n} x {}]", klass(e)),
                None => ty.into(),
            }
        }
        _ => "int".into(),
    }
}

/// Register classes, one per register-sized unit.
fn units(types: &[String], arch: &str, ret: bool, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for t in types {
        if let Some(c) = INT.captures(t)
            && c[1].parse::<usize>().unwrap_or(0) > width
        {
            out.extend(std::iter::repeat_n("int".to_owned(), c[1].parse::<usize>().unwrap() / width));
        } else if let Some(c) = ARRAY.captures(t)
            && (ret || (arch == "arm" && (&c[2] == "float" || &c[2] == "double")))
        {
            // A float array is a homogeneous aggregate: in a return, or an ARM VFP argument, it
            // takes consecutive registers like a struct of its elements.
            let elems: Vec<String> = std::iter::repeat_n(c[2].to_owned(), c[1].parse().unwrap_or(0)).collect();
            out.extend(units(&elems, arch, ret, width));
        } else if t == "agg" {
            out.push("agg".into());
        } else {
            let mut k = klass(t);
            // x86's SSE registers hold floats, doubles and vectors alike.
            if (arch == "x86_64" || arch == "x86") && (k == "fp" || k == "vec") {
                k = "sse".into();
            }
            out.push(k);
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Diff {
    CallingConvention { clang: Vec<String>, rustc: Vec<String> },
    ReturnAttributes { clang: Vec<String>, rustc: Vec<String>, ret: Vec<String> },
    Return { clang: Vec<String>, rustc: Vec<String> },
    Parameters { clang: Vec<String>, rustc: Vec<String> },
    ParameterAttributes { index: usize, clang: Vec<String>, rustc: Vec<String>, ty: String },
}

impl Diff {
    fn label(&self) -> &'static str {
        match self {
            Diff::CallingConvention { .. } => "calling convention",
            Diff::ReturnAttributes { .. } => "return attributes",
            Diff::Return { .. } => "return",
            Diff::Parameters { .. } => "parameters",
            Diff::ParameterAttributes { .. } => "parameter attributes",
        }
    }
}

fn compare(c: &Sig, r: &Sig, arch: &str, slot: usize) -> (Vec<Diff>, Vec<String>) {
    let (mut findings, mut notes) = (Vec::new(), Vec::new());
    let width = slot * 8;
    if c.cc != r.cc {
        findings.push(Diff::CallingConvention { clang: c.cc.clone(), rustc: r.cc.clone() });
    }
    if c.ret_attrs != r.ret_attrs {
        // x86's psABI leaves the bits above a small integer return undefined (#142389); only
        // bool's bits 1-7 must be zero (#163911).
        if (arch == "x86_64" || arch == "x86") && c.ret != ["i1"] {
            notes.push(format!("return attributes {:?} vs {:?} (x86 psABI: upper bits undefined)", c.ret_attrs, r.ret_attrs));
        } else {
            findings.push(Diff::ReturnAttributes { clang: c.ret_attrs.clone(), rustc: r.ret_attrs.clone(), ret: c.ret.clone() });
        }
    }
    if units(&c.ret, arch, true, width) != units(&r.ret, arch, true, width) {
        findings.push(Diff::Return { clang: c.ret.clone(), rustc: r.ret.clone() });
    } else if c.ret != r.ret {
        notes.push(format!("return types {:?} vs {:?}", c.ret, r.ret));
    }
    // ARM and 64-bit PowerPC split a byval aggregate between registers and stack as they do an
    // array argument of the same size: both are "an aggregate".
    let agg = |p: &Param| -> Param {
        if (arch == "arm" || arch == "powerpc64") && (p.attrs.iter().any(|a| a == "byval") || INT_ARRAY.is_match(&p.ty)) {
            Param { ty: "agg".into(), attrs: vec![], noundef: false }
        } else {
            p.clone()
        }
    };
    let cp: Vec<Param> = c.params.iter().map(agg).collect();
    let rp: Vec<Param> = r.params.iter().map(agg).collect();
    let ct: Vec<String> = cp.iter().map(|p| p.ty.clone()).collect();
    let rt: Vec<String> = rp.iter().map(|p| p.ty.clone()).collect();
    if units(&ct, arch, false, width) != units(&rt, arch, false, width) {
        // i386 passes every argument on the stack: an expanded struct and a byval copy of it are
        // the same bytes.
        if arch == "x86" && cp.iter().chain(&rp).any(|p| p.attrs.iter().any(|a| a == "byval")) {
            notes.push(format!("parameters {ct:?} vs {rt:?} (x86: same stack bytes)"));
        } else {
            findings.push(Diff::Parameters { clang: ct, rustc: rt });
        }
    } else if ct != rt {
        notes.push(format!("parameter types {ct:?} vs {rt:?}"));
    } else {
        for (i, (a, b)) in cp.iter().zip(&rp).enumerate() {
            if a.attrs != b.attrs {
                let aligns: Vec<u32> = a.attrs.iter().chain(&b.attrs).filter_map(|x| x.strip_prefix("align ")).filter_map(|x| x.parse().ok()).collect();
                let rest_c: Vec<&String> = a.attrs.iter().filter(|x| !x.starts_with("align ")).collect();
                let rest_r: Vec<&String> = b.attrs.iter().filter(|x| !x.starts_with("align ")).collect();
                let only_byval = rest_c.iter().chain(&rest_r).all(|x| x.as_str() == "byval");
                let ext_only_rust = rest_c.is_empty() && rest_r.iter().all(|x| x.as_str() == "zeroext" || x.as_str() == "signext");
                if rest_c == rest_r && !aligns.is_empty() && aligns.iter().max().copied().unwrap_or(0) as usize <= slot {
                    notes.push(format!("parameter {i} alignment within a stack slot"));
                } else if (arch == "wasm32" || arch == "wasm64") && only_byval {
                    notes.push(format!("parameter {i}: wasm byval is a pointer to a copy"));
                } else if arch == "x86" && only_byval {
                    notes.push(format!("parameter {i}: x86 same stack bytes"));
                } else if arch == "x86_64" && ext_only_rust {
                    // Win64: rustc extends, clang does not; the callee re-extends either way.
                    notes.push(format!("parameter {i}: win64 extension not relied on"));
                } else {
                    findings.push(Diff::ParameterAttributes { index: i, clang: a.attrs.clone(), rustc: b.attrs.clone(), ty: a.ty.clone() });
                }
            }
            if a.noundef != b.noundef {
                notes.push(format!("parameter {i} noundef"));
            }
        }
    }
    (findings, notes)
}

/// A difference that is a bug already recorded, or one this host cannot decide: its label.
fn label(arch: &str, target: &str, d: &Diff) -> Option<&'static str> {
    let fp = |v: &[String]| v.iter().filter(|t| *t == "float" || *t == "double").count();
    match d {
        Diff::ReturnAttributes { clang, .. } if arch == "x86_64" && clang == &["zeroext"] => Some("rust-lang/rust#163911"),
        Diff::ParameterAttributes { clang, rustc, .. }
            if matches!(arch, "riscv64" | "riscv32" | "loongarch64") && rustc.is_empty() && (clang == &["signext"] || clang == &["zeroext"]) =>
        {
            Some("finding 19 (docs/hunt.md)")
        }
        Diff::Parameters { clang, rustc } | Diff::Return { clang, rustc } if matches!(arch, "riscv64" | "riscv32" | "loongarch64") && fp(rustc) > fp(clang) => {
            Some("finding 20 (docs/hunt.md)")
        }
        // clang returns an 8-byte struct with a 3-byte array field indirectly; rustc and MSVC's
        // documentation in edx:eax. Needs MSVC.
        Diff::Return { rustc, .. } if arch == "x86" && target.ends_with("windows-msvc") && rustc != &["void"] => {
            Some("i686 msvc small-struct return (needs MSVC)")
        }
        Diff::Parameters { clang, rustc } if arch == "x86" && target.ends_with("windows-msvc") && clang.first().map(String::as_str) == Some("ptr") && clang[1..] == rustc[..] => {
            Some("i686 msvc small-struct return (needs MSVC)")
        }
        // clang marks a float from a single-member aggregate inreg; needs a run on the target.
        Diff::ParameterAttributes { clang, rustc, ty, .. } if arch == "powerpc64" && clang == &["inreg"] && rustc.is_empty() && (ty == "float" || ty == "double") => {
            Some("ppc64 inreg float (needs a run)")
        }
        _ => None,
    }
}

// ---- running ----

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct Spec {
    #[serde(default)]
    llvm_target: String,
    #[serde(default)]
    arch: String,
    #[serde(default)]
    target_pointer_width: serde_json::Value,
    #[serde(default)]
    c_int_width: serde_json::Value,
    #[serde(default)]
    cpu: String,
    #[serde(default)]
    features: String,
    #[serde(default)]
    llvm_abiname: String,
    #[serde(default)]
    llvm_floatabi: String,
}

fn num(v: &serde_json::Value, default: usize) -> usize {
    v.as_u64().map(|x| x as usize).or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(default)
}

#[derive(Serialize, Default)]
struct TargetResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    skip: Option<String>,
    compared: usize,
    findings: BTreeMap<String, Vec<Diff>>,
    labelled: BTreeMap<String, usize>,
    notes: usize,
}

fn run_cmd(cmd: &mut Command) -> Result<(), String> {
    match cmd.env("RUSTC_BOOTSTRAP", "1").output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).chars().rev().take(1000).collect::<String>().chars().rev().collect()),
        Err(e) => Err(e.to_string()),
    }
}

fn one_target(args: &Args, target: &str) -> TargetResult {
    let skip = |why: String| TargetResult { skip: Some(why), ..Default::default() };
    let spec_out = Command::new(&args.rustc).args(["--print", "target-spec-json", "-Zunstable-options", "--target", target]).env("RUSTC_BOOTSTRAP", "1").output();
    let Ok(spec_out) = spec_out else { return skip("no spec".into()) };
    let Ok(spec) = serde_json::from_slice::<Spec>(&spec_out.stdout) else { return skip("no spec".into()) };
    let width = num(&spec.target_pointer_width, 64);
    if width < 32 || num(&spec.c_int_width, 32) != 32 {
        return skip("16-bit int".into());
    }
    let d = args.work.join(target);
    let _ = std::fs::create_dir_all(&d);
    let (csrc, rsrc, fns) = program(width == 64 && spec.arch != "sparc64", args.seed, args.count);
    let _ = std::fs::write(d.join("a.c"), &csrc);
    let _ = std::fs::write(d.join("a.rs"), &rsrc);
    let base = |c: &mut Command| {
        c.args(["--target", target, "-Zunstable-options", "--edition", "2021", "-Cpanic=abort", "--out-dir"]).arg(&d);
    };
    let mut mc = Command::new(&args.rustc);
    base(&mut mc);
    mc.args(["--crate-type", "rlib", "--crate-name", "minicore", "-Awarnings"]).arg(args.rust.join("tests/auxiliary/minicore.rs"));
    if let Err(e) = run_cmd(&mut mc) {
        return skip(format!("minicore does not build: {e}"));
    }
    let mut rc = Command::new(&args.rustc);
    base(&mut rc);
    rc.args(["--emit=llvm-ir", "-Copt-level=0", "--extern"]).arg(format!("minicore={}", d.join("libminicore.rlib").display())).arg("-o").arg(d.join("r.ll")).arg(d.join("a.rs"));
    if let Err(e) = run_cmd(&mut rc) {
        return skip(format!("rust side does not build: {e}"));
    }
    let mut cc = Command::new(&args.clang);
    cc.arg(format!("--target={}", spec.llvm_target)).args(["-ffreestanding", "-S", "-emit-llvm", "-O0", "-Wno-everything"]);
    // The target's CPU and features decide parts of the ABI in clang too (soft-float, SSE).
    if !spec.cpu.is_empty() && spec.cpu != "generic" {
        cc.args(["-Xclang", "-target-cpu", "-Xclang", &spec.cpu]);
    }
    for f in spec.features.split(',').filter(|f| !f.is_empty()) {
        cc.args(["-Xclang", "-target-feature", "-Xclang", f]);
    }
    if !spec.llvm_abiname.is_empty() {
        cc.arg(format!("-mabi={}", spec.llvm_abiname));
    }
    if spec.llvm_floatabi == "hard" {
        cc.arg("-mfloat-abi=hard");
    }
    cc.arg("-o").arg(d.join("c.ll")).arg(d.join("a.c"));
    if let Err(e) = run_cmd(&mut cc) {
        return skip(format!("clang does not build: {e}"));
    }
    let cs = signatures(&std::fs::read_to_string(d.join("c.ll")).unwrap_or_default());
    let rsig = signatures(&std::fs::read_to_string(d.join("r.ll")).unwrap_or_default());
    let mut res = TargetResult { compared: fns.len(), ..Default::default() };
    for name in &fns {
        let (Some(c), Some(r)) = (cs.get(name), rsig.get(name)) else { continue };
        let (f, n) = compare(c, r, &spec.arch, width / 8);
        res.notes += n.len();
        let mut unlabelled = Vec::new();
        for diff in f {
            match label(&spec.arch, target, &diff) {
                Some(l) => *res.labelled.entry(l.to_owned()).or_default() += 1,
                None => unlabelled.push(diff),
            }
        }
        if !unlabelled.is_empty() {
            res.findings.insert(name.clone(), unlabelled);
        }
    }
    res
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    std::fs::create_dir_all(&args.work)?;
    let targets: Vec<String> = if let Some(t) = &args.targets {
        t.split(',').map(str::to_owned).collect()
    } else if args.all {
        let out = Command::new(&args.rustc).args(["--print", "target-list"]).output()?;
        String::from_utf8_lossy(&out.stdout).split_whitespace().map(str::to_owned).collect()
    } else {
        MAIN.iter().map(|s| s.to_string()).collect()
    };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(args.jobs).build()?;
    let results: BTreeMap<String, TargetResult> = pool.install(|| targets.par_iter().map(|t| (t.clone(), one_target(&args, t))).collect());
    std::fs::write(args.work.join("results.json"), serde_json::to_string_pretty(&results)?)?;
    let mut labelled: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    let (mut compared, mut skipped) = (0, BTreeMap::<String, usize>::new());
    for (t, r) in &results {
        if let Some(s) = &r.skip {
            *skipped.entry(s.split(':').next().unwrap_or("").to_owned()).or_default() += 1;
            continue;
        }
        compared += 1;
        for (l, n) in &r.labelled {
            *labelled.entry(l.as_str()).or_default() += n;
        }
        for diffs in r.findings.values() {
            for d in diffs {
                *kinds.entry(d.label()).or_default().entry(t.as_str()).or_default() += 1;
            }
        }
    }
    println!("{compared} targets compared; skipped: {skipped:?}");
    if !labelled.is_empty() {
        println!("known: {labelled:?}");
    }
    for (kind, per) in &kinds {
        let total: usize = per.values().sum();
        let mut top: Vec<(&&str, &usize)> = per.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1));
        println!("{kind}: {total} in {} targets: {}", per.len(), top.iter().take(8).map(|(t, n)| format!("{t} {n}")).collect::<Vec<_>>().join(", "));
    }
    Ok(ExitCode::SUCCESS)
}
