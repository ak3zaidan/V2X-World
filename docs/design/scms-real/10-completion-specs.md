# SCMS completion specs: waves C and D (2026-10-30)

Build specs for wave C (the real-pipeline replacements, `09-completion-plan.md` §3 SC-P1 to
SC-P12) and wave D (the §4 rule gaps and the 2026-10-29 UI engine requests). They refresh
`rule-gap-specs-2026-10-27.md` against the code at `5c5dfc4e`. Every file:line below was
re-grepped on that commit. Waves A and B land first and move lines, so **every agent
re-greps its anchors before editing** and records any anchor that has moved in its commit.

Doc-only track: nothing here was built or run.

## 0. Rules for every piece

- **Shared rules:** `docs/design/frameworks/wp16-24-build-plans.md` §0. In short:
  - three agents at most, and one heavy cargo command at a time through `heavy.sh`;
  - a piece needs at most one crate test run and one engine test file;
  - draws use keyed hashes, never a new position in a shared stream;
  - detectors read only what the unit verified;
  - every new setting has a schema field and its serde default, its bounds in validate, a
    label and choices in options, a unit in publish, a guide row, and a line in
    `docs/guides/configuration-reference.md`.
- **Settings rule (memory "settings need menus").** A setting is a select with labelled
  values, or a number with its unit, range and step. It is never a bare text box. Each
  setting below gives its menu values, its label and a one-line description.
- **Checks that can fail.** Every accept test is shown red under its named fault. Apply the
  fault, run, revert it, and `touch` the reverted file so cargo rebuilds. List the fault in
  the commit as `fault: <what> -> <test> red`.
- **Defaults.** An SC-P item replaces a stand-in with the real pipeline, so the new path is
  the default. The table in 09 §3 is that decision. The old path survives only where it
  names a research mode (`modelled_reports`, the `speed-difference` reading). Settings that
  are new behaviour rather than a replacement are off by default (SC-P12, the G-10 ballot,
  G-11).
- **Goldens:**
  - **G-SCMS** is `crates/v2xw-engine/tests/regression/*.json`. On `5c5dfc4e` the record
    and frame digests are:
    - `i-s1-phase2-60s`: `a24806dc4c43…` and `648b5200541c…`;
    - `phase2-path`: `8c8c8e56f676…` and `736a1216d48e…`;
    - `ttc-triggered-brake`: `5047d6fccac9…` and `ba01d4823379…`.

    Waves A and B may re-bless them first, so take the "old" digest from your own base.
    A replacement may move G-SCMS. It does so in its own commit, which states the cause
    and both digests (re-bless with `V2XW_BLESS_SCMS_DIGESTS=1`).
  - **G-ML** (the corpus digest and the `detnorm_*` columns) and **G-LEG** must not move
    in waves C and D. Each spec says how it keeps them still.
- **Records:**
  - When an item closes, add a dated note under its id in `09-completion-plan.md` §7.
  - Update the expectation or audit rows it closes, with the evidence.
  - Rows that need a release city run are written as `verdict: open` with the predicted
    direction, and run in wave E.
- **A piece** is about 3 hours: one design step, one or two commits, and at most two heavy
  commands.

Source keys are as in `03-protocols-and-operations.md` §0 and `rule-gap-specs-2026-10-27.md`:
[SMP], [SM-MBR], [SM-PROV], [CAMP122], [BRECHT], [PRIMER], [CVP-P2], Elector Policy v1.0,
and the cached ASN.1 in `tools/oracle/asn1/`.

---

## 1. Wave C: real-pipeline replacements

### Wave C dependencies on waves A and B

| Item | Needs merged first | Why |
|---|---|---|
| SC-P4 | SC-D3 | The published list must actually fill. Today the CRL Generator and store sit at 0 for 46 ks |
| SC-P6 | SC-D5, SC-P7, SC-M1 | No honest reports from GNSS drift; BrakeAcceleration exists; the MA presets are settled |
| SC-P5 | SC-M1 | Per-provider MA policy decides what a receiving MA does with a re-verified report |
| SC-P12 | SC-D4, SC-M2 | Outages lose answers without the backend retry; availability sits on SC-M2's per-site capacity |
| SC-P8, SC-P9 | SC-D12 | ECA and ACA certificate counts must be right before components are revoked |
| SC-P1, SC-P2, SC-P3 | SC-D13 | Report-store order and the late link stamps confuse the new path's evidence |

### SC-P1: the MA learns the subject from the report, and the ACA from its own table

**Today.**
- The ACA side is already real. `ScmsState::handle_revoke` answers `PcaLookupRequest` from
  `self.pca.issued.get(&(i, lv))` (`v2xw-proto/src/scms/run/revoke.rs:205-216`). It routes
  to the issuing ACA by `pca.issued_by` (`revoke.rs:178-185`).
- The engine bypasses that table. `Phase2::resolve_subject` (`v2xw-engine/src/phase2.rs:4901`)
  maps the report's digest to `(node, i, lv)` through `by_digest` (`phase2.rs:2171`, filled
  by `note_digest`, `phase2.rs:4315-4327`) and the engine's own `creds`.
- It is called on the decision path four times:
  - `report_at_proxy` (`phase2.rs:5609`) drops a report whose digest is not in the map
    before any MA sees it;
  - `ingest_arrivals` (`phase2.rs:6871`) routes sharing by `provider_of_device(subject)`
    (`phase2.rs:6865`);
  - the decision de-duplication (`phase2.rs:7005`, SC-P3);
  - `start_cases_of` (`phase2.rs:7303`).
- The `(i, lv)` that reaches the MA travels in clear in `ReportSubmission.subject_i/lv`. The
  reporting device fills it (`run/report.rs:172-179`) beside the encrypted bytes. The MA
  then opens the report (`run/report.rs:571-626`), but never reads the subject from what it
  opened.

**Design.**
- [SM-MBR] §3 and `scms/mbr.rs:16-17`: the report carries `suspect_bsm.signingCertificate`.
  The MA decrypts it, verifies the evidence, and reads `toBeSigned.id.linkageData`
  (`iCert`, `linkage-value`) and the issuer's HashedId8 from that certificate.
  [CAMP122] §5 and [BRECHT] §VI-D: it sends `(i, lv)` to the ACA its issuer names, and only
  that ACA's issuance table resolves it.
- **Proto (P1.1).**
  - `handle_report`'s MA arm takes the subject `(i, lv, issuer id8)` from
    `opened.report.evidence.suspect().cert`, and stores it on the MA's report row.
  - `ReportSubmission.subject_i/lv` stay only as the modelled path's carrier: a report with
    `secured: None`, under `modelled_reports: true`.
  - Add `MaReport { subject_i, subject_lv, issuer: [u8; 8] }` (or the same fields on the
    existing row). The routing key is the issuer.
- **Engine (P1.2).**
  - Rename `resolve_subject` to `truth_subject` and keep it for record and truth code
    only: `log_fate`, `log_received`, `note_report_truth`, the attacker tallies.
  - `report_at_proxy` stops dropping unmapped digests. The report goes to the RA and MA
    whatever the engine's map says.
  - `ingest_arrivals` routes a report to the provider whose ACA certificate has the
    issuer id8 (each run's `pki` lists its ACA certificates). It no longer uses
    `provider_of_device`.
  - `start_cases_of` finds the MA's report by the MA-held `(i, lv)` (as at
    `phase2.rs:7307-7313`, which already uses `subject_lv`).
  - `InFlightReport.subject` becomes `Option<NodeId>`, filled for truth only.
- **What stays modelled:** a report on the modelled path (`modelled_reports: true`) has no
  bytes, so the engine keeps reading the subject from `by_digest` there. That path is
  labelled in the report as `subject_from: "engine-map (modelled report)"`.

**Accept tests.**
1. `v2xw-engine/tests/report_signing.rs`, new
   `a_report_is_decided_without_the_engine_map`. Use a test hook,
   `Phase2::forget_digest_map_for_test()` (cfg(test) or `#[doc(hidden)]`), which clears
   `by_digest` after the first report is filed. The MA still opens a case and revokes the
   attacker, and `cases_opened` equals the run without the hook.
   - **Fault:** restore the `resolve_subject` early return in `report_at_proxy`.
     `cases_opened` drops to 0 and the test goes red.
2. `v2xw-proto/tests/resolution.rs`, new `the_ma_reads_the_subject_from_the_suspect_certificate`.
   Submit a secured report whose clear `ReportSubmission.subject_lv` is deliberately wrong.
   The case resolves the true device.
   - **Fault:** read `r.subject_lv` instead of the opened certificate. The lookup finds
     nothing and the test goes red.
3. `v2xw-engine/tests/scms_providers.rs`, new
   `a_report_routes_by_the_issuer_named_in_its_certificate`. A provider-b certificate
   reported through provider-a's RA reaches provider-b's MA.
   - **Fault:** route by `provider_of_device`, after swapping two devices' provider entries
     in the truth map with a test hook. The report is misrouted and the test goes red.

**Settings:** none. `security.scms.lifecycle.modelled_reports` already selects the modelled
path.

**Goldens:**
- G-SCMS may move. The decision timing is unchanged, but reports whose digests were absent
  from the map now reach the MA, and records gain the `subject_from` field. The commit
  states the cause and both digests.
- G-ML does not move: the corpus reads only detector outputs.

**Records:** close audit-mbpipe rows that cite `resolve_subject`. Add a live-scms row
`LS.MA.subject-from-cert` (predicted: every real report has
`subject_from = "suspect-certificate"`).

**Size:** 2 pieces. P1.1 is proto (report.rs, run/mod.rs). P1.2 is the engine (phase2.rs).

### SC-P2: every case runs the PCA lookup and the LA linking

**Today.**
- The engine opens every case with `request_revocation` (`phase2.rs:7333`). That calls
  `open_case(..., direct: true, ...)` (`run/revoke.rs:69-86`).
- In a `direct` case the PCA's answer goes straight to the RA blocklist
  (`run/revoke.rs:231-246`). The LAs are asked only for seeds afterwards. The
  "same device" exchange runs only through `investigate` or `request_link`
  (`revoke.rs:17-36`, `88-108`), and the engine never calls either.

**Design.**
- [CAMP122] §5 (the MA's linkage flow), Whyte et al. VNC 2013 §V, and [BRECHT] §VI-C/D:
  1. The MA sends the certificate's `(i, lv)` to the issuing PCA, which returns the
     encrypted pre-linkage values (LCI1, LCI2) and the request hash.
  2. The MA sends LCI_k to LA_k, and each LA returns a **linkage chain identifier** for its
     chain.
  3. The MA uses that pair to link the case to earlier reports and earlier decisions.
  4. Only then does it ask the RA to blocklist and the LAs to release seeds.
- New messages (modelled, 09 §6: the LCI encodings are unpublished, sizes derived and
  charged):
  - `ScmsMsg::ChainIdRequest { la, lci }` and `ChainIdResponse { la, chain: [u8; 16] }`;
  - sizes in `ScmsSizes` beside `same_device_request` (`scms/msg.rs:1018-1035`).
- The LA computes `chain` as a keyed hash under the LA's own secret over its chain record
  (`st.owner` in `revoke.rs:263-270`). It is stable per device chain, and two LAs never
  produce comparable values.
- `open_case` loses its `direct` argument. `Case.direct` stays only for the
  `investigate`/`request_link` research API, which keeps the bit-answer exchange.
- **Sizing, ASSUMPTION:** the response is the 16-byte identifier plus an AES-CCM envelope,
  like `same_device_response`.

**Accept tests.**
1. `v2xw-proto/tests/revocation_path.rs`, new
   `every_case_asks_both_las_for_the_chain_before_the_blocklist`. The flow's step list
   contains `chain-id-request` ×2 and `chain-id-response` ×2, all before
   `blocklist-request`.
   - **Fault:** keep the `direct` shortcut. No chain steps are logged and the test goes
     red.
2. The existing `ma_cases.rs` suite stays green (no weakened asserts).

**Settings:** none. This is the 09 §3 replacement.

**Goldens:** G-SCMS moves in every case that revokes: two more backend steps, and a later
blocklist by two link delays. Own commit, with both digests.

**Size:** 1 piece, done with SC-P3 as "P2/P3.1".

### SC-P3: de-duplicate decisions by linkage chain, not by device

**Today.** `phase2.rs:7005-7010` de-duplicates on `ma_decided_devices: BTreeSet<NodeId>`
(`phase2.rs:2327`), keyed by the engine's device identity from `resolve_subject`. The MA
cannot know that identity.

**Design.**
- The MA keeps `decided_chains: BTreeSet<([u8;16],[u8;16])>` per provider MA, in
  `run/mod.rs`'s MA state.
- When SC-P2's chain identifiers arrive for a case whose pair is already decided, the case
  ends with the new outcome `linked`. There is no blocklist request and no seed release.
- The engine reads the outcome from the case and counts `decisions_linked` then, not at
  decision time. The engine-side `ma_decided_devices` check at `phase2.rs:7005` is removed.
- One visible effect: a duplicate costs one case (a PCA lookup and two LA calls) where
  today it costs nothing. That is the real cost of linking, so it is charged.
- The `max_open_cases` back-pressure (`start_cases_of`, `phase2.rs:7296-7341`) now sees
  those cases.
- Source: [CAMP122] §5, and the comment at `phase2.rs:6998-7004`, which describes exactly
  this.

**Accept tests.**
1. `v2xw-engine/tests/report_signing.rs`, new
   `a_second_pseudonym_of_a_decided_device_links_without_a_second_revocation`. One
   attacker changes pseudonym after its first decision. The results:
   - `ma_revoke_decisions` = 2;
   - `decisions_linked` = 1;
   - `crl_entries` for the device = 1;
   - one `linked` case outcome.
   - **Fault:** drop the `decided_chains` check. Two CRL entries appear and the test goes
     red.
2. The same test with the hook from SC-P1 test 1 (engine map cleared): unchanged results.

**Settings:** none.

**Goldens:** the same G-SCMS commit as SC-P2 if both land together. Otherwise a separate
commit.

**Records:** `report.decisions_linked` keeps its name. Its catalogue text changes to "cases
the MA closed because the chain was already decided".

**Size:** 1 piece ("P2/P3.2"), after P2/P3.1.

### SC-P4: the MA checks the published CRL

**Today.** `start_cases_of` builds `covered` from `self.runs[k].state.crlg.entries`
(`phase2.rs:7317-7325`). That is the CRL Generator's working set, including entries not yet
published.

**Design.**
- Build `covered` from what the provider has published. That is
  `self.runs[k].state.crl_store.entries` (set at publication, `run/crl.rs:747`), or the
  newest `CrlVersion` of `crl_feeds[k]` (`visible_versions`, `phase2.rs:7346`).
- A decision made after another decision about the same chain, but before that one is
  published, is caught by SC-P3's chain check, not by the list.
- Source: [BRECHT] §VI-D (the MA consults the CRL), and 09 §3.

**Accept test.** `v2xw-engine/tests/report_signing.rs`, new
`the_ma_counts_a_decision_covered_only_after_publication`. Under the real calendar's CRL
SLA, with a decision whose entry is still pending:
- a report about the same device before publication gives `decisions_already_covered` = 0
  and a `linked` case;
- after publication it gives `decisions_already_covered` = 1.
- **Fault:** read `crlg.entries`. Covered is counted before publication and the test goes
  red.

**Settings:** none. **Goldens:** G-SCMS moves only where a case is opened between a
decision and its publication. Say so in the commit and list the cases.

**Size:** half a piece. It is paired with SC-P5's tail or done alone after SC-D3 merges.

### SC-P5: the receiving MA re-verifies a shared report

**Today.**
- `ingest_arrivals` (`phase2.rs:6854-6930`) forwards a report about another provider's
  certificate.
- `shared_report_at_ma` (`run/mod.rs:1572-1625`) injects a `ReportSubmission` with
  `secured: None`, and the forwarding MA's evidence reading travels as
  `pending_evidence` (`run/mod.rs:1536-1546`).
- The doc comment says it plainly: "this MA does not re-verify evidence the forwarding MA
  verified".

**Design.**
- The forwarding MA holds the opened inner SPDU, the reporter-signed `SignedData`, in
  `ma.opened_reports`. It re-encrypts that SPDU to the receiving provider's MA encryption
  key, with the same `mbr::secure_report` path minus the signing step.
- A new `mbr::reencrypt_for_ma(inner_signed, peer_ma_cert, peer_ma_enc, rng)` builds the
  encrypted `Ieee1609Dot2Data`.
- `shared_report_at_ma` takes `secured: Option<SecuredReport>` (header `None`: no RA
  `Download-Req` on the MA-to-MA leg).
- The receiving MA's `handle_report` arm (`run/report.rs:571-626`) then decrypts, verifies
  the reporter signature and the evidence, and refuses what fails.
  - Its `ma_trust` already covers the peers (`run/mod.rs:1056`).
  - The cost is charged with `decrypt` + `verify`.
- `pending_evidence` remains only for modelled reports.
- **Modelled, 09 §6:** the MA-to-MA envelope is unpublished. This is a real 1609.2
  encryption to the peer MA, labelled ASSUMPTION in `constants.md`. The bytes crossing the
  link are the real ciphertext length.
- Sources: 09 §3 SC-P5; [SM-MBR] §3, where the MA verifies evidence it acts on.

**Accept tests.**
1. `v2xw-proto/tests/provider_runs.rs`, new `a_shared_report_is_reverified_by_the_receiving_ma`.
   Corrupt one byte of the evidence BSM's signature before sharing. The receiving MA
   records a `SecuredMisbehaviorReport` rejection, and the case does not open.
   - **Fault:** keep `secured: None` on the share. The corrupt report is acted on and the
     test goes red.
2. `scms_providers.rs`: on the same seed, `shared.received` is unchanged and the receiving
   MA's `opened_reports` grows by `shared.received`.

**Settings:** none.

**Goldens:** G-SCMS moves only for multi-provider cases with sharing on (the US default).
The bytes on the MA-to-MA link change. Own commit.

**Size:** 1 piece.

### SC-P6: SM-MBR is the default local detector under a US deployment

**Today.**
- With `detection.local` empty, every receiver runs `Legacy12`: `local_suite`
  (`phase2.rs:4400-4409`) picks `us_profile` only when the scenario names
  `detect/us-sm-mbr` (`phase2.rs:2428-2431`).
- The option's label (`scenario/options.rs:652-655`) says "no streak gate".

**Design.**
- When `detection.local` is empty, the deployment `is_us()` (`scms/deployment.rs:516`) and
  the credential protocol is the SCMS (not ETSI), set `us_profile =
  Some(UsMbrParams::default())`.
- Every other case keeps `Legacy12`: a non-US preset, the CAMP reference, or an explicit
  list.
- The run report and model card name the suite actually used (a new `report.local_suite`
  string, or the existing card field).
- Add the `BrakeAcceleration` arm to `us_subtype` (`phase2.rs:9499-9520`), mapping SC-P7's
  check to `mbr::Subtype::BrakeAcceleration` (`scms/mbr.rs:279`).
- Sources: [SM-MBR] §2.3 (US OBUs detect and file only these subtypes), and 09 §3.

**Accept tests.**
1. `v2xw-engine/tests/scms_deployment.rs`, new `a_us_deployment_defaults_to_the_sm_mbr_profile`:
   - `us-production-2025` with no `detection.local` gives `local_suite = detect/us-sm-mbr`;
   - `camp-reference` gives `detect/legacy-12`.
   - **Fault:** drop the `is_us()` branch. The test goes red.
2. `a_us_device_files_brake_acceleration` (same file, with the SC-P7 test vehicle): an
   SM-MBR report with subtype 3 reaches the MA.
   - **Fault:** remove the new `us_subtype` arm. The report is `not_filed` and the test
     goes red.

**Settings:**
- No new key.
- The text of `detection.local` changes: the guide row and the configuration reference
  say "empty: `detect/us-sm-mbr` under a US deployment, else `detect/legacy-12`".
- The option description of `detect/us-sm-mbr` adds "(the default under a US
  deployment)".
- Those text edits are in scenario files, so D1 makes them in wave D (§3, D1 round 1).

**Goldens:**
- G-SCMS moves in every case with a US deployment and an empty `detection.local`. Own
  commit naming 09 §3 SC-P6, with both digests.
- **G-ML must not move.** Before the flip, find the corpus scenario (grep `corpus` in
  `crates/v2xw-threat/tests` and `crates/v2xw-record/tests`). If it runs a US deployment
  with an empty `detection.local`, pin `detection.local: [{id: detect/legacy-12}]` in it.
  Show the corpus digest unchanged.
- G-LEG: pin the same way.

**Records:** add open rows for wave E in `scms-final-bench.yaml`:
- `FS.honest-reports.smmbr-default`, predicted ≤ the Legacy12 baseline after SC-D5;
- `FS.attackers-revoked.smmbr-default`, predicted no fewer attackers revoked than the
  baseline, or a documented reason per attack class.

**Size:** 1 piece. Last in C1's queue, after the mid-wave merge brings in SC-P7.

### SC-P7: BrakeAcceleration and MaxAcceleration read the BSM's own fields

**Today.**
- `BsmExtras` (`v2xw-core/src/bsm_fields.rs:9-19`) carries `sec_mark_ms`,
  `accel_long_mps2`, `speed_mps` and `heading_rad`. It has no brake status.
- The sender fills `accel_long_mps2` from its bus (`v2xw-node/src/runtime.rs:2417-2421`).
- `UsSmMbr` stores `accel` in `Last` (`v2xw-threat/src/us_mbr.rs:440`). But MaxAcceleration
  still tests `|v₂ − v₁| / Δt` (`us_mbr.rs:380-383`).
- BrakeAcceleration is "not modelled" (`us_mbr.rs:12`, `33-34`).
- The BSM's `wheelBrakes` is encoded from `bus.brake_applied()`
  (`v2xw-node/src/vehicle.rs:247-250`).

**Design.**
- [SM-MBR] §2.3, as `us_mbr.rs:11-12` tabulates it:
  - MaxAcceleration (2): the BSM's acceleration is above the threshold, default 10 m/s²;
  - BrakeAcceleration (3): brakes on, acceleration not negative.
- `BsmExtras` gains `brakes_applied: Option<bool>`, set from the encoded
  `BrakeSystemStatus.wheelBrakes`: `unavailable` gives `None`, any wheel bit gives
  `Some(true)`, and no bit gives `Some(false)`. The receiver's copy is what the frame
  carried (`runtime.rs:125`, `174`, `329`), so an attacker that forges the frame forges it.
- MaxAcceleration under the new reading `accel-set` fires when
  `|accel_long_mps2| > max_accel_mps2`. A BSM without `accelSet` falls back to the
  speed-difference test, counted as `accel_unavailable_fallbacks`.
- **BrakeAcceleration** is a new `DetectorId::BrakeAcceleration`, US-only and outside
  `DetectorId::ALL`, like `ImplausibleSpeed` (`detect.rs:146`, `177`, `283`). It therefore
  adds no `detnorm_*` column, and G-ML cannot move. It fires when all of these hold:
  - `brakes_applied == Some(true)`;
  - `accel_long_mps2 > brake_accel_tolerance_mps2`;
  - `claimed_speed ≥ speed_resolution_mps`.

  The speed gate is an ASSUMPTION: a vehicle held at a red light has brakes on and
  `accelSet.long = 0`, which "not negative" would otherwise flag. Before coding, read
  [SM-MBR] §2.3's text for a stated speed condition. If the text has one, use it and drop
  the assumption.
- Model card: add the new params with units and clauses through `param()`
  (`us_mbr.rs:489`), and extend `PARAM_KEYS` (`us_mbr.rs:189`).
- Remove the two stale deviation bullets (`us_mbr.rs:31-34`) once both checks read the
  fields.

**Accept tests.**
1. Unit tests in `us_mbr.rs`:
   - `brake_acceleration_fires_on_brakes_with_positive_acceleration`: brakes on with
     +0.5 m/s² fires;
   - the same function asserts that brakes on with −3 m/s², and brakes on at standstill
     with 0, do not fire.
   - **Fault:** drop the speed gate. Standstill fires and the test goes red.
2. `max_acceleration_reads_accel_set`: `accelSet` 12 m/s² with equal consecutive speeds
   fires under `accel-set`.
   - **Fault:** keep the speed-difference test. It does not fire and the test goes red.
3. `v2xw-node/tests/runtime_behaviour.rs`: a braking vehicle's transmission carries
   `brakes_applied = Some(true)`.
   - **Fault:** leave the field at `None`. The test goes red.

**Settings** (`detection.local[].params` of `detect/us-sm-mbr`, published by the model
card):

| Key | Values (label) | Default | Description |
|---|---|---|---|
| `max_accel_reading` | `accel-set` ("Reported acceleration (SM-MBR)"), `speed-difference` ("From consecutive speeds") | `accel-set` | Which acceleration MaxAcceleration judges |
| `brake_acceleration` | on, off ("Brake with positive acceleration") | on | SM-MBR Implausible/BrakeAcceleration |
| `brake_accel_tolerance_mps2` | 0, 0.01, 0.1, 0.5 m/s² (step 0.01, range 0–2) | 0.01 (one J2735 LSB, ASSUMPTION) | How far above zero counts as "not negative" |

**Goldens:** none move. `us-sm-mbr` is not yet the default, and the G-SCMS cases do not name
it. Check this by grepping the three scenarios. If one names it, G-SCMS moves in its own
commit.

**Records:** live-atk rows for RandomSpeed under `us-sm-mbr` will change (the speed
falsification no longer shows in `accelSet`). Add `verdict: open` rows for wave E with that
prediction.

**Size:** 1 piece.

### SC-P8: component CRLs (series 2 and 256)

**Today.**
- The CRL Generator issues hash CRLs only for series 3 and 4 (`run/crl.rs:450-452`, from
  `crlg.hash_entries`, `run/mod.rs:902`).
- `v2xw-sec` encodes hash CRLs for 2, 3, 4 and 256 (`v2xw-sec/src/crl.rs:69-77`,
  test `crl.rs:787-789`).
- Receivers already refuse a certificate, or its issuer, found on a matching
  `(cracaId, crlSeries)` hash list (`v2xw-node/src/rxval.rs:13`, `1045-1052`, `1167-1191`,
  `RxReject::IssuerRevoked`).
- `governance.rs:65` defers this to "the CRL work package". Nothing revokes a component.

**Design.**
- [SMP] Table 1 and IEEE 1609.2 §5.1.3: a hash CRL revokes the certificates that name its
  CRACA and series. Each component's certificate carries its own `crlSeries`
  (`pdu.rs:1055-1058`):
  - S2X profile: ICA-issued components name the ICA on series 2; root-issued ones name the
    root on series 256;
  - ISS profile (`governance.rs:534-538`): every sub-CA names the root on series 256.
- **P8.1, proto:**
  - `ScmsRun::revoke_component(role: &str, at: SimTime)` looks up the component's
    certificate in the governance PKI. It adds a `HashRevocation { id: HashedId10, expiry }`
    under the series and CRACA that certificate names, and publishes at the next
    publication.
  - Extend the series loop at `crl.rs:450-452` to every series with entries, including 2
    and 256. A CRL is issued only when entries exist, so a run with no component revocation
    stays byte-identical.
  - The signer is the CRL signer whose `CrlSsp` lists that series (`pdu.rs:696-740`). Read
    [SMP] Table 1's rows for series 2 and 256 before coding, and label any choice the table
    leaves open.
- **P8.2, proto distribution and effects:**
  - The composite CRL the RA serves carries the new list, and `device_check_crl`
    (`crl.rs:131`) accepts it.
  - A revoked PCA makes every pseudonym it issued fail `IssuerRevoked`.
  - A revoked RA means devices refuse its signed responses.
  - A revoked MA certificate means devices stop encrypting reports to it
    (`mbr::select_ma` skips revoked certificates).
  - The new `crl_component_entries` counter goes per provider.
- **P8.3, engine wiring and keys (D1, wave D round 1):** the scenario keys below; at
  `component_revoked_at_s` the home or named provider calls `revoke_component`.

**Accept tests.**
1. `v2xw-proto/tests/crl_issuance.rs`, new `a_revoked_pca_appears_on_its_component_crl`:
   - S2X gives series 2 with CRACA ICA;
   - ISS gives series 256 with CRACA root;
   - the entry is the PCA certificate's HashedId10.
   - **Fault:** leave the series loop at {3, 4}. No list is issued and the test goes red.
2. `crl_issuance.rs`, new `a_device_refuses_pseudonyms_of_a_revoked_pca_after_install`, via
   `device_check_crl` plus `v2xw-sec` chain validation in the proto test, or the engine
   test in P8.3.
   - **Fault:** sign the component CRL under the wrong CRACA. The device installs nothing,
     accepts the pseudonym, and the test goes red.
3. P8.3, `v2xw-engine/tests/rsu_certs.rs` or a new `component_crl.rs`: after
   `component_revoked: pca` the receivers' `issuer-revoked` count rises above 0, and frames
   signed after the install are refused.
   - **Fault:** do not distribute the series-256 list. The count is 0 and the test goes
     red.

**Settings** (P8.3, `security.scms.lifecycle.*`, D1):

| Key | Values (label) | Default | Description |
|---|---|---|---|
| `component_revoked` | none ("No component revoked"), `ica` ("Intermediate CA"), `pca` ("Pseudonym CA (ACA)"), `ra` ("Registration Authority"), `eca` ("Enrolment CA"), `la1`, `la2` ("Linkage Authority 1/2"), `ma` ("Misbehaviour Authority"), `crlg` ("CRL Generator") | none | The SCMS component whose certificate the provider revokes |
| `component_revoked_provider` | the deployment's provider names | the home provider | Whose component |
| `component_revoked_at_s` | s, range 0 to the run length, step 1 | unset | When the revocation is decided |

**Goldens:** none move (off by default, and no list without entries).

**Size:** 3 pieces: P8.1 and P8.2 by C3 in wave C, P8.3 by D1 in wave D.

### SC-P9: identification certificates for emergency vehicles

**Today.**
- `AppCertKind::ObuIdentification` exists (`scms/appcert.rs:122-154`: 721 h, PSIDs BSM and
  SRM, original butterfly).
- `Options::identification_certs` is on for the US (`scms/deployment.rs:363`).
- The engine preloads only `RsuApplication` (`phase2.rs:3359-3363`).
- Emergency vehicles sign SRMs with pseudonyms. That works because the fleet's pseudonym
  `appPermissions` include SRM for everyone (`phase2.rs:873-890`).

**Design.**
- [CAMP122] Table 1 row 44 and `appcert.rs:4-6`, `81`: privileged vehicles hold OBU
  identification certificates, and SRM preemption (PSID 0x204096,
  `traffic-signal-request`) is signed under them.
- **P9.1, proto (C3):**
  - `preload_app_cert(device, ObuIdentification, now, connected)` and its renewal cycle
    work for an OBU device, today they are exercised only for RSUs
    (`run/appcert.rs:453`, `582`, `634`);
  - the series-3 hash revocation of an identification certificate
    (`revoke_identification(device)`) goes through the CRL Generator's series-3 list
    (`crl.rs:450`).
- **P9.2, node and engine wiring (C2):**
  - `ObuRuntime` gains one identification signer slot, `install_identification`, beside
    `install_credential` (`runtime.rs:813`).
  - The SRM is signed with the slot when it holds a valid certificate (`events.rs` SRM
    path). It is never rotated.
  - `wiring.rs` gets `install_identification_certs`, after `install_rsu_app_certs`
    (`wiring.rs:4521`).
  - The BSM stays on pseudonyms. Whether US emergency vehicles sign BSMs with
    identification certificates in emergency mode is unpublished, so this is labelled
    ASSUMPTION.
- **P9.3, phase2 (D1, wave D):**
  - Emergency-class vehicles under a US deployment with `identification_certs` on are
    preloaded at `provision`.
  - The pseudonym PSID list drops SRM when identification certificates are on
    (`phase2.rs:873-890`).
  - New counters: `ident_certs_preloaded`, `ident_certs_renewed`, `srm_unsigned_no_ident`.

**Accept tests.**
1. P9.1 `v2xw-proto/tests/appcert.rs`, new `an_obu_identification_cycle_preloads_and_renews`.
   - **Fault:** the kind is mapped to `RsuApplication` PSIDs. The PSID assert goes red.
2. P9.2 `v2xw-node/tests/runtime_behaviour.rs`, new `an_srm_is_signed_by_the_identification_slot`.
   - **Fault:** sign with the pseudonym. The signer digest assert goes red.
3. P9.3 `v2xw-engine/tests/emergency_yield.rs`, new
   `a_forged_srm_from_a_normal_vehicle_is_refused`:
   - the emergency vehicle's SRM is granted (`priority.rs`);
   - an ordinary vehicle's SRM is refused `psid-not-permitted`.
   - **Fault:** keep SRM in the pseudonym PSIDs. The forged SRM is granted and the test
     goes red.
4. P9.3: revoking the emergency vehicle's identification certificate stops its grants after
   the CRL install.
   - **Fault:** skip series 3 in the loop. Grants continue and the test goes red.

**Settings:** none new. `identification_certs` already exists in the deployment options. If
it is not user-facing, D1 adds `security.scms.deployment.identification_certs` with the
values on ("Emergency vehicles hold identification certificates", default under the US) and
off ("Emergency vehicles sign with pseudonyms").

**Goldens:**
- G-SCMS moves only in cases with an emergency-class vehicle (check the three scenarios'
  fleet mixes first). The pseudonym `appPermissions` bytes change in every US case once SRM
  leaves the list, so G-SCMS probably moves in all three.
- Own commit, P9.3 by D1, citing 09 §3 SC-P9.

**Size:** 3 pieces: P9.1 C3, P9.2 C2, P9.3 D1.

### SC-P10: no bootstrap stand-in for a vehicle the SCMS did not provision

**Today.**
- Every OBU is built with `bootstrap_credentials` (`wiring.rs:3797`, `4159-4180`: 20
  pseudonyms with no linkage value).
- `run.rs:4246-4276` replaces them only `if !creds.is_empty()`. A vehicle whose pre-run
  provisioning failed, or returned nothing (`phase2.rs:3997-4021`, counted in
  `vehicles_unprovisioned`), keeps signing with the stand-in.
- `phase2.rs:4014-4020` names the cause seen on `credential-lifecycle`: vehicles that join
  a few seconds after an i-period boundary get no certificate. That is a defect of its own.

**Design.**
- **P10.1:** trace why `preload_at(device, aged.valid_from.min(now), start,
  pool_periods, jmax)` (`phase2.rs:3990-3996`) yields no certificate for a joiner just
  past a boundary. Suspects: `start` versus `period_at(now)`, and the
  `aged.valid_from.min(now)` window. Find the cause to file:line. A real device on the road
  holds its current period's pseudonyms, so fix the window.
- **P10.2:** under an SCMS run, `run.rs:4257` always installs. An empty `creds` empties the
  store through `install_provisioned`, which builds a fresh `CertStore`
  (`wiring.rs:4496-4514`). The vehicle then cannot sign: `tx_uncredentialed` counts its
  due messages (`runtime.rs:828`). `vehicles_unprovisioned` is renamed or documented as
  "vehicles unable to sign". The hosted path (`hosted.rs:432`) does the same.
- Runs without an SCMS keep the bootstrap pool: it is their only credential source.
- Source: 09 §3. 1609.2: an unprovisioned device has no signing credential.

**Accept tests.**
1. P10.1 `v2xw-engine/tests/credential_handoff.rs`, new
   `a_vehicle_joining_after_a_period_boundary_holds_that_periods_pseudonyms` (60 s
   i-periods, a join at boundary + 2 s).
   - **Fault:** revert the window fix. The vehicle has no certificate and the test goes
     red.
2. P10.2 same file, new `a_vehicle_whose_provisioning_fails_cannot_sign`. Force the failure
   with `actors.backend.failures: [{node: pca, mode: offline, from_s: 0}]` on the vehicle's
   provider:
   - no frame on the air carries a bootstrap digest (`pseudo_signer`);
   - `tx_uncredentialed` > 0.
   - **Fault:** restore `if !creds.is_empty()`. Stand-in frames appear and the test goes
     red.

**Settings:** none.

**Goldens:**
- P10.1 moves G-SCMS only where joiners hit a boundary.
- P10.2 moves G-SCMS only where `vehicles_unprovisioned` > 0 after P10.1. Record each case's
  `vehicles_unprovisioned` before and after.
- Separate commits, each with both digests.

**Size:** 1.5 pieces (P10.1 one, P10.2 half).

### SC-P11: country and state regions checked against the location

**Today.**
- `location_in` (`v2xw-node/src/rxval.rs:660-679`) handles circular and rectangular
  regions. `identifiedRegion` falls to `_ => None`, which counts `region_unchecked`
  (`rxval.rs:1139-1140`).
- The receiver reads only the end-entity certificate's own region.
- The US default's provider-a uses the ISS profile, which puts `identifiedRegion {124, 484,
  840}` on the ICA and every ICA-issued component (`governance.rs:520-538`, `1572`).

**Design.**
- **Inheritance.** IEEE 1609.2 `ToBeSignedCertificate.region` (cached,
  `tools/oracle/asn1/ieee1609.2-2022/Ieee1609Dot2.asn` l.978-984): "If it is omitted … the
  certificate has the same validity region as the certificate that issued it", and it is
  worldwide under a self-signed issuer. Under ISS, an end-entity certificate with no region
  inherits {124, 484, 840}.
  - Apply that: the effective region is the first region found walking up the validated
    path (`CaPath`).
  - Cache it per end-entity certificate in `EeCache`.
  - Chain consistency is an EE region inside its issuer's (1609.2 §5.1.3). Check it once
    per path, refusing with `OutsideRegion`.
- **Countries:**
  - `countryOnly(n)` for n ∈ {840, 124, 484} is checked against Natural Earth admin-0
    polygons, 1:50m (public domain, per 09 §3). An unrecognised country counts as
    `region_unchecked`, as the ASN.1 permits (`Ieee1609Dot2BaseTypes.asn` l.462-490).
  - A SEQUENCE OF identified regions is their union.
- **States.** `countryAndRegions(840, regions)` uses the 2010 FIPS state codes
  (`Ieee1609Dot2BaseTypes.asn` l.497-501). The polygons come from US Census cartographic
  boundary states (`cb_*_us_state_20m`, public domain, `STATEFP`). Other countries' regions
  are undefined in the standard, so they count as unchecked.
- **Data:**
  - `tools/regions/build.py` (run with `python3 -I` on downloads kept in their own
    directory) simplifies the polygons to about 2,000 vertices per country and 300 per
    state;
  - it writes `crates/v2xw-node/data/regions.bin` as i32 microdegree rings, plus a
    0.25° grid of inside, outside and border cells, so only border cells run
    point-in-polygon;
  - the file is compiled in with `include_bytes!` (`v2xw-node` cannot read the world,
    `firewall.rs`);
  - record the sha256 and the source URLs in `vectors-provenance.md`.
- **Cost:** a grid lookup per verified message, and point-in-polygon only in border cells.
  `per-message` cost is not charged as a primitive (no published figure).
- **Determinism:** integer microdegree rings, and a crossing-number test in integer
  arithmetic.

**Accept tests.**
1. `v2xw-node/tests/rx_validation.rs`, new `a_us_country_certificate_is_refused_in_toronto`.
   A `countryOnly(840)` certificate with a generation location of (43.65, −79.38) is
   refused `outside-region`. The same certificate in Manhattan is accepted.
   - **Fault:** the arm returns `None`. `region_unchecked` rises instead and the test goes
     red.
2. `an_iss_pseudonym_inherits_its_icas_region`: a region-less end-entity certificate under
   an ICA with {124, 484, 840} is accepted in Toronto and refused in Havana.
   - **Fault:** ignore inheritance. Havana is accepted and the test goes red.
3. `a_fips_state_region_is_checked`: `countryAndRegions(840, [36])` (New York) is accepted
   in Manhattan and refused in Newark, NJ (34).
   - **Fault:** swap to FIPS 10-4 codes. The test goes red.
4. A grid self-test in `regions.rs`: on 10,000 deterministic points, the grid result equals
   brute-force point-in-polygon.
   - **Fault:** mark one border cell "inside". The equality assert goes red.

**Settings:** none. The check is the standard's. Natural Earth's resolution is fixed at
build time.

**Goldens:**
- G-SCMS may move: under ISS, `region_unchecked` falls to 0 for provider-a devices, and the
  stats are in records.
- Manhattan receptions stay accepted, so there are no new refusals. Own commit, with both
  digests and the stat delta.

**Records:** close veh R10 in the register (wave E rewrite) with these tests. Add
`LS.region-unchecked` = 0 to live-scms.

**Size:** 2 pieces. P11.1 is the data, the builder and the grid. P11.2 is the receiver,
inheritance and tests.

### SC-P12: availability and stochastic service times (off by default)

**Today.**
- `ServiceModelSpec::availability: Option<()>` is a documented hook, `None` everywhere
  (`v2xw-proto/src/service.rs:8-10`, `56-59`, `69`).
- `ServiceQueue::admit` (`service.rs:130-155`) uses deterministic service times.
- Scheduled outages exist only as scenario windows (`actors.backend.failures[]`,
  `scenario/schema.rs:4239-4259`, applied through `v2xw_proto::fault`).

**Design.**
- **Availability:** a two-state alternating renewal process per backend entity
  (06-node-models §4). Up times are exponential with mean MTBF, and down times exponential
  with mean MTTR.
  - The k-th interval of entity e is drawn as
    `-mean·ln(1 − keyed_uniform(seed, e, k, kind))`, with a keyed hash per §0 rule 5. It
    takes no stream position.
  - A down interval behaves exactly like a `failures[]` window in mode `offline`: it goes
    through `v2xw_proto::fault`, so SC-D4's retry handles it.
  - Counted per entity: `outages`, `downtime_s`.
- **Service time:** a log-normal multiplier with median 1 and coefficient of variation `cv`
  on `work + fixed_overhead` in `admit`. It is keyed by `(node, served)`.
  `σ = sqrt(ln(1 + cv²))` and `μ = 0`, so the median is the deterministic time. Integer
  nanoseconds after rounding.
- Every parameter is labelled ASSUMPTION in `constants.md`: 06-node-models §4 leaves them
  `todo-calibrate`, and 09 §6 sends them to the sensitivity sweeps.
- **P12.1, proto (C3):** `AvailabilityModel { mtbf, mttr }` replaces the `Option<()>`. The
  schedule generator lives in `kernel.rs`, where each hosted queue gets its windows.
- **P12.2, proto (C3):** the log-normal multiplier in `admit`, with `ServiceTimeModel
  { Deterministic, LogNormal { cv_milli } }` in `ServiceModelSpec`. Determinism and
  distribution tests.
- **P12.3, engine (D1, wave D):** the scenario keys, the mapping in `crypto_config.rs`, and
  the RunReport leaves.

**Accept tests.**
1. `v2xw-proto/tests/availability.rs` (new):
   - `outages_follow_the_mtbf_and_mttr`: over 10⁴ simulated hours the down share is within
     ±10 % of MTTR/(MTBF+MTTR);
   - `the_same_seed_gives_the_same_outages` (byte-identical).
   - **Fault:** draw from a shared RNG stream. The second run's schedule differs after a
     reorder and the determinism test goes red.
2. `service_times_are_lognormal_with_median_one`: over 10⁵ admits, the median multiplier is
   within 1 % of 1 and the sample cv within 5 % of the setting.
   - **Fault:** use cv as σ. The cv check goes red.
3. P12.3, `v2xw-engine/tests/backend_entities.rs`: with `availability: two-state` on the
   RA, `ra.outages` > 0 and every outage-time request completes after its retry (SC-D4).
   - **Fault:** the model is not applied. `outages` is 0 and the test goes red.

**Settings** (P12.3, `actors.backend.availability` and `actors.backend.service_time`,
applying to every entity or to `entities.<role>`):

| Key | Values (label) | Default | Description |
|---|---|---|---|
| `availability.model` | `always-up` ("Always up"), `two-state` ("Random outages (two-state)") | `always-up` | Whether backend servers fail at random |
| `availability.mtbf_h` | 24, 168, 720, 8,760 h (range 1–87,600, step 1) | 720 | Mean time between failures |
| `availability.mttr_min` | 5, 30, 60, 240 min (range 1–10,080, step 1) | 30 | Mean time to repair |
| `service_time.model` | `deterministic` ("Fixed (cost table)"), `log-normal` ("Variable (log-normal)") | `deterministic` | How service times vary |
| `service_time.cv` | 0.1, 0.25, 0.5, 1.0 (range 0–3, step 0.05) | 0.25 | Spread of service times (coefficient of variation) |

**Goldens:** none move (off by default, and the deterministic path is unchanged).

**Records:** sensitivity rows in `scms-knobs.yaml`, for example:
`{setting: availability.model, change: two-state, metric: report.revocation_latency_s,
expect: up}` and `{service_time.cv, 0 → 1, report.topup_latency_p99_s, up}`.

**Size:** 3 pieces: P12.1 and P12.2 by C3, P12.3 by D1.

---

## 2. Wave D: rule gaps, refreshed from `rule-gap-specs-2026-10-27.md`

### 2.1 What changed since 2026-10-27 (re-grepped on `5c5dfc4e`)

- **Moved anchors:**
  - `CRL_NOT_MODIFIED_BYTES`: `phase2.rs:420` is now `phase2.rs:523`;
  - `issue_secured_crls`/`next_index`: `crl.rs:309/330` is now `crl.rs:339/360`;
  - `LaSsp::new`: `pdu.rs:816/826` is now `pdu.rs:828/838`;
  - `runtime.rs:1168` is now `runtime.rs:1189`, and `vru.rs:1501` is now `vru.rs:1503`;
  - `governance_due`: `phase2.rs:4935` is now `phase2.rs:5306`.
  - Unchanged: `decide` `governance.rs:2256`, `build_ctl` `governance.rs:1985`,
    `remove_root` `governance.rs:2320`, `replace_elector` `governance.rs:2414`,
    `test_ballot` `governance.rs:2466`, `next_sequence` `governance.rs:2247`,
    `stores.rs:143/168/296/348`, `ServiceQueue::admit` `service.rs:130`, `msg.rs:298`.
- **Governance keys** live under `security.scms.lifecycle.*`. They are typed in
  `scenario/schema.rs:3885-3900` and parsed by name in `phase2.rs:985-989` (key list
  `phase2.rs:1268-1290`). The `governance.*` names in the 10-27 file are replaced below.
- **Moved to other items:**
  - veh R10 is now SC-P11, with Natural Earth country polygons per 09 §3 and the FIPS
    states from Census. The inheritance rule found in the cached ASN.1 widens its effect to
    every ISS-chain device.
  - veh U5 and M11 are SC-M4/WP-19, and proto A16 is SC-M3, all wave B.
  - proto F9 (TS 103 759 for the EU profile) is not in 09's wave D. It has no effect on US
    runs. It is recorded here as skipped with the reason "EU profile only", to go into
    09 §6 at the wave E rewrite.
- **proto B9:** TLS records and handshakes are already charged
  (`v2xw-radio/src/ip_transport.rs:441`, `461`, `618`; `v2xw-engine/src/backend.rs:264-353`).
  B9 is now only the HTTP request and status lines and their headers.
- **proto D11:** the request-age reading is already enforced, and more strictly. The RA
  refuses a `Download-Req` whose `generationTime` is more than
  `download_request_tolerance` = 5 s off ([CAMP122] SCMS-1203, `provision.rs:2280-2289`,
  `params.rs:246-248`, `593`; tests `pdu_prov_tests.rs:363-396`). The cache reading of
  `download-maxAge` (1 h) equals the 3,600 s poll (`phase2.rs:513`). Only `ra-maxAge`
  (1 week) is left to check (§2.12).
- **gov G-10:** the default becomes **off**, not 365, because new behaviour is off by
  default (§0). Proposed to the owner as the realistic default.

### 2.2 gov C-9: the CTL's `effectiveDate` is the ballot date

- **Today:** `decide` (`governance.rs:2256-2287`) sets
  `effective = clock.time32(at)` where `at = ctl_publication_deadline(kind, decided_at)`
  (`governance.rs:2265-2266`). So `effectiveDate` is the publication deadline.
- **Design:** `effective = clock.time32(decided_at)`, with publication staying at `at`.
  Source: the decoded production CTL, sequence 1 effective 2025-07-16 08:00 UTC and
  published 2025-08-13 (`validation/governance-deployment.md` C-9).
- **Accept test:** `governance/tests.rs`, new
  `a_ctl_is_effective_at_its_vote_and_published_by_the_deadline`. An add-root decision on a
  Friday gives `effectiveDate` equal to the vote and `published_at` equal to the 3-business-
  day deadline.
  - **Fault:** restore `time32(at)`. The test goes red.
- **Settings:** none (a deviation fixed).
- **Goldens:** G-SCMS moves wherever a non-genesis CTL is issued in a case. The genesis CTL
  is unaffected, because its deadline equals the vote (`governance.rs:1226`). Own commit,
  "gov C-9 fixed".
- **Size:** half a piece, done with C-10.

### 2.3 gov C-10: the genesis CTL sequence

- **Today:** the genesis CTL is built with sequence 0 (`governance.rs:1513`, log
  `ctl_sequence: Some(0)`, l.1517). `next_sequence` (l.2247-2254) counts from it.
- **Design:** the live production series has sequence 0 on 2019-12-12 and sequence 1
  effective 2025-07-16 (C-10). Rule: `t0` on or after 2025-07-16 00:00 UTC starts at 1, and
  earlier starts at 0, read from the run's wall clock (`scms/calendar.rs`). It can be
  overridden.
- **Accept test:** `governance/tests.rs`, new `the_genesis_sequence_follows_the_production_series`.
  A 2026 `t0` gives 1, and a 2024 `t0` gives 0.
  - **Fault:** hard-code 0. The 2026 assert goes red.
- **Settings** (D1): `security.scms.lifecycle.ctl_genesis_sequence`, with the values
  `auto` ("By date, as the production series"), `0`, and `1`. Default `auto`. Description:
  "The sequence number of the run's first trust list".
- **Goldens:** G-SCMS moves in every case with `t0` ≥ 2025-07-16. Own commit,
  "gov C-10 fixed".
- **Size:** half a piece, with C-9 (D2 round 1). The key is D1, round 5.

### 2.4 gov C-11: a CTL that drops a root carries it once

- **Today:** `build_ctl` (`governance.rs:1985-2035`) puts only `approved_roots` into
  `root_certs` (l.2024-2027). `remove_root` (l.2320-2330) passes nothing about the removed
  root.
- **Design:**
  - The decoded test CTL package, sequence 3, carries the root it stops approving (C-11).
    In the removal CTL only, `root_certs` = approved + removed, and later sequences drop it.
  - `decide` threads an `Option<Certificate>` of the removed root to `build_ctl`.
  - Labelled "seen in the test CTL only; production practice unknown" in `constants.md`.
- **Accept test:** `governance/tests.rs`, new `a_removal_ctl_carries_the_dropped_root_once`.
  The removal CTL has approve entries + 1 certificates, and the next CTL has exactly the
  approve entries.
  - **Fault:** no carry. The +1 assert goes red.
- **Settings:** none. **Goldens:** G-SCMS moves only for cases that remove a root. List
  them.
- **Size:** half a piece, with L-11.

### 2.5 gov L-11: the ISS `LaSsp`

- **Today:** both LAs carry `LaSsp::new(Uint8(2), Uint16(1|2))` (`pdu.rs:828`, `838`) for
  every profile. The engine's `la_id` is at `msg.rs:298`.
- **Design:**
  - The decoded ISS LA certificates carry version 1 with `laId` 4 and 5 (L-11).
  - Under `CertProfile::Iss`, LA1 and LA2 get `(1, 4)` and `(1, 5)`. The CRL's `LAGroup`
    ids (`run/crl.rs` linkage CRL builder) follow the provider's ids.
- **Risk:** the 2022 module may constrain the version to 2. If `check_*` refuses version 1,
  encode through the 2020 edition's type. Do not drop the constraint check.
- **Accept test:** the oracle compares the provider-a LA certificate's SSP bytes with the
  decoded ISS ones, and a provider-a linkage CRL entry carries `la1Id` 4.
  - **Fault:** version 2. The byte compare goes red.
- **Settings:** none.
- **Goldens:** G-SCMS moves under the US default, whose provider-a is ISS
  (`deployment.rs:465-470`). Own commit.
- **Size:** half a piece (D2 round 2). If it needs `msg.rs`, it waits until C1's wave-C
  merge.

### 2.6 gov G-2: board review and investigation delays

- **Today:** `governance_due` (`phase2.rs:5306-5347`) treats `root_rollover_at_s`,
  `root_removal_at_s` and `peer_root_removal_at_s` as vote instants. Business days come from
  `calendar.rs:185`, and publication deadlines from `ctl_publication_deadline`
  (`governance.rs:1206-1231`).
- **Design:** Elector Policy §2.4.2 says the BOD reviews an addition within 10 business
  days. §2.4.4-2.4.5 say it investigates a removal within 24 h before voting.
  - D2 adds `governance::review_vote_at(kind, requested_at, clock)`, which returns the
    request + 10 business days (add or rollover) or + 24 h (removal).
  - D1: when `eac_review` is `elector-policy`, the existing `*_at_s` keys are read as
    request instants. An `eac-request` event is logged, and the vote happens at
    `review_vote_at`.
- **Accept test:** `governance/tests.rs`, new `a_request_before_a_holiday_votes_ten_business_days_later`.
  A request on Thursday 2026-07-02 skips Friday 2026-07-03 (Independence Day, observed) and the weekends.
  - **Fault:** a delay of 0. The date assert goes red.
- **Settings** (D1): `security.scms.lifecycle.eac_review`, with the values `none` ("The
  times given are the votes") and `elector-policy` ("The times given are requests; the
  board reviews per Elector Policy"). Default `none`. Description: "Whether the board's
  review delay comes before an EAC vote".
- **Goldens:** none (off by default).
- **Size:** half a piece in D2 (the API) and a quarter in D1 (the key).

### 2.7 gov L-8: root termination with 365 days' notice

- **Today:** `remove_root` (`governance.rs:2320`) removes a root at once. There is no notion
  of notice.
- **Design:**
  - [SM-PROV] §3.5.6: termination needs 365 days' notice.
  - `Governance::termination_notice(root, at)` logs `provider-termination-notice`.
    `remove_root` for that root before notice + 365 d returns `None` and counts
    `early_root_removal_refused`.
  - The subscriber-majority transfer vote is not modelled (unpublished, like G-1).
  - A run longer than a year uses fast-forward.
- **Accept test:** `governance/tests.rs`, new `a_root_removal_waits_for_its_notice`. A removal
  at notice + 200 d is refused, and one at notice + 366 d publishes.
  - **Fault:** skip the check. The refusal assert goes red.
- **Settings** (D1): `security.scms.lifecycle.root_termination_notice_at_s`, in s, range 0
  to the run length, step 1, unset by default. Label "Provider gives termination notice at".
  Description: "Starts the 365-day notice before the provider's root may leave the trust
  list".
- **Goldens:** none.
- **Size:** half a piece in D2 and a quarter in D1.

### 2.8 gov G-10: the yearly Test Ballot

- **Today:** `Governance::test_ballot` (`governance.rs:2466`) exists, but nothing schedules
  it.
- **Design:** Elector Policy §1.1 requires a Test Ballot at least yearly. D1's
  `governance_due` calls `test_ballot(now)` once no elector has signed for the configured
  interval, measured from the last signature in `Governance::log`. D2 adds
  `Governance::last_signature_at(elector)`.
- **Accept test:** `v2xw-engine/tests/security_lifecycle.rs`, new
  `a_long_run_holds_a_test_ballot_each_year` (fast-forward 400 days, interval 365). There is
  at least one `eac-test-ballot`, and every elector has signed within 365 d.
  - **Fault:** the scheduler is not called. Zero ballots and the test goes red.
- **Settings** (D1): `security.scms.lifecycle.test_ballot_interval_days`, with the values
  off, 90, 180, and 365 ("Yearly, per Elector Policy") days, range 30-730, step 1. Default
  off. Description: "How long electors may go without signing before a test ballot".
- **Goldens:** none.
- **Size:** a quarter piece in D2 and a quarter in D1.

### 2.9 gov G-11: dual signatures in a post-quantum elector replacement

- **Today:** `replace_elector` (`governance.rs:2414-2460`) signs the transition CTL with
  P-384 electors only. `v2xw-sec/src/scheme/hybrid_mldsa65.rs` has ML-DSA-65 hybrid
  signatures.
- **Design:**
  - Elector Technical Specification v1.1 §2.5 uses dual signatures during a transition, but
    no quantum-safe method has been chosen.
  - With `pq` on, the transition CTL is signed by the old P-384 elector key and the new
    ML-DSA-65 key.
  - The 1609.2.1 CTL `Signature` CHOICE cannot carry ML-DSA. The PQ signature therefore
    travels in an engine sidecar (`SignedCtl.pq_signatures`) and the encoded CTL bytes stay
    as today.
  - Labelled ASSUMPTION ("no PQ CTL encoding exists").
- **Accept test:** `governance/tests.rs`, new `a_pq_transition_ctl_verifies_under_either_key`.
  A device knowing only the old key accepts it, and so does a device knowing only the new
  PQ key.
  - **Fault:** sign once. The old-key device refuses and the test goes red.
- **Settings** (D1): `security.scms.lifecycle.elector_replacement_pq`, with the values off
  ("Classical keys") and `dual` ("Dual signatures (classical + ML-DSA-65)"). Default off.
- **Goldens:** none.
- **Size:** 1 piece (D2 round 6, last: lowest realism impact).

### 2.10 proto H10: `indexWithinI` exhaustion

- **Today:** `next_index` (`run/crl.rs:356-364`) saturates at 255. `issueDate` then orders
  the lists.
- **Design:** the Uint8 is public (`Ieee1609Dot2CrlBaseTypes`). What happens at 256 is
  unpublished, so this is an ASSUMPTION, labelled: once 255 is used at an `iRev`, further
  publications in that i-period coalesce into one pending CRL. It is issued at the next
  i-period with index 0 and counted in `crl_index_exhausted`. The one-business-day SLA check
  (`params.crl.business_day_sla`, `scms/params.rs:316`) records any breach.
- **Accept test:** `v2xw-proto/tests/crl_issuance.rs`, new `no_two_crls_share_an_index_within_a_period`.
  300 publish-on-decision revocations in one week give no two CRLs with equal
  `(iRev, indexWithinI)`.
  - **Fault:** today's saturation. Duplicate 255s turn the test red.
- **Settings:** none. **Goldens:** none (only past 255 publications in one period).
- **Records:** the RunReport leaf `crl_index_exhausted` is added by D1.
- **Size:** half a piece (D3).

### 2.11 proto B9: HTTP request framing on the EE-RA exchanges

- **Today:** request lines and headers are not charged. Responses are charged
  `CRL_NOT_MODIFIED_BYTES` (`phase2.rs:523`), and TLS is charged (§2.1).
- **Design:**
  - [CAMP122] §6.2-6.3 Table 79 (03 §4.2) give the methods, paths and ports. Each request
    is charged its request line, a minimal header set (`Host`, `Content-Type`,
    `Content-Length`) and the Base64 4/3 expansion of a `Download-Req`.
  - Each answer is charged its status line and headers.
  - `v2xw-proto/src/scms/http.rs` (new): `request_bytes(method, path, headers, body_len)`
    and `response_bytes(status, headers, body_len)`.
  - B9a (D2): use them in the `run/provision.rs` and `run/enrol.rs` flow builders. The
    upload's 315 B `Download-Req` (`scms/mbr.rs:94`) is reused, not counted twice.
  - B9b (D1): the CRL poll in `phase2.rs`.
- **ASSUMPTION:** the header set, and the 1609.2.1 path strings (UNCONFIRMED, 03 §4.1).
  Both go in `constants.md`.
- **Accept test:** `v2xw-proto/tests/http_framing.rs` (new). A download request is charged
  its Table 79 path length + headers + ⌈4/3·n⌉.
  - **Fault:** drop the Base64 expansion. The byte assert goes red.
- **Settings:** none.
- **Goldens:** G-SCMS moves (backend and Uu byte counts). Own commit, "proto B9".
- **Size:** 1 piece B9a (D2) and half a piece B9b (D1).

### 2.12 proto D11: `ra-maxAge` (the rest is already closed)

- **Today:** see §2.1. The request-age reading and the `download-maxAge` cache reading
  already hold.
- **Design:**
  - Check whether the device's RA certificate is refreshed within a week: grep the
    chain-file (LCCF) fetch in the `phase2.rs` poll path and `run/gov.rs`.
  - If it is, close D11 as "matches under both readings", with that evidence and the
    existing tests named above.
  - If it is not, the device re-fetches the chain file when its RA certificate copy is older
    than `ra-maxAge` (1 week, [SMP] Table 3), counted in `ra_cert_refreshes`. This is the
    reading labelled ASSUMPTION.
- **Accept test** (only if code is needed): a 9-day run has `ra_cert_refreshes` ≥ 1 per
  device.
  - **Fault:** an interval of `u64::MAX`. The count is 0 and the test goes red.
- **Settings:** none. **Goldens:** none on runs shorter than a week.
- **Size:** a quarter piece if it is a relabel, half a piece with code (D2).

### 2.13 proto B5: the RA's 1 s acknowledgement as a measured SLA

- **Today:** `ServiceQueue::admit` returns `done` (`service.rs:130-155`). The kernel calls
  it at `kernel.rs:596`. No ACK delay is recorded.
- **Design:**
  - [CAMP122] §5.2.8.3 SCMS-521 (ACK within 1 s). For the RA's request handlers, record
    `done − arrival` per request.
  - Keep a per-entity histogram (fixed buckets, integers), `ra_ack_over_1s`, and a p99.
    Nothing is refused, because no deployed RA figure exists.
- **Accept test:** `v2xw-proto/tests/provisioning.rs`, new `an_overloaded_ra_misses_the_one_second_ack`.
  With c = 1 and a fleet start, `ra_ack_over_1s` > 0. An idle RA gives 0.
  - **Fault:** record `arrival` as admission. The count is always 0 and the test goes red.
- **Settings:** none.
- **Goldens:** none (a counter only, not written to records unless D1 adds the leaf).
- **Size:** half a piece (D3, kernel.rs and service.rs). The RunReport leaves
  `ra_ack_over_1s` and `ra_ack_p99_ms` are added by D1.

### 2.14 veh C6: 2 km as straight-line distance from the last change point

- **Today:** `CertStore::travelled` (`v2xw-node/src/stores.rs:243-246`) accumulates
  `distance_since_change_m` (l.168), which is reset at l.296 and l.348.
  `RotationPolicy::is_due` (l.143) reads it (l.369). The callers are `runtime.rs:1189` and
  `vru.rs:1503`.
- **Design:**
  - [CVP-P2] item 25 and [PRIMER] pp.8-9: change after 5 min **and** ≥ 2 km "from the last
    change point".
  - Add `change_point: Option<(f64, f64)>`, set at the l.296 and l.348 resets, and a
    `CertStore::position(x, y)` called beside `travelled` at both callers.
  - `is_due` takes the displacement. The travelled distance stays as a reported figure.
- **Accept test:** `stores.rs`, new `a_loop_back_to_the_change_point_is_not_far_enough`.
  2.5 km travelled with 0.3 km displacement gives no change under `J2945_1`.
  - **Fault:** pass the travelled distance. A change happens and the test goes red.
- **Settings:** none (the rule's own reading).
- **Goldens:** G-SCMS moves where a rotation policy with a distance condition is active in a
  case and the route winds. Own commit.
- **Records:** the PT30 change rate in live-scms, open, predicted equal or lower.
- **Size:** half a piece (D3).
- **J2945/1 primary (2026-10-31, `11-j2945-1-conformance.md` F9-F11, J8):** the design is
  confirmed. The 6.5.3 note defines absolute distance as the straight-line distance, not the
  distance driven. The nominal `vCertChangeDistance` is 2,100 m with a 100 m tolerance
  (Table 21), so 2,000 m conforms. Moving `J2945_1` to 2,100 m may fold in here (same
  lines, same golden commit, about a quarter of a piece). The critical-flag gate
  (`CERTCHG-002`) is J6, in wave J.

---

## 3. Wave D: the UI engine requests (BUILD-STATUS 2026-10-29, both lists)

Requests L1-1 and L2-1 (the enrolment certificate written as `hours(0)`) and L2-14 (memory)
are wave A's SC-D14. The rest, de-duplicated:

| Id | Request (list-item) | Today | Change | Owner |
|---|---|---|---|---|
| U1 | The installed CRL's provider and id8 (L1-2) | `HeldCrlView` has `craca`, `series`, `counter`, `issue_date`, `next_crl` (`sec_records.rs:205`) | Add `provider` (from the CRACA's root on the CTL) and `crl_id8` (HashedId8 of the CRL bytes) | D3 |
| U2 | CRLG: pending vs published, each version's signing time and `next_crl_ns`, per-provider entries and bytes (L1-8, L2-6, L2-11) | `crlg.entries` mixes both (`scms/inspect.rs:317-318`) | `entries_published`, `entries_pending`, `last_signed_ns`, `next_crl_ns`, `crl_bytes`, per provider | D3 |
| U3 | `pki.case` "corroborated": the rule's threshold and the attack onset (L1-3) | `drain_cases` (`pki_view.rs:1135`) has no threshold | U3a (D1): the engine attaches the MA rule's threshold to the case evidence (`run/mod.rs` `ReportEvidence`). U3b (D3): `pki_view` emits it, and the onset in the ground-truth view only | D1, then D3 |
| U4 | `inspect.pki` vehicles: `in_run`/`left_at_ns`, and the EC's `valid_until` (L1-4, L2-2, L2-3) | `vehicle_row` (`v2xw-server/src/pki.rs:923`) guesses from `security.t` | Read the node removal record if one exists, else add `left_at_ns` to `node.security`. EC `valid_until` from `NodeCredentials` (`pki_view.rs:1438`) | D3 |
| U5 | A metrics deep link to one series (L1-5) | No stable series key | A stable `series_id` on each `MetricDef` series (`v2xw-metrics/src/def.rs:217`). The link itself is Studio work | D3 |
| U6 | An RSU's ee device has no node (L1-6) | `component_ids`/`node_id` (`pki_view.rs:303`, `428`) | Publish `kind: rsu` and the RSU's node for device ids in the RSU range | D3 |
| U7 | ee snapshot: `topups_refused_blocklisted` (L1-7); `reports_evicted/expired/not_filed/refused_mtu` (L2-7); vehicles per provider (L1-13) | Counters in `Phase2Report` (`phase2.rs:1704`, `1751`, `1841`); the ee arm of `backend_view` (`phase2.rs:6638ff`) sets `vehicles` on the home provider only | Set them in the ee arm, per provider | D1 |
| U8 | `security.next_change_ns` (L2-4; `cert_i`/`cert_j` already exist) | `NodeSecurityView` (`sec_records.rs:146`) | `CertStore::next_change_at` from `RotationPolicy` (`stores.rs:143`), after C6 | D3 |
| U9 | Authority certificates outside the 1,000-record ring; electors openable (L2-5) | `put_cert` (`v2xw-server/src/pki.rs:304`) | A separate unbounded authority map keyed by id8 | D3 |
| U10 | RA `next_shuffle_release_ns` (L2-8) | Only `shuffle_window_s` (`inspect.rs:242`) | From `report_shuffle_armed` and the timer start | D3 |
| U11 | MA `received_direct` vs `received_shared` (L1-9, L2-9) | `reports_received` = `ma.reports.len()` (`inspect.rs:299`); `shared.received` exists (`run/mod.rs:1047`) | Two fields | D3 |
| U12 | `detection_pending` / MA decision outcomes and times (L2-10) | Cases carry stages (`pki_view.rs:1135`) | `outcome` (`revoked`, `linked` from SC-P3, `unresolved`) and `decided_at_ns` per case | D3 |
| U13 | `time_to_decision` pooled over the run (L1-10) | A mean of windows (`v2xw-metrics/src/detection.rs:681`, `935`) | A pooled run-wide value beside the windowed one | D3 |
| U14 | Catalogue: scheduling-window stages, the cumulative marker, linkability's coverage and reference as data (L1-11, L1-12, L2-12) | Text only in `definition_md` | `MetricDef` fields `aggregate` (`cumulative` or `windowed`), `stage_kind: scheduling-window`, `reference`/`coverage` (`privacy_metrics.rs`) | D3 |
| U15 | Full-resolution card series for sparse runs (L2-13) | The binned overview has 1-3 bins after a fast-forward | The server returns unbinned points when the bin count is below 8 (grep the binning in `v2xw-server`) | D3 |

**Accept tests:** one unit test per field, on the smallest SCMS scenario, asserting presence
and the value against an independent count. The fault for each: emit a constant 0 or omit
the field, which turns the assert red. Studio specs are not run by these agents (UI rule:
typecheck only).

**Settings:** none. **Goldens:** none. Snapshots are not in the record digests; check
`DigestRecorder` excludes `backend_view` before relying on that.

**Size:**
- D1: 1 piece (U3a, U7).
- D3: 3 pieces:
  - U1, U4, U6, U9;
  - U2, U10, U11, U12, U3b;
  - U5, U8, U13, U14, U15.

---

## 4. Schedules

Each round is one piece of about 3 h per agent. There is one heavy command at a time across
the three agents (`heavy.sh`), with the per-worktree cargo config, the shared `target`, and
`worlds/cache` symlinked. No agent merges. A separate non-isolated integrator merges
`--no-ff` at the marked merge points (memory: worktree agents cannot merge), and the
remaining agents rebase onto main before their next round. **Hot files** (`phase2.rs`,
`run.rs`, and the scenario schema, validate, options, guide and publish files, plus
`configuration-reference.md`) are edited by one named agent per wave and by no other.

### 4.1 Wave C: real pipeline (after waves A and B are merged)

| Agent | Owns (exclusive in wave C) |
|---|---|
| **C1, MA pipeline** | `v2xw-engine/src/phase2.rs`, `report_records.rs`; `v2xw-proto/src/scms/run/revoke.rs`, `run/report.rs`, `run/mod.rs`, `scms/msg.rs`, `scms/mbr.rs`; tests `v2xw-proto/tests/{resolution, revocation_path, ma_cases, provider_runs, mbr}.rs`, `v2xw-engine/tests/{report_signing, scms_providers, scms_deployment}.rs` |
| **C2, devices and receivers** | `v2xw-engine/src/run.rs`, `wiring.rs`, `hosted.rs`; `v2xw-node/src/{runtime, rxval, secure, vehicle, events}.rs`, new `v2xw-node/src/regions.rs` and `v2xw-node/data/regions.bin`; `v2xw-threat/src/{us_mbr, detect}.rs`; `v2xw-core/src/bsm_fields.rs`; new `tools/regions/`; `vectors-provenance.md`; tests `v2xw-node/tests/{rx_validation, runtime_behaviour}.rs`, `v2xw-engine/tests/credential_handoff.rs`. Also owns the scenario files and `configuration-reference.md`, though no wave C piece is expected to touch them |
| **C3, backend** | `v2xw-proto/src/scms/run/{crl, appcert}.rs`, `scms/appcert.rs`, `service.rs`, `kernel.rs`, `fault.rs`, `spec.rs`, `scms/params.rs`; `v2xw-sec/src/crl.rs`; tests `v2xw-proto/tests/{crl_issuance, appcert}.rs`, new `availability.rs` |

| Round | C1 | C2 | C3 |
|---|---|---|---|
| R1 | SC-P1.1 (proto: subject from the opened report) | SC-P10.1 (joiner-at-boundary cause and fix) | SC-P8.1 (component hash CRLs, series 2/256) |
| R2 | SC-P1.2 (engine: `truth_subject`, issuer routing) | SC-P10.2 + SC-P7 | SC-P8.2 (distribution and refusal effects) |
| R3 | SC-P2/P3.1 (chain-id flow, no `direct`) | SC-P11.1 (polygons, grid, provenance) | SC-P9.1 (OBU identification cycle) |
| R4 | SC-P2/P3.2 (chain de-duplication) | SC-P11.2 (receiver, inheritance) | SC-P12.1 (availability) |
| **M-C1** | The integrator merges C2 (P7, P10, P11) and C3 (P8.1-2, P9.1, P12.1). C1 rebases | | |
| R5 | SC-P5 (re-verified sharing) | SC-P9.2 (identification signer slot) | SC-P12.2 (log-normal service times) |
| R6 | SC-P4 (published CRL) | done | done |
| R7 | SC-P6 (US default: SM-MBR, the BrakeAcceleration arm, G-ML pins) | | |
| **M-C2** | The integrator merges everything. Wave C ends | | |

- **Golden commits, in merge order:** P10.1, P10.2, P11, P1, P2/P3, P5, P4, P6. Each is
  re-blessed on its own commit, in that order, so every digest change has one cause.
- **Spare capacity:** C2 and C3 finish after R5. From R6, D2 and D3 may start in their
  place: their files are disjoint from C1's (check the owner table in §4.2; C3's
  `run/crl.rs`, `service.rs` and `kernel.rs` are free once C3 is merged at M-C1 + R5).
  That keeps three agents busy and saves about two rounds.

### 4.2 Wave D: rule gaps and the UI engine requests (after M-C2; D2 and D3 may start at C's R6)

| Agent | Owns (exclusive in wave D) |
|---|---|
| **D1, engine wiring and settings** | `v2xw-engine/src/phase2.rs`, `run.rs`, `crypto_config.rs`; `v2xw-engine/src/scenario/{schema, validate, options, guide, publish}.rs`; `docs/guides/configuration-reference.md`; `v2xw-proto/src/scms/run/mod.rs` (U3a only); tests `v2xw-engine/tests/{security_lifecycle, emergency_yield, backend_entities, rsu_certs}.rs` and new `component_crl.rs` |
| **D2, governance and flows** | `v2xw-proto/src/scms/governance.rs`, `scms/governance/tests.rs`, `scms/calendar.rs`, `scms/pdu.rs`, new `scms/http.rs`, `scms/run/provision.rs`, `scms/run/enrol.rs`, `scms/run/gov.rs`; `v2xw-sec/src/ctl.rs`; tests `v2xw-proto/tests/{governance, http_framing, pki_profile}.rs` |
| **D3, observability, CRL index, ACK, rotation** | `v2xw-proto/src/scms/{inspect, pki_view}.rs`, `scms/run/crl.rs`, `service.rs`, `kernel.rs`; `v2xw-engine/src/{sec_records, privacy_metrics}.rs`; `v2xw-server/src/pki.rs` and the server metrics binning; `v2xw-metrics/src/{def, detection}.rs`; `v2xw-node/src/{stores, runtime, vru}.rs`; tests `v2xw-proto/tests/{crl_issuance, provisioning, pki_view}.rs`, `v2xw-engine/tests/pki_view.rs` |

| Round | D1 | D2 | D3 |
|---|---|---|---|
| R1 | C tail 1: SC-P8.3 (component revocation keys and engine test) + the SC-P6 text in the guide, options and reference | gov C-9 + C-10 (two golden commits) | veh C6 |
| R2 | C tail 2: SC-P12.3 (availability and service-time keys, `crypto_config.rs`, engine test) | gov C-11 + L-11 | proto H10 |
| R3 | C tail 3: SC-P9.3 (emergency preload, SRM out of the pseudonym PSIDs) | gov G-2 + L-8 + G-10 APIs | proto B5 |
| R4 | UI U7 + U3a | proto B9a | UI: U1, U4, U6, U9 |
| **M-D1** | The integrator merges D2 (R1-R4) and D3 (R1-R4). D1 rebases | | |
| R5 | Governance keys and scheduling: `ctl_genesis_sequence`, `eac_review`, `root_termination_notice_at_s`, `test_ballot_interval_days`, `elector_replacement_pq`; RunReport leaves `crl_index_exhausted`, `ra_ack_over_1s`, `ra_ack_p99_ms` | proto D11 | UI: U2, U10, U11, U12, U3b |
| R6 | B9b (CRL-poll framing in `phase2.rs`) | gov G-11 | UI: U5, U8, U13, U14, U15 |
| **M-D2** | The integrator merges everything. Wave D ends, and wave E (validation) starts | | |

- **No shared hot file:**
  - `phase2.rs`, `run.rs` and the scenario files are D1's only;
  - `governance.rs` and `pdu.rs` are D2's only;
  - `inspect.rs`, `pki_view.rs`, `run/crl.rs`, `kernel.rs` and `service.rs` are D3's only.
- D1's settings pieces carry every key of §1 and §2 that names D1, each with its schema,
  validate, options, publish, guide and reference lines.
- **Golden commits in wave D:** C-9, C-10, L-11, B9 (B9a and B9b may share one re-bless after
  M-D2 if B9a alone moves nothing), C6, P9.3, each with its cause.

### 4.3 Load per round

There are three agents, and at most two heavy commands per piece (one crate test, one
engine test file), queued through `heavy.sh`. Nothing runs a city scenario: every new
expectation row is written `verdict: open` and run in wave E. Source-only steps (the C2
R3 polygon download and the D2 R2 oracle bytes) can share a round with build pieces
without adding load.
