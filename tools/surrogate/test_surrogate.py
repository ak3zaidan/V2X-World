"""Unit tests: python3 -I -m unittest discover -s tools/surrogate"""
import json
import os
import random
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import records  # noqa: E402
import surrogate as sg  # noqa: E402

WINDOWS = [(0.0, 60.0), (43185.0, 43215.0), (172800.0, 172800.0)]


def synth(seed=1, n_free=60, n_locked=20, lam_free=0.05, lam_locked=0.002, scale=1.0,
          shuffle_w=86400.0):
    """A run summary drawn from a known model: free devices in corridor av+1 at lam_free,
    locked ones in av+2 at lam_locked; every verdict is stored; 90 % uploaded at once, 30 %
    of them relayed; the MA gets every upload one shuffle window after the first."""
    rng = random.Random(seed)
    expo, V, R, subj = {}, [], [], []
    for i in range(n_free + n_locked):
        cls, k, lam = ('free', 'av+1', lam_free) if i < n_free else ('locked', 'av+2', lam_locked)
        tot, nv = 0.0, 0
        for w, (a, b) in enumerate(WINDOWS[:2]):
            e = (b - a) * scale
            key = f'{w}|{cls}|{k}'
            expo[key] = expo.get(key, 0.0) + e
            tot += e
            t = a
            while True:
                t += rng.expovariate(lam)
                if t > a + e:
                    break
                V.append(dict(t=t, w=w, det='randomPosition', cls=cls, corr=k))
                nv += 1
                rid = f'r{len(R)}'
                r = dict(id=rid, cls=cls, corr=k, relay=rng.random() < 0.3, stored=t)
                if rng.random() < 0.9:
                    r['uploaded'] = t + 0.5
                R.append(r)
        subj.append([cls, tot, nv])
    V.sort(key=lambda v: v['t'])
    ups = sorted(r['uploaded'] for r in R if 'uploaded' in r)
    ma = [(ups[0] + shuffle_w + 0.2, 0, None) for _ in ups] if ups else []
    return {'schema': 'v2xw.surrogate.run.v1', 'run': f'synth{seed}', 't0': '2026-10-19T21:00:00',
            'duration_s': 172800.0, 'windows': WINDOWS,
            'corridors': {'grid_deg': 29.0, 'band_m': 250.0}, 'exposure_s': expo,
            'verdicts': V, 'reports': R, 'subjects': subj, 'ma': ma, 'cases': 0,
            'not_filed': 0, 'shuffle': {'window_s': shuffle_w, 'threshold': 10000.0}}


class Corridors(unittest.TestCase):
    def test_band_follows_the_grid(self):
        # a point along an avenue (29 deg east of north) stays in its band
        import math
        a = math.radians(29.0)
        k0 = records.corridor_of(10.0, 0.0)
        for d in (100.0, 500.0, 900.0):
            self.assertEqual(records.corridor_of(10.0 + d * math.sin(a), d * math.cos(a)), k0)
        # one band across the avenues is a different corridor
        self.assertNotEqual(records.corridor_of(10.0 + 260.0 * math.cos(a), -260.0 * math.sin(a)), k0)

    def test_live_windows_are_the_complement_of_fast_forward(self):
        w = records.live_windows(172800.0, [(60.0, 43185.0), (43215.0, 172800.0)])
        self.assertEqual(w, [(0.0, 60.0), (43185.0, 43215.0)])
        self.assertEqual(records.window_of(43200.0, w), 1)
        self.assertIsNone(records.window_of(1000.0, w))


class FitAndPredict(unittest.TestCase):
    def test_rates_recover_the_generating_model(self):
        r = sg.fit(synth(1, n_free=400, n_locked=200))
        self.assertAlmostEqual(r['lambda_per_s']['free|av+1']['randomPosition'], 0.05, delta=0.005)
        self.assertAlmostEqual(r['lambda_per_s']['locked|av+2']['randomPosition'], 0.002, delta=0.001)
        self.assertAlmostEqual(r['conversion']['free']['uploaded'], 0.9, delta=0.02)
        self.assertAlmostEqual(r['conversion']['free']['relay_share'], 0.3, delta=0.03)
        self.assertAlmostEqual(r['ma']['first_after_first_upload_s'], 86400.2, delta=0.01)

    def test_prediction_transfers_to_another_seed(self):
        cal, tgt = synth(1), synth(2)
        p = sg.predict(sg.fit(cal), tgt['exposure_s'], tgt['windows'])
        o = sg.observe(tgt)
        for m in ('x.e10.triggers', 'x.e10.stored', 'x.e10.uploaded', 'x.e10.ma_batch'):
            self.assertLess(abs(p[m] / o[m] - 1.0), 0.2, m)
        self.assertLess(abs(p['x.e10.ma_first_t'] - o['x.e10.ma_first_t']), 120.0)

    def test_exposure_scales_the_prediction(self):
        r = sg.fit(synth(1))
        tgt = synth(2, scale=2.0)
        p1 = sg.predict(r, synth(2)['exposure_s'], WINDOWS)
        p2 = sg.predict(r, tgt['exposure_s'], WINDOWS)
        self.assertAlmostEqual(p2['x.e10.triggers'] / p1['x.e10.triggers'], 2.0, places=6)

    def test_unseen_cell_falls_back_to_its_class(self):
        r = sg.fit(synth(1))
        p = sg.predict(r, {'0|free|av+9': 60.0}, WINDOWS)
        self.assertAlmostEqual(p['x.e10.triggers'], 60.0 * r['lambda_class_per_s']['free']['randomPosition'])

    def test_every_fault_moves_its_measure(self):
        cal, tgt = synth(1), synth(2)
        r = sg.fit(cal)
        moved = {'rate_x1.5': 'x.e10.triggers', 'no_shuffle': 'x.e10.ma_first_t',
                 'pool_class': 'x.e10.triggers.free_share', 'drop_window1': 'x.e10.triggers.w1',
                 'no_conversion': 'x.e10.uploaded'}
        mixed = {'0|free|av+1': 600.0, '0|locked|av+1': 600.0}   # both classes in one corridor
        for f, m in moved.items():
            ex = mixed if f == 'pool_class' else tgt['exposure_s']
            base = sg.predict(r, ex, WINDOWS)
            p = sg.predict(r, ex, WINDOWS, f)
            self.assertGreater(abs(p[m] - base[m]), 0.05 * max(1.0, abs(base[m])), f)

    def test_pooled_corridor_changes_the_profile_when_corridors_differ(self):
        r = sg.fit(synth(1))
        # a quiet corridor for the free class: a tenth of its pooled rate
        r['lambda_per_s']['free|av+3'] = {'randomPosition': 0.005}
        expo = {'0|free|av+1': 600.0, '0|free|av+3': 600.0}
        a = sg.predict(r, expo, WINDOWS)
        b = sg.predict(r, expo, WINDOWS, 'pool_corridor')
        self.assertGreater(sg.tvd(a['corridor_profile'], b['corridor_profile']), 0.3)
        self.assertAlmostEqual(b['corridor_profile']['av+3'], 0.5)

    def test_dispersion_detects_overdispersion(self):
        rng = random.Random(3)
        pois = [['free', 100.0, sum(1 for _ in range(1000) if rng.random() < 0.01)] for _ in range(300)]
        clump = [['free', 100.0, (40 if i % 10 == 0 else 0)] for i in range(300)]
        self.assertGreater(sg._dispersion(pois)['free'], 20.0)
        self.assertLess(sg._dispersion(clump)['free'], 0.5)


class Projection(unittest.TestCase):
    PROFILE = {'devices': 300, 'days': 4, 'seed': 7, 'drive_s_per_day': 120.0,
               'drive_hour_local': 8.0, 't0_hour_local': 17.0, 'quota_reports': 58,
               'k_rules': [1, 5, 50]}

    def test_deterministic(self):
        r = sg.fit(synth(1))
        a = json.dumps(sg.project(r, self.PROFILE), sort_keys=True)
        b = json.dumps(sg.project(r, self.PROFILE), sort_keys=True)
        self.assertEqual(a, b)

    def test_shuffle_delays_the_ma_by_a_window(self):
        r = sg.fit(synth(1))
        out = sg.project(r, self.PROFILE)
        self.assertEqual(out['per_day'][0]['ma'], 0)          # nothing reaches the MA on day 1
        self.assertGreater(out['per_day'][1]['ma'], 0)
        self.assertEqual(out['label'], 'analytical')

    def test_threshold_releases_early(self):
        # a fleet that files 10,000 reports in its first day is released that day
        out = sg.project(sg.fit(synth(1)), dict(self.PROFILE, drive_s_per_day=3600.0))
        self.assertGreater(out['per_day'][0]['ma'], 0)

    def test_rules_are_monotone_in_k(self):
        out = sg.project(sg.fit(synth(1)), self.PROFILE)
        b = [out['k_rules'][k]['honest_blocked'] for k in ('1', '5', '50')]
        self.assertGreaterEqual(b[0], b[1])
        self.assertGreaterEqual(b[1], b[2])

    def test_quota_caps_what_a_device_holds(self):
        r = sg.fit(synth(1))
        for cv in r['conversion'].values():
            cv['uploaded'] = 0.0                               # a device that never uploads
        out = sg.project(r, dict(self.PROFILE, quota_reports=5, drive_s_per_day=36000.0))
        self.assertGreater(sum(d['lost_quota'] for d in out['per_day']), 0)
        self.assertEqual(sum(d['uploaded'] for d in out['per_day']), 0)


class Summarise(unittest.TestCase):
    """records.summarise end to end on a small synthetic recording."""

    def test_classes_exposure_triggers_and_fates(self):
        import tempfile
        from mcap.writer import Writer
        d = tempfile.mkdtemp()
        open(os.path.join(d, 'scenario.resolved.yaml'), 'w').write(
            "time:\n  t0: '2026-10-19T21:00:00Z'\n  duration_s: 100.0\n  fast_forward:\n"
            "  - from_s: 20.0\n    to_s: 100.0\n")
        json.dump({'report': {'phase2': {'verdicts_fired': 7, 'reports_not_filed': 1}}},
                  open(os.path.join(d, 'run-report.json'), 'w'))
        rng = random.Random(5)
        with open(os.path.join(d, 'recording.mcap'), 'wb') as f:
            w = Writer(f)
            w.start()
            sid = w.register_schema('j', 'jsonschema', b'{}')
            ch = {t: w.register_channel('record/' + t, 'json', sid)
                  for t in ('node.tx', 'pki.report_fate', 'ma.received')}

            def put(t, ts, q):
                w.add_message(ch[t], int(ts * 1e9), json.dumps(q).encode(), int(ts * 1e9))
            for i in range(150):                       # 15 s of 10 Hz BSMs from two devices
                ts = i * 0.1
                put('node.tx', ts, {'t': int(ts * 1e9), 'node': 1, 'msg_type': 'bsm', 'pseudonym': 'aa',
                                    'content': {'sec_mark_ms': (i * 100) % 60000,
                                                'claimed_x_m': 10.0, 'claimed_y_m': 0.0}})
                put('node.tx', ts + 0.01, {'t': int((ts + 0.01) * 1e9), 'node': 2, 'msg_type': 'bsm',
                                           'pseudonym': 'bb',
                                           'content': {'sec_mark_ms': rng.randrange(60000),
                                                       'claimed_x_m': 600.0, 'claimed_y_m': 0.0}})
            for j in range(5):
                put('pki.report_fate', 16 + j, {'t': int((16 + j) * 1e9), 'report_id': f'r{j}', 'fate': 'stored',
                                                'subject': 'bb', 'reporter': 1})
                put('pki.report_fate', 16.5 + j, {'t': int((16.5 + j) * 1e9), 'report_id': f'r{j}',
                                                  'fate': 'uploaded', 'subject': 'bb', 'reporter': 1})
            put('node.tx', 17.0, {'t': int(17e9), 'node': 1, 'msg_type': 'mbr', 'report_id': 'r1'})
            put('ma.received', 90.0, {'t': int(90e9), 'report_id': 'r0', 'ma': 0, 'set_aside': None})
            w.finish()
        S = records.summarise(d)
        self.assertEqual(S['windows'], [(0.0, 20.0)])
        self.assertEqual(S['trigger'], 'stored')
        self.assertEqual(len(S['verdicts']), 5)
        self.assertTrue(all(v['cls'] == 'free' for v in S['verdicts']))
        self.assertEqual(S['verdicts'][0]['corr'], records.corridor_of(600.0, 0.0))
        ex = {k.split('|')[1]: v for k, v in S['exposure_s'].items()}
        self.assertAlmostEqual(ex['locked'], 15.0)
        self.assertAlmostEqual(ex['free'], 15.0)
        self.assertEqual(sum(1 for r in S['reports'] if r['relay']), 1)
        self.assertEqual(S['verdicts_fired'], 7)
        self.assertEqual(len(S['ma']), 1)


class Fill(unittest.TestCase):
    TABLE = """# head comment
schema: v2xw/scms-run-expectations/1
runs:
  X: {scenario: x}
  E10A: {scenario: a}
  E10S: {scenario: s}
rows:
- id: OTHER.row
  base: X
  variant: X
  measure: m
  expect: eq
  value: 1
  measured: pending
# a comment between rows
- id: P1.E10.a
  base: E10S
  variant: E10A
  measure: x.e10.triggers
  expect: ratio
  band: [0.75, 1.33]
  measured: pending
- id: P1.E10.b
  base: E10S
  variant: E10A
  measure: x.e10.triggers
  expect: ratio
  band: [0.75, 0.9]
  measured: pending
"""

    def test_fill_writes_only_its_rows_and_goes_red(self):
        import tempfile
        d = tempfile.mkdtemp()
        p = os.path.join(d, 't.yaml')
        open(p, 'w').write(self.TABLE)
        for run, v in (('E10A', 100.0), ('E10S', 105.0)):
            os.makedirs(os.path.join(d, run))
            json.dump({'x.e10.triggers': v}, open(os.path.join(d, run, 'realism.json'), 'w'))
        sg.fill(p, d)
        s = open(p).read()
        self.assertIn('# a comment between rows', s)
        self.assertEqual(s.count('measured: pending'), 1)        # OTHER.row untouched
        self.assertIn('verdict: pass', s)                        # 0.952 in [0.75, 1.33]
        self.assertIn('verdict: FAIL', s)                        # 0.952 outside [0.75, 0.9]


if __name__ == '__main__':
    unittest.main()
