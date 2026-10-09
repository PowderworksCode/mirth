#!/usr/bin/env python3
"""Write a PICT model of rustc's option universe from flag-universe.py's results.

    rustc/flag-model.py <work> <all|untracked|tracked> <out> [--transitions] [--cargo] [--allow-known]

One parameter per option. Its values are absence, the values rustc accepted alone, the
values it accepts once `-Cunsafe-allow-abi-mismatch` names every target modifier (FLAG_BASE,
passed on every row), and the values that need another option, with IF/THEN constraints for
those needs. Options that stop compilation early (help, parse-only, link-only) are left out.
A value whose need lies outside the subset is dropped.

With --cargo the model is for building a Cargo workspace (flag-walk.py): it leaves out the
values in CARGO_DROP, which fail there for reasons of Cargo or this machine, not the options.

Combinations that hit bugs already in docs/hunt.md are excluded, so walks look for new
ones; --allow-known keeps those that have a local stopgap (for a compiler with them).

With --transitions every parameter appears twice, A_ before and B_ after, for covering the
changes between two sessions.

Run it with PICT (github.com/microsoft/pict): `pict <out> /o:2` gives a pairwise covering
array, `/o:3` three-way.
"""

import json
import re
import sys

FLAG_BASE = ("-Cunsafe-allow-abi-mismatch=sanitizer,sanitizer-cfi-normalize-integers,"
             "sanitizer-cfi-minimal-runtime,retpoline,retpoline-external-thunk,"
             "indirect-branch-cs-prefix,fixed-x18,reg-struct-return,regparm,branch-protection")
STOP = {"-Chelp", "-Zhelp", "-Zparse-crate-root-only", "-Zno-analysis", "-Zlink-only",
        "-Zimplicit-sysroot-deps"}
CARGO_DROP = {
    "-Zassert-incr-state": None,  # fails whenever the cache state differs, by design
    "-Zbuild-sdylib-interface": None,  # Cargo's target probe fails
    "-Zchecksum-hash-algorithm": ["md5", "sha1"],  # Cargo cannot parse the dep info
    "-Zdirect-access-external-data": None,  # link fails
    "-Zfunction-return": ["thunk-extern"],  # link fails: no thunk
    "-Zlink-native-libraries": None,  # link fails
    "-Zlint-llvm-ir": None,  # aborts on known LLVM lint findings (rust-lang/rust#59793)
    "-Zno-codegen": None, "-Zno-link": None,  # later crates need the output
    "-Zpanic-in-drop": None,  # std is built with unwind
    "-Zsanitizer": None,  # no sanitizer runtimes in this sysroot: link fails
    "-Zretpoline-external-thunk": None,  # link fails: no thunk
    "-Ztiny-const-eval-limit": None,  # the fixture's const evaluation exceeds it
    "-Cpanic": ["immediate-abort"],  # core is built with unwind
    "-Ccode-model": ["tiny"],  # LLVM ERROR: not supported on x86_64
    "-Ztls-model": ["local-exec", "emulated"],  # dylib cannot link
    # the dylib cannot link (static, pie, ropi); rwpi: finding 12
    "-Crelocation-model": ["static", "pie", "ropi", "rwpi", "ropi-rwpi"],
    "-Clto": None,  # rejected for rlibs and dylibs; Cargo's profile applies it to final artifacts only
    # LLVM's pass listing from codegen threads interleaves with rustc's lines on stderr; a split
    # -Ztime-passes-format=json line then reaches Cargo as a bare JSON message.
    # Makes a crate behave like the standard library, which needs stability attributes on
    # `const trait`s (fixtures/sink/nightly has one).
    "-Zforce-unstable-if-unmarked": ["yes"],
    "-Zprint-llvm-passes": ["yes"],
    "-Ztime-passes-format": ["json"],
}
# Constraints that only a real workspace shows: a binary, a dylib, Cargo's own flags.
CARGO_NEEDS = [
    ("-Cprefer-dynamic", "yes", ["-Cpanic"], '[Cpanic] <> "abort"'),  # libstd.so has panic_unwind
    ("-Cprefer-dynamic", "yes", ["-Clto"], '[Clto] IN {"absent","no","off"}'),
    # Findings 13 and 14 (in LLVM, not patched): retpolines with the machine outliner, or with
    # the large code model.
    ("-Zretpoline", "yes", ["-Ccode-model"], '[Ccode_model] <> "large"'),
    # Finding 16 (no stopgap): cached derive expansions with the HIR crate hash.
    ("-Zcache-proc-macros", "yes", ["-Zmetadata-crate-hash"], '[Zmetadata_crate_hash] <> "no"'),
]
# Bugs in docs/hunt.md that have a local stopgap: excluded unless --allow-known (for a
# compiler with the stopgaps).
KNOWN_NEEDS = [
    # Finding 9: without the default passes, local ThinLTO leaves undefined hidden symbols.
    ("-Cno-prepopulate-passes", "present", ["-Zthinlto", "-Copt-level"],
     '[Zthinlto] <> "yes" AND [Copt_level] IN {"absent","0"}'),
]
# Values left out of every model: LLVM's machine outliner crashes in many combinations
# (finding 13), which buries everything else; and see below.
DROP = {"-Cllvm-args": ["-enable-machine-outliner"],
        # Not a bug: the limit counts MIR pass runs across the session, so which bodies stay
        # under it depends on how many bodies the session computes (an incremental session
        # computes fewer) and, with -Zthreads, on thread timing.
        "-Zmir-opt-bisect-limit": ["1", "16"],
        # Not a bug: a testing option that leaves spans out of the incremental hashes, so a
        # rebuild keeps stale spans by design.
        "-Zincremental-ignore-spans": ["yes"]}
# Rejected alone; accepted with FLAG_BASE or with the needs below.
EXTRA = {"-Zindirect-branch-cs-prefix": ["yes"], "-Zretpoline-external-thunk": ["yes"],
         "-Zretpoline": ["yes"],
         "-Zsanitizer": ["dataflow", "memory", "safestack", "thread", "cfi", "kcfi"],
         "-Cforce-frame-pointers": ["non-leaf"], "-Cpanic": ["immediate-abort"],
         "-Zdump-dep-graph": ["yes"], "-Zsanitizer-cfi-canonical-jump-tables": ["no"],
         "-Zsanitizer-cfi-diag": ["yes"], "-Zsanitizer-cfi-generalize-pointers": ["yes"],
         "-Zsanitizer-cfi-minimal-runtime": ["yes"], "-Zsanitizer-cfi-normalize-integers": ["yes"],
         "-Zsanitizer-cfi-recover": ["yes"], "-Zsanitizer-kcfi-arity": ["yes"],
         "-Zsplit-lto-unit": ["yes"], "-Zvirtual-function-elimination": ["yes"]}
LTO_ON = '{"yes","on","thin","fat"}'
LTO_FAT = '{"yes","on","fat"}'
# (option, value, options needed, PICT condition)
NEEDS = [
    ("-Cembed-bitcode", "no", ["-Clto"], '[Clto] IN {"absent","no","off"}'),
    ("-Zsplit-lto-unit", "yes", ["-Clto"], "[Clto] IN " + LTO_ON),
    ("-Zvirtual-function-elimination", "yes", ["-Clto"], "[Clto] IN " + LTO_FAT),
    ("-Zsanitizer", "cfi", ["-Clto", "-Ccodegen-units"], "[Clto] IN " + LTO_FAT + ' AND [Ccodegen_units] = "1"'),
    ("-Zsanitizer", "kcfi", ["-Cpanic"], '[Cpanic] = "abort"'),
    ("-Cforce-frame-pointers", "non-leaf", ["-Zunstable-options"], '[Zunstable_options] = "present"'),
    ("-Cpanic", "immediate-abort", ["-Zunstable-options"], '[Zunstable_options] = "present"'),
    ("-Zdump-dep-graph", "yes", ["-Zquery-dep-graph"], '[Zquery_dep_graph] = "yes"'),
    ("-Zsanitizer-cfi-diag", "yes", ["-Zsanitizer"], '[Zsanitizer] = "cfi"'),
    ("-Zsanitizer-cfi-recover", "yes", ["-Zsanitizer"], '[Zsanitizer] = "cfi"'),
    ("-Zsanitizer-cfi-minimal-runtime", "yes", ["-Zsanitizer"], '[Zsanitizer] = "cfi"'),
    ("-Zsanitizer-cfi-canonical-jump-tables", "no", ["-Zsanitizer"], '[Zsanitizer] = "cfi"'),
    ("-Zsanitizer-cfi-generalize-pointers", "yes", ["-Zsanitizer"], '[Zsanitizer] IN {"cfi","kcfi"}'),
    ("-Zsanitizer-cfi-normalize-integers", "yes", ["-Zsanitizer"], '[Zsanitizer] IN {"cfi","kcfi"}'),
    ("-Zsanitizer-kcfi-arity", "yes", ["-Zsanitizer"], '[Zsanitizer] = "kcfi"'),
    ("-Zsanitizer-cfi-minimal-runtime", "yes", ["-Zsanitizer-cfi-recover", "-Zsanitizer-cfi-diag"],
     '([Zsanitizer_cfi_recover] = "yes" OR [Zsanitizer_cfi_diag] = "yes")'),
]


def pname(opt):
    return re.sub(r"[^A-Za-z0-9]", "_", opt.lstrip("-"))


def main():
    work, subset, out = sys.argv[1:4]
    transitions = "--transitions" in sys.argv[4:]
    cargo = "--cargo" in sys.argv[4:]
    known = "--allow-known" not in sys.argv[4:]
    opts = {o["flag"] + o["name"]: o for o in json.load(open(work + "/options.json"))}
    domains = {}
    for s in json.load(open(work + "/singles.json")):
        if s["ok"] and s["option"] not in STOP:
            domains.setdefault(s["option"], []).append("present" if s["value"] is None else s["value"])
    for k, vs in EXTRA.items():
        for v in vs:
            if v not in domains.setdefault(k, []):
                domains[k].append(v)
    for k, vs in DROP.items():
        domains[k] = [v for v in domains.get(k, []) if v not in vs]
    if cargo:
        for k, vs in CARGO_DROP.items():
            domains[k] = [] if vs is None else [v for v in domains.get(k, []) if v not in vs]
        domains = {k: v for k, v in domains.items() if v}

    keep = [k for k in sorted(domains)
            if subset == "all" or (subset == "untracked") == (opts[k]["tracking"] == "UNTRACKED")]
    cons = []
    for k, v, needs, cond in NEEDS + (CARGO_NEEDS if cargo else []) + (KNOWN_NEEDS if known else []):
        if k not in keep:
            continue
        if all(n in keep for n in needs):
            cons.append(f'IF [{pname(k)}] = "{v}" THEN {cond};')
        elif v in domains[k]:
            domains[k].remove(v)

    params = [f"{pname(k)}: {', '.join(['absent'] + [v.replace(',', ';') for v in domains[k]])}" for k in keep]
    if transitions:
        params = [f"{t}_{p}" for t in "AB" for p in params]
        cons = [re.sub(r"\[(\w+)\]", lambda m: f"[{t}_{m.group(1)}]", c) for t in "AB" for c in cons]
        if known and "-Zprint-type-sizes" in keep:
            # Finding 11 in docs/hunt.md: the rebuild ICEs once -Zprint-type-sizes is dropped.
            cons.append('IF [A_Zprint_type_sizes] = "yes" THEN [B_Zprint_type_sizes] = "yes";')
    open(out, "w").write("\n".join(params) + "\n\n" + "\n".join(cons) + "\n")
    print(f"{len(params)} parameters, {len(cons)} constraints")


if __name__ == "__main__":
    main()
