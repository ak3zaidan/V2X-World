# SCMS Manager published artefacts (WP-A5)

Source: https://www.scmsmanager.org/publications/ (read 2026-10-16). The page publishes
the US SCMS certificate trust lists (CTLs), certificate chain files and the elector
certificates, each with a SHA-384 fingerprint.

**Not committed.** The page carries "© SCMS Manager LLC All Rights Reserved" and no
licence or terms of use, so per the project rule (store reference files only when their
licence allows) the files live in the git-ignored `files/` directory and are fetched by
`tools/scms-artefacts/fetch.py`, which checks each one against the published SHA-384 and
the sha256 below. Owner decision: if SCMS Manager's terms allow redistribution (the
files are trust anchors meant to be installed on every device), commit `files/` and drop
the `#[ignore]` from `crates/v2xw-sec/tests/real_artefacts.rs`.

| Local path (`files/…`) | Download (under `wp-content/uploads/`) | Published SHA-384 (of the file) | sha256 |
|---|---|---|---|
| `production-2025-08-13/production_ctl.oer` | `2025/08/20250813-production_ctl.zip` | `ca871c47…3ada3f5921` | `4c0b7d61…c4b33d65` |
| `production-2025-08-13/cert-chain_0.oer` | same zip | — | `738b3400…be6808b` |
| `production-2025-08/production_ctl.oer` | `2025/08/production_ctl.zip` | `c7401113…e897445a9` | `c554f1c3…6077ccd5` |
| `test-2025-07-17/test_ctl.oer` | `2025/07/SCMS_Manager-Test_CTL-0348000000000000-679737605.zip` | `1327546c…112514dd18` | `b7f2a46c…715ae71` |
| `test-2025-07-17/test_cert_chain.oer` | same zip | — | `9dbc552e…a598414c` |
| `test-2025-07-17/roots/*` (5 root certificates) | same zip | — | in `fetch.py` |
| `test-2025-03-18/test_ctl.oer` | `2025/03/SCMS_Manager-Test_CTL-0348000000000000-669060983.zip` | `0518312d…b72519fe2` | `529d1c2f…5bdd5daa` |
| `test-2024-04-19/test_ctl.oer` | `2024/05/SCMS_Manager-Test_CTL-0348000000000000-640624233.zip` | `57529db9…8300ec6f` | `fbf3f0f2…c4424ec` |
| `test-2023/test_ctl.oer` | `2023/04/test_ctl.zip` | `bf4dfa46…f4ba9f9b93` | `a43948ea…093bce514` |
| `electors/v2xelector{1..5}.certificate.oer` | `2020/01/v2xelector{n}.certificate.oer_.zip` | in `fetch.py` and the test | in `fetch.py` |
| `electors/v2xelector{1..5}.test.certificate.oer` | `2020/08/v2xelector{n}.test_.certificate.zip` | in `fetch.py` and the test | in `fetch.py` |

`fetch.py` holds the full values. All 16 published SHA-384 fingerprints matched the
downloaded files on 2026-10-16.

## What the files show (checked by `tests/real_artefacts.rs` and `oracle.py real`)

- Every CTL is `Ieee1609Dot2Data{unsecuredData(ScmsPdu{v2, cert: multiSignedCtl})}`,
  type `fullIeeeCtl`, CtlSeriesId `0348000000000000`, signed by all 5 electors with
  ECDSA P-384 / SHA-384. Our `HashedId48` of each attached elector certificate equals
  SCMS Manager's published fingerprint.
- Sequence history. Production: 0 (effective Time32 503 222 405, 1 root), then 1
  (2025-08-13; effective 679 737 605; quorum 3; roots `rootca.prod.s2x.io` and
  `v2xrootca.ghsiss.com`). Test: 0 (2023), 1 (2024-04-19), 2 (2025-03-18), 3
  (2025-07-17). Sequence 0 of both series **omits `quorum`** (an extension addition),
  which the generated binding could not decode: fixed by `crates/v2xw-msg/patches/0006`
  and `0007`.
- `rootCaApprove` = `HashedId32` of each approved root; for the 2025-07-17 test CTL each
  entry equals the SHA-256 of the separately published root file. The Eviden root
  (`https://rca.plugfest.c-its-pki.org`, P-384, `self(sha384)`) is approved by the
  low-order 32 bytes of its SHA-384 in 2024-04-19 (per the 1609.2.1 note), by its
  **SHA-256** in 2025-03-18 (contrary to the note), and not at all in 2025-07-17.
- The production chain file's `homeCtl` is **not** the SCMS Manager CTL: it is a
  provider-local CTL, series `ffffffffffffff00`, sequence 0, signed by five electors
  named `elector1`…`elector5` that are not SCMS Manager's. Its six certificates
  (SAESOL `*.prod.s2x.io`) verify up to `rootca.prod.s2x.io`, which the production CTL
  approves. The test chain file's 23 certificates verify up to roots on the test CTL.
- No chain file contains an implicit certificate, so there is no published ECQV
  reconstruction to check.
- One SAESOL test ACA certificate is transmitted with a 2-bit extension bitmap where
  1609.2-2022 has 4 (see `tools/oracle/scms_oracle/real.py` `KNOWN_NONCANONICAL`); its
  signature verifies over the canonical re-encoding.
