//! The F2MD (VeReMi Extension) and VeReMi NextGen attack behaviours the legacy catalogue
//! does not render with the same semantics.
//!
//! # Why a module of its own
//!
//! [`crate::attack::AttackKind`] is asserted against the legacy Python source at test time,
//! so its names cannot grow. Several F2MD names *are* in it with a different behaviour —
//! the engine's `RandomPos` is F2MD's `RandomPosOffset`, its `DataReplay` replays its own
//! past state where F2MD replays a neighbour's — so the F2MD behaviours here carry names
//! that do not collide: [`F2mdKind::UniformRandomPos`] is F2MD `RandomPos`,
//! [`F2mdKind::NeighbourReplay`] is F2MD `DataReplay`. [`F2mdKind::f2md_code`] is the
//! F2MD `attackTypes` value each one matches, which the VeReMi exporter's codebook uses.
//!
//! # Sources, read for behaviour only (no code copied)
//!
//! * F2MD (Kamel et al., IEEE T-VT 69(6), 2020), `mdAttacks/MDAttack.cc` and
//!   `F2MDParameters`: `RandomPos`, `ConstSpeed`, `RandomSpeedOffset`, `DataReplay`,
//!   `DoSDisruptive`, `DataReplaySybil`, `DoSRandomSybil`, `DoSDisruptiveSybil`, and the
//!   defaults `MaxRandomSpeedOffset` 7 m/s, `ConstSpeed` 40 m/s, `DosMultipleFreq` 4,
//!   `ReplaySeqNum` 6, `SybilVehNumber` 5.
//! * VeReMi NextGen `docs/attackGenerator`: `feignedBraking` (×2–4 of the negated
//!   acceleration), `accelerationMultiplication` (×2–4), `positionMirroring` (the
//!   distance to the road edge × 2), `zeroSpeedReport`, and `dataReplay`'s 400 m victim
//!   radius, 5 s look-back and 4–8 messages per victim.
//!
//! # What each one edits
//!
//! Only the fields its definition names: the unit tests below assert, per kind, that every
//! other field of the [`Emission`] is untouched. Every edit is recorded as an
//! [`AttackAction`], whatever its size — the per-message label thresholds VeReMi NextGen
//! applies (acceleration > 0.25 m/s², speed > 1 m/s and moved > 1 m) are the label's to
//! apply, and the action carries the edit's magnitude so a consumer can apply them.
//!
//! # The flooding and Sybil variants
//!
//! The host sends one frame per generation, plus one per ghost
//! ([`Emission::ghosts`]). A DoS copy is therefore a ghost signed with the attacker's own
//! pseudonym, stamped `k × (generation interval / multiplier)` after the beacon, so the
//! receivers see a beacon stream at the multiplied rate. A Sybil copy is a ghost signed
//! with one of the *concurrent* pseudonyms the attacker holds (an SCMS issues several valid
//! at once), cycled message by message — never a fabricated certificate.

use crate::attack::{AttackAction, AttackFamily, Attacker, AttackerView, Emission, HonestClaim};
use crate::capability::{AttackSchedule, Capabilities, wrap_heading};
use crate::cards::{paper, ranged};
use crate::ctx::ThreatCtx;
use crate::obs::{ObservedKind, ObservedMessage};
use v2xw_core::card::{Family, ModelCard, Tier};
use v2xw_core::ids::NodeId;
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::NS_PER_S;

/// The model id this module's card carries.
pub const MODEL_ID: &str = "threat/attacker/f2md";

/// The scenario id prefix of this module's attacks.
pub const ID_PREFIX: &str = "threat/attacker/f2md/";

/// One F2MD / VeReMi NextGen behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum F2mdKind {
    /// VeReMi NextGen `feignedBraking`: the stated acceleration is the true one negated and
    /// multiplied.
    FeignedBraking,
    /// VeReMi NextGen `accelerationMultiplication`: the stated acceleration multiplied.
    AccelerationMultiplication,
    /// VeReMi NextGen `positionMirroring`: the position mirrored across the road edge to
    /// the vehicle's left, onto the opposite side of the road.
    PositionMirroring,
    /// VeReMi NextGen `zeroSpeedReport`: the stated speed is zero.
    ZeroSpeedReport,
    /// F2MD `RandomPos`: a position drawn uniformly over the playground every message.
    UniformRandomPos,
    /// F2MD `ConstSpeed`: one speed, drawn once, stated on every message.
    ConstSpeed,
    /// F2MD `RandomSpeedOffset`: a fresh bounded offset on the true speed every message.
    RandomSpeedOffset,
    /// F2MD `DataReplay`: a neighbour's latest claim, restated under the attacker's own
    /// pseudonym, a few messages per victim.
    NeighbourReplay,
    /// F2MD `DoSDisruptive`: the beacon rate multiplied, every message a random
    /// neighbour's latest claim.
    DoSDisruptive,
    /// F2MD `DataReplaySybil`: [`F2mdKind::NeighbourReplay`] from another held pseudonym,
    /// the pseudonym changing with the victim.
    DataReplaySybil,
    /// F2MD `DoSRandomSybil`: the rate multiplied, every copy random content under the
    /// next held pseudonym.
    DoSRandomSybil,
    /// F2MD `DoSDisruptiveSybil`: the rate multiplied, every copy a random neighbour's
    /// claim under the next held pseudonym.
    DoSDisruptiveSybil,
}

impl F2mdKind {
    /// Every kind, in catalogue order.
    pub const ALL: [F2mdKind; 12] = [
        F2mdKind::FeignedBraking,
        F2mdKind::AccelerationMultiplication,
        F2mdKind::PositionMirroring,
        F2mdKind::ZeroSpeedReport,
        F2mdKind::UniformRandomPos,
        F2mdKind::ConstSpeed,
        F2mdKind::RandomSpeedOffset,
        F2mdKind::NeighbourReplay,
        F2mdKind::DoSDisruptive,
        F2mdKind::DataReplaySybil,
        F2mdKind::DoSRandomSybil,
        F2mdKind::DoSDisruptiveSybil,
    ];

    /// The name a scenario carries after [`ID_PREFIX`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            F2mdKind::FeignedBraking => "FeignedBraking",
            F2mdKind::AccelerationMultiplication => "AccelerationMultiplication",
            F2mdKind::PositionMirroring => "PositionMirroring",
            F2mdKind::ZeroSpeedReport => "ZeroSpeedReport",
            F2mdKind::UniformRandomPos => "UniformRandomPos",
            F2mdKind::ConstSpeed => "ConstSpeed",
            F2mdKind::RandomSpeedOffset => "RandomSpeedOffset",
            F2mdKind::NeighbourReplay => "NeighbourReplay",
            F2mdKind::DoSDisruptive => "DoSDisruptive",
            F2mdKind::DataReplaySybil => "DataReplaySybil",
            F2mdKind::DoSRandomSybil => "DoSRandomSybil",
            F2mdKind::DoSDisruptiveSybil => "DoSDisruptiveSybil",
        }
    }

    /// Parses a name. `None` for anything not in [`F2mdKind::ALL`].
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        F2mdKind::ALL.into_iter().find(|k| k.as_str() == name)
    }

    /// The behavioural family.
    #[must_use]
    pub const fn family(self) -> AttackFamily {
        match self {
            F2mdKind::PositionMirroring | F2mdKind::UniformRandomPos => AttackFamily::Position,
            F2mdKind::FeignedBraking
            | F2mdKind::AccelerationMultiplication
            | F2mdKind::ZeroSpeedReport
            | F2mdKind::ConstSpeed
            | F2mdKind::RandomSpeedOffset => AttackFamily::Speed,
            F2mdKind::NeighbourReplay | F2mdKind::DoSDisruptive => AttackFamily::Timing,
            F2mdKind::DataReplaySybil | F2mdKind::DoSRandomSybil | F2mdKind::DoSDisruptiveSybil => {
                AttackFamily::Identity
            }
        }
    }

    /// The F2MD `attackTypes` value (VeReMi Extension `attackerType`) this behaviour
    /// matches; `None` for the VeReMi NextGen behaviours F2MD has no code for.
    #[must_use]
    pub const fn f2md_code(self) -> Option<u8> {
        match self {
            F2mdKind::UniformRandomPos => Some(3),
            F2mdKind::ConstSpeed => Some(5),
            F2mdKind::RandomSpeedOffset => Some(8),
            F2mdKind::NeighbourReplay => Some(11),
            F2mdKind::DoSDisruptive => Some(15),
            F2mdKind::DataReplaySybil => Some(17),
            F2mdKind::DoSRandomSybil => Some(18),
            F2mdKind::DoSDisruptiveSybil => Some(19),
            F2mdKind::FeignedBraking
            | F2mdKind::AccelerationMultiplication
            | F2mdKind::PositionMirroring
            | F2mdKind::ZeroSpeedReport => None,
        }
    }

    /// The upstream name and where it is defined.
    #[must_use]
    pub const fn upstream(self) -> &'static str {
        match self {
            F2mdKind::FeignedBraking => "VeReMi NextGen attackGenerator feignedBraking",
            F2mdKind::AccelerationMultiplication => {
                "VeReMi NextGen attackGenerator accelerationMultiplication"
            }
            F2mdKind::PositionMirroring => "VeReMi NextGen attackGenerator positionMirroring",
            F2mdKind::ZeroSpeedReport => "VeReMi NextGen attackGenerator zeroSpeedReport",
            F2mdKind::UniformRandomPos => "F2MD attackTypes::RandomPos (MDAttack.cc)",
            F2mdKind::ConstSpeed => "F2MD attackTypes::ConstSpeed (MDAttack.cc)",
            F2mdKind::RandomSpeedOffset => "F2MD attackTypes::RandomSpeedOffset (MDAttack.cc)",
            F2mdKind::NeighbourReplay => "F2MD attackTypes::DataReplay (MDAttack.cc)",
            F2mdKind::DoSDisruptive => "F2MD attackTypes::DoSDisruptive (MDAttack.cc)",
            F2mdKind::DataReplaySybil => "F2MD attackTypes::DataReplaySybil (MDAttack.cc)",
            F2mdKind::DoSRandomSybil => "F2MD attackTypes::DoSRandomSybil (MDAttack.cc)",
            F2mdKind::DoSDisruptiveSybil => "F2MD attackTypes::DoSDisruptiveSybil (MDAttack.cc)",
        }
    }

    /// Whether the behaviour floods (copies at a multiplied rate).
    #[must_use]
    pub const fn floods(self) -> bool {
        matches!(
            self,
            F2mdKind::DoSDisruptive | F2mdKind::DoSRandomSybil | F2mdKind::DoSDisruptiveSybil
        )
    }

    /// Whether the behaviour transmits from pseudonyms other than the active one.
    #[must_use]
    pub const fn is_sybil(self) -> bool {
        matches!(
            self,
            F2mdKind::DataReplaySybil | F2mdKind::DoSRandomSybil | F2mdKind::DoSDisruptiveSybil
        )
    }

    /// Whether the behaviour reads what the attacker received.
    #[must_use]
    pub const fn replays_neighbours(self) -> bool {
        matches!(
            self,
            F2mdKind::NeighbourReplay
                | F2mdKind::DoSDisruptive
                | F2mdKind::DataReplaySybil
                | F2mdKind::DoSDisruptiveSybil
        )
    }
}

impl core::fmt::Display for F2mdKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The parameters an F2MD behaviour reads.
#[derive(Debug, Clone, PartialEq)]
pub struct F2mdParams {
    /// Which behaviour.
    pub kind: F2mdKind,
    /// The acceleration multiplier of `FeignedBraking` / `AccelerationMultiplication`
    /// (VeReMi NextGen: 2–4).
    pub accel_multiplier: f64,
    /// The distance from the vehicle to the road edge it is mirrored across, m
    /// (`PositionMirroring`; the claim moves twice this to the left).
    pub mirror_edge_distance_m: f64,
    /// The playground `[x_min, y_min, x_max, y_max]`, world-local metres
    /// (`UniformRandomPos`, `DoSRandomSybil`). The host fills it from the map.
    pub playground_m: [f64; 4],
    /// The largest speed `ConstSpeed` and the random content draw, m/s (F2MD 40).
    pub max_speed_mps: f64,
    /// `RandomSpeedOffset`'s bound, m/s (F2MD `MaxRandomSpeedOffset` 7).
    pub speed_offset_max_mps: f64,
    /// Messages replayed from one victim before another is chosen (F2MD `ReplaySeqNum` 6).
    pub replay_seq: u32,
    /// How far back a victim's message may be, s (VeReMi NextGen 5).
    pub replay_window_s: f64,
    /// How far a victim may be, m (VeReMi NextGen 400).
    pub replay_radius_m: f64,
    /// The flooding rate multiplier (F2MD `DosMultipleFreq` 4).
    pub dos_multiplier: u32,
    /// The beacon generation interval the flood multiplies, s (J2945/1: 0.1).
    pub generation_interval_s: f64,
    /// How many concurrent pseudonyms a Sybil variant cycles through (F2MD
    /// `SybilVehNumber` 5), bounded by what the attacker holds.
    pub sybil_pseudonyms: u32,
}

impl F2mdParams {
    /// The defaults for `kind`.
    #[must_use]
    pub fn new(kind: F2mdKind) -> Self {
        Self {
            kind,
            accel_multiplier: 3.0,
            mirror_edge_distance_m: 1.83,
            playground_m: [0.0; 4],
            max_speed_mps: 40.0,
            speed_offset_max_mps: 7.0,
            replay_seq: 6,
            replay_window_s: 5.0,
            replay_radius_m: 400.0,
            dos_multiplier: 4,
            generation_interval_s: 0.1,
            sybil_pseudonyms: 5,
        }
    }

    fn playground_set(&self) -> bool {
        let [x0, y0, x1, y1] = self.playground_m;
        x1 > x0 && y1 > y0
    }
}

#[derive(Debug, Default)]
struct State {
    prev: Option<HonestClaim>,
    const_speed: Option<f64>,
    target: Option<[u8; 8]>,
    replayed: u32,
    cursor: usize,
}

/// An attacker running one [`F2mdKind`].
pub struct F2mdAttacker {
    card: ModelCard,
    params: F2mdParams,
    capabilities: Capabilities,
    schedule: AttackSchedule,
    node: NodeId,
    state: State,
}

impl F2mdAttacker {
    /// An attacker at `node` running `params`.
    #[must_use]
    pub fn new(
        node: NodeId,
        params: F2mdParams,
        capabilities: Capabilities,
        schedule: AttackSchedule,
    ) -> Self {
        Self {
            card: card(&params),
            params,
            capabilities,
            schedule,
            node,
            state: State::default(),
        }
    }

    /// The behaviour.
    #[must_use]
    pub fn kind(&self) -> F2mdKind {
        self.params.kind
    }

    /// The parameters.
    #[must_use]
    pub fn params(&self) -> &F2mdParams {
        &self.params
    }

    /// The beacons heard from other signers within the window and radius, latest per
    /// signer, sorted by signer so a choice among them is reproducible.
    fn victims<'a>(&self, view: &'a AttackerView<'_>) -> Vec<&'a ObservedMessage> {
        let t = view.believed_time;
        let window = (self.params.replay_window_s * NS_PER_S as f64) as u64;
        let mut latest: std::collections::BTreeMap<[u8; 8], &ObservedMessage> =
            std::collections::BTreeMap::new();
        for m in view.own_rx {
            if !matches!(m.kind, ObservedKind::Beacon)
                || view.own_credentials.contains(&m.signer)
                || m.received_at > t
                || t - m.received_at > window
                || math::hypot(
                    m.claimed_x_m - view.own_belief.x_m,
                    m.claimed_y_m - view.own_belief.y_m,
                ) > self.params.replay_radius_m
            {
                continue;
            }
            let e = latest.entry(m.signer).or_insert(m);
            if m.received_at >= e.received_at {
                *e = m;
            }
        }
        latest.into_values().collect()
    }

    fn pick(&self, ctx: &mut dyn ThreatCtx, n: usize) -> usize {
        let mut rng = ctx.rng(RngDomain::Attack, EntityRef::Node(self.node));
        rng.below(n as u64) as usize
    }

    /// The victim a replay restates this message: the current one while it is still heard
    /// and has replays left, otherwise a fresh draw.
    fn replay_victim(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        view: &AttackerView<'_>,
    ) -> Option<ObservedMessage> {
        let victims = self.victims(view);
        if victims.is_empty() {
            self.state.target = None;
            return None;
        }
        let keep = self.state.target.and_then(|s| {
            (self.state.replayed < self.params.replay_seq.max(1))
                .then(|| victims.iter().find(|m| m.signer == s))
                .flatten()
        });
        let m = match keep {
            Some(m) => (*m).clone(),
            None => {
                let m = victims[self.pick(ctx, victims.len())].clone();
                self.state.target = Some(m.signer);
                self.state.replayed = 0;
                m
            }
        };
        self.state.replayed += 1;
        Some(m)
    }

    fn random_victim(
        &self,
        ctx: &mut dyn ThreatCtx,
        view: &AttackerView<'_>,
    ) -> Option<ObservedMessage> {
        let victims = self.victims(view);
        (!victims.is_empty()).then(|| victims[self.pick(ctx, victims.len())].clone())
    }

    fn random_content(&self, ctx: &mut dyn ThreatCtx) -> (f64, f64, f64, f64) {
        let [x0, y0, x1, y1] = self.params.playground_m;
        let mut rng = ctx.rng(RngDomain::Attack, EntityRef::Node(self.node));
        let x = rng.uniform(x0, x1);
        let y = rng.uniform(y0, y1);
        let s = rng.uniform(0.0, self.params.max_speed_mps);
        let h = rng.uniform(-core::f64::consts::PI, core::f64::consts::PI);
        (x, y, s, h)
    }

    /// The pseudonyms a Sybil variant cycles through: the held ones other than the active.
    fn sybil_signers(&self, view: &AttackerView<'_>, own: [u8; 8]) -> Vec<[u8; 8]> {
        view.own_credentials
            .iter()
            .copied()
            .filter(|c| *c != own)
            .take(self.params.sybil_pseudonyms as usize)
            .collect()
    }

    fn next_signer(&mut self, signers: &[[u8; 8]]) -> Option<[u8; 8]> {
        if signers.is_empty() {
            return None;
        }
        let s = signers[self.state.cursor % signers.len()];
        self.state.cursor = self.state.cursor.wrapping_add(1);
        Some(s)
    }
}

/// Writes a restated claim onto `e` and returns the fields it changed.
fn restate(e: &mut Emission, x: f64, y: f64, s: f64, h: f64) -> Vec<String> {
    let mut f = Vec::new();
    if x != e.x_m || y != e.y_m {
        f.push("position".to_string());
    }
    if s != e.speed_mps {
        f.push("speed".to_string());
    }
    if h != e.heading_rad {
        f.push("heading".to_string());
    }
    e.x_m = x;
    e.y_m = y;
    e.speed_mps = s;
    e.heading_rad = h;
    f
}

fn copy_of(out: &Emission) -> Emission {
    let mut g = out.clone();
    g.ghosts = Vec::new();
    g.events = Vec::new();
    g
}

impl Model for F2mdAttacker {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Attacker for F2mdAttacker {
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn schedule(&self) -> &AttackSchedule {
        &self.schedule
    }

    #[allow(clippy::too_many_lines)]
    fn act(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        view: &AttackerView<'_>,
        out: &mut Emission,
    ) -> Vec<AttackAction> {
        let t = view.believed_time;
        let honest = view.honest;
        let prev = self.state.prev.replace(honest);
        if !self
            .schedule
            .active_at(t, view.own_belief.x_m, view.own_belief.y_m)
        {
            return Vec::new();
        }
        let p = self.params.clone();
        let mut actions = Vec::new();
        let mut fields: Vec<String> = Vec::new();
        let mut magnitude = 0.0;
        match p.kind {
            F2mdKind::FeignedBraking | F2mdKind::AccelerationMultiplication => {
                if let Some(a) = out.accel_long_mps2 {
                    let k = p.accel_multiplier;
                    let lie = if p.kind == F2mdKind::FeignedBraking {
                        -k * a
                    } else {
                        k * a
                    };
                    if lie != a {
                        out.accel_long_mps2 = Some(lie);
                        fields.push("acceleration".to_string());
                        magnitude = (lie - a).abs();
                    }
                }
            }
            F2mdKind::PositionMirroring => {
                let shift = 2.0 * p.mirror_edge_distance_m;
                let (s, c) = math::sin_cos(honest.heading_rad + core::f64::consts::FRAC_PI_2);
                out.x_m = honest.x_m + shift * c;
                out.y_m = honest.y_m + shift * s;
                fields.push("position".to_string());
                magnitude = shift;
            }
            F2mdKind::ZeroSpeedReport => {
                if out.speed_mps != 0.0 {
                    magnitude = out.speed_mps;
                    out.speed_mps = 0.0;
                    fields.push("speed".to_string());
                }
                // NextGen labels it only when the vehicle moved; the magnitude says so.
                let _ = prev;
            }
            F2mdKind::UniformRandomPos => {
                if p.playground_set() {
                    let (x, y, _, _) = self.random_content(ctx);
                    magnitude = math::hypot(x - honest.x_m, y - honest.y_m);
                    out.x_m = x;
                    out.y_m = y;
                    fields.push("position".to_string());
                }
            }
            F2mdKind::ConstSpeed => {
                let v = match self.state.const_speed {
                    Some(v) => v,
                    None => {
                        let mut rng = ctx.rng(RngDomain::Attack, EntityRef::Node(self.node));
                        let v = rng.uniform(0.0, p.max_speed_mps);
                        drop(rng);
                        self.state.const_speed = Some(v);
                        v
                    }
                };
                magnitude = (v - out.speed_mps).abs();
                out.speed_mps = v;
                fields.push("speed".to_string());
            }
            F2mdKind::RandomSpeedOffset => {
                let mut rng = ctx.rng(RngDomain::Attack, EntityRef::Node(self.node));
                let d = rng.uniform(-p.speed_offset_max_mps, p.speed_offset_max_mps);
                drop(rng);
                let v = (honest.speed_mps + d).max(0.0);
                magnitude = (v - out.speed_mps).abs();
                out.speed_mps = v;
                fields.push("speed".to_string());
            }
            F2mdKind::NeighbourReplay => {
                if let Some(m) = self.replay_victim(ctx, view) {
                    fields = restate(
                        out,
                        m.claimed_x_m,
                        m.claimed_y_m,
                        m.claimed_speed_mps,
                        m.claimed_heading_rad,
                    );
                    magnitude = math::hypot(m.claimed_x_m - honest.x_m, m.claimed_y_m - honest.y_m);
                }
            }
            F2mdKind::DoSDisruptive => {
                if let Some(m) = self.random_victim(ctx, view) {
                    fields = restate(
                        out,
                        m.claimed_x_m,
                        m.claimed_y_m,
                        m.claimed_speed_mps,
                        m.claimed_heading_rad,
                    );
                    magnitude = math::hypot(m.claimed_x_m - honest.x_m, m.claimed_y_m - honest.y_m);
                }
            }
            F2mdKind::DataReplaySybil | F2mdKind::DoSRandomSybil | F2mdKind::DoSDisruptiveSybil => {
                // The active pseudonym's own beacon stays honest; the lies go out under
                // the other held pseudonyms (below).
            }
        }
        if !fields.is_empty() {
            fields.sort_unstable();
            actions.push(AttackAction::FalsifyOutgoing {
                fields: fields.clone(),
                magnitude,
            });
        }

        // The copies: DoS copies under the own pseudonym, Sybil copies under the others.
        let base = copy_of(out);
        let step_ns =
            (p.generation_interval_s / f64::from(p.dos_multiplier.max(1)) * NS_PER_S as f64) as u64;
        match p.kind {
            F2mdKind::DoSDisruptive => {
                let n = p.dos_multiplier.max(1) - 1;
                for k in 1..=n {
                    let mut g = base.clone();
                    if let Some(m) = self.random_victim(ctx, view) {
                        restate(
                            &mut g,
                            m.claimed_x_m,
                            m.claimed_y_m,
                            m.claimed_speed_mps,
                            m.claimed_heading_rad,
                        );
                    }
                    g.generation_time = out.generation_time + u64::from(k) * step_ns;
                    out.ghosts.push(g);
                }
                if n > 0 {
                    actions.push(AttackAction::FalsifyOutgoing {
                        fields: vec!["repetitions".to_string()],
                        magnitude: f64::from(p.dos_multiplier),
                    });
                }
            }
            F2mdKind::DataReplaySybil => {
                let signers = self.sybil_signers(view, out.signer);
                let before = self.state.target;
                if let Some(m) = self.replay_victim(ctx, view) {
                    // A new victim moves the attacker onto its next pseudonym.
                    if before != self.state.target || self.state.replayed == 1 {
                        self.state.cursor = self.state.cursor.wrapping_add(1);
                    }
                    if let Some(signer) =
                        (!signers.is_empty()).then(|| signers[self.state.cursor % signers.len()])
                    {
                        let mut g = base.clone();
                        g.signer = signer;
                        restate(
                            &mut g,
                            m.claimed_x_m,
                            m.claimed_y_m,
                            m.claimed_speed_mps,
                            m.claimed_heading_rad,
                        );
                        out.ghosts.push(g);
                        actions.push(AttackAction::Ghost { signer });
                    }
                }
            }
            F2mdKind::DoSRandomSybil | F2mdKind::DoSDisruptiveSybil => {
                let signers = self.sybil_signers(view, out.signer);
                for k in 1..=p.dos_multiplier.max(1) {
                    let Some(signer) = self.next_signer(&signers) else {
                        break;
                    };
                    let content = if p.kind == F2mdKind::DoSRandomSybil {
                        p.playground_set().then(|| self.random_content(ctx))
                    } else {
                        self.random_victim(ctx, view).map(|m| {
                            (
                                m.claimed_x_m,
                                m.claimed_y_m,
                                m.claimed_speed_mps,
                                m.claimed_heading_rad,
                            )
                        })
                    };
                    let Some((x, y, s, h)) = content else {
                        continue;
                    };
                    let mut g = base.clone();
                    g.signer = signer;
                    restate(&mut g, x, y, s, wrap_heading(h));
                    g.generation_time = out.generation_time + u64::from(k - 1) * step_ns;
                    out.ghosts.push(g);
                    actions.push(AttackAction::Ghost { signer });
                }
            }
            _ => {}
        }
        actions
    }
}

/// The model card for an [`F2mdAttacker`] rendering `params.kind`.
#[must_use]
pub fn card(params: &F2mdParams) -> ModelCard {
    use serde_json::json;
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Attacker,
        "1.0.0",
        format!(
            "{}: {}. Re-implemented from the published definition; edits only the fields it \
             names.",
            params.kind.as_str(),
            params.kind.upstream()
        ),
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    let ng = |what: &str| paper(&format!("VeReMi NextGen docs/attackGenerator: {what}"));
    let f2md = |what: &str| {
        paper(&format!(
            "F2MD (Kamel et al., IEEE T-VT 69(6), 2020) F2MDParameters: {what}"
        ))
    };
    let mut mirror = ranged(
        "mirror_edge_distance_m",
        "m",
        json!(params.mirror_edge_distance_m),
        json!(0.0),
        json!(20.0),
        ng("positionMirroring offset = distance to road edge x 2"),
    );
    mirror.calibration = Some(
        "NextGen measures the edge in SUMO; 1.83 m is half a 12 ft (3.66 m) US lane, so the \
         claim lands one lane to the left, in the opposing lane of a two-lane road"
            .to_string(),
    );
    card.parameters = vec![
        ranged(
            "accel_multiplier",
            "1",
            json!(params.accel_multiplier),
            json!(2.0),
            json!(4.0),
            ng("feigned_braking / accelerationMult 2.0 to 4.0"),
        ),
        mirror,
        ranged(
            "max_speed_mps",
            "m/s",
            json!(params.max_speed_mps),
            json!(0.0),
            json!(70.0),
            f2md("ConstSpeedX / MaxRandomSpeedX = 40"),
        ),
        ranged(
            "speed_offset_max_mps",
            "m/s",
            json!(params.speed_offset_max_mps),
            json!(1.0),
            json!(7.0),
            f2md("MaxRandomSpeedOffsetX = 7; NextGen randomSpeedOffset +-1 to +-7 m/s"),
        ),
        ranged(
            "replay_seq",
            "messages",
            json!(params.replay_seq),
            json!(1),
            json!(20),
            f2md("ReplaySeqNum = 6; NextGen max_replay_seq 4 to 8"),
        ),
        ranged(
            "replay_window_s",
            "s",
            json!(params.replay_window_s),
            json!(0.5),
            json!(10.0),
            ng("dataReplay time window 5 s"),
        ),
        ranged(
            "replay_radius_m",
            "m",
            json!(params.replay_radius_m),
            json!(10.0),
            json!(1000.0),
            ng("dataReplay detection radius 400 m"),
        ),
        ranged(
            "dos_multiplier",
            "1",
            json!(params.dos_multiplier),
            json!(2),
            json!(10),
            f2md("DosMultipleFreq = 4; NextGen dosAttack amount 2 to 4"),
        ),
        ranged(
            "sybil_pseudonyms",
            "pseudonyms",
            json!(params.sybil_pseudonyms),
            json!(1),
            json!(20),
            f2md("SybilVehNumber = 5, bounded by the pseudonyms the SCMS issued concurrently"),
        ),
    ];
    card.sources = vec![
        paper(
            "Kamel, Wolf, van der Heijden, Kaiser, Urien, Kargl, VeReMi Extension, ICC 2020 \
             (attackerType 0-19)",
        ),
        paper("VeReMi NextGen, docs/attackGenerator (attack types and labelling)"),
    ];
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::CollectingCtx;
    use crate::obs::{SelfBelief, StationType, VerificationState};
    use v2xw_core::time::SimTime;

    const OWN: [u8; 8] = [1; 8];
    const HELD: [[u8; 8]; 3] = [[1; 8], [2; 8], [3; 8]];

    fn attacker(kind: F2mdKind) -> F2mdAttacker {
        let mut p = F2mdParams::new(kind);
        p.playground_m = [0.0, 0.0, 1000.0, 800.0];
        F2mdAttacker::new(
            NodeId::new(7),
            p,
            Capabilities::insider(3),
            AttackSchedule {
                from: 10 * NS_PER_S,
                to: 100 * NS_PER_S,
                ..AttackSchedule::default()
            },
        )
    }

    fn heard(signer: u8, x: f64, at: SimTime) -> ObservedMessage {
        ObservedMessage {
            signer: [signer; 8],
            kind: ObservedKind::Beacon,
            received_at: at,
            claimed_generation_time: at,
            claimed_x_m: x,
            claimed_y_m: 50.0,
            claimed_speed_mps: 9.0,
            claimed_heading_rad: 1.0,
            claimed_pos_confidence_m: 2.0,
            repetitions: 1,
            cert_valid_from: 0,
            cert_valid_to: SimTime::MAX,
            station_type: StationType::Vehicle,
            bsm: v2xw_core::bsm_fields::BsmExtras::default(),
            verification: VerificationState::Valid,
        }
    }

    fn step(
        a: &mut F2mdAttacker,
        t: SimTime,
        rx: &[ObservedMessage],
    ) -> (Emission, Emission, Vec<AttackAction>) {
        let honest = HonestClaim {
            x_m: 100.0,
            y_m: 20.0,
            speed_mps: 15.0,
            heading_rad: 0.3,
        };
        let view = AttackerView {
            own_rx: rx,
            own_credentials: &HELD,
            crl_revocations_seen: None,
            own_belief: SelfBelief {
                node: NodeId::new(7),
                believed_time: t,
                x_m: 100.0,
                y_m: 20.0,
                radio_range_m: 300.0,
            },
            honest,
            believed_time: t,
        };
        let mut before = Emission::honest(OWN, honest, t, 0, u64::MAX);
        before.accel_long_mps2 = Some(1.2);
        let mut out = before.clone();
        let mut ctx = CollectingCtx::new(42);
        let actions = a.act(&mut ctx, &view, &mut out);
        (before, out, actions)
    }

    /// Every field of the beacon an emission could change, by name.
    fn changed(a: &Emission, b: &Emission) -> Vec<&'static str> {
        let mut v = Vec::new();
        if (a.x_m, a.y_m) != (b.x_m, b.y_m) {
            v.push("position");
        }
        if a.speed_mps != b.speed_mps {
            v.push("speed");
        }
        if a.heading_rad != b.heading_rad {
            v.push("heading");
        }
        if a.accel_long_mps2 != b.accel_long_mps2 {
            v.push("acceleration");
        }
        if a.generation_time != b.generation_time {
            v.push("generation_time");
        }
        if a.signer != b.signer
            || a.signature_valid != b.signature_valid
            || (a.cert_valid_from, a.cert_valid_to) != (b.cert_valid_from, b.cert_valid_to)
        {
            v.push("envelope");
        }
        if a.station_type != b.station_type
            || a.repetitions != b.repetitions
            || a.suppressed != b.suppressed
        {
            v.push("other");
        }
        v
    }

    fn rx(t: SimTime) -> Vec<ObservedMessage> {
        vec![
            heard(9, 140.0, t - NS_PER_S),
            heard(8, 160.0, t - NS_PER_S / 2),
        ]
    }

    #[test]
    fn each_kind_changes_exactly_the_fields_its_definition_names() {
        let t = 20 * NS_PER_S;
        let want: &[(F2mdKind, &[&str], usize)] = &[
            (F2mdKind::FeignedBraking, &["acceleration"], 0),
            (F2mdKind::AccelerationMultiplication, &["acceleration"], 0),
            (F2mdKind::PositionMirroring, &["position"], 0),
            (F2mdKind::ZeroSpeedReport, &["speed"], 0),
            (F2mdKind::UniformRandomPos, &["position"], 0),
            (F2mdKind::ConstSpeed, &["speed"], 0),
            (F2mdKind::RandomSpeedOffset, &["speed"], 0),
            (
                F2mdKind::NeighbourReplay,
                &["position", "speed", "heading"],
                0,
            ),
            (
                F2mdKind::DoSDisruptive,
                &["position", "speed", "heading"],
                3,
            ),
            (F2mdKind::DataReplaySybil, &[], 1),
            (F2mdKind::DoSRandomSybil, &[], 4),
            (F2mdKind::DoSDisruptiveSybil, &[], 4),
        ];
        for (kind, fields, ghosts) in want {
            let mut a = attacker(*kind);
            let rx = rx(t);
            let (before, out, actions) = step(&mut a, t, &rx);
            assert_eq!(changed(&before, &out), *fields, "{kind}: beacon fields");
            assert_eq!(out.ghosts.len(), *ghosts, "{kind}: copies");
            let recorded: Vec<String> = actions
                .iter()
                .filter_map(|x| match x {
                    AttackAction::FalsifyOutgoing { fields, .. } => Some(fields.clone()),
                    _ => None,
                })
                .flatten()
                .filter(|f| f != "repetitions")
                .collect();
            let mut want_sorted: Vec<String> = fields.iter().map(|s| (*s).to_string()).collect();
            want_sorted.sort_unstable();
            assert_eq!(recorded, want_sorted, "{kind}: recorded fields");
        }
    }

    #[test]
    fn the_acceleration_lies_are_negated_and_multiplied() {
        let t = 20 * NS_PER_S;
        let (_, out, _) = step(&mut attacker(F2mdKind::FeignedBraking), t, &[]);
        assert!((out.accel_long_mps2.unwrap() + 3.6).abs() < 1e-9);
        let (_, out, _) = step(&mut attacker(F2mdKind::AccelerationMultiplication), t, &[]);
        assert!((out.accel_long_mps2.unwrap() - 3.6).abs() < 1e-9);
    }

    #[test]
    fn mirroring_moves_the_claim_twice_the_edge_distance_to_the_left() {
        let (b, out, _) = step(
            &mut attacker(F2mdKind::PositionMirroring),
            20 * NS_PER_S,
            &[],
        );
        let d = math::hypot(out.x_m - b.x_m, out.y_m - b.y_m);
        assert!((d - 3.66).abs() < 1e-9);
        // Left of a heading of 0.3 rad: the cross product is positive.
        let (s, c) = math::sin_cos(0.3);
        assert!(c * (out.y_m - b.y_m) - s * (out.x_m - b.x_m) > 0.0);
    }

    #[test]
    fn the_random_position_is_anywhere_on_the_playground() {
        let mut a = attacker(F2mdKind::UniformRandomPos);
        let mut far = 0;
        for i in 0..200u64 {
            let (_, out, _) = step(&mut a, 20 * NS_PER_S + i * NS_PER_S / 10, &[]);
            assert!((0.0..=1000.0).contains(&out.x_m) && (0.0..=800.0).contains(&out.y_m));
            far += usize::from(math::hypot(out.x_m - 100.0, out.y_m - 20.0) > 300.0);
        }
        assert!(
            far > 100,
            "a uniform draw over the playground lands far: {far}"
        );
    }

    #[test]
    fn const_speed_is_drawn_once_and_the_offset_is_bounded() {
        let mut a = attacker(F2mdKind::ConstSpeed);
        let v: Vec<f64> = (0..20u64)
            .map(|i| {
                step(&mut a, 20 * NS_PER_S + i * NS_PER_S / 10, &[])
                    .1
                    .speed_mps
            })
            .collect();
        assert!(v.iter().all(|x| *x == v[0]) && (0.0..=40.0).contains(&v[0]));
        let mut a = attacker(F2mdKind::RandomSpeedOffset);
        for i in 0..50u64 {
            let s = step(&mut a, 20 * NS_PER_S + i * NS_PER_S / 10, &[])
                .1
                .speed_mps;
            assert!((8.0..=22.0).contains(&s));
        }
    }

    #[test]
    fn neighbour_replay_restates_one_victim_for_replay_seq_messages_then_moves_on() {
        let mut a = attacker(F2mdKind::NeighbourReplay);
        let mut targets = Vec::new();
        for i in 0..12u64 {
            let t = 20 * NS_PER_S + i * NS_PER_S / 10;
            let (_, out, _) = step(&mut a, t, &rx(t));
            assert!(out.x_m == 140.0 || out.x_m == 160.0);
            assert_eq!(out.signer, OWN, "under its own pseudonym");
            targets.push(out.x_m);
        }
        assert!(targets[..6].iter().all(|x| *x == targets[0]));
        // Out of range, out of window, or its own: nothing to replay, nothing changed.
        let t = 20 * NS_PER_S;
        let stale = vec![
            heard(9, 140.0, t - 6 * NS_PER_S),
            heard(8, 900.0, t),
            heard(1, 140.0, t),
        ];
        let (b, out, actions) = step(&mut attacker(F2mdKind::NeighbourReplay), t, &stale);
        assert!(changed(&b, &out).is_empty() && actions.is_empty());
    }

    #[test]
    fn the_floods_stamp_their_copies_at_the_multiplied_rate() {
        let t = 20 * NS_PER_S;
        let (_, out, _) = step(&mut attacker(F2mdKind::DoSDisruptive), t, &rx(t));
        let stamps: Vec<u64> = out.ghosts.iter().map(|g| g.generation_time - t).collect();
        assert_eq!(stamps, [25_000_000, 50_000_000, 75_000_000]);
        assert!(out.ghosts.iter().all(|g| g.signer == OWN));
    }

    #[test]
    fn the_sybil_copies_sign_only_with_other_held_pseudonyms() {
        let t = 20 * NS_PER_S;
        for kind in [
            F2mdKind::DataReplaySybil,
            F2mdKind::DoSRandomSybil,
            F2mdKind::DoSDisruptiveSybil,
        ] {
            let (_, out, actions) = step(&mut attacker(kind), t, &rx(t));
            assert!(!out.ghosts.is_empty());
            for g in &out.ghosts {
                assert_ne!(g.signer, OWN, "{kind}");
                assert!(HELD.contains(&g.signer), "{kind}: a held pseudonym");
            }
            let ghosts = actions
                .iter()
                .filter(|a| matches!(a, AttackAction::Ghost { .. }))
                .count();
            assert_eq!(ghosts, out.ghosts.len(), "{kind}: every copy recorded");
        }
    }

    #[test]
    fn nothing_changes_outside_the_window() {
        for kind in F2mdKind::ALL {
            let (b, out, actions) = step(&mut attacker(kind), 5 * NS_PER_S, &rx(5 * NS_PER_S));
            assert_eq!(b, out, "{kind}");
            assert!(actions.is_empty(), "{kind}");
        }
    }

    #[test]
    fn the_names_parse_back_and_the_cards_validate() {
        for k in F2mdKind::ALL {
            assert_eq!(F2mdKind::parse(k.as_str()), Some(k));
            card(&F2mdParams::new(k))
                .validate()
                .expect("card validates");
        }
        assert_eq!(F2mdKind::parse("RandomPos"), None);
    }
}
