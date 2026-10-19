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
| 2 | Clock skew and GNSS time error at the validity edges | A sender whose clock lags by *d* keeps an expiring certificate *d* longer, and accurate receivers refuse it as expired for *d* (1609.2 §5.2.3.2.2) | Not run. No scenario setting gives a node a clock error: `ClockModel::new(0.0)` for every node, and `clock_mut` is never called | **open** |
| 3a | RA offline from 590 to 900 s, across the top-ups at 600 s (main) | With no retry the requests are lost: the deviation recorded as validation/protocols.md B6 | 12 top-ups started, **6 completed**: all 6 vehicles of the home provider were lost; 144 deliveries dropped | confirmed, then fixed (3b) |
| 3b | Same run (new) | Every top-up completes. Ten retries 10 s apart meet the outage; the eleventh waits for the SCMS-523 window, then succeeds ([CAMP122] SCMS-522/523) | see §3 | see §3 |
| 3c | RA 15 s slow on the first request | The device retries after 10 s. The RA sees the same request twice, accepts it once and acknowledges both; 6 of 6 certificates are issued once | 1 accepted, at least 1 re-acknowledged, 0 `RequestLimit` refusals, 6 certificates, one request byte string | pass (proto test) |
| 3d | The RA itself down after the ack | The batch arrives once the RA is back | The LAs' and ACA's answers to the down RA are dropped and never resent, so the batch is never made | **open**: no specification says the LA or ACA retries; the test uses a cut between device and RA (at the LOP) instead |
| 4 | A device offline for weeks | The pool runs out after `pool_periods` weeks and the device goes silent. On reconnecting it tops up the current week first | Partly covered by 10: two weeks of pool outlast the 5-day run | **open** (needs a run longer than 2 weeks; see 5) |
| 5 | No new revocation for 3 weeks | A healthy CRL Generator re-issues by `nextCrl` (issue + 7 d), so no receiver ever reaches nextCrl + 14 d (1609.2 `nextCrl`; [CAMP122] Table 67) | On main the Generator signed **only when an entry was appended**. A list idle for 21 days would leave every receiver refusing the CRACA's certificates. Vehicles also fetched only when the entry count grew, so a re-issue could never reach them | **fixed**: re-issued on days 7, 14 and 21; vehicles and RSUs fetch a re-issue (publication serial). Proto tests `an_idle_crl_generator_*` and `without_renewal_*` |
| 6 | A standing 10,000-entry CRL (`crl_background_entries: 10000`) | A download is charged the encoded `CompositeCrlSpdu`: about 33 B per entry (F2: 33.08 B) plus the signature, the CRL signer's certificate and the home CTL, about 0.33 MB rather than 0.40 MB | see §3 | see §3 |
| 7 | Revoked liars keep transmitting | Refused by vehicles of both providers and by RSUs (CAMP-EE §2.2.10.2) | Outage run: 7,298 refusals by vehicles and 1,188 by RSUs. Many-cases run: 29,050 and 5,012, across all four receiver ← sender provider pairs | pass |
| 8 | A provider's root removed from the CTL mid-run | That provider's devices are refused once the CTL installs | `root_removal_at_s` only removes the home root that a rollover superseded; no setting removes a peer provider's root. In the base run the rollover CTL is published at its 3-business-day deadline, after the last top-up, so 0 vehicles install it (`trust_lists_installed` 0) | **open** |
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
| same | Random standing entries match no device of the run, so refusals are the base's | 23,284 refusals against 7,298, now including provider-a senders (6,649) where the base has none; 3 CRL versions published against 1 | **open**: needs a recorded rerun |
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
