//! How smoothly people and two-wheelers move, measured on a real scenario.
//!
//! A diagnostic, not a gate (`--ignored`): it loads the scenario named by `V2XW_SCENARIO`
//! (default `scenarios/vru-grid.yaml`), runs its mobility alone for `V2XW_SECONDS`
//! (default 60) at the scenario's own step, and reports what the 3D view would be handed:
//!
//! * per-step jumps (a displacement no walking speed explains),
//! * the published speed against the speed the positions imply,
//! * heading turns per step, and heading spins while standing,
//! * direction reversals (oscillation) and lateral jitter (the second difference of
//!   position across the direction of travel),
//! * the walking-speed distribution, stops and how long they last,
//! * pedestrians closer than two bodies, and two-wheelers' acceleration and lateral jitter.
//!
//! `V2XW_WATCH=<id,id>` prints those actors' traces.

use std::collections::BTreeMap;
use std::path::PathBuf;

use v2xw_core::geom::Vec3;
use v2xw_core::rng::RngRegistry;
use v2xw_engine::Scenario;
use v2xw_mobility::vru::social_force::PedActivity;
use v2xw_mobility::{Mobility, MobilityCtx, VehicleClass};

#[derive(Default)]
struct Track {
    pos: Option<Vec3>,
    vel: Option<Vec3>,
    heading: Option<f64>,
    still_s: f64,
    act: u8,
    lane: Option<u32>,
    act_prev: u8,
}

#[derive(Default)]
struct Tally {
    samples: u64,
    jumps: u64,
    speed_mismatch: u64,
    turn_over_30: u64,
    spin_standing: u64,
    reversals: u64,
    accel_over_3: u64,
    lateral_jitter_over_5cm: u64,
    worst_jump: (f64, u32, f64),
    worst_accel: (f64, u32, f64),
    worst_lat: (f64, u32, f64),
    worst_spin: (f64, u32, f64),
}

fn angle(a: f64) -> f64 {
    let mut d = a % std::f64::consts::TAU;
    if d > std::f64::consts::PI {
        d -= std::f64::consts::TAU;
    }
    if d < -std::f64::consts::PI {
        d += std::f64::consts::TAU;
    }
    d
}

#[test]
#[ignore = "a diagnostic measurement, not a gate"]
fn measure_vru_motion() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = std::env::var("V2XW_SCENARIO").unwrap_or_else(|_| "scenarios/vru-grid.yaml".into());
    let seconds: f64 = std::env::var("V2XW_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60.0);
    let watch: Vec<u32> = std::env::var("V2XW_WATCH")
        .ok()
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_default();
    let watch_t: (f64, f64) = (
        std::env::var("V2XW_WATCH_T0").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        std::env::var("V2XW_WATCH_T1").ok().and_then(|s| s.parse().ok()).unwrap_or(1e9),
    );
    std::env::set_current_dir(&root).unwrap();
    let mut scenario = Scenario::load(root.join(&path)).expect("scenario loads");
    if let Ok(p) = std::env::var("V2XW_PEDESTRIANS") {
        scenario.actors.vru.pedestrians = p.parse().unwrap();
    }
    if let Ok(r) = std::env::var("V2XW_RATE") {
        if let Some(rate) = scenario.actors.vehicles.demand.rate_veh_per_h.as_mut() {
            *rate = r.parse().unwrap();
        }
    }
    let world = v2xw_engine::wiring::build_world(&scenario).expect("world");
    let rng = RngRegistry::new(scenario.seed);
    let mut mobility = v2xw_engine::wiring::native_mobility(&scenario);
    {
        let demand = v2xw_engine::wiring::build_demand(&scenario, &world).expect("demand");
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        mobility.init(&mut ctx, demand).expect("init");
    }
    if let Ok(ids) = std::env::var("V2XW_LANES") {
        for id in ids.split(',').filter_map(|x| x.trim().parse::<u32>().ok()) {
            if let Some(l) = world.try_lane(v2xw_core::ids::LaneId::new(id)) {
                let a = l.point_at(0.0);
                let b = l.point_at(l.length_m);
                println!(
                    "lane {id} {:?} len {:.2} width {:.2} from ({:.2},{:.2}) to ({:.2},{:.2}) succ {:?}",
                    l.kind,
                    l.length_m,
                    l.width_m,
                    a.x,
                    a.y,
                    b.x,
                    b.y,
                    world.successor_lanes(l.id).iter().map(|x| x.index()).collect::<Vec<_>>()
                );
            }
        }
    }
    let step = scenario.time.mobility_step();
    let dt = step.as_secs_f64();
    let mut classes: BTreeMap<u32, VehicleClass> = BTreeMap::new();
    let mut tracks: BTreeMap<u32, Track> = BTreeMap::new();
    let mut tallies: BTreeMap<&'static str, Tally> = BTreeMap::new();
    let mut walk_speeds: Vec<f64> = Vec::new();
    let mut stops: Vec<(f64, u32, u8)> = Vec::new();
    let mut overlaps = 0u64;
    let mut overlap_pairs = 0u64;
    let mut off_walk = 0u64;
    let mut activity_counts: BTreeMap<u8, u64> = BTreeMap::new();
    let mut accel_why: BTreeMap<String, u64> = BTreeMap::new();
    let mut overlap_why: BTreeMap<String, u64> = BTreeMap::new();
    let mut overlap_run: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut overlap_longest = (0u32, 0u32, 0u32);
    let mut t = 0u64;
    let end = (seconds * 1e9) as u64;
    let wall = std::time::Instant::now();
    while t < end {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = mobility.step(&mut ctx, step);
        t = update.t;
        let ts = t as f64 / 1e9;
        for s in &update.spawned {
            classes.insert(s.actor.index(), s.class);
        }
        for (d, _) in &update.despawned {
            if let Some(tr) = tracks.remove(&d.index())
                && tr.still_s >= 5.0
            {
                stops.push((tr.still_s, d.index(), tr.act));
            }
        }
        let mut peds: Vec<(u32, Vec3, Option<u32>, u8, Option<(u32, f64, f64)>)> = Vec::new();
        // Last step's pedestrian positions by 1 m cell, to count who was near an event.
        let mut cells: BTreeMap<(i64, i64), Vec<Vec3>> = BTreeMap::new();
        for (tid, tr) in &tracks {
            if classes.get(tid) == Some(&VehicleClass::Pedestrian)
                && let Some(p) = tr.pos
            {
                cells.entry((p.x.floor() as i64, p.y.floor() as i64)).or_default().push(p);
            }
        }
        for (actor, k) in &update.states {
            let id = actor.index();
            let class = classes.get(&id).copied().unwrap_or_default();
            let kind = match class {
                VehicleClass::Pedestrian => "pedestrian",
                VehicleClass::Bicycle | VehicleClass::Scooter => "cyclist",
                VehicleClass::Motorcycle | VehicleClass::Moped => "motorcycle",
                _ => "vehicle",
            };
            let person = if kind == "pedestrian" {
                mobility.vru().and_then(|v| v.get(*actor)).cloned()
            } else {
                None
            };
            let act = person.as_ref().map_or(0, |p| p.activity.code());
            if kind == "pedestrian" {
                *activity_counts.entry(act).or_default() += 1;
                peds.push((id, k.pos, person.as_ref().and_then(|p| p.group), act, k.lane.map(|l| (l.lane.index(), l.s_m, l.d_m))));
                if k.lane.is_none() && !person.as_ref().is_some_and(|p| p.crossing_midblock()) {
                    off_walk += 1;
                }
            }
            let tally = tallies.entry(kind).or_default();
            let tr = tracks.entry(id).or_default();
            let speed = k.vel.norm_2d();
            if watch.contains(&id) && ts >= watch_t.0 && ts <= watch_t.1 {
                println!(
                    "watch {id} t={ts:.1} pos=({:.2},{:.2}) v={speed:.2} hdg={:.1} lane={:?} act={act}",
                    k.pos.x,
                    k.pos.y,
                    k.heading_rad.to_degrees(),
                    k.lane.map(|l| (l.lane.index(), (l.s_m * 10.0).round() / 10.0, (l.d_m * 100.0).round() / 100.0))
                );
            }
            if let (Some(p0), Some(h0)) = (tr.pos, tr.heading) {
                tally.samples += 1;
                let disp = k.pos - p0;
                let d = disp.norm_2d();
                if kind == "pedestrian" && d > 4.0 {
                    println!(
                        "jump {id} t {ts:.1} {d:.1} m: lane {:?} -> {:?} act {}->{act}",
                        tr.lane,
                        k.lane.map(|l| (l.lane.index(), l.s_m, l.d_m)),
                        tr.act_prev
                    );
                }
                let implied = d / dt;
                let vmax = match kind {
                    "pedestrian" => 2.6,
                    "cyclist" => 8.0,
                    _ => 40.0,
                };
                if implied > vmax * 1.2 {
                    tally.jumps += 1;
                    if d > tally.worst_jump.0 {
                        tally.worst_jump = (d, id, ts);
                    }
                }
                if (implied - speed).abs() > 0.5 + 0.2 * speed {
                    tally.speed_mismatch += 1;
                }
                let dh = angle(k.heading_rad - h0).abs().to_degrees();
                if speed < 0.1 && dh > 20.0 {
                    tally.spin_standing += 1;
                    if dh > tally.worst_spin.0 {
                        tally.worst_spin = (dh, id, ts);
                    }
                } else if dh > 30.0 {
                    tally.turn_over_30 += 1;
                }
                let v_now = disp.scale(1.0 / dt);
                if let Some(v0) = tr.vel {
                    let a = (v_now - v0).norm_2d() / dt;
                    let a_lim = if kind == "pedestrian" { 3.0 } else { 5.0 };
                    if a > a_lim {
                        if kind == "pedestrian" {
                            let lc = tr.lane != k.lane.map(|l| l.lane.index());
                            let mut near = 0usize;
                            let (cx, cy) = (p0.x.floor() as i64, p0.y.floor() as i64);
                            for dx in -1..=1 {
                                for dy in -1..=1 {
                                    if let Some(v) = cells.get(&(cx + dx, cy + dy)) {
                                        near += v.iter().filter(|q| q.distance_2d(p0) < 1.0).count();
                                    }
                                }
                            }
                            let near = near.saturating_sub(1).min(3);
                            let key = format!("act {}->{act} lanechange {lc} near {near}", tr.act_prev);
                            let n = accel_why.entry(key.clone()).or_default();
                            *n += 1;
                            if *n <= 2 && a > 6.0 {
                                println!("accel example {key}: id {id} t {ts:.1} a {a:.1} speed {speed:.2}");
                            }
                        }
                        tally.accel_over_3 += 1;
                        if a > tally.worst_accel.0 {
                            tally.worst_accel = (a, id, ts);
                        }
                    }
                    if v0.norm_2d() > 0.3 && v_now.norm_2d() > 0.3 && v0.dot(v_now) < 0.0 {
                        tally.reversals += 1;
                    }
                    // Lateral jitter: the change of velocity across the previous direction.
                    if v0.norm_2d() > 0.3 {
                        let n = Vec3::new_2d(-v0.y, v0.x).scale(1.0 / v0.norm_2d());
                        let lat = (v_now - v0).dot(n).abs() * dt;
                        if lat > 0.05 {
                            tally.lateral_jitter_over_5cm += 1;
                            if lat > tally.worst_lat.0 {
                                tally.worst_lat = (lat, id, ts);
                            }
                        }
                    }
                }
                tr.vel = Some(v_now);
                if kind == "pedestrian" && act == PedActivity::Walking.code() && speed > 0.2 {
                    walk_speeds.push(implied);
                }
                if implied < 0.1 {
                    tr.still_s += dt;
                    tr.act = act;
                } else {
                    if tr.still_s >= 5.0 {
                        stops.push((tr.still_s, id, tr.act));
                    }
                    tr.still_s = 0.0;
                }
            }
            tr.pos = Some(k.pos);
            tr.heading = Some(k.heading_rad);
            tr.lane = k.lane.map(|l| l.lane.index());
            tr.act_prev = act;
        }
        // Overlaps: pedestrians closer than 0.35 m, centre to centre (a body is ~0.5 m).
        peds.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        let mut any = false;
        for i in 0..peds.len() {
            for j in i + 1..peds.len() {
                if peds[j].1.x - peds[i].1.x > 0.35 {
                    break;
                }
                let dist = peds[i].1.distance_2d(peds[j].1);
                if dist < 0.35 {
                    overlap_pairs += 1;
                    any = true;
                    let (a, b) = (&peds[i], &peds[j]);
                    let same_group = a.2.is_some() && a.2 == b.2;
                    let same_lane = a.4.zip(b.4).is_some_and(|(x, y)| x.0 == y.0);
                    let key = format!(
                        "d<{} group {} lane {} acts {}/{}",
                        if dist < 0.05 { "0.05" } else if dist < 0.2 { "0.2" } else { "0.35" },
                        same_group,
                        same_lane,
                        a.3.min(b.3),
                        a.3.max(b.3)
                    );
                    *overlap_why.entry(key).or_default() += 1;
                    let pk = (a.0.min(b.0), a.0.max(b.0));
                    let n = overlap_run.entry(pk).or_default();
                    *n += 1;
                    if *n > overlap_longest.0 {
                        overlap_longest = (*n, pk.0, pk.1);
                    }
                    if overlap_pairs <= 3 {
                        println!(
                            "overlap t={ts:.1} {} and {} at ({:.1},{:.1}) groups {:?}/{:?}",
                            peds[i].0, peds[j].0, peds[i].1.x, peds[i].1.y, peds[i].2, peds[j].2
                        );
                    }
                }
            }
        }
        if any {
            overlaps += 1;
        }
    }
    for (id, tr) in &tracks {
        if tr.still_s >= 5.0 {
            stops.push((tr.still_s, *id, tr.act));
        }
    }
    println!(
        "scenario {path}: {seconds} s in {:.1} s wall, step {dt} s",
        wall.elapsed().as_secs_f64()
    );
    for (kind, t) in &tallies {
        if t.samples == 0 {
            continue;
        }
        let per_k = |n: u64| n as f64 * 1000.0 / t.samples as f64;
        println!(
            "{kind:10} samples {:8} | jumps {:.2}/k | spd-mismatch {:.2}/k | turn>30 {:.2}/k | spin-standing {:.2}/k | reversals {:.2}/k | accel>lim {:.2}/k | lat>5cm {:.2}/k",
            t.samples,
            per_k(t.jumps),
            per_k(t.speed_mismatch),
            per_k(t.turn_over_30),
            per_k(t.spin_standing),
            per_k(t.reversals),
            per_k(t.accel_over_3),
            per_k(t.lateral_jitter_over_5cm)
        );
        println!(
            "{kind:10} worst jump {:.2} m (id {} t {:.1}) | accel {:.1} (id {} t {:.1}) | lat {:.3} m (id {} t {:.1}) | spin {:.0} deg (id {} t {:.1})",
            t.worst_jump.0, t.worst_jump.1, t.worst_jump.2,
            t.worst_accel.0, t.worst_accel.1, t.worst_accel.2,
            t.worst_lat.0, t.worst_lat.1, t.worst_lat.2,
            t.worst_spin.0, t.worst_spin.1, t.worst_spin.2
        );
    }
    if !walk_speeds.is_empty() {
        walk_speeds.sort_by(f64::total_cmp);
        let n = walk_speeds.len();
        let mean = walk_speeds.iter().sum::<f64>() / n as f64;
        let sd = (walk_speeds.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n as f64).sqrt();
        println!(
            "walking speed (implied, walking & moving): mean {mean:.2} sd {sd:.2} p5 {:.2} p50 {:.2} p95 {:.2} m/s",
            walk_speeds[n / 20],
            walk_speeds[n / 2],
            walk_speeds[n * 19 / 20]
        );
    }
    let total: u64 = activity_counts.values().sum();
    println!(
        "activity share: {}",
        activity_counts
            .iter()
            .map(|(a, c)| format!("{a}:{:.1}%", *c as f64 * 100.0 / total.max(1) as f64))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("pedestrian steps off every walkable lane (not mid-block): {off_walk}");
    println!("steps with a pedestrian overlap (<0.35 m): {overlaps}, pairs {overlap_pairs}, distinct pairs {}, longest {} steps ({} & {})", overlap_run.len(), overlap_longest.0, overlap_longest.1, overlap_longest.2);
    let mut ow: Vec<_> = overlap_why.into_iter().collect();
    ow.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, n) in ow.iter().take(8) {
        println!("  overlap {k}: {n}");
    }
    let mut aw: Vec<_> = accel_why.into_iter().collect();
    aw.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, n) in aw.iter().take(8) {
        println!("  accel>lim {k}: {n}");
    }
    stops.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut by_kind: BTreeMap<&str, (u64, f64)> = BTreeMap::new();
    for (s, id, _) in &stops {
        let kind = match classes.get(id).copied().unwrap_or_default() {
            VehicleClass::Pedestrian => "pedestrian",
            VehicleClass::Bicycle | VehicleClass::Scooter => "cyclist",
            VehicleClass::Motorcycle | VehicleClass::Moped => "motorcycle",
            _ => "vehicle",
        };
        let e = by_kind.entry(kind).or_default();
        e.0 += 1;
        e.1 = e.1.max(*s);
    }
    println!("stops >= 5 s by kind (count, longest): {by_kind:?}");
    for (s, id, act) in stops.iter().take(8) {
        println!("  long stop {s:.1} s actor {id} {:?} activity {act}", classes.get(id));
    }
}
