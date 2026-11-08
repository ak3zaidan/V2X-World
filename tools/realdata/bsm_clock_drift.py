"""Tampa-only free-class BSM clock: per-ID median of secMark dt - 100 ms over consecutive-msgCnt
pairs (IDs with >= 20 pairs), and each period minus its ID median, on the calibration's bins.
Prints the two `bsm_clock` lines in the calibration file's format."""
import csv, sys, collections, datetime as dt, statistics
rows = collections.defaultdict(list)
for r in csv.DictReader(open(sys.argv[1])):
    try:
        t = dt.datetime.fromisoformat(r["metadata_recordgeneratedat"]).timestamp()
        rows[r["coredata_id"]].append((t, int(r["coredata_secmark"]), int(float(r["coredata_msgcnt"]))))
    except Exception:
        pass


def near(s):
    m = s % 50
    return min(m, 50 - m) <= 1


med, dev = [], []
for v in rows.values():
    if len(v) < 10 or sum(near(x[1]) for x in v) / len(v) >= 0.3:
        continue
    v.sort()
    d = [((b[1] - a[1]) % 60000) - 100 for a, b in zip(v, v[1:]) if (b[2] - a[2]) % 128 == 1]
    d = [x for x in d if -100 < x < 400]
    if len(d) < 20:
        continue
    m = statistics.median(d)
    med.append(m)
    dev += [x - m for x in d]


def hist(xs, bins):
    c = [0] * (len(bins) - 1)
    for x in xs:
        for i in range(len(bins) - 1):
            if bins[i] <= x < bins[i + 1]:
                c[i] += 1
                break
    n = sum(c)
    return [round(k / n, 6) for k in c], n


b1 = list(range(-10, 51, 2))
b2 = list(range(-50, 51, 5))
p1, n1 = hist(med, b1)
p2, n2 = hist(dev, b2)
print("# Tampa only: %d free IDs with >= 20 pairs, per-ID median p50 %.1f ms; %d periods" % (
    len(med), statistics.median(med), n2))
print("      drift_ms: {dist: empirical, bins_ms: [%s], p: [%s]}" % (", ".join(map(str, b1)), ", ".join(map(str, p1))))
print("      period_dev_ms: {dist: empirical, bins_ms: [%s], p: [%s]}" % (", ".join(map(str, b2)), ", ".join(map(str, p2))))
