//! The control surface under random use: user input never becomes an internal error, and
//! nothing a caller sends leaves the run inconsistent.
//!
//! Two kinds of test live here.
//!
//! * **The five QA causes, end to end.** Each of the settings the 2026-09-24 QA found
//!   coming back from Run as `-32603 internal error` — a missing map, a missing DEM, a
//!   roadside unit at a `site` on an imported city, a wrong `net.backend_net` id, an unknown
//!   `detection.ma.params` key — is sent the way the page sends it, through Check
//!   (`scenario.validate`), Apply (`scenario.set`) and Run (`run.start`). Each must be
//!   refused by its setting (`-32004` or a `valid: false` row naming the field), and the run
//!   on screen must keep working afterwards.
//! * **The fuzzer.** A seeded random sequence of valid and invalid calls — every method,
//!   well-formed and malformed parameters, and edits to every leaf of the published
//!   scenario surface with in-range, out-of-range and wrongly typed values — interleaved
//!   with ticks of the producer. After every call: no panic, no `-32603` for anything, a
//!   known error code, and a `run.status` that answers with a consistent state. At the end
//!   the original scenario is staged again and must run to its end with the digest it had
//!   before the fuzzing, so no call left anything behind.
//!
//! `fuzz_short` runs in the suite (a few hundred calls). The same fuzzer runs longer with
//! `V2XW_FUZZ_ITERS=<calls> V2XW_FUZZ_SEED=<n> cargo test -p v2xw-server --test fuzz --
//! --ignored fuzz_long`.

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use v2xw_server::Run;
use v2xw_server::live::{LiveEngine, LiveOptions};
use v2xw_server::rpc::{self, Context};
use v2xw_server::session::{ConnectParams, Session};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<crate>/..")
        .to_path_buf()
}

/// `phase1-grid.yaml` on a 4 × 4 grid for `seconds`, with a fleet, written to a scratch
/// directory of its own so the presets this server lists are the ones this file wrote.
fn scenario(name: &str, seconds: u32) -> PathBuf {
    let source = repo_root().join("scenarios/phase1-grid.yaml");
    let text = std::fs::read_to_string(&source)
        .expect("read phase1-grid.yaml")
        .replace("duration_s: 60.0", &format!("duration_s: {seconds}.0"))
        .replace("rate_veh_per_h: 30.0", "rate_veh_per_h: 3000.0")
        .replace("      cols: 13\n", "      cols: 4\n")
        .replace("      rows: 34\n", "      rows: 4\n");
    let dir = std::env::temp_dir().join(format!("v2xw-server-fuzz-{name}"));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(format!("{name}.yaml"));
    std::fs::write(&path, text).expect("write scenario");
    path
}

fn serve(path: &Path) -> Arc<Run> {
    let engine = LiveEngine::open(
        path,
        LiveOptions {
            build_utc: "2026-09-30T00:00:00Z".to_string(),
            paused: true,
            speed: 0.0,
            ..LiveOptions::default()
        },
    )
    .expect("build");
    let world_json = engine.world_json().to_string();
    Run::new(Box::new(engine), world_json).expect("run")
}

/// One JSON-RPC call on the HTTP path, as the page's Run and Apply make it. Returns the
/// `result`, or the error object.
fn call(run: &Run, method: &str, params: Value) -> Result<Value, Value> {
    call_on(run, None, method, params)
}

/// One JSON-RPC call, on a connection (`session`) as the socket makes it, or on the HTTP
/// path (`None`).
fn call_on(
    run: &Run,
    session: Option<&mut Session>,
    method: &str,
    params: Value,
) -> Result<Value, Value> {
    let text = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let request = rpc::parse(&text).expect("parse");
    let mut ctx = Context {
        run,
        session,
        pending: None,
        received_at: None,
    };
    match rpc::dispatch(&mut ctx, &request) {
        Ok(outcome) => Ok(outcome.result),
        Err(e) => Err(rpc::failure(&json!(1), &e)["error"].clone()),
    }
}

fn ok(run: &Run, method: &str, params: Value) -> Value {
    call(run, method, params).unwrap_or_else(|e| panic!("{method} failed: {e}"))
}

fn drive_to_end(run: &Run) -> Value {
    for _ in 0..100_000 {
        match run.tick() {
            Ok(true) => {}
            Ok(false) => {
                if run.state() == v2xw_server::RunState::Finished {
                    break;
                }
            }
            Err(e) => panic!("the engine aborted: {e}"),
        }
    }
    ok(run, "run.status", json!({}))
}

// --- the five causes -----------------------------------------------------------------

/// Sets `pointer` in `doc` to `value`, creating objects on the way.
fn set_at(doc: &mut Value, pointer: &str, value: Value) {
    let parts: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
    let mut cur = doc;
    for (i, part) in parts.iter().enumerate() {
        if i + 1 == parts.len() {
            cur[*part] = value;
            return;
        }
        if cur.get(*part).is_none_or(Value::is_null) {
            cur[*part] = json!({});
        }
        cur = &mut cur[*part];
    }
}

/// The error rows of a refusal, from either shape: a `-32004`'s `data.errors`, or a
/// `valid: false` result's `errors`.
fn rows_of(v: &Value) -> Vec<(String, String)> {
    let list = v
        .pointer("/data/errors")
        .or_else(|| v.get("errors"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    list.iter()
        .map(|r| {
            (
                r["path"].as_str().unwrap_or("").to_string(),
                r["message"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

#[test]
fn the_five_qa_causes_are_refused_by_their_setting_through_check_apply_and_run() {
    let run = serve(&scenario("five", 2));
    let running = ok(&run, "scenario.get", json!({}))["scenario"].clone();
    let osm = |doc: &mut Value, path: &str| {
        set_at(
            doc,
            "/world/source",
            json!({"kind": "osm-xml", "path": path}),
        );
        set_at(doc, "/world/highway_preset", json!("urban-us-nyc"));
    };
    type Edit = Box<dyn Fn(&mut Value)>;
    let cases: Vec<(&str, &str, Edit)> = vec![
        (
            "a missing map",
            "/world/source/path",
            Box::new(move |d: &mut Value| osm(d, "worlds/no-such-map.osm.xml")),
        ),
        (
            "a missing DEM",
            "/world/terrain/dem",
            Box::new(|d: &mut Value| set_at(d, "/world/terrain/dem", json!("worlds/no.hgt"))),
        ),
        (
            "an RSU site on an OSM world",
            "/actors/rsus/0/site",
            Box::new(move |d: &mut Value| {
                osm(d, &repo_root().join("Cargo.toml").display().to_string());
                set_at(d, "/actors/rsus", json!([{"site": 0}]));
            }),
        ),
        (
            "a wrong net.backend_net id",
            "/net/backend_net",
            Box::new(|d: &mut Value| {
                set_at(d, "/net/backend_net", json!({"id": "backend-net/fibre"}));
            }),
        ),
        (
            "an unknown detection.ma.params key",
            "/detection/ma/params/report_threshold",
            Box::new(|d: &mut Value| {
                set_at(
                    d,
                    "/detection/ma",
                    json!({"id": "threat/ma/legacy-window", "params": {"report_threshold": 3}}),
                );
            }),
        ),
    ];
    for (what, field, edit) in &cases {
        let mut doc = running.clone();
        edit(&mut doc);

        // Check.
        let check = ok(&run, "scenario.validate", json!({"scenario": doc}));
        assert_eq!(check["valid"], false, "{what}: Check accepted it: {check}");
        let rows = rows_of(&check);
        assert!(
            rows.iter().any(|(p, _)| p == field),
            "{what}: Check did not name {field}: {rows:?}"
        );
        let (_, message) = rows.iter().find(|(p, _)| p == field).expect("row");
        assert!(
            !message.contains("internal error"),
            "{what}: the message still says internal error: {message}"
        );

        // Apply, as the page applies (`validate: false`, then reads `valid`).
        let apply = ok(
            &run,
            "scenario.set",
            json!({"scenario": doc, "validate": false}),
        );
        assert_eq!(apply["valid"], false, "{what}: Apply accepted it: {apply}");
        assert!(
            rows_of(&apply).iter().any(|(p, _)| p == field),
            "{what}: {apply}"
        );

        // Run, with the document inline: `-32004` naming the field, never `-32603`.
        let refused = call(&run, "run.start", json!({"paused": true, "scenario": doc}))
            .expect_err(&format!("{what}: Run started"));
        assert_eq!(refused["code"], -32004, "{what}: {refused}");
        assert!(
            rows_of(&refused).iter().any(|(p, _)| p == field),
            "{what}: {refused}"
        );

        // And the run on screen is untouched: not in `error`, and it still steps.
        let status = ok(&run, "run.status", json!({}));
        assert_ne!(status["state"], "error", "{what}: {status}");
        ok(&run, "run.step", json!({"n": 1}));
    }
}

/// A refusal only the built world can decide — a closure naming an edge the grid does not
/// have — comes after the run on screen has been stopped, so the engine is left in `error`.
/// It is still the user's to fix: `run.start` answers `-32004` naming the event, `run.status`
/// says why, a `run.step` is told there is no run (`-32002`) rather than that the engine
/// broke (`-32603`), and the next `run.start` with the setting fixed runs normally.
#[test]
fn a_refusal_found_while_building_is_the_users_and_the_next_run_recovers() {
    let run = serve(&scenario("build-refusal", 2));
    let running = ok(&run, "scenario.get", json!({}))["scenario"].clone();
    let mut doc = running.clone();
    set_at(
        &mut doc,
        "/events",
        json!([{"t": 0.5, "until": 1.5, "type": "closure", "target": "edge:999999"}]),
    );
    // Check cannot know: the edge ids are the world's.
    let check = ok(&run, "scenario.validate", json!({"scenario": doc}));
    assert_eq!(check["valid"], true, "{check}");

    let refused = call(&run, "run.start", json!({"paused": true, "scenario": doc}))
        .expect_err("a closure of an edge that does not exist started");
    assert_eq!(refused["code"], -32004, "{refused}");
    let rows = rows_of(&refused);
    assert!(
        rows.iter().any(|(p, _)| p.starts_with("/events")),
        "the refusal names the event: {rows:?}"
    );

    let status = ok(&run, "run.status", json!({}));
    assert_eq!(status["state"], "error", "{status}");
    let why = status["engine"]["failure"].as_str().unwrap_or("");
    assert!(why.contains("events"), "run.status says why: {why}");
    let step = call(&run, "run.step", json!({})).expect_err("a step with no run");
    assert_eq!(step["code"], -32002, "not an internal error: {step}");

    // Fixed, it runs.
    ok(
        &run,
        "run.start",
        json!({"paused": true, "scenario": running}),
    );
    ok(&run, "run.resume", json!({}));
    let done = drive_to_end(&run);
    assert_eq!(done["state"], "finished", "{done}");
}

/// The history kept for seeking is bounded by memory as well as by step count. Before the
/// byte budget a step carried every reception record and the count alone let an hour of the
/// SCMS lifecycle grow to gigabytes (the long soak measured 1.8 MB per simulated second).
/// Here a 64 kB budget on a 20 s run: the run still streams to its end, the retained bytes
/// stay within the budget plus one step, the oldest steps are dropped, and a seek before
/// them is refused with the range (`-32003`) rather than failing.
#[test]
fn the_seek_history_is_bounded_by_memory_and_a_seek_before_it_is_refused() {
    let path = scenario("retain-bytes", 20);
    let engine = LiveEngine::open(
        &path,
        LiveOptions {
            build_utc: "2026-09-30T00:00:00Z".to_string(),
            paused: true,
            speed: 0.0,
            retain_bytes: 64 * 1024,
            ..LiveOptions::default()
        },
    )
    .expect("build");
    let world_json = engine.world_json().to_string();
    let run = Run::new(Box::new(engine), world_json).expect("run");
    ok(&run, "run.start", json!({"paused": true, "speed": 0}));
    ok(&run, "run.resume", json!({}));
    let done = drive_to_end(&run);
    assert_eq!(done["state"], "finished", "{done}");
    assert!(done["engine"]["output_digest"].is_string(), "{done}");
    let kept = done["engine"]["retained_bytes"]
        .as_u64()
        .expect("retained_bytes");
    let steps = done["engine"]["retained_steps"]
        .as_u64()
        .expect("retained_steps");
    assert!(
        steps < 201,
        "all 201 steps were kept under a 64 kB budget: {done}"
    );
    // Steps the stream has not reached yet are never dropped (the lookahead's worth may sit
    // above the budget for a moment), so the bound checked is loose; the default 1 GiB
    // budget would have kept every step of this run.
    assert!(
        kept <= 8 * 64 * 1024,
        "{kept} bytes retained against a 64 kB budget: {done}"
    );
    let (min_ns, _) = run.seek_range();
    assert!(min_ns > 0, "the oldest steps were dropped: {done}");
    // A seek before the window, on a connection as the page makes it.
    let mut session = Session::new(ConnectParams::default(), &run.descriptor());
    session.regreet(&run).expect("greet");
    let refused = call_on(&run, Some(&mut session), "run.seek", json!({"t_ns": 0}))
        .expect_err("a seek before the retained window");
    assert_eq!(refused["code"], -32003, "{refused}");
    assert_eq!(refused["data"]["min_ns"], json!(min_ns), "{refused}");
}

// --- the fuzzer ------------------------------------------------------------------------

/// xorshift64*: the fuzzer's own generator, seeded, so a failure reproduces from its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A value for a scenario leaf: one of its allowed values, its default, its bounds, just
/// past them, or something of the wrong type.
fn leaf_value(rng: &mut Rng, field: &Value) -> Value {
    let enumerated = field.get("enum").and_then(Value::as_array).cloned();
    let lo = field
        .get("minimum")
        .or_else(|| field.get("exclusiveMinimum"))
        .and_then(Value::as_f64);
    let hi = field.get("maximum").and_then(Value::as_f64);
    let kind = field["kind"].as_str().unwrap_or("");
    match rng.below(10) {
        0 | 1 if enumerated.as_ref().is_some_and(|e| !e.is_empty()) => {
            rng.pick(enumerated.as_deref().unwrap_or(&[])).clone()
        }
        2 => field.get("default").cloned().unwrap_or(Value::Null),
        3 if lo.is_some() => json!(lo),
        4 if hi.is_some() => json!(hi),
        5 if lo.is_some() => json!(lo.unwrap_or(0.0) - 1.0),
        6 if hi.is_some() => json!(hi.unwrap_or(0.0) * 2.0 + 1.0),
        7 => rng
            .pick(&[
                json!("not-a-value"),
                json!(true),
                json!(-1),
                json!(1e308),
                json!([]),
                json!({}),
                Value::Null,
                json!(""),
                json!("0x"),
                json!("é\u{0}\u{7f}"),
            ])
            .clone(),
        _ => match kind {
            "number" | "integer" => {
                let (a, b) = (lo.unwrap_or(0.0), hi.unwrap_or(100.0).min(1e4));
                let v = a + (b - a) * rng.unit();
                if kind == "integer" {
                    json!(v.round() as i64)
                } else {
                    json!(v)
                }
            }
            "boolean" => json!(rng.chance(50)),
            _ => field.get("default").cloned().unwrap_or(json!("x")),
        },
    }
}

/// The error codes §6.4 defines; anything else is a server bug.
const CODES: [i64; 20] = [
    -32700, -32600, -32601, -32602, -32000, -32001, -32002, -32003, -32004, -32005, -32006, -32007,
    -32008, -32009, -32010, -32011, -32013, -32040, -32041, -32050,
];

/// One random call: a method and its parameters, some well formed and some not.
fn random_call(rng: &mut Rng, fields: &[Value], out_dir: &Path, t_end_ns: u64) -> (String, Value) {
    let garbage = || {
        vec![
            json!({"n": -3}),
            json!({"n": "x"}),
            json!({"t_ns": -1}),
            json!({"speed": 1e9}),
            json!({"node": "abc"}),
            json!({"scenario": 7}),
            json!({"patch": {"op": "replace"}}),
            json!({"metrics": 3}),
            json!({"id": null}),
        ]
    };
    let method = match rng.below(20) {
        0..=5 => "scenario.set",
        6 => "scenario.validate",
        7 | 8 => "run.start",
        9 => "run.step",
        10 => "run.seek",
        11 => rng.pick(&["run.pause", "run.resume", "run.speed"]),
        12 => rng.pick(&["run.stop", "run.status", "scenario.get"]),
        13 => rng.pick(&["inspect.node", "inspect.link", "inspect.entity", "explain"]),
        14 => rng.pick(&["metrics.query", "metrics.plot"]),
        15 => rng.pick(&[
            "events.set",
            "scenario.load",
            "scenario.list",
            "scenario.schema",
        ]),
        16 => rng.pick(&["world.generate", "world.import_osm", "rpc.discover"]),
        17 => rng.pick(&[
            "export.dataset",
            "export.recording",
            "experiment.define",
            "experiment.run",
            "experiment.status",
        ]),
        18 => rng.pick(&["view.follow", "view.camera", "overlay.set", "scenario.save"]),
        _ => rng.pick(&["no.such.method", "run.start", "run.step"]),
    }
    .to_string();
    if rng.chance(8) {
        return (method, rng.pick(&garbage()).clone());
    }
    let params = match method.as_str() {
        "scenario.set" => {
            let field = rng.pick(fields);
            let pointer = field["x-pointer"]
                .as_str()
                .unwrap_or("/seed")
                .replace("/-", "/0");
            let op = rng.pick(&["replace", "add", "remove", "test"]).to_string();
            // `world.cache` names a directory the world is written to, relative to the working
            // directory: a random string there filled the crate directory with cache folders.
            // It gets the fuzzer's own scratch directory, or nothing.
            let value = if pointer == "/world/cache" {
                if rng.chance(50) {
                    json!(out_dir.join("world-cache").display().to_string())
                } else {
                    Value::Null
                }
            } else {
                leaf_value(rng, field)
            };
            json!({
                "patch": [{"op": op, "path": pointer, "value": value}],
                "validate": rng.chance(50),
            })
        }
        "scenario.validate" => {
            if rng.chance(50) {
                json!({"strict": rng.chance(50)})
            } else {
                json!({"scenario": {"schema": "v2xw/scenario/1", "seed": rng.below(10)}})
            }
        }
        "run.start" => {
            let mut p = json!({"paused": rng.chance(70)});
            if rng.chance(50) {
                p["speed"] = json!(rng.pick(&[0.0, 1.0, 50.0, 100.0, 101.0, -1.0]));
            }
            if rng.chance(30) {
                p["seed"] = rng
                    .pick(&[
                        json!(7),
                        json!("0xBEEF"),
                        json!("nope"),
                        json!(-5),
                        json!(1.5),
                    ])
                    .clone();
            }
            p
        }
        "run.step" => json!({
            "count": rng.pick(&[0, 1, 3, 100_000]),
            "unit": rng.pick(&["step", "second", "keyframe", "bogus"]),
        }),
        "view.follow" => json!({
            "node": rng.pick(&[0u64, 2, 999_999]),
            "telemetry": rng.chance(50),
            "feed": rng.pick(&[json!(true), json!({"max_rx": 5}), json!("x")]).clone(),
        }),
        "view.camera" => json!({"mode": rng.pick(&["chase", "orbit", "bogus"])}),
        "overlay.set" => json!({
            "name": rng.pick(&["links", "buildings", "bogus"]),
            "on": rng.chance(50),
        }),
        "run.seek" => {
            let t = match rng.below(4) {
                0 => 0,
                1 => t_end_ns / 2,
                2 => t_end_ns,
                _ => t_end_ns.saturating_mul(10),
            };
            json!({"t_ns": t})
        }
        "run.speed" => json!({"speed": rng.pick(&[0.0, 0.5, 1.0, 100.0, 1e6])}),
        "inspect.node" | "explain" => json!({"node": rng.pick(&[0u64, 1, 3, 999_999])}),
        "inspect.link" => json!({"tx": rng.below(4), "rx": rng.below(4)}),
        "inspect.entity" => {
            json!({"entity": rng.pick(&["backend", "ra", "nonsense", ""]), "id": rng.below(3)})
        }
        "metrics.query" | "metrics.plot" => json!({
            "metrics": [rng.pick(&["pdr", "cbr", "no_such_metric", "pdr[100m]", ""])],
            "t0_ns": 0, "t1_ns": t_end_ns,
        }),
        "events.set" => json!({"list": rng.chance(50), "subscribe": ["pdr"], "only": ["x"]}),
        "scenario.load" => {
            json!({"path": rng.pick(&["stub/grid", "no/such.yaml", "", "../../etc/passwd"])})
        }
        "scenario.list" => {
            json!({"kind": rng.pick(&["all", "presets", "bogus"]), "limit": rng.below(5)})
        }
        "scenario.schema" => json!({"sections": [rng.pick(&["fields", "bogus", "version"])]}),
        "world.generate" => json!({
            "kind": rng.pick(&["grid", "ring", "bogus"]),
            "block_m": rng.pick(&[10.0, 120.0, 1e9]),
            "lanes_per_direction": rng.pick(&[0, 2, 9]),
        }),
        "world.import_osm" => json!({"bbox": [1, 2, 3]}),
        "export.dataset" => json!({
            "exporter": rng.pick(&["metrics", "telemetry", "bogus"]),
            "out_dir": out_dir.display().to_string(),
        }),
        "export.recording" => json!({
            "path": out_dir.join("rec.mcap").display().to_string(),
            "profile": rng.pick(&["full", "node", "bogus"]),
        }),
        "experiment.define" => json!({"name": "e", "sweep": {"seed": [1, 2]}}),
        "experiment.run" | "experiment.status" => json!({"id": rng.pick(&["e", "nope"])}),
        "scenario.save" => json!({"path": out_dir.join("s.yaml").display().to_string()}),
        _ => json!({}),
    };
    (method, params)
}

/// Runs the fuzzer: `iters` calls from `seed`, with the checks the module header lists.
fn fuzz(seed: u64, iters: usize) {
    let path = scenario(&format!("fuzz-{seed}"), 3);
    let run = serve(&path);
    let out_dir = std::env::temp_dir().join(format!("v2xw-server-fuzz-out-{seed}"));
    std::fs::create_dir_all(&out_dir).expect("out dir");
    let fields: Vec<Value> = v2xw_engine::scenario::publish::fields();
    assert!(!fields.is_empty());

    // The reference: the original scenario, to its end, before any fuzzing.
    let original = ok(&run, "scenario.get", json!({}))["scenario"].clone();
    ok(&run, "run.start", json!({"paused": true, "speed": 0}));
    ok(&run, "run.resume", json!({}));
    let reference = drive_to_end(&run);
    let reference_digest = reference["engine"]["output_digest"].clone();
    assert!(reference_digest.is_string(), "{reference}");

    // One connection, as the page holds one: calls go through it half the time (the socket
    // path, where `view.*`, `overlay.set` and `run.seek` act) and over HTTP otherwise. After a
    // new run it is greeted again, as the transport does.
    let mut session = Session::new(ConnectParams::default(), &run.descriptor());
    session.regreet(&run).expect("greet");
    let mut rng = Rng(seed.max(1));
    let mut log: Vec<String> = Vec::new();
    let mut codes: std::collections::BTreeMap<i64, usize> = Default::default();
    let mut oks = 0usize;
    for i in 0..iters {
        if rng.chance(15) {
            let n = rng.below(6) + 1;
            for _ in 0..n {
                let _ = std::panic::catch_unwind(AssertUnwindSafe(|| run.tick()))
                    .unwrap_or_else(|_| panic!("tick panicked after:\n{}", log.join("\n")));
            }
            log.push(format!("{i}: tick x{n}"));
            continue;
        }
        let t_end = run.descriptor().duration;
        let (method, params) = random_call(&mut rng, &fields, &out_dir, t_end);
        let line = format!("{i}: {method} {params}");
        log.push(line.clone());
        if log.len() > 40 {
            log.remove(0);
        }
        let before = ok(&run, "scenario.get", json!({}));
        if session.generation() != run.generation() {
            session.regreet(&run).expect("greet the new run");
        }
        let on_socket = rng.chance(50);
        let answer = std::panic::catch_unwind(AssertUnwindSafe(|| {
            call_on(
                &run,
                on_socket.then_some(&mut session),
                &method,
                params.clone(),
            )
        }))
        .unwrap_or_else(|_| panic!("the server panicked on\n{}", log.join("\n")));
        match &answer {
            Ok(result) => {
                oks += 1;
                // A staged edit is what `scenario.get` then says is next; a refused one
                // changed nothing.
                if method == "scenario.set" {
                    let after = ok(&run, "scenario.get", json!({}));
                    if result["valid"] == true {
                        assert_eq!(after["hash"], result["hash"], "{line}\n{after}");
                    } else {
                        assert_eq!(
                            after["hash"], before["hash"],
                            "a refused edit changed the next run: {line}"
                        );
                    }
                }
            }
            Err(error) => {
                let code = error["code"].as_i64().unwrap_or(0);
                *codes.entry(code).or_default() += 1;
                assert!(
                    code != -32603,
                    "internal error for user input: {error}\nafter:\n{}",
                    log.join("\n")
                );
                assert!(
                    CODES.contains(&code),
                    "unknown error code {code}: {error}\n{line}"
                );
                if matches!(code, -32602 | -32004) {
                    // A refusal names what to fix.
                    let rows = error["data"]
                        .get("errors")
                        .or(Some(&error["data"]))
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    assert!(!rows.is_empty(), "a refusal with no row: {error}\n{line}");
                    for r in &rows {
                        assert!(
                            r["path"].is_string()
                                && !r["message"].as_str().unwrap_or("").is_empty(),
                            "a row with no path or message: {error}\n{line}"
                        );
                    }
                }
            }
        }
        // The run is always in a state it can report, and its clock inside its horizon.
        let status = ok(&run, "run.status", json!({}));
        let state = status["state"].as_str().unwrap_or("");
        assert!(
            ["idle", "running", "paused", "finished", "error"].contains(&state),
            "state {state}: {status}\n{line}"
        );
        let (t, end) = (
            status["t_ns"].as_u64().unwrap_or(0),
            status["t_end_ns"].as_u64().unwrap_or(0),
        );
        assert!(t <= end, "t_ns {t} past the horizon {end}: {line}");
        if state == "error" {
            assert!(
                status["engine"]["failure"].is_string(),
                "an error state with no reason: {status}"
            );
        }
    }

    // Recoverable and unchanged: the original scenario staged again runs to its end with
    // the digest it had before the fuzzing.
    let _ = call(&run, "run.pause", json!({}));
    ok(&run, "scenario.set", json!({"scenario": original}));
    ok(
        &run,
        "run.start",
        json!({"paused": true, "speed": 0, "seed": original["seed"]}),
    );
    ok(&run, "run.resume", json!({}));
    let last = drive_to_end(&run);
    assert_eq!(last["state"], "finished", "{last}");
    assert_eq!(
        last["engine"]["output_digest"], reference_digest,
        "the original scenario no longer reproduces after the fuzzing"
    );
    let _ = std::fs::remove_dir_all(&out_dir);
    eprintln!("fuzz seed {seed}: {iters} calls, {oks} ok, errors by code {codes:?}");
}

#[test]
fn fuzz_short() {
    fuzz(0x5EED_F0CC, 250);
}

#[test]
#[ignore = "long: run on purpose with V2XW_FUZZ_ITERS and V2XW_FUZZ_SEED"]
fn fuzz_long() {
    let iters = std::env::var("V2XW_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5_000);
    let seed = std::env::var("V2XW_FUZZ_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    fuzz(seed, iters);
}
