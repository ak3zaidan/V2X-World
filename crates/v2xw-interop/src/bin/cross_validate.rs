//! `cross_validate`: runs the SUMO cross-validation and writes
//! `docs/validation/cross-validation.md`.
//!
//! ```text
//! cross_validate [--quick] [--no-radio] [--out FILE] [--osm FILE] [--work DIR]
//!                [--case NAME@RATE] [--no-idm]
//! ```
//!
//! SUMO is found through `$SUMO_HOME`, else `$V2XW_SUMO_VENV` (a virtualenv with the
//! `eclipse-sumo` wheel installed). Without SUMO the traffic sections say so and only the
//! radio section is produced.

use std::fmt::Write as _;
use std::path::PathBuf;

use v2xw_core::time::Duration;
use v2xw_interop::xval::{
    self, Measured, Study, Timing, edge_agreement, make_trips, paired_tt, run_native, run_sumo,
};

struct Args {
    quick: bool,
    radio: bool,
    out: PathBuf,
    osm: PathBuf,
    work: PathBuf,
    case: Option<String>,
    no_idm: bool,
}

fn args() -> Args {
    let mut a = Args {
        quick: false,
        radio: true,
        out: PathBuf::from("docs/validation/cross-validation.md"),
        osm: PathBuf::from("worlds/cache/manhattan.osm.xml"),
        work: std::env::temp_dir().join("v2xw-xval"),
        case: None,
        no_idm: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--quick" => a.quick = true,
            "--no-radio" => a.radio = false,
            "--case" => a.case = it.next(),
            "--no-idm" => a.no_idm = true,
            "--out" => a.out = it.next().map(PathBuf::from).unwrap_or(a.out),
            "--osm" => a.osm = it.next().map(PathBuf::from).unwrap_or(a.osm),
            "--work" => a.work = it.next().map(PathBuf::from).unwrap_or(a.work),
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    a
}

fn f(x: f64, p: usize) -> String {
    if x.is_finite() {
        format!("{x:.p$}")
    } else {
        "—".to_string()
    }
}

fn rel(a: f64, b: f64) -> String {
    if a.is_finite() && b.is_finite() && b.abs() > 1e-9 {
        format!("{:+.0} %", (a / b - 1.0) * 100.0)
    } else {
        "—".to_string()
    }
}

struct Case {
    study_name: String,
    rate: f64,
    runs: Vec<Measured>,
}

fn metrics_table(s: &mut String, runs: &[Measured]) {
    let _ = write!(s, "| measure |");
    for r in runs {
        let _ = write!(s, " {} |", r.label);
    }
    let _ = writeln!(s, " native vs SUMO-Krauss |");
    let _ = write!(s, "|---|");
    for _ in runs {
        let _ = write!(s, "---:|");
    }
    let _ = writeln!(s, "---:|");
    type Get = fn(&Measured) -> f64;
    let rows: [(&str, Get, usize); 21] = [
        ("trips offered", |m| m.offered as f64, 0),
        ("trips inserted", |m| m.inserted as f64, 0),
        ("trips completed", |m| m.completed as f64, 0),
        ("flow q, veh/h/lane (Edie)", |m| m.flow(), 1),
        ("density k, veh/km/lane (Edie)", |m| m.density(), 2),
        ("space-mean speed, km/h", |m| m.speed_kmh(), 1),
        ("mean travel time, s", |m| m.mean_tt(), 1),
        ("median travel time, s", |m| m.tt_pct(0.5), 1),
        ("85th pct travel time, s", |m| m.tt_pct(0.85), 1),
        (
            "mean insertion delay, s",
            |m| xval_mean(&m.insert_delay_s),
            2,
        ),
        ("mean vehicles halting (< 0.1 m/s)", |m| m.halting_mean, 1),
        ("max vehicles halting", |m| m.halting_max as f64, 0),
        ("halting time per trip, s", |m| m.wait_per_trip_s, 1),
        (
            "  of which at a stop line (last 30 m), s",
            |m| m.halt_where_s[0] / m.inserted.max(1) as f64,
            1,
        ),
        (
            "  of which mid-block, s",
            |m| m.halt_where_s[1] / m.inserted.max(1) as f64,
            1,
        ),
        (
            "  of which inside a junction, s",
            |m| m.halt_where_s[2] / m.inserted.max(1) as f64,
            1,
        ),
        ("stops per trip", |m| m.stops_per_trip(), 2),
        (
            "lane changes per vehicle-km",
            |m| m.lane_changes_per_km(),
            2,
        ),
        ("junction speed (moving), km/h", |m| m.junction_kmh(), 1),
        ("queue discharge headway, s", |m| m.discharge_headway_s(), 2),
        (
            "vehicles left in network at end",
            |m| m.left_in_network as f64,
            0,
        ),
    ];
    for (name, g, p) in rows {
        let _ = write!(s, "| {name} |");
        for r in runs {
            let _ = write!(s, " {} |", f(g(r), p));
        }
        let cmp = if runs.len() > 1 {
            rel(g(&runs[0]), g(&runs[1]))
        } else {
            "—".into()
        };
        let _ = writeln!(s, " {cmp} |");
    }
    let _ = write!(s, "| wall-clock, s |");
    for r in runs {
        let _ = write!(s, " {} |", f(r.wall_s, 1));
    }
    let _ = writeln!(s, " |");
}

fn xval_mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn main() {
    let a = args();
    let home = v2xw_interop::sumo_home();
    let timing = if a.quick {
        Timing {
            step: Duration::from_millis(100),
            demand_s: 300.0,
            total_s: 480.0,
            warm_s: 120.0,
        }
    } else {
        Timing {
            step: Duration::from_millis(100),
            demand_s: 900.0,
            total_s: 1200.0,
            warm_s: 180.0,
        }
    };
    let seed = 20_261_007;
    let mut doc = String::new();
    let _ = writeln!(doc, "# Cross-validation against SUMO\n");
    let _ = writeln!(
        doc,
        "Generated by `just cross-validate{}` (`cargo run -p v2xw-interop --bin cross_validate`). \
         Regenerate, do not edit. Seed {seed}; mobility step {} ms in both simulators; \
         demand for {:.0} s, {:.0} s simulated, measurement window {:.0}–{:.0} s.\n",
        if a.quick { " --quick" } else { "" },
        timing.step.as_nanos() / 1_000_000,
        timing.demand_s,
        timing.total_s,
        timing.warm_s,
        timing.demand_s
    );
    doc.push_str(METHOD);

    let mut cases: Vec<Case> = Vec::new();
    let mut sumo_version = None;
    let mut setup_notes = Vec::new();
    match &home {
        None => setup_notes.push(
            "SUMO was not found (`$SUMO_HOME` or `$V2XW_SUMO_VENV`): the traffic comparison was not run.".to_string(),
        ),
        Some(home) => {
            let mut studies: Vec<(Study, Vec<f64>)> = Vec::new();
            match xval::grid_study(home, &a.work) {
                Ok(s) => studies.push((s, if a.quick { vec![600.0, 2400.0] } else { vec![600.0, 1800.0, 3600.0] })),
                Err(e) => setup_notes.push(format!("grid: {e}")),
            }
            if a.osm.exists() {
                match xval::manhattan_study(home, &a.osm, &a.work, MANHATTAN_BBOX) {
                    Ok(s) => studies.push((s, if a.quick { vec![1200.0] } else { vec![900.0, 2400.0, 3600.0] })),
                    Err(e) => setup_notes.push(format!("manhattan: {e}")),
                }
            } else {
                setup_notes.push(format!("{} is missing: the Manhattan study was not run (worlds/fetch.sh fetches it).", a.osm.display()));
            }
            for (study, rates) in &studies {
                let _ = writeln!(
                    doc,
                    "\n### Network `{}`\n\n{}; imported: {} normal edges, {} lanes in total, {:.2} lane-km; {} importer anomalies.\n",
                    study.name,
                    study.provenance,
                    study.ids.edge_id.len(),
                    study.world.roads.lanes().len(),
                    study.world.roads.lanes().iter().filter(|l| l.kind.is_motorised()).map(|l| l.length_m).sum::<f64>() / 1000.0,
                    study.import.total_anomalies()
                );
                for rate in rates {
                    if a.case.as_ref().is_some_and(|c| *c != format!("{}@{rate}", study.name)) {
                        continue;
                    }
                    let trips = make_trips(&study.world, &study.ids, *rate, timing.demand_s, seed);
                    eprintln!("{} @ {rate} veh/h: {} trips", study.name, trips.len());
                    let native = run_native(study, &trips, timing, seed);
                    eprintln!("  native done in {:.0} s", native.wall_s);
                    let (krauss, v) = run_sumo(study, home, &trips, timing, seed, "SUMO Krauss (default)", &[]);
                    sumo_version = sumo_version.or(v);
                    eprintln!("  sumo krauss done in {:.0} s", krauss.wall_s);
                    if std::env::var_os("V2XW_XVAL_OUTLIERS").is_some() {
                        let mut v: Vec<(f64, u64, f64, f64)> = native
                            .travel_s
                            .iter()
                            .filter_map(|(q, a)| krauss.travel_s.get(q).map(|b| (a - b, *q, *a, *b)))
                            .collect();
                        v.sort_by(|x, y| y.0.total_cmp(&x.0));
                        for q in krauss.travel_s.keys().filter(|q| !native.travel_s.contains_key(q)).take(8) {
                            eprintln!("  outlier trip {q}: not completed natively");
                        }
                        for (d, q, a, b) in v.iter().take(8) {
                            eprintln!("  outlier trip {q}: native {a:.0} s, krauss {b:.0} s, +{d:.0} s");
                        }
                    }
                    let mut runs = vec![native, krauss];
                    if !a.no_idm {
                        let (idm, _) = run_sumo(study, home, &trips, timing, seed, "SUMO IDM", &["--default.carfollowmodel", "IDM"]);
                        eprintln!("  sumo idm done in {:.0} s", idm.wall_s);
                        runs.push(idm);
                    }
                    cases.push(Case { study_name: study.name.clone(), rate: *rate, runs });
                }
            }
            if let Some(v) = &sumo_version {
                setup_notes.push(format!("SUMO: {v}, driven over TraCI by `mobility/sumo-traci`."));
            }
            match xval::export_round_trip(home, &a.work) {
                Ok(rt) => {
                    let _ = writeln!(
                        doc,
                        "\n### Network export round trip\n\nThis engine's procedural TR 36.885 3 × 3 grid exported to plain XML, built by netconvert, and imported back: \
                         normal edges {} → {}, normal lanes {} → {}, normal-lane length {:.3} → {:.3} km, signalised junctions {} → {}. \
                         Lane lengths change because netconvert recomputes junction shapes and trims edges back to them; signal programmes are regenerated by netconvert (not exported).\n",
                        rt.edges.0, rt.edges.1, rt.lanes_before, rt.lanes_after, rt.km_before, rt.km_after, rt.signals.0, rt.signals.1
                    );
                }
                Err(e) => setup_notes.push(format!("export round trip: {e}")),
            }
        }
    }
    let mut body = String::new();
    let _ = writeln!(body, "\n## Traffic results\n");
    for n in &setup_notes {
        let _ = writeln!(body, "- {n}");
    }
    if !cases.is_empty() {
        // The headline, over every case: this engine against SUMO's default.
        let mut flow = (f64::MAX, f64::MIN);
        let mut speed = (f64::MAX, f64::MIN);
        let mut geh_min = f64::MAX;
        let mut r_min = f64::MAX;
        let mut tt = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for c in cases.iter().filter(|c| c.runs.len() >= 2) {
            let (a, b) = (&c.runs[0], &c.runs[1]);
            let df = (a.flow() / b.flow() - 1.0) * 100.0;
            let dv = (a.speed_kmh() / b.speed_kmh() - 1.0) * 100.0;
            flow = (flow.0.min(df), flow.1.max(df));
            speed = (speed.0.min(dv), speed.1.max(dv));
            let (share, _, r, _) = edge_agreement(a, b);
            geh_min = geh_min.min(share * 100.0);
            r_min = r_min.min(r);
            let (_, mr, md) = paired_tt(a, b);
            tt = (tt.0.min(mr), tt.1.max(mr), tt.2.min(md), tt.3.max(md));
        }
        let _ = writeln!(
            body,
            "\n**Summary, this engine against SUMO's default model, over all {} cases:** network flow {:+.0} to {:+.0} %, space-mean speed {:+.0} to {:+.0} %; at least {:.0} % of edges within GEH 5 (acceptance: 85 %); edge-speed correlation at least {:.2}; paired travel-time ratio {:.2}–{:.2} on the mean and {:.2}–{:.2} on the median.\n",
            cases.len(),
            flow.0,
            flow.1,
            speed.0,
            speed.1,
            geh_min,
            r_min,
            tt.0,
            tt.1,
            tt.2,
            tt.3
        );
    }
    for c in &cases {
        let _ = writeln!(
            body,
            "\n### {} at {:.0} veh/h offered\n",
            c.study_name, c.rate
        );
        metrics_table(&mut body, &c.runs);
        for r in &c.runs {
            if let Some(e) = &r.error {
                let _ = writeln!(body, "\n- {}: {e}", r.label);
            }
        }
        if c.runs.len() >= 2 {
            let _ = writeln!(
                body,
                "\n| pair | edges | edges with GEH < 5 | edge-speed Pearson r | mean abs edge-speed diff, km/h | trips completed by both | mean TT ratio | median TT ratio |"
            );
            let _ = writeln!(body, "|---|---:|---:|---:|---:|---:|---:|---:|");
            for (i, j) in [(0, 1), (0, 2), (2, 1)]
                .into_iter()
                .filter(|p| p.0.max(p.1) < c.runs.len())
            {
                let (share, n, r, mad) = edge_agreement(&c.runs[i], &c.runs[j]);
                let (np, mr, md) = paired_tt(&c.runs[i], &c.runs[j]);
                let _ = writeln!(
                    body,
                    "| {} vs {} | {n} | {} % | {} | {} | {np} | {} | {} |",
                    c.runs[i].label,
                    c.runs[j].label,
                    f(share * 100.0, 0),
                    f(r, 2),
                    f(mad, 1),
                    f(mr, 2),
                    f(md, 2)
                );
            }
            let _ = writeln!(
                body,
                "\nVehicles in the network per 2-minute bin (native / SUMO Krauss / SUMO IDM): {}\n",
                (0..c.runs[0].bins.len())
                    .step_by(2)
                    .map(|k| {
                        let g =
                            |m: &Measured| m.bins.get(k).map_or("—".to_string(), |b| f(b.0, 0));
                        c.runs.iter().map(g).collect::<Vec<_>>().join("/")
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    doc.push_str(&body);
    doc.push_str(EXPLANATIONS);
    if a.radio {
        let _ = writeln!(
            doc,
            "\n## Radio: delivery versus distance against Veins, OpenCV2X/Artery and Molina-Masegosa\n"
        );
        doc.push_str(RADIO_INTRO);
        let rows = v2xw_interop::radio::run_all(a.quick);
        doc.push_str(&v2xw_interop::radio::markdown(&rows));
        doc.push_str(RADIO_EXPLAIN);
    }
    if let Some(dir) = a.out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&a.out, doc).expect("write report");
    eprintln!("wrote {}", a.out.display());
}

/// Midtown: about 1.1 km by 1.1 km around Herald Square and Bryant Park (32nd to 42nd
/// Street, 5th to 8th Avenue).
const MANHATTAN_BBOX: [f64; 4] = [-73.9920, 40.7480, -73.9790, 40.7580];

const METHOD: &str = "## Method\n\n\
Both simulators drive **the same network, the same trips and the same routes**, and are measured by **the same code**:\n\n\
1. The network is SUMO's own (`netgenerate` grid; `netconvert` on the cached OpenStreetMap extract of Midtown Manhattan). This engine imports that `net.xml` (`v2xw_world::sumo`), so lanes correspond one to one.\n\
2. Trips are drawn once (exponential headways, uniform origins and destinations over passenger edges, routes of at least 300 m) and routed once by this engine's free-flow Dijkstra.\n\
3. This engine drives them with its native mobility (shipped defaults); SUMO drives them through the `mobility/sumo-traci` plug-in (`crates/v2xw-interop/src/sumo.rs`), which inserts each trip over TraCI with its route, at the same instant, lane and position. SUMO is run twice: its default Krauss model, and IDM.\n\
4. One observer (`crates/v2xw-interop/src/xval.rs`) reads the published states of both: Edie's generalised flow, density and speed over the measurement window and the whole network's lane length; travel time from insertion to arrival, paired by trip; vehicles below 0.1 m/s (SUMO's halting threshold) as the queue; per-edge entries compared with the GEH statistic (acceptance customarily GEH < 5 on 85 % of links, UK DMRB Vol. 12).\n\n";

const EXPLANATIONS: &str = "\n## Why the simulators differ\n\n\
Network, trips, routes, signal programmes and measurement are shared, so what remains is the driving models. Two calibrations of the comparison come first:\n\n\
- **The same simulator with a different car-following equation moves the results by a few per cent.** SUMO-IDM against SUMO-Krauss (rows above) agrees within about 1–4 % on speed and travel time, with edge-speed correlation 0.95–0.99. Where this engine differs from SUMO by more than that, the difference is not the car-following equation alone: it is the rest of the driver model (lane changing, junction behaviour, insertion).\n\
- **Mean and median travel-time ratios separate a tail from a shift.** Where the paired mean ratio is well above the median ratio, most trips take about as long in both simulators and a minority take much longer in one of them.\n\n\
The mechanisms that can produce the observed differences, with what each predicts:\n\n\
- **Lane changing before turns: found and fixed (2026-10-11).** The travel-time tail was traced trip by trip (`V2XW_XVAL_OUTLIERS`, `V2XW_XVAL_TRACE`). Three defects of this engine, each fixed: (1) after a speed-gain change the route was re-planned on the lane graph, which has no lane changes, so a car that could not make a later turn from its new lane went round the block (296 s against SUMO's 109 s on one grid trip); (2) two cars level with each other at the same speed stayed level to the end of the lane, so the one that needed the other's lane missed its turn or was removed; it now drops behind, as LC2013's `informLeader` does (LC2013's other courtesy, the target-lane follower braking for an urgent change, `informFollower`, was tried and removed: it held queues back across the junction upstream and locked Manhattan at 3600 veh/h); (3) a car owing a change it could not make drove past the end of its lane and was removed, where SUMO waits there; it now brakes for the end of its lane from the start of the lane, and its give-up re-plan accepts any lane of the destination road. A lane change near the start of a road now also sees the car on the junction connector behind it in the target lane (the traffic validation's safety classes caught a cut-in there to a net gap of -0.05 m). With these, and with the native run on a scenario's shipped defaults (strategic lane choice on), lane changes per vehicle-km agree with SUMO within about 20 %, and the grid's paired mean travel-time ratio at 3600 veh/h fell from 1.21 to 1.09.\n\
- **Junctions.** Both run netconvert's signal programmes from the same `net.xml`, but yield behaviour differs: SUMO uses its junction model with impatience and look-ahead over internal lanes; this engine uses gap acceptance on the connector lanes the importer builds. The traffic validation report records this engine's permitted-left critical gap by Raff's method at 6.8 s against the HCM's 4.5 s (`docs/validation/traffic.md`, `calibration/permitted-left-gap`; the decision rule itself uses 4.5 s, and the estimate rests on 21 accepted lags, three long rejections deciding it), which lengthens left-turn delays in this engine specifically. Moving through a junction this engine is 1-14 % faster than SUMO Krauss (the junction-speed row): both cap a turn at sqrt(5.5 m/s² · R) over the connector's average radius (netconvert `--junctions.limit-turn-speed`), and the apex of a curved connector is then taken at about 7.7 m/s² (85th percentile, `calibration/turning`). With the opt-in `traffic.turn_curve_window_m` at 6 m (radius judged over the tightest 6 m; `V2XW_XVAL_TURN_WINDOW_M=6` here) the grid's junction-speed gap falls to +2 to +7 %, Manhattan's moves to -2 to -6 %, and the space-mean speed difference widens by up to 2 points (2026-10-13 run).\n\
- **Car following.** SUMO's default Krauss has driver imperfection σ = 0.5 (SUMO vType defaults: accel 2.6 m/s², decel 4.5 m/s², τ = 1 s, minGap 2.5 m). The queue-discharge row is where most of the remaining travel-time gap comes from: Krauss discharges a queue a few tenths of a second per vehicle faster than this engine, while SUMO's own IDM discharges at about this engine's rate, so the residual is mostly the car-following equation, not this engine's junctions or lane changing. **Here SUMO's default is the less realistic one:** field discharge headways are 3.8, 3.1, 2.7, 2.4 and 2.2 s for queue positions 1-5 (Greenshields 1947), settling near the HCM's 2.1 s in a CBD (1,900 pc/h/ln × 0.90), and this engine's saturation-flow check measures 2.13 s with a 2.85 s start-up lost time against those (`docs/validation/traffic.md`, `calibration/saturation-flow`). Krauss's quicker discharge, and its full stops where this engine's queues creep (more stops per trip and more halting time per trip in the tables above), follow from its safe-speed rule and dawdling, not from data.\n\
- **Desired speed.** Both draw each passenger car's speed factor from N(1, 0.1) of the lane limit, bounded to 0.2-2 (SUMO `speedFactor` default `normc(1,0.1,0.2,2)`; this engine's `DriverTraits::speed_factor`), so free-flow link speeds match; the speed difference is in queues and at junctions.\n\
- **Insertion.** SUMO inserts at the highest safe speed (`departSpeed=max`); this engine inserts at rest and waits longer for space (the insertion-delay row, under a second). Starting from rest is most of this engine's mid-block halting row (about 1.4 s per trip on the lightly loaded grid, where SUMO has none).\n\
- **Manhattan at 3600 veh/h: spill-back found and fixed (2026-10-11).** It was the one case where this engine was clearly worse than both SUMO models (327 trips completed against SUMO Krauss's 368, 69 vehicles left at the end against 32). The worst trips stood 200-300 s at single places: mid-link queues on ~62 m links and a car on a junction connector, i.e. spill-back locking junctions. Ablated feature by feature, the cause was this engine's follower courtesy for urgent lane changes: the car behind braked for a car waiting to change in beside it, and on short links the queue behind it reached back into the junction upstream. With the courtesy removed (SUMO's own leader-side fall-back kept) this engine completes 376 trips against SUMO Krauss's 368 and leaves 24 vehicles against 32 (the Manhattan 3600 rows above).\n\
- **Teleports.** SUMO removes a vehicle stuck for 300 s (`--time-to-teleport` default 300 s), and this engine does the same with the same 300 s (a scenario's `traffic.gridlock_timeout_s` default). Before the spill-back fix only Manhattan at 3600 veh/h reached it (one traced native car stood 300 s and was removed).\n\
- **Reference point.** Positions are compared at the rear bumper in both (SUMO's front bumper minus vehicle length); this shifts per-edge entry times by under a second and nothing else.\n";

const RADIO_INTRO: &str = "This engine's 802.11p and LTE-V2X Mode 4 stacks are run through the radio crate's highway sweep harness (`v2xw_radio::sweep`) in each study's configuration. Published curves are figures, not tables, so every reference value below was **digitised**: the Veins curve by colour-tracing the rendered figure (±0.01), the OpenCV2X curves by reading the plotted markers (±0.02). OpenCV2X is built on Artery, so it stands in for Artery here.\n\n";

const RADIO_EXPLAIN: &str = "\n### Why the radio curves differ\n\n\
- **Channel.** All three studies used WINNER+ B1. This workspace has no calibrated WINNER+ B1 (04-models.md §3.3: the WINNER II D1.1.2 coefficients are not in the cache), only a shaped stand-in (`winner-b1-shaped-todo-calibrate`); the fully cited TR 37.885 highway-LOS channel is shown beside it so the channel's share of the difference is visible. The range at which delivery collapses is set almost entirely by the path-loss slope and breakpoint, which is exactly the uncalibrated part.\n\
- **Geometry.** The harness is a ring with 3 lanes per direction; Veins used 2 lanes per direction on a 5 km straight road, OpenCV2X 3 per direction over 2 km. At equal veh/km the per-lane spacing and so the interferer geometry differ.\n\
- **Error model.** Veins used the link-level FER-vs-Eb/N0 tables of the analytical paper's reference [32]; this engine uses the NIST PER bound for 802.11p and its BLER tables for the sidelink (04-models.md §5).\n\
- **802.11p range.** The largest disagreement in this report: at 300–400 m this engine's 802.11p still delivers most packets on both of its channels, where the Veins study's delivery has collapsed (the Veins curve falls from 0.86 at 200 m to 0.02 at 400 m). On one shared geometry the cut-off distance is set by path loss against receiver sensitivity and the FER table's threshold; WINNER+ B1 is an urban micro-cell model with a steeper loss than either highway-LOS channel this engine can run. Until WINNER+ B1 is calibrated this is a finding about the channel, not yet about the 802.11p implementation (next step: run the sweep with a WINNER+ B1 implementation and the paper's FER table, then compare again).\n\
- **OpenCV2X sub-channelisation.** The paper uses 2 sub-channels of 12 RB in 10 MHz; the harness keeps its 50-PRB pool with 12-PRB sub-channels and sets only MCS (QPSK r0.5) and the −126 dBm RSRP threshold, so its resource pool is larger and its collision rate at 120 veh/km lower.\n";
