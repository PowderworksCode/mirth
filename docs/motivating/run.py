#!/usr/bin/env python3
"""Run the reproductions in bugs.json on the toolchain from before each fix and the
one after it, and save both outputs.

    docs/motivating/run.py [issue…]

bugs.json is a list of {issue, before_toolchain, after_toolchain, files: [{path,
content}], commands, observe, ...}; `commands` uses the word TOOLCHAIN where the
toolchain goes. Outputs go to out/<issue>/{before,after}.txt.
"""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

here = Path(__file__).resolve().parent
bugs = json.loads((here / "bugs.json").read_text())
only = {int(a) for a in sys.argv[1:]}


def install(toolchain):
    r = subprocess.run(["rustup", "toolchain", "install", toolchain, "--profile", "minimal"],
                       capture_output=True, text=True)
    return r.returncode == 0


for bug in bugs:
    if only and bug["issue"] not in only:
        continue
    out = here / "out" / str(bug["issue"])
    out.mkdir(parents=True, exist_ok=True)
    for side in ("before", "after"):
        toolchain = bug[f"{side}_toolchain"]
        if not install(toolchain):
            (out / f"{side}.txt").write_text(f"could not install {toolchain}\n")
            continue
        with tempfile.TemporaryDirectory() as d:
            for f in bug["files"]:
                path = Path(d) / f["path"]
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(f["content"])
            env = dict(os.environ, RUSTC_WRAPPER="", CARGO_TERM_COLOR="never")
            script = bug["commands"].replace("TOOLCHAIN", toolchain)
            try:
                r = subprocess.run(["bash", "-c", script], cwd=d, env=env, capture_output=True,
                                   text=True, timeout=900)
                text = f"$ toolchain {toolchain}\n{r.stdout}\n--- stderr ---\n{r.stderr}\nexit {r.returncode}\n"
            except subprocess.TimeoutExpired:
                text = f"$ toolchain {toolchain}\ntimed out\n"
        (out / f"{side}.txt").write_text(text)
    print(f"#{bug['issue']}: done", flush=True)
