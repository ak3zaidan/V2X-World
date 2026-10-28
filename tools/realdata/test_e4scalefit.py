"""Unit tests of e4scalefit.py: python3 -I -m unittest discover tools/realdata -p 'test_e4scalefit.py'"""
import math
import random
import unittest

import e4scalefit as S
import e4stepfit as F
import normalise
import pairing


def device(tid, s, speeds, n=80, lat0=27.95, seed=0, noise_m=0.0):
    """A straight north-bound locked 10 Hz trace on the epoch: true speed from `speeds` (one per
    stretch of 20 BSMs), reported speed v (1 + s), optional white position noise."""
    rng = random.Random(seed)
    out, pos = [], 0.0
    for i in range(n):
        v = speeds[(i // 20) % len(speeds)]
        if i:
            pos += v * 0.1
        y = pos + rng.gauss(0, noise_m)
        lat = lat0 + math.degrees(y / pairing.R_EARTH)
        sm = (1000 + 100 * i) % 60000
        out.append(normalise.Bsm(tid, sm, i % 128, 1000.0 + 0.1 * i, 1000.0 + 0.1 * i, None, lat, -82.45,
                                 round(v * (1 + s) / 0.02) * 0.02, 0.0, 0.0, 0.5, 0.4, None))
    return (tid, out)


class ScaleTests(unittest.TestCase):
    def test_a_device_scale_error_is_read_back_with_its_sign(self):
        seqs = F.sequences([device("A", -0.004, [25.0]), device("B", 0.003, [25.0])])
        per = {tid: m for tid, m, _, _, _ in S.per_id_eps(seqs)}
        # reported v (1 + s): the device moves further than it says when s < 0, eps = -s
        self.assertAlmostEqual(per["A"], 0.004, delta=0.0006)
        self.assertAlmostEqual(per["B"], -0.003, delta=0.0006)

    def test_the_device_spread_takes_the_within_id_noise_out(self):
        per = [("a", 0.002, 0.001, 50, 20.0), ("b", -0.002, 0.001, 50, 20.0), ("c", 0.0, 0.001, 50, 20.0)]
        mu, sd, between, within = S.device_spread(per)
        self.assertAlmostEqual(mu, 0.0)
        self.assertAlmostEqual(between, 0.002)
        self.assertAlmostEqual(within, 0.001)
        self.assertAlmostEqual(sd, math.sqrt(0.002 ** 2 - 0.001 ** 2))

    def test_a_scale_device_is_told_from_a_bias_device(self):
        sc = F.sequences([device("S%d" % k, s, [11.0, 26.0], n=120, seed=k, noise_m=0.002)
                          for k, s in enumerate((-0.004, 0.003, 0.005))])
        r = S.scale_or_bias(sc)
        self.assertEqual(r["ids"], 3)
        self.assertGreater(r["loglik_scale_minus_bias"], 0.0)

    def test_corrected_takes_the_term_out(self):
        seqs = F.sequences([device("A", -0.004, [25.0])])
        per = S.per_id_eps(seqs)
        fixed = S.corrected(seqs, {per[0][0]: per[0][1]}, 0.0)
        self.assertLess(abs(S.per_id_eps(fixed)[0][1]), 1e-4)


class EmulatorTests(unittest.TestCase):
    def test_no_scale_draws_nothing_and_a_scale_widens_the_body_at_speed(self):
        # candidate E's core walk (bsm-stamp-steps-candidates.yaml tampa_chain)
        core = {"p": [0.765345, 0.234655], "along_m": [0.00376468, 0.00497894],
                "cross_m": [0.00261522, 0.0105702], "tau_s": 50.0}
        kw = {"chain": (0.33, 0.77), "walk": core}
        a, _ = F.emulate([34.4] * 10, 4000, seed=3, **kw)
        b, _ = F.emulate([34.4] * 10, 4000, seed=3, scale=None, **kw)
        self.assertEqual(a, b)  # the default path is unchanged, draw for draw
        c, _ = F.emulate([34.4] * 10, 4000, seed=3, scale=(0.0, 0.002), **kw)
        under = lambda r: sum(x < 0.0068 for x in r) / len(r)  # noqa: E731
        self.assertLess(under(c), under(a) - 0.03)


if __name__ == "__main__":
    unittest.main()
