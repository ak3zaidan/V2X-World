//! The PKI view's publication and inspection (WP-E1, 00-gap-analysis §3.3).
//!
//! Five record shapes, each keyed by stable ids, and three inspect answers built from
//! nothing but those records, so a live run, a recording and a fixture answer alike:
//!
//! * [`PkiState`] (`pki.state`): every provider, its components with their roles, region
//!   and state (online, offline, faulted), the CTL with its history, and every CRL head.
//!   Certificates are carried once, the first time a snapshot sees them
//!   ([`PkiState::certificates`]), so a reader accumulates a certificate store.
//! * [`PkiFlow`] (`pki.flow`): one record per wire message, with its flow instance, its
//!   sender and receiver and the real SPDUs it carried ([`PduRef`], hex included), and
//!   one per stage stamp, so every flow has per-stage timestamps.
//! * [`PkiCase`] (`pki.case`): each misbehaviour case's stage transitions.
//! * [`NodeCredentials`] (`node.credentials`): one vehicle's i × j pool.
//! * [`Knowledge`] (`pki.knowledge`): what one entity holds, with the ground-truth join
//!   kept apart ([`Knowledge::truth`]) so the per-entity view is the record minus it.
//!
//! Stable ids: a component is `provider/role/instance` (`provider-a/ra/0`); the SCMS
//! Manager and the electors are under `scms-manager`; a device is `ee/<node>`; a flow
//! instance is `provider/flow/run`; a PDU is `provider/pdu/<n>`; a certificate is its
//! HashedId8 in lower-case hex.
//!
//! # What "opened" means
//!
//! A PDU encrypted to an authority is opened by really decrypting it with that
//! authority's private key (ECIES for the RA and MA, the LA–ACA AES key for a sealed
//! PLV), at publication. The plaintext is the *recipient's* knowledge: an inspector shows
//! it in ground truth and as that recipient, and never as anyone else. A PDU encrypted to
//! a device's butterfly key (`AcaEeCertResponsePrivate`) is recorded as sealed to `ee`:
//! the backend holds no key that opens it, which is the point of the design.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use v2xw_core::hash::hex_encode;
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_msg::sec_types::ieee1609_dot2::{Certificate, Ieee1609Dot2Content, Ieee1609Dot2Data};

use crate::scms::pdu::{SpduKind, cert_id8};
use crate::scms::run::ScmsRun;
use crate::view::{EdgeTracker, EntityView, QueueView};

/// The schema version of every record here; bumped on any incompatible change.
pub const PKI_VIEW_SCHEMA: u32 = 1;

/// The provider label the SCMS Manager and the electors are filed under.
pub const MANAGER: &str = "scms-manager";

/// Lower-case hex.
fn hex(b: &[u8]) -> String {
    hex_encode(b)
}

/// Decodes lower- or upper-case hex; `None` for anything else.
#[must_use]
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

// ---------------------------------------------------------------------------------------
// pki.state
// ---------------------------------------------------------------------------------------

/// `pki.state`: the whole trust domain at an instant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PkiState {
    /// [`PKI_VIEW_SCHEMA`].
    pub schema: u32,
    /// The instant, ns.
    pub t: SimTime,
    /// `Time32` of the instant (TAI seconds since 2004-01-01, the home run's clock).
    pub time32: u32,
    /// The wire edition every PDU follows (`ieee-1609.2-2022`, `ieee-1609.2-2025`).
    pub edition: &'static str,
    /// The deployment preset.
    pub preset: &'static str,
    /// The SCMS Manager, the electors: governance shared by every provider.
    pub governance: Vec<ComponentState>,
    /// Every provider, the home provider first.
    pub providers: Vec<ProviderState>,
    /// The CTL every device installs (the home run's), with its history.
    pub ctl: CtlState,
    /// Every CRL head, by provider, series and kind.
    pub crls: Vec<CrlHead>,
    /// Certificates (HashedId8 hex → COER hex) not carried by an earlier snapshot of the
    /// same publisher: every authority certificate and every CTL root. The first snapshot
    /// carries them all.
    pub certificates: BTreeMap<String, String>,
    /// The deployment's labelled assumptions (regions, fleet shares, ...).
    pub assumptions: Vec<String>,
}

/// One provider.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderState {
    /// The neutral label, `provider-a`.
    pub label: String,
    /// Its index in the deployment.
    pub index: usize,
    /// Whether it is the home provider (the one whose governance the engine runs).
    pub home: bool,
    /// Where its online components run. Always an assumption (01 §2.3).
    pub region: RegionState,
    /// Whether its root is on the CTL.
    pub on_ctl: bool,
    /// Its MA's decision mode: `manual-review` or `automatic`.
    pub ma_mode: String,
    /// The operator's review delay in manual-review mode, s.
    pub ma_review_s: Option<f64>,
    /// Its CRL Generator's publication rule.
    pub crl_publication: &'static str,
    /// The share of the fleet it serves (assumption).
    pub fleet_share: f64,
    /// Every component, offline ones included.
    pub components: Vec<ComponentState>,
    /// Its devices.
    pub devices: DeviceTotals,
}

/// A region, labelled as the assumption it is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegionState {
    /// The neutral name.
    pub name: &'static str,
    /// The metro it stands for.
    pub metro: &'static str,
    /// Latitude, degrees north.
    pub lat_deg: f64,
    /// Longitude, degrees east.
    pub lon_deg: f64,
    /// Always true: no provider publishes where it runs.
    pub assumption: bool,
}

/// A provider's devices.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DeviceTotals {
    /// Devices the run knows.
    pub devices: u32,
    /// Enrolled.
    pub enrolled: u32,
    /// Holding at least one pseudonym credential.
    pub provisioned: u32,
    /// Silenced by a CRL entry naming them.
    pub revoked: u32,
    /// Blocklisted at the RA.
    pub blocklisted: u32,
}

/// One component.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComponentState {
    /// `provider/role/instance`.
    pub id: String,
    /// Its provider label (or [`MANAGER`]).
    pub provider: String,
    /// Its role: `root`, `ica`, `eca`, `ra`, `aca`, `la1`, `la2`, `ma`, `crlg`, `lop`,
    /// `crl-store`, `crl-broadcast`, `dcm`, `pg`, `manager`, `elector`.
    pub role: String,
    /// The instance number.
    pub instance: u32,
    /// Its name as the standard spells it.
    pub name: String,
    /// `online`, `offline` (by design: root, ICA, electors) or `faulted` (an injected
    /// outage now).
    pub status: &'static str,
    /// Its backend node id.
    pub node: Option<u32>,
    /// Its certificate's HashedId8, hex, when it holds one.
    pub cert: Option<String>,
    /// Its queue.
    pub queue: Option<QueueView>,
    /// Messages and bytes in and out.
    pub traffic: crate::kernel::NodeTraffic,
    /// Cryptographic operations by kind.
    pub ops: BTreeMap<String, u64>,
    /// Its own counts.
    pub counters: BTreeMap<String, Value>,
}

/// The CTL.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CtlState {
    /// Its sequence number.
    pub sequence: u32,
    /// The elector quorum it requires of its successor.
    pub quorum: u32,
    /// Electors listed.
    pub electors: u32,
    /// Roots listed, HashedId32 hex as the CTL carries them.
    pub roots: Vec<String>,
    /// Its effective `Time32`.
    pub effective: u32,
    /// The electors that endorsed it, by index.
    pub signers: Vec<u32>,
    /// Its bytes (`SignedCtl` SPDU) length.
    pub bytes: u32,
    /// Every CTL published so far, oldest first.
    pub history: Vec<CtlEntry>,
    /// CTLs decided but not yet effective.
    pub pending: u32,
}

/// One past CTL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CtlEntry {
    /// Sequence.
    pub sequence: u32,
    /// Effective `Time32`.
    pub effective: u32,
    /// Roots listed.
    pub roots: u32,
    /// Endorsing electors.
    pub signers: Vec<u32>,
}

/// One CRL head: the newest list of one provider, series and kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrlHead {
    /// The provider label.
    pub provider: String,
    /// `crlSeries` (1 pseudonym linkage, 2/3/4 hash, 256 ...).
    pub series: u16,
    /// `linkage` or `hash`.
    pub kind: String,
    /// A delta list.
    pub delta: bool,
    /// Lists of this series and kind issued so far (its version).
    pub version: u32,
    /// `issueDate`, `Time32`.
    pub issue_date: u32,
    /// `nextCrl`, `Time32`.
    pub next_crl: u32,
    /// Entries.
    pub entries: u32,
    /// Its encoded size.
    pub bytes: u32,
    /// When it was issued, ns.
    pub issued_at: SimTime,
    /// Seconds to `nextCrl` (negative: overdue): the SLA clock.
    pub sla_s: i64,
    /// Its HashedId8, hex.
    pub id8: String,
    /// The `SecuredCrl` hex, carried once (the snapshot that first sees this version).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
}

/// What a publisher has already carried, so each snapshot carries only what is new.
#[derive(Debug, Clone, Default)]
pub struct PkiTracker {
    /// Per run: the cursor into its wire log, its stage log and its PDU log.
    cursors: BTreeMap<usize, (usize, usize, usize)>,
    /// Per run: the Backend view's fold (`view::EdgeTracker`), for component counters.
    edges: BTreeMap<usize, EdgeTracker>,
    /// Certificates already carried.
    certs: BTreeSet<String>,
    /// CRL versions already carried, by id8.
    crls: BTreeSet<String>,
    /// Case stages already published: `(provider, case run, stage)`.
    case_stages: BTreeSet<(usize, u32, &'static str)>,
    /// Per device: the digest of its last `node.credentials`, and every pseudonym it has
    /// used (the history).
    creds: BTreeMap<u32, (u64, BTreeSet<[u8; 8]>)>,
}

impl PkiTracker {
    /// Records that `device` signed with `id8`, for the pool's history.
    pub fn note_active(&mut self, device: NodeId, id8: [u8; 8]) {
        self.creds.entry(device.index()).or_default().1.insert(id8);
    }
}

/// The label of the provider a run serves.
#[must_use]
pub fn provider_label(run: &ScmsRun) -> String {
    let st = &run.state;
    st.deployment
        .providers
        .get(st.provider)
        .map_or_else(|| format!("provider-{}", st.provider), |p| p.name.clone())
}

/// Every node id of one run's components, with its stable id and role.
#[must_use]
pub fn component_ids(run: &ScmsRun) -> BTreeMap<u32, (String, String, u32)> {
    let st = &run.state;
    let label = provider_label(run);
    let mut out = BTreeMap::new();
    let mut put = |node: NodeId, provider: &str, role: &str, inst: u32| {
        out.entry(node.index())
            .or_insert_with(|| (format!("{provider}/{role}/{inst}"), role.to_string(), inst));
    };
    let n = st.nodes;
    for (node, role) in [
        (n.eca, "eca"),
        (n.ra, "ra"),
        (n.pca, "aca"),
        (n.la1, "la1"),
        (n.la2, "la2"),
        (n.ma, "ma"),
        (n.crlg, "crlg"),
        (n.lop, "lop"),
        (n.crl_store, "crl-store"),
        (n.crl_broadcast, "crl-broadcast"),
        (n.dcm, "dcm"),
    ] {
        put(node, &label, role, 0);
    }
    let g = &st.gov.nodes;
    put(g.root, &label, "root", 0);
    put(g.ica, &label, "ica", 0);
    put(g.pg, &label, "pg", 0);
    put(g.manager, MANAGER, "manager", 0);
    for (k, e) in g.electors.iter().enumerate() {
        put(*e, MANAGER, "elector", u32::try_from(k).unwrap_or(u32::MAX));
    }
    if st.provider < st.deployment.providers.len() {
        for (role, inst, node) in st.deployment.nodes(st.provider).instances {
            let role = if role == "crl_store" {
                "crl-store".to_string()
            } else {
                role
            };
            put(node, &label, &role, inst);
        }
    }
    out
}

/// The stable id of `node` in `run`: a component's, or `ee/<node>`.
#[must_use]
pub fn node_id(ids: &BTreeMap<u32, (String, String, u32)>, node: NodeId) -> String {
    ids.get(&node.index())
        .map_or_else(|| format!("ee/{}", node.index()), |(id, ..)| id.clone())
}

/// The role a Backend-view entity id stands for, or `None` for one the PKI view does not
/// list as a component (the pooled devices, committees).
fn role_of_entity(id: &str) -> Option<&'static str> {
    Some(match id {
        "eca" => "eca",
        "ra" => "ra",
        "pca" => "aca",
        "la1" => "la1",
        "la2" => "la2",
        "ma" => "ma",
        "crlg" => "crlg",
        "lop" => "lop",
        "crl-store" => "crl-store",
        "crl-broadcast" => "crl-broadcast",
        "dcm" => "dcm",
        "root" => "root",
        "ica" => "ica",
        "pg" => "pg",
        "manager" => "manager",
        "electors" => "elector",
        _ => return None,
    })
}

fn status_of(
    run: &ScmsRun,
    role: &str,
    online: bool,
    node: Option<u32>,
    now: SimTime,
) -> &'static str {
    if crate::scms::deployment::is_offline(role) || role == "elector" || !online {
        "offline"
    } else if node.is_some_and(|n| run.state.faults.offline(NodeId::new(n), now)) {
        "faulted"
    } else {
        "online"
    }
}

fn component_from(
    run: &ScmsRun,
    e: &EntityView,
    id: String,
    provider: &str,
    role: &str,
    instance: u32,
    cert: Option<[u8; 8]>,
    now: SimTime,
) -> ComponentState {
    ComponentState {
        id,
        provider: provider.to_string(),
        role: role.to_string(),
        instance,
        name: e.name.clone(),
        status: status_of(run, role, e.online, e.node, now),
        node: e.node,
        cert: cert.map(|c| hex(&c)),
        queue: e.queue,
        traffic: e.traffic,
        ops: e.ops.clone(),
        counters: e.state.clone(),
    }
}

/// The certificate a run's PDU PKI holds for `role`.
fn pki_cert<'a>(run: &'a ScmsRun, role: &str, instance: u32) -> Option<&'a Certificate> {
    let pki = run.state.pki.as_deref()?;
    if instance > 0 {
        let name = format!("{role}-{instance}");
        return pki
            .extra
            .iter()
            .find(|a| a.name.contains(&name))
            .map(|a| &a.cert);
    }
    Some(match role {
        "root" => &pki.root.cert,
        "ica" => &pki.ica.cert,
        "eca" => &pki.eca.cert,
        "ra" => &pki.ra.cert,
        "aca" => &pki.aca.cert,
        "ma" => &pki.ma.cert,
        "la1" => &pki.la1.cert,
        "la2" => &pki.la2.cert,
        "dcm" => &pki.dcm.cert,
        "crlg" => &pki.crlg.cert,
        _ => return None,
    })
}

fn coer_hex(c: &Certificate) -> Option<String> {
    rasn::coer::encode(c).ok().map(|b| hex(&b))
}

fn device_totals(run: &ScmsRun) -> DeviceTotals {
    let st = &run.state;
    let mut d = DeviceTotals::default();
    for (node, dev) in &st.devices {
        d.devices += 1;
        d.enrolled += u32::from(dev.enrolled);
        d.provisioned += u32::from(!dev.credentials.is_empty());
        d.revoked += u32::from(dev.silenced);
        d.blocklisted += u32::from(st.ra.blocklist.contains(node));
    }
    d
}

/// `pki.state` over every provider's run at `now`. `runs[0]` is the home provider.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn pki_state(runs: &[ScmsRun], now: SimTime, tracker: &mut PkiTracker) -> Option<PkiState> {
    let home = runs.first()?;
    let time32 = home.state.time32(now);
    let mut certificates = BTreeMap::new();
    let mut carry = |c: &Certificate, tracker: &mut PkiTracker| {
        let id = hex(&cert_id8(c));
        if tracker.certs.insert(id.clone())
            && let Some(h) = coer_hex(c)
        {
            certificates.insert(id, h);
        }
    };
    let mut governance = Vec::new();
    let mut providers = Vec::new();
    let mut crls = Vec::new();
    for (k, run) in runs.iter().enumerate() {
        let label = provider_label(run);
        let edges = tracker.edges.entry(k).or_default();
        let view = run.backend_view(now, edges);
        let ids = component_ids(run);
        let mut components = Vec::new();
        for e in &view.entities {
            let Some(role) = role_of_entity(&e.id) else {
                continue;
            };
            match role {
                "manager" | "elector" if k == 0 => {
                    if role == "manager" {
                        governance.push(component_from(
                            run,
                            e,
                            format!("{MANAGER}/manager/0"),
                            MANAGER,
                            "manager",
                            0,
                            None,
                            now,
                        ));
                    } else {
                        for (i, node) in run.state.gov.nodes.electors.iter().enumerate() {
                            let mut c = component_from(
                                run,
                                e,
                                format!("{MANAGER}/elector/{i}"),
                                MANAGER,
                                "elector",
                                u32::try_from(i).unwrap_or(u32::MAX),
                                None,
                                now,
                            );
                            c.node = Some(node.index());
                            c.cert = run.state.gov.ctl.electors.get(i).map(|h| hex(&h[..8]));
                            governance.push(c);
                        }
                    }
                }
                "manager" | "elector" => {}
                _ => {
                    let cert = pki_cert(run, role, 0);
                    if let Some(c) = cert {
                        carry(c, tracker);
                    }
                    components.push(component_from(
                        run,
                        e,
                        format!("{label}/{role}/0"),
                        &label,
                        role,
                        0,
                        cert.map(cert_id8),
                        now,
                    ));
                }
            }
        }
        // Instances beyond the first (a second ACA): the kernel's numbers, no counters
        // of their own because the run routes every request to the primary.
        for (_, (id, role, inst)) in ids.iter().filter(|(_, (_, _, i))| *i > 0) {
            if id.starts_with(MANAGER) {
                continue;
            }
            let node = ids
                .iter()
                .find(|(_, (other, ..))| other == id)
                .map(|(n, _)| NodeId::new(*n));
            let e = crate::view::entity(
                &run.kernel,
                now,
                "scms",
                role,
                &format!("{role} instance {inst}"),
                "ca",
                "an extra instance of the role",
                node,
                true,
            );
            let cert = pki_cert(run, role, *inst);
            if let Some(c) = cert {
                carry(c, tracker);
            }
            components.push(component_from(
                run,
                &e,
                id.clone(),
                &label,
                role,
                *inst,
                cert.map(cert_id8),
                now,
            ));
        }
        // CRL heads: the newest of each (series, delta).
        let mut heads: BTreeMap<(u16, bool), (u32, &crate::scms::run::IssuedCrl)> = BTreeMap::new();
        for c in &run.state.crlg.issued {
            let e = heads.entry((c.series, c.delta)).or_insert((0, c));
            e.0 += 1;
            e.1 = c;
        }
        for ((series, delta), (version, c)) in heads {
            let id8 = hex(&c.pdu.hashed_id8());
            let fresh = tracker.crls.insert(id8.clone());
            crls.push(CrlHead {
                provider: label.clone(),
                series,
                kind: if series == 1 { "linkage" } else { "hash" }.to_string(),
                delta,
                version,
                issue_date: c.issue_date,
                next_crl: c.next_crl,
                entries: c.entries,
                bytes: c.pdu.len(),
                issued_at: c.at,
                sla_s: i64::from(c.next_crl) - i64::from(time32),
                id8,
                hex: fresh.then(|| hex(c.pdu.bytes())),
            });
        }
        let p = run.state.deployment.providers.get(run.state.provider);
        let mode = &run.state.params.ma.mode;
        providers.push(ProviderState {
            label,
            index: run.state.provider,
            home: k == 0,
            region: p.map_or(
                RegionState {
                    name: "unknown",
                    metro: "unknown",
                    lat_deg: 0.0,
                    lon_deg: 0.0,
                    assumption: true,
                },
                |p| RegionState {
                    name: p.region.name,
                    metro: p.region.metro,
                    lat_deg: p.region.lat_deg,
                    lon_deg: p.region.lon_deg,
                    assumption: true,
                },
            ),
            on_ctl: p.is_none_or(|p| p.on_ctl),
            ma_mode: match mode {
                crate::scms::params::MaMode::ManualReview { .. } => "manual-review",
                crate::scms::params::MaMode::Automatic => "automatic",
            }
            .to_string(),
            ma_review_s: match mode {
                crate::scms::params::MaMode::ManualReview { review } => Some(review.as_secs_f64()),
                crate::scms::params::MaMode::Automatic => None,
            },
            crl_publication: p.map_or("cadence", |p| p.crl_publication.id()),
            fleet_share: p.map_or(1.0, |p| p.fleet_share),
            components,
            devices: device_totals(run),
        });
    }
    let g = &home.state.gov;
    let ctl = CtlState {
        sequence: g.ctl.sequence,
        quorum: g.ctl.quorum,
        electors: u32::try_from(g.ctl.electors.len()).unwrap_or(u32::MAX),
        roots: g.ctl.roots.iter().map(|r| hex(r)).collect(),
        effective: g.ctl.effective,
        signers: g.ctl.signers.clone(),
        bytes: u32::try_from(g.ctl.spdu.len()).unwrap_or(u32::MAX),
        history: g
            .ctl_history
            .iter()
            .map(|c| CtlEntry {
                sequence: c.sequence,
                effective: c.effective,
                roots: u32::try_from(c.roots.len()).unwrap_or(u32::MAX),
                signers: c.signers.clone(),
            })
            .collect(),
        pending: u32::try_from(g.pending.len()).unwrap_or(u32::MAX),
    };
    Some(PkiState {
        schema: PKI_VIEW_SCHEMA,
        t: now,
        time32,
        edition: match v2xw_msg::scms_types::WireEdition::default() {
            v2xw_msg::scms_types::WireEdition::Ieee2022 => "ieee-1609.2-2022",
            v2xw_msg::scms_types::WireEdition::Ieee2025 => "ieee-1609.2-2025",
        },
        preset: home.state.deployment.preset.id(),
        governance,
        providers,
        ctl,
        crls,
        certificates,
        assumptions: home.state.deployment.assumptions(),
    })
}

// ---------------------------------------------------------------------------------------
// pki.flow
// ---------------------------------------------------------------------------------------

/// `pki.flow`: one wire message, one stage stamp, or one PDU no message carried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PkiFlow {
    /// [`PKI_VIEW_SCHEMA`].
    pub schema: u32,
    /// `msg`, `stage` or `pdu`.
    pub kind: String,
    /// `provider/m/<n>` for a message, `provider/s/<n>` for a stamp, the PDU's id for a
    /// lone PDU.
    pub id: String,
    /// When it left (a message), was stamped (a stage) or was built (a PDU), ns.
    pub t: SimTime,
    /// The provider whose run logged it.
    pub provider: String,
    /// The flow (`enrolment`, `provisioning`, `report`, `revocation`, ...).
    pub flow: String,
    /// The flow instance, `provider/flow/run`.
    pub instance: String,
    /// The step (a message) or stage (a stamp) name.
    pub name: String,
    /// Sender (a message) or the stamping node (a stage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Receiver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Bytes on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u32>,
    /// Transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    /// The real SPDUs the message carried.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pdus: Vec<PduRef>,
}

/// One SPDU.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PduRef {
    /// `provider/pdu/<n>`, its index in the run's PDU log.
    pub id: String,
    /// The 1609.2.1 type name.
    #[serde(rename = "type")]
    pub kind: String,
    /// Encoded length.
    pub len: u32,
    /// Sender, stable id.
    pub from: String,
    /// Receiver, stable id.
    pub to: String,
    /// When it was built, ns.
    pub t: SimTime,
    /// HashedId8 of the bytes, hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id8: Option<String>,
    /// The bytes, hex (absent when the run keeps lengths only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    /// Who the outer layer is encrypted to (`provider/ra/0`, `ee`), if it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_to: Option<String>,
    /// The plaintext, as the recipient opened it with its own key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened: Option<Opened>,
}

/// A PDU's plaintext as its recipient decrypted it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Opened {
    /// The component that opened it.
    pub by: String,
    /// The plaintext, hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    /// Why it did not open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Who a PDU kind's outer layer is encrypted to, by role (`ee` for a device key).
#[must_use]
pub const fn sealed_role(kind: SpduKind) -> Option<&'static str> {
    match kind {
        SpduKind::EeRaCertRequest
        | SpduKind::EeRaDownloadRequest
        | SpduKind::EeRaSuccessorEnrollmentCertRequest => Some("ra"),
        SpduKind::SecuredMisbehaviorReport => Some("ma"),
        SpduKind::EncryptedIndividualPlv => Some("aca"),
        SpduKind::AcaEeCertResponsePrivate => Some("ee"),
        _ => None,
    }
}

/// Opens a PDU the way its recipient does, with the recipient's real key.
fn open_pdu(
    run: &ScmsRun,
    p: &crate::scms::pdu::EmittedPdu,
    bytes: &[u8],
) -> Option<(String, Option<Opened>)> {
    let role = sealed_role(p.kind)?;
    let label = provider_label(run);
    if role == "ee" {
        return Some(("ee".to_string(), None));
    }
    let by = format!("{label}/{role}/0");
    let Some(pki) = run.state.pki.as_deref() else {
        return Some((by, None));
    };
    let result = match role {
        "ra" | "ma" => {
            let a = if role == "ra" { &pki.ra } else { &pki.ma };
            match a.enc_private.as_ref() {
                Some(k) => {
                    crate::scms::pdu::decrypt_cert(bytes, &a.cert, k).map_err(|e| e.to_string())
                }
                None => Err(format!("the {role} certificate carries no encryption key")),
            }
        }
        _ => {
            // A sealed PLV: the ACA's own routine (`pdu_prov::aca_open_plv`), AES-CCM
            // under the LA–ACA key the symmRecipInfo names.
            use v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ra_interface::EncryptedIndividualPLV;
            let st = &run.state;
            match (st.la_aca.as_ref(), exact::<EncryptedIndividualPLV>(bytes)) {
                (Some(keys), Some(e)) => {
                    crate::scms::pdu_prov::aca_open_plv(&e, keys, st.time32(p.at))
                        .map(|(_, plv)| plv.as_bytes().to_vec())
                        .map_err(|e| e.to_string())
                }
                (None, _) => Err("no LA–ACA key in this run".to_string()),
                (_, None) => Err("not an EncryptedIndividualPLV".to_string()),
            }
        }
    };
    Some((
        by.clone(),
        Some(match result {
            Ok(plain) => Opened {
                by,
                hex: Some(hex(&plain)),
                error: None,
            },
            Err(e) => Opened {
                by,
                hex: None,
                error: Some(e),
            },
        }),
    ))
}

fn pdu_ref(
    run: &ScmsRun,
    ids: &BTreeMap<u32, (String, String, u32)>,
    label: &str,
    n: usize,
    p: &crate::scms::pdu::EmittedPdu,
) -> PduRef {
    let bytes = p.bytes.as_ref();
    let opened = bytes.and_then(|b| open_pdu(run, p, b.bytes()));
    PduRef {
        id: format!("{label}/pdu/{n}"),
        kind: p.kind.name().to_string(),
        len: p.len,
        from: node_id(ids, NodeId::new(p.from)),
        to: node_id(ids, NodeId::new(p.to)),
        t: p.at,
        id8: bytes.map(|b| hex(&b.hashed_id8())),
        hex: bytes.map(|b| hex(b.bytes())),
        sealed_to: opened
            .as_ref()
            .map(|(by, _)| by.clone())
            .or_else(|| sealed_role(p.kind).map(str::to_string)),
        opened: opened.and_then(|(_, o)| o),
    }
}

/// Every message, stage stamp and PDU each run logged since the last call.
#[must_use]
pub fn drain_flows(runs: &[ScmsRun], tracker: &mut PkiTracker) -> Vec<PkiFlow> {
    let mut out = Vec::new();
    for (k, run) in runs.iter().enumerate() {
        let label = provider_label(run);
        let ids = component_ids(run);
        let (sc, gc, pc) = tracker.cursors.get(&k).copied().unwrap_or_default();
        let (steps, s_end) = run.kernel.steps_since(sc);
        let (stamps, g_end) = run.kernel.stages_since(gc);
        let pdus = &run.state.pdus[pc.min(run.state.pdus.len())..];
        let mut by_step: BTreeMap<u32, Vec<PduRef>> = BTreeMap::new();
        let lone = |r: PduRef, flow: &str| PkiFlow {
            schema: PKI_VIEW_SCHEMA,
            kind: "pdu".to_string(),
            id: r.id.clone(),
            t: r.t,
            provider: label.clone(),
            flow: flow.to_string(),
            instance: format!("{label}/{flow}/0"),
            name: r.kind.clone(),
            from: Some(r.from.clone()),
            to: Some(r.to.clone()),
            bytes: Some(r.len),
            transport: None,
            pdus: vec![r],
        };
        for (off, p) in pdus.iter().enumerate() {
            let r = pdu_ref(run, &ids, &label, pc + off, p);
            match p.step {
                Some(s) if (sc..s_end).contains(&(s as usize)) => {
                    by_step.entry(s).or_default().push(r);
                }
                // A pre-run flow (enrolment and the first batches before the drive).
                _ if p.preload => out.push(lone(r, "preload")),
                _ => out.push(lone(r, "unattributed")),
            }
        }
        for (off, s) in steps.iter().enumerate() {
            let n = sc + off;
            let flow = s.flow.as_str();
            out.push(PkiFlow {
                schema: PKI_VIEW_SCHEMA,
                kind: "msg".to_string(),
                id: format!("{label}/m/{n}"),
                t: s.t,
                provider: label.clone(),
                flow: flow.to_string(),
                instance: format!("{label}/{flow}/{}", s.run.0),
                name: s.step.to_string(),
                from: Some(node_id(&ids, s.from)),
                to: Some(node_id(&ids, s.to)),
                bytes: Some(s.bytes),
                transport: Some(s.transport.as_str().to_string()),
                pdus: u32::try_from(n)
                    .ok()
                    .and_then(|n| by_step.remove(&n))
                    .unwrap_or_default(),
            });
        }
        // Every PDU is published: one whose step was not in this drain stays visible.
        for r in std::mem::take(&mut by_step).into_values().flatten() {
            out.push(lone(r, "unattributed"));
        }
        for (off, st) in stamps.iter().enumerate() {
            let flow = st.flow.as_str();
            out.push(PkiFlow {
                schema: PKI_VIEW_SCHEMA,
                kind: "stage".to_string(),
                id: format!("{label}/s/{}", gc + off),
                t: st.t,
                provider: label.clone(),
                flow: flow.to_string(),
                instance: format!("{label}/{flow}/{}", st.run.0),
                name: st.stage.as_str().to_string(),
                from: st.node.map(|n| node_id(&ids, n)),
                to: None,
                bytes: st.size,
                transport: None,
                pdus: Vec::new(),
            });
        }
        tracker
            .cursors
            .insert(k, (s_end, g_end, run.state.pdus.len()));
    }
    out
}

// ---------------------------------------------------------------------------------------
// pki.case
// ---------------------------------------------------------------------------------------

/// The case board's columns, in order (00 §3.2 Cases tab).
pub const CASE_STAGES: [&str; 7] = [
    "reports-in",
    "corroborated",
    "decision",
    "plv-lookup",
    "la-seeds",
    "crl-published",
    "enforced",
];

/// `pki.case`: one misbehaviour case entering one stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PkiCase {
    /// [`PKI_VIEW_SCHEMA`].
    pub schema: u32,
    /// `provider/case/<run>`.
    pub case: String,
    /// The MA's provider.
    pub provider: String,
    /// One of [`CASE_STAGES`], or `refused` for a case that ended without an entry.
    pub stage: String,
    /// When it entered the stage, ns (the stamp's time where the kernel stamped one).
    pub t: SimTime,
    /// The resolution's flow instance.
    pub instance: String,
    /// What the stage established.
    pub detail: Value,
}

fn stage_at(run: &ScmsRun, r: crate::stage::FlowRun, s: crate::stage::StageId) -> Option<SimTime> {
    run.kernel.stages.at(r, s)
}

/// Every case stage each run reached since the last call. `enforced` comes from the
/// caller (`enforced(provider, case_run)` → `(installed, of)`), because enforcement is a
/// vehicle-side fact the backend run does not hold.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn drain_cases(
    runs: &[ScmsRun],
    tracker: &mut PkiTracker,
    now: SimTime,
    enforced: &dyn Fn(usize, &crate::scms::run::Case) -> Option<(u32, u32)>,
) -> Vec<PkiCase> {
    use crate::stage::StageId as S;
    let mut out = Vec::new();
    for (k, run) in runs.iter().enumerate() {
        let label = provider_label(run);
        let ma = &run.state.ma;
        for (r, c) in &ma.cases {
            let instance = format!("{label}/linkage-resolution/{}", r.0);
            let case = format!("{label}/case/{}", r.0);
            let mut push = |tracker: &mut PkiTracker,
                            stage: &'static str,
                            t: Option<SimTime>,
                            detail: Value| {
                if let Some(t) = t
                    && tracker.case_stages.insert((k, r.0, stage))
                {
                    out.push(PkiCase {
                        schema: PKI_VIEW_SCHEMA,
                        case: case.clone(),
                        provider: label.clone(),
                        stage: stage.to_string(),
                        t,
                        instance: instance.clone(),
                        detail,
                    });
                }
            };
            let subjects: Vec<Value> = c
                .subjects
                .iter()
                .take(if c.direct { 1 } else { 2 })
                .map(|(i, lv)| json!({"i": i, "lv": hex(lv.as_bytes())}))
                .collect();
            let received = stage_at(run, *r, S::ReportReceived);
            let decision = stage_at(run, *r, S::Decision);
            push(
                tracker,
                "reports-in",
                received.or(decision),
                json!({"subjects": subjects, "direct": c.direct}),
            );
            push(
                tracker,
                "corroborated",
                decision,
                json!({"reports": if c.direct { 1 } else { 2 }}),
            );
            push(
                tracker,
                "decision",
                decision,
                json!({"mode": match run.state.params.ma.mode {
                    crate::scms::params::MaMode::ManualReview { .. } => "manual-review",
                    crate::scms::params::MaMode::Automatic => "automatic",
                }}),
            );
            if !c.lookups.is_empty() {
                push(
                    tracker,
                    "plv-lookup",
                    stage_at(run, *r, S::Resolved).or(Some(now)),
                    json!({"lookups": c.lookups.len(),
                           "handles": c.lookups.iter().map(|l| hex(&l.handle[..8])).collect::<Vec<_>>()}),
                );
            }
            if c.seeds.iter().all(Option::is_some) {
                push(
                    tracker,
                    "la-seeds",
                    stage_at(run, *r, S::Resolved).or(Some(now)),
                    json!({"la_ids": c.seeds.iter().flatten().map(|(id, _)| id.0).collect::<Vec<_>>(),
                           "i_rev": c.i_rev, "jmax": c.jmax, "same_device": c.same}),
                );
            }
            let published = stage_at(run, c.crl_run, S::Published)
                .or_else(|| stage_at(run, c.crl_run, S::Issued));
            if c.resolved {
                push(
                    tracker,
                    "crl-published",
                    published,
                    json!({"crl_instance": format!("{label}/crl-issuance/{}", c.crl_run.0)}),
                );
            } else if c.done {
                push(
                    tracker,
                    "refused",
                    Some(now),
                    json!({"same_device": c.same}),
                );
            }
            if c.resolved
                && published.is_some()
                && let Some((n, of)) = enforced(k, c)
                && n > 0
            {
                push(
                    tracker,
                    "enforced",
                    Some(now),
                    json!({"installed": n, "of": of}),
                );
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// node.credentials
// ---------------------------------------------------------------------------------------

/// One cell of the i × j grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredCell {
    /// i-period.
    pub i: u32,
    /// j.
    pub j: u32,
    /// `active`, `valid`, `downloaded` (a future period), `expired`, `revoked`,
    /// `not-yet-available` (in the pool horizon, not held).
    pub state: String,
    /// The certificate's HashedId8, hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id8: Option<String>,
    /// Whether the device has signed with it (its pseudonym history).
    #[serde(default)]
    pub used: bool,
}

/// `node.credentials`: one vehicle's pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeCredentials {
    /// [`PKI_VIEW_SCHEMA`].
    pub schema: u32,
    /// The instant, ns.
    pub t: SimTime,
    /// The vehicle's node id.
    pub node: u32,
    /// Its backend device id, `ee/<n>`: what `pki.flow` names it as sender or receiver.
    pub device: String,
    /// Its provider.
    pub provider: String,
    /// Enrolled.
    pub enrolled: bool,
    /// The enrolment certificate's HashedId8, hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrolment: Option<String>,
    /// The current i-period.
    pub current_i: u32,
    /// Certificates per period.
    pub jmax: u32,
    /// The active certificate, HashedId8 hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    /// The grid, `(i, j)` ascending.
    pub cells: Vec<CredCell>,
    /// Pseudonyms used so far, in HashedId8 order.
    pub history: Vec<String>,
    /// The CTL sequence it holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctl_sequence: Option<u32>,
    /// Silenced by a CRL naming it.
    pub revoked: bool,
    /// Certificates not carried by this device's earlier records (HashedId8 → COER hex).
    #[serde(default)]
    pub certificates: BTreeMap<String, String>,
}

/// One held certificate, as the node's store has it.
#[derive(Debug, Clone)]
pub struct HeldCert {
    /// i.
    pub i: u32,
    /// j.
    pub j: u32,
    /// HashedId8.
    pub id8: [u8; 8],
    /// COER bytes.
    pub coer: Vec<u8>,
    /// `active`, `valid`, `downloaded`, `expired`, `revoked`.
    pub state: &'static str,
}

/// The pool of `device` (in `run`) from what its node holds, or `None` when the record
/// would repeat the last one. `active` is the certificate it signs with now; `revoked`
/// whether the vehicle knows itself revoked (it found itself on a CRL).
#[must_use]
pub fn node_credentials(
    run: &ScmsRun,
    device: NodeId,
    now: SimTime,
    held: &[HeldCert],
    active: Option<[u8; 8]>,
    revoked: bool,
    tracker: &mut PkiTracker,
) -> Option<NodeCredentials> {
    let st = &run.state;
    let dev = st.devices.get(&device);
    let current_i = st.params.period_at(now);
    let jmax = st.params.certs_per_period;
    let entry = tracker.creds.entry(device.index()).or_default();
    if let Some(a) = active {
        entry.1.insert(a);
    }
    let mut cells: BTreeMap<(u32, u32), CredCell> = BTreeMap::new();
    for h in held {
        cells.insert(
            (h.i, h.j),
            CredCell {
                i: h.i,
                j: h.j,
                state: if Some(h.id8) == active {
                    "active"
                } else {
                    h.state
                }
                .to_string(),
                id8: Some(hex(&h.id8)),
                used: entry.1.contains(&h.id8),
            },
        );
    }
    let horizon = st.params.max_periods_ahead.max(1);
    for i in current_i..current_i.saturating_add(horizon) {
        for j in 0..jmax {
            cells.entry((i, j)).or_insert(CredCell {
                i,
                j,
                state: "not-yet-available".to_string(),
                id8: None,
                used: false,
            });
        }
    }
    let cells: Vec<CredCell> = cells.into_values().collect();
    let cells_revoked = cells.iter().any(|c| c.state == "revoked");
    let enrolment = dev
        .and_then(|d| d.enrolment_credential.as_ref())
        .map(|e| hex(&cert_id8(&e.cert)));
    let digest = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for c in &cells {
            (c.i, c.j, &c.state, &c.id8, c.used).hash(&mut h);
        }
        (current_i, &enrolment, dev.map(|d| (d.enrolled, d.silenced))).hash(&mut h);
        dev.and_then(|d| d.trust.ctl_sequence).hash(&mut h);
        revoked.hash(&mut h);
        h.finish()
    };
    if entry.0 == digest && digest != 0 {
        return None;
    }
    entry.0 = digest;
    let history = entry.1.iter().map(|d| hex(d)).collect();
    let mut certificates = BTreeMap::new();
    for h in held {
        let id = hex(&h.id8);
        if tracker.certs.insert(format!("{}#{id}", device.index())) {
            certificates.insert(id, hex(&h.coer));
        }
    }
    if let Some(e) = dev.and_then(|d| d.enrolment_credential.as_ref()) {
        let id = hex(&cert_id8(&e.cert));
        if tracker.certs.insert(format!("{}#{id}", device.index()))
            && let Some(h) = coer_hex(&e.cert)
        {
            certificates.insert(id, h);
        }
    }
    Some(NodeCredentials {
        schema: PKI_VIEW_SCHEMA,
        t: now,
        node: device.index(),
        device: format!("ee/{}", device.index()),
        provider: provider_label(run),
        enrolled: dev.is_some_and(|d| d.enrolled),
        enrolment,
        current_i,
        jmax,
        active: active.map(|a| hex(&a)),
        cells,
        history,
        ctl_sequence: dev.and_then(|d| d.trust.ctl_sequence),
        revoked: revoked || dev.is_some_and(|d| d.silenced) || cells_revoked,
        certificates,
    })
}

// ---------------------------------------------------------------------------------------
// pki.knowledge
// ---------------------------------------------------------------------------------------

/// `pki.knowledge`: what one entity holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Knowledge {
    /// [`PKI_VIEW_SCHEMA`].
    pub schema: u32,
    /// The instant, ns.
    pub t: SimTime,
    /// The entity, `provider/role/0`.
    pub entity: String,
    /// Its role.
    pub role: String,
    /// What it holds, in its own identifiers.
    pub holds: Value,
    /// What it handles but cannot read, and why.
    pub cannot_see: Vec<String>,
    /// The ground-truth join of what it holds onto devices: shown only in ground truth.
    pub truth: Value,
}

/// The roles [`knowledge`] answers for.
pub const KNOWLEDGE_ROLES: [&str; 6] = ["ra", "aca", "la1", "la2", "ma", "eca"];

/// What `role` of `run` holds at `now`, rows capped at `limit` each (totals uncapped).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn knowledge(run: &ScmsRun, role: &str, now: SimTime, limit: usize) -> Option<Knowledge> {
    let st = &run.state;
    let label = provider_label(run);
    let (holds, cannot_see, truth) = match role {
        "ra" => {
            let rows: Vec<Value> = st
                .ra
                .registered
                .values()
                .flat_map(|s| s.iter())
                .take(limit)
                .map(|e| json!({"enrolment": hex(e)}))
                .collect();
            let requests: Vec<Value> = st
                .ra
                .requests
                .keys()
                .take(limit)
                .map(|h| json!({"request_hash": hex(&h[..8])}))
                .collect();
            let truth_rows: Vec<Value> = st
                .ra
                .registered
                .iter()
                .flat_map(|(n, s)| s.iter().map(move |e| (n, e)))
                .take(limit)
                .map(|(n, e)| json!({"enrolment": hex(e), "device": format!("ee/{}", n.index())}))
                .collect();
            let req_truth: Vec<Value> = st
                .ra
                .requests
                .iter()
                .take(limit)
                .map(|(h, r)| json!({"request_hash": hex(&h[..8]), "device": format!("ee/{}", r.device.index())}))
                .collect();
            (
                json!({
                    "enrolments": st.ra.registered.values().map(BTreeSet::len).sum::<usize>(),
                    "enrolment_rows": rows,
                    "requests": st.ra.requests.len(),
                    "request_rows": requests,
                    "blocklisted": st.ra.blocklist.len(),
                    "per_certificate_handles": st.ra.handles.len(),
                    "knows": ["the enrolment certificate (the device's identity) of every request",
                              "the i-periods and j range it asked the ACA for",
                              "the caterpillar and cocoon public keys"],
                }),
                vec![
                    "the pseudonym certificates: each ACA response is encrypted to the device's J_ij".to_string(),
                    "the linkage values: each PLV is sealed for the ACA under the LA-ACA key".to_string(),
                ],
                json!({"enrolments": truth_rows, "requests": req_truth}),
            )
        }
        "aca" => {
            let rows: Vec<Value> = st
                .pca
                .issued
                .iter()
                .take(limit)
                .map(|((i, lv), l)| json!({"i": i, "lv": hex(lv), "handle": hex(&l.handle[..8])}))
                .collect();
            let truth_rows: Vec<Value> = st
                .pca
                .issued
                .iter()
                .take(limit)
                .map(|((i, lv), l)| {
                    let device = st
                        .ra
                        .handles
                        .get(&l.handle)
                        .and_then(|req| st.ra.requests.get(req))
                        .map(|r| format!("ee/{}", r.device.index()));
                    json!({"i": i, "lv": hex(lv), "device": device})
                })
                .collect();
            (
                json!({
                    "certificates_issued": st.pca.issued_count,
                    "issued_rows": rows,
                    "lookups_answered": st.pca.lookups,
                    "knows": ["the linkage value of every certificate it issues",
                              "the RA's per-certificate handle (not the request)"],
                }),
                vec![
                    "which device a certificate is for: the RA strips the enrolment identity"
                        .to_string(),
                    "whether two certificates belong to one device: handles are per certificate"
                        .to_string(),
                ],
                json!({"issued": truth_rows}),
            )
        }
        "la1" | "la2" => {
            let la = &st.la[usize::from(role == "la2")];
            let rows: Vec<Value> = la
                .chains
                .keys()
                .take(limit)
                .map(|lci| json!({"lci": format!("{:016x}", lci.0)}))
                .collect();
            let truth_rows: Vec<Value> = la
                .owner
                .iter()
                .take(limit)
                .map(|(lci, n)| json!({"lci": format!("{:016x}", lci.0), "device": format!("ee/{}", n.index())}))
                .collect();
            (
                json!({
                    "la_id": la.la_id.0,
                    "chains": la.chains.len(),
                    "chain_rows": rows,
                    "plvs_issued": la.plv_issued,
                    "seeds_released": la.seeds_released,
                    "knows": ["a linkage chain (seed) per LCI it was asked for"],
                }),
                vec![
                    "which device a chain belongs to: the RA asks by chain id".to_string(),
                    "the other LA's seeds, so it cannot compute a linkage value alone".to_string(),
                ],
                json!({"chains": truth_rows}),
            )
        }
        "ma" => {
            let rows: Vec<Value> = st
                .ma
                .reports
                .iter()
                .rev()
                .take(limit)
                .map(|r| {
                    json!({"subject_i": r.subject_i, "subject_lv": hex(r.subject_lv.as_bytes()),
                               "observed_at": r.observed_at, "secured": r.secured.is_some()})
                })
                .collect();
            let truth_rows: Vec<Value> = st
                .ma
                .reports
                .iter()
                .rev()
                .take(limit)
                .map(|r| {
                    json!({"subject_lv": hex(r.subject_lv.as_bytes()),
                               "reporter": format!("ee/{}", r.reporter.index())})
                })
                .collect();
            (
                json!({
                    "reports": st.ma.reports.len(),
                    "report_rows": rows,
                    "cases": st.ma.cases.len(),
                    "open_cases": st.ma.open_cases(),
                    "decisions": st.ma.decisions.len(),
                    "awaiting_review": st.ma.awaiting_review.len(),
                    "opened_reports": st.ma.opened_reports.len(),
                    "knows": ["each report's evidence and the accused pseudonym's linkage value",
                              "the reporter's pseudonym certificate"],
                }),
                vec![
                    "the identity of a reporter or of the accused before the PCA and LAs answer"
                        .to_string(),
                ],
                json!({"reports": truth_rows}),
            )
        }
        "eca" => {
            let rows: Vec<Value> = st
                .eca
                .names
                .keys()
                .take(limit)
                .map(|n| json!({"canonical_id": n}))
                .collect();
            let truth_rows: Vec<Value> = st
                .eca
                .names
                .iter()
                .take(limit)
                .map(|(n, d)| json!({"canonical_id": n, "device": format!("ee/{}", d.index())}))
                .collect();
            (
                json!({
                    "enrolment_certificates_issued": st.eca.issued,
                    "successors": st.eca.successors,
                    "refused": st.eca.refused,
                    "name_rows": rows,
                    "knows": ["each device's canonical identifier and enrolment key"],
                }),
                vec!["anything after enrolment: the RA, not the ECA, provisions".to_string()],
                json!({"names": truth_rows}),
            )
        }
        _ => return None,
    };
    Some(Knowledge {
        schema: PKI_VIEW_SCHEMA,
        t: now,
        entity: format!("{label}/{role}/0"),
        role: role.to_string(),
        holds,
        cannot_see,
        truth,
    })
}

// ---------------------------------------------------------------------------------------
// Inspection: from records alone
// ---------------------------------------------------------------------------------------

/// The ASN.1 value tree of `bytes` (COER) as the real decoder reads it: the outer
/// `Ieee1609Dot2Data` in JER (ITU-T X.697) and every layer below it that is not
/// encrypted: a signed payload's `Ieee1609Dot2Data`, an unsecured `ScmsPdu`, a
/// `SignedCertificateRequest`, a `CrlContents`.
#[must_use]
pub fn decode_tree(bytes: &[u8]) -> Value {
    let mut layers = Vec::new();
    decode_data(bytes, "$", &mut layers, 0);
    json!({ "layers": layers })
}

fn jer<T: rasn::Encode>(v: &T) -> Value {
    rasn::jer::encode(v)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

fn exact<T: rasn::Decode>(bytes: &[u8]) -> Option<T> {
    match rasn::coer::decode_with_remainder::<T>(bytes) {
        Ok((v, rest)) if rest.is_empty() => Some(v),
        _ => None,
    }
}

fn decode_data(bytes: &[u8], path: &str, layers: &mut Vec<Value>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Some(d) = exact::<Ieee1609Dot2Data>(bytes) else {
        decode_payload(bytes, path, layers, depth);
        return;
    };
    let content = match &d.content {
        Ieee1609Dot2Content::unsecuredData(_) => "unsecuredData",
        Ieee1609Dot2Content::signedData(_) => "signedData",
        Ieee1609Dot2Content::encryptedData(_) => "encryptedData",
        Ieee1609Dot2Content::signedCertificateRequest(_) => "signedCertificateRequest",
        _ => "other",
    };
    layers.push(json!({"path": path, "type": "Ieee1609Dot2Data", "hex": hex(bytes), "content": content, "len": bytes.len(), "jer": jer(&d)}));
    match d.content {
        Ieee1609Dot2Content::unsecuredData(o) => {
            decode_payload(
                &o.0,
                &format!("{path}.content.unsecuredData"),
                layers,
                depth + 1,
            );
        }
        Ieee1609Dot2Content::signedData(sd) => {
            if let Some(inner) = sd.tbs_data.payload.data.as_ref()
                && let Ok(b) = rasn::coer::encode(inner)
            {
                decode_data(
                    &b,
                    &format!("{path}.content.signedData.tbsData.payload.data"),
                    layers,
                    depth + 1,
                );
            }
        }
        Ieee1609Dot2Content::signedCertificateRequest(o) => {
            decode_payload(
                &o.0,
                &format!("{path}.content.signedCertificateRequest"),
                layers,
                depth + 1,
            );
        }
        _ => {}
    }
}

fn decode_payload(bytes: &[u8], path: &str, layers: &mut Vec<Value>, depth: usize) {
    use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::CrlContents;
    use v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ra_interface::EncryptedIndividualPLV;
    use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::{ScmsPdu, SignedCertificateRequest};
    if let Some(p) = exact::<ScmsPdu>(bytes) {
        layers.push(json!({"path": path, "type": "ScmsPdu", "hex": hex(bytes), "len": bytes.len(), "jer": jer(&p)}));
    } else if let Some(r) = exact::<SignedCertificateRequest>(bytes) {
        layers.push(json!({"path": path, "type": "SignedCertificateRequest", "hex": hex(bytes), "len": bytes.len(), "jer": jer(&r)}));
    } else if let Some(e) = exact::<EncryptedIndividualPLV>(bytes) {
        layers.push(json!({"path": path, "type": "EncryptedIndividualPLV", "hex": hex(bytes), "len": bytes.len(), "jer": jer(&e)}));
    } else if let Some(c) = exact::<CrlContents>(bytes) {
        layers.push(json!({"path": path, "type": "CrlContents", "hex": hex(bytes), "len": bytes.len(), "jer": jer(&c)}));
    } else if depth > 0 && exact::<Ieee1609Dot2Data>(bytes).is_some() {
        decode_data(bytes, path, layers, depth);
    } else {
        layers.push(json!({"path": path, "type": "opaque", "hex": hex(bytes), "len": bytes.len()}));
    }
}

/// Which views may see a PDU's plaintext: ground truth, and the entity that opened it.
#[must_use]
pub fn may_see_plaintext(view: &str, opened_by: &str) -> bool {
    view == "ground-truth" || opened_by == view || opened_by.ends_with(&format!("/{view}/0"))
}

/// `inspect.pdu`'s answer for one recorded PDU, as `view` (`ground-truth`, or a stable
/// component id or role such as `provider-a/ra/0` or `ra`) may see it.
#[must_use]
pub fn inspect_pdu(p: &PduRef, view: &str) -> Value {
    let tree = p.hex.as_deref().and_then(unhex).map(|b| decode_tree(&b));
    let opened = p.opened.as_ref().map(|o| {
        if may_see_plaintext(view, &o.by) {
            json!({
                "by": o.by,
                "hex": o.hex,
                "error": o.error,
                "tree": o.hex.as_deref().and_then(unhex).map(|b| decode_tree(&b)),
            })
        } else {
            json!({"by": o.by, "hidden": format!("only {} (and ground truth) can read this plaintext", o.by)})
        }
    });
    json!({
        "id": p.id,
        "type": p.kind,
        "len": p.len,
        "from": p.from,
        "to": p.to,
        "t": p.t,
        "id8": p.id8,
        "hex": p.hex,
        "tree": tree,
        "sealed_to": p.sealed_to,
        "opened": opened,
        "view": view,
    })
}

/// One step of a certificate's validation trace.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceStep {
    /// The rule.
    pub rule: &'static str,
    /// `pass`, `fail` or `skipped`.
    pub status: &'static str,
    /// What failed, per certificate (0 = the one inspected).
    pub detail: Vec<String>,
}

/// The rules of `v2xw_sec::chain::validate`, in the order a trace lists them, with the
/// failure kinds each one covers.
const CHAIN_RULES: [(&str, &[v2xw_sec::chain::ChainErrorKind]); 9] = {
    use v2xw_sec::chain::ChainErrorKind as K;
    [
        ("well-formed", &[K::Empty, K::TooLong, K::Malformed]),
        (
            "chain-to-root",
            &[
                K::NotRootedInSelfSigned,
                K::IssuerMismatch,
                K::BadRootSignature,
            ],
        ),
        ("root-on-ctl", &[K::UntrustedRoot]),
        ("signatures", &[K::BadSignature, K::BadReconstruction]),
        (
            "issuer-permissions",
            &[
                K::PermissionNotCovered,
                K::EeTypeNotAllowed,
                K::SspNotCovered,
            ],
        ),
        ("chain-length", &[K::ChainLength]),
        ("validity", &[K::ValidityNotContained, K::Expired]),
        ("region", &[K::RegionNotContained, K::RegionUnsupported]),
        ("psid-ssp", &[]),
    ]
};

/// `inspect.cert`'s answer: the certificate `id8` decoded and validated as a US device
/// validates it at `time32`: its chain resolved through `store` (HashedId8 hex → COER
/// hex), the root checked against `roots` (HashedId32 hex, the CTL), every chain rule,
/// and every CRL in `crls` that names its CRACA and series.
#[must_use]
pub fn inspect_cert(
    id8: &str,
    store: &BTreeMap<String, String>,
    roots: &[String],
    crls: &[CrlHead],
    crl_hex: &BTreeMap<String, String>,
    time32: u32,
) -> Option<Value> {
    use v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier;
    let id8 = id8.to_ascii_lowercase();
    let decode = |h: &str| unhex(h).and_then(|b| exact::<Certificate>(&b));
    let cert = decode(store.get(&id8)?)?;
    let mut chain = vec![cert.clone()];
    let mut missing = None;
    while chain.len() < v2xw_sec::chain::MAX_CHAIN_LENGTH + 1 {
        let last = chain.last()?;
        let issuer = match &last.0.issuer {
            IssuerIdentifier::sha256AndDigest(h) | IssuerIdentifier::sha384AndDigest(h) => {
                hex(&v2xw_sec::hashedid::id8_bytes(h))
            }
            _ => break,
        };
        match store.get(&issuer).and_then(|h| decode(h)) {
            Some(c) => chain.push(c),
            None => {
                missing = Some(issuer);
                break;
            }
        }
    }
    let anchors: BTreeSet<[u8; 32]> = roots
        .iter()
        .filter_map(|r| unhex(r).and_then(|b| <[u8; 32]>::try_from(b).ok()))
        .collect();
    let policy = v2xw_sec::chain::ChainPolicy::us_device(anchors, time32);
    let (errors, tolerated) = match v2xw_sec::chain::validate(&chain, &policy) {
        Ok(v) => (Vec::new(), v.tolerated),
        Err(e) => (e, Vec::new()),
    };
    let mut trace: Vec<TraceStep> = CHAIN_RULES
        .iter()
        .filter(|(rule, _)| *rule != "psid-ssp")
        .map(|(rule, kinds)| {
            let detail: Vec<String> = errors
                .iter()
                .filter(|e| kinds.contains(&e.kind))
                .map(|e| format!("cert {}: {}", e.index, e.detail))
                .collect();
            TraceStep {
                rule,
                status: if detail.is_empty() { "pass" } else { "fail" },
                detail,
            }
        })
        .collect();
    if let Some(m) = &missing {
        trace[1] = TraceStep {
            rule: "chain-to-root",
            status: "fail",
            detail: vec![format!("issuer {m} is not in the published certificates")],
        };
    }
    // Revocation: every CRL head whose CRACA and series match.
    let mut crl_detail = Vec::new();
    let mut revoked = false;
    for head in crls {
        let Some(h) = crl_hex.get(&head.id8) else {
            continue;
        };
        let Some(b) = unhex(h) else { continue };
        let Ok(opened) = v2xw_sec::crl::decode_crl(&b) else {
            continue;
        };
        if v2xw_sec::crl::revokes(&opened.contents, &cert) {
            revoked = true;
            crl_detail.push(format!(
                "{} series {} v{} ({}) revokes it",
                head.provider, head.series, head.version, head.id8
            ));
        }
    }
    trace.push(TraceStep {
        rule: "crl",
        status: if revoked { "fail" } else { "pass" },
        detail: crl_detail,
    });
    let chain_ids: Vec<String> = chain.iter().map(|c| hex(&cert_id8(c))).collect();
    Some(json!({
        "id8": id8,
        "tree": jer(&cert),
        "chain": chain_ids,
        "time32": time32,
        "valid": errors.is_empty() && missing.is_none() && !revoked,
        "trace": trace,
        "tolerated": tolerated.iter().map(|e| format!("cert {}: {}", e.index, e.detail)).collect::<Vec<_>>(),
    }))
}

/// `inspect.knowledge`'s answer from one `pki.knowledge` record, as `view` sees it: the
/// ground-truth join only in ground truth.
#[must_use]
pub fn inspect_knowledge(k: &Knowledge, view: &str) -> Value {
    let mut v = json!({
        "entity": k.entity,
        "role": k.role,
        "t": k.t,
        "holds": k.holds,
        "cannot_see": k.cannot_see,
        "view": view,
    });
    if view == "ground-truth" {
        v["truth"] = k.truth.clone();
    }
    v
}
