"""tools/realdata: the Paper 1 real-data pipeline (paper-plan-2026-10.md §4.2, stages 1-7).

  python3 tools/realdata/rdtool.py fetch --raw DIR   stage 1: pinned SODA queries + manifest
  python3 tools/realdata/rdtool.py fit   --raw DIR   stages 2-5 on the fit sets (Tampa, Orem);
                                                     writes the frozen classifier (commit it!)
  python3 tools/realdata/rdtool.py all   --raw DIR   every number: verify manifest, refit and
                                                     compare with the frozen classifier, run the
                                                     falsification gate, score Wyoming (held out)
                                                     and NYC (separate), write the outputs

DIR holds the raw rows and is never inside the repository. Over a minute of CPU: run it
through the machine's run lock (see the baseline doc).
"""
import argparse
import json
import os
import subprocess
import sys

import calib
import devclass
import fgate
import manifest
import normalise
import pairing
import pipeline
import report
import sources
from version import CLASSIFIER_VERSION, VERSION

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
FROZEN = os.path.join(HERE, "frozen", "classifier-v1.json")
YAML_OUT = os.path.join(REPO, "data", "calibration", "bsm-timing-cvpilot.yaml")
RESULTS_OUT = os.path.join(REPO, "research", "synthesis", "p1-realdata-results.json")
DOC = os.path.join(REPO, "research", "synthesis", "p1-realdata-baseline.md")
BEGIN, END = "<!-- BEGIN GENERATED: tools/realdata rdtool.py all -->", "<!-- END GENERATED -->"

NAMES = {"tampa": "Tampa", "orem": "Orem", "wyoming": "Wyoming", "nyc": "NYC (warning-selected)"}


def log(*a):
    print(*a, flush=True)


def files_of(raw, prefix):
    out = []
    for q in sources.queries():
        if q["name"] == prefix or q["name"].startswith(prefix + "_"):
            pre = q["name"][len(prefix) + 1:] if q["name"] != prefix else None
            out.append((os.path.join(raw, q["file"]), pre))
    return out


def fit_sets(raw, orem_days=None):
    t = pipeline.analyse_files(NAMES["tampa"], "fit", files_of(raw, "tampa"), "tampa")
    log("analysed Tampa: %d IDs" % len(t.ids))
    of = files_of(raw, "orem")
    if orem_days:
        of = [f for f in of if f[1] in orem_days]
    o = pipeline.analyse_files(NAMES["orem"], "fit", of, "orem")
    log("analysed Orem (%d day files): %d IDs" % (len(of), len(o.ids)))
    return [t, o]


def fit_file_hashes(names):
    return {e["name"]: e["sha256"] for e in manifest.load()["files"] if e["name"].split("_")[0] in names}


def git_committed_clean(path):
    try:
        subprocess.run(["git", "-C", REPO, "ls-files", "--error-unmatch", path], check=True,
                       capture_output=True)
        r = subprocess.run(["git", "-C", REPO, "diff", "--quiet", "HEAD", "--", path], capture_output=True)
        if r.returncode != 0:
            return None
        c = subprocess.run(["git", "-C", REPO, "log", "-1", "--format=%h", "--", path], check=True,
                           capture_output=True, text=True)
        return c.stdout.strip() or None
    except (OSError, subprocess.CalledProcessError):
        return None


def cmd_fetch(a):
    import fetchall
    fetchall.run(a.raw)


def cmd_fit(a):
    bad = manifest.verify(a.raw, names=None)
    if bad:
        sys.exit("raw data do not match the manifest: " + "; ".join(bad))
    fs = fit_sets(a.raw)
    doc = calib.frozen_doc(fs, devclass.params(), CLASSIFIER_VERSION, fit_file_hashes(("tampa", "orem")))
    os.makedirs(os.path.dirname(FROZEN), exist_ok=True)
    with open(FROZEN, "w") as f:
        f.write(calib.dump_json(doc))
    log("wrote", os.path.relpath(FROZEN, REPO), "- commit it before running `all` (Wyoming is scored there)")
    log(json.dumps(doc["fit"], sort_keys=True))


def wyoming_trace(wy, params):
    """The locked Wyoming ID with the most fast single-interval pairs (ties: smallest id)."""
    best = None
    for vid, bsms in normalise.stream_ids(files_of(RAW, "wyoming")[0][0], "wyoming"):
        r = wy.ids[vid]
        if devclass.classify(r.frac, r.nsm, params) != "locked":
            continue
        bs = pairing.dedupe_sort(bsms)
        n = sum(1 for p in pairing.pairs(bs) if p.dt[0] and 0.05 < p.dt[0] < 0.15 and (p.v1 or 0) >= 15)
        if best is None or n > best[0] or (n == best[0] and vid < best[1]):
            best = (n, vid, bs)
    return best


RAW = None


def class_test(tab, frozen, wy, params):
    pred = frozen["predictions"]
    tab.add("### T7. Device-class test out of sample (P1-E2)", "",
            "Thresholds and predictions frozen in `tools/realdata/frozen/classifier-v1.json` (committed %s) "
            "before Wyoming was scored. Headline cell as T2." % tab.json.get("frozen_commit", "?"), "",
            "| Set | Class | IDs | Rate [95% CI] | Frozen prediction | Result |", "|---|---|---|---|---|---|")
    verdicts = {}
    rates = {}
    for c in ("locked", "free"):
        cell = tab.cell(wy, (c,), 0, 2, report.FAST, 2, "T7|Wyoming|" + c)
        if not cell["ids"]:
            tab.add("| Wyoming (held out) | %s | 0 | - | %s | not testable: no %s IDs |" % (
                c, json.dumps(pred[c]), c))
            verdicts[c] = None
            continue
        rates[c] = cell["rate"]
        if c == "locked":
            ok = cell["rate"] <= pred[c]["rate_at_most"]
        else:
            ok = cell["rate"] >= pred[c]["rate_at_least"]
        verdicts[c] = ok
        tab.add("| Wyoming (held out) | %s | %d | %s | %s | %s |" % (c, cell["ids"], tab.rc(cell),
                                                                   json.dumps(pred[c]), "PASS" if ok else "FAIL"))
    if len(rates) == 2:
        verdicts["order"] = rates["free"] > rates["locked"]
    tab.json["T7|verdicts"] = verdicts
    tab.add("")
    return verdicts


def transfer_test(tab, fs, params):
    """Secondary, in-fit: thresholds from Tampa alone predict Orem's free class."""
    t, o = fs
    tl = tab.cell(t, ("locked",), 0, 2, report.FAST, 2, "T7b|Tampa|locked")
    tf = tab.cell(t, ("free",), 0, 2, report.FAST, 2, "T7b|Tampa|free")
    ol = tab.cell(o, ("locked",), 0, 2, report.FAST, 2, "T7b|Orem|locked")
    of = tab.cell(o, ("free",), 0, 2, report.FAST, 2, "T7b|Orem|free")
    tab.add("Secondary (in-fit, not out of sample): bounds from Tampa alone, scored on Orem. "
            "Tampa locked %s, free %s; Orem locked %s (%s), free %s (%s)." % (
                tab.rc(tl), tab.rc(tf), tab.rc(ol),
                ("≤ %s: pass" % report.fmt(tl["ci95"][1]) if ol["ids"] and ol["rate"] <= tl["ci95"][1]
                 else ("no IDs" if not ol["ids"] else "fail")),
                tab.rc(of),
                ("≥ %s: pass" % report.fmt(tf["ci95"][0]) if of["ids"] and of["rate"] >= tf["ci95"][0]
                 else ("no IDs" if not of["ids"] else "fail"))), "")


def gate_table(tab, rows, passed):
    tab.add("### T0. Falsification gate (run before every table)", "",
            "| Test | Result | Detail |", "|---|---|---|")
    for name, ok, det in rows:
        tab.add("| %s | %s | %s |" % (name, "pass" if ok else "FAIL", det))
    tab.add("", "Gate: **%s**." % ("PASSED" if passed else "FAILED"), "")
    tab.json["T0|gate"] = {"passed": passed, "rows": [[n, o, d] for n, o, d in rows]}


def timing_table(tab, fit_agg, yaml_text):
    tab.add("### T8. Fitted timing-model parameters (fit sets: Tampa + Orem)", "",
            "Written to `data/calibration/bsm-timing-cvpilot.yaml` (schema v2xw.bsm_timing.v1).", "",
            "| Class | IDs | single-interval moving pairs at risk (v ≥ 5 m/s) | of them v ≥ 10 m/s | slip onsets (v ≥ 10 m/s) | frozen-repeat onsets |",
            "|---|---|---|---|---|---|")
    for c in ("locked", "mixed", "free"):
        a = fit_agg.get(c)
        if a:
            tab.add("| %s | %d | %d | %d | %d | %d |" % (c, a["ids"], a["at_risk"], a["at_risk_fast"], a["slip"],
                                                      a["frozen"]))
    tab.add("", "```yaml", yaml_text.rstrip("\n"), "```", "")


def write_doc(md):
    block = BEGIN + "\n" + md + "\n" + END
    if os.path.exists(DOC):
        s = open(DOC).read()
        if BEGIN in s and END in s:
            s = s[:s.index(BEGIN)] + block + s[s.index(END) + len(END):]
        else:
            s = s.rstrip("\n") + "\n\n" + block + "\n"
    else:
        s = "# P1 real-data baseline\n\n" + block + "\n"
    with open(DOC, "w") as f:
        f.write(s)


def cmd_all(a):
    global RAW
    RAW = a.raw
    bad = manifest.verify(a.raw)
    if bad:
        sys.exit("raw data do not match the committed manifest: " + "; ".join(bad))
    params = devclass.params()
    fs = fit_sets(a.raw)
    doc = calib.frozen_doc(fs, params, CLASSIFIER_VERSION, fit_file_hashes(("tampa", "orem")))
    if not os.path.exists(FROZEN):
        sys.exit("no frozen classifier: run `fit`, commit %s, then `all`" % os.path.relpath(FROZEN, REPO))
    if open(FROZEN).read() != calib.dump_json(doc):
        sys.exit("the refit differs from the frozen classifier: re-run `fit`, review and commit it before scoring")
    commit = git_committed_clean(FROZEN)
    if commit is None and not a.allow_uncommitted_frozen:
        sys.exit("the frozen classifier is not committed (or has local edits): commit it before Wyoming is scored")
    frozen = json.loads(open(FROZEN).read())
    log("frozen classifier verified (commit %s)" % commit)

    tab = report.Tables(params)
    tab.json["frozen_commit"] = commit
    # Stage 6: the gate, before any table. It reads one locked Wyoming trace for injection only.
    wy = pipeline.analyse_files(NAMES["wyoming"], "held_out", files_of(a.raw, "wyoming"), "wyoming")
    tr = wyoming_trace(wy, params)
    units = []
    for ds in fs:
        for c in ("locked", "free"):
            units += [(c, t, m) for t, m, _, _ in pipeline.cell_units(ds, 0, 2, report.FAST, 2, (c,), params)]
    passed, rows = fgate.full(tr[2], units)
    gate_table(tab, rows, passed)
    log("gate:", "PASSED" if passed else "FAILED")
    for r in rows:
        log("  ", r[0], "ok" if r[1] else "RED", "-", r[2])
    if not passed:
        sys.exit("falsification gate failed: no tables produced")

    nyc = pipeline.analyse_files(NAMES["nyc"], "separate", files_of(a.raw, "nyc"), "nyc")
    log("analysed Wyoming %d IDs, NYC %d IDs" % (len(wy.ids), len(nyc.ids)))
    datasets = fs + [wy, nyc]
    bases = {NAMES["tampa"]: (0, 2), NAMES["orem"]: (0, 2), NAMES["wyoming"]: (0, 1), NAMES["nyc"]: (0,)}
    tab.describe(datasets)
    tab.headline(datasets)
    verdicts = class_test(tab, frozen, wy, params)
    transfer_test(tab, fs, params)
    tab.matrix(datasets, bases)
    tab.bands(datasets)
    tab.bands(datasets, 4, "T4b. ConstantPosition", "frozen (< 1 cm) while both speeds >= 0.02 m/s")
    tab.others(datasets)
    tab.anatomy(datasets)

    fit_agg = calib.timing_stats(fs, params)
    hdr_agg = calib.timing_stats([wy], params)
    mf = manifest.load()["files"]
    prov = {"sources": [{"dataset": e["dataset"], "rows": e["rows"], "query": e["query"]} for e in mf
                        if e["name"].split("_")[0] in ("tampa", "orem")],
            "manifest_sha256": manifest.manifest_sha256(),
            "fitted_on": [sources.TAMPA, sources.OREM, sources.WYOMING + " (header_jitter_ms only)"],
            "held_out": [sources.WYOMING + " (device class and trigger rates)"]}
    notes = []
    for ds in fs:
        agg = calib.timing_stats([ds], params)
        nl, nf = agg.get("locked", {}).get("ids", 0), agg.get("free", {}).get("ids", 0)
        notes.append("%s: %d locked, %d free IDs (locked share %.3f); the share is deployment-specific."
                     % (ds.name, nl, nf, nl / (nl + nf) if nl + nf else float("nan")))
    yaml_text = calib.timing_yaml(fit_agg, hdr_agg, prov, VERSION, notes)
    timing_table(tab, fit_agg, yaml_text)
    with open(YAML_OUT, "w") as f:
        f.write(yaml_text)
    tab.json["verdicts"] = verdicts
    tab.json["tool"] = "tools/realdata " + VERSION
    tab.json["manifest_sha256"] = manifest.manifest_sha256()
    with open(RESULTS_OUT, "w") as f:
        json.dump(tab.json, f, indent=1, sort_keys=True)
        f.write("\n")
    write_doc("\n".join(tab.md))
    log("wrote", os.path.relpath(YAML_OUT, REPO), os.path.relpath(RESULTS_OUT, REPO), os.path.relpath(DOC, REPO))
    log("class test verdicts:", verdicts)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=("fetch", "fit", "all"))
    ap.add_argument("--raw", required=True, help="directory of raw rows (outside the repository)")
    ap.add_argument("--allow-uncommitted-frozen", action="store_true",
                    help="development only: score Wyoming although the frozen classifier is not committed")
    a = ap.parse_args(argv)
    if os.path.abspath(a.raw).startswith(REPO + os.sep):
        sys.exit("--raw must be outside the repository (share-alike and unstated licences)")
    {"fetch": cmd_fetch, "fit": cmd_fit, "all": cmd_all}[a.cmd](a)


if __name__ == "__main__":
    main()
