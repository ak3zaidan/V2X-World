# SCMS audit: governance and trust anchors

Date 2026-10-24. Base `8becf72a`. Expectations:
`crates/v2xw-engine/tests/expectations/scms-audit-gov.yaml`, committed with
`measured: pending` (`a8f4434c`) before the runs they judge. Checker:
`tools/realworld/audit_gov.py`. It reads the raw records (`recording.mcap`, `run-report.json`)
and decodes certificates with the independent oracle's COER codec. It compares the
certificate profiles with SCMS Manager's published chain files in
`third_party/scms-artefacts`, and computes Time32, the i-period and Eastern business days from
their definitions. It reads no engine code.

Components: the electors and the CTL, each provider's Root CA and ICA, each provider's
certificate profiles, the ECA and enrolment, and the real calendar.

Runs: `scenarios/scms-conformance.yaml` at `--record-level events`, one run per run lock
(second slot, about 200 MB). It covers enrolment before t0, a root rollover decided at
120 s, the i-period boundary at 600 s, and fast-forward through the CTL publication.
- **P0:** the provisioning audit's run of the release build, copied. Used for probing and as
  A's determinism twin.
- **A:** the release build of `8becf72a`.
- **B and B2:** this branch's build.

## 0. Verdict

The table has 34 rows: 28 pass, 2 fixed, 1 wrong expectation (corrected) and 3 open. Fault
proof: on B, 33 of 33 rows go red under their injected fault, and the server row goes red
under its own fault.

- **The governance works as the published system does:**
  - 5 electors, quorum 3, every CTL signed 5 of 5.
  - CTL 0's `rootCaApprove` equals SHA-256 of both providers' root certificates.
  - The rollover decided Tuesday 08:52 UTC is published at Friday 08:52 UTC, exactly 3
    Eastern business days later, and keeps the old root. `effectiveDate` is the publication
    instant, as documented in C-9.
- **Every component certificate matches its provider's published form:** validity,
  crlSeries, encryption key, region and SSP. That is 15 certificates against SCMS Manager's
  2025 chain files: provider-a in the ISS form, provider-b in the SAESOL S2X production form.
  All 20 chains verify to a CTL root.
- **Enrolment:**
  - 65 enrolment certificates: implicit, `hours(52560)`, region {124, 484, 840}, crlSeries 4.
  - Each is issued by its own provider's ECA, has a unique name and a fresh key, and chains
    to a CTL root.
  - One request and one response per device, before its first provisioning request.
  - The ECA counters (37 and 30) equal the records.
- **The calendar:**
  - Time32 is right on all 9,563 `pki.state` records, with 5 leap seconds.
  - `current_i` and `cert_i` follow the 2015-01-06 epoch, with the 1 h overlap.
  - All 2,840 pseudonym certificates start at Time32 of their UTC boundary and last 169 h.
- **Metrics:**
  - `ctls_published`, `gov_decisions` and `trust_list_verifications` (24 × (5 + 3)) equal
    the records.
  - `trust_lists_installed` now equals the record installs plus the roadside units
    (22 + 2).
  - The server's `inspect.trust`, the Studio PKI view's source, equals the recorded
    `pki.state` at 5 instants.
- **Determinism:** A and P0 have the same record digest (`f00dc8b0…`), and so do B and B2
  (`681eeb98…`).
- **Fixed:** four engine defects (§2), each with a test that is red without its fix. The
  fixes move the regression digests, and they were re-blessed (§6).
- **Open:** three defects (§4). One component has two certificates, a rollover changes
  nothing on the wire, and provider-b's second ACA is uncertified.

## 1. Inventory

A is the release build. B is the fixed build. Where A and B agree, one value is given.

| Row | Behaviour | Spec value (source) | Measured | Verdict |
|---|---|---|---|---|
| G3 | Electors | 5 (Tech Spec §2.1) | 5 entities, 5 certificates | pass |
| G4 | Quorum | 3 (Tech Spec §2.4) | 3 on every record | pass |
| G5 | Signers per CTL | all 5 (Tech Spec §2.4) | [5, 5] | pass |
| C1 | `rootCaApprove` | SHA-256 of each root ([SMP] §6.1) | 2 of 2 recomputed | pass |
| L6 | Rollover keeps the old root | [SM-PROV] §3.5.4 | seq 0→1, roots 2→3 | pass |
| C6 | Root addition published within 3 business days | [SM-PROV] §3.4.11 | Fri 08:52 UTC, at the deadline | pass |
| C9 | `effectiveDate` | the ballot date in reality; the publication instant in the engine (deviation C-9) | 719,830,325 = Time32(publication) | pass |
| C12m | CTL sequence on devices | monotone, never ahead of publication | 0 violations | pass |
| C12r | `trust_lists_installed` = record installs + RSUs | metric integrity | A: 24 vs 0. B: 24 vs 22 + 2 | **fixed** |
| C12v | `trust_list_verifications` | installs × (signers + roots) | 192 = 24 × 8 | pass |
| M | `ctls_published`, `gov_decisions` | 1, 1 | 1, 1; electors' counter 1 | pass |
| L1 | Validity per role | published chain files | 15 of 15 equal | pass |
| L3 | crlSeries | [SMP] Table 1 | 15 of 15 | pass |
| L4 | Encryption key | published | 15 of 15 | pass |
| L5 | Region | ISS {124, 484, 840}; S2X none | 15 of 15 | pass |
| L10 | SSP | published (version: §4 item 5) | 15 of 15 | pass |
| L.chain | Signatures, nesting, CTL root | 1609.2 §5.1 | 20 of 20 | pass (check corrected, §3) |
| L.names | Component names carry their own label | Provider Req. §3.3.1.1 | 0 mismatches | pass |
| L.onecert | One certificate per component | [SMP] §6.1 | ECA and ACA of both providers carry two | **open** |
| L.counters | Root/ICA `certs_issued` per provider | metric integrity | A: 9 / 14 / 0 / 0. B: 5 / 8 / 4 / 6 (b ICA: 7 recorded) | **open** (part fixed) |
| D1 | Two certified ACAs at provider-b | published SAESOL chains | 2 placed, 1 certified | **open** |
| E1 | Enrolment validity | 6 y (CAMP122 Table 3) | `hours(52560)` on all 65 | pass |
| E3 | Enrolment profile | [SMP] §6.3.1 | 0 of 65 violate | pass |
| E3.name | Name names its ECA's provider | [SMP] §6.3.1 | A: 30 say provider-a. B: 0 | **fixed** |
| E3.key | Fresh key | [SMP] §6.3.1 | 65 distinct | pass |
| E.chain | ECQV reconstruction and chain to a CTL root | 1609.2 §5.3 | 65 of 65 | pass (check corrected, §3) |
| E.once | One bootstrap per device, before provisioning | CAMP122 | 67 devices, 0 violations | pass |
| E.count | ECA counters | metric integrity | 37 / 30 = records | pass |
| K1 | Time32 (TAI, 5 leap seconds) | 1609.2; IERS | 9,563 of 9,563 | pass |
| K3 | i-period from the 2015-01-06 epoch | [SMP] Table 2 | 0 differ (176 + 16,558 records) | pass |
| K5 | Pseudonym start and 169 h | [SMP] Table 2; CAMP122 Table 5 | 2,840 of 2,840 | wrong expectation (§3) |
| SRV | `inspect.trust` = `pki.state` | server contract | 5 of 5 instants equal | pass |
| DET.release | A = P0 | same seed | `f00dc8b0…` both | pass |
| DET.fixed | B = B2 | same seed | `681eeb98…` both | pass |

These items are not observable in the records:
- **Elector certificates (G-6, G-7):** P-384, SHA-384, the 12/15/18/21/24-year validity.
  `pki.state` carries only their HashedId8s. `real_artefacts` covers them.
- **Signature bytes of the CTL and chain files (C-13):** these files are not on `pki.flow`.
- **Test ballots and the EAC review delay (G-2, G-10):** documented deviations.

## 2. Fixes

Each fix is at its cause and has a test that fails without it (§5).

1. **GOV.E3.name: enrolment certificate names** (06 §6 item 12, plan §9 item 7). Every
   enrolment `id.name` said `ee-<device>.provider-a`, including the 30 that provider-b's
   ECA issued in A.
   - **Fix:** `run/enrol.rs` `device_name` takes the issuing provider's label.
   - **Test:** `provider_runs`
     `an_enrolment_certificate_names_the_provider_whose_eca_issued_it`.
   - **Effect:** the bytes of provider-b enrolment certificates change. The name has the
     same length, so sizes do not move. The record digest moves (see §6).
2. **GOV.L.counters: the Root CA and ICA counted every provider's certificates.** Every
   peer certificate was counted on the home governance. At t0 the home `pki.state` therefore
   read root 9 and ICA 14, and provider-b's root and ICA read 0. These counters are what the
   Studio's PKI view shows.
   - **Fix:** `governance.rs` `make_authority` counts only the view's own provider, and
     `rehome` counts the peer's own set.
   - **Test:** `provider_runs` `each_providers_root_and_ica_count_their_own_certificates`
     (5 / 4 / 6).
3. **GOV.C12.record: `node.credentials` showed none of the CTL installs.** The record took
   `ctl_sequence` from the backend's enrolment-time trust. A vehicle that installed CTL 1 with
   a composite CRL download (`take_ctl_install`) still read 0. A counted 24
   `trust_lists_installed`, but the records showed 0 installs.
   - **Fix:** `pki_view::node_credentials` takes the CTL the node installed (`installed_ctl`)
     and includes it in the change digest, so the install writes a record.
   - **Test:** phase2 `the_credentials_record_names_the_ctl_the_vehicle_installed`.
4. **GOV.C12.record: a joiner installed a CTL it already held.** A vehicle enrolled after
   the CTL change gets that CTL with its enrolment (`enrol.rs` installs the CTL in force).
   On its first download it still "installed" the CTL again from the factory trust, and that
   was counted.
   - **Fix:** `take_ctl_install` starts from the vehicle's enrolment trust when it is newer
     than the factory trust.
   - **Test:** phase2 `a_vehicle_enrolled_after_a_ctl_change_does_not_install_it_again`.

## 3. Wrong checks, corrected with evidence

- **GOV.K5.pseudonym** expected `hours(169)`. The engine encodes `minutes(10140)`, which is the
  same 169 h. `constants.md` (`cert_lifetime_minutes`, CITED CAMP-EE R1.1 §2.1.5.3.2) and
  crypto #31 give the minutes form. The row now accepts 169 h in any unit. Every one of the
  2,840 pseudonym certificates starts at the Time32 of its UTC i-period boundary.
- **GOV.L.chain and GOV.E.chain**: the oracle's `chain.validate` rejects SCMS Manager's own
  published production chain (`cert-chain_0.oer`). For example, `ica.prod.s2x.io` gives "issuer
  minChainLength 3 > subordinate 1 + 1". The cause is in `check_pair`: it takes the maximum
  permission group on each side, which is wrong when the issuer has both `all` and `explicit`
  groups. The checker therefore uses its own validator. It checks the issuer HashedId8,
  ECDSA signatures, validity nesting, region containment and the root on the CTL. It does not
  judge chain-length permissions. The oracle's group rule is an oracle defect, reported here
  and left in place. Only `corpus.py` uses it, on its own three-level synthetic chains, and
  `records.py` never calls it.

## 4. Open

1. **GOV.L.onecert: two certificates for one component.** For the ECA and the ACA of both
   providers, `pki.state` names one certificate (`cert`) and the entity's counters name
   another (`counters.certificate`).
   - The first is the PDU layer's (`pdu.rs` `ScmsPki::under_anchors_profiled`). It signs on
     the wire: every enrolment certificate's issuer is that one.
   - The second is the governance's (`governance.rs` `make_authority`). It goes into the
     chain files and is the one a rollover re-certifies.
   - In the real system, a provider's chain file lists the certificate that signs.
   - Trust effect: none seen, because devices anchor on the PDU layer's certificates
     (`Phase2::trust_bundle`). But the Studio shows both, and GOV.L.counters cannot be exact
     for provider-b's ICA. Its counter is 6 (governance set) and the records show 7 (PDU set,
     with the DCM).
   - Fixing it means building the PDU layer's components from the governance authorities.
     That moves every certificate byte and every digest.
2. **After a root rollover, nothing on the wire chains to the new root** (code reading). The
   PDU-layer PKI is fixed when the run starts (an `Arc`), and `rollover_root` re-certifies
   only the governance's set. [SM-PROV] §3.5.4 says that after a changeover the provider
   signs only with the new key. Gov L-6 is marked MATCHES for the CTL side, and that holds:
   the old root stays approved. In this run nothing is issued after CTL 1 is published, so
   no record can show the gap. A run that provisions after a rollover's publication would.
3. **GOV.D1.acas: provider-b's second ACA has no certificate.** `aca/1` is placed, online,
   and carries no traffic. SAESOL publishes two ACA certificates in both chain files.
4. **The enrolment request's DCM→ECA hop is not on `pki.flow`.** EE→DCM, ECA→DCM and DCM→EE
   are recorded. Because of this, the ECA's traffic counters show 0 requests received
   against 37 and 30 issued. All of it happens before t0 (the factory), so the impact is
   observability only.
5. **SSP version:** the published ISS pre-production certificates carry SSP version 1
   (`83 00 01`, `85 00 01`, `8b 00 01`, `86 00 01`). The engine uses version 2 for both
   forms. This is gov L-11's reasoning (2022 module) extended past the LAs. GOV.L10 checks
   the choice byte for ISS and every byte for S2X. No trust effect.
6. `trust_fetches` is ETSI-only and reads 0 under the US profile. That is correct: a US CTL
   rides the composite CRL download and is counted in `trust_lists_installed`.

## 5. Fault proof

**The checker's rows.** `audit_gov.py RUN --selftest --base RUN` injects each row's fault
into the loaded records and checks that the row goes red. The faults include:
- a certificate mutated and re-encoded;
- a record time shifted;
- a counter +1;
- a signer dropped;
- a name duplicated;
- a key reused.

33 of 33 rows go red on P0 and on B. The two rows that A fails and B passes (C12.record,
E3.name) are among them, so each has been seen both green and red.

**The server row.** `tools/realworld/audit_gov_server_compare.py` adds 1 to provider-b's
root `certs_issued` in the 121 s answer, and that instant goes red. The row reads
`inspect.trust` with `tools/realworld/audit_gov_server_query.py` against `v2xw-server
--scenario tools/realworld/audit_gov_server.yaml --speed 0 --record`. The server leaves out
the byte stores (`certificates`, each CRL's `hex`) and serves them through `inspect.cert`.
Every other field is equal.

**The engine tests.** All four new tests were run with the fixes undone and the tests kept,
and all four fail:
- `provider_runs`: 2 failed of 9;
- `phase2` `ctl_device_tests`: 2 failed of 4.

With the fixes, every test passes: `provider_runs` 9/9, `governance` 9/9 and
`ctl_device_tests` 4/4.

## 6. Determinism and goldens

**Determinism.** The digest is SHA-256 over every record's topic, log time and payload.
- Same binary, same seed:
  - A (`--no-verify`) and P0: `f00dc8b049a9114a…` both.
  - B and B2 (fixed build): `681eeb980c7be640…` both.
- A and B differ, as the fixes intend. Provider-b enrolment certificates change their bytes,
  and `node.credentials` records the CTL installs.

**Goldens.** Re-blessed with `V2XW_BLESS_SCMS_DIGESTS=1`:
- `phase2-path`: record digest `9939…` → `f5df…`;
- `i-s1-phase2-60s`: `fd19…` → `e4dc…`.

Frame digests and record counts are unchanged, so no air traffic moved. The cause is the
provider-b enrolment certificate name: its bytes enter `pki.flow` and `node.credentials`.

**Rows.** `crates/v2xw-engine/tests/expectations/scms-audit-gov.yaml` has 34 rows: 28 pass,
2 fixed, 1 wrong expectation and 3 open.
