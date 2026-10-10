#!/usr/bin/env python3
"""Scaling and budgets: compile time, memory, future sizes and stack frames must grow at most
about linearly with the size of a program of a fixed shape.

Each generator writes a program of size N for N in a doubling series. For each N the check records
the compiler's user CPU time and peak memory (wait4), and for some shapes a size the program itself
reports (`size_of_val` of a future) or the largest stack frame in the assembly. It fits the growth
exponent k in value ~ N^k from the largest sizes (log-log slope) and reports:

  superlinear  k above --max-exponent (default 1.6) for time or memory, or above 1.3 for a size
  timeout      a compile over --timeout seconds

    rustc/scale-check.py --rustc <rustc> --work <dir> [--only fields,enum] [--opt 0,2]
        [--sizes 50,100,200,400,800] [--timeout 300] [--max-exponent 1.6] [--jobs 4]
"""

import argparse
import json
import math
import os
import re
import subprocess
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--rustc", required=True)
p.add_argument("--work", required=True)
p.add_argument("--only")
p.add_argument("--opt", default="0,2")
p.add_argument("--sizes", default="50,100,200,400,800")
p.add_argument("--timeout", type=int, default=300)
p.add_argument("--max-exponent", type=float, default=1.6)
p.add_argument("--jobs", type=int, default=4)
args = p.parse_args()
WORK = Path(args.work).resolve()
WORK.mkdir(parents=True, exist_ok=True)


def g_fields(n):
    fields = "\n".join(f"    pub f{i}: u{8 << (i % 4)}," for i in range(n))
    return f"#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]\npub struct S {{\n{fields}\n}}\nfn main() {{ let s = S::default(); println!(\"{{}}\", format!(\"{{:?}}\", s.clone()).len()); }}\n"


def g_enum(n):
    variants = "\n".join(f"    V{i}(u32)," for i in range(n))
    arms = "\n".join(f"        E::V{i}(x) => x + {i}," for i in range(n))
    return f"#[derive(Debug, Clone, PartialEq)]\npub enum E {{\n{variants}\n}}\npub fn f(e: &E) -> u32 {{\n    match *e {{\n{arms}\n    }}\n}}\nfn main() {{ println!(\"{{}}\", f(&E::V0(1))); }}\n"


def g_nested_generic(n):
    ty = "u8"
    for _ in range(n):
        ty = f"W<{ty}>"
    return ("#[derive(Clone, Debug, Default)] pub struct W<T>(T);\n"
            "pub trait T { fn t(&self) -> usize; }\nimpl T for u8 { fn t(&self) -> usize { 1 } }\n"
            "impl<X: T> T for W<X> { fn t(&self) -> usize { self.0.t() + 1 } }\n"
            f"fn main() {{ let v: {ty} = Default::default(); println!(\"{{}}\", v.t()); }}\n")


def g_iter_chain(n):
    chain = "".join(f".map(|x| x.wrapping_add({i}))" for i in range(n))
    return f"fn main() {{ let s: u64 = (0u64..10){chain}.sum(); println!(\"{{}}\", s); }}\n"


def g_async_forward(n):
    fns = ["async fn f0(x: [u8; 64]) -> u8 { x[0] }"]
    for i in range(1, n):
        fns.append(f"async fn f{i}(x: [u8; 64]) -> u8 {{ f{i - 1}(x).await }}")
    return ("\n".join(fns) + f"\nfn main() {{ let fut = f{n - 1}([1; 64]); "
            "println!(\"SIZE {}\", std::mem::size_of_val(&fut)); }\n")


def g_seq_calls(n):
    calls = "\n".join(f"    let a{i} = big({i}); acc ^= a{i}[{i % 512}];" for i in range(n))
    return ("#[inline(never)] fn big(x: u64) -> [u64; 512] { [x; 512] }\n"
            f"#[inline(never)] pub fn many() -> u64 {{\n    let mut acc = 0u64;\n{calls}\n    acc\n}}\n"
            "fn main() { println!(\"{}\", many()); }\n")


def g_trait_impls(n):
    impls = "\n".join(f"pub struct S{i}; impl Tr for S{i} {{ fn v(&self) -> u32 {{ {i} }} }}" for i in range(n))
    uses = " + ".join(f"S{i}.v()" for i in range(n))
    return f"pub trait Tr {{ fn v(&self) -> u32; }}\n{impls}\nfn main() {{ println!(\"{{}}\", {uses}); }}\n"


def g_nested_expr(n):
    expr = "1u64"
    for i in range(n):
        expr = f"({expr} + {i % 7})"
    return f"fn main() {{ let x = std::hint::black_box({expr}); println!(\"{{}}\", x); }}\n"


GENERATORS = {
    "fields": (g_fields, None), "enum": (g_enum, None), "nested-generic": (g_nested_generic, None),
    "iter-chain": (g_iter_chain, None), "async-forward": (g_async_forward, "run-size"),
    "seq-calls": (g_seq_calls, "frame"), "trait-impls": (g_trait_impls, None), "nested-expr": (g_nested_expr, None),
}
# Shapes where a size of N cannot be compiled meaningfully past a bound (recursion limits).
CAP = {"nested-generic": 120, "nested-expr": 400, "async-forward": 200}


def measure(source, opt):
    with tempfile.TemporaryDirectory(dir=WORK) as d:
        d = Path(d)
        (d / "m.rs").write_text(source)
        argv = [args.rustc, "m.rs", "--edition", "2021", f"-Copt-level={opt}", "-o", "m", "--emit=link,asm"]
        start = time.time()
        proc = subprocess.Popen(argv, cwd=d, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        deadline = start + args.timeout
        while True:
            pid, status, usage = os.wait4(proc.pid, os.WNOHANG)
            if pid:
                break
            if time.time() > deadline:
                proc.kill()
                os.wait4(proc.pid, 0)
                return {"timeout": True}
            time.sleep(0.05)
        err = proc.stderr.read().decode(errors="replace")
        if os.waitstatus_to_exitcode(status) != 0:
            return {"error": err[-500:]}
        out = {"user": usage.ru_utime, "rss_kb": usage.ru_maxrss, "wall": time.time() - start}
        asm = (d / "m.s").read_text(errors="replace")
        frames = [int(x, 16) if x.startswith("0x") else int(x)
                  for x in re.findall(r"sub[q]?\s+\$(0x[0-9a-f]+|\d+),\s*%rsp", asm)]
        out["max_frame"] = max(frames) if frames else 0
        try:
            run = subprocess.run([str(d / "m")], capture_output=True, text=True, timeout=30)
            m = re.search(r"SIZE (\d+)", run.stdout)
            if m:
                out["run_size"] = int(m.group(1))
        except subprocess.TimeoutExpired:
            pass
        return out


def exponent(ns, values):
    pts = [(math.log(n), math.log(v)) for n, v in zip(ns, values) if v and v > 0]
    if len(pts) < 3:
        return None
    pts = pts[-3:]  # the largest sizes: constant overheads dominate the small ones
    mx = sum(x for x, _ in pts) / len(pts)
    my = sum(y for _, y in pts) / len(pts)
    den = sum((x - mx) ** 2 for x, _ in pts)
    return sum((x - mx) * (y - my) for x, y in pts) / den if den else None


def one(job):
    name, opt = job
    gen, extra = GENERATORS[name]
    sizes = [n for n in map(int, args.sizes.split(",")) if n <= CAP.get(name, 10**9)]
    rows = []
    for n in sizes:
        r = measure(gen(n), opt)
        r["n"] = n
        rows.append(r)
        if r.get("timeout") or r.get("error"):
            break
    ok = [r for r in rows if "user" in r]
    ns = [r["n"] for r in ok]
    result = {"shape": name, "opt": opt, "rows": rows,
              "k_time": exponent(ns, [max(r["user"] - ok[0]["user"] * 0.5, 1e-3) for r in ok]) if ok else None,
              "k_rss": exponent(ns, [r["rss_kb"] for r in ok]),
              "k_frame": exponent(ns, [r["max_frame"] for r in ok]) if extra == "frame" else None,
              "k_size": exponent(ns, [r.get("run_size", 0) for r in ok]) if extra == "run-size" else None}
    found = []
    if any(r.get("timeout") for r in rows):
        found.append(f"timeout at N={rows[-1]['n']}")
    if any(r.get("error") for r in rows):
        found.append(f"error at N={rows[-1]['n']}: {rows[-1]['error'][-200:]}")
    for key, limit in (("k_time", args.max_exponent), ("k_rss", args.max_exponent), ("k_frame", 1.3), ("k_size", 1.3)):
        if result[key] is not None and result[key] > limit:
            found.append(f"{key} = {result[key]:.2f}")
    result["found"] = found
    return result


def main():
    names = args.only.split(",") if args.only else list(GENERATORS)
    jobs = [(n, int(o)) for n in names for o in args.opt.split(",")]
    with ThreadPoolExecutor(args.jobs) as ex:
        results = list(ex.map(one, jobs))
    (WORK / "results.json").write_text(json.dumps(results, indent=1))
    for r in results:
        last = next((x for x in reversed(r["rows"]) if "user" in x), {})
        ks = " ".join(f"{k}={r[k]:.2f}" for k in ("k_time", "k_rss", "k_frame", "k_size") if r[k] is not None)
        print(f"{r['shape']:15} O{r['opt']}  N<={last.get('n')}  user {last.get('user', 0):.1f}s  "
              f"rss {last.get('rss_kb', 0) // 1024}MB  {ks}  {'; '.join(r['found'])}")


main()
