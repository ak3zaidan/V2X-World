"""Checks of the reference ML baseline (WP-24 P24.3).

  python3 -I tools/realworld/test_veremi_baseline.py           # needs numpy and scikit-learn
                                                                  # (xgboost optional), see the guide
"""
import csv
import json
import os
import random
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import veremi_baseline as vb  # noqa: E402


def beacon(t, x, vx, pseudo="aa", mid=1):
    return {"type": 3, "pos": [x, 0.0, 0.0], "spd": [vx, 0.0, 0.0], "hed": [1.0, 0.0, 0.0],
            "sendTime": t, "rcvTime": t + 0.002, "senderPseudo": pseudo, "messageID": mid,
            "pos_noise": [3.0, 3.0, 0.0]}


def write_export(d, attacker_type):
    """One receiver standing at the origin; one sender driving at 20 m/s, 1 Hz."""
    with open(os.path.join(d, "traceJSON-0-0-A0-0-0.json"), "w") as f:
        f.write(json.dumps({"type": 2, "pos": [0.0, 0.0, 0.0], "rcvTime": 0.0}) + "\n")
        for k in range(6):
            f.write(json.dumps(beacon(1.0 + k, 50.0 + 20.0 * k, 20.0, mid=k + 1)) + "\n")
    with open(os.path.join(d, "traceGroundTruthJSON-0.json"), "w") as f:
        for k in range(6):
            f.write(json.dumps({"type": 4, "messageID": k + 1, "attackerType": attacker_type}) + "\n")


def synthetic_csv(path, n, seed):
    """Two classes of feature rows with overlapping scores."""
    rng = random.Random(seed)
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["receiver", "messageID", "label"] + vb.feature_names())
        for i in range(n):
            lab = 1 if rng.random() < 0.3 else 0
            vec = [round(abs(rng.gauss(1.2 if lab and j % 11 == 4 else 0.4, 0.4)), 3)
                   for j in range(len(vb.feature_names()) - 1)] + [2]
            w.writerow([0, i, lab] + vec)


class Checks(unittest.TestCase):
    def test_a_frozen_claim_fails_position_speed_and_an_honest_one_passes(self):
        prev = beacon(1.0, 50.0, 20.0)
        honest = vb.check_scores(beacon(2.0, 70.0, 20.0), prev, [0.0, 0.0], [])
        frozen = vb.check_scores(beacon(2.0, 50.0, 20.0), prev, [0.0, 0.0], [])
        self.assertLess(honest["pos_speed"], 1.0)
        self.assertLess(honest["cv_predict"], 1.0)
        self.assertGreater(frozen["pos_speed"], 1.0)
        self.assertGreater(frozen["cv_predict"], 1.0)

    def test_features_never_read_the_ground_truth(self):
        rows = []
        for code in (0, 1):
            with tempfile.TemporaryDirectory() as d:
                write_export(d, code)
                out = os.path.join(d, "f.csv")
                vb.cmd_features(d, out)
                with open(out) as f:
                    rows.append([r[3:] for r in csv.reader(f)][1:])
                    f.seek(0)
                    labs = {r[2] for r in list(csv.reader(f))[1:]}
                self.assertEqual(labs, {str(code)})
        self.assertEqual(rows[0], rows[1])

    def test_the_window_holds_this_message_and_the_two_before(self):
        with tempfile.TemporaryDirectory() as d:
            write_export(d, 0)
            out = os.path.join(d, "f.csv")
            vb.cmd_features(d, out)
            with open(out) as f:
                hist = [int(r[-1]) for r in list(csv.reader(f))[1:]]
        self.assertEqual(hist, [0, 1, 2, 2, 2, 2])


class Training(unittest.TestCase):
    def test_the_recipe_is_reproducible_from_its_seed(self):
        with tempfile.TemporaryDirectory() as d:
            tr, te = os.path.join(d, "tr.csv"), os.path.join(d, "te.csv")
            synthetic_csv(tr, 1500, 1)
            synthetic_csv(te, 500, 2)
            a = vb.train([tr], [te], seed=0)
            b = vb.train([tr], [te], seed=0)
            for k in ("trees", "mlp"):
                self.assertEqual(a["models"][k]["hash"], b["models"][k]["hash"], k)
                self.assertEqual(a["models"][k]["scores"], b["models"][k]["scores"], k)
            c = vb.train([tr], [te], seed=1)
            self.assertNotEqual(a["models"]["mlp"]["hash"], c["models"]["mlp"]["hash"],
                                "the seed must reach the MLP")
            self.assertNotEqual(a["models"]["trees"]["hash"], c["models"]["trees"]["hash"],
                                "the seed must reach the trees")

    def test_scores_match_hand_counts(self):
        s = vb.scores([1, 1, 0, 0, 0], [1, 0, 0, 0, 1])
        self.assertEqual((s["tp"], s["fn"], s["tn"], s["fp"]), (1, 1, 2, 1))
        self.assertAlmostEqual(s["bm"], 0.5 + 2 / 3 - 1, places=6)
        self.assertAlmostEqual(s["mk"], 0.5 + 2 / 3 - 1, places=6)
        self.assertAlmostEqual(s["mcc"], (2 - 1) / (2 * 2 * 3 * 3) ** 0.5, places=6)


if __name__ == "__main__":
    unittest.main()
