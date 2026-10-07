//! Layer 5: whole runs. Conservation and the measurement invariants on every shipped
//! scenario, determinism across thread counts, and three metamorphic relations.
//!
//! Every run is short (a few simulated seconds): the properties checked here hold at every
//! instant of a run, so a long run adds cost without adding coverage.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use v2xw_core::registry::Registry;
use v2xw_engine::{DigestRecorder, Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::EventLedger;
use v2xw_metrics::channels::{MacCbrView, NodeRxView, RxFate, decode};

use crate::{Check, Cost, Layer, Mode, Outcome};

/// The checks of this layer.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "E2E-01",
            layer: Layer::EndToEnd,
            title: "Conservation on every shipped scenario: every reception attempt has exactly one fate and at most one loss cause, delivered + lost = attempts, latency stages tile end-to-end latency, frame layers sum to octets on air, per-bucket bytes sum to totals",
            reference: "The measurement invariants M-RX1, M-LAT1, M-BYTE1, M-BYTE2, M-SHARE of 08-measurement-and-data.md, evaluated by v2xw-metrics over the run's own records; invariant I-R3 (one loss cause); RunReport identity attempts = ok + Σ losses",
            tolerance: "exact counts; the invariants' own tolerances (1 µs, 1 octet)",
            fault: "One phy.rx attempt dropped from each run's record (an attempt that never gets a fate)",
            cost: Cost::Slow,
            run: conservation,
        },
        Check {
            id: "E2E-02",
            layer: Layer::EndToEnd,
            title: "Determinism: the same scenario gives a byte-identical record on 1 and 4 threads, and again on a second run",
            reference: "ADR 0004 (keyed RNG streams, id-ordered reductions, no wall clock); the record digest over every emitted record",
            tolerance: "identical SHA-256 digests",
            fault: "The second run made with a different master seed (what an unkeyed or thread-local generator produces)",
            cost: Cost::Slow,
            run: determinism,
        },
        Check {
            id: "E2E-03",
            layer: Layer::EndToEnd,
            title: "Metamorphic: more transmit power never shortens range — the set of vehicle pairs that ever deliver does not shrink at +6 dB",
            reference: "Monotonicity of the link budget in transmit power (Friis; every PER model here is monotone in SINR)",
            tolerance: "pairs(+6 dB) ⊇-count ≥ pairs(0 dB) on the 10-vehicle Manhattan rung",
            fault: "The comparison run made at −6 dB instead of +6 dB",
            cost: Cost::Slow,
            run: more_power,
        },
        Check {
            id: "E2E-04",
            layer: Layer::EndToEnd,
            title: "Metamorphic: more vehicles never lower the channel load — mean CBR with 100 vehicles exceeds mean CBR with 10",
            reference: "CBR is the busy fraction of the medium (EN 302 571 §4.2.10.1); offered load grows with the number of transmitters",
            tolerance: "strictly greater mean CBR",
            fault: "The two runs' roles swapped",
            cost: Cost::Slow,
            run: more_density,
        },
        Check {
            id: "E2E-05",
            layer: Layer::EndToEnd,
            title: "Metamorphic: turning buildings on never raises delivery on the Manhattan extract",
            reference: "Obstruction only adds loss (Sommer 2011 building model; 04-models.md §3.5)",
            tolerance: "delivered(buildings on) ≤ delivered(buildings off)",
            fault: "The two runs' roles swapped",
            cost: Cost::Slow,
            run: buildings,
        },
    ]
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Loads a shipped scenario with its world path made absolute.
pub fn load(rel: &str) -> Result<Scenario, String> {
    let mut s = Scenario::load(repo().join("scenarios").join(rel)).map_err(|e| format!("{rel}: {e}"))?;
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        let abs = repo().join(&*path);
        if !abs.exists() {
            return Err(format!("{rel}: world {} not present (not in git)", abs.display()));
        }
        *path = abs.to_string_lossy().into_owned();
    }
    Ok(s)
}

/// Every shipped top-level scenario, by file name.
#[must_use]
pub fn shipped() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(repo().join("scenarios"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".yaml"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn run_memory(s: Scenario) -> Result<(RunReport, MemoryRecorder), String> {
    let mut engine = Engine::build(s, "").map_err(|e| format!("build: {e}"))?;
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).map_err(|e| format!("run: {e}"))?;
    Ok((report, rec))
}

fn run_digest(s: Scenario) -> Result<String, String> {
    let mut engine = Engine::build(s, "").map_err(|e| format!("build: {e}"))?;
    let mut rec = DigestRecorder::new();
    engine.run(&mut rec).map_err(|e| format!("run: {e}"))?;
    Ok(rec.digest_hex())
}

/// The simulated seconds each conservation run lasts.
const SHORT_S: f64 = 3.0;

fn conservation(mode: Mode) -> Outcome {
    let mut parts = Vec::new();
    let mut ran = 0;
    for name in shipped() {
        let mut s = match load(&name) {
            Ok(s) => s,
            Err(e) => {
                parts.push((true, format!("{name}: not run ({e})")));
                continue;
            }
        };
        s.time.duration_s = s.time.duration_s.min(SHORT_S);
        s.metrics = vec!["all".to_string()];
        let catalog_ok = v2xw_engine::wiring::build_metrics(&s, &mut Registry::new()).is_ok();
        let (report, rec) = match run_memory(s) {
            Ok(r) => r,
            Err(e) => {
                parts.push((false, format!("{name}: {e}")));
                continue;
            }
        };
        ran += 1;
        let mut ledger = EventLedger::new();
        let mut dropped = false;
        for (_, r) in rec.records() {
            if mode.faulted() && !dropped && r.channel == "phy.rx" {
                dropped = true;
                continue;
            }
            ledger.ingest(r);
        }
        let samples: Vec<v2xw_metrics::MetricSample> = rec
            .records()
            .iter()
            .filter(|(_, r)| r.channel == "metric.sample")
            .filter_map(|(_, r)| serde_json::from_slice(&r.json).ok())
            .collect();
        let checks = v2xw_metrics::check_all(&ledger, &samples);
        let failed: Vec<String> = checks
            .outcomes
            .iter()
            .filter(|o| !o.violations.is_empty())
            .map(|o| format!("{} ({} violations)", o.invariant, o.violations.len()))
            .collect();
        let losses: u64 = report.rx_losses.values().sum();
        let identity = report.reception_attempts == report.receptions_ok + losses;
        let fates = ledger.node_rx.len() as u64 == report.reception_attempts
            && ledger.rx.len() as u64 == report.reception_attempts;
        let ok = failed.is_empty() && identity && fates && catalog_ok && ledger.decode_failure_total() == 0;
        parts.push((
            ok,
            format!(
                "{name}: {} attempts, {} delivered, {} losses in {} causes{}{}{}",
                report.reception_attempts,
                report.receptions_ok,
                losses,
                report.rx_losses.len(),
                if identity { "" } else { "; attempts ≠ ok + losses" },
                if fates { "" } else { "; fate count ≠ attempts" },
                if failed.is_empty() { String::new() } else { format!("; {}", failed.join(", ")) }
            ),
        ));
    }
    if ran < 3 {
        return Outcome::skip(format!("only {ran} scenarios could run"));
    }
    Outcome::all(parts)
}

fn grid(duration_s: f64) -> Result<Scenario, String> {
    let mut s = load("phase1-grid.yaml")?;
    s.time.duration_s = duration_s;
    Ok(s)
}

fn determinism(mode: Mode) -> Outcome {
    let in_pool = |threads: usize, s: Scenario| -> Result<String, String> {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().map_err(|e| e.to_string())?;
        pool.install(|| run_digest(s))
    };
    let result = (|| -> Result<(String, String, String), String> {
        let a = in_pool(1, grid(SHORT_S)?)?;
        let b = in_pool(4, grid(SHORT_S)?)?;
        let mut third = grid(SHORT_S)?;
        if mode.faulted() {
            third.seed ^= 1;
        }
        let c = in_pool(1, third)?;
        Ok((a, b, c))
    })();
    match result {
        Ok((a, b, c)) => Outcome::judge(
            a == b && a == c,
            format!("1 thread {}…, 4 threads {}…, rerun {}…", &a[..12], &b[..12], &c[..12]),
        ),
        Err(e) => Outcome::skip(e),
    }
}

fn delivered_pairs(rec: &MemoryRecorder) -> BTreeSet<(u32, u32)> {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == "node.rx")
        .filter_map(|(_, r)| decode::<NodeRxView>(r).ok())
        .filter(|v| v.outcome == RxFate::Delivered)
        .filter_map(|v| v.tx.map(|tx| (tx.index(), v.rx.index())))
        .collect()
}

fn more_power(mode: Mode) -> Outcome {
    let result = (|| -> Result<(usize, usize, f64), String> {
        let base = load("scale/10.yaml")?;
        let p0 = base.radio.devices.obu.tx_power_dbm;
        let mut louder = base.clone();
        louder.radio.devices.obu.tx_power_dbm = p0 + if mode.faulted() { -6.0 } else { 6.0 };
        let (_, r0) = run_memory(base)?;
        let (_, r1) = run_memory(louder)?;
        Ok((delivered_pairs(&r0).len(), delivered_pairs(&r1).len(), p0))
    })();
    match result {
        Ok((a, b, p0)) => Outcome::judge(b >= a && a > 0, format!("{a} delivering pairs at {p0} dBm, {b} at the changed power")),
        Err(e) => Outcome::skip(e),
    }
}

fn mean_cbr(rec: &MemoryRecorder) -> f64 {
    let v: Vec<f64> = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "mac.cbr")
        .filter_map(|(_, r)| decode::<MacCbrView>(r).ok())
        .map(|v| v.cbr)
        .collect();
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}

fn more_density(mode: Mode) -> Outcome {
    let result = (|| -> Result<(f64, f64), String> {
        let (_, sparse) = run_memory(load("scale/10.yaml")?)?;
        let (_, dense) = run_memory(load("scale/100.yaml")?)?;
        let (a, b) = (mean_cbr(&sparse), mean_cbr(&dense));
        Ok(if mode.faulted() { (b, a) } else { (a, b) })
    })();
    match result {
        Ok((a, b)) => Outcome::judge(b > a, format!("mean CBR {a:.4} with 10 vehicles, {b:.4} with 100")),
        Err(e) => Outcome::skip(e),
    }
}

fn buildings(mode: Mode) -> Outcome {
    let result = (|| -> Result<(u64, u64), String> {
        let mut on = load("scale/10.yaml")?;
        on.world.buildings.enabled = true;
        let mut off = on.clone();
        off.world.buildings.enabled = false;
        let (r_on, _) = run_memory(on)?;
        let (r_off, _) = run_memory(off)?;
        Ok(if mode.faulted() {
            (r_off.receptions_ok, r_on.receptions_ok)
        } else {
            (r_on.receptions_ok, r_off.receptions_ok)
        })
    })();
    match result {
        Ok((on, off)) => Outcome::judge(on <= off && off > 0, format!("{on} delivered with buildings, {off} without")),
        Err(e) => Outcome::skip(e),
    }
}
