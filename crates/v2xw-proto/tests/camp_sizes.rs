//! WP-F2 real-world cross-check: the engine's certificate and SPDU sizes against the
//! example sizes of CAMP Release 1.2.2 Table 3 ("Example Size in Bytes (Certs are Not
//! Fixed Size)", [CAMP122] pp. 34-36, 03 §12) and against the certificates SCMS Manager
//! publishes (the production and test chain files, 01 §2.2).
//!
//! One run of the default profile enrols an OBU and an RSU, provisions one week of 20
//! pseudonyms, and issues one OBU identification and one RSU application certificate.
//! Every certificate is encoded with the real COER encoder and its length pinned below.
//! The field-by-field reconciliation with CAMP is in
//! `docs/design/scms-real/04-validation-report.md` §3; the independent decode of the same
//! bytes is `tools/realworld/sizes.py` (the A0 oracle's codec).
//!
//! * with `V2XW_SIZES_OUT=<file>` the test writes every certificate and SPDU as hex JSON,
//!   the input of `tools/realworld/sizes.py`.

mod common;

use std::collections::BTreeMap;

use common::{DEVICE_A, DEVICE_B};
use v2xw_proto::scms::appcert::AppCertKind;
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::run::ScmsRun;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

struct Corpus {
    /// Certificate role -> COER bytes (one representative each).
    certs: BTreeMap<String, Vec<u8>>,
    /// Every pseudonym certificate's length (one week, j = 0..19).
    pseudonym_lens: Vec<usize>,
    /// SPDU kind -> lengths seen.
    spdus: BTreeMap<String, Vec<usize>>,
    spdu_hex: BTreeMap<String, String>,
}

fn corpus() -> Corpus {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    let mut run = ScmsRun::new(p).expect("certificates encode");
    run.add_device(DEVICE_A);
    run.add_rsu(DEVICE_B);
    run.enrol(DEVICE_A);
    run.enrol(DEVICE_B);
    run.run().expect("enrolment runs");
    let i = 0;
    run.provision(DEVICE_A, i, 1, 20);
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_A, AppCertKind::ObuIdentification, t0);
    run.start_app_certs(DEVICE_B, AppCertKind::RsuApplication, t0);
    let horizon = run.state.params.period_start(2);
    run.run_until(horizon).expect("runs");
    assert!(
        run.state.pdu_rejections.is_empty(),
        "{:?}",
        run.state.pdu_rejections
    );

    let enc = |c: &v2xw_msg::sec_types::ieee1609_dot2::Certificate| {
        v2xw_sec::cert::encode(c).expect("encodes")
    };
    let mut certs = BTreeMap::new();
    let st = &run.state;
    for (role, node) in [("obe-enrolment", DEVICE_A), ("rse-enrolment", DEVICE_B)] {
        let e = st.devices[&node]
            .enrolment_credential
            .as_ref()
            .expect("enrolled");
        certs.insert(role.to_string(), enc(&e.cert));
    }
    let dev = &st.devices[&DEVICE_A];
    let mut pseudonym_lens = Vec::new();
    for c in dev.credentials.values() {
        let ic = c.implicit.as_ref().expect("implicit pseudonym");
        let b = enc(&ic.certificate);
        pseudonym_lens.push(b.len());
        certs.entry("obe-pseudonym".to_string()).or_insert(b);
    }
    for (role, node) in [
        ("obe-identification", DEVICE_A),
        ("rse-application", DEVICE_B),
    ] {
        let a = st.apps.devices[&node]
            .installed
            .first()
            .expect("an application certificate installed");
        certs.insert(role.to_string(), enc(&a.cred.cert));
    }
    let pki = st.pki.as_ref().expect("the PDU PKI");
    for a in pki.all() {
        certs.insert(format!("pdu-pki/{}", a.name), enc(&a.cert));
    }
    for role in [
        "root", "ica", "eca", "ra", "pca", "ma", "la1", "la2", "crlg", "pg", "dcm",
    ] {
        if let Some(c) = run.state.gov.cert(role) {
            certs.insert(format!("governance/{role}"), enc(c));
        }
    }
    let mut spdus: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut spdu_hex = BTreeMap::new();
    for p in &st.pdus {
        if let Some(b) = p.bytes.as_ref() {
            spdus
                .entry(p.kind.name().to_string())
                .or_default()
                .push(b.bytes().len());
            spdu_hex
                .entry(p.kind.name().to_string())
                .or_insert_with(|| hex(b.bytes()));
        }
    }
    Corpus {
        certs,
        pseudonym_lens,
        spdus,
        spdu_hex,
    }
}

#[test]
fn certificate_sizes_against_camp_table_3() {
    let c = corpus();
    if let Ok(out) = std::env::var("V2XW_SIZES_OUT") {
        let certs: BTreeMap<_, _> = c.certs.iter().map(|(k, v)| (k, hex(v))).collect();
        let j = serde_json::json!({
            "certs": certs,
            "pseudonym_lens": c.pseudonym_lens,
            "spdus": c.spdus,
            "spdu_hex": c.spdu_hex,
        });
        std::fs::write(out, serde_json::to_string_pretty(&j).unwrap()).unwrap();
    }
    let sizes: BTreeMap<&str, usize> = c.certs.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
    eprintln!("certificate sizes: {sizes:?}");
    eprintln!(
        "spdu sizes: {:?}",
        c.spdus
            .iter()
            .map(|(k, v)| (k, v.iter().min(), v.iter().max(), v.len()))
            .collect::<Vec<_>>()
    );

    // (role, engine bytes, CAMP 1.2.2 Table 3 example bytes). The engine values are pinned
    // as of WP-F2; a change to a certificate profile must update this table and the
    // reconciliation in 04-validation-report.md §3 together.
    const PINNED: &[(&str, usize, usize)] = &[
        // End entities. CAMP's examples are "not fixed size"; the differences are the
        // fields: the pseudonym lacks a region (with countryOnly 840 it is exactly 86 B,
        // tools/realworld/sizes.py); the enrolment name is 22 characters and carries the
        // three-country region; identification and application carry no region or SSP.
        ("obe-pseudonym", 80, 86),
        ("obe-enrolment", 99, 87),
        ("rse-enrolment", 99, 87),
        ("obe-identification", 70, 89),
        ("rse-application", 65, 89),
        // The components the devices see (the PDU PKI). Each encodes like the published
        // certificate of the same role field for field except the name
        // (tools/realworld/sizes.py `same_encoding_but_name_as`).
        ("pdu-pki/root.governance", 206, 211),
        ("pdu-pki/ica.governance", 179, 195),
        ("pdu-pki/eca.provider-a", 171, 150),
        ("pdu-pki/ra.provider-a", 192, 217),
        ("pdu-pki/aca.provider-a", 156, 216),
        ("pdu-pki/la1.provider-a", 154, 205),
        ("pdu-pki/la2.provider-a", 154, 205),
        ("pdu-pki/ma.provider-a", 190, 205),
        ("pdu-pki/crlg.provider-a", 172, 190),
        ("pdu-pki/dcm.provider-a", 152, 219),
        // The governance hierarchy (WP-B5) names its components `*.sim`: 4 B more.
        ("governance/root", 206, 211),
        ("governance/ica", 179, 195),
        ("governance/eca", 175, 150),
        ("governance/ra", 196, 217),
        ("governance/pca", 160, 216),
        ("governance/la1", 158, 205),
        ("governance/la2", 158, 205),
        ("governance/ma", 194, 205),
        ("governance/crlg", 176, 190),
        ("governance/pg", 155, 172),
        ("governance/dcm", 156, 219),
    ];
    for &(role, engine, camp) in PINNED {
        assert_eq!(
            sizes.get(role).copied(),
            Some(engine),
            "{role}: CAMP Table 3 example {camp} B"
        );
    }
    assert_eq!(
        sizes.len(),
        PINNED.len(),
        "every certificate is in the table"
    );
    // Every pseudonym of the week is the same size (only linkage value and key differ).
    assert_eq!(c.pseudonym_lens.len(), 20);
    assert!(c.pseudonym_lens.iter().all(|&l| l == 80));
    // CAMP leaves the pseudonym response size UNCONFIRMED (03 §12): the engine's real
    // AcaEeCertResponsePrivateSpdu is 454-470 B per certificate.
    let r = &c.spdus["AcaEeCertResponsePrivateSpdu"];
    assert_eq!(
        r.len(),
        24,
        "20 pseudonyms and 4 identification or application certificates"
    );
    assert!(r.iter().all(|&l| (454..=470).contains(&l)), "{r:?}");
}

/// CAMP's design sizing for 17 M new vehicles a year ([CAMP122] §6.3.3.4, §6.3.4.4,
/// §6.3.6.4; 03 §12) assumes, per vehicle: one provisioning request in its life, 156
/// batch files (3 years of weeks) at first download, then one `.info` poll and one new
/// file a week. This test measures those per-device counts in the engine (a THEA-style
/// 3-year preload and a weekly top-off on the default profile), then scales them to the
/// fleet and compares each rate with CAMP's figure.
#[test]
fn camp_sizing_rates_follow_from_the_engines_per_device_counts() {
    use v2xw_proto::scms::pdu::SpduKind;
    const WEEKS: u32 = 4;
    // One run per device, so every SPDU the RA receives is that device's.
    let one = |preload: u32, jmax: u32, weeks: u32| {
        let mut p = ScmsParams::default().quick();
        p.pdu.keep_bytes = true;
        let mut run = ScmsRun::new(p).expect("certificates encode");
        run.add_device(DEVICE_A);
        run.enrol(DEVICE_A);
        run.run().expect("enrolment runs");
        run.provision(DEVICE_A, 614, preload, jmax);
        run.run().expect("runs");
        for w in 1..=weeks {
            run.topup(DEVICE_A, 614 + preload - 1 + w, jmax);
            run.run().expect("runs");
        }
        let ra = run.state.nodes.ra.index();
        let to_ra = |k: SpduKind| {
            run.state
                .pdus
                .iter()
                .filter(|p| p.kind == k && p.to == ra)
                .count() as f64
        };
        (
            to_ra(SpduKind::EeRaCertRequest),
            to_ra(SpduKind::EeRaDownloadRequest),
            f64::from(run.state.devices[&DEVICE_A].info_requests),
        )
    };
    // A: THEA, 3 years at once (jmax 1 keeps it fast; the file count is the point).
    let (requests, first_files, _) = one(156, 1, 0);
    // B: the default 20 a week, topped off weekly.
    let (b_requests, b_downloads, b_info) = one(1, 20, WEEKS);
    assert_eq!(b_requests, 1.0);
    let b_files = b_downloads - b_info; // a Download-Req is either `.info` or a batch file
    eprintln!(
        "per device: {requests} request, {first_files} first files; per week {} .info, {} files",
        b_info / f64::from(WEEKS),
        (b_files - 1.0) / f64::from(WEEKS)
    );
    assert_eq!(requests, 1.0, "ra-maxReqs = 1");
    assert_eq!(first_files, 156.0, "3 years of weekly batch files");
    assert_eq!(b_info, f64::from(WEEKS), "one .info a week");
    assert_eq!(
        b_files,
        1.0 + f64::from(WEEKS),
        "the first week, then one file a week"
    );

    // Scale: 17 M new vehicles a year; the polling fleet is 17 M x (year - 1) plus half
    // the year's new cars (CAMP's "half of new cars plus all existing cars").
    const NEW: f64 = 17.0e6;
    const YEAR_S: f64 = 31_557_600.0;
    const WEEK_S: f64 = 604_800.0;
    let rel = |a: f64, b: f64| (a - b).abs() / b;
    let provisioning = NEW * requests / YEAR_S;
    assert!(
        rel(provisioning, 0.5) < 0.1,
        "{provisioning} req/s vs CAMP 0.5"
    );
    for (year, camp) in [(1.0, 14.0), (3.0, 70.0), (5.0, 126.0), (10.0, 267.0)] {
        let fleet = NEW * (year - 1.0) + NEW / 2.0;
        let polls = fleet * (b_info / f64::from(WEEKS)) / WEEK_S;
        eprintln!("year {year}: {polls:.1} .info polls/s (CAMP {camp})");
        assert!(
            rel(polls, camp) < 0.05,
            "year {year}: {polls} polls/s vs CAMP {camp}"
        );
    }
    let files_y1 = NEW * first_files / YEAR_S;
    let weekly = (b_files - 1.0) / f64::from(WEEKS);
    let files_y10 = files_y1 + NEW * 9.0 * weekly / WEEK_S;
    eprintln!("batch files/s: year 1 {files_y1:.1} (CAMP 85), year 10 {files_y10:.1} (CAMP ~337)");
    assert!(rel(files_y1, 85.0) < 0.05, "{files_y1} files/s vs CAMP 85");
    assert!(
        rel(files_y10, 337.0) < 0.05,
        "{files_y10} files/s vs CAMP ~337"
    );
}
