"""Reads one finished engine run into the surrogate's compact run summary.

The summary keeps what the surrogate needs and nothing else: the live windows, each BSM
pseudonym's device class (the secMark-phase classifier of tools/realdata/devclass.py,
P1-E2, read from the bytes on the air, so no ground truth), the exposure in seconds per
(window, class, corridor), every detector verdict and every report's life, the MA's
receipts and the cases. Inputs (record level `events` or richer, and
`security.scms.report_records: true`): `recording.mcap` (node.tx, det.observation,
pki.report_fate, ma.received, ma.case, pki.case, proto.revocation), `run-report.json`,
`scenario.resolved.yaml`. Needs the `mcap` package.

The pass reads `pki.report_fate` (NODE_AND_GT) only for the report's fate and instant, never
its ground-truth join; nothing here reads a GT channel.
"""
import collections
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'realdata'))
import devclass  # noqa: E402  (the frozen P1-E2 thresholds)

NS = 1_000_000_000
BSM_PERIOD_S = 0.1          # J2945/1 10 Hz: one BSM stands for 0.1 s of exposure
# Corridors (ASSUMPTION, stated): Manhattan's avenues run 29 degrees east of true north
# (the 1811 Commissioners' grid); a corridor is a band of the cross-avenue coordinate,
# one avenue block wide (Midtown's avenue spacing is about 250-280 m).
GRID_DEG = 29.0
BAND_M = 250.0


def corridor_of(x, y, grid_deg=GRID_DEG, band_m=BAND_M):
    """The avenue band of a world-frame point (x east, y north, metres)."""
    a = math.radians(grid_deg)
    s = x * math.cos(a) - y * math.sin(a)       # across the avenues
    return f'av{int(math.floor(s / band_m)):+d}'


def live_windows(duration_s, fast_forward):
    """[0, duration] less the fast-forward spans, as [(a, b)] in seconds."""
    out, t = [], 0.0
    for ff in sorted(fast_forward, key=lambda f: f[0]):
        if ff[0] > t:
            out.append((t, ff[0]))
        t = max(t, ff[1])
    if t < duration_s:
        out.append((t, duration_s))
    return out


def window_of(t_s, windows):
    for i, (a, b) in enumerate(windows):
        if a <= t_s <= b:
            return i
    return None


def _resolved(run_dir):
    txt = open(os.path.join(run_dir, 'scenario.resolved.yaml')).read()
    dur = float(re.search(r'\n\s+duration_s:\s*([0-9.]+)', txt).group(1))
    ff = [(float(a), float(b)) for a, b in re.findall(
        r'from_s:\s*([0-9.]+)\s*\n?\s*,?\s*to_s:\s*([0-9.]+)', txt)]
    t0 = re.search(r"\n\s+t0:\s*'?\"?([0-9T:\-]+)", txt).group(1)
    return dur, ff, t0


def _param(rep_txt, key, default):
    m = re.search(r'"%s":\s*([0-9.]+)' % re.escape(key), rep_txt)
    return float(m.group(1)) if m else default


def summarise(run_dir, grid_deg=GRID_DEG, band_m=BAND_M):
    from mcap.reader import make_reader
    dur, ff, t0 = _resolved(run_dir)
    windows = live_windows(dur, ff)
    rep_path = os.path.join(run_dir, 'run-report.json')
    rep_txt = open(rep_path).read() if os.path.exists(rep_path) else '{}'
    secmarks = collections.defaultdict(list)          # pseudonym -> secMarks (first 400)
    expo = collections.Counter()                      # (pseudonym, window, corridor) -> BSMs
    last_pos = {}                                     # pseudonym -> corridor of its latest BSM
    verdicts = []                                     # (t_s, detector, subject, corridor)
    fates = collections.defaultdict(list)             # report -> [(t_s, fate)]
    subject_of, corr_of_rep = {}, {}
    relayed = set()
    ma = []                                           # (t_s, ma, set_aside)
    stored = []                                       # (t_s, 'stored', subject, corridor)
    cases = 0
    with open(os.path.join(run_dir, 'recording.mcap'), 'rb') as f:
        for _, ch, m in make_reader(f).iter_messages():
            t = ch.topic[7:] if ch.topic.startswith('record/') else ch.topic
            if t == 'node.tx':
                q = json.loads(m.data)
                if q.get('report_id'):
                    relayed.add(q['report_id'])
                if q.get('msg_type') != 'bsm':
                    continue
                p, c = q.get('pseudonym'), q.get('content') or {}
                if not p or c.get('claimed_x_m') is None:
                    continue
                if len(secmarks[p]) < 400 and c.get('sec_mark_ms') is not None:
                    secmarks[p].append(c['sec_mark_ms'])
                k = corridor_of(c['claimed_x_m'], c['claimed_y_m'], grid_deg, band_m)
                last_pos[p] = k
                w = window_of(q['t'] / NS, windows)
                if w is not None:
                    expo[(p, w, k)] += 1
            elif t == 'det.observation':
                q = json.loads(m.data)
                s = q['subject']
                verdicts.append((q['t'] / NS, q['detector'], s, last_pos.get(s)))
            elif t == 'pki.report_fate':
                q = json.loads(m.data)
                r = q['report_id']
                fates[r].append((q['t'] / NS, q['fate']))
                if r not in subject_of:
                    subject_of[r] = q['subject']
                    corr_of_rep[r] = last_pos.get(q['subject'])
                if q['fate'] == 'stored':
                    stored.append((q['t'] / NS, 'stored', q['subject'], last_pos.get(q['subject'])))
            elif t == 'ma.received':
                q = json.loads(m.data)
                ma.append((q['t'] / NS, q['ma'], q.get('set_aside')))
            elif t in ('ma.case', 'pki.case'):
                cases += 1
    # The trigger stream: the detector verdicts (det.observation) when the recording has
    # them, else each report put in the device's store (this build's events-level
    # recordings carry no det.observation; the run report's verdicts_fired is the total).
    trigger = 'det.observation' if verdicts else 'stored'
    if not verdicts:
        verdicts = stored
    cls = {p: devclass.classify(*devclass.locked_fraction(v)) for p, v in secmarks.items()}
    exposure = collections.defaultdict(float)
    for (p, w, k), n in expo.items():
        exposure[f'{w}|{cls.get(p, "unclassified")}|{k}'] += n * BSM_PERIOD_S
    V = [dict(t=t, w=window_of(t, windows), det=d, cls=cls.get(s, 'unclassified'), corr=k)
         for t, d, s, k in verdicts]
    # Per subject: its class, exposure and verdicts, for the per-device dispersion fit.
    subj = collections.defaultdict(lambda: [0.0, 0])
    for (p, w, k), n in expo.items():
        subj[p][0] += n * BSM_PERIOD_S
    for t, d, s, k in verdicts:
        if s in subj:
            subj[s][1] += 1
    subjects = sorted([cls.get(p, 'unclassified'), round(e, 1), n] for p, (e, n) in subj.items())
    R = []
    for r, fl in fates.items():
        fl.sort()
        s = subject_of[r]
        d = dict(id=r, cls=cls.get(s, 'unclassified'), corr=corr_of_rep[r], relay=r in relayed)
        for tt, fa in fl:
            d.setdefault(fa, tt)
        R.append(d)
    R.sort(key=lambda d: (d.get('stored', 0), d['id']))
    return {
        'schema': 'v2xw.surrogate.run.v1', 'run': os.path.basename(run_dir.rstrip('/')),
        't0': t0, 'duration_s': dur, 'windows': windows,
        'corridors': {'grid_deg': grid_deg, 'band_m': band_m},
        'exposure_s': dict(sorted(exposure.items())),
        'verdicts': V, 'reports': R, 'subjects': subjects, 'ma': sorted(ma, key=lambda x: (x[0], x[1])),
        'cases': cases, 'trigger': trigger,
        'verdicts_fired': int(_param(rep_txt, 'verdicts_fired', 0)),
        'not_filed': int(_param(rep_txt, 'reports_not_filed', 0)),
        'shuffle': {'window_s': _param(rep_txt, 'report_shuffle_window_s', 86400.0),
                    'threshold': _param(rep_txt, 'report_shuffle_threshold', 10000.0)},
    }
