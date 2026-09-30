//! `perf_probe` — where a run's wall-clock time goes, simulated second by simulated second.
//!
//! [`scale`](scale.rs) answers "how long did the whole run take"; this answers "is the cost
//! of one simulated second flat over the run, and what does it scale with". It wraps the
//! run's recorder in one that notes the wall clock each time the record stream crosses a
//! simulated-time bin boundary, so the cost of every bin is measured *outside* the engine:
//! nothing here reads a clock the engine can see, and the record stream it hashes is the
//! same one [`DigestRecorder`] hashes everywhere else — the content digest this prints is
//! the determinism check an optimisation has to leave unchanged.
//!
//! ```text
//! cargo run -p v2xw-engine --example perf_probe -- <scenario.yaml>
//!     [--duration S] [--rate VEH_PER_H] [--max-veh N] [--drop [--drop-gap S]]
//!     [--peds N] [--cyclists N] [--threads N] [--bin S] [--null] [--quiet]
//! ```
//!
//! * `--threads N` runs the engine's phase-parallel maps on a pool of exactly `N` threads,
//!   which is how "the same digest at 1, 2 and 4 threads" is checked.
//! * `--drop` switches the demand to the 3GPP TR 36.885 drop (`mobility/demand/tr36885-drop`);
//!   `--drop-gap S` sets its mean gap in seconds (the TR's is 2.5), which scales the fleet.
//! * `--null` skips the digest (a null recorder), to separate the hashing from the run.
//!
//! It is an example rather than a test because it is a measurement: the numbers are the
//! answer, and on a shared machine a single wall-clock figure is a figure of the load too.

use std::time::Instant;

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::{DigestRecorder, Engine, NullRecorder, RunRecorder, Scenario};

/// A recorder that times the run by simulated-time bin and forwards everything.
struct Timing<'a> {
    inner: &'a mut dyn RunRecorder,
    bin_ns: u64,
    start: Instant,
    /// Wall seconds at which the stream first reached each bin.
    marks: Vec<f64>,
    /// Records written in each bin.
    counts: Vec<u64>,
    /// `gt.kinematics` records in each bin: one per live actor per mobility step, so the
    /// bin's mean population is this over the steps in the bin.
    actors: Vec<u64>,
    /// `node.tx` records in each bin: frames put on the air.
    txs: Vec<u64>,
}

impl Timing<'_> {
    fn note(&mut self, at: SimTime, channel: &str) {
        let bin = (at / self.bin_ns) as usize;
        while self.marks.len() <= bin {
            self.marks.push(self.start.elapsed().as_secs_f64());
            self.counts.push(0);
            self.actors.push(0);
            self.txs.push(0);
        }
        // A record stamped before the newest bin (mobility files its state at `k.t`, one
        // step behind the phase) is counted in the bin being run, where its cost is paid.
        let last = self.counts.len() - 1;
        self.counts[bin.min(last)] += 1;
        match channel {
            "gt.kinematics" => self.actors[bin.min(last)] += 1,
            "node.tx" => self.txs[bin.min(last)] += 1,
            _ => {}
        }
    }
}

impl RunRecorder for Timing<'_> {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        self.note(at, &record.channel);
        self.inner.write(at, record);
    }
    fn write_wire_frame(&mut self, frame: &v2xw_record::wire::Frame) {
        self.inner.write_wire_frame(frame);
    }
    fn tap_frame(&mut self, at: SimTime, node: v2xw_core::ids::NodeId, msg: u64, spdu: &[u8]) {
        self.inner.tap_frame(at, node, msg, spdu);
    }
    fn refused(&self) -> u64 {
        self.inner.refused()
    }
    fn records_written(&self) -> Option<u64> {
        self.inner.records_written()
    }
    fn frames_written(&self) -> Option<u64> {
        self.inner.frames_written()
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or("usage: perf_probe <scenario.yaml> [flags]")?;
    let mut scenario = Scenario::load(path)?;
    if let Some(d) = flag(&args, "--duration") {
        scenario.time.duration_s = d.parse()?;
        // A shortened run keeps its attackers: each schedule is cut at the new horizon,
        // and one that would start after it starts half-way through instead.
        let horizon = scenario.time.duration_s;
        for attacker in &mut scenario.threats.attackers {
            if let Some(w) = attacker.schedule.as_mut() {
                if w.from_s >= horizon {
                    w.from_s = 0.5 * horizon;
                }
                w.to_s = w.to_s.min(horizon);
            }
        }
    }
    if args.iter().any(|a| a == "--drop") {
        scenario.actors.vehicles.demand.kind = "mobility/demand/tr36885-drop".into();
        scenario.actors.vehicles.demand.params = serde_json::Value::Null;
        // The TR 36.885 law with another mean gap (2.5 s is the TR's): the same drop at a
        // lower density, for cost against vehicle count.
        if let Some(g) = flag(&args, "--drop-gap") {
            let gap: f64 = g.parse()?;
            scenario.actors.vehicles.demand.params = serde_json::json!({
                "variant": { "variant": "tr36885", "mean_gap_time_s": gap }
            });
        }
    }
    if let Some(r) = flag(&args, "--rate") {
        scenario.actors.vehicles.demand.rate_veh_per_h = Some(r.parse()?);
    }
    if let Some(n) = flag(&args, "--max-veh") {
        let n: u64 = n.parse()?;
        let params = &mut scenario.actors.vehicles.demand.params;
        if !params.is_object() {
            *params = serde_json::json!({});
        }
        params["max_total_vehicles"] = serde_json::json!(n);
    }
    if let Some(n) = flag(&args, "--peds") {
        scenario.actors.vru.pedestrians = n.parse()?;
    }
    if let Some(n) = flag(&args, "--cyclists") {
        scenario.actors.vru.cyclists = n.parse()?;
    }
    let bin_s: f64 = flag(&args, "--bin").map_or(Ok(1.0), |b| b.parse())?;
    let null = args.iter().any(|a| a == "--null");
    let quiet = args.iter().any(|a| a == "--quiet");
    if let Some(n) = flag(&args, "--threads") {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n.parse()?)
            .build_global()?;
    }
    println!(
        "scenario {} | {} s | threads {} | demand {} rate {:?} params {}",
        scenario.meta.name,
        scenario.time.duration_s,
        rayon::current_num_threads(),
        scenario.actors.vehicles.demand.kind,
        scenario.actors.vehicles.demand.rate_veh_per_h,
        scenario.actors.vehicles.demand.params,
    );
    println!(
        "vru pedestrians {} cyclists {}",
        scenario.actors.vru.pedestrians, scenario.actors.vru.cyclists
    );

    let t = Instant::now();
    let mut engine = Engine::build(scenario.clone(), "")?;
    println!("build {:.2} s", t.elapsed().as_secs_f64());

    let mut digest = DigestRecorder::new();
    let mut nothing = NullRecorder::new();
    let inner: &mut dyn RunRecorder = if null { &mut nothing } else { &mut digest };
    let mut timing = Timing {
        inner,
        bin_ns: (bin_s * 1e9) as u64,
        start: Instant::now(),
        marks: Vec::new(),
        counts: Vec::new(),
        actors: Vec::new(),
        txs: Vec::new(),
    };
    let report = engine.run(&mut timing)?;
    let total = timing.start.elapsed().as_secs_f64();
    let mut marks = timing.marks.clone();
    marks.push(total);
    let counts = timing.counts.clone();
    let step_s = scenario.time.mobility_step_ms as f64 / 1e3;
    let steps_per_bin = bin_s / step_s;

    // Only whole bins inside the horizon: the end-of-run instant opens a last bin that
    // holds a handful of records and no simulated time.
    let whole = ((scenario.time.duration_s / bin_s).floor() as usize).min(marks.len() - 1);
    let per_bin: Vec<f64> = marks.windows(2).take(whole).map(|w| w[1] - w[0]).collect();
    if !quiet {
        println!("\n bin(s)   wall ms   records  actors  tx/s   ms per actor-s");
        for (i, (dt, n)) in per_bin.iter().zip(&counts).enumerate() {
            let actors = timing.actors[i] as f64 / steps_per_bin;
            let tx = timing.txs[i] as f64 / bin_s;
            println!(
                "{:7.1} {:9.1} {:9} {:7.0} {:6.0} {:9.3}",
                i as f64 * bin_s,
                dt * 1e3,
                n,
                actors,
                tx,
                dt * 1e3 / (actors * bin_s).max(1e-9)
            );
        }
    }
    let k = (per_bin.len() / 10).max(1);
    // The first bin carries the spawn transient; the mean over the first and last tenths
    // is the flatness figure, reported beside the whole-run ratio.
    let head: f64 = per_bin.iter().skip(1).take(k).sum::<f64>() / k as f64;
    let tail: f64 = per_bin.iter().rev().take(k).sum::<f64>() / k as f64;
    println!("\nrun total        {total:.2} s wall for {} s simulated", scenario.time.duration_s);
    println!(
        "wall per sim s   {:.4} (whole run)",
        total / scenario.time.duration_s
    );
    println!(
        "per-bin mean     first tenth {:.1} ms, last tenth {:.1} ms, ratio {:.2}",
        head * 1e3,
        tail * 1e3,
        tail / head.max(1e-12)
    );
    println!(
        "report           nodes {} frames {} attempts {} decoded {} records {}",
        report.nodes_created,
        report.frames_transmitted,
        report.reception_attempts,
        report.receptions_ok,
        report.records
    );
    if !null {
        println!("content digest   {}", digest.digest_hex());
        println!("frame digest     {}", digest.frame_digest_hex());
        println!("records hashed   {}", digest.written());
    }
    Ok(())
}
