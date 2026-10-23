# Fast-forward defects from the full-scale benchmark (2026-10-23)

The full-scale run (FB2, `final-run.md` §4, benchmark §00) found six items in how a
`time.fast_forward` window is left and how it is stepped. This note gives each one's cause
as found in the code, the fix, and the test that guards it.

## 1. R-FFBURST: the honest-report burst after the resume (fixed)

FB2 filed 4,876 honest reports in the 10 s after the 43,170 s resume (about 80 per 10 s
live) and the node count fell from 1,833 to 1,281. Two causes, both the first step after
the window reading the frozen hours as elapsed time.

- **Traffic.** Every clock the mobility model keeps of elapsed traffic time (standstill,
  lane-change start, entry queue, stop end, the demand's next arrival) counted the window.
  A vehicle standing at a red lamp when the window began had "stood" 42,930 s, past the
  300 s gridlock timeout, so the gridlock resolution teleported or removed it. Each
  teleport is a position jump that every honest receiver reports.
  - Fix: `Mobility::resume_after_pause` (native, meso, tiered, social-force pedestrians,
    Poisson and composite demand) moves those clocks on by the pause.
    `Engine::resume_after_fast_forward` calls it on the first live step and re-dates the
    frozen actor state to that step.
  - Test: `calendar.rs` `the_traffic_resumes_after_a_fast_forward_as_if_no_time_had_passed`.
    The run with the window [20, 980] s, from 980 s on, equals the run with no window from
    20 s on, state for state.
- **GNSS.** The first epoch after the window had `dt` = 42,930 s. The outlier and
  multipath-burst probabilities are per-second hazards, `1 − (1 − p)^dt`, so both were 1.
  Every vehicle's first position after the resume carried the 12 m outlier and its next
  BSM, 0.1 s later, was an [SM-MBR] RandomPosition jump.
  - Fix: `GnssModel::resume_after_pause` (Gauss-Markov and legacy models) moves each
    receiver's last-epoch instant and its outage, burst and jamming ends on by the pause.
  - Test: `gauss_markov.rs`
    `a_receiver_resumed_after_a_pause_continues_its_error_as_if_no_time_had_passed`
    (50 receivers, errors equal to 1e-9 m with 12 h cut in).

The US receiver suite itself needs no change. `UsSmMbr` compares two BSMs only when their
generation times are within `max_gap_s` (3 s), so a sender last heard before the window is
not judged against its pre-window BSM.

## 2. `ped_wait` and `pseudonym_change_rate` across the window (fixed)

- `ped_wait` charged the window as waiting (mean 3,465 s). `MetricProvider::on_pause`
  tells every provider about the pause; the VRU provider moves each standing pedestrian's
  start on by it. Test: `vru.rs` `a_fast_forward_pause_is_not_a_wait`.
- `pseudonym_change_rate` divided by vehicle-hours that included the frozen hours. After a
  12 h window that made the rate of every later metric window hundreds of times too small,
  under the metric's 0.001 quantum. The privacy provider now leaves the pause out. Test:
  `privacy_metrics.rs` `a_fast_forward_window_is_not_observed_vehicle_time`.

## 3. "The rollover counts no pseudonym change" (not an engine defect)

FB2's expectation assumed that each vehicle changes pseudonym at 43,200 s. Nothing in
the cited sources requires that.
- A pseudonym certificate is valid for the week plus 1 hour ([CAMP-EE] §2.1.5.3.2; 03
  §1 item 4). So the old period's certificate stays valid until 46,800 s.
- The scenario's rule is J2945/1 CERTCHG: 5 minutes **and** 2 km since the last change
  (03 §11).
- A gridlocked vehicle in a 60 s live window meets neither condition. It is not forced to
  change before its certificate expires at 46,800 s, which is inside the second window.

The counter itself is correct (`CertStore::rotate`, `Phase2::note_change`). The rate was
the part that read 0, and that is fixed above (§2). FB2.privacy's expectation should be
corrected: no change is due at the boundary under CERTCHG with a 1 h overlap. The NYC
pilot rule (`nyc-pilot`) or a live window that reaches 46,800 s shows the expiry change.

## 4. R-FFHOLD: reports held for the whole window (not a fast-forward defect)

The 1,258 reports filed before the window stayed in flight until 43,200–43,210 s. They
were in the RA's report shuffle, which releases at 10,000 reports or one day, whichever
comes first ([CAMP122] §5.2.8.4, SCMS-765; 03 §8.2; gap row 26).
- The shuffle's one-day timer is a kernel delivery, and `Phase2::next_wake` includes it
  (`kernel`). `fast_forward_lands_every_backend_event_on_the_step_a_full_run_does`
  guards that every kernel event lands on the step a full run takes.
- The batch was released when the rollover window's 10,357 new reports pushed the count
  past 10,000.
- In a continuous run, the traffic of those 12 h would have filled the shuffle within
  about 20 minutes (8 reports/s). A frozen road files no reports, so the shuffle waits.

So the 12 h delay comes from the modelling choice to freeze the road, not from a missed
wake. A study of MA timing should do one of two things:
- keep the report-collection period live; or
- set `report_shuffle_threshold` / `report_shuffle_window_s` for the volume it models, and
  state it as an assumption.

`p2.in_flight` in the census shows the held count.

## 5. R-FFSLOW: the second window stops at every mobility step

(See §5 below — filled in from the wake-cause count.)
