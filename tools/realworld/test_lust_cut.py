"""Checks of the LuST subsection cut (WP-24 P24.1). python3 -I -m unittest tools/realworld/test_lust_cut.py"""
import hashlib
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lust_cut  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
NET = os.path.join(REPO, "worlds", "sumo", "lust-veremi-sub.net.xml")
NET_SHA256 = "46eea2b0b70676c223077cae3223dc448538fbd3ef8e8615e173dbe72788c066"

TOY = """<?xml version="1.0" encoding="UTF-8"?>
<net>
  <junction id="a" type="priority" x="1" y="1"/>
  <junction id="b" type="priority" x="9" y="1"/>
  <junction id="c" type="priority" x="20" y="1"/>
  <edge id=":a_0" function="internal"><lane id=":a_0_0" shape="1,1 2,2"/></edge>
  <edge id="in" from="a" to="b"><lane id="in_0" shape="1,1 9,1"/></edge>
  <edge id="cross" from="b" to="c"><lane id="cross_0" shape="9,1 20,1"/></edge>
  <edge id="bulge" from="a" to="b"><lane id="bulge_0" shape="1,1 5,30 9,1"/></edge>
</net>
"""


class LustCut(unittest.TestCase):
    def test_fully_within_drops_partial_edges_and_shapes_that_leave_the_box(self):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "toy.net.xml")
            open(p, "w").write(TOY)
            self.assertEqual(lust_cut.fully_within(p, (0, 0, 10, 10)), ["in"])

    def test_committed_cut_is_the_recorded_file(self):
        self.assertEqual(hashlib.sha256(open(NET, "rb").read()).hexdigest(), NET_SHA256)

    def test_committed_cut_measures_as_recorded(self):
        m = lust_cut.measure(NET)
        self.assertEqual((m["junctions"], m["signalised"], m["edges"]), (77, 5, 205))
        # the published scenario description: "approximately 1.61 km2" (VeReMi Extension site)
        self.assertLess(abs(m["junction_extent_km2"] - 1.61), 0.1)

    def test_committed_cut_carries_the_mit_licence(self):
        head = open(NET, encoding="utf-8").read(4000)
        self.assertIn("The MIT License (MIT)", head)
        self.assertIn(lust_cut.SOURCE_COMMIT, head)

    def test_stamp_is_byte_reproducible_and_refuses_a_non_mit_licence(self):
        with tempfile.TemporaryDirectory() as d:
            src = os.path.join(d, "nc.xml")
            open(src, "w").write('<?xml version="1.0" encoding="UTF-8"?>\n<!-- generated on X -->\n<net/>\n')
            lic = os.path.join(d, "LICENSE.md")
            open(lic, "w").write("The MIT License (MIT)\n\nCopyright (c) 2016 L\n")
            a, b = os.path.join(d, "a"), os.path.join(d, "b")
            lust_cut.stamp(src, lic, a)
            lust_cut.stamp(src, lic, b)
            self.assertEqual(open(a).read(), open(b).read())
            self.assertNotIn("generated on", open(a).read())
            open(lic, "w").write("GPL-3.0\n")
            with self.assertRaises(SystemExit):
                lust_cut.stamp(src, lic, a)


if __name__ == "__main__":
    unittest.main()
