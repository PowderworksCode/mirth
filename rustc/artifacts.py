"""What a Cargo build produced, in a form two builds can be compared in.

Shared by fuzz.py and replay.py. From the JSON messages of `cargo build
--message-format=json-render-diagnostics` it collects, for packages built
from a path:

  rmeta   every .rmeta, by path
  rlib    every .rlib's members by name, with the incremental session suffix
          of codegen-unit object names removed (`<crate>.<cgu>.<session>.rcgu.o`),
          since it differs between sessions while the objects are identical
  exe     every executable
  diag    every diagnostic, rendered, counted per crate
"""

import hashlib
import json
import re
from collections import Counter
from pathlib import Path

SESSION = re.compile(rb"\.[0-9a-z]{7}(\.rcgu\.(?:o|dwo))")


def ar_members(data):
    """The members of a Unix ar archive (GNU format), as {name: bytes}."""
    if not data.startswith(b"!<arch>\n"):
        return {"<not an archive>": data}
    members, names, pos = {}, b"", 8
    while pos + 60 <= len(data):
        header = data[pos:pos + 60]
        name = header[:16].rstrip()
        size = int(header[48:58].strip() or 0)
        body = data[pos + 60:pos + 60 + size]
        pos += 60 + size + (size & 1)
        if name == b"//":
            names = body
            continue
        if name in (b"/", b"/SYM64/"):
            continue
        if name.startswith(b"/") and name[1:].isdigit():
            offset = int(name[1:])
            name = names[offset:names.index(b"/\n", offset)]
        name = name.rstrip(b"/")
        members[SESSION.sub(rb"\1", name).decode("utf-8", "replace")] = body
    return members


def normalized_rlib(path):
    out = {}
    for name, body in ar_members(Path(path).read_bytes()).items():
        # The link metadata and, with split debuginfo, the objects name session-suffixed files.
        body = SESSION.sub(rb"\1", body)
        out[name] = hashlib.sha256(body).hexdigest()
    return out


def collect(stdout, target):
    """Artifacts from cargo's JSON messages, keyed by path relative to `target`."""
    target = Path(target)
    found = {"rmeta": {}, "rlib": {}, "exe": {}, "diag": Counter()}
    for line in stdout.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if "path+file" not in msg.get("package_id", ""):
            continue
        if msg.get("reason") == "compiler-message":
            m = msg.get("message", {})
            text = m.get("rendered") or m.get("message") or ""
            found["diag"][(msg["target"]["name"], text)] += 1
            continue
        if msg.get("reason") != "compiler-artifact":
            continue
        for f in msg.get("filenames", []):
            rel = str(Path(f).relative_to(target)) if f.startswith(str(target)) else f
            if f.endswith(".rmeta"):
                found["rmeta"][rel] = hashlib.sha256(Path(f).read_bytes()).hexdigest()
            elif f.endswith(".rlib"):
                found["rlib"][rel] = normalized_rlib(f)
        if msg.get("executable"):
            f = msg["executable"]
            rel = str(Path(f).relative_to(target)) if f.startswith(str(target)) else f
            found["exe"][rel] = hashlib.sha256(SESSION.sub(rb"\1", Path(f).read_bytes())).hexdigest()
    return found


def compare(a, b):
    """{kind: [what differs]} for the kinds that differ between two collections."""
    out = {}
    for kind in ("rmeta", "exe"):
        diff = sorted(k for k in set(a[kind]) | set(b[kind]) if a[kind].get(k) != b[kind].get(k))
        if diff:
            out[kind] = diff
    diff = []
    for rel in sorted(set(a["rlib"]) | set(b["rlib"])):
        ma, mb = a["rlib"].get(rel, {}), b["rlib"].get(rel, {})
        members = sorted(m for m in set(ma) | set(mb) if ma.get(m) != mb.get(m))
        if members:
            diff.append(f"{rel}: {', '.join(members[:5])}{' …' if len(members) > 5 else ''}")
    if diff:
        out["rlib"] = diff
    if a["diag"] != b["diag"]:
        only_a = a["diag"] - b["diag"]
        only_b = b["diag"] - a["diag"]
        out["diag"] = [f"{c} only in the first: {t[:200]!r}" for (c, t), n in only_a.items()] + \
                      [f"{c} only in the second: {t[:200]!r}" for (c, t), n in only_b.items()]
    return out
