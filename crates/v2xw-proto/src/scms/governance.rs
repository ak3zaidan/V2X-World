//! The SCMS's governance and trust-management entities, and what a device does with
//! their products.
//!
//! The SCMS of [BRECHT Fig. 1] has entities that never touch a pseudonym request: the SCMS
//! Manager, which sets policy; the Policy Generator, which signs it; the Root CA and the
//! electors, which anchor trust; and the Intermediate CA, which certifies the online
//! authorities. They are offline or nearly so in the CAMP proof of concept (05-protocols
//! §3.1), so none of them sits on the per-request path — but a device cannot verify a
//! single pseudonym certificate, a CRL or a policy without their artefacts, and a run that
//! left them out would be trusting everything for free.
//!
//! What this module makes real:
//!
//! * **Keys and certificates.** Every authority has a P-256 key drawn from its own keyed
//!   RNG stream and a certificate signed by its issuer with real ECDSA — the Root CA and
//!   the electors self-signed, the Intermediate CA by the Root, the online authorities by
//!   the Intermediate CA (the Policy Generator, the Misbehaviour Authority and the CRL
//!   Generator by the Root). A device checks those signatures, so a forged certificate is
//!   refused by arithmetic, not by a flag.
//! * **The certificate trust list.** IEEE 1609.2.1's `MultiSignedCtl`: the root and elector
//!   certificates, endorsed by the electors, and accepted only with a quorum of valid
//!   endorsements. Elector certificates are the device's trust anchors, installed at
//!   bootstrap by the Device Configuration Manager.
//! * **Policy and certificate-chain files.** The Global Policy File and Global Certificate
//!   Chain File, signed by the Policy Generator; the Local Policy File and Local
//!   Certificate Chain File the Registration Authority serves its devices. The policy is
//!   what the RA enforces — certificates per i-period, how far ahead a device may be
//!   provisioned — and what the device reads back.
//!
//! What it does **not** do: root rollover by elector ballot, elector revocation, and the
//! composite CRL of revoked authorities. Their artefacts exist only as the data they would
//! carry; nothing in a run exercises them.
//!
//! Which authority certifies which component is this build's reading of [BRECHT Fig. 1]
//! and §III, not a quotation: the Root certifies the ICA and the entities whose
//! compromise would be a system-level event (PG, MA, CRLG); the ICA certifies the rest.

use std::collections::{BTreeMap, BTreeSet};

use p256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use v2xw_core::hash::sha256;
use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngDomain, RngRegistry};
use v2xw_core::time::{Duration, SimTime};

use crate::scms::params::{ScmsNodes, ScmsParams};

/// The node ids the governance entities are known by. None of them is hosted on the
/// request path; the SCMS Manager and the Policy Generator are hosted so a policy change
/// is a message with a latency like any other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceNodes {
    /// The SCMS Manager: sets policy.
    pub manager: NodeId,
    /// The Policy Generator: signs the global policy and chain files.
    pub pg: NodeId,
    /// The Root CA (offline).
    pub root: NodeId,
    /// The Intermediate CA (offline between certifications).
    pub ica: NodeId,
    /// The electors (offline), in index order.
    pub electors: Vec<NodeId>,
}

impl GovernanceNodes {
    /// The reference ids, clear of [`ScmsNodes`]' 1–11.
    #[must_use]
    pub fn new(electors: u32) -> GovernanceNodes {
        GovernanceNodes {
            manager: NodeId::new(20),
            pg: NodeId::new(21),
            root: NodeId::new(22),
            ica: NodeId::new(23),
            electors: (0..electors.max(1)).map(|k| NodeId::new(24 + k)).collect(),
        }
    }
}

/// How long a Root CA certificate is valid. **Uncited**: the CAMP documents this build can
/// read give no figure; no run is long enough to reach it, and it is a constant so that
/// nobody mistakes it for a measured one.
pub const ROOT_LIFETIME: Duration = Duration::from_secs(20 * 365 * 86_400);

/// How long every other authority certificate is valid. **Uncited**, as above.
pub const AUTHORITY_LIFETIME: Duration = Duration::from_secs(5 * 365 * 86_400);

/// Why a device refused a trust artefact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrustError {
    /// A trust list without a quorum of valid elector endorsements.
    CtlQuorum {
        /// Endorsements that verified.
        valid: u32,
        /// The quorum the list itself states.
        needed: u32,
    },
    /// A certificate or file whose issuer the device does not trust.
    UnknownIssuer,
    /// A signature that does not verify.
    BadSignature,
    /// A file no newer than the one installed.
    Stale,
    /// A certificate outside its validity window.
    Expired,
}

impl core::fmt::Display for TrustError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrustError::CtlQuorum { valid, needed } => write!(
                f,
                "trust list carries {valid} valid elector endorsement(s), quorum is {needed}"
            ),
            TrustError::UnknownIssuer => f.write_str("issuer not in the device's trust store"),
            TrustError::BadSignature => f.write_str("signature does not verify"),
            TrustError::Stale => f.write_str("file is not newer than the installed one"),
            TrustError::Expired => f.write_str("certificate outside its validity window"),
        }
    }
}

/// An authority's certificate: explicit, ECDSA P-256, signed by its issuer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityCert {
    /// The role it certifies (`root`, `ica`, `eca`, `pca`, `ra`, …).
    pub role: &'static str,
    /// The node that holds the key.
    pub node: NodeId,
    /// The issuer's certificate digest; `None` for a self-signed certificate.
    pub issuer: Option<[u8; 8]>,
    /// The certified public key, SEC 1 compressed.
    pub public: [u8; 33],
    /// Start of validity.
    pub valid_from: SimTime,
    /// End of validity.
    pub valid_until: SimTime,
    /// The issuer's ECDSA signature over [`AuthorityCert::tbs_hash`], `r ‖ s`.
    pub signature: [u8; 64],
}

impl AuthorityCert {
    /// SHA-256 over the to-be-signed fields.
    #[must_use]
    pub fn tbs_hash(&self) -> [u8; 32] {
        let mut b = Vec::with_capacity(96);
        b.extend_from_slice(self.role.as_bytes());
        b.push(0);
        b.extend_from_slice(&self.node.index().to_be_bytes());
        b.extend_from_slice(&self.issuer.unwrap_or([0; 8]));
        b.extend_from_slice(&self.public);
        b.extend_from_slice(&self.valid_from.to_be_bytes());
        b.extend_from_slice(&self.valid_until.to_be_bytes());
        sha256(&b)
    }

    /// The certificate's `HashedId8`: the low-order eight octets of SHA-256 over it
    /// (IEEE 1609.2 §6.4.3), here over its to-be-signed hash and signature.
    #[must_use]
    pub fn digest(&self) -> [u8; 8] {
        let mut b = Vec::with_capacity(96);
        b.extend_from_slice(&self.tbs_hash());
        b.extend_from_slice(&self.signature);
        let h = sha256(&b);
        let mut d = [0u8; 8];
        d.copy_from_slice(&h[24..32]);
        d
    }

    /// The certified key.
    #[must_use]
    pub fn verifying_key(&self) -> Option<VerifyingKey> {
        VerifyingKey::from_sec1_bytes(&self.public).ok()
    }

    /// Whether `issuer_key` signed this certificate.
    #[must_use]
    pub fn signed_by(&self, issuer_key: &VerifyingKey) -> bool {
        verify(issuer_key, &self.tbs_hash(), &self.signature)
    }
}

fn verify(key: &VerifyingKey, hash: &[u8; 32], sig: &[u8; 64]) -> bool {
    Signature::from_slice(sig)
        .ok()
        .is_some_and(|s| key.verify_prehash(hash, &s).is_ok())
}

fn sign(key: &SigningKey, hash: &[u8; 32]) -> [u8; 64] {
    // RFC 6979 deterministic nonces: the same key and hash give the same signature, which
    // is what keeps a run reproducible without a nonce stream.
    let sig: Signature = key
        .sign_prehash(hash)
        .expect("a 32-byte prehash always signs");
    let mut out = [0u8; 64];
    out.copy_from_slice(&sig.to_bytes());
    out
}

/// IEEE 1609.2.1's multi-signed certificate trust list: which roots and electors are
/// trusted, endorsed by the electors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCtl {
    /// The list's sequence number; a device installs only a newer one.
    pub sequence: u32,
    /// How many valid elector endorsements the list needs.
    pub quorum: u32,
    /// Digests of the trusted root certificates.
    pub roots: Vec<[u8; 8]>,
    /// Digests of the electors' certificates.
    pub electors: Vec<[u8; 8]>,
    /// `(elector index, signature)`.
    pub endorsements: Vec<(u32, [u8; 64])>,
}

impl SignedCtl {
    /// SHA-256 over what the electors endorse.
    #[must_use]
    pub fn tbs_hash(&self) -> [u8; 32] {
        let mut b = Vec::new();
        b.extend_from_slice(&self.sequence.to_be_bytes());
        b.extend_from_slice(&self.quorum.to_be_bytes());
        for r in &self.roots {
            b.extend_from_slice(r);
        }
        b.push(0xFF);
        for e in &self.electors {
            b.extend_from_slice(e);
        }
        sha256(&b)
    }

    /// Counts the endorsements that verify against `anchors` (the elector certificates
    /// the device holds), each elector counted once, and refuses a list below quorum.
    ///
    /// # Errors
    /// [`TrustError::CtlQuorum`] when fewer than [`SignedCtl::quorum`] endorsements verify.
    pub fn verify(&self, anchors: &[AuthorityCert]) -> Result<u32, TrustError> {
        let hash = self.tbs_hash();
        let mut seen = BTreeSet::new();
        for (idx, sig) in &self.endorsements {
            let Some(cert) = anchors.get(*idx as usize) else {
                continue;
            };
            if cert.verifying_key().is_some_and(|k| verify(&k, &hash, sig)) {
                seen.insert(*idx);
            }
        }
        let valid = u32::try_from(seen.len()).unwrap_or(u32::MAX);
        // A list that asks for less than a majority of the anchors the device holds is
        // not trusted either: the quorum is a property of the governance, and a list
        // cannot lower it by stating a smaller one.
        let floor = (u32::try_from(anchors.len()).unwrap_or(u32::MAX) / 2) + 1;
        let needed = self.quorum.max(floor);
        if valid >= needed {
            Ok(valid)
        } else {
            Err(TrustError::CtlQuorum { valid, needed })
        }
    }
}

/// What a policy file fixes. The fields are the lifecycle numbers this build's flows
/// read; every one of them is also on the plug-in's model card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Policy {
    /// The i-period, seconds.
    pub i_period_s: u64,
    /// A pseudonym certificate's lifetime, seconds.
    pub cert_lifetime_s: u64,
    /// Certificates valid at once in one i-period.
    pub certs_per_period: u32,
    /// How many i-periods ahead the RA provisions at most.
    pub max_periods_ahead: u32,
    /// The RA's request-shuffle window, seconds.
    pub shuffle_window_s: u64,
    /// The CRL cadence, seconds.
    pub crl_cadence_s: u64,
    /// An enrolment certificate's lifetime, seconds.
    pub enrolment_lifetime_s: u64,
    /// How long before its enrolment certificate expires a device asks for a successor,
    /// seconds.
    pub reenrol_lead_s: u64,
}

impl Policy {
    /// The policy a deployment's parameters describe.
    #[must_use]
    pub fn of(p: &ScmsParams) -> Policy {
        Policy {
            i_period_s: p.i_period.as_nanos() / 1_000_000_000,
            cert_lifetime_s: p.cert_lifetime.as_nanos() / 1_000_000_000,
            certs_per_period: p.certs_per_period,
            max_periods_ahead: p.max_periods_ahead,
            shuffle_window_s: p.shuffle_window.as_nanos() / 1_000_000_000,
            crl_cadence_s: p.crl_cadence.as_nanos() / 1_000_000_000,
            enrolment_lifetime_s: p.enrolment_lifetime.as_nanos() / 1_000_000_000,
            reenrol_lead_s: p.reenrol_lead.as_nanos() / 1_000_000_000,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [
            self.i_period_s,
            self.cert_lifetime_s,
            u64::from(self.certs_per_period),
            u64::from(self.max_periods_ahead),
            self.shuffle_window_s,
            self.crl_cadence_s,
            self.enrolment_lifetime_s,
            self.reenrol_lead_s,
        ] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b
    }

    /// How many fields a policy file encodes, for its size.
    pub const FIELDS: u32 = 8;
}

/// Global or local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Signed by the Policy Generator for the whole system.
    Global,
    /// Served by the Registration Authority to its devices.
    Local,
}

/// A signed policy file (GPF or LPF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyFile {
    /// Global or local.
    pub scope: Scope,
    /// Its version; a device installs only a newer one.
    pub version: u32,
    /// What it says.
    pub policy: Policy,
    /// The signer's certificate digest.
    pub signer: [u8; 8],
    /// The signature over [`PolicyFile::tbs_hash`].
    pub signature: [u8; 64],
}

impl PolicyFile {
    /// SHA-256 over the signed fields.
    #[must_use]
    pub fn tbs_hash(&self) -> [u8; 32] {
        let mut b = vec![u8::from(self.scope == Scope::Local)];
        b.extend_from_slice(&self.version.to_be_bytes());
        b.extend_from_slice(&self.policy.bytes());
        b.extend_from_slice(&self.signer);
        sha256(&b)
    }
}

/// A signed certificate-chain file (GCCF or LCCF): the authority certificates a device
/// needs to verify everything it receives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainFile {
    /// Global or local.
    pub scope: Scope,
    /// Its version.
    pub version: u32,
    /// The certificates, root first.
    pub certs: Vec<AuthorityCert>,
    /// The signer's certificate digest.
    pub signer: [u8; 8],
    /// The signature over [`ChainFile::tbs_hash`].
    pub signature: [u8; 64],
}

impl ChainFile {
    /// SHA-256 over the signed fields.
    #[must_use]
    pub fn tbs_hash(&self) -> [u8; 32] {
        let mut b = vec![u8::from(self.scope == Scope::Local)];
        b.extend_from_slice(&self.version.to_be_bytes());
        for c in &self.certs {
            b.extend_from_slice(&c.digest());
        }
        b.extend_from_slice(&self.signer);
        sha256(&b)
    }
}

/// What the governance entities have done, for an inspector.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct GovernanceCounters {
    /// Certificates the Root CA signed (its own included).
    pub root_certs_issued: u32,
    /// Certificates the Intermediate CA signed.
    pub ica_certs_issued: u32,
    /// Trust-list endorsements the electors signed.
    pub elector_endorsements: u32,
    /// Policy decisions the SCMS Manager took.
    pub manager_decisions: u32,
    /// Files the Policy Generator signed.
    pub pg_files_signed: u32,
    /// Local files the Registration Authority signed.
    pub ra_files_signed: u32,
}

/// The governance entities, their keys and their artefacts.
pub struct Governance {
    /// Where they are.
    pub nodes: GovernanceNodes,
    keys: BTreeMap<NodeId, SigningKey>,
    /// Every authority certificate, by role (`elector-0`, `elector-1`, … for electors).
    pub certs: BTreeMap<String, AuthorityCert>,
    /// The current trust list.
    pub ctl: SignedCtl,
    /// The current Global Policy File.
    pub gpf: PolicyFile,
    /// The current Global Certificate Chain File.
    pub gccf: ChainFile,
    /// The Registration Authority's current Local Policy File.
    pub lpf: PolicyFile,
    /// The Registration Authority's current Local Certificate Chain File.
    pub lccf: ChainFile,
    /// What they have done.
    pub counters: GovernanceCounters,
}

impl core::fmt::Debug for Governance {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Governance")
            .field("nodes", &self.nodes)
            .field("certs", &self.certs.len())
            .field("ctl_sequence", &self.ctl.sequence)
            .field("gpf_version", &self.gpf.version)
            .field("lpf_version", &self.lpf.version)
            .finish_non_exhaustive()
    }
}

/// The RNG scope authority keys are drawn from: the node's id under this module's own
/// custom scope, so an authority key never consumes a draw from the stream the same node's
/// protocol role uses (the PCA's per-certificate randomiser, an LA's seeds) and adding the
/// governance entities moves no credential a run already produced.
fn key_scope(node: NodeId) -> EntityRef {
    EntityRef::custom(
        "protocol/scms/camp/governance-keys",
        u64::from(node.index()),
    )
}

fn key_for(rng: &RngRegistry, node: NodeId) -> SigningKey {
    let mut g = rng.checkout(RngDomain::Crypto, key_scope(node));
    loop {
        let mut bytes = [0u8; 32];
        g.fill_bytes(&mut bytes);
        // A draw of zero or of at least the group order is not a key; the next draw is,
        // with overwhelming probability, and the stream is deterministic either way.
        if let Ok(k) = SigningKey::from_slice(&bytes) {
            return k;
        }
    }
}

fn public_of(key: &SigningKey) -> [u8; 33] {
    let point = key.verifying_key().to_encoded_point(true);
    let mut out = [0u8; 33];
    out.copy_from_slice(point.as_bytes());
    out
}

/// The roles the Intermediate CA certifies, in issuance order.
pub const ICA_ISSUED: [&str; 8] = ["eca", "pca", "ra", "la1", "la2", "lop", "dcm", "crl-store"];

/// The roles the Root CA certifies directly, besides the ICA.
pub const ROOT_ISSUED: [&str; 3] = ["pg", "ma", "crlg"];

impl Governance {
    /// The setup ceremony: keys for every authority, the Root's self-signed certificate,
    /// the electors' self-signed certificates, the ICA's and every component's, the
    /// electors' endorsement of the first trust list, and the first global and local
    /// policy and chain files.
    ///
    /// It happens before the run, at `t0`, and charges nothing to the run's clock: the
    /// Root CA and the electors are offline and their ceremonies are not part of any flow
    /// a scenario measures.
    #[must_use]
    pub fn ceremony(
        p: &ScmsParams,
        online: &ScmsNodes,
        rng: &RngRegistry,
        t0: SimTime,
    ) -> Governance {
        let nodes = GovernanceNodes::new(p.electors);
        let mut keys = BTreeMap::new();
        let mut counters = GovernanceCounters::default();
        let mut certs: BTreeMap<String, AuthorityCert> = BTreeMap::new();

        let root_key = key_for(rng, nodes.root);
        let mut root = AuthorityCert {
            role: "root",
            node: nodes.root,
            issuer: None,
            public: public_of(&root_key),
            valid_from: t0,
            valid_until: ROOT_LIFETIME.after(t0),
            signature: [0; 64],
        };
        root.signature = sign(&root_key, &root.tbs_hash());
        counters.root_certs_issued += 1;
        let root_digest = root.digest();
        certs.insert("root".into(), root);
        keys.insert(nodes.root, root_key);

        let mut elector_certs = Vec::new();
        for (k, &node) in nodes.electors.iter().enumerate() {
            let key = key_for(rng, node);
            let mut c = AuthorityCert {
                role: "elector",
                node,
                issuer: None,
                public: public_of(&key),
                valid_from: t0,
                valid_until: ROOT_LIFETIME.after(t0),
                signature: [0; 64],
            };
            c.signature = sign(&key, &c.tbs_hash());
            elector_certs.push(c.clone());
            certs.insert(format!("elector-{k}"), c);
            keys.insert(node, key);
        }

        let issue = |role: &'static str,
                     node: NodeId,
                     issuer_role: &str,
                     certs: &mut BTreeMap<String, AuthorityCert>,
                     keys: &mut BTreeMap<NodeId, SigningKey>|
         -> [u8; 8] {
            let key = key_for(rng, node);
            let issuer = certs.get(issuer_role).expect("issuer exists").clone();
            let mut c = AuthorityCert {
                role,
                node,
                issuer: Some(issuer.digest()),
                public: public_of(&key),
                valid_from: t0,
                valid_until: AUTHORITY_LIFETIME.after(t0),
                signature: [0; 64],
            };
            let issuer_key = keys.get(&issuer.node).expect("issuer key exists");
            c.signature = sign(issuer_key, &c.tbs_hash());
            let d = c.digest();
            certs.insert(role.to_string(), c);
            keys.insert(node, key);
            d
        };
        issue("ica", nodes.ica, "root", &mut certs, &mut keys);
        counters.root_certs_issued += 1;
        for (role, node) in ROOT_ISSUED.iter().zip([nodes.pg, online.ma, online.crlg]) {
            issue(role, node, "root", &mut certs, &mut keys);
            counters.root_certs_issued += 1;
        }
        let ica_nodes = [
            online.eca,
            online.pca,
            online.ra,
            online.la1,
            online.la2,
            online.lop,
            online.dcm,
            online.crl_store,
        ];
        for (role, node) in ICA_ISSUED.iter().zip(ica_nodes) {
            issue(role, node, "ica", &mut certs, &mut keys);
            counters.ica_certs_issued += 1;
        }

        let mut ctl = SignedCtl {
            sequence: 1,
            quorum: p.elector_quorum,
            roots: vec![root_digest],
            electors: elector_certs.iter().map(AuthorityCert::digest).collect(),
            endorsements: Vec::new(),
        };
        let hash = ctl.tbs_hash();
        for (k, node) in nodes.electors.iter().enumerate() {
            let key = keys.get(node).expect("elector key");
            ctl.endorsements
                .push((u32::try_from(k).unwrap_or(u32::MAX), sign(key, &hash)));
            counters.elector_endorsements += 1;
        }

        let policy = Policy::of(p);
        let placeholder = PolicyFile {
            scope: Scope::Global,
            version: 0,
            policy,
            signer: [0; 8],
            signature: [0; 64],
        };
        let chain_placeholder = ChainFile {
            scope: Scope::Global,
            version: 0,
            certs: Vec::new(),
            signer: [0; 8],
            signature: [0; 64],
        };
        let mut gov = Governance {
            nodes,
            keys,
            certs,
            ctl,
            gpf: placeholder.clone(),
            gccf: chain_placeholder.clone(),
            lpf: placeholder,
            lccf: chain_placeholder,
            counters,
        };
        gov.sign_global(policy);
        gov.sign_local();
        gov
    }

    fn signer(&self, role: &str) -> (&SigningKey, [u8; 8]) {
        let cert = self.certs.get(role).expect("role certified at setup");
        (self.keys.get(&cert.node).expect("key held"), cert.digest())
    }

    /// The Policy Generator signs a new Global Policy File and Global Certificate Chain
    /// File. Returns the new GPF version.
    pub fn sign_global(&mut self, policy: Policy) -> u32 {
        let version = self.gpf.version + 1;
        let (key, signer) = self.signer("pg");
        let mut gpf = PolicyFile {
            scope: Scope::Global,
            version,
            policy,
            signer,
            signature: [0; 64],
        };
        gpf.signature = sign(key, &gpf.tbs_hash());
        let mut gccf = ChainFile {
            scope: Scope::Global,
            version: self.gccf.version + 1,
            certs: self.chain_certs(),
            signer,
            signature: [0; 64],
        };
        gccf.signature = sign(key, &gccf.tbs_hash());
        self.gpf = gpf;
        self.gccf = gccf;
        self.counters.pg_files_signed += 2;
        version
    }

    /// The Registration Authority re-issues its Local Policy File and Local Certificate
    /// Chain File from the current global ones. Returns the new LPF version.
    pub fn sign_local(&mut self) -> u32 {
        let version = self.lpf.version + 1;
        let (key, signer) = self.signer("ra");
        let mut lpf = PolicyFile {
            scope: Scope::Local,
            version,
            policy: self.gpf.policy,
            signer,
            signature: [0; 64],
        };
        lpf.signature = sign(key, &lpf.tbs_hash());
        let mut lccf = ChainFile {
            scope: Scope::Local,
            version: self.lccf.version + 1,
            certs: self.chain_certs(),
            signer,
            signature: [0; 64],
        };
        lccf.signature = sign(key, &lccf.tbs_hash());
        self.lpf = lpf;
        self.lccf = lccf;
        self.counters.ra_files_signed += 2;
        version
    }

    /// The authority certificates a device's chain file carries: root first, then the
    /// ICA, then every component, in role order.
    fn chain_certs(&self) -> Vec<AuthorityCert> {
        let mut out = Vec::new();
        for role in ["root", "ica"] {
            if let Some(c) = self.certs.get(role) {
                out.push(c.clone());
            }
        }
        for (role, c) in &self.certs {
            if role != "root" && role != "ica" && !role.starts_with("elector") {
                out.push(c.clone());
            }
        }
        out
    }

    /// The SCMS Manager's policy decision: the Policy Generator signs a new global policy,
    /// and the RA a new local one. Returns the new LPF version.
    pub fn decide_policy(&mut self, policy: Policy) -> u32 {
        self.counters.manager_decisions += 1;
        self.sign_global(policy);
        self.sign_local()
    }

    /// The elector certificates, in index order: the trust anchors a device receives at
    /// bootstrap.
    #[must_use]
    pub fn anchors(&self) -> Vec<AuthorityCert> {
        (0..self.nodes.electors.len())
            .filter_map(|k| self.certs.get(&format!("elector-{k}")).cloned())
            .collect()
    }

    /// Signs `hash` as the entity certified for `role`, for a flow that signs an artefact
    /// the device will check (the CRL Generator's list).
    #[must_use]
    pub fn sign_as(&self, role: &str, hash: &[u8; 32]) -> Option<([u8; 8], [u8; 64])> {
        let cert = self.certs.get(role)?;
        let key = self.keys.get(&cert.node)?;
        Some((cert.digest(), sign(key, hash)))
    }

    /// A trust list re-endorsed by only `electors` of the electors, for the negative test
    /// a device must pass: the list below quorum is refused.
    #[must_use]
    pub fn ctl_endorsed_by(&self, electors: &[u32]) -> SignedCtl {
        let mut ctl = self.ctl.clone();
        let hash = ctl.tbs_hash();
        ctl.endorsements = electors
            .iter()
            .filter_map(|&k| {
                let node = self.nodes.electors.get(k as usize)?;
                Some((k, sign(self.keys.get(node)?, &hash)))
            })
            .collect();
        ctl
    }
}

/// A device's trust store: its anchors, the trust list, the chain and the policy it holds.
#[derive(Debug, Clone, Default)]
pub struct DeviceTrust {
    /// The elector certificates installed at bootstrap.
    pub anchors: Vec<AuthorityCert>,
    /// The sequence number of the trust list installed.
    pub ctl_sequence: Option<u32>,
    /// Root digests the installed trust list endorses.
    pub roots: BTreeSet<[u8; 8]>,
    /// Authority certificates whose chain verified, by digest.
    pub chain: BTreeMap<[u8; 8], AuthorityCert>,
    /// The installed Local Certificate Chain File's version.
    pub lccf_version: Option<u32>,
    /// The installed Local Policy File.
    pub lpf: Option<PolicyFile>,
    /// Artefacts refused, with the last reason.
    pub rejected: u32,
    /// Why the last one was refused.
    pub last_rejection: Option<TrustError>,
    /// Signature verifications the device performed on trust artefacts.
    pub verifications: u32,
}

impl DeviceTrust {
    fn reject(&mut self, e: TrustError) -> TrustError {
        self.rejected += 1;
        self.last_rejection = Some(e);
        e
    }

    /// Installs a trust list that a quorum of the device's anchors endorsed.
    ///
    /// # Errors
    /// [`TrustError::CtlQuorum`] below quorum; [`TrustError::Stale`] for a list that is
    /// not newer than the installed one.
    pub fn install_ctl(&mut self, ctl: &SignedCtl) -> Result<u32, TrustError> {
        if self.ctl_sequence.is_some_and(|s| ctl.sequence <= s) {
            return Err(self.reject(TrustError::Stale));
        }
        self.verifications += u32::try_from(ctl.endorsements.len()).unwrap_or(u32::MAX);
        match ctl.verify(&self.anchors) {
            Ok(n) => {
                self.ctl_sequence = Some(ctl.sequence);
                self.roots = ctl.roots.iter().copied().collect();
                Ok(n)
            }
            Err(e) => Err(self.reject(e)),
        }
    }

    /// Installs a chain file whose every certificate chains to an endorsed root and whose
    /// signer is one of them. Returns the verifications it took.
    ///
    /// # Errors
    /// [`TrustError::UnknownIssuer`], [`TrustError::BadSignature`], [`TrustError::Stale`],
    /// [`TrustError::Expired`].
    pub fn install_chain(&mut self, file: &ChainFile, now: SimTime) -> Result<u32, TrustError> {
        if self.lccf_version.is_some_and(|v| file.version <= v) {
            return Err(self.reject(TrustError::Stale));
        }
        let mut accepted: BTreeMap<[u8; 8], AuthorityCert> = BTreeMap::new();
        let mut checks = 0u32;
        for c in &file.certs {
            if now < c.valid_from || now >= c.valid_until {
                return Err(self.reject(TrustError::Expired));
            }
            let d = c.digest();
            let issuer_key = match c.issuer {
                None if self.roots.contains(&d) => c.verifying_key(),
                None => return Err(self.reject(TrustError::UnknownIssuer)),
                Some(i) => accepted
                    .get(&i)
                    .or_else(|| self.chain.get(&i))
                    .and_then(AuthorityCert::verifying_key),
            };
            let Some(k) = issuer_key else {
                return Err(self.reject(TrustError::UnknownIssuer));
            };
            checks += 1;
            if !c.signed_by(&k) {
                return Err(self.reject(TrustError::BadSignature));
            }
            accepted.insert(d, c.clone());
        }
        let Some(signer) = accepted
            .get(&file.signer)
            .and_then(AuthorityCert::verifying_key)
        else {
            return Err(self.reject(TrustError::UnknownIssuer));
        };
        checks += 1;
        if !verify(&signer, &file.tbs_hash(), &file.signature) {
            return Err(self.reject(TrustError::BadSignature));
        }
        self.verifications += checks;
        self.chain.extend(accepted);
        self.lccf_version = Some(file.version);
        Ok(checks)
    }

    /// Installs a policy file signed by an authority in the device's chain.
    ///
    /// # Errors
    /// [`TrustError::UnknownIssuer`], [`TrustError::BadSignature`], [`TrustError::Stale`].
    pub fn install_policy(&mut self, file: &PolicyFile) -> Result<(), TrustError> {
        if self.lpf.as_ref().is_some_and(|l| file.version <= l.version) {
            return Err(self.reject(TrustError::Stale));
        }
        let Some(k) = self
            .chain
            .get(&file.signer)
            .and_then(AuthorityCert::verifying_key)
        else {
            return Err(self.reject(TrustError::UnknownIssuer));
        };
        self.verifications += 1;
        if !verify(&k, &file.tbs_hash(), &file.signature) {
            return Err(self.reject(TrustError::BadSignature));
        }
        self.lpf = Some(file.clone());
        Ok(())
    }

    /// Whether a signature over `hash` by the authority whose certificate digest is
    /// `signer` verifies against the device's chain: how a device checks a CRL.
    pub fn verify_signed(&mut self, signer: [u8; 8], hash: &[u8; 32], sig: &[u8; 64]) -> bool {
        self.verifications += 1;
        self.chain
            .get(&signer)
            .and_then(AuthorityCert::verifying_key)
            .is_some_and(|k| verify(&k, hash, sig))
    }

    /// The policy the device follows, if it has one.
    #[must_use]
    pub fn policy(&self) -> Option<Policy> {
        self.lpf.as_ref().map(|l| l.policy)
    }

    /// Whether the device holds a verified chain for `role`.
    #[must_use]
    pub fn trusts_role(&self, role: &str) -> bool {
        self.chain.values().any(|c| c.role == role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gov() -> Governance {
        let p = ScmsParams::default();
        Governance::ceremony(&p, &ScmsNodes::default(), &RngRegistry::new(7), 0)
    }

    fn bootstrapped(g: &Governance) -> DeviceTrust {
        let mut t = DeviceTrust {
            anchors: g.anchors(),
            ..DeviceTrust::default()
        };
        t.install_ctl(&g.ctl)
            .expect("the ceremony's list is endorsed by every elector");
        t.install_chain(&g.lccf, 1)
            .expect("the local chain verifies");
        t.install_policy(&g.lpf).expect("the RA's policy verifies");
        t
    }

    #[test]
    fn a_bootstrapped_device_trusts_every_online_authority() {
        let g = gov();
        let t = bootstrapped(&g);
        for role in ICA_ISSUED.iter().chain(ROOT_ISSUED.iter()) {
            assert!(t.trusts_role(role), "{role} not trusted");
        }
        assert_eq!(t.policy().map(|p| p.certs_per_period), Some(20));
        assert_eq!(t.rejected, 0);
    }

    #[test]
    fn a_trust_list_below_quorum_is_refused() {
        let g = gov();
        let mut t = DeviceTrust {
            anchors: g.anchors(),
            ..DeviceTrust::default()
        };
        let weak = g.ctl_endorsed_by(&[0]);
        assert_eq!(
            t.install_ctl(&weak),
            Err(TrustError::CtlQuorum {
                valid: 1,
                needed: 2
            })
        );
        // Without a trusted root nothing chains.
        assert_eq!(t.install_chain(&g.lccf, 1), Err(TrustError::UnknownIssuer));
        // The same list with two endorsements is accepted.
        assert!(t.install_ctl(&g.ctl_endorsed_by(&[0, 2])).is_ok());
    }

    #[test]
    fn a_list_cannot_lower_its_own_quorum() {
        let g = gov();
        let mut t = DeviceTrust {
            anchors: g.anchors(),
            ..DeviceTrust::default()
        };
        let mut ctl = g.ctl_endorsed_by(&[1]);
        ctl.quorum = 1;
        // Lowering the quorum changes what was signed, so the one endorsement fails too.
        assert!(matches!(
            t.install_ctl(&ctl),
            Err(TrustError::CtlQuorum { .. })
        ));
    }

    #[test]
    fn a_tampered_policy_or_chain_is_refused() {
        let g = gov();
        let mut t = bootstrapped(&g);
        let mut next = g.lpf.clone();
        next.version += 1;
        next.policy.certs_per_period = 100;
        assert_eq!(t.install_policy(&next), Err(TrustError::BadSignature));
        let mut chain = g.lccf.clone();
        chain.version += 1;
        chain.certs[2].public[5] ^= 1;
        assert_eq!(t.install_chain(&chain, 1), Err(TrustError::BadSignature));
        assert_eq!(t.rejected, 2);
    }

    #[test]
    fn a_policy_decision_reaches_the_device_as_a_newer_signed_file() {
        let mut g = gov();
        let mut t = bootstrapped(&g);
        let mut p = g.gpf.policy;
        p.certs_per_period = 10;
        let v = g.decide_policy(p);
        assert_eq!(v, 2);
        t.install_policy(&g.lpf).expect("verifies");
        assert_eq!(t.policy().map(|p| p.certs_per_period), Some(10));
        // The old file again is stale.
        assert_eq!(t.install_policy(&g.lpf), Err(TrustError::Stale));
        assert_eq!(g.counters.manager_decisions, 1);
    }

    #[test]
    fn the_ceremony_is_deterministic() {
        let a = gov();
        let b = gov();
        assert_eq!(a.ctl, b.ctl);
        assert_eq!(a.lccf, b.lccf);
        let c = Governance::ceremony(
            &ScmsParams::default(),
            &ScmsNodes::default(),
            &RngRegistry::new(8),
            0,
        );
        assert_ne!(
            a.ctl.roots, c.ctl.roots,
            "a different seed makes different keys"
        );
    }
}
