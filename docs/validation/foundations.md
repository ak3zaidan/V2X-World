# Foundations, checked from the ground up

2026-10-07. Suite: `crates/v2xw-engine/tests/foundations.rs` (9 tests, about 28 s in a debug
build, counts and simulated time only), plus `settings_audit` (ignored by default, about 80 s).

```
cargo test -p v2xw-engine --test foundations
cargo test -p v2xw-engine --test foundations -- --ignored --nocapture settings_audit
```

## Method

The world is `phase1-grid.yaml` cut to 4 × 8 junctions (822 × 560 m). The vehicles come from
a `v2xw/trips/1` file: `n` trips on the `n` driving lanes nearest the centre, each bound for
the farthest lane it can reach. A Poisson cap was not usable for this (see the open items).
The exact-value tests use free space, no fading and no buildings, so every received power
follows Friis exactly and every number can be computed by hand.

## Two vehicles (3 s): every number is its hand value

| Quantity | Hand value | Engine |
|---|---|---|
| BSMs | 10 Hz × 3 s × 2 = 60, generated exactly 100 ms apart, msgCnt +1 each | 60, exact |
| Signer | full certificate once a second (J2945/1), digest otherwise | 3 per vehicle |
| Frame octets | 56 BSM + 164 / 93 envelope (with the 77-octet implicit pseudonym) + 5 WSMP + 38 LLC/MAC/FCS = 263 / 192 | equal, and layers sum for every frame |
| Airtime | 802.11p 10 MHz, 6 Mb/s: 40 µs + ⌈(22 + 8·L)/48⌉ × 8 µs = 400 / 304 µs | equal for every frame |
| Received power | P_tx + 2 × (5 dBi − 2 dB cable) − FSPL(d, 5.860 GHz) | within 0.02 dB for all 60 |
| Receptions | one link each way: 60 attempts, 60 decoded, delivered, verified | 60 / 60 / 60 / 60 |
| Propagation | d / c | within 1 ns |
| Sign / verify | 9 ms (eHSM "<9 ms") / 400 µs (1/2500 s); the first SPDU under a new pseudonym also pays its validation, SHA-256 over the 77-octet certificate + one ECQV reconstruction priced on the profile (≈ 1.95 ms, 2,349 µs in all); every later one 400 µs | exact for every message |
| Position in BSM | within GNSS error of the true position | mean < 5.3 m, ≥ 80 % under 9.38 m (Reid 2019 p99), max < 30 m |
| Heading | J2735 clockwise from north = 90° − ENU heading | within 10° when moving |
| Window at 2 s | bytes_air 3,982; offered = carried 31,856 b/s; channel_load 0.0063; occupancy 0.0031; envelope/payload 2,002/1,120; security 2,002/3,982; cert share 154/2,002; PDR 20/20; NAR(300 m) 1; verify_rate 20; AoI = latency + 50 ms | all equal |
| Zero metrics | collisions, PER, half-duplex, loss, unverified | 0 |

## A roadside unit (2 s)

A unit on the signalised junction nearest the two vehicles, roles SPaT and MAP: 20 SPaT
(10 Hz), 2 MAP (1 Hz). Each vehicle receives all 22, and the unit receives all 40 BSMs.
Attempts are 2 × (40 + 22) = 124, and all 124 decode.

## The backend (45 s, 30 s certificate periods)

- **Two vehicles, top-ups over cellular.** Each vehicle tops up once, with 5 certificates.
  The cellular uplink, cellular downlink and backend byte counters agree to the octet with
  the `net.bytes` records and with the per-window metrics summed. A top-up's `bytes` equals
  what crossed that vehicle's downlink.
- **One attacker among six (ConstPos).** It is detected, reported (cellular and RSU relay)
  and revoked: 1 decision, 0 honest vehicles revoked. One CRL is issued, broadcast and
  installed by all 6 stations. The attacker's next top-up is refused as blocklisted; the
  other 5 complete. Revocation stages are in order. Every refused reception (1,240) appears
  on `node.rx` with cause `revoked`.

## More vehicles (1 s each): 2, 5, 10, 25, 50, 100

| n | frames | offered b/s per vehicle | attempts = frames × (n−1) | channel_load (≈ n × 0.003136) | frames overlapping on air | collision losses |
|---|---|---|---|---|---|---|
| 2 | 20 | 15,928 | 20 | 0.0063 | 0 | 0 |
| 5 | 50 | 15,928 | 200 | 0.0157 | 0 | 0 |
| 10 | 100 | 15,928 | 900 | 0.0314 | 0 | 0 |
| 25 | 248 | 15,805 | 5,952 | 0.078 | 0 | 0 |
| 50 | 495 | 15,774 | 24,255 | 0.155 | 1 | 94 (+2 half-duplex) |
| 100 | 988 | 15,744 | 97,812 | 0.309 | 30 | 4,693 (+60 half-duplex, +65 hidden-terminal) |

Measured 2026-10-18 with the 77-octet implicit pseudonym (WP-C1); the 100-vehicle row
already read this way before the validated-certificate cache landed.

Offered load, attempts and the channel busy ratio are linear in n, as theory says.

**The 65 hidden-terminal losses at 100 vehicles are geometry, not a defect.** Everyone
*decodes* everyone in free space here, but not everyone *defers* to everyone. CCA is
−85 dBm (`CcaConfig::ETSI`, the EN 302 571 §4.2.10.1 busy threshold), while a 6 Mbit/s
BSM still decodes down to −88 dBm (the static sensitivity). At this EIRP (20.5 dBm
including both antennas) Friis at 5.860 GHz puts −85 dBm at 767 m and −88 dBm at
1,083 m. The grid's diagonal is 995 m, and the 100-vehicle fleet fills lanes out to its
edges, so some pairs are 767–995 m apart: each hears the other's frames but senses the
channel idle during them. In the 1 s run this happens once. Vehicle 51's frame starts at
68.003 ms and lasts 400 µs. It reaches vehicle 82, 858.1 m away, at −85.98 dBm, below CCA.
Vehicle 82 starts a frame 382.5 µs later, and the two overlap for 17.5 µs. Both frames are
lost at the receivers the overlap ruins (65 hidden-terminal losses), and so is 51's frame
at 82, which was transmitting by then (part of the 60 half-duplex losses). The 50-vehicle
fleet's lanes sit nearer the centre, so no overlapping pair lies in that band. Everything
else needs two frames on the air at once.

`the_load_metrics_scale_with_the_fleet_as_theory_says` now derives this from the records
alone. For every pair of overlapping frames with different start times, it takes the
earlier frame's RSSI at the later sender (`phy.rx`). It then asserts that hidden-terminal
losses occur exactly when some pair is below −85 dBm, and that every such pair lies in the
−88 to −85 dBm band and more than 760 m apart. A fault that moves the engine's classifier
threshold by 3 dB turns it red.

**A realism gap this exposes (not changed here):** a real 802.11 receiver that detects a
preamble is busy for as long as it receives the frame, whatever its CCA level. IEEE
802.11-2016 §17.3.10.6 sets −85 dBm only as the *minimum* level that must read busy in a
10 MHz channel, and ns-3 keeps a PHY in RX (CCA busy) once it has locked on a frame
above its sensitivity. The engine takes CCA from received power against the single
threshold, so in the 3 dB band a vehicle transmits over a frame it is decoding. Making
preamble-detected frames hold CCA busy would remove these losses, and would change every
radio digest. It belongs in a radio work package, not in this cleanup. The test asserts that collisions happen
exactly when frames overlap, and that one overlapping pair costs at most both frames at
every other receiver. At most one frame per vehicle is still in the MAC when the run ends,
which is why some rows show slightly fewer than 10n frames.

## Defects found and fixed

1. **The last metric window of every run was dropped.** This hit every run longer than one
   metric period. The end-of-run event was scheduled at build, so its sequence number was
   older than the flush due at the same instant, and it ran first. A 3 s run reported
   metrics at 1 s and 2 s only. Now whatever else is due at the horizon runs first
   (`run.rs`).
2. **A top-up's "bytes downloaded" counted the backend's internal traffic.** It summed
   every wire step of the flow (RA, ACA and LA hops) and reported 7,319 octets, against
   1,141 that actually crossed the vehicle's downlink (`phase2.rs`).
3. **A unit's SPaT or MAP role did nothing without the message set.** A unit with
   `roles: [spat, map]` sent nothing unless `messages.sets` also carried them, and nothing
   said why. This is now a validation conflict that names the fix.
4. **`world.source.generator` was ignored.** Every procedural world was a grid, so picking
   the radial city in the settings changed nothing. The radial generator is now built,
   its parameters are validated, and an unknown generator is refused.
5. **The OBU dropdown offered units the loader always refuses.** Units such as the Cohda
   MK5 publish no ECDSA signing cost. They are now listed as unavailable, with the reason.
6. **`radio.tiers.phy: abstract` runs the medium PHY.** This build has no abstract 802.11p
   PHY, and the sidelink maps abstract to medium. The dropdown now says so.

## Settings audit: what changed nothing, and why

Method: change every `x-status: wired` field of the published form, one at a time, on a
5-vehicle, 4 s run with an RSU (SPaT, MAP, CRL, relay) and the backend, and compare the
whole record stream with the unchanged run. **37 fields changed the run.** Every field
below changed nothing in this run. Each is listed with the reason, checked in code.

- **Has no effect on this world type.** These apply to OpenStreetMap imports only:
  `world.source.bbox.*`, `world.buildings.metres_per_level`, `world.highway_preset`,
  `world.signals.coordinate`, `world.signals.progression_speed_mps`,
  `world.service_roads`. `world.buildings.keep_holes` has nothing to act on, because grid
  buildings have no holes. *Suggestion: hide these in the form when the world is
  procedural.*
- **Has no effect on this demand.** `actors.vehicles.demand.rate_veh_per_h` and
  `rate_veh_per_h_per_lane_km` do nothing under a trip file.
- **Has no effect without pedestrians or the feature.** `actors.vru.*` and
  `radio.devices.vru.*` need VRUs. `apps.*_ttc_s` and the other app thresholds need
  applications. `threats.eavesdropper.id` and `detection.ma.id` need detectors.
  `experiment.replications` is read by the experiment runner, not by one run.
- **Takes effect after 4 s.** `security.pseudonym_change.period_s` and the `mix_zone_*`
  fields; `security.kem`, used at the first top-up (30 s); `net.uu.id`, `net.backhaul.id`
  and `net.backend_net.id`, since no backend traffic happened in 4 s.
- **The value tried was the default.** `radio.channel` 172 and `messages.generator.id`
  `bsm-j2945-1`.
- **Honest no-ops that the form should explain:**
  - `weather.intensity` does nothing while `weather.initial` is `clear`.
  - `radio.devices.obu.tx_power_dbm` 23 → 20 does nothing, because J2945/1 power control
    already radiates 17.5–18.8 dBm. A lower power does take effect (`propagation.rs` tests
    10 dBm).
  - `radio.devices.obu.antenna_pattern: tr37885` gives cars a roof antenna with no
    horizontal pattern. Only trucks and buses change.
  - `messages.codec_tier: size-model` changes only the sets with no real encoder (SRM,
    SSM).
  - `security.envelope: etsi103097` leaves a BSM the same length on the air.
  - `radio.tiers.phy: abstract` (fixed, item 6 above).

**Refused by validation, correctly.** Each refusal names the conflict:
`radio.tiers.propagation: abstract` with a medium PHY; `radio.region: us` with DSRC (FCC
24-123); the ETSI backend with a 1609.2 envelope; `pseudonym_change.strategy: distance`
without a distance; `threshold.members` below the quorum.

## Sign 9 ms vs verify 0.4 ms (the professor's question)

**The numbers are what the hardware profile says, and they are not ECDSA arithmetic.**
For ECDSA on one engine, verify costs about twice what sign costs (one fixed-base scalar
multiplication to sign, a double scalar multiplication to verify). The profile's own
software proxy shows this: Pi 3B, 613 µs sign against 1,290 µs verify (R7 §D8).

The Unex OBU-301 on the CRATON2 does the two on different hardware:

- **Signing** runs in the eHSM, a Cortex-M0 secure element that holds the private key.
  Its FIPS security policy publishes ">110 signatures/s" and "<9 ms signing latency"
  (R7 §A4).
- **Verification** runs on a separate line-rate verification engine, rated at ">2500
  ECDSA P-256 verifications/s" (R7 §A4). The profile derives 400 µs per verify as
  1/2500 s.

**Both figures are bounds used as point values.** The 9 ms is an *upper* bound. The 400 µs
is a single-server derivation from a *lower* bound on throughput. The ratio is a property
of this unit's split architecture: the private key never leaves the slow secure element,
and public-key checks go to a fast accelerator. A unit that does both on one CPU
(`obu/generic-automotive-soc-no-hsm`) shows the textbook order.

**Recommendation for the BSM detail panel:** label the two stages with where they ran:
"signed in the HSM (≤ 9 ms, vendor bound)" and "verified on the accelerator (1/2500 s)".

## Open items (not fixed here)

- **CCA does not follow preamble detection.** In the 3 dB band between the −85 dBm CCA
  threshold and the −88 dBm 6 Mbit/s sensitivity, a vehicle decodes a frame it does not
  defer to (above, "More vehicles").

- **The Poisson demand cap counts trips, not vehicles.** A trip the router cannot route is
  dropped but still counted. On the 4 × 8 grid, a cap of 2 gave 0 vehicles. On a 3 × 3
  grid every trip was dropped. The suite uses trip files for this reason, but "start with
  2 vehicles" in the Studio needs a cap on placed vehicles.
- **The router refuses a destination lane with no successor.** The 8 dead-end lanes of the
  grid were reachable and still unroutable.
- **The GNSS model's multipath bursts reach 20–25 m.** Five BSMs of one vehicle within
  0.8 s did so. That is within the model's design, but it is about 10× the share Reid's
  99th percentile implies for the run.
- **`phase2.messages_checked` (99) is far below the attacker's frames received (300).**
  This was not investigated.
- **Three `scenario::publish` unit tests fail on this branch.** They are
  `every_leaf_has_a_status` (`mobility.provider`), `every_number_publishes_a_unit`
  (`actors.rsu_placement.count`) and `every_section_has_a_group` (`city`). They name fields this work does not touch and were not re-run on the bare base commit; they
  most likely come from the merge of the other tracks.
