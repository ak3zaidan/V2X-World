//! The credential hand-off: what a node signs with, and where it came from (WP-C1).
//!
//! Every credential a node holds is an implicit (ECQV) certificate under a shared ACA, with
//! the private key `d = e·b + r` its holder derived, and it reaches the node through
//! [`v2xw_node::ObuRuntime::install_credential`] (or the VRU device's). There are exactly
//! two sources, and the run report says which one each credential came from:
//!
//! * **The SCMS run** (`phase2`, the real 1609.2.1 path): the certificate the ACA issued in
//!   the device's batch, opened by the device (`ee_open_aca_response`), with `d` derived
//!   from the device's own caterpillar. Its linkage value is the LAs' `plv1 ⊕ plv2`, so a
//!   CRL entry the run publishes revokes exactly it.
//! * **Factory issuance** ([`CredentialFactory`]): the same ECQV issuance by the same ACA
//!   (the run's own ACA when an SCMS runs, otherwise one CA for the whole run), without the
//!   protocol's flows. Used where no protocol run issues anything: a scenario with no SCMS
//!   backend (the device left the factory with its pool), an RSU (whose application
//!   certificate cycle, WP-B3b, is not yet wired to the engine), the modelled explicit
//!   credential path under a hybrid or KEM scheme (1609.2.1 defines no SPDU for it; the
//!   protocol's linkage value is kept), and a vehicle the pre-run provisioning left without
//!   a batch.
//!
//! Neither source is per node: there is no per-node authority, and no linkage value derived
//! from a node id.

use std::sync::{Arc, OnceLock};

use v2xw_core::ids::NodeId;
use v2xw_core::time::{SimTime, WallClock};
use v2xw_sec::credential::{FactoryProfile, ImplicitIssuer, ImplicitKey};
use v2xw_sec::linkage::{DeviceLinkageContext, LaId, LinkageSeed, LinkageValue};

/// Issues implicit certificates for the run without the protocol's flows. See the module
/// documentation for where it is used and why.
#[derive(Debug)]
pub struct CredentialFactory {
    issuer: ImplicitIssuer,
    seed: [u8; 32],
    wall: WallClock,
    /// Whether the issuer is the SCMS run's own ACA.
    scms_aca: bool,
    /// The CTL-approved roots every node installs (WP-C2).
    roots: Vec<Arc<v2xw_msg::sec_types::Certificate>>,
    /// The local certificate chain file every node installs: the CA certificates.
    chain_file: Vec<Arc<v2xw_msg::sec_types::Certificate>>,
    /// Every provider's ACA (WP-D1), in deployment order, and the deployment and seed
    /// that assign a roadside unit to one of them. Empty: `issuer` certifies every node.
    providers: Vec<ImplicitIssuer>,
    assignment: Option<(v2xw_proto::scms::deployment::Deployment, u64)>,
    /// Under `security.scms.chain_file: home-provider`, each provider's own chain file in
    /// deployment order: a device installs its certifying provider's alone. Empty: every
    /// device installs `chain_file`.
    home_chain_files: Vec<Vec<Arc<v2xw_msg::sec_types::Certificate>>>,
    /// The receiver limits every device's trust store is configured with.
    limits: ReceiverLimits,
}

/// What a scenario sets of every receiver's trust store (SCMS settings audit,
/// docs/guides/config-inventory-scms.md): `None` keeps the node's own default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReceiverLimits {
    /// `nodes.validated_cert_cache`: validated end-entity certificates remembered
    /// (`v2xw_node::rxval::EE_CACHE` unless set).
    pub validated_cert_cache: Option<usize>,
    /// `security.scms.crl_overdue_tolerance_s`: `overdue_CRL_tolerance`
    /// (`v2xw_node::rxval::OVERDUE_CRL_TOLERANCE_S` unless set).
    pub crl_overdue_tolerance_s: Option<u32>,
}

impl ReceiverLimits {
    /// The limits `scenario` sets.
    #[must_use]
    pub fn of(scenario: &crate::Scenario) -> ReceiverLimits {
        ReceiverLimits {
            validated_cert_cache: scenario.nodes.validated_cert_cache.map(|n| n as usize),
            crl_overdue_tolerance_s: scenario
                .security
                .scms
                .as_ref()
                .and_then(|s| s.crl_overdue_tolerance_s),
        }
    }

    /// Applies the limits to `sec`'s trust store. The validated cache is replaced only
    /// when its capacity changes, so a re-installed device keeps what it validated.
    pub fn apply(self, sec: &mut v2xw_node::NodeSecurity) {
        let t = sec.trust_mut();
        if let Some(n) = self.validated_cert_cache
            && t.ee_cache().capacity() != n.max(1)
        {
            t.set_ee_capacity(n);
        }
        if let Some(s) = self.crl_overdue_tolerance_s {
            t.set_overdue_crl_tolerance(s);
        }
    }
}

impl CredentialFactory {
    /// The factory for one run: the SCMS run's ACA when one exists
    /// ([`crate::phase2::Phase2::aca_issuer`]), otherwise one P-256 CA for the run derived
    /// from the scenario seed, valid from the run's start.
    #[must_use]
    pub fn for_run(seed: u64, wall: WallClock, aca: Option<ImplicitIssuer>) -> CredentialFactory {
        let secret = v2xw_core::hash::sha256(
            &[
                b"v2xw-engine/credential-factory/v1".as_slice(),
                &seed.to_be_bytes(),
            ]
            .concat(),
        );
        let (issuer, scms_aca) = match aca {
            Some(i) => (i, true),
            None => (
                ImplicitIssuer::standalone(&secret, wall.time32(0).unwrap_or(0), &[0x20])
                    .expect("a standalone CA from a nonzero key always issues"),
                false,
            ),
        };
        // With no SCMS the standalone CA is its own self-signed root; an SCMS run's
        // roots and chain file are set by `with_trust`.
        let roots = if scms_aca {
            Vec::new()
        } else {
            vec![Arc::clone(issuer.certificate())]
        };
        CredentialFactory {
            issuer,
            seed: secret,
            wall,
            scms_aca,
            roots,
            chain_file: Vec::new(),
            providers: Vec::new(),
            home_chain_files: Vec::new(),
            limits: ReceiverLimits::default(),
            assignment: None,
        }
    }

    /// The same factory with one ACA per provider (WP-D1): a roadside unit's
    /// application certificate comes from the provider [`Deployment::provider_of`]
    /// assigns it under `seed`. Ignored unless `providers` has one ACA per provider.
    ///
    /// [`Deployment::provider_of`]: v2xw_proto::scms::deployment::Deployment::provider_of
    #[must_use]
    pub fn with_providers(
        mut self,
        providers: Vec<ImplicitIssuer>,
        deployment: v2xw_proto::scms::deployment::Deployment,
        seed: u64,
    ) -> CredentialFactory {
        if self.scms_aca && providers.len() == deployment.providers.len() && providers.len() > 1 {
            self.providers = providers;
            self.assignment = Some((deployment, seed));
        }
        self
    }

    /// Every provider's ACA certificate, in deployment order (just the run's issuer
    /// without a multi-provider deployment).
    #[must_use]
    pub fn provider_certificates(&self) -> Vec<Arc<v2xw_msg::sec_types::Certificate>> {
        if self.providers.is_empty() {
            return vec![Arc::clone(self.issuer.certificate())];
        }
        self.providers
            .iter()
            .map(|i| Arc::clone(i.certificate()))
            .collect()
    }

    /// The provider (index in the deployment) that certifies `node`'s roadside
    /// application certificates: 0, the home provider, without a multi-provider
    /// deployment.
    #[must_use]
    pub fn provider_of(&self, node: NodeId) -> usize {
        self.assignment
            .as_ref()
            .map_or(0, |(d, seed)| d.provider_of(*seed, u64::from(node.index())))
    }

    /// The same factory with the trust every node installs: the CTL-approved `roots` and
    /// the CA certificates of the local certificate chain file (WP-C2).
    #[must_use]
    pub fn with_trust(
        mut self,
        roots: Vec<Arc<v2xw_msg::sec_types::Certificate>>,
        chain_file: Vec<Arc<v2xw_msg::sec_types::Certificate>>,
    ) -> CredentialFactory {
        if !roots.is_empty() {
            self.roots = roots;
            self.chain_file = chain_file;
        }
        self
    }

    /// The same factory with every receiver's trust store configured with `limits`.
    #[must_use]
    pub fn with_receiver_limits(mut self, limits: ReceiverLimits) -> CredentialFactory {
        self.limits = limits;
        self
    }

    /// The same factory with one chain file per provider (`security.scms.chain_file:
    /// home-provider`, in deployment order): a device then installs only its certifying
    /// provider's, and learns the others' CA certificates by P2PCD.
    #[must_use]
    pub fn with_home_chain_files(
        mut self,
        files: Vec<Vec<Arc<v2xw_msg::sec_types::Certificate>>>,
    ) -> CredentialFactory {
        self.home_chain_files = files;
        self
    }

    /// Installs this run's trust on `node`'s security stack: every CTL root, and the
    /// chain file of the provider that certifies it under `home-provider`
    /// ([`CredentialFactory::provider_of`]: since WP-D1 one seeded draw assigns roadside
    /// units, vehicles and handsets alike), otherwise the whole chain file.
    pub fn install_trust_for(&self, node: NodeId, sec: &mut v2xw_node::NodeSecurity) {
        self.limits.apply(sec);
        if self.roots.is_empty() {
            return;
        }
        let provider = self.provider_of(node);
        let Some(file) = self.home_chain_files.get(provider) else {
            self.install_trust(sec);
            return;
        };
        sec.install_trust(&self.roots, file);
        let _ = sec.take_validation_ops();
    }

    /// Installs this run's trust on a node's security stack: the roots and the chain
    /// file. The chain file's validation is done at install, before the node is deployed,
    /// so its operations are not charged to the node's first received frame.
    pub fn install_trust(&self, sec: &mut v2xw_node::NodeSecurity) {
        self.limits.apply(sec);
        if self.roots.is_empty() {
            return;
        }
        sec.install_trust(&self.roots, &self.chain_file);
        let _ = sec.take_validation_ops();
    }

    /// The factory a node built outside an engine run gets (tools and tests that call the
    /// wiring directly): one CA for the process, from a fixed seed.
    #[must_use]
    pub fn fallback() -> Arc<CredentialFactory> {
        static F: OnceLock<Arc<CredentialFactory>> = OnceLock::new();
        Arc::clone(F.get_or_init(|| {
            Arc::new(CredentialFactory::for_run(
                0,
                WallClock::new(1_767_225_600),
                None,
            ))
        }))
    }

    /// Whether the issuer is the SCMS run's ACA (otherwise the run's standalone CA).
    #[must_use]
    pub fn is_scms_aca(&self) -> bool {
        self.scms_aca
    }

    /// The issuing CA's certificate: what a node's CA cache must hold to verify a
    /// credential from here.
    #[must_use]
    pub fn issuer_certificate(&self) -> &Arc<v2xw_msg::sec_types::Certificate> {
        self.issuer.certificate()
    }

    fn device_secret(&self, node: NodeId, kind: &[u8]) -> [u8; 32] {
        v2xw_core::hash::sha256(
            &[
                self.seed.as_slice(),
                b"/device/",
                kind,
                &node.index().to_be_bytes(),
            ]
            .concat(),
        )
    }

    /// The linkage value the two LAs would give `node`'s pseudonym `(i, j)` when no SCMS
    /// runs: CAMP SCP2 over initial seeds the LAs drew for this device from the run's own
    /// seed (not from the node id alone, so two runs with different seeds share none).
    #[must_use]
    pub fn factory_linkage(&self, node: NodeId, i: u32, j: u32) -> LinkageValue {
        let s1 = self.device_secret(node, b"la1");
        let s2 = self.device_secret(node, b"la2");
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];
        a.copy_from_slice(&s1[..16]);
        b.copy_from_slice(&s2[..16]);
        DeviceLinkageContext::new(LaId(1), LaId(2), LinkageSeed::new(a), LinkageSeed::new(b))
            .linkage_value_for(i, j)
    }

    /// A pseudonym certificate for `node`'s `(i, j)` carrying `lv`, valid from `from` (on
    /// the run's wall clock), permitting `psids`.
    ///
    /// # Errors
    /// `psids` is empty.
    pub fn pseudonym(
        &self,
        node: NodeId,
        i: u32,
        j: u32,
        lv: LinkageValue,
        from: SimTime,
        psids: &[u64],
    ) -> Result<ImplicitKey, v2xw_sec::SecError> {
        self.issuer.issue_factory(
            FactoryProfile::Pseudonym { lv },
            &self.device_secret(node, b"obu"),
            i,
            j,
            self.wall.time32(from).unwrap_or(0),
            psids,
        )
    }

    /// An RSU application certificate for `node`'s period `i` (no linkage data, CRL
    /// series 3, 169 hours), valid from `from`, permitting `psids`.
    ///
    /// # Errors
    /// `psids` is empty.
    pub fn rsu_application(
        &self,
        node: NodeId,
        i: u32,
        from: SimTime,
        psids: &[u64],
    ) -> Result<ImplicitKey, v2xw_sec::SecError> {
        let issuer = self
            .providers
            .get(self.provider_of(node))
            .unwrap_or(&self.issuer);
        issuer.issue_factory(
            FactoryProfile::RsuApplication,
            &self.device_secret(node, b"rsu"),
            i,
            0,
            self.wall.time32(from).unwrap_or(0),
            psids,
        )
    }
}
