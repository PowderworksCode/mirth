#!/usr/bin/env python3
"""Make random edits to a fixture and check each incremental rebuild.

Each worker keeps one copy of the fixture and repeats:

  1. apply a random mechanical edit (a comment, a moved item, a changed
     literal, a new function, ...);
  2. rebuild incrementally; if the edit does not compile, revert it (the next
     build then also exercises recovery from a failed session);
  3. build the same source from scratch, at the same path;
  4. compare:
       P6     every .rmeta Cargo reports for a workspace member
       rlib   every rlib's members, object code included (artifacts.py)
       exe    the binary's bytes
       diag   the diagnostics each crate printed
       run    the binaries' output and exit status
       reuse  the compiler's own check of what it reused (RUSTC_VERIFY_REUSE,
              docs/hunt/verify-reuse.patch) found nothing stale
       P5     when anything differs, a second clean build is made; if the two
              clean builds differ, that is reported instead (nondeterminism)
       ICE    neither build crashed the compiler
       split  both builds succeed or both fail

Every --reset edits the worker starts again from the pristine fixture. A
finding keeps the diffs since the last reset, which replay it, and both
builds' differing files and logs.

    rustc/fuzz.py --rustc <rustc> --fixture fixtures/sink --work <dir> [--workers 8] [--edits N]

Stop it early by creating <work>/STOP. Progress is in <work>/stats.json.
"""

import argparse
import difflib
import json
import multiprocessing
import os
import random
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import artifacts  # noqa: E402

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--fixture", required=True)
p.add_argument("--work", required=True)
p.add_argument("--workers", type=int, default=8)
p.add_argument("--edits", type=int, default=10**9, help="per worker")
p.add_argument("--reset", type=int, default=40)
p.add_argument("--keep", type=int, default=5, help="findings kept per kind")
p.add_argument("--toolchain", default="nightly-2026-10-06")
p.add_argument("--rustflags", default="-Zincremental-verify-ich")
p.add_argument("--seed", type=int, default=0)
p.add_argument("--timeout", type=int, default=180, help="seconds before a build counts as hung")
p.add_argument("--p5-builds", type=int, default=1,
               help="clean builds made again when anything differs, to tell nondeterminism from P6")
p.add_argument("--check", action="store_true",
               help="cargo check instead of cargo build: metadata only, no code or binaries")
p.add_argument("--no-verify-reuse", action="store_true",
               help="do not set RUSTC_VERIFY_REUSE (needs a compiler with docs/hunt/verify-reuse.patch)")
args = p.parse_args()

WORK = Path(args.work).resolve()
FIXTURE = Path(args.fixture).resolve()
BIN = FIXTURE.name

# ---------------------------------------------------------------- edits


def blocks(text):
    """Top-level items: blank-line separated, continuation blocks merged."""
    out = []
    for b in text.split("\n\n"):
        if out and (b[:1].isspace() or b.startswith("}") or b.startswith("where")):
            out[-1] += "\n\n" + b
        else:
            out.append(b)
    return out


def edit_lines(text, rng, f):
    lines = text.split("\n")
    r = f(lines, rng)
    return None if r is None else "\n".join(r)


def comment_line(text, rng, n):
    def f(lines, rng):
        i = rng.randrange(len(lines) + 1)
        indent = re.match(r"\s*", lines[i] if i < len(lines) else "").group(0)
        return lines[:i] + [f"{indent}// fuzz {n}"] + lines[i:]
    return edit_lines(text, rng, f)


def blank_line(text, rng, n):
    return edit_lines(text, rng, lambda lines, rng: (lambda i: lines[:i] + [""] + lines[i:])(rng.randrange(len(lines) + 1)))


def remove_comment(text, rng, n):
    def f(lines, rng):
        idx = [i for i, l in enumerate(lines) if l.strip().startswith("//") and not l.strip().startswith("//!")]
        if not idx:
            return None
        i = rng.choice(idx)
        return lines[:i] + lines[i + 1:]
    return edit_lines(text, rng, f)


def indent_line(text, rng, n):
    def f(lines, rng):
        idx = [i for i, l in enumerate(lines) if l.strip()]
        i = rng.choice(idx)
        lines[i] = "    " + lines[i]
        return lines
    return edit_lines(text, rng, f)


def swap_items(text, rng, n):
    b = blocks(text)
    if len(b) < 3:
        return None
    i = rng.randrange(1, len(b) - 1)
    b[i], b[i + 1] = b[i + 1], b[i]
    return "\n\n".join(b)


def move_item_to_end(text, rng, n):
    b = blocks(text)
    if len(b) < 3:
        return None
    i = rng.randrange(1, len(b))
    item = b.pop(i)
    return "\n\n".join(b + [item.rstrip("\n")]) + "\n"


def delete_item(text, rng, n):
    b = blocks(text)
    if len(b) < 3:
        return None
    b.pop(rng.randrange(1, len(b)))
    return "\n\n".join(b)


def duplicate_fn(text, rng, n):
    b = blocks(text)
    fns = [i for i, x in enumerate(b) if re.search(r"^(pub(\([^)]*\))? )?(const )?(async )?fn \w+", x, re.M)]
    if not fns:
        return None
    i = rng.choice(fns)
    copy = re.sub(r"\bfn (\w+)", lambda m: f"fn {m.group(1)}_fuzz{n}", b[i], count=1)
    b.insert(i + 1, copy)
    return "\n\n".join(b)


ADDITIONS = [
    "fn fuzz_private_{n}() -> u32 {{ {n} }}",
    "pub fn fuzz_public_{n}(x: u32) -> u32 {{ x.wrapping_mul({n}) }}",
    "#[inline]\npub fn fuzz_inline_{n}<T: Clone>(x: &T) -> (T, u32) {{ (x.clone(), {n}) }}",
    "pub const FUZZ_{n}: &str = \"fuzz {n}\";",
    "pub static FUZZ_STATIC_{n}: [u8; 3] = [{n} as u8, 1, 2];",
    "#[derive(Debug, Clone, PartialEq)]\npub struct Fuzz{n}<T, const N: usize> {{ pub items: [T; N], pub tag: &'static str }}",
    "pub enum FuzzEnum{n} {{ A(u32), B {{ x: i64 }}, C }}",
    "pub trait FuzzTrait{n} {{ fn go(&self) -> impl Sized; const K: u32 = {n}; }}",
    "pub async fn fuzz_async_{n}() -> u32 {{ {n} }}",
    "pub type FuzzAlias{n}<T> = Vec<(T, u32)>;",
    "macro_rules! fuzz_macro_{n} {{ ($e:expr) => {{ $e + {n} }}; }}",
    "pub mod fuzz_mod_{n} {{ pub fn inner() -> &'static str {{ \"{n}\" }} }}",
]


def add_item(text, rng, n):
    b = blocks(text)
    i = rng.randrange(1, len(b) + 1)
    b.insert(i, rng.choice(ADDITIONS).format(n=n))
    return "\n\n".join(b)


def int_literal(text, rng, n):
    ms = [m for m in re.finditer(r"(?<![\w.])(\d+)(?![\w.])", text)]
    if not ms:
        return None
    m = rng.choice(ms)
    return text[: m.start()] + str(int(m.group(1)) + 1) + text[m.end():]


def str_literal(text, rng, n):
    ms = [m for m in re.finditer(r'(?<![\w\\])"([^"\\\n]*)"', text)]
    if not ms:
        return None
    m = rng.choice(ms)
    return text[: m.start()] + '"' + m.group(1) + "~" + '"' + text[m.end():]


def toggle_inline(text, rng, n):
    lines = text.split("\n")
    inl = [i for i, l in enumerate(lines) if l.strip() in ("#[inline]", "#[inline(never)]", "#[inline(always)]")]
    fns = [i for i, l in enumerate(lines) if re.match(r"\s*(pub(\([^)]*\))? )?(const )?fn ", l)]
    if inl and rng.random() < 0.5:
        lines.pop(rng.choice(inl))
    elif fns:
        i = rng.choice(fns)
        indent = re.match(r"\s*", lines[i]).group(0)
        lines.insert(i, indent + rng.choice(["#[inline]", "#[inline(never)]", "#[cold]", "#[must_use]"]))
    else:
        return None
    return "\n".join(lines)


def doc_comment(text, rng, n):
    lines = text.split("\n")
    idx = [i for i, l in enumerate(lines) if re.match(r"\s*(pub(\([^)]*\))? )?(fn|struct|enum|trait|const|static|type|mod) ", l)]
    if not idx:
        return None
    i = rng.choice(idx)
    indent = re.match(r"\s*", lines[i]).group(0)
    lines.insert(i, f"{indent}/// Fuzz doc {n}, see [`Vec`].")
    return "\n".join(lines)


def reorder_derive(text, rng, n):
    ms = list(re.finditer(r"#\[derive\(([^)]*)\)\]", text))
    if not ms:
        return None
    m = rng.choice(ms)
    names = [x.strip() for x in m.group(1).split(",") if x.strip()]
    if len(names) < 2:
        return None
    rng.shuffle(names)
    return text[: m.start()] + "#[derive(" + ", ".join(names) + ")]" + text[m.end():]


def narrow_visibility(text, rng, n):
    ms = list(re.finditer(r"\bpub (fn|struct|enum|const|static|trait|mod|type) ", text))
    if not ms:
        return None
    m = rng.choice(ms)
    return text[: m.start()] + "pub(crate) " + m.group(1) + " " + text[m.end():]


def rename_local(text, rng, n):
    ms = list(re.finditer(r"\blet (mut )?([a-z_][a-z0-9_]*)\b", text))
    if not ms:
        return None
    m = rng.choice(ms)
    name = m.group(2)
    if name == "_":
        return None
    # Rename from the binding to the end of the enclosing top-level block.
    end = text.find("\n}\n", m.end())
    end = len(text) if end < 0 else end
    body = re.sub(rf"\b{name}\b", f"{name}_f{n}", text[m.start():end])
    return text[: m.start()] + body + text[end:]


EDITS = [
    (comment_line, 10), (blank_line, 6), (remove_comment, 3), (indent_line, 4),
    (swap_items, 6), (move_item_to_end, 3), (delete_item, 2), (duplicate_fn, 4),
    (add_item, 8), (int_literal, 6), (str_literal, 4), (toggle_inline, 4),
    (doc_comment, 4), (reorder_derive, 2), (narrow_visibility, 2), (rename_local, 3),
]

# ---------------------------------------------------------------- building


def env():
    e = dict(os.environ)
    e.update(RUSTC=args.rustc, RUSTC_WRAPPER="", CARGO_INCREMENTAL="1",
             RUSTFLAGS=args.rustflags, CARGO_TERM_COLOR="never")
    if not args.no_verify_reuse:
        e["RUSTC_VERIFY_REUSE"] = "1"
    return e


class Result:
    def __init__(self, returncode, stdout, stderr):
        self.returncode, self.stdout, self.stderr = returncode, stdout, stderr


def build(src, target):
    """Build; a build running longer than --timeout is killed and reported as a hang
    if a rustc was still running (a looping build script is the fixture's problem)."""
    t = time.time()
    proc = subprocess.Popen(
        ["cargo", f"+{args.toolchain}", "check" if args.check else "build", "--workspace", "--offline", "-j", "4",
         "--target-dir", str(target), "--message-format=json-render-diagnostics"],
        cwd=src, env=env(), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        start_new_session=True)
    hang = False
    try:
        out, err = proc.communicate(timeout=args.timeout)
    except subprocess.TimeoutExpired:
        group = subprocess.run(["ps", "-o", "args=", "-g", str(proc.pid)], capture_output=True, text=True).stdout
        hang = any(line.split()[0].endswith("/rustc") for line in group.splitlines() if line.strip())
        os.killpg(proc.pid, 9)
        out, err = proc.communicate()
        err += f"\nkilled after {args.timeout}s; still running:\n{group}"
    r = Result(proc.returncode if not hang else 1, out, err)
    rmetas, exe = {}, None
    for line in r.stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("reason") != "compiler-artifact":
            continue
        for f in msg.get("filenames", []):
            if f.endswith(".rmeta"):
                rmetas[str(Path(f).relative_to(target))] = Path(f).read_bytes()
            elif f.endswith(".so") and "proc-macro" in msg["target"]["kind"]:
                # A proc macro's metadata is in the .rustc section of its shared library.
                section = subprocess.run(["objcopy", "--dump-section", ".rustc=/dev/stdout", f, "/dev/null"],
                                         capture_output=True).stdout
                rmetas[str(Path(f).relative_to(target)) + ":.rustc"] = section
        if msg.get("executable") and msg["target"]["name"] == BIN:
            exe = msg["executable"]
    log = r.stderr
    return {
        "ok": r.returncode == 0,
        "ice": "internal compiler error" in log or "the compiler unexpectedly panicked" in log,
        "hang": hang,
        "reuse": reuse_checks(log),
        "secs": time.time() - t, "log": log, "rmetas": rmetas, "exe": exe,
        "art": artifacts.collect(r.stdout, target),
    }


def reuse_checks(log):
    """What the compiler's own check of reused results (docs/hunt/verify-reuse.patch) found
    stale: `query <name>`, `metadata`, `codegen unit` or `allocation sharing <queries>`,
    once each."""
    found = set()
    for line in log.splitlines():
        if line.startswith("rustc-verify-reuse: query `"):
            found.add("query " + line.split("`")[1])
        elif line.startswith("rustc-verify-reuse: metadata"):
            found.add("metadata")
        elif line.startswith("rustc-verify-reuse: codegen unit"):
            found.add("codegen unit")
        elif line.startswith("rustc-verify-reuse: allocation shared differently"):
            # Named by the two queries and typing modes, so that a new pattern is kept apart
            # from a known one.
            queries = re.findall(r"query `(\w+)`", line)
            modes = re.findall(r"TypingModeEqWrapper\((\w+)\)", line)
            found.add("allocation sharing " + " / ".join(queries + sorted(set(modes))))
    return sorted(found)


def run_exe(exe):
    if not exe:
        return None
    try:
        r = subprocess.run([exe], capture_output=True, text=True, timeout=30)
        return (r.returncode, r.stdout[-2000:])
    except subprocess.TimeoutExpired:
        return ("timeout", "")


def files(root):
    return {str(p.relative_to(root)): p.read_text() for p in root.rglob("*.rs") if "target" not in p.parts}


# ---------------------------------------------------------------- a worker


def worker(k):
    rng = random.Random(args.seed * 1000 + k)
    home = WORK / f"w{k}"
    src, target, inc_target = home / "src", home / "target", home / "target-inc"
    findings = WORK / "findings"
    findings.mkdir(parents=True, exist_ok=True)
    stats = {"edits": 0, "built": 0, "failed": 0, "compared": 0, "findings": {}, "secs": 0.0,
             "by_edit": {}}
    kept = {}
    history = []
    previous = {}

    def reset():
        if home.exists():
            shutil.rmtree(home)
        home.mkdir(parents=True)
        shutil.copytree(FIXTURE, src, ignore=shutil.ignore_patterns("target", "edits", "edit"))
        history.clear()
        previous.clear()
        b = build(src, target)
        previous.update(b["rmetas"])
        if not b["ok"]:
            # Raising here would hang the pool; record it and stop this worker.
            (WORK / f"error-w{k}.log").write_text(b["log"][-6000:])
            return False
        return True

    def report(kind, detail, inc, clean, extra=None):
        stats["findings"][kind] = stats["findings"].get(kind, 0) + 1
        key = kind + ":" + ",".join(sorted(detail))
        if kept.get(key, 0) >= args.keep:
            return
        kept[key] = kept.get(key, 0) + 1
        d = findings / f"w{k}-{stats['edits']:07}-{kind}"
        d.mkdir(parents=True, exist_ok=True)
        (d / "history.json").write_text(json.dumps(history, indent=1))
        (d / "finding.json").write_text(json.dumps({"kind": kind, "detail": detail, "extra": extra}, indent=1))
        (d / "inc.log").write_text(inc["log"][-8000:])
        (d / "clean.log").write_text(clean["log"][-8000:] if clean else "")
        for rel in detail:
            if rel in inc["rmetas"]:
                (d / (Path(rel).name + ".inc")).write_bytes(inc["rmetas"][rel])
            if clean and rel in clean["rmetas"]:
                (d / (Path(rel).name + ".clean")).write_bytes(clean["rmetas"][rel])
        shutil.make_archive(str(d / "src"), "gztar", src)

    if not reset():
        return stats
    started = time.time()
    while stats["edits"] < args.edits and not (WORK / "STOP").exists():
        if len([h for h in history if h["kept"]]) >= args.reset and not reset():
            break
        n = stats["edits"]
        stats["edits"] += 1
        paths = sorted(p for p in src.rglob("*.rs") if "target" not in p.parts)
        path = rng.choice(paths)
        old = path.read_text()
        fn = rng.choices([e for e, _ in EDITS], weights=[w for _, w in EDITS])[0]
        if fn in (str_literal, int_literal) and path.name == "build.rs":
            # Cargo keeps stale OUT_DIR files, so renaming a generated file splits the builds,
            # and a changed number can make the build script loop forever.
            continue
        new = fn(old, rng, n)
        if new is None or new == old:
            continue
        path.write_text(new)
        rel = str(path.relative_to(src))
        diff = "".join(difflib.unified_diff(old.splitlines(True), new.splitlines(True), "a/" + rel, "b/" + rel))
        inc = build(src, target)
        history.append({"edit": fn.__name__, "file": rel, "diff": diff, "before": old, "after": new, "kept": inc["ok"]})
        by = stats["by_edit"].setdefault(fn.__name__, [0, 0])
        by[0] += 1
        if inc["ice"]:
            report("ICE", [], inc, None)
        if inc["hang"]:
            report("hang", [], inc, None)
        if inc["reuse"]:
            lines = [l[:3000] for l in inc["log"].splitlines() if l.startswith("rustc-verify-reuse")]
            report("verify-reuse", inc["reuse"], inc, None, lines[:20])
        if not inc["ok"]:
            stats["failed"] += 1
            path.write_text(old)
            history.append({"edit": "revert", "file": rel, "diff": "", "kept": False})
            continue
        by[1] += 1
        stats["built"] += 1

        # The clean build, at the same path.
        if inc_target.exists():
            shutil.rmtree(inc_target)
        target.rename(inc_target)
        clean = build(src, target)
        if clean["ice"]:
            report("ICE", ["clean"], inc, clean)
        if clean["hang"]:
            report("hang", ["clean"], inc, clean)
        if not clean["ok"]:
            report("split", [], inc, clean)
        else:
            stats["compared"] += 1
            a, b = inc["rmetas"], clean["rmetas"]
            differ = sorted(r for r in set(a) | set(b) if a.get(r) != b.get(r))
            others = {k: v for k, v in artifacts.compare(inc["art"], clean["art"]).items() if k != "rmeta"}
            if differ or others:
                # Build clean once more: if two clean builds differ, the difference is
                # nondeterminism (P5), not incremental reuse.
                p5 = []
                for _ in range(args.p5_builds):
                    shutil.rmtree(target)
                    again = build(src, target)
                    c = again["rmetas"]
                    p5 = sorted(r for r in set(b) | set(c) if b.get(r) != c.get(r))
                    p5 += [f"{k}: {v[0]}" for k, v in artifacts.compare(clean["art"], again["art"]).items()
                           if k != "rmeta"]
                    if p5 or not again["ok"]:
                        break
                if again["ok"] and p5:
                    report("P5", p5[:10], clean, again)
                    differ, others = [], {}
            # Known: metadata reused unchanged from the previous session although a source
            # file changed (its hash and length in the source map are stale).
            stale = [r for r in differ if r in previous and previous[r] == a.get(r)]
            if differ and stale:
                stats["findings"]["P6-stale-reuse"] = stats["findings"].get("P6-stale-reuse", 0) + 1
            elif differ:
                report("P6", differ, inc, clean)
            # More oracles: object code in the rlibs, the binary, and the diagnostics.
            for kind, detail in others.items():
                report(kind, detail[:10], inc, clean)
            ra = run_exe(str(inc["exe"]).replace(str(target), str(inc_target)) if inc["exe"] else None)
            rb = run_exe(clean["exe"])
            if ra != rb:
                report("run", [], inc, clean, {"inc": ra, "clean": rb})
        shutil.rmtree(target)
        inc_target.rename(target)
        previous.clear()
        previous.update(inc["rmetas"])

        if stats["edits"] % 20 == 0:
            stats["secs"] = time.time() - started
            (WORK / f"stats-w{k}.json").write_text(json.dumps(stats))
    stats["secs"] = time.time() - started
    (WORK / f"stats-w{k}.json").write_text(json.dumps(stats))
    return stats


if __name__ == "__main__":
    WORK.mkdir(parents=True, exist_ok=True)
    with multiprocessing.Pool(args.workers) as pool:
        results = pool.map(worker, range(args.workers))
    total = {"edits": sum(r["edits"] for r in results), "built": sum(r["built"] for r in results),
             "compared": sum(r["compared"] for r in results)}
    print(json.dumps(total))
