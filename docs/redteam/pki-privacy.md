# Red team: PKI, insiders and revocation

Research simulation only. This study looks at the engine's clone of the US SCMS as an attacker inside or next to the backend would see it. It covers what each coalition of authorities learns, what a compromised roadside unit can do to CRLs and reports, what a stolen enrolment key buys, and how long revocation takes on the real calendar. Each number below comes from a reproducible test or a cited parameter. Each test was shown to fail when its fault was injected (§6).

Written 2026-10-18 at `18f9291` plus this package. Reproduce with:

```
cargo test -p v2xw-proto --test redteam_pki -- --nocapture --test-threads=1   # RT lines = the tables below
```

Sources use the keys of `docs/design/scms-real/03-protocols-and-operations.md` §0 ([CAMP122], [SM-PROV], [SM-MBR], [5GAA], [BRECHT], [PRIMER], [CVP-LOG], [CVP-P2]).

## 0. Ranking

Severity is what the attacker gains. Likelihood is how hard the precondition is. Both run 1 (low) to 3 (high).

| # | Weakness | Severity | Likelihood | Evidence | Status in the engine |
|---|---|---|---|---|---|
| R1 | **Revocation is slow by design:** 18 h median and 96 h worst from detection to a published CRL, even with an automatic MA. Up to 264–312 h before a vehicle that fetches weekly enforces it | 3 | 3 | §4, `the_revocation_window_on_the_real_calendar` | Modelled faithfully (the real SLA and calendar) |
| R2 | **A replayed older CRL erased a newer revocation.** A compromised RSU replays the CRL Generator's own earlier, validly signed list. The proto device model replaced its list with it, and a revoked device that checked itself resumed transmitting | 3 | 2 | §3.1, `a_replayed_older_crl_is_refused_and_the_newer_revocation_holds` | **Fixed** (validation/protocols.md I3, I4): devices refuse a CRL whose 1609.2 counter is not newer |
| B4 | **A second provisioning chain survived revocation.** After any policy change the RA accepts a second butterfly request from the same enrolment ([CAMP122] SCMS-520). Revocation published only the reported chain's seeds, so 2 of the device's 4 certificates stayed valid | 3 | 2 | §2.3, `every_chain_of_a_revoked_enrolment_is_revoked` | **Fixed** (validation/protocols.md G6): the RA returns every chain pair of the blocklisted enrolment |
| B1 | **Insider collusion:** LA1+LA2 can link every certificate of every device forward *and backward* from their seeds. RA+ACA or RA+LA1+LA2 can name every device | 3 | 1 | §2.1, `coalitions_learn_what_the_separation_rules_predict` | Modelled faithfully. The separation is organisational only ([BRECHT] §X) |
| R4 | Revoked device's remaining lifetime: up to 3 years of pre-downloaded certificates ([CAMP122] Table 67, THEA [CVP-LOG] p.16). Only receivers with a fresh CRL refuse them | 2 | 3 | §4.3 | Modelled (CRL entries persist; receivers enforce the overdue tolerance in `v2xw-node::rxval`) |
| R5 | CRL growth: 33.08 B per linkage entry (measured), retained until the revoked batch expires. About 1 MB at 30,000 entries, which is a large RSU broadcast | 2 | 2 | §4.4 | Modelled (real encoder) |
| B2 | A compromised root or ICA: removal needs a new elector-signed CTL (≤ 24 h after the decision in the draft CP) or a series-2 CRL entry (1 business day), plus each device's fetch interval | 3 | 1 | §2.4 | Partly modelled (root-rollover CTL in F1). Compromise-to-removal is not timed by a test |
| B3 | A compromised RSU editing or forging CRLs or MBRs: refused, because the list is signed by the CRLG and the MBR is signed by the reporter and encrypted to the MA. Dropping only delays | 1 | 2 | §3.2, `an_rsu_edited_crl_is_refused_whole`; existing `ma_cases`/`mbr` tamper tests | Modelled faithfully |
| B5 | A stolen enrolment key without the butterfly keys: a second initial batch is refused (`RequestLimit`) and the existing batch is encrypted to the butterfly keys | 1 | 2 | §2.3 | Modelled faithfully |

## 1. Threat model and what is observable

- **Insiders.** Any subset of the RA, ACA, LA1, LA2 and MA reads its own state as the engine keeps it (`ScmsState`) and nothing else. The test's ground truth comes from the devices, never from an authority.
- **Compromised RSU.** It holds whatever the CRL broadcast path carried (`state.crl_broadcast`), including every earlier signed version, and it relays uploads it cannot open.
- **Recorded.** Every refusal is counted: `DeviceState::crls_rejected`, `ScmsState::pdu_rejections`, `RaState::max_reqs_refused`. Revocation stage times are stamped on `kernel.stages`. The engine's PKI view (`pki.knowledge`, E1/E3) shows who learned what on a live run.

## 2. Backend and insider threats

### 2.1 Collusion matrix (B1)

The fleet is three devices, two periods and two certificates per period: 12 certificates on the air. "Linked" is the fraction of same-device certificate pairs the coalition can group. "Named" is the fraction of certificates it attributes to the right device. Every coalition made 0 wrong links.

| Coalition | Linked | Named | How (engine state it uses) |
|---|---|---|---|
| Eavesdropper alone | 0.00 | 0.00 | Linkage values are all distinct |
| ACA alone | 0.00 | 0.00 | Its handle is per certificate (hash of one RA→ACA request), never per device |
| LA1 alone (or LA2) | 0 hits | — | A pre-linkage value never equals a linkage value |
| RA alone | — | 0 | Holds identities and request records, no linkage value |
| **LA1+LA2** | **1.00** | 0.00 | They expand every chain's `plv1`, `plv2` and match `plv1 ⊕ plv2` against the air. A hash set keeps it linear in the number of chains, so this scales to a deployment. They hold `ls(0)`, so this links **past** periods as well, which defeats revocation's forward-only property ([BRECHT] §VI-D; `backward_privacy.rs`) |
| **RA+LA1+LA2** | 1.00 | **1.00** | The RA's request record maps the (LCI1, LCI2) pair to the device |
| **ACA+RA** | 1.00 | **1.00** | ACA handle → `ra.handles` → request → device. This matches [BRECHT]/S8 §4.1: "no privacy if RA and ACA collude" |

The table does not include ACA+one LA, which links a chain without naming it (02 §9.5, derived). The engine's ACA does not retain decrypted PLVs, so this coalition cannot be computed from state. Modelling note: `LaState::owner` stores the device's `NodeId` as the LA's opaque per-device handle. The collusion test does not use it, because a real LA knows only the chain, but the PKI view shows it as ground truth.

**Mitigations.** The separation rules ([BRECHT] §X: PCA ≠ RA, LA1 ≠ LA2, MA ≠ RA/LA/PCA) are organisational, so the operators must be truly independent. The threshold linkage authority (`threshold-revocation-3of5.yaml`) raises the LA1+LA2 collusion bar from 2 parties to t. The RA shuffle (1,000 requests or 2 days) limits what the ACA can infer from timing.

### 2.2 Separation the engine already proves

`tests/privacy.rs` shows that the RA holds no linkage value or seed and cannot compute the certified key, and that the PCA holds no identity. `tests/resolution.rs` shows that one LA answers one bit and the MA needs both. These are the single-party rows above.

### 2.3 A stolen enrolment key, and hoarding (B4, B5)

- **Key alone (B5).** A second initial request from the same enrolment is refused with `RequestLimit` (ra-maxReqs = 1, [SMP] Table 3). The thief can download the victim's batch files, but they are encrypted to the victim's butterfly keys.
- **Key plus a policy change (B4, fixed).** The RA accepts a new request "when parameters change" ([CAMP122] SCMS-520). Measured before the fix: 2 accepted requests and 1 CRL entry after the device is revoked. 2 certificates were revoked and **2 escaped**, and the enrolment was blocklisted. After the fix: 2 CRL entries, 0 escape. The escaping chain is the thief's own batch, or a misbehaving device's spare, and it stays valid for its whole issued lifetime (up to 3 years). A thief who strikes before the victim's first request takes the only batch, and the victim is refused.
- **Mitigation (done).** The RA returns every LCI pair recorded for the enrolment (`RaState::requests` filtered by device), and the CRL Generator publishes one entry per chain. Still open: a request after a policy change could retire the previous account's chains. Hardware key storage ([SM-EESEC]) is the primary control.

### 2.4 A compromised root or ICA (B2)

There is no timed test yet. The bounds come from the documents:

- **Root.** A new CTL is issued only on SCMS Manager request, signed by a quorum of electors (3 of 5 on the real CTL, A5). The draft CP requires publication ≤ 24 h after a removal ([5GAA] (47)). Components fetch GPF/GCCF at least daily ([CAMP122] Table 67). The EE fetch interval is unspecified.
- **ICA.** A series-2 CRL entry is published within 1 business day ([SM-PROV] §3.4.9). Component revocation takes ≤ 5 days, or as soon as possible on compromise ([5GAA] (255)).
- **Worst-case window.** It is decision time plus up to 24 h (CTL) or up to 96 h (business-day slot over a long weekend, §4), plus the device's fetch interval. Until then, an attacker with the ICA key can issue enrolment and pseudonym certificates that every receiver accepts.
- **Owed.** A test that compromises the ICA in `scms-conformance.yaml` and times the CTL/CRL removal. The governance flows exist (F1 root rollover).

## 3. A compromised roadside unit

### 3.1 CRL replay (R2): rollback

X is revoked (CRL v1 = [X]), then A (v2 = [X, A]). Vehicle C and device A fetch v2 over cellular. The RSU then broadcasts v1 again, byte for byte as the CRL Generator signed it.

- **Measured before the fix.** v1 was accepted, with 0 rejections. A was **no longer revoked** at C, X still was, and A (revoked) **un-silenced**.
- **Cause.** The proto device model (`scms/run/crl.rs`, `dev.crl = list;`) replaced its list with any list that verified. It did not compare `crlSerial` or `issueDate` with the list it held.
- **Fixed.** Devices now refuse a CRL whose counter (`crlSerial`, or `iRev` and `indexWithinI`, then `issueDate`) is not above the one held for that CRACA and series (`run::crl::CrlOrder`). After the fix: 1 stale refusal at C and 1 at A, A still revoked and silent. The engine node's rule (below) now also covers deltas and same-second lists (validation/protocols.md I3, I4).
- **Engine receivers.** By reading the code, the receivers that enforce the CRL in the engine (`v2xw-node::stores::CrlGate::add_verified_linkage`) only add entries, so a replay there cannot remove one. It can still hide a revocation from a vehicle that has not yet heard v2, until v1's `nextCrl` plus the 2-week overdue tolerance (`rxval::OVERDUE_CRL_TOLERANCE_S`). With the 7-day `next_crl` assumption that is up to **21 days**.
- **Mitigation.** Refuse a CRL whose `crlSerial` or `issueDate` is older than the one held, for the same series and CRACA. Keep CRL processing additive per series. The fault test F3 applies this fix and turns the pinned test red.

### 3.2 Editing, forging and dropping (B3)

- **Editing.** An RSU that drops A's entry and adds a duplicate entry has the list **refused whole**: 1 rejection, and C keeps its list with A still revoked. Forging needs the CRLG key.
- **Reports.** The RSU cannot retarget an MBR: it is signed by the reporter's pseudonym and encrypted to the MA ([SM-MBR] §3). Tampered, misaddressed and unauthenticated uploads are refused (`ma_cases::the_ra_and_the_ma_refuse_unauthenticated_misattributed_and_tampered_uploads`, `mbr::the_ma_refuses_tampered_misaddressed_and_wrong_psid_reports`).
- **Dropping.** An RSU that drops uploads only delays them. The device deletes a report only after the TCP-confirmed upload, so it retries at the next RSU or over cellular. Unsent reports older than a week may be deleted ([SM-MBR] §2.2), so an attacker who controls every RSU on a route for over a week can suppress reports.
- **CRL starvation.** An RSU-only vehicle near compromised RSUs never hears the CRL. The receiver's defence is the overdue tolerance (reject everything 2 weeks past `nextCrl`, [CAMP122] Table 67), which turns starvation into a denial of service rather than acceptance.

## 4. Revocation weaknesses

### 4.1 The window from detection to publication (R1)

A detection is placed every 15 minutes through calendar year 2026, using the real Eastern-time calendar with DST and federal holidays (`scms::calendar`). The CRL is published at the next business-day 10:00 ET slot (`next_publication_slot`), and the "within 1 business day" deadline was met for every sample.

| Pipeline (RA→MA flush, MA decision) | p50 | p95 | max | Worst detection | + weekly fetch, max |
|---|---|---|---|---|---|
| Automatic MA, report reaches the MA at once | 17.8 h | 67.8 h | 96.0 h | Fri 2026-01-16 (MLK weekend) | 264 h |
| Automatic MA, 24 h RA shuffle flush ([CAMP122] SCMS-765) | 41.8 h | 92.0 h | 120.0 h | 2026-01-15 | 288 h |
| Manual 8 h review, flat (engine default, ASSUMPTION) | 49.8 h | 100.0 h | 128.0 h | 2026-01-15 | 296 h |
| Manual 8 working hours (09–17 ET, business days) | 70.5 h | 121.0 h | 144.0 h | 2026-01-14 | 312 h |

- **Reading.** Even a perfect automatic MA leaves a median of 18 h, because the publication slot is daily on business days. The flush and the human reviewer add a day each. The last column adds the CAMP minimum fetch cadence for vehicles (no later than once a week, [CAMP122] §3 item 10). RSU broadcast and cellular fetch on every RA contact take that to near zero for connected vehicles.
- **Engine note.** The engine's `MaMode::ManualReview` is a flat 8 h that ignores nights and weekends, so it is about 21 h optimistic at the median. A business-hours operator would be more realistic for the pilot-style MA.

### 4.2 Vehicles that never receive CRLs

During the CV pilots there was no standard CRL distribution ([CVP-P2] item 21), and NYC loaded one week of certificates instead ([CVP-LOG] p.16). A vehicle with neither cellular nor RSU contact never enforces any revocation, and it rejects all traffic 2 weeks after its last CRL's `nextCrl`. The engine shows both behaviours: `revocation_path::the_two_distribution_paths_are_reported_separately` and `rxval::CrlOverdue`.

### 4.3 Staying offline, and the remaining lifetime (R4)

The RA blocklist stops future downloads (SCMS-509), but a revoked device keeps every certificate it already holds:

- up to **3 years** (156 weeks × 20 = 3,120) with the THEA-style full download, or
- **1 week** with the NYC policy.

Staying offline does not help the attacker against receivers that have the CRL. The linkage entry covers every later period ([BRECHT] §VI-D). It does help against receivers that lack the CRL, for as long as those receivers keep within the overdue tolerance. The NYC one-week policy cuts the exposure by up to 156×.

### 4.4 CRL size growth (R5)

The measured size is 33.08 B per linkage entry (04-validation-report §5, real encoder, oracle-checked), plus about 300 B of envelope. Entries stay until the revoked batch expires ([SM-PROV] §3.4.10), so the steady state is the revocation rate × the batch horizon. The fleet sizes and rates below are ASSUMPTIONS:

| Revocations / year | Horizon | Entries | Full CRL |
|---|---|---|---|
| 1,000 | 3 y | 3,000 | ≈ 100 KB |
| 10,000 | 3 y | 30,000 | ≈ 1.0 MB |
| 100,000 | 3 y | 300,000 | ≈ 9.9 MB |
| 10,000 | 1 wk (NYC) | ≈ 200 | ≈ 7 KB |

Per entry, the OBE does 2 SHA-256 operations per period walked and 2·jmax AES operations ([BRECHT] §VII; charged by `crl.rs`). At 30,000 entries that is about 1.2 M AES operations per week, which is negligible. The size, however, is a large broadcast for a vehicle passing an RSU. Delta CRLs (`CrlIssuance::deltas`, on by default) mitigate the steady-state cost. A shorter batch horizon is the strongest lever.

## 5. Privacy (pseudonym tracking): status

This package did not build a new tracking study. It relies on what the engine already establishes:

- **Identifiers change together.** The certificate, BSM temporary ID and link-layer address change together. The path history is cleared and `msgCnt` restarts at a random value on a pseudonym change (`v2xw-msg::j2945::clear`, `v2xw-node::runtime`), so BSM content gives no link by design.
- **Passive observer.** `v2xw-threat::privacy::PrivacyObserver` and its tests reproduce the Wiedersheim et al. (WONS 2010) result that a change without a silent period is linkable. The `pseudonym-privacy.yaml` and `pseudonym-strategies.yaml` scenarios report `linkability` per strategy.
- **Backend correlation.** The LOP hides the device's address from the RA ([CAMP122] SCMS-517). By design the RA still learns the enrolment identity of every upload (§2.1, RA row).

**Owed (not done here).** A sweep over observer network density and change strategy, BSM-attribute linkability (vehicle size), and RSU/cellular contact correlation.

## 6. Fault injection

Each fault was applied with a restore-and-touch wrapper (same contract as `tools/mutate/mutate.py`) and the suite was run. Clean runs before and after passed 5/5.

| Fault | Test that must go red | Result |
|---|---|---|
| F1: ACA handle per device request instead of per certificate | coalitions | RED (coalitions failed; 4 others passed) |
| F2: linkage value = plv1 (LA2 contribution dropped) | coalitions | RED (coalitions failed; 4 others passed) |
| F3: device refuses a CRL shorter than the one held (the R2 fix) | crl replay | RED (crl replay failed; 4 others passed) |
| F4: device skips both CRL signature checks | edited CRL | RED (edited CRL failed; 4 others passed) |
| F5: RA refuses every second request, even after a policy change (the B4 fix) | second chain | RED (second chain failed; 4 others passed) |
| F6: weekends count as business days | calendar window | RED (calendar window failed; 4 others passed) |

## 7. Owed

- A compromised root/ICA timed through the CTL and CRL (B2).
- ~~The R2 and B4 fixes~~: done (validation/protocols.md §J records the choices, since real devices' `crlSerial` handling and the RA's LCI return are not published).
- A business-hours `MaMode`.
- The privacy study of §5.
