//! The node's own security stack: real payloads, real signatures, real bytes.
//!
//! # What this module exists to stop
//!
//! Until 2026-09-22 a node's `generate` path called neither a codec nor a crypto backend.
//! It produced a `u32` — `93 + signer identifier`, with a zero payload — and handed that
//! to the engine as if it were a message. The tell an audit found before reading a line of
//! code: a basic safety message and a cooperative awareness message both came out at
//! exactly 101 bytes. Two different standards, two different formats, two different field
//! sets, one number. That cannot happen to real bytes, and it is why
//! [`crate::runtime::ObuRuntime`] now encodes and signs for real and why the first test in
//! `tests/wire_bytes.rs` is that the two sizes *differ*.
//!
//! It matters more here than it would in most projects: security overhead as a fraction of
//! airtime, verification cost under load and what a node does when it cannot keep up with
//! signing are the three headline results this simulator exists to produce, and all three
//! are ratios whose numerator was a constant.
//!
//! # The three pieces
//!
//! | Piece | What it is |
//! |---|---|
//! | [`SecCtx`] | the bridge from [`NodeCtx`] to [`v2xw_core::ctx::Ctx`], which `v2xw-sec` is generic over. It carries **no** world and **no** actors: both associated types are `()`, and [`crate::firewall::scan_security_bridge`] fails the build if either stops being. |
//! | [`NodeCrypto`] | the backend, `real` or `modeled`. Nothing outside this enum branches on which — invariant I-S1, and Phase 1 acceptance criterion 3. |
//! | [`NodeSecurity`] | the envelope, the signer handles installed from the SCMS credentials, and the node's CA certificate cache. |
//!
//! # Where the credentials come from (WP-C1)
//!
//! A node does not issue itself anything. Every credential it signs with was handed down
//! by the credential protocol as a [`v2xw_sec::credential::ImplicitKey`]: the implicit
//! (ECQV) certificate the ACA issued, the ACA's own certificate, and `d = e·b + r`, which
//! the device derived from its download (02 §9.3). [`NodeSecurity::install`] imports the
//! key after the device's acceptance check `d·G = e·P_U + Q_ACA` and names the certificate
//! by its own `HashedId8`, so the air identity, the linkage value a CRL revokes and the
//! certificate bytes on the air are all the protocol's. A store handle with no installed
//! key cannot sign, and the node then stops transmitting rather than sending unsigned.
//!
//! A relying party reconstructs a sender's key as `e·P_U + Q_ACA`, which needs the issuing
//! ACA's certificate. A node holds the ACA certificates of its own downloads; a sender
//! whose ACA it does not hold is [`SpduVerdict::Unverifiable`], never `Invalid` (the
//! P2PCD case). What this module still does **not** do is walk the chain from the ACA to
//! the root, check validity, region, PSID and SSP, or check the ACA itself for
//! revocation: that is WP-C2's receiver validation, stated rather than papered over in
//! [`NodeSecurity::verify_parsed`].

use std::collections::BTreeMap;
use std::sync::Arc;

use v2xw_core::ctx::{Ctx, ErasedRecord};
use v2xw_core::event::{EventClass, EventHandle};
use v2xw_core::ids::NodeId;
use v2xw_core::provenance::ProvSubject;
use v2xw_core::registry::{ModelRef, ParamSet, ParamSetId};
use v2xw_core::rng::{EntityRef, RngDomain, RngGuard};
use v2xw_core::time::{SimTime, WallClock};
use v2xw_msg::MsgType;
use v2xw_msg::sec_types::ieee1609_dot2::{CertificateType, IssuerIdentifier};
use v2xw_msg::sec_types::{Certificate, HashedId8};
use v2xw_sec::cert;
use v2xw_sec::credential::ImplicitKey;
use v2xw_sec::crypto::{CryptoBackend, CryptoBackendInfo, KeyHandle, PubHandle, SigToken};
use v2xw_sec::envelope::{
    Envelope, EnvelopeProfile, GenerationLocation, HeaderInfoSpec, ParsedSecured, ParsedSigner,
    SecuredPdu, SecurityEnvelope, SecurityEnvelopeInfo, SignerHandle, SignerIdChoice,
    SignerIdPolicy,
};
use v2xw_sec::primitive::PrimitiveId;
use v2xw_sec::{SecError, hashedid};

use crate::ctx::NodeCtx;

/// The PSID a CAM or a BSM is signed under.
///
/// 0x20, the value 04-models.md §9.1 uses in its envelope-overhead derivation and the one
/// the 93-byte figure is measured at. A PSID of 0x100 or more costs a third COER byte, so
/// this is not a free choice and a scenario that changes it changes the overhead.
pub const PSID_SAFETY: u64 = 0x20;

/// The signature primitive the node signs and verifies with.
///
/// The only entry in [`v2xw_sec::MODE_INDEPENDENT_PRIMITIVES`] that a safety message uses,
/// which is what makes the backend a swap rather than a scenario decision: a run that
/// selected a post-quantum primitive would refuse to start in `real` mode instead of
/// quietly diverging (see that constant's own documentation).
pub const SIGN_PRIMITIVE: PrimitiveId = PrimitiveId::ECDSA_P256_SHA256;

// =========================================================================================
// The context bridge
// =========================================================================================

/// Adapts [`NodeCtx`] to the [`Ctx`] that `v2xw-sec` is generic over.
///
/// # Why this is not a hole in the firewall
///
/// [`Ctx`] has `world()` and `actors()`; [`NodeCtx`] deliberately does not (build decision
/// D12.2). Bridging one to the other is exactly the shape of change that would re-open the
/// firewall, so the bridge is built so that it *cannot*: `World` and `Actors` are both the
/// unit type. There is no world here to return and no actor list to index — not "the node
/// promises not to look", but "there is nothing to look at".
///
/// That is a textual property, so [`crate::firewall::scan_security_bridge`] checks it
/// textually, and `tests/firewall_sentinel.rs` names the fault injected to prove the check
/// goes red.
///
/// # Scheduling
///
/// The security layer never schedules. `v2xw-sec`'s own test double records the same
/// observation — "a crypto backend needs `now()` and `rng()`, and an envelope needs
/// `now()`" — and this bridge turns it into an assertion: [`SecCtx::schedule`] panics.
/// A silently dropped event would be a queue that never fires and a cost never charged,
/// which is precisely the class of defect this crate keeps finding.
pub struct SecCtx<'a> {
    inner: &'a mut dyn NodeCtx,
    params: ParamSet,
    /// The unit value `world()` and `actors()` return. See the type documentation.
    nothing: (),
}

impl core::fmt::Debug for SecCtx<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SecCtx")
            .field("now", &self.inner.now())
            .finish_non_exhaustive()
    }
}

impl<'a> SecCtx<'a> {
    /// Wraps a node context for the length of one security operation.
    pub fn new(inner: &'a mut dyn NodeCtx) -> SecCtx<'a> {
        SecCtx {
            inner,
            params: ParamSet::new(),
            nothing: (),
        }
    }
}

impl Ctx for SecCtx<'_> {
    type World = ();
    type Actors = ();
    type Payload = ();

    fn now(&self) -> SimTime {
        self.inner.now()
    }

    fn rng(&self, domain: RngDomain, entity: EntityRef) -> RngGuard<'_> {
        self.inner.rng(domain, entity)
    }

    fn schedule(&mut self, _at: SimTime, _class: EventClass, _payload: ()) -> EventHandle {
        panic!(
            "the node's security bridge carries no scheduler: signing and verification are \
             synchronous, and the modelled time they take is charged against the node's own \
             service model (06-node-models.md §2.1). A security model that needs to schedule \
             needs the engine's context, not this one."
        )
    }

    fn cancel(&mut self, _handle: EventHandle) -> bool {
        false
    }

    fn world(&self) -> &Self::World {
        &self.nothing
    }

    fn actors(&self) -> &Self::Actors {
        &self.nothing
    }

    fn emit_erased(&mut self, record: &dyn ErasedRecord) {
        self.inner.emit_erased(record);
    }

    fn why(&mut self, _subject: ProvSubject, _model: ModelRef, _params: ParamSetId) {}

    fn params(&self) -> &ParamSet {
        &self.params
    }
}

// =========================================================================================
// The backend
// =========================================================================================

/// Which crypto backend a node runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CryptoMode {
    /// [`v2xw_sec::Modeled`]: right sizes, right outcomes, no curve arithmetic.
    #[default]
    Modeled,
    /// [`v2xw_sec::Real`]: ECDSA P-256 with RFC 6979.
    Real,
}

/// The node's crypto backend.
///
/// An enum and not a `Box<dyn CryptoBackend<_>>` because [`CryptoBackend`] is generic over
/// the context type and its object form would need a higher-ranked trait object Rust
/// cannot express. What matters for invariant I-S1 is not the dispatch mechanism but that
/// **nothing outside these two `match` arms knows which backend is running**: the sizes are
/// equal by construction (a modelled token is exactly the descriptor's `sig_bytes`, a
/// modelled public key is 33 bytes in the compressed shape), so every encoded length, every
/// record and every queueing decision is identical in both modes.
#[derive(Debug)]
pub enum NodeCrypto {
    /// The modelled backend.
    Modeled(Box<v2xw_sec::Modeled>),
    /// The real backend.
    Real(Box<v2xw_sec::Real>),
}

impl NodeCrypto {
    /// A backend for `mode`, holding no keys.
    pub fn new(mode: CryptoMode) -> NodeCrypto {
        match mode {
            CryptoMode::Modeled => NodeCrypto::Modeled(Box::new(v2xw_sec::Modeled::new())),
            CryptoMode::Real => NodeCrypto::Real(Box::new(v2xw_sec::Real::new())),
        }
    }

    /// Which mode this backend implements.
    pub fn mode(&self) -> CryptoMode {
        match self {
            NodeCrypto::Modeled(_) => CryptoMode::Modeled,
            NodeCrypto::Real(_) => CryptoMode::Real,
        }
    }

    /// The backend's model id, as a manifest records it.
    pub fn backend_id(&self) -> &'static str {
        match self {
            NodeCrypto::Modeled(b) => b.backend_id(),
            NodeCrypto::Real(b) => b.backend_id(),
        }
    }

    /// Generates a signing key for `owner` on the node's own deterministic stream.
    pub fn keygen(&mut self, ctx: &mut dyn NodeCtx, owner: NodeId) -> Result<KeyHandle, SecError> {
        let mut c = SecCtx::new(ctx);
        match self {
            NodeCrypto::Modeled(b) => b.keygen(&mut c, SIGN_PRIMITIVE, owner),
            NodeCrypto::Real(b) => b.keygen(&mut c, SIGN_PRIMITIVE, owner),
        }
    }

    /// The 33-byte compressed public material of a key this backend holds.
    pub fn public_material_of(&self, key: &KeyHandle) -> Result<Vec<u8>, SecError> {
        match self {
            NodeCrypto::Modeled(b) => b.public_material(&b.public_of(key)?),
            NodeCrypto::Real(b) => b.public_material(&b.public_of(key)?),
        }
    }

    /// Imports a peer's public key from the 33 bytes a certificate carries.
    pub fn import_public(&mut self, owner: NodeId, material: &[u8]) -> Result<PubHandle, SecError> {
        match self {
            NodeCrypto::Modeled(b) => b.import_public(SIGN_PRIMITIVE, owner, material),
            NodeCrypto::Real(b) => b.import_public(SIGN_PRIMITIVE, owner, material),
        }
    }

    /// Imports the signing key of an SCMS-issued implicit certificate, `d = e·b + r`,
    /// after the device's own acceptance check (both backends make it).
    pub fn import_implicit(
        &mut self,
        owner: NodeId,
        key: &ImplicitKey,
    ) -> Result<KeyHandle, SecError> {
        match self {
            NodeCrypto::Modeled(b) => b.import_implicit(SIGN_PRIMITIVE, owner, key),
            NodeCrypto::Real(b) => b.import_implicit(SIGN_PRIMITIVE, owner, key),
        }
    }

    /// The key a relying party reconstructs from an implicit certificate and its issuer,
    /// `Q_U = e·P_U + Q_CA`.
    pub fn import_reconstructed(
        &mut self,
        owner: NodeId,
        cert: &Certificate,
        issuer: &Certificate,
    ) -> Result<PubHandle, SecError> {
        match self {
            NodeCrypto::Modeled(b) => b.import_reconstructed(SIGN_PRIMITIVE, owner, cert, issuer),
            NodeCrypto::Real(b) => b.import_reconstructed(SIGN_PRIMITIVE, owner, cert, issuer),
        }
    }

    /// Signs a 32-byte digest.
    pub fn sign_prehashed(
        &mut self,
        ctx: &mut dyn NodeCtx,
        key: &KeyHandle,
        digest: &[u8; 32],
    ) -> Result<SigToken, SecError> {
        let mut c = SecCtx::new(ctx);
        match self {
            NodeCrypto::Modeled(b) => b.sign_prehashed(&mut c, key, digest),
            NodeCrypto::Real(b) => b.sign_prehashed(&mut c, key, digest),
        }
    }

    /// Checks a signature over a 32-byte digest.
    pub fn verify_prehashed(
        &mut self,
        ctx: &mut dyn NodeCtx,
        pk: &PubHandle,
        digest: &[u8; 32],
        sig: &SigToken,
    ) -> bool {
        let mut c = SecCtx::new(ctx);
        match self {
            NodeCrypto::Modeled(b) => b.verify_prehashed(&mut c, pk, digest, sig),
            NodeCrypto::Real(b) => b.verify_prehashed(&mut c, pk, digest, sig),
        }
    }

    /// Signs a payload into a `SignedData` SPDU through `envelope`.
    pub fn sign_envelope(
        &mut self,
        envelope: &Envelope,
        ctx: &mut dyn NodeCtx,
        signer: &SignerHandle,
        payload: &[u8],
        hdr: &HeaderInfoSpec,
        sid: SignerIdChoice,
    ) -> Result<SecuredPdu, SecError> {
        let mut c = SecCtx::new(ctx);
        match self {
            NodeCrypto::Modeled(b) => envelope.sign(&mut c, b.as_mut(), signer, payload, hdr, sid),
            NodeCrypto::Real(b) => envelope.sign(&mut c, b.as_mut(), signer, payload, hdr, sid),
        }
    }
}

// =========================================================================================
// The node's security stack
// =========================================================================================

/// What a receiver concluded about an SPDU's signature, having spent the time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpduVerdict {
    /// The signature checks out against the certificate the SPDU named.
    Valid,
    /// The bytes do not decode, or the signature does not check out.
    Invalid,
    /// The signer's certificate is not held, so nothing has been concluded. This is the
    /// P2PCD case, and it is *not* a rejection: a node that answered `Invalid` here would
    /// blame a peer for its own empty cache.
    Unverifiable,
}

/// One signed frame: the payload, the SPDU, and the split between them.
///
/// The split is the whole point. `bytes_on_wire = payload_bytes + envelope_bytes`, and the
/// security overhead as a fraction of airtime — one of the three results this simulator
/// exists to produce — is `envelope_bytes / bytes_on_wire` aggregated over a run. With a
/// constant-size stub that ratio was a constant; here it moves with the message type, with
/// the signer-identifier cadence and with the certificate's own encoded size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedFrame {
    /// The facilities-layer payload, as the codec encoded it.
    pub payload: Vec<u8>,
    /// The IEEE 1609.2 `SignedData` SPDU: what actually goes on the air.
    pub spdu: Vec<u8>,
    /// The octets of the signer's certificate inside the envelope when a full certificate
    /// was attached (its COER encoding), zero when the signer was named by an eight-octet
    /// digest. What the certificate-versus-digest share of the envelope is computed from.
    pub cert_bytes: u32,
}

impl SignedFrame {
    /// The payload's length in octets.
    pub fn payload_bytes(&self) -> u32 {
        u32::try_from(self.payload.len()).unwrap_or(u32::MAX)
    }

    /// The SPDU's length in octets — payload plus envelope.
    pub fn bytes_on_wire(&self) -> u32 {
        u32::try_from(self.spdu.len()).unwrap_or(u32::MAX)
    }

    /// The security envelope's cost in octets: everything the SPDU carries that is not the
    /// payload.
    pub fn envelope_bytes(&self) -> u32 {
        self.bytes_on_wire().saturating_sub(self.payload_bytes())
    }
}

/// A node's security stack: the envelope, the backend, and the credentials in use.
#[derive(Debug)]
pub struct NodeSecurity {
    envelope: Envelope,
    crypto: NodeCrypto,
    psid: u64,
    /// Whether the node is on the European stack, which picks each message's ITS-AID
    /// ([`v2xw_msg::registry::psid`]); `None` signs every message under `psid`, as the
    /// stack did before the registry was wired in.
    stack: Option<bool>,
    /// The PSIDs the node's certificates permit ([`v2xw_msg::registry::permissions`]);
    /// empty means `[psid]`.
    permissions: Vec<u64>,
    /// The CA certificates this node holds, by whole-certificate `HashedId8`: the issuers
    /// of its own credentials (each download carries its ACA certificate) and any the
    /// engine seeded from the local certificate chain file. A relying party needs the
    /// issuer to reconstruct an implicit certificate's key at all, so a signer whose ACA is
    /// not here is [`SpduVerdict::Unverifiable`], never `Invalid`.
    issuers: BTreeMap<[u8; 8], Arc<Certificate>>,
    /// One signer handle per pseudonym, keyed by `(i_period, j_index)`.
    ///
    /// Every credential the store holds gets one, not just the active one, and that is
    /// load-bearing rather than eager: a credential's `digest` is written back from the
    /// certificate here, the revocation path matches credentials *by digest*, and a
    /// certificate that only came into existence when the node rotated onto it would let
    /// a revoked node dodge its own revocation by rotating.
    signers: BTreeMap<(u32, u32), SignerHandle>,
    /// Which pseudonym is signing now.
    active: Option<(u32, u32)>,
    /// Public key handles for peers, keyed by certificate digest, so a repeat sender costs
    /// one import rather than one per message.
    peer_keys: BTreeMap<[u8; 8], PubHandle>,
    /// When a full certificate was last attached, per message type, for the
    /// [`SignerIdPolicy`] cadence of 05-protocols.md §2.4.
    last_certificate: BTreeMap<&'static str, SimTime>,
    cam_policy: SignerIdPolicy,
    bsm_policy: SignerIdPolicy,
    /// Attach the certificate to the next awareness message after a station this node has
    /// not heard before appears (`security.signer_id_policy.on_new_neighbour`).
    attach_on_new_neighbour: bool,
    /// A new station has been heard since the last certificate went out.
    new_neighbour_pending: bool,
}

impl NodeSecurity {
    /// A stack signing under `psid` on `wall`, with `mode`'s backend.
    pub fn new(wall: WallClock, mode: CryptoMode, psid: u64) -> NodeSecurity {
        NodeSecurity {
            envelope: Envelope::ieee1609(wall),
            crypto: NodeCrypto::new(mode),
            psid,
            stack: None,
            permissions: Vec::new(),
            issuers: BTreeMap::new(),
            signers: BTreeMap::new(),
            active: None,
            peer_keys: BTreeMap::new(),
            last_certificate: BTreeMap::new(),
            // 05-protocols.md §2.4: J2945/1 attaches a full certificate every 450 ms, the
            // ETSI stack once a second. Two cadences because they are two standards, and
            // the difference is real bytes on the air.
            cam_policy: SignerIdPolicy::ETSI_1S,
            bsm_policy: SignerIdPolicy::SAE_450MS,
            attach_on_new_neighbour: false,
            new_neighbour_pending: false,
        }
    }

    /// The same stack, attaching the certificate to the next awareness message whenever a
    /// station it has not heard before appears, on top of the cadence. This is the ETSI
    /// CAM rule (TS 103 097 V1.3.1 §7.1.1: a station that receives a CAM from a station
    /// it does not know includes its own certificate in its next CAM), so a newcomer can
    /// verify this node without waiting up to one cadence or asking by P2PCD.
    #[must_use]
    pub fn with_new_neighbour_attach(mut self, on: bool) -> NodeSecurity {
        self.attach_on_new_neighbour = on;
        self
    }

    /// Notes that a station this node had not heard before has just been heard. Under
    /// [`NodeSecurity::with_new_neighbour_attach`] the next awareness message carries the
    /// certificate; otherwise this does nothing.
    pub fn note_new_neighbour(&mut self) {
        if self.attach_on_new_neighbour {
            self.new_neighbour_pending = true;
        }
    }

    /// The same stack under a different envelope profile.
    #[must_use]
    pub fn with_profile(mut self, profile: EnvelopeProfile, wall: WallClock) -> NodeSecurity {
        self.envelope = match profile {
            EnvelopeProfile::Ieee1609Dot2 => Envelope::ieee1609(wall),
            EnvelopeProfile::EtsiTs103097 => Envelope::etsi(wall),
        };
        self
    }

    /// The same stack with different certificate-attachment cadences.
    #[must_use]
    pub fn with_signer_id_policies(
        mut self,
        cam: SignerIdPolicy,
        bsm: SignerIdPolicy,
    ) -> NodeSecurity {
        self.cam_policy = cam;
        self.bsm_policy = bsm;
        self
    }

    /// Which backend is running.
    pub fn mode(&self) -> CryptoMode {
        self.crypto.mode()
    }

    /// The backend, for a caller that needs to import a peer key directly.
    pub fn crypto_mut(&mut self) -> &mut NodeCrypto {
        &mut self.crypto
    }

    /// The envelope.
    pub fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    /// The signer handle currently in use, once one has been provisioned and selected.
    pub fn signer(&self) -> Option<&SignerHandle> {
        self.active.and_then(|k| self.signers.get(&k))
    }

    /// The signer handle for one pseudonym, if it has been provisioned.
    pub fn signer_for_pseudonym(&self, i_period: u32, j_index: u32) -> Option<&SignerHandle> {
        self.signers.get(&(i_period, j_index))
    }

    /// Selects the pseudonym that signs from now on. `false` if it is not provisioned.
    pub fn set_active(&mut self, i_period: u32, j_index: u32) -> bool {
        if self.signers.contains_key(&(i_period, j_index)) {
            self.active = Some((i_period, j_index));
            true
        } else {
            false
        }
    }

    /// The PSID messages are signed under when no stack is set
    /// ([`NodeSecurity::set_stack`]).
    pub fn psid(&self) -> u64 {
        self.psid
    }

    /// Signs each message under its registered PSID / ITS-AID on the US (`etsi` false) or
    /// the European stack, and makes the certificates permit exactly the messages in
    /// `sends` ([`v2xw_msg::registry`]). Call before the first credential is provisioned:
    /// a certificate already issued keeps its permissions.
    pub fn set_stack(&mut self, etsi: bool, sends: &[MsgType]) {
        self.stack = Some(etsi);
        self.permissions = v2xw_msg::registry::permissions(sends, etsi);
    }

    /// The PSID / ITS-AID `msg_type` is signed under.
    pub fn psid_for(&self, msg_type: MsgType) -> u64 {
        match self.stack {
            Some(etsi) => v2xw_msg::registry::psid(msg_type, etsi),
            None => self.psid,
        }
    }

    /// The PSIDs this node's certificates must permit: what a factory or a credential
    /// request asks the issuer for.
    pub fn cert_permissions(&self) -> Vec<u64> {
        if self.permissions.is_empty() {
            vec![self.psid]
        } else {
            self.permissions.clone()
        }
    }

    /// Installs — or reuses — the signer handle for one SCMS-issued credential.
    ///
    /// `key` is what the credential protocol handed down: the implicit certificate, the
    /// issuing ACA's certificate and `d = e·b + r`. The key is imported after the device's
    /// own check `d·G = e·P_U + Q_ACA`, the handle names the certificate by its own
    /// `HashedId8`, and the ACA certificate goes into this node's CA cache so that it can
    /// verify its peers' certificates from the same ACA.
    ///
    /// # Errors
    /// The key does not reconstruct, or the certificate does not encode.
    pub fn install(
        &mut self,
        node: NodeId,
        i_period: u32,
        j_index: u32,
        key: &ImplicitKey,
    ) -> Result<&SignerHandle, SecError> {
        if !self.signers.contains_key(&(i_period, j_index)) {
            let handle = self.crypto.import_implicit(node, key)?;
            self.learn_issuer(Arc::clone(key.issuer()));
            let signer = SignerHandle::new(node, handle, Arc::clone(key.certificate()))?;
            self.signers.insert((i_period, j_index), signer);
        }
        Ok(&self.signers[&(i_period, j_index)])
    }

    /// Adds a CA certificate to this node's cache (an ACA certificate from a download or
    /// from the local certificate chain file). Idempotent.
    pub fn learn_issuer(&mut self, ca: Arc<Certificate>) {
        if let Ok(coer) = cert::encode(&ca) {
            let id = hashedid::id8_bytes(&hashedid::hashed_id8(&coer));
            self.issuers.entry(id).or_insert(ca);
        }
    }

    /// The CA certificate an implicit certificate names as its issuer, if this node holds
    /// it.
    pub fn issuer_of(&self, c: &Certificate) -> Option<&Arc<Certificate>> {
        match &c.0.issuer {
            IssuerIdentifier::sha256AndDigest(d) => self.issuers.get(&hashedid::id8_bytes(d)),
            _ => None,
        }
    }

    /// Whether a full certificate is due for `msg_type` at `believed`.
    ///
    /// The cadence of 05-protocols.md §2.4, resolved through the envelope so that the
    /// profile's own rules (TS 103 097 §7.1.2 makes a DENM's signer always a certificate)
    /// apply first and no caller can forget them.
    pub fn signer_id_for(&self, msg_type: MsgType, believed: SimTime) -> SignerIdChoice {
        let policy = match msg_type {
            MsgType::Bsm => self.bsm_policy,
            _ => self.cam_policy,
        };
        let choice = self.envelope.choose_signer_id(
            policy,
            believed,
            self.last_certificate.get(msg_type.as_str()).copied(),
            Some(msg_type),
        );
        if self.new_neighbour_pending && matches!(msg_type, MsgType::Bsm | MsgType::Cam) {
            return SignerIdChoice::Certificate;
        }
        choice
    }

    /// Signs `payload` and returns the SPDU with the payload beside it.
    ///
    /// The caller must have called [`NodeSecurity::provision`] for the credential in use;
    /// signing without a credential is [`SecError`], never a zero-length signature.
    pub fn sign(
        &mut self,
        ctx: &mut dyn NodeCtx,
        msg_type: MsgType,
        payload: &[u8],
        sid: SignerIdChoice,
        location: Option<GenerationLocation>,
    ) -> Result<(SignedFrame, SecuredPdu), SecError> {
        let signer = self.signer().cloned().ok_or(SecError::MissingField {
            profile: "node/security",
            field: "no credential has been provisioned; a node without one stops \
                    transmitting rather than sending unsigned [CAMP-EE §2.2.10.2]",
        })?;
        let hdr = HeaderInfoSpec {
            psid: self.psid_for(msg_type),
            msg_type: Some(msg_type),
            generation_location: location,
            ..HeaderInfoSpec::default()
        };
        let pdu = self
            .crypto
            .sign_envelope(&self.envelope, ctx, &signer, payload, &hdr, sid)?;
        let cert_bytes = if pdu.signer_id == v2xw_sec::SignerIdChoice::Certificate {
            u32::try_from(signer.cert_coer().len()).unwrap_or(u32::MAX)
        } else {
            0
        };
        let frame = SignedFrame {
            payload: payload.to_vec(),
            spdu: pdu.bytes().to_vec(),
            cert_bytes,
        };
        Ok((frame, pdu))
    }

    /// Signs `payload` under one of the node's *other* provisioned pseudonyms, with the
    /// full certificate attached, without changing which pseudonym is active.
    ///
    /// What a Sybil attacker does with the concurrent pseudonyms an SCMS issues (20 a
    /// week under CAMP, `certs_per_period`): each ghost frame is a genuine SPDU a receiver
    /// can verify, and it carries its certificate because no receiver has ever heard that
    /// pseudonym before. `None` if the pseudonym was never provisioned.
    pub fn sign_as(
        &mut self,
        ctx: &mut dyn NodeCtx,
        i_period: u32,
        j_index: u32,
        msg_type: MsgType,
        payload: &[u8],
    ) -> Option<(SignedFrame, SecuredPdu)> {
        let signer = self.signers.get(&(i_period, j_index)).cloned()?;
        let hdr = HeaderInfoSpec {
            psid: self.psid_for(msg_type),
            msg_type: Some(msg_type),
            ..HeaderInfoSpec::default()
        };
        let pdu = self
            .crypto
            .sign_envelope(
                &self.envelope,
                ctx,
                &signer,
                payload,
                &hdr,
                SignerIdChoice::Certificate,
            )
            .ok()?;
        let cert_bytes = u32::try_from(signer.cert_coer().len()).unwrap_or(u32::MAX);
        let frame = SignedFrame {
            payload: payload.to_vec(),
            spdu: pdu.bytes().to_vec(),
            cert_bytes,
        };
        Some((frame, pdu))
    }

    /// Records that a full certificate went out for `msg_type` at `believed`.
    ///
    /// A certificate sent only because a new station appeared does not restart the
    /// cadence: the two are independent triggers (TS 103 097 V1.3.1 §7.1.1 states the
    /// once-a-second rule and the new-station rule side by side). Restarting it measurably
    /// hurt: when a fleet changes pseudonym together, every vehicle hears newcomers just
    /// before its own change, spends the forced certificate on its old pseudonym, and the
    /// restarted cadence then held back its new certificate for a full period — unknown
    /// signers in the change second rose from 289 to 2,105 (20 vehicles, 1 s cadence).
    pub fn note_certificate_attached(&mut self, msg_type: MsgType, believed: SimTime) {
        let awareness = matches!(msg_type, MsgType::Bsm | MsgType::Cam);
        let forced_only = awareness && self.new_neighbour_pending && {
            let policy = match msg_type {
                MsgType::Bsm => self.bsm_policy,
                _ => self.cam_policy,
            };
            self.envelope.choose_signer_id(
                policy,
                believed,
                self.last_certificate.get(msg_type.as_str()).copied(),
                Some(msg_type),
            ) == SignerIdChoice::Digest
        };
        if !forced_only {
            self.last_certificate.insert(msg_type.as_str(), believed);
        }
        if awareness {
            self.new_neighbour_pending = false;
        }
    }

    /// Takes back a certificate attachment whose frame never reached the air.
    ///
    /// A silent period after a pseudonym change discards the safety messages the node
    /// signed during it, after signing. Had the cadence kept their certificates, the first
    /// frame after the silence would name the new pseudonym by digest while no neighbour
    /// had ever heard its certificate, and every receiver would miss it (a fleet of 20 with
    /// a 2-4 s silence: 843 unknown signers became 4,051). The cadence restarts with the
    /// first frame that is actually sent.
    pub fn withdraw_certificate(&mut self, msg_type: MsgType) {
        self.last_certificate.remove(msg_type.as_str());
    }

    /// Decodes an SPDU, or `None` when the bytes are not a `SignedData` this profile
    /// admits.
    ///
    /// Separate from [`NodeSecurity::verify_parsed`] because the caller has to resolve the
    /// signer's certificate in between, out of its own bounded cache — which is what makes
    /// P2PCD a cost rather than a formality.
    ///
    /// Every receiver of one frame is handed the same bytes, one after another, so the
    /// last decode is kept per thread and reused when the profile and the bytes match.
    /// Decoding is a pure function of those two, so the reuse cannot change a verdict.
    pub fn parse(&self, bytes: &[u8]) -> Option<ParsedSecured> {
        let profile = SecurityEnvelopeInfo::profile(&self.envelope);
        LAST_PARSE.with(|cell| {
            let mut last = cell.borrow_mut();
            if let Some((p, b, parsed)) = last.as_ref() {
                if *p == profile && b.as_slice() == bytes {
                    return parsed.clone();
                }
            }
            let parsed = self.envelope.parse(bytes).ok();
            *last = Some((profile, bytes.to_vec(), parsed.clone()));
            parsed
        })
    }

    /// Checks a parsed SPDU's signature against `certificate`.
    ///
    /// An implicit certificate's key is reconstructed under its issuing ACA's certificate
    /// from this node's CA cache; without it the answer is `Unverifiable`.
    ///
    /// What this does **not** do yet is validate the chain above the ACA, validity,
    /// region, PSID or SSP (WP-C2). See the module documentation.
    pub fn verify_parsed(
        &mut self,
        ctx: &mut dyn NodeCtx,
        parsed: &ParsedSecured,
        certificate: &Certificate,
        owner: NodeId,
    ) -> SpduVerdict {
        let Ok(cert_coer) = cert::encode(certificate) else {
            return SpduVerdict::Invalid;
        };
        let digest = parsed.signing_digest(&cert_coer);
        // An implicit certificate's key exists only once it is reconstructed under the
        // issuer, so a signer whose ACA certificate this node lacks has told it nothing.
        if certificate.0.r_type == CertificateType::implicit
            && self.issuer_of(certificate).is_none()
        {
            return SpduVerdict::Unverifiable;
        }
        // Material that is not a compressed point, or a reconstruction no holder's key
        // matches, is a malformed certificate: the sender's fault and a rejection rather
        // than an inconclusive answer.
        let Ok(key) = self.peer_key(owner, certificate, &cert_coer) else {
            return SpduVerdict::Invalid;
        };
        let token = SigToken {
            primitive: SIGN_PRIMITIVE,
            bytes: parsed.signature.clone(),
        };
        if self.crypto.verify_prehashed(ctx, &key, &digest, &token) {
            SpduVerdict::Valid
        } else {
            SpduVerdict::Invalid
        }
    }

    /// The certificate a parsed SPDU attached, if it attached one.
    pub fn attached_certificate(parsed: &ParsedSecured) -> Option<Arc<Certificate>> {
        match &parsed.signer {
            ParsedSigner::Certificate { certificate, .. } => {
                Some(Arc::new((**certificate).clone()))
            }
            ParsedSigner::Digest(_) | ParsedSigner::SelfSigned => None,
        }
    }

    /// The digest a parsed SPDU names its signer by, whichever identifier it used.
    pub fn parsed_signer_digest(parsed: &ParsedSecured) -> Option<HashedId8> {
        match &parsed.signer {
            ParsedSigner::Certificate { digest, .. } => Some(digest.clone()),
            ParsedSigner::Digest(d) => Some(d.clone()),
            ParsedSigner::SelfSigned => None,
        }
    }

    fn peer_key(
        &mut self,
        owner: NodeId,
        certificate: &Certificate,
        cert_coer: &[u8],
    ) -> Result<PubHandle, SecError> {
        let mut id = [0u8; 8];
        id.copy_from_slice(&hashedid::hashed_id8(cert_coer).0[..]);
        if let Some(handle) = self.peer_keys.get(&id) {
            return Ok(*handle);
        }
        let handle = if certificate.0.r_type == CertificateType::implicit {
            let issuer = self
                .issuer_of(certificate)
                .cloned()
                .ok_or(SecError::MissingField {
                    profile: "implicit certificate",
                    field: "the issuer certificate (needed to reconstruct the key)",
                })?;
            self.crypto
                .import_reconstructed(owner, certificate, &issuer)?
        } else {
            let material = cert::public_key_material(certificate)?;
            self.crypto.import_public(owner, &material)?
        };
        self.peer_keys.insert(id, handle);
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::NodeRuntimeCtx;
    use v2xw_core::rng::RngRegistry;
    use v2xw_sec::credential::{FactoryProfile, ImplicitIssuer};
    use v2xw_sec::linkage::LinkageValue;

    fn stack(mode: CryptoMode) -> NodeSecurity {
        NodeSecurity::new(WallClock::new(1_767_225_600), mode, PSID_SAFETY)
    }

    /// The bridge carries no truth: both associated types are the unit type, so there is
    /// nothing for a security model to read even if it asked.
    #[test]
    fn the_security_bridge_carries_no_world_and_no_actors() {
        let reg = RngRegistry::new(1);
        let mut inner = NodeRuntimeCtx::new(0, &reg);
        let c = SecCtx::new(&mut inner);
        assert_eq!(core::mem::size_of_val(c.world()), 0);
        assert_eq!(core::mem::size_of_val(c.actors()), 0);
    }

    fn issuer() -> ImplicitIssuer {
        ImplicitIssuer::standalone(b"node-tests", 600_000_000, &[PSID_SAFETY]).expect("issuer")
    }

    fn key(iss: &ImplicitIssuer, node: u8, i: u32, j: u32) -> ImplicitKey {
        iss.issue_factory(
            FactoryProfile::Pseudonym {
                lv: LinkageValue::new([node; 9]),
            },
            &[node; 32],
            i,
            j,
            600_000_100,
            &[PSID_SAFETY],
        )
        .expect("issues")
    }

    /// A node installs once per credential and reuses the handle; the handle names the
    /// SCMS certificate by its own `HashedId8`, and a new pseudonym is a new certificate.
    #[test]
    fn installation_is_idempotent_per_credential() {
        let iss = issuer();
        let mut s = stack(CryptoMode::Real);
        let k30 = key(&iss, 1, 3, 0);
        let first = s
            .install(NodeId::new(1), 3, 0, &k30)
            .expect("installs")
            .digest()
            .clone();
        let coer = cert::encode(k30.certificate()).expect("enc");
        assert_eq!(first, hashedid::hashed_id8(&coer));
        let again = s
            .install(NodeId::new(1), 3, 0, &k30)
            .expect("installs")
            .digest()
            .clone();
        assert_eq!(first, again);
        let rotated = s
            .install(NodeId::new(1), 3, 1, &key(&iss, 1, 3, 1))
            .expect("installs")
            .digest()
            .clone();
        assert_ne!(
            first, rotated,
            "a new pseudonym must be a new certificate, or rotation buys nothing"
        );
    }

    /// A key that does not reconstruct is refused at installation, in both modes.
    #[test]
    fn a_key_that_does_not_reconstruct_is_refused() {
        let iss = issuer();
        let k = key(&iss, 2, 0, 0);
        let bad = ImplicitKey::new(
            Arc::clone(k.certificate()),
            Arc::clone(k.issuer()),
            *key(&iss, 2, 0, 1).private(),
        );
        for mode in [CryptoMode::Real, CryptoMode::Modeled] {
            let mut s = stack(mode);
            assert!(s.install(NodeId::new(2), 0, 0, &bad).is_err(), "{mode:?}");
            assert!(s.install(NodeId::new(2), 0, 0, &k).is_ok(), "{mode:?}");
        }
    }

    /// Sender and receiver are two stacks: the receiver verifies an SPDU signed under an
    /// implicit certificate once it holds the issuing ACA certificate, answers
    /// `Unverifiable` while it does not, and `Invalid` for a tampered signature, in both
    /// modes.
    #[test]
    fn a_peer_verifies_under_the_reconstructed_key() {
        let reg = RngRegistry::new(4);
        let iss = issuer();
        let k = key(&iss, 3, 0, 0);
        for mode in [CryptoMode::Real, CryptoMode::Modeled] {
            let mut ctx = NodeRuntimeCtx::new(0, &reg);
            let mut tx = stack(mode);
            tx.install(NodeId::new(3), 0, 0, &k).expect("installs");
            assert!(tx.set_active(0, 0));
            let (frame, _) = tx
                .sign(
                    &mut ctx,
                    MsgType::Bsm,
                    b"bsm",
                    SignerIdChoice::Certificate,
                    None,
                )
                .expect("signs");
            let mut rx = stack(mode);
            let parsed = rx.parse(&frame.spdu).expect("parses");
            let c = NodeSecurity::attached_certificate(&parsed).expect("attached");
            assert_eq!(
                rx.verify_parsed(&mut ctx, &parsed, &c, NodeId::new(9)),
                SpduVerdict::Unverifiable,
                "{mode:?}: no ACA certificate, nothing concluded"
            );
            rx.learn_issuer(Arc::clone(iss.certificate()));
            assert_eq!(
                rx.verify_parsed(&mut ctx, &parsed, &c, NodeId::new(9)),
                SpduVerdict::Valid,
                "{mode:?}"
            );
            let mut tampered = frame.spdu.clone();
            let last = tampered.len() - 1;
            tampered[last] ^= 1;
            let parsed = rx.parse(&tampered).expect("parses");
            assert_eq!(
                rx.verify_parsed(&mut ctx, &parsed, &c, NodeId::new(9)),
                SpduVerdict::Invalid,
                "{mode:?}"
            );
            assert_eq!(
                v2xw_sec::credential::linkage_of(&c),
                Some((0, LinkageValue::new([3; 9])))
            );
        }
    }

    /// Both backends produce certificates and SPDUs of identical size — half of I-S1, and
    /// the half a size model depends on. The certificate is the implicit one the SCMS
    /// issues, and the size the node reports is its encoding.
    #[test]
    fn the_two_backends_agree_on_every_size() {
        let reg = RngRegistry::new(3);
        let iss = issuer();
        let k = key(&iss, 4, 0, 0);
        let payload = b"a payload of some length";
        let sizes = |mode| {
            let mut ctx = NodeRuntimeCtx::new(0, &reg);
            let mut s = stack(mode);
            s.install(NodeId::new(1), 0, 0, &k).expect("installs");
            assert!(s.set_active(0, 0));
            let cert_len = s.signer().expect("signer").cert_coer().len();
            assert_eq!(cert_len, cert::encode(k.certificate()).expect("enc").len());
            let (frame, pdu) = s
                .sign(
                    &mut ctx,
                    MsgType::Cam,
                    payload,
                    SignerIdChoice::Certificate,
                    None,
                )
                .expect("signs");
            assert_eq!(frame.cert_bytes as usize, cert_len);
            (cert_len, frame.bytes_on_wire(), pdu.overhead())
        };
        assert_eq!(sizes(CryptoMode::Real), sizes(CryptoMode::Modeled));
    }
}

type LastParse = Option<(EnvelopeProfile, Vec<u8>, Option<ParsedSecured>)>;

thread_local! {
    /// The last SPDU [`NodeSecurity::parse`] decoded on this thread (see there).
    static LAST_PARSE: std::cell::RefCell<LastParse> = const { std::cell::RefCell::new(None) };
}
