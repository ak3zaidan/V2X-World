"""Unit tests of e4stepfit.py: python3 -m unittest discover tools/realdata -p 'test_e4stepfit.py'"""
import math
import random
import unittest

import e4stepfit as F
import normalise
import pairing


def stream(offsets, v=20.0, step_at=(), step_m=0.0, dv_at=(), dv=0.0, follow=True, lat0=27.95):
    """A straight north-bound 10 Hz trace whose secMark is epoch + offsets[i] ms; a lasting
    along step of step_m at each BSM in step_at; a lasting speed change dv at each BSM in dv_at
    (the position moving with it when follow)."""
    out = []
    pos = 0.0
    vel = v
    for i, o in enumerate(offsets):
        if i:
            pos += (vel if follow else v) * 0.1
        if i in step_at:
            pos += step_m
        if i in dv_at:
            vel += dv
        lat = lat0 + math.degrees(pos / pairing.R_EARTH)
        sm = (1000 + 100 * i + o) % 60000
        out.append(normalise.Bsm("X", sm, i % 128, 1000.0 + 0.1 * i, 1000.0 + 0.1 * i, None, lat, -82.45,
                                 round(vel / 0.02) * 0.02, 0.0, 0.0, 0.5, 0.4, None))
    return out


class SequenceTests(unittest.TestCase):
    def test_the_stamp_term_is_removed_and_the_ids_split(self):
        offs = [0, 0, -1, -1, 0, 0, 0, -1, 0, 0] * 6
        seqs = F.sequences([("A", stream(offs)), ("B", stream([0] * 60))])
        chain, fixed = F.split(seqs)
        self.assertEqual([s[0] for s in chain], ["A"])
        self.assertEqual([s[0] for s in fixed], ["B"])
        tr = F.transition_pairs(chain)
        self.assertTrue(tr)
        for x in tr:
            # raw along residual +v x 1 ms on a 0 -> -1 pair (position on the epoch); removed in "al"
            self.assertAlmostEqual(x["alr"], 0.020, delta=0.006)
            self.assertAlmostEqual(x["al"], 0.0, delta=0.006)

    def test_joint_pairs_label_by_the_first_state(self):
        offs = [0, 0, -1, -1, 0] * 8
        seqs = F.sequences([("A", stream(offs))])
        obs = F.joint_pairs(seqs)
        # per cycle: 0->0, 0->-1, -1->-1, -1->0, 0->0 (wrap): labels 0, 1, 2, 2, 0
        n = [sum(o[4] == k for o in obs) for k in range(3)]
        self.assertEqual(sum(n), len(offs) - 1)
        self.assertAlmostEqual(n[0] / sum(n), 0.4, delta=0.03)
        self.assertAlmostEqual(n[1] / sum(n), 0.2, delta=0.03)
        self.assertAlmostEqual(n[2] / sum(n), 0.4, delta=0.03)

    def test_chain_rates_are_the_transition_counts(self):
        offs = [0, 0, 0, -1, -1, 0] * 20
        seqs = F.sequences([("A", stream(offs))])
        s_m, s_0, c = F.chain_rates(seqs)
        self.assertAlmostEqual(s_m, c[(-1, -1)] / (c[(-1, -1)] + c[(-1, 0)]))
        self.assertAlmostEqual(s_0, c[(0, 0)] / (c[(0, 0)] + c[(0, -1)]))
        self.assertAlmostEqual(s_m, 0.5, delta=0.02)
        self.assertAlmostEqual(s_0, 0.75, delta=0.02)


def synth_transition_pairs(n, pi, a50, a90, c50, c90, walk, seed=3):
    rng = random.Random(seed)
    sa, sga = math.log(a50), math.log(a90 / a50) / F.Z90
    sc, sgc = math.log(c50), math.log(c90 / c50) / F.Z90
    out = []
    for _ in range(n):
        j = F.random_component(rng, walk)
        a = rng.gauss(0, walk["along_m"][j])
        c = rng.gauss(0, walk["cross_m"][j])
        if rng.random() < pi:
            a += math.copysign(math.exp(sa + sga * rng.gauss(0, 1)), rng.random() - 0.5)
            c += math.copysign(math.exp(sc + sgc * rng.gauss(0, 1)), rng.random() - 0.5)
        out.append({"al": a, "cr": c, "v": 25.0, "q2": 0.0})
    return out


WALK = {"p": [0.8, 0.2], "along_m": [0.003, 0.008], "cross_m": [0.003, 0.008], "tau_s": 50.0}


class StepFitTests(unittest.TestCase):
    def test_the_transition_fit_recovers_rate_and_size_including_small_steps(self):
        # a step law with much of its mass under 5 cm: the over-5-cm share is far below pi
        obs = synth_transition_pairs(400, 0.8, 0.04, 0.10, 0.02, 0.06, WALK)
        over5 = sum(math.hypot(x["al"], x["cr"]) > 0.05 for x in obs) / len(obs)
        self.assertLess(over5, 0.55)
        th, ll, ll0 = F.fit_steps(obs, WALK)
        self.assertAlmostEqual(th[0], 0.8, delta=0.07)
        a50, a90 = F.lognormal_q(th[1], th[2])
        self.assertAlmostEqual(a50, 0.04, delta=0.008)
        self.assertAlmostEqual(a90, 0.10, delta=0.025)
        self.assertGreater(ll, ll0 + 50)

    def test_no_steps_fits_a_small_rate(self):
        obs = synth_transition_pairs(300, 0.0, 0.04, 0.10, 0.02, 0.06, WALK)
        th, _, _ = F.fit_steps(obs, WALK)
        self.assertLess(th[0], 0.05)

    def test_the_joint_fit_finds_steps_only_from_state_zero(self):
        rng = random.Random(5)
        obs = []
        for lab, n, pi in ((0, 300, 0.10), (1, 120, 0.9), (2, 120, 0.0)):
            for x in synth_transition_pairs(n, pi, 0.06, 0.15, 0.03, 0.09,
                                            {"p": [1.0], "along_m": [0.004], "cross_m": [0.004]},
                                            seed=rng.randrange(1 << 30)):
                obs.append((x["al"], x["cr"], 1e-6, 1e-6, lab))
        f = F.fit_joint(obs, k=1, iters=900)
        self.assertAlmostEqual(f["prob_stay_zero"], 0.10, delta=0.05)
        self.assertAlmostEqual(f["prob_on_transition"], 0.9, delta=0.08)
        self.assertLess(f["prob_from_minus1"], 0.03)
        self.assertAlmostEqual(f["size"]["along_p50_m"], 0.06, delta=0.015)
        self.assertAlmostEqual(f["core"]["along_m"][0], 0.004, delta=0.0015)


class SpeedCouplingTests(unittest.TestCase):
    def _seqs(self, follow):
        ids = []
        for n in range(12):
            offs = ([0] * 9 + [-1]) * 4
            st = [9 + 10 * k for k in range(4)]
            size = 0.08 if n % 2 else -0.08
            ids.append((str(n), stream(offs, step_at=st, step_m=size, dv_at=st, dv=0.5 * size, follow=follow)))
        return F.split(F.sequences(ids))[0]

    def test_the_speed_change_and_whether_the_position_follows_it(self):
        th = (0.9, math.log(0.08), 0.3, math.log(0.01), 0.3)
        walk = {"p": [1.0], "along_m": [0.004], "cross_m": [0.004]}
        f = F.speed_coupling(self._seqs(True), th, walk)
        self.assertAlmostEqual(f["k_mps_per_m"], 0.5, delta=0.1)
        self.assertGreater(f["follow"]["follow_fraction"], 0.8)
        g = F.speed_coupling(self._seqs(False), th, walk)
        self.assertLess(g["follow"]["follow_fraction"], 0.2)


class EmulatorTests(unittest.TestCase):
    QUIET = {"p": [1.0], "along_m": [1e-5], "cross_m": [1e-5], "tau_s": 50.0}

    def test_the_chain_puts_v_times_1ms_on_transitions_only(self):
        res, lab = F.emulate([30.0], 4000, chain=(0.5, 0.5), walk=self.QUIET, extra_m=0.0, hop=(0.0, 1.0),
                             on=(0.0, 0.0))
        for r, ll in zip(res, lab):
            if ll[0] != ll[1]:
                self.assertAlmostEqual(r, 0.030, delta=0.012)
            else:
                self.assertLess(r, 0.012)

    def test_state_zero_steps_never_land_on_pairs_from_minus1(self):
        law = (1.0, 0.1, 0.2, 0.05, 0.1)
        res, lab = F.emulate([30.0], 6000, chain=(0.5, 0.5), step0=law, walk=self.QUIET, extra_m=0.0,
                             hop=(0.0, 1.0), on=(0.0, 0.0))
        big = {}
        for r, ll in zip(res, lab):
            big.setdefault(ll, []).append(r > 0.05)
        self.assertGreater(sum(big[(0, 0)]) / len(big[(0, 0)]), 0.9)
        for ll in ((-1, -1), (-1, 0), (0, -1)):
            self.assertEqual(sum(big[ll]), 0, ll)

    def test_a_speed_change_the_position_follows_leaves_no_residual(self):
        law = (1.0, 0.1, 0.1001, 1e-4, 1.001e-4)
        kw = dict(chain=(0.0, 0.9), step=law, walk=self.QUIET, extra_m=0.0, hop=(0.0, 1.0), on=(0.0, 0.0),
                  k_v=0.5, tau_v=1.0, lsb_m=1e-5)

        def after(follow):
            res, lab = F.emulate([30.0], 6000, follow=follow, **kw)
            # the pair after a stepped 0 -> -1 pair is -1 -> 0: its stamp term is v x 1 ms = 3 cm
            d = [abs(r - 0.030) for r, pl in zip(res[1:], lab) if pl == (0, -1)]
            return sum(d) / len(d)
        self.assertLess(after(1.0), 0.0015)  # the J2735 speed LSB: 0.01 m/s x 0.1 s
        # kept at the old speed: -dv * dt = 0.5 x 0.1 m x 0.1 s = 5 mm on the next pair
        self.assertAlmostEqual(after(0.0), 0.005, delta=0.0015)

    def test_ks(self):
        self.assertEqual(F.ks_at([1, 2, 3], [1, 2, 3])[0], 0.0)
        self.assertEqual(F.ks_at([1, 2], [3, 4])[0], 1.0)


if __name__ == "__main__":
    unittest.main()
