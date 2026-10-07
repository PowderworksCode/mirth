#!/usr/bin/env python3
"""Audit rustc's [UNTRACKED] options for stale incremental reuse.

An option rustc marks [UNTRACKED] is left out of the dependency-tracking hash,
so changing it between incremental sessions reuses the previous session's
results. That is only correct if the option cannot change them. For each
untracked option that takes no value or a boolean, this builds a crate
incrementally without it, then again with it, and compares the result with a
clean build that has it: the .rmeta, each .rlib member (object code, with the
incremental session suffix removed from names), the diagnostics, and the files
written. A difference means the option changes output that incremental
compilation reuses: it should be tracked, or the reuse checked.

    rustc/audit-options.py --rustc <rustc> --source <rust checkout> --crate <lib.rs> [--edition 2018] [ARG…]

With ARGs, audits those arguments instead of the options found in the
checkout's compiler/rustc_session/src/options.rs. The crate should have code of
its own for codegen options to change (non-generic functions) and a warning or
two for diagnostic options to change.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import artifacts  # noqa: E402

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--source", help="a rust checkout, to read the untracked options from")
p.add_argument("--crate", required=True, help="the crate root to build, as a library")
p.add_argument("--crate-name", default="audited")
p.add_argument("--edition", default="2021")
p.add_argument("args", nargs="*")
args = p.parse_args()

# Options that stop compilation or change how arguments are read.
SKIP = {"-Chelp", "-Zhelp", "-Zno-analysis", "-Zparse-crate-root-only=yes", "-Zshell-argfiles=yes"}


def untracked_boolean_options(source):
    text = (Path(source) / "compiler/rustc_session/src/options.rs").read_text()
    codegen = re.search(r"options! \{\s*CodegenOptions,", text).start()
    unstable = re.search(r"options! \{\s*UnstableOptions,", text).start()
    found = []
    for m in re.finditer(r"^\s{4}(\w+):\s*([^=\n]+?)\s*=\s*\(([^,]*),\s*(parse_\w+),\s*\[UNTRACKED\]", text, re.M):
        name, _ty, default, parser = m.groups()
        if parser not in ("parse_bool", "parse_no_value", "parse_opt_bool"):
            continue
        if m.start() < codegen:
            continue
        group = "Z" if m.start() > unstable else "C"
        flag = f"-{group}{name.replace('_', '-')}"
        if parser != "parse_no_value":
            flag += "=no" if default.strip() in ("true", "Some(true)") else "=yes"
        if flag not in SKIP:
            found.append(flag)
    return found


crate = Path(args.crate).resolve()
name = args.crate_name


def build(work, incremental, out, extra):
    """One build, always from the same working directory, which rustc records."""
    (work / out).mkdir(exist_ok=True)
    r = subprocess.run([args.rustc, "--edition", args.edition, "--crate-type", "lib", "--crate-name", name,
                        "--emit=metadata,link", f"-Cincremental={work / incremental}", "--out-dir", str(work / out),
                        str(crate)] + extra, capture_output=True, text=True, cwd=work)
    diagnostics = sorted(l for l in r.stderr.splitlines() if l.startswith(("warning", "error")) or "-->" in l)
    return r.returncode, diagnostics


def audit(flag):
    work = Path(tempfile.mkdtemp(prefix="audit-"))
    try:
        extra = [flag] if flag else []
        rc0, _ = build(work, "i", "o1", [])
        rc1, diag_inc = build(work, "i", "o1", extra)
        rc2, diag_clean = build(work, "j", "o2", extra)
        if rc0 or rc2:
            return [f"the crate does not build (exit {rc0} without the option, {rc2} with it)"]
        problems = []
        if rc1 != rc2:
            problems.append(f"the incremental rebuild exits {rc1}, a clean build {rc2}")
        if (work / f"o1/lib{name}.rmeta").read_bytes() != (work / f"o2/lib{name}.rmeta").read_bytes():
            problems.append("metadata")
        a = artifacts.normalized_rlib(work / f"o1/lib{name}.rlib")
        b = artifacts.normalized_rlib(work / f"o2/lib{name}.rlib")
        members = [m for m in set(a) | set(b) if a.get(m) != b.get(m)]
        if members:
            problems.append(f"object code ({len(members)} rlib members differ or exist on one side)")
        if diag_inc != diag_clean:
            problems.append(f"diagnostics ({len(diag_inc)} lines incrementally, {len(diag_clean)} clean)")
        missing = set(os.listdir(work / "o2")) - set(os.listdir(work / "o1"))
        if missing:
            problems.append(f"{len(missing)} files only a clean build writes")
        return problems
    finally:
        shutil.rmtree(work)


flags = args.args or untracked_boolean_options(args.source)
results = {}
control = audit("")
print(f"{'(control: no option)':40} {'; '.join(control) or 'same'}", flush=True)
if control:
    sys.exit("the control differs: incremental and clean builds disagree without any option")
for flag in flags:
    results[flag] = audit(flag)
    print(f"{flag:40} {'; '.join(results[flag]) or 'same'}", flush=True)
sys.exit(1 if any(results.values()) else 0)
