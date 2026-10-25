# SCMS audit: pseudonym provisioning (audit-prov)

Component group: butterfly requests, the LOP, the RA (expansion, request shuffle,
blocklist), LA1 and LA2 pre-linkage values, the PCA (implicit ECQV certificates encrypted to
the device), batch download, weekly pre-generation, top-ups, pool sizes, RSU application
certificates and the per-entity backend load against CAMP sizing. Audited on main
`8becf72a`, branch `worktree-wf_4ffcc778-db1-2`.

## 1. Method

- **Runs.** The integrator's release build of `8becf72a`, copied. `scenarios/scms-conformance.yaml`
  (real calendar, real cryptography, 65 vehicles of two providers, 2 RSUs, the i-period rolls
  614 → 615 at 600 s, fast-forward to day 5), `--record-level events`, seed `0xc0ffee5eed`:
  `conf` and its same-seed twin `conf2`. Each run took about 4 minutes and wrote a 50 MB recording.
- **Checker.** `tools/pdu_check/audit_prov.py` reads only the raw recording and
  `run-report.json`:
  - `pki.flow` messages, stages and SPDUs;
  - `node.credentials` certificate bytes;
  - `node.security`, `backend.state` and `pki.state`.

  It decodes every certificate with the oracle's own COER codec (`tools/oracle`). It never
  reads engine code or engine-computed summaries, except where a row compares an engine
  metric against the records.
- **Rows.** `crates/v2xw-engine/tests/expectations/scms-audit-prov.yaml` has 32 rows. They
  were committed with `measured: pending` before the checker ran (`ece23338`, `28f732b4`).
  Three rows were added after the first pass, and each is labelled in its `note`.
- **Re-measured with the RA shuffle fix** (`0d62f322`, `2af798f1`, optimised debug build): `conf`
  and `conf2` again, 859,555 records each. 31 of 32 rows pass; PV.rsu-cert stays open (§4.3).
  `--mutate all`: 0 silent. Oracle `records conf`: **PASS, 221,884 / 221,884** over 20,843 PDUs.
- **Fault proofs.** `--mutate all` injects one record fault per row. All 28 rows that pass
  on the real records go red under their fault (0 silent). Two rows are red on the real
  records. For those, `GREEN` repairs the records to the shape the spec asks for, and the
  row turns green. That shows each is a finding, not a rule that can never pass.

## 2. Inventory

| Row | Behaviour | Spec value | Measured (conf) | Verdict |
|---|---|---|---|---|
| PV.request-once | One butterfly `EeRaCertRequestSpdu` per vehicle | `ra-maxReqs` 1 ([SMP] T3; SCMS-520) | 65 vehicles × 1 | pass |
| PV.ack-per-request | One `RaEeCertAck` per request | [ASN-2.1] | 71 = 71 | pass |
| PV.plv-per-cert | One PLV per certificate per LA, sealed to the ACA | 40 per vehicle (20 × 2 weeks) | 1,400 / 1,400 (35 veh), 1,200 / 1,200 (30); 0 unsealed | pass |
| PV.aca-per-cert | RaAca = AcaRa; every AcaEe-private delivered matches its AcaRa; the rest held at the RA | 1 per certificate | 3,146 = 3,146; 2,846 delivered, 300 held (pregeneration accounts), 0 lost | pass (rule corrected, §3) |
| PV.encrypted-to-device | PCA response sealed to the EE; never opened at the RA | [ASN-2.1]; CAMP 5.2.7.6.6 | 2,846 sealed to `ee`; 0 opened at an RA | pass |
| PV.aca-learns-no-device | No device identifier in RA→ACA SPDUs | ACA never sees EE identity | 2,846 searched for 199 ids: 0 | pass |
| PV.aca-cannot-group | ACA batches mix devices: 1 request per message, largest share of one device = 1/k | RA shuffles across devices ([CAMP122] 5.2.7.6.4; [BRECHT] III-C, V) | 3 batches, 542 requests; 1 per message; ≥ 2 devices; share at 1/k (17, 10 and 2 devices) | **fixed** (§4.1) |
| PV.aca-grouping-attack | An ACA-side attack (arrival adjacency, equal length, any shared 8-byte string) links no better than the population baseline | the PCA learns nothing linking two certificates ([BRECHT] VI-D) | order 52 vs 38.0 (z 2.36); length 0 links; bytes 1,317 vs 1,325.7 (z −0.25) | **fixed** (§4.1) |
| PV.lop-not-deployed | No LOP hop in the US profile | not deployed (01 §2.2) | 0 hops in `pki.flow` | pass (view note §4.4) |
| PV.cert-implicit | Pseudonyms implicit (ECQV) | 1609.2 6.4.8 | 2,840, explicit 0 | pass |
| PV.cert-validity | Start on the i-period grid, hours(169) | [SMP] T2; CAMP T3 | 0 off grid, 0 not 169 h | pass |
| PV.cert-linkage | linkageData, iCert = i, 9 B LV, series 1 | 1609.2 6.4.12 | 0 off | pass |
| PV.pool-20-per-week | 20 certificates per (vehicle, i) | 20 ([SMP] T2) | {20: 142} | pass |
| PV.pool-at-join | Two i-periods at join | `device_pool_periods` 2 (ASSUMPTION) | {2: 65} | pass |
| PV.topup-at-boundary | Every vehicle on the road with its pool ending at 615 tops up at the boundary | `topup_below_periods` 1 | 12 of 12; start ≤ 16.6 ms | pass |
| PV.topup-no-new-request | Top-off = `.info` + 1 batch, no new butterfly | CAMP 5.2.7.8, SCMS-576 | 0 new; 2 downloads each | pass |
| PV.topup-20-certs | 20 certificates for i = 616 | 20 per period | 12 × 20 delivered and held | pass |
| PV.pregen-delay | Weekly job within `ra-maxGenDelay` | ≤ 1 week | 2 weekly jobs (one per provider, every account), 0.100–0.104 s | pass |
| PV.download-after-ready | Batch asked for only once held, within 300 s | `nextDlTime`; minWait 5 min | 0 early, 0 late | pass |
| PV.topup-conservation | requested = installed + refused + in flight | conservation | 12 = 12 + 0 + 0 | pass |
| PV.rsu-renewal | 1 request per RSU at the boundary; no LA, no shuffle; sealed to RA | CAMP T3, T68, SCMS-1213 | 1 + 1; 0 LA; 0 shuffle | pass |
| **PV.rsu-cert** | RSU certificate implicit, 169 h, series 3, no linkageData | CAMP T3; [SMP] 6.3.3 | **not observable**: no record carries an RSU's certificate | **open** (§4.3) |
| PV.m.topups | `topups_started/completed`, `certs_topped_up` | records | 12/12/240 both | pass |
| PV.m.topup-latency | `exchange_latency.topup` n, sum, max | records | 12, 724.094 s, 60.350 s both | pass (rule corrected, §3) |
| PV.m.rsu-app-certs | `rsu_app_certs_renewed/preloaded` | records | 2 / 4 both | pass |
| PV.m.backend-traffic | Backend view rx/tx of RA, LA1, LA2, ACA | `pki.flow` | 8 entities, 0 off | pass |
| PV.m.issued-counters | PKI view `plv_issued`, `certs_issued`, `requests_accepted` | SPDUs | 8 counters, 0 off | pass |
| PV.m.pool-fields | `node.security.pool_stored` | `node.credentials` | 16,558 rows, 0 off | pass (rule corrected, §3) |
| PV.cell-expiry | A cell is `expired` only after start + 169 h | 1609.2 validityPeriod | 7,540 cells, 0 off | pass |
| **PV.view-pdu-carrier** | No message shorter than its SPDUs; `.info` SPDU on `info-request` | message ≥ its SPDUs | **16 short**: 12 `file-request`, 4 `report-forward` | **fixed** (§4.2) |
| PV.camp-sizing | Per device-week: 1 `.info`, 1 file (per top-up); 20 ACA, 20 PLV per LA (per weekly job); year-1 `.info` rate | CAMP 6.3.4.4: 14 req/s | 1, 1, 20, 20 (12 top-ups, 27 jobs); 14.05 req/s | pass (rule corrected, §3) |
| PV.determinism | Same seed → identical `pki.flow` + `node.credentials` | invariant | sha256 `2db9550d…` both (content digest `af165814…` both) | pass |

Not observable in a run's records, and covered elsewhere:
- **The 1,000-request / 2-day request shuffle.** Every first request happens before the
  run, and `preload_at` zeroes the shuffle window there. A weekly pre-generated week goes
  straight to the ACA (`provision.rs`, `pregen`). Covered by `provisioning_b3a.rs`
  (protocols D5).
- **The blocklist stopping pre-generation.** No i-period boundary falls after the MA's
  decisions in `conf`. Covered by `pregeneration.rs`
  `a_weekly_job_armed_before_the_blocklist_generates_nothing` and the oracle's RA-BLOCK
  rule.

## 3. Wrong checks corrected (before any verdict was recorded)

- **PV.m.topup-latency.** The first pass measured from the `requested` stamp, and the engine
  was 16.6 ms longer per top-up. `backend.rs` defines `exchange_latency` as "request to
  completion". The request is the device's decision at the trigger: the boundary, under
  `topup_below_periods` 1. The `requested` stamp comes after the device has signed. Measured
  from the boundary (taken from the calendar, not the engine), all 12 match to the
  nanosecond.
- **PV.m.pool-fields.** The first pass left out expired cells (1,967 rows off).
  `sec_records.rs` documents `pool_stored` as "certificates stored at all".

- **PV.aca-per-cert, with the fix.** The weekly job now makes every account's week, so 300
  AcaRa answers (15 accounts that did not ask in the run) stay in the RA's batch files and
  have no AcaEe-private delivery. The rule now matches each delivered private response to the
  AcaRa that carried it (by bytes) and requires every other answer to be on a
  `pregeneration` flow: 0 lost, 0 orphans.
- **PV.camp-sizing, with the fix.** ACA requests and PLVs per device-week are counted over
  the weekly jobs (one per account-week), not over the top-ups: a device that asks while
  another account's request started the job carries no RA→ACA traffic on its own flow.

## 4. Findings

### 4.1 Fixed: the ACA could group each device's certificates (PV.aca-cannot-group)

**Was** (8becf72a): `release_jobs` sent one `cert-request` per (job, i) carrying all `jmax`
`RaAcaCertRequestSpdu`s of one device's week in `j` order, and the RA routed the PCA's answer
by the job's flow `run`. A pre-generated week went to the ACA alone, the moment its device's
`.info` arrived. Under the real 1,000 / 2-day shuffle the batch was also released in arrival
order. The ACA could group every device-week: 20 certificates of one device per message.

**Now** (`0d62f322`, `v2xw-proto/src/scms/run/provision.rs`):
- **Shuffle across devices.** A release (the initial-request shuffle at 1,000 requests or
  2 days, or one weekly job) sends one `cert-request` per request, in a seeded Fisher-Yates
  permutation of the whole batch (its own stream, `scms/cert-request-shuffle`, keyed by the
  release). No message or position ties two requests to one device.
- **Route by handle.** The RA keeps `aca_routes`: HashedId8 of each request → (job, i, j). It
  files each `AcaRaCertResponseSpdu` by the request hash it carries (the modelled path: by
  the handle), never by the flow it arrived on. `CertResponse` names the requests it answers,
  so the RA knows when a period's `<requestHash>_<i>.zip` is whole and assembles it itself.
- **Weekly job for the population.** When the schedule reaches week `i` (or a device first
  asks for a week already due), the RA expands it for every account whose week is due, in
  one batch. The asking device's week rides its own top-up flow; every other account's runs
  on a new `pregeneration` flow (RA, LA1, LA2, PCA). A flow that asks while the job runs is
  told `batch_ready` when its file is assembled.

**Measured (`conf`).** At the 615 boundary each RA released one batch 0.10 s after the boundary:
provider-a 340 requests of 17 accounts, provider-b 200 of 10 (plus the 2 RSU renewals). One
request per message; each device's share is exactly 1/k. The ACA-side attacks:
- arrival adjacency: 52 one-device links of 539 against 38.0 at baseline (z 2.36; a
  permutation null for the two batches gives p 0.025 and 0.14, consistent with chance);
- equal length: every request has the same length, so 0 links;
- any shared 8-byte string held by at most half the batch: 1,317 of 20,055 links one
  device's, against 1,325.7 at baseline (z −0.25).

The faults turn both rows red: each device's week at its own instant in one message (29
batches, 20 per message, share 1.0), and each batch grouped by device (z 80).

**Cost.** The PCA now serves one message per certificate: provider-a 343 received (was one per
device-week), provider-b 204. The weekly job certifies 27 account-weeks instead of the 12
that asked (540 against 240 certificates). The top-up latency is unchanged in shape: 12
top-ups, max 60.350 s (was 60.342 s).

**Not observable, unchanged.** A pre-run pool is issued on a scratch kernel one device at a
time, and those batches carry no message envelope (`preload` PDUs). The real RA mixed them
with requests from devices outside the simulated fleet; the checker judges in-run batches.

**Still open: provider-b's second ACA** (GOV.D1.acas). `aca/1` is placed but has no
certificate and carries no traffic. Design for the fix: issue it in the PDU PKI with the core
ACA's form (`under_anchors_profiled`, after the core draws, so no other certificate moves);
host and link it in the provider's own run (`new_peer`); have the RA spread its shuffled
requests over its ACAs; and sign at the receiving ACA. The deployment's split rule is not
published (ASSUMPTION: round-robin).

### 4.2 Fixed: SPDUs attributed to the wrong message (PV.view-pdu-carrier)

`attribute_pdus` (`v2xw-proto/src/scms/run/mod.rs`) gave each SPDU to the first step of the
handler on its leg. When one handler sends several messages on a leg, that is wrong in two
places:
- **Top-off session.** It sends a 103 B `file-request` and then the `info-request`, so the
  318 B `.info` `EeRaDownloadRequestSpdu` rode the `file-request`, and the `info-request`
  showed no SPDU.
- **Report-shuffle release.** It sends one `report-forward` per report, so every released
  report (up to 149) was put on the first forward.

The Studio PKI view and every `pki.flow` reader (the oracle, `audit_prov.py`) saw messages
shorter than their contents.

**Fix (`ccff6ad4`).** A step takes an SPDU only while it still has room for it (step bytes ≥
the SPDUs it carries). The old leg-only rule stays as the fallback.

**Test.** `v2xw-proto/tests/pki_view.rs::a_top_off_s_info_request_carries_its_own_spdu_and_no_step_is_shorter_than_its_spdus`.
Without the fix it fails with "EeRaCertRequest rode the file-request": a device's first
provisioning session had the same fault.

**Goldens (`966de5d0`).** The `phase2-path` record digest moved
(`fd19a3ed…` → `57c38506…`). Its frame digest and record count did not change, and
`i-s1-phase2-60s` did not move.

**Tests run.** `phase2` 13/13, `scms_scale` 1/1, engine `pki_view` 8/8, proto `pki_view`
10/10.

**Re-run on the fixed build** (`conf-fix`, same seed):
- PV.view-pdu-carrier is green: 0 short messages.
- Every other row keeps its verdict.
- `phase2` report, `apps` and the record count are identical to `conf`. Only `pki.flow`'s
  bytes change (32,195,437 → 32,203,537 B, same 24,384 records).
- Oracle `records` on `conf-fix`: **PASS, 220,684 / 220,684** checks over 19,643 PDUs, the
  same count as oracle-2026-10-24.
- Proto `appcert` 11/11, `ma_cases` 7/7, `pregeneration` 11/11 and `provider_runs` 7/7
  pass.

### 4.3 Open: no record carries an RSU's application certificate (PV.rsu-cert)

`node.credentials` is written only for vehicles. `pki.state` carries the CA certificates. The
`AcaEeCertResponsePrivateSpdu` to an RSU is sealed to it and recorded without its plaintext.
A run therefore cannot show, and the Studio cannot inspect, an RSU's application
certificate. The library capture covers its format (`check_appcerts.py`, 18/18; protocols
E1, E2). Suggested: write `node.credentials` for RSUs as well.

### 4.4 Notes

- **The Backend view still draws the LOP.** `backend.state` draws a `lop` box with 2,087
  messages for each provider. `pki.flow` folds the LOP away because the US profile deploys
  none, and its link is transparent (zero latency, no queue), so timing is right. Only the
  Backend view disagrees with the PKI view.
- **Server API.** The v2xw-server PKI store absorbs these same `pki.flow`, `pki.state` and
  `node.credentials` records (`v2xw-server/src/pki.rs` `absorb`). Its answers were not
  measured separately in this audit.

## 5. Determinism, conservation, metrics

- **Determinism.** `conf` and `conf2` are byte-identical in `pki.flow` and
  `node.credentials`.
- **Conservation.** Top-ups conserve: 12 = 12 + 0 + 0. RaAca = AcaRa = AcaEe-private =
  2,846. The LA counters equal the PLVs: 1,520 for provider A, 1,320 for provider B.
- **Metrics.** Every provisioning metric the run report and the PKI and Backend views emit
  for these components equals its recount from the records (rows PV.m.*).
