#!/usr/bin/env python3
"""Release-to-release: code that one toolchain accepts the next must accept too, in comparable
time.

Runs `cargo check --locked` on each repository of a corpus of real crates with two toolchains
(an older and a newer rustup toolchain, or a local rustc via a rustup-linked name), each in its
own target directory, deleted afterwards. Compared:

  regression  the older toolchain checks the repository, the newer one does not (the new
              error codes and the first error are recorded)
  fixed       the other way round (reported, not a finding)
  slower      the newer toolchain takes more than --slower times as long (both succeed)
  ice         the newer toolchain crashes

Dependencies are fetched first (`cargo fetch --locked`), so the timed runs are offline.

    rustc/release-diff.py --corpus <dir of repositories> --old <toolchain> --new <toolchain>
        --work <dir> [--only <substr>] [--jobs 2] [--slower 1.5] [--timeout 1800]

Writes <work>/results.jsonl and prints regressions, crashes and slowdowns.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--corpus", required=True)
p.add_argument("--old", required=True)
p.add_argument("--new", required=True)
p.add_argument("--work", required=True)
p.add_argument("--only")
p.add_argument("--jobs", type=int, default=2)
p.add_argument("--slower", type=float, default=1.5)
p.add_argument("--timeout", type=int, default=1800)
args = p.parse_args()
WORK = Path(args.work).resolve()
WORK.mkdir(parents=True, exist_ok=True)


def check(repo, toolchain):
    target = WORK / "target" / f"{repo.name}-{toolchain}"
    shutil.rmtree(target, ignore_errors=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_TERM_COLOR="never",
               CARGO_INCREMENTAL="0", RUSTFLAGS="--cap-lints=warn")
    env.pop("RUSTC_WRAPPER", None)
    start = time.time()
    try:
        r = subprocess.run(["cargo", f"+{toolchain}", "check", "--locked", "--offline", "--workspace",
                            "--message-format=short"], cwd=repo, env=env, capture_output=True, text=True,
                           timeout=args.timeout)
        code, err = r.returncode, r.stderr
    except subprocess.TimeoutExpired:
        code, err = "timeout", ""
    seconds = round(time.time() - start, 1)
    shutil.rmtree(target, ignore_errors=True)
    ice = "internal compiler error" in err or "the compiler unexpectedly panicked" in err
    codes = sorted(set(re.findall(r"error\[(E\d{4})\]", err)))
    first = next((l for l in err.splitlines() if re.search(r"\berror(\[E\d+\])?:", l)), "")
    return {"code": code, "seconds": seconds, "ice": ice, "codes": codes, "first": first[:300],
            "tail": err[-3000:] if code != 0 else ""}


def one(repo):
    fetch = subprocess.run(["cargo", f"+{args.new}", "fetch", "--locked"], cwd=repo, capture_output=True,
                           text=True, timeout=1800)
    if fetch.returncode != 0:
        return {"repo": repo.name, "skip": "fetch failed", "why": fetch.stderr[-400:]}
    old = check(repo, args.old)
    new = check(repo, args.new)
    rec = {"repo": repo.name, "old": old, "new": new, "found": []}
    if old["code"] == 0 and new["code"] != 0:
        rec["found"].append("ice" if new["ice"] else "regression")
    elif old["code"] != 0 and new["code"] == 0:
        rec["notes"] = ["fixed"]
    elif old["code"] == 0 and new["code"] == 0 and old["seconds"] > 5 and new["seconds"] > args.slower * old["seconds"]:
        rec["found"].append(f"slower: {old['seconds']}s -> {new['seconds']}s")
    if new["ice"] and "ice" not in rec["found"]:
        rec["found"].append("ice")
    return rec


def main():
    repos = sorted(p for p in Path(args.corpus).iterdir() if (p / "Cargo.toml").exists()
                   and (not args.only or args.only in p.name))
    print(f"{len(repos)} repositories, {args.old} -> {args.new}", flush=True)
    with ThreadPoolExecutor(args.jobs) as ex, (WORK / "results.jsonl").open("a") as out:
        for rec in ex.map(one, repos):
            out.write(json.dumps(rec) + "\n")
            out.flush()
            tag = rec.get("skip") or ", ".join(rec.get("found", [])) or "same"
            old, new = rec.get("old", {}), rec.get("new", {})
            print(f"{rec['repo']:45} {tag:30} old {old.get('code')} {old.get('seconds')}s  "
                  f"new {new.get('code')} {new.get('seconds')}s  {new.get('first', '')[:80]}", flush=True)


main()
