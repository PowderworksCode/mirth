#!/usr/bin/env python3
"""Check that each closed-bug query still finds the code its bug's fix changed.

For each case, fetches the files the fixing PR changed, as they were at the PR's
base commit (GH_HOST=github.com gh), runs the query over them with Ur, and looks
for a site whose label and file match.

    ur/verify-closed.py <path to the ur binary> [work dir]
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

UR = sys.argv[1]
WORK = Path(sys.argv[2] if len(sys.argv) > 2 else "verify-closed").resolve()
HERE = Path(__file__).resolve().parent
ENV = dict(os.environ, GH_HOST="github.com")

# (bug, fixing PR, module, query, label contains, file contains, profile)
CASES = [
    ("#82920", 83074, "ClosedBugs", "sortByDefId", "dedup_by_key", "astconv", None),
    ("#89598", 89619, "ClosedBugs", "contextCache", "vtables_cache", "context.rs", None),
    ("#84252", 84260, "ClosedBugs", "untrackedCrateStore", "has_global_allocator", "cstore_impl", None),
    ("#40364", 71858, "ClosedBugs", "envRead", "env::var", "env.rs", "rust-2018"),
    ("#45841", 45899, "ClosedBugs", "writeInPlace", "out_filename", "link.rs", "rust-2015"),
    ("#117254", 117301, "ClosedBugs", "encoderNotFinished", "", "encoder.rs", None),
    ("#119456", 119510, "ClosedBugs", "writeErrorNotFatal", "encode_metadata", "encoder.rs", None),
    ("#34902", 35984, "ClosedBugs", "hashIterated", "xrefs", "encoder.rs", "rust-2015"),
    ("#65036", 65043, "RoundTrip", "hashOrderAlias", "Resolutions", "lib.rs", "rust-2018"),
    ("#66955", 84233, "ClosedBugs", "optionRead", "remap_path_prefix", "", None),
    ("#111227", 111641, "ClosedBugs", "fileRead", "", "debugger_visualizer", None),
]
# Files the fix did not change but the bug's site is in.
EXTRA = {84260: ["compiler/rustc_metadata/src/rmeta/decoder/cstore_impl.rs"]}


def gh(*args):
    return subprocess.run(["gh", *args], capture_output=True, text=True, env=ENV, check=True).stdout


def fetch(pr):
    d = WORK / str(pr)
    if d.exists():
        return d
    info = json.loads(gh("pr", "view", str(pr), "-R", "rust-lang/rust", "--json", "baseRefOid,files"))
    paths = [f["path"] for f in info["files"] if f["path"].endswith(".rs") and not f["path"].startswith(("tests/", "src/test/"))]
    for path in paths + EXTRA.get(pr, []):
        target = d / path
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            text = gh("api", f"repos/rust-lang/rust/contents/{path}?ref={info['baseRefOid']}", "-H", "Accept: application/vnd.github.raw")
        except subprocess.CalledProcessError:
            continue  # added by the fix: it did not exist before

        # Ur's grammars lack the old unstable `crate` visibility; it is not what the queries test.
        text = re.sub(r"^(\s*)crate (fn|struct|enum|type|mod|use|trait|const|static|unsafe fn) ", r"\1pub(crate) \2 ", text, flags=re.M)
        target.write_text(text)
    return d


failed = 0
for bug, pr, module, query, label, file, profile in CASES:
    d = fetch(pr)
    report = WORK / "report.json"
    cmd = [UR, "rewrite", "--classify", "--rules", str(HERE / f"rustc/{module}.rsc"), "--report", str(report), str(d)]
    if profile:
        cmd += ["--profile", profile]
    subprocess.run(cmd, capture_output=True, text=True)
    sites = [(l["label"], s["file"], s["span"]["line"]) for l in json.loads(report.read_text())["labels"]
             if l["classifier"] == query for s in l["sites"]]
    hit = next((s for s in sites if label in s[0] and file in s[1]), None)
    failed += hit is None
    where = f"{hit[0][:60]} ({Path(hit[1]).name}:{hit[2]})" if hit else ""
    print(f"{bug:8} {query:20} {'finds it' if hit else 'MISSES IT'}  {where}")
sys.exit(1 if failed else 0)
