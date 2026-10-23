//! WP-B4: the CRL Generator issues real 1609.2 `SecuredCrl`s (gap row 29), the device
//! verifies them itself (row 37), and publication follows the provider's business-day
//! calendar ([SM-PROV] §3.4.9).
//!
//! * with `V2XW_PDU_OUT=<dir>` the first test writes the CRLs it issued for
//!   `tools/pdu_check/check_run_crl.py`.

mod common;

use common::{DEVICE_A, DEVICE_B, deployment, provisioned};
use v2xw_core::ids::NodeId;
use v2xw_core::time::NS_PER_S;
use v2xw_msg::scms_types::WireEdition;
use v2xw_proto::scms::calendar;
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::SpduKind;
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::StageId;
use v2xw_sec::crl;

const JMAX: u32 = 2;

fn revoke_a(run: &mut ScmsRun) -> (v2xw_proto::stage::FlowRun, v2xw_proto::stage::FlowRun) {
    let lv = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    run.submit_report(DEVICE_B, 0, lv);
    run.run().expect("the report runs");
    let runs = run.revoke(0, 0, JMAX).expect("a report is held");
    run.run().expect("the revocation runs");
    runs
}

fn implicit(run: &ScmsRun, dev: NodeId, j: u32) -> v2xw_msg::sec_types::ieee1609_dot2::Certificate {
    run.state.devices[&dev].credentials[&(0, j)]
        .implicit
        .as_ref()
        .expect("real implicit certificate")
        .certificate
        .as_ref()
        .clone()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn a_revocation_publishes_a_real_secured_crl_that_revokes_exactly_the_device() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    provisioned(&mut run, DEVICE_B, 0, 1, JMAX);
    revoke_a(&mut run);
    let st = &run.state;
    let pki = st.pki.clone().expect("layer on");
    let bytes = st
        .crl_store
        .secured
        .clone()
        .expect("the store holds a SecuredCrl");
    let opened = crl::decode_crl(bytes.bytes()).expect("decodes");
    let v2xw_sec::sig1609::SpduSigner::Certificate(signer) = opened.signed.signer.clone() else {
        panic!("the CRL carries its signer's certificate");
    };
    assert_eq!(signer, pki.crlg.cert, "signed by the provider's CRL signer");
    let verdict = crl::verify_crl(&opened, &signer, Some(&pki.ica.cert), WireEdition::Ieee2022);
    assert!(verdict.accepted(), "{verdict:?}");
    assert_eq!(opened.contents.crl_series.0.0, crl::SERIES_PSEUDONYM);
    assert_eq!(
        opened.contents.crl_craca.0[..],
        pki.ica.id8()[..],
        "crlCraca = the ICA"
    );
    // nextCrl is a week after issueDate.
    assert_eq!(
        opened.contents.next_crl.0.0 - opened.contents.issue_date.0.0,
        7 * 86_400
    );
    // Every one of A's certificates is revoked by the signed bytes; none of B's.
    for j in 0..JMAX {
        assert!(
            crl::revokes(&opened.contents, &implicit(&run, DEVICE_A, j)),
            "A (0,{j})"
        );
        assert!(
            !crl::revokes(&opened.contents, &implicit(&run, DEVICE_B, j)),
            "B (0,{j})"
        );
    }
    // The publication and the device download are charged the CRL's encoded length.
    let len = bytes.len();
    assert!(
        st.pdus
            .iter()
            .any(|p| p.kind == SpduKind::SecuredCrl && p.len == len)
    );
    // A delta exists only after a first full CRL; the issuance log has the full one.
    assert_eq!(st.crlg.issued.len(), 1);
    assert!(!st.crlg.issued[0].delta);

    run.distribute_crl(DEVICE_A);
    run.distribute_crl(DEVICE_B);
    run.run().expect("runs");
    let steps: Vec<_> = run
        .kernel
        .steps
        .iter()
        .filter(|s| s.step == "crl-download")
        .collect();
    assert_eq!(steps.len(), 2);
    assert!(steps.iter().all(|s| s.bytes == len));
    let st = &run.state;
    assert!(st.devices[&DEVICE_A].silenced, "A finds itself revoked");
    assert!(!st.devices[&DEVICE_B].silenced);
    assert_eq!(st.devices[&DEVICE_A].crls_rejected, 0);
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);

    if let Ok(dir) = std::env::var("V2XW_PDU_OUT") {
        let dir = std::path::Path::new(&dir).join("crl");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("00_full.oer"), bytes.bytes()).unwrap();
        let m = serde_json::json!({
            "crls": [{"file": "00_full.oer", "series": 1, "delta": false}],
            "ica": hex(&v2xw_sec::cert::encode(&pki.ica.cert).unwrap()),
            "root": hex(&v2xw_sec::cert::encode(&pki.root.cert).unwrap()),
            "revoked": (0..JMAX).map(|j| hex(&v2xw_sec::cert::encode(&implicit(&run, DEVICE_A, j)).unwrap())).collect::<Vec<_>>(),
            "not_revoked": (0..JMAX).map(|j| hex(&v2xw_sec::cert::encode(&implicit(&run, DEVICE_B, j)).unwrap())).collect::<Vec<_>>(),
            "aca": hex(&v2xw_sec::cert::encode(&pki.aca.cert).unwrap()),
        });
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_string_pretty(&m).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn a_second_revocation_issues_a_full_and_a_delta_crl() {
    let mut run = deployment(3);
    // Deltas are off under the US profile (O-1); selected here for their encoding.
    run.state.params.crl.deltas = true;
    for d in [DEVICE_A, DEVICE_B] {
        provisioned(&mut run, d, 0, 1, JMAX);
    }
    revoke_a(&mut run);
    let lv = run.state.devices[&DEVICE_B].credentials[&(0, 0)].lv;
    run.submit_report(DEVICE_A, 0, lv);
    run.run().expect("runs");
    run.revoke(1, 0, JMAX).expect("held");
    run.run().expect("runs");
    let log = &run.state.crlg.issued;
    let kinds: Vec<(bool, u32)> = log.iter().map(|c| (c.delta, c.entries)).collect();
    assert_eq!(kinds, vec![(false, 1), (false, 2), (true, 1)]);
    let delta = crl::decode_crl(log[2].pdu.bytes()).expect("decodes");
    assert!(matches!(
        delta.contents.type_specific,
        v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents::deltaLinkedCrl(
            _
        )
    ));
    // The second full CRL at the same iRev counts up indexWithinI.
    let full2 = crl::decode_crl(log[1].pdu.bytes()).unwrap();
    let v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents::fullLinkedCrl(
        t,
    ) = &full2.contents.type_specific
    else {
        panic!("full linkage CRL");
    };
    assert_eq!(t.index_within_i.0, 1);
    // The delta is a CRL of its own: indexWithinI "increments by 1 every time a new full
    // or delta CRL is issued" (Ieee1609Dot2CrlBaseTypes), so it takes the next value.
    let v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents::deltaLinkedCrl(
        d,
    ) = &delta.contents.type_specific
    else {
        panic!("delta linkage CRL");
    };
    assert_eq!(d.i_rev.0.0, t.i_rev.0.0);
    assert_eq!(d.index_within_i.0, 2, "full 1, then its delta 2");
    assert!(crl::revokes(&delta.contents, &implicit(&run, DEVICE_B, 0)));
    assert!(!crl::revokes(&delta.contents, &implicit(&run, DEVICE_A, 0)));
}

/// O-1 (oracle-final.md): the Generator signed a delta with every full CRL from the
/// second on, and nothing ever sent it, so a device saw `indexWithinI` jump 1 -> 3.
/// Under the US profile (full lists only, `CrlIssuance::deltas`) every series-1 CRL it
/// signs is a full one, the last is what both distribution paths hold, and the index
/// counts up with no gap.
#[test]
fn the_us_profile_signs_only_the_full_crls_it_distributes() {
    use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents as T;
    let mut run = deployment(3);
    assert!(!run.state.params.crl.deltas, "full lists only by default");
    for d in [DEVICE_A, DEVICE_B] {
        provisioned(&mut run, d, 0, 1, JMAX);
    }
    revoke_a(&mut run);
    let lv = run.state.devices[&DEVICE_B].credentials[&(0, 0)].lv;
    run.submit_report(DEVICE_A, 0, lv);
    run.run().expect("runs");
    run.revoke(1, 0, JMAX).expect("held");
    run.run().expect("runs");
    let st = &run.state;
    let series1: Vec<_> = st.crlg.issued.iter().filter(|c| c.series == 1).collect();
    assert_eq!(
        series1.iter().map(|c| c.delta).collect::<Vec<_>>(),
        vec![false, false],
        "two publications, two full CRLs and no delta"
    );
    assert!(st.crlg.delta.is_none());
    let index: Vec<u8> = series1
        .iter()
        .map(|c| {
            match crl::decode_crl(c.pdu.bytes())
                .unwrap()
                .contents
                .type_specific
            {
                T::fullLinkedCrl(t) => t.index_within_i.0,
                other => panic!("a full linkage CRL, not {other:?}"),
            }
        })
        .collect();
    assert_eq!(index, vec![0, 1], "no gap a device could see");
    let last = series1.last().unwrap().pdu.bytes();
    for (path, store) in [("store", &st.crl_store), ("roadside", &st.crl_broadcast)] {
        assert_eq!(
            store.secured.as_ref().map(|p| p.bytes()),
            Some(last),
            "the {path} path distributes the last CRL signed"
        );
    }
}

#[test]
fn a_secured_crl_altered_in_transit_is_refused_by_the_device() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    revoke_a(&mut run);
    // The last byte of the signed CrlContents (inside the revoked device's second
    // linkage seed) is changed between the Generator and the Store.
    let p = run.state.crl_store.secured.clone().unwrap();
    let mut b = p.bytes().to_vec();
    let opened = crl::decode_crl(&b).unwrap();
    let payload = opened.signed.payload.clone();
    let at = b
        .windows(payload.len())
        .position(|w| w == payload.as_slice())
        .expect("the CrlContents bytes are in the SPDU");
    b[at + payload.len() - 1] ^= 0x01;
    run.state.crl_store.secured = Some(v2xw_proto::scms::pdu::Pdu::new(b));
    run.distribute_crl(DEVICE_A);
    run.run().expect("runs");
    let st = &run.state;
    let d = &st.devices[&DEVICE_A];
    assert_eq!(d.crls_rejected, 1);
    assert!(!d.silenced, "a CRL that does not verify revokes nothing");
    assert!(
        st.pdu_rejections
            .iter()
            .any(|r| r.kind == SpduKind::SecuredCrl)
    );
}

/// The publication instant (Unix s) of a revocation decided at Eastern local `start`,
/// with the business-day SLA on.
fn published_unix(start: (i32, u32, u32, u32, u32)) -> (i64, i64) {
    let (y, m, d, hh, mm) = start;
    let t0 = calendar::eastern_to_unix(y, m, d, hh, mm);
    let mut p = ScmsParams::default().quick();
    p.pdu.t0_unix_s = t0;
    p.crl.business_day_sla = true;
    let mut run = ScmsRun::new(p).expect("encodes");
    run.add_device(DEVICE_A);
    run.add_device(DEVICE_B);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    let (_, issuance) = revoke_a(&mut run);
    let issued = run
        .kernel
        .stages
        .at(issuance, StageId::Issued)
        .expect("issued");
    let published = run
        .kernel
        .stages
        .at(issuance, StageId::Published)
        .expect("published");
    let to_unix = |t: u64| t0 + i64::try_from(t / NS_PER_S).unwrap();
    // The decision was taken the same Eastern day it started (minutes of protocol).
    assert_eq!(
        calendar::eastern_date(to_unix(issued)),
        (y, m, d),
        "decided on the start day"
    );
    (to_unix(issued), to_unix(published))
}

#[test]
fn a_friday_evening_decision_publishes_on_monday() {
    let (decided, published) = published_unix((2026, 10, 16, 19, 0));
    let slot = calendar::eastern_to_unix(2026, 10, 19, 10, 0);
    assert!(
        (slot..slot + 5).contains(&published),
        "published {published}, slot {slot}"
    );
    assert!(published <= calendar::publication_deadline(decided));
}

#[test]
fn a_federal_holiday_decision_publishes_on_the_next_business_day() {
    // Thanksgiving, Thursday 2026-11-26, decided at 07:00: a working day would publish
    // at 10:00 the same morning; the holiday moves it to Friday.
    let (decided, published) = published_unix((2026, 11, 26, 7, 0));
    let slot = calendar::eastern_to_unix(2026, 11, 27, 10, 0);
    assert!(
        (slot..slot + 5).contains(&published),
        "published {published}, slot {slot}"
    );
    assert!(published <= calendar::publication_deadline(decided));
}

#[test]
fn a_tuesday_morning_decision_publishes_the_same_morning() {
    let (_, published) = published_unix((2026, 10, 20, 8, 0));
    let slot = calendar::eastern_to_unix(2026, 10, 20, 10, 0);
    assert!((slot..slot + 5).contains(&published));
}

#[test]
fn a_later_crl_walks_earlier_seeds_forward_to_its_i_rev() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 2, JMAX);
    provisioned(&mut run, DEVICE_B, 0, 2, JMAX);
    revoke_a(&mut run);
    // B is reported in period 1 (the report reaches the proxy a week and an hour in).
    let p1 = run.state.params.period_start(1) + 3_600 * NS_PER_S;
    let lv = run.state.devices[&DEVICE_B].credentials[&(1, 0)].lv;
    run.report_at_proxy(
        DEVICE_A,
        1,
        lv,
        p1,
        p1,
        p1,
        v2xw_proto::net::Transport::CellularUu,
        1_000,
    );
    run.run().expect("runs");
    run.revoke(1, 1, JMAX).expect("held");
    run.run().expect("runs");
    let last_full = run
        .state
        .crlg
        .issued
        .iter()
        .rev()
        .find(|c| !c.delta)
        .unwrap()
        .pdu
        .clone();
    let c = crl::decode_crl(last_full.bytes()).unwrap().contents;
    let v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents::fullLinkedCrl(
        t,
    ) = &c.type_specific
    else {
        panic!("full linkage CRL");
    };
    assert_eq!(t.i_rev.0.0, 1, "iRev is the current period");
    assert_eq!(t.index_within_i.0, 0, "the first CRL at iRev 1");
    let cert = |d, i, j| {
        run.state.devices[&d].credentials[&(i, j)]
            .implicit
            .as_ref()
            .unwrap()
            .certificate
            .clone()
    };
    for j in 0..JMAX {
        // A's entry, released at i = 0, was walked one step: A's period-1 certificates
        // are revoked, its period-0 ones are no longer listed (forward-only).
        assert!(crl::revokes(&c, &cert(DEVICE_A, 1, j)), "A (1,{j})");
        assert!(!crl::revokes(&c, &cert(DEVICE_A, 0, j)), "A (0,{j})");
        assert!(crl::revokes(&c, &cert(DEVICE_B, 1, j)), "B (1,{j})");
    }
}
