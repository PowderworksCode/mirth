#!/usr/bin/env python3
"""Miri differential: an accepted program must be free of undefined behavior, MIR optimizations
must not introduce any, and the compiled program must do what Miri says it does.

For each runnable UI test (run-pass, run-fail), interprets it with Miri three times: without MIR
optimizations (Miri's default), at `-Zmir-opt-level=2` (what `-O` runs) and at
`-Zmir-opt-level=4` (every MIR pass), and builds and runs it with the compiler under test,
unoptimized. Findings:

  ub          Miri reports undefined behavior at mir-opt-level 0 in an accepted program without
              `unsafe` code: the compiler accepted something unsound (UB in a test with its own
              unsafe code is noted, not reported)
  ub-opt      undefined behavior only after MIR optimization: a MIR pass broke the program
  opt-differs Miri's exit status or stdout changes with the MIR optimization level
  native      the compiled program's exit status or stdout differs from Miri's

Overflow checks and debug assertions are on everywhere. Tests Miri cannot run (foreign
functions, inline assembly, unsupported operations) or that time out are skipped.

    rustc/miri-diff.py --rustc <rustc> --tests <rust>/tests/ui --work <dir> [--only <substr>]
        [--known <file>] [--jobs 8] [--timeout 120] [--pause-on-finding] [--recheck]

Writes <work>/results.jsonl and <work>/findings/<test>/.
"""

import argparse
import json
import re
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import uitest  # noqa: E402

FIXED = ["-Coverflow-checks=on", "-Cdebug-assertions=on"]
# Miri without preemption: threads switch only where they block, the same at every MIR level.
MIRI_FLAGS = ["-Zmiri-preemption-rate=0"]
# Tests asserting what Rust leaves unspecified, which Miri varies on purpose: function pointer
# equality, the addresses of zero-sized values, stack addresses, function alignment.
UNSPECIFIED = {"consts/const-extern-function.rs", "consts/zst_no_llvm_alloc.rs",
               "layout/null-pointer-optimization.rs", "mir/mir_misc_casts.rs", "mir/mir_coercions.rs",
               "extern/extern-compare-with-return-type.rs", "fn/fn-ptr-trait-run.rs", "mir/mir_raw_fat_ptr.rs",
               "codegen/StackColoring-not-blowup-stack-issue-40883.rs", "attributes/fn-align-dyn.rs",
               # Miri calls .init_array functions without glibc's (argc, argv, envp).
               "runtime/stdout-before-main.rs"}
THREADS = re.compile(r"thread::(spawn|scope)|std::sync::mpsc|\bspawn\(")
LEVELS = {"miri0": [], "miri2": ["-Zmir-opt-level=2"], "miri4": ["-Zmir-opt-level=4"]}
OWN = re.compile(r"^-O$|opt-level|overflow-checks|debug-assertions|codegen-backend|mir-enable-passes|"
                 r"panic=|-Cpanic|prefer-dynamic|-Zbuild-std|-Clink|-Ctarget")
# Code Miri cannot interpret: skip without trying.
NOT_FOR_MIRI = re.compile(r"\basm!|global_asm!|naked_asm!|extern\s+\"C\"\s*\{|#\[link\(|std::process::Command|"
                          r"\bfork\b|libc::|dlopen|std::os::unix::process")

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--work", required=True)
p.add_argument("--only")
p.add_argument("--known")
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--timeout", type=int, default=120)
p.add_argument("--pause-on-finding", action="store_true")
p.add_argument("--recheck", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
(WORK / "scratch").mkdir(parents=True, exist_ok=True)
known = set(Path(args.known).read_text().split()) if args.known else set()


def clean(text, path):
    text = re.sub(r"(thread '[^']*') \(\d+\)", r"\1", text)
    text = re.sub(r"\S*/lib/rustlib/src/rust/library/", "library/", text)
    # argv[0]: the source file under Miri, the binary natively.
    text = text.replace(str(path.resolve()), "<argv0>")
    text = re.sub(r"\S*/native/prog\b", "<argv0>", text)
    # The test harness: timings, and result lines in completion order.
    text = re.sub(r"finished in \d+\.\d+s", "finished in …s", text)
    lines = text.split("\n")
    results = iter(sorted(l for l in lines if l.startswith("test ") and " ... " in l))
    return "\n".join(next(results) if (l.startswith("test ") and " ... " in l) else l for l in lines)


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    record = {"test": rel, "kind": kind}
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        status, _, binary = uitest.compile(args.rustc, path.resolve(), d / "native", flags, edition,
                                           FIXED + ["-Copt-level=0"])
        if binary is None:
            record["skip"] = f"native build {status}"
            return record, []
        runs = [uitest.run(binary) for _ in range(3)]
        outs = {(c, clean(o.decode(errors="replace"), path)) for c, o, _ in runs}
        if len(outs) > 1:
            record["skip"] = "native run is nondeterministic"
            return record, []
        code, out = outs.pop()
        native = {"exit": code, "stdout": out}
        miri = {}
        for name, extra in LEVELS.items():
            m = uitest.miri(path.resolve(), flags, edition, FIXED + MIRI_FLAGS + extra, timeout=args.timeout, cwd=d)
            m["stdout"] = clean(m["stdout"], path)
            miri[name] = m
            if name == "miri0" and m["status"] in ("unsupported", "error", "timeout"):
                record["skip"] = f"miri {m['status']}"
                record["why"] = m["stderr"][-300:]
                return record, []
        record["miri"] = {k: v["status"] for k, v in miri.items()}
        found = []
        # UB in a program without `unsafe` code can only be the compiler's; in a test with its
        # own unsafe code it is most likely the test's (noted, not a finding).
        safe = "unsafe" not in path.read_text(errors="replace")
        if miri["miri0"]["status"] == "ub" and safe and rel not in UNSPECIFIED:
            found.append({"what": "ub (safe code)", "stderr": miri["miri0"]["stderr"][-3000:]})
        elif miri["miri0"]["status"] == "ub":
            record["note"] = "ub in a test with unsafe code"
        for name in ("miri2", "miri4"):
            m = miri[name]
            if m["status"] == "ub" and miri["miri0"]["status"] != "ub":
                found.append({"what": f"ub-opt ({name})", "stderr": m["stderr"][-3000:]})
            elif m["status"] == "ice":
                found.append({"what": f"ice ({name})", "stderr": m["stderr"][-3000:]})
            elif (m["status"] == "ok" and miri["miri0"]["status"] == "ok"
                  and (m["exit"], m["stdout"]) != (miri["miri0"]["exit"], miri["miri0"]["stdout"])):
                found.append({"what": f"opt-differs ({name})", "miri0": miri["miri0"]["stdout"][-1500:],
                              "got": m["stdout"][-1500:], "exits": [miri["miri0"]["exit"], m["exit"]]})
        m0 = miri["miri0"]
        threaded = THREADS.search(path.read_text(errors="replace"))
        # Native threads race; Miri's do not without preemption: no comparison for threaded tests.
        if m0["status"] == "ok" and not threaded and rel not in UNSPECIFIED:
            # Miri exits 1 on a panic that reaches main, native code 101.
            exit_m = 101 if m0["exit"] == 1 and "panicked" in m0["stderr"] else m0["exit"]
            if (exit_m, m0["stdout"]) != (native["exit"], native["stdout"]):
                found.append({"what": "native", "miri": [m0["exit"], m0["stdout"][-1500:]],
                              "native": [native["exit"], native["stdout"][-1500:]]})
        if found:
            outdir = WORK / "findings" / rel.replace("/", "__")
            shutil.rmtree(outdir, ignore_errors=True)
            outdir.mkdir(parents=True)
            shutil.copy(path, outdir / path.name)
            (outdir / "finding.json").write_text(json.dumps(
                {"test": rel, "flags": flags, "edition": edition, "fixed": FIXED, "found": found}, indent=1))
        record["found"] = [f["what"] for f in found]
        return record, found


def main():
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    todo = []
    for path, flags, edition, kind in uitest.tests(
            args.tests, ("run-pass", "run-fail"),
            lambda text, flags: any(OWN.search(f) for f in flags) or NOT_FOR_MIRI.search(text)
            # Compile-time output (trace_macros, log_syntax) would land in Miri's stdout.
            or "trace_macros" in text or "log_syntax" in text):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()
