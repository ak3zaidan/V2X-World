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
    Measured { report, m, last, rec }
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
    println!("pdr {:.3} attempts {}", pdr(&b.report), b.report.reception_attempts);
    let _ = &b.rec;
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
    assert!(q.report.fragments_sent > e.report.fragments_sent, "nothing fragmented");
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
    assert!(all.get("full_cert_share") > 0.9, "every message carries one");
    assert!(all.get("full_cert_share") > hz.get("full_cert_share"));
    assert!(hz.get("full_cert_share") > slow.get("full_cert_share"));
    assert!(all.report.air_bytes.security > hz.report.air_bytes.security);
    assert!(hz.report.air_bytes.security > slow.report.air_bytes.security);
    assert!(all.get("security_overhead") > slow.get("security_overhead"));
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
    assert!(o.get("verify_wait") <= a.get("verify_wait"));
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
        assert!(w[1].get("cbr") > w[0].get("cbr"));
        assert!(w[1].get("aoi") < w[0].get("aoi"));
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
    for (n, m) in [("5g-first-hop", &fast), ("4g-west-coast", &slow), ("half", &half)] {
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
    assert!(ts > tf, "4g-west-coast {ts} ms against 5g-first-hop {tf} ms");
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
    assert!(often.report.phase2.crl_versions_published > 0, "nothing was revoked");
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
            m.last.get("det_tp{\"level\":\"report\",\"cell\":\"tp\"}").copied().unwrap_or(0.0),
            m.last.get("det_fp{\"level\":\"report\",\"cell\":\"fp\"}").copied().unwrap_or(0.0),
            m.last.get("det_fn{\"level\":\"report\",\"cell\":\"fn\"}").copied().unwrap_or(0.0),
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
    assert!(tp(few) > 0.0, "no report about an attacker reached the authority");
    assert!(tp(many) > tp(few));
}
