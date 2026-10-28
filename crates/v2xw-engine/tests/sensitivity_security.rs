//! Security, message and backend settings move the metrics a researcher reads.
//!
//! Every test runs a baseline and one setting on the same seed and the same small fleet,
//! and asserts the direction the cause predicts on the run report or on the published
//! metric samples (`metric.sample`). A setting that changed nothing fails its row. The
//! table these rows back is docs/validation/sensitivity-security.md.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::MetricSample;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A small procedural grid (5 avenues by 8 streets, about 1.1 km by 0.6 km) in open air,
/// with `n` vehicles spawned at once so every node hears the others from the first second.
fn fleet(n: u32, secs: f64) -> Scenario {
    let mut s = Scenario::load(root().join("scenarios/phase1-grid.yaml")).expect("loads");
    s.time.duration_s = secs;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(5);
        params["rows"] = json!(8);
    }
    s.world.buildings.enabled = false;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600_000.0);
    s.actors.vehicles.demand.params = json!({ "max_total_vehicles": n });
    s.security.verification_policy = "verify-all".to_string();
    s.metrics = vec!["all".to_string()];
    s
}

/// What one run measured: the report and each metric's sample-weighted mean over every
/// window and dimension (counts are summed).
struct Measured {
    report: RunReport,
    m: BTreeMap<String, f64>,
    last: BTreeMap<String, f64>,
    rec: MemoryRecorder,
}

impl Measured {
    fn get(&self, metric: &str) -> f64 {
        self.m.get(metric).copied().unwrap_or(0.0)
    }
}

fn run(s: Scenario) -> Measured {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the scenario should load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    let mut acc: BTreeMap<String, (f64, f64, bool)> = BTreeMap::new();
    let mut last: BTreeMap<String, f64> = BTreeMap::new();
    for (_, r) in rec.records() {
        if r.channel != "metric.sample" {
            continue;
        }
        let Ok(sample) = serde_json::from_slice::<MetricSample>(&r.json) else {
            continue;
        };
        let Some(p) = sample.value.point() else {
            continue;
        };
        let is_count = matches!(sample.value, v2xw_metrics::SampleValue::Count { .. });
        let mut keys = vec![sample.metric.clone()];
        if !sample.dims.is_empty() {
            let dims = serde_json::to_string(&sample.dims).unwrap_or_default();
            keys.push(format!("{}{dims}", sample.metric));
        }
        for key in keys {
            last.insert(key.clone(), p);
            let e = acc.entry(key).or_insert((0.0, 0.0, is_count));
            if is_count {
                e.0 += p;
                e.1 = 1.0;
            } else {
                let n = sample.value.n().max(1) as f64;
                e.0 += p * n;
                e.1 += n;
            }
        }
    }
    let m = acc
        .into_iter()
        .map(|(k, (s, n, _))| (k, s / n.max(1.0)))
        .collect();
    Measured {
        report,
        m,
        last,
        rec,
    }
}

fn refused(s: &Scenario, field: &str) -> bool {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .any(|e| e.to_string().contains(field))
}

/// How many `node.rx` fates name `cause` (the node-level losses: the run report's
/// `rx_losses` holds only the PHY's).
fn node_lost(m: &Measured, cause: &str) -> usize {
    let needle = format!("\"{cause}\"");
    m.rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "node.rx")
        .filter(|(_, r)| String::from_utf8_lossy(&r.json).contains(&needle))
        .count()
}

/// A rate metric's events over the whole run: the sum of each window's sample count, so a
/// busy window is not over-weighted the way a count-weighted mean of rates would be.
fn events(m: &Measured, metric: &str) -> u64 {
    m.rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "metric.sample")
        .filter_map(|(_, r)| serde_json::from_slice::<MetricSample>(&r.json).ok())
        .filter(|s| s.metric == metric)
        .map(|s| s.value.n())
        .sum()
}

fn pdr(r: &RunReport) -> f64 {
    r.receptions_ok as f64 / r.reception_attempts.max(1) as f64
}

/// Lists every metric a baseline run publishes, for writing rows (`--ignored`).
#[test]
#[ignore = "a probe, not a check"]
fn probe_metrics() {
    let b = run(fleet(30, 3.0));
    for (k, v) in &b.m {
        println!("{k} = {v:.4}");
    }
    println!("air {:?}", b.report.air_bytes);
    println!("phase2 states {:?}", b.report.phase2.verification_states);
    println!("losses {:?}", b.report.rx_losses);
    println!(
        "pdr {:.3} attempts {}",
        pdr(&b.report),
        b.report.reception_attempts
    );
    let _ = &b.rec;
}

/// The recorded bytes per channel of an attack run, largest first (`--ignored`): what an
/// events-level recording is made of.
#[test]
#[ignore = "a probe, not a check"]
fn probe_record_bytes() {
    let mut s = backend(30, 20.0);
    attacked(&mut s, 0.3);
    let m = run(s);
    let mut by: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for (_, r) in m.rec.records() {
        let e = by.entry(r.channel.to_string()).or_insert((0, 0));
        e.0 += 1;
        e.1 += r.json.len() as u64;
    }
    let mut v: Vec<_> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.1.cmp(&a.1.1));
    for (k, (n, b)) in v.iter().take(12) {
        println!("ROW bytes {k}: {n} records {b} bytes");
    }
    for ch in ["node.rx", "node.verify", "phy.rx"] {
        if let Some((_, r)) = m.rec.records().iter().find(|(_, r)| r.channel == ch) {
            println!("ROW sample {ch}: {}", String::from_utf8_lossy(&r.json));
        }
    }
}

/// The SCMS lifecycle on a compressed calendar (20 s i-periods, a top-up as soon as only
/// the current period is left), with a cellular modem in every vehicle.
fn backend(n: u32, secs: f64) -> Scenario {
    let mut s = fleet(n, secs);
    let id = v2xw_engine::phase2::CAMP_SCMS.to_string();
    s.actors.backend.protocol = Some(id.clone());
    s.security.protocol = Some(ModelChoice {
        id,
        params: json!({
            "i_period_s": 20,
            "cert_lifetime_s": 21,
            "certs_per_period": 3,
            "pool_periods": 2,
            "topup_below_periods": 1,
            "cert_shuffle_window_s": 1,
            "first_batch_delay_s": 1,
            "download_poll_interval_s": 1
        }),
    });
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s
}

fn exchange_ms(r: &RunReport, kind: &str) -> (u64, f64) {
    r.phase2
        .access
        .exchange_latency
        .get(kind)
        .map_or((0, 0.0), |e| {
            (e.count, e.sum_ns as f64 / e.count.max(1) as f64 / 1e6)
        })
}

fn row(name: &str, m: &Measured) {
    let r = &m.report;
    println!(
        "ROW {name}: sec_bytes {} frags {} sec_overhead {:.3} env_overhead {:.3} \
         full_cert {:.3} cbr {:.4} pdr {:.4} e2e_ms {:.3} verify_rate {:.0} verify_wait {:.4} \
         verify_q {:.4} unverified {:.4} aoi {:.1} frames {} air_bytes {} losses {:?}",
        r.air_bytes.security,
        r.fragments_sent,
        m.get("security_overhead"),
        m.get("envelope_overhead"),
        m.get("full_cert_share"),
        m.get("cbr"),
        pdr(r),
        m.get("e2e_latency"),
        m.get("verify_rate"),
        m.get("verify_wait"),
        m.get("verify_queue_depth"),
        m.get("unverified_ratio"),
        m.get("aoi"),
        r.frames_transmitted,
        r.air_bytes.psdu,
        r.rx_losses,
    );
}

/// ECDSA P-256 -> hybrid ML-DSA-44 + ECDSA -> pure ML-DSA-44: the signature and the
/// certificate grow (64 B -> about 2.5 kB -> 2.4 kB signature; FIPS 204 Table 2), so the
/// security bytes, the fragments over the MTU, the security share of the air, the
/// channel busy ratio and the end-to-end latency rise, and delivery does not improve.
#[test]
fn the_signature_scheme_moves_bytes_airtime_and_latency() {
    let base = || {
        let mut s = fleet(30, 3.0);
        s.nodes.default_obu = "obu/generic-automotive-soc-no-hsm".to_string();
        s.net.fragmenter = Some(ModelChoice::new("fragmenter/generic-sdu"));
        s
    };
    let mut out = Vec::new();
    for scheme in ["ecdsa-p256", "hybrid-mldsa44-ecdsa-p256", "ml-dsa-44"] {
        let mut s = base();
        s.security.signature = scheme.to_string();
        let m = run(s);
        row(scheme, &m);
        out.push(m);
    }
    let (e, h, q) = (&out[0], &out[1], &out[2]);
    assert!(h.report.air_bytes.security > 5 * e.report.air_bytes.security);
    assert!(q.report.air_bytes.security > 5 * e.report.air_bytes.security);
    assert!(h.get("security_overhead") > e.get("security_overhead"));
    assert!(q.get("security_overhead") > e.get("security_overhead"));
    assert!(
        q.report.fragments_sent > e.report.fragments_sent,
        "nothing fragmented"
    );
    assert!(h.get("cbr") > e.get("cbr") && q.get("cbr") > e.get("cbr"));
    assert!(q.get("e2e_latency") > e.get("e2e_latency"));
    assert!(pdr(&q.report) <= pdr(&e.report));
}

/// A certificate on every message (`full_cert_every_ms: 0`) against once a second
/// (J2945/1 §6.1.2.2's default) and once every 5 s: the share of messages carrying a
/// certificate and the security bytes follow the cadence.
#[test]
fn the_certificate_attach_cadence_moves_overhead() {
    let mut out = Vec::new();
    for every in [0u64, 1000, 5000] {
        let mut s = fleet(30, 6.0);
        s.security.signer_id_policy.full_cert_every_ms = every;
        s.security.signer_id_policy.digest_otherwise = every > 0;
        let m = run(s);
        row(&format!("cert every {every} ms"), &m);
        out.push(m);
    }
    let (all, hz, slow) = (&out[0], &out[1], &out[2]);
    assert!(
        all.get("full_cert_share") > 0.9,
        "every message carries one"
    );
    assert!(all.get("full_cert_share") > hz.get("full_cert_share"));
    assert!(hz.get("full_cert_share") > slow.get("full_cert_share"));
    assert!(all.report.air_bytes.security > hz.report.air_bytes.security);
    assert!(hz.report.air_bytes.security > slow.report.air_bytes.security);
    assert!(all.get("security_overhead") > slow.get("security_overhead"));
    // Fewer certificates on the air means more digests whose certificate a receiver has
    // not cached, and so more peer-to-peer certificate requests (IEEE 1609.2 clause 8).
    let p2pcd = |m: &Measured| events(m, "digest_misses");
    println!(
        "ROW p2pcd: every msg {}, 1 Hz {}, 5 s {}",
        p2pcd(all),
        p2pcd(hz),
        p2pcd(slow)
    );
    assert!(
        p2pcd(all) < p2pcd(hz),
        "a certificate on every message needs no request"
    );
    assert!(p2pcd(hz) < p2pcd(slow));
}

/// Verify-all against verify-on-demand: on demand verifies only what an application acts
/// on, so the verification rate and the queue fall and more messages reach applications
/// unverified.
#[test]
fn the_verification_policy_moves_the_verify_load() {
    let mut all = fleet(30, 3.0);
    all.nodes.default_obu = "obu/unex-obu-301-craton2-software".to_string();
    let mut od = all.clone();
    od.security.verification_policy = "on-demand".to_string();
    let a = run(all);
    let o = run(od);
    row("verify-all", &a);
    row("on-demand", &o);
    assert!(o.get("verify_rate") < a.get("verify_rate"));
    assert!(o.get("unverified_ratio") > a.get("unverified_ratio"));
    // WP-C2 prices receiver validation into a frame's service time: the first frame from
    // a signer pays for its certificate (reconstruction, chain), later ones hit the cache.
    // `on-demand` with no applications verifies only strangers, so every frame it checks
    // is such a miss: its mean verify cost is higher and its frames queue behind full
    // validations, so `verify_wait` is no longer lower. What the policy still moves is
    // the load: the verifier is busy for far less time in all.
    let busy = |m: &Measured| m.get("verify_rate") * m.get("verify_cost");
    assert!(
        busy(&o) < busy(&a),
        "on-demand kept the verifier busier: {} vs {}",
        busy(&o),
        busy(&a)
    );
    assert!(o.get("verify_cost") > a.get("verify_cost"));
    let (rx_o, rx_a) = (&o.report.receiver_validation, &a.report.receiver_validation);
    assert_eq!(
        rx_o.ee_validations, rx_o.checked,
        "on-demand verifies only strangers, each a certificate the node had not validated"
    );
    assert!(rx_a.ee_validations < rx_a.checked);
}

/// The crypto hardware: the Craton2 HSM verifies in hundreds of microseconds, the same
/// unit verifying in software and a SoC without an HSM take longer, so the verify cost and
/// the end-to-end latency rise.
#[test]
fn the_crypto_hardware_moves_the_verify_latency() {
    let mut out = Vec::new();
    for obu in [
        "obu/unex-obu-301-craton2",
        "obu/unex-obu-301-craton2-software",
        "obu/generic-automotive-soc-no-hsm",
    ] {
        let mut s = fleet(30, 3.0);
        s.nodes.default_obu = obu.to_string();
        let m = run(s);
        row(obu, &m);
        for (k, v) in m.m.iter().filter(|(k, _)| k.starts_with("latency_stage{")) {
            println!("   {k} = {v:.4}");
        }
        println!("   verify_cost = {:.4}", m.get("verify_cost"));
        out.push(m);
    }
    // The Craton2 signs inside its eHSM (< 9 ms, R7 §A4) and verifies on a line-rate
    // engine (<= 400 us); in software the same unit signs in 613 us and verifies in
    // 1290 us (R7 §D8). The stages follow the profile and the end-to-end latency follows
    // the sum: the HSM unit is slower end to end because it signs slower.
    let (hsm, sw) = (&out[0], &out[1]);
    let stage = |m: &Measured, st: &str| m.get(&format!("latency_stage{{\"stage\":\"{st}\"}}"));
    assert!(stage(sw, "verify") > stage(hsm, "verify"));
    assert!(stage(sw, "sign") < stage(hsm, "sign"));
    assert!(sw.get("verify_wait") > hsm.get("verify_wait"));
    assert!(sw.get("e2e_latency") < hsm.get("e2e_latency"));
}

/// A one-slot verification queue against the default 64, under a dense fleet verifying
/// in software: the short queue overflows and drops.
#[test]
fn the_verification_queue_depth_moves_drops_under_load() {
    let mut deep = fleet(60, 3.0);
    deep.nodes.default_obu = "obu/unex-obu-301-craton2-software".to_string();
    let mut short = deep.clone();
    short.nodes.verify_queue_depth = Some(1);
    let d = run(deep);
    let s = run(short);
    row("queue 64", &d);
    row("queue 1", &s);
    let drops = |m: &Measured| node_lost(m, "verify-overflow");
    println!("ROW queue: overflow {} vs {}", drops(&s), drops(&d));
    assert!(drops(&s) > drops(&d), "a one-slot queue dropped no more");
}

/// A 5 s pseudonym period against J2945/1's 300 s on a 20 s run: changes happen, and each
/// new certificate is first heard by digest, so more messages arrive from an unknown signer
/// (the P2PCD case).
#[test]
fn the_pseudonym_period_moves_changes_and_unknown_signers() {
    let mut slow = backend(20, 20.0);
    slow.security.signer_id_policy.full_cert_every_ms = 5000;
    let mut fast = slow.clone();
    fast.security.pseudonym_change.period_s = Some(5.0);
    let s = run(slow);
    let f = run(fast);
    row("period 300 s", &s);
    row("period 5 s", &f);
    let unknown = |m: &Measured| node_lost(m, "unknown-signer");
    println!(
        "ROW pseudonym: changes {} vs {}, unknown signer {} vs {}",
        f.report.phase2.pseudonym_changes,
        s.report.phase2.pseudonym_changes,
        unknown(&f),
        unknown(&s)
    );
    assert!(f.report.phase2.pseudonym_changes > s.report.phase2.pseudonym_changes);
    assert!(unknown(&f) > unknown(&s));
}

/// A pseudonym change every 5 s against the same change followed by a 2-4 s radio silence
/// (Huang et al., "Silent cascade", IEEE SECON 2005): the observer (the Wiedersheim et al.
/// WONS 2010 kinematic tracker, global by default) bridges fewer changes after a silence,
/// so linkability falls. The silence also costs frames on the air. A short period also
/// raises the peer-to-peer certificate requests against the 300 s baseline: every change
/// is a certificate the neighbours have not cached until its next attachment.
#[test]
fn the_pseudonym_strategy_moves_linkability_and_p2pcd() {
    // 20 s, so the 300 s baseline changes nothing before the first i-period ends, and the
    // 1 s certificate cadence (J2945/1's order) is well inside the 5 s period: with a 5 s
    // cadence the count would follow where each change falls in the cadence rather than
    // how often changes happen.
    let base = backend(20, 20.0);
    let mut time = base.clone();
    time.security.pseudonym_change.period_s = Some(5.0);
    let mut silent = time.clone();
    silent.security.pseudonym_change.silent_period_s = Some(vec![2.0, 4.0]);
    let b = run(base);
    let t = run(time.clone());
    let q = run(silent.clone());
    let linkability = |m: &Measured| {
        m.report.phase2.privacy_links_correct as f64
            / m.report.phase2.pseudonym_changes.max(1) as f64
    };
    for (name, m) in [("300 s", &b), ("5 s", &t), ("5 s + silence", &q)] {
        println!(
            "ROW strategy {name}: changes {} links {}/{} linkability {:.3} metric {:.3} \
             p2pcd {} unknown {} frames {}",
            m.report.phase2.pseudonym_changes,
            m.report.phase2.privacy_links_correct,
            m.report.phase2.privacy_links_claimed,
            linkability(m),
            m.last.get("linkability").copied().unwrap_or(f64::NAN),
            events(m, "digest_misses"),
            node_lost(m, "unknown-signer"),
            m.report.frames_transmitted
        );
    }
    assert!(t.report.phase2.pseudonym_changes > b.report.phase2.pseudonym_changes);
    assert!(t.get("digest_misses") > b.get("digest_misses"));
    assert!(
        t.report.phase2.privacy_links_correct > 0,
        "the observer links some changes"
    );
    assert!(
        linkability(&q) < linkability(&t),
        "a silence hides the change"
    );
    assert!(q.report.frames_transmitted < t.report.frames_transmitted);
    // The frames a silence discards were signed, but their certificates reached no one:
    // the first frame after it must still carry one. Before the fix the cadence counted
    // them and unknown signers rose from 843 to 4,051.
    //
    // The mechanism, on the records: the first signed frame each vehicle generates after
    // each change carries the full certificate.
    let opened = first_frames_after_changes(&q);
    assert!(opened.0 > 0, "the silence run changed no pseudonym");
    assert_eq!(
        opened.1, 0,
        "a silence hid the new certificate: {} of {} changes opened with a digest",
        opened.1, opened.0
    );
    // The count, pooled over three seeds: all 20 vehicles spawn at once, so one seed's
    // unknown signers follow a single hidden-terminal collision pattern (897 against 731
    // on the scenario's seed, every one a certificate frame lost on the air), not the
    // mechanism; seeds 2 and 3 give 760 -> 579 and 1,009 -> 702
    // (docs/validation/live-scms-2026-10-25.md, 2026-10-26).
    let (mut lost_t, mut lost_q) = (
        node_lost(&t, "unknown-signer"),
        node_lost(&q, "unknown-signer"),
    );
    for seed in [2_u64, 3] {
        let mut ts = time.clone();
        ts.seed = seed;
        let mut qs = silent.clone();
        qs.seed = seed;
        let (tm, qm) = (run(ts), run(qs));
        let opened = first_frames_after_changes(&qm);
        assert_eq!(
            opened.1, 0,
            "seed {seed}: a silence hid the new certificate"
        );
        lost_t += node_lost(&tm, "unknown-signer");
        lost_q += node_lost(&qm, "unknown-signer");
    }
    println!("ROW strategy pooled seeds: unknown 5 s {lost_t} 5 s + silence {lost_q}");
    assert!(
        lost_q <= lost_t,
        "a silence must not hide the new certificate: {lost_q} unknown signers against {lost_t}"
    );
}

/// Per run: how many pseudonym changes it made, and after how many of them the first
/// signed frame the vehicle generated carried a digest rather than the new certificate.
fn first_frames_after_changes(m: &Measured) -> (usize, usize) {
    use serde_json::Value;
    let mut changes: Vec<(u64, u64)> = Vec::new();
    let mut frames: BTreeMap<u64, Vec<(u64, bool)>> = BTreeMap::new();
    for (_, r) in m.rec.records() {
        if r.channel != "sec.pseudonym" && r.channel != "node.tx" {
            continue;
        }
        let Ok(o) = serde_json::from_slice::<Value>(&r.json) else {
            continue;
        };
        let n = |k: &str| o.get(k).and_then(Value::as_u64);
        let (Some(node), Some(t)) = (n("node"), n("t")) else {
            continue;
        };
        if r.channel == "sec.pseudonym" {
            changes.push((node, t));
            continue;
        }
        let Some(signer) = o.get("signer").and_then(Value::as_str) else {
            continue;
        };
        let generated = n("t_generated").unwrap_or(t);
        frames
            .entry(node)
            .or_default()
            .push((generated, signer == "certificate"));
    }
    let digest_first = changes
        .iter()
        .filter(|(node, at)| {
            frames
                .get(node)
                .and_then(|f| f.iter().filter(|(g, _)| g >= at).min_by_key(|(g, _)| *g))
                .is_some_and(|(_, cert)| !cert)
        })
        .count();
    (changes.len(), digest_first)
}

/// The BSM's Part II path history: none, 1 point, and J2945/1's 15. Each point is about
/// 8 to 9 bytes of UPER, so the payload and the bytes on the air rise with the cap, and
/// the security envelope's share of the payload falls.
#[test]
fn the_path_history_length_moves_part_ii_bytes() {
    let mut out = Vec::new();
    for points in [0u32, 1, 15] {
        let mut s = fleet(30, 12.0);
        // 15 is the default; leaving it unset keeps the run as it was.
        s.messages.path_history_points = (points != 15).then_some(points);
        let m = run(s);
        row(&format!("path history {points}"), &m);
        out.push(m);
    }
    let (none, one, full) = (&out[0], &out[1], &out[2]);
    // A 12 s drive on a straight grid keeps about one point (a chord may run 300 m), so
    // the cap binds at 1 and not between 1 and 15: only a long, curving path fills 15.
    let payload = |m: &Measured| m.report.air_bytes.payload;
    println!(
        "ROW path history: payload {} / {} / {} B, air {} / {} / {} B, env/payload {:.3} / {:.3} / {:.3}",
        payload(none),
        payload(one),
        payload(full),
        none.report.air_bytes.psdu,
        one.report.air_bytes.psdu,
        full.report.air_bytes.psdu,
        none.get("envelope_overhead"),
        one.get("envelope_overhead"),
        full.get("envelope_overhead"),
    );
    assert!(payload(none) < payload(one));
    assert!(payload(one) <= payload(full));
    assert!(none.report.air_bytes.psdu < full.report.air_bytes.psdu);
    assert!(none.get("envelope_overhead") > full.get("envelope_overhead"));

    // Refused where it would do nothing, and past the J2735 limit.
    let mut no_awareness = fleet(5, 1.0);
    no_awareness.messages.sets = vec!["spat".to_string()];
    no_awareness.messages.path_history_points = Some(4);
    assert!(refused(&no_awareness, "messages.path_history_points"));
    let mut too_many = fleet(5, 1.0);
    too_many.messages.path_history_points = Some(24);
    assert!(refused(&too_many, "messages.path_history_points"));
}

/// 5 Hz against 10 Hz against 20 Hz BSMs: frames, bytes and the channel busy ratio rise
/// with the rate and the age of information falls.
#[test]
fn the_message_rate_moves_bytes_cbr_and_aoi() {
    let mut out = Vec::new();
    for hz in [5.0, 10.0, 20.0] {
        let mut s = fleet(30, 3.0);
        // 20 Hz needs a 50 ms step (one generator run per step); all three share it.
        s.time.mobility_step_ms = 50;
        s.messages.rate_hz = Some(hz);
        let m = run(s);
        row(&format!("{hz} Hz"), &m);
        out.push(m);
    }
    for w in out.windows(2) {
        assert!(w[1].report.frames_transmitted > w[0].report.frames_transmitted);
        // Doubling the rate roughly doubles the airtime.
        let ratio = w[1].report.air_bytes.airtime_us as f64
            / w[0].report.air_bytes.airtime_us.max(1) as f64;
        assert!(
            (1.6..2.4).contains(&ratio),
            "airtime ratio {ratio:.2} for twice the rate"
        );
        assert!(w[1].get("aoi") < w[0].get("aoi"));
    }
    // The CBR the MAC reports is its last 100 ms at each 1 s metric flush. A fleet spawned
    // in one instant used to send at 5 Hz on the even 100 ms slots only (the phase window
    // was 100 ms whatever the period), so that sample read about 0 at 5 Hz. The phase now
    // spans the period, and the CBR doubles at each step.
    for w in out.windows(2) {
        let cbr = w[1].get("cbr") / w[0].get("cbr").max(1e-9);
        assert!(
            (1.5..2.5).contains(&cbr),
            "CBR ratio {cbr:.2} for twice the rate"
        );
    }
}

/// IEEE 1609.2 against ETSI TS 103 097 over GeoNetworking. TS 103 097 is a profile of
/// 1609.2 with identical bytes (§5.1-5.2), so a CAM costs the same under both; what
/// differs is that a DENM must be signed with a full certificate (§7.1.2), so a run with
/// warnings carries more security bytes under the ETSI profile.
#[test]
fn the_envelope_moves_the_security_bytes() {
    let mut us = fleet(30, 40.0);
    // DENMs repeat once a second (EN 302 637-3), so at the default 1 s cadence every one
    // carries a certificate under either profile; a 5 s cadence leaves 1609.2 sending
    // digests between certificates while TS 103 097 still certifies every DENM.
    us.security.signer_id_policy.full_cert_every_ms = 5000;
    us.net.layer = "gn-btp".to_string();
    us.messages.sets = vec!["cam".into(), "denm".into()];
    // A full ABS stop (8 m/s², above UN R48's 6 m/s² emergency-stop signal) raises the
    // emergency-brake-light DENM; a breakdown raises the stationary-vehicle one once its
    // Triggering Timer (at most 30 s standing with hazards on) has run.
    us.events = serde_json::from_value(json!([
        {"t": 4.0, "until": 40.0, "type": "safety.breakdown"},
        {"t": 10.0, "type": "safety.hard-brake", "decel_mps2": 8.0},
    ]))
    .expect("timeline items");
    let mut eu = us.clone();
    eu.security.envelope = "etsi103097".to_string();
    let u = run(us);
    let e = run(eu);
    row("ieee1609.2", &u);
    row("etsi103097", &e);
    let denms = |m: &Measured| {
        m.rec
            .records()
            .iter()
            .filter(|(_, r)| r.channel == "node.tx")
            .filter(|(_, r)| String::from_utf8_lossy(&r.json).contains("\"denm\""))
            .count()
    };
    println!("ROW envelope: denm tx {} vs {}", denms(&u), denms(&e));
    assert!(denms(&u) > 0, "no DENM was sent");
    assert!(e.report.air_bytes.security > u.report.air_bytes.security);
}

/// The cellular link: a 5G first hop against 4G to the West Coast changes how long a
/// certificate top-up takes; half the fleet without a modem leaves fewer vehicles able to
/// reach the backend.
#[test]
fn the_cellular_link_moves_topup_latency_and_availability() {
    let preset = |p: &str, penetration: f64| {
        let mut s = backend(20, 45.0);
        s.net.uu = Some(ModelChoice {
            id: "cellular/uu/fixed-latency".to_string(),
            params: json!({"preset": p, "penetration": penetration}),
        });
        run(s)
    };
    let fast = preset("5g-first-hop", 1.0);
    let slow = preset("4g-west-coast", 1.0);
    let half = preset("4g-east-coast", 0.5);
    for (n, m) in [
        ("5g-first-hop", &fast),
        ("4g-west-coast", &slow),
        ("half", &half),
    ] {
        let p = &m.report.phase2;
        println!(
            "ROW uu {n}: topup {:?} cellular {} topups {}/{} starved {} unavailable_ms {}",
            exchange_ms(&m.report, "topup"),
            p.access.cellular_vehicles,
            p.topups_completed,
            p.topups_started,
            p.vehicles_starved,
            p.cert_unavailable_ns / 1_000_000
        );
    }
    let (nf, tf) = exchange_ms(&fast.report, "topup");
    let (ns, ts) = exchange_ms(&slow.report, "topup");
    assert!(nf > 0 && ns > 0, "no top-up completed");
    assert!(
        ts > tf,
        "4g-west-coast {ts} ms against 5g-first-hop {tf} ms"
    );
    assert!(
        half.report.phase2.access.cellular_vehicles < fast.report.phase2.access.cellular_vehicles
    );
}

/// `fraction` of the fleet broadcasting a constant false position (F2MD's ConstPos), the
/// legacy detector suite on every node and the corroborating misbehaviour authority.
fn attacked(s: &mut Scenario, fraction: f64) {
    if fraction > 0.0 {
        s.threats.attackers = serde_json::from_value(json!([
            {"id": "threat/attacker/legacy/ConstPos", "fraction": fraction}
        ]))
        .expect("attackers");
    }
    s.detection = serde_json::from_value(json!({
        "local": [{"id": "detect/legacy-12"}],
        "ma": {"id": "threat/ma/corroborated"}
    }))
    .expect("detection");
}

/// The CRL fetch interval: a new list is published only when the authority revokes, and a
/// vehicle polls the distribution centre every `crl_fetch_interval_s` for a version newer
/// than its own. With attackers revoked inside the run, polling every 5 s against every
/// 30 s downloads sooner, so revocation reaches the fleet faster.
#[test]
fn the_crl_fetch_interval_moves_downloads_and_revocation_latency() {
    let with = |cadence: u64, fetch: u64| {
        let mut s = backend(30, 60.0);
        attacked(&mut s, 0.2);
        // One provider serves the whole fleet (WP-D1): with the fleet split, each
        // provider's CRL Generator publishes on its own clock, and the first revocation
        // the report times is then whichever provider's vehicle polls first, so the fetch
        // interval would pick a different case rather than move this case's last leg.
        s.security.scms = Some(
            serde_json::from_value(json!({"deployment": {"providers": [
                {"name": "provider-a", "fleet_share": 1.0},
                {"name": "provider-b", "fleet_share": 0.0}
            ]}}))
            .expect("an scms section"),
        );
        if let Some(p) = s.security.protocol.as_mut() {
            p.params["report_shuffle_window_s"] = json!(5);
            p.params["crl_cadence_s"] = json!(cadence);
            p.params["crl_fetch_interval_s"] = json!(fetch);
        }
        run(s)
    };
    let often = with(10, 5);
    let rare = with(10, 30);
    let slow_list = with(30, 5);
    for (n, m) in [
        ("cadence 10 fetch 5", &often),
        ("cadence 10 fetch 30", &rare),
        ("cadence 30 fetch 5", &slow_list),
    ] {
        let p = &m.report.phase2;
        println!(
            "ROW crl {n}: revoked {} versions {} downloads {} installed {} fetch {:?} \
             uu_dl {} revocation_latency_ms {} stages {:?}",
            p.revoked_attackers,
            p.crl_versions_published,
            p.crl_downloads,
            p.crls_installed,
            exchange_ms(&m.report, "crl-fetch"),
            p.access.uu_dl_bytes,
            p.revocation_latency_ns / 1_000_000,
            p.revocation_stages
        );
    }
    assert!(
        often.report.phase2.crl_versions_published > 0,
        "nothing was revoked"
    );
    let stage = |m: &Measured, st: &str| {
        m.report
            .phase2
            .revocation_stages
            .iter()
            .find(|(s, _)| s == st)
            .map(|(_, t)| *t)
    };
    // The fetch interval moves the last leg (published -> enforced), not publication.
    assert_eq!(stage(&often, "published"), stage(&rare, "published"));
    assert!(rare.report.phase2.revocation_latency_ns > often.report.phase2.revocation_latency_ns);
    // The cadence moves publication itself.
    assert!(stage(&slow_list, "published") > stage(&often, "published"));
    assert!(
        slow_list.report.phase2.revocation_latency_ns > often.report.phase2.revocation_latency_ns
    );
}

/// The CRL's size is not a setting: it follows the revocations. A larger attacker share
/// revokes more vehicles, so the published list holds more entries and bytes, and every
/// download costs more.
#[test]
fn the_revocations_move_the_crl_size() {
    let with = |fraction: f64| {
        let mut s = backend(30, 60.0);
        attacked(&mut s, fraction);
        if let Some(p) = s.security.protocol.as_mut() {
            p.params["report_shuffle_window_s"] = json!(5);
            p.params["crl_cadence_s"] = json!(10);
            // A 30 s poll, not 5 s: the linkage check below needs a revoked device to be
            // reported under its next pseudonym before the receivers hold the CRL that
            // revokes it. With a 5 s poll every receiver holds it first, so the next
            // pseudonym is refused, not reported (0.3: 35,094 revoked receptions and 0
            // linked verdicts at 5 s; 16,086 and 7 at 30 s; 2,876 and 7 at 120 s;
            // docs/design/scms-real/testing/rerun-2026-10-22.md §2). The poll moves how
            // soon receivers enforce, not what one revocation adds to the list, which is
            // what this test sizes.
            p.params["crl_fetch_interval_s"] = json!(30);
        }
        run(s)
    };
    let few = with(0.1);
    let many = with(0.3);
    let last = |m: &Measured, k: &str| m.last.get(k).copied().unwrap_or(0.0);
    for (n, m) in [("0.1", &few), ("0.3", &many)] {
        let p = &m.report.phase2;
        println!(
            "ROW crl size {n}: revoked {} entries {:.0} bytes {:.0} versions {} downloads {} \
             uu_dl {} decisions {} linked {} repeated {} attackers {}",
            p.revoked_attackers + p.revoked_honest,
            last(m, "crl_entries"),
            last(m, "crl_bytes"),
            p.crl_versions_published,
            p.crl_downloads,
            p.access.uu_dl_bytes,
            p.ma_revoke_decisions,
            p.decisions_linked,
            p.revocations_repeated,
            p.attackers,
        );
        // One decision per device: the linkage authorities tie a device's pseudonyms to
        // one chain, so no device is revoked twice and no device is decided twice.
        assert_eq!(p.revocations_repeated, 0, "a device was revoked twice");
        assert!(
            p.ma_revoke_decisions <= p.attackers + p.decisions_honest,
            "more decisions than devices"
        );
    }
    assert!(
        many.report.phase2.decisions_linked > 0,
        "no second pseudonym was linked to a decided device"
    );
    assert!(
        few.report.phase2.revoked_attackers > 0,
        "nothing was revoked"
    );
    assert!(last(&many, "crl_entries") > last(&few, "crl_entries"));
    assert!(last(&many, "crl_bytes") > last(&few, "crl_bytes"));
}

/// The attacker share: none, 10 % and 30 % of the fleet broadcasting a constant false
/// position. Verdicts, reports about attackers and the authority's true positives rise with
/// the share; an honest fleet has no true positive, and its false positives are what the
/// detectors file against honest vehicles (the confusion matrix, cumulative, read at the
/// last window).
#[test]
fn the_attacker_share_moves_detections() {
    let with = |fraction: f64| {
        let mut s = backend(30, 30.0);
        attacked(&mut s, fraction);
        if let Some(p) = s.security.protocol.as_mut() {
            p.params["report_shuffle_window_s"] = json!(5);
        }
        run(s)
    };
    let out = [with(0.0), with(0.1), with(0.3)];
    for (n, m) in ["0", "0.1", "0.3"].iter().zip(&out) {
        let p = &m.report.phase2;
        println!(
            "ROW attackers {n}: attackers {} falsified {} verdicts {} reports_at_ma {} \
             tp {:.0} fp {:.0} fn {:.0} about_attackers {} about_honest {} revoked {}/{}",
            p.attackers,
            p.falsified_claims,
            p.verdicts_fired,
            p.reports_at_ma,
            m.last
                .get("det_tp{\"level\":\"report\",\"cell\":\"tp\"}")
                .copied()
                .unwrap_or(0.0),
            m.last
                .get("det_fp{\"level\":\"report\",\"cell\":\"fp\"}")
                .copied()
                .unwrap_or(0.0),
            m.last
                .get("det_fn{\"level\":\"report\",\"cell\":\"fn\"}")
                .copied()
                .unwrap_or(0.0),
            p.reports_about_attackers,
            p.reports_about_honest,
            p.revoked_attackers,
            p.revoked_honest
        );
    }
    let tp = |m: &Measured| {
        m.last
            .get("det_tp{\"level\":\"report\",\"cell\":\"tp\"}")
            .copied()
            .unwrap_or(0.0)
    };
    let (none, few, many) = (&out[0], &out[1], &out[2]);
    assert_eq!(tp(none), 0.0);
    assert_eq!(none.report.phase2.reports_about_attackers, 0);
    assert!(few.report.phase2.attackers < many.report.phase2.attackers);
    assert!(few.report.phase2.verdicts_fired > none.report.phase2.verdicts_fired);
    assert!(many.report.phase2.verdicts_fired > few.report.phase2.verdicts_fired);
    assert!(
        tp(few) > 0.0,
        "no report about an attacker reached the authority"
    );
    assert!(tp(many) > tp(few));
}

/// The roadside backhaul: vehicles with no modem top up through provisioning-proxy units on
/// a 300 m lattice. The exchange is a request, an acknowledgement and download polls every
/// `download_poll_interval_s` (1 s here) until the batch is ready, so its length is set by
/// the polls as much as by the line. On lines that need the same number of polls, a 600 ms
/// line is slower than a 200 ms one and a 128 kbit/s line slower than the default. Until
/// RAPRE a 5 ms line was not the fastest: its first poll reached the authority before the
/// batch was ready and needed a second poll. With the RA pre-generating, the batch is
/// ready when asked for, so every line sends the same messages and the 5 ms line is the
/// fastest.
/// Each unit's line also carries its CRL fetches. At the start provider-b's list reaches
/// provider-a's RA one inter-provider hop after its own, so each unit fetches twice. Until
/// 2026-10-27 a fetch still on a 200 ms line installed the composite as it was on landing,
/// both lists, and the unit never fetched the second: 264 messages against the 5 ms
/// line's 272, though the top-up messages were identical (scms-fix4-b BH.cause). A
/// download now carries the file as the RA served it.
/// (The thin line was 64 kbit/s until wave 4 of the SCMS realism plan: real enrolment
/// certificates and stored-zip batch files made each exchange longer, so a 64 kbit/s line
/// needed one extra poll and no longer met the same-polls precondition.)
#[test]
fn the_backhaul_latency_and_bandwidth_move_relay_topups() {
    let with = |params: serde_json::Value| {
        let mut s = backend(20, 45.0);
        s.net.uu = None;
        for x in [150.0, 450.0, 750.0, 1050.0] {
            for y in [150.0, 450.0] {
                s.actors.rsus.push(v2xw_engine::scenario::Rsu {
                    site: None,
                    position_m: Some([x, y, 0.0]),
                    roles: vec!["provisioning-proxy".to_string()],
                    profile: None,
                    backhaul: Some("backhaul/fixed".to_string()),
                    backhaul_params: Some(params.clone()),
                });
            }
        }
        run(s)
    };
    let quick = with(json!({"latency_ms": 5.0}));
    let base = with(json!({"latency_ms": 200.0}));
    let slow = with(json!({"latency_ms": 600.0}));
    let thin = with(json!({"latency_ms": 200.0, "capacity_mbps": 0.128, "down_mbps": 0.128}));
    let msgs = |m: &Measured| m.report.phase2.access.carrier.backhaul_messages;
    for (n, m) in [
        ("5 ms", &quick),
        ("200 ms", &base),
        ("600 ms", &slow),
        ("200 ms 128 kbit/s", &thin),
    ] {
        let p = &m.report.phase2;
        println!(
            "ROW backhaul {n}: topup {:?} relay {} topups {}/{} messages {} bytes {}",
            exchange_ms(&m.report, "topup"),
            p.access.relay_only_vehicles,
            p.topups_completed,
            p.topups_started,
            msgs(m),
            p.access.backhaul_bytes
        );
    }
    let (nb, tb) = exchange_ms(&base.report, "topup");
    let (_, ts) = exchange_ms(&slow.report, "topup");
    let (_, tt) = exchange_ms(&thin.report, "topup");
    assert!(nb > 0, "no top-up went through a roadside unit");
    assert_eq!(
        msgs(&base),
        msgs(&slow),
        "the comparison needs the same polls"
    );
    assert_eq!(
        msgs(&base),
        msgs(&thin),
        "the comparison needs the same polls"
    );
    assert!(ts > tb, "600 ms: {ts} ms against 200 ms: {tb} ms");
    assert!(tt > tb, "128 kbit/s: {tt} ms against the default: {tb} ms");
    let (_, tq) = exchange_ms(&quick.report, "topup");
    assert!(tq < tb, "5 ms: {tq} ms against 200 ms: {tb} ms");
    // RAPRE: a top-off downloads a batch the RA pre-generated, ready when the `.info`
    // answer arrives, so a fast line no longer reaches the RA before its batch and polls
    // again: every line sends the same messages and only the time moves.
    assert_eq!(
        msgs(&quick),
        msgs(&base),
        "a pre-generated batch needs no extra poll on a fast line"
    );
}

/// Certificate on a new neighbour (TS 103 097 V1.3.1 §7.1.1) on top of a 5 s cadence, with
/// pseudonyms changing every 5 s so new stations keep appearing: more messages carry the
/// certificate, the security bytes rise, and fewer messages arrive from a signer the
/// receiver cannot yet verify. The loader refuses the setting where every message already
/// carries the certificate.
#[test]
fn the_new_neighbour_attach_moves_overhead_and_unknown_signers() {
    let mut base = backend(20, 20.0);
    base.security.signer_id_policy.full_cert_every_ms = 5000;
    base.security.pseudonym_change.period_s = Some(5.0);
    let mut on = base.clone();
    on.security.signer_id_policy.on_new_neighbour = true;
    let b = run(base.clone());
    let o = run(on);
    row("cadence 5 s", &b);
    row("cadence 5 s + new neighbour", &o);
    println!(
        "ROW new-neighbour: unknown signer {} vs {}",
        node_lost(&o, "unknown-signer"),
        node_lost(&b, "unknown-signer")
    );
    assert!(o.get("full_cert_share") > b.get("full_cert_share"));
    assert!(o.report.air_bytes.security > b.report.air_bytes.security);
    assert!(node_lost(&o, "unknown-signer") < node_lost(&b, "unknown-signer"));

    let mut always = base;
    always.security.signer_id_policy.full_cert_every_ms = 0;
    always.security.signer_id_policy.digest_otherwise = false;
    always.security.signer_id_policy.on_new_neighbour = true;
    assert!(refused(&always, "on_new_neighbour"));
}

/// The on-demand relevance threshold: 0.5 (a 1 s time to collision) against 0.05 (19 s).
/// The lower bar marks more known signers relevant, so more messages are verified and
/// fewer reach the applications unverified. Before the claim-based relevance existed no
/// message carried a score and the threshold moved nothing. Refused under verify-all.
#[test]
fn the_on_demand_threshold_moves_the_verify_load() {
    let mut high = fleet(30, 12.0);
    high.nodes.default_obu = "obu/unex-obu-301-craton2-software".to_string();
    high.security.verification_policy = "on-demand".to_string();
    let mut low = high.clone();
    low.security.on_demand_threshold = Some(0.05);
    let h = run(high.clone());
    let l = run(low);
    row("on-demand 0.5", &h);
    row("on-demand 0.05", &l);
    assert!(
        served(&l).len() > served(&h).len(),
        "no more messages verified"
    );
    assert!(l.get("unverified_ratio") < h.get("unverified_ratio"));

    let mut wrong = high;
    wrong.security.verification_policy = "verify-all".to_string();
    wrong.security.on_demand_threshold = Some(0.05);
    assert!(refused(&wrong, "on_demand_threshold"));
}

/// The peer certificate cache: the default 128 against 4 on a 20-vehicle fleet sending a
/// certificate once a second and a digest otherwise. A unit that holds 4 of its 19
/// neighbours' certificates forgets the rest, so their digest-signed messages miss and
/// need P2PCD requests, and are unverifiable until the next certificate. Before the fix
/// an evicted certificate stayed in the store and still verified.
#[test]
fn the_peer_cert_cache_moves_p2pcd_and_unknown_signers() {
    let big = backend(20, 10.0);
    let mut small = big.clone();
    small.nodes.peer_cert_cache = Some(4);
    let b = run(big);
    let s = run(small);
    for (name, m) in [("cache 128", &b), ("cache 4", &s)] {
        println!(
            "ROW {name}: p2pcd {} unknown {}",
            events(m, "digest_misses"),
            node_lost(m, "unknown-signer")
        );
    }
    assert!(s.get("digest_misses") > b.get("digest_misses"));
    assert!(node_lost(&s, "unknown-signer") > node_lost(&b, "unknown-signer"));
}

/// Each verified message's queue wait (start - enqueue, ms) and the priority it was
/// served by, from the `node.verify` records.
fn served(m: &Measured) -> Vec<(u64, f64)> {
    let ns = |v: &serde_json::Value| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64));
    m.rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "node.verify")
        .filter_map(|(_, r)| serde_json::from_slice::<serde_json::Value>(&r.json).ok())
        .filter(|v| v["outcome"] == "valid")
        .filter_map(|v| {
            let start = ns(&v["t_start"])?;
            let enq = ns(&v["t_enqueue"])?;
            Some((
                v["priority"].as_u64()?,
                start.saturating_sub(enq) as f64 / 1e6,
            ))
        })
        .collect()
}

/// The `prioritized` policy under a software verifier that cannot keep up with 60
/// vehicles: the near (priority at least 500, inside half the horizon) are served ahead
/// of the far, so their queue wait is shorter. It used to serve first in, first out and
/// the two waits were the same. Narrowing the horizon from 300 m to 10 m leaves almost no
/// sender near, so the share served ahead collapses. Refused under verify-all.
#[test]
fn the_prioritized_horizon_moves_the_service_order() {
    let mut wide = fleet(60, 3.0);
    wide.nodes.default_obu = "obu/unex-obu-301-craton2-software".to_string();
    wide.security.verification_policy = "prioritized".to_string();
    let mut narrow = wide.clone();
    narrow.security.prioritized_horizon_m = Some(10.0);
    let w = run(wide.clone());
    let n = run(narrow);
    let split = |m: &Measured| {
        let s = served(m);
        let mean = |hi: bool| {
            let v: Vec<f64> = s
                .iter()
                .filter(|(p, _)| (*p >= 500) == hi)
                .map(|x| x.1)
                .collect();
            (v.len(), v.iter().sum::<f64>() / v.len().max(1) as f64)
        };
        (mean(true), mean(false))
    };
    let (wh, wl) = split(&w);
    let (nh, nl) = split(&n);
    println!(
        "ROW horizon 300: near {} wait {:.3} ms, far {} wait {:.3} ms",
        wh.0, wh.1, wl.0, wl.1
    );
    println!(
        "ROW horizon 10: near {} wait {:.3} ms, far {} wait {:.3} ms",
        nh.0, nh.1, nl.0, nl.1
    );
    assert!(wh.0 > 0 && wl.0 > 0);
    assert!(wh.1 < 0.75 * wl.1, "the near are not served first");
    let share = |h: (usize, f64), l: (usize, f64)| h.0 as f64 / (h.0 + l.0).max(1) as f64;
    assert!(share(nh, nl) < share(wh, wl));

    let mut wrong = wide;
    wrong.security.verification_policy = "verify-all".to_string();
    wrong.security.prioritized_horizon_m = Some(100.0);
    assert!(refused(&wrong, "prioritized_horizon_m"));
}

/// A compromised-RSU attack with no compromised unit does nothing, so the loader refuses it
/// on any scenario, not only on one that runs the credential lifecycle (the security-track
/// catalogue sweep found it accepted and dead on the DSRC family).
#[test]
fn a_compromised_rsu_attack_without_units_is_refused() {
    let mut s = Scenario::load(root().join("scenarios/phase1-grid.yaml")).expect("loads");
    s.threats.compromised_rsu_attack = Some(ModelChoice::new("threat/attacker/compromised-rsu"));
    assert!(refused(&s, "compromised_rsu_attack"));
    s.threats.compromised_rsu_attack = None;
    assert!(!refused(&s, "compromised_rsu_attack"));
}
