//! The `lamps` byte (vwp-v1 §3.3.5, v1.2): carried in the keyframe column v1.0 reserved as
//! `flags8` and in the moved and spawn rows' former `reserved` byte, round-tripped exactly,
//! and — unlike heading, speed and acceleration — a change of lamps alone makes a moved row,
//! because a car stopped at a red whose brake lamps go out as it releases the pedal has not
//! moved yet.

use v2xw_core::ids::ActorId;
use v2xw_record::encoder::{ActorPose, Cadence, Snapshot, SnapshotEncoder};
use v2xw_record::profile::{NodeProfileStripper, Profile};
use v2xw_record::wire::snapshot::{DeltaBody, KeyframeBody};

fn parked(slot: u32, lamps: u8) -> ActorPose {
    let mut p = ActorPose::new(slot, ActorId::new(slot), [10.0 + f64::from(slot) * 8.0, 5.0, 0.0]);
    p.lamps = lamps;
    p
}

#[test]
fn a_lamp_change_alone_is_a_moved_row_and_round_trips() {
    let mut enc = SnapshotEncoder::new([0.0, 0.0, 0.0], Cadence::DEFAULT, Profile::Full, 0);
    // Step 0, keyframe: slot 0 braking, slot 1 dark.
    let kf = enc
        .encode(&Snapshot::new(0, vec![parked(0, 0x01), parked(1, 0)]))
        .expect("keyframe");
    let body = KeyframeBody::decode(kf.frame().body()).expect("keyframe body");
    assert_eq!(body.actors[0].lamps, 0x01);
    assert_eq!(body.actors[1].lamps, 0);

    // Step 1: nothing moves; slot 1 puts its left indicator on.
    let d = enc
        .encode(&Snapshot::new(100_000_000, vec![parked(0, 0x01), parked(1, 0x02)]))
        .expect("delta");
    let body = DeltaBody::decode(d.frame().body()).expect("delta body");
    assert_eq!(body.moved.len(), 1, "only the slot whose lamps changed");
    assert_eq!(body.moved[0].slot, 1);
    assert_eq!(body.moved[0].lamps, 0x02);
    assert_eq!((body.moved[0].dx_mm, body.moved[0].dy_mm), (0, 0));

    // Step 2: nothing changes at all: no row.
    let d = enc
        .encode(&Snapshot::new(200_000_000, vec![parked(0, 0x01), parked(1, 0x02)]))
        .expect("delta");
    assert!(DeltaBody::decode(d.frame().body()).expect("body").moved.is_empty());

    // Step 3: a spawn carries its lamps.
    let d = enc
        .encode(&Snapshot::new(
            300_000_000,
            vec![parked(0, 0x01), parked(1, 0x02), parked(2, 0x40 | 0x10)],
        ))
        .expect("delta");
    let body = DeltaBody::decode(d.frame().body()).expect("body");
    assert_eq!(body.spawns.len(), 1);
    assert_eq!(body.spawns[0].lamps, 0x50);
    // And the encoded bytes decode to the same body.
    let again = body.to_frame(9, 0).expect("re-encode");
    assert_eq!(DeltaBody::decode(again.body()).expect("body"), body);
}

#[test]
fn the_node_profile_keeps_the_lamps_because_they_are_public() {
    let mut enc = SnapshotEncoder::new([0.0, 0.0, 0.0], Cadence::DEFAULT, Profile::Full, 0);
    let mut strip = NodeProfileStripper::new();
    let kf = enc.encode(&Snapshot::new(0, vec![parked(0, 0)])).expect("keyframe");
    strip.strip(kf.frame()).expect("strip");
    let d = enc
        .encode(&Snapshot::new(100_000_000, vec![parked(0, 0x04)]))
        .expect("delta");
    let blind = strip.strip(d.frame()).expect("strip").expect("kept");
    let body = DeltaBody::decode(blind.body()).expect("body");
    assert_eq!(body.moved.len(), 1, "a lamp change is visible to anyone");
    assert_eq!(body.moved[0].lamps, 0x04);
}
