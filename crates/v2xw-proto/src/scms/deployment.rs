//! The deployment topology: which SCMS providers run, what each runs, where, and which
//! devices each serves (WP-D1; 00-gap-analysis §2, gap rows 41–42).
//!
//! The US system is not one SCMS. Two production providers are on one CTL, each with its
//! own offline Root CA and ICA and its own online ECA, RA, ACA, LA1, LA2, MA, CRL
//! Generator and Policy Generator (01 §2.1–2.3, §9). The five electors are shared and the
//! SCMS Manager is a governance actor with no network node (01 §1.1, §3.1). Cross-provider
//! trust is the CTL plus the certificate chain file and nothing else (01 §9).
//!
//! A [`Deployment`] is built from a [`Preset`] and then adjusted by settings. It says:
//!
//! * the **providers**, with neutral labels (owner decision), the component count of every
//!   role, which roles are offline, the provider's MA mode and CRL publication rule, its
//!   cloud **region** and its **fleet share**;
//! * the **electors** and the quorum (5 and 3; anything else marks the run non-US);
//! * the **options** for what the US profile specifies but nobody deploys (LOP, CRL
//!   broadcast, MBMS CRL, group linkage) and the butterfly variant, of which only the
//!   original is accepted (owner decision: unified and compact unified are refused);
//! * the **latency model** between the scenario's city, the regions and each other.
//!
//! Nothing here publishes a data-centre location: no source does (01 §2.3, 03 §13). Every
//! region is a labelled ASSUMPTION ([`Region::assumed`]), and so is every fleet share.
//!
//! `camp-reference` is today's single-SCMS topology (one of each role, LOP, CRL Store and
//! broadcast, a central MA) on [`ScmsNodes::default`]'s ids, kept for comparison.

use std::collections::BTreeMap;

use v2xw_core::ids::NodeId;
use v2xw_core::time::Duration;

use crate::scms::params::{MaMode, ScmsNodes};

/// The SCMS roles a provider can host, in a fixed order.
///
/// `aca` is the 1609.2.1 name of CAMP's PCA. `pg` is the provider's Policy Generator, `crlg`
/// its CRL Generator (the CRL signer). `lop`, `crl_store` and `crl_broadcast` belong to the
/// CAMP reference design only (00 §2.3).
pub const ROLES: [&str; 13] = [
    "root",
    "ica",
    "eca",
    "ra",
    "aca",
    "la1",
    "la2",
    "ma",
    "crlg",
    "pg",
    "lop",
    "crl_store",
    "crl_broadcast",
];

/// The roles every US provider must run (01 §9: "each needs an offline RCA, an ICA, online
/// ECA, RA, ACA, LA1/LA2, MA and PG, and a CRL signer"; Provider Req. §2.2 for the MA).
pub const US_REQUIRED_ROLES: [&str; 10] = [
    "root", "ica", "eca", "ra", "aca", "la1", "la2", "ma", "crlg", "pg",
];

/// Whether a role is offline: the Root CA "shall not have network connectivity at any
/// time" (Provider Req. §3.5.1–3.5.2) and the ICA is modelled offline between
/// certifications (01 §2.2, UNCONFIRMED; 00 §2.1).
#[must_use]
pub fn is_offline(role: &str) -> bool {
    matches!(role, "root" | "ica")
}

/// Electors and quorum in the US system: 5, any 3 (Elector Tech. Spec. v1.1 §2.1–2.2;
/// the published CTLs' quorum; 01 §3.1).
pub const US_ELECTORS: u32 = 5;
/// See [`US_ELECTORS`].
pub const US_QUORUM: u32 = 3;

/// The node-id block of provider `k` for `k ≥ 1`: `PEER_NODE_BASE + k · PEER_NODE_STRIDE`.
/// Provider 0 keeps [`ScmsNodes::default`]'s 1–11, so a one-provider deployment reproduces
/// today's ids. The blocks are clear of the governance ids (20–59, roots from 60), the
/// committees (200–391), the library tests' devices (from 1,000) and the engine's devices
/// (from 1,000,000).
pub const PEER_NODE_BASE: u32 = 900_000;
/// See [`PEER_NODE_BASE`].
pub const PEER_NODE_STRIDE: u32 = 100;
/// Within a provider's block, the extra instances of a role (a second ACA, say) start here.
pub const EXTRA_INSTANCE_OFFSET: u32 = 40;

/// A deployment preset (00 §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// The US production system of 2025: two providers on one CTL (01 §2.1 [A]).
    UsProduction2025,
    /// The 17 July 2025 test CTL: six roots (01 §2.1 [A]), neutral labels a–f.
    UsTestCtl2025,
    /// The CAMP reference design: one SCMS with LOP, CRL Store and broadcast and a central
    /// MA, no providers. Today's topology.
    CampReference,
    /// Providers stated by the scenario.
    Custom,
}

impl Preset {
    /// Every preset id, in the order the settings list them.
    pub const IDS: [&'static str; 4] = [
        "us-production-2025",
        "us-test-ctl-2025",
        "camp-reference",
        "custom",
    ];

    /// The preset with id `s`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Preset> {
        Some(match s {
            "us-production-2025" => Preset::UsProduction2025,
            "us-test-ctl-2025" => Preset::UsTestCtl2025,
            "camp-reference" => Preset::CampReference,
            "custom" => Preset::Custom,
            _ => return None,
        })
    }

    /// The preset's id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Preset::UsProduction2025 => "us-production-2025",
            Preset::UsTestCtl2025 => "us-test-ctl-2025",
            Preset::CampReference => "camp-reference",
            Preset::Custom => "custom",
        }
    }
}

/// When a provider's CRL Generator publishes after a revocation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrlPublication {
    /// Within one business day of the decision: the SCMS Manager SLA (01 §1.4, 03 §9).
    SlaOneBusinessDay,
    /// On the run's fixed cadence (`crl_cadence_s`), the CAMP reference behaviour.
    Cadence,
    /// As soon as the decision is taken.
    OnDecision,
}

impl CrlPublication {
    /// Every id.
    pub const IDS: [&'static str; 3] = ["sla-1-business-day", "cadence", "on-decision"];

    /// Parses an id.
    #[must_use]
    pub fn parse(s: &str) -> Option<CrlPublication> {
        Some(match s {
            "sla-1-business-day" => CrlPublication::SlaOneBusinessDay,
            "cadence" => CrlPublication::Cadence,
            "on-decision" => CrlPublication::OnDecision,
            _ => return None,
        })
    }

    /// The id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            CrlPublication::SlaOneBusinessDay => "sla-1-business-day",
            CrlPublication::Cadence => "cadence",
            CrlPublication::OnDecision => "on-decision",
        }
    }
}

/// A cloud region: a name and an assumed point. No provider publishes where its online
/// components run (01 §2.3: "UNCONFIRMED for every provider"), so every coordinate here is
/// an ASSUMPTION — a representative metro of a common public-cloud region, not a claim
/// about any provider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    /// The neutral region name.
    pub name: &'static str,
    /// The assumed point's latitude, degrees north.
    pub lat_deg: f64,
    /// The assumed point's longitude, degrees east.
    pub lon_deg: f64,
    /// The metro the point stands for (for the manifest).
    pub metro: &'static str,
}

/// The region catalogue (all ASSUMPTIONS, see [`Region`]).
pub const REGIONS: [Region; 6] = [
    Region {
        name: "us-east",
        lat_deg: 39.0438,
        lon_deg: -77.4874,
        metro: "Ashburn, VA",
    },
    Region {
        name: "us-east-2",
        lat_deg: 39.9612,
        lon_deg: -82.9988,
        metro: "Columbus, OH",
    },
    Region {
        name: "us-central",
        lat_deg: 41.2619,
        lon_deg: -95.8608,
        metro: "Council Bluffs, IA",
    },
    Region {
        name: "us-south",
        lat_deg: 29.4241,
        lon_deg: -98.4936,
        metro: "San Antonio, TX",
    },
    Region {
        name: "us-west",
        lat_deg: 45.8399,
        lon_deg: -119.7006,
        metro: "Boardman, OR",
    },
    Region {
        name: "us-west-1",
        lat_deg: 37.3541,
        lon_deg: -121.9552,
        metro: "Santa Clara, CA",
    },
];

impl Region {
    /// The catalogue region named `name`.
    #[must_use]
    pub fn assumed(name: &str) -> Option<Region> {
        REGIONS.iter().copied().find(|r| r.name == name)
    }
}

/// Mean Earth radius, km (IUGG R1 = 6371.0088 km; Moritz, *Geodetic Reference System
/// 1980*, J. Geodesy 74 (2000)).
pub const EARTH_RADIUS_KM: f64 = 6_371.008_8;
/// The speed of light in vacuum, km/s (SI definition).
pub const C_KM_S: f64 = 299_792.458;

/// Great-circle distance between two points by the haversine formula, km.
#[must_use]
pub fn great_circle_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().min(1.0).asin()
}

/// How link latencies are derived.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LatencyModel {
    /// One-way latency = great-circle distance at `fibre_fraction` of c, plus
    /// `per_link_overhead` (routing, switching, TLS record processing). Both numbers are
    /// ASSUMPTIONS: 2/3 c is the usual figure for light in silica fibre (refractive index
    /// ≈ 1.47); the overhead is a settable placeholder.
    Region {
        /// Signal speed as a fraction of c.
        fibre_fraction: f64,
        /// Added to every link.
        per_link_overhead: Duration,
    },
    /// Today's fixed backend latency (`ScmsParams::backend_link_latency`) on every link.
    Flat,
}

impl LatencyModel {
    /// The default region model: 2/3 c and 1 ms per link (ASSUMPTION).
    pub const DEFAULT_REGION: LatencyModel = LatencyModel::Region {
        fibre_fraction: 2.0 / 3.0,
        per_link_overhead: Duration::from_millis(1),
    };

    /// The one-way latency over `km` of great-circle path; `None` under [`LatencyModel::Flat`].
    #[must_use]
    pub fn over_km(&self, km: f64) -> Option<Duration> {
        match *self {
            LatencyModel::Region {
                fibre_fraction,
                per_link_overhead,
            } => {
                let s = km.max(0.0) / (C_KM_S * fibre_fraction);
                Some(Duration::from_nanos((s * 1e9).round() as u64) + per_link_overhead)
            }
            LatencyModel::Flat => None,
        }
    }
}

/// One provider.
#[derive(Debug, Clone, PartialEq)]
pub struct Provider {
    /// The neutral label (owner decision), e.g. `provider-a`.
    pub name: String,
    /// How many instances of each role it runs (0 or absent: none).
    pub components: BTreeMap<String, u32>,
    /// Where its online components run (ASSUMPTION, [`Region`]).
    pub region: Region,
    /// Its MA's decision mode.
    pub ma_mode: MaMode,
    /// Its CRL Generator's publication rule.
    pub crl_publication: CrlPublication,
    /// The share of the fleet it serves (ASSUMPTION: no market share is published).
    pub fleet_share: f64,
    /// Whether the electors approved its root onto the CTL. Off models a provider whose
    /// root nobody trusts (a test-CTL root on the production system, a rogue SCMS): it
    /// still certifies, and nothing it certifies verifies. The home provider must be on.
    pub on_ctl: bool,
}

impl Provider {
    /// How many instances of `role` it runs.
    #[must_use]
    pub fn count(&self, role: &str) -> u32 {
        self.components.get(role).copied().unwrap_or(0)
    }

    fn standard(name: &str, aca: u32, ma_mode: MaMode, share: f64) -> Provider {
        let mut components = BTreeMap::new();
        for role in US_REQUIRED_ROLES {
            components.insert(role.to_string(), if role == "aca" { aca } else { 1 });
        }
        Provider {
            name: name.to_string(),
            components,
            region: Region::assumed("us-east").expect("in the catalogue"),
            ma_mode,
            crl_publication: CrlPublication::SlaOneBusinessDay,
            fleet_share: share,
            on_ctl: true,
        }
    }
}

/// The butterfly variant (owner decision: only the original, the US profile).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Butterfly {
    /// The original butterfly key expansion with the signing and encryption caterpillars.
    Original,
}

/// What the US profile specifies but nobody deploys (00 §2.3): each off by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Location Obscurer Proxy (not in 1609.2.1 or the profile; likely absent).
    pub lop: bool,
    /// CRL broadcast over RSUs or satellite (not used; CRLs come from the RA).
    pub crl_broadcast: bool,
    /// MBMS CRL distribution (not deployed).
    pub mbms_crl: bool,
    /// Group linkage (specified; no deployment evidence).
    pub group_linkage: bool,
    /// Identification certificates for emergency vehicles (on).
    pub identification_certs: bool,
    /// The butterfly variant.
    pub butterfly: Butterfly,
}

impl Options {
    /// The US deployment: nothing undeployed, identification certificates on.
    pub const US: Options = Options {
        lop: false,
        crl_broadcast: false,
        mbms_crl: false,
        group_linkage: false,
        identification_certs: true,
        butterfly: Butterfly::Original,
    };
}

/// Why a deployment was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeploymentError {
    /// An unknown preset, region, role, rule or option value.
    #[error("{field}: '{value}' is not one of {allowed}")]
    Unknown {
        /// The setting.
        field: String,
        /// What was given.
        value: String,
        /// What is accepted.
        allowed: String,
    },
    /// A value outside its range, or settings that contradict each other.
    #[error("{field}: {detail}")]
    Invalid {
        /// The setting.
        field: String,
        /// Why.
        detail: String,
    },
}

/// The node ids of one provider's components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderNodes {
    /// The primary instance of each online role, in the run's [`ScmsNodes`] shape (roles
    /// the provider does not run keep their id but host nothing).
    pub online: ScmsNodes,
    /// The offline Root CA.
    pub root: NodeId,
    /// The offline ICA.
    pub ica: NodeId,
    /// The Policy Generator.
    pub pg: NodeId,
    /// Every instance of every role the provider runs, `(role, instance, node)`.
    pub instances: Vec<(String, u32, NodeId)>,
}

/// A whole deployment.
#[derive(Debug, Clone, PartialEq)]
pub struct Deployment {
    /// What it was built from.
    pub preset: Preset,
    /// The providers; the first is the home provider of a single-backend run.
    pub providers: Vec<Provider>,
    /// Electors (shared by every provider).
    pub electors: u32,
    /// The elector quorum.
    pub quorum: u32,
    /// The undeployed options.
    pub options: Options,
    /// Device Configuration Managers (one per OEM or vendor, at the factory).
    pub dcms: u32,
    /// How latencies are derived.
    pub latency: LatencyModel,
    /// MAs forward a report about another provider's certificate to that provider's MA
    /// (Provider Req. §2.2 requires every provider to run an MA; only the issuing
    /// provider's LAs and ACA can resolve its pseudonyms).
    pub ma_report_sharing: bool,
}

impl Deployment {
    /// The deployment a preset describes.
    #[must_use]
    pub fn preset(p: Preset) -> Deployment {
        let manual = MaMode::ManualReview {
            review: Duration::from_secs(8 * 3_600),
        };
        let us = |providers: Vec<Provider>| Deployment {
            preset: p,
            providers,
            electors: US_ELECTORS,
            quorum: US_QUORUM,
            options: Options::US,
            dcms: 1,
            latency: LatencyModel::DEFAULT_REGION,
            ma_report_sharing: true,
        };
        match p {
            // 01 §2.1–2.2 [A]: provider-a has the ISS/OmniTrust component pattern (one
            // `pca`), provider-b the SAESOL S2X pattern (`aca` × 2).
            Preset::UsProduction2025 | Preset::Custom => us(vec![
                Provider::standard("provider-a", 1, manual, 0.5),
                Provider::standard("provider-b", 2, manual, 0.5),
            ]),
            Preset::UsTestCtl2025 => us(["a", "b", "c", "d", "e", "f"]
                .iter()
                .map(|s| Provider::standard(&format!("provider-{s}"), 1, manual, 1.0 / 6.0))
                .collect()),
            Preset::CampReference => {
                // Labelled like the home provider so today's certificates (named
                // `<role>.provider-a.sim`) are reproduced byte for byte.
                let mut camp = Provider::standard("provider-a", 1, manual, 1.0);
                for role in ["lop", "crl_store", "crl_broadcast"] {
                    camp.components.insert(role.to_string(), 1);
                }
                camp.crl_publication = CrlPublication::Cadence;
                Deployment {
                    preset: p,
                    providers: vec![camp],
                    electors: US_ELECTORS,
                    quorum: US_QUORUM,
                    options: Options {
                        lop: true,
                        crl_broadcast: true,
                        ..Options::US
                    },
                    dcms: 1,
                    latency: LatencyModel::Flat,
                    ma_report_sharing: false,
                }
            }
        }
    }

    /// Whether the run is the US system: 5 electors, quorum 3, the original butterfly,
    /// every provider running every required role. Anything else is flagged non-US.
    #[must_use]
    pub fn is_us(&self) -> bool {
        self.preset != Preset::CampReference
            && self.electors == US_ELECTORS
            && self.quorum == US_QUORUM
            && self
                .providers
                .iter()
                .all(|p| US_REQUIRED_ROLES.iter().all(|r| p.count(r) >= 1))
    }

    /// Checks the deployment.
    ///
    /// # Errors
    /// [`DeploymentError`] for no provider, a duplicate name, a provider without a Root CA,
    /// an ICA, an RA or an ACA, a quorum above the elector count or zero, fleet shares that
    /// are negative or do not sum to 1, or more than nine providers.
    pub fn validate(&self) -> Result<(), DeploymentError> {
        let invalid = |field: &str, detail: String| DeploymentError::Invalid {
            field: field.to_string(),
            detail,
        };
        if self.providers.is_empty() || self.providers.len() > 9 {
            return Err(invalid(
                "providers",
                format!(
                    "{} providers; a deployment has 1 to 9",
                    self.providers.len()
                ),
            ));
        }
        if self.quorum == 0 || self.quorum > self.electors || self.electors > 32 {
            return Err(invalid(
                "quorum",
                format!(
                    "{} of {} electors; it must be 1 to the elector count (≤ 32)",
                    self.quorum, self.electors
                ),
            ));
        }
        if !self.providers[0].on_ctl {
            return Err(invalid(
                &format!("providers.{}.on_ctl", self.providers[0].name),
                "the first (home) provider runs the run's flows and must be on the CTL".into(),
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        let mut share = 0.0;
        for p in &self.providers {
            if !names.insert(p.name.as_str()) {
                return Err(invalid("providers", format!("'{}' appears twice", p.name)));
            }
            for (role, n) in &p.components {
                if !ROLES.contains(&role.as_str()) {
                    return Err(DeploymentError::Unknown {
                        field: format!("providers.{}.components", p.name),
                        value: role.clone(),
                        allowed: ROLES.join(", "),
                    });
                }
                if *n > 8 {
                    return Err(invalid(
                        &format!("providers.{}.components.{role}", p.name),
                        format!("{n} instances; at most 8"),
                    ));
                }
            }
            for role in ["root", "ica", "ra", "aca"] {
                if p.count(role) == 0 {
                    return Err(invalid(
                        &format!("providers.{}.components", p.name),
                        format!(
                            "runs no {role}; every provider needs a root, an ICA, an RA and an ACA"
                        ),
                    ));
                }
            }
            if p.count("root") > 1 || p.count("ica") > 1 {
                return Err(invalid(
                    &format!("providers.{}.components", p.name),
                    "one root and one ICA per provider (01 §2.3: one root per provider)".into(),
                ));
            }
            if !(p.fleet_share.is_finite() && p.fleet_share >= 0.0) {
                return Err(invalid(
                    &format!("providers.{}.fleet_share", p.name),
                    format!("{} must be a share ≥ 0", p.fleet_share),
                ));
            }
            share += p.fleet_share;
        }
        if (share - 1.0).abs() > 1e-9 {
            return Err(invalid(
                "providers",
                format!("fleet shares sum to {share}; they must sum to 1"),
            ));
        }
        Ok(())
    }

    /// Parses a butterfly variant: only `original` (owner decision; 00 §2.3: the unified
    /// and compact unified variants are excluded by the profile and the CUBK response
    /// key is UNCONFIRMED).
    ///
    /// # Errors
    /// [`DeploymentError::Invalid`] for `unified` or `cubk`, [`DeploymentError::Unknown`]
    /// for anything else.
    pub fn butterfly(s: &str) -> Result<Butterfly, DeploymentError> {
        match s {
            "original" => Ok(Butterfly::Original),
            "unified" | "cubk" => Err(DeploymentError::Invalid {
                field: "options.butterfly".into(),
                detail: format!(
                    "'{s}' is refused: the US profile uses the original butterfly only, \
                     and the unified/compact-unified response key is UNCONFIRMED (02 §6)"
                ),
            }),
            other => Err(DeploymentError::Unknown {
                field: "options.butterfly".into(),
                value: other.into(),
                allowed: "original".into(),
            }),
        }
    }

    /// The node ids of provider `k`. Provider 0 is [`ScmsNodes::default`] with the
    /// governance root, ICA and PG ids (22, 23, 21); provider `k ≥ 1` lives in its own
    /// block ([`PEER_NODE_BASE`]).
    #[must_use]
    pub fn nodes(&self, k: usize) -> ProviderNodes {
        let (online, root, ica, pg) = if k == 0 {
            (
                ScmsNodes::default(),
                NodeId::new(22),
                NodeId::new(23),
                NodeId::new(21),
            )
        } else {
            let b = PEER_NODE_BASE + u32::try_from(k).unwrap_or(9) * PEER_NODE_STRIDE;
            let at = |o: u32| NodeId::new(b + o);
            (
                ScmsNodes {
                    ra: at(1),
                    pca: at(2),
                    la1: at(3),
                    la2: at(4),
                    ma: at(5),
                    crlg: at(6),
                    lop: at(7),
                    crl_store: at(8),
                    crl_broadcast: at(9),
                    eca: at(10),
                    dcm: at(11),
                },
                at(22),
                at(23),
                at(21),
            )
        };
        let p = &self.providers[k];
        let base_of = |role: &str| -> NodeId {
            match role {
                "root" => root,
                "ica" => ica,
                "pg" => pg,
                "eca" => online.eca,
                "ra" => online.ra,
                "aca" => online.pca,
                "la1" => online.la1,
                "la2" => online.la2,
                "ma" => online.ma,
                "crlg" => online.crlg,
                "lop" => online.lop,
                "crl_store" => online.crl_store,
                _ => online.crl_broadcast,
            }
        };
        let block = if k == 0 {
            0
        } else {
            PEER_NODE_BASE + u32::try_from(k).unwrap_or(9) * PEER_NODE_STRIDE
        };
        let mut instances = Vec::new();
        for (r, role) in ROLES.iter().enumerate() {
            for i in 0..p.count(role) {
                let node = if i == 0 {
                    base_of(role)
                } else {
                    // Extra instances: `EXTRA_INSTANCE_OFFSET + 4·role + i` in the block
                    // (provider 0's extras sit at 640+, clear of every reference id).
                    let o = EXTRA_INSTANCE_OFFSET + 4 * u32::try_from(r).unwrap_or(0) + i;
                    NodeId::new(if k == 0 { 600 + o } else { block + o })
                };
                instances.push(((*role).to_string(), i, node));
            }
        }
        ProviderNodes {
            online,
            root,
            ica,
            pg,
            instances,
        }
    }

    /// Which provider serves the device with stable key `key` (its node index), under the
    /// run's `seed`: a uniform draw from SHA-256(seed, key) against the cumulative fleet
    /// shares, so the split is the shares' in expectation, the same device always lands on
    /// the same provider, and two seeds give two independent splits.
    #[must_use]
    pub fn provider_of(&self, seed: u64, key: u64) -> usize {
        if self.providers.len() == 1 {
            return 0;
        }
        let h = v2xw_core::hash::sha256(
            &[
                b"v2xw-proto/scms/deployment/fleet/v1".as_slice(),
                &seed.to_be_bytes(),
                &key.to_be_bytes(),
            ]
            .concat(),
        );
        let mut b = [0u8; 8];
        b.copy_from_slice(&h[..8]);
        // 53 bits: exactly representable, uniform on [0, 1).
        let u = (u64::from_be_bytes(b) >> 11) as f64 / (1u64 << 53) as f64;
        let mut acc = 0.0;
        for (k, p) in self.providers.iter().enumerate() {
            acc += p.fleet_share;
            if u < acc {
                return k;
            }
        }
        self.providers.len() - 1
    }

    /// The one-way latency between two of a provider's own components: one link inside
    /// its region; `None` under [`LatencyModel::Flat`].
    #[must_use]
    pub fn intra_region_latency(&self) -> Option<Duration> {
        self.latency.over_km(0.0)
    }

    /// The one-way latency between providers `a` and `b` (the MA ↔ MA report-sharing
    /// link): the great-circle distance between their regions.
    #[must_use]
    pub fn inter_provider_latency(&self, a: usize, b: usize) -> Option<Duration> {
        let (ra, rb) = (self.providers[a].region, self.providers[b].region);
        self.latency.over_km(great_circle_km(
            ra.lat_deg, ra.lon_deg, rb.lat_deg, rb.lon_deg,
        ))
    }

    /// The internet leg from the scenario's city at `(lat, lon)` to provider `k`'s region,
    /// added after the device's access leg (cellular Uu or RSU backhaul).
    #[must_use]
    pub fn city_latency(&self, k: usize, lat_deg: f64, lon_deg: f64) -> Option<Duration> {
        let r = self.providers[k].region;
        self.latency
            .over_km(great_circle_km(lat_deg, lon_deg, r.lat_deg, r.lon_deg))
    }

    /// The MA a misbehaviour report travels through: the reporter's provider's MA, then,
    /// when the accused certificate is another provider's and sharing is on, that
    /// provider's MA over the inter-provider link. Returns the provider indices in order.
    #[must_use]
    pub fn report_route(&self, reporter: usize, accused: usize) -> Vec<usize> {
        if reporter == accused || !self.ma_report_sharing {
            vec![reporter]
        } else {
            vec![reporter, accused]
        }
    }

    /// Every labelled assumption the deployment rests on, for the run manifest.
    #[must_use]
    pub fn assumptions(&self) -> Vec<String> {
        let mut out = Vec::new();
        for p in &self.providers {
            out.push(format!(
                "ASSUMPTION: {} runs in region {} (point: {}); no provider publishes its \
                 data-centre locations (01 §2.3)",
                p.name, p.region.name, p.region.metro
            ));
        }
        if self.providers.len() > 1 {
            out.push(
                "ASSUMPTION: fleet shares are not published; the split is the setting's".into(),
            );
        }
        if let LatencyModel::Region {
            fibre_fraction,
            per_link_overhead,
        } = self.latency
        {
            out.push(format!(
                "ASSUMPTION: link latency = great-circle distance at {fibre_fraction:.3} c \
                 + {} ms per link",
                per_link_overhead.as_secs_f64() * 1e3
            ));
        }
        out.push(
            "ASSUMPTION: the ICA is offline between certifications (01 §2.2, UNCONFIRMED)".into(),
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_production_has_two_providers_each_with_every_required_role() {
        let d = Deployment::preset(Preset::UsProduction2025);
        d.validate().unwrap();
        assert!(d.is_us());
        assert_eq!(d.providers.len(), 2);
        assert_eq!(
            d.providers
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["provider-a", "provider-b"]
        );
        for p in &d.providers {
            for role in US_REQUIRED_ROLES {
                let want = if role == "aca" && p.name == "provider-b" {
                    2
                } else {
                    1
                };
                assert_eq!(p.count(role), want, "{} {role}", p.name);
            }
            for role in ["lop", "crl_store", "crl_broadcast"] {
                assert_eq!(p.count(role), 0, "{} runs no {role}", p.name);
            }
            assert!(matches!(p.ma_mode, MaMode::ManualReview { .. }));
        }
        assert_eq!((d.electors, d.quorum), (5, 3));
        assert!(!d.options.lop && !d.options.crl_broadcast && !d.options.mbms_crl);
        assert!(!d.options.group_linkage && d.options.identification_certs);
        assert!(d.ma_report_sharing);
        // 10 + 11 instances (provider-b's second ACA).
        let n: usize = (0..2).map(|k| d.nodes(k).instances.len()).sum();
        assert_eq!(n, 10 + 11);
    }

    #[test]
    fn the_test_ctl_preset_has_six_roots() {
        let d = Deployment::preset(Preset::UsTestCtl2025);
        d.validate().unwrap();
        assert_eq!(d.providers.len(), 6);
        assert_eq!(d.providers.iter().map(|p| p.count("root")).sum::<u32>(), 6);
    }

    #[test]
    fn camp_reference_is_todays_topology() {
        let d = Deployment::preset(Preset::CampReference);
        d.validate().unwrap();
        assert!(!d.is_us());
        assert_eq!(d.nodes(0).online, ScmsNodes::default());
        assert_eq!(d.latency, LatencyModel::Flat);
        let p = &d.providers[0];
        for role in ["lop", "crl_store", "crl_broadcast"] {
            assert_eq!(p.count(role), 1);
        }
        assert!(d.options.lop && d.options.crl_broadcast);
    }

    #[test]
    fn node_blocks_never_collide() {
        let d = Deployment::preset(Preset::UsTestCtl2025);
        let mut seen = std::collections::BTreeSet::new();
        for k in 0..d.providers.len() {
            let n = d.nodes(k);
            for (_, _, node) in &n.instances {
                assert!(seen.insert(node.index()), "node {} twice", node.index());
            }
        }
        let mut b = Deployment::preset(Preset::UsProduction2025);
        b.providers[0].components.insert("aca".into(), 3);
        let ids: Vec<u32> = b.nodes(0).instances.iter().map(|x| x.2.index()).collect();
        let set: std::collections::BTreeSet<u32> = ids.iter().copied().collect();
        assert_eq!(set.len(), ids.len());
        // Clear of the governance ids (20–59 and 60+ roots), committees (200–391).
        for id in ids.iter().filter(|i| **i > 11 && !(21..=23).contains(*i)) {
            assert!(*id >= 600, "{id}");
        }
    }

    #[test]
    fn unified_and_cubk_butterflies_are_refused() {
        assert_eq!(Deployment::butterfly("original"), Ok(Butterfly::Original));
        assert!(matches!(
            Deployment::butterfly("unified"),
            Err(DeploymentError::Invalid { .. })
        ));
        assert!(matches!(
            Deployment::butterfly("cubk"),
            Err(DeploymentError::Invalid { .. })
        ));
        assert!(matches!(
            Deployment::butterfly("x"),
            Err(DeploymentError::Unknown { .. })
        ));
    }

    #[test]
    fn validation_refuses_what_cannot_run() {
        let mut d = Deployment::preset(Preset::UsProduction2025);
        d.providers[1].fleet_share = 0.6;
        assert!(d.validate().is_err(), "shares sum to 1.1");
        let mut d = Deployment::preset(Preset::UsProduction2025);
        d.providers[1].components.remove("root");
        assert!(d.validate().is_err(), "a provider without a root");
        let mut d = Deployment::preset(Preset::UsProduction2025);
        d.quorum = 6;
        assert!(d.validate().is_err(), "quorum above the electors");
        let mut d = Deployment::preset(Preset::UsProduction2025);
        d.providers[1].name = "provider-a".into();
        assert!(d.validate().is_err(), "duplicate label");
        let mut d = Deployment::preset(Preset::UsProduction2025);
        d.providers[0].components.insert("dc".into(), 1);
        assert!(matches!(d.validate(), Err(DeploymentError::Unknown { .. })));
        let mut d = Deployment::preset(Preset::UsProduction2025);
        d.electors = 4;
        d.validate().unwrap();
        assert!(!d.is_us(), "four electors is not the US system");
    }

    /// Haversine against an independent computation (Python `math` with the same formula
    /// and R = 6371.0088 km, and GeographicLib's WGS-84 geodesic as a sanity bound): New
    /// York City Hall (40.7128, -74.0060) to the Ashburn point is 350.2351 km on the
    /// sphere (350.7212 km geodesic, 0.14 % away), and Ashburn to the Boardman point
    /// 3502.5798 km (3511.2445 km geodesic).
    #[test]
    fn latency_is_distance_at_fibre_speed_plus_overhead() {
        const NYC_ASHBURN_KM: f64 = 350.2351;
        const ASHBURN_BOARDMAN_KM: f64 = 3_502.5798;
        let km = great_circle_km(40.7128, -74.0060, 39.0438, -77.4874);
        assert!((km - NYC_ASHBURN_KM).abs() < 5e-4, "{km}");
        let km = great_circle_km(39.0438, -77.4874, 45.8399, -119.7006);
        assert!((km - ASHBURN_BOARDMAN_KM).abs() < 5e-4, "{km}");
        let mut d = Deployment::preset(Preset::UsProduction2025);
        // 350.2351 km / (2/3 · 299792.458 km/s) = 1.752388 ms, + 1 ms.
        let l = d.city_latency(0, 40.7128, -74.0060).unwrap();
        assert!((l.as_nanos() as f64 - 2_752_388.0).abs() < 2.0, "{l:?}");
        assert_eq!(d.intra_region_latency(), Some(Duration::from_millis(1)));
        assert_eq!(
            d.inter_provider_latency(0, 1),
            Some(Duration::from_millis(1))
        );
        // 3502.5798 km → 17.525023 ms, + 1 ms.
        d.providers[1].region = Region::assumed("us-west").unwrap();
        let l = d.inter_provider_latency(0, 1).unwrap();
        assert!((l.as_nanos() as f64 - 18_525_023.0).abs() < 2.0, "{l:?}");
        let flat = Deployment::preset(Preset::CampReference);
        assert_eq!(flat.city_latency(0, 40.0, -74.0), None);
    }

    #[test]
    fn fleet_assignment_follows_the_shares_and_is_stable() {
        let d = Deployment::preset(Preset::UsProduction2025);
        let n = 20_000u64;
        let b = (0..n).filter(|k| d.provider_of(7, *k) == 1).count() as f64 / n as f64;
        assert!((b - 0.5).abs() < 0.015, "{b}");
        assert_eq!(d.provider_of(7, 42), d.provider_of(7, 42));
        let same = (0..1_000u64)
            .filter(|k| d.provider_of(7, *k) == d.provider_of(8, *k))
            .count();
        assert!(
            (400..600).contains(&same),
            "two seeds give independent splits: {same}"
        );
        let mut skew = d.clone();
        skew.providers[0].fleet_share = 0.9;
        skew.providers[1].fleet_share = 0.1;
        let b = (0..n).filter(|k| skew.provider_of(7, *k) == 1).count() as f64 / n as f64;
        assert!((b - 0.1).abs() < 0.01, "{b}");
    }

    #[test]
    fn reports_about_another_providers_certificate_reach_its_ma() {
        let mut d = Deployment::preset(Preset::UsProduction2025);
        assert_eq!(d.report_route(0, 1), [0, 1]);
        assert_eq!(d.report_route(1, 1), [1]);
        d.ma_report_sharing = false;
        assert_eq!(d.report_route(0, 1), [0]);
    }
}
