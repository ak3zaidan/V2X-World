# Fix wave 3, package 1: R-CCSIG, per-node memory, RSU CRL under PQ (2026-10-22)

Branch from main `77cffb5b`. Three items from `final-bench.md` and `pq-profile.md` §3.

## 1. R-CCSIG: the certificate cycle signs messages with ECDSA (fixed)

**Cause** (`final-bench.md` §3). Under `fragmenter/cert-cycle-partial-hybrid` the engine
sized, signed and priced every SPDU with the scenario's hybrid scheme, and `45f0942f` had
shifted the cycle's fragment budget to fit beside that signature. The design the fragmenter
cites (Twardokus et al., NDSS 2024, Partially-Hybrid, §III and Fig. 4) keeps ECDSA on every
BSM: frames 2–5 of a cycle are 204 B.

**Fix.**
- `v2xw_engine::signature::partially_hybrid` names the mode: the cycle plus a hybrid
  scheme. `message_signature` gives the scheme a node's own V2X messages use. In this mode
  it is the hybrid's classical half (`ecdsa-p256`). It covers sizing (`hand_down_app`, the
  Sybil ghosts), the node's sign and verify operations (`wiring`), the RSU signing check,
  the HSM lower edge and `signing_facts` (mode `partially-hybrid`).
- The cycle's fragment budget is the model's own again (MTU less the 204 B base frame),
  which reverses `45f0942f`. The fragments are equal across a whole cycle.
- Some things keep the full scheme: the credential system and its backend messages, the
  CRL a unit relays (the CRL Generator's signed object), a relayed misbehaviour report
  bound for the MA, and P2PCD. The R-PQMBR refusal test therefore still holds under the
  cycle.
- A receiver pays the hybrid's post-quantum verification once for each certificate it
  learns (`NodeConfig::cert_verify_op`, e.g. `ml-dsa-44-verify`). The cost is priced on
  its profile as message verification is. It is counted in
  `receiver_validation.cert_pq_verifications`, which is serialised only when non-zero.
- The validator refuses two cases:
  - the cycle with a purely post-quantum scheme, which has no classical half to sign
    with;
  - a certificate the cycle cannot carry in τ fragments under the MTU (`alpha()` error).

**Tests** (`tests/cert_cycle_signature.rs`, on the Phase 1 grid under hybrid ML-DSA-44):
- no MTU refusal;
- BSMs without a fragment are under 450 B, and the first three of each cycle carry about
  1,009 B;
- certificate groups have 3 fragments and 3,027 B;
- every `node.verify` uses `ecdsa-p256-verify`;
- the PQ certificate checks are > 0 and fewer than the verifications.

A second test covers the validator refusals.

**Digests.** Only certificate-cycle runs move: the PQHFC/PQHMC rows and `PQHM.determinism`'s
PQHMC digest `f2afcf0b…`. No pinned regression digest uses the cycle.

## 2. Per-node memory (partly fixed; the full-scale target is not met)

**Profile.** These are live-heap snapshots (`MallocStackLogging=lite`, `malloc_history
-callTree -invert`) of the release binary of `77cffb5b`, grouped by the nearest v2xw frame.
The scripts are in the scratchpad `ccm/heap.py` and `ccm/prof.sh`.

| Run | Nodes | Live heap | Footprint |
|---|---|---|---|
| quarter, 50 s | 288 | 184 MB | 228 MB |
| quarter, 140 s | 406 | 258 MB | 411 MB (peak 455) |
| full, 20 s | 783 | 446 MB | 618 MB |
| full, 30 s | 900 | 525 MB | 824 MB |

Where the live heap goes at full scale, 30 s:
- **Credential copies, about 45 %:**
  - `Phase2::creds_of_device` deep-cloned each pseudonym certificate and the issuing
    ACA certificate for every one of a device's 40 credentials (61 MB);
  - the backend's issued records (50 MB) and the RA's filed `AcaEeCertResponsePrivateSpdu`
    batch files (34 MB);
  - the governance certificate maps (30 MB);
  - the node's imported keys (23 MB).
- **In-flight frames,** `Engine::start_frame` (53 MB): per-frame arrival and faint maps
  for every receiver.
- **Per-node runtime and detectors:**
  - a cloned hardware profile per node (17 MB);
  - the evidence buffers and delivered messages (21 MB).

No collection grows without bound with time at quarter scale. Most of the growth from
20 s to 30 s at full scale is not in the live heap: the footprint grew 206 MB and the live
heap 79 MB. The rest is allocator retention after transient peaks, such as the per-step
reception fan-out and the 10 s metric windows.

**Bounded or de-duplicated here (results unchanged):**
- The ACA certificate is shared once per device batch rather than cloned 40 times.
- `ImplicitCredential::certificate` is an `Arc`, and the engine's ground-truth credential
  shares it.
- `Phase2::checked_at` is not written under the US profile, which never reads it.
- `UsSmMbr` evicts a sender not heard for 30 s, swept every 1,024 inserts. This is an
  ASSUMPTION about a device limit: at least 10× `max_gap_s`, so no consecutive check loses
  its predecessor.

**Still open:** see §4 for the measurement. Getting under 2.5 GB at full scale needs work
on the transient peaks and allocator retention:
- per-step buffers reused rather than reallocated;
- latency windows as a sketch rather than every sample;
- RA batch files dropped after a confirmed download, which changes re-download behaviour;
- a shared `Arc<HardwareProfile>`.

## 3. RSUs learn the CRL under the post-quantum profile (fixed, R-PQRSUCRL)

Under PQ, `ScmsState::pki` is `None`, so a unit cannot join the application-certificate
cycle. Until now `rsu_crls_due` returned nothing for it, and it never refused a revoked
sender.
- A unit outside the cycle under a CAMP SCMS with no PDU layer is now tracked in
  `Phase2::rsu_crl_modelled`.
- It fetches the composite CRL over its backhaul whenever the RA serves a newer version
  or serial, as a vehicle does, and the bytes ride its line.
- It installs the entries into its gate as the vehicles' modelled CRL does, and it is
  counted in `rsu_crls_installed`.

The test is `pq_relay.rs::under_the_post_quantum_profile_a_roadside_unit_fetches_the_crl_and_refuses_the_revoked`.
P-256 runs are unchanged, because the path is taken only when `pki` is `None`.

## 4. Validation

See the report of this package for the test runs, the injected faults and the measured
runs.
