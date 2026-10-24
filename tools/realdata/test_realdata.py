"""Unit tests: python3 -m unittest discover tools/realdata"""
import math
import os
import tempfile
import unittest

import bootci
import calib
import devclass
import fgate
import mbrchecks as mc
import normalise
import pairing
import pipeline
import soda
import sources


def trace(n=60, v=25.0, heading=90.0, slip_every=None, phase_ms=0, jitter=None, start_sm=1000):
    """A synthetic straight trace at 10 Hz: positions on fix epochs, secMark = epoch + phase."""
    out = []
    lat0, lon0 = 41.0, -106.0
    for i in range(n):
        t = i * 0.1
        k = i + (1 if slip_every and i % slip_every == 0 and i else 0)  # position from a later epoch
        d = v * k * 0.1
        h = math.radians(heading)
        lat = lat0 + math.degrees(d * math.cos(h) / pairing.R_EARTH)
        lon = lon0 + math.degrees(d * math.sin(h) / (pairing.R_EARTH * math.cos(math.radians(lat0))))
        sm = (start_sm + i * 100 + phase_ms + (jitter[i] if jitter else 0)) % 60000
        out.append(normalise.Bsm("X", sm, i % 128, 1000.0 + t + (jitter[i] if jitter else 0) / 1000.0,
                                 1000.0 + t, None, lat, lon, v, heading, 0.0, 0.5, 0.4, None))
    return out


class NormaliseTests(unittest.TestCase):
    def test_tampa_row_raw_j2735_units(self):
        r = {"metadata_recordgeneratedat": "2021-03-19T06:39:44.426", "coredata_id": "8007492",
             "coredata_secmark": "44481", "coredata_msgcnt": "29.0", "coredata_lat": "279448282.0",
             "coredata_long": "-824534088.0", "coredata_speed": "1403.0", "coredata_heading": "18361.0",
             "coredata_accelset_long": "-3.0", "coredata_accuracy_semimajor": "4.0",
             "coredata_accuracy_semiminor": "4.0", "coredata_wheelbrakes": ""}
        b = normalise.tampa_row(r)
        self.assertEqual((b.secmark, b.msgcnt), (44481, 29))
        self.assertAlmostEqual(b.lat, 27.9448282)
        self.assertAlmostEqual(b.lon, -82.4534088)
        self.assertAlmostEqual(b.speed, 28.06)
        self.assertAlmostEqual(b.heading, 229.5125)
        self.assertAlmostEqual(b.accel, -0.03)
        self.assertAlmostEqual(b.smaj, 0.2)
        self.assertIsNone(b.brakes)
        self.assertIsNotNone(b.t_rx)
        r["coredata_speed"] = "8191"
        r["coredata_heading"] = "28800"
        self.assertIsNone(normalise.tampa_row(r).speed)
        self.assertIsNone(normalise.tampa_row(r).heading)

    def test_orem_row_epoch_ms_and_stripped_brake_bits(self):
        r = {"tmstp_epoch": "1738481326447", "temp_id": "05075fde", "msg_count": "59", "sec_mark": "45328",
             "latitude": "402980537", "longitude": "-1116955028", "speed": "1038", "heading": "12944",
             "longitudinal_acceleration": "2001", "semi_major_axis_accuracy": "40",
             "semi_minor_axis_accuracy": "40", "wheel_brakes_flags": "1111"}
        b = normalise.orem_row(r)
        self.assertAlmostEqual(b.t_rx, 1738481326.447)
        self.assertAlmostEqual(b.speed, 20.76)
        self.assertAlmostEqual(b.heading, 161.8)
        self.assertIsNone(b.accel)  # 2001 = unavailable
        self.assertAlmostEqual(b.smaj, 2.0)
        self.assertTrue(b.brakes)  # '1111' = 01111: all four wheels applied
        self.assertIsNone(normalise.j_brakes("10000"))  # unavailable bit
        self.assertFalse(normalise.j_brakes("0"))
        self.assertTrue(normalise.j_brakes("1110"))

    def test_wyoming_row_si_units_and_secmark_sequencing(self):
        r = {"metadata_generatedat": "2022-06-30T12:50:19.859", "metadata_recordtype": "bsmLogDuringEvent",
             "coredata_id": "9ED1F74E", "coredata_secmark": "19800.0", "coredata_msgcnt": "86.0",
             "coredata_position_lat": "41.6329647", "coredata_position_long": "-106.2827627",
             "coredata_speed": "34.92", "coredata_heading": "130.325", "coredata_accuracy_semimajor": "0.5",
             "coredata_accuracy_semiminor": "0.4"}
        b = normalise.wyoming_row(r)
        self.assertEqual(b.secmark, 19800)
        self.assertAlmostEqual(b.speed, 34.92)
        self.assertAlmostEqual(b.heading, 130.325)
        self.assertIsNone(b.accel)
        self.assertIsNone(b.t_rx)
        # sequenced by its own secMark: 12:50:19.800, not the header's 19.859
        self.assertAlmostEqual(b.order - b.t_hdr, -0.059, places=6)

    def test_nyc_event_xy_metres(self):
        bl = ('[{"bsmRecord": {"bsmMsg": {"coreData": {"msgCnt": 101, "id": "9CA04A11", "accuracy": '
              '{"semiMajor": 5}, "accelSet": {"long_mpss": 1.43}, "brakes": {"wheelBrakes": "00"}, '
              '"X_m": 10.526, "Y_m": -59.009, "T_s": -6.553, "speed_mps": 4.3, "heading_deg": 354.85}}}}]')
        b = normalise.nyc_event_bsms("row-1", bl)[0]
        self.assertEqual(b.id, "row-1:9CA04A11")
        self.assertEqual((b.lon, b.lat), (10.526, -59.009))  # X east, Y north in metres
        self.assertAlmostEqual(b.order, -6.553)
        self.assertAlmostEqual(b.accel, 1.43)
        self.assertFalse(b.brakes)

    def test_stream_requires_contiguous_ids(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "w.csv")
            cols = ["metadata_generatedat", "metadata_recordtype", "coredata_id", "coredata_secmark",
                    "coredata_msgcnt", "coredata_position_lat", "coredata_position_long", "coredata_speed",
                    "coredata_heading", "coredata_accuracy_semimajor", "coredata_accuracy_semiminor"]
            with open(p, "w") as f:
                f.write(",".join(cols) + "\n")
                for vid in ("A", "B", "A"):
                    f.write("2022-06-30T12:50:19.859,x,%s,100,1,41,-106,30,90,0.5,0.4\n" % vid)
            with self.assertRaises(ValueError):
                list(normalise.stream_ids(p, "wyoming"))


class PairingTests(unittest.TestCase):
    def test_dedupe_on_id_secmark_msgcnt(self):
        t = trace(5)
        self.assertEqual(len(pairing.dedupe_sort(t + t[:2])), 5)

    def test_secmark_wrap_and_disagreement(self):
        self.assertAlmostEqual(pairing.secmark_dt(59950, 50, 0.1), 0.1)
        self.assertIsNone(pairing.secmark_dt(100, 200, 5.0))

    def test_displacement_is_metres_east_north(self):
        t = trace(2, v=10.0, heading=90.0)
        dx, dy = pairing.displacement(t[0], t[1])
        self.assertAlmostEqual(dx, 1.0, places=3)
        self.assertAlmostEqual(dy, 0.0, places=6)


class CheckTests(unittest.TestCase):
    def pair(self, **kw):
        base = dict(dt=(0.1, None, None), v1=20.0, v2=20.0, h1=0.0, h2=0.0, a1=None, a2=None, dx=0.0, dy=2.0,
                    brakes2=None, dcnt=1, sm1=0, sm2=100)
        base.update(kw)
        return pairing.Pair(**base)

    def test_random_position_v10(self):
        self.assertFalse(mc.run(self.pair(dy=2.5), 0.1)[0][0])
        self.assertTrue(mc.run(self.pair(dy=3.1), 0.1)[0][0])
        self.assertTrue(mc.run(self.pair(dy=0.0), 0.1)[0][0])  # stale fix at 20 m/s

    def test_random_position_v11_uses_acceleration(self):
        # a = 400 m/s^2 is absurd but isolates the term: 0.5*400*0.01 = 2 m extra along track
        p = self.pair(dy=4.0, a1=400.0)
        f = mc.run(p, 0.1)[0]
        self.assertTrue(f[0])
        self.assertFalse(f[1])

    def test_constant_position_speed_accel_brake(self):
        f = mc.run(self.pair(dy=0.005, v1=1.0, v2=1.0), 0.1)[0]
        self.assertTrue(f[2])
        self.assertFalse(mc.run(self.pair(dy=0.005, v1=0.0, v2=0.0), 0.1)[0][2])
        self.assertTrue(mc.run(self.pair(v2=91.0, v1=91.0, dy=9.1), 0.1)[0][3])
        self.assertTrue(mc.run(self.pair(v2=21.5), 0.1)[0][4])  # 15 m/s^2 implied
        self.assertTrue(mc.run(self.pair(a2=-10.5), 0.1)[0][5])
        self.assertTrue(mc.run(self.pair(brakes2=True, a2=0.0), 0.1)[0][6])
        self.assertFalse(mc.run(self.pair(brakes2=True, a2=-0.5), 0.1)[0][6])
        self.assertFalse(mc.run(self.pair(brakes2=None, a2=1.0), 0.1)[0][6])

    def test_gap_limit_tolerance(self):
        self.assertTrue(mc.judged(0.104, 0.1))
        self.assertFalse(mc.judged(0.2, 0.1))
        self.assertTrue(mc.judged(0.53, 0.5))
        self.assertFalse(mc.judged(None, 0.5))

    def test_slip_signature(self):
        self.assertEqual(mc.slip_signature(self.pair(dy=0.0), 0.1), "zero")
        self.assertEqual(mc.slip_signature(self.pair(dy=4.0), 0.1), "double")
        self.assertEqual(mc.slip_signature(self.pair(dy=2.0), 0.1), "other")


class ClassAndPipelineTests(unittest.TestCase):
    def test_classifier(self):
        f, n = devclass.locked_fraction([100, 201, 349, 450, 523])
        self.assertAlmostEqual(f, 0.8)
        self.assertEqual(devclass.classify(0.81, 50), "locked")
        self.assertEqual(devclass.classify(0.29, 50), "free")
        self.assertEqual(devclass.classify(0.5, 50), "mixed")
        self.assertEqual(devclass.classify(0.95, 5), "unclassified")
        self.assertEqual(devclass.phase_wrapped(149), -1)
        self.assertEqual(devclass.phase_wrapped(51), 1)

    def test_clean_locked_trace_has_no_triggers_and_slips_are_counted(self):
        r = pipeline.analyse_id("c", trace(80))
        self.assertEqual(devclass.classify(r.frac, r.nsm), "locked")
        self.assertEqual(sum(a[2] for a in r.acc.values()), 0)
        s = pipeline.analyse_id("s", trace(80, slip_every=10))
        trig = sum(a[2] for k, a in s.acc.items() if k[:2] == (0, 2))
        self.assertEqual(trig, 2 * 7)  # each slipped BSM fires its two pairs
        self.assertEqual(s.slip_sizes[1], 7)
        self.assertEqual(s.ent_slip, 7)

    def test_free_phase_jitter_alone_fires_at_speed(self):
        import random
        rnd = random.Random(1)
        jit = [rnd.randint(-45, 45) for _ in range(80)]
        r = pipeline.analyse_id("f", trace(80, v=30.0, phase_ms=25, jitter=jit))
        self.assertEqual(devclass.classify(r.frac, r.nsm), "free")
        self.assertGreater(sum(a[2] for k, a in r.acc.items() if k[:2] == (0, 2)), 10)


class StatsAndGateTests(unittest.TestCase):
    def test_bootstrap_deterministic_and_clustered(self):
        u = [(10, 1.0)] * 5 + [(0, 1.0)] * 5
        a = bootci.ci(u, "k")
        self.assertEqual(a, bootci.ci(u, "k"))
        self.assertAlmostEqual(a[0], 5.0)
        self.assertLess(a[1], 5.0)
        self.assertGreater(a[2], 5.0)

    def units(self):
        return ([("locked", 0, 1.0 + i % 3) for i in range(40)] + [("free", 30 + i % 7, 1.0) for i in range(40)])

    def test_gate_passes_on_real_checks_and_fails_on_broken(self):
        ok, rows = fgate.full(trace(80), self.units())
        self.assertTrue(ok, rows)
        bad, _ = fgate.run(trace(80), self.units(), gps_drift=1e9)
        self.assertFalse(bad)

    def test_placebo_fails_without_class_effect(self):
        flat = [("locked", 5, 1.0)] * 40 + [("free", 5, 1.0)] * 40
        self.assertFalse(fgate.g3_placebo(flat)[0])

    def test_frozen_doc_is_deterministic(self):
        ds = pipeline.Dataset("T", "fit")
        ds.add(pipeline.analyse_id("a", trace(80)))
        ds.add(pipeline.analyse_id("b", trace(80, slip_every=5)))
        d1 = calib.dump_json(calib.frozen_doc([ds], devclass.params(), "x", {}))
        d2 = calib.dump_json(calib.frozen_doc([ds], devclass.params(), "x", {}))
        self.assertEqual(d1, d2)


class FetchTests(unittest.TestCase):
    def test_queries_pin_select_order_and_row_id(self):
        qs = sources.queries()
        self.assertEqual({q["dataset"] for q in qs}, {sources.TAMPA, sources.OREM, sources.WYOMING, sources.NYC})
        for q in qs:
            self.assertEqual(q["order"][-1], ":id")
            self.assertNotIn("*", q["select"])
            u = soda.url(q["dataset"], soda.query_params(q))
            self.assertTrue(u.startswith("https://data.transportation.gov/resource/"))
        self.assertEqual(len([q for q in qs if q["dataset"] == sources.OREM]), 7)

    def test_user_agent_has_no_identity(self):
        from version import USER_AGENT
        self.assertRegex(USER_AGENT, r"^v2xw-realdata/[0-9.]+$")

    def test_sha256_detects_a_changed_file(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "f")
            with open(p, "w") as f:
                f.write("a")
            h = soda.sha256(p)
            with open(p, "w") as f:
                f.write("b")
            self.assertNotEqual(h, soda.sha256(p))


if __name__ == "__main__":
    unittest.main()
