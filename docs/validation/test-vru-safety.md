# Engine test: vulnerable road users and safety applications

Round-15 testing track. Each setting was moved one at a time, same seed, on the release
engine built from `e94e43c`. Runs are 60 s and record metrics only:

- **Pedestrian grid** (`scenarios/vru-grid.yaml`): 150 pedestrians, 15 cyclists, 30 % of
  them carrying a device, 1500 veh/h, every application on.
- **Connected intersections** (`scenarios/connected-intersections.yaml`): six signalised
  junctions with SPaT/MAP roadside units, 3000 veh/h, every vehicle equipped, every
  application on.

The rows the expectation table checks are in
`crates/v2xw-engine/tests/expectations/vru-safety.yaml` and
`interactions/vru-safety.yaml`. Run them with:

```
v2xw sensitivity --expect --expectations crates/v2xw-engine/tests/expectations/vru-safety.yaml \
  --interactions crates/v2xw-engine/tests/expectations/interactions/vru-safety.yaml
```

**Expectation run** (debug build of `10fdc41`, 40 s / 60 s families, 751 s): 17 of 19 rows
pass. The 2 failures were both caused by note 7, and those rows were removed. The table
below is from the 60 s release runs.

Counts are summed over the run. Means are weighted by sample size.

## Pedestrian grid

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| `actors.vru.device_fraction` | 0.3 → 0.6 | PCW warnings | up | 11 → 31 | yes |
| `actors.vru.device_fraction` | 0.3 → 0 | PCW warnings | down (to 0) | 11 → 0 | yes |
| `actors.vru.device_fraction` | 0.3 → 0 | mean CBR over nodes | down | +8 % | no (see note 1) |
| `actors.vru.pedestrians` | 150 → 300 | frames sent | up | 5410 → 7532 (40 s) | yes |
| `actors.vru.pedestrians` | 150 → 300 | CBR | up | +0.04 % | no (see note 1) |
| `actors.vru.pedestrians` | 150 → 300 | VRU AoI | not predicted | +34 % | n/a |
| `actors.vru.pedestrians` | 150 → 300 / 50 | vehicle mean speed | down / up | +7 % / +7 % | no (see note 2) |
| `actors.vru.cyclists` | 15 → 60 | mean speed | down | −22 % | yes |
| `actors.vru.cyclists` | 15 → 60 | PCW warnings | up | 11 → 15 | yes |
| `actors.vru.ebike_share` | default → 1 | mean speed | up | +4 % | yes |
| `actors.vru.walking_speed_mps` | 1.2 / 1.6 | PCW warnings | slower walkers are exposed longer | 15 / 9 | yes |
| `actors.vru.red_crossing_share` | default → 0.5 | conflicts, near misses | up | 0 → 1, 0 → 1 | yes (sparse) |
| `actors.vru.red_crossing_share` | default → 0.5 | PCW warnings | up | 11 → 14 | yes |
| `actors.vru.midblock_rate_per_100m` | default → 50 | mid-block share | up | 0.017 → 0.377 | yes |
| `actors.vru.midblock_rate_per_100m` | default → 0 | mid-block share | down | 0.017 → 0 | yes |
| `actors.vru.behaviour` | `document` | mid-block share | down | 0.017 → 0 | yes |
| `actors.vru.crossing_gap_margin_s` | 2 → 0.5 / 6 | anything | changes | bit-identical run | **dead** (fixed, note 3) |
| `actors.vru.crossing_gap_margin_s` | 2 → 6 (with red share 0.5, mid-block 20) | pedestrian wait / conflicts | up / down | +5 % / 2 → 1 | yes |
| `actors.vru.crossing_gap_margin_s` | 2 → 0.5 (same context) | conflicts | up | 2 → 3 | yes |
| `apps.pcw_ttc_s` | 3 → 4 | PCW warnings | up | 11 → 19 | yes |
| `apps.pcw_ttc_s` | 3 → 2 | PCW warnings | down | 11 → 6 | yes |
| `actors.vehicles.equipped_fraction` | 1 → 0.5 / 0.2 | PCW warnings | down | 12 → 11 / 0 | yes |
| `actors.vehicles.equipped_fraction` | 1 → 0.5 | CBR | down | −34 % | yes |
| `apps.lta_gap_s` | 4.1 → 3.5 / 5.5 | LTA warnings | down / up | 0 / 0 | no warnings either way (note 4) |
| `apps.enabled` | `[]` | app warnings | none | 11 → 0, nothing else moves | yes |

## Connected intersections

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| `apps.fcw_ttc_s` | 2.4 → 3.0 | FCW warnings | up | 20 → 29 | yes |
| `apps.fcw_ttc_s` | 2.4 → 3.0 | FCW lead time | up | 2.35 → 2.70 s | yes |
| `apps.fcw_ttc_s` | 2.4 → 2.0 | FCW warnings | down | 20 → 15 | yes |
| `apps.ima_tti_s` | 4 → 6 | IMA warnings, lead time | up, up | 13 → 17, 2.8 → 5.3 s | yes |
| `apps.ima_tti_s` | 4 → 3 | IMA warnings | down | 13 → 4 | yes |
| `actors.vehicles.equipped_fraction` | 1 → 0.5 | all V2V warnings | down, about p² = 0.25× | 149 → 19 (0.13×) | yes |
| `actors.vehicles.equipped_fraction` | 1 → 0.5 | CBR | down | −24 % | yes |
| `apps.glosa_compliance` | 0 → 1 | DRAC | down | −17 % | yes |
| `apps.glosa_compliance` | 0 → 1 | mean speed | not predicted | +0.2 % | n/a |
| `apps.rlvw_decel_mps2` | 3.4 → 2.5 / 4.5 | RLVW warnings | up / down | 0 / 0 | no warnings either way (note 4) |
| `apps.lta_gap_s` | 4.1 → 3.5 / 5.5 | LTA warnings | down / up | 0 / 0 | no warnings either way (note 4) |
| `apps.enabled` | `[]` | app warnings | none | 149 → 0 | yes |

**Run cost:** none of these settings changes run time much. A 60 s pedestrian-grid run
takes 3 to 5 s. With 300 pedestrians it takes 10 s, and with 60 % devices it takes 9 s:
handsets add radio nodes, and the radio cost grows with node count. Removing devices
(2 s) or applications does the opposite.

## What was fixed

1. **The `apps.*` settings could not be swept and showed no default or range in the settings
   window.** A default `apps` block is left out of the saved scenario, so content hashes
   do not change. As a result, the published defaults had no `apps` entry. The thresholds
   also had no `BOUNDS` row (the loader checked them by hand) and no recommended range. So
   the catalogue found no value to move, and `v2xw sensitivity --catalogue --only apps`
   planned 0 settings. The fix:
   - The defaults now come from `Apps::default()`.
   - Six `BOUNDS` rows replace the hand checks.
   - Guide rows give each threshold a real-world sentence and a cited range: NHTSA FCW
     2.4 s, HCM critical gap 4.1 s, AASHTO 3.4 m/s².
   - The loader now refuses `glosa_compliance > 0` when `glosa` is not enabled, because
     the setting would do nothing then.
2. **Dimensioned metrics were pooled.** The sensitivity fingerprint averaged every
   dimension of a metric together. For example, `aoi` mixed vehicle links with VRU
   links, and `app_warnings` mixed every application. It now also keeps
   `metric.<name>[dim=value]`, so a row can check `metric.app_warnings[app=pcw]` or
   `metric.aoi[class=vru]`.
3. **The crossing gap margin was dead on the pedestrian family.** Only a pedestrian who
   crosses against the signal, or mid-block, into a gap reads it. At the observed defaults,
   40 to 60 s has almost no such decisions, and the runs were bit-identical. The fix is a
   family context (red share 0.5, mid-block rate 20) where the margin acts. The model
   itself is wired correctly.
4. **Routing:** `apps.*` routes to a new `intersections` family, and `apps.pcw_ttc_s`
   routes to the pedestrian family.

## Notes and open items

1. **CBR does not rise with VRU devices.** The mean CBR is averaged over all nodes,
   including the handsets. A handset on the sidewalk, 1.5 m up behind buildings, senses
   little, so adding handsets lowers the mean as much as their PSMs raise it. Frames sent
   rise as expected. A CBR measured at vehicles only would be the metric to check. It is
   not built.
2. **Vehicle mean speed is not monotone in the pedestrian count.** It rises 7 % at both 50
   and 300 pedestrians. The `mean_speed` metric pools vehicles and VRUs, and changing the
   population redraws the whole spawn sequence. The fingerprint's
   `traffic.speed_mean_mps` (vehicles only) is the right metric. It is not claimed until
   the expectation run measures it.
3. **No VRU awareness ratio exists.** The only awareness metric for VRUs is `aoi[class=vru]`,
   which is per heard device. "Higher device share → more VRUs known" needs a share of
   nearby VRUs that a vehicle has heard from. That metric is not built.
4. **RLVW and LTA never warn on either family**, so their thresholds cannot be checked
   here. RLVW warns only when a vehicle would enter on red, and the engine's drivers stop
   for red or clear on amber. LTA needs an opposed left turn inside the gap, and none
   occurred in 60 s. Checking them needs a family with red-light runners and protected or
   permissive left turns.
   **Round 16:** RLVW could not fire because no driver ever entered on red (the amber
   rule goes only if the line is reached 0.3 s before the red). New setting
   `actors.vehicles.drivers.red_runner_share` (0 by default) makes that share of drivers
   go inside their dilemma-zone threshold regardless of the red; rows for it and for
   `apps.lta_gap_s` are in `vru-safety.yaml` (not yet run). LTA does warn on the osm and
   traffic families (sensitivity catalogue), only not on these two.
5. **Not covered:** cyclist and motorcycle driver parameters (the traffic track has rows),
   VAM message rate (PSMs beacon at about 1.2 Hz per device in this build, and no rate
   setting exists), DENM hazards (the networking track), and emergency vehicle priority
   (no setting).
7. **A zero count is not reported as 0.** `metrics/apps` emits `app_warnings` only for
   an application that issued at least one warning in the period. A run where PCW never
   warns therefore has no PCW sample. The expectation runner reads that as "not measured"
   rather than 0, which failed the PCW low-end row and the PCW × devices interaction. Both
   were dropped. The fix is to seed the provider with the enabled applications. That
   changes the metric stream, and so the content digest of every run with applications,
   so it was left for a round that re-pins digests.
   **Fixed in round 16:** the provider is seeded with the enabled warning applications
   and reports 0; the PCW low-end row and the PCW x devices interaction are back.
6. **Crossing delay (`ped_wait`)** reports `insufficient` (fewer than 30 waits) in a 40 to
   60 s run at the defaults. It is measurable only with more crossings, such as the
   gap-margin context.
