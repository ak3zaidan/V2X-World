//! WP-06: the SUMO-format exporters, end to end.
//!
//! One short run on a small procedural grid names every `sumo-*` exporter. The files must
//!
//! 1. validate against SUMO's own XSDs (`xmllint --schema`). The schemas are not vendored:
//!    they come from a local SUMO install (`$SUMO_HOME` or `$V2XW_SUMO_VENV`,
//!    `<sumo>/data/xsd`) or are fetched at test time, pinned to SUMO 1.27.1, into the
//!    git-ignored `target/sumo-xsd/`. `$V2XW_SUMO_XSD_DIR` names a directory to use
//!    instead. With no schema and no network the test fails rather than skipping, unless
//!    `V2XW_SKIP_SUMO_XSD=1` says so explicitly;
//! 2. agree with the engine's own trip records (the run report's despawn causes) and with
//!    the recording's own counts (an edge-by-edge recount of `gt.kinematics`);
//! 3. be byte-identical across two runs of the same seed;
//! 4. be read by SUMO's own tools (`xml2csv.py`, `sumolib`) where SUMO is installed, and
//!    name only edges of the `net.xml` netconvert builds from the same world.
//!
//! **Negative controls:** a copy of each file with one required attribute dropped must fail
//! validation, so a validator that accepted anything would turn this test red.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use v2xw_cli::run::{RunOptions, run};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels below the workspace root")
        .to_path_buf()
}

fn out_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("v2xw-cli-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The SUMO release the schemas are pinned to (the one `docs/validation/cross-validation.md`
/// was measured with).
const SUMO_TAG: &str = "v1_27_1";

const XSDS: [&str; 8] = [
    "fcd_file.xsd",
    "tripinfo_file.xsd",
    "meandata_file.xsd",
    "summary_file.xsd",
    "det_e1_file.xsd",
    "det_e2_file.xsd",
    "types/base.xsd",
    "types/meandata.xsd",
];

/// SUMO's `summary_file.xsd` declares `step/@time` with the type `nonNegativeFloat`, which
/// no SUMO schema defines (1.24.0 to 1.27.1 alike), so the schema does not compile in any
/// validator. The cached copy names the type `base.xsd` defines, `nonNegativeFloatType`,
/// which is what every other SUMO output uses for a time; nothing else is changed.
fn patch_summary(dir: &Path) {
    let p = dir.join("summary_file.xsd");
    let s = std::fs::read_to_string(&p).expect("summary_file.xsd");
    let fixed = s.replace("type=\"nonNegativeFloat\"", "type=\"nonNegativeFloatType\"");
    std::fs::write(&p, fixed).expect("write the patched summary schema");
}

/// Where SUMO's schemas are, fetching them if need be. `None` only with the explicit
/// opt-out.
fn xsd_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("V2XW_SUMO_XSD_DIR") {
        return Some(PathBuf::from(d));
    }
    let cache = repo_root().join("target").join("sumo-xsd");
    if let Some(home) = v2xw_interop::sumo_home() {
        let src = home.join("data").join("xsd");
        let dst = cache.join("local");
        for f in XSDS {
            let to = dst.join(f);
            std::fs::create_dir_all(to.parent().expect("a parent")).expect("cache dir");
            std::fs::copy(src.join(f), &to).expect("copy a schema from the SUMO install");
        }
        patch_summary(&dst);
        return Some(dst);
    }
    let dst = cache.join(SUMO_TAG);
    if !XSDS.iter().all(|f| dst.join(f).exists()) {
        for f in XSDS {
            let to = dst.join(f);
            std::fs::create_dir_all(to.parent().expect("a parent")).expect("cache dir");
            let url = format!(
                "https://raw.githubusercontent.com/eclipse-sumo/sumo/{SUMO_TAG}/data/xsd/{f}"
            );
            let ok = Command::new("curl")
                .args(["-sfSL", "--max-time", "60", "-o"])
                .arg(&to)
                .arg(&url)
                .status()
                .is_ok_and(|s| s.success());
            if !ok {
                let _ = std::fs::remove_file(&to);
                if std::env::var("V2XW_SKIP_SUMO_XSD").as_deref() == Ok("1") {
                    eprintln!("SKIPPED: no SUMO schema ({url}) and V2XW_SKIP_SUMO_XSD=1");
                    return None;
                }
                panic!(
                    "could not fetch {url}: install SUMO, set V2XW_SUMO_XSD_DIR, or set \
                     V2XW_SKIP_SUMO_XSD=1 to skip schema validation explicitly"
                );
            }
        }
        patch_summary(&dst);
    }
    Some(dst)
}

/// `xmllint --schema`: Ok, or the validator's complaint.
fn validate(xsd: &Path, file: &Path) -> Result<(), String> {
    let out = Command::new("xmllint")
        .arg("--noout")
        .arg("--schema")
        .arg(xsd)
        .arg(file)
        .output()
        .expect("xmllint (libxml2) is needed to validate against SUMO's schemas");
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr)
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join(" | "))
    }
}

/// A small signalised grid with enough demand that trips finish inside the run.
fn scenario(dir: &Path, detectors: &serde_json::Value) -> PathBuf {
    let mut s = v2xw_engine::Scenario::load(repo_root().join("scenarios/phase1-grid.yaml"))
        .expect("load phase1-grid");
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = 4.into();
        params["rows"] = 6.into();
    }
    s.actors.vehicles.equipped_fraction = 0.0;
    let ex = |id: &str, opts: serde_json::Value| v2xw_engine::scenario::ExporterSpec {
        id: id.to_string(),
        opts,
    };
    s.exporters = vec![
        ex("sumo-fcd", serde_json::Value::Null),
        ex(
            "sumo-tripinfo",
            serde_json::json!({"write_unfinished": true}),
        ),
        ex("sumo-edgedata", serde_json::json!({"period_s": 30.0})),
        ex("sumo-lanedata", serde_json::Value::Null),
        ex("sumo-summary", serde_json::Value::Null),
        ex(
            "sumo-e1",
            serde_json::json!({"detectors": detectors["e1"], "period_s": 30.0}),
        ),
        ex(
            "sumo-e2",
            serde_json::json!({"detectors": detectors["e2"], "period_s": 30.0}),
        ),
    ];
    std::fs::create_dir_all(dir).expect("dir");
    let path = dir.join("sumo-outputs.yaml");
    std::fs::write(&path, s.to_yaml().expect("yaml")).expect("write the scenario");
    path
}

/// An E1 at mid-lane and an E2 over the last 40 m before the stop line of every SUMO lane
/// at least 50 m long.
fn detectors(world: &v2xw_world::World) -> serde_json::Value {
    let ids = v2xw_interop::SumoIds::new(world);
    let (mut e1, mut e2) = (Vec::new(), Vec::new());
    for (lid, lane) in &ids.lane_of {
        let len = world.lane(*lane).length_m;
        if len < 50.0 {
            continue;
        }
        e1.push(serde_json::json!({"id": format!("e1_{lid}"), "lane": lid, "pos_m": len / 2.0}));
        e2.push(serde_json::json!({
            "id": format!("e2_{lid}"), "lane": lid, "pos_m": len - 40.0, "length_m": 40.0
        }));
    }
    serde_json::json!({"e1": e1, "e2": e2})
}

fn options(scenario: PathBuf, out: PathBuf) -> RunOptions {
    RunOptions {
        scenario,
        out: Some(out),
        build_utc: Some("2026-10-26T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: true,
        attach_world: false,
        verify: false,
        stream_digest: false,
        duration_s: Some(150.0),
        rate_veh_per_h: Some(1800.0),
        veremi: None,
        city: None,
        json: false,
        long: v2xw_cli::run::LongRun::default(),
    }
}

/// Every `<tag .../>` element's attributes, in file order.
fn elements(xml: &str, tag: &str) -> Vec<BTreeMap<String, String>> {
    let open = format!("<{tag} ");
    let mut out = Vec::new();
    for line in xml.lines() {
        let Some(i) = line.find(&open) else { continue };
        let body = &line[i + open.len()..];
        let mut m = BTreeMap::new();
        let mut rest = body;
        while let Some(eq) = rest.find("=\"") {
            let key = rest[..eq].trim().to_string();
            let after = &rest[eq + 2..];
            let end = after.find('"').expect("a closing quote");
            m.insert(key, after[..end].to_string());
            rest = &after[end + 1..];
        }
        out.push(m);
    }
    out
}

const FILES: [(&str, &str, &str); 7] = [
    ("sumo-fcd", "fcd.xml", "fcd_file.xsd"),
    ("sumo-tripinfo", "tripinfo.xml", "tripinfo_file.xsd"),
    ("sumo-edgedata", "edgedata.xml", "meandata_file.xsd"),
    ("sumo-lanedata", "lanedata.xml", "meandata_file.xsd"),
    ("sumo-summary", "summary.xml", "summary_file.xsd"),
    ("sumo-e1", "e1.xml", "det_e1_file.xsd"),
    ("sumo-e2", "e2.xml", "det_e2_file.xsd"),
];

/// A required attribute of each file, for the negative control.
const REQUIRED: [(&str, &str); 7] = [
    ("fcd.xml", " time=\""),
    ("tripinfo.xml", " departLane=\""),
    ("edgedata.xml", " begin=\""),
    ("lanedata.xml", " end=\""),
    ("summary.xml", " running=\""),
    ("e1.xml", " nVehContrib=\""),
    ("e2.xml", " jamLengthInMetersSum=\""),
];

/// Removes the first occurrence of attribute `attr` (`" name=\""`) and its value.
fn drop_attr(xml: &str, attr: &str) -> Option<String> {
    let i = xml.find(attr)?;
    let after = &xml[i + attr.len()..];
    let end = after.find('"')?;
    Some(format!("{}{}", &xml[..i], &after[end + 1..]))
}

#[test]
fn sumo_outputs_validate_and_agree_with_the_run() {
    let base = out_dir("sumo-outputs");
    let pre =
        v2xw_engine::Scenario::load(repo_root().join("scenarios/phase1-grid.yaml")).expect("load");
    let mut small = pre.clone();
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut small.world.source {
        params["cols"] = 4.into();
        params["rows"] = 6.into();
    }
    let world_engine = v2xw_engine::Engine::build(small, "").expect("build the small grid");
    let world = world_engine.world().clone();
    drop(world_engine);
    let dets = detectors(&world);
    assert!(
        !dets["e1"].as_array().expect("e1").is_empty(),
        "no lane for a detector"
    );

    let path = scenario(&base, &dets);
    let a = run(&options(path.clone(), base.join("a"))).expect("run a");
    let b = run(&options(path, base.join("b"))).expect("run b");

    // 3. Determinism: the same seed writes the same bytes.
    for (id, file, _) in FILES {
        let fa = std::fs::read(base.join("a").join(id).join(file)).expect("file a");
        let fb = std::fs::read(base.join("b").join(id).join(file)).expect("file b");
        assert!(!fa.is_empty(), "{file} is empty");
        assert_eq!(fa, fb, "{file} differs between two runs of one seed");
    }
    let dir = |id: &str| base.join("a").join(id);
    let read = |id: &str, f: &str| std::fs::read_to_string(dir(id).join(f)).expect("read");

    // 1. Schema validation, with a negative control per file.
    if let Some(xsd) = xsd_dir() {
        for (id, file, schema) in FILES {
            let f = dir(id).join(file);
            validate(&xsd.join(schema), &f)
                .unwrap_or_else(|e| panic!("{file} does not validate against {schema}: {e}"));
            let (_, attr) = REQUIRED.iter().find(|(x, _)| *x == file).expect("a fault");
            let broken = drop_attr(&read(id, file), attr).expect("the attribute is written");
            let bad = base.join(format!("broken-{file}"));
            std::fs::write(&bad, broken).expect("write the faulty copy");
            assert!(
                validate(&xsd.join(schema), &bad).is_err(),
                "{file} with{attr}…\" dropped still validates: the check cannot fail"
            );
        }
    }

    // 2a. tripinfo against the engine's own trip records.
    let trips = elements(&read("sumo-tripinfo", "tripinfo.xml"), "tripinfo");
    let report = &a.report;
    let left: u64 = report.despawn_causes.values().sum();
    let complete = report
        .despawn_causes
        .get("TripComplete")
        .copied()
        .unwrap_or(0);
    let finished: Vec<_> = trips
        .iter()
        .filter(|t| t.get("vaporized").map(String::as_str) != Some("end"))
        .collect();
    assert!(
        complete > 0,
        "no trip finished in the run; the comparison would be empty"
    );
    assert_eq!(
        finished.len() as u64,
        left,
        "tripinfo rows against the vehicles that left"
    );
    assert_eq!(
        finished
            .iter()
            .filter(|t| !t.contains_key("vaporized"))
            .count() as u64,
        complete,
        "tripinfo arrivals against the TripComplete despawns"
    );
    let vehicles_spawned = report.actors_spawned;
    assert_eq!(
        trips.len() as u64,
        vehicles_spawned,
        "with write_unfinished every spawned vehicle has one tripinfo"
    );
    for t in &trips {
        let n = |k: &str| t[k].parse::<f64>().expect("a number");
        assert!(
            (n("arrival") - n("depart") - n("duration")).abs() <= 0.011,
            "duration is arrival - depart: {t:?}"
        );
        assert!(
            n("waitingTime") <= n("duration") + 0.011 && n("timeLoss") <= n("duration") + 0.011
        );
    }

    // 2b. edgeData counts against a recount of the recording.
    let recording = std::fs::read_dir(base.join("a"))
        .expect("out dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "mcap"))
        .expect("the run wrote an MCAP recording");
    let mut reader = v2xw_record::Reader::open(&recording).expect("open the recording");
    let records = reader.records(None).expect("records");
    let ids = v2xw_interop::SumoIds::new(&world);
    let mut lane_at: BTreeMap<u32, Vec<(u64, Option<u32>, String)>> = BTreeMap::new();
    for r in records.iter().filter(|r| r.channel == "gt.kinematics") {
        let v: serde_json::Value = serde_json::from_slice(&r.json).expect("json");
        let class = v["class"].as_str().unwrap_or("").to_string();
        lane_at
            .entry(v["actor"].as_u64().expect("actor") as u32)
            .or_default()
            .push((
                v["t"].as_u64().expect("t"),
                v["lane"].as_u64().map(|x| x as u32),
                class,
            ));
    }
    // (interval begin ns, edge) → (departed, entered, left)
    let mut want: BTreeMap<(u64, String), (u64, u64, u64)> = BTreeMap::new();
    let period = 30_000_000_000u64;
    let edge_of = |l: Option<u32>| {
        l.and_then(|l| {
            ids.edge_of_lane(&world, v2xw_core::ids::LaneId::new(l))
                .map(str::to_string)
        })
    };
    for samples in lane_at.values_mut() {
        samples.sort();
        if samples.first().is_some_and(|s| s.2 == "pedestrian") {
            continue;
        }
        let mut prev: Option<Option<String>> = None;
        for (t, lane, _) in samples.iter() {
            let here = edge_of(*lane);
            let k = t / period * period;
            match &prev {
                None => {
                    if let Some(e) = &here {
                        want.entry((k, e.clone())).or_default().0 += 1;
                    }
                }
                Some(before) if *before != here => {
                    if let Some(e) = before {
                        want.entry((k, e.clone())).or_default().2 += 1;
                    }
                    if let Some(e) = &here {
                        want.entry((k, e.clone())).or_default().1 += 1;
                    }
                }
                Some(_) => {}
            }
            prev = Some(here);
        }
    }
    let edgedata = read("sumo-edgedata", "edgedata.xml");
    let mut got: BTreeMap<(u64, String), (u64, u64, u64)> = BTreeMap::new();
    let mut begin = 0u64;
    for line in edgedata.lines() {
        if let Some(iv) = elements(line, "interval").first() {
            begin = (iv["begin"].parse::<f64>().expect("begin") * 1e9).round() as u64;
        } else if let Some(e) = elements(line, "edge").first() {
            let n = |k: &str| e[k].parse::<u64>().expect("a count");
            let v = (n("departed"), n("entered"), n("left"));
            if v != (0, 0, 0) {
                got.insert((begin, e["id"].clone()), v);
            }
        }
    }
    want.retain(|_, v| *v != (0, 0, 0));
    assert!(!want.is_empty(), "the recount found no traffic");
    assert_eq!(
        got, want,
        "edgeData departed/entered/left against the recording's recount"
    );

    // 2c. The summary's final step against the run report.
    let steps = elements(&read("sumo-summary", "summary.xml"), "step");
    let last = steps.last().expect("a summary step");
    assert_eq!(
        last["inserted"].parse::<u64>().expect("n"),
        vehicles_spawned
    );
    assert_eq!(last["arrived"].parse::<u64>().expect("n"), complete);

    // 4. SUMO's own tools, where SUMO is installed.
    let Some(home) = v2xw_interop::sumo_home() else {
        eprintln!("SUMO not installed: SUMO's tools were not run on the outputs");
        return;
    };
    let python = std::env::var("V2XW_SUMO_VENV")
        .map(|v| PathBuf::from(v).join("bin").join("python"))
        .unwrap_or_else(|_| PathBuf::from("python3"));
    for (id, file, _) in FILES {
        let out = Command::new(&python)
            .arg(home.join("tools").join("xml").join("xml2csv.py"))
            .arg(dir(id).join(file))
            .arg("-o")
            .arg(base.join(format!("{file}.csv")))
            .output()
            .expect("run xml2csv.py");
        assert!(
            out.status.success(),
            "SUMO's xml2csv.py could not read {file}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let (net, _) = v2xw_interop::netexport::export_net(
        &world,
        &ids,
        &v2xw_interop::sumo_tool(&home, "netconvert"),
        &base.join("net"),
        "grid",
    )
    .expect("netconvert the world");
    let check = format!(
        "import sys, sumolib\n\
         net = sumolib.net.readNet(sys.argv[1])\n\
         known = {{e.getID() for e in net.getEdges()}}\n\
         seen = {{e.id for iv in sumolib.output.parse(sys.argv[2], 'interval') for e in (iv.edge or [])}}\n\
         trips = list(sumolib.output.parse(sys.argv[3], 'tripinfo'))\n\
         missing = sorted(seen - known)\n\
         print(len(seen), len(trips), missing[:5])\n\
         sys.exit(1 if missing or not seen or len(trips) != {} else 0)\n",
        trips.len()
    );
    let out = Command::new(&python)
        .env("PYTHONPATH", home.join("tools"))
        .arg("-c")
        .arg(check)
        .arg(&net)
        .arg(dir("sumo-edgedata").join("edgedata.xml"))
        .arg(dir("sumo-tripinfo").join("tripinfo.xml"))
        .output()
        .expect("run sumolib");
    assert!(
        out.status.success(),
        "sumolib: {} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&base);
}
