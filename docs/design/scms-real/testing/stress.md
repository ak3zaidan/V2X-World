# SCMS stress testing: scale, multi-week runs and post-quantum (stress package, 2026-10-22)

Branch `wf_8b9f7369-bed-6`, from main `2004bb87` (after the PKI, flows and policy fix wave).
Every expectation was written down and committed before its run
(`crates/v2xw-engine/tests/expectations/scms-stress.yaml`, commit `5051242f`). The runs used
one opt-level-2 debug build of this branch, `v2xw-b1` (sha256 prefix `bae6a737f5b3770e`).
They went through the machine's run lock, one run per call, with the same seed as their
base. The release binary was not used: it predates the fix wave.

The machine was shared with two other agents' runs and builds for the whole session. About
75 minutes of the three hours went to waiting for the run lock. Wall times are shared-machine
times, so CPU time is the figure to compare.

## 0. What was fixed

**F-OFF: a switched-off device still reached the SCMS** (`phase2.rs`, `run.rs`, commit
`5f17ed2d`).
- A timeline `outage` turned a vehicle's radio off. Its SCMS client kept running: it
  polled the CRL, topped up, re-enrolled and uploaded reports over the cellular link.
- So a car parked "offline for weeks" came back with a current CRL and a full pool, as if
  it had never left.
- `Phase2::set_powered` now marks the device off. While off it does none of those things,
  and the backend does not wake for it. Its timers keep their due times, so it catches up
  at its first step after power returns.
- Test: `security_lifecycle::a_switched_off_device_reaches_no_scms_until_it_is_back_on`.
  Parked with power on: 2 top-ups between 31 and 80 s. Switched off: 0. After power
  returns: 4.
- Fault injected with `tools/mutate/mutate.py`: the outage no longer reaches Phase2. The
  test went **RED** (2 top-ups while off; panic at the `off_win == 0` assertion). The tool
  printed "GREEN" only because my wrapper script exited 0. The log shows the panic, and
  the wrapper now passes on the exit code.
- The test's CRL-download assertion cannot fail on the compressed grid, which has no new
  CRL in the window. The multi-week parked run (§2) covers the CRL side.

## 1. Full-scale benchmark

**Not run to the end. Its cost does not fit this package.**
- On a quiet machine, a quarter-fleet density run (`pq-hybrid-falcon512`, 60 s live) cost
  145 CPU-s, or 2.4 CPU-s per simulated second.
- The ECDSA and ML-DSA quarter runs each took about 1,100 CPU-s. They ran while the
  68-minute test suite shared the machine, so their run cost is not comparable.
- `RAYON_NUM_THREADS=4` shortened no run: MW took 414 s at 4 threads and 343 s at 1.
- The full file has 300 live seconds at four times the fleet. Pairs grow roughly with the
  square of the fleet (F3: 15× for 4× vehicles). That gives about 24–39 CPU-s per live
  second: 7,200–11,700 CPU-s (2–3.3 CPU-hours), plus about 0.4 CPU-s per device for the
  real-P-256 provisioning.
- Neither the full file nor its 150 s first window (`stress/fs-first-window.yaml`, about
  1–1.6 h) fit the time left after 75 minutes of waiting for the run lock. Both scenarios
  are committed.

**What was measured at full fleet:** the first 5 s (`stress/fs-det.yaml`, `--no-recording
--digest`).
- 646 devices (321 / 325 by provider), 574 of them handsets: 4.0× the quarter's 142.
- 539,245 reception attempts; 2,223 misbehaviour reports in 5 s (R-USFP, below).
- 155 CPU-s and 415 MB.
- **Determinism: the stream digest `2a238e72…57a4a5a0` is identical at 4 threads and at 1
  thread. Pass.**

**The quarter-fleet density run on this branch** (`pq-base`: the quarter benchmark, 60 s
live, software OBU, fragmenter on):
- 313 devices (162 / 151 by provider), 142 of them handsets.
- 3.03 M reception attempts. CBR 0.019 mean, 0.061 p95. BSM e2e 3.7 ms mean. Verify
  queue p99 1 message.
- Peak RSS 431 MB.

**The quarter run also showed the dominant realism defect of every US real-world preset
today: R-USFP at rush-hour density** (open; 07 §8.3).
- `detect/us-sm-mbr` fired RandomPosition (positionJump) on 306,151 of 391,345 checked BSMs:
  78 % of honest BSMs.
- That gave 37,769 reports about 141 honest devices in 60 s, and 88 cases.
- The 5,710 over-the-air MBR uploads (relay-only vehicles) took 9.1 s of airtime against
  13.6 s for every BSM: 34 % of the channel. My expectation FS.mbr-airtime (under 1 %)
  fails.
- The cause is the one 07 §8.3 names. The SM-MBR 1 m `gpsDrift` threshold has no streak
  gate, and the engine puts raw GNSS fixes (independent Gauss-Markov jumps) into the BSM,
  where a deployed OBU sends a filtered position. A real filter's error is correlated over
  100 ms, so consecutive BSMs differ by centimetres, not metres.
- Until a filtered-position model exists, no misbehaviour, MBR-airtime or revocation
  number from the US presets is a real-world number. The full-scale run would be dominated
  by this defect.

## 2. Multi-week runs (fast-forward, real calendar)

**Scenarios** (`scenarios/edge/`):
- `multi-week.yaml`: the conformance run for 33 days. It has real cryptography, 75
  devices, 10 % ConstPos attackers until 433,200 s, the manual MA and the business-day
  CRL. The live windows are:
  - W0: 0–660 s
  - W1–W3: the last two minutes of weeks 615–617
  - W4: 2,419,500–2,419,920 s, across the 618→619 rollover
  - W5: day 33
- Variants:
  - `-crlg-down` (home CRL Generator down from 700 s to 2,600,000 s)
  - `-crlg-down-late` (down from 200,000 s, after its first CRL)
  - `-parked` (node 20 broken down at 640 s, OBU off from 645 s to 2,419,510 s)

| Row | Expected (before) | Measured | Verdict |
|---|---|---|---|
| MW.rollovers | five rollovers; each week's pool in use; 1 top-up a week per device, 20 certs; 0 starved | signing i-period: W0 614→615, W1 615, W2 616, W3 617, W4 618→619, W5 619. Top-ups by week 12/14/15/16/7, 1,280 certs = 64 × 20. 3 refused (blocklisted). The 1 starved vehicle is node 28, a **revoked attacker** whose top-up was refused at 605,400 s: correct | pass |
| MW.crl-reissue | each CRL Generator reissues by nextCrl; 0 overdue refusals | 10 CRL versions in 33 days (2 providers × weekly reissue + publications); 12,592 conditional polls answered 412, 126 downloads; 0 refusals of any kind | pass |
| MW.crl-growth | entries grow with each window's attackers | 1 → 2 → 3 entries (252 / 292 / 332 B, **+40 B an entry**, 33.1 B in 04 §5 with a different header share), then flat: the inherited attack schedule ends at 433,200 s (day 5), so later windows have no attackers | wrong-expectation (the schedule, not the engine) |
| MW.revocation-timing | shuffle ≤ 1 day, +8 h manual review, next business-day slot | shuffled 85,904 s; decided 114,704 s (Wed 04:41 EDT, +28,800 s); published 190,769 s (Thu 09:49 EDT, by the 10:00 ET slot of the next business day); enforced 191,102 s | pass |
| MW.reenrolment | about N × 33 d / 6 y = 1.1 of 75 | 1 started, 1 completed | pass |
| MW.cost | ≤ ~5 × 10^4 backend ticks; < 20 min; < 500 MB | 54,511 ticks (33 d × 1,440 idle minutes = 47,520, plus due events); 414 s wall / 428 CPU-s; 84 MB; 1.84 M events | pass |
| MWC.overdue-onset | from issue + 21 d, every message under the home CRACA refused | **0 refusals in every window**. The home provider published **no CRL before its first revocation** (190,769 s), so with the Generator down from 700 s no device ever held a home CRL, and a list one does not hold cannot become overdue. Versions published 10 → 6 | **open (R-NOCRL)**, see below |
| MWC.revocation-blocked | home revocations unpublished until the return | the home attacker's entry (CRL of 1 entry, 252 B) published only after 2,600,000 s; downloads of the 3-entry composite only in W5 | pass |
| MWC.recovery | 0 overdue refusals in W5 | 0 (vacuous: there were none to recover from) | open (R-NOCRL) |
| MWC2.overdue-onset | home CRL issued 190,769 s → nextCrl 795,569 → overdue from 2,005,169 s: W0–W3 none, W4 every home-CRACA message refused by every receiver, W5 recovered | crl-overdue refusals 0 / 0 / 0 / 0 / **87,823** / 0. In W4 the base's 203,395 valid became 115,572 valid plus 87,823 refused. Verified a←a fell 150,320 → 104,912 and b←a 124,661 → 82,246; the falls sum to 87,823. Provider-b-signed messages are unchanged. **Reports sent 1,930 → 7,869**: the legacy-12 detectors report the refused honest senders. Cases (3) and honest revocations (0) are unchanged, because the MA sets aside signature-only evidence | **pass**. Security finding: keeping one provider's CRL Generator down for 3 weeks after its last CRL silences everything that provider's fleet signs, at every receiver of both providers, and quadruples the reports at the MAs |
| MWP.off-no-backend | the parked car polls and tops up nothing while off | node 20: no security record, top-up or CRL download from 645 s to 2,419,510 s. Its first CRL download is at 2,419,510 s, the power-on instant | **fixed** (F-OFF; before the fix it kept polling and topping up) |
| MWP.overdue-on-return | refuses everything until its first poll; empty pool until a top-up | Pool: empty for about 60 s after power-on (121 records), then 4 catch-up top-ups (at 2,419,570, 631, 691 and 860 s), signing under 618 and then 619. CRL: no overdue refusal, because the car was switched off before any CRL existed (R-NOCRL) | pool: pass; CRL: **open (R-NOCRL)** |
| MW.determinism | identical record digest at 1 and 4 threads | `0e97a8ac…5e08bee` at both (343 s at 1 thread, 414 s at 4) | pass |

**R-NOCRL (open, engine realism defect).**
- A provider's CRL Generator publishes nothing until its first revocation in the run
  (`v2xw-proto/src/scms/run/crl.rs`: the reissue timer is armed only by a publication).
  The only exception is `seed_standing_crl`, which is set only when standing entries are
  configured, and only for the home provider.
- The real deployment has run since 2015 (i = 614 at t0). Every CRACA's CRL exists at t0,
  and every device holds a copy issued within the past week.
- Consequences in the engine:
  - the 2-week overdue rule can never act on a CRACA that has not revoked in the run;
  - an attack that keeps a CRL Generator or CRL store down from the start is invisible
    to devices;
  - the "refuse until a fresh CRL" behaviour of a car that was offline for weeks cannot
    occur for such a CRACA.
- Proposed fix: publish each provider's (empty or standing) CRL at t = 0, with an issue
  time at the last weekly reissue before t0, and let devices install it at provisioning.
  It changes every SCMS digest and many counters (CRL versions, downloads, 412 polls), so
  it belongs in a fix wave with a re-bless. It was not made here because of the build
  budget.

## 3. Post-quantum at realistic density (quarter fleet, 60 s live, fragmentation on)

| Row | Expected (before) | Measured (`pq-base` → `pq-mldsa44`) | Verdict |
|---|---|---|---|
| PQ.sizes | 2.3–2.8 fragments per BSM; digest frame ~2.5 kB | BSM frames 41,903 → 96,562 (**2.30 frames per BSM**); 50,180 SDUs fragmented, 137,968 fragments; PSDU mean 207 → 1,299 B per fragment | pass |
| PQ.airtime | BSM airtime ×10–14; CBR to ~0.15–0.4 | BSM airtime 13.6 → 171.8 s (**×12.6**); PSM 3.0 → 72.9 s; CBR mean 0.019 → **0.142**, p95 0.061 → 0.338, max 1.0 | pass (CBR at the low end) |
| PQ.loss | reassembly loss > 0; message PDR ≈ PDR_frag^k | frame PDR 80–100 m 0.705 → 0.538, 40–60 m 0.880 → 0.803; 277,402 reassemblies complete against 2,382,569 lost (partial receptions at all ranges); verified BSMs 378,425 → 253,462 (**−33 %**) | pass |
| PQ.verify-load | pure PQ cheaper to verify than software ECDSA | verify cost mean 0.628 → **0.430 ms**; charged verify time 2.78 → 1.71 s; e2e BSM latency 3.7 → 11.3 ms (airtime and queueing) | pass |
| PQ.backend-bytes | credential bytes grow with certificate size | backend bytes 56.3 → 222.9 MB (×3.96). This includes reports, which fell, so the per-certificate factor is not isolated | open (not isolated) |
| PQ.mbr-mtu | (found) | under ML-DSA-44 **every over-the-air report is refused at the WSMP MTU** (`net_mtu_refusals` 4,001 = `reports_uploaded_relay` 4,001; MBR airtime 9.1 s → 0). Engine application frames keep `fragmenter/none`'s rule (`run.rs` hand_down_app), so relay-only vehicles cannot report at all under PQ, **and each refused report is still counted as uploaded** | **open (R-PQMBR)**: the count is wrong; the loss itself is a real PQ finding (1609.3 WSMP has no fragmentation) |
| PQ.hybrid (Falcon) | hybrid Falcon-512: digest frame ~0.8 kB, 1 fragment; full ~2.4 kB, 2 fragments; airtime ×3–5; verify hybrid > ECDSA > pure PQ | 46,156 BSM frames (**1.10 per BSM**), PSDU mean 941 B; BSM airtime 60.2 s (**×4.4**); CBR mean 0.057; verified BSMs −11 %; verify cost mean **0.751 ms** (ECDSA 0.628, ML-DSA-44 0.430); e2e 6.7 ms; 5,064 over-the-air reports refused at the MTU (R-PQMBR) | pass |
| PQ.hybrid ML-DSA, PQ.certcycle | — | `pq-hybrid-mldsa44` and `-certcycle` not run (the run lock was the bottleneck) | open (owed) |
| PQ.determinism | identical at 1 and 4 threads | not run (20 min a run) | open (owed) |

**PQ cost.** 1,143 s wall, 1,090 CPU-s and 379 MB, against 1,210 s, 1,175 CPU-s and 431 MB
for ECDSA. Fewer messages were verified and detected, so the run was cheaper even with
2.3× the frames.

## 4. The 36-minute suite (`scms_knobs_runs`)

- **18 of 18 pass** on this branch (test profile, `RAYON_NUM_THREADS=4`), in 4,061 s. It
  took 68 minutes on the shared machine.
- My expectation said 21 tests. The file has 18, so the expectation is corrected.
- The 1-thread re-run for determinism was not made: it is another 68 minutes of build
  lock.

## 5. Other tests run

- `security_lifecycle`: 16 of 16 pass, including the new test.
- `timeline`: 8 of 9 pass. `a_param_change_of_the_arrival_rate_takes_effect_at_its_instant`
  fails ("90 against 17: 5.29x for a fourfold rate", band 2.5–5.0). It is a traffic-only
  test on an unequipped fleet, with no outage, no radio and no SCMS. My change acts only on
  an outage's device, so it cannot reach this test: the failure was already on the
  branch's base (found on the way, not fixed).

## 6. Owed

- **Full-scale runs.** The full benchmark and its 150 s first window need about a CPU-day
  (§1). The engine does not use more than one core here; that is worth profiling first.
- **R-USFP filtered-position model.** Without it, the US presets' misbehaviour numbers are
  dominated by honest GNSS noise (§1).
- **R-NOCRL** (§2) and **R-PQMBR** (§3), with a re-bless of the SCMS digests.
- **Not run:** the hybrid PQ runs, the certificate cycle, PQ determinism, and the
  1-thread knobs suite.
