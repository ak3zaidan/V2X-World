//! WP-E1: the PKI view's records and inspect answers, on a real run.
//!
//! One device enrolled and provisioned with real SPDUs, reported twice, resolved through
//! both LAs and revoked by a signed CRL; then every record shape and every inspect answer
//! is checked against what the run did:
//!
//! * every SPDU is attributed to the wire step that carried it, and the step's name is
//!   the one that kind travels under (`pdu_layer.rs`'s table);
//! * `pki.flow` carries each SPDU exactly once, and a PDU encrypted to the RA is opened
//!   by really decrypting it with the RA's key; the plaintext is hidden from every view
//!   but ground truth and the RA;
//! * `pki.state` lists every component with its offline status and carries each
//!   certificate once;
//! * `inspect.cert` validates the device's pseudonym certificate through the published
//!   chain to a CTL root, and fails the CRL rule once the signed CRL revokes it;
//! * `pki.case` walks the board's stages in order;
//! * `pki.knowledge` keeps the separation the design rests on: the ACA holds linkage
//!   values and no identity, the RA identities and no linkage value.
//!
//! With `V2XW_PKI_OUT=<dir>` it writes every PDU (hex) with the JER tree the real decoder
//! produced, for `tools/pki_view/check_jer.py` to compare against asn1tools.

mod common;

use std::collections::BTreeMap;

use common::{DEVICE_A, DEVICE_B, provisioned};
use serde_json::Value;
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::SpduKind;
use v2xw_proto::scms::pki_view::{self, PkiTracker};
use v2xw_proto::scms::run::ScmsRun;

const JMAX: u32 = 2;

fn kind_of(step: &str) -> Option<&'static [SpduKind]> {
    use SpduKind as K;
    Some(match step {
        "enrol-request" => &[K::EeEcaCertRequest],
        "enrol-response" | "enrol-response-proxied" => &[K::EcaEeCertResponse],
        "provisioning-request" | "provisioning-request-proxied" => &[K::EeRaCertRequest],
        "provisioning-ack" | "provisioning-ack-proxied" => &[K::RaEeCertAck],
        "batch-download-request" | "batch-download-request-proxied" => &[K::EeRaDownloadRequest],
        "report" => &[K::SecuredMisbehaviorReport, K::EeRaDownloadRequest],
        _ => return None,
    })
}

/// The revoked run: DEVICE_A provisioned, reported twice by DEVICE_B, resolved, revoked.
fn revoked() -> ScmsRun {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    let mut run = ScmsRun::new(p).expect("certificates encode");
    for d in [DEVICE_A, DEVICE_B] {
        run.add_device(d);
    }
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    provisioned(&mut run, DEVICE_B, 0, 1, JMAX);
    let (lv0, lv1) = {
        let d = &run.state.devices[&DEVICE_A];
        (d.credentials[&(0, 0)].lv, d.credentials[&(0, 1)].lv)
    };
    run.submit_report(DEVICE_B, 0, lv0);
    run.submit_report(DEVICE_B, 0, lv1);
    run.run().expect("the reports run");
    run.investigate(0, 1, 0, JMAX).expect("two reports");
    run.run().expect("the investigation runs");
    run
}

fn now(run: &ScmsRun) -> u64 {
    run.kernel.now()
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
fn every_spdu_rides_the_step_that_carried_it() {
    let run = revoked();
    let (steps, _) = run.kernel.steps_since(0);
    let mut checked = 0;
    for p in &run.state.pdus {
        let s = p.step.unwrap_or_else(|| panic!("{:?} has no step", p.kind));
        let step = &steps[s as usize];
        assert_eq!(step.to.index(), p.to, "{:?} on {}", p.kind, step.step);
        if let Some(kinds) = kind_of(step.step) {
            assert!(kinds.contains(&p.kind), "{:?} rode {}", p.kind, step.step);
            checked += 1;
        }
    }
    assert!(checked >= 8, "only {checked} PDUs were on named steps");

    // A device provisioned before the run (on a scratch kernel whose log is discarded):
    // its PDUs name no step of the run's kernel and are published as pre-run.
    let mut run = revoked();
    let before = run.state.pdus.len();
    run.preload(common::DEVICE_C, 0, 1, JMAX).expect("preload");
    let pre = &run.state.pdus[before..];
    assert!(!pre.is_empty(), "the preload emits PDUs");
    assert!(
        pre.iter().all(|p| p.preload && p.step.is_none()),
        "pre-run PDUs carry no step"
    );
    let flows = pki_view::drain_flows(std::slice::from_ref(&run), &mut PkiTracker::default());
    let lone: Vec<&pki_view::PkiFlow> = flows.iter().filter(|f| f.kind == "pdu").collect();
    assert_eq!(
        lone.len(),
        pre.len(),
        "only the pre-run PDUs ride no message"
    );
    assert!(lone.iter().all(|f| f.flow == "preload"));
}

#[test]
#[allow(clippy::too_many_lines)]
fn the_flow_records_carry_each_spdu_once_and_only_its_recipient_reads_it() {
    let run = revoked();
    let runs = std::slice::from_ref(&run);
    let mut tracker = PkiTracker::default();
    let flows = pki_view::drain_flows(runs, &mut tracker);
    assert!(
        pki_view::drain_flows(runs, &mut tracker).is_empty(),
        "a second drain repeats nothing"
    );
    let pdus: Vec<&pki_view::PduRef> = flows.iter().flat_map(|f| &f.pdus).collect();
    assert_eq!(pdus.len(), run.state.pdus.len(), "every PDU exactly once");
    let mut ids: Vec<&str> = pdus.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), pdus.len(), "PDU ids are unique");
    assert!(
        flows
            .iter()
            .any(|f| f.kind == "stage" && f.name == "installed"),
        "stage stamps are published"
    );
    let msg = flows
        .iter()
        .find(|f| f.name == "provisioning-request-proxied")
        .expect("a proxied request");
    assert_eq!(msg.to.as_deref(), Some("provider-a/ra/0"));
    assert!(
        msg.instance.starts_with("provider-a/provisioning/"),
        "{}",
        msg.instance
    );

    let req = pdus
        .iter()
        .find(|p| p.kind == "EeRaCertRequestSpdu" && p.to == "provider-a/ra/0")
        .expect("the RA receives a request");
    let opened = req.opened.as_ref().expect("the RA opens it");
    assert_eq!(opened.by, "provider-a/ra/0");
    assert!(opened.error.is_none(), "{:?}", opened.error);
    let plain = pki_view::unhex(opened.hex.as_deref().expect("plaintext")).expect("hex");
    let inner = pki_view::decode_tree(&plain);
    assert_eq!(inner["layers"][0]["type"], "Ieee1609Dot2Data", "{inner}");
    assert_eq!(
        inner["layers"][0]["content"], "signedCertificateRequest",
        "{inner}"
    );
    assert_eq!(
        inner["layers"][1]["type"], "SignedCertificateRequest",
        "{inner}"
    );

    let outer = pki_view::inspect_pdu(req, "ground-truth");
    assert_eq!(outer["tree"]["layers"][0]["content"], "encryptedData");
    assert!(outer["opened"]["tree"].is_object());
    for view in ["ra", "provider-a/ra/0"] {
        assert!(
            pki_view::inspect_pdu(req, view)["opened"]["hex"].is_string(),
            "{view}"
        );
    }
    for view in [
        "aca",
        "ma",
        "provider-a/la1/0",
        "ee/1000",
        "provider-b/ra/0",
    ] {
        let v = pki_view::inspect_pdu(req, view);
        assert!(
            v["opened"]["hex"].is_null() && v["opened"]["tree"].is_null(),
            "{view} must not read it"
        );
        assert!(v["opened"]["hidden"].is_string(), "{view}");
    }
    // A sealed PLV is opened by the ACA with the LA–ACA key; a private response is sealed
    // to the device and the backend cannot open it.
    let plv = pdus
        .iter()
        .find(|p| p.kind == "EncryptedIndividualPLV")
        .expect("a sealed PLV");
    assert_eq!(
        plv.opened
            .as_ref()
            .map(|o| (o.by.as_str(), o.error.clone())),
        Some(("provider-a/aca/0", None))
    );
    let private = pdus
        .iter()
        .find(|p| p.kind == "AcaEeCertResponsePrivateSpdu")
        .expect("a private response");
    assert_eq!(private.sealed_to.as_deref(), Some("ee"));
    assert!(private.opened.is_none());
    // (A real SM-MBR, opened by the MA, is checked on the engine run:
    // `v2xw-engine/tests/pki_view.rs`; `submit_report` here carries no secured report.)

    // The UI fixture of a whole in-run sequence (enrolment, provisioning, two reports, the
    // resolution, the CRL): every record this run publishes, in the engine's JSONL shape.
    if let Ok(dir) = std::env::var("V2XW_PKI_FIXTURES") {
        let t = now(&run);
        let mut tr = PkiTracker::default();
        let mut lines = Vec::new();
        let mut put = |channel: &str, t: u64, v: Value| {
            lines.push(serde_json::json!({"channel": channel, "t": t, "record": v}).to_string());
        };
        let state = pki_view::pki_state(runs, t, &mut tr).expect("state");
        put("pki.state", t, serde_json::to_value(&state).expect("json"));
        for f in pki_view::drain_flows(runs, &mut tr) {
            put("pki.flow", f.t, serde_json::to_value(&f).expect("json"));
        }
        for c in pki_view::drain_cases(runs, &mut tr, t, &|_, _| None) {
            put("pki.case", c.t, serde_json::to_value(&c).expect("json"));
        }
        for role in pki_view::KNOWLEDGE_ROLES {
            let k = pki_view::knowledge(&run, role, t, 64).expect("knowledge");
            put("pki.knowledge", t, serde_json::to_value(&k).expect("json"));
        }
        std::fs::write(format!("{dir}/pki-sequence.jsonl"), lines.join("\n") + "\n")
            .expect("write");
    }

    if let Ok(dir) = std::env::var("V2XW_PKI_OUT") {
        std::fs::create_dir_all(&dir).expect("out dir");
        let mut rows: Vec<Value> = Vec::new();
        for p in &pdus {
            let Some(b) = p.hex.as_deref().and_then(pki_view::unhex) else {
                continue;
            };
            rows.push(serde_json::json!({"id": p.id, "type": p.kind, "hex": p.hex,
                "tree": pki_view::decode_tree(&b)}));
            if let Some(h) = p.opened.as_ref().and_then(|o| o.hex.as_deref())
                && let Some(b) = pki_view::unhex(h)
            {
                rows.push(
                    serde_json::json!({"id": format!("{}#opened", p.id), "type": p.kind,
                    "hex": h, "tree": pki_view::decode_tree(&b)}),
                );
            }
        }
        std::fs::write(
            format!("{dir}/pdus.json"),
            serde_json::to_vec_pretty(&rows).expect("json"),
        )
        .expect("write");
    }
}

#[test]
fn the_state_lists_every_component_and_carries_each_certificate_once() {
    let run = revoked();
    let runs = std::slice::from_ref(&run);
    let mut tracker = PkiTracker::default();
    let s = pki_view::pki_state(runs, now(&run), &mut tracker).expect("a state");
    let p = &s.providers[0];
    assert_eq!(p.label, "provider-a");
    assert!(p.region.assumption);
    let status: BTreeMap<&str, &str> = p
        .components
        .iter()
        .map(|c| (c.role.as_str(), c.status))
        .collect();
    for role in ["eca", "ra", "aca", "la1", "la2", "ma", "crlg", "dcm"] {
        assert_eq!(status.get(role), Some(&"online"), "{role}: {status:?}");
    }
    for role in ["root", "ica"] {
        assert_eq!(status.get(role), Some(&"offline"), "{role}: {status:?}");
    }
    let electors: Vec<_> = s
        .governance
        .iter()
        .filter(|c| c.role == "elector")
        .collect();
    assert_eq!(electors.len(), s.ctl.electors as usize);
    assert!(electors.iter().all(|c| c.status == "offline"));
    for c in p.components.iter().filter_map(|c| c.cert.as_ref()) {
        assert!(s.certificates.contains_key(c), "{c} carried");
    }
    let root = run.state.pki.as_ref().expect("a PDU PKI").root.cert.clone();
    let root32 = v2xw_core::hash::hex_encode(&v2xw_sec::cert::hashed_id32(&root).expect("hash"));
    assert!(
        s.ctl.roots.contains(&root32),
        "the provider's root is on the CTL: {:?}",
        s.ctl.roots
    );
    let crl = s
        .crls
        .iter()
        .find(|c| c.series == 1 && !c.delta)
        .expect("a linkage CRL head");
    assert!(crl.hex.is_some() && crl.entries >= 1, "{crl:?}");
    let again = pki_view::pki_state(runs, now(&run), &mut tracker).expect("a state");
    assert!(
        again.certificates.is_empty(),
        "certificates are carried once"
    );
    assert!(
        again.crls.iter().all(|c| c.hex.is_none()),
        "CRL bytes are carried once"
    );
}

/// What an inspector holds after reading `pki.state` and the device's `node.credentials`:
/// the state, the certificate store, the CRL bytes and the device's first certificate.
fn store(
    run: &ScmsRun,
) -> (
    pki_view::PkiState,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    String,
) {
    let mut tracker = PkiTracker::default();
    let s =
        pki_view::pki_state(std::slice::from_ref(run), now(run), &mut tracker).expect("a state");
    let mut certs = s.certificates.clone();
    let cred = &run.state.devices[&DEVICE_A].credentials[&(0, 0)];
    let c = &cred
        .implicit
        .as_ref()
        .expect("an implicit certificate")
        .certificate;
    let id8 = v2xw_core::hash::hex_encode(&v2xw_proto::scms::pdu::cert_id8(c));
    certs.insert(
        id8.clone(),
        v2xw_core::hash::hex_encode(&rasn::coer::encode(c).expect("coer")),
    );
    let crl_hex = s
        .crls
        .iter()
        .filter_map(|c| Some((c.id8.clone(), c.hex.clone()?)))
        .collect();
    (s, certs, crl_hex, id8)
}

#[test]
fn a_pseudonym_certificate_validates_to_a_ctl_root_and_the_crl_revokes_it() {
    let run = revoked();
    let (s, certs, crl_hex, id8) = store(&run);
    let none = BTreeMap::new();
    let clean =
        pki_view::inspect_cert(&id8, &certs, &s.ctl.roots, &[], &none, s.time32).expect("known");
    for r in [
        "well-formed",
        "chain-to-root",
        "root-on-ctl",
        "signatures",
        "issuer-permissions",
        "chain-length",
        "validity",
        "region",
        "crl",
    ] {
        assert_eq!(rule(&clean, r), "pass", "{r}: {clean}");
    }
    assert_eq!(clean["valid"], true);
    assert_eq!(
        clean["chain"].as_array().map(Vec::len),
        Some(4),
        "pseudonym, ACA, ICA, root: {}",
        clean["chain"]
    );
    assert!(clean["tree"]["toBeSigned"].is_object(), "{}", clean["tree"]);
    // With the run's signed CRL, the revoked device's certificate fails the CRL rule.
    let revoked = pki_view::inspect_cert(&id8, &certs, &s.ctl.roots, &s.crls, &crl_hex, s.time32)
        .expect("known");
    assert_eq!(rule(&revoked, "crl"), "fail", "{revoked}");
    assert_eq!(revoked["valid"], false);
    // A root not on the CTL fails the anchor rule.
    let other = vec!["00".repeat(32)];
    let untrusted =
        pki_view::inspect_cert(&id8, &certs, &other, &[], &none, s.time32).expect("known");
    assert_eq!(rule(&untrusted, "root-on-ctl"), "fail", "{untrusted}");
    // A chain with its ICA missing does not reach a root.
    let ica = clean["chain"][2].as_str().expect("ica").to_string();
    let mut gap = certs.clone();
    gap.remove(&ica);
    let broken =
        pki_view::inspect_cert(&id8, &gap, &s.ctl.roots, &[], &none, s.time32).expect("known");
    assert_eq!(rule(&broken, "chain-to-root"), "fail", "{broken}");
    // Far past its validity it is expired.
    let late = pki_view::inspect_cert(
        &id8,
        &certs,
        &s.ctl.roots,
        &[],
        &none,
        s.time32 + 20 * 365 * 86_400,
    )
    .expect("known");
    assert_eq!(rule(&late, "validity"), "fail", "{late}");
}

#[test]
fn a_case_walks_the_board_in_order() {
    let run = revoked();
    let runs = std::slice::from_ref(&run);
    let mut tracker = PkiTracker::default();
    let t = now(&run);
    let vehicle = |n: u32, dt: u64| pki_view::Enforcer {
        node: v2xw_core::ids::NodeId::new(n),
        device: v2xw_core::ids::NodeId::new(100 + n),
        at: t + dt,
    };
    let one = vec![vehicle(1, 0)];
    let cases = pki_view::drain_cases(runs, &mut tracker, t, &|_, _| Some((one.clone(), 5)));
    let stages: Vec<&str> = cases.iter().map(|c| c.stage.as_str()).collect();
    assert_eq!(stages, pki_view::CASE_STAGES.to_vec(), "{stages:?}");
    assert!(
        cases.windows(2).all(|w| w[0].t <= w[1].t),
        "stages go forward in time: {cases:?}"
    );
    // The case names flow instances the kernel ran: its resolution and its CRL issuance.
    let flows = pki_view::drain_flows(runs, &mut PkiTracker::default());
    let crl = cases
        .iter()
        .find_map(|c| c.detail["crl_instance"].as_str())
        .expect("a CRL instance");
    for i in [cases[0].instance.as_str(), crl] {
        assert!(flows.iter().any(|f| f.instance == i), "no flow is {i}");
    }
    // The reports the case was opened on are named with their evidence; these two came
    // on the modelled path (no secured report), so their type is null.
    let evidence = cases[0].detail["evidence"].as_array().expect("evidence");
    assert_eq!(
        evidence.iter().map(|e| e["report"].as_u64()).collect::<Vec<_>>(),
        vec![Some(0), Some(1)]
    );
    assert!(evidence.iter().all(|e| e["type"].is_null()));
    let enforced = cases.last().expect("enforced");
    assert_eq!(enforced.detail["installed"], 1);
    assert_eq!(enforced.detail["of"], 5);
    assert_eq!(enforced.detail["device"], "ee/101");
    // Two more vehicles install the entry: each is stamped at its own instant, with the
    // running count, so "n of N" keeps counting (it froze at the first install before).
    let three = vec![vehicle(1, 0), vehicle(2, 7), vehicle(3, 9)];
    let more = pki_view::drain_cases(runs, &mut tracker, t + 10, &|_, _| Some((three.clone(), 5)));
    assert_eq!(
        more.iter()
            .map(|c| (c.stage.as_str(), c.t, c.detail["installed"].as_u64(), c.detail["node"].as_u64()))
            .collect::<Vec<_>>(),
        vec![("enforced", t + 7, Some(2), Some(2)), ("enforced", t + 9, Some(3), Some(3))]
    );
    assert!(
        pki_view::drain_cases(runs, &mut tracker, t + 20, &|_, _| Some((three.clone(), 5))).is_empty()
    );
}

#[test]
fn each_entity_knows_only_its_share() {
    let run = revoked();
    let t = now(&run);
    let lvs: Vec<String> = run.state.devices[&DEVICE_A]
        .credentials
        .values()
        .map(|c| v2xw_core::hash::hex_encode(c.lv.as_bytes()))
        .collect();
    let ra = pki_view::knowledge(&run, "ra", t, 64).expect("ra");
    let aca = pki_view::knowledge(&run, "aca", t, 64).expect("aca");
    let ra_holds = ra.holds.to_string();
    let aca_holds = aca.holds.to_string();
    for lv in &lvs {
        assert!(
            aca_holds.contains(lv),
            "the ACA knows the linkage value it certified"
        );
        assert!(!ra_holds.contains(lv), "the RA never sees a linkage value");
    }
    assert!(
        !aca_holds.contains("ee/"),
        "the ACA holds no device identity: {aca_holds}"
    );
    assert!(
        aca.truth.to_string().contains("ee/1000"),
        "ground truth joins the ACA's rows to the device: {}",
        aca.truth
    );
    let enrol = v2xw_core::hash::hex_encode(&v2xw_proto::scms::pdu::cert_id8(
        &run.state.devices[&DEVICE_A]
            .enrolment_credential
            .as_ref()
            .expect("enrolled")
            .cert,
    ));
    assert!(
        ra_holds.contains(&enrol),
        "the RA knows the enrolment identity"
    );
    assert!(!aca_holds.contains(&enrol), "the ACA does not");
    let view = pki_view::inspect_knowledge(&aca, "provider-a/aca/0");
    assert!(
        view.get("truth").is_none(),
        "the per-entity view omits ground truth"
    );
    assert!(pki_view::inspect_knowledge(&aca, "ground-truth")["truth"].is_object());
    for role in ["la1", "la2", "ma", "eca"] {
        let k = pki_view::knowledge(&run, role, t, 64).expect(role);
        assert!(
            !k.holds.to_string().contains("ee/"),
            "{role} holds no device id"
        );
    }
}

/// The `node.credentials` change digest is SHA-256 over its documented encoding, truncated
/// to 8 bytes, so it is the same on every toolchain. The pinned value was computed
/// independently from the encoding in the function's doc comment, in Python with
/// `hashlib` (not from this code). std's `DefaultHasher`, used before, gives another
/// value and may change between Rust releases.
#[test]
fn the_credentials_change_digest_is_a_stable_sha256() {
    use pki_view::{CredCell, credentials_digest};
    let cell = |i: u32, j: u32, state: &str, id8: Option<&str>, used: bool| CredCell {
        i,
        j,
        state: state.to_string(),
        id8: id8.map(str::to_string),
        used,
    };
    let cells = vec![
        cell(614, 0, "active", Some("0102030405060708"), true),
        cell(614, 1, "valid", None, false),
        cell(615, 0, "not-yet-available", None, false),
    ];
    let e = Some("a1b2c3d4e5f60708");
    let base = credentials_digest(&cells, 614, e, Some((true, false)), Some(3), false);
    assert_eq!(
        base, 0x6928_3c68_20a4_f4c5,
        "SHA-256 of the encoding, first 8 bytes"
    );
    // Every input the record shows moves the digest.
    let mut used = cells.clone();
    used[1].used = true;
    let others = [
        credentials_digest(&used, 614, e, Some((true, false)), Some(3), false),
        credentials_digest(&cells[..2], 614, e, Some((true, false)), Some(3), false),
        credentials_digest(&cells, 615, e, Some((true, false)), Some(3), false),
        credentials_digest(&cells, 614, None, Some((true, false)), Some(3), false),
        credentials_digest(&cells, 614, e, Some((true, true)), Some(3), false),
        credentials_digest(&cells, 614, e, None, Some(3), false),
        credentials_digest(&cells, 614, e, Some((true, false)), Some(4), false),
        credentials_digest(&cells, 614, e, Some((true, false)), None, false),
        credentials_digest(&cells, 614, e, Some((true, false)), Some(3), true),
    ];
    for (k, d) in others.iter().enumerate() {
        assert_ne!(*d, base, "variant {k} must change the digest");
    }
}
