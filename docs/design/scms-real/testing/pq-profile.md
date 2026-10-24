# Report uploads and the post-quantum profile: what the engine does and cannot do

Report and PQ fix package, 2026-10-22, on `176d6f6b`. It covers OF-2
(`oracle-final.md`), R-PQMBR (`stress.md` §3) and the open PQ rows of `matrix.md`
("no top-off and no RSU CRL under the hybrid scheme").

## 1. OF-2: every vehicle upload carries its SPDUs (fixed)

**Cause.** A detection the device could not build an SM-MBR v1.1 report for still went to
the MA on the modelled path, as a 1,119-octet report with no bytes. Three reasons led
there: no v1.1 subtype maps its checks (the research suite), too few of the accused's
signed BSMs were heard, or the accused certificate could not be named. In the US
deployment such a report cannot exist, because an upload *is* its bytes ([SM-MBR] §3–4).

**Fix.** `Phase2::files_report` makes the check before the access leg. A detection that
cannot be filed is never uploaded. It is counted in `reports_not_filed`, with the reason
in `reports_not_filed_by`. `report_at_proxy` catches the one case that can change on the
way, a failed securing, in the same way. The modelled path is now taken only:

- with no PDU layer (modelled cryptography, the PQ profile in §3);
- on the ETSI path;
- for a roadside unit's own reports. This is R-UNSEC, still open: a unit holds no
  pseudonym to sign one with, so these reports stay sized-only and are labelled apart;
- when the new lifecycle key `modelled_reports: 1` is set. It is the research setting,
  not the US one, and it reverses this design note: `validation/vehicle-and-roadside.md`
  M2 had kept such reports flowing for studies of the research detectors.

**Tests.**

- `oracle_final.rs::every_cellular_report_upload_carries_its_spdus` (no longer ignored).
- `report_filing.rs`. Default: 0 cellular uploads without SPDUs, and `reports_not_filed`
  > 0 with every reason recorded. `modelled_reports: 1`: the sized-only uploads come back.

**Effect.** In research-suite runs, unmappable detections (for example Sybil and replay)
no longer reach the MA. That is the real US behaviour. A study of those detectors sets
`modelled_reports: 1`.

## 2. R-PQMBR: oversized relayed reports and roadside CRL frames (fixed)

**Cause.** Under a post-quantum signature, two of the engine's own application frames are
signed SDUs larger than the 1,400-octet WSMP MTU:

- a relay-only vehicle's report to an RSU (Falcon-512 hybrid with its certificate:
  1,119 + 2,229 octets);
- the RSU's CRL frame.

Both kept `fragmenter/none`'s rule. Every one was refused at the MTU, and every one was
still counted as uploaded or broadcast. On `scms-pq-relay-repro.yaml`, the main build
counted 1,251 relay uploads and 14,280 CRL broadcasts, against 15,531 MTU refusals.

**Fix (`run.rs`).**

- `hand_down_app` now says what became of a frame (`HandDown`).
- A frame counts as an upload or a broadcast only once it is on the air. A refused frame
  is counted in `reports_refused_mtu` or `crl_frames_refused_mtu` and is not retried,
  because the stack would refuse the same SDU every time.
- With `fragmenter/generic-sdu`, an oversized application frame is split like any SDU. The
  RSU or the vehicle acts on it once it is reassembled (`resolve_group`).
- A frame that fits is never split, so P-256 runs are unchanged.
- With `fragmenter/none` (IEEE 1609.3 WSMP has no fragmentation), with facilities
  segmentation, or with the certificate cycle, the frame is refused and counted. That
  loss is a real finding about PQ over WSMP. A relay-only vehicle cannot report at all.

**Tests (`pq_relay.rs`, 120 s live of the hybrid matrix run).**

- With generic-sdu: 616 relay uploads, 391 vehicle reports at the proxy through an RSU,
  2,148 report and 16,595 CRL-frame reassemblies, 14,700 CRL broadcasts, and 0 frames
  refused at the MTU. These are the 2026-10-22 numbers, before the wave-3 GNSS fix and
  before the repro gained its own reporter; the current ones are below.
- **Current repro (re-run 2026-10-24 on `81a12183`, release binary, `rerun-2026-10-24-b.md`
  §2).** Since fix wave 3 (R-USFP's smoothing receiver, R-UNSEC) the base's honest traffic
  no longer feeds the MA, and finalize 4 added one ConstPos attacker (actor 112) on the road
  of relay-only node 56 (06 §6 item 9). The scenario now has 4 attackers, 113 nodes (18
  relay-only vehicles) and, in 120 s: **3 MA revoke decisions, 3 attackers revoked, 0
  honest**; 36 relay uploads and 182 cellular uploads; 346 reports received, 345 at the MA,
  3 cases; 164 reports over the relay leg of which 135 are the units' own (R-PQUNSEC, 06 §6
  item 10), so 29 vehicle reports reached the proxy through a unit; 0 reports and 0 CRL
  frames refused at the MTU; 1,140 CRL broadcasts; 150 RSU CRL installs and 4,382 revoked
  receptions refused at units (8,678 fleet-wide); 7,669 SDUs fragmented into 17,654
  fragments, 21,097 reassemblies complete and 133,551 lost. 38.7 s wall, 120 MB. The
  fourth attacker (node 46) is never reported (0 reports), which is the handset gap of
  06 §6 item 14. Rows `RR.RELAY.*` in `scms-variants.yaml`.
- With the certificate cycle, which splits only the certificate: 0 relay uploads and 0
  CRL broadcasts are counted, and the refusals are > 0. This variant is the one that
  shows the refusal. Under `fragmenter/none` the hybrid BSM with its certificate is
  itself refused, so nothing is detected, and no report or revocation exists to test.

## 3. What the PQ profile cannot do (documented, not fixed)

With `security.signature` set to a hybrid or pure post-quantum scheme, or `security.kem`
set to a post-quantum KEM, `ScmsState::pki` is `None` (`v2xw-proto` `run/mod.rs`).
IEEE 1609.2-2022 / 1609.2.1-2022 define no PQ certificate, signature or SPDU encoding,
so the engine has nothing bit-exact to build. Under the PQ profile, therefore:

| Capability | P-256 (US profile) | PQ profile | Why |
|---|---|---|---|
| Real 1609.2.1 SPDUs (requests, acks, batches, CRLs, CTLs, SM-MBR) | yes | **no**: sized and charged only | no standard encoding exists |
| SM-MBR reports | real, encrypted to the MA | **modelled** (MbrBuild::Off) | same |
| Receivers verify the signed CRL and its chain (`rv.crls_accepted`) | yes | **no**: entries checked against the CRL Generator's list | no signed CRL bytes |
| RSU application-certificate cycle (`rsus_in_cert_cycle`), and with it the RSU's own CRL fetch (`rsu_crls_installed`) | yes | **no**: refused with `AppEventKind::NoPki` | the flow builds real 1609.2.1 PDUs |
| RSU CRL broadcast to vehicles | yes | yes, fragmented under generic-sdu (§2); refused under `none` | — |
| Butterfly pre-generation (`.info` top-off, one request per device) | yes | **no**: every top-up is a fresh request with fresh PQ keys | no PQ butterfly key expansion |
| Top-up certificates delivered | at the next download | yes, **after the RA's request shuffle** (1,000 requests or the 2-day age) | — |

**Top-ups are complete, not missing.** `matrix.md` reported 0 certificates topped up under
the hybrid scheme, against 1,100 under P-256. This is the 2-day shuffle, not a broken flow:

- The 55 top-ups start at the 614 → 615 rollover (about 43,200 s).
- Below the shuffle's 1,000-request count, each one waits the whole 2-day window. That
  is about 216,000 s, past the run's 172,800 s horizon.
- Same scenario with `cert_shuffle_window_s: 3600` (main build): 55 of 55 complete,
  1,100 certificates.
- The proto test `hybrid_credentials.rs::a_hybrid_top_up_is_installed_after_the_request_shuffle`
  pins the flow: a hybrid top-up's period is installed no earlier than one shuffle window
  after the request.

The 2-week pool absorbs the delay (`vehicles_starved` 0). This latency is a real cost of
having no PQ butterfly.

**Still owed:**

- a PQ RSU certificate cycle;
- (done in fix wave 3, `ccsig-memory.md` §3) a modelled CRL fetch for RSUs under PQ, so
  that RSUs learn revocations (until then
  `rsu_revoked_receptions` stays 0 under PQ);
- the hybrid ML-DSA and certificate-cycle runs (`stress.md` §6).
