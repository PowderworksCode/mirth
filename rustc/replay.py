#!/usr/bin/env python3
"""Replay a crate's git history through incremental compilation.

For each first-parent commit, oldest first, the workspace is checked out and
built incrementally on top of the previous commit's build, then built again
from scratch, and the two are compared:

  P6     every .rmeta Cargo reports for a workspace member is identical
  rlib   every rlib's members are identical, object code included
  diag   both builds printed the same diagnostics
  reuse  the compiler's own check of what it reused (RUSTC_VERIFY_REUSE,
         docs/hunt/verify-reuse.patch) found nothing stale
  ICE    neither build crashed the compiler
  split  both builds succeed or both fail

Registry dependencies are not compiled incrementally by Cargo, so the clean
build starts from a copy of the incremental target directory with the
workspace members and the incremental cache removed, and only the members are
built again. Both builds use the same target directory path, since Cargo
derives a crate's identity from paths.

    rustc/replay.py --rustc <rustc> --repo <git checkout> --work <dir> [--commits 2000]

Writes <work>/results.jsonl, one line per commit, and keeps the logs of every
problem and both .rmeta files of the first few differences per crate in
<work>/findings.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import artifacts  # noqa: E402

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--repo", required=True)
p.add_argument("--work", required=True)
p.add_argument("--commits", type=int, default=2000)
p.add_argument("--toolchain", default="nightly-2026-10-06", help="for cargo")
p.add_argument("--jobs", default="4")
p.add_argument("--keep", type=int, default=3, help="differences kept per crate")
p.add_argument("--from", dest="start", type=int, default=0, help="first commit index to replay")
p.add_argument("--to", dest="end", type=int, default=None, help="last commit index to replay")
p.add_argument("--no-verify-reuse", action="store_true",
               help="do not set RUSTC_VERIFY_REUSE (needs a compiler with docs/hunt/verify-reuse.patch)")
args = p.parse_args()

work = Path(args.work).resolve()
src, target, inc_target = work / "src", work / "target", work / "target-inc"
findings = work / "findings"
work.mkdir(parents=True, exist_ok=True)
findings.mkdir(exist_ok=True)
if not src.exists():
    subprocess.run(["git", "clone", "-q", args.repo, str(src)], check=True)

env = dict(os.environ)
env.update(
    RUSTC=args.rustc,
    RUSTC_WRAPPER="",
    CARGO_INCREMENTAL="1",
    # A commit that denies warnings would otherwise stop building with a newer compiler.
    RUSTFLAGS="--cap-lints=warn",
    CARGO_TERM_COLOR="never",
)
if not args.no_verify_reuse:
    env["RUSTC_VERIFY_REUSE"] = "1"
cargo = ["cargo", f"+{args.toolchain}"]


def run(cmd, **kw):
    return subprocess.run(cmd, cwd=src, env=env, capture_output=True, text=True, **kw)


def members():
    """Every package built from a path: workspace members and path dependencies."""
    r = run(cargo + ["metadata", "--format-version", "1"])
    if r.returncode != 0:
        r = run(cargo + ["metadata", "--no-deps", "--format-version", "1"])
        if r.returncode != 0:
            return []
    return sorted({pkg["name"] for pkg in json.loads(r.stdout)["packages"] if pkg.get("source") is None})


def build(target_dir):
    """Build, and collect the .rmeta files Cargo reports for workspace members."""
    t = time.time()
    r = run(cargo + ["build", "--lib", "-j", args.jobs, "--target-dir", str(target_dir),
                     "--message-format=json-render-diagnostics"])
    rmetas, fresh = {}, []
    for line in r.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") != "compiler-artifact" or "path+file" not in msg.get("package_id", ""):
            continue
        for f in msg.get("filenames", []):
            if f.endswith(".rmeta"):
                rmetas[str(Path(f).relative_to(target_dir))] = Path(f).read_bytes()
                if msg.get("fresh"):
                    fresh.append(msg["target"]["name"])
    log = r.stderr
    return {
        "ok": r.returncode == 0,
        "ice": "internal compiler error" in log or "the compiler unexpectedly panicked" in log,
        "secs": round(time.time() - t, 1),
        "log": log[-6000:],
        "reuse": [line[:3000] for line in log.splitlines() if line.startswith("rustc-verify-reuse:")],
        "rmetas": rmetas,
        "fresh": fresh,
        "art": artifacts.collect(r.stdout, target_dir),
    }


def keep_dir(i, commit):
    d = findings / f"{i:05}-{commit[:10]}"
    d.mkdir(exist_ok=True)
    return d


commits = run(["git", "rev-list", "--first-parent", "--reverse", f"--max-count={args.commits}", "origin/HEAD"]).stdout.split()
if not commits:
    commits = run(["git", "rev-list", "--first-parent", "--reverse", f"--max-count={args.commits}", "HEAD"]).stdout.split()
results = work / "results.jsonl"
done = {json.loads(line)["commit"] for line in results.open()} if results.exists() else set()
kept = {}

for i, commit in enumerate(commits):
    if commit in done or i < args.start or (args.end is not None and i > args.end):
        continue
    run(["git", "checkout", "-q", "--force", commit])
    run(["git", "clean", "-fdxq"])
    date = run(["git", "log", "-1", "--format=%cs", commit]).stdout.strip()
    names = members()
    inc = build(target)

    # The clean build: the same target directory path, starting from a copy with the
    # workspace members and the incremental cache removed.
    if inc_target.exists():
        shutil.rmtree(inc_target)
    if target.exists():
        target.rename(inc_target)
        shutil.copytree(inc_target, target, symlinks=True)
    else:
        inc_target.mkdir()
    # Cargo refuses to clean a directory it did not create; this one may have been
    # created here when the first build failed early.
    target.mkdir(exist_ok=True)
    tag = target / "CACHEDIR.TAG"
    if not tag.exists():
        tag.write_text("Signature: 8a477f597d28d172789f06886806bc55\n")
    for name in names:
        run(cargo + ["clean", "-p", name, "--target-dir", str(target)])
    for d in target.glob("*/incremental"):
        shutil.rmtree(d)
    clean = build(target)

    problems, differ = [], []
    if inc["ice"] or clean["ice"]:
        problems.append("ICE")
    if inc["ok"] != clean["ok"]:
        problems.append("split")
    if inc["reuse"]:
        # The compiler's own check found something it reused stale.
        problems.append("reuse")
        (keep_dir(i, commit) / "reuse.txt").write_text("\n".join(inc["reuse"]))
    if clean["fresh"]:
        # A member the clean build did not compile again would be compared with itself.
        problems.append("stale")
    if inc["ok"] and clean["ok"] and not clean["fresh"]:
        a, b = inc["rmetas"], clean["rmetas"]
        differ = sorted(rel for rel in set(a) | set(b) if a.get(rel) != b.get(rel))
        # More oracles: object code in the rlibs and the diagnostics.
        for kind, detail in artifacts.compare(inc["art"], clean["art"]).items():
            if kind in ("rlib", "diag"):
                problems.append(kind)
                (keep_dir(i, commit) / f"{kind}.txt").write_text("\n".join(detail))
        if differ:
            problems.append("P6")
            for rel in differ:
                crate = Path(rel).name.split("-")[0]
                if kept.get(crate, 0) < args.keep:
                    kept[crate] = kept.get(crate, 0) + 1
                    d = keep_dir(i, commit)
                    (d / f"{Path(rel).name}.inc").write_bytes(a.get(rel, b""))
                    (d / f"{Path(rel).name}.clean").write_bytes(b.get(rel, b""))
    if problems:
        d = keep_dir(i, commit)
        (d / "inc.log").write_text(inc["log"])
        (d / "clean.log").write_text(clean["log"])

    # Continue incrementally from the incremental build.
    shutil.rmtree(target, ignore_errors=True)
    inc_target.rename(target)
    record = {
        "i": i, "commit": commit, "date": date, "members": names,
        "inc": {k: inc[k] for k in ("ok", "ice", "secs")},
        "clean": {k: clean[k] for k in ("ok", "ice", "secs")},
        "compared": len(inc["rmetas"]), "differ": differ, "problems": problems,
    }
    with results.open("a") as f:
        f.write(json.dumps(record) + "\n")
    status = "both ok" if inc["ok"] and clean["ok"] else f"inc {'ok' if inc['ok'] else 'failed'}, clean {'ok' if clean['ok'] else 'failed'}"
    print(f"{i:5} {date} {commit[:10]} {status}, {len(inc['rmetas'])} compared, "
          f"{inc['secs']}s/{clean['secs']}s {' '.join(problems)}", flush=True)
