"""Feasibility check: SM-MBR v1.0 RandomPosition residual on public CV-pilot / Utah BSM samples.

residual = | p1 + v1*dt*(sin h1, cos h1) - p2 |  (E,N metres; heading clockwise from north)
over consecutive BSMs of one temporary ID, dt from secMark (generation time) where present.
Also: trapezoid residual (mean of v and h of both BSMs) to separate kinematic-model error
from position noise; ConstantPosition (speed >= 0.02 m/s, move < 1 cm); MaxAcceleration
(|dv|/dt > 10 m/s^2). Usage: python3 -I mbr_residual.py <data_dir>
"""
import csv, json, math, sys, os
from collections import defaultdict

D = sys.argv[1]
R = 6371008.8
GAP = 0.5


def fnum(s):
    try:
        return float(s)
    except (TypeError, ValueError):
        return None


def enu(lat, lon, lat0, lon0):
    return ((lon - lon0) * math.radians(1) * R * math.cos(math.radians(lat0)),
            (lat - lat0) * math.radians(1) * R)


def pct(xs, q):
    if not xs:
        return float('nan')
    xs = sorted(xs)
    k = (len(xs) - 1) * q
    f = math.floor(k); c = min(f + 1, len(xs) - 1)
    return xs[f] + (xs[c] - xs[f]) * (k - f)


def secmark_dt(s1, s2, tgap):
    """dt from secMark (ms in minute), disambiguated by a coarse wall-clock gap."""
    if s1 is None or s2 is None or s1 >= 60000 or s2 >= 60000:
        return None
    d = (s2 - s1) % 60000
    if tgap is not None and abs(d / 1000 - tgap) > 30:
        return None
    return d / 1000.0


def iso(t):
    # 'YYYY-MM-DDTHH:MM:SS.sss' -> seconds of the epoch-ish (only differences used)
    import datetime
    t = t.replace('Z', '')
    for fmt in ('%Y-%m-%dT%H:%M:%S.%f', '%Y-%m-%dT%H:%M:%S'):
        try:
            return datetime.datetime.strptime(t[:26], fmt).timestamp()
        except ValueError:
            pass
    return None


# each record: (id, t_wall, secmark_ms, x, y, speed, heading_deg, acc_major_m, msgcnt)
def load_tampa():
    rows = list(csv.DictReader(open(os.path.join(D, 'tampa.csv'))))
    lat0, lon0 = 27.95, -82.45
    out, seen = [], set()
    for r in rows:
        lat = fnum(r['coredata_lat']); lon = fnum(r['coredata_long'])
        if lat is None or lon is None:
            continue
        key = (r['coredata_id'], r['coredata_secmark'], r['coredata_msgcnt'])
        if key in seen:
            continue
        seen.add(key)
        x, y = enu(lat * 1e-7, lon * 1e-7, lat0, lon0)
        sp = fnum(r['coredata_speed']); hd = fnum(r['coredata_heading']); am = fnum(r['coredata_accuracy_semimajor'])
        out.append((r['coredata_id'], iso(r['metadata_recordgeneratedat']), fnum(r['coredata_secmark']), x, y,
                    None if sp is None or sp >= 8191 else sp * 0.02,
                    None if hd is None or hd >= 28800 else hd * 0.0125,
                    None if am is None or am >= 255 else am * 0.05, fnum(r['coredata_msgcnt'])))
    return out, len(rows)


def load_wyoming():
    rows = list(csv.DictReader(open(os.path.join(D, 'wyoming.csv'))))
    lat0, lon0 = 41.3, -106.0
    out, seen = [], set()
    for r in rows:
        lat = fnum(r['coredata_position_lat']); lon = fnum(r['coredata_position_long'])
        if lat is None:
            continue
        key = (r['coredata_id'], r['coredata_secmark'], r['coredata_msgcnt'])
        if key in seen:
            continue
        seen.add(key)
        x, y = enu(lat, lon, lat0, lon0)
        out.append((r['coredata_id'], iso(r['metadata_generatedat']), fnum(r['coredata_secmark']), x, y,
                    fnum(r['coredata_speed']), fnum(r['coredata_heading']), fnum(r['coredata_accuracy_semimajor']),
                    fnum(r['coredata_msgcnt'])))
    return out, len(rows)


def load_orem():
    rows = list(csv.DictReader(open(os.path.join(D, 'utah_orem.csv'))))
    lat0, lon0 = 40.3, -111.7
    out, seen = [], set()
    for r in rows:
        lat = fnum(r['latitude']); lon = fnum(r['longitude'])
        if lat is None:
            continue
        key = (r['temp_id'], r['sec_mark'], r['msg_count'])
        if key in seen:
            continue
        seen.add(key)
        x, y = enu(lat * 1e-7, lon * 1e-7, lat0, lon0)
        sp = fnum(r['speed']); hd = fnum(r['heading']); am = fnum(r['semi_major_axis_accuracy'])
        out.append((r['temp_id'], iso(r['tmstp_utc']), fnum(r['sec_mark']), x, y,
                    None if sp is None or sp >= 8191 else sp * 0.02,
                    None if hd is None or hd >= 28800 else hd * 0.0125,
                    None if am is None or am >= 255 else am * 0.05, fnum(r['msg_count'])))
    return out, len(rows)


def load_pooled():
    rows = list(csv.DictReader(open(os.path.join(D, 'utah_pooled.csv'))))
    lat0, lon0 = 41.7, -111.8
    out, seen = [], set()
    for r in rows:
        lat = fnum(r['latitude']); lon = fnum(r['longitude'])
        if lat is None:
            continue
        key = (r['temp_id'], r['tmstp_utc'])
        if key in seen:
            continue
        seen.add(key)
        x, y = enu(lat, lon, lat0, lon0)
        sp = fnum(r['speed_kmph'])
        out.append((r['temp_id'], iso(r['tmstp_utc']), None, x, y, None if sp is None else sp / 3.6,
                    fnum(r['heading_degrees']), None, None))
    return out, len(rows)


def load_nyc():
    ev = json.load(open(os.path.join(D, 'nyc_events.json')))
    out, n = [], 0
    for ei, e in enumerate(ev):
        bl = e['bsmlist']; bl = json.loads(bl) if isinstance(bl, str) else bl
        seen = set()
        for b in bl:
            n += 1
            cd = b['bsmRecord']['bsmMsg']['coreData']
            vid = f"{ei}:{cd['id']}"
            key = (vid, cd['T_s'])
            if key in seen:
                continue
            seen.add(key)
            acc = cd.get('accuracy', {}).get('semiMajor')
            out.append((vid, cd['T_s'], None, cd['X_m'], cd['Y_m'], cd.get('speed_mps'), cd.get('heading_deg'),
                        None if acc is None or acc >= 255 else acc * 0.05, cd.get('msgCnt')))
    return out, n


def analyse(name, recs, nraw):
    by = defaultdict(list)
    for r in recs:
        by[r[0]].append(r)
    dts, pairs, cpos, maxacc, cnt_ok, cnt_n = [], [], 0, 0, 0, 0
    for vid, rs in by.items():
        rs.sort(key=lambda r: (r[1] if r[1] is not None else 0))
        for a, b in zip(rs, rs[1:]):
            tgap = None if a[1] is None or b[1] is None else b[1] - a[1]
            dt = secmark_dt(a[2], b[2], tgap) if a[2] is not None else tgap
            if dt is None or dt <= 0:
                continue
            dts.append(dt)
            if a[8] is not None and b[8] is not None:
                cnt_n += 1
                cnt_ok += ((b[8] - a[8]) % 128 == 1)
            if dt > GAP or a[5] is None or a[6] is None:
                continue
            h1 = math.radians(a[6])
            px, py = a[3] + a[5] * dt * math.sin(h1), a[4] + a[5] * dt * math.cos(h1)
            res = math.hypot(b[3] - px, b[4] - py)
            trap = float('nan')
            if b[5] is not None and b[6] is not None:
                dh = ((b[6] - a[6] + 180) % 360) - 180
                hm = math.radians(a[6] + dh / 2); vm = (a[5] + b[5]) / 2
                trap = math.hypot(b[3] - (a[3] + vm * dt * math.sin(hm)), b[4] - (a[4] + vm * dt * math.cos(hm)))
                if abs(b[5] - a[5]) / dt > 10:
                    maxacc += 1
            move = math.hypot(b[3] - a[3], b[4] - a[4])
            if a[5] >= 0.02 and b[5] is not None and b[5] >= 0.02 and move < 0.01:
                cpos += 1
            pairs.append((dt, a[5], res, trap, a[7], vid))
    n = len(pairs)
    print(f"\n=== {name}: raw rows {nraw}, unique BSMs {len(recs)}, IDs {len(by)}")
    if not dts:
        print('no pairs'); return
    near100 = sum(1 for d in dts if 0.09 <= d <= 0.11)
    print(f"consecutive same-ID gaps: {len(dts)}; dt in [90,110] ms: {near100} ({100*near100/len(dts):.1f}%); "
          f"dt<=0.5 s: {sum(1 for d in dts if d<=GAP)} ({100*sum(1 for d in dts if d<=GAP)/len(dts):.1f}%); "
          f"dt p50 {pct(dts,.5)*1000:.0f} ms p90 {pct(dts,.9)*1000:.0f} ms")
    if cnt_n:
        print(f"msgCnt +1 between consecutive kept BSMs: {cnt_ok}/{cnt_n} ({100*cnt_ok/cnt_n:.1f}%)")
    if not n:
        return
    res = [p[2] for p in pairs]; tr = [p[3] for p in pairs if not math.isnan(p[3])]
    over = sum(1 for r in res if r > 1.0)
    print(f"pairs judged (dt<=0.5 s): {n}; RandomPosition residual p50 {pct(res,.5):.3f} p90 {pct(res,.9):.3f} "
          f"p99 {pct(res,.99):.3f} max {max(res):.2f} m; >1 m: {over} ({100*over/n:.2f}%), "
          f"IDs with >=1 trigger: {len(set(p[5] for p in pairs if p[2]>1))}/{len(set(p[5] for p in pairs))}")
    if tr:
        print(f"  trapezoid residual p50 {pct(tr,.5):.3f} p99 {pct(tr,.99):.3f} m; >1 m: {sum(1 for r in tr if r>1)}")
    print(f"  ConstantPosition triggers: {cpos}; MaxAcceleration triggers: {maxacc}")
    for lo, hi in ((0, .15), (.15, .25), (.25, .5)):
        s = [p[2] for p in pairs if lo < p[0] <= hi]
        if s:
            print(f"  dt ({lo*1000:.0f},{hi*1000:.0f}] ms: n {len(s)}, p50 {pct(s,.5):.3f} p99 {pct(s,.99):.3f}, >1 m {100*sum(1 for r in s if r>1)/len(s):.2f}%")
    for lo, hi in ((-1, .5), (.5, 5), (5, 15), (15, 99)):
        s = [p[2] for p in pairs if lo < p[1] <= hi]
        if s:
            print(f"  speed ({max(lo,0)},{hi}] m/s: n {len(s)}, p50 {pct(s,.5):.3f} p99 {pct(s,.99):.3f}, >1 m {100*sum(1 for r in s if r>1)/len(s):.2f}%")
    acc = [p[4] for p in pairs if p[4] is not None]
    if acc:
        vals = sorted(set(round(a, 2) for a in acc))
        print(f"  stated semiMajor (m): p50 {pct(acc,.5):.2f}, distinct values {vals[:12]}{'...' if len(vals)>12 else ''}")


for name, fn in (('Tampa THEA sample (RSU-received)', load_tampa), ('Wyoming one-day sample (OBU log, sanitised)', load_wyoming),
                 ('Utah Orem UDOT/Panasonic (RSU-received)', load_orem), ('Utah pooled-fund 10 Hz trajectories', load_pooled),
                 ('NYC EVENT records (obfuscated XY)', load_nyc)):
    recs, nraw = fn()
    analyse(name, recs, nraw)
