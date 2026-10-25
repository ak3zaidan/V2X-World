#!/usr/bin/env python3
"""Radio-validation runs and their judging (crates/v2xw-engine/tests/expectations/radio-validation.yaml).

  python3 -I tools/radioval/radioval.py plan  <expectations.yaml> <outdir>
      writes one base-only sensitivity spec per run into <outdir> and prints the v2xw
      command for each (run them one at a time, each through the run lock).
  python3 -I tools/radioval/radioval.py check <expectations.yaml> <outdir> [--fill] [--mutate]
      reads <outdir>/<run>.out (the text `v2xw sensitivity --base-only` prints) and judges
      every row with a `kind` this script knows; --fill writes measured/verdict back into the
      rows that are still `pending`; --mutate perturbs each row's input the way its fault says
      and reports the rows that stay green (a check that cannot fail).

Row kinds (theory is computed here, never read back from the engine):
  abs     metric of one run inside band [lo, hi]
  ratio   metric(run) / metric(base) inside band
  chain   metric over runs [a, b, c, ...] moves `expect` (up|down) by min_change at each step
  curve   census PRR over [lo, hi] m against a fading/shadowing law, weighted by each 20 m
          bin's receivers, times the run's own near-range floor (PRR at 40-200 m):
            step:      1 below d0, 0 above
            rayleigh:  exp(-(d/d0)^n)
            nakagami:  Q(m, m (d/d0)^n)  (regularised upper incomplete gamma, integer m)
            lognormal: Phi(10 n log10(d0/d) / sigma)
          pass when |measured - expected| <= tol
  txtime  airtime_us equals the 802.11 OFDM TXTIME of psdu_bytes at the run's rate,
          40 us + 8 us * ceil((16 + 6 + 8 L) / bits_per_symbol) (IEEE 802.11-2020 17.4.3, 10 MHz)
  same    the two runs' outputs are byte-identical (determinism)
"""
import math
import os
import re
import sys

import yaml  # PyYAML

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def load(path):
    with open(path) as f:
        return yaml.safe_load(f)


def plan(doc, outdir):
    os.makedirs(outdir, exist_ok=True)
    common = doc.get("common", {}) or {}
    for rid, r in doc["runs"].items():
        base = os.path.join(ROOT, "scenarios/sensitivity", r.get("base", "dsrc-grid.yaml"))
        st = dict(common) if r.get("common", True) else {}
        st.update(r.get("set", {}) or {})
        spec = {"base": base, "set": st, "rows": []}
        p = os.path.join(outdir, f"{rid}.spec.yaml")
        with open(p, "w") as f:
            yaml.safe_dump(spec, f, sort_keys=True)
        print(f"{rid}\tsensitivity --base-only {p}")


def parse_out(path):
    """The values and PRR bins of one base-only output."""
    vals, bins = {}, []
    with open(path) as f:
        for line in f:
            if line.startswith("prr per 20 m bin"):
                for tok in line[len("prr per 20 m bin"):].split():
                    m = re.match(r"([0-9.]+)\((\d+)\)", tok)
                    bins.append((0, 0.0) if not m else (int(m.group(2)), float(m.group(1))))
                continue
            parts = line.split()
            if len(parts) != 2:
                continue
            k, v = parts
            m = re.match(r"Some\(([-0-9.e]+)\)", v)
            if m:
                v = m.group(1)
            try:
                vals[k] = float(v)
            except ValueError:
                pass
    return vals, bins


def q_upper(m, x):
    """Regularised upper incomplete gamma Q(m, x) for integer m."""
    return math.exp(-x) * sum(x ** k / math.factorial(k) for k in range(int(m)))


def law_p(law, d):
    d0, n = law["d0"], law.get("n", 2.5)
    x = (d / d0) ** n
    k = law["kind"]
    if k == "step":
        return 1.0 if d < d0 else 0.0
    if k == "rayleigh":
        return math.exp(-x)
    if k == "nakagami":
        return q_upper(law["m"], law["m"] * x)
    if k == "lognormal":
        return 0.5 * (1 + math.erf(10 * n * math.log10(d0 / d) / law["sigma"] / math.sqrt(2)))
    raise ValueError(k)


def census(bins, lo, hi):
    c = sum(n for i, (n, _) in enumerate(bins) if lo <= i * 20 and (i + 1) * 20 <= hi)
    ok = sum(n * p for i, (n, p) in enumerate(bins) if lo <= i * 20 and (i + 1) * 20 <= hi)
    return c, (ok / c if c else None)


def judge(row, runs, mutate=False):
    """(measured text, pass bool) or None when the row is not this script's."""
    k = row.get("kind")
    tweak = (lambda v: v * row.get("fault_scale", 1.25)) if mutate else (lambda v: v)
    if k == "abs":
        v = tweak(runs[row["run"]][0].get(row["metric"]))
        if v is None:
            return "missing", False
        lo, hi = row["band"]
        return f"{v:.4g}", lo <= v <= hi
    if k == "ratio":
        a = runs[row["base"]][0].get(row["metric"])
        b = runs[row["run"]][0].get(row["metric"])
        if a is None or b is None or a == 0:
            return "missing", False
        b = tweak(b) if not mutate else a  # a fault: the change does nothing
        lo, hi = row["band"]
        return f"{a:.4g} -> {b:.4g} (x{b / a:.3f})", lo <= b / a <= hi
    if k == "chain":
        vs = [runs[r][0].get(row["metric"]) for r in row["runs"]]
        if any(v is None for v in vs):
            return "missing", False
        if mutate:
            vs = list(reversed(vs))
        mc = row.get("min_change", 0.01)
        ok = True
        for a, b in zip(vs, vs[1:]):
            rel = (b - a) / abs(a) if a else (math.inf if b > a else (-math.inf if b < a else 0))
            ok &= rel >= mc if row["expect"] == "up" else rel <= -mc
        return " -> ".join(f"{v:.4g}" for v in vs), ok
    if k == "curve":
        vals, bins = runs[row["run"]]
        lo, hi = row["range"]
        c, meas = census(bins, lo, hi)
        fc, floor = census(bins, *row.get("floor_range", [40, 200]))
        if not c or floor is None:
            return "no census", False
        law = dict(row["law"])
        if mutate:
            law["d0"] *= 0.85
        exp = floor * sum(n * law_p(law, (i + 0.5) * 20) for i, (n, _) in enumerate(bins)
                          if lo <= i * 20 and (i + 1) * 20 <= hi) / c
        return (f"{meas:.3f} vs theory {exp:.3f} (floor {floor:.3f}, n={c})",
                abs(meas - exp) <= row.get("tol", 0.06))
    if k == "txtime":
        vals = runs[row["run"]][0]
        L, air = vals.get("psdu_bytes"), vals.get("airtime_us")
        if L is None or air is None:
            return "missing", False
        bps = 24 * row.get("rate_mbps", 6) / 6  # data bits per OFDM symbol at 10 MHz
        exp = 40 + 8 * math.ceil((22 + 8 * L) / bps)
        if mutate:
            exp += 8
        want = row.get("equals")
        ok = abs(air - exp) < 0.01 and (want is None or abs(air - want) < 0.01)
        return f"airtime {air:.2f} us, PSDU {L:.2f} B, TXTIME {exp:.0f} us", ok
    if k == "same":
        a, b = (open(os.path.join(OUT, f"{r}.out"), "rb").read() for r in row["runs"])
        if mutate:
            b = b + b"x"
        return ("identical" if a == b else "differ"), a == b
    return None


OUT = None


def main():
    global OUT
    cmd, path, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
    doc = load(path)
    if cmd == "plan":
        plan(doc, OUT)
        return
    runs = {}
    for rid in doc["runs"]:
        p = os.path.join(OUT, f"{rid}.out")
        if os.path.exists(p):
            runs[rid] = parse_out(p)
    mutate = "--mutate" in sys.argv
    results = {}
    green = 0
    for row in doc["rows"]:
        needs = [row.get("run"), row.get("base")] + row.get("runs", [])
        if any(r and r not in runs for r in needs):
            continue
        j = judge(row, runs, mutate)
        if j is None:
            continue
        results[row["id"]] = j
        green += j[1]
        print(f"{'PASS' if j[1] else 'FAIL'}\t{row['id']}\t{j[0]}")
    print(f"{green}/{len(results)} {'green under mutation (want 0)' if mutate else 'pass'}")
    if "--fill" in sys.argv and not mutate:
        text = open(path).read()
        for rid, (meas, ok) in results.items():
            pat = re.compile(r"(- id: " + re.escape(rid) + r"\n(?:  .*\n)*?)  measured: pending\n  verdict: pending\n")
            text = pat.sub(lambda m: m.group(1) + f"  measured: {yaml.safe_dump(meas, default_style=None).strip().removesuffix('...').strip()}\n  verdict: {'pass' if ok else 'FAIL'}\n", text, count=1)
        open(path, "w").write(text)


if __name__ == "__main__":
    main()
