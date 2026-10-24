//! The VeReMi exporter against a hand-made fixture (adoption plan WP-03; f2md.md §5 P2,
//! P3): both codebooks, both layouts, byte for byte and field for field.
//!
//! The fixture under `tests/fixtures/veremi/` is written by `make_fixture.py` from the
//! published VeReMi and VeReMi Extension schemas with Python's own JSON encoder, not
//! copied from either dataset. Its attack codes are literals there and table lookups
//! here, so swapping two codes in a codebook table fails `the_writer_reproduces_…`
//! (checked: ConstPos ↔ ConstPosOffset swapped in the Extension table turned it red).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use v2xw_record::dataset::fixture::{DatasetShape, records};
use v2xw_record::dataset::veremi::{
    self, Codebook, OwnPosition, VEREMI_COMPATIBILITY_NOTE, VeremiExport, VeremiOptions,
    VeremiReception, VeremiTruth, pseudonym_int,
};
use v2xw_record::fixture::scratch_dir;
use v2xw_record::reader::RecordedRecord;

const Z: [f64; 3] = [0.0; 3];
const PA: &str = "1a2b3c4d5e6f7081";
const PB: &str = "99aa88bb77cc66dd";

fn fixture_dir(codebook: Codebook) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/veremi")
        .join(codebook.id())
}

#[allow(clippy::too_many_arguments)]
fn rx(
    receiver: u64,
    rcv: f64,
    send: f64,
    pseudo: &str,
    msg: u64,
    rssi: f64,
    pos: [f64; 3],
    pos_noise: [f64; 3],
    spd: [f64; 3],
    acl: [f64; 3],
    hed: [f64; 3],
) -> VeremiReception {
    VeremiReception {
        receiver,
        rcv_time: rcv,
        send_time: send,
        sender: pseudonym_int(pseudo),
        sender_pseudo: pseudo.to_string(),
        message_id: msg,
        pos,
        pos_noise,
        spd,
        acl,
        hed,
        rssi,
        ..VeremiReception::default()
    }
}

fn own(t: f64, pos: [f64; 3], spd: [f64; 3], acl: [f64; 3], hed: [f64; 3]) -> OwnPosition {
    OwnPosition {
        t,
        pos,
        spd,
        acl,
        hed,
        ..OwnPosition::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn truth(
    msg: u64,
    send: f64,
    sender: i64,
    pseudo: &str,
    code: i64,
    pos: [f64; 3],
    spd: [f64; 3],
    hed: [f64; 3],
) -> VeremiTruth {
    VeremiTruth {
        send_time: send,
        sender,
        sender_pseudo: pseudo.to_string(),
        message_id: msg,
        pos,
        spd,
        hed,
        attacker_type: code,
        ..VeremiTruth::default()
    }
}

/// The export the fixture encodes, with every attack code taken from the codebook.
fn fixture_export(codebook: Codebook) -> VeremiExport {
    let c = |f: &str| codebook.code(f);
    let mut e = VeremiExport {
        codebook,
        slice_s: 3600.0,
        ..VeremiExport::default()
    };
    e.by_receiver.insert(
        7,
        vec![
            rx(7, 3599.9, 3599.898, PA, 101, -71.25, [50.25, -12.5, 0.0], [1.25, 2.5, 0.0],
               [0.0, 13.5, 0.0], [0.0, 0.75, 0.0], [0.0, 1.0, 0.0]),
            rx(7, 3600.2, 3600.198, PB, 102, -88.5, [-3.125, 40.0, 1.5], [0.6, 0.6, 0.0],
               [-8.66, 5.0, 0.0], Z, [-0.866_025, 0.5, 0.0]),
        ],
    );
    e.by_receiver.insert(
        9,
        vec![rx(9, 12.5, 12.499, PA, 7, -60.0, [120.5, 3.0, 0.0], [3.0, 1.5, 0.0],
                [12.0, 0.0, 0.0], Z, [1.0, 0.0, 0.0])],
    );
    e.own_positions.insert(
        7,
        vec![
            own(3599.5, [10.0, 20.0, 0.0], [5.0, 0.0, 0.0], [0.5, 0.0, 0.0], [1.0, 0.0, 0.0]),
            own(3600.5, [15.0, 20.0, 0.0], [5.0, 0.0, 0.0], [0.5, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ],
    );
    e.own_positions
        .insert(9, vec![own(12.0, [100.0, 0.0, 0.0], Z, Z, [0.0, -1.0, 0.0])]);
    // The Extension layout carries a receiver's own code in its file names; VeReMi 1.0's
    // does not, so a 1.0 round trip cannot hold it.
    if codebook == Codebook::VeremiExt {
        e.receiver_attacker_type.insert(9, c("EventualStop"));
    }
    e.truth = vec![
        truth(7, 12.499, 3, PA, c("ConstPos"), [120.0, 3.0, 0.0], [12.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        truth(101, 3599.898, 3, PA, c("ConstPos"), [50.0, -12.5, 0.0], [0.0, 13.5, 0.0], [0.0, 1.0, 0.0]),
        truth(102, 3600.198, 4, PB, c("ConstSpeed"), [-3.125, 40.0, 1.5], [-4.33, 2.5, 0.0], [-0.866_025, 0.5, 0.0]),
        truth(103, 3700.0, 5, "0011223344556677", c("ConstPosOffset"), [1.0, 2.0, 0.0], Z, Z),
        truth(104, 10.0, 6, "8899aabbccddeeff", c("benign"), [7.0, 7.0, 0.0], Z, Z),
    ];
    e
}

fn json_files(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .expect("dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect()
}

/// The writer's files are the fixture's files, name for name and byte for byte.
#[test]
fn the_writer_reproduces_the_hand_made_fixture_byte_for_byte() {
    for codebook in Codebook::ALL {
        let dir = scratch_dir(&format!("veremi-fixture-{}", codebook.id())).expect("scratch");
        let files = veremi::write(&dir, &fixture_export(codebook)).expect("write");
        let want = fixture_dir(codebook);
        assert_eq!(json_files(&dir), json_files(&want), "{}: file names", codebook.id());
        for name in json_files(&want) {
            let got = std::fs::read_to_string(dir.join(&name)).expect("written");
            let exp = std::fs::read_to_string(want.join(&name)).expect("fixture");
            assert_eq!(got, exp, "{}: {name} differs from the fixture", codebook.id());
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("VEREMI-NOTE.txt")).expect("note"),
            VEREMI_COMPATIBILITY_NOTE
        );
        assert!(files.iter().any(|f| f.path.ends_with("VEREMI-NOTE.txt")));
    }
}

/// Reading the fixture gives back the export, field for field.
#[test]
fn the_fixture_reads_back_field_for_field() {
    for codebook in Codebook::ALL {
        let mut want = fixture_export(codebook);
        want.quantise();
        let got = veremi::read_dir(fixture_dir(codebook), codebook, 3600.0).expect("read");
        assert_eq!(got.by_receiver, want.by_receiver, "{}: receptions", codebook.id());
        assert_eq!(got.own_positions, want.own_positions, "{}: own", codebook.id());
        assert_eq!(got.truth, want.truth, "{}: truth", codebook.id());
        assert_eq!(got, want, "{}", codebook.id());
    }
}

/// The two codebooks' codes as the fixture carries them, and as the accept line states
/// them: VeReMi 1.0 writes 16 for EventualStop and 1024 for const-speed.
#[test]
fn the_codebooks_write_the_codes_the_fixture_holds() {
    let v1 = fixture_export(Codebook::Veremi1);
    let codes: Vec<i64> = v1.truth.iter().map(|t| t.attacker_type).collect();
    assert_eq!(codes, [1, 1, 1024, 2, 0]);
    assert_eq!(Codebook::Veremi1.code("EventualStop"), 16);
    assert_eq!(Codebook::Veremi1.code("const-speed"), 1024);
    let ext = fixture_export(Codebook::VeremiExt);
    assert_eq!(ext.receiver_attacker_type.get(&9), Some(&9));
}

/// The receiver-visible files pass the leakage linter: `sender` is the pseudonym's
/// integer, and no real id is a value in them.
#[test]
fn the_fixture_traces_pass_the_leakage_linter() {
    for codebook in Codebook::ALL {
        let dir = scratch_dir(&format!("veremi-lint-{}", codebook.id())).expect("scratch");
        let e = fixture_export(codebook);
        let files = veremi::write(&dir, &e).expect("write");
        let report = veremi::lint(&files, &e).expect("lint");
        assert!(report.is_clean(), "{}", report.summary());
    }
}

/// A run through the Extension codebook: sliced files, type-4 truth, codes from the
/// Extension table, and a clean lint.
#[test]
fn a_run_exports_in_the_extension_layout() {
    let recs = records(&DatasetShape::default());
    let opts = VeremiOptions {
        codebook: Codebook::VeremiExt,
        slice_s: 3600.0,
    };
    let e = veremi::from_records_with(&recs, &opts).expect("build");
    assert!(e.truth.iter().any(|t| t.attacker_type == 2), "ConstPosOffset is 2");
    assert!(e.truth.iter().all(|t| t.attacker_type == 0 || t.attacker_type == 2));
    let dir = scratch_dir("veremi-ext-run").expect("scratch");
    let files = veremi::write(&dir, &e).expect("write");
    let names: BTreeSet<String> = files
        .iter()
        .filter_map(|f| f.path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert!(names.contains("traceGroundTruthJSON-0.json"), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("traceJSON-") && n.contains("-A")));
    let truth = v2xw_record::export::jsonl::read(dir.join("traceGroundTruthJSON-0.json"))
        .expect("read");
    assert!(truth.iter().all(|r| r["type"] == 4));
    assert!(veremi::lint(&files, &e).expect("lint").is_clean());
    // …and it reads back as what was written.
    let back = veremi::read_dir(&dir, Codebook::VeremiExt, 3600.0).expect("read");
    assert_eq!(back.truth, e.truth);
    assert_eq!(back.by_receiver, e.by_receiver);
}

fn rec(channel: &str, t: u64, v: serde_json::Value) -> RecordedRecord {
    RecordedRecord {
        channel: channel.to_string(),
        sim_time: t,
        json: serde_json::to_vec(&v).expect("json"),
    }
}

/// With `node.tx` content, a reception line carries what went on the air — the falsified
/// claim, the frame's own pseudonym, the BSM's accuracy ellipse as `pos_noise` and its
/// longitudinal acceleration along the heading — and the truth line the true state.
#[test]
fn a_reception_carries_the_claim_that_went_on_the_air() {
    let t = 1_000_000_000u64;
    let recs = vec![
        rec("gt.kinematics", t, serde_json::json!({"t": t, "actor": 1, "x_m": 10.0,
            "y_m": 5.0, "speed_mps": 10.0, "acc_mps2": 1.0,
            "heading_rad": std::f64::consts::FRAC_PI_2})),
        rec("gt.attack.action", t, serde_json::json!({"t": t, "actor": 1,
            "attacker": "threat/attacker/legacy/ConstPos", "action": "falsify",
            "changed_bytes_on_air": true, "msg": 5})),
        rec("node.tx", t, serde_json::json!({"t": t, "node": 1, "msg": 5, "bytes_on_wire": 300,
            "pseudonym": "cafef00dcafef00d",
            "content": {"claimed_x_m": 300.0, "claimed_y_m": 400.0, "claimed_speed_mps": 2.0,
                        "claimed_heading_rad": 0.0, "pos_semi_major_m": 2.0,
                        "pos_semi_minor_m": 1.0, "pos_orientation_deg": 0.0,
                        "accel_long_mps2": -1.5}})),
        rec("phy.rx", t, serde_json::json!({"t_start": t, "t_end": t + 500_000, "tx": 1,
            "rx": 2, "msg": 5, "rssi_dbm": -70.0, "outcome": "ok"})),
    ];
    let e = veremi::from_records_with(&recs, &VeremiOptions::default()).expect("build");
    let r = &e.by_receiver[&2][0];
    assert_eq!(r.sender_pseudo, "cafef00dcafef00d");
    assert_eq!(r.pos, [300.0, 400.0, 0.0], "the on-air claim, not the truth");
    assert_eq!(r.spd, [2.0, 0.0, 0.0]);
    assert_eq!(r.hed, [1.0, 0.0, 0.0]);
    assert_eq!(r.acl, [-1.5, 0.0, 0.0]);
    assert_eq!(r.pos_noise, [1.0, 2.0, 0.0], "major axis north: 1 m east, 2 m north");
    let k = &e.truth[0];
    assert_eq!(k.attacker_type, 1, "ConstPos in the Extension codebook");
    assert_eq!(k.pos, [10.0, 5.0, 0.0]);
    assert_eq!(k.hed, [0.0, 1.0, 0.0]);
    assert_eq!(k.spd, [0.0, 10.0, 0.0]);
    assert_eq!(k.sender, 1);
}
