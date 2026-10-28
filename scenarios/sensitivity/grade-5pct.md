# `grade-5pct.asc`: a synthetic 5 % ramp (provenance)

**What it is.** An ESRI/USGS ASCII grid (`v2xw_world::dem::read_ascii_grid`) of 100 × 25
posts at 0.0002° (about 22 m), covering longitude −0.002° to 0.018° and latitude −0.002°
to 0.003° around Null Island (0°, 0°), the origin every procedural world has
(`v2xw-world/src/procedural.rs`). It covers the `highway-congestion.yaml` world (two
1,500 m blocks) with a margin.

**Not measured terrain.** The heights are written by a formula, not taken from a survey:
`z = 50 m + 0.05 · x`, with `x = longitude · 111,319.49 m/°`, the metres east of the
origin at the equator. The surface is a plane rising 5 % to the east. Draped
(`world.terrain.drape`), each east-west lane climbs or descends at a constant 5 % over
its 1,500 m, and each north-south lane is level.

**Why 5 % over 1.5 km.** That is the long upgrade of the AASHTO Green Book's
critical-length-of-grade analysis. That analysis is built on a 120 kg/kW design truck
(NCHRP Report 505, 2003, App. D, which quotes the 2001 Green Book), and the engine's
`calibration/truck-grade-crawl` check and `engine::tests` hold their trailers to it
(`docs/design/research/grade-dynamics.md`). The grid gives the sensitivity family
`grade-truck` (`catalogue.yaml`) a hill that a shipped file can carry. No measured DEM of
a real road is vendored (SRTM tiles need an Earthdata login).

**Written by.** The fix-round-5 traffic track (2026-10-28), with a ten-line Python loop
over the header above (`%.3f` heights). Regenerating it from the formula gives the same
bytes.
