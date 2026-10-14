# Test: medium access and congestion

Round-15 engine test of the 802.11p EDCA, MAC queue, DCC and C-V2X SPS settings, and
of offered load (vehicles, rate, payload). Each setting is moved one at a time on a
fixed seed, and the metric is compared with what theory predicts.

- **Specs:** `scenarios/sensitivity/test-mac-congestion.yaml` (802.11p) and
  `test-mac-congestion-sidelink.yaml` (LTE-V2X).
- **Base:** `dsrc-dense.yaml`, about 100 vehicles in a 300 m square, so every vehicle
  is in range of every other. DCC is off and the rate is 20 Hz unless a row says
  otherwise.
- **Sparse-channel rows:** `crates/v2xw-engine/tests/expectations/mac-congestion.yaml`,
  run on the 40-vehicle DSRC catalogue family.

## Engine fix: same-slot collisions never happened

**Found:** at 100 vehicles, 20 Hz and CBR 0.61, collisions were only 0.04 % of
attempts. Raising CWmin from 3 to 63 *raised* collisions instead of lowering them.

**Cause:** carrier sense counted a neighbour's frame from its very first nanosecond
(`Phy::cca`, `start <= now`). Arrivals carry no propagation delay, and the back-off
slots of every station end on the same nanosecond. So when two back-offs ended in the
same slot, whichever station the event queue served first transmitted, and the other
saw a busy medium and deferred.

The real standard does not work like that. The slot time is a budget:
`aSlotTime = aCCATime + aRxTxTurnaroundTime + aAirPropagationTime + aMACProcessingDelay`
(IEEE 802.11-2020 10.3.7). It exists because a station cannot detect a frame that
began in the same slot. That collision is the one that CWmin exists to spread out.

**Fix:** `timing::CCA_TIME` = 8 µs (Table 17-21, 10 MHz). CCA now reports a frame only
8 µs after it begins (`Phy::sensed_energy_dbm`), and the engine dates the freeze of
the countdown from that same instant (`medium_busy_since`).

**After the fix** (debug build of the commit, same base):

| Change | Collision rate | PDR | Access delay |
|---|---|---|---|
| CW 3 (base) | 0.136 | 0.862 | 0.25 ms |
| CWmin 3 → 63 | 0.063 (×0.47) | 0.936 | 0.69 ms (×2.8) |
| AC_VO → AC_BK (CW 15, AIFSN 9) | 0.104 (×0.77) | | |
| 10 → 20 Hz | 0.033 → 0.136 (×4.1) | | |

The broadcast literature reports the same order of magnitude at this load (Jiang, Chen
and Delgrossi, VTC 2008; Ma and Chen, IEEE TVT 2007).

**Results that changed:** every 802.11p run with contention. Expect more collisions,
lower PDR and lower access delay, because overlapping frames no longer queue one
behind the other. Sidelink runs are unchanged, because their CCA is `cv2x.rs`.

## Loader fix: EDCA numbers for a queue nobody uses

`radio.mac.cw_min_vi` and `aifsn_vi` changed nothing unless a message was mapped to
AC_VI. Likewise `cw_min_vo` and `aifsn_vo` did nothing under `etsi-profiles` without
DENMs, and `event_access_category` did nothing without a GeoNetworking stack. Each
combination is now refused, with a message that says how to make the setting take
effect (`validate.rs` `mac_queue_in_use`).

## Results

The base and changed values are fingerprint metrics. "Pre-fix" rows ran on the
current-main release binary, before the CCA fix. Their direction does not depend on
it, because they test load, payload or DCC rather than same-slot contention.

### 802.11p, offered load (pre-fix)

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| vehicles (trips) | 150 → 300 | cbr_mean | up, ×1.3–2.3 | 0.613 → 0.836 (×1.36) | yes |
| vehicles | 150 → 300 | collision_rate | up | 0.0004 → 0.043 | yes |
| vehicles | 150 → 300 | pdr | down | 0.9995 → 0.957 | yes |
| vehicles | 150 → 300 | access_ms | up | 0.47 → 60.0 | yes |
| vehicles | 150 → 300 | e2e_ms_mean | up | 15.3 → 102 | yes |
| messages.rate_hz | 10 → 20 | cbr_mean | up, ×1.5–2.2 | 0.309 → 0.613 (×1.99) | yes |
| messages.rate_hz | 10 → 20 | collision_rate | up | 0 → 0.0004 (post-fix 0.033 → 0.136) | yes |
| messages.rate_hz | 10 → 20 | access_ms | up | 0.11 → 0.47 | yes |
| messages.rate_hz | 10 → 20 | aoi_ms | down | 64.9 → 40.4 | yes |
| messages.rate_hz | 10 → 20 | ipg_ms_mean | down, ×0.4–0.65 | 100 → 50.0 | yes |
| messages.path_history_points | 15 → 0 | psdu_bytes | down | 199 → 189 | yes |
| messages.path_history_points | 15 → 0 | airtime_us | down | 314 → 298 | yes |
| messages.path_history_points | 15 → 0 | cbr_mean | down | 0.613 → 0.579 | yes |
| certificate on every message | digest → cert | psdu_bytes | up | 199 → 330 | yes |
| certificate on every message | digest → cert | cbr_mean | up | 0.613 → 0.885 | yes |
| certificate on every message | digest → cert | collision_rate | up | 0.0004 → 0.037 | yes |

### 802.11p, EDCA

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| radio.mac.cw_min_vo | 3 → 63 | collision_rate | down | pre-fix 0.0004 → 0.0007 (wrong way); post-fix 0.136 → 0.063 | fixed |
| radio.mac.cw_min_vo | 3 → 63 | pdr | up | pre-fix −0.0 %; post-fix 0.862 → 0.936 | fixed |
| radio.mac.cw_min_vo | 3 → 63 | access_ms | up | post-fix 0.25 → 0.69 | yes |
| radio.mac.aifsn_vo | 2 → 9 | access_ms | up | 0.47 → 1.20 | yes |
| radio.mac.aifsn_vo | 2 → 9 | collision_rate | up* | post-fix 0.136 → 0.165 | yes |
| radio.mac.periodic_access_category | vo → bk | collision_rate | down | pre-fix ×4.7 (wrong way); post-fix 0.136 → 0.104 | fixed |
| radio.mac.periodic_access_category | vo → bk | access_ms | up | 0.47 → 1.49 | yes |
| radio.mac.periodic_access_category | vo → vi | access_ms | up | 0.47 → 0.58 | yes |
| radio.mac.cw_min_vi (under AC_VI) | 7 → 63 | backoff_us | up | 27.6 → 252 | yes |
| radio.mac.queue_length | 64 → 16 | mac_drops | up | not run (corrected base, see below) | |
| radio.mac.queue_length | 64 → 16 | access_ms | down | not run (corrected base, see below) | |

\* AIFSN: this expectation was first written as "same", on the reasoning that one
AIFSN for every station does not separate them. The measured +21 % is explained by
Bianchi (2000): every busy period now ends with 91 µs more idle time, during which more
frames arrive and defer, so more of them contend for the same few slots. The row was
corrected to "up".

**Queue:** the first queue row asked for drops at 60 vehicles and 40 Hz, and none came
at a queue of 16 or 64. A 16-frame queue overflows only when the channel is overloaded
(offered airtime above 1). The base is now 300 trips at 40 Hz with a certificate on
every frame, an offered load of about 5.4. `stage_queue_ms` was also the wrong metric:
it measures the signing queue, while the MAC queue wait is part of `access_ms`.

### DCC (pre-fix)

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| radio.dcc.enabled (J2945/1) | off → on | tx_rate_hz | down | 9.38 → 8.78 | yes |
| radio.dcc.enabled (J2945/1) | off → on | cbr_mean | down | 0.312 → 0.279 | yes |
| radio.dcc.enabled (J2945/1) | off → on | aoi_ms | up | 64.9 → 68.8 | yes |
| radio.dcc.max_itt_ms | 600 → 100 | tx_rate_hz | up | 8.78 → 9.39 | yes |
| radio.dcc.max_itt_ms | 600 → 100 | aoi_ms | down | 68.8 → 62.4 | yes |
| radio.dcc.power_control | on → off, CBR 0.8 | cbr_mean | up | not run (corrected base, see below) | |
| radio.dcc.min_power_dbm | 10 → 20, CBR 0.8 | cbr_mean | up | not run (corrected base, see below) | |
| ETSI adaptive (LIMERIC) | off → on, 300 trips | tx_rate_hz | down | 17.9 → 13.5 | yes |
| ETSI adaptive | off → on, 300 trips | cbr_mean | down | 0.836 → 0.584 (target 0.68) | yes |
| radio.dcc.adaptive_beta | 0.0012 → 0.05, 300 trips | tx_rate_hz | up | not run (corrected base, see below) | |
| radio.dcc.reactive_table | a1 → a2 | tx_rate_hz | changes | 7.60 → 9.64 | yes |
| radio.dcc.reactive_hold_ms | 0 → 1000 | tx_rate_hz | changes | not run (corrected base, see below) | |

**Corrected expectations:**

- **vRP_Min and power control:** at CBR 0.28, SUPRA's target is vRP_Max for any
  busy percentage below vMinCU 0.50 (Ahmad 2019 Eq. 9), so vRP_Min cannot bind. The
  first run measured 0.279 → 0.279. The +12 % that switching power control off gave on
  that base is only the start-up ramp from vRP_Initial 15 dBm. Both rows now run at
  CBR about 0.8 (300 trips, vMax_ITT 100 ms).
- **LIMERIC β:** below the 0.68 target LIMERIC does not bind (18.49 → 18.49 Hz). The
  row now uses 300 trips.

### LTE-V2X mode 4 (dense, release binary)

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| vehicles | 150 → 300 | cbr_mean | up | 0.798 → 0.843 | yes |
| vehicles | 150 → 300 | collision_rate | up | 0.072 → 0.157 | yes |
| vehicles | 150 → 300 | pdr | down | 0.924 → 0.836 | yes |
| messages.rate_hz (RRI 50) | 10 → 20 | collision_rate | up | 0.146 → 0.233 | yes |
| max_transmissions | 2 → 1 | cbr_mean | down | 0.798 → 0.558 | yes |
| sensing | on → off | collision_rate | up | 0.072 → 0.129 | yes |
| resource_keep_probability | 0.8 → 0 | sl_selections | up | 168 → 387 | yes |
| mcs | 7 → 11 | cbr_mean | down | 0.798 → 0.773 | yes |
| sizing | code-rate → ts-36-213 (200 B BSM) | cbr_mean | changes | 0.798 → 0.798 | corrected |
| sizing | code-rate → ts-36-213 (cert, ~330 B) | cbr_mean | changes, corrected to same | 0.8093 → 0.8093 | corrected |
| congestion_control | J3161 → off, 300 trips | cbr_mean | up | 0.843 → 0.843 | corrected |
| congestion_control | J3161 → off, 300 trips | cbr_mean | same | 0.843 → 0.843 | yes |
| congestion_control | J3161 → ETSI TS 103 574, 300 trips | cbr_mean | down | 0.843 → 0.783 | yes |

- **Sizing:** under the J3161/1 pool, every transport block is at least two
  sub-channels of 10 PRB. A BSM of about 200 B fits that minimum under either sizing,
  so the setting cannot change a plain BSM. The same holds at about 330 B (a
  certificate on every frame). The profile's code rate (`LTE_MCS7_J3161`) is fitted to
  the J3161/1 allocations that the TS 36.213 table reproduces
  (`tbs_sizing_reproduces_j3161_and_opencv2x`: 301 B takes 3 sub-channels either way).
  So on the only profile where the loader admits it, the setting is a cross-check of
  the fit rather than a study variable. **Finding:** the loader does not refuse it,
  because the two sizings may still differ at some payload size.
- **CR limit:** a 10 Hz BSM with 2 transmissions on 2 of 10 sub-channels occupies
  0.4 %. The J3161/1 limit at CBR ≥ 0.65 is 1.5 %, so it never binds, and switching it
  off changes nothing. ETSI TS 103 574's 0.3 % above CBR 0.8 does bind.

**Not run this round:** the corrected queue, power-control, vRP_Min, β and
reactive-hold rows (they are in the full spec). The reactive hold moved `tx_rate_hz` by only −0.8 % in 2 s at a steady CBR,
below the 1 % bar. A hold only matters when CBR falls, so it needs a scenario whose
load changes.

## Not exposed, not tested

The SPS reselection counter range (TS 36.321: 5 to 15 at RRI 100) and the sub-channel
count per profile are fixed by the profile and are not settings.

## Run cost

Not measured this round, because of lock contention.

**Theory:** a medium-tier 802.11p run evaluates every frame at every receiver in
range. Its cost therefore grows with frames × receivers: with N² × rate for vehicles,
linearly with rate, and hardly at all with payload or EDCA numbers. On the dense base,
150 → 300 trips should cost about 4× and 10 → 20 Hz about 2×.

The cost scenarios are ready in the scratch folder, and `v2xw run --no-recording`
times them. Only one sweep was timed here: 39 dense runs took 355 s on the release
binary, and the 300-trip rows dominated.
