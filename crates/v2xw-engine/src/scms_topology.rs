//! Where each SCMS server runs, how much it can serve, and how deep each provider's CA
//! hierarchy is (docs/design/scms-real/08-settings-ux-and-gaps.md §5, §6 and §8).
//! **Scaffolding (piece A0).**
//!
//! What is here: the keys' menus, ranges, titles and status, the refusal, and the three
//! calls [`crate::phase2`] already makes ([`apply_sites`] and [`ica_levels`] for each
//! provider in `deployment_of`, [`apply_capacity`] once every provider's run exists).
//! Every value but the default is refused with "not built yet", so nothing half-works;
//! the defaults (no site, no capacity, `ica_levels: 1`) are today's deployment exactly.
//!
//! Pieces C1a-C3b build each gap here and in `v2xw-proto`'s `scms::deployment`,
//! `governance`, `service` and `kernel`, and make a key live by flipping its row in
//! [`KEY_STATUS`] and its refusal in [`refusals`] (§11).

use serde_json::{Value, json};
use v2xw_proto::scms::deployment::{Provider, ROLES};
use v2xw_proto::scms::run::ScmsRun;

use crate::error::{EngineError, Result, ScenarioError};
use crate::scenario::Scenario;
use crate::scenario::module_keys::{self, KeyWords, Opt};
use crate::scenario::schema::ProviderSpec;
use crate::scenario::validate::{Bound, KeyStatus, Status};

const PROV: &str = "security.scms.deployment.providers[]";
const SITE: &str = "security.scms.deployment.providers[].site";
const SITES: &str = "security.scms.deployment.providers[].sites";
const SITES_EACH: &str = "security.scms.deployment.providers[].sites.*";
const LAT: &str = "security.scms.deployment.providers[].lat_deg";
const LON: &str = "security.scms.deployment.providers[].lon_deg";
const PROV_CAPACITY: &str = "security.scms.deployment.providers[].capacity";
const CAPACITY: &str = "security.scms.deployment.capacity";
const LEVELS: &str = "security.scms.deployment.providers[].ica_levels";

/// Intermediate CA levels in the US system: one ICA under each root (`ROLE_SPECS`).
pub const US_ICA_LEVELS: u32 = 1;

/// `capacity.<role>.service`: the workers in parallel, or one request at a time.
pub const SERVICE_IDS: [&str; 2] = ["mmc", "fifo-1"];

/// The site catalogue's ids (08 §5): today's six assumed regions, every US region of the
/// three clouds with its published location, and `custom`.
pub const SITE_IDS: [&str; 29] = [
    "us-east",
    "us-east-2",
    "us-central",
    "us-south",
    "us-west",
    "us-west-1",
    "gcp-us-central1",
    "gcp-us-east1",
    "gcp-us-east4",
    "gcp-us-east5",
    "gcp-us-south1",
    "gcp-us-west1",
    "gcp-us-west2",
    "gcp-us-west3",
    "gcp-us-west4",
    "azure-eastus",
    "azure-eastus2",
    "azure-centralus",
    "azure-northcentralus",
    "azure-southcentralus",
    "azure-westcentralus",
    "azure-westus",
    "azure-westus2",
    "azure-westus3",
    "aws-us-east-1",
    "aws-us-east-2",
    "aws-us-west-1",
    "aws-us-west-2",
    "custom",
];

/// The keys' implementation status. `validate::status_of` reads these beside its own
/// table, longest match winning.
pub static KEY_STATUS: &[KeyStatus] = &[
    KeyStatus {
        path: SITE,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §5, piece C1a): any site is refused; \
               a provider runs at its region.",
    },
    KeyStatus {
        path: SITES,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §5, piece C1b): a per-role site is \
               refused; every role of a provider runs at its region.",
    },
    KeyStatus {
        path: LAT,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §5, piece C1a): custom coordinates \
               are refused.",
    },
    KeyStatus {
        path: LON,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §5, piece C1a): custom coordinates \
               are refused.",
    },
    KeyStatus {
        path: CAPACITY,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §6, piece C2a): any capacity is \
               refused; every server keeps 4 workers, 1 ms overhead and an unlimited queue \
               (ASSUMPTION).",
    },
    KeyStatus {
        path: PROV_CAPACITY,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §6, piece C2b): a provider's own \
               capacity is refused.",
    },
    KeyStatus {
        path: LEVELS,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §8, piece C3a): only 1, the US \
               system's one intermediate CA under each root, loads.",
    },
];

const fn range(path: &'static str, lo: f64, hi: f64, what: &'static str) -> Bound {
    Bound {
        path,
        lo,
        hi,
        exclusive_lo: false,
        what,
    }
}

/// The ranges (08 §5, §6, §8 "Settings and menus").
static BOUNDS: &[Bound] = &[
    range(LAT, -90.0, 90.0, "the site's latitude"),
    range(LON, -180.0, 180.0, "the site's longitude"),
    range(
        "security.scms.deployment.capacity.*.workers",
        1.0,
        1_024.0,
        "the workers",
    ),
    range(
        "security.scms.deployment.capacity.*.queue_limit",
        1.0,
        10_000_000.0,
        "the queue limit",
    ),
    range(
        "security.scms.deployment.capacity.*.overhead_ms",
        0.0,
        10_000.0,
        "the per-request overhead",
    ),
    range(
        "security.scms.deployment.providers[].capacity.*.workers",
        1.0,
        1_024.0,
        "the workers",
    ),
    range(
        "security.scms.deployment.providers[].capacity.*.queue_limit",
        1.0,
        10_000_000.0,
        "the queue limit",
    ),
    range(
        "security.scms.deployment.providers[].capacity.*.overhead_ms",
        0.0,
        10_000.0,
        "the per-request overhead",
    ),
    // Root + n ICA levels + ACA + pseudonym certificate must fit the receiver's
    // eight-certificate chain (`v2xw_sec::chain::MAX_CHAIN_LENGTH`), so n <= 5.
    range(
        LEVELS,
        1.0,
        5.0,
        "the intermediate CA levels (a receiver accepts at most 8 certificates in a \
         chain: root, the levels, the ACA and the pseudonym certificate)",
    ),
];

const fn w(path: &'static str, title: &'static str, unit: &'static str, step: f64) -> KeyWords {
    KeyWords {
        path,
        title,
        unit,
        step,
    }
}

/// Titles, units and steps.
static WORDS: &[KeyWords] = &[
    w(SITE, "Server site", "", 0.0),
    w(SITES, "Server site per role", "", 0.0),
    w(SITES_EACH, "Site", "", 0.0),
    w(LAT, "Custom site latitude", "°", 0.0001),
    w(LON, "Custom site longitude", "°", 0.0001),
    w(CAPACITY, "Server capacity per role", "", 0.0),
    w(
        "security.scms.deployment.capacity.*.workers",
        "Workers (parallel servers)",
        "workers",
        1.0,
    ),
    w(
        "security.scms.deployment.capacity.*.queue_limit",
        "Queue limit (unset: unlimited)",
        "requests",
        1.0,
    ),
    w(
        "security.scms.deployment.capacity.*.overhead_ms",
        "Overhead per request",
        "ms",
        0.1,
    ),
    w(
        "security.scms.deployment.capacity.*.service",
        "How the workers serve",
        "",
        0.0,
    ),
    w(
        PROV_CAPACITY,
        "This provider's server capacity per role",
        "",
        0.0,
    ),
    w(
        "security.scms.deployment.providers[].capacity.*.workers",
        "Workers (parallel servers)",
        "workers",
        1.0,
    ),
    w(
        "security.scms.deployment.providers[].capacity.*.queue_limit",
        "Queue limit (unset: unlimited)",
        "requests",
        1.0,
    ),
    w(
        "security.scms.deployment.providers[].capacity.*.overhead_ms",
        "Overhead per request",
        "ms",
        0.1,
    ),
    w(
        "security.scms.deployment.providers[].capacity.*.service",
        "How the workers serve",
        "",
        0.0,
    ),
    w(LEVELS, "Intermediate CA levels", "levels", 1.0),
];

const fn site(value: &'static str, label: &'static str, description: &'static str) -> Opt {
    Opt {
        value,
        label,
        description,
        available: false,
    }
}

/// The site menu (08 §5 table; cloud pages read 2026-10-28). Every site is refused until
/// C1a builds the catalogue's points.
static SITE_MENU: &[Opt] = &[
    site(
        "us-east",
        "Assumed region us-east: Ashburn, VA",
        "Today's catalogue point for us-east (ASSUMPTION). Not built yet as a site (08 §5, \
         piece C1a); set region instead.",
    ),
    site(
        "us-east-2",
        "Assumed region us-east-2: Columbus, OH",
        "Today's catalogue point for us-east-2 (ASSUMPTION). Not built yet as a site (08 \
         §5, piece C1a); set region instead.",
    ),
    site(
        "us-central",
        "Assumed region us-central: Council Bluffs, IA",
        "Today's catalogue point for us-central (ASSUMPTION). Not built yet as a site (08 \
         §5, piece C1a); set region instead.",
    ),
    site(
        "us-south",
        "Assumed region us-south: San Antonio, TX",
        "Today's catalogue point for us-south (ASSUMPTION). Not built yet as a site (08 \
         §5, piece C1a); set region instead.",
    ),
    site(
        "us-west",
        "Assumed region us-west: Boardman, OR",
        "Today's catalogue point for us-west (ASSUMPTION). Not built yet as a site (08 \
         §5, piece C1a); set region instead.",
    ),
    site(
        "us-west-1",
        "Assumed region us-west-1: Santa Clara, CA",
        "Today's catalogue point for us-west-1 (ASSUMPTION). Not built yet as a site (08 \
         §5, piece C1a); set region instead.",
    ),
    site(
        "gcp-us-central1",
        "Google us-central1: Council Bluffs, IA (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-east1",
        "Google us-east1: Moncks Corner, SC (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-east4",
        "Google us-east4: Ashburn, VA (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-east5",
        "Google us-east5: Columbus, OH (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-south1",
        "Google us-south1: Dallas, TX (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-west1",
        "Google us-west1: The Dalles, OR (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-west2",
        "Google us-west2: Los Angeles, CA (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-west3",
        "Google us-west3: Salt Lake City, UT (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "gcp-us-west4",
        "Google us-west4: Las Vegas, NV (published city)",
        "Google Cloud names the city (regions and zones page). Not built yet (08 §5, piece \
         C1a).",
    ),
    site(
        "azure-eastus",
        "Azure eastus: Virginia (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-eastus2",
        "Azure eastus2: Virginia (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-centralus",
        "Azure centralus: Iowa (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-northcentralus",
        "Azure northcentralus: Illinois (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-southcentralus",
        "Azure southcentralus: Texas (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-westcentralus",
        "Azure westcentralus: Wyoming (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-westus",
        "Azure westus: California (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-westus2",
        "Azure westus2: Washington (published state; metro assumed)",
        "Microsoft names the state only (Azure regions list), so the point is a named \
         metro marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "azure-westus3",
        "Azure westus3: Phoenix, AZ (published city)",
        "Microsoft names the city (Azure regions list). Not built yet (08 §5, piece C1a).",
    ),
    site(
        "aws-us-east-1",
        "AWS us-east-1: N. Virginia (published state; metro assumed)",
        "Amazon names the state only (AWS regions page), so the point is a named metro \
         marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "aws-us-east-2",
        "AWS us-east-2: Ohio (published state; metro assumed)",
        "Amazon names the state only (AWS regions page), so the point is a named metro \
         marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "aws-us-west-1",
        "AWS us-west-1: N. California (published state; metro assumed)",
        "Amazon names the state only (AWS regions page), so the point is a named metro \
         marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "aws-us-west-2",
        "AWS us-west-2: Oregon (published state; metro assumed)",
        "Amazon names the state only (AWS regions page), so the point is a named metro \
         marked ASSUMPTION. Not built yet (08 §5, piece C1a).",
    ),
    site(
        "custom",
        "Custom coordinates",
        "Your own site at lat_deg and lon_deg, for an on-premises server (your \
         assumption). Not built yet (08 §5, piece C1a).",
    ),
];

static SERVICE_MENU: &[Opt] = &[
    Opt {
        value: "mmc",
        label: "Workers in parallel (M/M/c)",
        description: "Each request goes to the first free worker, as today's backend serves. \
                      Not built yet as a setting (08 §6, piece C2a).",
        available: false,
    },
    Opt {
        value: "fifo-1",
        label: "One at a time (M/M/1)",
        description: "One worker serves requests in arrival order. Not built yet as a \
                      setting (08 §6, piece C2a); actors.backend.entities.*.service_model \
                      offers it today.",
        available: false,
    },
];

/// The ICA-level menu: numbers, with the US value first and the chain limit stated.
fn levels_menu() -> Vec<Value> {
    let mut out = vec![json!({
        "value": US_ICA_LEVELS,
        "label": "One intermediate CA (US)",
        "description": "Root, one ICA, then the issuing CAs, as every US provider runs.",
        "available": true,
    })];
    for n in 2..=5u32 {
        out.push(json!({
            "value": n,
            "label": format!("{n} intermediate CAs (not US)"),
            "description": "A deeper hierarchy, each level issued by the one above; the run \
                            is then not the US system. Not built yet (08 §8, piece C3a).",
            "available": false,
        }));
    }
    out.push(json!({
        "value": 6,
        "label": "6 or more (refused)",
        "description": "A receiver accepts at most 8 certificates in a chain (IEEE 1609.2), \
                        and root, 6 levels, the ACA and the pseudonym certificate are 9.",
        "available": false,
    }));
    out
}

/// The range for `path`.
pub fn bound(path: &str) -> Option<&'static Bound> {
    BOUNDS.iter().find(|b| b.path == path)
}

/// The title, unit and step for `path`.
pub fn words(path: &str) -> Option<&'static KeyWords> {
    WORDS.iter().find(|k| k.path == path)
}

/// The menu for `path`, and whether it is closed.
pub fn options(path: &str) -> Option<(Vec<Value>, bool)> {
    match path {
        SITE | SITES_EACH => Some((module_keys::rows(SITE_MENU), true)),
        "security.scms.deployment.capacity.*.service"
        | "security.scms.deployment.providers[].capacity.*.service" => {
            Some((module_keys::rows(SERVICE_MENU), true))
        }
        LEVELS => Some((levels_menu(), true)),
        _ => None,
    }
}

/// Every path these tables name.
pub fn table_paths() -> Vec<String> {
    KEY_STATUS
        .iter()
        .map(|k| k.path)
        .chain(BOUNDS.iter().map(|b| b.path))
        .chain(WORDS.iter().map(|k| k.path))
        .map(str::to_string)
        .collect()
}

/// The closed key set of a map: every role a provider can run (`components`' roles).
fn map_keys(_template: &str) -> Option<&'static [&'static str]> {
    let roles: &'static [&'static str] = &ROLES;
    Some(roles)
}

/// Which of a provider's new keys to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Sites,
    Capacity,
    Levels,
}

fn provider_refusals(p: &ProviderSpec, group: Group, e: &mut Vec<ScenarioError>) {
    let v = match group {
        Group::Sites => json!({"site": p.site, "sites": p.sites, "lat_deg": p.lat_deg,
                               "lon_deg": p.lon_deg}),
        Group::Capacity => json!({"capacity": p.capacity}),
        Group::Levels => json!({"ica_levels": p.ica_levels}),
    };
    let at = format!("security.scms.deployment.providers.{}", p.name);
    let (piece, instead) = match group {
        Group::Sites => (
            "docs/design/scms-real/08-settings-ux-and-gaps.md §5, pieces C1a-C1b",
            "leave it unset; the provider runs at its region",
        ),
        Group::Capacity => (
            "docs/design/scms-real/08-settings-ux-and-gaps.md §6, pieces C2a-C2b",
            "leave it unset; every server keeps today's 4 workers and 1 ms",
        ),
        Group::Levels => (
            "docs/design/scms-real/08-settings-ux-and-gaps.md §8, pieces C3a-C3b",
            "leave it unset or 1, the US hierarchy",
        ),
    };
    for leaf in module_keys::leaves(&v, &at, PROV, &[SITES, PROV_CAPACITY], &map_keys, e) {
        if let Some(err) = module_keys::malformed(&leaf) {
            e.push(err);
        } else if !(leaf.template == LEVELS && leaf.value == json!(US_ICA_LEVELS)) {
            e.push(module_keys::not_built(&leaf, piece, instead));
        }
    }
}

fn capacity_refusals(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let Some(d) = s.security.scms.as_ref().and_then(|x| x.deployment.as_ref()) else {
        return;
    };
    let v = json!(d.capacity);
    for leaf in module_keys::leaves(&v, CAPACITY, CAPACITY, &[CAPACITY], &map_keys, e) {
        e.push(module_keys::malformed(&leaf).unwrap_or_else(|| {
            module_keys::not_built(
                &leaf,
                "docs/design/scms-real/08-settings-ux-and-gaps.md §6, piece C2a",
                "leave it unset; every server keeps today's 4 workers and 1 ms",
            )
        }));
    }
    for p in &d.providers {
        provider_refusals(p, Group::Capacity, e);
    }
}

/// Every refusal of the topology keys, in document order.
pub fn refusals(s: &Scenario) -> Vec<ScenarioError> {
    let mut e = Vec::new();
    if let Some(d) = s.security.scms.as_ref().and_then(|x| x.deployment.as_ref()) {
        for p in &d.providers {
            provider_refusals(p, Group::Sites, &mut e);
            provider_refusals(p, Group::Levels, &mut e);
        }
    }
    capacity_refusals(s, &mut e);
    e
}

/// The loader's check: every refusal, once (`deployment_of`, which the loader also runs,
/// reports the first of a provider's).
pub fn validate(s: &Scenario, e: &mut Vec<ScenarioError>) {
    for err in refusals(s) {
        if !e.contains(&err) {
            e.push(err);
        }
    }
}

fn first(e: Vec<ScenarioError>) -> Result<()> {
    match e.into_iter().next() {
        Some(err) => Err(EngineError::Scenario(err)),
        None => Ok(()),
    }
}

/// `providers[].site`, `.sites`, `.lat_deg` and `.lon_deg` onto one provider of the
/// deployment (`phase2::deployment_of`). The default, all unset, leaves it at its region.
///
/// # Errors
/// The first refusal of the provider's site keys.
pub(crate) fn apply_sites(p: &ProviderSpec, _provider: &mut Provider) -> Result<()> {
    let mut e = Vec::new();
    provider_refusals(p, Group::Sites, &mut e);
    first(e)
}

/// `providers[].ica_levels` onto one provider of the deployment (`phase2::deployment_of`).
/// The default, unset or 1, is the US hierarchy.
///
/// # Errors
/// A level count other than 1.
pub(crate) fn ica_levels(p: &ProviderSpec, _provider: &mut Provider) -> Result<()> {
    let mut e = Vec::new();
    provider_refusals(p, Group::Levels, &mut e);
    first(e)
}

/// `security.scms.deployment.capacity` and `providers[].capacity` onto every provider's
/// run, once each exists (`phase2`, after the peer runs are built). The default, all
/// unset, keeps each server as `actors.backend` and the tier left it.
///
/// # Errors
/// The first refusal of a capacity key.
pub(crate) fn apply_capacity(s: &Scenario, _runs: &mut [ScmsRun]) -> Result<()> {
    let mut e = Vec::new();
    capacity_refusals(s, &mut e);
    first(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario(deployment: Value) -> Scenario {
        let mut s = Scenario::minimal();
        s.security.protocol = Some(crate::scenario::ModelChoice {
            id: crate::phase2::CAMP_SCMS.to_string(),
            params: Value::Null,
        });
        s.security.scms = Some(
            serde_json::from_value(json!({"deployment": deployment})).expect("the block parses"),
        );
        s
    }

    fn refused(deployment: Value) -> Vec<String> {
        refusals(&scenario(deployment))
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn provider(fields: Value) -> Value {
        let mut p = json!({"name": "provider-a"});
        for (k, v) in fields.as_object().expect("an object") {
            p[k] = v.clone();
        }
        json!({"providers": [p]})
    }

    /// Every topology key refuses any value but the default, naming its path, and the
    /// defaults load with no refusal.
    #[test]
    fn every_non_default_value_is_refused() {
        assert!(refused(json!({})).is_empty());
        assert!(refused(provider(json!({"ica_levels": 1}))).is_empty());
        assert!(refused(json!({"capacity": {"ra": {}}})).is_empty());
        let at = "security.scms.deployment.providers.provider-a";
        let cases = [
            (
                provider(json!({"site": "gcp-us-west1"})),
                format!("{at}.site"),
            ),
            (
                provider(json!({"sites": {"ma": "us-west"}})),
                format!("{at}.sites.ma"),
            ),
            (provider(json!({"lat_deg": 40.0})), format!("{at}.lat_deg")),
            (
                provider(json!({"lon_deg": -100.0})),
                format!("{at}.lon_deg"),
            ),
            (
                provider(json!({"capacity": {"ra": {"workers": 4}}})),
                format!("{at}.capacity.ra.workers"),
            ),
            (
                provider(json!({"ica_levels": 2})),
                format!("{at}.ica_levels"),
            ),
            (
                json!({"capacity": {"ra": {"queue_limit": 10}}}),
                "security.scms.deployment.capacity.ra.queue_limit".to_string(),
            ),
            (
                json!({"capacity": {"crlg": {"service": "mmc"}}}),
                "security.scms.deployment.capacity.crlg.service".to_string(),
            ),
        ];
        for (deployment, path) in cases {
            let got = refused(deployment.clone());
            assert!(
                got.iter()
                    .any(|m| m.starts_with(&path) && m.contains("not built yet")),
                "{deployment} must be refused at {path} as not built yet; got {got:#?}"
            );
        }
    }

    /// Each menu offers exactly its `_IDS` constant, in order.
    #[test]
    fn menus_offer_exactly_their_ids() {
        let vals = |o: &[Opt]| o.iter().map(|x| x.value).collect::<Vec<_>>();
        assert_eq!(vals(SITE_MENU), SITE_IDS);
        assert_eq!(vals(SERVICE_MENU), SERVICE_IDS);
    }

    /// A malformed value is refused as malformed: six ICA levels name the chain limit, an
    /// unknown site and an unknown role are named.
    #[test]
    fn malformed_values_are_named_as_such() {
        let six = refused(provider(json!({"ica_levels": 6})));
        assert!(
            six.iter().any(|m| m.contains("at most 8 certificates")),
            "{six:#?}"
        );
        let site = refused(provider(json!({"site": "mars-1"})));
        assert!(
            site.iter().any(|m| m.contains("'mars-1' is not one of")),
            "{site:#?}"
        );
        let role = refused(json!({"capacity": {"cdn": {"workers": 2}}}));
        assert!(
            role.iter().any(|m| m.contains("'cdn' is not a key")),
            "{role:#?}"
        );
    }

    /// The run refuses them too, without the loader: `deployment_of` calls the site and
    /// level hooks for every provider, and the capacity hook refuses on its own.
    #[test]
    fn the_build_refuses_without_the_loader() {
        for fields in [json!({"site": "gcp-us-west1"}), json!({"ica_levels": 3})] {
            let s = scenario(provider(fields.clone()));
            let got = crate::phase2::deployment_of(&s, true);
            assert!(
                got.as_ref()
                    .is_err_and(|e| e.to_string().contains("not built yet")),
                "{fields}: deployment_of must refuse it; got {got:?}"
            );
        }
        assert!(
            crate::phase2::deployment_of(&scenario(provider(json!({"ica_levels": 1}))), true)
                .is_ok()
        );
        let s = scenario(json!({"capacity": {"ra": {"workers": 1}}}));
        assert!(apply_capacity(&s, &mut []).is_err());
        assert!(apply_capacity(&scenario(json!({})), &mut []).is_ok());
    }
}
