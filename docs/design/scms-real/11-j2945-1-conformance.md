# SAE J2945/1 conformance check and fix specs (2026-10-31)

**Source.** SAE J2945/1 APR2020, *On-Board System Requirements for V2V Safety
Communications*, a single-user licensed copy that the owner supplied. It is kept out of the
repository (gitignored). This file paraphrases it and cites clause, requirement id and table
numbers. No passage longer than one sentence is quoted. Requirement ids are written the
standard's way, e.g. `6.3.6-DATAACC-039`, which is short for `[6.3.6-V2V-BSMTX-DATAACC-039]`.
Parameter values are from Table 21.

**Base.** `main` at `3fcd8a45`. The code was only read. Nothing was built or run.

**Scope.** J2945/1 covers light vehicles (3.1.7: FHWA classes 2 and 3, excluding ambulances,
police, fire and construction vehicles) sending BSMs over DSRC on channel 172. Every row
below is about what the engine can model or observe. The application thresholds are out of
scope, because 4.2.2 leaves them to the implementer (row H2).

**Owner rule.** Match the US architecture first, then do what makes sense for a simulation.
A negligible difference is recorded and skipped (§7).

## 1. Summary

| Verdict | Count |
|---|---|
| Matches | 63 |
| Deviates | 23 (19 to fix, 3 negligible, 1 kept on purpose) |
| Missing | 5 |
| Not applicable | 8 |
| **Total** | **99** |

The deviations that move results, most important first:

1. **The BSM queues at AC_VO with the ETSI EDCA set.** J2945/1 6.3.4 and Table 18 put a BSM
   with no critical event at user priority 5, which is AC_VI with CWmin 15, CWmax 1023 and
   AIFSN 4. The engine uses CWmin 3 and AIFSN 2. With four backoff slots instead of
   sixteen, two deferring stations pick the same slot about four times as often. The engine
   therefore **underestimates PDR and overestimates collisions at high density**, and it
   understates access delay by about 0.1 ms per deferral. Every medium- or high-tier US
   802.11p result is affected. Fix: J2.
2. **The BSM position is the rear-axle centre, not the centre of the vehicle rectangle**
   (6.2.3 `COORDSYSREF-001`). This is a systematic along-track bias of about 1.2 m for a car
   and about 3 m for a bus, on top of the GNSS error. Receivers (`apps.rs` gaps,
   `consistency.rs` footprints) read the position as the centre. So in mixed-class queues
   the footprints overlap, the overlap and intercept detectors fire falsely, and the ranges
   are off by the difference in class geometry. Fix: J4.
3. **The certificate cadence is effectively 1,000 ms, and the menu labels are swapped.**
   `vMaxCertDigestInterval` is 450 ms (6.5.2 `BSMSIGN-003`). At 10 Hz that means a
   certificate on every fifth BSM, which is the "about every 500 ms" of 5.1.3.2. The schema
   default is 1,000 ms, applied to BSMs and CAMs alike, and the menu calls 1,000 "the SAE
   J2945/1 reading". So the full-certificate share is about 10 % instead of about 20 %, BSMs
   are about 7 B lighter on average, and unknown-signer misses are higher. Fix: J1.
4. **The security relevance window is ETSI's, not J2945/1's.** Table 11 accepts a BSM
   generated up to 30 s in the past or 30 s in the future. The engine uses C2C-CC's 2 s and
   40 ms (`rxval.rs:148-157`), which was labelled an ASSUMPTION for the US. So
   **replays and clock-skewed senders between 2 s and 30 s are dropped at the US receiver's
   security layer, which J2945/1 would let through**. Attack-replay and clock-skew results
   change. Fix: J7.
5. **The congestion-control schedule is quantised to the 100 ms node step**, with a
   one-sided 0 to 5 ms jitter on a fixed phase. J2945/1 schedules each BSM at
   `LastTx + Max_ITT ± U(5 ms)`, generates it within ±10 ms (6.3.3 `GENTIM-002`), and sends
   a new critical event at once (6.3.8.5-6.3.8.8). Under congestion the engine's intervals
   come out as 100 or 200 ms instead of, for example, 150 ms, and the collision pattern
   repeats instead of decorrelating. The inter-packet-gap and awareness metrics are biased.
   Fix: J3b.
6. **Smaller congestion-control differences.**
   - No CBP smoothing (Eq. 2, `vCBPWeightFactor` 0.5).
   - A 0.9-weight smoothing on Π that J2945/1 does not have (Eq. 4 is a plain mean capped
     at `vPERMax`).
   - The EU EN 302 571 duty-cycle floor applied to a US unit. At CBR 0.7 it caps a 0.46 ms
     BSM at about 4.8 Hz, which J2945/1 has no rule for.

   Fix: J3a.
7. **Path history.** The engine keeps 300 m with chords of up to 300 m.
   `vMinPHistDistance` is 200 m and `vMaxChordLength` is 210 m (6.3.6.16). The engine also
   clears the history at every pseudonym change. J2945/1 has no such rule: 6.3.6-DATAACC-039
   requires 200 m except at start-up or without position, and 6.3.7 even keeps the history
   across a shutdown. So BSMs are longer than J2945/1's, and a privacy study using the
   engine loses the path-history link a real tracker has. Fix: J5.
8. **Critical events.**
   - The ABS flag is never set, although the vehicle bus knows ABS activity
     (6.3.6-DATAACC-035).
   - A BSM with a critical flag does not force the full certificate (`BSMSIGN-004`).
   - A pseudonym change is not held off while a critical flag is set (`CERTCHG-002`).

   Fix: J6.
9. **The pseudonym-change default is 5 minutes alone**, `strategy: time`. `CERTCHG-001`
   makes the change mandatory only outside a 2,100 m (±100 m) circle around the last change
   point. Under `j2945-1` the engine uses 2,000 m of travelled distance, where the standard
   wants straight-line distance (§2.14 of 10). Fixes: J8 and §2.14.
10. **The default GNSS is a measured mass-market receiver** (Reid 2019: 3.07 m horizontal,
    4.59 m vertical at 68 %). A J2945/1 unit must meet 1.5 m and 3 m under open sky
    (6.3.6.5, 6.3.6.6, A.7). The choice of default is the owner's. Fix: J10.

Earlier rows that the standard now settles are listed in §3. They include veh C6, which is
confirmed by the 6.5.3 note that "absolute distance" is the straight-line distance and not
the distance driven.

## 2. Requirements and verdicts

Verdicts: **M** matches, **D** deviates, **X** missing, **NA** not applicable. Every
`file:line` is relative to `crates/`. The "Fix" column names a piece of §5. A dash means no
fix.

### A. Positioning and timing (6.2)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| A1 | 6.2.1 `POSDETER-001` | GNSS with GPS | `v2xw-mobility/src/gnss/gauss_markov.rs` (a GPS error model) | M | — |
| A2 | 6.2.1 `POSDETER-002`; Table 21 `vPosDetRate` | Position and UTC at 10 Hz | beliefs updated every 100 ms step (`v2xw-engine/src/run.rs:4689`) | M | — |
| A3 | 6.2.2 `WAAS-001` | Use WAAS when available | the error statistics stand in for the receiver; no constellation or augmentation is modelled | NA | — |
| A4 | 6.2.3 `COORDSYSREF-001` | Position = centre of the rectangle around the vehicle, on the road plane | the belief and the BSM are at the rear-axle centre (`v2xw-mobility/src/views.rs:89`, `run.rs:4690-4747`, `v2xw-node/src/runtime.rs:2524`, no offset in `v2xw-msg/src/j2735/bsm.rs:2121`) | **D** | J4 |
| A5 | 6.2.4 `SYSTIMCOORD-001..003`; `vTimeAccuracy` 1 ms | UTC clock within 1 ms; time of position | clock model locked to GNSS, holdover drift (04-models §3.9) | M | — |

### B. BSM content (6.3.1, 6.3.5, 6.3.6, 6.3.7)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| B1 | 6.3.1 `BSMCONT-001` | WSM per 1609.3 | WSMP stack default (`run.rs:1855`) | M | — |
| B2 | 6.3.1 `BSMCONT-002`; Table 17 | `MessageFrame` + BSM, UPER | `runtime.rs:2570` `encode_message_frame` | M | — |
| B3 | 6.3.1 `BSMCONT-003/004`; 6.3.5 `MINTX-001`; Table 20 | `VehicleSafetyExtensions` with PathHistory and PathPrediction in **every** BSM; otherwise do not transmit | the first BSM after start, and the first after every change (B34), has no PathHistory (`v2xw-msg/src/j2945.rs:238-240`, `v2xw-node/src/vehicle.rs:303`) | **D** | J5 |
| B4 | 6.3.1 `BSMCONT-005`; Table 20 | `ExteriorLights` only when a bit is set | an all-off value is sent for 2 s after the lights change (`vehicle.rs:280-284`) | D (negligible) | — |
| B5 | 6.3.1 `BSMCONT-006` | `VehicleEventFlags` only when a flag is set | `vehicle.rs:276` | M | — |
| B6 | 6.3.6.19 `DATAACC-051/052` | Nothing beyond the standard; unknown parts are ignored on receipt | Part II is only `VehicleSafetyExtensions` (`runtime.rs:2561-2567`); other Part II ids are carried opaquely (`bsm.rs:93-96`) | M | — |
| B7 | Table 19 | Part I elements that must not be "unavailable" | filled from the belief and the bus (`runtime.rs:2541-2558`, `bsm.rs:2144-2152`) | M | — |
| B8 | 6.3.6.1 `DATAACC-001` | MessageID = BSM | `bsm.rs` frame | M | — |
| B9 | 6.3.6.2 `DATAACC-002` | `msgCnt` random at the first BSM after start-up | starts at 0, so every node's first BSM carries 1 (`v2xw-msg/src/generator.rs:775`, `:824`) | **D** | J5 |
| B10 | 6.3.6.2 `DATAACC-003/004` | New random `msgCnt` at a certificate change; otherwise +1, wrapping after 127 | `runtime.rs:1197-1211`; `generator.rs:824` | M | — |
| B11 | 6.3.6.3 `DATAACC-005..007` | `TemporaryID` random at start and at change, fixed otherwise | from the certificate digest, so it changes with it (`runtime.rs:1198`); no re-randomisation over an outage without a change (§7) | M | — |
| B12 | 6.3.6.4 `DATAACC-008..010`; `vMaxPosAge` 150 ms | `DSecond` = time of the position, 0 ≤ age < 150 ms | `secMark` from the belief or the timing model (`runtime.rs:2546`) | M | — |
| B13 | 6.3.6.5 `DATAACC-011/012`; `vPosAccuracy` 1.5 m | 2D within 1.5 m at 68 %, open sky (A.7) | Reid fit, 3.07 m at 68 % (`gauss_markov.rs:117`) | **D** | J10 |
| B14 | 6.3.6.6 `DATAACC-013/014`; `vElevAccuracy` 3 m | Elevation within 3 m at 68 % | Reid vertical, 4.59 m (`gauss_markov.rs:126`) | **D** | J10 |
| B15 | 6.3.6.7 `DATAACC-015/016` | `PositionalAccuracy` at 1 σ | `bsm.rs:2144-2148` (the 95 % radius converted to 1 σ) | M | — |
| B16 | 6.3.6.8 `DATAACC-017`; `vSpeedAccuracy` 0.28 m/s | Speed at 68 % | σ 0.102 m/s per axis (`gauss_markov.rs:307`) | M | — |
| B17 | 6.3.6.9 `DATAACC-018` | Transmission state | forward gears (`runtime.rs:2557`); the mobility model has no reverse or park | M | — |
| B18 | 6.3.6.10 `DATAACC-019..023`; 2°/3°, 12.5 m/s, latch 1.10, unlatch 1.38 m/s | Heading accuracy; latch at low speed | heading from the vehicle pose, with no GNSS heading noise (`gauss_markov.rs:310`), so the latch is never needed | M | J10 adds noise and the latch |
| B19 | 6.3.6.11 `DATAACC-024` | Steering angle accurate, or unavailable | not sent (allowed by Table 19) | M | — |
| B20 | 6.3.6.12 `DATAACC-025..027` | Accelerations and yaw rate | exact, from the vehicle bus (`runtime.rs:2553-2555`) | M | — |
| B21 | 6.3.6.13 `DATAACC-028..032` | Brake status from the bus | `self.own.brakes()` (`runtime.rs:2556`) | M | — |
| B22 | 6.3.6.14 `DATAACC-033`; 0.2 m | Size | the scenario's own dimensions | M | — |
| B23 | 6.3.6.15 `DATAACC-034`; `vEventDetectLatency` 250 ms | Flag within 250 ms of the event | set at the next 100 ms step (`vehicle.rs:263`) | M | — |
| B24 | 6.3.6.15 `DATAACC-035` | Hard-braking flag; ABS, traction and stability flags when the information exists | hard braking yes; `abs_active` is on the bus (`vehicle.rs:103`) but no ABS flag is ever set (`vehicle.rs:258-277`) | **X** | J6 |
| B25 | 6.3.6.16 `DATAACC-036..038` | PathHistory point format, nothing extra | `j2945.rs:232-270` | M | — |
| B26 | 6.3.6.16 `DATAACC-039`; `vMinPHistDistance` **200 m** | Covered distance | 300 m (`j2945.rs:73`) | **D** | J5 |
| B27 | 6.3.6.16 `DATAACC-040/041`; `vPathPerpendicularDist` 1 m | Concise path within 1 m | `j2945.rs:72`, `:147-180` (A.5 estimator) | M | — |
| B28 | 6.3.6.16 `DATAACC-042`; `vMaxChordLength` **210 m** | Longest chord | 300 m (`j2945.rs:73`) | **D** | J5 |
| B29 | 6.3.6.16 `DATAACC-043..045`; `vMaxPHistPoints` 15 | Fewest points, newest first, at most 15 | `j2945.rs:76`, `:193-215` | M | — |
| B30 | 6.3.6.17 `DATAACC-046..049`; 80-2,500 m, 10 %, 4 s, 1 m/s | Radius and confidence; straight with confidence 200 below 1 m/s | R = v/ψ̇ from the bus, straight beyond 2,500 m and below 1 m/s (`j2945.rs:336-366`). The confidence curve is the engine's own; A.6 Table A3 is only informative | M | — |
| B31 | 6.3.6.18 `DATAACC-050` | Light bits from vehicle status | `vehicle.rs:225-250` (`light_bits_of`) | M | — |
| B32 | 6.3.6.4 note | Other Part I fields aligned to `DSecond` as far as possible | same belief instant | M | — |
| B33 | 6.3.7 `DATAPERSIST-001..004` | Heading and PathHistory kept over a shutdown | vehicles join mid-trip with an empty history | D (negligible) | — |
| B34 | 6.3.6.16 `DATAACC-039` (no clearing rule exists) | PathHistory continuous across a certificate change | cleared at every change (`vehicle.rs:216-223`, called from `runtime.rs:2231`) | **D** | J5 |

### C. Channel, data rate, EDCA, generation timing (6.3.2-6.3.4; 6.1.3-6.1.5)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| C1 | 6.3.2 `CHDATARATE-001`; `vChannelNumber` 172 | Channel 172, 10 MHz | `v2xw-radio/src/regulation.rs:114` | M | — |
| C2 | 6.3.2 `CHDATARATE-002`; `vDataRate` 6 Mbps | 6 Mbit/s | `v2xw-engine/src/wiring.rs:2257-2268` | M | — |
| C3 | 6.3.3 `GENTIM-001` | Random first-BSM epoch | phase drawn uniformly over 100 ms (`generator.rs:1132-1135`) | M | — |
| C4 | 6.3.3 `GENTIM-002`; `vBSMRateTolerance` 10 ms | Each BSM within ±10 ms of its schedule | checked only on the 100 ms step (`v2xw-node/src/generate.rs:196-198`, `generator.rs:799-826`). A 150 ms `Max_ITT` comes out as 100 and 200 ms | **D** | J3b |
| C5 | 6.3.4 `UPEDCA-001` | UP 5 (AC_VI) for a BSM without critical flags | every periodic message is AC_VO (`wiring.rs:4271-4278`; the doc at `v2xw-engine/src/scenario/schema.rs:2576-2579` says this is J2945/1's) | **D** | J2 |
| C6 | 6.3.4 `UPEDCA-002` | UP 7 (AC_VO) for a BSM with critical flags | AC_VO, as all BSMs are | M | J2 keeps it |
| C7 | 6.3.4 `UPEDCA-003`; Table 18 | AC_VI CWmin 15, CWmax 1023, AIFSN 4; AC_VO 3/7/2; AC_BE 15/1023/6; AC_BK 15/1023/9 | AC_VI 7/15/3 from EN 302 663 (`v2xw-radio/src/types.rs:398-425`); AC_VO matches | **D** | J2 |
| C8 | 6.1.4, Table 15 | Continuous access on 172 | continuous mode is the US default (`v2xw-net/src/stack/wave.rs:9-10`) | M | — |
| C9 | 6.1.3, Table 14 `N1.3.2.6` | WSMP N-header carries "transmit power used" = the computed RP | a BSM's WSM carries no extension (`v2xw-net/src/netlayer.rs:191-196`, `v2xw-net/src/frame.rs:123`) | **X** | J9 |
| C10 | Table 9, Table 16 | PSID 0x20 | `v2xw-sec/src/cert.rs:1163` | M | — |

### D. Congestion control (6.3.8, A.3, A.8)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| D1 | 6.3.8 `CONGCTRL-001` | The algorithm is used | the US 802.11p default (`wiring.rs:4354-4366`) | M | — |
| D2 | 6.3.8.1 Eq. 1; `vCBPMeasInt` 100 ms | Raw CBP includes own TX and CCA, not the NAV | `v2xw-radio/src/mac.rs:99-103`, `:893` | M | — |
| D3 | 6.3.8.1 Eq. 2; `vCBPWeightFactor` 0.5 | Smoothed CBP | the raw CBR is used (`v2xw-radio/src/dcc.rs:1808`) | **X** | J3a |
| D4 | 6.3.8.1 Eq. 3; A.8.3; 5,000/1,000 ms | PER from the `msgCnt` span, for RVs with ≥ 2 receptions | true sent counts, for RVs with ≥ 1 reception (`dcc.rs:1677-1760`) | D | J3a (optional) |
| D5 | 6.3.8.1; `vPERRange` 100 m | RV position from its BSM | the RV's own belief (`run.rs:5109-5113`) | M | — |
| D6 | 6.3.8.1 Eq. 4; `vPERMax` 0.3 | Π = mean PER, capped at 0.3 | `Π = 0.9·Π_prev + 0.1·avg`, capped (`dcc.rs:1582-1589`, `:1247`, from the FMVSS 150 NPRM) | **D** | J3a |
| D7 | 6.3.8.1 | N = unique TempIDs within 100 m | the heard table at the parse (`run.rs:5137-5143`) | M | — |
| D8 | 6.3.8.2; A.3; `vHV*PosEstInt*` 0/200/0/3000 ms | Local and remote estimates, straight extrapolation | `dcc.rs:1643-1660`, `:1295-1301`; the 3,000 ms cap is absent (§7) | M | — |
| D9 | 6.3.8.3 Eq. 5; 0.2 m, 0.5 m, α 75 | p(k) = 1 − exp(−α(e−T)²) between T and S | `dcc.rs:1562-1571`, `:1240-1242` | M | — |
| D10 | 6.3.8.4 Eq. 6; `vDensityWeightFactor` 0.05 | Density smoothing | `dcc.rs:1551-1554`, `:1228` | M | — |
| D11 | 6.3.8.4 Eq. 7; B 25, 600 ms, 100 ms | Max_ITT = 100·N/B between B and 6B | `dcc.rs:1513-1527` (the same line, written as an interpolation) | M | — |
| D12 | 6.3.8.5; `vRescheduleTh` 25 ms | Decision rules: reschedule thresholds, the `100 + vTxRand` guard, a new critical event at once | no threshold; an exception only lowers the interval to 100 ms at the next step (`dcc.rs:1469-1480`) | **D** | J3b |
| D13 | 6.3.8.6, 6.3.8.8; `vTxRand` 5 ms | Next = LastTx + Max_ITT + U(−5, +5) ms | a fixed phase grid plus a one-sided U(0, 5) ms hand-off jitter (`wiring.rs:2431`, `generator.rs:1037-1045`) | **D** | J3b |
| D14 | 6.3.8.7 Eq. 8-11; 20/10 dBm, 50/80 %, 0.5, vRP 15 | RP = vRPMax for an event or dynamics BSM, SUPRA otherwise | `dcc.rs:1454-1466`, `:1531-1548`. SUPRA steps on every CBR sample (`dcc.rs:1810`), where the standard steps once per BSM | M (timing differs only below 10 Hz) | J3a |
| D15 | A.8.1; `vMaxSuccessiveFail` 3 | Bernoulli inference | `dcc.rs:1616-1640` | M | — |
| D16 | Table 21 `vTxRateCntrlInt` 100 ms | Control interval | tracking step every 100 ms (`run.rs:5195`) | M | — |
| D17 | none in J2945/1 | (no duty-cycle floor) | the EU EN 302 571 floor is enforced inside the SAE model (`dcc.rs:1414`, gate `:1825`). Above CBR 0.62 it stretches the interval past Max_ITT and drops frames over 4 ms | **D** | J3a |

### E. RF (6.4)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| E1 | 6.4.1 `DSRCTX-001`; vRP 15 dBm | Can radiate vRP; capped by MaxTxPowerCap | `power.min(req.power_dbm)` (`dcc.rs:1835`); profiles 20 dBm | M | — |
| E2 | 6.4.1.1 `DSRCTX-002..004`; ±2 dB, 1 dB steps | Power accuracy and steps | continuous and exact; the note says finer steps are allowed | M | — |
| E3 | 6.4.2 `DSRCRXSENS-001`; −92 dBm | ≤ 10 % PER at −92 dBm, 6 Mbit/s, 400 octets | `measured-obu` −92 dBm at 6 Mbit/s (`v2xw-radio/src/phy.rs:162`, `:172`) | M | — |
| E4 | 6.4.2 `DSRCRXSENS-002` | Enhanced adjacent-channel rejection | the BSM studies use one channel | NA | — |
| E5 | 6.4.1; vUpperEl/vLowerEl | Sector-gain measurement method | the antenna is a hardware-profile input | NA | — |

### F. Security and privacy (6.1.2, 6.5)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| F1 | 6.5.1 `IDRAND-001` | L2 address random at power-up | from the certificate digest (`v2xw-engine/src/run.rs:10331`); see §7 for an outage | M | — |
| F2 | 6.5.1 `IDRAND-002` | Re-randomised at a change | same | M | — |
| F3 | 6.5.2 `BSMSIGN-001/002` | Every BSM signed; certificate or digest | `runtime.rs:2290-2310` | M | — |
| F4 | 6.5.2 `BSMSIGN-003`; `vMaxCertDigestInterval` **450 ms** | Certificate once ≥ 450 ms have passed since the last one | the node's default is right (`v2xw-sec/src/envelope.rs:164`), but the scenario default of 1,000 ms overrides it for BSM and CAM (`schema.rs:4307-4309`, `wiring.rs:3950`). The menu labels are swapped (`v2xw-engine/src/scenario/options.rs:2573-2584`) | **D** | J1 |
| F5 | 6.5.2 `BSMSIGN-004` | Full certificate whenever a critical flag is set | not done (`v2xw-node/src/secure.rs:876-891`) | **X** | J6 |
| F6 | 6.5.2 `BSMSIGN-005/006` | No BSM without a valid certificate, or with an expired one | `runtime.rs:2215-2226`; `v2xw-node/src/stores.rs:359-366` | M | — |
| F7 | 6.5.2 `BSMSIGN-007/008` | Certificate on the first BSM after start and after a change | `envelope.rs:182-190`; `runtime.rs:1210` | M | — |
| F8 | 6.5.2 `BSMSIGN-009` | Digest otherwise | yes. The optional new-neighbour trigger is ETSI's, not J2945/1's, and is off by default (`secure.rs:887`) | M | — |
| F9 | 6.5.3 `CERTCHG-001`; 5 min, **2,100 m**, tolerance 100 m | Change by 5 min unless within 2,100 m of the last change point or a critical flag is set | default `time` changes at 5 min whatever the distance (`schema.rs:4349-4351`). `j2945-1` uses 2,000 m travelled (`stores.rs:129-134`) | **D** | J8, §2.14 of 10 |
| F10 | 6.5.3 `CERTCHG-002` | No change while a critical flag is set (unless the certificate expires) | not checked (`stores.rs:359-381`) | **X** | J6 |
| F11 | 6.5.3 `CERTCHG-003` and note | Within the circle no change, unless expiry or a power cycle; distance is straight-line | travelled distance (`stores.rs:243-246`, `:296`, `:348`) | **D** | §2.14 of 10 (D3 R1) |
| F12 | 6.5.4 `BSMVERIFY-001`; 5.1.3.3 | Verify with the attached certificate or a cached one; verify-on-demand allowed | the register's S1 and S5 | M | — |
| F13 | 6.5.5 `CERTREV-001` | Never transmit under a revoked certificate | `stores.rs:253-273`, register C8 | M | — |
| F14 | Tables 9 and 10 | Generation time yes, location and expiry no, signer chain 1, compressed, no encryption | `runtime.rs:2293-2294` (location only for a DENM) | M | — |
| F15 | Table 10; Table 6 S3.4-S3.6; Table 21 | P2PCD out of band and optional; inline **not** supported; 0.25 s ×4, 1 min, 3 | out-of-band default; `v2xw-sec/src/p2pcd.rs:124` | M | — |
| F16 | Table 11 | Relevance: generation time up to **30 s** old and **30 s** ahead | 2 s / 40 ms for beacons (`v2xw-node/src/rxval.rs:148`, `:157`) | **D** | J7 |
| F17 | Tables 11 and 12 | Chain length ≤ 8 | `v2xw-sec/src/chain.rs:58` | M | — |
| F18 | Table 11; Table 13 | Overdue CRL tolerance 3 years, subject to SCMS policy update | 2 weeks from CAMP EE Table 67 (`rxval.rs:94`) | D (kept: Table 13 lets the SCMS update it, and the SCMS Manager value is the later policy) | — |
| F19 | Table 12 | ECDSA-256, implicit certificates, linkage IDs, regions US/CA/MX | the SCMS profile; SC-P11 for regions | M | — |
| F20 | 6.5.3 notes | No silent period, no change at start-up required | `silent_period_s` off (register C9) | M | — |

### G. Security management (6.6)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| G1 | 6.6.2 | Pseudonym batches from the SCMS, root update | the proto pipeline (03, 09) | M | — |
| G2 | 6.6.3-6.6.4 | 500 kB / 100 kB / 400 kB minimum memories | device-qualification figures; the engine reports storage use and does not size it | NA | — |
| G3 | 6.6.4 `CRLLOAD-001` | Keep the newest CRL | `stores.rs` CRL gate | M | — |
| G4 | 6.6.5 | FIPS 140-2 level 2 HSM | hardware profiles (`obu-generic-automotive-soc-no-hsm` is a research option) | NA | — |

### H. Applications and scope (3, 4)

| # | Clause / id | Requirement | Engine | Verdict | Fix |
|---|---|---|---|---|---|
| H1 | 3.1.4 | Hard braking > 0.4 g | `v2xw-node/src/safety.rs:96`, `v2xw-msg/src/j2945.rs:41-44` | M | — |
| H2 | 4.2.2 | Warning thresholds are left to the implementer | FCW/IMA/LTA/BSW `TODO: calibrate` rows | NA (J2945/1 gives none) | — |
| H3 | none | (no staleness clause) | `apps.rs:604-606` cites "J2945/1's 1 s staleness", a clause that does not exist (comment fix only) | NA | — |
| H4 | 1, 3.1.7 | Light vehicles only | the same rules are applied to every class | NA | — |

### Table 21 values that differ from the engine's default

| Parameter | J2945/1 | Engine default | Where |
|---|---|---|---|
| `vMinPHistDistance` | 200 m | 300 m | `j2945.rs:73` |
| `vMaxChordLength` | 210 m | 300 m | `j2945.rs:73` |
| `vMaxCertDigestInterval` | 450 ms | 1,000 ms (scenario default) | `schema.rs:4307-4309` |
| `vCertChangeDistance` (± tolerance) | 2,100 m ± 100 m, straight-line | 2,000 m travelled (`j2945-1`); none (default `time`) | `stores.rs:131`; `schema.rs:4349` |
| Table 18 AC_VI CWmin / CWmax / AIFSN | 15 / 1023 / 4 | 7 / 15 / 3, and the BSM is at AC_VO | `types.rs:398-425`; `wiring.rs:4277` |
| UP of a non-critical BSM | 5 | 7 (AC_VO) | `wiring.rs:4277` |
| Table 11 past / future window | 30 s / 30 s | 2 s / 40 ms | `rxval.rs:148`, `:157` |
| Table 11 overdue CRL tolerance | 3 years | 2 weeks (kept, F18) | `rxval.rs:94` |
| `vCBPWeightFactor` | 0.5 | none | `dcc.rs:1808` |
| Π smoothing | none (Eq. 4) | 0.9 | `dcc.rs:1247` |
| `vRescheduleTh` | 25 ms | absent | — |
| `vTxRand` | ±5 ms around LastTx + Max_ITT | U(0, 5) ms after a fixed phase | `wiring.rs:2431` |
| `vBSMRateTolerance` | 10 ms | up to 100 ms (step grid) | `generate.rs:196` |
| `vPosAccuracy` / `vElevAccuracy` | 1.5 m / 3 m at 68 % | 3.07 m / 4.59 m | `gauss_markov.rs:117`, `:126` |
| `vHVRemotePosEstIntMax` | 3,000 ms | none (§7) | `dcc.rs:1650-1654` |
| hard-braking g | 0.4 × 9.80665 | 0.4 × 9.8 in the DCC (§7) | `dcc.rs:1250` |
| EN 302 571 floor | none | on | `dcc.rs:1414` |

All other Table 21 values that the engine uses match: 10 Hz, 1 ms, 6 Mbps, 172, 150 ms,
0.28 m/s, 1 m, 15 points, 80/2,500 m, 1 m/s, 100/5,000/1,000/100 ms, 100 m, 0.3, 0/200 ms,
0.2/0.5 m, 75, 0.05, 25, 600/100 ms, 20/10/15 dBm, 0.5, 50/80 %, 5 ms, −92 dBm, 5 min, the
P2PCD 0.25 s/1 min/3, and `vMaxSuccessiveFail` 3. These are now primary: the earlier
"Rostami 2018 / Ahmad 2019 / NPRM" citations are confirmed. The one exception is the NPRM's
0.9 Π weight, which J2945/1 does not have.

## 3. Rows whose verdict changes

| Row | File | Old → new | Clause |
|---|---|---|---|
| veh S1 | `scms-real/validation/vehicle-and-roadside.md` | U → M | 5.1.3.3, 6.5.4 (verify on demand is allowed; no policy is mandated) |
| veh S2 | same | M → D | 6.5.2 `BSMSIGN-003`, Table 21: 450 ms. The effective 1,000 ms deviates (J1) |
| veh S4 | same | M (text: no J2945/1 rule; off is conformant) | 6.5.2 |
| veh S7 (new) | same | D | 6.5.2 `BSMSIGN-004` (J6) |
| veh P2, P4 | same | M (J2945/1 Table 10 and 21 added as a primary) | Table 10 |
| veh P8 | same | U → M | Table 10 (out of band, optional); Table 6 S3.6 (inline: N) |
| veh C4 | same | F (note: 2,100 m ± 100 m nominal) | 6.5.3, Table 21 |
| veh C6 | same | D, confirmed by the primary | 6.5.3 note (straight-line distance) |
| veh C10 | same | U → D | 6.3.6-DATAACC-039; 6.3.7 (J5) |
| veh C12 (new) | same | D | 6.5.3 `CERTCHG-002` (J6) |
| Summary counts | same | M 41→42, F 7, D 8→12, U 11→8 | — |
| E2.clock-ahead-5000s note | `v2xw-engine/tests/expectations/scms/scms-edge-cases.yaml` | US window ASSUMPTION → DEVIATION (verdict unchanged; the 5,000 s skew is still outside 30 s) | Table 11 (J7) |
| SC-D6 note | `scms-real/09-completion-plan.md` §7 | ASSUMPTION → DEVIATION | Table 11 |
| owner-actions row (purchase) | `scms-real/09-completion-plan.md` §2 | J2945/1 read 2026-10-31 | — |
| receiver window | `scms-real/07-config-testing-report.md`; `06-validation-summary.md` | ASSUMPTION → DEVIATION | Table 11 |
| veh C10; P7/P8 | `scms-real/rule-gap-specs-2026-10-27.md` §3-§4 | unverifiable → D (C10), M (P8) | as above |
| B1 | `docs/design/11-open-questions.md` | open → settled: 450 ms | 6.5.2, 5.1.3.2 |
| unverified names row | same | settled | Table 21; 6.3.6.5 |
| `cert_attach_interval_ms`, `pseudonym_change_interval_s` | `scms-real/constants.md` | via secondary → J2945/1 Table 21 | Table 21 |
| GNSS requirement anchor; §6.4 header; generator row; FCW row | `docs/design/04-models.md` | UNVERIFIED → verified (with deviations) / "not in J2945/1" | 6.3.6.5; 6.3.8; 6.3.6.16; 4.2.2 |
| SCMS/J2945-1 rows | `docs/design/05-protocols.md` §2.4 | secondary → primary; "P2PCD inline" corrected | 6.5.2, 6.5.3, Table 10 |
| density rows (clause id 6.3.6 → 6.3.8.4) | `v2xw-engine/tests/expectations/mac-congestion.yaml` | citation fixed, verdict unchanged | 6.3.8.4 Eq. 7 |
| cadence menu labels | `scms-real/08-settings-ux-and-gaps.md` | swapped labels corrected | 6.5.2 |
| veh C6 spec | `scms-real/10-completion-specs.md` §2.14 | design confirmed by the primary; notes 2,100 m and `CERTCHG-002` | 6.5.3 |
| J2945/1 row | `docs/design/findings/physical-layer-register.md` | Rostami values confirmed against the primary | 6.3.8.4, 6.3.8.7 |

## 4. Read before fixing

- J2945/1 is a light-vehicle standard. Heavy vehicles, buses and emergency vehicles keep
  their own rules (J2945/1B or none). Every fix below applies to light vehicles under the
  US profile. Where the engine cannot tell the classes apart cheaply, it applies to every
  US 802.11p vehicle, and the card says so.
- **G-ML is frozen in waves C and D**, and every J piece changes either BSM bytes, BSM
  timing or reception. So **all J pieces merge after M-D2**, in a wave J, before or
  alongside wave E's validation runs. Wave E's city runs should wait for J1, J2, J4 and J5:
  those four move every US number.
- There is one exception: the `vCertChangeDistance` value (J8 part 1) can fold into D3's
  R1 piece veh C6 (§2.14 of 10). It changes the same lines and has the same golden commit.

## 5. Fix specs

Each piece follows the rules of 10 §0: one design step, at most two heavy commands, every
accept test shown red with its fault injected, and the goldens re-blessed in their own
commit. File paths are under `crates/` unless stated.

### J1. Certificate cadence per message type: BSM 450 ms, CAM 1 s (F4)

- **Today:**
  - `v2xw-engine/src/scenario/schema.rs:4292-4309` defaults `full_cert_every_ms` to 1000.
  - `wiring.rs:3686-3697` turns it into one policy, and `wiring.rs:3950` applies it to both
    BSM and CAM. The same happens at `hosted.rs:410`.
  - The menu (`options.rs:2573-2584`) labels 1,000 ms "the SAE J2945/1 reading" and 450 ms
    "the NYC CV pilot's".
  - Ten shipped scenarios set 1,000 explicitly (`scenarios/{pq-signatures, revocation-latency,
    midtown-street-life, ccms-lifecycle, vru-grid, pseudonym-strategies,
    threshold-revocation-2of2, pdr-vs-distance, berlin, phase2-manhattan}.yaml`).
- **Change:**
  - `full_cert_every_ms: Option<u64>`. Unset gives each message type its standard: BSM
    `SignerIdPolicy::SAE_450MS` (J2945/1 6.5.2), CAM `ETSI_1S` (TS 103 097). A set value
    overrides both, as it does today.
  - Remove the explicit 1,000 from the US scenarios. Berlin keeps 1,000. For Midtown and
    the benchmark, the owner should confirm that "once a second" was never a deliberate
    study choice.
- **Accept tests:**
  - `wiring::tests::bsm_and_cam_take_their_own_standard_cadence_when_unset`: a US
    scenario with the field unset gives a BSM policy of 450 ms and a CAM policy of
    1,000 ms. *Fault:* keep one policy at 1,000. Red.
  - Engine `security_lifecycle.rs::a_lone_obu_attaches_its_certificate_to_every_fifth_bsm`:
    60 s, one node, 10 Hz, full-certificate share 0.20 ± 0.01. *Fault:* default 1,000.
    Share 0.10, red.
  - `options` string test: the 450 entry names "SAE J2945/1 vMaxCertDigestInterval".
    *Fault:* the old labels. Red.
- **Settings:** `security.signer_id_policy.full_cert_every_ms`, select, in ms:
  - unset "Per standard (BSM 450 ms, SAE J2945/1; CAM 1 s, ETSI TS 103 097)";
  - 450 "Every 450 ms (SAE J2945/1 6.5.2)";
  - 1000 "Once a second (ETSI TS 103 097)";
  - 0 "Every message";
  - "Other…".
- **Goldens:** G-SCMS, all three files: BSM byte counts and certificate shares. G-ML moves:
  unknown-signer counts change. Rows `docs/validation/sensitivity-security.md:24` and
  benchmark §9.5 are re-measured in wave E (`verdict: open`, full-certificate share up,
  `unverified_ratio` down).
- **Size:** 1 piece.
- **Files:** `v2xw-engine/src/scenario/{schema, validate, options, guide, publish}.rs`,
  `v2xw-engine/src/{wiring, hosted}.rs`, `docs/guides/configuration-reference.md`, the
  scenario files above.

### J2. EDCA per Table 18; UP 5 for a BSM, UP 7 for a critical BSM (C5, C7)

- **Today:**
  - `AccessCategory::{cw_min, cw_max, aifsn}` (`v2xw-radio/src/types.rs:398-425`) are
    EN 302 663's.
  - `build_mac` (`wiring.rs:4290-4318`) overrides only on explicit keys.
  - `access_category` (`wiring.rs:4271-4278`) defaults periodic and event traffic to AC_VO.
  - Frames are queued at `run.rs:6612` by message type alone.
- **Change:**
  - J2a (radio crate): add `EdcaParams` preset sets: `EN302663` (today's) and `J2945_1`
    (Table 18). `build_mac` picks `J2945_1` under the US region with 802.11p, and
    EN 302 663 otherwise. Explicit `radio.mac.*` keys still win. CWmax is carried even
    though OCB broadcast never doubles CW, so the card can show it.
  - J2b (engine): a BSM is queued at AC_VI unless its Part II carries a critical flag
    (hard braking, ABS, traction, stability: 3.1.2), in which case it goes at AC_VO. The
    flag is already in the BSM extras that `runtime.rs` returns for each frame. Other
    message types keep today's mapping. Correct the `schema.rs:2576-2579` doc.
- **Accept tests:**
  - `types.rs` or `mac.rs` `the_j2945_edca_set_is_table_18`: VI (15, 1023, 4), VO (3, 7, 2),
    BE (15, 1023, 6), BK (15, 1023, 9), and AIFS(VI) 84 µs. *Fault:* VI CWmin 7. Red.
  - Engine `radio_access_layers.rs::a_plain_bsm_contends_at_ac_vi_and_a_hard_braking_bsm_at_ac_vo`.
    *Fault:* map every BSM to VO. Red.
  - Engine: over 20 s, the backoff slots drawn for a node's BSMs reach a maximum above 3
    and none exceeds 15. *Fault:* the EN 302 663 preset. The maximum is 3, red.
- **Settings:**
  - `radio.mac.edca_profile`, select:
    - unset "By region";
    - `sae-j2945-1` "SAE J2945/1 Table 18 (BSM at AC_VI, critical BSM at AC_VO)";
    - `en-302-663` "ETSI EN 302 663 (OCB defaults)".
  - The `periodic_access_category` labels gain "(SAE J2945/1 UP 5)" on `vi`.
- **Goldens:** G-SCMS and G-ML, all medium- and high-tier US runs: access delay, collisions
  and PDR.
  - `mac-congestion.yaml` row `periodic_access_category → vi, aifs_us up` is now the
    default case. The row is rewritten as `→ vo, aifs_us down`.
  - The `cw_min_vo`/`aifsn_vo` rows move to `cw_min_vi`/`aifsn_vi` for the BSM.
  - Re-run in wave E. Predicted: PDR at density up and access delay up by about
    0.1 ms per deferral.
- **Size:** 1.5 pieces (J2a 1, J2b 0.5).
- **Files:** `v2xw-radio/src/{types, mac}.rs`, `v2xw-engine/src/{wiring, run}.rs`,
  `scenario/{schema, options, guide}.rs`, `configuration-reference.md`,
  `v2xw-engine/tests/expectations/mac-congestion.yaml`.

### J3a. Congestion-control equations (D3, D4, D6, D14, D17)

- **Today:**
  - `on_cbr` stores the raw CBR (`dcc.rs:1808`) and steps SUPRA on every sample
    (`:1810`).
  - `smooth_quality` uses a weight of 0.9 (`:1582-1589`, `:1247`).
  - `SaeJ2945Dcc::with_params` installs `En302571Floor::CONFORMANT` (`:1414`), and the
    gate applies it (`:1825`).
  - `PerWindow` counts true sends (`:1677-1760`).
- **Change:**
  - Eq. 2: `cbp = 0.5·raw + 0.5·cbp_prev`.
  - Eq. 4: Π = min(avg, 0.3). The current behaviour is kept as a `J2945Params::FMVSS150_NPRM`
    preset (`per_weight` 0.9).
  - SUPRA (Eq. 9-11) steps once per generated BSM. A new `on_generated` hook is called from
    `gate` when the frame is a BSM.
  - The SAE model's floor is `disabled()` unless the region is the EU. This needs no
    setting, because a US unit is not under EN 302 571.
  - Optional, if the piece has room: PER from the `msgCnt` span of the first and last
    receptions per RV in δk, for RVs with two or more. `note_received` takes the msgCnt.
- **Accept tests:**
  - `cbp_is_smoothed_with_weight_one_half`: 0.8 then 0.0 gives 0.4 then 0.2. *Fault:* raw.
    Red.
  - `channel_quality_is_the_capped_mean_per`: 0.2 gives Π = 0.2 at once, and 0.5 gives
    0.3. *Fault:* weight 0.9. 0.02, red.
  - `a_us_sae_unit_has_no_en302571_floor`: at CBR 0.75 a 0.46 ms frame is gated at Max_ITT,
    not at about 318 ms. *Fault:* the floor on. Red.
  - `supra_steps_once_per_bsm`: three CBR samples with no BSM leave RP unchanged.
    *Fault:* step in `on_cbr`. Red.
  - If the PER is done: `per_ignores_edge_losses_like_the_msgcnt_span`, a lost first frame
    is not counted. *Fault:* true counts. Red.
- **Settings:** `radio.dcc.channel_quality`, select:
  - unset "SAE J2945/1 Eq. 4 (capped mean PER)";
  - `fmvss150-nprm` "FMVSS 150 NPRM (0.9 smoothing)".

  `radio.dcc.en302571_floor`, select:
  - unset "By region (EU on, US off)";
  - `on`;
  - `off`.
- **Goldens:** G-SCMS and G-ML only where CBR exceeds 0.62 or density exceeds 25. That is
  `phase2-path` if dense; the 60 s `i-s1` probably does not move. The DCC rows in
  `radio-*.yaml` and `scenarios/sensitivity/test-mac-congestion.yaml` are re-run in wave E.
- **Size:** 1 piece.
- **Files:** `v2xw-radio/src/dcc.rs`, `v2xw-engine/src/wiring.rs` (`build_dcc` `:4364`),
  `v2xw-engine/src/run.rs` (only if the PER item is done), `scenario/{schema, options}.rs`,
  `configuration-reference.md`.

### J3b. The J2945/1 schedule (C4, D12, D13)

- **Today:**
  - The BSM is decided on the 100 ms node step (`generate.rs:196-198`,
    `generator.rs:813-826`).
  - An exception sets `pending`, which lowers the interval to `vMin_ITT` at the next step
    (`dcc.rs:1469-1480`, `:1660-1668`).
  - Each frame gets an extra U(0, 5) ms (`wiring.rs:2431`).
- **Change:** implement 6.3.8.5-6.3.8.8 literally, on the node's clock:
  - `NextScheduledMsgTime = LastTxTime + Max_ITT + U(−vTxRand, +vTxRand)`, drawn per BSM
    from the node's keyed stream.
  - The engine wakes the node at that instant. Reuse the WP-34 `align_to`/`fire_at` path
    (`generate.rs:306-372`), which already wakes a node off-grid.
  - Every 100 ms, apply the decision rules in this order:
    - a new critical event: transmit now;
    - critical or dynamics, when `Next − LastTx > 100 + vTxRand`: transmit now;
    - dynamics only when `Next − now ≥ vRescheduleTh` (25 ms);
    - when `Next − (LastTx + Max_ITT) ≥ 25 ms`: move the BSM earlier, to
      `max(now, LastTx + Max_ITT)`.
  - The onset of a critical event also runs the decision at once, not at the next step.
  - Drop the hand-off jitter under J2945 (it is replaced by RandOffset).
  - Give the `ts103759.rs:920` BSM profile a critical-onset exemption, because a first
    critical BSM may follow the previous one by less than 100 ms.
- **Accept tests:**
  - `generator.rs` `a_150_ms_max_itt_is_kept_within_the_tolerance`: N_s = 37.5, 40 BSMs,
    every interval in [145, 155] ms. *Fault:* the step grid. 100/200 ms, red.
  - `rand_offset_is_symmetric`: 10,000 intervals at 100 ms have a mean of 100 ± 0.2 ms, a
    minimum ≥ 95 and a maximum ≤ 105, and some fall below 100. *Fault:* U(0, 5). None below
    100, red.
  - `a_new_critical_event_goes_out_at_once`: the onset at LastTx + 30 ms under a 600 ms
    Max_ITT gives a BSM at the onset instant. *Fault:* wait for the step. Red.
  - `a_tracking_error_within_25_ms_of_the_next_bsm_does_not_reschedule`. *Fault:* no
    threshold. An extra BSM, red.
  - `ts103759` `the_first_critical_bsm_is_not_a_frequency_violation`. *Fault:* drop the
    exemption. Red.
- **Settings:** none. This is the standard's schedule. The old grid is not kept.
- **Goldens:** G-SCMS and G-ML everywhere, because the BSM instants move. Predicted (wave
  E, `open`): the IPG spread narrows, consecutive collisions fall under load, and the EEBL
  latency falls by up to 100 ms.
- **Size:** 1.5 pieces.
- **Files:** `v2xw-msg/src/generator.rs`, `v2xw-node/src/generate.rs`, `v2xw-radio/src/dcc.rs`,
  `v2xw-engine/src/{run, wiring}.rs`, `v2xw-threat/src/ts103759.rs`.

### J4. The BSM position is the centre of the vehicle rectangle (A4)

- **Today:**
  - Kinematics are referenced to the rear-axle centre (`v2xw-mobility/src/views.rs:89`,
    `classes.rs:8`).
  - The GNSS truth is that point (`run.rs:4690-4747`), and the BSM sends the belief as it
    is (`runtime.rs:2524`, `bsm.rs:2121`).
  - Receivers assume the centre (`v2xw-node/src/apps.rs:233`, `:291`;
    `v2xw-threat/src/consistency.rs:35`).
- **Change:**
  - Compute the GNSS truth at the J2945/1 position reference: shift it forward along the
    heading by `centre_offset_m(class, dims) = (rear axle from the front) − length/2`. The
    rear-axle distance is already in `v2xw-mobility/src/calibration.rs:1205`; move it next
    to `classes.rs` as a public helper.
  - Everything node-side then follows: the BSM, the path history and the apps' own
    position. The antenna position (the radio index) does not change.
  - The CAM's own reference (the front centre, TS 102 894-2) is a separate ETSI item. It is
    recorded and not done here.
- **Accept tests:**
  - `a_bsm_reports_the_centre_of_the_vehicle_rectangle`: a 4.5 m car with its rear axle at
    (0, 0), heading east, GNSS noise off. The decoded BSM is at x = offset ± 0.01 m.
    *Fault:* no shift. x = 0, red.
  - `a_car_queued_behind_a_bus_does_not_overlap_it_in_their_bsms`, with a 1.0 m gap and
    perfect GNSS. *Fault:* no shift. An overlap is reported, red.
- **Settings:** none.
- **Goldens:** G-SCMS and G-ML everywhere (every position). Wave E re-runs the false-positive
  detector rows (predicted: overlap and intercept reports fall in mixed queues).
- **Size:** 1 piece.
- **Files:** `v2xw-engine/src/run.rs` (`update_beliefs`), `v2xw-mobility/src/{classes,
  calibration}.rs`, a new engine test.

### J5. Path history and message counter (B3, B9, B26, B28, B34)

- **Today:**
  - `PathHistoryParams::J2945_1` is 300 m and 300 m (`j2945.rs:69-76`).
  - `set_identity` clears the history (`vehicle.rs:216-223`), called at `runtime.rs:2231`.
  - An empty history sends no PathHistory (`j2945.rs:238-240`).
  - `msg_count: 0` (`generator.rs:775`).
- **Change:**
  - `min_distance_m` 200 and `max_chord_m` 210 (Table 21). The module table is updated
    from "recalled" to "J2945/1 Table 21".
  - At a change the history is kept by default (J2945/1 has no clearing rule, and
    6.3.6-DATAACC-039 needs ≥ 200 m). Clearing becomes a research option.
  - When the history is empty, the BSM carries one point at the latest fix before the
    anchor, so PathHistory is never absent (Table 20). A newborn device's first fix becomes
    its first crumb, which is the 6.3.7-DATAPERSIST-004 start-up behaviour.
  - The first `msgCnt` is drawn from the node's keyed stream (6.3.6-DATAACC-002).
- **Accept tests:**
  - `the_path_history_covers_200_m_with_chords_of_at_most_210_m`, on a straight 1 km drive.
    *Fault:* 300/300. Red.
  - `a_pseudonym_change_keeps_the_path_history`. *Fault:* clear. Red. Under `clear` it is
    empty.
  - `every_bsm_carries_a_path_history`, including the first. *Fault:* no seed. Red.
  - `the_first_msg_count_is_random`: over 1,000 nodes, at least 100 distinct first values.
    *Fault:* start at 0. All are 1, red.
- **Settings:** `messages.bsm.path_history_on_change`, select:
  - `keep` "Keep (SAE J2945/1 6.3.6.16)", the default;
  - `clear` "Clear at each pseudonym change (privacy research)".
- **Goldens:** G-SCMS and G-ML everywhere, because BSM sizes change. Wave E rows: BSM bytes
  are predicted equal or lower on straight roads. The privacy linkage rows
  (`v2xw-threat/src/privacy.rs` trackers) are predicted to show higher linkability across
  a change under `keep`.
- **Size:** 1 piece.
- **Files:** `v2xw-msg/src/{j2945, generator}.rs`, `v2xw-node/src/{vehicle, runtime}.rs`,
  `scenario/{schema, options, guide}.rs`, `wiring.rs` (pass the setting),
  `configuration-reference.md`.

### J6. Critical events: the ABS flag, the full certificate, and no change (B24, F5, F10)

- **Today:**
  - `events()` never sets ABS (`vehicle.rs:258-277`).
  - `signer_id_for` has no event input (`secure.rs:876-891`).
  - `rotate_in_use` has no event input (`stores.rs:293-300`, called at `runtime.rs:1195`
    and `vru.rs:1509`).
- **Change:**
  - `bus.abs_active` sets the ABS flag. The critical-flag test is
    hard braking | ABS | traction | stability (3.1.2).
  - `signer_id_for(msg, t, critical)` returns `Certificate` when `critical` is set
    (`BSMSIGN-004`). This does not restart the cadence, as with the new-neighbour trigger.
  - `rotate_in_use(t, fix, critical)`: while `critical` is set, only expiry or revocation
    changes the pseudonym (`CERTCHG-002`). The flag is also the exception to the 5-minute
    rule (`CERTCHG-001`).
- **Accept tests:**
  - `abs_activity_sets_the_abs_flag`. *Fault:* omit. Red.
  - `a_critical_bsm_carries_the_full_certificate`, 100 ms after the last one. *Fault:*
    ignore the flag. A digest, red.
  - `no_change_while_a_critical_flag_is_set` under `J2945_1`, with 6 min and 3 km due: the
    change waits for the flag to clear, and expiry still changes. *Fault:* ignore the flag.
    Red.
- **Settings:** none.
- **Goldens:** G-SCMS only where a run has hard braking (`ttc-triggered-brake` surely). G-ML
  moves where events occur.
- **Size:** 1 piece.
- **Files:** `v2xw-node/src/{vehicle, secure, runtime, stores, vru}.rs`.

### J7. The relevance window of the US profile, 30 s each way (F16)

- **Today:** `BEACON_MAX_AGE_S` 2.0 and `MAX_FUTURE_S` 0.040 (`rxval.rs:148`, `:157`) for
  every profile. The menu cites only C2C-CC (`options.rs:2543-2560`).
- **Change:**
  - Per-profile defaults: IEEE 1609.2 / SAE gets 30 s past and 30 s future for BSM (Table 11)
    and keeps 600 s for other messages. ETSI keeps RS_BSP_168.
  - The three keys still override.
  - Update the E2 row's expectation: skews below 30 s are now accepted. Re-run it in wave E.
- **Accept tests:**
  - `the_us_bsm_window_is_thirty_seconds_each_way`: −20 s and +20 s are accepted, −31 s and
    +31 s are discarded. An ETSI CAM is still limited to 2 s and 40 ms. *Fault:* C2C-CC
    values for the US. Red.
  - The engine replay test: a replay 10 s old reaches the detectors under the US profile.
    *Fault:* the 2 s window. Red.
- **Settings:**
  - `security.scms.rx_beacon_max_age_s`: unset "By profile (US 30 s, SAE J2945/1 Table 11;
    EU 2 s, C2C-CC RS_BSP_168)", 30, 2, "Other…", in s.
  - `rx_max_future_s`: unset "By profile (US 30 s; EU 40 ms)", 30, 0.04, in s.
- **Goldens:** G-SCMS where a run has skew or replay. The edge cases E2 and
  `attack-replay` move: replays 2-30 s old are no longer dropped at the security layer, so
  detection has to come from the MBD.
- **Size:** 0.5 piece.
- **Files:** `v2xw-node/src/rxval.rs`, `v2xw-engine/src/wiring.rs`, `scenario/{options,
  guide}.rs`, `configuration-reference.md`,
  `v2xw-engine/tests/expectations/scms/scms-edge-cases.yaml`.

### J8. `CERTCHG` distance and the US default rule (F9)

- **Today:**
  - `RotationPolicy::J2945_1.min_distance_m` 2000 (`stores.rs:131`).
  - `CERTCHG_DISTANCE_CM` 200,000 (`v2xw-proto/src/pseudonym.rs:35`).
  - The default strategy is `time` (`schema.rs:4349-4351`), so a stationary vehicle changes
    every 5 min.
- **Change:**
  - **Part 1**, which can fold into D3 R1, veh C6:
    - `J2945_1` takes 2,100 m of **straight-line** distance (§2.14). The 100 m tolerance is
      documented, because any change at 2,000 m or more is conformant.
    - The NYC pilot keeps its 2 km.
  - **Part 2**, in wave J: with `strategy` unset, US 1609.2 scenarios get `j2945-1` and
    others keep `time`.
- **Accept tests:**
  - `the_j2945_distance_is_2100_m`: after 6 min, a displacement of 2,050 m gives no
    change and 2,150 m gives a change. *Fault:* 2,000. A change at 2,050 m, red.
  - `a_us_scms_scenario_defaults_to_the_j2945_rule`. *Fault:* `time`. Red.
- **Settings:**
  - `security.pseudonym_change.strategy`: the label of `j2945-1` becomes "SAE J2945/1
    (5 min unless within 2,100 m of the last change; not during a critical event)", and
    unset becomes "By region (US: SAE J2945/1)".
  - `distance_m` menu: 2100 "2,100 m (SAE J2945/1)", 2000 "2 km (NYC pilot)", in m.
- **Goldens:** part 1 is in C6's golden commit. Part 2 moves G-SCMS and G-ML for default
  US runs (fewer changes in slow traffic).
- **Size:** 0.5 piece (part 1 adds about 0.25 to C6).
- **Files:** `v2xw-node/src/stores.rs`, `v2xw-proto/src/pseudonym.rs`,
  `scenario/{schema, options, guide}.rs`, `configuration-reference.md`.

### J9. The "transmit power used" WSMP extension (C9)

- **Today:** `NetMeta::for_bsm` has `WsmpExtensions::NONE` (`v2xw-net/src/frame.rs:123`,
  `netlayer.rs:191-196`).
- **Change:** a BSM's WSM carries `transmit_power` (3 octets), with the value the RP
  of 6.3.8.7 rounded to dBm. Channel and data rate stay optional and absent.
- **Accept tests:** `a_bsm_wsm_carries_the_transmit_power_extension`: the header is
  5 + 3 = 8 octets and the value is round(RP). *Fault:* `NONE`. 5 octets, red.
- **Settings:** none.
- **Goldens:** G-SCMS and G-ML everywhere (+3 B per BSM).
- **Size:** 0.5 piece.
- **Files:** `v2xw-net/src/{frame, netlayer, wsmp}.rs`, and the call that builds the BSM
  `NetMeta` with the frame's power (`v2xw-engine/src/run.rs` or `run/air_bytes.rs`).

### J10. A J2945/1-conformant GNSS preset, with the heading latch (B13, B14, B18)

- **Today:** the default is the Reid fit (`gauss_markov.rs:117-126`, `:294-310`), with no
  heading noise.
- **Change:**
  - Add a `j2945-1` quality preset. σ and τ are fitted at construction so that under open
    sky the 68th percentiles are 1.5 m horizontal and 3 m vertical, speed 0.28 m/s, and
    heading 2° above 12.5 m/s and 3° below.
  - Add the latch: the heading is held below 1.10 m/s and released above 1.38 m/s
    (6.3.6.10).
  - The urban classes scale as today: J2945/1 binds only under open sky (A.7).
  - **Owner decision:** whether the US OBU default becomes `j2945-1`. Recommended,
    because conformance is a deployment prerequisite. Reid stays as the "measured
    production receiver" choice.
- **Accept tests:**
  - `the_j2945_preset_meets_the_open_sky_accuracies`, over 10,000 fixes. *Fault:* the Reid
    parameters. 3.07 m, red.
  - `heading_latches_below_1_10_and_unlatches_above_1_38`, on a speed ramp. *Fault:* no
    latch. The heading wanders at standstill, red.
- **Settings:** `vehicles.gnss.quality`, select:
  - `j2945-1` "SAE J2945/1 open sky (1.5 m / 3 m at 68 %)";
  - `reid-2019` "Production receiver, measured (3.07 m at 68 %)";
  - `rtk` "RTK (0.26 m)".
- **Goldens:** none if the default does not change. Otherwise G-SCMS and G-ML everywhere.
- **Size:** 1 piece.
- **Files:** `v2xw-mobility/src/gnss/gauss_markov.rs`, `scenario/{schema, options}.rs`,
  `wiring.rs` (`build_gnss`), `configuration-reference.md`.

**Total:** 10.5 pieces, about 32 h, of which about 0.25 is in D3 R1.

## 6. Schedule (wave J, after M-D2)

Ownership in wave J, one owner per hot file:

| Agent | Owns |
|---|---|
| **JA, radio and engine loop** | `v2xw-radio/*`, `v2xw-msg/src/generator.rs`, `v2xw-node/src/generate.rs`, `v2xw-engine/src/{run, wiring, hosted}.rs`, `v2xw-threat/src/ts103759.rs`, `v2xw-net/*` |
| **JB, node security and settings** | `v2xw-node/src/{secure, stores, rxval, runtime, vru}.rs`, `v2xw-proto/src/pseudonym.rs`, `v2xw-engine/src/scenario/*`, `configuration-reference.md`, `scenarios/*.yaml` |
| **JC, message content and position** | `v2xw-msg/src/j2945.rs`, `v2xw-node/src/vehicle.rs`, `v2xw-mobility/src/{classes, calibration, gnss/*}.rs` |

A piece that needs a line in another agent's file states that line in its spec, and the
owner lands it in the round shown.

| Round | JA | JB | JC |
|---|---|---|---|
| R1 | J2a | J6 (its `vehicle.rs` ABS line is landed by JC in R1) | J5 core (`j2945.rs`, `vehicle.rs`, the ABS line) |
| R2 | J3a | J7 + J8 part 2 | J10 core |
| R3 | J3b | J1 settings and the scenarios; the J5 and J10 keys | J4 helper (`classes.rs`) and its test fixture |
| R4 | J3b tail; the wiring lines for J1, J5, J7, J10 (`hosted.rs` too); J4's `run.rs` change; J9 | J5's `runtime.rs` lines (change hook, `msgCnt` seed); J2b settings | — |
| R5 | J2b | — | — |

- **Golden commits, in merge order:** J2, J3a, J3b, J4, J5, J9, J1, J6, J7, J8 part 2, J10
  (only if the owner changes the default). Each is re-blessed on its own commit with its
  cause, as in 10 §0.
- **Expectations:** every new row is written `verdict: open` with the predicted direction,
  and wave E runs them.
- **Wave E dependency:** wave E's US city baselines should be taken after J1, J2, J4 and J5
  have merged. Otherwise they are measured twice.

## 7. Recorded and skipped (negligible)

- **B4.** An all-off `ExteriorLights` for 2 s after the lights change. A few bytes per
  event.
- **B33.** No path history or heading carried over from before a vehicle enters the map. It
  is the "initially" exception of DATAACC-039, and a vehicle that enters mid-trip has no
  previous shutdown to restore.
- **TemporaryID and L2 over an outage without a certificate change** (`run.rs:3309-3335`).
  J2945/1 re-randomises both at start-up. The engine's are tied to the certificate, so they
  stay. An observer links through the unchanged certificate digest anyway, so nothing
  observable changes. The `CERTCHG-003` power-cycle exception is likewise not modelled.
- **D8.** The 3,000 ms cap on the remote estimate. `vMaxSuccessiveFail` 3 with
  `Max_ITT` ≤ 600 ms keeps the estimate younger than about 2.4 s, so the cap never binds.
- **The DCC's hard-braking threshold** is 0.4 × 9.8 (`dcc.rs:1250`), against 0.4 g at
  9.80665 elsewhere. The difference is 0.0027 m/s².
- **F18.** The overdue-CRL tolerance stays at 2 weeks. Table 13 lists it as a field the
  SCMS may update, and the SCMS Manager EE requirement is the later policy.
- **B30.** The path-prediction confidence curve. A.6 and its Table A3 are informative. The
  engine's curve meets the normative radius rules.
- **H3.** The comment at `apps.rs:604` should drop "J2945/1's 1 s staleness". It is text
  only, and can be fixed by whoever next edits the file.
