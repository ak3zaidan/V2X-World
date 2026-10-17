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
        }
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

    /// Installs this run's trust on a node's security stack: the roots and the chain
    /// file. The chain file's validation is done at install, before the node is deployed,
    /// so its operations are not charged to the node's first received frame.
    pub fn install_trust(&self, sec: &mut v2xw_node::NodeSecurity) {
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
        self.issuer.issue_factory(
            FactoryProfile::RsuApplication,
            &self.device_secret(node, b"rsu"),
            i,
            0,
            self.wall.time32(from).unwrap_or(0),
            psids,
        )
    }
}
