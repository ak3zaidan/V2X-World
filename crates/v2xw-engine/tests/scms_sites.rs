//! Exact server locations (docs/design/scms-real/08-settings-ux-and-gaps.md §5, pieces
//! C1a-C1b): a provider placed at a cloud site measures every latency from the site's
//! point, a role placed at its own site pays the leg to it on every link it has, the
//! combinations that cannot run are refused at their path, and the manifest says where
//! every point comes from.

use serde_json::{Value, json};
use v2xw_core::ids::NodeId;
use v2xw_core::time::Duration;
use v2xw_engine::Scenario;
use v2xw_engine::phase2::{CAMP_SCMS, deployment_of, deployment_warnings};
use v2xw_proto::ScmsParams;
use v2xw_proto::scms::deployment::{
    C_KM_S, Deployment, LatencyModel, Preset, REGIONS, SITES, great_circle_km,
};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::StageId;

/// The Census 2024 Gazetteer internal point of The Dalles city, OR (GEOID 4172950), the
/// city Google Cloud names for us-west1.
const THE_DALLES: (f64, f64) = (45.598_386, -121.152_411);

fn scenario(deployment: Value) -> Scenario {
    let mut s = Scenario::minimal();
    s.security.protocol = Some(serde_json::from_value(json!({"id": CAMP_SCMS})).expect("a choice"));
    s.security.scms =
        Some(serde_json::from_value(json!({"deployment": deployment})).expect("the block parses"));
    s
}

fn errors(deployment: Value) -> Vec<String> {
    v2xw_engine::scenario::validate::validate(&scenario(deployment))
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// One way at 2/3 c over `km`, plus the 1 ms per-link overhead.
fn expected_ns(km: f64) -> f64 {
    km / (C_KM_S * 2.0 / 3.0) * 1e9 + 1e6
}

/// Accept test 1: provider-b at Google us-west1 is The Dalles' distance from provider-a's
/// Ashburn point, at 2/3 c plus 1 ms, within 1 µs; and its city leg starts there too.
/// Fault: the parser reads `site` but leaves `region` in place.
#[test]
fn a_site_moves_every_latency_to_its_point() {
    let d = deployment_of(
        &scenario(json!({"providers": [{"name": "provider-b", "site": "gcp-us-west1"}]})),
        true,
    )
    .expect("builds");
    assert_eq!(d.latency, LatencyModel::DEFAULT_REGION);
    let ashburn = REGIONS[0];
    let km = great_circle_km(ashburn.lat_deg, ashburn.lon_deg, THE_DALLES.0, THE_DALLES.1);
    assert!(km > 3_400.0 && km < 3_700.0, "{km}");
    let got = d.inter_provider_latency(0, 1).expect("the region model");
    assert!(
        (got.as_nanos() as f64 - expected_ns(km)).abs() < 1_000.0,
        "{got:?} against {} ns",
        expected_ns(km)
    );
    // The city leg from Portland City Hall (45.5152, -122.6784) starts at the site too.
    let city = d
        .city_latency(1, 45.5152, -122.6784)
        .expect("the region model");
    let km = great_circle_km(45.5152, -122.6784, THE_DALLES.0, THE_DALLES.1);
    assert!(
        (city.as_nanos() as f64 - expected_ns(km)).abs() < 1_000.0,
        "{city:?}"
    );
    // Custom coordinates are a site too.
    let d = deployment_of(
        &scenario(
            json!({"providers": [{"name": "provider-a", "site": "custom",
                                        "lat_deg": 40.7128, "lon_deg": -74.006}]}),
        ),
        true,
    )
    .expect("builds");
    let km = great_circle_km(40.7128, -74.006, ashburn.lat_deg, ashburn.lon_deg);
    let got = d.inter_provider_latency(0, 1).expect("the region model");
    assert!(
        (got.as_nanos() as f64 - expected_ns(km)).abs() < 1_000.0,
        "{got:?}"
    );
}

/// A scenario naming one of today's regions as its site runs exactly as with `region`, and
/// one naming nothing is the preset, field for field.
#[test]
fn todays_regions_as_sites_and_the_default_are_unchanged() {
    let site = deployment_of(
        &scenario(json!({"providers": [{"name": "provider-b", "site": "us-west"}]})),
        true,
    )
    .expect("builds");
    let region = deployment_of(
        &scenario(json!({"providers": [{"name": "provider-b", "region": "us-west"}]})),
        true,
    )
    .expect("builds");
    assert_eq!(site.providers[1].region, region.providers[1].region);
    assert_eq!(
        site.inter_provider_latency(0, 1),
        region.inter_provider_latency(0, 1)
    );
    let none = deployment_of(&scenario(json!({})), true).expect("builds");
    assert_eq!(none, Deployment::preset(Preset::UsProduction2025));
    assert!(none.providers.iter().all(|p| p.site.is_none()));
    assert_eq!(
        none.inter_provider_latency(0, 1),
        Some(Duration::from_millis(1))
    );
    // Every catalogue site that stands for one of today's regions has its point.
    for r in REGIONS {
        let s = SITES
            .iter()
            .find(|s| s.id == r.name)
            .expect("in the catalogue");
        assert_eq!(s.region(), r);
    }
}

/// Accept test 3: `site` with `region`, an unknown site, and coordinates without
/// `site: custom` (or `custom` without them) are each refused at their own path, by the
/// loader and by the run. Fault: drop the exclusivity check.
#[test]
fn site_conflicts_are_refused_at_their_path() {
    let at = "security.scms.deployment.providers.provider-b";
    let cases = [
        (
            json!({"site": "gcp-us-west1", "region": "us-east"}),
            format!("{at}.site"),
            "a site replaces the region",
        ),
        (
            json!({"site": "mars-1"}),
            format!("{at}.site"),
            "'mars-1' is not one of",
        ),
        (
            json!({"lat_deg": 45.0, "lon_deg": -120.0}),
            format!("{at}.lat_deg"),
            "only to site: custom",
        ),
        (
            json!({"site": "gcp-us-east4", "lon_deg": -120.0}),
            format!("{at}.lon_deg"),
            "only to site: custom",
        ),
        (
            json!({"site": "custom", "lat_deg": 45.0}),
            format!("{at}.lon_deg"),
            "set both",
        ),
        (
            json!({"site": "custom", "lat_deg": 95.0, "lon_deg": 0.0}),
            format!("{at}.lat_deg"),
            "outside the allowed range",
        ),
    ];
    for (fields, path, words) in cases {
        let mut p = json!({"name": "provider-b"});
        for (k, v) in fields.as_object().expect("an object") {
            p[k] = v.clone();
        }
        let deployment = json!({"providers": [p]});
        let got = errors(deployment.clone());
        assert!(
            got.iter()
                .any(|m| m.starts_with(&path) && m.contains(words)),
            "{fields}: must be refused at {path} with '{words}'; got {got:#?}"
        );
        let run = deployment_of(&scenario(deployment), true);
        assert!(
            run.as_ref().is_err_and(|e| e.to_string().contains(&path)),
            "{fields}: the run must refuse it at {path} too; got {run:?}"
        );
    }
    // Every catalogue site and a well-formed custom site load.
    for s in SITES {
        let got = errors(json!({"providers": [{"name": "provider-b", "site": s.id}]}));
        assert!(got.is_empty(), "{}: {got:#?}", s.id);
    }
    assert!(
        errors(
            json!({"providers": [{"name": "provider-b", "site": "custom",
                                      "lat_deg": 45.0, "lon_deg": -120.0}]})
        )
        .is_empty()
    );
}

/// Accept test 4: the manifest names each placed provider's site and its `point_source`,
/// with the published location's source and the stated error bound; an unplaced provider
/// keeps today's region line. Fault: omit the sites from `assumptions()`.
#[test]
fn the_manifest_names_every_sites_point_source() {
    let w = deployment_warnings(&scenario(json!({"providers": [
        {"name": "provider-a", "site": "custom", "lat_deg": 40.7128, "lon_deg": -74.006},
        {"name": "provider-b", "site": "gcp-us-west1"},
    ]})));
    let has = |needle: &str| w.iter().any(|l| l.contains(needle));
    assert!(
        has("provider-b runs at site gcp-us-west1")
            && has("point_source published-city")
            && has("docs.cloud.google.com/compute/docs/regions-zones"),
        "{w:#?}"
    );
    assert!(
        has("provider-a runs at site custom") && has("point_source custom"),
        "{w:#?}"
    );
    assert!(has("within about 30 km") && has("0.150 ms"), "{w:#?}");
    assert!(has("2024_Gaz_place_national"), "{w:#?}");
    let azure = deployment_warnings(&scenario(
        json!({"providers": [{"name": "provider-b", "site": "azure-eastus"}]}),
    ));
    assert!(
        azure
            .iter()
            .any(|l| l.contains("point_source assumed-metro") && l.contains("Boydton")),
        "{azure:#?}"
    );
    let plain = deployment_warnings(&scenario(json!({})));
    assert!(
        plain
            .iter()
            .any(|l| l.contains("provider-b runs in region us-east")),
        "{plain:#?}"
    );
    assert!(!plain.iter().any(|l| l.contains("within about 30 km")));
}

/// When a report a device files about another's pseudonym reaches provider-a's MA, in a
/// home run of the deployment the scenario builds.
fn report_reaches_the_ma(deployment: Value) -> u64 {
    let d = deployment_of(&scenario(deployment), true).expect("builds");
    let mut run = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    let (accused, reporter) = (NodeId::new(5_000_301), NodeId::new(5_000_302));
    run.preload(accused, 0, 1, 2).expect("provisions");
    run.preload(reporter, 0, 1, 2).expect("provisions");
    let (&(i, _), c) = run.state.devices[&accused]
        .credentials
        .iter()
        .next()
        .expect("a pseudonym");
    let flow = run.submit_report(reporter, i, c.lv);
    run.run_until(3_600_000_000_000).expect("runs");
    run.kernel
        .stages
        .at(flow, StageId::ReportReceived)
        .expect("the report reached the MA")
}

/// Accept test 2: `sites: {ma: us-west}` on provider-a. The report's RA -> MA hop is later
/// than in the base run by the Ashburn -> Boardman leg less the in-region hop, ±1 µs; the
/// MA-to-MA link is measured from the MA's site and the city leg still from the RA's.
/// Fault: intra-provider links still use `over_km(0)`.
#[test]
fn a_roles_own_site_delays_its_links_by_the_computed_leg() {
    let moved = json!({"providers": [{"name": "provider-a", "sites": {"ma": "us-west"}}]});
    let (east, west) = (REGIONS[0], REGIONS[4]);
    assert_eq!((east.name, west.name), ("us-east", "us-west"));
    let km = great_circle_km(east.lat_deg, east.lon_deg, west.lat_deg, west.lon_deg);
    // 3,502.5798 km at 2/3 c is 17,525,023 ns (the proto test's independent figure).
    let leg_ns = expected_ns(km) - 1e6;
    assert!((leg_ns - 17_525_023.0).abs() < 2.0, "{leg_ns}");

    let d = deployment_of(&scenario(moved.clone()), true).expect("builds");
    assert_eq!(d.providers[0].point("ma"), west);
    assert_eq!(d.providers[0].point("ra"), east);
    let mama = d.inter_provider_latency(0, 1).expect("the region model");
    assert!(
        (mama.as_nanos() as f64 - expected_ns(km)).abs() < 1_000.0,
        "{mama:?}"
    );
    let base = deployment_of(&scenario(json!({})), true).expect("builds");
    assert_eq!(
        d.city_latency(0, 40.7128, -74.006),
        base.city_latency(0, 40.7128, -74.006)
    );

    let before = report_reaches_the_ma(json!({}));
    let after = report_reaches_the_ma(moved.clone());
    let delta = after as f64 - before as f64;
    assert!(
        (delta - leg_ns).abs() < 1_000.0,
        "the RA -> MA hop moved by {delta} ns, not the {leg_ns} ns leg ({before} -> {after})"
    );
    let w = deployment_warnings(&scenario(moved));
    assert!(
        w.iter()
            .any(|l| l.contains("provider-a's ma runs at site us-west")),
        "{w:#?}"
    );
}
