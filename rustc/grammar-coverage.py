#!/usr/bin/env python3
"""Which parts of the Rust grammar a fixture's sources use, by Ur's grammar.

    rustc/grammar-coverage.py --ur <ur binary> --grammar <ur>/ecosystems/rust/language \\
        <fixture dir> [--json out.json] [--edition 2024]

Ur's Rust grammar (Urscal modules, `syntax Sort = Label: ... | Label: ... | Other ;`) names
each alternative of each syntax sort. `ur parse --tree` prints a file's tree with every node as
`(Sort::Label ...)` and every literal token in quotes. Two measures:

  alternatives  each labeled alternative, by construct: the label within its module, with
                Conditions.rsc (expressions in condition position) counted as Expressions
  literals      each keyword and operator the syntax rules mention, used or not

Prints what is missing, by module, and a summary. Lexical rules, layout and keyword lists are
not syntax alternatives and are left out.
"""

import argparse
import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--ur", required=True)
p.add_argument("--grammar", required=True)
p.add_argument("fixture")
p.add_argument("--json")
args = p.parse_args()


def strip_comments(text):
    return re.sub(r"//[^\n]*", "", text)


STRING = r'"(?:[^"\\]|\\.)*"'


def syntax_rules(text):
    """(sort, body) for each `syntax` rule of a module: from `syntax Name ... =` to the `;`
    that ends it, outside quotes and brackets."""
    for m in re.finditer(r"(?m)^syntax\s+(\w+)[^=]*=", text):
        i, depth = m.end(), 0
        while i < len(text):
            c = text[i]
            if c == '"':
                q = re.match(STRING, text[i:])
                i += q.end() if q else 1
                continue
            if c in "([{":
                depth += 1
            elif c in ")]}":
                depth -= 1
            elif c == ";" and depth == 0:
                break
            i += 1
        yield m.group(1), text[m.end():i]


# Conditions.rsc repeats the expression sorts for condition position (no struct literals): the
# same constructs, so counted with the expressions.
FAMILY = {"Conditions": "Expressions"}

grammar = {}  # (family, label) -> [sorts]
literals = {}  # literal -> family
for f in sorted(Path(args.grammar).glob("*.rsc")):
    if f.stem in ("Testing", "Semantics", "Language", "Rust"):
        continue
    family = FAMILY.get(f.stem, f.stem)
    text = strip_comments(f.read_text())
    for sort, body in syntax_rules(text):
        body = re.sub(r'@\w+=' + STRING, "", body)
        for lit in re.findall(STRING, body):
            lit = lit[1:-1].replace("\\<", "<").replace("\\>", ">").replace('\\"', '"').replace("\\\\", "\\")
            literals.setdefault(lit, family)
        bare = re.sub(STRING, '""', body)
        bare = re.sub(r'@\w+="[^"]*"', "", bare)
        for label in re.findall(r"(?<![\w:])([A-Z]\w*)\s*:(?!:)", bare):
            grammar.setdefault((family, label), []).append(sort)

sort_family = {}
for f in sorted(Path(args.grammar).glob("*.rsc")):
    for sort, _ in syntax_rules(strip_comments(f.read_text())):
        sort_family.setdefault(sort, FAMILY.get(f.stem, f.stem))

files = sorted(p for p in Path(args.fixture).rglob("*.rs") if "target" not in p.parts)
used_alts, used_lits = defaultdict(int), defaultdict(int)
failed = []
for f in files:
    r = subprocess.run([args.ur, "parse", "--tree", str(f)], capture_output=True, text=True)
    if r.returncode != 0 or "(File::" not in r.stdout:
        failed.append(str(f))
        continue
    for sort, label in re.findall(r"\((\w+)::(\w+)", r.stdout):
        used_alts[(sort_family.get(sort, sort), label)] += 1
    for lit in re.findall(r'(?<![\w:])"((?:[^"\\]|\\.)*)"', r.stdout):
        used_lits[lit.replace('\\"', '"').replace("\\\\", "\\")] += 1

missing_alts = sorted((fam, l, "/".join(sorted(set(sorts)))) for (fam, l), sorts in grammar.items()
                      if (fam, l) not in used_alts)
missing_lits = sorted((m, lit) for lit, m in literals.items() if lit not in used_lits)
by_module = defaultdict(list)
for fam, l, sorts in missing_alts:
    by_module[fam].append(f"{l} ({sorts})")
lit_by_module = defaultdict(list)
for m, lit in missing_lits:
    lit_by_module[m].append(lit)
for m in sorted(set(by_module) | set(lit_by_module)):
    print(f"== {m}")
    if by_module[m]:
        print("  alternatives: " + ", ".join(by_module[m]))
    if lit_by_module[m]:
        print("  literals: " + " ".join(repr(x) for x in lit_by_module[m]))
print(f"{len(files)} files, {len(failed)} failed to parse" + (f": {failed}" if failed else ""))
print(f"alternatives: {len(grammar) - len(missing_alts)} of {len(grammar)} used")
print(f"literals: {len(literals) - len(missing_lits)} of {len(literals)} used")
if args.json:
    Path(args.json).write_text(json.dumps({
        "alternatives": {f"{fam}::{l}": used_alts.get((fam, l), 0) for (fam, l) in grammar},
        "literals": {lit: used_lits.get(lit, 0) for lit in literals},
        "failed": failed}, indent=1))
