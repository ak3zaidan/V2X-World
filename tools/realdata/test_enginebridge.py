"""Unit tests: python3 -I -m unittest discover -s tools/realdata -p 'test_enginebridge.py'"""
import io
import json
import math
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stdout

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import enginebridge as eb  # noqa: E402
import pairing  # noqa: E402

PS = "aabbccddeeff0011"
SCEN = """detection:
  local:
  - id: detect/us-sm-mbr
    params:
      consecutive: received
      time_base: secmark
      max_gap_s: 0.1
"""


def fixture(d, n=20, v=10.0, jump_at=None, brake_at=None):
    """One honest vehicle heading north at v m/s, 10 Hz, secMark on the 100 ms grid; one
    receiver (node 7) hears all of it. Optional: a 3 m jump the engine also flagged, and a
    braked BSM with positive accelSet."""
    os.makedirs(os.path.join(d, "jsonl"))
    lat0, lon0 = 40.75, -73.98
    tx, rx, obs = [], [], []
    for i in range(n):
        t = 1_000_000_000 + i * 100_000_000
        north = v * i * 0.1 + (3.0 if i == jump_at else 0.0)
        c = {"temp_id": "t1", "sec_mark_ms": (1000 + i * 100) % 60000, "msg_count": i % 128,
             "lat_deg": lat0 + math.degrees(north / pairing.R_EARTH), "lon_deg": lon0,
             "speed_mps": v, "heading_deg": 0.0, "accel_long_mps2": 0.5 if i == brake_at else 0.0,
             "brakes": 2 if i == brake_at else 0}
        tx.append({"msg": i, "node": 1, "pseudonym": PS, "msg_type": "bsm", "t": t, "t_generated": t, "content": c})
        rx.append({"msg": i, "tx": 1, "rx": 7, "msg_type": "bsm", "outcome": "delivered", "verification": "valid",
                   "t_delivered": t + 2_000_000, "t_verify_done": t + 2_000_000})
    if jump_at is not None:
        for j in (jump_at, jump_at + 1):
            obs.append({"t": rx[j]["t_delivered"], "node": 7, "detector": "positionJump", "subject": PS})
    if brake_at is not None:
        obs.append({"t": rx[brake_at]["t_delivered"], "node": 7, "detector": "brakeAcceleration", "subject": PS})
    for name, rows in (("node_tx", tx), ("node_rx", rx), ("det_observation", obs)):
        with open(os.path.join(d, "jsonl", name + ".jsonl"), "w") as f:
            f.writelines(json.dumps(r) + "\n" for r in rows)
    with open(os.path.join(d, "scenario.resolved.yaml"), "w") as f:
        f.write(SCEN)


def run(d, *args):
    with redirect_stdout(io.StringIO()):
        return eb.main([d, *args])


def total(out, check):
    return sum(r[check]["n"] for r in out["rows"] if r["class"] == "all")


class Bridge(unittest.TestCase):
    def test_honest_trace_is_quiet_and_rated(self):
        with tempfile.TemporaryDirectory() as d:
            fixture(d)
            out = run(d)
            self.assertEqual(out["judged_pairs"], 19)
            self.assertEqual(out["id_classes"], {"locked": 1})
            self.assertTrue(all(total(out, c) == 0 for c in eb.mc.CHECKS))
            row = [r for r in out["rows"] if r["class"] == "all"][0]
            self.assertEqual(row["band"], "10-20")
            self.assertAlmostEqual(row["id_minutes"], 19 * 0.1 / 60, places=4)

    def test_injected_3m_fault_raises_rp10(self):
        with tempfile.TemporaryDirectory() as d:
            fixture(d)
            base = total(run(d), "rp10")
            out = run(d, "--inject", "auto")
            self.assertIsNotNone(out["injected_msg"])
            self.assertGreater(total(out, "rp10"), base)
            self.assertEqual(total(out, "rp10"), 2)  # the jump in and the jump back

    def test_every_clock_judges(self):
        with tempfile.TemporaryDirectory() as d:
            fixture(d)
            for clock in eb.CLOCKS:
                self.assertEqual(run(d, "--clock", clock)["judged_pairs"], 19, clock)
            self.assertEqual(run(d, "--side", "rx")["judged_pairs"], 19)

    def test_k2_joins_engine_and_bridge_verdicts(self):
        with tempfile.TemporaryDirectory() as d:
            fixture(d, jump_at=10, brake_at=5)
            k = run(d)["k2"]
            self.assertTrue(k["joined"])
            self.assertEqual(k["obs_unjoined"], 0)
            self.assertEqual(k["agreement"]["rp10"], {"both": 2, "engine_only": 0, "bridge_only": 0})
            self.assertEqual(k["agreement"]["brakeacc"], {"both": 1, "engine_only": 0, "bridge_only": 0})

    def test_k2_disagreement_is_visible(self):
        # an engine verdict the bridge does not reproduce must show as engine-only
        with tempfile.TemporaryDirectory() as d:
            fixture(d, jump_at=10)
            p = os.path.join(d, "jsonl", "det_observation.jsonl")
            with open(p, "a") as f:
                f.write(json.dumps({"t": 1_000_000_000 + 3 * 100_000_000 + 2_000_000, "node": 7,
                                    "detector": "implausibleSpeed", "subject": PS}) + "\n")
            k = run(d)["k2"]
            self.assertEqual(k["agreement"]["maxspeed"]["engine_only"], 1)

    def test_wheel_brakes_bit_order_is_the_engines(self):
        # BrakeAppliedStatus.0: 'unavailable' is 0x10, the four wheels 0x0F
        self.assertIs(eb.brakes_of({"brakes": 0x0F}), True)  # all wheels braked
        self.assertIs(eb.brakes_of({"brakes": 0x01}), True)  # rightRear
        self.assertIs(eb.brakes_of({"brakes": 0}), False)
        self.assertIsNone(eb.brakes_of({"brakes": 0x10}))

    def test_k2_requires_the_engine_msgcnt_pair(self):
        # a per-verdict row naming another predecessor is not agreement: the bridge's
        # verdict on that pair stays bridge-only and the mismatch is counted
        with tempfile.TemporaryDirectory() as d:
            fixture(d, jump_at=10)
            p = os.path.join(d, "jsonl", "det_observation.jsonl")
            rows = [json.loads(x) for x in open(p)]
            rows[0].update(msg_cnt=10, prev_msg_cnt=9, dt_s=0.1, residual=3.0)
            rows[1].update(msg_cnt=11, prev_msg_cnt=7, dt_s=0.1, residual=3.0)
            with open(p, "w") as f:
                f.writelines(json.dumps(r) + "\n" for r in rows)
            k = run(d)["k2"]
            self.assertEqual(k["agreement"]["rp10"], {"both": 1, "engine_only": 0, "bridge_only": 1})
            self.assertEqual(k["pair_mismatch"], 1)
            self.assertEqual([x["msg_cnt"] for x in k["disagreements"] if x["check"] == "rp10"], [(10, 11)])


if __name__ == "__main__":
    unittest.main()
