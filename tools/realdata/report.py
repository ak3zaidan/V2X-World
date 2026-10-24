"""Stage 7 (outputs), part 2: the tables of P1-E1/P1-E2 as Markdown plus a results JSON."""
import bootci
import devclass
import mbrchecks as mc
import pipeline
from calib import r6

BASE_NAMES = ("secMark", "header", "rx")
FAST = (3, 4)
ALL = tuple(range(len(mc.SPEED_BANDS)))
CLS_ROWS = (("locked", ("locked",)), ("mixed", ("mixed",)), ("free", ("free",)), ("all", None))


def fmt(x, nd=1):
    if x is None or x != x:
        return "n/a"
    return ("%." + str(nd) + "f") % x


class Tables:
    def __init__(self, params):
        self.params = params
        self.md = []
        self.json = {}

    def cell(self, ds, cls, base, li, bands, check_i, tag):
        u = pipeline.cell_units(ds, base, li, bands, check_i, cls, self.params)
        units = [(t, m) for t, m, _, _ in u]
        key = "|".join(map(str, (ds.name, cls, base, li, bands, check_i)))
        rate, lo, hi = bootci.ci(units, key)
        out = {"ids": len(u), "pairs": sum(x[2] for x in u), "triggers": int(sum(x[0] for x in u)),
               "minutes": r6(sum(x[1] for x in u)), "rate": r6(rate), "ci95": [r6(lo), r6(hi)]}
        self.json[tag] = out
        return out

    @staticmethod
    def rc(c):
        return "%s [%s, %s]" % (fmt(c["rate"]), fmt(c["ci95"][0]), fmt(c["ci95"][1])) if c["ids"] else "-"

    def add(self, *lines):
        self.md.extend(lines)

    def describe(self, datasets):
        self.add("### T1. Datasets", "",
                 "| Dataset | Role | Raw rows | Unique BSMs | IDs | locked / mixed / free / unclassified IDs | secMark Δt in 90-110 ms | msgCnt +1 |",
                 "|---|---|---|---|---|---|---|---|")
        for ds in datasets:
            n = sum(r.nbsm for r in ds.ids.values())
            cnt = {c: 0 for c in devclass.CLASSES + ("unclassified",)}
            for r in ds.ids.values():
                cnt[devclass.classify(r.frac, r.nsm, self.params)] += 1
            near = sum(r.dt_near100 for r in ds.ids.values())
            alld = sum(r.dt_all for r in ds.ids.values())
            ok = sum(r.cnt_ok for r in ds.ids.values())
            cn = sum(r.cnt_n for r in ds.ids.values())
            cl = "n/a (no secMark phase)" if ds.name.startswith("NYC") else " / ".join(
                str(cnt[c]) for c in ("locked", "mixed", "free", "unclassified"))
            self.add("| %s | %s | %d | %d | %d | %s | %s%% | %s%% |" % (
                ds.name, ds.role, ds.raw_rows, n, len(ds.ids), cl, fmt(100.0 * near / alld if alld else None),
                fmt(100.0 * ok / cn if cn else None)))
            self.json["T1|" + ds.name] = {"raw_rows": ds.raw_rows, "bsms": n, "ids": len(ds.ids), "classes": cnt}
        self.add("")

    def headline(self, datasets):
        self.add("### T2. RandomPosition v1.0 (gpsDrift 1 m), secMark Δt, gap limit 0.5 s, v ≥ 10 m/s", "",
                 "Rate is triggers per observed ID-minute of judged pairs, with an ID-clustered bootstrap 95%% CI "
                 "(%d replicates)." % bootci.B, "",
                 "| Dataset | Class | IDs | Pairs | Triggers | % of pairs | Triggers per ID-minute [95% CI] |",
                 "|---|---|---|---|---|---|---|")
        for ds in datasets:
            for cname, cls in self._cls_rows(ds):
                c = self.cell(ds, cls, 0, 2, FAST, 2, "T2|%s|%s" % (ds.name, cname))
                if not c["ids"]:
                    continue
                self.add("| %s | %s | %d | %d | %d | %s | %s |" % (
                    ds.name, cname, c["ids"], c["pairs"], c["triggers"],
                    fmt(100.0 * c["triggers"] / c["pairs"], 2), self.rc(c)))
        self.add("")

    def _cls_rows(self, ds):
        if ds.name.startswith("NYC"):
            return (("all", None),)
        return CLS_ROWS

    def matrix(self, datasets, bases):
        self.add("### T3. RandomPosition by reading, time base and gap limit (v ≥ 10 m/s)", "",
                 "v1.0 = velocity only; v1.1 = velocity and accelSet.long (ASN.1 v1.1; its canonical cell is "
                 "the 0.1 s limit). Cells: triggers per ID-minute [95% CI]. Wyoming has no accelSet.long, so its "
                 "v1.1 rows equal v1.0.", "",
                 "| Dataset | Class | Reading | Δt base | ≤ 0.1 s | ≤ 0.2 s | ≤ 0.5 s |", "|---|---|---|---|---|---|---|")
        for ds in datasets:
            for cname, cls in self._cls_rows(ds):
                if cname == "mixed":
                    continue
                for reading, ci in (("v1.0", 2), ("v1.1", 3)):
                    for b in bases[ds.name]:
                        cells = [self.cell(ds, cls, b, li, FAST, ci, "T3|%s|%s|%s|%s|%s" % (
                            ds.name, cname, reading, BASE_NAMES[b], mc.GAP_LIMITS_S[li])) for li in range(3)]
                        if not any(c["ids"] for c in cells):
                            continue
                        self.add("| %s | %s | %s | %s | %s |" % (ds.name, cname, reading, BASE_NAMES[b],
                                                                 " | ".join(self.rc(c) for c in cells)))
        self.add("")

    def bands(self, datasets, check_i=2, title="T4. RandomPosition v1.0", what="over 1 m"):
        self.add("### %s by speed band (secMark, 0.5 s)" % title, "",
                 "Cells: %% of judged pairs %s (triggers per ID-minute)." % what, "",
                 "| Dataset | Class | " + " | ".join(mc.BAND_NAMES) + " m/s |", "|---|---|" + "---|" * len(mc.BAND_NAMES))
        for ds in datasets:
            for cname, cls in self._cls_rows(ds):
                if cname == "mixed":
                    continue
                row = []
                for bi in range(len(mc.SPEED_BANDS)):
                    u = pipeline.cell_units(ds, 0, 2, (bi,), check_i, cls, self.params)
                    t = sum(x[0] for x in u)
                    n = sum(x[2] for x in u)
                    m = sum(x[1] for x in u)
                    row.append("%s%% (%s)" % (fmt(100.0 * t / n, 2), fmt(t / m)) if n else "-")
                    self.json["%s|%s|%s|%s" % (title.split(".")[0], ds.name, cname, mc.BAND_NAMES[bi])] = {
                        "pairs": n, "triggers": int(t)}
                self.add("| %s | %s | %s |" % (ds.name, cname, " | ".join(row)))
        self.add("")

    def others(self, datasets):
        names = ("ConstantPosition", "MaxSpeed", "MaxAccel (implied)", "MaxAccel (accelSet)", "BrakeAccel")
        self.add("### T5. The other SM-MBR checks (secMark, 0.5 s, all speeds)", "",
                 "Cells: triggers per ID-minute [95% CI]. MaxAccel (implied) is |Δv|/Δt > 10 m/s² (the engine's "
                 "reading); MaxAccel (accelSet) is the transmitted |accelSet.long| > 10. BrakeAccel needs brakes and "
                 "accelSet.long: only Orem and NYC carry both.", "",
                 "| Dataset | Class | " + " | ".join(names) + " |", "|---|---|" + "---|" * len(names))
        for ds in datasets:
            for cname, cls in self._cls_rows(ds):
                if cname == "mixed":
                    continue
                cells = [self.cell(ds, cls, 0, 2, ALL, 2 + ci, "T5|%s|%s|%s" % (ds.name, cname, nm))
                         for ci, nm in zip(range(2, 7), names)]
                if not cells[0]["ids"]:
                    continue
                self.add("| %s | %s | %s |" % (ds.name, cname, " | ".join(self.rc(c) for c in cells)))
        self.add("")

    def anatomy(self, datasets):
        self.add("### T6. Anatomy of the RandomPosition triggers (secMark, 0.5 s, v ≥ 10 m/s)", "",
                 "Epoch-slip signature: |Δp| ≈ 0 (< 0.4 v Δt, stale fix) or ≈ 2 v Δt (1.6-2.4, fix from the next "
                 "epoch). Residual centiles of all judged pairs, from 1 dB log bins (upper bin edge).", "",
                 "| Dataset | Class | Triggers | ≈ 0 | ≈ 2 v Δt | cross-track dominant | residual p50 / p99 (m) |",
                 "|---|---|---|---|---|---|---|")
        for ds in datasets:
            for cname, cls in self._cls_rows(ds):
                if cname == "mixed":
                    continue
                t = z = d = x = 0
                hist = {}
                for r in ds.ids.values():
                    if cls is not None and devclass.classify(r.frac, r.nsm, self.params) not in cls:
                        continue
                    for b in FAST:
                        a = r.acc.get((0, 2, b))
                        if a:
                            t += a[2]
                            z += a[pipeline.I_SLIPZ]
                            d += a[pipeline.I_SLIPD]
                            x += a[pipeline.I_CROSS]
                    for k, n in r.res_hist.items():
                        hist[k] = hist.get(k, 0) + n
                if not hist:
                    continue
                q = hist_q(hist, (0.5, 0.99))
                self.json["T6|%s|%s" % (ds.name, cname)] = {"triggers": t, "zero": z, "double": d, "cross": x,
                                                           "p50": q[0], "p99": q[1]}
                pc = (lambda v: fmt(100.0 * v / t, 0) + "%") if t else (lambda v: "-")
                self.add("| %s | %s | %d | %s | %s | %s | %s / %s |" % (ds.name, cname, t, pc(z), pc(d), pc(x),
                                                                       fmt(q[0], 3), fmt(q[1], 2)))
        self.add("")


def hist_q(hist, qs):
    tot = sum(hist.values())
    out = []
    for q in qs:
        acc = 0
        for k in sorted(hist):
            acc += hist[k]
            if acc >= q * tot:
                out.append(r6(pipeline.RES_EDGES[min(k + 1, len(pipeline.RES_EDGES) - 1)]))
                break
    return out
