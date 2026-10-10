#!/usr/bin/env python3
"""Determinism: what rustc writes must depend only on its inputs and options.

Builds each standalone UI test that compiles (build-pass, run-pass, check-pass) several times
and compares the outputs (`.rmeta`, `.rlib` normalized as artifacts.py does, executables) with
the first build:

  repeat    the same build again, in the same directory
  path      the same build in another directory, both with `--remap-path-prefix` to one name:
            what is left of the directory is a path leaking past the remapping
  threads   `-Zthreads=8` (the parallel front end; tests marked `ignore-parallel-frontend` skip
            it); known: async fns (rust-lang/rust#162202), RPIT and impl Trait in traits (#163878)
  decoy     a `-L` directory holding an unrelated library whose name starts with the crate's
            name (rust-lang/rust#159677's shape)

    rustc/repro-diff.py --rustc <rustc> --tests <rust>/tests/ui --work <dir> [--variants repeat,path]
        [--only <substr>] [--known <file>] [--jobs 8] [--pause-on-finding] [--recheck]
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import artifacts  # noqa: E402
import uitest  # noqa: E402

VARIANTS = ["repeat", "path", "threads", "decoy"]

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--tests", required=True)
p.add_argument("--work", required=True)
p.add_argument("--variants")
p.add_argument("--only")
p.add_argument("--known")
p.add_argument("--jobs", type=int, default=8)
p.add_argument("--pause-on-finding", action="store_true")
p.add_argument("--recheck", action="store_true")
args = p.parse_args()
WORK = Path(args.work).resolve()
(WORK / "scratch").mkdir(parents=True, exist_ok=True)
variants = args.variants.split(",") if args.variants else VARIANTS
known = set(Path(args.known).read_text().split()) if args.known else set()
OWN = re.compile(r"threads|remap-path|-o\b|--out-dir|emit|crate-name|extern|-L\b|-Cincremental")


def build(src_dir, name, flags, edition, kind, extra):
    """Compile `src_dir/name`; {output file: digest}, or None if it fails."""
    out = src_dir / "out"
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir()
    emit = "--emit=metadata" if kind == "check-pass" else "--emit=link,metadata"
    argv = [args.rustc, name, "--edition", edition or "2015", emit, "--out-dir", "out", "--crate-name", "t",
            "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features",
            f"--remap-path-prefix={src_dir}=/src", *flags, *extra]
    try:
        r = subprocess.run(argv, capture_output=True, timeout=300, cwd=src_dir,
                           env=dict(os.environ, RUSTC_BOOTSTRAP="1", RUST_BACKTRACE="0"))
    except subprocess.TimeoutExpired:
        return None
    if r.returncode != 0:
        return None
    files = {}
    for f in sorted(out.iterdir()):
        if f.suffix == ".rlib":
            files[f.name] = artifacts.normalized_rlib(f)
        elif f.is_file():
            files[f.name] = hashlib.sha256(f.read_bytes()).hexdigest()
    return files


def one(path, flags, edition, kind):
    rel = str(path.relative_to(args.tests))
    record = {"test": rel}
    with tempfile.TemporaryDirectory(dir=WORK / "scratch") as d:
        d = Path(d)
        a, b = d / "a", d / "elsewhere-b"
        for x in (a, b):
            x.mkdir()
            shutil.copy(path, x / path.name)
        base = build(a, path.name, flags, edition, kind, [])
        if base is None:
            record["skip"] = "does not build"
            return record, []
        found = []
        text = path.read_text(errors="replace")
        for v in variants:
            # Tests the parallel front end is known not to handle say so.
            if v == "threads" and re.search(r"^//@\s*ignore-parallel-frontend", text, re.M):
                continue
            if v == "repeat":
                got = build(a, path.name, flags, edition, kind, [])
            elif v == "path":
                got = build(b, path.name, flags, edition, kind, [])
            elif v == "threads":
                got = build(a, path.name, flags, edition, kind, ["-Zthreads=8"])
            elif v == "decoy":
                decoy = d / "decoy"
                decoy.mkdir(exist_ok=True)
                # An unrelated library sharing the crate's name as a prefix.
                (decoy / "libtother.rlib").write_bytes(b"!<arch>\n")
                (decoy / "libt-0123456789abcdef.rmeta").write_bytes(b"rust\0\0\0\0")
                got = build(a, path.name, flags, edition, kind, ["-L", str(decoy)])
            if got is None:
                found.append({"variant": v, "what": "does not build"})
            elif got != base:
                differ = sorted(k for k in set(got) | set(base) if got.get(k) != base.get(k))
                found.append({"variant": v, "what": "outputs differ: " + ", ".join(differ)})
        # A difference also in `repeat` is nondeterminism of the build itself: report only that.
        if any(f["variant"] == "repeat" for f in found):
            found = [f for f in found if f["variant"] == "repeat"]
        record["found"] = [f"{f['variant']}: {f['what']}" for f in found]
        if found:
            out = WORK / "findings" / rel.replace("/", "__")
            shutil.rmtree(out, ignore_errors=True)
            out.mkdir(parents=True)
            shutil.copy(path, out / path.name)
            (out / "finding.json").write_text(json.dumps({"test": rel, "flags": flags, "edition": edition,
                                                          "found": found}, indent=1))
        return record, found


def main():
    wanted = None
    if args.recheck:
        wanted = {json.loads((f / "finding.json").read_text())["test"] for f in (WORK / "findings").glob("*")}
    todo = []
    for path, flags, edition, kind in uitest.tests(args.tests, ("build-pass", "run-pass", "check-pass"),
                                                   lambda text, flags: any(OWN.search(f) for f in flags)):
        rel = str(path.relative_to(args.tests))
        if rel in known or (args.only and args.only not in rel) or (wanted is not None and rel not in wanted):
            continue
        todo.append((path, flags, edition, kind))
    print(f"{len(todo)} tests, variants: {', '.join(variants)}", flush=True)
    sys.exit(uitest.drive(todo, one, WORK / "results.jsonl", args.jobs, args.pause_on_finding))


main()
