//! The VeReMi-compatible exporter — 08-measurement-and-data.md §5, `receiver-logs` with
//! `--veremi-json`, and the adoption plan's WP-03 (docs/design/frameworks/f2md.md §5 P2,
//! P3).
//!
//! VeReMi (Van der Heijden, Lukaseder, Kargl, SecureComm 2018) and the VeReMi Extension
//! (Kamel, Wolf, van der Heijden, Kaiser, Urien, Kargl, ICC 2020) are the formats the
//! misbehaviour-detection literature reads, and comparability with them is the point: a
//! detector written against VeReMi should run on this engine's output with no reader
//! changes. So the field names are VeReMi's — `rcvTime`, `sendTime`, `senderPseudo`,
//! `messageID`, `pos`, `pos_noise`, `spd`, `spd_noise`, `acl`, `acl_noise`, `hed`,
//! `hed_noise`, `RSSI`, `attackerType` — and so is the file layout. Re-implemented from the
//! published schema; F2MD's `VeReMiPrintable` was read for behaviour only and no code was
//! copied. No VeReMi dataset row is reproduced: the test fixture is written by hand.
//!
//! # Two codebooks, chosen by an export option
//!
//! [`Codebook::Veremi1`] is VeReMi 1.0's bit-coded `attackerType` (1 ConstPos,
//! 2 ConstPosOffset, 4 RandomPos, 8 RandomPosOffset, 16 EventualStop) with this engine's
//! established layout: one `traceJSON-<receiver>.json` per receiver and one
//! `GroundTruthJSONlog.json`. [`Codebook::VeremiExt`] is the VeReMi Extension's: F2MD's
//! attack enum 0–19 as the code, `traceJSON-<rx>-<module>-A<code>-<t0>-<slice>.json` per
//! receiver and slice, type-4 truth in `traceGroundTruthJSON-<slice>.json`, and hourly
//! slices (F2MD `VeReMiSliceTime` = 3600 s). An engine attack family is given a code only
//! where its behaviour matches the codebook's attack ([`Codebook::code`]); every other
//! family is [`UNNAMED_CODE`] (1024), which a consumer can recognise, rather than a VeReMi
//! code it would silently misread.
//!
//! # One deliberate deviation, and why
//!
//! VeReMi's per-receiver trace carries a `sender` field holding the **transmitter's real
//! module id**, beside `senderPseudo` holding its pseudonym. That is a ground-truth column
//! inside a receiver-visible file. F2MD shipped the same defect under the name
//! `senderRealId` (f2md.md defect D5), and it is the single leak this project's leakage
//! linter was written against — a detector trained on a VeReMi trace can read the real id,
//! link every pseudonym for free, and score far better than any real receiver could.
//!
//! This exporter therefore writes `sender` as a **stable integer derived from the
//! pseudonym**, not from the device: a receiver really can see that two messages carried
//! the same certificate, and really cannot see that two different certificates are the
//! same car. The true identity goes into the ground-truth file alone, keyed by
//! `messageID`, which is the join a consumer performs offline — VeReMi's own ground-truth
//! file works exactly that way.
//!
//! The deviation is recorded in [`VEREMI_COMPATIBILITY_NOTE`], written into the export's
//! `VEREMI-NOTE.txt` and stated in the datasheet, because a silent incompatibility is
//! worse than a declared one.
//!
//! # Message types and what each field holds
//!
//! `type: 2` is the receiver's own position, `type: 3` a received beacon and `type: 4`
//! (Extension layout) a transmission's truth. On a type-3 line the claim is what went on
//! the air (`node.tx` `content`: the attacker's falsified claim, or the honest sender's
//! belief), `spd` is the claimed speed along the claimed heading, `hed` is that heading as
//! an ENU unit vector, and `pos_noise` is the BSM's `PositionalAccuracy` ellipse projected
//! on the east and north axes. A BSM's Part I states no speed, heading or acceleration
//! confidence (SAE J2735 `BSMcoreData`), so `spd_noise`, `hed_noise` and `acl_noise` are
//! zero, meaning "not stated", and `acl` is the longitudinal acceleration along the heading
//! (the lateral component is left out rather than guessed in sign). The confidences reach
//! the recording only when the scenario names the `veremi` exporter, which is what turns
//! the engine's broadcast-accuracy fields on; a recording without them writes zeros.
//!
//! The own-position lines come from `gt.kinematics` for the receiving node, which is
//! ground truth about the receiver *itself*, and a node reading its own position is not a
//! firewall breach.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use v2xw_core::math::{self, quantize_to};

use super::pyjson;
use crate::error::{RecordError, Result};
use crate::export::{ExportFormat, ExportedFile};
use crate::grid::{Q_DB, Q_METRES, Q_PROBABILITY, Q_SECONDS};

/// The note this exporter writes beside its output, and the datasheet repeats.
pub const VEREMI_COMPATIBILITY_NOTE: &str = "\
VeReMi compatibility note
=========================

Field names, file layout and message types follow VeReMi (Van der Heijden, Lukaseder,
Kargl, 2018) and the VeReMi Extension (Kamel et al., ICC 2020) so that a detector written
against VeReMi reads these traces unchanged. The export option `codebook` picks the
attackerType codes: `veremi-1` (1 ConstPos, 2 ConstPosOffset, 4 RandomPos,
8 RandomPosOffset, 16 EventualStop) or `veremi-ext` (the F2MD attack enum 0-19). An engine
attack whose behaviour matches none of the codebook's attacks is written as 1024
(unnamed), never as a VeReMi code it does not implement.

ONE DELIBERATE DEVIATION. In VeReMi, a per-receiver trace's `sender` field holds the
transmitter's real module id, beside `senderPseudo` holding its pseudonym. That is ground
truth inside a receiver-visible file: a model trained on it can link every pseudonym for
free and will score far above anything a real receiver could achieve. (F2MD shipped the
same defect as `senderRealId`.)

Here, `sender` is a stable integer derived from the PSEUDONYM, not from the device. Two
messages carrying one certificate share a `sender`; two certificates of one car do not.
The true identity is in the ground-truth file only, keyed by `messageID` — join it
offline, exactly as VeReMi's own ground-truth file is joined.

Consequence for benchmarking: a VeReMi-trained detector that relied on `sender` being a
stable per-vehicle id will score lower here. That is the honest number; the higher one was
measuring the leak.

NOISE FIELDS. `pos_noise` is the BSM's PositionalAccuracy ellipse on the east and north
axes. A BSM states no speed, heading or acceleration confidence, so `spd_noise`,
`hed_noise` and `acl_noise` are 0, which means \"not stated\", not \"exact\".
";

/// The code a family gets when the codebook has no attack with its behaviour.
pub const UNNAMED_CODE: i64 = 1024;

/// The default slice length, seconds: F2MD's `VeReMiSliceTime`.
pub const DEFAULT_SLICE_S: f64 = 3600.0;

/// The shortest slice the export accepts, seconds.
pub const MIN_SLICE_S: f64 = 1.0;

/// The longest slice the export accepts, seconds: one week.
pub const MAX_SLICE_S: f64 = 604_800.0;

/// Which `attackerType` codebook, and with it which file layout, an export uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Codebook {
    /// VeReMi 1.0's bit codes; one trace file per receiver, `GroundTruthJSONlog.json`.
    #[default]
    Veremi1,
    /// The VeReMi Extension's F2MD enum 0–19; sliced, `-A<code>`-named files.
    VeremiExt,
}

/// VeReMi 1.0's codes for the engine families whose behaviour matches.
///
/// The engine's `ConstPos` freezes the claim at one point (VeReMi's constant position);
/// its `RandomPos` adds a fresh bounded offset to the true position every message, which
/// is VeReMi's random position *offset* (8), not its uniform-over-the-playground random
/// position (4), which the engine does not have. `EventualStop` is the eventual-stop
/// family; the engine's version keeps claiming a creep speed after it freezes (f2md.md
/// §4.1), which the card states. Keys are normalised by [`family_key`].
const VEREMI_1_CODES: &[(&str, i64)] = &[
    ("constpos", 1),
    ("constposoffset", 2),
    ("randompos", 8),
    ("eventualstop", 16),
];

/// The VeReMi Extension's codes (F2MD `attackTypes`) for the engine families whose
/// behaviour matches (f2md.md §4.1's EQ rows and the same-semantics FB rows).
///
/// Left unnamed on purpose: `DataReplay` (the engine replays its own past state, F2MD a
/// neighbour's), `RelayReplay` (verbatim retransmission, not replay under one's own
/// pseudonym), `Disruptive` (random falsification, not neighbour replay), `DoSRandom`
/// (an offset, not fully random content) and every attack F2MD has no code for.
const VEREMI_EXT_CODES: &[(&str, i64)] = &[
    ("constpos", 1),
    ("constposoffset", 2),
    ("randompos", 4),
    ("constspeedoffset", 6),
    ("randomspeed", 7),
    ("eventualstop", 9),
    ("delayedmessages", 12),
    ("dos", 13),
    ("sybil", 16),
    ("ghostvehicles", 16),
];

/// An attack family's lookup key: the last segment of a model id
/// (`threat/attacker/legacy/ConstPos`), lower-cased, with `-` and `_` removed, so
/// `ConstPos`, `const-pos` and the full model id all name one family.
fn family_key(family: &str) -> String {
    family
        .rsplit('/')
        .next()
        .unwrap_or(family)
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

impl Codebook {
    /// Both codebooks, in a fixed order.
    pub const ALL: [Codebook; 2] = [Codebook::Veremi1, Codebook::VeremiExt];

    /// The option value that names it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Codebook::Veremi1 => "veremi-1",
            Codebook::VeremiExt => "veremi-ext",
        }
    }

    /// The codebook an option value names.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }

    /// The `attackerType` code for an engine attack family: `0` for benign, the
    /// codebook's code where the behaviour matches, [`UNNAMED_CODE`] otherwise.
    #[must_use]
    pub fn code(self, family: &str) -> i64 {
        let key = family_key(family);
        if matches!(key.as_str(), "" | "none" | "benign" | "genuine") {
            return 0;
        }
        let table = match self {
            Codebook::Veremi1 => VEREMI_1_CODES,
            Codebook::VeremiExt => VEREMI_EXT_CODES,
        };
        table
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(UNNAMED_CODE, |(_, c)| *c)
    }
}

/// The export options a scenario's `veremi` exporter takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VeremiOptions {
    /// The codebook, and with it the layout. Default `veremi-ext`.
    pub codebook: Codebook,
    /// The slice length in seconds for the Extension layout. Default 3600.
    pub slice_s: f64,
}

impl Default for VeremiOptions {
    fn default() -> Self {
        Self {
            codebook: Codebook::VeremiExt,
            slice_s: DEFAULT_SLICE_S,
        }
    }
}

impl VeremiOptions {
    /// Reads `{codebook, slice_s}` from an exporter's `opts`, refusing an unknown key or
    /// value with a message naming it. `profile` is accepted and ignored, because every
    /// exporter takes it; a VeReMi trace is node-visible by construction.
    ///
    /// # Errors
    /// `(field, why)` for the first bad option.
    pub fn from_opts(opts: &serde_json::Value) -> core::result::Result<Self, (String, String)> {
        let mut out = Self::default();
        let Some(map) = opts.as_object() else {
            if opts.is_null() {
                return Ok(out);
            }
            return Err(("opts".into(), "must be an object".into()));
        };
        for (k, v) in map {
            match k.as_str() {
                "codebook" => {
                    out.codebook = v.as_str().and_then(Codebook::parse).ok_or_else(|| {
                        (
                            "opts.codebook".to_string(),
                            format!("is {v}; one of veremi-1, veremi-ext"),
                        )
                    })?;
                }
                "slice_s" => {
                    out.slice_s = v
                        .as_f64()
                        .filter(|s| (MIN_SLICE_S..=MAX_SLICE_S).contains(s))
                        .ok_or_else(|| {
                            (
                                "opts.slice_s".to_string(),
                                format!(
                                    "is {v}; the slice length is seconds in \
                                     [{MIN_SLICE_S}, {MAX_SLICE_S}]"
                                ),
                            )
                        })?;
                }
                "profile" => {}
                other => {
                    return Err((
                        format!("opts.{other}"),
                        "is not a veremi option; codebook, slice_s".to_string(),
                    ));
                }
            }
        }
        Ok(out)
    }
}

/// One received beacon as a receiver saw it — VeReMi's `type: 3` line, node-visible.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VeremiReception {
    /// The receiving node.
    pub receiver: u64,
    /// When the frame was received, in seconds.
    pub rcv_time: f64,
    /// When it was generated, in seconds.
    pub send_time: f64,
    /// The sender, as a stable integer derived from its **pseudonym** (see the note).
    pub sender: i64,
    /// The pseudonym itself.
    pub sender_pseudo: String,
    /// The message id, which is the join key to the ground-truth log.
    pub message_id: u64,
    /// The claimed position, `[x, y, z]` in metres.
    pub pos: [f64; 3],
    /// The stated position confidence on each axis, metres (0 = not stated).
    pub pos_noise: [f64; 3],
    /// The claimed velocity, `[x, y, z]` in m/s.
    pub spd: [f64; 3],
    /// The stated speed confidence, m/s (0 = not stated).
    pub spd_noise: [f64; 3],
    /// The claimed acceleration, m/s².
    pub acl: [f64; 3],
    /// The stated acceleration confidence, m/s² (0 = not stated).
    pub acl_noise: [f64; 3],
    /// The claimed heading as a unit vector, ENU.
    pub hed: [f64; 3],
    /// The stated heading confidence (0 = not stated).
    pub hed_noise: [f64; 3],
    /// The received signal strength.
    pub rssi: f64,
}

/// One transmission as it really was — the ground-truth log, ORACLE.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VeremiTruth {
    /// When it was sent, in seconds.
    pub send_time: f64,
    /// The **real** transmitter. This field exists only in this file.
    pub sender: i64,
    /// The pseudonym it used.
    pub sender_pseudo: String,
    /// The message id.
    pub message_id: u64,
    /// The true position.
    pub pos: [f64; 3],
    /// Zero: truth has no noise.
    pub pos_noise: [f64; 3],
    /// The true velocity.
    pub spd: [f64; 3],
    /// Zero.
    pub spd_noise: [f64; 3],
    /// The true acceleration along the heading.
    pub acl: [f64; 3],
    /// Zero.
    pub acl_noise: [f64; 3],
    /// The true heading, unit vector.
    pub hed: [f64; 3],
    /// Zero.
    pub hed_noise: [f64; 3],
    /// The `attackerType` code in the export's codebook: `0` for benign.
    pub attacker_type: i64,
}

/// One receiver's own position report — VeReMi's `type: 2` line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OwnPosition {
    /// The instant, seconds.
    pub t: f64,
    /// The position, metres.
    pub pos: [f64; 3],
    /// Its noise (0 = not stated).
    pub pos_noise: [f64; 3],
    /// The velocity, m/s.
    pub spd: [f64; 3],
    /// Its noise.
    pub spd_noise: [f64; 3],
    /// The acceleration, m/s².
    pub acl: [f64; 3],
    /// Its noise.
    pub acl_noise: [f64; 3],
    /// The heading, unit vector.
    pub hed: [f64; 3],
    /// Its noise.
    pub hed_noise: [f64; 3],
}

/// A whole VeReMi-shaped export.
#[derive(Debug, Clone, PartialEq)]
pub struct VeremiExport {
    /// The codebook the codes are in, which also picks the layout.
    pub codebook: Codebook,
    /// The slice length for the Extension layout, seconds.
    pub slice_s: f64,
    /// Receptions grouped by receiving node, each already in time order.
    pub by_receiver: BTreeMap<u64, Vec<VeremiReception>>,
    /// The receivers' own position reports (VeReMi's `type: 2`), by node.
    pub own_positions: BTreeMap<u64, Vec<OwnPosition>>,
    /// Each receiver's own `attackerType` (the `-A<code>` of its file name); absent is 0.
    pub receiver_attacker_type: BTreeMap<u64, i64>,
    /// The ground-truth log, in message-id order.
    pub truth: Vec<VeremiTruth>,
}

impl Default for VeremiExport {
    fn default() -> Self {
        Self {
            codebook: Codebook::Veremi1,
            slice_s: DEFAULT_SLICE_S,
            by_receiver: BTreeMap::new(),
            own_positions: BTreeMap::new(),
            receiver_attacker_type: BTreeMap::new(),
            truth: Vec::new(),
        }
    }
}

/// The stable integer a pseudonym maps to in the `sender` field.
///
/// Derived from the pseudonym's own bytes, so it is reproducible across runs of the same
/// scenario and carries no device identity. VeReMi's `sender` is an `int`, so the value is
/// folded into a positive 32-bit range rather than handed out as a 64-bit hash: a consumer
/// that stores it in an `int32` column — and VeReMi consumers do — must not see it wrap.
#[must_use]
pub fn pseudonym_int(pseudonym: &str) -> i64 {
    let h = v2xw_core::hash::sha256(pseudonym.as_bytes());
    let v = u32::from_be_bytes([h[0], h[1], h[2], h[3]]);
    i64::from(v & 0x7fff_ffff)
}

/// VeReMi 1.0's `attackerType` code for an attack family ([`Codebook::Veremi1`]).
///
/// VeReMi 1.0 bit-codes its five attacks — 1 `ConstPos`, 2 `ConstPosOffset`,
/// 4 `RandomPos`, 8 `RandomPosOffset`, 16 `EventualStop` — with `0` for benign. A family
/// this engine has and VeReMi does not gets [`UNNAMED_CODE`] rather than being squeezed
/// into one of VeReMi's, because a consumer that maps `16` to `EventualStop` would
/// otherwise silently mislabel it.
#[must_use]
pub fn attacker_type_code(family: &str) -> i64 {
    Codebook::Veremi1.code(family)
}

fn q3v(v: &mut [f64; 3], quantum: f64) {
    for x in v {
        *x = quantize_to(*x, quantum);
    }
}

impl VeremiReception {
    fn quantise(&mut self) {
        self.rcv_time = quantize_to(self.rcv_time, Q_SECONDS);
        self.send_time = quantize_to(self.send_time, Q_SECONDS);
        for v in [
            &mut self.pos,
            &mut self.pos_noise,
            &mut self.spd,
            &mut self.spd_noise,
            &mut self.acl,
            &mut self.acl_noise,
        ] {
            q3v(v, Q_METRES);
        }
        q3v(&mut self.hed, Q_PROBABILITY);
        q3v(&mut self.hed_noise, Q_PROBABILITY);
        self.rssi = quantize_to(self.rssi, Q_DB);
    }
}

impl VeremiExport {
    /// Quantises every float to its declared grid (D9) — positions, speeds, accelerations
    /// and their noise on the metre grid, the heading vector on the 1e-6 grid, RSSI on the
    /// dB grid.
    ///
    /// Called by [`write`], so an export written through this module is on-grid whatever
    /// the caller handed in; it is public because a caller assembling an export by hand
    /// should be able to normalise it before comparing.
    pub fn quantise(&mut self) {
        for rows in self.by_receiver.values_mut() {
            for r in rows {
                r.quantise();
            }
        }
        for rows in self.own_positions.values_mut() {
            for o in rows {
                o.t = quantize_to(o.t, Q_SECONDS);
                for v in [
                    &mut o.pos,
                    &mut o.pos_noise,
                    &mut o.spd,
                    &mut o.spd_noise,
                    &mut o.acl,
                    &mut o.acl_noise,
                ] {
                    q3v(v, Q_METRES);
                }
                q3v(&mut o.hed, Q_PROBABILITY);
                q3v(&mut o.hed_noise, Q_PROBABILITY);
            }
        }
        for t in &mut self.truth {
            t.send_time = quantize_to(t.send_time, Q_SECONDS);
            for v in [
                &mut t.pos,
                &mut t.pos_noise,
                &mut t.spd,
                &mut t.spd_noise,
                &mut t.acl,
                &mut t.acl_noise,
            ] {
                q3v(v, Q_METRES);
            }
            q3v(&mut t.hed, Q_PROBABILITY);
            q3v(&mut t.hed_noise, Q_PROBABILITY);
        }
    }
}

/// The ENU unit vector of a heading in radians (0 = east, counter-clockwise).
fn unit(heading_rad: f64) -> [f64; 3] {
    let (s, c) = math::sin_cos(heading_rad);
    [c, s, 0.0]
}

/// A scalar along a unit vector.
fn along(v: f64, u: [f64; 3]) -> [f64; 3] {
    [v * u[0], v * u[1], v * u[2]]
}

/// The position-confidence ellipse projected on the east and north axes, metres.
///
/// `orientation_deg` is the semi-major axis's bearing clockwise from true north (J2735
/// `SemiMajorAxisOrientation`), so the major axis points along `(sin θ, cos θ)` in ENU and
/// the ellipse's half-extent along east is `√((a sin θ)² + (b cos θ)²)`. With no
/// orientation the major axis bounds both; with no ellipse the noise is not stated (0).
#[must_use]
pub fn ellipse_noise(
    semi_major_m: Option<f64>,
    semi_minor_m: Option<f64>,
    orientation_deg: Option<f64>,
) -> [f64; 3] {
    let Some(a) = semi_major_m else {
        return [0.0; 3];
    };
    let b = semi_minor_m.unwrap_or(a);
    match orientation_deg {
        Some(o) => {
            let (s, c) = math::sin_cos(o.to_radians());
            [math::hypot(a * s, b * c), math::hypot(a * c, b * s), 0.0]
        }
        None => [a, a, 0.0],
    }
}

/// Which messages an actor's attack labels: the message ids its byte-changing actions
/// named, and, for actions that named none, the window they span.
#[derive(Default)]
struct AttackLabel {
    family: String,
    msgs: BTreeSet<u64>,
    window: Option<(u64, u64)>,
}

impl AttackLabel {
    fn labels(&self, msg: u64, at: u64) -> bool {
        self.msgs.contains(&msg) || self.window.is_some_and(|(a, b)| at >= a && at <= b)
    }
}

/// [`from_records_with`] in the VeReMi 1.0 codebook and layout.
///
/// # Errors
/// [`RecordError::Malformed`] if a record does not fit its channel's view.
pub fn from_records(records: &[crate::reader::RecordedRecord]) -> Result<VeremiExport> {
    from_records_with(
        records,
        &VeremiOptions {
            codebook: Codebook::Veremi1,
            slice_s: DEFAULT_SLICE_S,
        },
    )
}

/// Builds a VeReMi-shaped export from a recording's records.
///
/// The join is the one a VeReMi trace needs and the one this engine's channels make
/// possible: `phy.rx` gives the reception and the receiver's measurements, `node.tx` gives
/// the generation instant, the message id, the pseudonym and what the message said,
/// `gt.kinematics` gives the truth, and `gt.attack.action` gives the attacker family and
/// the messages it falsified. A message whose transmission was never recorded still
/// produces a reception line — a receiver saw it, after all — with the send time taken
/// from the reception, because dropping it would silently shrink a trace. A transmission
/// recorded without its content (older recordings) falls back to the true state, offset
/// by the emissions tables' stand-in while the sender was attacking.
///
/// What is **not** joined is the device behind the pseudonym. `sender` is a function of the
/// pseudonym (see [`VEREMI_COMPATIBILITY_NOTE`]); the device appears in the ground-truth
/// log alone.
///
/// # Errors
/// [`RecordError::Malformed`] if a record does not fit its channel's view.
pub fn from_records_with(
    records: &[crate::reader::RecordedRecord],
    opts: &VeremiOptions,
) -> Result<VeremiExport> {
    use v2xw_metrics::channels::{
        GtAttackActionView, GtKinematicsView, MsgContentView, NodeTxView, PhyRxView, RxOutcome,
        SecCertView,
    };

    fn view<T: serde::de::DeserializeOwned>(rec: &crate::reader::RecordedRecord) -> Result<T> {
        serde_json::from_slice(&rec.json).map_err(|e| {
            RecordError::malformed(
                "veremi",
                format!("a record on {} does not fit its view: {e}", rec.channel),
            )
        })
    }
    let secs = |t: u64| math::q3(t as f64 / 1e9);

    // The transmission side, keyed by message id.
    struct Tx {
        node: u32,
        generated: u64,
        pseudonym: Option<String>,
        content: Option<MsgContentView>,
    }
    let mut tx_of_msg: BTreeMap<u64, Tx> = BTreeMap::new();
    // The truth, keyed by (actor, instant) so a reception can find the state its sender
    // really was in.
    let mut truth_at: BTreeMap<(u32, u64), GtKinematicsView> = BTreeMap::new();
    // The actor each node rides (`gt.kinematics` `node`). Node ids and actor ids are
    // different numberings in an engine run (roadside units take the first node ids), so
    // a transmitter's node must be resolved to its actor before its truth, its attack
    // label or its real id is read; a recording that names no node keeps them equal.
    let mut node_actor: BTreeMap<u32, u32> = BTreeMap::new();
    // Which attacker family each device belongs to, and which messages it falsified.
    let mut label: BTreeMap<u32, AttackLabel> = BTreeMap::new();
    // The `sec.cert` bindings per device, in record order.
    let mut certs: BTreeMap<u32, Vec<(u64, String)>> = BTreeMap::new();
    for r in records {
        match r.channel.as_str() {
            "node.tx" => {
                let v: NodeTxView = view(r)?;
                if let Some(msg) = v.msg {
                    tx_of_msg.insert(
                        msg,
                        Tx {
                            node: v.node.0,
                            generated: v.t_generated.unwrap_or(v.t),
                            pseudonym: v.pseudonym,
                            content: v.content,
                        },
                    );
                }
            }
            "gt.kinematics" => {
                let v: GtKinematicsView = view(r)?;
                if let Some(n) = v.node {
                    node_actor.entry(n.0).or_insert(v.actor.0);
                }
                truth_at.insert((v.actor.0, v.t), v);
            }
            "gt.attack.action" => {
                let v: GtAttackActionView = view(r)?;
                let l = label.entry(v.actor.0).or_default();
                if l.family.is_empty() {
                    l.family.clone_from(&v.attacker);
                }
                match v.msg {
                    Some(m) if v.changed_bytes_on_air => {
                        l.msgs.insert(m);
                    }
                    Some(_) => {}
                    None => {
                        let w = l.window.get_or_insert((v.t, v.t));
                        w.0 = w.0.min(v.t);
                        w.1 = w.1.max(v.t);
                    }
                }
            }
            "sec.cert" => {
                if let Ok(v) = serde_json::from_slice::<SecCertView>(&r.json)
                    && let Some(d) = v.digest
                {
                    certs.entry(v.node.0).or_default().push((v.t, d));
                }
            }
            _ => {}
        }
    }
    for v in certs.values_mut() {
        v.sort_by_key(|(t, _)| *t);
    }
    // An actor with byte-changing actions that named messages and none that did not still
    // needs its window for the fallback claim; it is the span of its labelled messages.
    let window_of = |actor: u32| -> Option<(u64, u64)> {
        let l = label.get(&actor)?;
        let mut w = l.window;
        for m in &l.msgs {
            if let Some(tx) = tx_of_msg.get(m) {
                let e = w.get_or_insert((tx.generated, tx.generated));
                e.0 = e.0.min(tx.generated);
                e.1 = e.1.max(tx.generated);
            }
        }
        w
    };
    let windows: BTreeMap<u32, (u64, u64)> = label
        .keys()
        .filter_map(|a| window_of(*a).map(|w| (*a, w)))
        .collect();

    let mut out = VeremiExport {
        codebook: opts.codebook,
        slice_s: opts.slice_s,
        ..VeremiExport::default()
    };
    let mut truth_seen: BTreeSet<u64> = BTreeSet::new();

    for r in records.iter().filter(|r| r.channel == "phy.rx") {
        let v: PhyRxView = view(r)?;
        // VeReMi logs what a receiver *received*. A frame that was lost never reached the
        // application, so it is not a trace line; the net-trace profile is where losses
        // live.
        if v.outcome != RxOutcome::Ok {
            continue;
        }
        let Some(msg) = v.msg else { continue };
        let tx = tx_of_msg.get(&msg);
        let (sender_node, generated) = match tx {
            Some(t) => (t.node, t.generated),
            // No transmission record: fall back to the transmitter the reception names and
            // to the reception instant, rather than dropping the line.
            None => (v.tx.map_or(0, |n| n.0), v.t_start),
        };
        let sender_actor = node_actor.get(&sender_node).copied().unwrap_or(sender_node);
        // The pseudonym the frame was signed with, as `node.tx` recorded it; else the
        // `sec.cert` binding in force (keyed by node); else a stable per-device stand-in.
        let pseudonym = tx
            .and_then(|t| t.pseudonym.clone())
            .unwrap_or_else(|| pseudonym_for(&certs, sender_node, v.t_start));
        let truth = truth_at
            .range((sender_actor, 0)..=(sender_actor, v.t_start))
            .next_back()
            .map(|(_, k)| k);
        let attacking = label
            .get(&sender_actor)
            .is_some_and(|l| l.labels(msg, generated));
        let claim = claimed(
            truth,
            tx.and_then(|t| t.content.as_ref()),
            windows
                .get(&sender_actor)
                .is_some_and(|(a, b)| v.t_start >= *a && v.t_start <= *b),
        );

        out.by_receiver
            .entry(u64::from(v.rx.0))
            .or_default()
            .push(VeremiReception {
                receiver: u64::from(v.rx.0),
                rcv_time: secs(v.t_end),
                send_time: secs(generated),
                sender: pseudonym_int(&pseudonym),
                sender_pseudo: pseudonym.clone(),
                message_id: msg,
                rssi: v.rssi_dbm.unwrap_or(0.0),
                ..claim
            });

        if truth_seen.insert(msg) {
            let k = truth.map(true_state).unwrap_or_default();
            out.truth.push(VeremiTruth {
                send_time: secs(generated),
                sender: i64::from(sender_actor),
                sender_pseudo: pseudonym,
                message_id: msg,
                pos: k.pos,
                spd: k.spd,
                acl: k.acl,
                hed: k.hed,
                attacker_type: if attacking {
                    label
                        .get(&sender_actor)
                        .map_or(0, |l| opts.codebook.code(&l.family))
                } else {
                    0
                },
                ..VeremiTruth::default()
            });
        }
    }

    // The receivers' own position reports (VeReMi's `type: 2`), and each receiver's own
    // attacker code for its file name.
    let receivers: Vec<u64> = out.by_receiver.keys().copied().collect();
    for rx in &receivers {
        let Ok(node) = u32::try_from(*rx) else {
            continue;
        };
        let actor = node_actor.get(&node).copied().unwrap_or(node);
        let own: Vec<OwnPosition> = truth_at
            .range((actor, 0)..=(actor, u64::MAX))
            .map(|((_, t), k)| OwnPosition {
                t: secs(*t),
                ..true_state(k)
            })
            .collect();
        if !own.is_empty() {
            out.own_positions.insert(*rx, own);
        }
    }
    for rx in out.by_receiver.keys() {
        let code = u32::try_from(*rx)
            .ok()
            .map(|n| node_actor.get(&n).copied().unwrap_or(n))
            .and_then(|a| label.get(&a))
            .map_or(0, |l| opts.codebook.code(&l.family));
        if code != 0 {
            out.receiver_attacker_type.insert(*rx, code);
        }
    }

    for rows in out.by_receiver.values_mut() {
        rows.sort_by(|a, b| {
            a.rcv_time
                .total_cmp(&b.rcv_time)
                .then_with(|| a.message_id.cmp(&b.message_id))
        });
    }
    out.truth.sort_by_key(|t| t.message_id);
    out.quantise();
    Ok(out)
}

/// The true state as VeReMi vectors: velocity and acceleration along the heading.
fn true_state(k: &v2xw_metrics::channels::GtKinematicsView) -> OwnPosition {
    let hed = unit(k.heading_rad.unwrap_or(0.0));
    OwnPosition {
        t: 0.0,
        pos: [k.x_m, k.y_m, k.z_m.unwrap_or(0.0)],
        spd: along(k.speed_mps, hed),
        acl: along(k.acc_mps2.unwrap_or(0.0), hed),
        hed,
        ..OwnPosition::default()
    }
}

/// What a receiver saw: the claim that went on the air.
///
/// With `node.tx` content, the claimed position, speed and heading are the ones the
/// engine handed every receiver's detectors (an attacker's are falsified), and the noise
/// and acceleration are the BSM's own `PositionalAccuracy` and `accelSet.long`. Without
/// it — a recording older than the content join — the true state stands in, offset by
/// the 25 m the emissions tables use while the sender was attacking, so the two exports
/// of such a run tell one story.
fn claimed(
    truth: Option<&v2xw_metrics::channels::GtKinematicsView>,
    content: Option<&v2xw_metrics::channels::MsgContentView>,
    attacking: bool,
) -> VeremiReception {
    if let Some(c) = content
        && let (Some(x), Some(y)) = (c.claimed_x_m, c.claimed_y_m)
    {
        let hed = unit(c.claimed_heading_rad.unwrap_or(0.0));
        let z = truth.and_then(|k| k.z_m).unwrap_or(0.0);
        return VeremiReception {
            pos: [x, y, z],
            pos_noise: ellipse_noise(
                c.pos_semi_major_m,
                c.pos_semi_minor_m,
                c.pos_orientation_deg,
            ),
            spd: along(c.claimed_speed_mps.unwrap_or(0.0), hed),
            acl: along(c.accel_long_mps2.unwrap_or(0.0), hed),
            hed,
            ..VeremiReception::default()
        };
    }
    let Some(k) = truth else {
        return VeremiReception::default();
    };
    let s = true_state(k);
    let offset = if attacking { 25.0 } else { 0.0 };
    VeremiReception {
        pos: [s.pos[0] + offset, s.pos[1], s.pos[2]],
        spd: s.spd,
        acl: s.acl,
        hed: s.hed,
        ..VeremiReception::default()
    }
}

/// The pseudonym a device was using at `at`, from the `sec.cert` bindings in the recording
/// (the latest at or before `at`; of several at one instant, the last recorded).
///
/// With no binding recorded, a stable per-device pseudonym keeps the trace joinable. That
/// is weaker than the real thing — it means a trace from a run with no credential layer
/// cannot exercise pseudonym unlinkability — and it is the honest fallback, because the
/// alternatives are to drop the line or to put the device id in the `sender` field.
fn pseudonym_for(certs: &BTreeMap<u32, Vec<(u64, String)>>, actor: u32, at: u64) -> String {
    certs
        .get(&actor)
        .and_then(|v| {
            let n = v.partition_point(|(t, _)| *t <= at);
            n.checked_sub(1).map(|i| v[i].1.clone())
        })
        .unwrap_or_else(|| {
            let hex =
                v2xw_core::hash::sha256_hex(format!("v2xw-unbound-pseudonym|{actor}").as_bytes());
            hex[..16].to_string()
        })
}

fn v3(v: &[f64; 3]) -> serde_json::Value {
    serde_json::json!([v[0], v[1], v[2]])
}

fn kin_fields(m: &mut serde_json::Map<String, serde_json::Value>, k: [&[f64; 3]; 8]) {
    for (name, v) in [
        "pos",
        "pos_noise",
        "spd",
        "spd_noise",
        "acl",
        "acl_noise",
        "hed",
        "hed_noise",
    ]
    .into_iter()
    .zip(k)
    {
        m.insert(name.to_string(), v3(v));
    }
}

fn reception_json(r: &VeremiReception) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert("type".into(), 3.into());
    m.insert("rcvTime".into(), r.rcv_time.into());
    m.insert("sendTime".into(), r.send_time.into());
    m.insert("sender".into(), r.sender.into());
    m.insert("senderPseudo".into(), r.sender_pseudo.clone().into());
    m.insert("messageID".into(), r.message_id.into());
    m.insert("RSSI".into(), r.rssi.into());
    kin_fields(
        &mut m,
        [
            &r.pos,
            &r.pos_noise,
            &r.spd,
            &r.spd_noise,
            &r.acl,
            &r.acl_noise,
            &r.hed,
            &r.hed_noise,
        ],
    );
    serde_json::Value::Object(m)
}

fn own_json(o: &OwnPosition) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert("type".into(), 2.into());
    m.insert("rcvTime".into(), o.t.into());
    kin_fields(
        &mut m,
        [
            &o.pos,
            &o.pos_noise,
            &o.spd,
            &o.spd_noise,
            &o.acl,
            &o.acl_noise,
            &o.hed,
            &o.hed_noise,
        ],
    );
    serde_json::Value::Object(m)
}

fn truth_json(t: &VeremiTruth, line_type: i64) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert("type".into(), line_type.into());
    m.insert("sendTime".into(), t.send_time.into());
    m.insert("sender".into(), t.sender.into());
    m.insert("senderPseudo".into(), t.sender_pseudo.clone().into());
    m.insert("messageID".into(), t.message_id.into());
    m.insert("attackerType".into(), t.attacker_type.into());
    kin_fields(
        &mut m,
        [
            &t.pos,
            &t.pos_noise,
            &t.spd,
            &t.spd_noise,
            &t.acl,
            &t.acl_noise,
            &t.hed,
            &t.hed_noise,
        ],
    );
    serde_json::Value::Object(m)
}

/// The slice an instant falls in.
fn slice_of(t: f64, slice_s: f64) -> u64 {
    if t <= 0.0 || slice_s <= 0.0 {
        0
    } else {
        (t / slice_s).floor() as u64
    }
}

fn write_lines(
    path: PathBuf,
    lines: &[serde_json::Value],
    channel: &str,
    out: &mut Vec<ExportedFile>,
) -> Result<()> {
    let mut text = String::new();
    for v in lines {
        text.push_str(&pyjson::canonical_line(v));
    }
    std::fs::write(&path, text.as_bytes()).map_err(|e| RecordError::io(&path, e))?;
    out.push(ExportedFile {
        path,
        format: ExportFormat::Jsonl,
        channel: Some(channel.to_string()),
        rows: lines.len(),
        bytes: text.len() as u64,
        schema: None,
    });
    Ok(())
}

/// One receiver's lines in time order: its own position reports interleaved with what it
/// received, which is how a detector consuming the trace expects to see them.
fn receiver_lines(export: &VeremiExport, receiver: u64) -> Vec<(f64, serde_json::Value)> {
    let mut lines: Vec<(f64, serde_json::Value)> = export
        .by_receiver
        .get(&receiver)
        .into_iter()
        .flatten()
        .map(|r| (r.rcv_time, reception_json(r)))
        .collect();
    for o in export.own_positions.get(&receiver).into_iter().flatten() {
        lines.push((o.t, own_json(o)));
    }
    lines.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| pyjson::canonical(&a.1).cmp(&pyjson::canonical(&b.1)))
    });
    lines
}

/// Writes a VeReMi-shaped export into `dir`, in the layout of its codebook.
///
/// [`Codebook::Veremi1`]: `traceJSON-<receiver>.json` per receiver and
/// `GroundTruthJSONlog.json` (truth as `type: 3` lines). [`Codebook::VeremiExt`]:
/// `traceJSON-<rx>-<rx>-A<code>-<t0>-<slice>.json` per receiver and slice (the receiver
/// node id stands for both VeReMi's vehicle and module ids; `<t0>` is the whole second of
/// the file's first line; `<code>` is the receiver's own attacker code) and
/// `traceGroundTruthJSON-<slice>.json` (truth as `type: 4`), sliced by `slice_s`. Both
/// write `VEREMI-NOTE.txt` carrying [`VEREMI_COMPATIBILITY_NOTE`]. Lines are the legacy
/// canonical JSON encoding, so the files are reproducible byte for byte.
///
/// # Errors
/// [`RecordError::Io`] if a file cannot be written.
pub fn write(dir: impl AsRef<Path>, export: &VeremiExport) -> Result<Vec<ExportedFile>> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir).map_err(|e| RecordError::io(dir, e))?;
    let mut out = Vec::new();
    let mut export = export.clone();
    export.quantise();
    let mut truth = export.truth.clone();
    truth.sort_by_key(|t| t.message_id);

    let receivers: BTreeSet<u64> = export
        .by_receiver
        .keys()
        .chain(export.own_positions.keys())
        .copied()
        .collect();
    match export.codebook {
        Codebook::Veremi1 => {
            for receiver in &receivers {
                let lines: Vec<_> = receiver_lines(&export, *receiver)
                    .into_iter()
                    .map(|(_, v)| v)
                    .collect();
                write_lines(
                    dir.join(format!("traceJSON-{receiver}.json")),
                    &lines,
                    "phy.rx",
                    &mut out,
                )?;
            }
            let lines: Vec<_> = truth.iter().map(|t| truth_json(t, 3)).collect();
            write_lines(
                dir.join("GroundTruthJSONlog.json"),
                &lines,
                "gt.kinematics",
                &mut out,
            )?;
        }
        Codebook::VeremiExt => {
            for receiver in &receivers {
                let code = export
                    .receiver_attacker_type
                    .get(receiver)
                    .copied()
                    .unwrap_or(0);
                let mut by_slice: BTreeMap<u64, Vec<(f64, serde_json::Value)>> = BTreeMap::new();
                for (t, v) in receiver_lines(&export, *receiver) {
                    by_slice
                        .entry(slice_of(t, export.slice_s))
                        .or_default()
                        .push((t, v));
                }
                for (slice, lines) in by_slice {
                    let t0 = lines.first().map_or(0, |(t, _)| t.max(0.0).floor() as u64);
                    let lines: Vec<_> = lines.into_iter().map(|(_, v)| v).collect();
                    write_lines(
                        dir.join(format!(
                            "traceJSON-{receiver}-{receiver}-A{code}-{t0}-{slice}.json"
                        )),
                        &lines,
                        "phy.rx",
                        &mut out,
                    )?;
                }
            }
            let mut by_slice: BTreeMap<u64, Vec<serde_json::Value>> = BTreeMap::new();
            for t in &truth {
                by_slice
                    .entry(slice_of(t.send_time, export.slice_s))
                    .or_default()
                    .push(truth_json(t, 4));
            }
            for (slice, lines) in by_slice {
                write_lines(
                    dir.join(format!("traceGroundTruthJSON-{slice}.json")),
                    &lines,
                    "gt.kinematics",
                    &mut out,
                )?;
            }
        }
    }

    let note_path = dir.join("VEREMI-NOTE.txt");
    std::fs::write(&note_path, VEREMI_COMPATIBILITY_NOTE.as_bytes())
        .map_err(|e| RecordError::io(&note_path, e))?;
    out.push(ExportedFile {
        path: note_path,
        format: ExportFormat::Json,
        channel: None,
        rows: 0,
        bytes: VEREMI_COMPATIBILITY_NOTE.len() as u64,
        schema: None,
    });
    Ok(out)
}

fn num(v: &serde_json::Value, key: &str, file: &str) -> Result<f64> {
    v.get(key)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| {
            RecordError::malformed("veremi", format!("{file}: a line has no number `{key}`"))
        })
}

fn int(v: &serde_json::Value, key: &str, file: &str) -> Result<i64> {
    v.get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            RecordError::malformed("veremi", format!("{file}: a line has no integer `{key}`"))
        })
}

fn vec3(v: &serde_json::Value, key: &str, file: &str) -> Result<[f64; 3]> {
    let a = v.get(key).and_then(serde_json::Value::as_array);
    match a.map(|a| {
        a.iter()
            .filter_map(serde_json::Value::as_f64)
            .collect::<Vec<_>>()
    }) {
        Some(x) if x.len() == 3 => Ok([x[0], x[1], x[2]]),
        _ => Err(RecordError::malformed(
            "veremi",
            format!("{file}: `{key}` is not a three-number array"),
        )),
    }
}

/// The eight kinematic arrays of a line, in the struct order.
fn kin(v: &serde_json::Value, file: &str) -> Result<[[f64; 3]; 8]> {
    Ok([
        vec3(v, "pos", file)?,
        vec3(v, "pos_noise", file)?,
        vec3(v, "spd", file)?,
        vec3(v, "spd_noise", file)?,
        vec3(v, "acl", file)?,
        vec3(v, "acl_noise", file)?,
        vec3(v, "hed", file)?,
        vec3(v, "hed_noise", file)?,
    ])
}

/// Reads a directory [`write`] wrote back into an export — the round trip a consumer's
/// reader performs, field for field. The layout is the codebook's; `slice_s` is not in the
/// files and is taken from the caller.
///
/// # Errors
/// [`RecordError::Io`] if the directory or a file cannot be read, and
/// [`RecordError::Malformed`] for a file name or a line that does not fit the layout.
pub fn read_dir(dir: impl AsRef<Path>, codebook: Codebook, slice_s: f64) -> Result<VeremiExport> {
    let dir = dir.as_ref();
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| RecordError::io(dir, e))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    let mut out = VeremiExport {
        codebook,
        slice_s,
        ..VeremiExport::default()
    };
    let bad_name = |n: &str| {
        RecordError::malformed(
            "veremi",
            format!("{n} is not a {} file name", codebook.id()),
        )
    };
    for name in &names {
        let path = dir.join(name);
        let is_truth = match codebook {
            Codebook::Veremi1 => name == "GroundTruthJSONlog.json",
            Codebook::VeremiExt => name.starts_with("traceGroundTruthJSON-"),
        };
        if is_truth {
            for v in crate::export::jsonl::read(&path)? {
                let k = kin(&v, name)?;
                out.truth.push(VeremiTruth {
                    send_time: num(&v, "sendTime", name)?,
                    sender: int(&v, "sender", name)?,
                    sender_pseudo: v["senderPseudo"].as_str().unwrap_or_default().to_string(),
                    message_id: int(&v, "messageID", name)?.unsigned_abs(),
                    pos: k[0],
                    pos_noise: k[1],
                    spd: k[2],
                    spd_noise: k[3],
                    acl: k[4],
                    acl_noise: k[5],
                    hed: k[6],
                    hed_noise: k[7],
                    attacker_type: int(&v, "attackerType", name)?,
                });
            }
            continue;
        }
        let Some(stem) = name
            .strip_prefix("traceJSON-")
            .and_then(|s| s.strip_suffix(".json"))
        else {
            continue;
        };
        let parts: Vec<&str> = stem.split('-').collect();
        let receiver: u64 = match (codebook, parts.as_slice()) {
            (Codebook::Veremi1, [rx]) => rx.parse().map_err(|_| bad_name(name))?,
            (Codebook::VeremiExt, [rx, _module, code, _t0, _slice]) => {
                let code: i64 = code
                    .strip_prefix('A')
                    .and_then(|c| c.parse().ok())
                    .ok_or_else(|| bad_name(name))?;
                let rx: u64 = rx.parse().map_err(|_| bad_name(name))?;
                if code != 0 {
                    out.receiver_attacker_type.insert(rx, code);
                }
                rx
            }
            _ => return Err(bad_name(name)),
        };
        for v in crate::export::jsonl::read(&path)? {
            let k = kin(&v, name)?;
            match int(&v, "type", name)? {
                2 => out
                    .own_positions
                    .entry(receiver)
                    .or_default()
                    .push(OwnPosition {
                        t: num(&v, "rcvTime", name)?,
                        pos: k[0],
                        pos_noise: k[1],
                        spd: k[2],
                        spd_noise: k[3],
                        acl: k[4],
                        acl_noise: k[5],
                        hed: k[6],
                        hed_noise: k[7],
                    }),
                3 => out
                    .by_receiver
                    .entry(receiver)
                    .or_default()
                    .push(VeremiReception {
                        receiver,
                        rcv_time: num(&v, "rcvTime", name)?,
                        send_time: num(&v, "sendTime", name)?,
                        sender: int(&v, "sender", name)?,
                        sender_pseudo: v["senderPseudo"].as_str().unwrap_or_default().to_string(),
                        message_id: int(&v, "messageID", name)?.unsigned_abs(),
                        pos: k[0],
                        pos_noise: k[1],
                        spd: k[2],
                        spd_noise: k[3],
                        acl: k[4],
                        acl_noise: k[5],
                        hed: k[6],
                        hed_noise: k[7],
                        rssi: num(&v, "RSSI", name)?,
                    }),
                other => {
                    return Err(RecordError::malformed(
                        "veremi",
                        format!("{name}: a receiver trace holds type {other}"),
                    ));
                }
            }
        }
    }
    for rows in out.by_receiver.values_mut() {
        rows.sort_by(|a, b| {
            a.rcv_time
                .total_cmp(&b.rcv_time)
                .then_with(|| a.message_id.cmp(&b.message_id))
        });
    }
    for rows in out.own_positions.values_mut() {
        rows.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    out.truth.sort_by_key(|t| t.message_id);
    Ok(out)
}

/// Lints the receiver-visible files [`write`] wrote: no forbidden key, and no true
/// identity of the ground-truth file as a value (the leakage linter's whole-dataset
/// check, [`super::leakage::lint_dataset`]).
///
/// # Errors
/// [`RecordError::Io`] if a written file cannot be read back.
pub fn lint(
    files: &[ExportedFile],
    export: &VeremiExport,
) -> Result<super::leakage::LeakageReport> {
    let mut visible: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    for f in files {
        let name = f
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with("traceJSON-") {
            visible.insert(name, crate::export::jsonl::read(&f.path)?);
        }
    }
    let identities: BTreeSet<String> = export.truth.iter().map(|t| t.sender.to_string()).collect();
    Ok(super::leakage::lint_dataset(&visible, &identities))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reception() -> VeremiReception {
        VeremiReception {
            receiver: 1,
            rcv_time: 10.0,
            send_time: 9.998,
            sender: pseudonym_int("aabbccdd"),
            sender_pseudo: "aabbccdd".to_string(),
            message_id: 7,
            pos: [100.5, 200.25, 0.0],
            spd: [12.0, 0.0, 0.0],
            hed: [1.0, 0.0, 0.0],
            rssi: -72.5,
            ..VeremiReception::default()
        }
    }

    #[test]
    fn the_receiver_visible_line_uses_veremi_field_names() {
        let v = reception_json(&reception());
        for key in [
            "type",
            "rcvTime",
            "sendTime",
            "sender",
            "senderPseudo",
            "messageID",
            "pos",
            "pos_noise",
            "spd",
            "spd_noise",
            "acl",
            "acl_noise",
            "hed",
            "hed_noise",
            "RSSI",
        ] {
            assert!(v.get(key).is_some(), "VeReMi field {key} is missing");
        }
        assert_eq!(v["type"], 3);
    }

    #[test]
    fn the_receiver_visible_line_carries_no_real_identity() {
        // The whole deviation, as a test: `sender` is a function of the pseudonym and of
        // nothing else, so two pseudonyms of one device do not share it.
        let a = pseudonym_int("pseudonym-a");
        let b = pseudonym_int("pseudonym-b");
        assert_ne!(a, b);
        assert_eq!(a, pseudonym_int("pseudonym-a"), "stable across calls");
        assert!(
            a >= 0 && a <= i64::from(i32::MAX),
            "fits a VeReMi int32 column"
        );
        // …and the linter agrees the line is clean.
        let v = reception_json(&reception());
        let report = crate::dataset::leakage::lint_rows("traceJSON-1.json", &[v]);
        assert!(report.is_clean(), "{}", report.summary());
    }

    #[test]
    fn the_ground_truth_log_is_where_the_real_identity_lives() {
        let t = VeremiTruth {
            send_time: 9.998,
            sender: 42,
            sender_pseudo: "aabbccdd".to_string(),
            message_id: 7,
            pos: [100.0, 200.0, 0.0],
            spd: [12.0, 0.0, 0.0],
            attacker_type: 2,
            ..VeremiTruth::default()
        };
        let v = truth_json(&t, 3);
        assert_eq!(v["attackerType"], 2);
        assert_eq!(v["sender"], 42);
        assert_eq!(truth_json(&t, 4)["type"], 4);
    }

    #[test]
    fn an_unmapped_attack_family_is_not_squeezed_into_a_veremi_code() {
        assert_eq!(attacker_type_code("none"), 0);
        assert_eq!(attacker_type_code("ConstPosOffset"), 2);
        assert_eq!(
            attacker_type_code("umbrella-threshold-pq-replay"),
            1024,
            "an unnamed family is reported as unnamed, not mislabelled as VeReMi's #2"
        );
    }

    /// WP-03's accept line: VeReMi 1.0's 16 is EventualStop, and the engine has no
    /// const-speed attack VeReMi 1.0 names, so a const-speed family is unnamed. The old
    /// table had these the other way round (16 const-speed, 32 eventual-stop).
    #[test]
    fn veremi_1_writes_16_for_eventual_stop_and_1024_for_const_speed() {
        let c = Codebook::Veremi1;
        assert_eq!(c.code("EventualStop"), 16);
        assert_eq!(c.code("threat/attacker/legacy/EventualStop"), 16);
        assert_eq!(c.code("eventual-stop"), 16);
        assert_eq!(c.code("const-speed"), UNNAMED_CODE);
        assert_eq!(c.code("ConstSpeed"), UNNAMED_CODE);
        assert_eq!(c.code("ConstPos"), 1);
        assert_eq!(
            c.code("RandomPos"),
            8,
            "the engine's RandomPos is an offset attack"
        );
        assert_eq!(c.code("DataReplay"), UNNAMED_CODE);
    }

    #[test]
    fn veremi_ext_uses_the_f2md_enum_only_where_the_behaviour_matches() {
        let c = Codebook::VeremiExt;
        assert_eq!(c.code(""), 0);
        assert_eq!(c.code("ConstPos"), 1);
        assert_eq!(c.code("ConstPosOffset"), 2);
        assert_eq!(c.code("RandomPos"), 4);
        assert_eq!(c.code("ConstSpeedOffset"), 6);
        assert_eq!(c.code("RandomSpeed"), 7);
        assert_eq!(c.code("EventualStop"), 9);
        assert_eq!(c.code("DelayedMessages"), 12);
        assert_eq!(c.code("DoS"), 13);
        assert_eq!(c.code("Sybil"), 16);
        assert_eq!(c.code("GhostVehicles"), 16);
        // Same name, different attack (f2md.md §4.1): left unnamed.
        for f in [
            "DataReplay",
            "Disruptive",
            "DoSRandom",
            "RelayReplay",
            "Teleport",
        ] {
            assert_eq!(c.code(f), UNNAMED_CODE, "{f}");
        }
    }

    #[test]
    fn the_options_name_the_codebook_and_refuse_what_they_do_not_know() {
        let o = VeremiOptions::from_opts(&serde_json::json!({})).expect("defaults");
        assert_eq!(o, VeremiOptions::default());
        assert_eq!(o.codebook, Codebook::VeremiExt);
        assert_eq!(o.slice_s, 3600.0);
        let o =
            VeremiOptions::from_opts(&serde_json::json!({"codebook": "veremi-1", "slice_s": 60}))
                .expect("parses");
        assert_eq!(o.codebook, Codebook::Veremi1);
        assert_eq!(o.slice_s, 60.0);
        for bad in [
            serde_json::json!({"codebook": "veremi-2"}),
            serde_json::json!({"slice_s": 0}),
            serde_json::json!({"codbook": "veremi-1"}),
        ] {
            assert!(VeremiOptions::from_opts(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_confidence_ellipse_projects_onto_the_axes() {
        assert_eq!(ellipse_noise(None, None, None), [0.0; 3]);
        assert_eq!(ellipse_noise(Some(2.0), Some(1.0), None), [2.0, 2.0, 0.0]);
        // Major axis due north: 1 m east, 2 m north.
        let n = ellipse_noise(Some(2.0), Some(1.0), Some(0.0));
        assert!(
            (n[0] - 1.0).abs() < 1e-12 && (n[1] - 2.0).abs() < 1e-12,
            "{n:?}"
        );
        // Due east: the other way round.
        let e = ellipse_noise(Some(2.0), Some(1.0), Some(90.0));
        assert!(
            (e[0] - 2.0).abs() < 1e-12 && (e[1] - 1.0).abs() < 1e-12,
            "{e:?}"
        );
    }

    #[test]
    fn every_float_is_quantised_before_it_is_written() {
        let mut e = VeremiExport::default();
        let mut r = reception();
        r.pos[0] = 100.123_456_789;
        r.rssi = -72.567_89;
        r.rcv_time = 10.000_4;
        r.hed = [0.707_106_781_2, 0.707_106_781_2, 0.0];
        e.by_receiver.insert(1, vec![r]);
        e.quantise();
        let r = &e.by_receiver[&1][0];
        assert_eq!(r.pos[0], 100.123);
        assert_eq!(r.rssi, -72.57);
        assert_eq!(r.rcv_time, 10.0);
        assert_eq!(r.hed[0], 0.707_107);
        assert!(v2xw_core::math::is_on_grid(r.pos[0], Q_METRES));
        assert!(v2xw_core::math::is_on_grid(r.rssi, Q_DB));
    }

    #[test]
    fn the_note_states_the_deviation_rather_than_leaving_it_to_be_discovered() {
        assert!(VEREMI_COMPATIBILITY_NOTE.contains("senderRealId"));
        assert!(VEREMI_COMPATIBILITY_NOTE.contains("will score lower here"));
        assert!(VEREMI_COMPATIBILITY_NOTE.contains("veremi-ext"));
    }
}
