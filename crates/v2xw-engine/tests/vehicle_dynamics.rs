//! `traffic.vehicle_dynamics` reaches the road (WP-22, P22.3;
//! docs/design/research/grade-dynamics.md).
//!
//! - With only the grade on, a flat world is byte-identical: a level lane chord applies
//!   nothing, so not a single state changes.
//! - The power limit slows tractor-trailers on a freeway: the design truck's acceleration
//!   is power-limited above about 15 m/s.
//! - The expectation rows of `tests/expectations/traffic.yaml` for the two keys hold on
//!   their families (`scenarios/sensitivity/catalogue.yaml`): the grade's on the flat
//!   `freight` road, the power limit's on `grade-truck`, the same road laid on a 5 % ramp
//!   (P22.4).
//! - `world.terrain.drape` lays the lanes on the DEM: on `grade-5pct.asc` every 1.5 km
//!   lane of that road climbs or falls 5 %, and undraped every lane stays at z = 0.

use std::path::{Path, PathBuf};

use v2xw_core::rng::RngRegistry;
use v2xw_engine::Scenario;
use v2xw_engine::scenario::schema::VehicleDynamicsSettings;
use v2xw_engine::sensitivity::{expect, sweep};
use v2xw_mobility::{Mobility, MobilityCtx, VehicleClass};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A signalised procedural grid with steady Poisson demand, `trailers` of it
/// tractor-trailers.
fn grid_scenario(trailers: f64) -> Scenario {
    scenario(
        serde_json::json!({
            "cols": 4, "rows": 4, "block_x_m": 200.0, "block_y_m": 150.0,
            "lanes_per_direction": 2, "speed_limit_mps": 15.6, "signalised": true,
            "corner_radius_m": 4.5
        }),
        trailers,
    )
}

/// The six-lane 65 mph road of `highway-congestion.yaml` (two 1.5 km blocks, no
/// signals), `trailers` of the fleet tractor-trailers.
fn freeway_scenario(trailers: f64) -> Scenario {
    scenario(
        serde_json::json!({
            "cols": 2, "rows": 2, "block_x_m": 1500.0, "block_y_m": 60.0,
            "lanes_per_direction": 3, "lane_width_m": 3.66, "sidewalk_m": 0.0,
            "speed_limit_mps": 29.06, "signalised": false, "crossings": false,
            "block_buildings": false, "rsu_at_junctions": false, "corner_radius_m": 15.0
        }),
        trailers,
    )
}

fn scenario(params: serde_json::Value, trailers: f64) -> Scenario {
    let mut s = Scenario::minimal();
    s.world.source =
        v2xw_world::WorldSourceSpec::procedural("world/source/procedural-grid", params);
    s.time.duration_s = 120.0;
    s.actors.vehicles.demand.kind = "mobility/demand/poisson".to_string();
    s.actors.vehicles.demand.rate_veh_per_h = Some(2400.0);
    if trailers > 0.0 {
        s.actors.vehicles.classes = serde_json::from_value(serde_json::json!({
            "passenger": {"fraction": 1.0 - trailers},
            "trailer": {"fraction": trailers}
        }))
        .expect("a class mix");
    }
    s
}

/// The run's mobility: a digest of every published state, and the mean speed of the
/// trailers and of everything, over every vehicle-step.
fn run(s: &Scenario) -> (String, f64, f64) {
    s.validate().expect("the scenario validates");
    let world = v2xw_engine::wiring::build_world(s).expect("world");
    let rng = RngRegistry::new(s.seed);
    let mut mobility = v2xw_engine::wiring::native_mobility(s);
    {
        let demand = v2xw_engine::wiring::build_demand(s, &world).expect("demand");
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        mobility.init(&mut ctx, demand).expect("init");
    }
    let step = s.time.mobility_step();
    let mut trace = String::new();
    let mut trailers = std::collections::BTreeSet::new();
    let (mut sum, mut n, mut tsum, mut tn) = (0.0, 0u64, 0.0, 0u64);
    let mut t = 0u64;
    while t < (s.time.duration_s * 1e9) as u64 {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = mobility.step(&mut ctx, step);
        for sp in &update.spawned {
            if sp.class == VehicleClass::Trailer {
                trailers.insert(sp.actor);
            }
        }
        trace.push_str(&format!("{:?}\n", update.states));
        for (id, k) in &update.states {
            let v = k.ground_speed_mps();
            sum += v;
            n += 1;
            if trailers.contains(id) {
                tsum += v;
                tn += 1;
            }
        }
        t = update.t;
    }
    assert!(n > 1000, "{n} vehicle-steps");
    (
        v2xw_core::hash::sha256_hex(trace.as_bytes()),
        tsum / tn.max(1) as f64,
        sum / n as f64,
    )
}

fn with(mut s: Scenario, grade: bool, power_limit: bool) -> Scenario {
    s.traffic
        .get_or_insert_with(Default::default)
        .vehicle_dynamics = Some(VehicleDynamicsSettings {
        grade: Some(grade),
        power_limit: Some(power_limit),
    });
    s
}

#[test]
fn flat_grade_only_is_byte_identical() {
    let base = grid_scenario(0.2);
    let (off, _, _) = run(&with(base.clone(), false, false));
    let (on, _, _) = run(&with(base, true, false));
    assert_eq!(off, on, "the grade alone moved a flat world");
}

#[test]
fn the_power_limit_slows_tractor_trailers_on_a_freeway() {
    // Power = force × speed: the 120 kg/kW design truck's tractive force falls below
    // what the IDM asks of it on the way up to its 23.6 m/s above about 15 m/s, so on a
    // freeway it takes longer to reach speed. In a 35 mph city it never binds (the IDM
    // asks less than the power gives below 13 m/s), which is the realistic answer.
    let base = freeway_scenario(0.3);
    let (d_off, trailer_off, _) = run(&with(base.clone(), false, false));
    let (d_on, trailer_on, _) = run(&with(base, false, true));
    assert_ne!(d_off, d_on, "the power limit reached nothing");
    assert!(
        trailer_on < trailer_off * 0.995,
        "trailers {trailer_on} m/s on, {trailer_off} m/s off"
    );
}

#[test]
fn the_vehicle_dynamics_rows_hold() {
    let path = root().join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| e.setting.starts_with("traffic.vehicle_dynamics"));
    assert_eq!(exp.len(), 2, "the grade and the power-limit rows");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "{unplanned:?}");
    let bases = fam.bases(&path).expect("bases load");
    let result =
        sweep::run_cached(settings, vec![], &bases, &exp, 2, None, |_, _, _| {}).expect("runs");
    let failed: Vec<String> = result
        .verdicts
        .iter()
        .filter(|v| !v.pass)
        .map(|v| {
            format!(
                "{} {}: {:?} -> {:?} ({})",
                v.expectation.setting, v.expectation.metric, v.base, v.value, v.note
            )
        })
        .collect();
    // The grade on `freight`, the power limit on `grade-truck` (its first family).
    assert_eq!(result.verdicts.len(), 2);
    assert!(failed.is_empty(), "{failed:#?}");
}

/// `world.terrain.drape` (P22.4) on the shipped ramp `scenarios/sensitivity/grade-5pct.asc`
/// (a synthetic plane rising 5 % to the east; `grade-5pct.md`): every east-west lane of the
/// freeway's 1.5 km blocks climbs or falls 5 % end to end, and with the same DEM undraped
/// every lane point stays on z = 0 (the DEM is then only the radio's ground).
#[test]
fn a_draped_ramp_tilts_the_lanes_and_undraped_nothing_moves() {
    let dem = root().join("scenarios/sensitivity/grade-5pct.asc");
    let world_with = |drape: bool| {
        let mut s = freeway_scenario(0.0);
        s.world.terrain.dem = Some(dem.to_string_lossy().to_string());
        s.world.terrain.drape = drape;
        s.validate().expect("the scenario validates");
        v2xw_engine::wiring::build_world(&s).expect("world")
    };
    let flat = world_with(false);
    assert!(
        flat.roads
            .lanes()
            .iter()
            .all(|l| l.centreline.iter().all(|p| p.z == 0.0)),
        "an undraped world moved its lanes"
    );
    let hill = world_with(true);
    let mut long = 0;
    for l in hill.roads.lanes().iter().filter(|l| l.length_m > 1000.0) {
        let (a, b) = (l.centreline[0], l.centreline[l.centreline.len() - 1]);
        assert!(
            (b.x - a.x).abs() > 900.0,
            "a long lane that is not east-west: {a:?} -> {b:?}"
        );
        let grade = (b.z - a.z) / (b.x - a.x);
        assert!(
            (grade - 0.05).abs() < 0.002,
            "lane {} rises {grade:.4} per metre east, not 0.05",
            l.id.index()
        );
        long += 1;
    }
    assert!(long >= 6, "only {long} long lanes");
}

/// The measured DEM (WP-26 P26.2): `worlds/dem/portland-downtown.asc`, read here directly
/// as a lattice of posts (row 0 is the north edge, as the file stores it).
struct Posts {
    ncols: usize,
    nrows: usize,
    lon0: f64,
    lat0: f64,
    cell: f64,
    z: Vec<f64>,
}

impl Posts {
    fn read(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).expect("the DEM is in the repo");
        let mut it = text.split_whitespace();
        let mut head = |key: &str| {
            assert_eq!(it.next(), Some(key));
            it.next().unwrap().parse::<f64>().unwrap()
        };
        let (ncols, nrows) = (head("ncols") as usize, head("nrows") as usize);
        let (lon0, lat0, cell) = (head("xllcenter"), head("yllcenter"), head("cellsize"));
        let _nodata = head("nodata_value");
        let z: Vec<f64> = it.map(|t| t.parse().unwrap()).collect();
        assert_eq!(z.len(), ncols * nrows);
        Self {
            ncols,
            nrows,
            lon0,
            lat0,
            cell,
            z,
        }
    }

    /// The post `i` columns east and `j` rows north of the south-west post.
    fn at(&self, i: usize, j: usize) -> f64 {
        self.z[(self.nrows - 1 - j) * self.ncols + i]
    }

    /// The cell holding (lat, lon): its south-west post and the fractions into it.
    fn cell_of(&self, lat: f64, lon: f64) -> (usize, usize, f64, f64) {
        let (fx, fy) = ((lon - self.lon0) / self.cell, (lat - self.lat0) / self.cell);
        let (i, j) = (fx.floor(), fy.floor());
        assert!(i >= 1.0 && j >= 1.0, "outside the DEM");
        let (i, j) = (i as usize, j as usize);
        assert!(i + 2 < self.ncols && j + 2 < self.nrows, "outside the DEM");
        (i, j, fx - i as f64, fy - j as f64)
    }

    fn bilinear(&self, lat: f64, lon: f64) -> f64 {
        let (i, j, u, v) = self.cell_of(lat, lon);
        let s = self.at(i, j) * (1.0 - u) + self.at(i + 1, j) * u;
        let n = self.at(i, j + 1) * (1.0 - u) + self.at(i + 1, j + 1) * u;
        s * (1.0 - v) + n * v
    }

    /// The highest minus the lowest of the four posts of the cell holding (lat, lon).
    fn cell_relief(&self, lat: f64, lon: f64) -> f64 {
        let (i, j, _, _) = self.cell_of(lat, lon);
        let c = [
            self.at(i, j),
            self.at(i + 1, j),
            self.at(i, j + 1),
            self.at(i + 1, j + 1),
        ];
        c.iter().copied().fold(f64::MIN, f64::max) - c.iter().copied().fold(f64::MAX, f64::min)
    }

    /// The lowest and highest of the 4 x 4 posts round the cell holding (lat, lon).
    fn hull(&self, lat: f64, lon: f64) -> (f64, f64) {
        let (i, j, _, _) = self.cell_of(lat, lon);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for a in i - 1..=i + 2 {
            for b in j - 1..=j + 2 {
                lo = lo.min(self.at(a, b));
                hi = hi.max(self.at(a, b));
            }
        }
        (lo, hi)
    }
}

/// `scenarios/real-world/portland-hills.yaml` (WP-26 P26.2) drapes its lanes on the
/// measured DEM: 20 keyed lane points sit on the heights a direct bilinear read of the
/// file gives, within one DEM cell's relief (the engine resamples the posts onto its own
/// 30 m grid before it drapes, so the two interpolations can differ by up to that), and
/// inside the hull of the posts round them. Heights are the rise over the undraped lane,
/// so a bridge deck keeps its level above the ground.
#[test]
fn a_measured_dem_drapes_the_lanes_to_its_heights() {
    let mut s = Scenario::load(root().join("scenarios/real-world/portland-hills.yaml"))
        .expect("the scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source {
        *path = root().join(path.as_str()).to_string_lossy().into_owned();
    }
    let dem = root().join(s.world.terrain.dem.clone().expect("a DEM"));
    s.world.terrain.dem = Some(dem.to_string_lossy().into_owned());
    assert!(s.world.terrain.drape, "the shipped scenario drapes");
    s.validate().expect("the scenario validates");
    let world = v2xw_engine::wiring::build_world(&s).expect("world");
    // A bridge deck already stands a level above the zero plane before the drape
    // (`osm.rs`: `TUNNEL_LEVEL_DEPTH_M` per `layer`), and the drape adds the ground to it
    // (`dem::with_terrain`: z_new = z_old + dem), so the DEM is compared with the rise
    // over the undraped world's z.
    s.world.terrain.drape = false;
    let flat = v2xw_engine::wiring::build_world(&s).expect("world");
    let posts = Posts::read(&dem);
    let proj = world.projection();
    let lift = |w: &v2xw_world::World| -> Vec<v2xw_core::geom::Vec3> {
        w.roads
            .lanes()
            .iter()
            .flat_map(|l| l.centreline.iter().copied())
            .collect()
    };
    let (draped, before) = (lift(&world), lift(&flat));
    assert_eq!(
        draped.len(),
        before.len(),
        "the drape changed the lane points"
    );
    let points: Vec<v2xw_core::geom::Vec3> = draped
        .iter()
        .zip(&before)
        .map(|(d, b)| {
            assert!((d.x - b.x).abs() < 1e-9 && (d.y - b.y).abs() < 1e-9);
            v2xw_core::geom::Vec3::new(d.x, d.y, d.z - b.z)
        })
        .collect();
    assert!(points.len() > 1_000, "{} lane points", points.len());
    assert!(
        before.iter().any(|p| p.z > 0.0),
        "no bridge deck: the rise is not exercised"
    );
    let mut state = 0x0026_5EED_u64;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) as usize
    };
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut bad = Vec::new();
    for _ in 0..20 {
        let p = points[next() % points.len()];
        let (lat, lon) = proj.to_geodetic(p.x, p.y);
        let want = posts.bilinear(lat, lon) - world.origin.alt_m;
        let tol = posts.cell_relief(lat, lon) + 0.5;
        let (h_lo, h_hi) = posts.hull(lat, lon);
        let (h_lo, h_hi) = (h_lo - world.origin.alt_m, h_hi - world.origin.alt_m);
        lo = lo.min(want);
        hi = hi.max(want);
        if (p.z - want).abs() > tol || p.z < h_lo - 0.1 || p.z > h_hi + 0.1 {
            bad.push(format!(
                "({lat:.5}, {lon:.5}): lane z {:.2} m, DEM {want:.2} +- {tol:.2} m, hull {h_lo}..{h_hi}",
                p.z
            ));
        }
    }
    assert!(
        hi - lo > 20.0,
        "the points span only {:.1} m: the check would not bite",
        hi - lo
    );
    assert!(
        bad.is_empty(),
        "{} of 20 lane points off the DEM: {bad:#?}",
        bad.len()
    );
}

/// The two `world.terrain.drape` rows of `tests/expectations/traffic.yaml` hold on the
/// `hills` family (WP-26 P26.2): on downtown Portland's measured ground, with the grade
/// and the power limit on, the drape lowers the mean speed and changes the PRR.
#[test]
fn the_hill_rows_hold() {
    let path = root().join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| e.setting == "world.terrain.drape");
    assert_eq!(exp.len(), 2, "the speed and the PRR rows");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "{unplanned:?}");
    let bases = fam.bases(&path).expect("bases load");
    let result =
        sweep::run_cached(settings, vec![], &bases, &exp, 2, None, |_, _, _| {}).expect("runs");
    for v in &result.verdicts {
        eprintln!(
            "hills: {} {}: {:?} -> {:?} pass {} ({})",
            v.expectation.setting, v.expectation.metric, v.base, v.value, v.pass, v.note
        );
    }
    let failed: Vec<&str> = result
        .verdicts
        .iter()
        .filter(|v| !v.pass)
        .map(|v| v.expectation.metric.as_str())
        .collect();
    assert_eq!(result.verdicts.len(), 2);
    assert!(failed.is_empty(), "{failed:?}");
}
