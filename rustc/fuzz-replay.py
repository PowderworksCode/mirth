#!/usr/bin/env python3
"""Replay a fuzz finding: apply its edits to the pristine fixture in order,
with an incremental build after each, then compare with a clean build.

    rustc/fuzz-replay.py --rustc <rustc> --fixture fixtures/sink --finding <dir> --work <dir> [--upto N]

Prints which .rmeta files differ, and keeps the compiler's output of the last
incremental build and the clean build as inc.log and clean.log. --upto replays only the first N edits that
were kept, to find where the difference appears.
"""

import argparse
import json
import os
import shutil
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--fixture", required=True)
p.add_argument("--finding", required=True)
p.add_argument("--work", required=True)
p.add_argument("--upto", type=int, default=None)
p.add_argument("--toolchain", default="nightly-2026-10-06")
p.add_argument("--rustflags", default="-Zincremental-verify-ich")
p.add_argument("--quiet", action="store_true")
args = p.parse_args()

work = Path(args.work).resolve()
src, target, inc_target = work / "src", work / "target", work / "target-inc"
env = dict(os.environ, RUSTC=args.rustc, RUSTC_WRAPPER="", CARGO_INCREMENTAL="1", RUSTFLAGS=args.rustflags)


def build(t):
    r = subprocess.run(["cargo", f"+{args.toolchain}", "build", "--workspace", "--offline", "-j", "4",
                        "--target-dir", str(t), "--message-format=json-render-diagnostics"],
                       cwd=src, env=env, capture_output=True, text=True)
    rmetas = {}
    for line in r.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") == "compiler-artifact":
            for f in msg["filenames"]:
                if f.endswith(".rmeta"):
                    rmetas[str(Path(f).relative_to(t))] = Path(f).read_bytes()
    build.log = r.stderr
    return r.returncode == 0, rmetas


if work.exists():
    shutil.rmtree(work)
work.mkdir(parents=True)
shutil.copytree(Path(args.fixture).resolve(), src, ignore=shutil.ignore_patterns("target", "edits", "edit"))
build(target)

history = json.loads((Path(args.finding) / "history.json").read_text())
kept = 0
last = None
for step in history:
    if step["edit"] == "revert":
        (src / last["file"]).write_text(last["before"])
    else:
        if args.upto is not None and step["kept"] and kept >= args.upto:
            break
        (src / step["file"]).write_text(step["after"])
        last = step
        kept += step["kept"]
    ok, _ = build(target)
    if not args.quiet:
        print(f"{step['edit']:18} {step['file']:24} {'built' if ok else 'failed'}")

ok, inc = build(target)
(work / "inc.log").write_text(build.log)
target.rename(inc_target)
ok2, clean = build(target)
(work / "clean.log").write_text(build.log)
differ = sorted(r for r in set(inc) | set(clean) if inc.get(r) != clean.get(r))
print(json.dumps({"kept_edits": kept, "inc_ok": ok, "clean_ok": ok2, "differ": [Path(d).name for d in differ]}))
for d in differ:
    name = Path(d).name
    (work / (name + ".inc")).write_bytes(inc.get(d, b""))
    (work / (name + ".clean")).write_bytes(clean.get(d, b""))
