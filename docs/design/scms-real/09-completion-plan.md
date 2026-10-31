# SCMS completion plan (2026-10-30)

The owner's goal: an SCMS that is complete end to end, tested end to end, free of known
defects, and identical to the US SCMS pipeline wherever the US system is observable, so it
can be written up in a publication. This file is the work list for the build waves that
deliver it, the decisions the owner delegated, and what was deliberately left out.

The owner's rule for every choice: follow the SCMS architecture first, then what makes
sense for a simulation; where a difference is negligible for this engine's results, skip it
and record it in §5.

Sources of the list: the rule register (06-validation-summary.md §2.1, last rewritten
2026-10-23), the expectation tables under crates/v2xw-engine/tests/expectations/, the
BUILD-STATUS sections of 2026-10-28 and 2026-10-29, and a code audit of the SCMS crates.

## 1. Defects (wave A)

| Id | Defect | Evidence |
|---|---|---|
| SC-D1 | Some RSUs air a new CRL 0.146 s after publication, before their backhaul could deliver it; some backhaul messages are counted twice | live-rsu LR.S.l240.bc4.fix.lag-min/lag-max (FAIL), LR.S.dbl.*; the ignored live_rsu.rs tests cover it |
| SC-D2 | A mid-run joiner holds the t0 CRL, which can be overdue: 755 honest refusals | scms-stress MWC.recovery (R-JOINCRL) |
| SC-D3 | CRL Generator and CRL-store entries stay 0 through 46 ks of a 2-day run | scms-final-run FB2.crl |
| SC-D4 | Answers sent to a down RA are lost: no backend-to-backend retry | scms-edge-cases E3 |
| SC-D5 | Slow GNSS drift has no streak gate, so honest devices are reported (88 cases at full scale) | scms-stress FS.mbr-airtime / FS.honest-revocations (R-USFP) |
| SC-D6 | An honest vehicle with its clock 83 min ahead is revoked | scms-edge-cases E2 |
| SC-D7 | The hybrid PQ signature is added to every SPDU even under the certificate cycle: 0 frames | scms-final-bench PQHMC.*, PQHFC.ccsig (R-CCSIG) |
| SC-D8 | The P-384 profile keeps P-256 operation costs | live-scms LS.S384.verify |
| SC-D9 | A report burst on resuming after a fast-forward; frequent backend stops in a second fast-forward | scms-final-run FB2.honest-reports (R-FFBURST), FB2.ff-ticks |
| SC-D10 | The BSM timing slip repeats a secMark; 26 truth rows at (0, 0) | scms-paper1 P1.RP.gt-vs-sm.* (TM-1), P1.VR.full-truth-pos (VR-2) |
| SC-D11 | Attack harness: CRL refusals scored as signature failures; Sybil has no onset on the release build; RandomSpeed ignores intensity; the expired-certificate host passes validity 0..MAX | live-atk F2-F5 rows |
| SC-D12 | ECA and ACA each carry two certificates; provider-b's ICA count is off | audit-gov GOV.L.onecert, GOV.L.counters |
| SC-D13 | No record shows an RSU installing a CRL or carrying its application certificate; 282 report-store order violations; privacy link claims stamped late; one unverified-with-certificate reception | audit-mbpipe AU.DIST.rsu, AU.STORE.order; audit-prov PV.rsu-cert; audit-sim-metrics RS.time-order; audit-db1-3 OA.RX.cached-verifies |
| SC-D14 | An enrolment certificate shorter than 1 h is written as `hours(0)`; `crl_version` is an entry count, not a version; server memory grows over long runs | BUILD-STATUS 10-29 engine requests 1 and 14; measure.md:73 |

## 2. Built work to finish and merge (wave B)

| Id | Work | Where it is |
|---|---|---|
| SC-M1 | MA policy: 8 presets, per-provider policy, family sweep and `ma-replay --policies` (08 §7 B1-B3); B4, the optional Python hook | branch worktree-wf_ab880052-ba6-4 |
| SC-M2 | Server sites, capacity, 1 to 5 ICA levels (08 §5-§8 C1a-C3a; C3b wip snapshot 538a10fa) | branch worktree-wf_ab880052-ba6-5 |
| SC-M3 | infoStatus device flow (WP-35 P35.2, unverified wip 13ae0463) | branch worktree-wf_0343ab94-a4a-1 |
| SC-M4 | WP-19 leftovers: the WSA MessageCodec arm, the 10 Hz WSA rate, WSAs on the control channel, the P19.5 live-rsu rows | WP-19 notes in 00-adoption-plan.md |
| SC-M5 | WP-34 SPS-aligned BSMs for LTE-V2X: the release measurement that closes lte-latency-budget | scms-matrix lte-aligned-* rows |

## 3. Real-pipeline replacements (wave C)

| Id | Today | Real US SCMS | Change |
|---|---|---|---|
| SC-P1 | The engine maps a reported digest to its device from its own provisioning records | The ACA (PCA) looks the certificate up in its own issuance table | A PCA issuance table keyed by (i, linkage value) that the lookup reads; the engine's map stays only as labelled ground truth for checks |
| SC-P2 | Cases open as `direct`; the MA-LA same-device exchange is skipped | The MA links reports through the PCA and both LAs | Every case runs the real lookup and linking path |
| SC-P3 | The MA de-duplicates decisions by the engine's device identity | The MA knows only what the PCA and LAs return | De-duplicate by linkage chain identifier / linkage value |
| SC-P4 | Before a case the MA reads the CRL Generator's unpublished entries | The MA checks the published list | Read the last published CRL |
| SC-P5 | A shared report is not re-verified by the receiving MA | Each MA verifies what it acts on | Re-verify the evidence at the receiving MA (format stays modelled: unpublished) |
| SC-P6 | The default local detector is the research Legacy12 suite; SM-MBR runs only when named | US OBUs detect and file SM-MBR subtypes | `detect/us-sm-mbr` is the default under a US deployment |
| SC-P7 | BrakeAcceleration never fires; MaxAcceleration comes from speed differences | SM-MBR §2.3 reads `brakes` and `accelSet` | Carry both fields in the receiver's observation |
| SC-P8 | Component CRLs (series 2, 256) are never issued | The US profile Table 1 has a provider CRL of revoked components | Issue them, revoke a component, and show devices refusing what it signs |
| SC-P9 | Emergency identification certificates exist but are never used | Privileged vehicles hold identification certificates | Wire them for emergency vehicles (WP-20) |
| SC-P10 | A vehicle whose provisioning fails falls back to a bootstrap stand-in with no linkage value | A device without credentials cannot sign | Remove the fallback (count the vehicle as unable to sign) |
| SC-P11 | Region checks for countryOnly / countryAndRegions fall through | The receiver checks generationLocation against the signer's region | Country polygons (Natural Earth, public domain) for USA, Canada, Mexico |
| SC-P12 | Availability and stochastic service times are hooks only | Real servers have outages and variable service times | Build both, off by default, parameters labelled ASSUMPTION |

## 4. Rule gaps (wave D) and records (wave E)

Rule gaps, each with the spec in rule-gap-specs-2026-10-27.md: gov C-9 (CTL effectiveDate
is the ballot date), C-10 (genesis sequence setting), C-11 (dropped root carried in the
test CTL), G-2 (board review deadlines), G-10 (yearly test ballot), L-8 (365-day root
termination notice), G-11 (dual signatures in a transition, labelled ASSUMPTION), L-11
(LaSsp v1, laId 4/5 as the ISS certificates); proto H10 (indexWithinI past 255, labelled
ASSUMPTION), B9 (request bytes), D11 (download-maxAge, ra-maxAge under a labelled reading),
B5 (RA acknowledgement within 1 s, measured); veh C6 (2 km from the last change point).
Observability: the 2026-10-29 UI engine requests (both lists in BUILD-STATUS).

Wave E, validation, runs after every merge, on a quiet machine, one long run at a time:
the full-fleet runs (FB4 unbroken, FB2 two days, the final bench under its memory cap), every
open and blank expectation row (live-chains, scms-paper1, live-studio), the ignored long
tests, the conformance kit, the Studio e2e specs, sensitivity sweeps over the unpublished
values of §6, and a rewrite of the rule register from current evidence (including the
relabels of rule-gap-specs §3).

## 5. Decisions the owner delegated (2026-10-30)

| Question | Decision | Why |
|---|---|---|
| Real 8 h MA review and one-business-day CRL SLA by default? | Yes on the real calendar (already so); automatic decisions on a compressed calendar (already so) | The US behaviour; a compressed week makes an 8 h review meaningless |
| SCMS through RSUs on the service channel with WSA by default? | Yes, `net.rsu_ip.air: sch`, after the P19.5 rows are measured; goldens re-blessed with the cause | USDOT RSU Spec v4.1 and SM-MBR §4: SCMS traffic rides a WSA-advertised service, safety channel 172 carries safety messages |
| Overdue-CRL scope | Keep `craca` | IEEE 1609.2's validity model is per CRACA (each certificate names its cracaId and crlSeries); CAMP122 Table 67's "all" reading stays selectable |
| RSUs run misbehaviour detection? | No (accept the DETPRIV reading of veh M13) | SM-MBR and RSU Spec v4.1 assign reporting to OBUs |
| Default BSM stamp profile | Unchanged (chain off) | Negligible for SCMS results; Paper 1 selects its profile per scenario |
| LTE-V2X BSM generation aligned to the SPS reservation by default? | Yes after SC-M5's measurement | A real LTE-V2X stack generates into its reservation; misaligned generation is what breaks the 100 ms budget |
| Hold late-stamped records to the next tick? | No | A logging artefact, explained in fix round 5; no effect on results |
| Alternating channel access by default? | No (continuous) | J2945/1 single-channel operation on 172 |
| Buy the paywalled standards (IEEE 1609.2.1 body, 1609.2 §5.3.8, J2945/1, J2945/9, J3287)? | Owner action (a purchase); until then the rows stay "unverifiable" with their labelled readings | Not something the build can do |

## 6. Skipped or kept as modelled, with the reason

| Item | Kept as | Why it is acceptable |
|---|---|---|
| Internal MA-PCA-LA-RA messages as modelled enums with derived sizes | Modelled | Their encodings (LCI, PLV request, LA keys) are unpublished; sizes are derived and charged |
| MA-to-MA sharing format | Modelled (re-verified, SC-P5) | No published spec |
| Policy file (GPF/LPF) payload layout | Engine layout inside a real signed envelope | CAMP's ASN.1 is not public |
| Threshold ACA signs with one key | Kept | Research mode only, not the US system |
| Epidemic V2V CRL exchange | Not built | Not part of the US deployment |
| Out-of-band CRL requests (missingCrlIdentifier) | Not built | US devices download CRLs from the RA; no effect on US runs |
| Delta CRLs | Off | The US profile distributes full lists |
| Unified and compact butterfly | Refused | Not deployed in the US |
| Unpublished numbers (data-centre sites, fleet shares, MA thresholds and review time, store quota, report rate, backend capacity) | Labelled ASSUMPTION | Covered by the sensitivity sweeps of wave E |
| Couriered key ceremony steps (gov C-14) | Not modelled | Off-network, no effect on a run |
| WSAs on a separate 1609.4 control channel (US 178; RSU Spec v4.1 Req_587) | WSAs on the safety channel 172; under `net.channel_access: alternating` in its CCH interval (Req_588, SC-M4) | The channel plan has one safety and one service channel; a third channel needs the second radio to switch between CCH and SCH in the MAC. Only under the non-default `net.rsu_ip.air: sch`: a signed WSA (about 250 B, 0.4 ms at 6 Mb/s) at 10 Hz is about 0.4 % busy time on 172 per relaying unit in range, and a one-radio J2945/1 unit hears it on 172 where it could not hear 178 |

## 7. Status

Dated notes go under each id as its wave lands.

### SC-M3

- 2026-10-30 (wave B): merged `worktree-wf_0343ab94-a4a-1` (13ae0463) and finished it.
  Review against IEEE 1609.2.1 §6.3.5.14 as the repo cites it (03 §3 l.111, [SMP] §4.4
  If-Modified-Since, [ASN-2.1] l.337) found one defect in the wip: the RA's answer was
  computed, logged and recorded as the vehicle's `If-Modified-Since` before the request had
  reached the RA, so a lost uplink still counted a signed status and a lost downlink left
  the vehicle answered 412 for a status it never received. Now the RA answers only a
  request that arrived (and logs the SPDU at its arrival plus service time), and the
  vehicle keeps the status only when the answer reaches it
  (`Phase2::info_status_received`). The wip's integration test ran the conformance
  scenario's first 420 s, where no list changes (0 downloads in both modes), so it could
  not fail; it now runs `credential-lifecycle.yaml`: conditional 270 x 412 + 206
  downloads; info-status 476 polls, 207 signed statuses, 269 x 412, 206 downloads, DL
  bytes +72 kB. Off by default (`conditional`). Kept as modelled: a CTL-only change fetches
  the composite (it carries the CTL), not the chain file (6.3.5.7); no source says
  deployed devices poll infoStatus first, so the default stays.

### SC-M4

- 2026-10-30 (wave B): the WSA `MessageCodec` arm, `codec/uper/ieee1609-3-wsa`
  (`v2xw_msg::wsa_codec::Ieee1609WsaCodec`): claims `MsgType::Wsa`, validates a payload by
  decode and canonical re-encode as the J2735 codecs do; registered in `size_model::codecs()`;
  the evidence row moves from "not encoded" to "byte-exact (oracle-validated)" (asn1tools,
  `tests/wsa_oracle.rs`). The 10 Hz rate: the engine's `net.rsu_ip.wsa_interval_ms`
  already defaulted to 100 ms under `sch` (`foundations::a_vehicle_uses_a_roadside_unit_only_after_verifying_its_wsa`:
  150-210 WSAs in 20 s); `v2xw-node` `RsuConfig::wsa_period` was still 5 s (`RepeatRate` 1)
  and is now 100 ms (`RepeatRate` 50, RSU Spec v4.1 App. D). The engine builds no
  `RsuRuntime`, so no golden moves. WSAs on the control channel: under IEEE 1609.4
  alternating access a two-radio provider's WSA (the RSU default) was not held by the MAC's
  gate, so 52 of 199 in 20 s went out outside the CCH interval (in the SCH interval or a
  guard) while one-radio users listened elsewhere; it now waits at the hand-off for the CCH
  interval (Req_588, time slot 0): 199 WSAs, 0 outside it. The separate 178 channel is kept as modelled (§6). Left to wave E:
  the P19.5 `live-rsu.yaml` rows and the `sch` default (release city runs).
