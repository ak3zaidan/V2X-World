//! A deterministic synthetic run that exercises every channel the dataset exporters read.
//!
//! `crate::fixture` produces a run for the *container* — frames, chunks, seeking. This one
//! produces a run for the *dataset*: spawns with labels, pseudonym changes, attacker
//! actions, receptions, verifications, detector observations, reports, cases, decisions and
//! revocation stages, in their declared shapes.
//!
//! It exists because the engine does not yet emit most of these channels — Phase 1 emits
//! `gt.kinematics`, `node.tx` and `metric.sample` — and an exporter whose only test data is
//! the three channels that exist today would be untested where it matters. The fixture is
//! not a stand-in for an engine run: it is a record stream in the declared shapes, which is
//! exactly what the exporter's contract is against.
//!
//! # Determinism without an RNG
//!
//! Nothing here draws a random number. Every value is a closed-form function of the actor
//! index and the step, so the fixture is reproducible without touching an RNG stream and
//! without the exporter's tests depending on the engine's random sequence. Where a value
//! needs to look irregular — a detector score, a position offset — it comes from an
//! integer mix, not from a generator.

use v2xw_core::math::q3;

use crate::reader::RecordedRecord;

/// The shape of a synthetic dataset run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatasetShape {
    /// How many devices, of which the first [`DatasetShape::attackers`] are attackers.
    pub devices: u32,
    /// How many of the devices attack.
    pub attackers: u32,
    /// How many of the devices are faulty rather than malicious. Taken from the end of
    /// the fleet, so a device is never both.
    pub faulty: u32,
    /// How many infrastructure reporters. Their certificates are not vehicle pseudonyms,
    /// which is the case the frozen audit's `R3` check makes an allowance for.
    pub rsus: u32,
    /// How many 100 ms steps to run.
    pub steps: u32,
    /// How many pseudonyms each device rotates through.
    pub pseudonyms: u32,
    /// Whether the authority revokes the attackers it convicts.
    pub revoke: bool,
    /// Whether one benign device is framed by a colluding attacker, which is what makes
    /// `malicious_false_report` and `false_positive` appear in the labels.
    pub collusion: bool,
}

impl Default for DatasetShape {
    fn default() -> Self {
        DatasetShape {
            devices: 6,
            attackers: 2,
            faulty: 1,
            rsus: 1,
            steps: 40,
            pseudonyms: 2,
            revoke: true,
            collusion: true,
        }
    }
}

impl DatasetShape {
    /// A shape with no attacker and no revocation — the all-benign edge case, where every
    /// report is a false positive and no device should have been revoked.
    #[must_use]
    pub fn all_benign() -> Self {
        DatasetShape {
            attackers: 0,
            faulty: 0,
            revoke: false,
            collusion: false,
            ..Default::default()
        }
    }

    fn is_attacker(&self, actor: u32) -> bool {
        actor < self.attackers
    }

    fn is_faulty(&self, actor: u32) -> bool {
        !self.is_attacker(actor) && actor >= self.devices.saturating_sub(self.faulty)
    }

    fn is_rsu(&self, actor: u32) -> bool {
        actor >= self.devices && actor < self.devices + self.rsus
    }

    fn total_actors(&self) -> u32 {
        self.devices + self.rsus
    }
}

/// The fixture's two ACAs' HashedId8s, the issuers its certificates name.
pub const FIXTURE_ISSUER_A: &str = "a1a1a1a1a1a1a1a1";
/// See [`FIXTURE_ISSUER_A`].
pub const FIXTURE_ISSUER_B: &str = "b2b2b2b2b2b2b2b2";

/// A pseudonym digest for one device's `k`th certificate.
///
/// A truncated SHA-256 of a domain-separated string, which is what the legacy pipeline
/// did and what a HashedId8 looks like on the wire: 16 hex characters.
#[must_use]
pub fn digest_of(actor: u32, k: u32) -> String {
    let hex = v2xw_core::hash::sha256_hex(format!("v2xw-fixture-pseudonym|{actor}|{k}").as_bytes());
    hex[..16].to_string()
}

/// A value in `[0, 1)` from an integer mix — irregular without an RNG.
fn mix(a: u64, b: u64) -> f64 {
    let h = v2xw_core::hash::sha256(format!("v2xw-fixture-mix|{a}|{b}").as_bytes());
    let v = u32::from_be_bytes([h[0], h[1], h[2], h[3]]);
    f64::from(v) / f64::from(u32::MAX)
}

fn rec(channel: &str, t: u64, value: &serde_json::Value) -> RecordedRecord {
    RecordedRecord {
        channel: channel.to_string(),
        sim_time: t,
        json: serde_json::to_vec(value).expect("a fixture record serialises"),
    }
}

const STEP_NS: u64 = 100_000_000;

/// The whole record stream, in emission order.
///
/// Channels covered: `gt.spawn`, `gt.kinematics`, `gt.attack.action`, `sec.cert`,
/// `node.tx`, `phy.rx`, `node.verify`, `mac.cbr`, `node.telemetry`, `det.observation`,
/// `ma.report`, `ma.case`, `ma.decision`, `proto.revocation`.
#[must_use]
pub fn records(shape: &DatasetShape) -> Vec<RecordedRecord> {
    let mut out = Vec::new();

    // ---- spawns, with their labels ----------------------------------------------
    for actor in 0..shape.total_actors() {
        let class = if shape.is_rsu(actor) { "rsu" } else { "car" };
        out.push(rec(
            "gt.spawn",
            0,
            &serde_json::json!({
                "t": 0,
                "actor": actor,
                "is_attacker": shape.is_attacker(actor),
                "attacker_role": if shape.is_attacker(actor) { "position_falsifier" } else { "none" },
                "colluding_group_id": if shape.collusion && shape.is_attacker(actor) { Some("grp_0") } else { None },
                "is_faulty": shape.is_faulty(actor),
                "class": class,
                "is_rsu": shape.is_rsu(actor),
            }),
        ));
    }

    // ---- pseudonyms: each device learns its certificates, rotating through them -----
    for actor in 0..shape.total_actors() {
        for k in 0..shape.pseudonyms.max(1) {
            // Spread the rotations across the run so a certificate has a real window.
            let at = u64::from(k) * u64::from(shape.steps) * STEP_NS
                / u64::from(shape.pseudonyms.max(1));
            out.push(rec(
                "sec.cert",
                at,
                &serde_json::json!({
                    "t": at,
                    "node": actor,
                    "event": "change",
                    "digest": digest_of(actor, k),
                }),
            ));
        }
    }

    // ---- the run --------------------------------------------------------------------
    let attack_from = u64::from(shape.steps) * STEP_NS / 4;
    let attack_to = u64::from(shape.steps) * STEP_NS * 3 / 4;
    let mut msg = 0u64;
    for step in 0..shape.steps {
        let t = u64::from(step) * STEP_NS;
        let t_s = q3(t as f64 / 1e9);
        let period = (u64::from(step) * u64::from(shape.pseudonyms.max(1))
            / u64::from(shape.steps.max(1))) as u32;

        for actor in 0..shape.total_actors() {
            if shape.is_rsu(actor) {
                continue;
            }
            let x = q3(f64::from(actor) * 40.0 + f64::from(step) * 12.5);
            let y = q3(f64::from(actor) * 3.5);
            let speed = q3(12.0 + mix(u64::from(actor), u64::from(step)) * 3.0);
            out.push(rec(
                "gt.kinematics",
                t,
                &serde_json::json!({
                    "t": t, "actor": actor, "x_m": x, "y_m": y, "z_m": 0.0,
                    "speed_mps": speed, "acc_mps2": 0.0, "heading_rad": 0.0,
                    "lane": 1, "lane_pos_m": x, "class": "car",
                }),
            ));

            let attacking = shape.is_attacker(actor) && t >= attack_from && t <= attack_to;
            if attacking {
                out.push(rec(
                    "gt.attack.action",
                    t,
                    &serde_json::json!({
                        "t": t, "actor": actor, "attacker": "ConstPosOffset",
                        "action": "falsify_position", "fields": ["pos"],
                        "changed_bytes_on_air": true, "msg": msg,
                    }),
                ));
            }

            let digest = digest_of(actor, period.min(shape.pseudonyms.saturating_sub(1)));
            out.push(rec(
                "node.tx",
                t,
                &serde_json::json!({
                    "t": t, "node": actor, "msg": msg, "msg_type": "bsm",
                    "bytes_on_wire": 421, "payload_bytes": 39, "envelope_bytes": 111,
                    "airtime_us": 588, "mcs": 2, "power_dbm": 20.0, "channel": 180,
                    "ac": 2, "dcc_state": "relaxed", "signer": "digest", "t_generated": t,
                }),
            ));
            let _ = &digest;

            // One neighbour receives it, which gives the receiver-logs and net-trace
            // profiles their rows.
            let rx = (actor + 1) % shape.devices.max(1);
            if rx != actor {
                let dist = q3(40.0 + mix(u64::from(actor), u64::from(step) + 7) * 200.0);
                let rssi = v2xw_core::math::quantize_to(
                    -60.0 - mix(u64::from(actor), u64::from(step) + 13) * 30.0,
                    crate::grid::Q_DB,
                );
                let lost = mix(u64::from(actor), u64::from(step) + 23) < 0.1;
                out.push(rec(
                    "phy.rx",
                    t,
                    &serde_json::json!({
                        "t_start": t, "t_end": t + 588_000, "tx": actor, "rx": rx,
                        "msg": msg, "rssi_dbm": rssi, "sinr_db": v2xw_core::math::quantize_to(rssi + 95.0, crate::grid::Q_DB),
                        "outcome": if lost { "lost" } else { "ok" },
                        "cause": if lost { Some("per") } else { None },
                        "dist_m": dist, "candidate": true, "payload_bytes": 39,
                    }),
                ));
                if !lost {
                    out.push(rec(
                        "node.verify",
                        t,
                        &serde_json::json!({
                            "t_enqueue": t, "t_start": t + 100_000, "t_done": t + 1_100_000,
                            "node": rx, "primitive": "ecdsa-p256", "cost_us": 1000,
                            "outcome": "valid", "policy": "verify-on-demand",
                            "msg": msg, "queue_depth": 1,
                        }),
                    ));
                }
            }
            msg += 1;
        }

        // Per-node MAC and telemetry samples, once a second.
        if step % 10 == 0 {
            for actor in 0..shape.devices {
                out.push(rec(
                    "mac.cbr",
                    t,
                    &serde_json::json!({
                        "t": t, "node": actor, "channel": 180,
                        "cbr": v2xw_core::math::quantize_to(
                            mix(u64::from(actor), u64::from(step) + 31) * 0.4,
                            crate::grid::Q_RATIO),
                        "busy_us": 12_000, "window_us": 100_000,
                    }),
                ));
                out.push(rec(
                    "node.telemetry",
                    t,
                    &serde_json::json!({
                        "t": t, "node": actor,
                        "cpu": v2xw_core::math::quantize_to(0.1 + mix(u64::from(actor), u64::from(step)) * 0.3, crate::grid::Q_RATIO),
                        "hsm": 0.05, "ram_bytes": 4_194_304, "storage_bytes": 1_048_576,
                        "verify_queue_depth": 2,
                    }),
                ));
            }
        }

        // ---- detection: neighbours observe an attacking device and report it ---------
        if t >= attack_from && t <= attack_to && step % 5 == 0 {
            for attacker in 0..shape.attackers {
                let subject = digest_of(attacker, period.min(shape.pseudonyms.saturating_sub(1)));
                // The reporters are the RSU (always trusted infrastructure) and one
                // neighbouring vehicle, which is the mix the audit's R3 allowance covers.
                let reporters: Vec<u32> = if shape.rsus > 0 {
                    vec![shape.devices, (attacker + 2) % shape.devices.max(1)]
                } else {
                    vec![(attacker + 2) % shape.devices.max(1)]
                };
                for reporter in reporters {
                    let score = v2xw_core::math::quantize_to(
                        1.2 + mix(u64::from(reporter), u64::from(step)) * 2.0,
                        1e-3,
                    );
                    out.push(rec(
                        "det.observation",
                        t,
                        &serde_json::json!({
                            "t": t, "node": reporter, "detector": "positionJump",
                            "subject": subject, "score": score,
                        }),
                    ));
                    out.push(rec(
                        "ma.report",
                        t,
                        &serde_json::json!({
                            "t": t, "reporter": reporter, "subject": subject,
                            "detector": "positionJump",
                        }),
                    ));
                }
            }
        }

        // ---- collusion: an attacker frames a benign device --------------------------
        if shape.collusion && shape.attackers > 0 && step == shape.steps / 2 {
            let victim = shape.devices - 1;
            let subject = digest_of(victim, period.min(shape.pseudonyms.saturating_sub(1)));
            out.push(rec(
                "det.observation",
                t,
                &serde_json::json!({
                    "t": t, "node": 0, "detector": "positionJump",
                    "subject": subject, "score": 1.5,
                }),
            ));
            out.push(rec(
                "ma.report",
                t,
                &serde_json::json!({
                    "t": t, "reporter": 0, "subject": subject, "detector": "positionJump",
                }),
            ));
        }
        let _ = t_s;
    }

    // ---- the authority's cases, decisions and revocation stages --------------------
    let decide_at = attack_to + STEP_NS;
    for attacker in 0..shape.attackers {
        let subject = digest_of(attacker, shape.pseudonyms.saturating_sub(1));
        out.push(rec(
            "ma.case",
            attack_to,
            &serde_json::json!({
                "t": attack_to, "subject": subject, "trigger": "report_threshold",
                "cluster_size": 4, "num_distinct_reporters": 2, "linkage_result": "same",
            }),
        ));
        out.push(rec(
            "ma.decision",
            decide_at,
            &serde_json::json!({
                "t": decide_at, "subject": subject,
                "decision": if shape.revoke { "revoke" } else { "dismiss" },
            }),
        ));
        if shape.revoke {
            for (i, stage) in ["decision", "issued", "published", "downloaded", "enforced"]
                .iter()
                .enumerate()
            {
                let at = decide_at + (i as u64) * STEP_NS;
                out.push(rec(
                    "proto.revocation",
                    at,
                    &serde_json::json!({
                        "t": at, "stage": stage, "id": subject,
                        "entries": u64::from(attacker) + 1,
                        "size_bytes": 64 * (u64::from(attacker) + 1),
                        "node": if *stage == "downloaded" || *stage == "enforced" { Some(1) } else { None },
                    }),
                ));
            }
        }
    }

    out
}

/// The node id of the `i`th fixture RSU (the engine's RSU id space starts at 2^30).
pub const FIXTURE_RSU_BASE: u32 = 1 << 30;

/// The first step at which the fixture's attackers falsify.
#[must_use]
pub fn release_attack_from_step(shape: &DatasetShape) -> u32 {
    shape.steps / 4
}

/// A synthetic run on the channels the dataset release reads (docs/dataset/schema.md and
/// docs/dataset/record-contract.md), carrying **every** record-contract field, so the
/// release writer's tests exercise the populated path and not only the null one.
///
/// Channels: `gt.device`, `gt.kinematics`, `gt.subject`, `gt.attack.action`,
/// `gt.tx_truth`, `node.tx`, `phy.rx`, `node.rx_sec`, `mac.cbr`, `node.pki_event`,
/// `pki.report_fate`, `node.report_built`, `ma.received`, `ma.decision`, `ma.crl_entry`, `app.warning`,
/// `gt.device_exposure`. Records come in emission order: at one instant the receptions of
/// a frame precede its `node.tx`, as the engine writes them.
#[must_use]
#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
pub fn release_records(shape: &DatasetShape) -> Vec<RecordedRecord> {
    use serde_json::json;
    let mut out = Vec::new();
    let vehicles = shape.devices;
    let end_ns = u64::from(shape.steps) * STEP_NS;
    let from = release_attack_from_step(shape);
    let report_step = from + 2;
    let decide_step = from + 6;
    let crl_step = decide_step + 1;
    let hex = |s: String| v2xw_core::hash::sha256_hex(s.as_bytes())[..16].to_string();
    let la_link = |n: u32| hex(format!("v2xw-fixture-la|{n}"));
    let cert_at =
        |n: u32, step: u32| digest_of(n, (step * shape.pseudonyms.max(1)) / shape.steps.max(1));
    let rsus: Vec<u32> = (0..shape.rsus).map(|i| FIXTURE_RSU_BASE + i).collect();

    for n in (0..vehicles).chain(rsus.iter().copied()) {
        let rsu = n >= FIXTURE_RSU_BASE;
        out.push(rec(
            "gt.device",
            0,
            &json!({"t": 0, "node": n, "class": if rsu { "rsu" } else { "vehicle" },
                    "provider": if n % 2 == 0 { "provider-a" } else { "provider-b" },
                    "gnss_class": if rsu { "surveyed" } else { "automotive-l1" },
                    "faulty": shape.is_faulty(n) && !rsu,
                    "fault_kind": if shape.is_faulty(n) && !rsu { json!("gnss_bias") } else { json!(null) }}),
        ));
        if !rsu {
            out.push(rec(
                "gt.kinematics",
                0,
                &json!({"t": 0, "actor": n, "node": n, "x_m": 0.0, "y_m": 0.0}),
            ));
            out.push(rec(
                "node.pki_event",
                1_000,
                &json!({"t": 1_000, "node": n, "event": "enrolled", "cert_id": null, "crl_series": null,
                        "crl_number": null, "crl_entries": null, "bytes": 512, "latency_ms": 120.0}),
            ));
            out.push(rec(
                "node.pki_event",
                2_000,
                &json!({"t": 2_000, "node": n, "event": "batch_downloaded", "cert_id": cert_at(n, 0),
                        "crl_series": null, "crl_number": null, "crl_entries": null, "bytes": 4096,
                        "latency_ms": 340.0}),
            ));
            // The OBU's credential store: each pseudonym certificate's COER octets
            // (implicit, version 3, issued by its provider's ACA).
            let issuer = if n % 2 == 0 {
                FIXTURE_ISSUER_A
            } else {
                FIXTURE_ISSUER_B
            };
            let certs: serde_json::Map<String, serde_json::Value> = (0..shape.pseudonyms)
                .map(|k| {
                    (
                        digest_of(n, k),
                        json!(format!("00030180{issuer}{}", "00".repeat(24))),
                    )
                })
                .collect();
            out.push(rec(
                "node.credentials",
                0,
                &json!({"t": 0, "node": n, "certificates": certs}),
            ));
            for k in 0..shape.pseudonyms {
                out.push(rec(
                    "gt.subject",
                    3_000,
                    &json!({"t": 3_000, "subject": digest_of(n, k), "actor": n}),
                ));
            }
        }
    }

    for step in 0..shape.steps {
        let base = u64::from(step) * STEP_NS;
        for n in 0..vehicles {
            let t = base + u64::from(n) * 1_000_000 + 1_000_000;
            let msg = u64::from(step) * u64::from(vehicles) + u64::from(n);
            let attacking = shape.is_attacker(n) && step >= from;
            let ts = t as f64 * 1e-9;
            let true_x = 100.0 + 10.0 * f64::from(n) + 12.0 * ts;
            let true_y = 30.0 * f64::from(n);
            let (x, y) = if attacking {
                (true_x - 12.0 * ts + 3.0, true_y)
            } else {
                (true_x, true_y)
            };
            let speed = if attacking { 0.0 } else { 12.0 };
            let cert = cert_at(n, step);
            let revoked = shape.revoke && shape.is_attacker(n) && step > crl_step;
            if attacking {
                out.push(rec(
                    "gt.attack.action",
                    t - 500_000,
                    &json!({"t": t - 500_000, "actor": n, "attacker": "threat/attacker/legacy/ConstPos",
                            "action": "FalsifyOutgoing", "fields": ["position", "speed"],
                            "changed_bytes_on_air": true, "msg": msg, "magnitude": 1.5}),
                ));
            }
            let spdu = hex(format!("v2xw-fixture-spdu|{msg}"));
            for r in (0..vehicles).filter(|&r| r != n) {
                let lost = mix(msg, u64::from(r)) < 0.2;
                out.push(rec(
                    "phy.rx",
                    t,
                    &json!({"t_start": t - 400_000, "t_end": t, "tx": n, "rx": r, "msg": msg,
                            "rssi_dbm": -60.0 - 3.0 * f64::from(r), "sinr_db": 20.0,
                            "outcome": if lost { "lost" } else { "ok" },
                            "dist_m": 10.0 * f64::from(n.abs_diff(r)) + 1.0}),
                ));
                if !lost {
                    out.push(rec(
                        "node.rx_sec",
                        t + 2_000_000,
                        &json!({"t": t + 2_000_000, "rx": r, "msg": msg, "spdu_digest": spdu, "cert_id": cert,
                                "verify_result": if revoked { "on_crl" } else { "ok" },
                                "verify_latency_us": 2349, "cbr": 0.12}),
                    ));
                }
            }
            out.push(rec(
                "node.tx",
                t,
                // the record contract's layout (docs/dataset/record-contract.md §1): new
                // fields under `content`, the certificate in the existing `pseudonym`
                &json!({"t": t - 400_000, "node": n, "msg": msg, "msg_type": "bsm", "bytes_on_wire": 270,
                        "spdu_bytes": 223, "signer": if step % 5 == 0 { "certificate" } else { "digest" },
                        "pseudonym": cert,
                        "content": {"spdu_digest": spdu, "digest_basis": "fields", "psid": 32,
                                    "msg_count": step % 128, "temp_id": &cert[..8],
                                    "sec_mark_ms": (t / 1_000_000) % 60_000,
                                    "generation_time_us": 700_000_000_000_000_u64 + t / 1_000,
                                    "lat_deg": y / 111_320.0, "lon_deg": x / 111_320.0, "elev_m": 4.9,
                                    "pos_semi_major_m": 2.5, "pos_semi_minor_m": 1.5, "pos_orientation_deg": 30.0,
                                    "speed_mps": speed, "heading_deg": 90.0,
                                    "accel_long_mps2": 0.1, "accel_lat_mps2": 0.0, "accel_vert_mps2": 0.0,
                                    "yaw_rate_dps": 0.0, "brakes": 0, "event_flags": 0, "length_m": 4.8,
                                    "width_m": 1.9, "claimed_x_m": x, "claimed_y_m": y}}),
            ));
            out.push(rec(
                "gt.tx_truth",
                t,
                &json!({"t": t, "msg": msg, "node": n, "x_m": true_x, "y_m": true_y, "speed_mps": 12.0,
                        "heading_deg": 90.0, "accel_mps2": 0.1,
                        "gnss_error_m": if shape.is_faulty(n) { 9.0 } else { 0.8 }, "fix_to_stamp_ms": 12.0}),
            ));
            if step % 10 == 0 {
                out.push(rec(
                    "mac.cbr",
                    t,
                    &json!({"t": t, "node": n, "cbr": 0.1 + 0.01 * f64::from(n)}),
                ));
            }
        }
        if step >= 1 && step % (shape.steps / shape.pseudonyms.max(1)).max(1) == 0 {
            for n in 0..vehicles {
                out.push(rec(
                    "node.pki_event",
                    base + 500_000,
                    &json!({"t": base + 500_000, "node": n, "event": "pseudonym_change", "cert_id": cert_at(n, step),
                            "crl_series": null, "crl_number": null, "crl_entries": null, "bytes": null,
                            "latency_ms": null}),
                ));
            }
        }
    }

    // ---- reports, the MA, the CRL ----
    let reporter_of = |a: u32| {
        (0..vehicles)
            .find(|r| !shape.is_attacker(*r) && *r != a)
            .unwrap_or(0)
    };
    let mut subjects: Vec<(u32, u32)> = (0..shape.attackers).map(|a| (a, reporter_of(a))).collect();
    if shape.collusion && shape.attackers > 0 {
        // attacker 0 files a false report against the last benign vehicle
        let victim = (0..vehicles)
            .rev()
            .find(|v| !shape.is_attacker(*v) && !shape.is_faulty(*v));
        if let Some(v) = victim {
            subjects.push((v, 0));
        }
    }
    for (i, (subject, reporter)) in subjects.iter().copied().enumerate() {
        let t = u64::from(report_step) * STEP_NS + 50_000_000 + i as u64 * 1_000;
        let cert = cert_at(subject, report_step);
        let internal = format!("r-{reporter}-{cert}-{t}");
        let digest = hex(format!("v2xw-fixture-mbr|{internal}"));
        let ev_msg = u64::from(report_step) * u64::from(vehicles) + u64::from(subject);
        let ev = hex(format!("v2xw-fixture-spdu|{ev_msg}"));
        let truth =
            json!({"subject_node": subject, "subject_attacker": shape.is_attacker(subject)});
        for (k, fate) in ["stored", "uploaded", "delivered"].iter().enumerate() {
            let tf = t + k as u64 * 100_000_000;
            // As the engine writes them: the report's octets (digest, size, path, evidence)
            // exist only once it is built for the MA, on `delivered`.
            let built = *fate == "delivered";
            out.push(rec(
                "pki.report_fate",
                tf,
                &json!({"t": tf, "report_id": internal, "reporter": reporter, "fate": fate, "weight": 1.0,
                        "subject": cert,
                        "report_digest": if built { json!(digest) } else { json!(null) },
                        "size_bytes": if built { json!(812) } else { json!(null) },
                        "path": if built { json!("cellular") } else { json!(null) },
                        "evidence_spdu_digests": if built { json!([ev]) } else { json!([]) },
                        "truth": truth,
                        "checks": ["ConstantPosition", "staleOrReplay"], "score": 1.5}),
            ));
            if *fate == "uploaded" {
                // The build, when the upload reaches the proxy (`node.report_built`).
                let tb = tf + 50_000_000;
                out.push(rec(
                    "node.report_built",
                    tb,
                    &json!({"t": tb, "reporter": reporter, "report_id": internal, "subject": cert,
                            "report_digest": digest, "size_bytes": 812, "path": "cellular",
                            "evidence_spdu_digests": [ev], "checks": [], "score": null}),
                ));
            }
        }
        if i == 0 {
            // A second report on the same evidence that its reporter's store evicted before
            // any upload: the MA never sees it, but its reporter knows its checks.
            let lost = format!("r-{reporter}-{cert}-{}", t + 1_000);
            for (k, fate) in ["stored", "evicted_quota"].iter().enumerate() {
                let tf = t + 1_000 + k as u64 * 200_000_000;
                out.push(rec(
                    "pki.report_fate",
                    tf,
                    &json!({"t": tf, "report_id": lost, "reporter": reporter, "fate": fate, "weight": 1.0,
                            "subject": cert, "report_digest": null, "size_bytes": null, "path": null,
                            "evidence_spdu_digests": [], "truth": truth,
                            "checks": ["ConstantPosition"], "score": 0.5}),
                ));
            }
        }
        let t_ma = t + 300_000_000;
        out.push(rec(
            "ma.received",
            t_ma,
            &json!({"t": t_ma, "report_id": internal, "ma": 1, "reporter_cert": cert_at(reporter, report_step),
                    "subject_cert": cert, "subtypes": ["ConstantPosition", "staleOrReplay"],
                    "generation_time": t, "evidence_start": t - 100_000_000, "evidence_end": t - 100_000_000,
                    "excess": 1.5, "issuer": "provider-a", "la_link": la_link(subject), "set_aside": null,
                    "report_digest": digest, "psid": 32, "mbr_type": "Implausible",
                    "subtype_codes": [{"type": 1, "subtype": 2, "name": "ConstantPosition", "params": ["window_ms=1000"]}],
                    "nonstandard_triggers": ["staleOrReplay"],
                    "generation_location": {"lat_deg": 0.0003, "lon_deg": 0.001, "elevation_m": 4.9},
                    "evidence": [{"spdu_digest": ev, "generation_time_us": 700_000_000_000_000_u64 + t / 1_000,
                                  "sec_mark_ms": 12, "msg_cnt": report_step % 128, "lat_deg": 0.0003,
                                  "lon_deg": 0.001, "speed_mps": 0.0, "heading_deg": 90.0,
                                  "accel_long_mps2": 0.1, "brakes": 0, "pos_accuracy_m": 2.5}],
                    "la_link_t": null}),
        ));
    }
    if shape.revoke {
        let t = u64::from(decide_step) * STEP_NS;
        for a in 0..shape.attackers {
            let cert = cert_at(a, report_step);
            let internal_t = u64::from(report_step) * STEP_NS + 50_000_000 + u64::from(a) * 1_000;
            let digest = hex(format!(
                "v2xw-fixture-mbr|r-{}-{cert}-{internal_t}",
                reporter_of(a)
            ));
            out.push(rec(
                "ma.decision",
                t,
                &json!({"t": t, "subject": cert, "decision": "revoke", "ma": 1, "rule": "legacy-window",
                        "subject_kind": "device", "la_link": la_link(a), "report_digests": [digest],
                        "n_reports": 1, "n_distinct_reporters": 1}),
            ));
            let tc = u64::from(crl_step) * STEP_NS;
            out.push(rec(
                "ma.crl_entry",
                tc,
                &json!({"t": tc, "t_issued": tc, "crl_series": 1, "crl_number": 1, "entry_kind": "linkage_seed",
                        "la_link": la_link(a), "i_period": 0, "t_effective": tc + 1_000_000}),
            ));
        }
        for n in 0..vehicles {
            let td = u64::from(crl_step) * STEP_NS + 20_000_000 + u64::from(n) * 1_000_000;
            out.push(rec(
                "node.pki_event",
                td,
                &json!({"t": td, "node": n, "event": "crl_downloaded", "cert_id": null, "crl_series": 1,
                        "crl_number": 1, "crl_entries": shape.attackers, "bytes": 300, "latency_ms": 80.0}),
            ));
        }
    }
    if shape.attackers > 0 {
        let t = u64::from(from + 1) * STEP_NS + 70_000_000;
        let victim = reporter_of(0);
        out.push(rec(
            "app.warning",
            t,
            &json!({"t": t, "node": victim, "app": "fcw", "subject": cert_at(0, from + 1), "fired": true,
                    "kind": "issue", "severity": "warning",
                    "msg": u64::from(from + 1) * u64::from(vehicles)}),
        ));
        // A warning that does not name the message it followed (as the apps record them).
        out.push(rec(
            "app.warning",
            t + 1_000_000,
            &json!({"t": t + 1_000_000, "node": victim, "app": "eebl", "subject": cert_at(0, from + 1),
                    "fired": true, "kind": "issue", "severity": "warning"}),
        ));
    }
    for n in (0..vehicles).chain(rsus.iter().copied()) {
        let attacker = shape.is_attacker(n);
        out.push(rec(
            "gt.device_exposure",
            end_ns,
            &json!({"t": end_ns, "node": n, "la_link": la_link(n), "joined": 0, "left": end_ns,
                    "attacker": attacker,
                    "attack_start": if attacker { json!(u64::from(from) * STEP_NS) } else { json!(null) }}),
        ));
    }
    out.sort_by_key(|r| r.sim_time);
    out
}

/// The whole dataset, assembled in `profile`.
///
/// # Errors
/// Whatever [`super::DatasetAssembler::finish`] returns.
pub fn dataset(
    shape: &DatasetShape,
    profile: super::DatasetProfile,
) -> crate::Result<super::MaDataset> {
    let mut a = super::DatasetAssembler::new(profile).with_sample_stride(4);
    a.ingest_all(&records(shape))?;
    a.finish()
}

/// A [`RunProvenance`](super::RunProvenance) with plausible, clearly-synthetic values —
/// never a fabricated real one.
///
/// The build time is a fixed string rather than a clock read, and it says `fixture` so it
/// cannot be mistaken for the provenance of a real run.
#[must_use]
pub fn provenance() -> super::RunProvenance {
    let mut config = std::collections::BTreeMap::new();
    config.insert(
        "scenario".to_string(),
        serde_json::Value::from("dataset-fixture"),
    );
    config.insert("duration_s".to_string(), serde_json::Value::from(4.0));
    config.insert("attacker_pct".to_string(), serde_json::Value::from(0.33));
    config.insert("n_rsus".to_string(), serde_json::Value::from(1));
    // `collude_pct` and `attacker_pct` are read by the frozen audit, not by this crate:
    // `C6_collusion_consistency` skips itself entirely when `collude_pct` is absent or
    // zero, and `V3_attacker_pct_plausible` skips when `attacker_pct` is. A config that
    // omitted them would make the audit report 34 passes and two silent skips over
    // exactly the labels the fixture exists to produce — a check that cannot fail.
    config.insert("collude_pct".to_string(), serde_json::Value::from(0.5));
    super::RunProvenance {
        master_seed: 0x00c0_ffee_5eed,
        scenario_hash: "4ef5aca22590dd69e10c91aede255eeb25e1d356b7a1f1d3da94510255969cf7"
            .to_string(),
        world_hash: "a939a698a4a19178d2d8a6639937d49baea15387d700e61fe4c8d5e586913d79".to_string(),
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit: "fixture".to_string(),
        platform: std::env::consts::ARCH.to_string(),
        build_utc: "fixture (no clock read)".to_string(),
        content_digest: "0921e0b9b03d0bf9c224a9c492f56000a6fcd53814616b05d7328f6addd47393"
            .to_string(),
        model_cards: vec![
            (
                "radio/propagation/log-distance".to_string(),
                "1.0.0".to_string(),
            ),
            (
                "security/envelope/1609dot2".to_string(),
                "1.0.0".to_string(),
            ),
            ("threat/detector/legacy-12".to_string(), "1.0.0".to_string()),
        ],
        models: vec![
            super::ModelProvenance {
                id: "radio/propagation/log-distance".to_string(),
                version: "1.0.0".to_string(),
                validation_status: v2xw_core::card::ValidationStatus::LiteratureChecked,
                content_hash: "11".repeat(32),
                todo_calibrate: 2,
                tiers: vec!["medium".to_string(), "high".to_string()],
            },
            super::ModelProvenance {
                id: "security/envelope/1609dot2".to_string(),
                version: "1.0.0".to_string(),
                validation_status: v2xw_core::card::ValidationStatus::UnitTested,
                content_hash: "22".repeat(32),
                todo_calibrate: 0,
                tiers: vec!["medium".to_string()],
            },
            super::ModelProvenance {
                id: "threat/detector/legacy-12".to_string(),
                version: "1.0.0".to_string(),
                validation_status: v2xw_core::card::ValidationStatus::Unvalidated,
                content_hash: "33".repeat(32),
                todo_calibrate: 5,
                tiers: vec!["abstract".to_string(), "medium".to_string()],
            },
        ],
        // The fixture's `node.tx` records all carry `msg_type: "bsm"`, and the J2735 BSM
        // codec is the hand-written real one (PSM, SRM and SSM are
        // the modelled ones), so the declaration says `uper`. `psm` is declared and unused,
        // which exercises the report's stale-declaration row — a fixture that agreed with
        // the implementation on every field would not test the join at all.
        message_encodings: std::collections::BTreeMap::from([
            (
                "bsm".to_string(),
                super::ByteProvenance::Real("uper".to_string()),
            ),
            (
                "psm".to_string(),
                super::ByteProvenance::SizeModel("1.0.0".to_string()),
            ),
        ]),
        config,
        world_licence: None,
        world_attribution: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::DatasetProfile;

    #[test]
    fn the_fixture_exercises_every_channel_the_exporters_read() {
        let recs = records(&DatasetShape::default());
        let mut channels: Vec<&str> = recs.iter().map(|r| r.channel.as_str()).collect();
        channels.sort_unstable();
        channels.dedup();
        for want in [
            "det.observation",
            "gt.attack.action",
            "gt.kinematics",
            "gt.spawn",
            "ma.case",
            "ma.decision",
            "ma.report",
            "mac.cbr",
            "node.telemetry",
            "node.tx",
            "node.verify",
            "phy.rx",
            "proto.revocation",
            "sec.cert",
        ] {
            assert!(channels.contains(&want), "the fixture never emits {want}");
        }
    }

    #[test]
    fn the_fixture_is_reproducible_without_an_rng() {
        let a = records(&DatasetShape::default());
        let b = records(&DatasetShape::default());
        assert_eq!(a, b);
    }

    #[test]
    fn a_device_is_never_both_an_attacker_and_faulty() {
        // The labels have to be disjoint or `C4_correctness_semantics` becomes
        // unfalsifiable: `correct` and `faulty_detection` would both be defensible.
        let shape = DatasetShape::default();
        for actor in 0..shape.devices {
            assert!(
                !(shape.is_attacker(actor) && shape.is_faulty(actor)),
                "{actor}"
            );
        }
    }

    #[test]
    fn the_fixture_assembles_into_a_dataset_with_reports_and_a_revocation() {
        let ds = dataset(&DatasetShape::default(), DatasetProfile::V1).expect("assemble");
        assert!(!ds.ma_reports.is_empty(), "no reports");
        assert_eq!(ds.ma_reports.len(), ds.gt_report_labels.len());
        assert!(!ds.gt_vehicle.is_empty());
        assert_eq!(ds.rsu_count, 1);
        assert!(!ds.revoked_devices().is_empty(), "nothing was revoked");
        // Every correctness class the fixture is meant to produce really appears, or the
        // conformance tests below would be checking an empty set.
        let classes: std::collections::BTreeSet<&str> = ds
            .gt_report_labels
            .iter()
            .map(|l| l.report_correctness.as_str())
            .collect();
        assert!(classes.contains("correct"), "{classes:?}");
        assert!(classes.contains("malicious_false_report"), "{classes:?}");
    }

    #[test]
    fn the_all_benign_shape_produces_no_attacker_and_no_revocation() {
        let ds = dataset(&DatasetShape::all_benign(), DatasetProfile::V2).expect("assemble");
        assert!(ds.gt_vehicle.iter().all(|v| !v.is_attacker));
        assert!(ds.revoked_devices().is_empty());
        assert!(ds.gt_attacks.is_empty());
        assert!(
            ds.gt_emissions_sample.iter().all(|e| !e.falsified),
            "no message is falsified in an all-benign run"
        );
    }
}
