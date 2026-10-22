# SCMS edge cases: real simulations (edge-case package, 2026-10-18)

Every check was written down before its run: what the real deployment does to the
metrics, to the run's cost, and to the PKI records. Base and variant run on the same seed.
The base is `scenarios/scms-conformance.yaml` (58 devices across two providers on the real
calendar). It has three radio windows (0–660 s, Thursday 15:20 UTC, Saturday 01:05 UTC),
fast-forwards between them, and runs real cryptography. The variants are in
`scenarios/edge/`. "main" is the release binary built from `d3039b53`; "new" is this
branch. The check rows, with their sources, are in
`crates/v2xw-engine/tests/expectations/scms/scms-edge-cases.yaml`. That file sits in a
subdirectory because the sensitivity loader reads every `*.yaml` in `expectations/`, and
it would fail on rows that no sweep plans.

## 1. Results

| # | Change | Expected (source) | Measured | Verdict |
|---|---|---|---|---|
| 1a | Base: the week rollover 614 → 615 at 600 s | No node signs with a certificate outside its validity window (CAMP-EE §2.1.5.3.2; 1609.2 §5.2) | 0 of 13,749 `node.security` records; a change inside the overlap hour always moves to 615 (4 of 4); 11 changes with reason "expired", all to 615, after the fast-forward | pass |
| 1b | Base: the PKI view after the rollover | A vehicle's last record shows nothing in use after the vehicle leaves | 15 of 56 vehicles left before 614 expired. Their last `node.credentials` record (614 active) stood for the rest of the run. **This was the reported "expired certificate in use"** | **fixed**: a departing vehicle records a final pool with `departed: true` and nothing active |
| 1c | Base: who signs an SM-MBR | Each report is signed with a pseudonym valid at its generationTime (SM-MBR v1.0; 1609.2 §5.2) | On main, checked with the A0 oracle's COER: **6 of 1,983 reports were signed by an expired 614 pseudonym**, the worst 2.2 days late. Also, **34 of 36 reporters signed every report of the run with one pseudonym**, so their reports were linkable to each other | **fixed**: `ScmsRun::mbr_signer` uses the vehicle's current BSM pseudonym if it is valid, else the newest valid one |
| 2 | Clock skew and GNSS time error at the validity edges | A sender whose clock lags by *d* keeps an expiring certificate *d* longer, and accurate receivers refuse it as expired for *d* (1609.2 §5.2.3.2.2) | Batch 3, §6. Before it, a node's clock offset never reached its security: it rotated on its own clock but stamped `generationTime` and judged validity on the simulation's | **fixed**; the run's outcome in §6 |
| 3a | RA offline from 590 to 900 s, across the top-ups at 600 s (main) | With no retry the requests are lost: the deviation recorded as validation/protocols.md B6 | 12 top-ups started, **6 completed**: all 6 vehicles of the home provider were lost; 144 deliveries dropped | confirmed, then fixed (3b) |
| 3b | Same run (new) | Every top-up completes. Ten retries 10 s apart meet the outage; the eleventh waits for the SCMS-523 window, then succeeds ([CAMP122] SCMS-522/523) | see §3 | see §3 |
| 3c | RA 15 s slow on the first request | The device retries after 10 s. The RA sees the same request twice, accepts it once and acknowledges both; 6 of 6 certificates are issued once | 1 accepted, at least 1 re-acknowledged, 0 `RequestLimit` refusals, 6 certificates, one request byte string | pass (proto test) |
| 3d | The RA itself down after the ack | The batch arrives once the RA is back | Main: the LAs' and ACA's answers to the down RA were dropped and never resent, and the RA's own shuffle timer was dropped too, so the batch was never made | **fixed** (protocol/flow gaps package): a component's message to a down component waits in the sender's queue and is retried every 10 s (`fault::BACKEND_RETRY`, ASSUMPTION); a down component's timers run when it restarts. `ee_retry.rs::an_ra_down_across_its_shuffle_release_releases_it_when_back`, `an_ra_down_when_the_aca_answers_gets_the_answer_on_a_retry`, `an_outage_that_never_ends_still_loses_the_answers`. Counted as `backend_deliveries_held` and `backend_timers_deferred` |
| 4 | A device offline for weeks | The pool runs out after `pool_periods` weeks and the device goes silent. On reconnecting it tops up the current week first | Batch 3, §6 (`edge/offline-weeks`, 8 days without the home RA) | pass (backend); see §6 |
| 5 | No new revocation for 3 weeks | A healthy CRL Generator re-issues by `nextCrl` (issue + 7 d), so no receiver ever reaches nextCrl + 14 d (1609.2 `nextCrl`; [CAMP122] Table 67) | On main the Generator signed **only when an entry was appended**. A list idle for 21 days would leave every receiver refusing the CRACA's certificates. Vehicles also fetched only when the entry count grew, so a re-issue could never reach them | **fixed**: re-issued on days 7, 14 and 21; vehicles and RSUs fetch a re-issue (publication serial). Proto tests `an_idle_crl_generator_*` and `without_renewal_*` |
| 6 | A standing 10,000-entry CRL (`crl_background_entries: 10000`) | A download is charged the encoded `CompositeCrlSpdu`: about 33 B per entry (F2: 33.08 B) plus the signature, the CRL signer's certificate and the home CTL, about 0.33 MB rather than 0.40 MB | see §3 | see §3 |
| 7 | Revoked liars keep transmitting | Refused by vehicles of both providers and by RSUs (CAMP-EE §2.2.10.2) | Outage run: 7,298 refusals by vehicles and 1,188 by RSUs. Many-cases run: 29,050 and 5,012, across all four receiver ← sender provider pairs | pass |
| 8 | A provider's root removed from the CTL mid-run | That provider's devices are refused once the CTL installs | Batch 3, §6. Before it, no device installed any CTL after the factory (US path), and no setting removed a peer's root | **fixed**; two items stay open (§6) |
| 9 | Thanksgiving week (t0 2026-11-24) | A case decided on Wednesday is published the next business day at 10:00 local time; Thursday 26 Nov is a federal holiday (5 U.S.C. 6103) | Decided Wed 25 16:52 UTC, **published Fri 27 15:00 UTC (10:00 EST)**. Ordinary week: decided Wed 21 16:51, published Thu 22 14:00 UTC (10:00 EDT) | pass |
| 10 | No RSU and no cellular | Signing and verification continue from the factory pool. No top-up, report, case, CRL or download | 0 top-ups, 0 reports at the MA, 0 cases, 0 CRLs; 39,446 verified; 1,405 reports left with departing vehicles and 14 were still queued at the end. Wall time 138 s against about 400 s (no backend work) | pass |
| 11 | Post-quantum and hybrid certificates with fragmentation at density | ML-DSA frames are fragmented and latency rises (`security-track.yaml` rows) | Not run in this package | **open** |
| 12 | 30 % liars: many concurrent MA cases | Every case passes the 8 h review and goes on the next business-day CRL; refusals rise | 10 cases (4 + 6 by provider), 10 decisions, 3 CRL versions, 265 enforcements, 29,050 refusals | pass |

## 2. What was fixed

- **The EE retry ([CAMP122] SCMS-522/523)**, in `run/provision.rs`. After every EE → RA
  request (the butterfly request, the `.info` top-off, each batch download) the device
  starts a 10 s timer. If no answer has come, it sends the request again: the butterfly
  request byte for byte, so its `requestHash` (which names the batch files) does not
  change. It sends at most 10 retries in any 60 minutes. A timer whose request was
  answered is dropped before the device's queue, so a run with no loss keeps its timing.
  The RA acknowledges a byte-identical retransmission again (ASSUMPTION: idempotent), and
  the device drops a second answer. New settings: `ee_retry_timeout_s` (10; 0 turns it
  off) and `ee_retry_max_per_hour` (10). New run-report counters: `ee_retries`,
  `ee_retries_capped`, `ee_duplicate_answers` and `ra_retransmissions_reacked`.
- **CRL re-issue by `nextCrl`**, in `run/crl.rs` and `phase2.rs`. `CrlIssuance::reissue`
  is on in the engine. Each CRL version carries the store's publication serial. Vehicles
  (unicast poll) and RSUs fetch when the serial grows, not only when the entry count does.
- **The real composite CRL size**. `ctl::encode_composite_crl_coer` rebuilds the
  `CompositeCrlSpdu` from the bytes the RA holds. It matches the committed, oracle-checked
  `composite_crl.oer` byte for byte. Vehicle polls, RSU backhaul fetches and MBMS are
  charged that size (cached per CRL set and CTL). The 40 B/entry estimate is now used only
  on the modelled path.
- **The standing CRL**: `crl_background_entries` (default 0) puts N earlier revocations on
  the home provider's first real CRL at t0.
- **The SM-MBR signer**: `ScmsRun::mbr_signer` / `secure_mbr_as` (1c).
- **The departed record**: `NodeCredentials::departed` (1b). It is serialised only when
  true, so records of vehicles still in the run are unchanged.

Fault injection (`tools/mutate/mutate.py`; see §4): each new test went red with its fault
and green after the restore.

## 3. New-binary runs

| Run | Expected | Measured | Verdict |
|---|---|---|---|
| `edge/ra-outage-topup`, first new binary | 12 of 12 top-ups. Each of the 6 home-provider vehicles makes 10 retries in the outage and an 11th after the SCMS-523 hour, so 66 retries and 6 capped | **12 of 12 completed, `ee_retries` 66, `ee_retries_capped` 6**, 264 deliveries dropped, 0 duplicate answers, 0 re-acks. User time 368 s (main 449 s) | fixed |
| `edge/standing-crl-10k`, first new binary | About 0.33 MB per download (33 B per entry) | 43 downloads, Uu downlink 19.64 MB against 0.24 MB without the list: **about 451 KB per download, 45 B per entry**. My standing list put each entry under a random LA pair, so every entry was its own `LAGroup`. A US provider runs one LA1 and one LA2, and its entries share a group. Fixed: `a_standing_crl_costs_thirty_three_bytes_an_entry` measures 32.5–33.5 B. The 10,000-entry rerun is open | expectation was right, scenario was wrong: corrected and pinned |
| same | The run costs more, because every receiver expands 10,000 entries | Wall time 1,143 s against 382 s; peak RSS 178 MB against 78 MB; charged receiver validation 656 ms against 425 ms | as expected |
| same | Random standing entries match no device of the run, so refusals are the base's | 23,284 against 29,704 in the new base. My first comparison, against the outage run (7,298), was wrong: in that run provider-a's case never reaches a CRL. 21 % fewer refusals with the list in place | expectation corrected; the 21 % drop is **open** (likely the receivers' extra CRL-processing time; not isolated) |
| `scms-conformance`, final binary (recorded) | 1b and 1c fixed; nothing else in the PKI moves | **0 of 1,949 SM-MBRs signed outside their validity** (main: 6 of 1,983); 12 of 36 reporters used more than one pseudonym (main: 2); **0 departed vehicles still showing a certificate in use** (main: 15); 0 of 13,749 `node.security` records past validity; the same 4 cases through every stage; 423,729 receptions verified (main: 423,749); 12 of 12 top-ups; 0 retries. 252 reports went out unsigned ("no pseudonym certificate valid at the report's time", main: 458 "no active pseudonym certificate"), and only 1,296 were real SM-MBRs | pass; see the next row |
| same: against the main base (`scms-conformance`, main, same seed) | Only 1b and 1c change | Revoked-sender refusals 29,704 against 29,644 (RSUs 4,311 against 4,311); real SM-MBRs 1,296 against 1,313; 252 unsigned in both runs; 4 cases in both; 12 of 12 top-ups in both. CRL downloads rise from 17 to 27 and Uu downlink from 160,690 to 245,631 B: re-issue serials, plus the real composite size, which includes the home CTL | pass |
| `edge/ra-outage-topup`, first new binary: CRL downloads | Unchanged within 5 days, since no re-issue falls inside the run | 27 against 17 on main: vehicles now fetch every new publication (by serial), not only one that grows the entry count | as designed |

## 4. Fault injections

Each fault was injected with `tools/mutate/mutate.py`, which restores the file and sets a
fresh mtime. The script is `scratchpad/edge/mut.sh`, and every fault went RED:

| Fault | Test | Red result |
|---|---|---|
| No SCMS-522 timer (`if false`) | `ee_retry::a_top_off_retries_through_an_ra_outage_and_completes_at_the_cap` | "the top-off completes once the RA is back" (never installed) |
| RA never re-acks a retransmission | `ee_retry::a_retried_request_is_acknowledged_again_and_issued_once` | `retransmissions_reacked >= 1` fails |
| No CRL re-issue | `ee_retry::an_idle_crl_generator_renews_its_list_every_next_crl` | reissued 0, expected 1 |
| MBR signer ignores the validity window | `mbr::a_report_after_the_rollover_is_signed_with_a_pseudonym_valid_then` | picked (1, 0) at 600 s, expected (0, 0) |
| Composite CRL built from the first CRL only | `crl_ctl_vectors::the_composite_crl_from_held_bytes_is_the_committed_vector` | the bytes differ from `composite_crl.oer` |
| Standing entries under a random LA pair | `ee_retry::a_standing_crl_costs_thirty_three_bytes_an_entry` | "every entry under the provider's LA1 and LA2" |

The retry-off tests (`without_the_retry_*`, `a_slow_ra_without_the_retry_*`,
`without_renewal_*`) also run the old behaviour through the setting and assert its
failure: the top-off is lost, and the list goes past its tolerance.

## 6. Batch 3 (2026-10-19): clock skew, CTL install, weeks offline, the 10,000-entry rerun

Rows written before the runs are at the end of the expectations file (`regression.base-new3`,
`E2.clock-ahead-5000s`, `E4.offline-weeks`, `E8.peer-root-removed`,
`E6.composite-crl-10k-rerun`). "new3" is commit `b688bf40`, "new5" is `21c8ae81`. Same seed
throughout. Wall times are shared-machine times.

### 6.1 What changed in the engine

- **The node's clock reaches its security** (`v2xw-node` `secure.rs`). `NodeSecurity` holds
  the clock's believed-minus-true offset, which the runtime copies on every step. A
  signature is stamped with the node's own time (`SecCtx::skewed`, 1609.2 §6.3.9), and a
  receiver checks validity and CRL currency at its own time (§5.2). New settings:
  `clock_skew_s` and `clock_skew_share`. The share is picked by a fixed hash of the node
  index, so a base and a variant skew the same vehicles.
- **Devices install a newer CTL.** The composite CRL file carries the home CTL (1609.2.1
  `CompositeCrlSpdu`). A vehicle polls when the CTL is newer than the one it holds, as well
  as when the CRL is newer. On the download it applies the CTL's root set
  (`NodeSecurity::apply_ctl`): it adds new roots, and drops the roots the CTL no longer
  approves, together with every path and signer validated under them. RSUs do the same on
  their backhaul fetch. New setting: `peer_root_removal_at_s`. Once the CTL that removes a
  provider is published, RAs stop serving that provider's CRL.
- **A removed provider keeps running.** `Governance::home_root` panicked on the removed
  provider's own view ("the home provider has an approved root"). It now falls back to the
  newest root that provider still runs under.
- **Refusals of departed nodes stay in the report.** `receiver_validation` summed only the
  nodes present at the end of the run. In the clock-skew run it reported 12,325 refusals
  for 56,417 refused SPDUs.
- A fetch that only brings a newer CTL, with no CRL published yet, is no longer counted as a
  refused CRL at an RSU.

### 6.2 Results

| Run | Expected (written before) | Measured | Verdict |
|---|---|---|---|
| `scms-conformance`, new3 against new2 | Only the CTL install moves. The rollover CTL (decided at 120 s, published Friday 08:52 UTC) is installed in the Saturday window with 0 anchors removed | `trust_lists_installed` 16 (14 vehicles and 2 RSUs), `trust_anchors_removed` 0. CRL downloads 41 against 27, RSU CRL installs 6 against 4, Uu downlink 295,933 against 245,631 B (16 × 3.1 kB composites). Refusals (29,704 / 4,311), cases (4), top-ups (12/12), BSMs and verifications per window are identical | pass |
| `edge/clock-skew-ahead` (28 of 56 vehicles 5,000 s ahead), new3 | Skewed senders sign under 615 from their first BSM. Accurate receivers refuse them as chain-expired until 600 s and accept them after. Skewed receivers refuse 614 BSMs. Later windows match the base. Top-ups, cases and CRLs unchanged | Window 0 signing records: 1,899 under 615 for provider-a, against 70 in the base. Accurate receivers (own clock 0–600 s): 33,362 refused, 35,358 valid; from 600 to 700 s: **0 refused**, 7,137 valid. Skewed receivers (own clock 5,000–5,700 s): 23,055 refused. Top-ups 12/12. **But 20 cases against 4, and 15 honest vehicles revoked** (base 0): reports at the MA rose from 2,545 to 16,238. Misbehaviour detectors flagged the skewed honest senders (positionJump 16,485, acceptanceRangeThreshold 17,351, mapOffRoad 18,566; base 6 / 0 / 0). Two of them were revoked and starved in the Thursday window | validity edges: pass. Expectation on cases **corrected**: a kinematic detector extrapolates a sender's position from its `generationTime`, so an honest vehicle 83 minutes off cannot be told from a time-spoofing attacker. That is the real system's behaviour, and a finding: a GNSS time attack gets honest vehicles revoked through the MA |
| same, new5 (refusal report) | `receiver_validation` counts every refused SPDU, including those of vehicles that left | 56,417 chain-expired, equal to the 33,362 + 23,055 refused `node.verify` records (new3 reported 12,325). Cases (20) and revocations (15 honest) unchanged | fixed |
| same, wall and memory | About the base | 615 s against 637 s. Peak RSS 212 MB against 71 MB: 16,238 reports held at the MA, against 2,545 | as expected for the work done |
| `edge/offline-weeks`, new3 | Window 611,700–612,000 s: the home vehicles are starved and silent. Retries at most 10 an hour reach the RA within an hour of its return at 700,000 s. All home top-ups complete | 6 vehicles starved, **0 BSMs** from them in that window (provider-b and newly provisioned vehicles sign under 616). 11,977 retries, 1,197 capped (10 an hour for 8 days). After the RA came back, the 7 requests were acknowledged at 702,610 s, certified, downloaded and installed by 702,671 s (2,610 s after the return, inside the SCMS-523 hour). Wall 398 s | pass (backend). The 6 vehicles had left the map before the last window, so it does not show them transmitting again. Their device records keep retrying after they leave, as a real device elsewhere would |
| same, main (no retry) | The home top-ups are lost and never complete | 7 requested, **0 acknowledged** after 590 s; 6 starved. The report's `topups_completed` is 14 in both runs, because it counts installs on vehicles still on the map | confirmed |
| `edge/peer-root-removed`, new3 | Removal decided at 300 s and published at 173,100 s. Every device refuses provider-b's BSMs once it installs the CTL | **Panicked** at 19 s: `home_root` on provider-b's own view | fixed (6.1) |
| same, new5 | Thursday and Saturday windows: provider-b refused everywhere; provider-a unaffected; window 0 as the base | Window 0 identical to the base. **Thursday: no change**. The CTL due at 173,100 s was published only at the next backend tick, at the window's start, after every vehicle had already polled; the next poll is an hour later (open, below). Saturday: 16 installs, **16 anchors removed**, 42,132 refused as chain-untrusted-root. That equals the fall in verified provider-b receptions (−19,906 by provider-a, −14,570 by provider-b) plus the revoked-sender refusals that became untrusted-root refusals (−7,656) | pass on the mechanism, **open** on the timing |
| `edge/standing-crl-10k`, new3 | About 0.33 MB a download | 57 downloads, Uu downlink 19,153,594 B, **336 kB a download** (33.1 B an entry with the signature, the signer and the CTL). Wall 1,092 s against 637 s; RSS 194 MB against 71 MB | pass |
| same, refusals | Random entries revoke no device of the run, so refusals match the base | Identical BSM counts per window and provider, but 23,289 revoked refusals against 29,704, and 498k valid verifications against 424k | **open** (as in §3): the receivers verify about 17 % more of the same frames. Not isolated |

### 6.3 Fault injections (batch 3)

| Fault | Test | Red result |
|---|---|---|
| The receiver checks validity at the simulation's time | `rx_validation::a_receiver_whose_clock_runs_ahead_refuses_a_certificate_still_valid` | accepted (left: Valid) |
| The sender stamps the true time | `rx_validation::a_sender_whose_clock_lags_stamps_its_own_time_and_keeps_an_expired_certificate` | "generationTime is the sender's believed time" |
| A dropped root keeps its validated paths | `rx_validation::a_root_a_newer_ctl_drops_is_refused_from_then_on_and_an_unchanged_ctl_forgets_nothing` | still Valid after the removal. The first version of this test went **GREEN** under the fault, because adding a root also cleared the cache. It now removes without adding |
| `home_root` requires an approved root | `provider_runs::a_provider_whose_root_the_eac_removed_takes_the_ctl_and_keeps_running` | the panic. The first version went **GREEN**, because it never looked up `cert("root")`. It does now |

### 6.4 Open from batch 3

- **A CTL publication waits for the next backend tick.** `Governance::publish_due` runs in
  `governance_due` only when the backend steps. A deadline that falls inside a fast-forward
  (removal at 173,100 s) is published at the next window's first tick, after that tick's
  polls. Fix: offer each pending publication time to the backend's next-event set, as
  `root_removal_at` is. The same applies to the rollover CTL.
- **One unverifiable CRL refuses the whole frame.** After the removal, 21 CRL frames
  carrying provider-b's CRL (RSU broadcasts held from before) were refused whole
  (`crl_frames_rejected` 21) by vehicles that had dropped the root. A device should
  discard only the CRL it cannot verify.
- **The CTL's signatures are not re-checked on the device.** `apply_ctl` applies the
  published root set. `DeviceTrust::install_ctl` (elector quorum) is not run on that path.
- **Clock skew is applied to vehicles only.** RSUs and VRUs carry the offset plumbing but
  no setting. The skew is a constant offset; drift while unlocked (`ClockModel`'s ppm)
  is not set by any scenario.
- Items 11 (post-quantum at density) and 3d (the LA and ACA answers to a down RA) are
  still open.

## 5. Found on the way, not fixed

- `v2xw run` on a scenario with `time.fast_forward` fails its own read-back check:
  "195541000000000 ns between keyframes exceeds the 1000000000 ns cadence". The keyframe
  verifier does not know about fast-forward windows. Every run here used `--no-verify`.
- `target/agents/debug/v2xw` is shared. Another agent's build replaced it 5 minutes after
  this package's build, so a copy taken "under the lock" can be someone else's binary.
  `build2.sh` picks this branch's build from `deps/` by a string only it contains.
- RSUs never call `CertStore::rotate`. An RSU's first certificate in its store is used
  until it expires, so its renewal relies on `install_rsu_app_certs(replace=true)`.
- During an RA outage the RA's own timers are dropped as well ("timers included"), so a
  weekly pre-generation job due during the outage is lost. A real RA would run it at
  restart.
- `check_run.py` (F1) does not check an SM-MBR signer's validity at its generationTime.
  That is why 1c passed the conformance oracle.
