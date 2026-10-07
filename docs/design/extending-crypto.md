# Extending the cryptography: signature schemes, KEMs and threshold protocols

Status: built 2026-10-07. Code: `crates/v2xw-sec/src/scheme/`, `crates/v2xw-proto/src/threshold/`,
`crates/v2xw-proto/src/hybrid.rs`, `crates/v2xw-proto/src/fault.rs`,
`crates/v2xw-engine/src/crypto_config.rs`.

This is the guide for adding a post-quantum signature, a KEM, or an interactive
multi-party protocol (FROST-like, threshold ECDSA, an umbrella scheme, a threshold
ML-DSA) to the simulator. Each is **one module and one registry line**. Nothing in the
engine, the node runtime or the credential protocols is edited.

---

## 1. The two extension points

| You are adding | You write | You register in | It becomes |
|---|---|---|---|
| A signature scheme (classical, hybrid or pure post-quantum) | a `const SignatureScheme` | `SIGNATURE_SCHEMES`, `crates/v2xw-sec/src/scheme/mod.rs` | a `security.signature` value |
| A key-encapsulation scheme | a `const KemScheme` | `KEM_SCHEMES`, same file | a `security.kem` value |
| A primitive the catalogue lacks | a `fn() -> PrimitiveDescriptor` beside the scheme, named in `defines` | nothing more | a catalogue entry with cited sizes and cost rows |
| An interactive multi-party protocol | an `impl ThresholdProtocol` and a `ProtocolEntry` | `THRESHOLD_PROTOCOLS`, `crates/v2xw-proto/src/threshold/mod.rs` | a `security.threshold.protocol` value |

Registration alone makes the thing:

* **selectable in the scenario** — the loader validates `security.signature`,
  `security.kem` and `security.threshold.protocol` against compile-time lists built from
  the registries (`SIGNATURE_IDS`, `KEM_IDS`, `THRESHOLD_IDS`);
* **selectable in the settings window** — the published schema's choice sets
  (`crates/v2xw-engine/src/scenario/validate.rs` `CHOICES`) are those same lists, and
  the page renders choice sets as pickers;
* **visible in the metrics and recordings** — the run report carries
  `phase2.signature_scheme`, `signature_mode`, `kem_scheme`, `phase2.committees.<role>`
  and `committee_failures`; the backend view (`backend.state`) names the signature and
  shows one entity per committee with its sessions, rounds, bytes, restarts and latency;
* **used by the credential protocols** — `v2xw_proto::hybrid::HybridScheme::from_signature`
  and `KemSpec::from_kem` read the registry, so the SCMS and the CCMS size every
  certificate and message and charge every operation from the scheme's parts; the SCMS
  runs its PCA, CRL Generator and linkage release on any registered threshold protocol
  through the protocol-agnostic session engine.

## 2. Adding a signature scheme — worked example

The worked example is `crates/v2xw-sec/src/scheme/hybrid_mldsa65.rs`: ECDSA P-256 +
ML-DSA-65, made by copying the template. Every step below is what was done to make it.

### Step 1 — copy the template

```sh
cp crates/v2xw-sec/src/scheme/template.rs crates/v2xw-sec/src/scheme/hybrid_mldsa65.rs
```

`template.rs` is compiled (so it cannot rot) and deliberately not registered; a test
checks it stays unregistered.

### Step 2 — fill in the `const`

```rust
pub const SCHEME: SignatureScheme = SignatureScheme {
    id: "hybrid-mldsa65-ecdsa-p256",          // what a scenario writes; recordings carry it
    label: "Hybrid: ECDSA P-256 + ML-DSA-65", // what the settings window shows
    mode: SchemeMode::Hybrid,                 // must match the parts (a test checks)
    parts: &[
        ECDSA_HALF,                            // primitive/ecdsa-p256-sha256, ops "ecdsa-p256-*"
        Part {
            primitive: PrimitiveId::ML_DSA_65, // sizes and cost anchors from the catalogue
            op_prefix: "ml-dsa-65",            // hardware-profile ops "ml-dsa-65-sign" etc.
            post_quantum: true,
        },
    ],
    standard: "FIPS 204 (ML-DSA-65) + IEEE 1609.2 ECDSA P-256, concatenated",
    real_crypto: false,                        // only ECDSA P-256 runs in real mode
    defines: None,                             // ML-DSA-65 is already in the catalogue
    notes: "…",
};
```

What the simulator derives from `parts`, so you do not write it:

* **octets on the air** — `extra_octets()`: the parts' encoded signatures concatenated,
  minus the 66-byte ECDSA baseline the node encodes, per signature; plus the parts' keys
  minus the 33-byte baseline key, per attached certificate. For this scheme: +3,309 per
  signature and +1,952 + 3,309 per certificate (FIPS 204 Table 2). A pure scheme
  *replaces* the classical fields: ML-DSA-44 alone is +2,354 / +3,633.
* **node operations** — a multi-part scheme signs with the compound op
  `hybrid-mldsa65-ecdsa-p256-sign`, which a hardware profile prices as the sum of
  `ecdsa-p256-sign` and `ml-dsa-65-sign`. A single-part scheme signs with its part's op.
* **credential-system charges** — the post-quantum part is charged to authorities against
  the Raspberry Pi 5 liboqs anchor and to devices against whichever Cohda MK6 library
  prices it (`hybrid::DEVICE_PQ_PROFILES`); a hybrid also pays the ECDSA half, a pure scheme
  does not; neither has a butterfly, so the device generates and uploads one post-quantum
  key per pseudonym certificate.
* **the certificate cycle** — `fragmenter/cert-cycle-partial-hybrid` carries this scheme's
  certificate (NDSS 2024's 858-byte Falcon hybrid plus the difference in certificate growth).

### Step 3 — register

In `crates/v2xw-sec/src/scheme/mod.rs`:

```rust
pub mod hybrid_mldsa65;                    // with the other modules
…
pub const SIGNATURE_SCHEMES: &[&SignatureScheme] = &[
    …
    &hybrid_mldsa65::SCHEME,               // one line
    …
];
```

### Step 4 — test

```sh
heavy.sh env CARGO_TARGET_DIR=… cargo test -p v2xw-sec scheme
```

The registry tests check that the id is unique, the mode matches the parts, every part is
in the catalogue, and **every post-quantum part has a sign and a verify time** on the
backend anchor and on the Cohda MK6 — published or derived, never invented. The engine
test `crates/v2xw-engine/tests/pq_threshold.rs` checks the worked example end to end: it
is offered by the published schema, a scenario naming it validates and runs, the node
charges both halves, the air carries its octets and the credential system its sizes.

### When the primitive is new

If your scheme uses a primitive `v2xw_sec::primitive` does not have (a HAWK, a MAYO, an
SQIsign), write its descriptor in the same module and name it in `defines`:

```rust
pub const MY_PRIMITIVE: PrimitiveId = PrimitiveId("primitive/my-scheme-1");

pub fn my_primitive() -> PrimitiveDescriptor {
    PrimitiveDescriptor {
        id: MY_PRIMITIVE,
        family: PrimitiveFamily::Signature,
        pk_bytes: …, sk_bytes: …,                       // from the specification, cited
        sig_bytes: SizeSpec::Fixed(…),                   // or Variable { mean, max }
        security_level: …,                               // NIST category
        cost: CostTable::new(vec![ /* one CostRow per published benchmark, each cited */ ]),
        cost_ratio: Some(CostRatio { base: …, reference: … }), // or None; see below
        sources: vec![ … ], notes: vec![ … ],
        ..
    }
}
```

The catalogue appends every registered scheme's `defines` (`scheme::contributed_primitives`).

**Costs you cannot find.** Do not invent a number. Three honest routes exist, in order:

1. `cost_proxy` — another primitive whose cost *is* this one's (ECQV reconstruction is
   one ECDSA-verify-sized point multiplication).
2. `cost_components` — a composite whose cost is the sum of published parts.
3. `cost_ratio` — scale a sibling's published per-profile cost by the two primitives'
   cycle ratio on a platform that publishes both. This is how ML-DSA-65/87 get Cortex-A
   and Cohda figures from ML-DSA-44 (`pqm4` Cortex-M4 cycles), Falcon-1024 from Falcon-512
   (Cortex-M7), and ML-KEM-1024 from ML-KEM-768. `cost_is_derived()` reports every such
   figure as derived, and `HardwareProfile::op_cost_is_derived()` does the same on a device.

A hardware profile that publishes no figure for an operation falls back to the catalogue
anchor it names in `crypto_anchors` (the Cortex-A72 automotive SoC names the Raspberry Pi
4 liboqs anchor; the Cohda MK6c names the MK6 liboqs and Botan anchors), so a new
scheme is priced on existing devices without editing their YAML.

## 3. Adding a KEM

Same pattern, `crates/v2xw-sec/src/scheme/kem.rs`:

```rust
pub const ML_KEM_768: KemScheme = KemScheme {
    id: "ml-kem-768", label: "Post-quantum: ML-KEM-768",
    mode: SchemeMode::PostQuantum,
    ecies: false,                                  // true for a hybrid: both encapsulations travel
    post_quantum: Some(PrimitiveId::ML_KEM_768),   // ciphertext size and costs from the catalogue
    standard: "FIPS 203 (2024), ML-KEM-768", notes: "…",
};
```

Add it to `KEM_SCHEMES`. The SCMS then sizes every encrypted-key structure as
`(33 if ecies) + ciphertext + 32` instead of IEEE 1609.2's 65-byte `EciesP256EncryptedKey`,
and charges an encapsulation or decapsulation wherever it encrypted or decrypted
(`Outbox::charge_kem`). For a KEM, a `CostRow`'s `sign` is encapsulation and `verify` is
decapsulation.

## 4. Adding an interactive multi-party protocol — worked example

Copy `crates/v2xw-proto/src/threshold/template.rs`; implement `ThresholdProtocol`; add
your `ENTRY` to `THRESHOLD_PROTOCOLS`. The required methods are the specification's
numbers:

| Method | What it says | FROST's answer (`threshold/frost.rs`) |
|---|---|---|
| `sign_rounds(signers, batch)` | the online rounds, coordinator ↔ participant, each a `StarRound` | round 1 "commit": response `B × 66 B`, 2B scalar mults per member; round 2 "sign": request `B × (32 + s × 98) B`, response `B × 32 B`, s mults per member, `4s` per message at the coordinator, `verifies_shares: true` |
| `dkg_rounds(committee)` | peer-to-peer key generation, for the report | Komlo–Goldberg: a broadcast of `(t+1) × 33 + 65 B`, then a 32 B share to each other member |
| `refresh_rounds(committee)` | proactive refresh | Herzberg et al.: a sharing of zero, same shape |
| `signature_bytes`, `verification_key_bytes` | output sizes | 65 B (`R ‖ z`), 33 B |
| `verifies_as` | what a verifier checks it as | "Schnorr over P-256 — not an IEEE 1609.2 signature" |
| `identifiable_abort` | can a bad share be attributed? | yes, when the coordinator verifies shares (RFC 9591 §5.4) |
| `knowledge` | what each party learns | coordinator: message, commitments, shares; ≤ t coalition: nothing |

A `StarRound` names its request and response **payloads** (the session adds the
envelope every backend message carries), the operations each participant computes
(`member_ops`, priced from the catalogue on the member's hardware profile), an optional
`member_time` for work only ever measured as a unit (CGGMP21's Paillier presigning, from a
cited benchmark), the coordinator's operations once the round is in, and whether this is
the round where shares are verified — the round where a malicious member is caught.
`batch` is how many messages one session signs; scale per-message fields by it.

### What the session engine does with it (`threshold/session.rs`)

You do not write any of this; it is the same for every protocol:

* **Rounds over the network.** Every request and response crosses a coordinator–member
  link (`security.threshold.member_latency_ms`, the members being separate
  organisations) and pays its bytes; every member's compute is charged to its own service
  queue; a round ends only when the coordinator holds a quorum of valid responses.
* **Selection.** With `ask_all` the coordinator asks every live member in round one and
  fixes the signing set to the first quorum to answer; otherwise it asks exactly a quorum.
* **Slow** members (`actors.backend.failures[].mode: slow`) pay extra service time per
  message: with `ask_all` the others carry the round.
* **Offline** members have their deliveries dropped; the round timer
  (`round_timeout_ms`) fires, the silent are excluded, the session restarts.
* **Malicious** members return invalid shares; in a `verifies_shares` round the
  coordinator detects it, excludes the member (identifiable abort) or rotates the signing
  set (no attribution), and restarts.
* **Too few** live members: the session waits `retry_after_s` with nobody excluded and
  tries again, up to ten attempts, then fails — and the host is told, so a certificate
  batch is never issued, a CRL never signed, a revocation never resolved, and
  `committee_failures` counts it.

### Where the SCMS uses a protocol

`security.threshold.roles` puts committees in place of single authorities:

| Role | Replaces | One session signs | Continuation |
|---|---|---|---|
| `pca` | the PCA's signature on each pseudonym certificate | one i-period batch (`batch` = certificates; one per umbrella under `umbrella`) | the batch goes to the RA |
| `crl` | the CRL Generator's signature | one CRL version | the list is published |
| `linkage` | two-of-two Linkage Authorities at revocation | both chains' seeds (`shamir-feldman`, batch 2) | the CRL entry is appended |

A different host protocol (the ETSI PKI, an umbrella-PQ system) uses the same
`Committees::start` / `Committees::handle` pair: wrap `CommitteeMsg` in its message type,
start a session from the coordinator's handler with a continuation, and act on the
`Outcome` it gets back.

## 5. What ships

**Signature schemes** (`security.signature`): `ecdsa-p256`, `ecdsa-brainpoolp256r1`,
`ecdsa-p384`; hybrids `hybrid-falcon512-ecdsa-p256`, `hybrid-mldsa44-ecdsa-p256`,
`hybrid-mldsa65-ecdsa-p256`; pure `ml-dsa-44`, `ml-dsa-65`, `ml-dsa-87`, `falcon-512`,
`falcon-1024`, `slh-dsa-sha2-128s`, `slh-dsa-sha2-128f`.

**KEMs** (`security.kem`): `ecies-p256`, `ml-kem-512`, `ml-kem-768`, `ml-kem-1024`,
`hybrid-ecies-p256-ml-kem-768`.

**Threshold protocols** (`security.threshold.protocol`): `frost-p256` (RFC 9591),
`cggmp21-ecdsa` (CGGMP21, measured costs), `umbrella` (05-protocols.md §5.4's placeholder
binding over any inner protocol), `shamir-feldman` (linkage release; used by the
`linkage` role).

### Sources for the numbers added in this build

* ML-KEM-512/768/1024 Cortex-M4 cycles: pqm4 `benchmarks.md`, `m4fspeed` (fetched
  2026-10-07): keygen/encaps/decaps 392,423/390,881/428,167; 642,096/658,754/707,827;
  1,018,976/1,031,565/1,094,008.
* ML-KEM-512/768 on Raspberry Pi 4 and 5: arXiv 2503.10238 Table 8 (the same table the
  ML-DSA-44 and Falcon-512 rows cite): Pi 4 3,807.3/3,462.0/2,783.0 and
  2,539.0/3,905.0/2,571.5 per second; Pi 5 23,168.3/19,178.0/15,136.3 and
  13,976.6/11,900.6/9,614.3 per second.
* FROST sizes: RFC 9591 §5 and §6.4 (FROST(P-256, SHA-256): Ne = 33, Ns = 32).
* FROST DKG: Komlo & Goldberg, SAC 2020, Figure 1. Refresh: Herzberg et al., CRYPTO 1995.
* CGGMP21: ePrint 2021/060 Table 1 (round structure, κ online share, setup sizes);
  measured bytes and time from Silence Laboratories' "Presign calculator — DKLs23 vs
  CGGMP21" (2-of-3, GCP n2d, 3072-bit Paillier; fetched 2026-10-07): ≈ 88 KiB one-way per
  presignature aggregate, 2,646 ms single-session presign at 4 vCPU, ~26 ms p50 online.
* Shamir 1979, Feldman 1987 for linkage release.

### Assumptions, stated

* **Umbrella.** The owner's umbrella-thresholding specification is still `[FILL IN]` in
  the brief (§8.3). `threshold/umbrella.rs` implements 05-protocols.md §5.4's placeholder
  binding: a t-of-n committee signs one umbrella credential per `k` leaf pseudonyms, the
  device derives and certifies its leaves locally, revocation is one entry per umbrella,
  and the `k` leaves under one umbrella are linkable whenever it travels with them. When
  the specification arrives it replaces that one module.
* **t-of-n linkage** uses a dealer: each chain's per-period seeds are Shamir-shared when
  the chain is allocated, which keeps forward-only revocation. A dealer-free version needs
  a distributed PRF per period — more rounds, same interface.
* **Committee messages** carry the deployment's digest-signed envelope (93 B under ECDSA,
  plus the post-quantum signature under a post-quantum scheme). FROST signers sign the
  32-byte digest of the to-be-signed structure.
* **CGGMP21 is run as a star** through the coordinator, which relays each round's
  peer-to-peer messages: twice the hops and the relayed bytes of a full mesh.
* **The real ECDSA signature** the SCMS computes for its CRL and certificates stands in for
  the committee's output; the committee's rounds, bytes, time and failures are what is
  modelled. (A CGGMP21 output *is* an ECDSA signature; a FROST one is a Schnorr signature
  whose size the model carries.)
