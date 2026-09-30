//! Each application warning labelled against ground truth: true, false, or missed.
//!
//! The node decides on what it heard; this module decides, with the engine's knowledge
//! of where everyone truly is, whether the node was right. The two run the **same**
//! decision functions (`v2xw_node::apps`): the node over the claims it received, this
//! module over the true states of the same two vehicles. Their disagreement is the
//! communication's effect on safety — a lost BSM becomes a missed or late warning, a
//! ghost vehicle a false one — which is what 07-threats-and-detection.md §5 says the
//! engine must measure because no published experiment gives it.
//!
//! # The rules
//!
//! * **Truth episode.** For every equipped ego and every equipped subject within
//!   [`TRUTH_RANGE_M`], each mobility step, the application's decision function is run
//!   over their true states. A run of consecutive steps where it fires is one episode.
//! * **True warning.** An `issue` whose (ego, subject, application) had a truth episode
//!   open, or one that ended within [`MATCH_WINDOW`] before it, or that opens within
//!   [`MATCH_WINDOW`] after it (a predictive warning).
//! * **False warning.** An `issue` with no such episode.
//! * **Missed.** A truth episode that lasted at least [`MIN_EPISODE`] and ended with no
//!   `issue` from the ego about that subject for that application.
//! * **Lead time.** For a true warning: the true time to collision left at the issue
//!   instant (FCW, IMA, LTA, PCW), and the delay from the episode's start to the issue.
//!
//! `bsw` and `lcw` are advisory and are neither matched nor counted as missed (a car
//! passing through a blind spot for 300 ms is not a missed warning anyone would miss);
//! `rlvw` is not labelled here, since its truth needs the ego's signal state, which this
//! module does not reconstruct. Both limits are stated in the run report.

use std::collections::BTreeMap;

use serde::Serialize;
use v2xw_core::ctx::{Record, Visibility};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::kinematics::Kinematics;
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime};
use v2xw_node::apps::{self, AppParams, Track};

/// How far apart two vehicles may be for a truth episode to be looked for, metres.
pub const TRUTH_RANGE_M: f64 = 150.0;

/// How close in time an issue and a truth episode must be to match.
pub const MATCH_WINDOW: Duration = Duration::from_secs(1);

/// The shortest truth episode that counts as missed when no warning came.
pub const MIN_EPISODE: Duration = Duration::from_millis(300);

/// The applications labelled against truth.
pub const LABELLED: [&str; 5] = ["fcw", "eebl", "ima", "lta", "pcw"];

/// `app.outcome` — one warning, or one missed warning, labelled against ground truth
/// (GT: it names both vehicles' nodes and reads their true states).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppOutcome {
    /// When the warning issued, or when the missed episode began.
    pub t: SimTime,
    /// The ego node.
    pub node: NodeId,
    /// The application.
    pub app: String,
    /// The subject's node, when it could be identified.
    pub subject: Option<NodeId>,
    /// `true`, `false` or `missed`.
    pub outcome: &'static str,
    /// The true time to collision at the issue, seconds, where the application has one.
    pub ttc_truth_s: Option<f64>,
    /// From the truth episode's start to the issue, seconds; negative when the warning
    /// came before the true condition (a predictive warning).
    pub delay_s: Option<f64>,
}

impl Record for AppOutcome {
    const CHANNEL: &'static str = "app.outcome";
    const VISIBILITY: Visibility = Visibility::NodeAndGt;
}

/// One equipped vehicle's true state, as the labeller reads it.
#[derive(Debug, Clone, Copy)]
pub struct TruthState {
    /// Its true kinematics.
    pub k: Kinematics,
    /// Body length, metres.
    pub length_m: f64,
    /// Body width, metres.
    pub width_m: f64,
    /// Whether it is a pedestrian or cyclist.
    pub vru: bool,
    /// When it is about to turn left: the junction and the distance to it.
    pub left_turn: Option<(Vec3, f64)>,
}

/// Per-application counts, for the run report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct AppTally {
    /// Warnings issued.
    pub issued: u64,
    /// Issued and true.
    pub true_warnings: u64,
    /// Issued and false.
    pub false_warnings: u64,
    /// True episodes with no warning.
    pub missed: u64,
    /// Sum of the true TTC at issue over true warnings, milliseconds.
    pub lead_ms_sum: u64,
    /// How many true warnings carried a TTC.
    pub lead_n: u64,
}

#[derive(Debug, Clone, Copy)]
struct Episode {
    start: SimTime,
    last: SimTime,
    warned: bool,
}

type Key = (NodeId, NodeId, &'static str);

/// The labeller.
#[derive(Debug, Default)]
pub struct AppTruth {
    params: Option<AppParams>,
    /// Pseudonym digest (lowercase hex) → the node that signs under it.
    signer_node: BTreeMap<String, NodeId>,
    /// Open truth episodes, by (ego, subject, app).
    open: BTreeMap<Key, Episode>,
    /// Issues not yet matched: (ego, subject, app) → issue instant and true TTC.
    pending: BTreeMap<Key, (SimTime, Option<f64>)>,
    /// Recently closed episodes, by when they ended.
    closed: BTreeMap<Key, SimTime>,
    tallies: BTreeMap<String, AppTally>,
    out: Vec<AppOutcome>,
}

impl AppTruth {
    /// A labeller for runs whose vehicles use `params`.
    pub fn new(params: AppParams) -> Self {
        Self {
            params: Some(params),
            ..Self::default()
        }
    }

    /// Whether it labels anything.
    pub fn enabled(&self) -> bool {
        self.params.is_some()
    }

    /// Learns which node signs under a pseudonym.
    pub fn note_signer(&mut self, digest: &[u8], node: NodeId) {
        if self.params.is_none() {
            return;
        }
        let mut s = String::with_capacity(16);
        for b in digest.iter().take(8) {
            s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
            s.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
        }
        self.signer_node.insert(s, node);
    }

    /// The per-application counts so far.
    pub fn tallies(&self) -> &BTreeMap<String, AppTally> {
        &self.tallies
    }

    /// Takes one `app.warning` record's JSON.
    pub fn on_warning(&mut self, json: &[u8], states: &BTreeMap<NodeId, TruthState>) {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(json) else {
            return;
        };
        if v["kind"].as_str() != Some("issue") {
            return;
        }
        let (Some(t), Some(node), Some(app)) = (
            v["t"].as_u64(),
            v["node"].as_u64().and_then(|n| u32::try_from(n).ok()),
            v["app"].as_str(),
        ) else {
            return;
        };
        let node = NodeId::new(node);
        self.tallies.entry(app.to_string()).or_default().issued += 1;
        let Some(app) = LABELLED.iter().copied().find(|a| *a == app) else {
            return;
        };
        let subject = v["subject"]
            .as_str()
            .and_then(|s| self.signer_node.get(s).copied());
        let Some(subject) = subject else {
            // A subject nobody signs under: a ghost, which no truth episode can match.
            let tally = self.tallies.entry(app.to_string()).or_default();
            tally.false_warnings += 1;
            self.out.push(AppOutcome {
                t,
                node,
                app: app.to_string(),
                subject: None,
                outcome: "false",
                ttc_truth_s: None,
                delay_s: None,
            });
            return;
        };
        let ttc = match (states.get(&node), states.get(&subject), self.params) {
            (Some(e), Some(s), Some(p)) => {
                truth(app, e, s, &p).and_then(|s| s.ttc_s.is_finite().then_some(s.ttc_s))
            }
            _ => None,
        };
        self.pending.insert((node, subject, app), (t, ttc));
    }

    /// One mobility step at `now`: open and close truth episodes and settle what can be
    /// settled. `states` holds every equipped vehicle's true state; `near` lists, per
    /// ego, the subjects within [`TRUTH_RANGE_M`].
    pub fn step(
        &mut self,
        now: SimTime,
        states: &BTreeMap<NodeId, TruthState>,
        near: &BTreeMap<NodeId, Vec<NodeId>>,
    ) {
        let Some(p) = self.params else {
            return;
        };
        for (ego, subjects) in near {
            let Some(e) = states.get(ego) else { continue };
            for subject in subjects {
                let Some(s) = states.get(subject) else {
                    continue;
                };
                for app in LABELLED {
                    if truth(app, e, s, &p).is_some() {
                        self.open
                            .entry((*ego, *subject, app))
                            .and_modify(|ep| ep.last = now)
                            .or_insert(Episode {
                                start: now,
                                last: now,
                                warned: false,
                            });
                    }
                }
            }
        }
        let window = MATCH_WINDOW.as_nanos();
        let pending = core::mem::take(&mut self.pending);
        for (key, (t, ttc)) in pending {
            let matched = if let Some(ep) = self.open.get_mut(&key) {
                ep.warned = true;
                Some(ep.start)
            } else {
                self.closed
                    .get(&key)
                    .filter(|end| t.saturating_sub(**end) <= window)
                    .copied()
            };
            let tally = self.tallies.entry(key.2.to_string()).or_default();
            match matched {
                Some(start) => {
                    tally.true_warnings += 1;
                    if let Some(x) = ttc {
                        tally.lead_ms_sum += (x * 1_000.0).round().max(0.0) as u64;
                        tally.lead_n += 1;
                    }
                    self.out.push(AppOutcome {
                        t,
                        node: key.0,
                        app: key.2.to_string(),
                        subject: Some(key.1),
                        outcome: "true",
                        ttc_truth_s: ttc.map(math::q3),
                        delay_s: Some(math::q3((t as f64 - start as f64) * 1e-9)),
                    });
                }
                None if now.saturating_sub(t) >= window => {
                    tally.false_warnings += 1;
                    self.out.push(AppOutcome {
                        t,
                        node: key.0,
                        app: key.2.to_string(),
                        subject: Some(key.1),
                        outcome: "false",
                        ttc_truth_s: None,
                        delay_s: None,
                    });
                }
                None => {
                    // Not yet: a predictive warning may precede its truth episode.
                    self.pending.insert(key, (t, ttc));
                }
            }
        }
        let ended: Vec<Key> = self
            .open
            .iter()
            .filter(|(_, ep)| ep.last < now)
            .map(|(k, _)| *k)
            .collect();
        for key in ended {
            let Some(ep) = self.open.remove(&key) else {
                continue;
            };
            self.closed.insert(key, ep.last);
            if !ep.warned && ep.last.saturating_sub(ep.start) >= MIN_EPISODE.as_nanos() {
                self.tallies.entry(key.2.to_string()).or_default().missed += 1;
                self.out.push(AppOutcome {
                    t: ep.start,
                    node: key.0,
                    app: key.2.to_string(),
                    subject: Some(key.1),
                    outcome: "missed",
                    ttc_truth_s: None,
                    delay_s: None,
                });
            }
        }
        self.closed
            .retain(|_, end| now.saturating_sub(*end) <= 4 * window);
    }

    /// The outcomes settled since the last call.
    pub fn drain(&mut self) -> Vec<AppOutcome> {
        core::mem::take(&mut self.out)
    }
}

/// A true state as an application track.
pub fn track_of(k: &Kinematics, length_m: f64, width_m: f64, vru: bool) -> Track {
    let (s, c) = math::sin_cos(k.heading_rad);
    let a_long = k.acc.x * c + k.acc.y * s;
    Track {
        pos: k.pos,
        heading_rad: k.heading_rad,
        speed_mps: math::hypot(k.vel.x, k.vel.y),
        accel_mps2: a_long,
        yaw_rate_rad_s: k.yaw_rate_rad_s,
        length_m,
        width_m,
        hard_braking: a_long <= -v2xw_node::safety::EEBL_DECEL_THRESHOLD_MPS2,
        vru,
    }
}

/// The application `app`'s decision over true states — the same function the node runs
/// over what it heard. LTA reads the ego's true intention (its route's next movement).
fn truth(
    app: &str,
    e: &TruthState,
    s: &TruthState,
    p: &AppParams,
) -> Option<v2xw_node::safety::Surrogates> {
    if e.vru {
        return None;
    }
    let ego = track_of(&e.k, e.length_m, e.width_m, false);
    let sub = track_of(&s.k, s.length_m, s.width_m, s.vru);
    match app {
        "fcw" if p.fcw => apps::fcw(&ego, &sub, &p.fcw_params),
        "eebl" if p.eebl => apps::eebl(&ego, &sub, &p.eebl_params),
        "ima" if p.ima => apps::ima(&ego, &sub, &p.ima_params),
        "pcw" if p.pcw => apps::pcw(&ego, &sub, &p.pcw_params),
        "lta" if p.lta => e
            .left_turn
            .and_then(|(j, d)| apps::lta(&ego, j, d, &sub, &p.lta_params)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::geom::Dims;

    fn state(x: f64, v: f64, a: f64) -> TruthState {
        TruthState {
            k: Kinematics {
                t: 0,
                pos: Vec3::new(x, 0.0, 0.0),
                vel: Vec3::new(v, 0.0, 0.0),
                acc: Vec3::new(a, 0.0, 0.0),
                heading_rad: 0.0,
                yaw_rate_rad_s: 0.0,
                lane: None,
                dims: Dims::CAR,
            },
            length_m: 4.5,
            width_m: 1.8,
            vru: false,
            left_turn: None,
        }
    }

    fn warning(t: u64, node: u32, app: &str, subject: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "t": t, "node": node, "app": app, "subject": subject, "kind": "issue"
        }))
        .expect("json")
    }

    /// A warning during a truth episode is true; one with no episode is false; an episode
    /// with no warning is missed.
    #[test]
    fn warnings_are_labelled_against_the_truth() {
        let mut truth = AppTruth::new(AppParams::default());
        truth.note_signer(&[1, 2, 3, 4, 5, 6, 7, 8], NodeId::new(2));
        let mut states = BTreeMap::new();
        states.insert(NodeId::new(1), state(0.0, 13.0, 0.0));
        states.insert(NodeId::new(2), state(25.0, 5.0, -4.0)); // braking hard ahead
        let near = BTreeMap::from([(NodeId::new(1), vec![NodeId::new(2)])]);
        let ms = 1_000_000;
        truth.step(0, &states, &near);
        truth.on_warning(&warning(50 * ms, 1, "fcw", "0102030405060708"), &states);
        for k in 1..=4 {
            truth.step(k * 100 * ms, &states, &near);
        }
        let out = truth.drain();
        assert!(out.iter().any(|o| o.app == "fcw" && o.outcome == "true"), "{out:?}");
        // EEBL fired in truth for 400 ms but no EEBL warning came: once the episode ends,
        // it is missed.
        states.insert(NodeId::new(2), state(25.0, 5.0, 0.0));
        truth.step(500 * ms, &states, &near);
        truth.step(600 * ms, &states, &near);
        let out = truth.drain();
        assert!(out.iter().any(|o| o.app == "eebl" && o.outcome == "missed"), "{out:?}");
        // A warning about nobody's pseudonym is false at once.
        truth.on_warning(&warning(700 * ms, 1, "fcw", "ffffffffffffffff"), &states);
        let out = truth.drain();
        assert!(out.iter().any(|o| o.outcome == "false"), "{out:?}");
        let t = truth.tallies()["fcw"];
        assert_eq!((t.issued, t.true_warnings, t.false_warnings), (2, 1, 1));
    }
}
