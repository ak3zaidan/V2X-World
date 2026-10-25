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
- **Rows.** `crates/v2xw-engine/tests/expectations/scms-audit-prov.yaml` has 31 rows. They
  were committed with `measured: pending` before the checker ran (`ece23338`, `28f732b4`).
  Three rows were added after the first pass, and each is labelled in its `note`.
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
| PV.aca-per-cert | RaAca = AcaRa = AcaEe-private, 40 per vehicle | 1 per certificate | 2,846 each; 0 off | pass |
| PV.encrypted-to-device | PCA response sealed to the EE; never opened at the RA | [ASN-2.1]; CAMP 5.2.7.6.6 | 2,846 sealed to `ee`; 0 opened at an RA | pass |
| PV.aca-learns-no-device | No device identifier in RA→ACA SPDUs | ACA never sees EE identity | 2,846 searched for 199 ids: 0 | pass |
| **PV.aca-cannot-group** | ACA cannot group a device's certificates by the carrying message | RA shuffles across devices ([CAMP122] 5.2.7.6.4; [BRECHT] III-C) | **20**: each RA→ACA message holds one device's whole week | **open** (§4.1) |
| PV.lop-not-deployed | No LOP hop in the US profile | not deployed (01 §2.2) | 0 hops in `pki.flow` | pass (view note §4.4) |
| PV.cert-implicit | Pseudonyms implicit (ECQV) | 1609.2 6.4.8 | 2,840, explicit 0 | pass |
| PV.cert-validity | Start on the i-period grid, hours(169) | [SMP] T2; CAMP T3 | 0 off grid, 0 not 169 h | pass |
| PV.cert-linkage | linkageData, iCert = i, 9 B LV, series 1 | 1609.2 6.4.12 | 0 off | pass |
| PV.pool-20-per-week | 20 certificates per (vehicle, i) | 20 ([SMP] T2) | {20: 142} | pass |
| PV.pool-at-join | Two i-periods at join | `device_pool_periods` 2 (ASSUMPTION) | {2: 65} | pass |
| PV.topup-at-boundary | Every vehicle on the road with its pool ending at 615 tops up at the boundary | `topup_below_periods` 1 | 12 of 12; start ≤ 16.6 ms | pass |
| PV.topup-no-new-request | Top-off = `.info` + 1 batch, no new butterfly | CAMP 5.2.7.8, SCMS-576 | 0 new; 2 downloads each | pass |
| PV.topup-20-certs | 20 certificates for i = 616 | 20 per period | 12 × 20 delivered and held | pass |
| PV.pregen-delay | Weekly job within `ra-maxGenDelay` | ≤ 1 week | 0.094–0.096 s | pass |
| PV.download-after-ready | Batch asked for only once held, within 300 s | `nextDlTime`; minWait 5 min | 0 early, 0 late | pass |
| PV.topup-conservation | requested = installed + refused + in flight | conservation | 12 = 12 + 0 + 0 | pass |
| PV.rsu-renewal | 1 request per RSU at the boundary; no LA, no shuffle; sealed to RA | CAMP T3, T68, SCMS-1213 | 1 + 1; 0 LA; 0 shuffle | pass |
| **PV.rsu-cert** | RSU certificate implicit, 169 h, series 3, no linkageData | CAMP T3; [SMP] 6.3.3 | **not observable**: no record carries an RSU's certificate | **open** (§4.3) |
| PV.m.topups | `topups_started/completed`, `certs_topped_up` | records | 12/12/240 both | pass |
| PV.m.topup-latency | `exchange_latency.topup` n, sum, max | records | 12, 724.085 s, 60.342 s both | pass (rule corrected, §3) |
| PV.m.rsu-app-certs | `rsu_app_certs_renewed/preloaded` | records | 2 / 4 both | pass |
| PV.m.backend-traffic | Backend view rx/tx of RA, LA1, LA2, ACA | `pki.flow` | 8 entities, 0 off | pass |
| PV.m.issued-counters | PKI view `plv_issued`, `certs_issued`, `requests_accepted` | SPDUs | 8 counters, 0 off | pass |
| PV.m.pool-fields | `node.security.pool_stored` | `node.credentials` | 16,558 rows, 0 off | pass (rule corrected, §3) |
| PV.cell-expiry | A cell is `expired` only after start + 169 h | 1609.2 validityPeriod | 7,540 cells, 0 off | pass |
| **PV.view-pdu-carrier** | No message shorter than its SPDUs; `.info` SPDU on `info-request` | message ≥ its SPDUs | **16 short**: 12 `file-request`, 4 `report-forward` | **fixed** (§4.2) |
| PV.camp-sizing | Per device-week: 1 `.info`, 1 file, 20 ACA, 20 PLV per LA; year-1 `.info` rate | CAMP 6.3.4.4: 14 req/s | 1, 1, 20, 20; 14.05 req/s | pass |
| PV.determinism | Same seed → identical `pki.flow` + `node.credentials` | invariant | sha256 `c55693ad…` both | pass |

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

## 4. Findings

### 4.1 Open: the ACA can group each device's certificates (PV.aca-cannot-group)

`release_jobs` (`v2xw-proto/src/scms/run/provision.rs`) sends one `cert-request` message per
(job, i). The message carries all `jmax` `RaAcaCertRequestSpdu`s of one device's week, in `j`
order. The PCA's answer goes back by the job's flow `run`, which the RA uses to route it
(`CertResponse` handler).

This holds under the real 1,000 / 2-day shuffle as well: the batch is released job by job.
It holds even more for a pre-generated week (`only = Some(run)`) and for the pre-run pool
(window zeroed). So the ACA knows which 20 certificates belong to one device-week. That is
the linkage the RA shuffle exists to deny ([CAMP122] 5.2.7.6.4; [BRECHT] III-C). The SPDU
contents leak nothing (PV.aca-learns-no-device). The grouping is the leak.

**Fix design (not made here: it moves timing and every SCMS golden, and needs owner sign-off
on cost):**
- Release each shuffle batch as RA→ACA requests interleaved across devices, either one
  message per request or one message per batch in shuffled order.
- Route the PCA's responses by the per-certificate handle the RA already keeps
  (`ra.handles`), not by `run`.
- Pre-generate the new week as a population job at the boundary. A real RA adds the week for
  every account; it does not wait for each device's `.info`.

The last change also moves the backend burst that 06 §6 item 3 attributes to the boundary.
Today the weekly pre-generation runs at the moment each device's `.info` arrives (`pregen_next`),
so the ACA's weekly load lands in the 100 ms after the boundary. CAMP's RA has the week
generated in advance.

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
