#!/usr/bin/env python3
"""Settings wiring audit: every scenario key must be read by something, and every knob a
guide names must exist.

Walks the scenario surface the way the engine's build script reflects it (the same
`REFLECT_SOURCES`, rustfmt layout: an item at column zero, a field at four spaces, a
struct-variant field at eight), from `Scenario` down, and for every field looks for a
read of its Rust name in the code that runs scenarios: a field access `.name` that is not
a method call, or a struct-pattern binding `name,` / `name }` / `name:`. The schema and
the loader-side files (validation, publishing, option labels, the guide, migration,
merging) do not count as readers: a key only they touch changes the scenario hash and
nothing else, which is the defect this audit exists to find.

A name match is necessary, not sufficient: a common name (`enabled`, `kind`) is found
somewhere whatever it belongs to, so this lists every key that is certainly unread and
cannot prove the rest are read. The catalogue sweep (`v2xw sensitivity --catalogue`) is the
run-based half of the same question.

Second half: every backticked dotted path in docs/guides/*.md whose first segment is a
scenario section must resolve to a key (list indices, `*` and `[]` accepted).

Usage: settings_wiring.py [--repo DIR] [--schema FILE] [--guides DIR] [--json]
Exit 1 when either list is non-empty. Prints one verdict line, then the lists.
"""
import argparse
import json
import os
import re
import sys

REFLECT_SOURCES = [
    "crates/v2xw-engine/src/scenario/schema.rs",
    "crates/v2xw-core/src/card.rs",
    "crates/v2xw-core/src/weather.rs",
    "crates/v2xw-world/src/lib.rs",
    "crates/v2xw-world/src/model.rs",
    "crates/v2xw-world/src/osm.rs",
    "crates/v2xw-energy/src/spec.rs",
    "crates/v2xw-energy/src/powertrain.rs",
    # Not reflected by the build script (the form shows `actors.vehicles.driving` as one
    # object) but loaded by serde, so its keys are scenario keys too.
    "crates/v2xw-mobility/src/carfollowing/library.rs",
    "crates/v2xw-mobility/src/coop.rs",
    "crates/v2xw-mobility/src/lateral/mod.rs",
]
SCHEMA = "crates/v2xw-engine/src/scenario/schema.rs"
# Loader-side files: touching a key here is not acting on it. (The schema's own `impl`
# blocks are read separately: see `schema_impls`.)
NOT_READERS = {
    "crates/v2xw-engine/src/scenario/schema.rs",
    "crates/v2xw-engine/src/scenario/validate.rs",
    "crates/v2xw-engine/src/scenario/publish.rs",
    "crates/v2xw-engine/src/scenario/options.rs",
    "crates/v2xw-engine/src/scenario/guide.rs",
    "crates/v2xw-engine/src/scenario/migrate.rs",
    "crates/v2xw-engine/src/scenario/merge.rs",
}


def rename_all(name, policy):
    words = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name).lower().split("_")
    if policy == "kebab-case":
        return "-".join(words)
    if policy == "snake_case":
        return "_".join(words)
    if policy == "lowercase":
        return "".join(words)
    return name


def parse(text):
    """{type name: {"fields": [(rust, wire, ty)], "variants": {wire: [(rust, wire, ty)]}}}"""
    items = {}
    lines = text.splitlines()
    i = 0
    attrs = []
    while i < len(lines):
        raw = lines[i]
        m = re.match(r"pub (struct|enum) (\w+)", raw)
        if raw.startswith("#["):
            attrs.append(raw)
        elif m and raw.rstrip().endswith("{"):
            kind, name = m.groups()
            policy = None
            for a in attrs:
                pm = re.search(r'rename_all\s*=\s*"([^"]+)"', a)
                if pm:
                    policy = pm.group(1)
            item = {"kind": kind, "fields": [], "variants": {}}
            fattrs = []
            variant = None
            i += 1
            while i < len(lines) and not lines[i].startswith("}"):
                line = lines[i]
                s = line.strip()
                if s.startswith("#["):
                    fattrs.append(s)
                elif s.startswith("//") or not s:
                    pass
                elif kind == "struct" and line.startswith("    pub ") and ":" in s:
                    fm = re.match(r"pub (\w+)\s*:\s*(.+?),?$", s)
                    if fm:
                        rust, ty = fm.groups()
                        wire = rust
                        for a in fattrs:
                            rm = re.search(r'rename\s*=\s*"([^"]+)"', a)
                            if rm and "rename_all" not in a:
                                wire = rm.group(1)
                        flat = any("flatten" in a for a in fattrs)
                        item["fields"].append((rust, wire, ty, flat))
                    fattrs = []
                elif kind == "enum" and re.match(r"    [A-Z]\w*", line) and not line.startswith("     "):
                    vm = re.match(r"    ([A-Z]\w*)", line)
                    vname = vm.group(1)
                    vwire = rename_all(vname, policy) if policy else vname
                    for a in fattrs:
                        rm = re.search(r'rename\s*=\s*"([^"]+)"', a)
                        if rm and "rename_all" not in a:
                            vwire = rm.group(1)
                    fattrs = []
                    variant = vwire if s.endswith("{") else None
                    item["variants"].setdefault(vwire, [])
                elif kind == "enum" and variant and line.startswith("        ") and ":" in s:
                    fm = re.match(r"(?:pub )?(\w+)\s*:\s*(.+?),?$", s)
                    if fm:
                        rust, ty = fm.groups()
                        item["variants"][variant].append((rust, rust, ty, False))
                    fattrs = []
                elif s.startswith("}"):
                    variant = None
                i += 1
            items[name] = item
            attrs = []
        elif raw and not raw.startswith((" ", "/", "#")):
            attrs = []
        i += 1
    return items


def inner_types(ty):
    """The struct/enum names a field type can lead to, and the path suffix it adds."""
    ty = ty.replace(" ", "")
    suffix = ""
    while True:
        m = re.match(r"^(?:Option|Box|std::option::Option)<(.+)>$", ty)
        if m:
            ty = m.group(1)
            continue
        m = re.match(r"^Vec<(.+)>$", ty)
        if m:
            ty = m.group(1)
            suffix += "[]"
            continue
        m = re.match(r"^(?:BTreeMap|HashMap|IndexMap)<[^,]+,(.+)>$", ty)
        if m:
            ty = m.group(1)
            suffix += ".*"
            continue
        break
    return ty.split("::")[-1], suffix


def walk(items):
    """[(path, owner type, rust name)] for every key reachable from Scenario."""
    out = []
    seen = set()

    def visit(tname, prefix, depth):
        item = items.get(tname)
        if item is None or depth > 12:
            return
        key = (tname, prefix)
        if key in seen:
            return
        seen.add(key)
        groups = [item["fields"]] + list(item["variants"].values())
        for fields in groups:
            for rust, wire, ty, flat in fields:
                path = prefix if flat else (f"{prefix}.{wire}" if prefix else wire)
                if not flat:
                    out.append((path, tname, rust))
                inner, suffix = inner_types(ty)
                visit(inner, path + suffix, depth + 1)

    visit("Scenario", "", 0)
    return out


def corpus(repo):
    texts = {}
    for root, dirs, files in os.walk(os.path.join(repo, "crates")):
        rel_root = os.path.relpath(root, repo)
        parts = rel_root.split(os.sep)
        if "tests" in parts or "benches" in parts or "target" in parts:
            continue
        for f in files:
            if not f.endswith(".rs"):
                continue
            rel = os.path.join(rel_root, f)
            if rel in NOT_READERS and rel != SCHEMA:
                continue
            with open(os.path.join(repo, rel), encoding="utf-8") as fh:
                text = strip_tests(fh.read())
            texts[rel] = schema_impls(text) if rel == SCHEMA else text
    return texts


def block_end(text, start):
    """The index just past the brace that closes the first `{` at or after `start`."""
    i = text.index("{", start)
    depth = 0
    while i < len(text):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return len(text)


def strip_tests(text):
    """Drops every `#[cfg(test)] mod x { ... }`: a test is not a reader."""
    pat = re.compile(r"^[ \t]*#\[cfg\(test\)\]\s*\n\s*(?:pub(?:\([^)]*\))? )?mod \w+ \{", re.M)
    while True:
        m = pat.search(text)
        if not m:
            return text
        text = text[: m.start()] + text[block_end(text, m.start()):]


def schema_impls(text):
    """The schema file's own code (its `impl` blocks) without the declarations, the
    `Default` impls and the minimal-scenario constructor: a method on a settings type
    that hands a field to a model is a reader; a default value is not."""
    out = []
    for m in re.finditer(r"^impl\b[^\n]*\{", text, re.M):
        if "Default for" in m.group(0):
            continue
        block = text[m.start(): block_end(text, m.start())]
        mm = re.search(r"^    pub fn minimal\(", block, re.M)
        if mm:
            block = block[: mm.start()] + block[block_end(block, mm.start()):]
        out.append(block)
    return "\n".join(out)


READ_PATTERNS = [
    re.compile(r"\.([A-Za-z_]\w*)\b(?!\s*\()"),  # a field access, not a method call
    re.compile(r"[{,(]\s*([A-Za-z_]\w*)\s*(?=[,}:])"),  # a struct-pattern binding
    re.compile(r"\b([A-Za-z_]\w*)\s*:\s*(?:ref\s+)?(?:mut\s+)?\w+\s*(?=[,}])"),  # `name: x`
]


def read_names(texts):
    """Every name the readers access as a field or bind in a struct pattern."""
    names = set()
    for t in texts.values():
        for pat in READ_PATTERNS:
            names.update(pat.findall(t))
    return names


def key_status(repo):
    """KEY_STATUS rows from validate.rs: [(path, status)]."""
    with open(os.path.join(repo, "crates/v2xw-engine/src/scenario/validate.rs"), encoding="utf-8") as fh:
        text = fh.read()
    return re.findall(r'path:\s*"([^"]+)",\s*status:\s*Status::(\w+)', text)


def status_of(path, rows):
    best = None
    for p, st in rows:
        if path == p or path.startswith(p + ".") or path.startswith(p + "["):
            if best is None or len(p) > len(best[0]):
                best = (p, st)
    return best[1] if best else None


def guide_paths(gdir, sections):
    found = []
    for f in sorted(os.listdir(gdir)):
        if not f.endswith(".md"):
            continue
        with open(os.path.join(gdir, f), encoding="utf-8") as fh:
            for n, line in enumerate(fh, 1):
                for tok in re.findall(r"`([a-z_][a-z0-9_]*(?:\.[A-Za-z0-9_\-\[\]*]+)+)(?:\s*[:=][^`]*)?`", line):
                    if re.search(r"\.(rs|md|ya?ml|json|py|csv|txt)$", tok):
                        continue  # a file name, not a key
                    if tok.split(".")[0] in sections:
                        found.append((f, n, tok))
    return found


def normalise(path):
    path = re.sub(r"\[\d*\]", "[]", path)
    path = re.sub(r"\.\d+(?=\.|$)", "[]", path)
    return path


_KEY_PATTERNS = None


def resolves(tok, keys, maps):
    t = normalise(tok)
    if t.endswith("*"):  # a wildcard names a section or a family of keys: one must exist
        stem = t.rstrip("*")
        if any(k.startswith(stem) for k in keys):
            return True
        t = stem.rstrip(".")  # `x.params.*`: the free-form map `x.params` itself
        if t in maps:
            return True
    if t in keys or t.rstrip(".") in keys:
        return True
    # a map key in place of `*`, or a list element named without its brackets
    global _KEY_PATTERNS
    if _KEY_PATTERNS is None:
        _KEY_PATTERNS = [re.compile(re.escape(k).replace(r"\.\*", r"\.[^.]+")
                                    .replace(r"\[\]", r"(?:\[\])?")) for k in keys]
    if any(p.fullmatch(t) for p in _KEY_PATTERNS):
        return True
    # a key under a free-form map (model params): the map itself must exist
    return any(t.startswith(m + ".") for m in maps)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=os.path.abspath(os.path.join(os.path.dirname(__file__), "../..")))
    ap.add_argument("--schema", help="read this file in place of schema.rs (to inject a fault)")
    ap.add_argument("--guides", help="the guides directory (default docs/guides; another to inject a fault)")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    items = {}
    for src in REFLECT_SOURCES:
        path = a.schema if (a.schema and src.endswith("scenario/schema.rs")) else os.path.join(a.repo, src)
        with open(path, encoding="utf-8") as fh:
            items.update(parse(fh.read()))
    keys = walk(items)
    texts = corpus(a.repo)
    names = read_names(texts)
    unread = []
    exempt = []
    rows = key_status(a.repo)
    for path, owner, rust in keys:
        if rust not in names:
            st = status_of(path, rows)
            # Descriptive keys change nothing by design; Refused keys are read by the
            # loader, which refuses every value it cannot act on (shown as Unsupported).
            if st in ("Descriptive", "Refused"):
                exempt.append((path, st))
            else:
                unread.append(path)
    keyset = {normalise(p) for p, _, _ in keys}
    maps = {normalise(p) for p, o, r in keys
            for (rr, w, ty, fl) in items[o]["fields"] + sum(items[o]["variants"].values(), [])
            if rr == r and re.search(r"(Map|Value|Params)\b", ty)}
    sections = {p.split(".")[0] for p in keyset}
    named = guide_paths(a.guides or os.path.join(a.repo, "docs/guides"), sections)
    missing = [(f, n, t) for f, n, t in named if not resolves(t, keyset, maps)]
    if a.json:
        print(json.dumps({"keys": len(keys), "unread": unread, "exempt": exempt,
                          "guide_missing": missing}, indent=1))
    else:
        verdict = "PASS" if not unread and not missing else "FAIL"
        print(f"{verdict}: {len(keys)} keys, {len(unread)} unread, {len(missing)} of {len(named)} guide knob mentions missing"
              f" ({len(exempt)} unread by design: " + ", ".join(f"{p} {s}" for p, s in exempt) + ")")
        for p in unread:
            print(f"  unread  {p}")
        for f, n, t in missing:
            print(f"  missing {t}  ({f}:{n})")
    sys.exit(0 if not unread and not missing else 1)


if __name__ == "__main__":
    main()
