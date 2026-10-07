//! The engine's cost at peak load, held by counts rather than by a clock.
//!
//! `scenarios/midtown-peak-load.yaml` is the heaviest scenario the page is asked to carry. Its
//! cost is set by how many links each frame is followed to: the release engine ran it at
//! 0.19x real time with every frame evaluated at every one of ~600 radios in the box, and at
//! 0.5x with full evaluation capped at 500 m (`radio.range.max_m`; BUILD-STATUS.md, stabilise
//! track, 2026-10-06). A wall-clock budget would be a figure of the machine's load as much as
//! of the engine, so this test budgets the work instead, on a procedural Midtown-shaped grid
//! that needs no map import, with the peak's mix: pedestrians, three in four without a
//! handset, among equipped vehicles.
//!
//! - **No link to an unequipped actor.** Every link a frame is followed on (a reception
//!   attempt, or energy only) ends at a node, so per frame they are at most the nodes alive.
//! - **The cap caps.** With `radio.range.max_m` set, the fully evaluated attempts fall, and
//!   the run's attempts per frame stay under a budget taken from the measured value. A change
//!   that stops honouring the cap, or follows frames to more receivers than the link budget
//!   reaches, fails here before it reaches the page as a slower run.

use std::path::PathBuf;

use v2xw_engine::{Engine, NullRecorder, Scenario};

fn peak_grid(cap_m: Option<f64>) -> Scenario {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/vru-grid.yaml")).expect("vru-grid loads");
    s.time.duration_s = 3.0;
    s.actors.vru.pedestrians = 300;
    s.actors.vru.cyclists = 20;
    s.actors.vru.device_fraction = 0.25;
    s.actors.vehicles.demand.rate_veh_per_h = Some(20_000.0);
    s.radio.range.max_m = cap_m;
    s.validate().expect("the peak grid validates");
    s
}

struct Cost {
    frames: u64,
    attempts: u64,
    links: u64,
    nodes: u64,
}

fn cost(s: Scenario) -> Cost {
    let mut engine = Engine::build(s, "2026-10-06T00:00:00Z").expect("builds");
    let mut recorder = NullRecorder::new();
    let r = engine.run(&mut recorder).expect("runs");
    Cost {
        frames: r.frames_transmitted,
        attempts: r.reception_attempts,
        links: r.reception_attempts + r.faint_arrivals,
        nodes: r.nodes_created,
    }
}

/// Attempts per transmitted frame with full evaluation capped at 150 m, as measured on
/// 2026-10-06 (284 frames, 85 nodes: 3.9 attempts and 9.5 links per frame capped, 61.8 and
/// 80.3 open). The budget is 25 % above it. The counts are deterministic, so the margin is
/// for a deliberate model change, which should move this number with a note saying why.
const CAPPED_ATTEMPTS_PER_FRAME: f64 = 3.9;

#[test]
fn a_frame_is_followed_only_to_radios_and_the_range_cap_caps_the_work() {
    let open = cost(peak_grid(None));
    let capped = cost(peak_grid(Some(150.0)));
    for (name, c) in [("uncapped", &open), ("capped", &capped)] {
        assert!(c.frames > 0, "{name}: nothing was transmitted");
        let per_frame = c.links as f64 / c.frames as f64;
        eprintln!(
            "{name}: {} frames, {} nodes, {:.1} links and {:.1} attempts per frame",
            c.frames,
            c.nodes,
            per_frame,
            c.attempts as f64 / c.frames as f64
        );
        assert!(
            per_frame <= c.nodes as f64,
            "{name}: {per_frame:.1} links per frame with only {} nodes ever created: a frame is \
             being followed to something that has no radio",
            c.nodes
        );
    }
    let open_per = open.attempts as f64 / open.frames as f64;
    let capped_per = capped.attempts as f64 / capped.frames as f64;
    assert!(
        capped_per < open_per,
        "capping full evaluation at 150 m did not reduce the attempts per frame \
         ({capped_per:.1} capped, {open_per:.1} open)"
    );
    assert!(
        capped_per <= CAPPED_ATTEMPTS_PER_FRAME * 1.25,
        "{capped_per:.1} attempts per frame against a budget of {:.1} (measured {:.1} + 25 %): \
         each frame is followed to more receivers than it was",
        CAPPED_ATTEMPTS_PER_FRAME * 1.25,
        CAPPED_ATTEMPTS_PER_FRAME
    );
}
