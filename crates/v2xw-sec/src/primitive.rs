//! Cryptographic primitive descriptors: sizes, security levels and cost anchors.
//!
//! This module is 04-models.md §9.4 turned into data. A [`PrimitiveDescriptor`] says how
//! large a key, a signature and a certificate are for one primitive, at what NIST
//! security level, and — per hardware profile — how long an operation takes, in the units
//! the source published. Nothing here computes cryptography; [`crate::crypto`] does that
//! for the two primitives that have implementations, and everything else exists so the
//! engine can *charge* and *size* an operation it does not perform.
//!
//! # Why the post-quantum primitives are here without implementations
//!
//! The question the simulator exists to answer is what happens to a V2X deployment when a
//! 2,420-byte ML-DSA-44 signature replaces a 64-byte ECDSA one: channel load, certificate
//! attachment cost, verification queue depth, CRL size. Every one of those follows from
//! the *sizes* and the *costs*, not from the signature bytes. So the descriptors and the
//! cost tables are what Phase 1 needs, and running PQClean is a later, separable job
//! ([`crate::crypto::Modeled`] already produces correctly sized tokens for them).
//!
//! # Fidelity of the numbers
//!
//! Each [`CostRow`] carries its own citation and, where the source qualifies itself, a
//! note. Two rows are recorded *as published* although they are implausible next to their
//! neighbours — the Cohda MK6 ECDSA verify at 0.001 ms and the Cohda SPHINCS+ sign at
//! 5.485 ms — because silently "fixing" a published figure is how a table stops being
//! evidence. They are flagged in [`CostRow::note`] and a test asserts the flag is there.
//!
//! A profile with no row is filled by [`CostTable::row_or_scaled`], which takes the
//! nearest cycle count and scales it by the clock ratio, and tags the result
//! [`CostRow::scaled`] so the manifest can record that the number was derived rather than
//! measured (04-models.md §9.4, last paragraph).

use std::sync::LazyLock;

use serde::Serialize;
use v2xw_core::card::{Family, ModelCard, Source, SourceKind, Tier, Validation, ValidationStatus};
use v2xw_core::math::quantize_to;
use v2xw_core::model::Model;
use v2xw_core::time::Duration;

use crate::error::{Result, SecError};

/// A citation on a descriptor or a cost row.
///
/// The same type the model cards use, so a descriptor's provenance and a card's
/// provenance are one vocabulary rather than two (03-interfaces.md §6 names the type
/// `Citation`; this alias keeps that spelling).
pub type Citation = Source;

/// A primitive's stable id, e.g. `primitive/ecdsa-p256-sha256`.
///
/// `Copy`, over `&'static str`, because the catalogue is closed at compile time and every
/// other type here keys off it: a [`crate::envelope::PrimitiveOp`] carries one, a cost
/// lookup takes one, and a `String` in either place would allocate on a per-message path.
/// The trade is that an out-of-process plug-in cannot mint a new id at run time, which
/// Phase 1 does not need — the primitive catalogue is standards material, not scenario
/// material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PrimitiveId(pub &'static str);

impl PrimitiveId {
    /// ECDSA over NIST P-256 with SHA-256 — the primitive IEEE 1609.2 and TS 103 097
    /// deployments actually use.
    pub const ECDSA_P256_SHA256: PrimitiveId = PrimitiveId("primitive/ecdsa-p256-sha256");
    /// ECDSA over brainpoolP256r1, the second P-256-class curve 1609.2 admits.
    pub const ECDSA_BRAINPOOL_P256R1: PrimitiveId = PrimitiveId("primitive/ecdsa-brainpoolp256r1");
    /// ECDSA over NIST P-384.
    pub const ECDSA_P384: PrimitiveId = PrimitiveId("primitive/ecdsa-p384");
    /// Ed25519. Present only because the legacy Python reference signed with it; no
    /// 1609.2 profile admits it.
    pub const ED25519: PrimitiveId = PrimitiveId("primitive/ed25519");
    /// ECQV implicit certificates over P-256 (SEC 4).
    pub const ECQV_P256: PrimitiveId = PrimitiveId("primitive/ecqv-p256");
    /// SHA-256.
    pub const SHA_256: PrimitiveId = PrimitiveId("primitive/sha-256");
    /// SHA-384 (FIPS 180-4): the hash of every P-384 signature, the elector and root
    /// signatures among them.
    pub const SHA_384: PrimitiveId = PrimitiveId("primitive/sha-384");
    /// The bare AES-128 block permutation (FIPS 197): the linkage-value and
    /// butterfly-expansion constructions and every block of CCM are made of it.
    pub const AES_128: PrimitiveId = PrimitiveId("primitive/aes-128");
    /// AES-128-CCM as IEEE 1609.2 profiles it (12-byte nonce, 16-byte tag, no associated
    /// data).
    pub const AES_128_CCM: PrimitiveId = PrimitiveId("primitive/aes-128-ccm");
    /// ECDH over NIST P-256: one variable-base scalar multiplication. ECIES and the ECQV
    /// reconstruction are priced from it.
    pub const ECDH_P256: PrimitiveId = PrimitiveId("primitive/ecdh-p256");
    /// ECIES over P-256 as IEEE 1609.2 §5.3.5 profiles it (KDF2 and MAC1 over SHA-256)
    /// wrapping a 16-byte AES key.
    pub const ECIES_P256: PrimitiveId = PrimitiveId("primitive/ecies-p256");
    /// The original (US-profile) butterfly key expansion of IEEE 1609.2.1 §9.3, one
    /// `(i, j)` certificate's worth.
    pub const BUTTERFLY_EXPAND_P256: PrimitiveId = PrimitiveId("primitive/butterfly-expand-p256");
    /// The SCMS linkage-value construction (CAMP `lv`; 02 §10.1).
    pub const LINKAGE_VALUE: PrimitiveId = PrimitiveId("primitive/linkage-value");
    /// ML-DSA-44 (FIPS 204).
    pub const ML_DSA_44: PrimitiveId = PrimitiveId("primitive/ml-dsa-44");
    /// ML-DSA-65 (FIPS 204).
    pub const ML_DSA_65: PrimitiveId = PrimitiveId("primitive/ml-dsa-65");
    /// ML-DSA-87 (FIPS 204).
    pub const ML_DSA_87: PrimitiveId = PrimitiveId("primitive/ml-dsa-87");
    /// Falcon-512, padded signature form.
    pub const FALCON_512: PrimitiveId = PrimitiveId("primitive/falcon-512");
    /// Falcon-1024, padded signature form.
    pub const FALCON_1024: PrimitiveId = PrimitiveId("primitive/falcon-1024");
    /// SLH-DSA-SHA2-128s (FIPS 205).
    pub const SLH_DSA_SHA2_128S: PrimitiveId = PrimitiveId("primitive/slh-dsa-sha2-128s");
    /// SLH-DSA-SHA2-128f (FIPS 205).
    pub const SLH_DSA_SHA2_128F: PrimitiveId = PrimitiveId("primitive/slh-dsa-sha2-128f");
    /// SLH-DSA-SHA2-192s (FIPS 205).
    pub const SLH_DSA_SHA2_192S: PrimitiveId = PrimitiveId("primitive/slh-dsa-sha2-192s");
    /// SLH-DSA-SHA2-192f (FIPS 205).
    pub const SLH_DSA_SHA2_192F: PrimitiveId = PrimitiveId("primitive/slh-dsa-sha2-192f");
    /// SLH-DSA-SHA2-256s (FIPS 205).
    pub const SLH_DSA_SHA2_256S: PrimitiveId = PrimitiveId("primitive/slh-dsa-sha2-256s");
    /// SLH-DSA-SHA2-256f (FIPS 205).
    pub const SLH_DSA_SHA2_256F: PrimitiveId = PrimitiveId("primitive/slh-dsa-sha2-256f");
    /// ML-KEM-512 (FIPS 203).
    pub const ML_KEM_512: PrimitiveId = PrimitiveId("primitive/ml-kem-512");
    /// ML-KEM-768 (FIPS 203).
    pub const ML_KEM_768: PrimitiveId = PrimitiveId("primitive/ml-kem-768");
    /// ML-KEM-1024 (FIPS 203).
    pub const ML_KEM_1024: PrimitiveId = PrimitiveId("primitive/ml-kem-1024");
    /// ML-DSA-44 + ECDSA P-256 hybrid.
    pub const HYBRID_MLDSA44_ECDSA_P256: PrimitiveId =
        PrimitiveId("primitive/hybrid-mldsa44-ecdsa-p256");
    /// ML-DSA-65 + ECDSA P-256 hybrid.
    pub const HYBRID_MLDSA65_ECDSA_P256: PrimitiveId =
        PrimitiveId("primitive/hybrid-mldsa65-ecdsa-p256");
    /// Falcon-512 + ECDSA P-256 hybrid.
    pub const HYBRID_FALCON512_ECDSA_P256: PrimitiveId =
        PrimitiveId("primitive/hybrid-falcon512-ecdsa-p256");

    /// The id as a string.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl core::fmt::Display for PrimitiveId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

/// What kind of primitive this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrimitiveFamily {
    /// A digital signature scheme.
    Signature,
    /// A key-encapsulation mechanism.
    Kem,
    /// A hash function.
    Hash,
    /// An authenticated cipher.
    Aead,
    /// An implicit-certificate scheme (ECQV).
    ImplicitCert,
    /// A key-agreement primitive (ECDH) or a hybrid encryption built on one (ECIES).
    KeyAgreement,
    /// A bare block cipher.
    BlockCipher,
    /// A construction made of other primitives (butterfly expansion, linkage values),
    /// costed only through its [`CostRecipe`].
    Construction,
}

/// How a primitive's cost grows with the length of its input.
///
/// A signature costs the same whatever it signs, because the message is hashed first and
/// the hash is a separate primitive. A hash or a cipher does not: hashing a 3 KB
/// certificate chain costs fifty times what hashing one 64-byte block does, and a CRL
/// expansion is thousands of AES calls. So these primitives are priced *per unit of
/// work* — a compression-function call, a block-cipher call — and
/// [`PrimitiveDescriptor::cost_for_len`] counts the units for an input length exactly as
/// the algorithm does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CostUnit {
    /// One operation, whatever the input length.
    #[default]
    PerOperation,
    /// A Merkle–Damgård hash: one compression call per `block` bytes after FIPS 180-4
    /// padding (one `0x80` byte and a `length_field`-byte length).
    HashBlocks {
        /// Compression-function block, in bytes.
        block: u32,
        /// Length-encoding field appended by the padding, in bytes.
        length_field: u32,
    },
    /// One block-cipher call per `block` bytes, and at least one.
    CipherBlocks {
        /// Cipher block, in bytes.
        block: u32,
    },
    /// AES calls of AES-128-CCM with no associated data (SP 800-38C §6.1, as IEEE 1609.2
    /// §5.3.8 profiles it): CBC-MAC over `B0` and the ⌈n/16⌉ payload blocks, then CTR
    /// over the payload and `A0` for the tag — `2·⌈n/16⌉ + 2` in all.
    CcmAesCalls,
}

impl CostUnit {
    /// How many units an input of `bytes` bytes costs.
    pub const fn units(self, bytes: u32) -> u64 {
        let n = bytes as u64;
        match self {
            CostUnit::PerOperation => 1,
            CostUnit::HashBlocks {
                block,
                length_field,
            } => (n + 1 + length_field as u64).div_ceil(block as u64),
            CostUnit::CipherBlocks { block } => {
                let b = n.div_ceil(block as u64);
                if b == 0 { 1 } else { b }
            }
            CostUnit::CcmAesCalls => 2 * n.div_ceil(16) + 2,
        }
    }

    fn is_per_operation(&self) -> bool {
        *self == CostUnit::PerOperation
    }
}

/// One term of a [`CostRecipe`]: `count` runs of `primitive`'s `op` over `bytes` bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RecipeStep {
    /// The primitive run.
    pub primitive: PrimitiveId,
    /// Which of its operations.
    pub op: PrimitiveOpKind,
    /// Input length, for a per-length primitive ([`CostUnit`]); ignored otherwise.
    pub bytes: u32,
    /// How many times.
    pub count: u32,
    /// The operation priced instead when `primitive`'s `op` has no figure on the
    /// profile, always an upper bound of it: a fixed-base scalar multiplication (a key
    /// generation) is priced as an ECDSA signature, which is that multiplication plus an
    /// inversion, where a profile publishes no key-generation figure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upper_bound: Option<(PrimitiveId, PrimitiveOpKind)>,
}

impl RecipeStep {
    const fn new(primitive: PrimitiveId, op: PrimitiveOpKind, bytes: u32, count: u32) -> Self {
        RecipeStep {
            primitive,
            op,
            bytes,
            count,
            upper_bound: None,
        }
    }

    /// A fixed-base P-256 scalar multiplication (`k·G`), `count` times: the ECDSA
    /// key-generation figure, or the ECDSA signing figure where none is published.
    const fn mul_base(count: u32) -> Self {
        RecipeStep {
            primitive: PrimitiveId::ECDSA_P256_SHA256,
            op: PrimitiveOpKind::KeyGen,
            bytes: 0,
            count,
            upper_bound: Some((PrimitiveId::ECDSA_P256_SHA256, PrimitiveOpKind::Sign)),
        }
    }
}

/// A primitive's cost as the sum of the primitive operations it is made of, per
/// operation kind.
///
/// For the constructions no benchmark publishes as a unit — ECIES, the ECQV
/// reconstruction, the butterfly expansion, a linkage value — but whose operation count
/// is fixed by the standard. Each step is a published figure; the recipe is the
/// standard's own arithmetic, written down so a reader can check it. Terms the recipe
/// leaves out (a point addition, a scalar addition, an XOR) are each far below 1 % of
/// the scalar multiplication beside them and are named in the descriptor's notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CostRecipe {
    /// Key generation.
    pub keygen: &'static [RecipeStep],
    /// Signing, encryption, issuance, or the construction's forward direction.
    pub sign: &'static [RecipeStep],
    /// Verification, decryption, reconstruction, or the construction's checking side.
    pub verify: &'static [RecipeStep],
}

impl CostRecipe {
    /// The steps for `op`.
    pub fn steps(&self, op: PrimitiveOpKind) -> &'static [RecipeStep] {
        match op {
            PrimitiveOpKind::KeyGen => self.keygen,
            PrimitiveOpKind::Sign => self.sign,
            PrimitiveOpKind::Verify => self.verify,
        }
    }
}

/// A size that is either fixed or variable.
///
/// Falcon is the reason this is not a `u32`: its signatures are genuinely
/// variable-length, and both numbers matter — the mean drives channel load, the maximum
/// drives buffer sizing and the fragmentation threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SizeSpec {
    /// Always exactly this many bytes.
    Fixed(u32),
    /// Variable, with a mean and a hard maximum.
    Variable {
        /// Typical size, in bytes.
        mean: u32,
        /// Largest possible size, in bytes.
        max: u32,
    },
}

impl SizeSpec {
    /// The size to use when one number is needed: the fixed size, or the mean.
    pub const fn nominal(self) -> u32 {
        match self {
            SizeSpec::Fixed(n) => n,
            SizeSpec::Variable { mean, .. } => mean,
        }
    }

    /// The largest size this primitive can produce — what a buffer must hold.
    pub const fn max(self) -> u32 {
        match self {
            SizeSpec::Fixed(n) => n,
            SizeSpec::Variable { max, .. } => max,
        }
    }
}

/// Where an operation runs on a given hardware profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CostSite {
    /// On the application processor.
    Cpu,
    /// In a hardware security module.
    Hsm,
    /// On a dedicated crypto accelerator or verification engine.
    Accel,
}

/// The cost of one operation on one hardware profile, in the units the source published.
///
/// Every field is optional because sources are: a cycle-count paper gives cycles, a
/// benchmark blog gives operations per second, a datasheet gives milliseconds. Nothing is
/// converted on the way in — [`OpCost::mean`] converts on the way *out*, once, so the
/// stored value stays comparable with its citation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct OpCost {
    /// Cycles, as published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycles: Option<u64>,
    /// Mean wall time in microseconds, as published or derived from `ops_per_s`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_us: Option<f64>,
    /// 95th-percentile wall time in microseconds, where the source gives a spread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_us: Option<f64>,
    /// Operations per second, as published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops_per_s: Option<f64>,
    /// Bytes per second, as published, for a per-length primitive ([`CostUnit`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_per_s: Option<f64>,
    /// The input length the published figure was measured over: the chunk size of a
    /// throughput benchmark, or the length of the one message a source timed. A
    /// per-length cost divides the figure by the units this many bytes cost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub over_bytes: Option<u32>,
    /// The number of [`CostUnit`]s the published figure covers, where the source states
    /// it in units rather than bytes (a cycle count per block).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub over_units: Option<u32>,
}

/// Microsecond quantum for every cost value (ADR 0004 decision 7, build decision D9).
///
/// A nanosecond, expressed in the microseconds these fields are stored in. The engine
/// charges costs as an integer [`Duration`] in nanoseconds, so storing a cost finer than
/// that grid would record a distinction the scheduler cannot represent.
pub const COST_US_QUANTUM: f64 = 1e-3;

impl OpCost {
    /// A cost published as a cycle count, with no wall time.
    pub const fn from_cycles(cycles: u64) -> OpCost {
        OpCost {
            cycles: Some(cycles),
            mean_us: None,
            p95_us: None,
            ops_per_s: None,
            bytes_per_s: None,
            over_bytes: None,
            over_units: None,
        }
    }

    /// A cost published as both a cycle count and a wall time.
    pub fn from_cycles_and_us(cycles: u64, mean_us: f64) -> OpCost {
        OpCost {
            cycles: Some(cycles),
            mean_us: Some(quantize_to(mean_us, COST_US_QUANTUM)),
            p95_us: None,
            ops_per_s: None,
            bytes_per_s: None,
            over_bytes: None,
            over_units: None,
        }
    }

    /// A cost published as a wall time in microseconds.
    pub fn from_us(mean_us: f64) -> OpCost {
        OpCost {
            cycles: None,
            mean_us: Some(quantize_to(mean_us, COST_US_QUANTUM)),
            p95_us: None,
            ops_per_s: None,
            bytes_per_s: None,
            over_bytes: None,
            over_units: None,
        }
    }

    /// A cost published as a throughput in operations per second.
    ///
    /// `mean_us` is filled in as `10^6 / rate`, quantised to the nanosecond, so a cost
    /// lookup does not have to know which form the source used.
    pub fn from_ops_per_s(rate: f64) -> OpCost {
        OpCost {
            cycles: None,
            mean_us: Some(quantize_to(1.0e6 / rate, COST_US_QUANTUM)),
            p95_us: None,
            ops_per_s: Some(rate),
            bytes_per_s: None,
            over_bytes: None,
            over_units: None,
        }
    }

    /// A per-length cost published as a throughput of `rate` bytes per second, measured
    /// on chunks of `chunk_bytes` bytes (an `openssl speed` column, a wolfCrypt
    /// `benchmark` block size).
    ///
    /// `mean_us` is the time for one chunk, `chunk_bytes / rate`, quantised; the chunk
    /// size is kept because the per-unit cost is that time divided by the units *one
    /// chunk* costs, padding included.
    pub fn from_bytes_per_s(rate: f64, chunk_bytes: u32) -> OpCost {
        OpCost {
            cycles: None,
            mean_us: Some(quantize_to(
                f64::from(chunk_bytes) / rate * 1.0e6,
                COST_US_QUANTUM,
            )),
            p95_us: None,
            ops_per_s: None,
            bytes_per_s: Some(rate),
            over_bytes: Some(chunk_bytes),
            over_units: None,
        }
    }

    /// A per-length cost published as one timed message of `bytes` bytes.
    pub fn from_us_over_bytes(mean_us: f64, bytes: u32) -> OpCost {
        OpCost {
            over_bytes: Some(bytes),
            ..OpCost::from_us(mean_us)
        }
    }

    /// The same figure, stated as covering `units` [`CostUnit`]s.
    pub fn over_units(mut self, units: u32) -> OpCost {
        self.over_units = Some(units);
        self
    }

    /// A cost published as a mean cycle count together with the worst case the source
    /// measured, at a known clock.
    ///
    /// `pqm4` publishes min/mean/max rather than percentiles for the schemes whose signing
    /// loop rejects and retries. The measured maximum is the honest stand-in for a p95 —
    /// it is conservative, and a queueing model that used the mean alone would understate
    /// the tail that matters.
    pub fn from_cycles_with_max(mean_cycles: u64, max_cycles: u64, clock_hz: f64) -> OpCost {
        OpCost {
            cycles: Some(mean_cycles),
            mean_us: None,
            p95_us: Some(quantize_to(
                (max_cycles as f64) / clock_hz * 1.0e6,
                COST_US_QUANTUM,
            )),
            ops_per_s: None,
            bytes_per_s: None,
            over_bytes: None,
            over_units: None,
        }
    }

    /// The mean cost as an engine [`Duration`], or `None` when only cycles are known and
    /// no clock was supplied.
    ///
    /// Rounds to the nanosecond, which is the engine's time quantum.
    pub fn mean(&self, clock_hz: Option<f64>) -> Option<Duration> {
        if let Some(us) = self.mean_us {
            return Some(Duration::from_nanos((us * 1000.0).round().max(0.0) as u64));
        }
        let (cycles, hz) = (self.cycles?, clock_hz?);
        let ns = (cycles as f64) / hz * 1.0e9;
        Some(Duration::from_nanos(ns.round().max(0.0) as u64))
    }
}

/// One hardware profile's costs for one primitive.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CostRow {
    /// The hardware profile's model id, as the scenario spells it.
    pub profile: &'static str,
    /// Clock frequency in hertz, where the source states one. Needed to turn a cycle
    /// count into a wall time and to scale a cycle count onto another profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock_hz: Option<f64>,
    /// Where the operation runs.
    pub site: CostSite,
    /// Key generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keygen: Option<OpCost>,
    /// Signing (or encapsulation, for a KEM).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign: Option<OpCost>,
    /// Verification (or decapsulation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<OpCost>,
    /// Where the numbers come from.
    pub source: Citation,
    /// Anything that qualifies them: an unstated OS, an anomalous published value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<&'static str>,
    /// True when this row was derived by scaling another profile's cycle count rather
    /// than measured. The run manifest records it (04-models.md §9.4).
    pub scaled: bool,
}

impl CostRow {
    fn new(profile: &'static str, site: CostSite, source: Citation) -> CostRow {
        CostRow {
            profile,
            clock_hz: None,
            site,
            keygen: None,
            sign: None,
            verify: None,
            source,
            note: None,
            scaled: false,
        }
    }

    fn at(mut self, clock_hz: f64) -> CostRow {
        self.clock_hz = Some(clock_hz);
        self
    }

    fn sign(mut self, c: OpCost) -> CostRow {
        self.sign = Some(c);
        self
    }

    fn verify(mut self, c: OpCost) -> CostRow {
        self.verify = Some(c);
        self
    }

    fn keygen(mut self, c: OpCost) -> CostRow {
        self.keygen = Some(c);
        self
    }

    /// The same figure for both directions: a hash, or a cipher whose encryption and
    /// decryption make the same number of block calls.
    fn both(self, c: OpCost) -> CostRow {
        self.sign(c).verify(c)
    }

    /// The cost of `units` units of `op` on this profile, for a primitive costed per
    /// `unit`.
    ///
    /// The published figure covers [`OpCost::over_units`] units, or the units
    /// [`OpCost::over_bytes`] bytes cost; the result is the figure scaled to `units`, in
    /// integer nanoseconds rounded half up. A figure that states neither is a per-call
    /// figure (an HSM's transactions per second, payload unstated) and is charged once
    /// whatever the length, which the row's note must say.
    fn duration_for_units(
        &self,
        op: PrimitiveOpKind,
        unit: CostUnit,
        units: u64,
    ) -> Option<Duration> {
        let c = self.op(op)?;
        let whole = c.mean(self.clock_hz)?.as_nanos();
        let covered = match (c.over_units, c.over_bytes) {
            (Some(u), _) => u64::from(u),
            (None, Some(b)) => unit.units(b),
            (None, None) => return Some(Duration::from_nanos(whole)),
        };
        if covered == 0 {
            return None;
        }
        let ns =
            (u128::from(whole) * u128::from(units) + u128::from(covered) / 2) / u128::from(covered);
        Some(Duration::from_nanos(u64::try_from(ns).unwrap_or(u64::MAX)))
    }

    fn note(mut self, n: &'static str) -> CostRow {
        self.note = Some(n);
        self
    }

    /// The cost of `op` on this profile.
    pub fn op(&self, op: PrimitiveOpKind) -> Option<&OpCost> {
        match op {
            PrimitiveOpKind::KeyGen => self.keygen.as_ref(),
            PrimitiveOpKind::Sign => self.sign.as_ref(),
            PrimitiveOpKind::Verify => self.verify.as_ref(),
        }
    }

    /// The mean duration of `op` on this profile.
    pub fn duration(&self, op: PrimitiveOpKind) -> Option<Duration> {
        self.op(op)?.mean(self.clock_hz)
    }
}

/// Which of the three costed operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrimitiveOpKind {
    /// Key generation.
    KeyGen,
    /// Signing or encapsulation.
    Sign,
    /// Verification or decapsulation.
    Verify,
}

/// A primitive's cost anchors, keyed by hardware profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CostTable {
    rows: Vec<CostRow>,
}

impl CostTable {
    /// A table from its rows, sorted by profile id.
    ///
    /// Sorted at construction, not at lookup: every iteration over a cost table —
    /// a report, a card payload, a manifest digest — must produce the same order on every
    /// platform, and sorting once at build time makes that free rather than a rule
    /// callers have to remember (02-architecture.md §6.4).
    pub fn new(mut rows: Vec<CostRow>) -> CostTable {
        rows.sort_by(|a, b| a.profile.cmp(b.profile));
        CostTable { rows }
    }

    /// The row for `profile`, if one was published.
    pub fn row(&self, profile: &str) -> Option<&CostRow> {
        self.rows.iter().find(|r| r.profile == profile)
    }

    /// Every row, in profile order.
    pub fn rows(&self) -> &[CostRow] {
        &self.rows
    }

    /// True when the table has no rows at all.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The row for `profile`, or one derived by scaling the nearest published cycle count
    /// to `target_clock_hz`.
    ///
    /// This is 04-models.md §9.4's rule, implemented: "A `CostTable` row missing for a
    /// `HardwareProfile` is filled by scaling the nearest cycle count by clock ratio and
    /// tagged `scaled`, which the manifest records." "Nearest" means nearest clock
    /// frequency among the rows that publish both a clock and a cycle count, with ties
    /// broken by profile id so the choice is deterministic.
    pub fn row_or_scaled(&self, profile: &'static str, target_clock_hz: f64) -> Option<CostRow> {
        if let Some(exact) = self.row(profile) {
            return Some(exact.clone());
        }
        let nearest = self
            .rows
            .iter()
            .filter(|r| r.clock_hz.is_some() && r.has_any_cycles())
            .min_by(|a, b| {
                let da = (a.clock_hz.unwrap_or_default() - target_clock_hz).abs();
                let db = (b.clock_hz.unwrap_or_default() - target_clock_hz).abs();
                // `total_cmp` rather than `partial_cmp`: an equal distance must fall
                // through to the profile-id tie-break rather than to `Ordering::Equal`
                // on a NaN, which would make the choice depend on iteration order.
                da.total_cmp(&db).then_with(|| a.profile.cmp(b.profile))
            })?;
        Some(CostRow {
            profile,
            clock_hz: Some(target_clock_hz),
            site: nearest.site,
            keygen: nearest.keygen.map(|c| scale(c, target_clock_hz)),
            sign: nearest.sign.map(|c| scale(c, target_clock_hz)),
            verify: nearest.verify.map(|c| scale(c, target_clock_hz)),
            source: nearest.source.clone(),
            note: Some("scaled from the nearest published cycle count by clock ratio"),
            scaled: true,
        })
    }
}

impl CostRow {
    /// True when any of the three operations published a cycle count, which is what
    /// [`CostTable::row_or_scaled`] needs to scale from.
    fn has_any_cycles(&self) -> bool {
        self.keygen.and_then(|c| c.cycles).is_some()
            || self.sign.and_then(|c| c.cycles).is_some()
            || self.verify.and_then(|c| c.cycles).is_some()
    }
}

/// Re-express a cost's cycle count at a new clock, dropping the source's own wall time.
fn scale(c: OpCost, target_clock_hz: f64) -> OpCost {
    match c.cycles {
        Some(cycles) => OpCost {
            cycles: Some(cycles),
            mean_us: Some(quantize_to(
                (cycles as f64) / target_clock_hz * 1.0e6,
                COST_US_QUANTUM,
            )),
            p95_us: None,
            ops_per_s: None,
            bytes_per_s: None,
            // The rescaled time still covers the same input as the published one.
            over_bytes: c.over_bytes,
            over_units: c.over_units,
        },
        // Nothing to scale: a throughput figure carries no cycle count, so it is passed
        // through unchanged rather than silently reinterpreted on other silicon.
        None => c,
    }
}

/// How a primitive's cost is derived on a profile it has no published row for: scale a
/// sibling primitive's published cost on that profile by the two primitives' cycle-count
/// ratio on a `reference` profile that publishes both (04-models.md §9.4's scaling rule,
/// applied across primitives rather than across clocks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CostRatio {
    /// The sibling whose published per-profile costs are scaled.
    pub base: PrimitiveId,
    /// The profile on which both primitives publish a cycle count for the operation.
    pub reference: &'static str,
}

/// One primitive's sizes, level and costs (03-interfaces.md §6).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrimitiveDescriptor {
    /// Stable id.
    pub id: PrimitiveId,
    /// What kind of primitive.
    pub family: PrimitiveFamily,
    /// Public key size in bytes, in its compact form (compressed, for an EC curve).
    pub pk_bytes: u32,
    /// Public key size in its uncompressed form, where the primitive has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pk_bytes_uncompressed: Option<u32>,
    /// Private key size in bytes.
    pub sk_bytes: u32,
    /// Signature (or ciphertext) size.
    pub sig_bytes: SizeSpec,
    /// Signature size *as carried in the IEEE 1609.2 envelope*, where the ASN.1 framing
    /// adds to the raw size.
    ///
    /// Separate from [`PrimitiveDescriptor::sig_bytes`] because both numbers are used and
    /// they differ: ECDSA P-256 is 64 raw bytes but 66 encoded (`Signature` choice tag 1 +
    /// `rSig` as an `EccP256CurvePoint` 1 + 32 + `sSig` 32), and 04-models.md §9.1's
    /// envelope derivation uses the encoded one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig_bytes_encoded: Option<u32>,
    /// Shared-secret size, for a KEM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_secret_bytes: Option<u32>,
    /// Encoded certificate size carrying this key under the envelope's profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert_bytes: Option<u32>,
    /// NIST security level 1..=5.
    pub security_level: u8,
    /// Cost anchors per hardware profile.
    pub cost: CostTable,
    /// The primitives whose costs add up to this one's, when it is a composite and was
    /// never benchmarked as a unit.
    ///
    /// The hybrids are the case: 04-models.md §9.4 gives sizes for
    /// `hybrid-mldsa44-ecdsa-p256` but no timing, because nobody published a benchmark of
    /// the pair. Signing one is signing twice, so the honest cost is the sum of the two
    /// components' anchors on the same profile — and naming the components makes that a
    /// computation a reader can check rather than a number that appeared.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cost_components: Vec<PrimitiveId>,
    /// The primitive whose cost anchors stand in for this one's, when this one has none
    /// published.
    ///
    /// ECQV is the case: no published reconstruction timing was found, and a
    /// reconstruction is one P-256 point multiplication plus an addition — the dominant
    /// term of an ECDSA verification. Naming the proxy explicitly is honest about where
    /// the number came from; leaving the table empty and letting each caller invent a
    /// fallback would not be.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_proxy: Option<PrimitiveId>,
    /// A published cost derived for profiles this primitive has no row on, by the ratio of
    /// its own cycle count to a sibling's on a platform that publishes both.
    ///
    /// ML-DSA-65 is the case: no Cortex-A or Cohda figure is published, but the `pqm4`
    /// Cortex-M4 table gives ML-DSA-65 and ML-DSA-44 on the same core, and ML-DSA-44 *is*
    /// published on every V2X-relevant profile. The derived figure is
    /// `base(profile) × own(reference) / base(reference)`, per operation, and
    /// [`PrimitiveDescriptor::cost_is_derived`] reports it as derived so a run can say so.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_ratio: Option<CostRatio>,
    /// How the cost grows with the input length (per operation for everything but the
    /// hashes and ciphers).
    #[serde(skip_serializing_if = "CostUnit::is_per_operation")]
    pub cost_unit: CostUnit,
    /// The operations this primitive's cost is the sum of, on a profile with no row of
    /// its own (ECIES, ECQV, the butterfly expansion, linkage values).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_recipe: Option<CostRecipe>,
    /// Where the sizes come from.
    pub sources: Vec<Citation>,
    /// Anything that qualifies the entry.
    pub notes: Vec<&'static str>,
}

impl PrimitiveDescriptor {
    /// The cost row for `profile`, following [`PrimitiveDescriptor::cost_proxy`] when this
    /// primitive publishes none of its own.
    ///
    /// Returns a *row*, so it cannot answer for a composite whose cost is a sum of two
    /// rows; [`PrimitiveDescriptor::cost_duration`] is the call that can.
    pub fn cost_row<'a>(
        &'a self,
        profile: &str,
        catalogue: &'a PrimitiveCatalogue,
    ) -> Option<&'a CostRow> {
        if let Some(row) = self.cost.row(profile) {
            return Some(row);
        }
        let proxy = self.cost_proxy?;
        catalogue.get(proxy)?.cost.row(profile)
    }

    /// The modelled duration of `op` on `profile`, through whichever of the three routes
    /// this primitive declares: its own anchors, its declared proxy, or the sum of its
    /// components.
    ///
    /// `None` when none of the three yields a number — which is the right answer, and not
    /// a zero: an unbenchmarked primitive on an unbenchmarked platform has no cost the
    /// simulator is entitled to invent.
    pub fn cost_duration(
        &self,
        op: PrimitiveOpKind,
        profile: &str,
        catalogue: &PrimitiveCatalogue,
    ) -> Option<Duration> {
        if !self.cost_unit.is_per_operation() {
            // A per-length primitive asked for "one operation" is charged one unit: one
            // compression call, one block-cipher call. Length-aware callers use
            // [`PrimitiveDescriptor::cost_for_len`].
            return self.cost_for_units(op, profile, 1, catalogue);
        }
        if let Some(row) = self.cost.row(profile) {
            // A row that publishes this operation wins; one that does not (an HSM that
            // publishes ECIES decryption only) falls to the recipe when there is one.
            let own = row.duration(op);
            if own.is_some() || self.cost_recipe.is_none() {
                return own;
            }
        }
        if let Some(recipe) = self.cost_recipe {
            return recipe_cost(recipe.steps(op), profile, catalogue);
        }
        if let Some(proxy) = self.cost_proxy {
            return catalogue.get(proxy)?.cost_duration(op, profile, catalogue);
        }
        if !self.cost_components.is_empty() {
            let mut total = Duration::ZERO;
            for c in &self.cost_components {
                total += catalogue.get(*c)?.cost_duration(op, profile, catalogue)?;
            }
            return Some(total);
        }
        if let Some(r) = self.cost_ratio {
            let base = catalogue.get(r.base)?;
            let base_here = base.cost_duration(op, profile, catalogue)?;
            let own = self.cost.row(r.reference)?.op(op)?.cycles?;
            let theirs = base.cost.row(r.reference)?.op(op)?.cycles?;
            if theirs == 0 {
                return None;
            }
            // Integer arithmetic: nanoseconds × cycles ÷ cycles, in u128 so no product of
            // a long signing time and a large cycle count can overflow.
            let ns = u128::from(base_here.as_nanos()) * u128::from(own) / u128::from(theirs);
            return Some(Duration::from_nanos(u64::try_from(ns).unwrap_or(u64::MAX)));
        }
        None
    }

    /// True when [`PrimitiveDescriptor::cost_duration`] for `op` on `profile` is not a
    /// figure published for this primitive on this profile but one derived from another
    /// (a proxy, a component sum is *not* derived — it is two published figures — while a
    /// cycle-ratio scaling is). `false` too when there is no figure at all.
    pub fn cost_is_derived(
        &self,
        op: PrimitiveOpKind,
        profile: &str,
        catalogue: &PrimitiveCatalogue,
    ) -> bool {
        if let Some(row) = self.cost.row(profile)
            && (row.duration(op).is_some() || self.cost_recipe.is_none())
        {
            return row.scaled && row.duration(op).is_some();
        }
        if self.cost_recipe.is_some() {
            return self.cost_duration(op, profile, catalogue).is_some();
        }
        if let Some(proxy) = self.cost_proxy {
            return catalogue
                .get(proxy)
                .is_some_and(|p| p.cost_duration(op, profile, catalogue).is_some());
        }
        if !self.cost_components.is_empty() {
            return self.cost_components.iter().any(|c| {
                catalogue
                    .get(*c)
                    .is_some_and(|d| d.cost_is_derived(op, profile, catalogue))
            });
        }
        self.cost_ratio.is_some() && self.cost_duration(op, profile, catalogue).is_some()
    }

    /// The modelled duration of `op` over an input of `bytes` bytes on `profile`.
    ///
    /// For a per-operation primitive this is [`PrimitiveDescriptor::cost_duration`]. For
    /// a hash or a cipher it is the per-unit figure times the units `bytes` costs
    /// ([`CostUnit::units`]): from this primitive's own row, or — for AES-128-CCM — from
    /// its proxy's per-block figure times CCM's own block-call count. `None` when no
    /// figure exists: an unpriced hash is not free, it is unknown.
    pub fn cost_for_len(
        &self,
        op: PrimitiveOpKind,
        profile: &str,
        bytes: u32,
        catalogue: &PrimitiveCatalogue,
    ) -> Option<Duration> {
        if self.cost_unit.is_per_operation() {
            return self.cost_duration(op, profile, catalogue);
        }
        self.cost_for_units(op, profile, self.cost_unit.units(bytes), catalogue)
    }

    fn cost_for_units(
        &self,
        op: PrimitiveOpKind,
        profile: &str,
        units: u64,
        catalogue: &PrimitiveCatalogue,
    ) -> Option<Duration> {
        if let Some(row) = self.cost.row(profile) {
            return row.duration_for_units(op, self.cost_unit, units);
        }
        let proxy = catalogue.get(self.cost_proxy?)?;
        // The proxy's figure is per *its* unit (an AES block call); `units` are this
        // primitive's own count of those calls.
        proxy
            .cost
            .row(profile)?
            .duration_for_units(op, proxy.cost_unit, units)
    }

    /// The nominal signature size as carried in the 1609.2 envelope.
    pub fn envelope_sig_bytes(&self) -> u32 {
        self.sig_bytes_encoded.unwrap_or(self.sig_bytes.nominal())
    }
}

/// The cost of a recipe's steps on one profile, or `None` when any step has no figure
/// there (a recipe that silently dropped a term would under-cost exactly the operation
/// it exists to price).
fn recipe_cost(
    steps: &[RecipeStep],
    profile: &str,
    catalogue: &PrimitiveCatalogue,
) -> Option<Duration> {
    if steps.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    for s in steps {
        let price = |p: PrimitiveId, op| {
            catalogue
                .get(p)?
                .cost_for_len(op, profile, s.bytes, catalogue)
        };
        let one =
            price(s.primitive, s.op).or_else(|| s.upper_bound.and_then(|(p, op)| price(p, op)))?;
        total = total.saturating_add(one.as_nanos().saturating_mul(u64::from(s.count)));
    }
    Some(Duration::from_nanos(total))
}

/// Every primitive descriptor, by id.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PrimitiveCatalogue {
    descriptors: Vec<PrimitiveDescriptor>,
}

/// The catalogue of 04-models.md §9.4.
static STANDARD: LazyLock<PrimitiveCatalogue> = LazyLock::new(build_standard_catalogue);

impl PrimitiveCatalogue {
    /// The standard catalogue: every primitive 04-models.md §9.4 tabulates.
    pub fn standard() -> &'static PrimitiveCatalogue {
        &STANDARD
    }

    /// A catalogue from a list of descriptors, sorted by id.
    pub fn new(mut descriptors: Vec<PrimitiveDescriptor>) -> PrimitiveCatalogue {
        descriptors.sort_by_key(|d| d.id);
        PrimitiveCatalogue { descriptors }
    }

    /// The descriptor for `id`.
    pub fn get(&self, id: PrimitiveId) -> Option<&PrimitiveDescriptor> {
        self.descriptors.iter().find(|d| d.id == id)
    }

    /// The descriptor for `id`, or [`SecError::UnknownPrimitive`].
    pub fn require(&self, id: PrimitiveId) -> Result<&PrimitiveDescriptor> {
        self.get(id)
            .ok_or(SecError::UnknownPrimitive { primitive: id })
    }

    /// Every descriptor, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &PrimitiveDescriptor> {
        self.descriptors.iter()
    }

    /// How many descriptors.
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// True when empty.
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    /// The cost of `op` of `primitive` over `bytes` bytes on a device priced by
    /// `anchors`: the first anchor with a figure, scaled as the anchor says.
    ///
    /// `None` when no anchor prices it — never a zero.
    pub fn price(
        &self,
        primitive: PrimitiveId,
        op: PrimitiveOpKind,
        bytes: u32,
        anchors: &[anchors::Anchor],
    ) -> Option<Priced> {
        let d = self.get(primitive)?;
        anchors.iter().find_map(|a| {
            let t = d.cost_for_len(op, a.profile, bytes, self)?;
            let ns = (u128::from(t.as_nanos()) * u128::from(a.num) + u128::from(a.den) / 2)
                / u128::from(a.den.max(1));
            Some(Priced {
                duration: Duration::from_nanos(u64::try_from(ns).unwrap_or(u64::MAX)),
                profile: a.profile,
                clock_scaled: a.is_scaled(),
                derived: a.is_scaled() || d.cost_is_derived(op, a.profile, self),
            })
        })
    }
}

/// One primitive operation this crate executes, the module that executes it, and a
/// representative input length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExecutedOp {
    /// The `v2xw_sec` module that runs it.
    pub module: &'static str,
    /// The primitive.
    pub primitive: PrimitiveId,
    /// Which operation.
    pub op: PrimitiveOpKind,
    /// A representative input length, for a per-length primitive.
    pub bytes: u32,
}

const fn exec(
    module: &'static str,
    primitive: PrimitiveId,
    op: PrimitiveOpKind,
    bytes: u32,
) -> ExecutedOp {
    ExecutedOp {
        module,
        primitive,
        op,
        bytes,
    }
}

/// Every primitive operation the crate's cryptographic modules run, by module.
///
/// The list a test walks to prove that every operation that can run in a simulation
/// costs time on every device that can run it (WP-C4 acceptance: "no zero-cost primitive
/// that runs"). A test also reads `lib.rs` and fails when a module appears that is
/// neither listed here nor declared free of cryptography, so a new primitive cannot
/// enter the crate unpriced.
pub const EXECUTED: &[ExecutedOp] = {
    use PrimitiveId as P;
    use PrimitiveOpKind::{KeyGen as K, Sign as S, Verify as V};
    &[
        exec("crypto", P::ECDSA_P256_SHA256, K, 0),
        exec("crypto", P::ECDSA_P256_SHA256, S, 0),
        exec("crypto", P::ECDSA_P256_SHA256, V, 0),
        exec("crypto", P::ECQV_P256, V, 0),
        exec("sig1609", P::ECDSA_P256_SHA256, S, 0),
        exec("sig1609", P::ECDSA_P256_SHA256, V, 0),
        exec("sig1609", P::SHA_256, V, 200),
        exec("p384", P::ECDSA_P384, S, 0),
        exec("p384", P::ECDSA_P384, V, 0),
        exec("p384", P::SHA_384, V, 200),
        exec("hashedid", P::SHA_256, V, 150),
        exec("cert", P::ECQV_P256, K, 0),
        exec("cert", P::ECQV_P256, S, 0),
        exec("cert", P::ECQV_P256, V, 0),
        exec("cert", P::SHA_256, V, 150),
        // WP-C1 implicit-credential keys: ECQV issuance (request key, CA contribution)
        // and the d·G = e·P_U + Q_ACA import check, plus the certificate hash e.
        exec("credential", P::ECQV_P256, K, 0),
        exec("credential", P::ECQV_P256, S, 0),
        exec("credential", P::ECQV_P256, V, 0),
        exec("credential", P::SHA_256, V, 150),
        exec("chain", P::ECDSA_P256_SHA256, V, 0),
        exec("chain", P::ECDSA_P384, V, 0),
        exec("chain", P::ECQV_P256, V, 0),
        exec("chain", P::SHA_256, V, 300),
        exec("chain", P::SHA_384, V, 300),
        exec("crl", P::SHA_256, V, 4_000),
        exec("crl", P::LINKAGE_VALUE, K, 0),
        exec("crl", P::LINKAGE_VALUE, V, 0),
        exec("ctl", P::SHA_256, V, 2_000),
        exec("ctl", P::SHA_384, V, 2_000),
        exec("ctl", P::ECDSA_P384, V, 0),
        exec("envelope", P::SHA_256, V, 300),
        exec("envelope", P::ECDSA_P256_SHA256, V, 0),
        exec("envelope", P::ECQV_P256, V, 0),
        exec("aes128", P::AES_128, S, 16),
        exec("ccm", P::AES_128_CCM, S, 100),
        exec("ccm", P::AES_128_CCM, V, 1_000),
        exec("ec", P::ECDH_P256, K, 0),
        exec("ec", P::ECDH_P256, V, 0),
        exec("ecies", P::ECIES_P256, K, 0),
        exec("ecies", P::ECIES_P256, S, 0),
        exec("ecies", P::ECIES_P256, V, 0),
        exec("kdf", P::SHA_256, V, 68),
        exec("butterfly", P::BUTTERFLY_EXPAND_P256, K, 0),
        exec("butterfly", P::BUTTERFLY_EXPAND_P256, S, 0),
        exec("butterfly", P::BUTTERFLY_EXPAND_P256, V, 0),
        exec("linkage", P::LINKAGE_VALUE, K, 0),
        exec("linkage", P::LINKAGE_VALUE, S, 0),
        exec("linkage", P::LINKAGE_VALUE, V, 0),
    ]
};

/// The `v2xw_sec` modules that run no cryptography of their own (types, errors, the
/// catalogue, scheme descriptors, the test PKI).
pub const MODULES_WITHOUT_CRYPTO: &[&str] = &["error", "primitive", "scheme", "testctx"];

/// One link of a certificate chain, as far as its validation cost is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainLink {
    /// An explicit certificate signed by an issuer with a P-256 or P-384 key.
    Explicit {
        /// The issuer's signature primitive (`ECDSA_P256_SHA256` or `ECDSA_P384`).
        signer: PrimitiveId,
        /// Length of the COER `ToBeSignedCertificate`.
        tbs_bytes: u32,
        /// Length of the COER issuer certificate.
        issuer_bytes: u32,
    },
    /// An implicit (ECQV) certificate.
    Implicit,
}

impl ChainLink {
    /// The primitive operations validating this link performs, as `(primitive, op,
    /// input bytes)`: for an explicit link, IEEE 1609.2 §5.3.1's signature input
    /// `H(H(tbs) ‖ H(issuer))` — three hash calls with the issuer's hash, SHA-384 for a
    /// P-384 issuer — then the verification; for an implicit link, the ECQV
    /// reconstruction (whose recipe carries its own hashes).
    pub fn ops(self) -> Vec<(PrimitiveId, PrimitiveOpKind, u32)> {
        use PrimitiveOpKind::Verify as V;
        match self {
            ChainLink::Explicit {
                signer,
                tbs_bytes,
                issuer_bytes,
            } => {
                let (h, d) = if signer == PrimitiveId::ECDSA_P384 {
                    (PrimitiveId::SHA_384, 48)
                } else {
                    (PrimitiveId::SHA_256, 32)
                };
                vec![
                    (h, V, tbs_bytes),
                    (h, V, issuer_bytes),
                    (h, V, 2 * d),
                    (signer, V, 0),
                ]
            }
            ChainLink::Implicit => vec![(PrimitiveId::ECQV_P256, V, 0)],
        }
    }
}

impl PrimitiveCatalogue {
    /// The total cost of `ops` on a device priced by `anchors`, or `None` when any one of
    /// them is unpriced (a validation cost that silently dropped a term would be an
    /// under-cost, which is the defect WP-C4 removes).
    pub fn price_all(
        &self,
        ops: &[(PrimitiveId, PrimitiveOpKind, u32)],
        anchors: &[anchors::Anchor],
    ) -> Option<Duration> {
        let mut total = 0u64;
        for &(p, op, bytes) in ops {
            total = total.saturating_add(self.price(p, op, bytes, anchors)?.duration.as_nanos());
        }
        Some(Duration::from_nanos(total))
    }
}

/// A priced operation: what it costs and which published figure the cost came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Priced {
    /// The modelled duration.
    pub duration: Duration,
    /// The catalogue profile whose figure was used.
    pub profile: &'static str,
    /// True when the figure was rescaled by clock or appliance tier.
    pub clock_scaled: bool,
    /// True when the figure is not one published for this primitive on that profile
    /// (scaled, proxied, or composed by a recipe).
    pub derived: bool,
}

/// A primitive as a registrable model: a descriptor plus its card.
///
/// `Primitive` is the `Family::Primitive` plug-in of 03-interfaces.md §6. It computes
/// nothing; what it *is* is a declared, cited set of sizes and costs, which is exactly
/// what a model card is for, so the card and the descriptor are the whole model.
#[derive(Debug, Clone)]
pub struct Primitive {
    descriptor: PrimitiveDescriptor,
    card: ModelCard,
}

impl Primitive {
    /// Wraps a descriptor, building its card.
    pub fn new(descriptor: PrimitiveDescriptor) -> Primitive {
        let card = card_for(&descriptor);
        Primitive { descriptor, card }
    }

    /// The sizes, level and costs.
    pub fn descriptor(&self) -> &PrimitiveDescriptor {
        &self.descriptor
    }
}

impl Model for Primitive {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

/// One [`Primitive`] model per descriptor in the standard catalogue.
///
/// The list the engine hands to [`v2xw_core::registry::Registry::register_model`], so
/// every primitive the scenario can name has a card and a content hash in the manifest.
pub fn standard_primitive_models() -> Vec<Primitive> {
    PrimitiveCatalogue::standard()
        .iter()
        .cloned()
        .map(Primitive::new)
        .collect()
}

/// The card for a descriptor: its sizes as parameters, its citations as sources.
fn card_for(d: &PrimitiveDescriptor) -> ModelCard {
    let mut card = ModelCard::new(
        d.id.as_str(),
        Family::Primitive,
        "1.0.0",
        format!(
            "Sizes, NIST security level and per-hardware-profile cost anchors for {} \
             (04-models.md §9.4). Declares cost and size; computes no cryptography.",
            d.id.as_str()
        ),
    );
    // Every tier: a size and a cost table are as correct at the abstract tier as at the
    // high tier. What changes with the tier is whether the *backend* runs the primitive,
    // and that is the backend's card, not this one.
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.sources = d.sources.clone();
    card.sources.extend(
        d.cost
            .rows()
            .iter()
            .map(|r| r.source.clone())
            .collect::<Vec<_>>(),
    );
    card.assumptions = d
        .notes
        .iter()
        .map(|n| (*n).to_string())
        .chain(
            d.cost
                .rows()
                .iter()
                .filter_map(|r| r.note.map(|n| format!("{}: {n}", r.profile))),
        )
        .collect();
    card.limitations = vec![
        "Declares sizes and costs only; a `CryptoBackend` decides whether the primitive \
         is actually executed."
            .to_string(),
    ];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: d.sources.clone(),
        tests: vec![
            "the_descriptor_sizes_are_the_published_ones".to_string(),
            "every_cost_row_carries_a_citation".to_string(),
            "every_executed_primitive_is_priced_on_every_device".to_string(),
        ],
    };
    card
}

// --------------------------------------------------------------------------------------
// The catalogue itself: 04-models.md §9.4, transcribed.
// --------------------------------------------------------------------------------------

/// Hardware-profile ids the cost tables key on.
///
/// These are the *model ids* of `Family::HardwareProfile` models. A scenario declares a
/// hardware profile once and refers to it by a dense [`v2xw_core::ids::HwProfileId`]; the
/// profile model carries this string, and that is the key a cost lookup uses. Two levels
/// of naming rather than one because a cost table is static, cited, cross-scenario data
/// and cannot be keyed by a number a scenario loader happened to assign.
pub mod profiles {
    /// Cortex-M4, Nordic nRF52840 at 64 MHz.
    pub const CORTEX_M4_NRF52840: &str = "hw-profile/cortex-m4-nrf52840-64mhz";
    /// Cortex-M4, STM32F407 at 168 MHz.
    pub const CORTEX_M4_STM32F407: &str = "hw-profile/cortex-m4-stm32f407-168mhz";
    /// Cortex-M4, the `pqm4` `m4f` benchmark platform at 24 MHz.
    pub const CORTEX_M4_PQM4: &str = "hw-profile/cortex-m4-pqm4-m4f-24mhz";
    /// Cortex-M7, STM32F767 at 216 MHz.
    pub const CORTEX_M7_STM32F767: &str = "hw-profile/cortex-m7-stm32f767-216mhz";
    /// Cortex-M33, STM32H563 at 250 MHz, wolfSSL.
    pub const CORTEX_M33_STM32H563_WOLFSSL: &str = "hw-profile/cortex-m33-stm32h563-250mhz-wolfssl";
    /// Cortex-M33, STM32H563 at 250 MHz, mbedTLS.
    pub const CORTEX_M33_STM32H563_MBEDTLS: &str = "hw-profile/cortex-m33-stm32h563-250mhz-mbedtls";
    /// Cortex-A53, Raspberry Pi 3B.
    pub const CORTEX_A53_PI3B: &str = "hw-profile/cortex-a53-pi3b";
    /// Cortex-A53, Raspberry Pi 3B+.
    pub const CORTEX_A53_PI3BPLUS: &str = "hw-profile/cortex-a53-pi3bplus";
    /// Cortex-A72, Raspberry Pi 4, OpenSSL 1.1.1d.
    pub const CORTEX_A72_PI4_OPENSSL: &str = "hw-profile/cortex-a72-pi4-openssl-1.1.1d";
    /// Cortex-A72, Raspberry Pi 4, liboqs.
    pub const CORTEX_A72_PI4_LIBOQS: &str = "hw-profile/cortex-a72-pi4-liboqs";
    /// Cortex-A76, Raspberry Pi 5, wolfSSL 5.9.1.
    pub const CORTEX_A76_PI5_WOLFSSL: &str = "hw-profile/cortex-a76-pi5-wolfssl-5.9.1";
    /// Cortex-A76, Raspberry Pi 5, mbedTLS 3.6.6.
    pub const CORTEX_A76_PI5_MBEDTLS: &str = "hw-profile/cortex-a76-pi5-mbedtls-3.6.6";
    /// Cortex-A76, Raspberry Pi 5, liboqs.
    pub const CORTEX_A76_PI5_LIBOQS: &str = "hw-profile/cortex-a76-pi5-liboqs";
    /// Intel i9-11950H, wolfSSL.
    pub const I9_11950H_WOLFSSL: &str = "hw-profile/i9-11950h-wolfssl";
    /// Intel i9-11950H, mbedTLS.
    pub const I9_11950H_MBEDTLS: &str = "hw-profile/i9-11950h-mbedtls";
    /// Intel i5-8259U at 2.3 GHz.
    pub const I5_8259U: &str = "hw-profile/i5-8259u-2.3ghz";
    /// Skylake, reference C implementation.
    pub const SKYLAKE_REF: &str = "hw-profile/skylake-ref-c";
    /// Skylake, AVX2 implementation.
    pub const SKYLAKE_AVX2: &str = "hw-profile/skylake-avx2";
    /// Skylake at 3.6 GHz, AVX2 (Falcon's own benchmark platform).
    pub const SKYLAKE_AVX2_3600: &str = "hw-profile/skylake-avx2-3.6ghz";
    /// Xeon E3-1220, reference C implementation.
    pub const XEON_E3_1220_REF: &str = "hw-profile/xeon-e3-1220-ref-c";
    /// Xeon E3-1220, AVX2 implementation.
    pub const XEON_E3_1220_AVX2: &str = "hw-profile/xeon-e3-1220-avx2";
    /// Cohda MK6 on-board unit, Botan.
    pub const COHDA_MK6_BOTAN: &str = "hw-profile/cohda-mk6-botan";
    /// Cohda MK6 on-board unit, liboqs.
    pub const COHDA_MK6_LIBOQS: &str = "hw-profile/cohda-mk6-liboqs";
    /// Infineon AURIX TC3xx hardware security module at 100 MHz.
    pub const AURIX_TC3XX_HSM: &str = "hw-profile/aurix-tc3xx-hsm-100mhz";
    /// CAMP's own planning assumption: a 2 GHz processor.
    pub const CAMP_PLANNING_2GHZ: &str = "hw-profile/camp-planning-2ghz";
    /// An x86-64 server, OpenSSL 1.1.1f, as Ristic's OpenSSL Cookbook ran `openssl speed`
    /// (CPU not stated) — the source `backend/cloud-vm-x86` already prices ECDSA from.
    pub const X86_OPENSSL_COOKBOOK: &str = "hw-profile/x86-64-openssl-1.1.1f-cookbook";
    /// Thales Luna Network HSM 7, A750/S750 enterprise tier.
    pub const LUNA7_A750: &str = "hw-profile/thales-luna7-a750";
    /// Thales Luna Network HSM 7, A790/S790 maximum tier.
    pub const LUNA7_A790: &str = "hw-profile/thales-luna7-a790";
    /// Thales TCT T-5000 (government) HSM: the one Thales sheet with a P-384 figure.
    pub const THALES_T5000: &str = "hw-profile/thales-tct-t5000";
}

/// Which catalogue profiles price each node hardware profile's cryptography.
///
/// A node's own hardware profile (`crates/v2xw-node/profiles/hardware/*.yaml`) publishes
/// ECDSA sign and verify figures for its secure element or accelerator, and almost
/// nothing else: no OBU vendor publishes a SHA-256, AES, CCM, ECIES or ECQV figure. Those
/// operations run, so they must cost time; the honest figure is the closest *published*
/// measurement of the same operation on comparable silicon, scaled by clock where the
/// node profile publishes a clock, and every such choice is written down here with its
/// reason. [`PrimitiveCatalogue::price`] walks a node's anchors in order and takes the
/// first that prices the operation.
pub mod anchors {
    use super::profiles as p;

    /// What kind of device a node hardware profile is.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum DeviceClass {
        /// An OBU whose keys live in a secure element or HSM, possibly with a separate
        /// verification accelerator.
        ObuHsm,
        /// An OBU doing all cryptography in software on its application processor.
        ObuCpu,
        /// A roadside unit.
        Rsu,
        /// A VRU device (handset class).
        Vru,
        /// A backend HSM appliance (RA, ACA, LA, MA, CRL signer keys).
        BackendHsm,
        /// A backend host CPU.
        BackendCpu,
        /// A node that performs no 1609.2 cryptography (a cellular base station).
        NoCrypto,
    }

    /// One catalogue profile a node's cost is read from, with an optional clock scaling.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
    pub struct Anchor {
        /// The catalogue profile.
        pub profile: &'static str,
        /// The catalogue figure is multiplied by `num / den` — the clock ratio
        /// `catalogue clock / node clock` when the node profile publishes its clock, or an
        /// appliance-tier ratio. `1/1` is unscaled.
        pub num: u64,
        /// See [`Anchor::num`].
        pub den: u64,
        /// Why this anchor stands in for the node's silicon.
        pub why: &'static str,
    }

    impl Anchor {
        const fn same(profile: &'static str, why: &'static str) -> Anchor {
            Anchor {
                profile,
                num: 1,
                den: 1,
                why,
            }
        }

        /// True when the figure is rescaled rather than used as published.
        pub const fn is_scaled(&self) -> bool {
            self.num != self.den
        }
    }

    /// A node hardware profile and the anchors that price its cryptography.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
    pub struct DeviceAnchors {
        /// The node hardware profile id (`id:` in its YAML).
        pub node_profile: &'static str,
        /// What kind of device it is.
        pub class: DeviceClass,
        /// The anchors, in the order they are tried.
        pub anchors: &'static [Anchor],
    }

    const PI3B_WHY: &str = "the slowest benchmarked Cortex-A (Pi 3B, Cortex-A53 @ 1.2 GHz, \
        OpenSSL 1.1.0f built for armhf, the same ARMv7 instruction set an in-order Cortex-A7 \
        or A9 runs) — the proxy the node profile itself names for its software path [R7 §D8]";

    /// Every node hardware profile shipped in `v2xw-node/profiles/hardware`.
    pub const DEVICES: &[DeviceAnchors] = &[
        DeviceAnchors {
            node_profile: "obu/unex-obu-301-craton2",
            class: DeviceClass::ObuHsm,
            anchors: &[Anchor {
                profile: p::CORTEX_A53_PI3B,
                num: 1_200,
                den: 600,
                why: "no CRATON2 figure for SHA, AES, ECDH or ECIES is published; the \
                      Pi 3B Cortex-A53 figure scaled by clock 1.2 GHz / 600 MHz to the \
                      CRATON2's dual Cortex-A7 [R7 §A4, §D8]",
            }],
        },
        DeviceAnchors {
            node_profile: "obu/unex-obu-301-craton2-software",
            class: DeviceClass::ObuCpu,
            anchors: &[Anchor {
                profile: p::CORTEX_A53_PI3B,
                num: 1_200,
                den: 600,
                why: "as obu/unex-obu-301-craton2: Pi 3B scaled by clock to 600 MHz",
            }],
        },
        DeviceAnchors {
            node_profile: "obu/cohda-mk5",
            class: DeviceClass::ObuHsm,
            anchors: &[Anchor::same(p::CORTEX_A53_PI3B, PI3B_WHY)],
        },
        DeviceAnchors {
            node_profile: "obu/cohda-mk6c-qualcomm-9150",
            class: DeviceClass::ObuHsm,
            // Not the NDSS 2024 MK6 Botan row: it publishes ECDSA only, and its verify
            // figure is the flagged 0.001 ms anomaly.
            anchors: &[Anchor {
                profile: p::CORTEX_A53_PI3B,
                num: 1_200,
                den: 800,
                why: "no MK6 SHA/AES/ECDH figure; Pi 3B scaled to the sibling MK6's \
                      published 800 MHz proxy clock [R7 §A2, §D8]",
            }],
        },
        DeviceAnchors {
            node_profile: "obu/generic-automotive-soc-no-hsm",
            class: DeviceClass::ObuCpu,
            anchors: &[Anchor::same(
                p::CORTEX_A72_PI4_OPENSSL,
                "the profile is the Pi 4B Cortex-A72 proxy; OpenSSL 1.1.1d [R7 §D8]",
            )],
        },
        DeviceAnchors {
            node_profile: "obu/pq-capable-hypothetical",
            class: DeviceClass::ObuCpu,
            anchors: &[
                Anchor::same(
                    p::CORTEX_A76_PI5_WOLFSSL,
                    "the profile is the Pi 5 Cortex-A76 proxy; wolfCrypt 5.9.1 [R5 §B.5]",
                ),
                Anchor::same(
                    p::CORTEX_A72_PI4_OPENSSL,
                    "P-384 is not in the Pi 5 benchmark; the Pi 4 OpenSSL figure is the \
                     nearest published (an over-estimate on a Cortex-A76)",
                ),
            ],
        },
        DeviceAnchors {
            node_profile: "rsu/cohda-mk5-rsu",
            class: DeviceClass::Rsu,
            anchors: &[Anchor::same(p::CORTEX_A53_PI3B, PI3B_WHY)],
        },
        DeviceAnchors {
            node_profile: "rsu/commsignia-its-rs4",
            class: DeviceClass::Rsu,
            anchors: &[Anchor {
                profile: p::CORTEX_A53_PI3B,
                num: 1_200_000,
                den: 792_000,
                why: "no RS4 crypto figure beyond ECDSA; Pi 3B scaled by clock to the \
                      published i.MX 6 792 MHz [R7 §C2, §D8]",
            }],
        },
        DeviceAnchors {
            node_profile: "vru-device/handset-generic",
            class: DeviceClass::Vru,
            anchors: &[Anchor::same(
                p::CORTEX_A72_PI4_OPENSSL,
                "the profile's own Cortex-A72-class assumption [R7 §D8]",
            )],
        },
        DeviceAnchors {
            node_profile: "backend/hsm-appliance-luna7",
            class: DeviceClass::BackendHsm,
            anchors: &[
                Anchor::same(p::LUNA7_A750, "the profile's own tier [R7 §F2]"),
                Anchor {
                    profile: p::LUNA7_A790,
                    num: 22_000,
                    den: 10_000,
                    why: "ECIES is published only for the A790 tier; scaled to the A750 by \
                          the two tiers' published ECC P-256 rates, 22,000 / 10,000 tps \
                          [R7 §F2]",
                },
                Anchor::same(
                    p::THALES_T5000,
                    "P-384 is published for no Luna 7 tier; the Thales T-5000 is the one \
                     Thales HSM with a P-384 figure [R7 §F2]",
                ),
                Anchor::same(
                    p::X86_OPENSSL_COOKBOOK,
                    "work the appliance does not publish runs on its x86 host [R7 §F1]",
                ),
                Anchor::same(
                    p::I9_11950H_WOLFSSL,
                    "hashing and ECDH on the host: the one x86 benchmark that publishes \
                     them [R5 §B.5]",
                ),
            ],
        },
        DeviceAnchors {
            node_profile: "backend/cloud-vm-x86",
            class: DeviceClass::BackendCpu,
            anchors: &[
                Anchor::same(
                    p::X86_OPENSSL_COOKBOOK,
                    "the source the profile prices ECDSA from [R7 §F1]",
                ),
                Anchor::same(
                    p::I9_11950H_WOLFSSL,
                    "hashing and ECDH: the one x86 benchmark that publishes them [R5 §B.5]",
                ),
            ],
        },
        DeviceAnchors {
            node_profile: "bs/lte-macro-generic",
            class: DeviceClass::NoCrypto,
            anchors: &[],
        },
    ];

    /// The anchors for node hardware profile `id`.
    pub fn for_node_profile(id: &str) -> Option<&'static DeviceAnchors> {
        DEVICES.iter().find(|d| d.node_profile == id)
    }
}

fn std_src(reference: &'static str) -> Citation {
    Source::new(SourceKind::Standard, reference)
}

fn paper(reference: &'static str) -> Citation {
    Source::new(SourceKind::Paper, reference)
}

fn code(reference: &'static str) -> Citation {
    Source::new(SourceKind::Code, reference)
}

fn sheet(reference: &'static str) -> Citation {
    Source::new(SourceKind::Datasheet, reference)
}

/// ECDSA P-256 cost anchors — 04-models.md §9.4, the whole ECDSA block.
fn ecdsa_p256_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::CORTEX_M4_NRF52840,
            CostSite::Cpu,
            code("Emill/P256-Cortex-M4"),
        )
        .at(64.0e6)
        .sign(OpCost::from_cycles_and_us(375_000, 5_900.0))
        .verify(OpCost::from_cycles_and_us(976_000, 15_300.0))
        .keygen(OpCost::from_cycles_and_us(327_000, 5_100.0)),
        CostRow::new(
            p::CORTEX_A72_PI4_OPENSSL,
            CostSite::Cpu,
            code("gist, OpenSSL 1.1.1d on Raspberry Pi 4"),
        )
        .sign(OpCost::from_ops_per_s(4_097.4))
        .verify(OpCost::from_ops_per_s(1_550.7))
        .note("operating system not stated by the source"),
        CostRow::new(p::CORTEX_A53_PI3B, CostSite::Cpu, code("gist [R7 §D8]"))
            .sign(OpCost::from_ops_per_s(1_631.1))
            .verify(OpCost::from_ops_per_s(775.3))
            .note("indicative only"),
        CostRow::new(p::CORTEX_A53_PI3BPLUS, CostSite::Cpu, code("gist [R7 §D8]"))
            .sign(OpCost::from_ops_per_s(1_914.7))
            .verify(OpCost::from_ops_per_s(908.4))
            .note("indicative only"),
        CostRow::new(
            p::CORTEX_A76_PI5_WOLFSSL,
            CostSite::Cpu,
            sheet("wolfSSL benchmark blog, wolfSSL 5.9.1"),
        )
        .verify(OpCost::from_ops_per_s(14_933.0)),
        CostRow::new(
            p::CORTEX_A76_PI5_MBEDTLS,
            CostSite::Cpu,
            sheet("wolfSSL benchmark blog, mbedTLS 3.6.6"),
        )
        .verify(OpCost::from_ops_per_s(592.0)),
        CostRow::new(
            p::I9_11950H_WOLFSSL,
            CostSite::Cpu,
            sheet("wolfSSL benchmark blog"),
        )
        .sign(OpCost::from_ops_per_s(64_194.0))
        .verify(OpCost::from_ops_per_s(61_357.0)),
        CostRow::new(
            p::I9_11950H_MBEDTLS,
            CostSite::Cpu,
            sheet("wolfSSL benchmark blog"),
        )
        .sign(OpCost::from_ops_per_s(4_227.0))
        .verify(OpCost::from_ops_per_s(1_244.0)),
        CostRow::new(
            p::CORTEX_M33_STM32H563_WOLFSSL,
            CostSite::Cpu,
            sheet("wolfSSL benchmark blog"),
        )
        .at(250.0e6)
        .verify(OpCost::from_ops_per_s(167.0))
        .note("the same blog's NUCLEO-F446ZE rows are UNVERIFIED and are not used"),
        CostRow::new(
            p::CORTEX_M33_STM32H563_MBEDTLS,
            CostSite::Cpu,
            sheet("wolfSSL benchmark blog"),
        )
        .at(250.0e6)
        .verify(OpCost::from_ops_per_s(12.1)),
        CostRow::new(
            p::CAMP_PLANNING_2GHZ,
            CostSite::Cpu,
            paper("CAMP EE Requirements 2016 p. 75"),
        )
        .at(2.0e9)
        .sign(OpCost::from_ops_per_s(1_500.0))
        .verify(OpCost::from_ops_per_s(300.0))
        .note("CAMP's own planning assumption, stated as \"about\""),
        CostRow::new(
            p::COHDA_MK6_BOTAN,
            CostSite::Cpu,
            paper("NDSS 2024 Table V"),
        )
        .sign(OpCost::from_us(7_820.0))
        .verify(OpCost::from_us(1.0))
        .note(
            "the published verify figure of 0.001 ms is anomalous — three orders of \
             magnitude faster than the sign on the same silicon — and is recorded as \
             published rather than corrected",
        ),
        CostRow::new(
            p::AURIX_TC3XX_HSM,
            CostSite::Hsm,
            sheet("Infineon AURIX training document [R7 §D]"),
        )
        .at(100.0e6)
        .sign(OpCost::from_ops_per_s(200.0))
        .verify(OpCost::from_ops_per_s(100.0)),
        CostRow::new(p::X86_OPENSSL_COOKBOOK, CostSite::Cpu, sheet(COOKBOOK))
            .sign(OpCost::from_ops_per_s(20_508.1))
            .verify(OpCost::from_ops_per_s(6_566.2))
            .note("CPU not stated by the source"),
        CostRow::new(p::LUNA7_A750, CostSite::Hsm, sheet(LUNA7))
            .sign(OpCost::from_ops_per_s(10_000.0))
            .verify(OpCost::from_ops_per_s(10_000.0))
            .note(LUNA_COMBINED),
        CostRow::new(p::LUNA7_A790, CostSite::Hsm, sheet(LUNA7))
            .sign(OpCost::from_ops_per_s(22_000.0))
            .verify(OpCost::from_ops_per_s(22_000.0))
            .note(LUNA_COMBINED),
    ])
}

// --------------------------------------------------------------------------------------
// WP-C4: the operations around the signature — hashing, AES, CCM, ECDH, ECIES, ECQV,
// P-384 — every one of which runs on a device or a backend and therefore costs time.
// Each figure is transcribed as its source published it (rate and chunk size, or a timed
// message and its length); the per-unit conversion happens on the way out
// (`CostRow::duration_for_units`).
// --------------------------------------------------------------------------------------

const GIST_PI3: &str = "gist dchest/6b624752d3b54200362f1e031c790f3d, `openssl speed`, OpenSSL \
     1.1.0f armhf (sha256 adb49576… 3B.txt, 3c8184c2… 3B-Plus.txt) [R7 §D8]";
const GIST_PI4: &str = "gist HimaJyun/f05d3017dfb05a4ccb0def010bb2c91a, `openssl speed`, OpenSSL \
     1.1.1d (sha256 50bcb60a… Pi4B-OpenSSL.txt) [R7 §D8]";
const WOLF: &str = "wolfSSL vs MbedTLS benchmark, 2026-06-26, 1024-byte blocks [R5 §B.5]";
const COOKBOOK: &str = "Ristic, OpenSSL Cookbook, 'Performance' chapter, OpenSSL 1.1.1f [R7 §F1]";
const LUNA7: &str = "Thales Luna Network HSM 7 product brief, per-tier performance [R7 §F2]";
const LUNA_COMBINED: &str = "the brief gives one ECC P-256 transactions-per-second figure, not \
     split into sign and verify; applied to both, as backend/hsm-appliance-luna7 does";
const SPEED_64: &str = "the 64-byte `openssl speed` column: two compression calls per message, \
     the size class of the hashes a 1609.2 receiver runs (tbsData, a certificate, KDF2 and \
     MAC1 inputs); the longer columns amortise a per-call overhead a short V2X hash pays";
const SPEED_16: &str = "the 16-byte `openssl speed` column: one block per call, the pattern of \
     the linkage-value and butterfly constructions; the key schedule is outside the timed \
     loop, so a construction that rekeys per call (a linkage seed as the AES key) is \
     under-costed by one key expansion per call";
const GCM_FOR_AES: &str = "AES-128-GCM throughput stands in for raw AES blocks: GCM is one AES \
     call per block plus GHASH, so the per-block figure is an upper bound of the AES call";

/// SHA-256 anchors, priced per 64-byte compression call ([`CostUnit::HashBlocks`]).
fn sha256_costs() -> CostTable {
    use profiles as p;
    const MIB: f64 = 1_048_576.0;
    const KIB: f64 = 1_024.0;
    CostTable::new(vec![
        CostRow::new(
            p::AURIX_TC3XX_HSM,
            CostSite::Hsm,
            sheet("Infineon AURIX TC3xx HSM training document [R7 §D1]"),
        )
        .at(100.0e6)
        .both(OpCost::from_cycles(65).over_units(1))
        .note(
            "65 clock cycles per 512-bit block on the SHA-224/256 accelerator; the same \
             document's '<2 µs latency per block' is an upper bound including setup and is \
             not charged",
        ),
        CostRow::new(
            p::CORTEX_M4_NRF52840,
            CostSite::Cpu,
            Source::new(
                SourceKind::Dataset,
                "Nordic DevZone thread 46695 (nrf52840), E. Thorsrud (Nordic): SHA-256 of a \
                 4,123-byte message, Oberon software backend 4.7413 ms, CC310 0.1596 ms",
            ),
        )
        .both(OpCost::from_us_over_bytes(4_741.3, 4_123))
        .note(
            "the Oberon software figure, for the software profile; the CryptoCell-310 \
             hardware figure is not this profile's path",
        ),
        CostRow::new(p::CORTEX_A53_PI3B, CostSite::Cpu, code(GIST_PI3))
            .both(OpCost::from_bytes_per_s(29_271.49e3, 64))
            .note(SPEED_64),
        CostRow::new(p::CORTEX_A53_PI3BPLUS, CostSite::Cpu, code(GIST_PI3))
            .both(OpCost::from_bytes_per_s(34_078.38e3, 64))
            .note(SPEED_64),
        CostRow::new(p::CORTEX_A72_PI4_OPENSSL, CostSite::Cpu, code(GIST_PI4))
            .both(OpCost::from_bytes_per_s(54_260.59e3, 64))
            .note(SPEED_64),
        CostRow::new(p::CORTEX_A76_PI5_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(1_464.0 * MIB, 1_024)),
        CostRow::new(p::CORTEX_A76_PI5_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(205.0 * MIB, 1_024)),
        CostRow::new(p::I9_11950H_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(1_703.0 * MIB, 1_024)),
        CostRow::new(p::I9_11950H_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(340.0 * MIB, 1_024)),
        CostRow::new(p::CORTEX_M33_STM32H563_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .both(OpCost::from_bytes_per_s(5_488.0 * KIB, 1_024)),
        CostRow::new(p::CORTEX_M33_STM32H563_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .both(OpCost::from_bytes_per_s(3_849.0 * KIB, 1_024)),
    ])
}

/// SHA-384 anchors, priced per 128-byte compression call. SHA-384 is SHA-512's
/// compression function with other initial values and a truncated output (FIPS 180-4
/// §6.5), so a SHA-512 throughput is a SHA-384 throughput.
fn sha384_costs() -> CostTable {
    use profiles as p;
    const MIB: f64 = 1_048_576.0;
    const KIB: f64 = 1_024.0;
    const SHA512: &str = "the source's SHA-512 row: SHA-384 runs the same compression function \
         (FIPS 180-4 §6.5)";
    CostTable::new(vec![
        CostRow::new(p::CORTEX_A53_PI3B, CostSite::Cpu, code(GIST_PI3))
            .both(OpCost::from_bytes_per_s(14_035.35e3, 64))
            .note(SHA512),
        CostRow::new(p::CORTEX_A53_PI3BPLUS, CostSite::Cpu, code(GIST_PI3))
            .both(OpCost::from_bytes_per_s(16_233.92e3, 64))
            .note(SHA512),
        CostRow::new(p::CORTEX_A72_PI4_OPENSSL, CostSite::Cpu, code(GIST_PI4))
            .both(OpCost::from_bytes_per_s(20_136.47e3, 64))
            .note(SHA512),
        CostRow::new(p::CORTEX_A76_PI5_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(416.0 * MIB, 1_024)),
        CostRow::new(p::CORTEX_A76_PI5_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(225.0 * MIB, 1_024))
            .note(SHA512),
        CostRow::new(p::I9_11950H_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(852.0 * MIB, 1_024)),
        CostRow::new(p::I9_11950H_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(516.0 * MIB, 1_024))
            .note(SHA512),
        CostRow::new(p::CORTEX_M33_STM32H563_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .both(OpCost::from_bytes_per_s(2_387.0 * KIB, 1_024))
            .note("code placed in SRAM; from cached flash the source measures 1,858 KiB/s"),
        CostRow::new(p::CORTEX_M33_STM32H563_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .both(OpCost::from_bytes_per_s(2_271.0 * KIB, 1_024))
            .note(SHA512),
    ])
}

/// AES-128 block-call anchors ([`CostUnit::CipherBlocks`]).
fn aes128_costs() -> CostTable {
    use profiles as p;
    const MIB: f64 = 1_048_576.0;
    const KIB: f64 = 1_024.0;
    CostTable::new(vec![
        CostRow::new(
            p::CORTEX_M4_NRF52840,
            CostSite::Cpu,
            Source::new(
                SourceKind::Dataset,
                "Nordic DevZone thread 46695 (nrf52840), E. Thorsrud (Nordic): AES-CCM-128, \
                 message 16 B, associated data 250 B, tag 8 B, nonce 13 B: 0.6879 ms in \
                 mbed TLS software (0.1389 ms on CC310)",
            ),
        )
        .both(OpCost::from_us(687.9).over_units(20))
        .note(
            "the CCM call is 20 AES calls (SP 800-38C): CBC-MAC over B0, 16 blocks of the \
             2-byte-length-prefixed 250-byte associated data and 1 payload block, plus CTR \
             over A0 and 1 payload block; the figure includes CCM's own bookkeeping, so the \
             per-call cost is an upper bound",
        ),
        CostRow::new(p::CORTEX_A53_PI3B, CostSite::Cpu, code(GIST_PI3))
            .both(OpCost::from_bytes_per_s(41_225.96e3, 16))
            .note(SPEED_16),
        CostRow::new(p::CORTEX_A53_PI3BPLUS, CostSite::Cpu, code(GIST_PI3))
            .both(OpCost::from_bytes_per_s(47_570.23e3, 16))
            .note(SPEED_16),
        CostRow::new(p::CORTEX_A72_PI4_OPENSSL, CostSite::Cpu, code(GIST_PI4))
            .both(OpCost::from_bytes_per_s(75_181.94e3, 16))
            .note(SPEED_16),
        CostRow::new(p::X86_OPENSSL_COOKBOOK, CostSite::Cpu, sheet(COOKBOOK))
            .both(OpCost::from_bytes_per_s(421_949.23e3, 16))
            .note("`openssl speed -evp aes-128-cbc` (AES-NI), 16-byte column"),
        CostRow::new(p::CORTEX_A76_PI5_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(2_104.0 * MIB, 1_024))
            .note(GCM_FOR_AES),
        CostRow::new(p::CORTEX_A76_PI5_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(489.0 * MIB, 1_024))
            .note(GCM_FOR_AES),
        CostRow::new(p::I9_11950H_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(7_663.0 * MIB, 1_024))
            .note(GCM_FOR_AES),
        CostRow::new(p::I9_11950H_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .both(OpCost::from_bytes_per_s(445.0 * MIB, 1_024))
            .note(GCM_FOR_AES),
        CostRow::new(p::CORTEX_M33_STM32H563_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .both(OpCost::from_bytes_per_s(5_175.0 * KIB, 1_024))
            .note("AES-128-CBC: one AES call per block"),
        CostRow::new(p::CORTEX_M33_STM32H563_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .both(OpCost::from_bytes_per_s(2_832.0 * KIB, 1_024))
            .note("AES-128-CBC: one AES call per block"),
        CostRow::new(
            p::CAMP_PLANNING_2GHZ,
            CostSite::Cpu,
            paper("CAMP SCMS PoC EE Requirements and Specifications, SCMS Software Release 1.1 (2016) [R7 §F4]"),
        )
        .at(2.0e9)
        .both(OpCost::from_bytes_per_s(81.0e6, 16))
        .note("\"81 MB/s encryption on a 2 GHz processor\", chunk size not stated; read per block"),
    ])
}

/// AES-128-CCM rows of its own: only the HSMs, which publish a per-transaction rate.
/// Everywhere else CCM is priced as its exact count of AES calls ([`CostUnit::CcmAesCalls`])
/// through the AES-128 proxy.
fn aes128_ccm_costs() -> CostTable {
    use profiles as p;
    const PER_CALL: &str = "AES-GCM transactions per second, payload size unstated: charged once \
         per CCM call whatever its length, with GCM standing in for CCM (both one AES pass \
         plus a MAC pass)";
    CostTable::new(vec![
        CostRow::new(p::LUNA7_A750, CostSite::Hsm, sheet(LUNA7))
            .both(OpCost::from_ops_per_s(10_000.0))
            .note(PER_CALL),
        CostRow::new(p::LUNA7_A790, CostSite::Hsm, sheet(LUNA7))
            .both(OpCost::from_ops_per_s(17_000.0))
            .note(PER_CALL),
    ])
}

/// ECDH P-256 anchors. `verify` is the shared-secret derivation (one variable-base scalar
/// multiplication), `keygen` the key pair (one fixed-base multiplication), the KEM
/// convention [`CostRow`] documents.
fn ecdh_p256_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::CORTEX_M4_NRF52840,
            CostSite::Cpu,
            code("Emill/P256-Cortex-M4 README [R5 §B]"),
        )
        .at(64.0e6)
        .keygen(OpCost::from_cycles_and_us(327_000, 5_100.0))
        .verify(OpCost::from_cycles(906_000)),
        CostRow::new(p::CORTEX_A53_PI3B, CostSite::Cpu, code(GIST_PI3))
            .verify(OpCost::from_ops_per_s(1_033.0)),
        CostRow::new(p::CORTEX_A53_PI3BPLUS, CostSite::Cpu, code(GIST_PI3))
            .verify(OpCost::from_ops_per_s(1_213.5)),
        CostRow::new(p::CORTEX_A72_PI4_OPENSSL, CostSite::Cpu, code(GIST_PI4))
            .verify(OpCost::from_ops_per_s(2_342.9)),
        CostRow::new(p::CORTEX_A76_PI5_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .verify(OpCost::from_ops_per_s(17_586.0)),
        CostRow::new(p::CORTEX_A76_PI5_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .verify(OpCost::from_ops_per_s(637.0)),
        CostRow::new(p::I9_11950H_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .verify(OpCost::from_ops_per_s(68_400.0)),
        CostRow::new(p::I9_11950H_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .verify(OpCost::from_ops_per_s(1_277.0)),
        CostRow::new(p::CORTEX_M33_STM32H563_WOLFSSL, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .verify(OpCost::from_ops_per_s(175.0)),
        CostRow::new(p::CORTEX_M33_STM32H563_MBEDTLS, CostSite::Cpu, sheet(WOLF))
            .at(250.0e6)
            .verify(OpCost::from_ops_per_s(18.6)),
    ])
}

/// ECDSA P-384 anchors: the root and elector signatures a chain validation and a CTL
/// check verify (01 §3.1).
fn ecdsa_p384_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(p::CORTEX_A53_PI3B, CostSite::Cpu, code(GIST_PI3))
            .sign(OpCost::from_ops_per_s(358.4))
            .verify(OpCost::from_ops_per_s(94.1)),
        CostRow::new(p::CORTEX_A53_PI3BPLUS, CostSite::Cpu, code(GIST_PI3))
            .sign(OpCost::from_ops_per_s(419.6))
            .verify(OpCost::from_ops_per_s(109.7)),
        CostRow::new(p::CORTEX_A72_PI4_OPENSSL, CostSite::Cpu, code(GIST_PI4))
            .sign(OpCost::from_ops_per_s(123.4))
            .verify(OpCost::from_ops_per_s(174.8))
            .note(
                "recorded as published: OpenSSL 1.1.1d signs P-384 slower here than 1.1.0f \
                 does on the slower Pi 3B (no constant-time nistp384 assembly in 1.1.1d)",
            ),
        CostRow::new(p::X86_OPENSSL_COOKBOOK, CostSite::Cpu, sheet(COOKBOOK))
            .sign(OpCost::from_ops_per_s(580.4))
            .verify(OpCost::from_ops_per_s(755.0))
            .note("CPU not stated by the source"),
        CostRow::new(
            p::THALES_T5000,
            CostSite::Hsm,
            sheet("Thales TCT HSM family sheet, T-5000 Enterprise, ECC P-384 [R7 §F2]"),
        )
        .sign(OpCost::from_ops_per_s(16_000.0))
        .verify(OpCost::from_ops_per_s(16_000.0))
        .note("one ECC P-384 tps figure, applied to sign and verify"),
    ])
}

/// ECIES P-256 rows of its own: only the published HSM decrypt rate. Everywhere else
/// ECIES is priced by its recipe.
fn ecies_p256_costs() -> CostTable {
    CostTable::new(vec![
        CostRow::new(
            profiles::LUNA7_A790,
            CostSite::Hsm,
            sheet("Thales Luna HSM for 5G performance brief [R7 §F2]"),
        )
        .verify(OpCost::from_ops_per_s(7_000.0))
        .note(
            "56,000 TPS ECIES P-256 decrypt from a cluster of 8 A790 appliances, divided \
             by 8 (derived per appliance); the brief's workload is 5G SUCI concealment, \
             whose ECIES differs from 1609.2's only in KDF parameters",
        ),
    ])
}

const SHA: PrimitiveId = PrimitiveId::SHA_256;
const AES: PrimitiveId = PrimitiveId::AES_128;
const ECDH: PrimitiveId = PrimitiveId::ECDH_P256;
const V: PrimitiveOpKind = PrimitiveOpKind::Verify;

/// ECIES (1609.2 §5.3.5; 02 §8.2): `v = k·G`, `Z = k·R`, `K = KDF2(Z, 48, P1)` — two
/// SHA-256 calls over `Z ‖ counter ‖ P1` (68 bytes) — and `t = MAC1(K2, c)`, an
/// HMAC-SHA-256 over a 16-byte `c` with a 32-byte key: inner hash 64 + 16 bytes, outer
/// 64 + 32 bytes.
const ECIES_STEPS_SHARED: [RecipeStep; 4] = [
    RecipeStep::new(ECDH, V, 0, 1),
    RecipeStep::new(SHA, V, 68, 2),
    RecipeStep::new(SHA, V, 80, 1),
    RecipeStep::new(SHA, V, 96, 1),
];
const ECIES_ENCRYPT: [RecipeStep; 5] = [
    RecipeStep::mul_base(1),
    ECIES_STEPS_SHARED[0],
    ECIES_STEPS_SHARED[1],
    ECIES_STEPS_SHARED[2],
    ECIES_STEPS_SHARED[3],
];
/// ECDSA key generation where a row publishes only sign and verify: one signature, an
/// upper bound of the fixed-base multiplication.
const ECDSA_KEYGEN_RECIPE: CostRecipe = CostRecipe {
    keygen: &[RecipeStep::new(
        PrimitiveId::ECDSA_P256_SHA256,
        PrimitiveOpKind::Sign,
        0,
        1,
    )],
    sign: &[],
    verify: &[],
};

/// `openssl speed ecdh` times the derivation only; the key pair is a fixed-base
/// multiplication, priced as such where no row publishes it.
const ECDH_RECIPE: CostRecipe = CostRecipe {
    keygen: &[RecipeStep::mul_base(1)],
    sign: &[],
    verify: &[],
};
const ECIES_RECIPE: CostRecipe = CostRecipe {
    keygen: &[RecipeStep::mul_base(1)],
    sign: &ECIES_ENCRYPT,
    verify: &ECIES_STEPS_SHARED,
};

/// ECQV as 1609.2 profiles it (02 §6): `e = Hn(H(tbs) ‖ H(issuer))` is one SHA-256 over
/// the to-be-signed certificate (bounded by the 80-byte implicit certificate) and one over
/// the two 32-byte digests; the issuer's digest is its HashedId8 input, computed once per
/// CA certificate and cached. Reconstruction is `Q_U = e·P_U + Q_CA` — one variable-base
/// multiplication, the ECDH operation; issuance is `P_U = R_U + k·G` — one fixed-base
/// multiplication.
const ECQV_RECIPE: CostRecipe = CostRecipe {
    keygen: &[RecipeStep::mul_base(1)],
    sign: &[
        RecipeStep::mul_base(1),
        RecipeStep::new(SHA, V, 80, 1),
        RecipeStep::new(SHA, V, 64, 1),
    ],
    verify: &[
        RecipeStep::new(ECDH, V, 0, 1),
        RecipeStep::new(SHA, V, 80, 1),
        RecipeStep::new(SHA, V, 64, 1),
    ],
};

/// The original butterfly expansion (IEEE 1609.2.1 §9.3; 02 §9.2): `f_int` is three AES
/// calls, and the original scheme expands a signing and an encryption key per `(i, j)`.
const BUTTERFLY_RECIPE: CostRecipe = CostRecipe {
    // The caterpillar key pairs `A = a·G`, `P = p·G`.
    keygen: &[RecipeStep::mul_base(2)],
    // The RA's public expansion: `A + f1(i,j)·G` and `P + f2(i,j)·G`.
    sign: &[RecipeStep::new(AES, V, 16, 6), RecipeStep::mul_base(2)],
    // The device's private expansion: `a + f1(i,j)` and `p + f2(i,j)`.
    verify: &[RecipeStep::new(AES, V, 16, 6)],
};

/// Linkage values (02 §10.1): a seed step is one SHA-256 over 32 bytes per LA; a linkage
/// value is one AES (Davies–Meyer) per LA, XORed.
const LINKAGE_RECIPE: CostRecipe = CostRecipe {
    // Advance both LAs' seeds one i-period.
    keygen: &[RecipeStep::new(SHA, V, 32, 2)],
    // One `lv(i, j)` from the two seeds — the LA's PLVs, or a receiver expanding a CRL
    // entry.
    sign: &[RecipeStep::new(AES, V, 16, 2)],
    verify: &[RecipeStep::new(AES, V, 16, 2)],
};

fn ml_dsa_44_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::SKYLAKE_REF,
            CostSite::Cpu,
            paper("CRYSTALS-Dilithium round-3 specification Table 1"),
        )
        .sign(OpCost::from_cycles(1_081_174))
        .verify(OpCost::from_cycles(327_362))
        .keygen(OpCost::from_cycles(300_751))
        .note("Dilithium2; the round-3 sizes differ slightly from FIPS 204 ML-DSA-44"),
        CostRow::new(
            p::SKYLAKE_AVX2,
            CostSite::Cpu,
            paper("CRYSTALS-Dilithium round-3 specification Table 1"),
        )
        .sign(OpCost::from_cycles(259_172))
        .verify(OpCost::from_cycles(118_412))
        .keygen(OpCost::from_cycles(124_031)),
        CostRow::new(
            p::CORTEX_M4_PQM4,
            CostSite::Cpu,
            code("pqm4 benchmarks.csv"),
        )
        .at(24.0e6)
        .sign(OpCost::from_cycles_with_max(3_943_121, 17_009_165, 24.0e6))
        .verify(OpCost::from_cycles(1_421_623))
        .keygen(OpCost::from_cycles(1_426_025))
        .note("sign is a mean over a 1,812,557-17,009,165 cycle spread (rejection sampling)"),
        CostRow::new(
            p::CORTEX_M7_STM32F767,
            CostSite::Cpu,
            paper("NIST 2022 ARM paper Table I"),
        )
        .at(216.0e6)
        .sign(OpCost::from_cycles(3_658_000))
        .verify(OpCost::from_cycles_and_us(1_429_000, 6_600.0))
        .keygen(OpCost::from_cycles(1_437_000)),
        CostRow::new(
            p::CORTEX_A72_PI4_LIBOQS,
            CostSite::Cpu,
            paper("arXiv 2503.10238 Table 8"),
        )
        .sign(OpCost::from_ops_per_s(286.1))
        .verify(OpCost::from_ops_per_s(3_213.9))
        .keygen(OpCost::from_ops_per_s(2_642.7)),
        CostRow::new(
            p::CORTEX_A76_PI5_LIBOQS,
            CostSite::Cpu,
            paper("arXiv 2503.10238 Table 8"),
        )
        .sign(OpCost::from_ops_per_s(1_885.0))
        .verify(OpCost::from_ops_per_s(8_139.0))
        .keygen(OpCost::from_ops_per_s(8_986.3)),
        CostRow::new(
            p::COHDA_MK6_BOTAN,
            CostSite::Cpu,
            paper("NDSS 2024 Table V"),
        )
        .sign(OpCost::from_us(2_634.0))
        .verify(OpCost::from_us(189.0))
        .note("published as \"Dilithium\" without a parameter set; taken as Dilithium2"),
    ])
}

fn ml_dsa_65_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::SKYLAKE_REF,
            CostSite::Cpu,
            paper("CRYSTALS-Dilithium round-3 specification Table 1"),
        )
        .sign(OpCost::from_cycles(1_713_783))
        .verify(OpCost::from_cycles(522_267))
        .keygen(OpCost::from_cycles(544_232)),
        CostRow::new(
            p::SKYLAKE_AVX2,
            CostSite::Cpu,
            paper("CRYSTALS-Dilithium round-3 specification Table 1"),
        )
        .sign(OpCost::from_cycles(428_587))
        .verify(OpCost::from_cycles(179_424))
        .keygen(OpCost::from_cycles(256_403)),
        CostRow::new(
            p::CORTEX_M4_PQM4,
            CostSite::Cpu,
            code("pqm4 benchmarks.csv"),
        )
        .at(24.0e6)
        .sign(OpCost::from_cycles(6_193_171))
        .verify(OpCost::from_cycles(2_415_944))
        .keygen(OpCost::from_cycles(2_516_006)),
        CostRow::new(
            p::CORTEX_M7_STM32F767,
            CostSite::Cpu,
            paper("NIST 2022 ARM paper Table I"),
        )
        .at(216.0e6)
        .sign(OpCost::from_cycles(6_009_000))
        .verify(OpCost::from_cycles_and_us(2_453_000, 11_400.0))
        .keygen(OpCost::from_cycles(2_566_000)),
    ])
}

fn ml_dsa_87_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::SKYLAKE_REF,
            CostSite::Cpu,
            paper("CRYSTALS-Dilithium round-3 specification Table 1"),
        )
        .sign(OpCost::from_cycles(2_383_399))
        .verify(OpCost::from_cycles(871_609))
        .keygen(OpCost::from_cycles(819_475)),
        CostRow::new(
            p::SKYLAKE_AVX2,
            CostSite::Cpu,
            paper("CRYSTALS-Dilithium round-3 specification Table 1"),
        )
        .sign(OpCost::from_cycles(538_986))
        .verify(OpCost::from_cycles(279_936))
        .keygen(OpCost::from_cycles(298_050)),
        CostRow::new(
            p::CORTEX_M4_PQM4,
            CostSite::Cpu,
            code("pqm4 benchmarks.csv"),
        )
        .at(24.0e6)
        .sign(OpCost::from_cycles(7_947_380))
        .verify(OpCost::from_cycles(4_193_104))
        .keygen(OpCost::from_cycles(4_275_859)),
        CostRow::new(
            p::CORTEX_M7_STM32F767,
            CostSite::Cpu,
            paper("NIST 2022 ARM paper Table I"),
        )
        .at(216.0e6)
        .sign(OpCost::from_cycles(8_157_000))
        .verify(OpCost::from_cycles_and_us(4_287_000, 19_800.0))
        .keygen(OpCost::from_cycles(4_368_000)),
    ])
}

fn falcon_512_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::SKYLAKE_AVX2_3600,
            CostSite::Cpu,
            paper("Pornin, ePrint 2019/893 §5.2"),
        )
        .at(3.6e9)
        .sign(OpCost::from_cycles_and_us(948_132, 263.37))
        .verify(OpCost::from_cycles_and_us(81_036, 22.51))
        .keygen(OpCost::from_cycles_and_us(26_604_000, 7_390.0))
        .note("dynamic signing; the expanded-tree variant signs in 467,964 cycles"),
        CostRow::new(p::I5_8259U, CostSite::Cpu, code("falcon-sign.info"))
            .at(2.3e9)
            .sign(OpCost::from_ops_per_s(5_948.1))
            .verify(OpCost::from_ops_per_s(27_933.0))
            .keygen(OpCost::from_us(8_640.0)),
        CostRow::new(
            p::CORTEX_M4_STM32F407,
            CostSite::Cpu,
            paper("Pornin, ePrint 2019/893 §5.3"),
        )
        .at(168.0e6)
        .sign(OpCost::from_cycles_and_us(43_301_915, 257_750.0))
        .verify(OpCost::from_cycles_and_us(504_051, 3_000.0))
        .keygen(OpCost::from_cycles_and_us(171_294_112, 1_019_610.0))
        .note("floating-point emulation; the expanded-tree variant signs in 21,155,551 cycles"),
        CostRow::new(
            p::CORTEX_M7_STM32F767,
            CostSite::Cpu,
            paper("NIST 2022 ARM paper Table II"),
        )
        .at(216.0e6)
        .sign(OpCost::from_cycles_and_us(4_778_000, 22_100.0))
        .verify(OpCost::from_cycles_and_us(559_000, 2_600.0))
        .keygen(OpCost::from_cycles(77_475_000))
        .note("with the hardware FPU"),
        CostRow::new(
            p::CORTEX_A72_PI4_LIBOQS,
            CostSite::Cpu,
            paper("arXiv 2503.10238 Table 8"),
        )
        .sign(OpCost::from_ops_per_s(615.3))
        .verify(OpCost::from_ops_per_s(7_866.3))
        .keygen(OpCost::from_ops_per_s(23.6)),
        CostRow::new(
            p::CORTEX_A76_PI5_LIBOQS,
            CostSite::Cpu,
            paper("arXiv 2503.10238 Table 8"),
        )
        .sign(OpCost::from_ops_per_s(3_360.6))
        .verify(OpCost::from_ops_per_s(19_831.3))
        .keygen(OpCost::from_ops_per_s(95.4)),
        CostRow::new(
            p::COHDA_MK6_LIBOQS,
            CostSite::Cpu,
            paper("NDSS 2024 Table V"),
        )
        .sign(OpCost::from_us(2_152.0))
        .verify(OpCost::from_us(446.0)),
    ])
}

fn falcon_1024_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::SKYLAKE_AVX2_3600,
            CostSite::Cpu,
            paper("Pornin, ePrint 2019/893 §5.2"),
        )
        .at(3.6e9)
        .sign(OpCost::from_cycles(1_926_252))
        .verify(OpCost::from_cycles_and_us(160_596, 44.61))
        .keygen(OpCost::from_cycles(79_164_000))
        .note("dynamic signing; the expanded-tree variant signs in 942,768 cycles"),
        CostRow::new(
            p::CORTEX_M7_STM32F767,
            CostSite::Cpu,
            paper("NIST 2022 ARM paper Table II"),
        )
        .at(216.0e6)
        .sign(OpCost::from_cycles_and_us(10_243_000, 47_400.0))
        .verify(OpCost::from_cycles_and_us(1_136_000, 5_300.0))
        .keygen(OpCost::from_cycles(193_707_000))
        .note("with the hardware FPU"),
    ])
}

fn slh_dsa_128s_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::XEON_E3_1220_REF,
            CostSite::Cpu,
            paper("SPHINCS+ round-3.1 specification Table 4"),
        )
        .sign(OpCost::from_cycles(2_721_595_944))
        .verify(OpCost::from_cycles(2_712_044))
        .keygen(OpCost::from_cycles(358_061_994))
        .note("simple variant"),
        CostRow::new(
            p::XEON_E3_1220_AVX2,
            CostSite::Cpu,
            paper("SPHINCS+ round-3.1 specification Table 6"),
        )
        .sign(OpCost::from_cycles(644_740_090))
        .verify(OpCost::from_cycles(861_478)),
        CostRow::new(
            p::CORTEX_M4_PQM4,
            CostSite::Cpu,
            code("pqm4 benchmarks.csv, `clean` implementation"),
        )
        .at(24.0e6)
        .sign(OpCost::from_cycles(7_657_558_168))
        .verify(OpCost::from_cycles(7_471_794))
        .keygen(OpCost::from_cycles(1_007_731_522)),
        CostRow::new(
            p::COHDA_MK6_LIBOQS,
            CostSite::Cpu,
            paper("NDSS 2024 Table V"),
        )
        .sign(OpCost::from_us(5_485.0))
        .verify(OpCost::from_us(5_436.0))
        .note(
            "published as \"SPHINCS+\" with no parameter set; the 5.485 ms sign is \
             anomalous next to the reference 2.7e9 cycles and is recorded as published \
             rather than corrected",
        ),
    ])
}

fn slh_dsa_128f_costs() -> CostTable {
    use profiles as p;
    CostTable::new(vec![
        CostRow::new(
            p::XEON_E3_1220_REF,
            CostSite::Cpu,
            paper("SPHINCS+ round-3.1 specification Table 4"),
        )
        .sign(OpCost::from_cycles(138_610_500))
        .verify(OpCost::from_cycles(7_757_942))
        .keygen(OpCost::from_cycles(5_590_602))
        .note("simple variant"),
        CostRow::new(
            p::XEON_E3_1220_AVX2,
            CostSite::Cpu,
            paper("SPHINCS+ round-3.1 specification Table 6"),
        )
        .sign(OpCost::from_cycles(33_651_546))
        .verify(OpCost::from_cycles(2_150_290)),
        CostRow::new(
            p::CORTEX_M4_PQM4,
            CostSite::Cpu,
            code("pqm4 benchmarks.csv, `clean` implementation"),
        )
        .at(24.0e6)
        .sign(OpCost::from_cycles(368_575_228))
        .verify(OpCost::from_cycles(21_923_628))
        .keygen(OpCost::from_cycles(15_742_990)),
    ])
}

fn ed25519_costs() -> CostTable {
    CostTable::new(vec![
        CostRow::new(
            profiles::CORTEX_A72_PI4_OPENSSL,
            CostSite::Cpu,
            paper("arXiv 2503.10238 Table 7"),
        )
        .sign(OpCost::from_ops_per_s(2_939.1))
        .verify(OpCost::from_ops_per_s(1_327.0)),
    ])
}

fn sig(
    id: PrimitiveId,
    pk: u32,
    sk: u32,
    sig_bytes: SizeSpec,
    level: u8,
    cost: CostTable,
    sources: Vec<Citation>,
) -> PrimitiveDescriptor {
    PrimitiveDescriptor {
        id,
        family: PrimitiveFamily::Signature,
        pk_bytes: pk,
        pk_bytes_uncompressed: None,
        sk_bytes: sk,
        sig_bytes,
        sig_bytes_encoded: None,
        shared_secret_bytes: None,
        cert_bytes: None,
        security_level: level,
        cost,
        cost_components: Vec::new(),
        cost_proxy: None,
        cost_ratio: None,
        cost_unit: CostUnit::PerOperation,
        cost_recipe: None,
        sources,
        notes: Vec::new(),
    }
}

/// ML-KEM cost anchors (FIPS 203). `sign` is encapsulation and `verify` decapsulation,
/// the convention [`CostRow`] documents for a KEM.
///
/// Two sources: the `pqm4` Cortex-M4 `m4fspeed` cycle counts (benchmarks.md, fetched
/// 2026-10-07), and liboqs on a Raspberry Pi 4 and 5 from the same Table 8 of arXiv
/// 2503.10238 that the ML-DSA-44 and Falcon-512 rows cite. ML-KEM-1024 is not in Table 8;
/// it is derived from ML-KEM-768 by the `pqm4` ratio ([`CostRatio`]).
fn ml_kem_costs(which: u16) -> CostTable {
    use profiles as p;
    const PQM4: &str = "pqm4 benchmarks.md, m4fspeed implementation (fetched 2026-10-07)";
    const TABLE8: &str = "arXiv 2503.10238 Table 8 (liboqs, reference code)";
    let (kg, enc, dec) = match which {
        512 => (392_423, 390_881, 428_167),
        768 => (642_096, 658_754, 707_827),
        _ => (1_018_976, 1_031_565, 1_094_008),
    };
    let mut rows = vec![
        CostRow::new(p::CORTEX_M4_PQM4, CostSite::Cpu, code(PQM4))
            .at(24.0e6)
            .keygen(OpCost::from_cycles(kg))
            .sign(OpCost::from_cycles(enc))
            .verify(OpCost::from_cycles(dec))
            .note("sign = encapsulate, verify = decapsulate"),
    ];
    let pi = match which {
        512 => Some(((3_807.3, 3_462.0, 2_783.0), (23_168.3, 19_178.0, 15_136.3))),
        768 => Some(((2_539.0, 3_905.0, 2_571.5), (13_976.6, 11_900.6, 9_614.3))),
        _ => None,
    };
    if let Some(((kg4, enc4, dec4), (kg5, enc5, dec5))) = pi {
        rows.push(
            CostRow::new(p::CORTEX_A72_PI4_LIBOQS, CostSite::Cpu, paper(TABLE8))
                .keygen(OpCost::from_ops_per_s(kg4))
                .sign(OpCost::from_ops_per_s(enc4))
                .verify(OpCost::from_ops_per_s(dec4))
                .note("sign = encapsulate, verify = decapsulate"),
        );
        rows.push(
            CostRow::new(p::CORTEX_A76_PI5_LIBOQS, CostSite::Cpu, paper(TABLE8))
                .keygen(OpCost::from_ops_per_s(kg5))
                .sign(OpCost::from_ops_per_s(enc5))
                .verify(OpCost::from_ops_per_s(dec5))
                .note("sign = encapsulate, verify = decapsulate"),
        );
    }
    CostTable::new(rows)
}

fn kem(id: PrimitiveId, ek: u32, dk: u32, ct: u32, ss: u32, level: u8) -> PrimitiveDescriptor {
    PrimitiveDescriptor {
        id,
        family: PrimitiveFamily::Kem,
        pk_bytes: ek,
        pk_bytes_uncompressed: None,
        sk_bytes: dk,
        sig_bytes: SizeSpec::Fixed(ct),
        sig_bytes_encoded: None,
        shared_secret_bytes: Some(ss),
        cert_bytes: None,
        security_level: level,
        cost: ml_kem_costs(match level {
            1 => 512,
            3 => 768,
            _ => 1024,
        }),
        cost_components: Vec::new(),
        cost_proxy: None,
        cost_ratio: None,
        cost_unit: CostUnit::PerOperation,
        cost_recipe: None,
        sources: vec![std_src("FIPS 203 Table 3")],
        notes: vec![
            "`sig_bytes` is the ciphertext size; `shared_secret_bytes` is the shared secret.",
            "Cost anchors: pqm4 m4fspeed cycles on a Cortex-M4 and liboqs on a \
             Raspberry Pi 4 and 5 (arXiv 2503.10238 Table 8); a profile with neither is \
             derived by the pqm4 cycle ratio to a sibling (`cost_ratio`).",
        ],
    }
}

/// Builds the catalogue of 04-models.md §9.4.
fn build_standard_catalogue() -> PrimitiveCatalogue {
    let mut v: Vec<PrimitiveDescriptor> = Vec::new();

    // --- classical signatures ----------------------------------------------------------
    let mut ecdsa = sig(
        PrimitiveId::ECDSA_P256_SHA256,
        33,
        32,
        SizeSpec::Fixed(64),
        1,
        ecdsa_p256_costs(),
        vec![
            std_src("SEC 1 §2.3.3"),
            std_src("IEEE 1609.2 Ieee1609Dot2BaseTypes.asn"),
            std_src("FIPS 186-4"),
        ],
    );
    ecdsa.pk_bytes_uncompressed = Some(65);
    ecdsa.sig_bytes_encoded = Some(66);
    ecdsa.cost_recipe = Some(ECDSA_KEYGEN_RECIPE);
    ecdsa.cert_bytes = Some(147);
    ecdsa.notes = vec![
        "66 encoded bytes under IEEE 1609.2: `Signature` choice tag 1 + `rSig` \
         (`EccP256CurvePoint` choice tag 1 + 32) + `sSig` 32 (04-models.md §9.1).",
        "`cert_bytes` is the DERIVED explicit-certificate size of §9.2 (NDSS's \
         30 + pk + sig accounting gives 162 for the same certificate).",
        "Key generation, where a profile publishes none, is charged as one signature: a \
         fixed-base multiplication plus an inversion, so an upper bound (WP-C4).",
    ];
    v.push(ecdsa);

    let mut bp256 = sig(
        PrimitiveId::ECDSA_BRAINPOOL_P256R1,
        33,
        32,
        SizeSpec::Fixed(64),
        1,
        CostTable::default(),
        vec![std_src("RFC 5639 §3.4, Annex A")],
    );
    bp256.pk_bytes_uncompressed = Some(65);
    bp256.sig_bytes_encoded = Some(66);
    bp256.cost_proxy = Some(PrimitiveId::ECDSA_P256_SHA256);
    bp256.notes = vec![
        "Cost anchors are the P-256 ones: the two curves have the same field size and the \
         same operation count, and no brainpool-specific figures were found.",
    ];
    v.push(bp256);

    let mut p384 = sig(
        PrimitiveId::ECDSA_P384,
        49,
        48,
        SizeSpec::Fixed(96),
        3,
        ecdsa_p384_costs(),
        vec![
            std_src("IEEE 1609.2 Ieee1609Dot2BaseTypes.asn"),
            std_src("SEC 1 §2.3.3"),
        ],
    );
    p384.pk_bytes_uncompressed = Some(97);
    p384.sig_bytes_encoded = Some(98);
    p384.notes = vec![
        "Covers brainpoolP384r1 too; 04-models.md §9.4 gives the two the same sizes.",
        "04-models.md §9.4 listed no P-384 anchor; WP-C4 adds the published `openssl speed` \
         and HSM figures. The P-256 anchors are never a proxy: P-384 is materially slower.",
        "§9.4 states the level as \"192-bit\" rather than as a NIST category; 3 is the \
         category that 192-bit classical strength sits in.",
    ];
    v.push(p384);

    let mut ed = sig(
        PrimitiveId::ED25519,
        32,
        32,
        SizeSpec::Fixed(64),
        1,
        ed25519_costs(),
        vec![std_src("RFC 8032 §7")],
    );
    ed.notes = vec![
        "Legacy stand-in only: the Python reference pipeline signed with Ed25519 for \
         reproducibility, and no IEEE 1609.2 or TS 103 097 profile admits it.",
    ];
    v.push(ed);

    // --- implicit certificates ---------------------------------------------------------
    v.push(PrimitiveDescriptor {
        id: PrimitiveId::ECQV_P256,
        family: PrimitiveFamily::ImplicitCert,
        pk_bytes: 33,
        pk_bytes_uncompressed: Some(65),
        sk_bytes: 32,
        // The "signature" of an implicit certificate scheme is the certificate itself:
        // there is no separate signature, which is the whole point of the construction.
        sig_bytes: SizeSpec::Fixed(80),
        sig_bytes_encoded: None,
        shared_secret_bytes: None,
        cert_bytes: Some(80),
        security_level: 1,
        cost: CostTable::default(),
        cost_components: Vec::new(),
        cost_proxy: None,
        cost_ratio: None,
        cost_unit: CostUnit::PerOperation,
        cost_recipe: Some(ECQV_RECIPE),
        sources: vec![std_src("SEC 4 §3.4-3.5"), std_src("SEC 1 §2.3.3")],
        notes: vec![
            "`pk_bytes` 33 is the reconstruction value, a compressed point (§9.2, VERIFIED).",
            "`cert_bytes` 80 is the DERIVED implicit pseudonym certificate size of §9.2; \
             cross-checks are Rostami's 250 - 180 = 70 B delta and VSC-A's 117.",
            "No published ECQV timing was found, so the cost is the recipe of the operations \
             1609.2's ECQV performs (02 §6): reconstruction is one variable-base point \
             multiplication (priced as ECDH P-256) and two SHA-256 calls; issuance one \
             fixed-base multiplication and the same hashes. Left out: one point addition \
             and the decompression of P_U (a field square root), measured on the host by \
             `benches/primitives.rs` at a few per cent of the multiplication.",
            "WP-C4 replaced the earlier proxy (the ECDSA P-256 verify figure, a double \
             multiplication): reconstruction now costs about as much as ECDH, not as an \
             ECDSA verification.",
        ],
    });

    // --- hashes ------------------------------------------------------------------------
    let hash = |id, digest, unit, cost, source, note| PrimitiveDescriptor {
        id,
        family: PrimitiveFamily::Hash,
        pk_bytes: 0,
        pk_bytes_uncompressed: None,
        sk_bytes: 0,
        sig_bytes: SizeSpec::Fixed(digest),
        sig_bytes_encoded: None,
        shared_secret_bytes: None,
        cert_bytes: None,
        security_level: 1,
        cost,
        cost_components: Vec::new(),
        cost_proxy: None,
        cost_ratio: None,
        cost_unit: unit,
        cost_recipe: None,
        sources: vec![source],
        notes: vec![note],
    };
    v.push(hash(
        PrimitiveId::SHA_256,
        32,
        CostUnit::HashBlocks {
            block: 64,
            length_field: 8,
        },
        sha256_costs(),
        std_src("FIPS 180-4"),
        "`sig_bytes` is the digest length. Priced per 64-byte compression call after \
         padding (WP-C4); a verify plan's hashes cost time on every profile with a row, \
         and are unknown — not free — on one without.",
    ));
    let mut sha384 = hash(
        PrimitiveId::SHA_384,
        48,
        CostUnit::HashBlocks {
            block: 128,
            length_field: 16,
        },
        sha384_costs(),
        std_src("FIPS 180-4 §6.5"),
        "Priced per 128-byte compression call: the hash of the P-384 root and elector \
         signatures (01 §3.1).",
    );
    sha384.security_level = 3;
    v.push(sha384);

    // --- symmetric ---------------------------------------------------------------------
    let symmetric = |id, family, unit, cost, proxy, sources, notes| PrimitiveDescriptor {
        id,
        family,
        pk_bytes: 0,
        pk_bytes_uncompressed: None,
        sk_bytes: 16,
        sig_bytes: SizeSpec::Fixed(16),
        sig_bytes_encoded: None,
        shared_secret_bytes: None,
        cert_bytes: None,
        security_level: 1,
        cost,
        cost_components: Vec::new(),
        cost_proxy: proxy,
        cost_ratio: None,
        cost_unit: unit,
        cost_recipe: None,
        sources,
        notes,
    };
    v.push(symmetric(
        PrimitiveId::AES_128,
        PrimitiveFamily::BlockCipher,
        CostUnit::CipherBlocks { block: 16 },
        aes128_costs(),
        None,
        vec![std_src("FIPS 197")],
        vec![
            "Priced per 16-byte block call; `sig_bytes` is the block. The key schedule is \
             not priced separately (see the rows' notes).",
        ],
    ));
    v.push(symmetric(
        PrimitiveId::AES_128_CCM,
        PrimitiveFamily::Aead,
        CostUnit::CcmAesCalls,
        aes128_ccm_costs(),
        Some(PrimitiveId::AES_128),
        vec![std_src("NIST SP 800-38C §6"), std_src("IEEE 1609.2 §5.3.8")],
        vec![
            "`sig_bytes` is the 16-byte tag. Priced as its exact AES-call count, \
             2·⌈n/16⌉ + 2 for an n-byte plaintext with no associated data, times the \
             AES-128 per-call figure on the same profile; an HSM that publishes a \
             per-transaction rate is charged that instead.",
        ],
    ));

    // --- key agreement -------------------------------------------------------------------
    let agreement = |id, cost, recipe, ct, notes| PrimitiveDescriptor {
        id,
        family: PrimitiveFamily::KeyAgreement,
        pk_bytes: 33,
        pk_bytes_uncompressed: Some(65),
        sk_bytes: 32,
        sig_bytes: SizeSpec::Fixed(ct),
        sig_bytes_encoded: None,
        shared_secret_bytes: Some(32),
        cert_bytes: None,
        security_level: 1,
        cost,
        cost_components: Vec::new(),
        cost_proxy: None,
        cost_ratio: None,
        cost_unit: CostUnit::PerOperation,
        cost_recipe: recipe,
        sources: vec![std_src("SEC 1 §3.3.1"), std_src("IEEE 1609.2 §5.3.5")],
        notes,
    };
    v.push(agreement(
        PrimitiveId::ECDH_P256,
        ecdh_p256_costs(),
        Some(ECDH_RECIPE),
        33,
        vec![
            "`verify` is the shared-secret derivation (one variable-base multiplication), \
             `keygen` the key pair (one fixed-base multiplication); `sig_bytes` is the \
             compressed public point sent.",
        ],
    ));
    v.push(agreement(
        PrimitiveId::ECIES_P256,
        ecies_p256_costs(),
        Some(ECIES_RECIPE),
        // `EciesP256EncryptedKey`: v (33, compressed) + c (16) + t (16).
        65,
        vec![
            "`sign` is encryption, `verify` decryption. Priced by its recipe (one fixed-base \
             and one variable-base multiplication to encrypt, the variable-base one to \
             decrypt, and six SHA-256 calls for KDF2 and MAC1) except where an HSM \
             publishes a rate. Left out: the 16-byte XOR and the on-curve check of v.",
            "`sig_bytes` 65 is the COER `EciesP256EncryptedKey` with v compressed \
             (33 + 16 + 16).",
        ],
    ));

    // --- SCMS constructions ----------------------------------------------------------------
    let construction = |id, recipe, sources, notes| PrimitiveDescriptor {
        id,
        family: PrimitiveFamily::Construction,
        pk_bytes: 0,
        pk_bytes_uncompressed: None,
        sk_bytes: 0,
        sig_bytes: SizeSpec::Fixed(0),
        sig_bytes_encoded: None,
        shared_secret_bytes: None,
        cert_bytes: None,
        security_level: 1,
        cost: CostTable::default(),
        cost_components: Vec::new(),
        cost_proxy: None,
        cost_ratio: None,
        cost_unit: CostUnit::PerOperation,
        cost_recipe: Some(recipe),
        sources,
        notes,
    };
    v.push(construction(
        PrimitiveId::BUTTERFLY_EXPAND_P256,
        BUTTERFLY_RECIPE,
        vec![std_src("IEEE 1609.2.1 §9.3"), std_src("CAMP bfkeyexp")],
        vec![
            "One (i, j) of the original (US-profile) butterfly: `keygen` the caterpillar key \
             pairs, `sign` the RA's public expansion (six AES calls, two fixed-base \
             multiplications), `verify` the device's private expansion (six AES calls). \
             Left out: two point additions and two scalar additions.",
        ],
    ));
    v.push(construction(
        PrimitiveId::LINKAGE_VALUE,
        LINKAGE_RECIPE,
        vec![std_src("CAMP lv.txt (02-cryptography-and-formats §10.1)")],
        vec![
            "`keygen` advances both LAs' seeds one i-period (two SHA-256 calls), `sign` and \
             `verify` compute one lv(i, j) (two AES calls) — the LA's work, and a receiver's \
             per-entry, per-j CRL expansion. Left out: the 9-byte XOR.",
        ],
    ));

    // --- ML-DSA (FIPS 204) -------------------------------------------------------------
    for (id, pk, sk, s, level, cost) in [
        (
            PrimitiveId::ML_DSA_44,
            1_312u32,
            2_560u32,
            2_420u32,
            2u8,
            ml_dsa_44_costs(),
        ),
        (
            PrimitiveId::ML_DSA_65,
            1_952,
            4_032,
            3_309,
            3,
            ml_dsa_65_costs(),
        ),
        (
            PrimitiveId::ML_DSA_87,
            2_592,
            4_896,
            4_627,
            5,
            ml_dsa_87_costs(),
        ),
    ] {
        let mut d = sig(
            id,
            pk,
            sk,
            SizeSpec::Fixed(s),
            level,
            cost,
            vec![std_src("FIPS 204 Table 2")],
        );
        d.notes = vec![
            "The private key may instead be stored as a 32-byte seed and expanded \
             (FIPS 204 §3.6), which is what a constrained OBU would do.",
            "Cost rows citing the Dilithium round-3 specification are for the round-3 \
             parameter set, whose sizes differ slightly from the FIPS 204 ones above.",
        ];
        v.push(d);
    }

    // --- Falcon ------------------------------------------------------------------------
    let mut f512 = sig(
        PrimitiveId::FALCON_512,
        897,
        1_281,
        SizeSpec::Variable {
            mean: 666,
            max: 752,
        },
        1,
        falcon_512_costs(),
        vec![
            code("PQClean falcon-padded-512 api.h"),
            code("falcon-sign.info"),
        ],
    );
    f512.notes = vec![
        "666 is the *padded* signature size PQClean's `falcon-padded-512` always emits; \
         the unpadded encoding is variable and bounded by 752.",
        "FIPS 206 draft status is UNVERIFIED.",
    ];
    v.push(f512);

    let mut f1024 = sig(
        PrimitiveId::FALCON_1024,
        1_793,
        2_305,
        SizeSpec::Variable {
            mean: 1_280,
            max: 1_462,
        },
        5,
        falcon_1024_costs(),
        vec![code("PQClean falcon-padded-1024 api.h")],
    );
    f1024.notes =
        vec!["1,280 is the padded size; the unpadded encoding is variable and bounded by 1,462."];
    v.push(f1024);

    // --- SLH-DSA (FIPS 205) ------------------------------------------------------------
    for (id, pk, sk, s, level, cost) in [
        (
            PrimitiveId::SLH_DSA_SHA2_128S,
            32u32,
            64u32,
            7_856u32,
            1u8,
            slh_dsa_128s_costs(),
        ),
        (
            PrimitiveId::SLH_DSA_SHA2_128F,
            32,
            64,
            17_088,
            1,
            slh_dsa_128f_costs(),
        ),
        (
            PrimitiveId::SLH_DSA_SHA2_192S,
            48,
            96,
            16_224,
            3,
            CostTable::default(),
        ),
        (
            PrimitiveId::SLH_DSA_SHA2_192F,
            48,
            96,
            35_664,
            3,
            CostTable::default(),
        ),
        (
            PrimitiveId::SLH_DSA_SHA2_256S,
            64,
            128,
            29_792,
            5,
            CostTable::default(),
        ),
        (
            PrimitiveId::SLH_DSA_SHA2_256F,
            64,
            128,
            49_856,
            5,
            CostTable::default(),
        ),
    ] {
        let mut d = sig(
            id,
            pk,
            sk,
            SizeSpec::Fixed(s),
            level,
            cost,
            vec![std_src("FIPS 205 Table 2")],
        );
        d.notes = vec![
            "The SHAKE parameter set has identical sizes; only the hash family differs.",
            "04-models.md §9.4 publishes cost anchors for the 128s and 128f parameter \
             sets only; the 192 and 256 sets carry no cost anchor and none is inferred, \
             because SLH-DSA's cost scales with its tree parameters and not with its \
             signature size.",
        ];
        v.push(d);
    }

    // --- ML-KEM (FIPS 203) -------------------------------------------------------------
    v.push(kem(PrimitiveId::ML_KEM_512, 800, 1_632, 768, 32, 1));
    v.push(kem(PrimitiveId::ML_KEM_768, 1_184, 2_400, 1_088, 32, 3));
    v.push(kem(PrimitiveId::ML_KEM_1024, 1_568, 3_168, 1_568, 32, 5));

    // --- hybrids -----------------------------------------------------------------------
    let mut h44 = sig(
        PrimitiveId::HYBRID_MLDSA44_ECDSA_P256,
        1_345,
        2_592,
        SizeSpec::Variable {
            mean: 2_484,
            max: 2_492,
        },
        2,
        CostTable::default(),
        vec![
            std_src("FIPS 204 Table 2 + SEC 1 §2.3.3 (raw concatenation, derived)"),
            std_src("draft-ietf-lamps-pq-composite-sigs-19 Appendix A (composite)"),
        ],
    );
    h44.notes = vec![
        "pk 1,345 = 1,312 ML-DSA-44 + 33 compressed P-256, raw; the composite encoding is \
         1,377.",
        "sig mean 2,484 = 2,420 + 64 raw; the composite maximum is 2,492.",
        "sk 2,592 = 2,560 + 32, derived.",
    ];
    h44.cost_components = vec![PrimitiveId::ML_DSA_44, PrimitiveId::ECDSA_P256_SHA256];
    h44.notes.push(
        "No benchmark of the hybrid as a unit was found. Signing or verifying one \
         means doing both halves, so its cost is the sum of the ML-DSA-44 and \
         ECDSA P-256 anchors on the same hardware profile.",
    );
    v.push(h44);

    let mut h65 = sig(
        PrimitiveId::HYBRID_MLDSA65_ECDSA_P256,
        2_017,
        83,
        SizeSpec::Variable {
            mean: 3_373,
            max: 3_381,
        },
        3,
        CostTable::default(),
        vec![std_src("draft-ietf-lamps-pq-composite-sigs-19")],
    );
    h65.notes = vec![
        "The composite figures, as §9.4 gives them: pk 2,017 composite, sk 83, sig maximum \
         3,381. The mean is the raw 3,309 + 64 = 3,373 concatenation.",
        "sk 83 is the composite *seed* encoding, not an expanded key.",
    ];
    h65.cost_components = vec![PrimitiveId::ML_DSA_65, PrimitiveId::ECDSA_P256_SHA256];
    h65.notes.push(
        "No benchmark of the hybrid as a unit was found. Signing or verifying one \
         means doing both halves, so its cost is the sum of the ML-DSA-65 and \
         ECDSA P-256 anchors on the same hardware profile.",
    );
    v.push(h65);

    let mut hf = sig(
        PrimitiveId::HYBRID_FALCON512_ECDSA_P256,
        930,
        1_313,
        SizeSpec::Variable {
            mean: 730,
            max: 816,
        },
        1,
        CostTable::default(),
        vec![Source::new(
            SourceKind::Paper,
            "derived from PQClean falcon-padded-512 + SEC 1",
        )],
    );
    hf.notes = vec![
        "pk 930 = 897 + 33 raw and sig 730 = 666 + 64 raw, as §9.4 derives them; the \
         maximum follows from Falcon's unpadded 752 + 64.",
        "sk 1,313 = 1,281 + 32, derived.",
    ];
    hf.cost_components = vec![PrimitiveId::FALCON_512, PrimitiveId::ECDSA_P256_SHA256];
    hf.notes.push(
        "No benchmark of the hybrid as a unit was found. Signing or verifying one \
         means doing both halves, so its cost is the sum of the Falcon-512 and \
         ECDSA P-256 anchors on the same hardware profile.",
    );
    v.push(hf);

    // --- derived costs ------------------------------------------------------------------
    // Where a primitive publishes no figure on a V2X-relevant profile, it is scaled from
    // a sibling that does by the two primitives' cycle ratio on a platform that publishes
    // both. Each pairing is a sibling of the same construction, so the ratio carries the
    // parameter-set difference and not an architecture difference — except the SLH-DSA
    // and ML-KEM ones, whose base is ML-DSA-44 because nothing of their own family is
    // published on a Cortex-A or a Cohda unit; those are the weakest figures in the
    // catalogue and `cost_is_derived` says so for every one of them.
    for d in &mut v {
        let ratio = |base, reference| Some(CostRatio { base, reference });
        d.cost_ratio = match d.id {
            PrimitiveId::ML_DSA_65 | PrimitiveId::ML_DSA_87 => {
                ratio(PrimitiveId::ML_DSA_44, profiles::CORTEX_M4_PQM4)
            }
            PrimitiveId::FALCON_1024 => {
                ratio(PrimitiveId::FALCON_512, profiles::CORTEX_M7_STM32F767)
            }
            PrimitiveId::SLH_DSA_SHA2_128S | PrimitiveId::SLH_DSA_SHA2_128F => {
                ratio(PrimitiveId::ML_DSA_44, profiles::CORTEX_M4_PQM4)
            }
            PrimitiveId::ML_KEM_512 | PrimitiveId::ML_KEM_768 => {
                ratio(PrimitiveId::ML_DSA_44, profiles::CORTEX_M4_PQM4)
            }
            PrimitiveId::ML_KEM_1024 => ratio(PrimitiveId::ML_KEM_768, profiles::CORTEX_M4_PQM4),
            _ => d.cost_ratio,
        };
    }

    // --- primitives contributed by registered schemes ------------------------------------
    // A scheme module that introduces a primitive the catalogue does not have declares it
    // beside the scheme (`crate::scheme::SignatureScheme::defines`), and it joins here, so
    // adding a scheme is one module and one registry line.
    for d in crate::scheme::contributed_primitives() {
        if !v.iter().any(|x| x.id == d.id) {
            v.push(d);
        }
    }

    PrimitiveCatalogue::new(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sizes 04-models.md §9.4 publishes, spot-checked against the catalogue. These
    /// numbers drive every channel-load and certificate-size result the simulator
    /// produces, so they are pinned here rather than trusted to a transcription.
    #[test]
    fn the_descriptor_sizes_are_the_published_ones() {
        let c = PrimitiveCatalogue::standard();
        let e = c.require(PrimitiveId::ECDSA_P256_SHA256).expect("present");
        assert_eq!(e.pk_bytes, 33);
        assert_eq!(e.pk_bytes_uncompressed, Some(65));
        assert_eq!(e.sk_bytes, 32);
        assert_eq!(e.sig_bytes, SizeSpec::Fixed(64));
        assert_eq!(e.sig_bytes_encoded, Some(66));
        assert_eq!(e.security_level, 1);

        for (id, pk, sk, s) in [
            (PrimitiveId::ML_DSA_44, 1_312, 2_560, 2_420),
            (PrimitiveId::ML_DSA_65, 1_952, 4_032, 3_309),
            (PrimitiveId::ML_DSA_87, 2_592, 4_896, 4_627),
        ] {
            let d = c.require(id).expect("present");
            assert_eq!((d.pk_bytes, d.sk_bytes), (pk, sk), "{id}");
            assert_eq!(d.sig_bytes, SizeSpec::Fixed(s), "{id}");
        }

        let f = c.require(PrimitiveId::FALCON_512).expect("present");
        assert_eq!(f.pk_bytes, 897);
        assert_eq!(f.sk_bytes, 1_281);
        assert_eq!(
            f.sig_bytes,
            SizeSpec::Variable {
                mean: 666,
                max: 752
            }
        );
        assert_eq!(f.sig_bytes.nominal(), 666);
        assert_eq!(f.sig_bytes.max(), 752);

        let s = c.require(PrimitiveId::SLH_DSA_SHA2_128S).expect("present");
        assert_eq!((s.pk_bytes, s.sk_bytes), (32, 64));
        assert_eq!(s.sig_bytes, SizeSpec::Fixed(7_856));
        let f128 = c.require(PrimitiveId::SLH_DSA_SHA2_128F).expect("present");
        assert_eq!(f128.sig_bytes, SizeSpec::Fixed(17_088));

        let k = c.require(PrimitiveId::ML_KEM_768).expect("present");
        assert_eq!((k.pk_bytes, k.sk_bytes), (1_184, 2_400));
        assert_eq!(k.sig_bytes, SizeSpec::Fixed(1_088));
        assert_eq!(k.shared_secret_bytes, Some(32));

        let q = c.require(PrimitiveId::ECQV_P256).expect("present");
        assert_eq!(
            q.pk_bytes, 33,
            "the reconstruction value is a compressed point"
        );
        assert_eq!(q.cert_bytes, Some(80));
        assert_eq!(q.family, PrimitiveFamily::ImplicitCert);

        let h = c
            .require(PrimitiveId::HYBRID_MLDSA44_ECDSA_P256)
            .expect("present");
        assert_eq!(h.pk_bytes, 1_345, "1,312 + 33 raw");
        assert_eq!(h.sig_bytes.nominal(), 2_484, "2,420 + 64 raw");
        assert_eq!(h.sig_bytes.max(), 2_492, "composite maximum");
    }

    /// No black box: a cost row without a citation is a number nobody can check.
    #[test]
    fn every_cost_row_carries_a_citation() {
        for d in PrimitiveCatalogue::standard().iter() {
            for row in d.cost.rows() {
                assert!(
                    !row.source.reference.trim().is_empty(),
                    "{} / {} has an empty citation",
                    d.id,
                    row.profile
                );
                assert!(
                    row.keygen.is_some() || row.sign.is_some() || row.verify.is_some(),
                    "{} / {} has a row with no costs at all",
                    d.id,
                    row.profile
                );
                assert!(!row.scaled, "a published row must not be tagged scaled");
            }
        }
    }

    /// The two figures that are implausible as published must stay flagged. A future
    /// editor who "tidies" one of them away removes the only warning a reader gets.
    #[test]
    fn the_anomalous_published_figures_stay_flagged() {
        let c = PrimitiveCatalogue::standard();
        let cohda = c
            .require(PrimitiveId::ECDSA_P256_SHA256)
            .expect("present")
            .cost
            .row(profiles::COHDA_MK6_BOTAN)
            .expect("the Cohda row is present");
        assert_eq!(cohda.verify.expect("verify").mean_us, Some(1.0));
        assert!(
            cohda.note.expect("a note").contains("anomalous"),
            "the 0.001 ms verify must stay flagged"
        );
        let sphincs = c
            .require(PrimitiveId::SLH_DSA_SHA2_128S)
            .expect("present")
            .cost
            .row(profiles::COHDA_MK6_LIBOQS)
            .expect("the Cohda row is present");
        assert!(sphincs.note.expect("a note").contains("anomalous"));
    }

    /// A cost is charged as an integer duration, whichever form the source published it
    /// in. Cycles need a clock; a throughput does not.
    #[test]
    fn a_cost_becomes_a_duration_whatever_form_it_was_published_in() {
        let c = PrimitiveCatalogue::standard();
        let ecdsa = c.require(PrimitiveId::ECDSA_P256_SHA256).expect("present");

        // Published as cycles *and* milliseconds: the published wall time wins.
        let m4 = ecdsa
            .cost
            .row(profiles::CORTEX_M4_NRF52840)
            .expect("present");
        assert_eq!(
            m4.duration(PrimitiveOpKind::Verify),
            Some(Duration::from_micros(15_300))
        );

        // Published as operations per second: 10^6 / 4097.4 = 244.0567 µs, which
        // quantises to 244.057 µs on the nanosecond grid.
        let pi4 = ecdsa
            .cost
            .row(profiles::CORTEX_A72_PI4_OPENSSL)
            .expect("present");
        assert_eq!(
            pi4.duration(PrimitiveOpKind::Sign),
            Some(Duration::from_nanos(244_057))
        );

        // Published as cycles only, with a clock: 259,172 cycles has no clock on the
        // Skylake AVX2 row, so no duration can be derived from it.
        let avx2 = c
            .require(PrimitiveId::ML_DSA_44)
            .expect("present")
            .cost
            .row(profiles::SKYLAKE_AVX2)
            .expect("present");
        assert_eq!(avx2.sign.expect("sign").cycles, Some(259_172));
        assert_eq!(avx2.duration(PrimitiveOpKind::Sign), None);

        // Published as cycles with a clock: pqm4 at 24 MHz.
        let pqm4 = c
            .require(PrimitiveId::ML_DSA_44)
            .expect("present")
            .cost
            .row(profiles::CORTEX_M4_PQM4)
            .expect("present");
        // 1,421,623 / 24e6 s = 59.234 ms.
        assert_eq!(
            pqm4.duration(PrimitiveOpKind::Verify),
            Some(Duration::from_nanos(59_234_292))
        );
    }

    /// A profile with no published row is filled by scaling the nearest cycle count, and
    /// the result says so.
    #[test]
    fn a_missing_profile_is_filled_by_scaling_and_tagged() {
        let c = PrimitiveCatalogue::standard();
        let d = c.require(PrimitiveId::ML_DSA_44).expect("present");
        assert!(d.cost.row("hw-profile/invented-600mhz").is_none());
        let scaled = d
            .cost
            .row_or_scaled("hw-profile/invented-600mhz", 600.0e6)
            .expect("a cycle-count row exists to scale from");
        assert!(scaled.scaled);
        assert_eq!(scaled.clock_hz, Some(600.0e6));
        assert!(scaled.note.expect("a note").contains("scaled"));
        // The nearest clocked row to 600 MHz among ML-DSA-44's rows is the 216 MHz
        // Cortex-M7 (the other clocked row is 24 MHz), whose verify is 1,429,000 cycles.
        // At 600 MHz that is 2,381.667 µs.
        assert_eq!(
            scaled.verify.expect("verify").cycles,
            Some(1_429_000),
            "the cycle count is carried over unchanged"
        );
        assert_eq!(
            scaled.duration(PrimitiveOpKind::Verify),
            Some(Duration::from_nanos(2_381_667))
        );
        // An exact hit is returned unscaled.
        let exact = d
            .cost
            .row_or_scaled(profiles::CORTEX_M4_PQM4, 24.0e6)
            .expect("present");
        assert!(!exact.scaled);
    }

    /// A primitive with no anchors of its own resolves through its declared proxy, and a
    /// proxy is never invented silently.
    #[test]
    fn a_cost_proxy_resolves_and_is_declared() {
        let c = PrimitiveCatalogue::standard();
        // brainpoolP256r1 borrows the P-256 anchors (same field size, same operation
        // count). ECQV used to borrow the ECDSA verify figure too; WP-C4 replaced that
        // with its recipe (`ecqv_reconstruction_is_one_variable_base_multiplication`).
        let bp = c
            .require(PrimitiveId::ECDSA_BRAINPOOL_P256R1)
            .expect("present");
        assert!(bp.cost.is_empty());
        assert_eq!(bp.cost_proxy, Some(PrimitiveId::ECDSA_P256_SHA256));
        let row = bp
            .cost_row(profiles::CORTEX_M4_NRF52840, c)
            .expect("resolves through the proxy");
        assert_eq!(row.verify.expect("verify").cycles, Some(976_000));
        let ecqv = c.require(PrimitiveId::ECQV_P256).expect("present");
        assert!(ecqv.cost.is_empty());
        assert_eq!(ecqv.cost_proxy, None, "the ECDSA proxy is gone");
        assert!(ecqv.cost_recipe.is_some());
        // Every primitive with an empty table either declares a proxy, declares the
        // components its cost is the sum of, declares a recipe, or says in its notes that
        // no anchor exists. The point of the assertion is that the last case is *stated*:
        // a silent zero or a silently borrowed number is what "no black box" forbids.
        for d in c.iter() {
            if d.cost.is_empty()
                && d.cost_proxy.is_none()
                && d.cost_components.is_empty()
                && d.cost_recipe.is_none()
            {
                assert!(
                    d.notes.iter().any(|n| n.to_lowercase().contains("no cost")
                        || n.to_lowercase().contains("lists no cost")),
                    "{} has neither anchors, a proxy, components, nor a note saying why",
                    d.id
                );
            }
        }

        // A hybrid's cost is the sum of its components', and the sum is arithmetic a
        // reader can redo. The Cohda MK6 is the one profile in §9.4 that was benchmarked
        // for both an ECDSA and an ML-DSA implementation, so it is the only profile where
        // the sum is entirely made of published numbers.
        let hybrid = c
            .require(PrimitiveId::HYBRID_MLDSA44_ECDSA_P256)
            .expect("present");
        let profile = profiles::COHDA_MK6_BOTAN;
        let parts: Duration = [PrimitiveId::ML_DSA_44, PrimitiveId::ECDSA_P256_SHA256]
            .iter()
            .map(|p| {
                c.require(*p)
                    .expect("present")
                    .cost_duration(PrimitiveOpKind::Verify, profile, c)
                    .expect("both components have a Cohda MK6 anchor")
            })
            .fold(Duration::ZERO, |a, b| a + b);
        // 189 µs for Dilithium plus the (anomalous, as published) 1 µs for ECDSA.
        assert_eq!(parts, Duration::from_micros(190));
        assert_eq!(
            hybrid.cost_duration(PrimitiveOpKind::Verify, profile, c),
            Some(parts)
        );

        // A composite whose components are not *both* benchmarked on a profile yields
        // nothing rather than a half sum: ML-DSA-44 has a Cortex-M7 anchor and ECDSA
        // P-256 does not, so the hybrid has no Cortex-M7 cost. Understating a hybrid by
        // the cost of its classical half is exactly the error a partial sum would make.
        assert!(
            c.require(PrimitiveId::ML_DSA_44)
                .expect("present")
                .cost_duration(PrimitiveOpKind::Verify, profiles::CORTEX_M7_STM32F767, c)
                .is_some()
        );
        assert_eq!(
            hybrid.cost_duration(PrimitiveOpKind::Verify, profiles::CORTEX_M7_STM32F767, c),
            None
        );

        // And a primitive nothing was benchmarked for yields nothing, not zero.
        assert_eq!(
            c.require(PrimitiveId::ECDSA_P384)
                .expect("present")
                .cost_duration(PrimitiveOpKind::Verify, profiles::CORTEX_M7_STM32F767, c),
            None
        );
    }

    /// Every primitive is a registrable model with a card that validates.
    #[test]
    fn every_primitive_has_a_card_that_validates() {
        let models = standard_primitive_models();
        assert_eq!(models.len(), PrimitiveCatalogue::standard().len());
        assert!(models.len() >= 23, "the §9.4 table has 23 rows at least");
        for m in &models {
            m.card().validate().unwrap_or_else(|e| {
                panic!("{}: {e}", m.id());
            });
            assert_eq!(m.family(), Family::Primitive);
            assert!(!m.card().sources.is_empty(), "{}: no source", m.id());
            assert!(m.implements_tier(Tier::Medium));
        }
    }

    /// Cost tables and the catalogue iterate in a fixed order, because a report, a card
    /// payload and a manifest digest all walk them (02-architecture.md §6.4).
    #[test]
    fn the_catalogue_and_its_tables_iterate_in_id_order() {
        let c = PrimitiveCatalogue::standard();
        let ids: Vec<&str> = c.iter().map(|d| d.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        for d in c.iter() {
            let profiles: Vec<&str> = d.cost.rows().iter().map(|r| r.profile).collect();
            let mut s = profiles.clone();
            s.sort_unstable();
            assert_eq!(profiles, s, "{}", d.id);
        }
    }

    /// Every cost value sits on the nanosecond grid build decision D9 requires of any
    /// number that reaches a recorded artefact.
    #[test]
    fn every_cost_value_is_quantised() {
        for d in PrimitiveCatalogue::standard().iter() {
            for row in d.cost.rows() {
                for (name, op) in [
                    ("keygen", row.keygen),
                    ("sign", row.sign),
                    ("verify", row.verify),
                ] {
                    let Some(op) = op else { continue };
                    for (field, value) in [("mean_us", op.mean_us), ("p95_us", op.p95_us)] {
                        let Some(v) = value else { continue };
                        assert!(
                            v2xw_core::math::is_on_grid(v, COST_US_QUANTUM),
                            "{} / {} / {name}.{field} = {v} is off the {COST_US_QUANTUM} µs grid",
                            d.id,
                            row.profile
                        );
                    }
                }
            }
        }
    }
    // ---------------------------------------------------------------------------------
    // WP-C4: every primitive that runs costs time.
    // ---------------------------------------------------------------------------------

    fn d(id: PrimitiveId) -> &'static PrimitiveDescriptor {
        PrimitiveCatalogue::standard().require(id).expect("present")
    }

    fn ns(t: Option<Duration>) -> u64 {
        t.expect("priced").as_nanos()
    }

    /// SHA-256 and SHA-384 cost their FIPS 180-4 compression calls: the published
    /// nRF52840 figure is one 4,123-byte message (65 calls), so one call is 1/65 of it,
    /// and the padding boundary (55/56 bytes for SHA-256, 111/112 for SHA-384) is where a
    /// second call starts.
    #[test]
    fn a_hash_costs_its_fips_180_4_compression_calls() {
        let c = PrimitiveCatalogue::standard();
        let sha = d(PrimitiveId::SHA_256);
        let at =
            |n| ns(sha.cost_for_len(PrimitiveOpKind::Verify, profiles::CORTEX_M4_NRF52840, n, c));
        let per_call = |calls: u64| (4_741_300 * calls + 32) / 65;
        assert_eq!(
            at(4_123),
            4_741_300,
            "the published message costs the published time"
        );
        assert_eq!(at(0), per_call(1));
        assert_eq!(at(55), per_call(1));
        assert_eq!(at(56), per_call(2), "56 bytes + 0x80 + 8-byte length > 64");
        assert_eq!(at(119), per_call(2));
        assert_eq!(at(120), per_call(3));
        // SHA-384: 128-byte blocks and a 16-byte length; the Pi 4 SHA-512 64-byte column,
        // 64 bytes in 64 / 20,136,470 s = 3.178 µs, is one call.
        let s384 = d(PrimitiveId::SHA_384);
        let at = |n| {
            ns(s384.cost_for_len(
                PrimitiveOpKind::Verify,
                profiles::CORTEX_A72_PI4_OPENSSL,
                n,
                c,
            ))
        };
        assert_eq!(at(64), 3_178);
        assert_eq!(at(111), 3_178);
        assert_eq!(
            at(112),
            2 * 3_178,
            "112 bytes + 0x80 + 16-byte length > 128"
        );
        // "One operation" of a hash is one call, never zero.
        assert_eq!(
            sha.cost_duration(PrimitiveOpKind::Verify, profiles::CORTEX_M4_NRF52840, c),
            Some(Duration::from_nanos(per_call(1)))
        );
    }

    /// AES-128-CCM costs exactly the AES calls SP 800-38C makes. The count is checked
    /// against an independent CCM written here over the crate's AES block and compared
    /// bit for bit with `ccm::encrypt` (itself vector-checked against 1609.2a D.6.1), so
    /// the formula is what CCM actually does, not what this module believes it does.
    #[test]
    fn ccm_costs_exactly_the_aes_calls_it_makes() {
        use crate::{aes128, ccm};
        fn counted_ccm(key: &[u8; 16], nonce: &[u8; 12], pt: &[u8]) -> (Vec<u8>, u64) {
            let mut calls = 0u64;
            let mut e = |b: &[u8; 16]| {
                calls += 1;
                aes128::encrypt_block(key, b)
            };
            // B0: flags (Adata 0, t = 16 -> M' = 7, q = 3 -> L' = 2), nonce, 3-byte length.
            let mut b0 = [0u8; 16];
            b0[0] = (7 << 3) | 2;
            b0[1..13].copy_from_slice(nonce);
            b0[13..16].copy_from_slice(&(pt.len() as u32).to_be_bytes()[1..]);
            let mut x = e(&b0);
            for chunk in pt.chunks(16) {
                let mut blk = [0u8; 16];
                blk[..chunk.len()].copy_from_slice(chunk);
                for i in 0..16 {
                    blk[i] ^= x[i];
                }
                x = e(&blk);
            }
            let ctr = |i: u32| {
                let mut a = [0u8; 16];
                a[0] = 2;
                a[1..13].copy_from_slice(nonce);
                a[13..16].copy_from_slice(&i.to_be_bytes()[1..]);
                a
            };
            let mut out = Vec::with_capacity(pt.len() + 16);
            for (i, chunk) in pt.chunks(16).enumerate() {
                let s = e(&ctr(i as u32 + 1));
                out.extend(chunk.iter().zip(s.iter()).map(|(p, k)| p ^ k));
            }
            let s0 = e(&ctr(0));
            out.extend(x.iter().zip(s0.iter()).map(|(t, k)| t ^ k));
            (out, calls)
        }
        let c = PrimitiveCatalogue::standard();
        let key = [0x2bu8; 16];
        let nonce = [0x5au8; 12];
        // Pi 3B: the 16-byte aes-128-cbc column, 16 / 41,225,960 s = 0.388 µs per call.
        let per_call = 388u64;
        let ccm_d = d(PrimitiveId::AES_128_CCM);
        for n in [0usize, 1, 15, 16, 17, 31, 32, 33, 100, 1_000] {
            let pt: Vec<u8> = (0..n).map(|i| i as u8).collect();
            let (mine, calls) = counted_ccm(&key, &nonce, &pt);
            assert_eq!(
                mine,
                ccm::encrypt(&key, &nonce, &pt).expect("encrypts"),
                "n = {n}"
            );
            assert_eq!(CostUnit::CcmAesCalls.units(n as u32), calls, "n = {n}");
            assert_eq!(
                ns(ccm_d.cost_for_len(
                    PrimitiveOpKind::Sign,
                    profiles::CORTEX_A53_PI3B,
                    n as u32,
                    c
                )),
                per_call * calls,
                "n = {n}"
            );
        }
        // An HSM that publishes a per-transaction rate is charged it once per call.
        assert_eq!(
            ccm_d.cost_for_len(PrimitiveOpKind::Verify, profiles::LUNA7_A750, 10_000, c),
            Some(Duration::from_micros(100))
        );
    }

    /// ECQV reconstruction is one variable-base multiplication and two SHA-256 calls
    /// (02 §6), not an ECDSA verification (a double multiplication).
    #[test]
    fn ecqv_reconstruction_is_one_variable_base_multiplication() {
        let c = PrimitiveCatalogue::standard();
        let ecqv = d(PrimitiveId::ECQV_P256);
        // Cortex-M4 @ 64 MHz: ECDH 906,000 cycles = 14,156,250 ns; SHA-256 over 80 and
        // 64 bytes is two calls each at 4,741,300 / 65 ns.
        let sha2 = (4_741_300 * 2 + 32) / 65;
        assert_eq!(
            ns(ecqv.cost_duration(PrimitiveOpKind::Verify, profiles::CORTEX_M4_NRF52840, c)),
            14_156_250 + 2 * sha2
        );
        assert!(ecqv.cost_is_derived(PrimitiveOpKind::Verify, profiles::CORTEX_M4_NRF52840, c));
        // On every profile that prices both, reconstructing costs less than verifying.
        let ecdsa = d(PrimitiveId::ECDSA_P256_SHA256);
        let mut compared = 0;
        for row in ecdsa.cost.rows() {
            let (Some(r), Some(v)) = (
                ecqv.cost_duration(PrimitiveOpKind::Verify, row.profile, c),
                row.duration(PrimitiveOpKind::Verify),
            ) else {
                continue;
            };
            assert!(r < v, "{}: reconstruct {r:?} >= verify {v:?}", row.profile);
            compared += 1;
        }
        assert!(compared >= 6, "compared on {compared} profiles");
    }

    /// ECIES is the 1609.2 §5.3.5 operation count: decryption one variable-base
    /// multiplication and six SHA-256 calls (KDF2 two calls of two blocks, MAC1's inner
    /// and outer hash of two blocks each), encryption one fixed-base multiplication more.
    #[test]
    fn ecies_costs_its_1609_2_operation_count() {
        let c = PrimitiveCatalogue::standard();
        let ecies = d(PrimitiveId::ECIES_P256);
        let p = profiles::CORTEX_A72_PI4_OPENSSL;
        // Pi 4: ECDH 2,342.9/s = 426.821 µs; SHA-256 64-byte column = 1.179 µs per
        // two-call message, so a two-call input is 1,179 ns; ECDSA sign 4,097.4/s =
        // 244.057 µs stands in (upper bound) for the unpublished fixed-base keygen.
        let decrypt = 426_821 + 4 * 1_179;
        assert_eq!(
            ns(ecies.cost_duration(PrimitiveOpKind::Verify, p, c)),
            decrypt
        );
        assert_eq!(
            ns(ecies.cost_duration(PrimitiveOpKind::Sign, p, c)),
            decrypt + 244_057
        );
        // The published HSM decrypt rate wins where it exists, and encryption, which the
        // HSM does not publish, falls to the recipe rather than to nothing.
        assert_eq!(
            ecies.cost_duration(PrimitiveOpKind::Verify, profiles::LUNA7_A790, c),
            Some(Duration::from_nanos(142_857))
        );
    }

    /// The acceptance test of WP-C4: every primitive operation the crate executes is
    /// priced, above zero, on every device that runs cryptography.
    #[test]
    fn every_executed_primitive_is_priced_on_every_device() {
        let c = PrimitiveCatalogue::standard();
        let mut checked = 0;
        for dev in anchors::DEVICES {
            if dev.class == anchors::DeviceClass::NoCrypto {
                assert!(dev.anchors.is_empty(), "{}", dev.node_profile);
                continue;
            }
            for e in EXECUTED {
                let priced = c.price(e.primitive, e.op, e.bytes, dev.anchors);
                let Some(priced) = priced else {
                    panic!(
                        "{} ({}) cannot price {} {:?} over {} B, run by `{}`",
                        dev.node_profile, e.module, e.primitive, e.op, e.bytes, e.module
                    );
                };
                assert!(
                    priced.duration > Duration::ZERO,
                    "{}: {} {:?} costs zero",
                    dev.node_profile,
                    e.primitive,
                    e.op
                );
                checked += 1;
            }
        }
        assert!(checked > 400, "checked {checked}");
    }

    /// A clock-scaled anchor scales: the CRATON2's 600 MHz Cortex-A7 is charged twice the
    /// Pi 3B's 1.2 GHz figure, and says it was scaled.
    #[test]
    fn a_clock_scaled_anchor_scales_and_says_so() {
        let c = PrimitiveCatalogue::standard();
        let craton = anchors::for_node_profile("obu/unex-obu-301-craton2").expect("mapped");
        let pi3 = [anchors::Anchor {
            profile: profiles::CORTEX_A53_PI3B,
            num: 1,
            den: 1,
            why: "",
        }];
        let a = c
            .price(
                PrimitiveId::SHA_256,
                PrimitiveOpKind::Verify,
                64,
                craton.anchors,
            )
            .expect("priced");
        let b = c
            .price(PrimitiveId::SHA_256, PrimitiveOpKind::Verify, 64, &pi3)
            .expect("priced");
        assert_eq!(a.duration.as_nanos(), 2 * b.duration.as_nanos());
        assert!(a.clock_scaled && a.derived && !b.clock_scaled);
        assert_eq!(a.profile, profiles::CORTEX_A53_PI3B);
    }

    /// Every node hardware profile the repository ships is mapped to anchors, and every
    /// mapping names a profile that exists — so a new OBU cannot arrive unpriced.
    #[test]
    fn every_node_hardware_profile_has_crypto_anchors() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../v2xw-node/profiles/hardware");
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("profiles directory") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable");
            let id = text
                .lines()
                .find_map(|l| l.strip_prefix("id: "))
                .expect("an id line")
                .trim()
                .to_string();
            assert!(
                anchors::for_node_profile(&id).is_some(),
                "{} ({}) has no crypto anchors",
                id,
                path.display()
            );
            ids.push(id);
        }
        assert!(ids.len() >= 12, "found {} profiles", ids.len());
        for dev in anchors::DEVICES {
            assert!(
                ids.iter().any(|i| i == dev.node_profile),
                "anchors for {} name no shipped profile",
                dev.node_profile
            );
            for a in dev.anchors {
                assert!(
                    !a.why.trim().is_empty(),
                    "{}: anchor without a reason",
                    dev.node_profile
                );
                assert!(a.den > 0 && a.num > 0);
            }
        }
    }

    /// Every cryptographic module of the crate is in [`EXECUTED`], and every module is
    /// either there or declared free of cryptography: adding a module that runs a
    /// primitive without pricing it fails here.
    #[test]
    fn every_crypto_module_is_in_the_executed_registry() {
        let lib = include_str!("lib.rs");
        let modules: Vec<&str> = lib
            .lines()
            .filter_map(|l| l.strip_prefix("pub mod "))
            .map(|l| l.trim_end_matches(';').trim())
            .collect();
        assert!(modules.len() >= 15, "{modules:?}");
        for m in &modules {
            let listed = EXECUTED.iter().any(|e| e.module == *m);
            let exempt = MODULES_WITHOUT_CRYPTO.contains(m);
            assert!(
                listed ^ exempt,
                "module `{m}`: listed {listed}, exempt {exempt}"
            );
        }
        for e in EXECUTED {
            assert!(
                modules.contains(&e.module),
                "`{}` is not a module",
                e.module
            );
            assert!(
                PrimitiveCatalogue::standard().get(e.primitive).is_some(),
                "{} not in the catalogue",
                e.primitive
            );
        }
    }

    /// A chain link costs its three hashes and its verification; a P-384 issuer hashes
    /// with SHA-384.
    #[test]
    fn a_chain_link_costs_its_hashes_and_its_verification() {
        let c = PrimitiveCatalogue::standard();
        let pi4 = [anchors::Anchor {
            profile: profiles::CORTEX_A72_PI4_OPENSSL,
            num: 1,
            den: 1,
            why: "test",
        }];
        let link = ChainLink::Explicit {
            signer: PrimitiveId::ECDSA_P384,
            tbs_bytes: 100,
            issuer_bytes: 200,
        };
        let ops = link.ops();
        assert_eq!(
            ops.iter().filter(|o| o.0 == PrimitiveId::SHA_384).count(),
            3
        );
        // Pi 4: P-384 verify 174.8/s = 5,720.824 µs; SHA-384 1 + 2 + 1 calls at 3,178 ns.
        assert_eq!(ns(c.price_all(&ops, &pi4)), 5_720_824 + 4 * 3_178);
        assert_eq!(
            ChainLink::Implicit.ops(),
            vec![(PrimitiveId::ECQV_P256, PrimitiveOpKind::Verify, 0)]
        );
    }
    /// The device × operation matrix the WP-C4 findings note prints, in microseconds
    /// (`cargo test -p v2xw-sec --lib device_cost_matrix -- --nocapture`). Every cell is
    /// priced, or the test fails.
    #[test]
    fn device_cost_matrix() {
        use PrimitiveOpKind::{Sign as S, Verify as V};
        let c = PrimitiveCatalogue::standard();
        let cols: [(&str, PrimitiveId, PrimitiveOpKind, u32); 11] = [
            ("SHA-256 150 B", PrimitiveId::SHA_256, V, 150),
            ("SHA-384 300 B", PrimitiveId::SHA_384, V, 300),
            ("AES block", PrimitiveId::AES_128, V, 16),
            ("CCM 1 KB", PrimitiveId::AES_128_CCM, V, 1_000),
            ("ECIES dec", PrimitiveId::ECIES_P256, V, 0),
            ("ECIES enc", PrimitiveId::ECIES_P256, S, 0),
            ("ECQV recon", PrimitiveId::ECQV_P256, V, 0),
            ("P-256 verify", PrimitiveId::ECDSA_P256_SHA256, V, 0),
            ("P-384 verify", PrimitiveId::ECDSA_P384, V, 0),
            ("lv(i,j)", PrimitiveId::LINKAGE_VALUE, V, 0),
            ("bf expand (RA)", PrimitiveId::BUTTERFLY_EXPAND_P256, S, 0),
        ];
        let mut head = String::from("| node profile |");
        for (n, ..) in &cols {
            head.push_str(&format!(" {n} |"));
        }
        println!("{head}");
        println!("|---|{}", "---:|".repeat(cols.len()));
        for dev in anchors::DEVICES.iter().filter(|d| !d.anchors.is_empty()) {
            let mut line = format!("| `{}` |", dev.node_profile);
            for (_, p, op, b) in &cols {
                let t = c.price(*p, *op, *b, dev.anchors).expect("priced");
                line.push_str(&format!(" {:.1} |", t.duration.as_nanos() as f64 / 1e3));
            }
            println!("{line}");
        }
    }
    /// Clock-scaling a per-block row keeps what the figure covers: the AURIX HSM's 65
    /// cycles per SHA-256 block at 100 MHz is 325 ns per block at 200 MHz, not 325 ns
    /// per call whatever its length.
    #[test]
    fn a_scaled_per_block_row_still_counts_blocks() {
        let sha = d(PrimitiveId::SHA_256);
        let row = sha
            .cost
            .row_or_scaled("hw-profile/test-200mhz", 200.0e6)
            .expect("scalable");
        assert!(row.scaled);
        let unit = sha.cost_unit;
        let t = |n| row.duration_for_units(PrimitiveOpKind::Verify, unit, unit.units(n));
        assert_eq!(t(0), Some(Duration::from_nanos(325)));
        assert_eq!(t(120), Some(Duration::from_nanos(3 * 325)));
    }
}
