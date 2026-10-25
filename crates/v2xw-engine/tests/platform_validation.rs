//! Platform validation (docs/validation/platform-2026-10-24.md): what a setting asks for
//! against what the run sends, and what goes on the air against what produced it.
//!
//! * A fixed `messages.rate_hz` is the BSM rate sent, at the low end of its range too.
//!   The generator clamps its interval into J2945/1's [vMinITT, vMaxITT] = [100, 600] ms,
//!   and the fixed rate lowered the floor for rates above 10 Hz but never raised the
//!   ceiling, so 1 Hz went out at 1.67 Hz.
//! * The SPaT and MAP a roadside unit transmits — the octets of the SPDU it signed, tapped
//!   as they leave the node — decode to the junction's controller state at the instant the
//!   SPaT says it was determined, and to the junction's surveyed geometry; every signal
//!   group the MAP connects a lane under is one the SPaT reports, and the other way round
//!   (CTI 4501 v01 §6: a receiver pairs the two by intersection id and signal group).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::ids::NodeId;
use v2xw_core::time::{NS_PER_MS, SimTime, WallClock};
use v2xw_engine::infra::{InfraFraming, IntersectionFeed};
use v2xw_engine::{Engine, MemoryRecorder, RunRecorder, Scenario};
use v2xw_metrics::channels::{NodeTxView, decode};
use v2xw_msg::generator::{BsmGenerator, DccState};
use v2xw_msg::j2735::{map, spat};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// BSMs a generator built from the scenario's own parameters asks for over `seconds`,
/// checked on a grid of the fixed period (the node steps at least that often).
fn bsms_in(s: &Scenario, seconds: u64) -> usize {
    let (params, _) = v2xw_engine::wiring::generator_params(s);
    let mut g = BsmGenerator::new(params);
    let step = v2xw_engine::wiring::fixed_message_period(s)
        .map_or(100 * NS_PER_MS, |p| p.as_nanos())
        .min(100 * NS_PER_MS);
    let dcc = DccState::UNRESTRICTED;
    let mut n = 0;
    let mut t: SimTime = 0;
    while t < seconds * 1_000_000_000 {
        if g.check(t, &dcc).is_some() {
            n += 1;
        }
        t += step;
    }
    n
}

/// `messages.rate_hz` across its whole range is the rate sent: 1 Hz is ten BSMs in ten
/// seconds, not the seventeen a 600 ms ceiling made of it.
#[test]
fn a_fixed_bsm_rate_is_the_rate_sent_across_its_range() {
    let mut s = Scenario::load(scenarios().join("sensitivity/dsrc-grid.yaml")).expect("loads");
    for (hz, want) in [(1.0, 10), (1.25, 13), (2.0, 20), (5.0, 50), (10.0, 100), (20.0, 200)] {
        s.messages.rate_hz = Some(hz);
        let got = bsms_in(&s, 10);
        assert_eq!(got, want, "messages.rate_hz {hz}: {got} BSMs in 10 s");
    }
    // Unset is J2945/1's 10 Hz.
    s.messages.rate_hz = None;
    assert_eq!(bsms_in(&s, 10), 100);
}

/// A recorder that keeps every tapped SPDU and forwards everything else.
struct Tapping {
    inner: MemoryRecorder,
    taps: BTreeMap<u64, (NodeId, Vec<u8>)>,
}

impl RunRecorder for Tapping {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        self.inner.write(at, record);
    }
    fn write_wire_frame(&mut self, frame: &v2xw_record::wire::Frame) {
        self.inner.write_wire_frame(frame);
    }
    fn tap_frame(&mut self, _at: SimTime, node: NodeId, msg: u64, spdu: &[u8]) {
        self.taps.insert(msg, (node, spdu.to_vec()));
    }
}

/// The unsecured payload inside a signed SPDU: the `payload_bytes`-long slice that
/// decodes as `decode` and re-encodes to itself. 1609.2 carries it as an octet-aligned
/// OCTET STRING, so it starts on a byte.
fn payload<T>(
    spdu: &[u8],
    len: usize,
    decode: impl Fn(&[u8]) -> Option<T>,
    encode: impl Fn(&T) -> Option<Vec<u8>>,
) -> Option<T> {
    (0..=spdu.len().saturating_sub(len)).find_map(|i| {
        let slice = &spdu[i..i + len];
        let v = decode(slice)?;
        (encode(&v)?.as_slice() == slice).then_some(v)
    })
}

/// Whether `on_air` is what `feed` reports at `t`: the same signal groups in the same
/// states, each changing at the same tenth of a second (±1 for the millisecond the SPaT's
/// `timeStamp` rounds its instant to).
fn same_spat(on_air: &spat::Spat, feed: &IntersectionFeed, t: SimTime, wall: WallClock) -> Result<(), String> {
    let bytes = feed.spat_bytes(t, wall, InfraFraming::J2735)?;
    let want = spat::decode_message_frame(&bytes).map_err(|e| e.to_string())?;
    let (a, b) = (&on_air.intersections[0], &want.intersections[0]);
    if a.id != b.id {
        return Err(format!("intersection {:?} on the air, {:?} from the feed", a.id, b.id));
    }
    if a.states.len() != b.states.len() {
        return Err(format!("{} movement states on the air, {} from the feed", a.states.len(), b.states.len()));
    }
    for (x, y) in a.states.iter().zip(&b.states) {
        let (ex, ey) = (&x.events[0], &y.events[0]);
        if x.signal_group != y.signal_group || ex.event_state != ey.event_state {
            return Err(format!(
                "group {} {:?} on the air, group {} {:?} from the plan",
                x.signal_group, ex.event_state, y.signal_group, ey.event_state
            ));
        }
        let (tx, ty) = (ex.timing.map(|m| m.min_end_time), ey.timing.map(|m| m.min_end_time));
        match (tx, ty) {
            (Some(p), Some(q)) if (i32::from(p) - i32::from(q)).abs() <= 1 => {}
            (None, None) => {}
            _ => return Err(format!("group {}: minEndTime {tx:?} on the air, {ty:?} from the plan", x.signal_group)),
        }
    }
    Ok(())
}

/// The SPaT and MAP on the air are the controller's state and the junction's survey.
#[test]
fn the_spat_and_map_on_the_air_are_the_plan_and_the_survey() {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml")).expect("loads");
    s.time.duration_s = 6.0;
    s.metrics = vec![];
    s.messages.codec_tier = "uper".into();
    // No priority requests: a served request moves the plan, and the feed built here from
    // the world would not know it.
    s.messages.sets = vec!["bsm".into(), "spat".into(), "map".into()];
    s.actors.vehicles.classes.remove("emergency");
    if let Some(p) = s.actors.vehicles.classes.get_mut("passenger") {
        p.fraction = 1.0;
    }
    let world = v2xw_engine::wiring::build_world(&s).expect("the world builds");
    let wall = WallClock::parse_rfc3339(&s.time.t0).expect("t0");
    let origin: v2xw_core::geo::GeoOrigin = world.origin.into();
    let feeds: BTreeMap<u16, IntersectionFeed> = (0..world.signals.len())
        .filter_map(|i| IntersectionFeed::build(&world, i, origin).ok())
        .map(|f| (f.intersection_id, f))
        .collect();

    let mut rec = Tapping {
        inner: MemoryRecorder::new(),
        taps: BTreeMap::new(),
    };
    Engine::build(s, "").expect("builds").run(&mut rec).expect("runs");

    let mut spats = 0usize;
    let mut maps = 0usize;
    let mut wrong_feed_caught = 0usize;
    let mut groups_by_id: BTreeMap<u16, (BTreeSet<u8>, BTreeSet<u8>)> = BTreeMap::new();
    for (_, r) in rec.inner.records().iter().filter(|(_, r)| r.channel == "node.tx") {
        let tx: NodeTxView = decode(r).expect("node.tx decodes");
        let (Some(kind), Some(msg), Some(len)) = (tx.msg_type.as_deref(), tx.msg, tx.payload_bytes) else {
            continue;
        };
        let Some((node, octets)) = rec.taps.get(&msg) else {
            continue;
        };
        assert_eq!(*node, tx.node);
        match kind {
            "spat" => {
                let on_air = payload(
                    octets,
                    len as usize,
                    |b| spat::decode_message_frame(b).ok(),
                    |v| spat::encode_message_frame(v).ok().map(|e| e.bytes),
                )
                .unwrap_or_else(|| panic!("SPaT {msg}: no decodable payload in its SPDU"));
                let state = &on_air.intersections[0];
                let feed = feeds
                    .get(&state.id.id)
                    .unwrap_or_else(|| panic!("SPaT {msg} names intersection {}", state.id.id));
                // The instant the state was determined: `timeStamp` is the millisecond
                // of the minute, at or before the frame went out.
                let stamp = state.time_stamp.expect("a SPaT states when it was determined");
                let t_gen = (0..=500u64)
                    .map(|d| tx.t.saturating_sub(d * NS_PER_MS) / NS_PER_MS * NS_PER_MS)
                    .find(|t| spat::d_second(wall, *t) == stamp)
                    .unwrap_or_else(|| panic!("SPaT {msg}: timeStamp {stamp} is not within 500 ms before it was sent"));
                assert_eq!(
                    state.moy,
                    Some(spat::minute_of_the_year(wall, t_gen)),
                    "SPaT {msg}: moy"
                );
                if let Err(e) = same_spat(&on_air, feed, t_gen, wall) {
                    panic!("SPaT {msg} from {:?} at {} ns: {e}", tx.node, t_gen);
                }
                // The check can fail: the same SPaT against the plan half a cycle later
                // disagrees on some group's state or end time.
                if same_spat(&on_air, feed, t_gen + 30_000 * NS_PER_MS, wall).is_err() {
                    wrong_feed_caught += 1;
                }
                let e = groups_by_id.entry(state.id.id).or_default();
                e.0.extend(state.states.iter().map(|m| m.signal_group));
                spats += 1;
            }
            "map" => {
                let on_air = payload(
                    octets,
                    len as usize,
                    |b| map::decode_message_frame(b).ok(),
                    |v| map::encode_message_frame(v).ok().map(|e| e.bytes),
                )
                .unwrap_or_else(|| panic!("MAP {msg}: no decodable payload in its SPDU"));
                let g = &on_air.intersections[0];
                let feed = feeds
                    .get(&g.id.id)
                    .unwrap_or_else(|| panic!("MAP {msg} names intersection {}", g.id.id));
                assert_eq!(&on_air, feed.map(), "MAP {msg}: the survey, as the feed holds it");
                let e = groups_by_id.entry(g.id.id).or_default();
                e.1.extend(g.lanes.iter().flat_map(|l| l.connects_to.iter().filter_map(|c| c.signal_group)));
                maps += 1;
            }
            _ => {}
        }
    }
    // Four units for 6 s: 10 Hz SPaT and 1 Hz MAP each.
    assert!((4 * 58..=4 * 61).contains(&spats), "{spats} SPaT from four units in 6 s");
    assert!((4 * 5..=4 * 7).contains(&maps), "{maps} MAP from four units in 6 s");
    assert!(
        wrong_feed_caught * 2 > spats,
        "only {wrong_feed_caught} of {spats} SPaT disagree with the plan half a cycle on: the comparison is too loose"
    );
    for (id, (in_spat, in_map)) in &groups_by_id {
        assert!(!in_map.is_empty(), "intersection {id}: the MAP connects no lane under a signal group");
        assert_eq!(in_spat, in_map, "intersection {id}: SPaT groups vs the MAP's connection groups");
    }
    assert_eq!(groups_by_id.len(), 4, "the four units' junctions");
}
