# SCMS configuration testing: summary report

Date: 2026-10-21. This page sums up four testing tracks, now all merged into main:
settings (knobs), effects, edge cases and runtime realism. The full tables, with sources,
are in `testing/scms-knobs.md`, `testing/effects.md`, `testing/edge-cases.md` and
`testing/realism.md`.

Every track worked the same way. The expected result was written down, with its source,
before each run. Then a base run and a variant ran on the same seed and were compared.
Each new check was broken on purpose to show that it can fail (fault injection).

## 1. Settings added

- **Certificate lifecycle**, now typed, validated and published under
  `security.scms.lifecycle`, with a "Certificates & SCMS" section in the settings window.
  Three are new to scenarios: `max_download_polls`, `download_request_tolerance_s` and
  `ma_max_open_cases`.
- `security.scms.tls`: TLS 1.3 or 1.2 on the backend links.
- `security.scms.crl_overdue_tolerance_s`: how far past `nextCrl` a receiver still accepts
  a CRL (two weeks, CAMP 1.2.2 Table 67).
- `nodes.validated_cert_cache`: the receiver's validated-certificate cache (4,096).
- Device retry after a lost request (CAMP SCMS-522/523): `ee_retry_timeout_s` (10 s, 0 turns
  it off) and `ee_retry_max_per_hour` (10).
- `crl_background_entries`: a standing CRL of N earlier revocations.
- `clock_skew_s` and `clock_skew_share`: vehicles whose clocks run ahead or behind.
- `peer_root_removal_at_s`: remove a provider's root from the trust list mid-run.
- The strict US misbehaviour profile `detect/us-sm-mbr`, from the SCMS Manager SM-MBR
  thresholds.

## 2. Changes tested: expected against measured

| Track | Rows | Pass | Fixed (a bug, then pass) | Expectation wrong, corrected | Open or partial |
|---|---|---|---|---|---|
| Settings (knobs) | 21 | 13 | 4 | 6 (also counted under pass) | 1 partial (request shuffle), 1 open (cache cost) |
| Effects | 88 | 77 | 7 | 4 | 0 |
| Edge cases | 12 | 6 | 4 | 0 | 2 (3d, 11) |
| Runtime realism, base | 27 | 22 | 4 | 0 | 1 (R-LOP, since fixed) |
| Runtime realism, changes | 8 | 6 | 1 | 0 | 1 (R-TRUSTLEAK) |

A few examples of what the settings do:

- 10 certificates a week instead of 5: certificates issued doubled exactly (950 to 1,900),
  with the same number of top-ups.
- No top-up at all: 62 vehicles ran out of certificates, against 2.
- CRL fetch every 120 s instead of 30 s: 40 downloads instead of 116.
- Automatic MA instead of an 8 h manual review: revocation 53 h to 29 h.
- Publishing the CRL at the decision instead of the next business-day slot: 53 h to 32 h.
- Hybrid post-quantum certificates: 9.2 times the security bytes on air, 4.2 times the
  channel load.
- Verifying only on demand cut verifications from 756/s to 46/s. It also left the fleet
  blind: reports fell from 897 to 79 and no attacker was revoked.

The wrong expectations were mostly about what the base already does. For example, the
pseudonym changes rose 2.8 times, not 10 times, when their period went from 300 s to 30 s,
because the base already changes at every 60 s period rollover.

## 3. Edge cases

| Case | Held? |
|---|---|
| Week rollover: nobody signs outside a certificate's validity | Yes, after a fix: 6 of 1,983 misbehaviour reports had been signed with an expired pseudonym |
| A vehicle that leaves still showed a certificate in use | Fixed |
| Clock skew at the validity edges | Yes, after the clock was wired into security |
| RA offline across the top-ups | Fixed: 6 of 12 top-ups were lost; with the new retry 12 of 12 complete |
| RA slow on the first request | Yes: the request is accepted once and acknowledged twice |
| RA down after its acknowledgement | **Open**: the LA and ACA answers are never resent |
| Device offline for weeks | Yes: silent once its pool runs out, tops up when back |
| Three weeks with no new revocation | Fixed: the CRL Generator now re-issues by `nextCrl` |
| A standing 10,000-entry CRL | Yes after a fix: about 33 B an entry, 336 kB a download |
| Revoked liars keep transmitting | Yes: refused by vehicles of both providers and by RSUs |
| A provider's root removed mid-run | Fixed (devices now install newer trust lists); two items stay open |
| Thanksgiving week | Yes: decided Wednesday, published Friday 10:00 EST |
| No RSU and no cellular | Yes: signing continues from the factory pool; nothing reaches the SCMS |
| Post-quantum at density | **Open**: not run |
| 30 % liars, many cases at once | Yes: 10 cases, 10 decisions, 3 CRL versions |

## 4. Runtime realism against the real US deployment

The Midtown benchmark (95 devices, two days on the real calendar) was compared with the
published US numbers.

| Number | Real world | Simulator |
|---|---|---|
| Provisioning requests per device | 1 over its life | 1 (median) |
| Fleet request rate, scaled to 17 M vehicles | about 0.5 per second | 0.54 per second |
| Weekly top-off | 20 certificates, one file a week | 20 each, 1,100 for 55 devices |
| Message sizes (bytes) | response 454–470, request 385–424, ack 292–294 | 454–470, 385–422, 292–294 |
| CRL size | 328 B with one entry, about 33 B more an entry | 328 B and 361 B |
| Report shuffle | 10,000 reports or 1 day | released at 24.0 h |
| MA decision | shuffle + 8 h review | 32.0 h |
| CRL publication | next business-day 10:00 slot | Wednesday 10:00 EDT |
| Separation of duties | no component links a device to its pseudonyms | 0 violations; sealed messages opened only by their recipient |
| Honest devices revoked | 0 | 0 (3 of 3 attackers revoked) |

Under the strict US profile the MA revoked 4 honest vehicles on 1 m position jumps from GNSS
noise (R-USFP). That is the cost of the strict thresholds, not a code defect.

## 5. Bugs fixed

1. The MA ran one case at a time whatever its capacity; cases now run side by side
   (also realism R-SERIAL).
2. "Enforced by n of N" could read "88 of 74"; N no longer falls as vehicles leave.
3. The PKI records showed a LOP the US deployment does not run (realism R-LOP), and a
   shared report as a device talking to the other provider's MA (R-SHARE).
4. No device retry after a lost request, so an RA outage lost top-ups.
5. The CRL Generator signed only when an entry was added, so an idle list went stale.
6. CRL downloads were charged an estimate; they are now charged the real encoded size.
7. Misbehaviour reports could be signed with an expired pseudonym across the rollover.
8. A node's clock offset never reached its security checks.
9. Devices never installed a newer trust list.
10. A provider off the trust list made the run panic (R-OFFCTL).
11. Fast-forward stopped at every 100 ms step after the first CRL: 255,658 backend stops,
    now 4,290, with the same results (R-FF).
12. The case board stamped "reports in" and "corroborated" at the decision, hiding the
    8 h review (R-CASE, R-CORR).
13. Fast-forward recordings failed their own read-back check.
14. Refusals by vehicles that had left were dropped from the report.
15. The RA did not answer a request it could not open (merge fix, 57c9e659).
16. After the merge, the PKI records lost the pseudonym batch files (1,280 of 1,286 in the
    conformance run): the LOP fold dropped the only leg that carried them. The device leg
    now carries them too (merge fix, 3d1ce391).
17. After the merge, `pki.state` listed each peer provider's components twice: two tracks
    had fixed peer naming in two places (merge fix, 3d1ce391).

## 6. Still open

- **R-TRUSTLEAK (security).** With provider-b off the trust list, provider-a receivers still
  verified 7 of its messages. Not root-caused. This should come first.
- The LA and ACA do not resend their answers to an RA that was down (edge 3d).
- A trust-list publication due during fast-forward waits for the next backend tick.
- One CRL a device cannot verify makes it refuse the whole frame.
- A device does not re-check the trust list's elector signatures.
- Clock skew applies to vehicles only, as a fixed offset.
- Post-quantum certificates at density were not run (edge 11).
- A small validated-certificate cache adds 12 % events; the cause is not shown.
- Verify-on-demand blinds misbehaviour detection: a policy question for the owner.
- The corroboration rule decided only about half the reported attackers within 54 h.
- Case evidence reads `verified: false` under modelled crypto (R-EVID); RSU-filed reports
  have no pseudonym to sign with (R-UNSEC); device CRL downloads are missing from the PKI
  records (R-CRLFETCH).
- US-profile false positives need a filtered-position GNSS model before the rate is trusted.
- The full quarter benchmark has not been re-run on main.
- The Studio's PKI fixtures (`ui/apps/studio/test/fixtures/pki/`) were recorded before the
  LOP fold; they still pass, but they no longer match what a run records now.
- The full `scms_knobs_runs` suite (36 minutes) ran before the two merge fixes, not after.

## BUILD-STATUS (2026-10-21, main 3c64d514 plus this report)

- `cargo check --workspace --tests`: clean (warnings only).
- `cargo test` for v2xw-sec, v2xw-msg, v2xw-proto, v2xw-node and v2xw-server: green. One
  server test still edited `metrics: [pdr]`, which the scenario had dropped before the
  merges; fixed (b97ad44d).
- v2xw-engine SCMS suites (phase2, pki_view, conformance, rsu_certs, calendar,
  credential_handoff, scms_*, sensitivity_expectations fast tests): green after 3d1ce391.
  Conformance and pki_view were rerun after the fix; the new twin check was proved red by
  removing the device-leg copy.
- Regression digests re-blessed: `phase2-path` and `i-s1-phase2-60s`. Frame digests are
  unchanged; only the PKI records moved (the LOP fold, peer naming, device-leg batch files).
  A second run reproduced both.
- `cargo fmt --all`: its own commit (3c64d514).
- Release build `cargo build --release -j 2 -p v2xw-cli --bins`: done in 8 m 26 s.
  `V2XW_PORT=8758 ./run-sim.sh` boots; `GET /agent/status` answers 200.
- Cleanup: the four `wf_1faba59c-7f5-*` worktrees and their branches, `resp-squash` and
  `wip/scms-knobs` removed, and `target/agents` deleted. `wip/scms-response` is kept: it is
  an older uncompiled snapshot that resp-squash replaced, and it is not part of main.
