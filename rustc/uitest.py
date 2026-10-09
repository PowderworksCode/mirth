"""What the oracle scripts need from rustc's UI tests: their `//@` headers, which of them can be
compiled on their own on this host, and a way to build and run one.

The header reading is the same as in ui-fuzz.py, ui-coverage.py and ui-solver-diff.py (the first
revision of a test with revisions).
"""

import os
import re
import subprocess
from pathlib import Path

# Tests that need more than one file, another target, or a tool this host may lack.
NOT_STANDALONE = re.compile(
    r"^//@\s*(aux-build|aux-crate|aux-bin|aux-codegen-backend|proc-macro|add-minicore|"
    r"needs-llvm-components|needs-sanitizer|needs-profiler|needs-rust-lld|needs-enzyme|"
    r"only-(?!x86_64|linux|unix|64bit|elf|gnu)|ignore-x86_64|ignore-linux|ignore-unix|ignore-64bit|"
    r"known-bug|rustc-env|unset-rustc-env)", re.M)


def headers(text):
    """(flags, edition, kind, revision) of a test, for its first revision."""
    flags, edition, revision, kind = [], None, None, None
    revs = re.search(r"^//@\s*revisions:\s*(.*)$", text, re.M)
    if revs:
        revision = revs.group(1).split()[0]
    for m in re.finditer(r"^//@(?:\[([\w,-]+)\])?\s*([a-z-]+)(?::\s*(.*))?$", text, re.M):
        only, key, value = m.group(1), m.group(2), (m.group(3) or "").strip()
        if only and (revision is None or revision not in only.split(",")):
            continue
        if key == "compile-flags":
            flags += value.split()
        elif key == "edition":
            edition = value.split()[0]
        elif key in ("check-pass", "build-pass", "run-pass", "check-fail", "build-fail", "run-fail"):
            kind = key
    if revision:
        flags += ["--cfg", revision]
    return flags, edition, kind, revision


def tests(root, kinds, extra_skip=None):
    """The standalone tests under `root` of the given kinds, as (path, flags, edition, kind)."""
    root = Path(root)
    for path in sorted(root.rglob("*.rs")):
        if "auxiliary" in path.parts:
            continue
        text = path.read_text(errors="replace")
        if NOT_STANDALONE.search(text):
            continue
        flags, edition, kind, _ = headers(text)
        if kind not in kinds:
            continue
        if extra_skip and extra_skip(text, flags):
            continue
        yield path, flags, edition, kind


def is_ice(stderr):
    return ("internal compiler error" in stderr or "the compiler unexpectedly panicked" in stderr
            or "rustc interrupted by SIG" in stderr)


def compile(rustc, source, out, flags, edition, extra=(), timeout=300, emit="link"):
    """Build `source` into the directory `out`; returns (status, stderr, binary).
    status: ok, error, ice, timeout."""
    out.mkdir(parents=True, exist_ok=True)
    binary = out / "prog"
    argv = [str(rustc), str(source), "--edition", edition or "2015", f"--emit={emit}", "-o", str(binary),
            "-Zunstable-options", "-Ainternal_features", "-Aincomplete_features", "--error-format=short",
            *flags, *extra]
    try:
        r = subprocess.run(argv, capture_output=True, text=True, timeout=timeout, cwd=out,
                           env=dict(os.environ, RUSTC_BOOTSTRAP="1", RUST_BACKTRACE="0"))
    except subprocess.TimeoutExpired:
        return "timeout", "", None
    if is_ice(r.stderr):
        return "ice", r.stderr, None
    return ("ok" if r.returncode == 0 else "error"), r.stderr, binary if r.returncode == 0 else None


def run(binary, timeout=20):
    """Run a built program; (exit, stdout, stderr), exit a number, or 'signal N' or 'timeout'."""
    try:
        r = subprocess.run([str(binary)], capture_output=True, timeout=timeout, cwd=binary.parent,
                           stdin=subprocess.DEVNULL, env=dict(os.environ, RUST_BACKTRACE="0"))
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""
    code = r.returncode if r.returncode >= 0 else f"signal {-r.returncode}"
    return code, r.stdout, r.stderr


MIRI_TOOLCHAIN = Path.home() / ".rustup/toolchains/nightly-2026-10-06-x86_64-unknown-linux-gnu"
MIRI_SYSROOT = Path.home() / ".cache/miri"


def miri(source, flags, edition, extra=(), timeout=120, cwd=None):
    """Interpret `source` with Miri (the pinned nightly's, sysroot from `cargo miri setup`).
    Returns {status, exit, stdout, stderr}; status: ok (ran to the end, any exit code), ub,
    unsupported, error (did not compile), timeout."""
    argv = [str(MIRI_TOOLCHAIN / "bin/miri"), "--sysroot", str(MIRI_SYSROOT), str(source),
            "--edition", edition or "2015", "-Zunstable-options", "-Ainternal_features",
            "-Aincomplete_features", "-Zmiri-disable-isolation", "-Zmiri-deterministic-floats", *flags, *extra]
    try:
        r = subprocess.run(argv, capture_output=True, timeout=timeout, cwd=cwd, stdin=subprocess.DEVNULL,
                           env=dict(os.environ, RUSTC_BOOTSTRAP="1", RUST_BACKTRACE="0"))
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "exit": None, "stdout": "", "stderr": ""}
    err = r.stderr.decode(errors="replace")
    if "Undefined Behavior:" in err:
        status = "ub"
    elif "unsupported operation" in err or "can't call foreign function" in err:
        status = "unsupported"
    elif is_ice(err):
        status = "ice"
    elif re.search(r"^error(\[E\d+\])?: ", err, re.M) and r.returncode == 1 and "panicked" not in err:
        status = "error"
    else:
        status = "ok"
    return {"status": status, "exit": r.returncode, "stdout": r.stdout.decode(errors="replace"), "stderr": err}


def drive(todo, one, results, jobs, pause_on_finding):
    """Run `one(*item)` for each item on `jobs` threads; each returns (record, findings). Records
    go to the `results` file as JSON lines. With `pause_on_finding`, stops starting new items at
    the first finding (the frontier loop) and returns 3; otherwise 0."""
    import json
    import sys
    from concurrent.futures import ThreadPoolExecutor, FIRST_COMPLETED, wait
    it = iter(todo)
    findings = done = 0
    stop = False
    with ThreadPoolExecutor(jobs) as ex, open(results, "a") as out:
        pending = set()

        def submit():
            nxt = None if stop else next(it, None)
            if nxt is not None:
                pending.add(ex.submit(one, *nxt))
        for _ in range(jobs * 2):
            submit()
        while pending:
            finished, _ = wait(pending, return_when=FIRST_COMPLETED)
            for fut in finished:
                pending.discard(fut)
                try:
                    record, found = fut.result()
                except Exception as error:  # a harness bug must not stop the run
                    record, found = {"test": "?", "harness": repr(error)[:300]}, []
                out.write(json.dumps(record) + "\n")
                out.flush()
                done += 1
                if found:
                    findings += 1
                    print(f"FINDING {record.get('test')}: {record.get('found')}", flush=True)
                    stop = stop or pause_on_finding
                if done % 100 == 0:
                    print(f"{done}/{len(todo)} done, {findings} with findings", flush=True)
                submit()
    print(f"{done} tests, {findings} with findings", flush=True)
    return 3 if findings and pause_on_finding else 0
