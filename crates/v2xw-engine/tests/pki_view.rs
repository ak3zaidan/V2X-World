//! WP-E1: the PKI view on a recorded engine run, and every inspect query answered from
//! the recording alone.
//!
//! One run of the NYC Manhattan scenario on the default deployment (two providers on one
//! CTL), every third roadside unit, four constant-position attackers, real cryptography
//! and the report shuffle compressed to 5 s so revocations land inside 150 s. The
//! records are then read the way a client reads them:
//!
//! * `pki.state` names both providers and every component by stable id, with the root
//!   and ICA offline, and over the run carries every component certificate;
//! * `pki.flow` carries every SPDU exactly once with its bytes; the RA opens every
//!   request encrypted to it and the MA every real SM-MBR, by really decrypting them;
//!   every SPDU's bytes decode;
//! * `node.credentials` gives each vehicle's i × j pool, and `inspect.cert` on its active
//!   certificate, with only the recorded certificates and CTL, validates it through its
//!   own provider's chain to a CTL root — for vehicles of both providers;
//! * `pki.case` walks the board in order for every case, and at least one case is
//!   revoked by a published CRL whose bytes then make `inspect.cert` fail the accused's
//!   certificate on the CRL rule;
//! * `pki.knowledge`: no entity's own view holds a device id; ground truth does.
//!
//! With `V2XW_PKI_FIXTURES=<dir>` it writes the UI fixtures (`pki-run.jsonl`, the records
//! of a trimmed slice of the run, and `pki-answers.json`, inspect answers on them).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{Value, json};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};
use v2xw_proto::scms::pki_view::{self, CrlHead, Knowledge, NodeCredentials, PduRef, PkiFlow};

fn scenario() -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut s = Scenario::load(root.join("scenarios").join("phase2-manhattan.yaml"))
        .expect("the shipped scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    s.time.duration_s = 150.0;
    s.actors.rsus = s
        .actors
        .rsus
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 3 == 0)
        .map(|(_, r)| r)
        .collect();
    s.threats.attackers[0].count = Some(4);
    if let Some(p) = s.security.protocol.as_mut() {
        p.params["report_shuffle_window_s"] = json!(5.0);
        p.params["crl_cadence_s"] = json!(20);
    }
    s.security.crypto_mode = serde_json::from_value(json!("real")).expect("mode");
    s
}

/// The recording, as `(channel, t, json)` rows.
fn recording() -> &'static Vec<(String, u64, Value)> {
    static RUN: OnceLock<Vec<(String, u64, Value)>> = OnceLock::new();
    RUN.get_or_init(|| {
        let mut engine = Engine::build(scenario(), "").expect("builds");
        let mut rec = MemoryRecorder::new();
        engine.run(&mut rec).expect("runs");
        rec.records()
            .iter()
            .filter(|(_, r)| r.channel.starts_with("pki.") || r.channel == "node.credentials")
            .map(|(t, r)| {
                (
                    r.channel.to_string(),
                    *t,
                    serde_json::from_slice(&r.json).expect("json"),
                )
            })
            .collect()
    })
}

fn of(channel: &str) -> Vec<&'static Value> {
    recording()
        .iter()
        .filter(|(c, ..)| c == channel)
        .map(|(_, _, v)| v)
        .collect()
}

/// What a reader holds after the whole recording: certificates, CRL heads and bytes.
struct Store {
    certs: BTreeMap<String, String>,
    crls: Vec<CrlHead>,
    crl_hex: BTreeMap<String, String>,
    roots: Vec<String>,
    time32: u32,
}

fn store() -> Store {
    let mut s = Store {
        certs: BTreeMap::new(),
        crls: Vec::new(),
        crl_hex: BTreeMap::new(),
        roots: Vec::new(),
        time32: 0,
    };
    for v in of("pki.state") {
        for (k, h) in v["certificates"].as_object().expect("certificates") {
            s.certs
                .insert(k.clone(), h.as_str().expect("hex").to_string());
        }
        for c in v["crls"].as_array().expect("crls") {
            let head: CrlHead = serde_json::from_value(c.clone()).expect("a CRL head");
            if let Some(h) = &head.hex {
                s.crl_hex.insert(head.id8.clone(), h.clone());
            }
            if !s.crls.iter().any(|x| x.id8 == head.id8) {
                s.crls.push(head);
            }
        }
        s.roots = serde_json::from_value(v["ctl"]["roots"].clone()).expect("roots");
        s.time32 = u32::try_from(v["time32"].as_u64().expect("time32")).expect("u32");
    }
    for v in of("node.credentials") {
        let c: NodeCredentials = serde_json::from_value(v.clone()).expect("node.credentials");
        s.certs.extend(c.certificates);
    }
    s
}

fn rule(v: &Value, r: &str) -> String {
    v["trace"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["rule"] == r))
        .and_then(|x| x["status"].as_str())
        .unwrap_or_default()
        .to_string()
}

#[test]
fn the_state_names_both_providers_and_every_component() {
    let states = of("pki.state");
    assert!(
        states.len() >= 140,
        "one state a second, got {}",
        states.len()
    );
    let last = states.last().expect("a state");
    let labels: Vec<&str> = last["providers"]
        .as_array()
        .expect("providers")
        .iter()
        .map(|p| p["label"].as_str().expect("label"))
        .collect();
    assert_eq!(labels, ["provider-a", "provider-b"]);
    let s = store();
    let mut ids = BTreeSet::new();
    for p in last["providers"].as_array().expect("providers") {
        for c in p["components"].as_array().expect("components") {
            let id = c["id"].as_str().expect("id");
            assert!(ids.insert(id.to_string()), "{id} twice");
            let role = c["role"].as_str().expect("role");
            if role == "root" || role == "ica" {
                assert_eq!(c["status"], "offline", "{id}");
            }
            if let Some(cert) = c["cert"].as_str() {
                assert!(
                    s.certs.contains_key(cert),
                    "{id}'s certificate {cert} was carried"
                );
            }
        }
        for role in [
            "eca", "ra", "aca", "la1", "la2", "ma", "crlg", "root", "ica",
        ] {
            let label = p["label"].as_str().expect("label");
            assert!(
                ids.contains(&format!("{label}/{role}/0")),
                "{label} has no {role}"
            );
        }
    }
}

#[test]
fn every_spdu_is_recorded_once_with_bytes_and_recipients_open_theirs() {
    let flows: Vec<PkiFlow> = of("pki.flow")
        .into_iter()
        .map(|v| serde_json::from_value(v.clone()).expect("pki.flow"))
        .collect();
    let pdus: Vec<&PduRef> = flows.iter().flat_map(|f| &f.pdus).collect();
    let ids: BTreeSet<&str> = pdus.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids.len(), pdus.len(), "each SPDU once");
    assert!(pdus.len() > 100, "only {} SPDUs", pdus.len());
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for p in &pdus {
        *kinds.entry(p.kind.as_str()).or_default() += 1;
        let b = pki_view::unhex(
            p.hex
                .as_deref()
                .unwrap_or_else(|| panic!("{} has no bytes", p.id)),
        )
        .expect("hex");
        assert_eq!(b.len(), p.len as usize, "{}", p.id);
        let tree = pki_view::decode_tree(&b);
        let top = tree["layers"][0]["type"].as_str().unwrap_or("none");
        assert!(top != "opaque", "{} ({}) does not decode", p.id, p.kind);
        if let Some(o) = &p.opened {
            assert!(
                o.error.is_none(),
                "{} ({}) did not open: {:?}",
                p.id,
                p.kind,
                o.error
            );
        }
    }
    println!("{kinds:?}");
    let mut flows_by: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for f in &flows {
        *flows_by
            .entry((f.flow.as_str(), f.kind.as_str()))
            .or_default() += 1;
    }
    println!("{flows_by:?}");
    // Every SPDU rides the message that carried it, or a pre-run flow: none is left
    // unattributed.
    let lone: Vec<&str> = flows
        .iter()
        .filter(|f| f.kind == "pdu" && f.flow != "preload")
        .map(|f| f.name.as_str())
        .collect();
    assert!(
        flows.iter().any(|f| f.flow == "preload"),
        "the pre-run provisioning is published"
    );
    assert!(
        lone.is_empty(),
        "{} SPDUs no message carried: {lone:?}",
        lone.len()
    );
    // A case's instances are the flows the kernel ran.
    for c in of("pki.case") {
        let i = c["instance"].as_str().expect("instance");
        assert!(
            flows.iter().any(|f| f.instance == i),
            "no flow has the case's instance {i}"
        );
    }
    for (kind, by) in [
        ("EeRaCertRequestSpdu", "/ra/0"),
        ("SecuredMisbehaviorReport", "/ma/0"),
    ] {
        let opened = pdus
            .iter()
            .filter(|p| p.kind == kind && p.to.ends_with(by))
            .filter(|p| p.opened.as_ref().is_some_and(|o| o.hex.is_some()))
            .count();
        assert!(opened > 0, "no {kind} was opened by its recipient {by}");
    }
    let providers: BTreeSet<&str> = flows.iter().map(|f| f.provider.as_str()).collect();
    assert_eq!(providers.len(), 2, "both providers' flows are published");
}

#[test]
fn a_vehicles_active_certificate_validates_from_the_recording_alone() {
    let s = store();
    let mut last: BTreeMap<u32, NodeCredentials> = BTreeMap::new();
    for v in of("node.credentials") {
        let c: NodeCredentials = serde_json::from_value(v.clone()).expect("node.credentials");
        last.insert(c.node, c);
    }
    assert!(last.len() > 20, "only {} vehicles", last.len());
    let mut valid_by_provider: BTreeMap<String, usize> = BTreeMap::new();
    for c in last.values().filter(|c| !c.revoked) {
        let Some(active) = &c.active else { continue };
        assert!(
            c.cells
                .iter()
                .any(|x| x.state == "active" && x.id8.as_ref() == Some(active)),
            "the grid marks the active certificate"
        );
        assert!(
            c.history.contains(active),
            "the history includes the active pseudonym"
        );
        let v = pki_view::inspect_cert(active, &s.certs, &s.roots, &[], &BTreeMap::new(), s.time32)
            .unwrap_or_else(|| panic!("{active} of node {} is not in the recording", c.node));
        if v["valid"] == true {
            *valid_by_provider.entry(c.provider.clone()).or_default() += 1;
        } else {
            panic!("node {} ({}): {v}", c.node, c.provider);
        }
    }
    assert_eq!(valid_by_provider.len(), 2, "{valid_by_provider:?}");
}

#[test]
fn cases_walk_the_board_and_a_published_crl_revokes_the_accused() {
    let cases = of("pki.case");
    let mut by_case: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for c in &cases {
        by_case
            .entry(c["case"].as_str().expect("case"))
            .or_default()
            .push(c);
    }
    assert!(!by_case.is_empty(), "no case was opened");
    let order = |s: &str| pki_view::CASE_STAGES.iter().position(|x| *x == s);
    let mut published = 0;
    for (id, stages) in &by_case {
        let pos: Vec<usize> = stages
            .iter()
            .filter_map(|c| order(c["stage"].as_str().expect("stage")))
            .collect();
        // Strictly forward, except `enforced`, which repeats once per enforcing vehicle.
        let enforced = order("enforced");
        assert!(
            pos.windows(2)
                .all(|w| w[0] < w[1] || (w[0] == w[1] && Some(w[0]) == enforced)),
            "{id} out of order: {stages:?}"
        );
        published += usize::from(stages.iter().any(|c| c["stage"] == "crl-published"));
    }
    assert!(
        published > 0,
        "no case reached a published CRL: {by_case:?}"
    );
    // The accused: every vehicle with a pseudonym the recorded CRLs revoke. The recorded
    // CRLs revoke someone, and no more vehicles than the run armed as attackers.
    let accused = revoked_vehicles();
    let vehicles: BTreeSet<u32> = of("node.credentials")
        .iter()
        .filter_map(|v| v["node"].as_u64().and_then(|n| u32::try_from(n).ok()))
        .collect();
    assert!(
        !accused.is_empty(),
        "no recorded certificate fails the CRL rule"
    );
    let attackers = scenario().threats.attackers[0].count.expect("a count") as usize;
    assert!(
        accused.len() <= attackers,
        "{} of {} vehicles revoked, {attackers} attackers",
        accused.len(),
        vehicles.len()
    );
}

/// E3 follow-ups. `enforced` is stamped once per vehicle that installs the case's entry, at
/// its install instant, with the running count, so "n of N" keeps counting (it was stamped
/// once and froze at the first count). And `reports-in` names the evidence type and
/// subtypes the MA decoded from each report the case was opened on ([SM-MBR] §2.3).
#[test]
fn enforcement_is_stamped_per_vehicle_and_cases_name_their_evidence() {
    let cases = of("pki.case");
    let mut by_case: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for c in &cases {
        by_case.entry(c["case"].as_str().expect("case")).or_default().push(c);
    }
    let mut most = 0;
    let (mut real, mut modelled, mut shared) = (0, 0, 0);
    for (id, stages) in &by_case {
        let reports_in = stages
            .iter()
            .find(|c| c["stage"] == "reports-in")
            .unwrap_or_else(|| panic!("{id} has no reports-in"));
        let evidence = reports_in["detail"]["evidence"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: no evidence list"));
        let direct = reports_in["detail"]["direct"].as_bool().expect("direct");
        assert_eq!(evidence.len(), if direct { 1 } else { 2 }, "{id}: one entry per report");
        for e in evidence {
            // A report on the modelled path says why, and which detector checks fired.
            let Some(t) = e["type"].as_u64() else {
                let why = e["modelled"].as_str().unwrap_or_else(|| panic!("{id}: unexplained {e}"));
                assert!(!why.is_empty(), "{id}: {e}");
                assert!(
                    e["checks"].as_array().is_some_and(|c| !c.is_empty()),
                    "{id}: a modelled report names its checks: {e}"
                );
                modelled += 1;
                continue;
            };
            real += 1;
            shared += usize::from(e["shared"] == true);
            // A real report: the type is the module's number and name.
            let name = e["type_name"].as_str().expect("type name");
            assert_eq!(
                (t, name),
                match t {
                    1 => (1, "1VInconsistent"),
                    2 => (2, "Implausible"),
                    3 => (3, "ObsImplausible"),
                    _ => panic!("{id}: type {t} is not in Psid20MbrType"),
                },
            );
            let subtypes = e["subtypes"].as_array().expect("subtypes");
            assert!(!subtypes.is_empty(), "{id}: a report has at least one subtype");
            assert!(e["verified"].is_boolean(), "{id}: {e}");
        }
        let enforced: Vec<&&Value> = stages.iter().filter(|c| c["stage"] == "enforced").collect();
        for (k, c) in enforced.iter().enumerate() {
            let d = &c["detail"];
            assert_eq!(d["installed"].as_u64(), Some(k as u64 + 1), "{id}: running count");
            assert!(d["installed"].as_u64() <= d["of"].as_u64(), "{id}: {d}");
            assert!(!d.to_string().contains("ee/"), "{id}: the MA's table names a vehicle");
        }
        assert!(
            enforced.windows(2).all(|w| w[0]["t"].as_u64() <= w[1]["t"].as_u64()),
            "{id}: install instants go forward"
        );
        most = most.max(enforced.len());
    }
    assert!(
        most > 1,
        "no case counted past its first enforcing vehicle (the frozen 'n of N'): {most}"
    );
    // Real cryptography: real SM-MBR reports reach the MA, some of them shared by the
    // other provider's MA (two providers on one CTL).
    assert!(real > 0, "no case names a real report's evidence ({modelled} modelled)");
    println!("case evidence: {real} real ({shared} shared), {modelled} modelled");
}

/// Vehicles a recorded CRL revokes: some pseudonym they used fails the CRL rule.
fn revoked_vehicles() -> BTreeSet<u32> {
    let s = store();
    let mut out = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for v in of("node.credentials") {
        let c: NodeCredentials = serde_json::from_value(v.clone()).expect("nc");
        for cell in c.cells.iter().filter(|x| x.used) {
            let id8 = cell.id8.as_deref().expect("a used cell has a certificate");
            if !seen.insert((c.node, id8.to_string())) {
                continue;
            }
            let v = pki_view::inspect_cert(id8, &s.certs, &s.roots, &s.crls, &s.crl_hex, s.time32)
                .expect("recorded");
            if rule(&v, "crl") == "fail" {
                out.insert(c.node);
            }
        }
    }
    out
}

#[test]
fn no_entity_view_names_a_device() {
    let ks: Vec<Knowledge> = of("pki.knowledge")
        .into_iter()
        .map(|v| serde_json::from_value(v.clone()).expect("pki.knowledge"))
        .collect();
    let entities: BTreeSet<&str> = ks.iter().map(|k| k.entity.as_str()).collect();
    assert_eq!(
        entities.len(),
        12,
        "six roles for each of two providers: {entities:?}"
    );
    for k in &ks {
        let own = pki_view::inspect_knowledge(k, &k.entity);
        assert!(!own.to_string().contains("\"ee/"), "{}: {own}", k.entity);
    }
    assert!(ks.iter().any(|k| k.truth.to_string().contains("\"ee/")));
}

/// WP-E3: the MA's ground truth names, per case, the accused it reached through the PCA's
/// handle and the RA's request table, and its reporters. Every vehicle a recorded CRL
/// revokes (the linkage-seed walk through the real decoder, a separate path) is the
/// accused of a published case, and the cases accuse no vehicle that is not revoked.
#[test]
fn the_cases_ground_truth_accuses_exactly_the_vehicles_the_crls_revoke() {
    let device: BTreeMap<u64, String> = of("node.credentials")
        .iter()
        .map(|v| {
            (
                v["node"].as_u64().expect("node"),
                v["device"].as_str().expect("device").to_string(),
            )
        })
        .collect();
    let revoked: BTreeSet<String> = revoked_vehicles()
        .into_iter()
        .map(|n| device[&u64::from(n)].clone())
        .collect();
    assert!(!revoked.is_empty(), "no vehicle revoked");
    let published: BTreeSet<String> = of("pki.case")
        .iter()
        .filter(|c| c["stage"] == "crl-published")
        .map(|c| c["case"].as_str().expect("case").to_string())
        .collect();
    let mut accused = BTreeSet::new();
    let mut last: BTreeMap<String, &Value> = BTreeMap::new();
    for k in of("pki.knowledge")
        .into_iter()
        .filter(|k| k["role"] == "ma")
    {
        last.insert(k["entity"].as_str().expect("e").to_string(), k);
    }
    let mut seen = 0;
    for k in last.values() {
        for c in k["truth"]["cases"].as_array().expect("truth.cases") {
            seen += 1;
            let reporters = c["reporters"].as_array().expect("reporters");
            assert!(!reporters.is_empty(), "a case with no reporter: {c}");
            for r in reporters {
                assert!(r["reports"].as_u64().unwrap_or(0) > 0, "{c}");
            }
            if published.contains(c["case"].as_str().expect("case")) {
                for a in c["accused"].as_array().expect("accused") {
                    accused.insert(a.as_str().expect("device").to_string());
                }
            }
        }
    }
    assert!(
        seen >= published.len(),
        "{seen} cases in truth, {} published",
        published.len()
    );
    assert_eq!(
        accused, revoked,
        "accused by the MA's truth vs revoked by the CRLs"
    );
}

/// Writes the UI fixtures (E2/E3): a trimmed slice of the recording and inspect answers.
#[test]
fn fixtures() {
    let Ok(dir) = std::env::var("V2XW_PKI_FIXTURES") else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("dir");
    let rec = recording();
    // Flow instances: one enrolment and one provisioning per provider, every report and
    // revocation instance the cases name, plus every stage stamp of those.
    let cases = of("pki.case");
    let mut keep: BTreeSet<String> = cases
        .iter()
        .map(|c| c["instance"].as_str().expect("i").to_string())
        .collect();
    for c in &cases {
        if let Some(i) = c["detail"]["crl_instance"].as_str() {
            keep.insert(i.to_string());
        }
    }
    for p in ["provider-a", "provider-b"] {
        for flow in ["enrolment", "provisioning", "report"] {
            if let Some((_, _, v)) = rec.iter().find(|(c, _, v)| {
                c == "pki.flow" && v["provider"] == p && v["flow"] == flow && v["kind"] == "msg"
            }) {
                keep.insert(v["instance"].as_str().expect("i").to_string());
            }
        }
    }
    let states: Vec<usize> = rec
        .iter()
        .enumerate()
        .filter(|(_, (c, ..))| c == "pki.state")
        .map(|(i, _)| i)
        .collect();
    let first_state = states[0];
    let last_state = *states.last().expect("a state");
    // Vehicles: two per provider, and every revoked one.
    let mut vehicles: BTreeSet<u64> = revoked_vehicles().into_iter().map(u64::from).collect();
    let mut per: BTreeMap<String, usize> = BTreeMap::new();
    for (c, _, v) in rec {
        if c == "node.credentials" {
            let p = v["provider"].as_str().expect("p").to_string();
            let n = per.entry(p).or_default();
            if v["revoked"] == true || (*n < 2 && vehicles.insert(v["node"].as_u64().expect("n"))) {
                vehicles.insert(v["node"].as_u64().expect("n"));
                *n += 1;
            }
        }
    }
    let mut last_knowledge: BTreeMap<String, usize> = BTreeMap::new();
    for (i, (c, _, v)) in rec.iter().enumerate() {
        if c == "pki.knowledge" {
            last_knowledge.insert(v["entity"].as_str().expect("e").to_string(), i);
        }
    }
    // The chosen vehicles' backend ids, for their pre-run SPDUs.
    let devices: BTreeSet<String> = rec
        .iter()
        .filter(|(c, _, v)| {
            c == "node.credentials" && vehicles.contains(&v["node"].as_u64().unwrap_or(0))
        })
        .filter_map(|(_, _, v)| v["device"].as_str().map(str::to_string))
        .collect();
    let mut out = String::new();
    let mut pdu_ids = Vec::new();
    for (i, (c, t, v)) in rec.iter().enumerate() {
        let take = match c.as_str() {
            // The first and last, and every one that carries something new (a
            // certificate, a CRL version's bytes), so the slice is self-sufficient.
            "pki.state" => {
                i == first_state
                    || i == last_state
                    || v["certificates"].as_object().is_some_and(|m| !m.is_empty())
                    || v["crls"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|c| c.get("hex").is_some()))
            }
            "pki.flow" => {
                keep.contains(v["instance"].as_str().unwrap_or(""))
                    || (v["flow"] == "preload"
                        && v["pdus"].as_array().into_iter().flatten().any(|p| {
                            devices.contains(p["from"].as_str().unwrap_or(""))
                                || devices.contains(p["to"].as_str().unwrap_or(""))
                        }))
            }
            "pki.case" => true,
            "node.credentials" => vehicles.contains(&v["node"].as_u64().unwrap_or(0)),
            "pki.knowledge" => last_knowledge.values().any(|k| *k == i),
            _ => false,
        };
        if take {
            if c == "pki.flow" {
                for p in v["pdus"].as_array().into_iter().flatten() {
                    pdu_ids.push(p["id"].as_str().expect("id").to_string());
                }
            }
            out.push_str(&json!({"channel": c, "t": t, "record": v}).to_string());
            out.push('\n');
        }
    }
    std::fs::write(Path::new(&dir).join("pki-run.jsonl"), &out).expect("write");
    // Inspect answers on the slice, as the server gives them (`v2xw-server::pki`).
    let s = store();
    let flows: BTreeMap<String, PduRef> = of("pki.flow")
        .into_iter()
        .flat_map(|v| {
            serde_json::from_value::<PkiFlow>(v.clone())
                .expect("flow")
                .pdus
        })
        .map(|p| (p.id.clone(), p))
        .collect();
    let mut pdus = serde_json::Map::new();
    for id in pdu_ids.iter().take(400) {
        let p = &flows[id];
        let mut views = serde_json::Map::new();
        views.insert(
            "ground-truth".into(),
            pki_view::inspect_pdu(p, "ground-truth"),
        );
        if let Some(o) = &p.opened {
            views.insert(o.by.clone(), pki_view::inspect_pdu(p, &o.by));
            views.insert("other".into(), pki_view::inspect_pdu(p, "provider-a/la1/0"));
        }
        pdus.insert(id.clone(), Value::Object(views));
    }
    let mut certs = serde_json::Map::new();
    for (c, _, v) in rec {
        if c == "node.credentials" && vehicles.contains(&v["node"].as_u64().unwrap_or(0)) {
            for cell in v["cells"].as_array().into_iter().flatten() {
                if let Some(id8) = cell["id8"].as_str()
                    && !certs.contains_key(id8)
                    && let Some(a) = pki_view::inspect_cert(
                        id8, &s.certs, &s.roots, &s.crls, &s.crl_hex, s.time32,
                    )
                {
                    certs.insert(id8.to_string(), a);
                }
            }
        }
    }
    let mut knowledge = serde_json::Map::new();
    for v in of("pki.knowledge") {
        let k: Knowledge = serde_json::from_value(v.clone()).expect("k");
        knowledge.insert(
            k.entity.clone(),
            json!({"ground-truth": pki_view::inspect_knowledge(&k, "ground-truth"),
                   "entity": pki_view::inspect_knowledge(&k, &k.entity)}),
        );
    }
    let answers =
        json!({"inspect.pdu": pdus, "inspect.cert": certs, "inspect.knowledge": knowledge});
    std::fs::write(
        Path::new(&dir).join("pki-answers.json"),
        serde_json::to_vec(&answers).expect("json"),
    )
    .expect("write");
    println!("kept instances: {keep:?}");
    println!(
        "fixture: {} bytes, {} flow instances, {} vehicles",
        out.len(),
        keep.len(),
        vehicles.len()
    );
}
