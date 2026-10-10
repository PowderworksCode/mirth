//! A PICT model of rustc's option universe, from flag-universe's results.
//!
//! One parameter per option. Its values are absence, the values rustc accepted alone, the
//! values it accepts once `-Cunsafe-allow-abi-mismatch` names every target modifier
//! (FLAG_BASE, passed on every row), and the values that need another option, with IF/THEN
//! constraints for those needs. Options that stop compilation early (help, parse-only,
//! link-only) are left out. A value whose need lies outside the subset is dropped.
//!
//! With --cargo the model is for building a Cargo workspace (flag-walk): it leaves out the
//! values in CARGO_DROP, which fail there for reasons of Cargo or this machine, not the options.
//!
//! Combinations that hit bugs already in docs/hunt.md are excluded, so walks look for new
//! ones; --allow-known keeps those that have a local stopgap (for a compiler with them).
//!
//! With --transitions every parameter appears twice, A_ before and B_ after, for covering the
//! changes between two sessions.
//!
//! Run it with PICT (github.com/microsoft/pict): `pict <out> /o:2` gives a pairwise covering
//! array, `/o:3` three-way.
//!
//! Also the parts the other flag tools share: options.json, table rows, a row's flags.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(clap::Args, Debug)]
pub struct Args {
    /// flag-universe's work directory.
    work: PathBuf,
    /// all, untracked or tracked.
    subset: String,
    out: PathBuf,
    #[arg(long)]
    transitions: bool,
    #[arg(long)]
    cargo: bool,
    #[arg(long)]
    allow_known: bool,
}

pub const FLAG_BASE: &str = "-Cunsafe-allow-abi-mismatch=sanitizer,sanitizer-cfi-normalize-integers,\
sanitizer-cfi-minimal-runtime,retpoline,retpoline-external-thunk,\
indirect-branch-cs-prefix,fixed-x18,reg-struct-return,regparm,branch-protection";

const STOP: &[&str] = &["-Chelp", "-Zhelp", "-Zparse-crate-root-only", "-Zno-analysis", "-Zlink-only", "-Zimplicit-sysroot-deps"];

/// (option, values to drop; None: the whole option)
const CARGO_DROP: &[(&str, Option<&[&str]>)] = &[
    ("-Zassert-incr-state", None),         // fails whenever the cache state differs, by design
    ("-Zbuild-sdylib-interface", None),    // Cargo's target probe fails
    ("-Zchecksum-hash-algorithm", Some(&["md5", "sha1"])), // Cargo cannot parse the dep info
    ("-Zdirect-access-external-data", None), // link fails
    ("-Zfunction-return", Some(&["thunk-extern"])), // link fails: no thunk
    ("-Zlink-native-libraries", None),     // link fails
    ("-Zlint-llvm-ir", None),              // aborts on known LLVM lint findings (rust-lang/rust#59793)
    ("-Zno-codegen", None),                // later crates need the output
    ("-Zno-link", None),
    ("-Zpanic-in-drop", None),             // std is built with unwind
    ("-Zsanitizer", None),                 // no sanitizer runtimes in this sysroot: link fails
    ("-Zretpoline-external-thunk", None),  // link fails: no thunk
    ("-Ztiny-const-eval-limit", None),     // the fixture's const evaluation exceeds it
    ("-Cpanic", Some(&["immediate-abort"])), // core is built with unwind
    ("-Ccode-model", Some(&["tiny"])),     // LLVM ERROR: not supported on x86_64
    ("-Ztls-model", Some(&["local-exec", "emulated"])), // dylib cannot link
    // the dylib cannot link (static, pie, ropi); rwpi: finding 12
    ("-Crelocation-model", Some(&["static", "pie", "ropi", "rwpi", "ropi-rwpi"])),
    ("-Clto", None), // rejected for rlibs and dylibs; Cargo's profile applies it to final artifacts only
    // Makes a crate behave like the standard library, which needs stability attributes on
    // `const trait`s (fixtures/sink/nightly has one).
    ("-Zforce-unstable-if-unmarked", Some(&["yes"])),
    // LLVM's pass listing from codegen threads interleaves with rustc's lines on stderr; a split
    // -Ztime-passes-format=json line then reaches Cargo as a bare JSON message.
    ("-Zprint-llvm-passes", Some(&["yes"])),
    ("-Ztime-passes-format", Some(&["json"])),
];

/// (option, value, options needed, PICT condition)
type Need = (&'static str, &'static str, &'static [&'static str], &'static str);

// Constraints that only a real workspace shows: a binary, a dylib, Cargo's own flags.
const CARGO_NEEDS: &[Need] = &[
    ("-Cprefer-dynamic", "yes", &["-Cpanic"], r#"[Cpanic] <> "abort""#), // libstd.so has panic_unwind
    ("-Cprefer-dynamic", "yes", &["-Clto"], r#"[Clto] IN {"absent","no","off"}"#),
    // Findings 13 and 14 (in LLVM, not patched): retpolines with the machine outliner, or with
    // the large code model.
    ("-Zretpoline", "yes", &["-Ccode-model"], r#"[Ccode_model] <> "large""#),
    // Finding 16 (no stopgap): cached derive expansions with the HIR crate hash.
    ("-Zcache-proc-macros", "yes", &["-Zmetadata-crate-hash"], r#"[Zmetadata_crate_hash] <> "no""#),
];
// Bugs in docs/hunt.md that have a local stopgap: excluded unless --allow-known (for a
// compiler with the stopgaps).
const KNOWN_NEEDS: &[Need] = &[
    // Finding 9: without the default passes, local ThinLTO leaves undefined hidden symbols.
    ("-Cno-prepopulate-passes", "present", &["-Zthinlto", "-Copt-level"], r#"[Zthinlto] <> "yes" AND [Copt_level] IN {"absent","0"}"#),
];
// Values left out of every model: LLVM's machine outliner crashes in many combinations
// (finding 13), which buries everything else; and see below.
const DROP: &[(&str, &[&str])] = &[
    ("-Cllvm-args", &["-enable-machine-outliner"]),
    // Not a bug: the limit counts MIR pass runs across the session, so which bodies stay under
    // it depends on how many bodies the session computes (an incremental session computes
    // fewer) and, with -Zthreads, on thread timing.
    ("-Zmir-opt-bisect-limit", &["1", "16"]),
    // Not a bug: a testing option that leaves spans out of the incremental hashes, so a rebuild
    // keeps stale spans by design.
    ("-Zincremental-ignore-spans", &["yes"]),
];
// Rejected alone; accepted with FLAG_BASE or with the needs below.
const EXTRA: &[(&str, &[&str])] = &[
    ("-Zindirect-branch-cs-prefix", &["yes"]),
    ("-Zretpoline-external-thunk", &["yes"]),
    ("-Zretpoline", &["yes"]),
    ("-Zsanitizer", &["dataflow", "memory", "safestack", "thread", "cfi", "kcfi"]),
    ("-Cforce-frame-pointers", &["non-leaf"]),
    ("-Cpanic", &["immediate-abort"]),
    ("-Zdump-dep-graph", &["yes"]),
    ("-Zsanitizer-cfi-canonical-jump-tables", &["no"]),
    ("-Zsanitizer-cfi-diag", &["yes"]),
    ("-Zsanitizer-cfi-generalize-pointers", &["yes"]),
    ("-Zsanitizer-cfi-minimal-runtime", &["yes"]),
    ("-Zsanitizer-cfi-normalize-integers", &["yes"]),
    ("-Zsanitizer-cfi-recover", &["yes"]),
    ("-Zsanitizer-kcfi-arity", &["yes"]),
    ("-Zsplit-lto-unit", &["yes"]),
    ("-Zvirtual-function-elimination", &["yes"]),
];
const NEEDS: &[Need] = &[
    ("-Cembed-bitcode", "no", &["-Clto"], r#"[Clto] IN {"absent","no","off"}"#),
    ("-Zsplit-lto-unit", "yes", &["-Clto"], r#"[Clto] IN {"yes","on","thin","fat"}"#),
    ("-Zvirtual-function-elimination", "yes", &["-Clto"], r#"[Clto] IN {"yes","on","fat"}"#),
    ("-Zsanitizer", "cfi", &["-Clto", "-Ccodegen-units"], r#"[Clto] IN {"yes","on","fat"} AND [Ccodegen_units] = "1""#),
    ("-Zsanitizer", "kcfi", &["-Cpanic"], r#"[Cpanic] = "abort""#),
    ("-Cforce-frame-pointers", "non-leaf", &["-Zunstable-options"], r#"[Zunstable_options] = "present""#),
    ("-Cpanic", "immediate-abort", &["-Zunstable-options"], r#"[Zunstable_options] = "present""#),
    ("-Zdump-dep-graph", "yes", &["-Zquery-dep-graph"], r#"[Zquery_dep_graph] = "yes""#),
    ("-Zsanitizer-cfi-diag", "yes", &["-Zsanitizer"], r#"[Zsanitizer] = "cfi""#),
    ("-Zsanitizer-cfi-recover", "yes", &["-Zsanitizer"], r#"[Zsanitizer] = "cfi""#),
    ("-Zsanitizer-cfi-minimal-runtime", "yes", &["-Zsanitizer"], r#"[Zsanitizer] = "cfi""#),
    ("-Zsanitizer-cfi-canonical-jump-tables", "no", &["-Zsanitizer"], r#"[Zsanitizer] = "cfi""#),
    ("-Zsanitizer-cfi-generalize-pointers", "yes", &["-Zsanitizer"], r#"[Zsanitizer] IN {"cfi","kcfi"}"#),
    ("-Zsanitizer-cfi-normalize-integers", "yes", &["-Zsanitizer"], r#"[Zsanitizer] IN {"cfi","kcfi"}"#),
    ("-Zsanitizer-kcfi-arity", "yes", &["-Zsanitizer"], r#"[Zsanitizer] = "kcfi""#),
    (
        "-Zsanitizer-cfi-minimal-runtime",
        "yes",
        &["-Zsanitizer-cfi-recover", "-Zsanitizer-cfi-diag"],
        r#"([Zsanitizer_cfi_recover] = "yes" OR [Zsanitizer_cfi_diag] = "yes")"#,
    ),
];

/// One -C or -Z option, as flag-universe writes it to options.json.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Opt {
    pub flag: String,
    pub name: String,
    pub parser: String,
    pub tracking: String,
    /// None: an option without a value.
    pub values: Vec<Option<String>>,
    pub free: bool,
}

impl Opt {
    pub fn full(&self) -> String {
        format!("{}{}", self.flag, self.name)
    }
}

/// One value tried alone, as flag-universe writes it to singles.json.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Single {
    pub arg: String,
    pub option: String,
    pub value: Option<String>,
    pub ok: bool,
    pub warn: bool,
    pub msg: String,
}

static NON_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^A-Za-z0-9]").unwrap());
static PARAM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(\w+)\]").unwrap());

/// An option's PICT parameter name.
pub fn pname(opt: &str) -> String {
    NON_WORD.replace_all(opt.trim_start_matches('-'), "_").into_owned()
}

/// options.json of a flag-universe directory, by PICT parameter name.
pub fn options(dir: &Path) -> anyhow::Result<BTreeMap<String, Opt>> {
    let list: Vec<Opt> = serde_json::from_str(&std::fs::read_to_string(dir.join("options.json"))?)?;
    Ok(list.into_iter().map(|o| (pname(&o.full()), o)).collect())
}

/// A PICT table: rows of (parameter, value) in column order.
pub fn table(path: &Path) -> anyhow::Result<Vec<Vec<(String, String)>>> {
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    Ok(lines
        .filter(|l| !l.is_empty())
        .map(|l| header.iter().zip(l.split('\t')).map(|(h, v)| (h.to_string(), v.to_string())).collect())
        .collect())
}

/// The value of `column` in a row.
pub fn get<'a>(row: &'a [(String, String)], column: &str) -> Option<&'a str> {
    row.iter().find(|(k, _)| k == column).map(|(_, v)| v.as_str())
}

/// FLAG_BASE and a row's options; with `side` ("A" or "B"), only that side's columns.
pub fn row_flags(row: &[(String, String)], side: Option<&str>, opts: &BTreeMap<String, Opt>) -> Vec<String> {
    let mut out = vec![FLAG_BASE.to_owned()];
    for (k, v) in row {
        if v == "absent" {
            continue;
        }
        let key = match side {
            Some(s) => match k.strip_prefix(s).and_then(|r| r.strip_prefix('_')) {
                Some(rest) => rest,
                None => continue,
            },
            None => k.as_str(),
        };
        let Some(o) = opts.get(key) else { continue };
        let f = o.full();
        out.push(if v == "present" { f } else { format!("{f}={}", v.replace(';', ",")) });
    }
    out
}

/// A JSON object whose keys keep their order (serde_json's own map sorts them).
pub struct OrderedMap<'a, V>(pub &'a [(String, V)]);

impl<V: Serialize> Serialize for OrderedMap<'_, V> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// JSON as Python's `json.dumps(x, indent=1)` writes it, so files stay byte-comparable.
pub fn to_json_indent1<T: Serialize>(value: &T) -> String {
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value.serialize(&mut ser).expect("serializable");
    let text = String::from_utf8(buf).expect("utf-8");
    // Python escapes non-ASCII.
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for u in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{u:04x}"));
            }
        }
    }
    out
}

pub fn run(args: Args) -> anyhow::Result<ExitCode> {
    let known = !args.allow_known;
    let opts: BTreeMap<String, Opt> = {
        let list: Vec<Opt> = serde_json::from_str(&std::fs::read_to_string(args.work.join("options.json"))?)?;
        list.into_iter().map(|o| (o.full(), o)).collect()
    };
    let singles: Vec<Single> = serde_json::from_str(&std::fs::read_to_string(args.work.join("singles.json"))?)?;
    let mut domains: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in singles.iter().filter(|s| s.ok && !STOP.contains(&s.option.as_str())) {
        domains.entry(s.option.clone()).or_default().push(s.value.clone().unwrap_or_else(|| "present".into()));
    }
    for (k, vs) in EXTRA {
        let d = domains.entry(k.to_string()).or_default();
        for v in *vs {
            if !d.iter().any(|x| x == v) {
                d.push(v.to_string());
            }
        }
    }
    for (k, vs) in DROP {
        domains.entry(k.to_string()).or_default().retain(|v| !vs.contains(&v.as_str()));
    }
    if args.cargo {
        for (k, vs) in CARGO_DROP {
            let d = domains.entry(k.to_string()).or_default();
            match vs {
                None => d.clear(),
                Some(vs) => d.retain(|v| !vs.contains(&v.as_str())),
            }
        }
        domains.retain(|_, v| !v.is_empty());
    }
    let untracked = |k: &str| opts.get(k).is_some_and(|o| o.tracking == "UNTRACKED");
    let keep: Vec<String> =
        domains.keys().filter(|k| args.subset == "all" || (args.subset == "untracked") == untracked(k)).cloned().collect();
    let mut cons = Vec::new();
    let needs = NEEDS.iter().chain(if args.cargo { CARGO_NEEDS } else { &[] }).chain(if known { KNOWN_NEEDS } else { &[] });
    for (k, v, need, cond) in needs {
        if !keep.iter().any(|x| x == k) {
            continue;
        }
        if need.iter().all(|n| keep.iter().any(|x| x == n)) {
            cons.push(format!("IF [{}] = \"{v}\" THEN {cond};", pname(k)));
        } else if let Some(d) = domains.get_mut(*k) {
            if let Some(i) = d.iter().position(|x| x == v) {
                d.remove(i);
            }
        }
    }
    let mut params: Vec<String> = keep
        .iter()
        .map(|k| {
            let vals: Vec<String> = std::iter::once("absent".to_owned()).chain(domains[k].iter().map(|v| v.replace(',', ";"))).collect();
            format!("{}: {}", pname(k), vals.join(", "))
        })
        .collect();
    if args.transitions {
        params = ["A", "B"].iter().flat_map(|t| params.iter().map(move |p| format!("{t}_{p}"))).collect();
        cons = ["A", "B"]
            .iter()
            .flat_map(|t| cons.iter().map(move |c| PARAM.replace_all(c, |m: &regex::Captures| format!("[{t}_{}]", &m[1])).into_owned()))
            .collect();
        if known && keep.iter().any(|k| k == "-Zprint-type-sizes") {
            // Finding 11 in docs/hunt.md: the rebuild ICEs once -Zprint-type-sizes is dropped.
            cons.push(r#"IF [A_Zprint_type_sizes] = "yes" THEN [B_Zprint_type_sizes] = "yes";"#.to_owned());
        }
    }
    std::fs::write(&args.out, format!("{}\n\n{}\n", params.join("\n"), cons.join("\n")))?;
    println!("{} parameters, {} constraints", params.len(), cons.len());
    Ok(ExitCode::SUCCESS)
}
