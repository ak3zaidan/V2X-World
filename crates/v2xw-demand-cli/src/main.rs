//! `v2xw-demand` — the demand and route-choice tools.
//!
//! ```text
//! v2xw-demand od2trips  --scenario S --matrix M.od [--zones grid:4x4] [--start 07:00] --out trips.json
//! v2xw-demand purposes  --scenario S --daily 20000 [--day 0] --out trips.json
//! v2xw-demand assign    --scenario S --trips trips.json [--choice gawron|logit|msa]
//!                       [--loader queue|bpr|micro] [--iterations 25] [--gap 0.02]
//!                       [--warm routes.json] --out routes.json
//! v2xw-demand sample    --scenario S --counts counts.csv [--method max-entropy|greedy] --out routes.json
//! v2xw-demand check     --scenario S --counts counts.csv --routes routes.json [--loader queue]
//! v2xw-demand gtfs-clip --feed DIR --bbox S,W,N,E --date YYYYMMDD --out DIR
//! v2xw-demand gtfs      --scenario S --feed DIR --date YYYYMMDD --start 08:00 [--run 900]
//! ```
//!
//! The world is `--scenario FILE` (the world that scenario builds, so route files match
//! it when the scenario runs them) or `--world grid` (the procedural test grid). Every
//! output is deterministic.

use std::collections::BTreeMap;
use std::path::PathBuf;

use v2xw_demand::assign::{AssignParams, Assigner};
use v2xw_demand::choice::ChoiceSpec;
use v2xw_demand::counts::{self, sampler};
use v2xw_demand::graph::LaneGraph;
use v2xw_demand::loader::{LoaderSpec, Window};
use v2xw_demand::od::zones::{ZoneSet, ZoneSpec, ZoneWeight};
use v2xw_demand::od::{Od2TripsParams, OdMatrix, od2trips};
use v2xw_demand::purpose::{PurposeModel, PurposeParams};
use v2xw_demand::routefile::{RouteFile, RoutedTrip, SavedAlternative};
use v2xw_demand::transit::{TransitParams, build_schedule};
use v2xw_demand::trips::{TripSet, world_hash};
use v2xw_world::{ClassMask, World};

type Res<T> = Result<T, String>;

struct Args {
    cmd: String,
    kv: BTreeMap<String, String>,
}

impl Args {
    fn parse() -> Res<Args> {
        let mut it = std::env::args().skip(1);
        let cmd = it
            .next()
            .ok_or_else(|| "usage: v2xw-demand <command> [--key value]...".to_string())?;
        let mut kv = BTreeMap::new();
        while let Some(k) = it.next() {
            let key = k
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected '{k}'"))?
                .to_string();
            let v = it.next().ok_or_else(|| format!("--{key} needs a value"))?;
            kv.insert(key, v);
        }
        Ok(Args { cmd, kv })
    }

    fn get(&self, k: &str) -> Option<&str> {
        self.kv.get(k).map(String::as_str)
    }

    fn need(&self, k: &str) -> Res<&str> {
        self.get(k).ok_or_else(|| format!("--{k} is required"))
    }

    fn num(&self, k: &str, default: f64) -> Res<f64> {
        self.get(k)
            .map(|v| {
                v.parse::<f64>()
                    .map_err(|_| format!("--{k} '{v}' is not a number"))
            })
            .transpose()
            .map(|v| v.unwrap_or(default))
    }
}

fn hhmm(s: &str) -> Res<f64> {
    let mut p = s.split(':');
    let h: f64 = p
        .next()
        .and_then(|x| x.parse().ok())
        .ok_or_else(|| format!("'{s}' is not HH:MM"))?;
    let m: f64 = p
        .next()
        .unwrap_or("0")
        .parse()
        .map_err(|_| format!("'{s}' is not HH:MM"))?;
    Ok(h * 3600.0 + m * 60.0)
}

fn bbox(s: &str) -> Res<[f64; 4]> {
    let v: Vec<f64> = s
        .split(',')
        .map(|x| x.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("'{s}' is not S,W,N,E"))?;
    if v.len() != 4 {
        return Err(format!("'{s}' is not S,W,N,E"));
    }
    Ok([v[0], v[1], v[2], v[3]])
}

fn world(a: &Args) -> Res<World> {
    if let Some(path) = a.get("scenario") {
        // Exactly the world a run of the scenario builds, so the route file's world hash
        // matches when the run loads it.
        let sc =
            v2xw_engine::scenario::Scenario::load_unvalidated(path).map_err(|e| e.to_string())?;
        return v2xw_engine::wiring::build_world(&sc).map_err(|e| e.to_string());
    }
    match a.get("world") {
        Some("grid") | None => {
            let p = v2xw_world::procedural::GridParams {
                signalised: true,
                ..v2xw_world::procedural::GridParams::legacy()
            };
            v2xw_world::procedural::grid(&p, &v2xw_world::ImportOptions::default())
                .map_err(|e| e.to_string())
        }
        Some(other) => Err(format!(
            "--world '{other}': give --scenario FILE, or --world grid"
        )),
    }
}

fn zones_of(a: &Args) -> Res<ZoneSpec> {
    let z = a.get("zones").unwrap_or("grid:3x3");
    let g = z
        .strip_prefix("grid:")
        .ok_or_else(|| "--zones is grid:CxR".to_string())?;
    let (c, r) = g
        .split_once('x')
        .ok_or_else(|| "--zones is grid:CxR".to_string())?;
    Ok(ZoneSpec::Grid {
        cols: c.parse().map_err(|_| "bad columns")?,
        rows: r.parse().map_err(|_| "bad rows")?,
        weight: ZoneWeight::FloorArea,
    })
}

fn loader_of(a: &Args) -> Res<LoaderSpec> {
    Ok(match a.get("loader").unwrap_or("queue") {
        "queue" => LoaderSpec::default(),
        "bpr" => LoaderSpec::Bpr {
            saturation_veh_per_h: 1900.0,
            alpha: 0.15,
            beta: 4.0,
        },
        "micro" => LoaderSpec::Micro {
            step_ms: 100,
            seed: 1,
        },
        other => return Err(format!("--loader '{other}' is not queue, bpr or micro")),
    })
}

fn run() -> Res<()> {
    let a = Args::parse()?;
    match a.cmd.as_str() {
        "od2trips" => {
            let w = world(&a)?;
            let zones =
                ZoneSet::build(&w, &zones_of(&a)?, ClassMask::CAR).map_err(|e| e.to_string())?;
            let ms =
                OdMatrix::load(&PathBuf::from(a.need("matrix")?)).map_err(|e| e.to_string())?;
            let p = Od2TripsParams {
                scale: a.num("scale", 1.0)?,
                run_start_s_of_day: a.get("start").map(hhmm).transpose()?.unwrap_or(0.0),
                ..Od2TripsParams::default()
            };
            let mut trips = Vec::new();
            for m in &ms {
                let mut t = od2trips(&w, &zones, m, &p).map_err(|e| e.to_string())?;
                let base = trips.len() as u64;
                t.iter_mut().for_each(|x| x.id += base);
                trips.extend(t);
            }
            let set = TripSet::new(
                world_hash(&w),
                serde_json::json!({"model": v2xw_demand::od::MODEL_ID, "zones": zones.len()}),
                trips,
            );
            set.save(&PathBuf::from(a.need("out")?))
                .map_err(|e| e.to_string())?;
            println!("{} trips over {} zones", set.trips.len(), zones.len());
        }
        "purposes" => {
            let w = world(&a)?;
            let m = PurposeModel::new(
                &w,
                PurposeParams {
                    daily_trips: a.num("daily", 20_000.0)?,
                    ..PurposeParams::default()
                },
            )
            .map_err(|e| e.to_string())?;
            let day = a.num("day", 0.0)? as u64;
            let set = TripSet::new(
                world_hash(&w),
                serde_json::json!({"model": v2xw_demand::purpose::MODEL_ID, "day": day}),
                m.trips_for_day(day),
            );
            set.save(&PathBuf::from(a.need("out")?))
                .map_err(|e| e.to_string())?;
            println!("{} trips on day {day}", set.trips.len());
        }
        "assign" => {
            let w = world(&a)?;
            let set = TripSet::load(&PathBuf::from(a.need("trips")?)).map_err(|e| e.to_string())?;
            set.check_world(&w).map_err(|e| e.to_string())?;
            let choice = match a.get("choice").unwrap_or("gawron") {
                "gawron" => ChoiceSpec::default(),
                "logit" => ChoiceSpec::Logit {
                    theta_per_s: None,
                    perception_cv: 0.1,
                    commonality_beta: 1.0,
                    commonality_gamma: 1.0,
                },
                "msa" => ChoiceSpec::Msa,
                other => return Err(format!("--choice '{other}' is not gawron, logit or msa")),
            };
            let params = AssignParams {
                choice,
                loader: loader_of(&a)?,
                max_iterations: a.num("iterations", 25.0)? as u32,
                gap_tolerance: a.num("gap", 0.02)?,
                ..AssignParams::default()
            };
            let warm = a
                .get("warm")
                .map(|p| RouteFile::load(&PathBuf::from(p)))
                .transpose()
                .map_err(|e| e.to_string())?;
            let r = Assigner::new(&w, params)
                .run(&set.trips, warm.as_ref(), |l| {
                    println!(
                        "iteration {:>3}  relative gap {:.5}  mean travel time {:7.1} s  arrived {:>6}  switched {:5.1}%  alternatives {:.2}",
                        l.iteration,
                        l.relative_gap,
                        l.mean_travel_time_s,
                        l.arrived,
                        100.0 * l.switched_share,
                        l.mean_alternatives
                    );
                })
                .map_err(|e| e.to_string())?;
            r.routes
                .save(&PathBuf::from(a.need("out")?))
                .map_err(|e| e.to_string())?;
            println!(
                "converged: {}  unroutable trips: {}",
                r.converged,
                r.unroutable.len()
            );
        }
        "sample" | "check" => {
            let w = world(&a)?;
            let g = LaneGraph::new(&w, ClassMask::CAR);
            let text = std::fs::read_to_string(a.need("counts")?).map_err(|e| e.to_string())?;
            let placed = counts::place(
                &w,
                &g,
                &counts::parse_counts(&text).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let (trips, routes) = if a.cmd == "sample" {
                let sp = sampler::SamplerParams {
                    method: match a.get("method").unwrap_or("max-entropy") {
                        "greedy" => sampler::SamplerMethod::Greedy,
                        _ => sampler::SamplerMethod::MaxEntropy,
                    },
                    ..sampler::SamplerParams::default()
                };
                let cands = sampler::candidates(&g, &placed, &sp);
                let s = sampler::sample(&g, &placed, &cands, &sp);
                println!(
                    "{} candidate routes, {} trips, uncovered counts {:?}",
                    cands.len(),
                    s.trips.len(),
                    s.uncovered
                );
                let rf = RouteFile::new(
                    world_hash(&w),
                    serde_json::json!({"model": sampler::MODEL_ID}),
                    Vec::new(),
                    true,
                    s.trips
                        .iter()
                        .zip(&s.routes)
                        .map(|(t, r)| RoutedTrip {
                            trip: t.clone(),
                            chosen: 0,
                            alternatives: vec![SavedAlternative {
                                lanes: g.lanes_of(r),
                                cost_s: 0.0,
                                probability: 1.0,
                            }],
                        })
                        .collect(),
                );
                rf.save(&PathBuf::from(a.need("out")?))
                    .map_err(|e| e.to_string())?;
                (s.trips, s.routes)
            } else {
                let rf = RouteFile::load(&PathBuf::from(a.need("routes")?))
                    .map_err(|e| e.to_string())?;
                let trips: Vec<_> = rf.trips.iter().map(|r| r.trip.clone()).collect();
                let routes: Vec<_> = rf
                    .trips
                    .iter()
                    .map(|r| r.route().and_then(|l| g.nodes_of(l)).unwrap_or_default())
                    .collect();
                (trips, routes)
            };
            // Load the network with the routes and count what passes.
            let last = trips.iter().map(|t| t.depart_s).fold(0.0, f64::max);
            let mut loader = loader_of(&a)?.build();
            let l = loader
                .load(
                    &w,
                    &g,
                    &trips,
                    &routes,
                    Window {
                        begin_s: 0.0,
                        end_s: last + 3600.0,
                        bin_s: 300.0,
                    },
                )
                .map_err(|e| e.to_string())?;
            let c = counts::check(&placed, &trips, &routes, &l.times);
            for r in &c.rows {
                println!(
                    "{:<16} t={:>6.0}  observed {:>7.1}  modelled {:>7.1}  GEH {:5.2}",
                    r.id, r.begin_s, r.observed, r.modelled, r.geh
                );
            }
            println!(
                "GEH<5 on {:.1}% of counts (TAG M3.1 needs 85%: {}), RMSE {:.1}%, R² {:.3}",
                100.0 * c.share_geh_below_5,
                if c.passes_tag_m3_1 { "pass" } else { "FAIL" },
                c.rmse_percent,
                c.r_squared
            );
        }
        "gtfs-clip" => {
            let b = bbox(a.need("bbox")?)?;
            let date: u32 = a.need("date")?.parse().map_err(|_| "--date is YYYYMMDD")?;
            let (stops, trips, rows) = v2xw_demand::transit::gtfs::clip(
                &PathBuf::from(a.need("feed")?),
                &PathBuf::from(a.need("out")?),
                &v2xw_demand::transit::gtfs::Filter {
                    bbox: Some(b),
                    date: Some(date),
                    route_types: Vec::new(),
                },
            )
            .map_err(|e| e.to_string())?;
            println!("{stops} stops, {trips} trips, {rows} stop times");
        }
        "gtfs" => {
            let w = world(&a)?;
            let p = TransitParams {
                feed: PathBuf::from(a.need("feed")?),
                start_s_of_day: hhmm(a.get("start").unwrap_or("08:00"))?,
                duration_s: a.num("run", 900.0)?,
                ..TransitParams::default()
            };
            let date: u32 = a.need("date")?.parse().map_err(|_| "--date is YYYYMMDD")?;
            let s = build_schedule(&w, &p, date).map_err(|e| e.to_string())?;
            let mut lines: BTreeMap<String, usize> = BTreeMap::new();
            for t in &s.trips {
                *lines
                    .entry(format!("{} (type {})", t.route, t.route_type))
                    .or_default() += 1;
            }
            println!(
                "{} vehicle trips, {} stops placed, {} patterns unplaced",
                s.trips.len(),
                s.placed_stops,
                s.unplaced_patterns
            );
            for (l, n) in lines {
                println!("  {l:<40} {n:>4} trips");
            }
        }
        other => return Err(format!("unknown command '{other}'")),
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("v2xw-demand: {e}");
        std::process::exit(2);
    }
}
