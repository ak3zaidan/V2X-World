# Why a BSM shows "sign 9 ms, verify 0.4 ms"

A reviewer read a message in the Studio as "sign 9 ms, verify 0.4 ms" and said that is wrong
for ECDSA, where verification costs more than signing. He is right about ECDSA running on one
processor. The two figures in the panel came from different hardware, and neither is a
measurement. This note explains where each figure comes from, what the simulator now shows,
and how to study the case where verification costs more.

## Where each figure comes from

The default vehicle runs the reference OBU profile, `obu/unex-obu-301-craton2`
(`crates/v2xw-node/profiles/hardware/obu-unex-obu-301-craton2.yaml`).

| Operation | Runs on | Published figure | What the model used |
|---|---|---|---|
| ECDSA P-256 sign | CRATON2 eHSM, a Cortex-M0 secure element that holds the private key (FIPS 140-2 L3, CMVP #3556) | "<9 ms signing latency", ">110 signatures/s" [R7 §A4] | 9 ms: the vendor's **upper bound**, charged as a worst case |
| ECDSA P-256 verify | A separate on-chip line-rate verification engine, outside the eHSM boundary | ">2500 verifications/s"; no latency published [R7 §A4] | 0.4 ms = 1/2500 s: **derived** from a lower bound on the rate, for one engine |
| Software proxy, same algorithm, one Cortex-A53 @ 1.2 GHz | application processor | OpenSSL: 1631.1 sign/s, 775.3 verify/s [R7 §D8] | sign 613 µs, verify 1290 µs: verify ≈ 2.1 × sign |

So on this OBU, verification looks faster than signing for two reasons:

1. **Different hardware.** Signing needs the private key, so it stays inside the
   tamper-resistant element, which is a small and slow core. Verification needs no secret,
   so the chip hands it to a dedicated engine built for the full channel rate.
2. **The figures are bounds, not measurements.** "<9 ms" only says signing takes at most
   9 ms. ">2500/s" only says the engine sustains at least that rate. Neither is a typical
   latency.

Run in software on one processor, ECDSA behaves as the reviewer expects. With the
Cortex-A53 figures, verification costs about twice as much as signing.

## What changed in the simulator

- **Each figure states what kind of number it is.** Every crypto cost now carries a basis
  (`v2xw_node::profile::CostBasis`): `published`, `upper-bound`, `within-bound`,
  `derived-from-throughput`, `software-benchmark`, `software-proxy`, `catalogue` or
  `composite`. The panel prints it in plain words. For example: "at most 9.000 ms: the
  vendor publishes only an upper bound, and the model charges the bound (a worst case, not
  a measurement)".
- **Each figure states where it ran.** The message panel labels the signing time "signed
  in the secure element" and the verification stage "verified by the hardware accelerator".
  It also prints one paragraph on why the two compare as they do on this hardware. The
  vehicle HUD's security row reads "signs in the secure element, verifies on the hardware
  accelerator", and its tooltip gives the same explanation. These come from the node feed's
  new `crypto` member (`HardwareProfile::crypto_placement`).
- **A bound can be modelled as a range.** A profile operation can now state
  `latency_lower_us` beside an `upper-bound` latency. Each call is then drawn uniformly
  inside the bound from the node's own `ServiceTime` random stream, so a run still depends
  only on its seed. The reference profile does **not** set a lower edge, because no source
  publishes one for the CRATON2 eHSM. Inventing one would quietly set the signing load of
  every run. So the shipped model still charges the bound, and now says that it does.
- **A software-crypto profile.** `obu/unex-obu-301-craton2-software` is the same OBU with
  the security hardware set aside. Signing and verification both run in software on the
  application cores, using the flagged Cortex-A53 proxies (613 µs and 1290 µs). Use it to
  study verification-dominated cost, for example by setting `nodes.default_obu` in a
  scenario. `obu/generic-automotive-soc-no-hsm` (Cortex-A72: 244 µs and 645 µs) is the
  second software option.

## Placement in every OBU profile

| Profile | Sign | Verify |
|---|---|---|
| `obu/unex-obu-301-craton2` (reference) | secure element (eHSM) | hardware verification engine |
| `obu/unex-obu-301-craton2-software` | software, application processor | software, application processor |
| `obu/cohda-mk5` | secure element | hardware accelerator (baseband engine) |
| `obu/cohda-mk6c-qualcomm-9150` | secure element | not published: the profile marks it unknown, and the model places it with the HSM until it is measured |
| `obu/generic-automotive-soc-no-hsm` | software | software |
| `obu/pq-capable-hypothetical` | software | software |

## Sources

- [R7 §A4] Unex OBU-301E information sheet; Autotalks CRATON2/SECTON eHSM FIPS 140-2
  Security Policy, CMVP #3556.
- [R7 §D8] OpenSSL `speed ecdsap256` on a Raspberry Pi 3B (Cortex-A53 @ 1.2 GHz) and a
  Raspberry Pi 4B (Cortex-A72 @ 1.5 GHz).
- [R5 §B] Cortex-M4 P-256 cycle counts (sign 375k, verify 976k cycles), which show the same
  ratio on a microcontroller.

## WP-C4: every primitive that runs now costs time (2026-10-17)

Before WP-C4, SHA-256 cost zero everywhere, AES, CCM, ECDH and ECIES had no entries,
P-384 had none, and an ECQV reconstruction was charged as a whole ECDSA verification.
Now every operation the `v2xw-sec` modules execute is priced on every node hardware
profile that runs cryptography. `primitive::tests::every_executed_primitive_is_priced_on_every_device`
fails if any one is not, and `every_crypto_module_is_in_the_executed_registry` fails if a
new module appears that is not in `primitive::EXECUTED`.

**How a cost is formed** (`crates/v2xw-sec/src/primitive.rs`):

- **Per unit of work, not per call.** A hash is priced per compression call after FIPS
  180-4 padding (`(n + 9)/64` calls for SHA-256, `(n + 17)/128` for SHA-384, rounded up).
  AES is priced per block call. AES-128-CCM is priced as its exact AES-call count,
  `2·⌈n/16⌉ + 2`, which a test checks against an independent CCM that matches
  `ccm::encrypt` bit for bit. Published throughputs are kept with their chunk size; the
  conversion is done on the way out.
- **Recipes for the constructions no one benchmarks as a unit.** These are written down
  term by term from the standard:
  - ECIES (1609.2 §5.3.5): decrypt is one ECDH and six SHA-256 calls; encrypt adds one
    fixed-base multiplication.
  - ECQV reconstruction (02 §6): one variable-base multiplication (ECDH) and two hashes,
    no longer the ECDSA double multiplication.
  - The original butterfly expansion (6 AES calls, 2 fixed-base multiplications for the
    RA's side).
  - Linkage values (2 AES per `lv(i,j)`, 2 SHA-256 per seed step).
- **Per-node anchors** (`primitive::anchors::DEVICES`). No OBU vendor publishes SHA, AES,
  ECDH or ECIES figures. So each node profile names the published benchmark of the
  closest silicon, scaled by clock where the profile publishes one. For example, the
  CRATON2's 600 MHz Cortex-A7 is charged 2× the Pi 3B's 1.2 GHz armhf figures.
  ECDSA sign and verify still come from each node profile's own HSM or accelerator
  table first. The anchors are for what that table does not publish.
- **The verify plan** (`envelope.rs`) charges each hash by its real length. An unpriced
  hash is now `None`, not zero.

**Sources** (one per number, in each `CostRow`; all are in the research sheets or were
fetched for WP-C4):

- `openssl speed` gists for the Raspberry Pi 3B, 3B+ and 4 [R7 §D8]. The files' sha256 is
  recorded in the row citations. These give SHA-256, SHA-512 (used for SHA-384, which
  runs the same compression function), AES-128-CBC, ECDH P-256, and ECDSA P-384.
- The wolfSSL vs MbedTLS benchmark of 2026-06-26 [R5 §B.5] for the Pi 5, i9-11950H and
  STM32H563. It gives SHA-256, SHA-384, AES-GCM (used as an upper bound for raw AES)
  and ECDH.
- The OpenSSL Cookbook [R7 §F1] for x86 P-256 and P-384, and AES-NI.
- Nordic DevZone thread 46695 (nRF52840, Nordic engineer): SHA-256 over 4,123 B takes
  4.7413 ms in Oberon software. A CCM call of 20 AES calls takes 0.6879 ms in mbed TLS.
- Emill P256-Cortex-M4 [R5 §B]: ECDH takes 906k cycles.
- AURIX TC3xx HSM [R7 §D1]: SHA-256 takes 65 cycles per block.
- Thales Luna 7 per-tier rates. ECIES decrypt is 56,000 TPS across 8 A790 appliances,
  which is 7,000 per appliance (derived) [R7 §F2]. The T-5000 does 16,000 P-384
  operations per second.

**Resulting costs, µs** (`cargo test -p v2xw-sec --lib device_cost_matrix -- --nocapture`):

| node profile | SHA-256 150 B | SHA-384 300 B | AES block | CCM 1 KB | ECIES dec | ECIES enc | ECQV recon | P-256 verify* | P-384 verify | lv(i,j) | bf expand (RA) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `obu/unex-obu-301-craton2` | 6.6 | 27.4 | 0.8 | 99.3 | 1953.6 | 3179.8 | 1944.9 | 2579.6 | 21254.0 | 1.6 | 2457.0 |
| `obu/cohda-mk5` | 3.3 | 13.7 | 0.4 | 49.7 | 976.8 | 1589.9 | 972.4 | 1289.8 | 10627.0 | 0.8 | 1228.5 |
| `obu/cohda-mk6c-qualcomm-9150` | 4.9 | 20.5 | 0.6 | 74.5 | 1465.2 | 2384.8 | 1458.6 | 1934.7 | 15940.5 | 1.2 | 1842.7 |
| `obu/generic-automotive-soc-no-hsm` | 1.8 | 9.5 | 0.2 | 27.3 | 431.5 | 675.6 | 429.2 | 644.9 | 5720.8 | 0.4 | 489.4 |
| `rsu/commsignia-its-rs4` | 5.0 | 20.7 | 0.6 | 75.2 | 1480.0 | 2408.9 | 1473.4 | 1954.3 | 16101.5 | 1.2 | 1861.4 |
| `backend/hsm-appliance-luna7` | 0.1 | 0.4 | 0.0 | 100.0 | 314.3 | 30.5 | 14.8 | 100.0 | 62.5 | 0.1 | 97.8 |
| `backend/cloud-vm-x86` | 0.1 | 0.4 | 0.0 | 4.9 | 14.9 | 30.5 | 14.8 | 152.3 | 1324.5 | 0.1 | 97.8 |

\* The anchor figure. A node prices ECDSA from its own HSM or accelerator table first
(the CRATON2 verifies in 400 µs on its accelerator).

**What this means for the attack and benchmark studies:**

- A P-384 verification (the root and electors, CTL checks) costs an in-order OBU about
  10–21 ms, roughly 8× a P-256 verification. A run that floods certificate chains up to
  the root or CTLs is now visibly expensive.
- CRL linkage expansion costs 2·jmax AES calls per entry per i-period, about 31 µs on a
  CRATON2 at jmax = 20. A 10,000-entry CRL is about 0.3 s of CPU per week, and a
  revocation-flooding adversary scales it.
- ECQV reconstruction now costs 0.67–0.75× an ECDSA verification on the same anchor,
  not 1×, so implicit-certificate chains are a little cheaper.

**Host benchmark** (`cargo bench -p v2xw-sec --bench primitives`). It times the crate's
own code. Its results never enter a cost table.

- Machine: macOS aarch64, Apple M2.
- ECQV reconstruction (decompress, multiply, add) takes 99.3 µs, ECDH takes 93.8 µs, and
  an ECDSA verify takes 199.9 µs.
- The ratio of reconstruction to ECDH is 1.058. Decompression is 0.062 of an ECDH. So the
  recipe under-costs a reconstruction by about 6 %, and the descriptor says so.
- ECIES encrypt takes 215.8 µs, about two multiplications, which matches the recipe.

**Open:**

- `envelope::verify_plan` and `chain::validate` do not yet emit the per-link hashes.
  `primitive::ChainLink::ops()` gives them: `H(H(tbs) ‖ H(issuer))`, using SHA-384 for a
  P-384 issuer. WP-C2 should charge a validation through `PrimitiveCatalogue::price_all`
  with the node's `anchors::for_node_profile`.
- The key schedule of a rekeyed AES (a linkage seed used as the key) is not priced.
- AES-GCM throughput is used as an upper bound for AES on the Pi 5 and i9.
- No vendor publishes CRATON2 or SXF1800 figures for SHA, AES or ECIES.
