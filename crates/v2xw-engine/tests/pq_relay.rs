//! R-PQMBR (docs/design/scms-real/testing/stress.md §3): under a post-quantum signature the
//! engine's own application frames, a report a relay-only vehicle puts on the air to a
//! roadside unit and a unit's CRL broadcast, are signed SDUs larger than the 1,400-octet
//! WSMP MTU. Before the fix every one was refused at the MTU and still counted as uploaded
//! or broadcast (`reports_uploaded_relay` 391 = `net_mtu_refusals` 391 on the matrix's
//! `pq-hybrid`; 14,280 CRL broadcasts counted with none on the air in this scenario).
//!
//! `scenarios/scms-pq-relay-repro.yaml` is that hybrid Falcon-512 run, 120 s live, with a
//! revocation inside the window. Two checks on it:
//!
//! * with the scenario's `fragmenter/generic-sdu`, such a frame goes out in pieces like any
//!   SDU, and the unit or vehicle acts on it once it is reassembled: relayed reports reach
//!   the proxy and CRL frames are reassembled, and nothing is refused at the MTU;
//! * with a fragmenter that does not carry whole SDUs in pieces, every such frame is refused
//!   and none is counted as an upload or a broadcast. The check uses the NDSS 2024
//!   certificate cycle (`fragmenter/cert-cycle-partial-hybrid`): it splits only the hybrid
//!   certificate, so BSMs still fit and the detectors still detect, while a report or CRL
//!   frame (1,119 or ~1,330 octets plus the 666-octet Falcon signature) cannot fit. With
//!   `fragmenter/none` the hybrid BSM with its certificate is itself refused, nothing is
//!   detected, and no report or revocation exists to test.

use std::path::Path;

use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenario() -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/scms-pq-relay-repro.yaml")).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    s
}

/// Complete reassemblies on `net.reassembly`, by message type.
fn reassembled(rec: &MemoryRecorder, msg_type: &str) -> u64 {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == "net.reassembly")
        .filter_map(|(_, r)| serde_json::from_slice::<serde_json::Value>(&r.json).ok())
        .filter(|v| v["msg_type"] == msg_type && v["outcome"] == "complete")
        .count() as u64
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    (report, rec)
}

fn relayed_at_proxy(r: &RunReport) -> u64 {
    let p = &r.phase2;
    let relay = p
        .access
        .exchange_latency
        .get("report-relay")
        .map_or(0, |e| e.count);
    // The units' own reports ride the same backhaul leg; the rest were a vehicle's.
    relay.saturating_sub(p.reports_from_rsus)
}

#[test]
fn an_oversized_relayed_report_or_crl_frame_is_fragmented_and_acted_on_whole() {
    let (r, rec) = run(scenario());
    let p = &r.phase2;
    let (mbr, crl) = (reassembled(&rec, "mbr"), reassembled(&rec, "crl"));
    println!(
        "relay uploads {}, refused at the MTU {} (CRL frames {}), at the proxy via a unit {}, \
         CRL broadcasts {}, reassembled mbr {mbr} crl {crl}, net_mtu_refusals {}",
        p.reports_uploaded_relay,
        p.reports_refused_mtu,
        p.crl_frames_refused_mtu,
        relayed_at_proxy(&r),
        p.crl_broadcasts,
        r.net_mtu_refusals
    );
    assert!(p.reports_uploaded_relay > 0, "no report was relayed");
    assert_eq!(
        p.reports_refused_mtu, 0,
        "a relayed report was refused at the MTU"
    );
    assert_eq!(
        p.crl_frames_refused_mtu, 0,
        "a CRL frame was refused at the MTU"
    );
    assert!(mbr > 0, "no relayed report was reassembled at a unit");
    assert!(
        relayed_at_proxy(&r) > 0,
        "no vehicle report reached the proxy through a unit"
    );
    assert!(p.crl_broadcasts > 0, "no CRL frame was broadcast");
    assert!(crl > 0, "no CRL frame was reassembled at a vehicle");
}

#[test]
fn without_an_sdu_fragmenter_an_oversized_frame_is_refused_and_not_counted_as_sent() {
    let mut s = scenario();
    s.net.fragmenter = Some(ModelChoice {
        id: v2xw_net::FRAGMENTER_CERT_CYCLE_ID.to_string(),
        params: serde_json::Value::Null,
    });
    let (r, _) = run(s);
    let p = &r.phase2;
    println!(
        "relay uploads {}, refused {}, CRL broadcasts {}, CRL frames refused {}, \
         net_mtu_refusals {}",
        p.reports_uploaded_relay,
        p.reports_refused_mtu,
        p.crl_broadcasts,
        p.crl_frames_refused_mtu,
        r.net_mtu_refusals
    );
    assert!(p.reports_refused_mtu > 0, "no relayed report met the MTU");
    assert!(p.crl_frames_refused_mtu > 0, "no CRL frame met the MTU");
    assert_eq!(
        p.reports_uploaded_relay, 0,
        "a report refused at the MTU was counted as uploaded"
    );
    assert_eq!(
        p.crl_broadcasts, 0,
        "a CRL frame refused at the MTU was counted as broadcast"
    );
    assert_eq!(
        relayed_at_proxy(&r),
        0,
        "a refused report reached the proxy"
    );
    assert!(
        r.net_mtu_refusals >= p.reports_refused_mtu + p.crl_frames_refused_mtu,
        "the per-message refusals are a part of the network layer's"
    );
}
