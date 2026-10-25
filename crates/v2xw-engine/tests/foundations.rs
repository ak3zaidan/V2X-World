//! Foundations, checked from the ground up (docs/validation/foundations.md).
//!
//! Two vehicles first, on a world small enough to reason about, with the radio made
//! deterministic (free space, no fading, no buildings) so that every number the run reports
//! can be computed by hand from the 802.11p, J2735 and IEEE 1609.2 rules and the hardware
//! profile — and is asserted to be exactly that. Then a roadside unit, then the fleet grown
//! linearly, with the load metrics held to what theory says they scale as.
//!
//! The fleet is placed with a `v2xw/trips/1` file rather than a Poisson cap, because the
//! cap counts *trips*, and a trip whose destination the router cannot reach is dropped and
//! still counted: on the 4 × 8 grid a cap of 2 put zero vehicles on the road. A trip file
//! names its lanes, so "two vehicles" is two vehicles.
//!
//! Every assertion is on counts and simulated time, never wall clock.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::channels::{
    ChannelView, GtKinematicsView, MacCbrView, NodeRxView, NodeTxView, NodeVerifyView, PhyRxView,
    RxFate, RxOutcome, SignerId, decode,
};
use v2xw_metrics::{MetricSample, SampleValue};
use v2xw_world::model::LaneKind;

const S: u64 = 1_000_000_000;

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios")
}

fn with_model(mut s: Scenario, family: &str, id: &str) -> Scenario {
    s.radio.models.insert(
        family.to_string(),
        v2xw_engine::scenario::schema::ModelChoice::new(id),
    );
    s
}

/// Open air, free space, no fading: every received power is Friis, exactly.
fn deterministic_radio(mut s: Scenario) -> Scenario {
    s.world.buildings.enabled = false;
    with_model(
        with_model(s, "fading", "fading/none"),
        "propagation",
        "propagation/free-space",
    )
}

/// `phase1-grid.yaml` cut to 4 × 8 junctions (822 × 560 m) with exactly `n` vehicles, all
/// departing at t = 0 on the `n` distinct driving lanes nearest the grid's centre, each
/// bound for the farthest lane its origin can reach.
fn fleet(n: usize, seconds: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = seconds;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = 4.into();
        params["rows"] = 8.into();
    }
    s.metrics = vec!["all".to_string()];
    let world = v2xw_engine::wiring::build_world(&s).expect("world");
    let roads = &world.roads;
    let driving: Vec<_> = roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving && l.junction.is_none() && l.length_m > 40.0)
        .collect();
    let mid = |l: &v2xw_world::model::Lane| l.centreline[l.centreline.len() / 2];
    let dist = |a: &v2xw_world::model::Lane, b: &v2xw_world::model::Lane| {
        (mid(a).x - mid(b).x).hypot(mid(a).y - mid(b).y)
    };
    let (sx, sy) = driving
        .iter()
        .fold((0.0, 0.0), |a, l| (a.0 + mid(l).x, a.1 + mid(l).y));
    let (cx, cy) = (sx / driving.len() as f64, sy / driving.len() as f64);
    let mut by_centre = driving.clone();
    by_centre.sort_by(|a, b| {
        let da = (mid(a).x - cx).hypot(mid(a).y - cy);
        let db = (mid(b).x - cx).hypot(mid(b).y - cy);
        da.total_cmp(&db).then(a.id.cmp(&b.id))
    });
    assert!(
        by_centre.len() >= n,
        "the grid has {} lanes",
        by_centre.len()
    );
    let trips: Vec<_> = by_centre
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, o)| {
            let mut seen = BTreeSet::from([o.id]);
            let mut queue = VecDeque::from([o.id]);
            while let Some(l) = queue.pop_front() {
                for c in roads.successors(l) {
                    for next in [c.via, Some(c.to_lane)].into_iter().flatten() {
                        if seen.insert(next) {
                            queue.push_back(next);
                        }
                    }
                }
            }
            // A dead-end lane (no successor) is reachable but the router refuses it as a
            // destination; see the report's open items.
            let dest = driving
                .iter()
                .filter(|l| {
                    l.id != o.id && seen.contains(&l.id) && !roads.successors(l.id).is_empty()
                })
                .max_by(|a, b| dist(a, o).total_cmp(&dist(b, o)).then(b.id.cmp(&a.id)))
                .expect("a reachable destination");
            serde_json::json!({"id": i, "depart_s": 0.0, "origin": o.id, "destination": dest.id})
        })
        .collect();
    let set = serde_json::json!({
        "schema": "v2xw/trips/1",
        "world_hash": v2xw_world::hash::content_hash_hex(&world),
        "source": {"test": "foundations"},
        "trips": trips,
    });
    // Named by content and published by rename: `CARGO_TARGET_TMPDIR` is shared by the
    // tests of this binary (several want the same `n` at once, and `fs::write` truncates
    // before it writes, so a reader can see an empty file) and by every worktree that
    // builds into the same target directory.
    let body = set.to_string();
    let name = format!(
        "foundations-{n}-{}.json",
        &v2xw_core::hash::sha256_hex(body.as_bytes())[..16]
    );
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if std::fs::read(&path).ok().as_deref() != Some(body.as_bytes()) {
        static WRITER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let k = WRITER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = path.with_extension(format!("{}-{k}.tmp", std::process::id()));
        std::fs::write(&tmp, &body).expect("writes the trip file");
        std::fs::rename(&tmp, &path).expect("publishes the trip file");
    }
    s.actors.vehicles.demand.kind = "mobility/demand/trips".into();
    s.actors.vehicles.demand.rate_veh_per_h = None;
    s.actors.vehicles.demand.params = serde_json::json!({ "path": path });
    s
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

fn on<T: ChannelView + serde::de::DeserializeOwned>(rec: &MemoryRecorder) -> Vec<T> {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == T::CHANNEL)
        .map(|(_, r)| decode(r).expect("decodes as its channel's view"))
        .collect()
}

/// Every metric sample in the window that closes at `t`, by `name{dims}`.
fn window(rec: &MemoryRecorder, t: u64) -> BTreeMap<String, SampleValue> {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == "metric.sample")
        .map(|(_, r)| serde_json::from_slice::<MetricSample>(&r.json).expect("a sample"))
        .filter(|m| m.t == t)
        .map(|m| {
            let dims = serde_json::to_string(&m.dims).unwrap_or_default();
            let key = if dims == "{}" || dims == "[]" || dims == "null" {
                m.metric
            } else {
                format!("{}{dims}", m.metric)
            };
            (key, m.value)
        })
        .collect()
}

fn point(w: &BTreeMap<String, SampleValue>, key: &str) -> f64 {
    let v = w
        .get(key)
        .unwrap_or_else(|| panic!("no {key} in the window: {:?}", w.keys().collect::<Vec<_>>()));
    let j = serde_json::to_value(v).expect("serialises");
    j.get("point")
        .or_else(|| j.get("mean"))
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| panic!("{key} has no value: {j}"))
}

/// The 802.11p OFDM PSDU airtime at 10 MHz: a 32 µs preamble, the 8 µs SIGNAL symbol, then
/// ceil((16 service + 8·octets + 6 tail) / N_DBPS) symbols of 8 µs (IEEE 802.11-2016
/// §17.3.2.3, §17.4.3, Table 17-5 at half clock). QPSK 1/2 at 10 MHz is 6 Mb/s, N_DBPS 48.
fn airtime_us(octets: u64, n_dbps: u64) -> u64 {
    40 + (16 + 8 * octets + 6).div_ceil(n_dbps) * 8
}

/// Free-space path loss, dB (Friis): 20·log10(4π d f / c).
fn fspl_db(d_m: f64, f_hz: f64) -> f64 {
    20.0 * (4.0 * std::f64::consts::PI * d_m * f_hz / 299_792_458.0).log10()
}

/// The two-vehicle run every hand computation below is made against.
fn two() -> (RunReport, MemoryRecorder) {
    run(deterministic_radio(fleet(2, 3.0)))
}

#[test]
fn two_vehicles_send_ten_bsms_a_second_each_with_the_j2945_signer_rule() {
    let (report, rec) = two();
    assert_eq!(report.actors_spawned, 2);
    assert_eq!(report.nodes_created, 2);
    let txs: Vec<NodeTxView> = on(&rec);
    assert!(txs.iter().all(|t| t.msg_type.as_deref() == Some("bsm")));
    // 10 Hz for 3 s, from each of the two.
    assert_eq!(txs.len(), 60);
    assert_eq!(report.frames_transmitted, 60);
    for node in 0..2 {
        let mine: Vec<&NodeTxView> = txs.iter().filter(|t| t.node.index() == node).collect();
        assert_eq!(mine.len(), 30, "node {node}");
        // Generated exactly every 100 ms (J2945/1 §6.3.8 ITT at low channel load).
        for w in mine.windows(2) {
            let gap = w[1].t_generated.unwrap() - w[0].t_generated.unwrap();
            assert_eq!(gap, 100_000_000, "node {node}: generation interval");
        }
        // A full certificate once a second, a digest otherwise.
        let certs = mine
            .iter()
            .filter(|t| t.signer == Some(SignerId::Certificate))
            .count();
        assert_eq!(certs, 3, "node {node}: full certificates in 3 s");
        let counts: Vec<u8> = mine
            .iter()
            .map(|t| t.content.as_ref().unwrap().msg_count.unwrap())
            .collect();
        for w in counts.windows(2) {
            assert_eq!(w[1], (w[0] + 1) % 128, "msgCnt advances by one");
        }
    }
    for t in &txs {
        // The layers add up: SPDU = payload + envelope; PSDU = SPDU + WSMP + LLC/MAC/FCS.
        let spdu = t.payload_bytes.unwrap() + t.envelope_bytes.unwrap();
        assert_eq!(t.spdu_bytes, Some(spdu));
        assert_eq!(
            t.bytes_on_wire,
            spdu + t.net_header_bytes.unwrap() + t.link_bytes.unwrap()
        );
        // A certificate is what makes a full frame longer, and nothing else.
        assert_eq!(
            t.cert_bytes.unwrap() > 0,
            t.signer == Some(SignerId::Certificate)
        );
        // Airtime is the 802.11p formula at the frame's MCS (QPSK 1/2, 6 Mb/s).
        assert_eq!(t.radio.as_ref().unwrap().mcs, "6mbps-qpsk-1/2");
        assert_eq!(
            t.airtime_us,
            Some(airtime_us(t.bytes_on_wire, 48)),
            "{} octets",
            t.bytes_on_wire
        );
    }
}

#[test]
fn every_bsm_says_where_the_vehicle_truly_was_within_gnss_error() {
    let (_, rec) = two();
    let gts: Vec<GtKinematicsView> = on(&rec);
    let txs: Vec<NodeTxView> = on(&rec);
    let mut errors = Vec::new();
    for t in &txs {
        let generated = t.t_generated.unwrap();
        // The latest ground truth at or before generation: the mobility step the BSM was
        // built from.
        let Some(truth) = gts
            .iter()
            .filter(|g| g.node == Some(t.node) && g.t <= generated)
            .max_by_key(|g| g.t)
        else {
            continue;
        };
        let c = t.content.as_ref().unwrap();
        let e = (c.claimed_x_m.unwrap() - truth.x_m).hypot(c.claimed_y_m.unwrap() - truth.y_m);
        // Under 15 m always: Reid 2019's 99th percentile for an automotive single-frequency
        // receiver is 9.38 m (gauss_markov.rs REID_HORIZONTAL_M), plus at most one step of
        // travel at the 11.2 m/s limit.
        // Never off by more than 30 m: the model's multipath bursts reach 20-25 m here
        // (measured: five BSMs of one vehicle in one burst at 2.1-2.9 s), which is Reid's
        // tail, not a frame of reference error (that is hundreds of metres).
        assert!(
            e < 30.0,
            "{:?} at {generated}: {e:.2} m from the truth",
            t.node
        );
        errors.push(e);
        // The encoded speed is the claim on J2735's 0.02 m/s grid, and the claim is the
        // true speed within GNSS velocity noise.
        assert!((c.speed_mps.unwrap() - c.claimed_speed_mps.unwrap()).abs() <= 0.021);
        assert!((c.claimed_speed_mps.unwrap() - truth.speed_mps).abs() < 2.0);
        // J2735 heading is clockwise from north; ground truth is ENU, counter-clockwise
        // from east.
        if truth.speed_mps > 1.0 {
            let want = (90.0 - truth.heading_rad.unwrap().to_degrees()).rem_euclid(360.0);
            let got = c.heading_deg.unwrap();
            let d = (got - want)
                .rem_euclid(360.0)
                .min((want - got).rem_euclid(360.0));
            assert!(d < 10.0, "heading {got} vs truth {want}");
        }
    }
    assert!(
        errors.len() >= 50,
        "only {} BSMs had a ground truth",
        errors.len()
    );
    let mean = errors.iter().sum::<f64>() / errors.len() as f64;
    // Reid 2019's 95th percentile is 5.30 m; a mean above it is not that receiver. And
    // most fixes sit inside the 99th percentile, 9.38 m.
    assert!(mean < 5.3, "mean horizontal error {mean:.2} m");
    let inside = errors.iter().filter(|&&e| e < 9.38).count();
    assert!(
        inside * 10 >= errors.len() * 8,
        "{inside} of {} inside 9.38 m",
        errors.len()
    );
}

#[test]
fn every_frame_reaches_the_other_vehicle_at_the_friis_power_and_is_verified() {
    let (report, rec) = two();
    let txs: Vec<NodeTxView> = on(&rec);
    let phy: Vec<PhyRxView> = on(&rec);
    let rxs: Vec<NodeRxView> = on(&rec);
    let ver: Vec<NodeVerifyView> = on(&rec);
    // One link each way, so one attempt per frame, every one decoded, delivered, verified.
    assert_eq!(report.reception_attempts, 60);
    assert_eq!(report.receptions_ok, 60);
    assert_eq!(phy.len(), 60);
    assert_eq!(rxs.len(), 60);
    assert!(
        rxs.iter().all(
            |r| r.outcome == RxFate::Delivered && r.verification.as_deref() == Some("verified")
        )
    );
    assert_eq!(ver.len(), 60);
    let by_msg: BTreeMap<u64, &NodeTxView> = txs.iter().map(|t| (t.msg.unwrap(), t)).collect();
    for p in &phy {
        assert_eq!(p.outcome, RxOutcome::Ok);
        let tx = by_msg[&p.msg.unwrap()];
        assert_ne!(tx.node, p.rx, "a node does not receive itself");
        let d = p.dist_m.unwrap();
        // Channel 172 is 5.860 GHz (IEEE 802.11-2016 Annex E: 5000 + 5 × 172 MHz).
        assert_eq!(tx.channel, Some(172));
        // The OBU-301's 5 dBi antenna [R7 §A10] less the 2 dB cable the device model
        // charges, at both ends: J2945/1's radiated power (dcc_state `rp`) is the reported
        // conducted power plus 3 dB.
        let net_gain = 2.0 * 3.0;
        let want = tx.power_dbm.unwrap() + net_gain - fspl_db(d, 5.860e9);
        let got = p.rssi_dbm.unwrap();
        assert!(
            (got - want).abs() < 0.02,
            "at {d:.1} m: {got} dBm, Friis says {want:.3}"
        );
    }
    // Verification is the profile's line-rate engine, 1/2500 s = 400 µs, never queued with
    // one sender; signing is the eHSM's published "<9 ms" (R7 §A4). The first SPDU a
    // receiver gets under a pseudonym also validates it (WP-C2): the EE link's SHA-256
    // over the 77-octet certificate and its ECQV reconstruction, priced on the profile.
    // The CA path was validated when the chain file was installed, and every later SPDU
    // under the same certificate costs the signature only (crate rxval, the EE cache).
    let first = first_sight_ns(77);
    let mut seen = BTreeSet::new();
    let mut by_node: Vec<&NodeVerifyView> = ver.iter().collect();
    by_node.sort_by_key(|v| (v.node, v.t_enqueue));
    for v in by_node {
        let want = if seen.insert(v.node) {
            400_000 + first
        } else {
            400_000
        };
        assert_eq!(v.t_done.unwrap() - v.t_start.unwrap(), want, "{v:?}");
        assert_eq!(v.cost_us, Some(want / 1_000));
    }
    assert_eq!(seen.len(), 2);
    assert_eq!(report.receiver_validation.ee_validations, 2);
    assert_eq!(report.receiver_validation.ee_hits, 58);
    let mut first_verify: BTreeMap<_, u64> = BTreeMap::new();
    for r in &rxs {
        let t = r.t_verify_start.unwrap();
        first_verify
            .entry(r.rx)
            .and_modify(|m: &mut u64| *m = (*m).min(t))
            .or_insert(t);
    }
    for t in &txs {
        assert_eq!(t.t_signed.unwrap() - t.t_sign_start.unwrap(), 9_000_000);
    }
    // Every delivered message's stages are the hand-computed ones.
    for r in &rxs {
        let tx = by_msg[&r.msg.unwrap()];
        let d = r.dist_m.unwrap();
        let prop = r.t_arrival.unwrap() - r.t_tx_end.unwrap();
        assert!(
            (prop as f64 - d / 0.299_792_458).abs() <= 1.0,
            "propagation {prop} ns at {d} m"
        );
        assert_eq!(
            r.t_tx_end.unwrap() - r.t_tx_start.unwrap(),
            tx.airtime_us.unwrap() * 1000
        );
        assert_eq!(r.t_tx_start.unwrap(), tx.t);
        let first_seen = first_verify[&r.rx] == r.t_verify_start.unwrap();
        assert_eq!(
            r.t_verify_done.unwrap() - r.t_verify_start.unwrap(),
            if first_seen { 400_000 + first } else { 400_000 }
        );
        assert_eq!(r.t_delivered, r.t_verify_done);
    }
}

/// A peer certificate cache too small for the neighbourhood evicts certificates, and an
/// evicted certificate takes its validated state with it: when it comes back attached to
/// a later BSM it is validated, and charged, as on first sight.
#[test]
fn a_certificate_the_peer_cache_evicts_is_validated_again_when_it_returns() {
    let (base, _) = run(deterministic_radio(fleet(3, 3.0)));
    let b = &base.receiver_validation;
    // Three vehicles that all hear each other: each validates the other two once.
    assert_eq!((b.ee_validations, b.ee_evictions), (6, 0), "{b:?}");
    let mut s = deterministic_radio(fleet(3, 3.0));
    // One slot for two neighbours: each attached certificate evicts the other's.
    s.nodes.peer_cert_cache = Some(1);
    let (small, _) = run(s);
    let r = &small.receiver_validation;
    assert!(r.ee_evictions > 0, "{r:?}");
    assert!(r.ee_validations > b.ee_validations, "{r:?}");
}

/// `nodes.validated_cert_cache` (SCMS settings audit) reaches every vehicle: with the
/// peer certificate cache at its default, a validated cache of one makes each vehicle
/// forget one neighbour's verdict for the other's, so a returning certificate is
/// validated, and charged, again (`v2xw_node::rxval::EE_CACHE` is 4,096 unless set).
#[test]
fn a_validated_certificate_cache_of_one_revalidates_returning_signers() {
    let (base, _) = run(deterministic_radio(fleet(3, 3.0)));
    let b = &base.receiver_validation;
    assert_eq!((b.ee_validations, b.ee_evictions), (6, 0), "{b:?}");
    let mut s = deterministic_radio(fleet(3, 3.0));
    s.nodes.validated_cert_cache = Some(1);
    let (small, _) = run(s);
    let r = &small.receiver_validation;
    assert!(r.ee_evictions > 0, "{r:?}");
    assert!(r.ee_validations > b.ee_validations, "{r:?}");
}

/// What validating a new implicit pseudonym of `cert_octets` costs the OBU-301 on top of
/// the signature: one SHA-256 over the certificate (its HashedId8) and one ECQV
/// reconstruction, priced by the WP-C4 catalogue on the profile's anchors.
fn first_sight_ns(cert_octets: u32) -> u64 {
    use v2xw_sec::primitive::{PrimitiveId, PrimitiveOpKind::Verify};
    v2xw_node::rxval::price(
        &[
            (PrimitiveId::SHA_256, Verify, cert_octets),
            (PrimitiveId::ECQV_P256, Verify, 0),
        ],
        "obu/unex-obu-301-craton2",
    )
    .expect("the OBU-301 prices both")
    .as_nanos()
}

#[test]
fn every_metric_of_the_two_vehicle_run_is_its_hand_computed_value() {
    let (_, rec) = two();
    // The window that closes at 2 s, one second of steady state; and the last window, at
    // the horizon, exists (it was dropped before 2026-10-07).
    let w = window(&rec, 2 * S);
    assert!(
        !window(&rec, 3 * S).is_empty(),
        "the window that closes at the end of the run is missing"
    );
    // Per vehicle and second: one 263-octet frame with a certificate, nine 192-octet ones
    // with a digest (56-octet BSM, 1609.2 envelope 164 / 93, WSMP 5, LLC + MAC + FCS 38).
    // The certificate is the ACA's 77-octet implicit pseudonym (WP-C1; it was a 144-octet
    // explicit stand-in before), so the envelope that carries it is 67 octets shorter.
    let per_vehicle_bytes = 263.0 + 9.0 * 192.0;
    assert_eq!(
        point(&w, "bytes_air{\"bucket\":\"air\"}"),
        2.0 * per_vehicle_bytes
    );
    assert_eq!(point(&w, "offered_load"), 2.0 * per_vehicle_bytes * 8.0);
    assert_eq!(point(&w, "carried_load"), 2.0 * per_vehicle_bytes * 8.0);
    // Airtime: 400 µs + 9 × 304 µs = 3,136 µs per vehicle per second.
    let per_vehicle_air = (airtime_us(263, 48) + 9 * airtime_us(192, 48)) as f64 * 1e-6;
    assert!((point(&w, "channel_load") - 2.0 * per_vehicle_air).abs() < 1e-4);
    assert!((point(&w, "channel_occupancy") - per_vehicle_air).abs() < 1e-4);
    // Security overhead: envelope octets over payload, and over the air.
    let env = 2.0 * (164.0 + 9.0 * 93.0);
    let payload = 2.0 * 10.0 * 56.0;
    assert!((point(&w, "envelope_overhead") - env / payload).abs() < 1e-3);
    assert!((point(&w, "security_overhead") - env / (2.0 * per_vehicle_bytes)).abs() < 1e-3);
    assert!((point(&w, "cert_bytes_share") - 2.0 * 77.0 / env).abs() < 1e-3);
    // Every frame delivered, every neighbour within 300 m known.
    let pdr = serde_json::to_value(&w["pdr"]).unwrap();
    assert_eq!(pdr["trials"], 20, "{pdr}");
    assert_eq!(pdr["successes"], 20, "{pdr}");
    assert_eq!(point(&w, "nar{\"radius\":\"300m\"}"), 1.0);
    assert_eq!(
        point(&w, "verify_rate{\"primitive\":\"ecdsa-p256-verify\"}"),
        20.0
    );
    // Age of information with a period T and a constant latency L is L + T/2 (Kaul, Yates
    // & Gruteser 2012, periodic updates): here L is 9 ms sign + hand-off + airtime + 0.4 ms
    // verify.
    let e2e = serde_json::to_value(&w["e2e_latency"]).unwrap();
    let latency_ms = e2e["sum"].as_f64().unwrap() / 20.0;
    assert!((9.4..20.0).contains(&latency_ms), "latency {latency_ms} ms");
    // The two start on the lanes nearest the grid's centre, which can be further apart
    // than the headline's 100 m (SAE J2945/1 vPERRange), so the neighbourhood age is read
    // at 300 m, the radius `nar` is checked at above.
    assert!((point(&w, "aoi{\"radius\":\"300m\"}") - (latency_ms + 50.0)).abs() < 1.0);
    for zero in [
        "collision_rate",
        "per",
        "half_duplex_rate",
        "loss_rate",
        "unverified_ratio",
    ] {
        let v = serde_json::to_value(&w[zero]).unwrap();
        assert_eq!(v["successes"], 0, "{zero}: {v}");
    }
}

#[test]
fn a_roadside_unit_is_heard_by_both_vehicles_and_hears_them() {
    let (_, base) = two();
    let g: Vec<GtKinematicsView> = on(&base);
    let mut s = deterministic_radio(fleet(2, 2.0));
    // On the signalised junction nearest the two vehicles: a SPaT unit is wired to the
    // controller of the junction it stands at.
    let centre = {
        let (mx, my) = ((g[0].x_m + g[1].x_m) / 2.0, (g[0].y_m + g[1].y_m) / 2.0);
        let world = v2xw_engine::wiring::build_world(&s).expect("world");
        let j = world
            .roads
            .junctions()
            .iter()
            .min_by(|a, b| {
                let da = (a.position.x - mx).hypot(a.position.y - my);
                let db = (b.position.x - mx).hypot(b.position.y - my);
                da.total_cmp(&db)
            })
            .expect("a junction");
        [j.position.x, j.position.y]
    };
    s.messages.sets = vec!["bsm".into(), "spat".into(), "map".into()];
    s.actors.rsus = vec![
        serde_json::from_value(serde_json::json!({
            "position_m": [centre[0], centre[1], 0.0],
            "roles": ["spat", "map"],
            "profile": "rsu/commsignia-its-rs4",
        }))
        .expect("an RSU"),
    ];
    let (report, rec) = run(s);
    let txs: Vec<NodeTxView> = on(&rec);
    let rxs: Vec<NodeRxView> = on(&rec);
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    for t in &txs {
        *by_type
            .entry(t.msg_type.clone().unwrap_or_default())
            .or_default() += 1;
    }
    println!(
        "rsu run: {by_type:?} attempts {} ok {}",
        report.reception_attempts, report.receptions_ok
    );
    assert_eq!(report.reception_attempts, 2 * (40 + 22));
    assert_eq!(report.receptions_ok, report.reception_attempts);
    let units: BTreeSet<_> = txs
        .iter()
        .filter(|t| t.msg_type.as_deref() != Some("bsm"))
        .map(|t| t.node)
        .collect();
    assert_eq!(units.len(), 1, "one unit sends: {units:?}");
    assert_eq!(by_type.get("bsm"), Some(&40));
    let cars: BTreeSet<_> = txs
        .iter()
        .filter(|t| t.msg_type.as_deref() == Some("bsm"))
        .map(|t| t.node)
        .collect();
    assert_eq!(cars.len(), 2);
    // SPaT at 10 Hz and MAP at 1 Hz (CTI 4501; infra.rs SPAT/MAP intervals).
    assert_eq!(by_type.get("spat"), Some(&20));
    assert_eq!(by_type.get("map"), Some(&2));
    for &car in &cars {
        let heard = rxs
            .iter()
            .filter(|r| r.rx == car && r.tx.is_some_and(|t| units.contains(&t)))
            .filter(|r| r.outcome == RxFate::Delivered)
            .count();
        assert_eq!(heard, 22, "{car:?} heard {heard} of the unit's 22 messages");
    }
    let unit = *units.iter().next().unwrap();
    let at_unit = rxs
        .iter()
        .filter(|r| r.rx == unit && r.msg_type.as_deref() == Some("bsm"))
        .count();
    assert_eq!(at_unit, 40, "the unit's receptions of 40 BSMs");
}

type Row = (usize, u64, u64, f64, f64, u64, u64, u64);

#[test]
fn the_load_metrics_scale_with_the_fleet_as_theory_says() {
    let mut rows: Vec<Row> = Vec::new();
    for n in [2usize, 5, 10, 25, 50, 100] {
        let (report, rec) = run(deterministic_radio(fleet(n, 1.0)));
        let w = window(&rec, S);
        let collisions = report.rx_losses.get("collision").copied().unwrap_or(0)
            + report
                .rx_losses
                .get("hidden-terminal")
                .copied()
                .unwrap_or(0);
        let row = (
            n,
            report.actors_spawned,
            report.frames_transmitted,
            point(&w, "offered_load"),
            point(&w, "channel_load"),
            report.reception_attempts,
            report.receptions_ok,
            collisions,
        );
        let cbr: Vec<MacCbrView> = on(&rec);
        let txs: Vec<NodeTxView> = on(&rec);
        let mut spans: Vec<(u64, u64)> = txs
            .iter()
            .map(|t| (t.t, t.t + t.airtime_us.unwrap() * 1000))
            .collect();
        spans.sort_unstable();
        let overlaps = spans.windows(2).filter(|w| w[1].0 < w[0].1).count();
        let same_start = spans.windows(2).filter(|w| w[1].0 == w[0].0).count();
        let phy: Vec<PhyRxView> = on(&rec);
        let min_sinr = phy
            .iter()
            .filter_map(|p| p.sinr_db)
            .fold(f64::INFINITY, f64::min);
        println!(
            "{row:?} losses {:?} mac.cbr {} overlaps {overlaps} same-start {same_start} min sinr {min_sinr:.1}",
            report.rx_losses,
            cbr.len()
        );
        // Hidden terminals, from the records alone: of two frames on the air at once, the
        // later one's sender could not have deferred if the earlier frame reached it below
        // the CCA threshold (−85 dBm, `CcaConfig::ETSI`, EN 302 571 §4.2.10.1). In free
        // space that is a separation beyond about 767 m at this EIRP, while frames still
        // decode out to about 1,083 m (6 Mbit/s static sensitivity −88 dBm): the 3 dB band
        // between the two is where a vehicle hears a frame it does not defer to. The grid's
        // diagonal is 995 m, so the outermost lanes of the 100-vehicle fleet are in it.
        const CCA_DBM: f64 = -85.0;
        let rssi: BTreeMap<(u64, u32), (f64, f64)> = phy
            .iter()
            .filter_map(|p| {
                Some((
                    (p.msg?, p.rx.index()),
                    (p.rssi_dbm?, p.dist_m.unwrap_or(f64::NAN)),
                ))
            })
            .collect();
        let mut frames_on_air: Vec<(u64, u64, u32, Option<u64>)> = txs
            .iter()
            .map(|t| {
                (
                    t.t,
                    t.t + t.airtime_us.unwrap() * 1000,
                    t.node.index(),
                    t.msg,
                )
            })
            .collect();
        frames_on_air.sort_unstable();
        let mut deaf_pairs = Vec::new();
        for (k, a) in frames_on_air.iter().enumerate() {
            for b in frames_on_air[k + 1..].iter().take_while(|b| b.0 < a.1) {
                if b.0 == a.0 {
                    continue; // same instant: neither could have sensed the other
                }
                let heard =
                    a.3.and_then(|m| rssi.get(&(m, b.2)))
                        .is_some_and(|&(p, _)| p >= CCA_DBM);
                if !heard {
                    deaf_pairs.push((a.2, b.2, a.3.and_then(|m| rssi.get(&(m, b.2))).copied()));
                }
            }
        }
        let hidden = report
            .rx_losses
            .get("hidden-terminal")
            .copied()
            .unwrap_or(0);
        println!("  pairs whose later sender heard the earlier frame below CCA: {deaf_pairs:?}");
        assert_eq!(
            hidden > 0,
            !deaf_pairs.is_empty(),
            "hidden-terminal losses {hidden} vs pairs below CCA {deaf_pairs:?} at {n}"
        );
        for &(_, _, link) in &deaf_pairs {
            // Each such pair is far apart and still within decoding range: the 3 dB band.
            let (p, d) = link.expect("every vehicle hears every frame at these ranges");
            assert!((-88.0..CCA_DBM).contains(&p), "{p} dBm at {d} m");
            assert!(
                d > 760.0,
                "a pair below CCA only 760 m or less apart: {d} m"
            );
        }
        // Outside that band a collision needs two frames on the air at once, and an
        // overlapping pair costs at most both frames at every other receiver.
        assert_eq!(
            collisions == 0,
            overlaps == 0,
            "collisions {collisions}, overlaps {overlaps} at {n}"
        );
        assert!(collisions <= 2 * overlaps as u64 * (n as u64 - 1));
        rows.push(row);
    }
    for &(n, spawned, frames, offered, load, attempts, _, _) in &rows {
        assert_eq!(spawned, n as u64, "fleet of {n}");
        // Offered load is linear: n vehicles × 10 Hz.
        // Each vehicle generates ten; one generated in the last ~12 ms (sign + hand-off) is
        // still in the MAC at the horizon.
        assert!(
            (9 * n as u64..=10 * n as u64).contains(&frames),
            "frames at {n}: {frames}"
        );
        let per = offered / n as f64;
        assert!(
            (per - 15_928.0).abs() < 200.0,
            "offered per vehicle at {n}: {per} b/s"
        );
        // Everyone hears everyone in free space at these ranges: n(n-1) links × 10 frames.
        assert_eq!(attempts, frames * (n as u64 - 1), "attempts at {n}");
        // Channel busy ratio: n × 3.1 ms of airtime per second (400 + 9 × 304 µs).
        let want = n as f64 * 0.003_136;
        assert!(
            (load - want).abs() < 0.1 * want + 1e-4,
            "load at {n}: {load} vs {want}"
        );
    }
    assert_eq!(rows[0].7, 0, "two vehicles collided");
    let share = |r: &Row| r.7 as f64 / r.5.max(1) as f64;
    assert!(
        share(&rows[4]) >= share(&rows[1]),
        "collision share fell from 5 to 50"
    );
}

/// The two vehicles with the US SCMS behind them: `credential-lifecycle.yaml`'s backend,
/// with the calendar compressed to 10 s periods so a 30 s run holds top-ups and CRLs.
fn with_backend(mut s: Scenario) -> Scenario {
    let lifecycle = Scenario::load(scenarios().join("credential-lifecycle.yaml")).expect("loads");
    s.actors.backend = lifecycle.actors.backend.clone();
    s.net.uu = lifecycle.net.uu.clone();
    s.security.protocol = lifecycle.security.protocol.clone();
    s.security.verification_policy = "verify-all".into();
    if let Some(p) = s.security.protocol.as_mut() {
        p.params = serde_json::json!({
            "i_period_s": 30, "cert_lifetime_s": 31, "certs_per_period": 5,
            "pool_periods": 2, "topup_below_periods": 1, "cert_shuffle_window_s": 1,
            "first_batch_delay_s": 2, "download_poll_interval_s": 2,
            "enrolment_lifetime_s": 600, "reenrol_lead_s": 120,
            "report_shuffle_window_s": 30, "crl_cadence_s": 10, "crl_fetch_interval_s": 10,
        });
    }
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    s
}

fn backend_run(n: usize, attacker: bool) -> (RunReport, MemoryRecorder) {
    let mut s = with_backend(deterministic_radio(fleet(n, 45.0)));
    s.actors.rsus = vec![
        serde_json::from_value(serde_json::json!({
            "position_m": [420.0, 280.0, 0.0],
            "roles": ["crl", "provisioning-proxy", "report-forward"],
            "profile": "rsu/commsignia-its-rs4",
        }))
        .expect("an RSU"),
    ];
    if attacker {
        let lifecycle =
            Scenario::load(scenarios().join("credential-lifecycle.yaml")).expect("loads");
        s.threats = lifecycle.threats.clone();
        s.detection = lifecycle.detection.clone();
        s.threats.attackers[0].fraction = None;
        s.threats.attackers[0].count = Some(1);
        s.threats.attackers[0].schedule = None;
        // Reports reach the MA within a second rather than a 30 s shuffle, so the
        // revocation lands before the 30 s top-up and the refusal can be seen.
        if let Some(p) = s.security.protocol.as_mut() {
            p.params["report_shuffle_window_s"] = 1.into();
        }
    }
    run(s)
}

#[test]
fn the_backend_tops_up_both_vehicles_over_cellular_and_every_byte_is_booked_once() {
    use v2xw_metrics::channels::ByteBucket as B;
    let (report, rec) = backend_run(2, false);
    let p = &report.phase2;
    let a = &p.access;
    assert_eq!(a.cellular_vehicles, 2);
    // A pool of two 30 s periods at t = 0; the first period ends at 30 s and leaves one,
    // so each vehicle tops up once by 45 s, five certificates a period.
    assert_eq!(p.topups_completed, 2, "{p:?}");
    assert_eq!(p.certs_topped_up, 10);
    assert_eq!(p.topups_refused, 0);
    let certs: Vec<v2xw_metrics::channels::SecCertView> = on(&rec);
    let nb: Vec<v2xw_metrics::channels::NetBytesView> = on(&rec);
    let bucket = |b: B| {
        nb.iter()
            .filter(|r| r.bucket == b)
            .map(|r| r.bytes_on_wire)
            .sum::<u64>()
    };
    // The counters, the records and the metrics are one set of books.
    assert_eq!(bucket(B::CellularDl), a.uu_dl_bytes);
    assert_eq!(bucket(B::CellularUl), a.uu_ul_bytes);
    assert_eq!(bucket(B::Backend), a.backend_bytes);
    assert!(a.uu_dl_bytes > 0 && a.uu_ul_bytes > 0 && a.backend_bytes > 0);
    let mut sums: BTreeMap<String, f64> = BTreeMap::new();
    for t in 1..=45 {
        for (k, v) in window(&rec, t * S) {
            if k.starts_with("bytes_uu") || k.starts_with("bytes_backend") {
                *sums.entry(k).or_default() += serde_json::to_value(&v).unwrap()["point"]
                    .as_f64()
                    .unwrap_or(0.0);
            }
        }
    }
    assert_eq!(
        sums["bytes_uu_dl{\"bucket\":\"cellular-dl\"}"],
        a.uu_dl_bytes as f64
    );
    assert_eq!(
        sums["bytes_uu_ul{\"bucket\":\"cellular-ul\"}"],
        a.uu_ul_bytes as f64
    );
    assert_eq!(
        sums["bytes_backend{\"bucket\":\"backend\"}"],
        a.backend_bytes as f64
    );
    // What a vehicle downloaded for a top-up is what crossed its cellular downlink while
    // the top-up ran, not the backend's own exchanges (7,319 octets were reported before
    // 2026-10-07 against 1,141 on the downlink). The same downlink also carries the
    // vehicle's CRL polls, one every `crl_fetch_interval_s` (10 s) from its first at
    // 2.5 s, so the books are: the top-up's octets inside its exchange (from the 30 s
    // period boundary that triggers it to its completion), and every other downlink
    // record a CRL poll on that cadence, each the same size. Before 2026-10-24 this test
    // summed the whole downlink and failed by exactly the five polls (3,574 = 3,114 +
    // 5 x 92).
    let topups: Vec<_> = certs.iter().filter(|c| c.event == "top-up").collect();
    assert_eq!(topups.len(), 2);
    for c in topups {
        let rows: Vec<(u64, u64)> = nb
            .iter()
            .filter(|r| r.bucket == B::CellularDl && r.node == Some(c.node))
            .map(|r| (r.t, r.bytes_on_wire))
            .collect();
        let inside = |t: u64| (30 * S..=c.t).contains(&t);
        let dl: u64 = rows.iter().filter(|(t, _)| inside(*t)).map(|r| r.1).sum();
        assert_eq!(
            c.bytes,
            Some(dl),
            "{:?}: top-up at {} ns; its downlink (ns, octets) {rows:?}",
            c.node,
            c.t
        );
        let polls: Vec<&(u64, u64)> = rows.iter().filter(|(t, _)| !inside(*t)).collect();
        assert_eq!(polls.len(), 5, "{:?}: CRL polls {polls:?}", c.node);
        for (t, bytes) in &polls {
            assert_eq!(
                (t - S / 2) % (10 * S),
                0,
                "{:?}: a poll off the cadence at {t}",
                c.node
            );
            assert_eq!(*bytes, polls[0].1, "{:?}: CRL polls of different sizes", c.node);
        }
    }
}

#[test]
fn one_attacker_among_six_is_reported_revoked_and_refused_and_the_crl_reaches_everyone() {
    let (report, rec) = backend_run(6, true);
    let p = &report.phase2;
    assert_eq!(p.attackers, 1);
    assert!(p.reports_at_ma > 0, "{p:?}");
    assert_eq!(p.ma_revoke_decisions, 1);
    assert_eq!(p.revoked_attackers, 1);
    assert_eq!(p.revoked_honest, 0);
    assert_eq!(p.crls_issued, 1);
    assert!(p.crl_broadcasts > 0);
    // Every vehicle and the roadside unit install it.
    assert_eq!(p.crls_installed, 6, "{p:?}");
    assert!(
        p.revoked_receptions > 0,
        "nobody refused the revoked sender"
    );
    // Passive revocation: the attacker's next top-up is refused as blocklisted, everyone
    // else's goes through.
    assert_eq!(p.topups_refused_blocklisted, 1);
    assert_eq!(p.topups_completed, 5);
    let order: Vec<u64> = p.revocation_stages.iter().map(|(_, t)| *t).collect();
    assert!(
        order.windows(2).all(|w| w[0] <= w[1]),
        "{:?}",
        p.revocation_stages
    );
    let rxs: Vec<NodeRxView> = on(&rec);
    let refused = rxs
        .iter()
        .filter(|r| r.cause.as_deref() == Some("revoked"))
        .count();
    assert_eq!(refused as u64, p.revoked_receptions);
}

// -----------------------------------------------------------------------------------------
// Every setting changes the run
// -----------------------------------------------------------------------------------------

/// Sets `pointer` in `doc`, creating the objects on the way.
fn set_pointer(doc: &mut serde_json::Value, pointer: &str, value: serde_json::Value) -> bool {
    let mut at = doc;
    let parts: Vec<String> = pointer
        .trim_start_matches('/')
        .split('/')
        .map(|p| p.replace("~1", "/").replace("~0", "~"))
        .collect();
    for (i, part) in parts.iter().enumerate() {
        if at.is_null() {
            *at = serde_json::Value::Object(serde_json::Map::new());
        }
        let Some(obj) = at.as_object_mut() else {
            return false;
        };
        if i == parts.len() - 1 {
            obj.insert(part.clone(), value);
            return true;
        }
        at = obj
            .entry(part.clone())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    }
    false
}

/// A value for `row` other than `current`: another available option, another preset, the
/// other boolean, or a number moved inside its range.
fn another_value(
    row: &serde_json::Value,
    current: &serde_json::Value,
) -> Option<serde_json::Value> {
    use serde_json::Value as V;
    let differs = |v: &V| v != current && !(current.is_null() && Some(v) == row.get("default"));
    if let Some(opts) = row.get("x-options").and_then(V::as_array) {
        return opts
            .iter()
            .filter(|o| o.get("available").and_then(V::as_bool) != Some(false))
            .filter_map(|o| o.get("value").cloned())
            .find(|v| !v.is_null() && differs(v));
    }
    if let Some(e) = row.get("enum").and_then(V::as_array) {
        return e.iter().find(|v| !v.is_null() && differs(v)).cloned();
    }
    if let Some(p) = row.get("x-presets").and_then(V::as_array) {
        if let Some(v) = p
            .iter()
            .filter_map(|o| o.get("value").cloned())
            .find(differs)
        {
            return Some(v);
        }
    }
    match row
        .get("kind")
        .and_then(V::as_str)
        .or(row.get("type").and_then(V::as_str))
    {
        Some("boolean") => {
            Some(V::Bool(!current.as_bool().unwrap_or(
                row.get("default").and_then(V::as_bool).unwrap_or(false),
            )))
        }
        Some("number" | "integer") => {
            let base = current
                .as_f64()
                .or_else(|| row.get("default").and_then(V::as_f64))
                .unwrap_or(1.0);
            let max = row.get("maximum").and_then(V::as_f64);
            let min = row
                .get("minimum")
                .or(row.get("exclusiveMinimum"))
                .and_then(V::as_f64);
            let mut v = if base == 0.0 { 1.0 } else { base * 2.0 };
            if max.is_some_and(|m| v > m) {
                v = (base + min.unwrap_or(0.0)) / 2.0;
            }
            let integer = row.get("type").and_then(V::as_str) == Some("integer")
                || row.get("kind").and_then(V::as_str) == Some("integer");
            Some(if integer {
                V::from(v.round() as i64)
            } else {
                V::from(v)
            })
        }
        _ => None,
    }
}

/// A fingerprint of everything the run recorded.
fn fingerprint(rec: &MemoryRecorder) -> (usize, u64) {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (t, r) in rec.records() {
        t.hash(&mut h);
        r.channel.hash(&mut h);
        r.json.hash(&mut h);
    }
    (rec.records().len(), h.finish())
}

/// Runs `doc` with `pointer` set to `value`: whether the run differs from `reference`, or
/// why it could not run.
fn audit_one(
    doc: &serde_json::Value,
    pointer: &str,
    value: serde_json::Value,
    reference: (usize, u64),
) -> Result<bool, String> {
    let mut edited = doc.clone();
    if !set_pointer(&mut edited, pointer, value) {
        return Err("cannot set".into());
    }
    let s: Scenario = serde_json::from_value(edited).map_err(|e| e.to_string())?;
    let mut engine = Engine::build(s, "").map_err(|e| e.to_string())?;
    let mut r = MemoryRecorder::new();
    engine.run(&mut r).map_err(|e| format!("run failed: {e}"))?;
    Ok(fingerprint(&r) != reference)
}

/// The settings audit: every field the settings window offers and the engine says it acts
/// on (`x-status: wired`), changed one at a time on a five-vehicle run with a roadside unit
/// and the backend, run for 4 s, and compared with the unchanged run record for record.
///
/// Ignored by default: 200 short runs take two minutes in a debug build. Run it with
/// `cargo test -p v2xw-engine --test foundations -- --ignored --nocapture settings_audit`;
/// docs/validation/foundations.md lists what it found.
#[test]
#[ignore = "two minutes; run by hand for the settings audit"]
fn settings_audit() {
    let mut base = with_backend(fleet(5, 4.0));
    base.messages.sets = vec!["bsm".into(), "spat".into(), "map".into()];
    base.actors.rsus = vec![
        serde_json::from_value(serde_json::json!({
            "position_m": [548.0, 240.0, 0.0],
            "roles": ["spat", "map", "crl", "provisioning-proxy", "report-forward"],
            "profile": "rsu/commsignia-its-rs4",
        }))
        .expect("an RSU"),
    ];
    let (_, rec) = run(base.clone());
    let reference = fingerprint(&rec);
    let doc = serde_json::to_value(&base).expect("serialises");
    let surface = v2xw_engine::scenario::scenario_surface();
    let mut changed = Vec::new();
    let mut inert = Vec::new();
    let mut refused = Vec::new();
    let mut skipped = Vec::new();
    for row in surface["fields"].as_array().expect("fields") {
        let path = row["x-path"].as_str().unwrap_or_default().to_string();
        let pointer = row["x-pointer"].as_str().unwrap_or_default();
        if row["x-status"].as_str() != Some("wired")
            || path.contains("[]")
            || path.contains('*')
            || pointer.is_empty()
        {
            continue;
        }
        let current = doc
            .pointer(pointer)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let Some(value) = another_value(row, &current) else {
            skipped.push(format!("{path} (no value to try)"));
            continue;
        };
        // A second value for a number, the other way from the first: a doubled power can
        // sit above a cap the halved one is below.
        let mut values = vec![value.clone()];
        if let (Some(v), Some(c)) = (value.as_f64(), current.as_f64().or(row["default"].as_f64()))
            && c != 0.0
            && v > c
        {
            values.push(if value.is_i64() || value.is_u64() {
                serde_json::Value::from((c / 2.0).round() as i64)
            } else {
                serde_json::Value::from(c / 2.0)
            });
        }
        let mut outcome = Ok(false);
        for v in &values {
            outcome = audit_one(&doc, pointer, v.clone(), reference);
            if !matches!(outcome, Ok(false)) {
                break;
            }
        }
        match outcome {
            Ok(true) => changed.push(path),
            Ok(false) => inert.push(format!("{path}: {current} -> {values:?}")),
            Err(e) => refused.push(format!("{path} = {value}: {e}")),
        }
    }
    println!("CHANGED {} fields", changed.len());
    for p in &inert {
        println!("INERT {p}");
    }
    for p in &refused {
        println!("REFUSED {p}");
    }
    for p in &skipped {
        println!("SKIPPED {p}");
    }
}

/// Three settings the audit found doing nothing, each now doing what it says or saying why
/// it cannot.
#[test]
fn settings_that_did_nothing_now_act_or_refuse() {
    // The radial city generator builds a radial city (it used to build the grid).
    let mut s = fleet(2, 1.0);
    let grid = v2xw_engine::wiring::build_world(&s).expect("grid");
    if let v2xw_world::WorldSourceSpec::Procedural { generator, params } = &mut s.world.source {
        *generator = "world/source/procedural-radial".into();
        *params = serde_json::Value::Null;
    }
    let radial = v2xw_engine::wiring::build_world(&s).expect("radial");
    assert_ne!(
        v2xw_world::hash::content_hash_hex(&radial),
        v2xw_world::hash::content_hash_hex(&grid)
    );
    if let v2xw_world::WorldSourceSpec::Procedural { generator, .. } = &mut s.world.source {
        *generator = "world/source/procedural-hex".into();
    }
    assert!(s.validate().is_err(), "an unknown generator was accepted");

    // A SPaT role without SPaT in messages.sets is refused, naming the fix.
    let mut s = fleet(2, 1.0);
    s.actors.rsus = vec![
        serde_json::from_value(serde_json::json!({
            "position_m": [548.0, 240.0, 0.0], "roles": ["spat"],
            "profile": "rsu/commsignia-its-rs4",
        }))
        .expect("an RSU"),
    ];
    let why = format!("{:?}", s.validate().expect_err("refused"));
    assert!(why.contains("messages.sets"), "{why}");

    // An on-board unit that cannot sign is offered as unavailable.
    let surface = v2xw_engine::scenario::scenario_surface();
    let row = surface["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["x-path"] == "nodes.default_obu")
        .expect("the OBU field");
    let mk5 = row["x-options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["value"] == "obu/cohda-mk5")
        .expect("the MK5 is listed");
    assert_eq!(mk5["available"], false);
}
