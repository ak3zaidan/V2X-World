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
