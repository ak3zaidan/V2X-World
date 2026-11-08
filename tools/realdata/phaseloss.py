"""msgCnt-attested BSM loss by secMark phase (mod 100 ms), per site, RSU / device and hour.

  python3 -I tools/realdata/phaseloss.py tampa <tampa_rsu.csv>
  python3 -I tools/realdata/phaseloss.py wyoming <wyoming_meta.csv>
  python3 -I tools/realdata/phaseloss.py nyc <nyc.csv>

Loss counting is fitoff.py's (tools/realworld/paper1, commit ca0d0ec4): consecutive records of
one temporary ID with msgCnt step dm in 1..30 (mod 128) and secMark step within 60 ms of
100*dm attest dm-1 missing BSMs, each placed at the interpolated secMark phase. Free devices
only (share of secMarks within 1 ms of a 50 ms epoch < 0.3). Tampa is sequenced per (ID, RSU)
here, so a loss is a loss at that RSU (fitoff pooled the RSUs; the pooled figure is printed too).
Statistic: loss share of slots at phase [10,30) ms vs [50,100) ms, with a 95 % bootstrap CI
over temporary IDs (2,000 resamples, seed 1). NYC has no UTC time (T_s is translated per event),
so only the phase relative to the event origin and to the host's own BSMs can be formed.
"""
import collections
import csv
import datetime as dt
import json
import random
import sys

csv.field_size_limit(1 << 30)
LO, HI = (10, 30), (50, 100)


def near(s):
    m = s % 50
    return min(m, 50 - m) <= 1


def free(v):
    return len(v) >= 10 and sum(near(x[1]) for x in v) / len(v) < 0.3


def slots(v, max_dm=30):
    """v sorted by time: (t, secmark, msgcnt). Yields (phase_ms, lost, t). Wyoming and NYC
    logs fall to 1 Hz outside an event's core (msgCnt steps of 10/11), so there max_dm is 9."""
    for a, c in zip(v, v[1:]):
        dm = (c[2] - a[2]) % 128
        ds = (c[1] - a[1]) % 60000
        if dm == 0 or dm > max_dm or abs(ds - 100 * dm) > 60:
            continue
        step = ds / dm
        for j in range(1, dm + 1):
            yield int(((a[1] + step * j) % 60000) % 100), j < dm, a[0]


class Acc:
    def __init__(self):
        self.by_id = collections.defaultdict(lambda: [0, 0, 0, 0])  # lo_n lo_lost hi_n hi_lost

    def add(self, key, ph, lost):
        b = self.by_id[key]
        if LO[0] <= ph < LO[1]:
            b[0] += 1
            b[1] += lost
        elif HI[0] <= ph < HI[1]:
            b[2] += 1
            b[3] += lost

    def stat(self, ids=None):
        ids = list(self.by_id) if ids is None else ids
        s = [sum(self.by_id[i][k] for i in ids) for k in range(4)]
        lo = s[1] / s[0] if s[0] else float("nan")
        hi = s[3] / s[2] if s[2] else float("nan")
        return lo, hi, s[0], s[2]

    def ci(self, reps=2000):
        ids = list(self.by_id)
        if len(ids) < 3:
            return None
        rng = random.Random(1)
        d = []
        for _ in range(reps):
            lo, hi, n0, n2 = self.stat([rng.choice(ids) for _ in ids])
            if n0 and n2:
                d.append(lo - hi)
        d.sort()
        return d[int(0.025 * len(d))], d[int(0.975 * len(d)) - 1]

    def row(self, name):
        lo, hi, n0, n2 = self.stat()
        c = self.ci()
        cs = "n/a" if c is None else "%+.3f..%+.3f" % c
        ex = (lo - hi) / (1 - hi) if hi < 1 else float("nan")
        # off-channel probability scaled from the pooled Tampa fit (0.45 at excess 0.372,
        # data/calibration/rsu-off-channel-tampa.yaml, same timing assumed)
        return "| %s | %d | %d | %.3f | %d | %.3f | %+.3f | %s | %.3f | %.2f |" % (
            name, len(self.by_id), n0, lo, n2, hi, lo - hi, cs, ex, min(1.0, max(0.0, 0.45 * ex / 0.372)))


HEAD = ("| group | IDs | slots 10-30 | loss 10-30 | slots 50-99 | loss 50-99 | diff | 95% CI (ID bootstrap) |"
        " excess | p_off |\n|---|---|---|---|---|---|---|---|---|---|")


def iso(s):
    # ODE times are UTC; never read them in the machine's local zone
    return dt.datetime.fromisoformat(s.replace("Z", "")).replace(tzinfo=dt.timezone.utc).timestamp()


def hour(t):
    return dt.datetime.fromtimestamp(t, dt.timezone.utc).strftime("%H")


def tampa(path):
    per, pooled = collections.defaultdict(list), collections.defaultdict(list)
    for r in csv.DictReader(open(path)):
        try:
            x = (iso(r["metadata_recordgeneratedat"]), int(float(r["coredata_secmark"])),
                 int(float(r["coredata_msgcnt"])))
        except (ValueError, KeyError):
            continue
        per[(r["coredata_id"], r["metadata_rsuid"])].append(x)
        pooled[r["coredata_id"]].append(x)
    freeids = {k for k, v in pooled.items() if free(v)}
    groups = collections.defaultdict(Acc)
    phase = collections.defaultdict(lambda: [0, 0])
    for (i, rsu), v in per.items():
        if i not in freeids:
            continue
        v.sort()
        for ph, lost, t in slots(v):
            h = hour(t)
            for g in ("all (per ID+RSU)", "rsu " + rsu, "hour %s UTC" % h):
                groups[g].add(i, ph, lost)
            phase[ph // 10][0] += 1
            phase[ph // 10][1] += lost
    pool = Acc()
    for i in freeids:
        v = sorted(pooled[i])
        for ph, lost, _ in slots(v):
            pool.add(i, ph, lost)
    print("free IDs %d of %d" % (len(freeids), len(pooled)))
    print(HEAD)
    print(pool.row("all (pooled RSUs, fitoff method)"))
    for g in sorted(groups, key=lambda g: (g.count(" hour") + g.startswith("hour"), g)):
        if groups[g].stat()[2] >= 200:
            print(groups[g].row(g))
    print("phase decade loss:", " ".join("%d:%.3f" % (k * 10, phase[k][1] / phase[k][0]) for k in sorted(phase)))


def wyoming(path):
    per = collections.defaultdict(list)
    kinds = collections.Counter()
    for r in csv.DictReader(open(path)):
        try:
            x = (iso(r["metadata_generatedat"]), int(float(r["coredata_secmark"])), int(float(r["coredata_msgcnt"])))
        except (ValueError, KeyError):
            continue
        dev = r["metadata_logfilename"].split("_", 2)[-1].rsplit(".", 1)[0][-9:]
        kinds[r["metadata_recordtype"]] += 1
        per[(r["coredata_id"], r["metadata_recordtype"], dev)].append(x)
    print("record types", dict(kinds))
    # Wyoming is the host OBU's own log (bsmSource EV): a msgCnt gap is a BSM the OBU did not
    # generate or did not log, not a radio loss. Every stream is locked, so the phase of a
    # slot is its 50 ms grid; loss is reported by grid, record type, device and hour.
    nfree = sum(free(sorted(set(v))) for v in per.values())
    print("free streams %d of %d (Wyoming is all locked: the 10-30 ms window is empty)" % (nfree, len(per)))
    g = collections.defaultdict(lambda: [0, 0, set()])
    for (i, kind, dev), v in per.items():
        v = sorted(set(v))
        for ph, lost, t in slots(v, 9):
            grid = ":00" if (ph + 25) % 100 < 50 else ":50"
            for k in ("all", "grid " + grid, kind, "device " + dev, "hour %s UTC" % hour(t)):
                g[k][0] += 1
                g[k][1] += lost
                g[k][2].add(i)
    print("| group | IDs | slots | msgCnt-attested missing share |")
    print("|---|---|---|---|")
    for k in sorted(g):
        print("| %s | %d | %d | %.4f |" % (k, len(g[k][2]), g[k][0], g[k][1] / g[k][0]))


def nyc(path):
    """Phase of a remote BSM's slot relative to the host's own BSM nearest before it. The host
    is the ID present for the whole event with the most records (NYC events are HV-centred)."""
    rel = collections.defaultdict(lambda: [0, 0])
    acc = Acc()
    for r in csv.DictReader(open(path)):
        by = collections.defaultdict(list)
        for b in json.loads(r["bsmlist"]):
            cd = b["bsmRecord"]["bsmMsg"]["coreData"]
            by[cd["id"]].append((round(cd["T_s"] * 1000), cd["msgCnt"]))
        if len(by) < 2:
            continue
        host = max(by, key=lambda k: len(by[k]))
        hph = sorted(t for t, _ in by[host])
        for k, v in by.items():
            if k == host:
                continue
            v.sort()
            for a, c in zip(v, v[1:]):
                dm = (c[1] - a[1]) % 128
                ds = c[0] - a[0]
                if dm == 0 or dm > 9 or abs(ds - 100 * dm) > 60:  # 1 Hz decimation out
                    continue
                for j in range(1, dm + 1):
                    t = a[0] + ds * j / dm
                    hs = [h for h in hph if abs(h - t) < 100]
                    if not hs:
                        continue
                    d = int(min(((t - h) % 100) for h in hs))
                    rel[d // 10][0] += 1
                    rel[d // 10][1] += j < dm
                    acc.add(r[":id"] + k, int(t % 100), j < dm)
    print("NYC remote-BSM loss by phase relative to the host's own BSM (10 ms bins):")
    print(" ".join("%d:%.3f(n=%d)" % (k * 10, rel[k][1] / rel[k][0], rel[k][0]) for k in sorted(rel)))
    print(HEAD)
    print(acc.row("NYC remote, phase vs event origin (not UTC)"))


if __name__ == "__main__":
    {"tampa": tampa, "wyoming": wyoming, "nyc": nyc}[sys.argv[1]](sys.argv[2])
