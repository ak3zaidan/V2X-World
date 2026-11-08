# Security envelope, cryptography and node security

Code: `crates/v2xw-sec` (envelope, certificates, primitives, backends, schemes),
`crates/v2xw-node` (what a device does with them, at what cost). The credential system
that issues and revokes certificates is in [scms.md](scms.md).

## The envelope

`v2xw_sec::envelope` builds IEEE 1609.2 `SignedData` (§5.3.1) and its ETSI TS 103 097
profile (`security.envelope: ieee1609.2 | etsi103097`) by real COER encoding through the
generated ASN.1 bindings, so a `SecuredPdu`'s bytes are bytes a conformant receiver would
accept and its size is exact; the overhead equals payload plus envelope as derived from
the profile tables, and tests assert equality. `v2xw_msg::sec_types` is the single
inclusion of the generated 1609.2 types.

| Module | Contents | Source |
|---|---|---|
| `hashedid` | `HashedId3/8/10` | IEEE 1609.2a-2017 §6.3.25-6.3.26 |
| `cert` | implicit (ECQV) certificates, about 80 octets, and explicit ones, about 147 | SEC 4; IEEE 1609.2 |
| `credential` | an implicit certificate with the private key its holder reconstructed | SEC 4 §3.4-3.5 |
| `chain` | chain validation to a root on the installed trust list | IEEE 1609.2 |
| `crl` | `Ieee1609Dot2Crl`: linkage-based (series 1) and hash-based lists, signing and verification | IEEE 1609.2-2022 |
| `ctl` | the IEEE 1609.2.1 certificate trust list (`MultiSignedCtl`, `fullIeeeCtl`), elector-signed, any 3 of 5, ECDSA P-384 | IEEE 1609.2.1 |
| `sig1609` | 1609.2 signatures: ECDSA P-256/SHA-256 and P-384/SHA-384 | IEEE 1609.2 |
| `ecies`, `kdf`, `ccm` | ECIES over P-256 wrapping an AES-128-CCM key; KDF2 and MAC1 | IEEE 1609.2a-2017 §5.3.5, Annex D.6.3 vectors |
| `p2pcd` | peer-to-peer certificate distribution wire formats | IEEE 1609.2 clause 8 |
| `ec`, `p384`, `aes128` | P-256 arithmetic, P-384, the raw AES-128 permutation | FIPS 186-4, SEC 1, FIPS 197 |
| `butterfly`, `linkage` | butterfly key expansion and linkage values | [scms.md](scms.md) |

## Crypto backends (`security.crypto_mode`)

| Mode | What runs |
|---|---|
| `real` (`crypto::Real`) | ECDSA P-256 with RFC 6979 nonces, SHA-256, SEC 4 ECQV reconstruction |
| `modeled` (`crypto::Modeled`) | a signature is a token derived from the key's secret and the digest; verification compares tokens; sizes from the primitive descriptors |

The two produce identical verification outcomes and identical encoded sizes: a modelled
token is exactly the descriptor's signature length and a modelled public key has the SEC 1
compressed shape, and nothing in the envelope branches on the mode
(`crates/v2xw-sec/tests/equivalence.rs`). Only the manifest's `crypto_mode` differs. Both
are deterministic, so neither affects run determinism.

In either mode, the compute a node spends signing and verifying comes from its hardware
profile, not from the host's wall time.

## Signature and KEM schemes (`security.signature`, `security.kem`)

`v2xw_sec::scheme` describes each scheme as a composition of primitives with cited sizes
and cost anchors (`scheme::SIGNATURE_IDS`, `scheme::KEM_IDS`):

| Mode | Signature ids |
|---|---|
| classical | `ecdsa-p256`, `ecdsa-brainpoolp256r1`, `ecdsa-p384` |
| hybrid: both signatures concatenated, both made and checked | `hybrid-falcon512-ecdsa-p256`, `hybrid-mldsa44-ecdsa-p256`, `hybrid-mldsa65-ecdsa-p256` |
| post-quantum alone: certificate key and issuer signature are post-quantum | `ml-dsa-44`, `ml-dsa-65`, `ml-dsa-87`, `falcon-512`, `falcon-1024`, `slh-dsa-sha2-128s`, `slh-dsa-sha2-128f` |

KEM ids: `ecies-p256`, `ml-kem-512`, `ml-kem-768`, `ml-kem-1024`,
`hybrid-ecies-p256-ml-kem-768`.

Post-quantum schemes are size and cost models: sizes are the standards' (FIPS 204 Table 2,
FIPS 205 Table 2, PQClean's padded Falcon), costs are the catalogue's cited anchors, scaled
by cycle ratio where a profile publishes nothing for a parameter set. No post-quantum
signature is computed; `crypto_mode: real` accepts `ecdsa-p256` only. Butterfly key
expansion is unavailable in the post-quantum modes, which changes provisioning (handled in
`v2xw_proto::hybrid`). Oversized certificates and signatures go through the fragmenters
of [network.md](network.md).

## The node's security stack (`crates/v2xw-node`)

| Concern | Module | Notes |
|---|---|---|
| hardware profiles | `profile`, `profiles` | OBU, RSU and backend profiles (for example `obu/unex-obu-301-craton2`, `obu/cohda-mk6c-qualcomm-9150`, `rsu/commsignia-its-rs4`, `backend/hsm-appliance-luna7`); every number cited, unpublished ones marked `todo-calibrate`; an operation a profile does not cost is refused rather than given a default |
| CPU and HSM service | `server`, `queue` | bounded queues with drops by cause; signing and verification take service time on the profile's engines |
| signing and envelopes | `secure` | real payloads, real signatures (or tokens), real bytes; signer identifier as certificate or digest per `security.signer_id_policy` |
| receiver validation | `rxval` | chain to a CTL root, validity at the receiver's time, PSID in `appPermissions`, generation time and location inside the signer's validity and region, hash-CRL and linkage-CRL checks, CRL overdue tolerance, message-age window (IEEE 1609.2 §5.1.2, §5.2) |
| verification policies | `policy` | `verify-all` (arrival order, drop what does not fit), `on-demand` (Krishnan & Weimerskirch, SAE 2011), `prioritized` (relevance and proximity, shed the oldest); every decision recorded with its reason |
| stores | `stores` | certificate pool and pseudonym rotation, neighbour ageing, the CRL gate, with their byte cost |
| P2PCD | `p2pcd` | a node requests a CA certificate it lacks and a peer answers (IEEE 1609.2 clause 8) |
| BSM timestamps | `bsm_timing` | per-device relation between a BSM's timestamp and its GNSS fix |
| telemetry | `telemetry` | the 208-byte `NodeTelemetry` record of VWP §3.5.2 |

Under load, a node queues, delays and drops verification work, and its telemetry says
which; the security metrics in [../recording-and-metrics.md](../recording-and-metrics.md)
(`verify_wait`, `verify_queue_depth`, `unverified_ratio`, `full_cert_share`,
`p2pcd_requests`) measure it.

## Applications

The V2X applications that consume verified messages (FCW, EEBL, IMA, LTA, BSW/LCW, PCW,
RLVW, GLOSA) and their truth labelling are on [V2X applications](applications.md).

## Modelled versus simplified

| Modelled | Simplified or absent |
|---|---|
| real COER IEEE 1609.2 / TS 103 097 `SignedData`, certificates, CRLs and CTLs; measured overhead ([Messages](messages.md#the-security-envelope)) | encrypted SPDUs, countersignatures and certificate requests on the envelope path; geometric region containment fails closed |
| ECDSA P-256 with RFC 6979 nonces and SEC 4 ECQV in `crypto_mode: real` | as a node's signature scheme, `real` accepts `ecdsa-p256` only; every post-quantum scheme is a size and cost model |
| signing and verification cost from the node's hardware profile, queued with drops by cause | an operation a profile does not cost is refused rather than given a default; unpublished profile numbers are `todo-calibrate` |
| receiver validation per IEEE 1609.2 §5.1.2, §5.2, with CRL and generation-time checks | the clock that judges freshness runs without drift in the engine ([Determinism and time](determinism-and-time.md#node-clocks-and-gnss-time)) |
| three verification policies, every decision recorded | |
