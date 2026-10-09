#!/usr/bin/env python3
"""Optimization differential: a program's behavior must not depend on how it was optimized.

Builds each runnable UI test (run-pass, run-fail) under a set of configurations (optimization
levels, MIR optimization levels, LTO, target CPU, the Cranelift backend) and runs it. Overflow
checks and debug assertions are fixed across configurations, so the program's semantics are the
same in all of them. Compared with the unoptimized baseline (`-Copt-level=0 -Zmir-opt-level=0`):

  run    exit status and stdout (stderr too, with panic locations kept, backtrace hints dropped)
  build  a configuration fails to build, or crashes the compiler, where the baseline builds

A baseline whose output varies between two runs is nondeterministic and skipped; a difference
is confirmed by running both binaries again before it counts.

    rustc/opt-diff.py --rustc <rustc> --tests <rust>/tests/ui --work <dir>
        [--cranelift <rustc with the cranelift backend>] [--configs O3,O3-lto] [--only <substr>]
        [--known <file of test paths>] [--jobs 8] [--pause-on-finding] [--recheck]

Writes <work>/results.jsonl, and <work>/findings/<test>/ with the source, the configurations'
argv and outputs. With --pause-on-finding, stops starting new tests at the first finding and
exits 3 (the frontier loop: docs/hunt.md). --recheck runs only the tests with findings.
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

FIXED = ["-Coverflow-checks=on", "-Cdebug-assertions=on", "-Cpanic=unwind", "-Cdebuginfo=0"]
CONFIGS = {
    "base": ["-Copt-level=0", "-Zmir-opt-level=0"],
    "O0": ["-Copt-level=0"],
    "O0-mir4": ["-Copt-level=0", "-Zmir-opt-level=4"],
    "O1": ["-Copt-level=1"],
    "O2": ["-Copt-level=2"],
    "O3": ["-Copt-level=3"],
    "Os": ["-Copt-level=s"],
    "Oz": ["-Copt-level=z"],
    "O3-mir4": ["-Copt-level=3", "-Zmir-opt-level=4"],
    "O3-lto": ["-Copt-level=3", "-Clto=fat", "-Ccodegen-units=1"],
    "O2-cgu16": ["-Copt-level=2", "-Ccodegen-units=16"],
    "O3-native": ["-Copt-level=3", "-Ctarget-cpu=native"],
    "cranelift": ["-Copt-level=0", "-Zcodegen-backend=cranelift"],
}
# Tests whose outcome legitimately depends on optimization: unspecified behavior (whether two
# equal promoted constants share an address), stack usage, or a backend's documented gaps.
NOISE = {
    "mir/mir_raw_fat_ptr.rs": {"cranelift"},  # compares the addresses of two `&0u8`
    "codegen/StackColoring-not-blowup-stack-issue-40883.rs": {"O0-mir4", "O0"},  # stack usage
    "attributes/fn-align-dyn.rs": {"cranelift"},  # Cranelift ignores #[align] on functions
    "backtrace/backtrace.rs": {"cranelift"},  # Cranelift backtraces lack frames
}
# Tests that choose these themselves are left out: the configuration would contradict them.
OWN = re.compile(r"^-O$|opt-level|mir-opt-level|overflow-checks|debug-assertions|codegen-backend|"
                 r"mir-enable-passes|^-Clto|lto=|target-cpu|panic=|-Cpanic|prefer-dynamic|-Zbuild-std")

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--cranelift", help="a rustc with the cranelift backend (the pinned nightly)")
p.add_argument("--tests", required=True)
p.add_argument("--work", required=True)
p.add_argument("--configs", help="comma-separated subset (base is always built)")
p.add_argument("--only", help="tests whose path contains this")
p.add_argument("--known", help="file of test paths to leave out (known findings)")
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--pause-on-finding", action="store_true")
p.add_argument("--recheck", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
WORK.mkdir(parents=True, exist_ok=True)
(WORK / "scratch").mkdir(exist_ok=True)
configs = {k: v for k, v in CONFIGS.items()
           if k == "base" or not args.configs or k in args.configs.split(",")}
if not args.cranelift:
    configs.pop("cranelift", None)
known = set(Path(args.known).read_text().split()) if args.known else set()


def normalized(stderr):
    lines = [l for l in stderr.decode(errors="replace").splitlines()
             if not l.startswith(("note: run with `RUST_BACKTRACE", "note: Some details are omitted"))]
    text = "\n".join(lines)
    # Panic messages name the thread with its OS id: `thread 'main' (909942) panicked`.
    text = re.sub(r"(thread '[^']*') \(\d+\)", r"\1", text)
    # A toolchain with rust-src prints std's own paths in full.
    return re.sub(r"\S*/lib/rustlib/src/rust/library/", "library/", text)


def observe(binary):
    # Every configuration's program runs from the same path: some tests print argv[0].
    fixed = binary.parent.parent / "run" / "prog"
    fixed.parent.mkdir(exist_ok=True)
    shutil.copy2(binary, fixed)
    code, out, err = uitest.run(fixed)
    stdout = out.decode(errors="replace")
    # The test harness prints how long tests took.
    stdout = re.sub(r"finished in \d+\.\d+s", "finished in …s", stdout)
    return {"exit": code, "stdout": stdout, "stderr": normalized(err)}


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        built, runs = {}, {}
        text = path.read_text(errors="replace")
        for name, cfg in configs.items():
            # Cranelift does not unwind on this target yet (catch_unwind catches nothing).
            if name == "cranelift" and ("catch_unwind" in text or "needs-unwind" in text):
                continue
            rustc = args.cranelift if name == "cranelift" else args.rustc
            status, stderr, binary = uitest.compile(rustc, path.resolve(), d / name, flags, edition, FIXED + cfg)
            built[name] = {"status": status, "stderr": stderr[-2000:]}
            if binary:
                runs[name] = observe(binary)
        record = {"test": rel, "kind": kind, "build": {k: v["status"] for k, v in built.items()}}
        if built["base"]["status"] != "ok":
            record["skip"] = "baseline does not build"
            return record, []
        again = observe(d / "base" / "prog")
        if again != runs["base"]:
            record["skip"] = "baseline is nondeterministic"
            return record, []
        found = []
        for name in configs:
            if name == "base" or name not in built or name in NOISE.get(rel, ()):
                continue
            b = built[name]["status"]
            if b != "ok":
                # The Cranelift backend has documented gaps (unsupported intrinsics, inline asm).
                if name == "cranelift" and b == "error":
                    continue
                found.append({"config": name, "what": f"build {b}", "stderr": built[name]["stderr"]})
                continue
            # Cranelift cannot unwind on this target yet: a panic aborts.
            if name == "cranelift" and "failed to initiate panic" in runs[name]["stderr"]:
                continue
            if runs[name] != runs["base"]:
                retry = observe(d / name / "prog")
                if retry != runs["base"] and retry == runs[name]:
                    diff = [k for k in ("exit", "stdout", "stderr") if runs[name][k] != runs["base"][k]]
                    found.append({"config": name, "what": "run differs: " + ",".join(diff),
                                  "base": runs["base"], "got": runs[name]})
        if found:
            out = WORK / "findings" / rel.replace("/", "__")
            shutil.rmtree(out, ignore_errors=True)
            out.mkdir(parents=True)
            shutil.copy(path, out / path.name)
            (out / "finding.json").write_text(json.dumps({
                "test": rel, "flags": flags, "edition": edition, "fixed": FIXED,
                "configs": {f["config"]: configs[f["config"]] for f in found}, "found": found,
                "base": runs["base"]}, indent=1))
        record["found"] = [f"{f['config']}: {f['what']}" for f in found]
        return record, found


def main():
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, ("run-pass", "run-fail"),
                                                   lambda text, flags: any(OWN.search(f) for f in flags)):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (args.recheck and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests, configurations: {', '.join(configs)}", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()
