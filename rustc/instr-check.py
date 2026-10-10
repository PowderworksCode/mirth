#!/usr/bin/env python3
"""Instrumentation round trip: instrumented programs must behave as uninstrumented ones and write
profiles that LLVM's tools accept.

For each runnable UI test (run-pass), with a toolchain that ships the profiler runtime and
llvm-tools (the pinned nightly):

  pgo       build with `-Cprofile-generate`, run, `llvm-profdata merge` the raw profile, rebuild
            with `-Cprofile-use`, run again
  coverage  build with `-Cinstrument-coverage`, run, merge, `llvm-cov export` the binary

Findings: an instrumented or profile-guided program whose exit status or stdout differs from the
plain build; no raw profile written; `llvm-profdata` or `llvm-cov` failing or crashing, or
warning about corrupt or malformed data; the compiler crashing on `-Cprofile-use`.

    rustc/instr-check.py --toolchain nightly-2026-10-06 --tests <rust>/tests/ui --work <dir>
        [--only <substr>] [--limit N] [--jobs 8] [--pause-on-finding] [--recheck]
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import uitest  # noqa: E402

p = argparse.ArgumentParser()
p.add_argument("--toolchain", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--work", required=True)
p.add_argument("--only")
p.add_argument("--limit", type=int)
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--pause-on-finding", action="store_true")
p.add_argument("--recheck", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
(WORK / "scratch").mkdir(parents=True, exist_ok=True)
TC = Path.home() / f".rustup/toolchains/{args.toolchain}-x86_64-unknown-linux-gnu"
RUSTC = TC / "bin/rustc"
TOOLS = TC / "lib/rustlib/x86_64-unknown-linux-gnu/bin"
OWN = re.compile(r"profile|instrument-coverage|coverage-options|-O$|opt-level|panic=|prefer-dynamic|"
                 r"codegen-backend|-Clto|lto=|no-prepopulate")
BAD = re.compile(r"corrupt|malformed|invalid|truncated|failed to|error", re.I)


def tool(name, *a, cwd=None):
    try:
        r = subprocess.run([str(TOOLS / name), *a], capture_output=True, text=True, timeout=120, cwd=cwd)
        return r.returncode, r.stderr + r.stdout[-200:]
    except subprocess.TimeoutExpired:
        return "timeout", ""


def observe(binary, env=None):
    try:
        r = subprocess.run([str(binary)], capture_output=True, timeout=30, cwd=binary.parent,
                           stdin=subprocess.DEVNULL, env=dict(os.environ, RUST_BACKTRACE="0", **(env or {})))
    except subprocess.TimeoutExpired:
        return ("timeout", "")
    code = r.returncode if r.returncode >= 0 else f"signal {-r.returncode}"
    return (code, r.stdout.decode(errors="replace"))


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    record = {"test": rel}
    found = []
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        status, _, plain = uitest.compile(RUSTC, path.resolve(), d / "plain", flags, edition, ["-Copt-level=1"])
        if plain is None:
            record["skip"] = f"plain build {status}"
            return record, []
        base = observe(plain)
        if base != observe(plain):
            record["skip"] = "nondeterministic"
            return record, []
        # PGO: generate, merge, use.
        raw = d / "pgo-raw"
        status, err, gen = uitest.compile(RUSTC, path.resolve(), d / "gen", flags, edition,
                                          ["-Copt-level=1", f"-Cprofile-generate={raw}"])
        if gen is None:
            found.append({"what": f"profile-generate build {status}", "stderr": err[-1500:]})
        else:
            got = observe(gen)
            if got != base:
                found.append({"what": "profile-generate changes behavior", "base": base, "got": got})
            profraws = list(raw.glob("*.profraw")) if raw.exists() else []
            if not profraws and got[0] == 0:
                found.append({"what": "no raw profile written"})
            elif profraws:
                merged = d / "merged.profdata"
                code, out = tool("llvm-profdata", "merge", "-o", str(merged), *map(str, profraws))
                if code != 0 or BAD.search(out):
                    found.append({"what": "llvm-profdata merge", "code": code, "out": out[-1500:]})
                else:
                    status, err, use = uitest.compile(RUSTC, path.resolve(), d / "use", flags, edition,
                                                      ["-Copt-level=2", f"-Cprofile-use={merged}"])
                    if status == "ice":
                        found.append({"what": "profile-use ICE", "stderr": err[-2000:]})
                    elif use is None:
                        found.append({"what": f"profile-use build {status}", "stderr": err[-1500:]})
                    elif observe(use) != base:
                        found.append({"what": "profile-use changes behavior", "base": base, "got": observe(use)})
        # Coverage: instrument, merge, export.
        status, err, cov = uitest.compile(RUSTC, path.resolve(), d / "cov", flags, edition, ["-Cinstrument-coverage"])
        if cov is None:
            found.append({"what": f"instrument-coverage build {status}", "stderr": err[-1500:]})
        else:
            craw = d / "cov-raw"
            craw.mkdir()
            got = observe(cov, {"LLVM_PROFILE_FILE": str(craw / "c-%p.profraw")})
            if got != base:
                found.append({"what": "instrument-coverage changes behavior", "base": base, "got": got})
            profraws = list(craw.glob("*.profraw"))
            if profraws:
                merged = d / "cov.profdata"
                code, out = tool("llvm-profdata", "merge", "-sparse", "-o", str(merged), *map(str, profraws))
                if code != 0 or BAD.search(out):
                    found.append({"what": "llvm-profdata merge (coverage)", "code": code, "out": out[-1500:]})
                else:
                    code, out = tool("llvm-cov", "export", "-summary-only", f"-instr-profile={merged}", str(cov))
                    if code != 0:
                        found.append({"what": "llvm-cov export", "code": code, "out": out[-1500:]})
            elif got[0] == 0:
                found.append({"what": "no coverage profile written"})
    record["found"] = [f["what"] for f in found]
    if found:
        out = WORK / "findings" / rel.replace("/", "__")
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        shutil.copy(path, out / path.name)
        (out / "finding.json").write_text(json.dumps({"test": rel, "flags": flags, "edition": edition,
                                                      "found": found}, indent=1, default=str))
    return record, found


def main():
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, ("run-pass",),
                                                   lambda text, flags: any(OWN.search(f) for f in flags)):
        rel = str(path.relative_to(args.tests))
        if (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    if args.limit:
        todo = todo[:args.limit]
    print(f"{len(todo)} tests", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()
