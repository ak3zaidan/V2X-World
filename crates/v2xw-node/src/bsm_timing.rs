//! `node/bsm-timing/per-device`: how a real OBU's BSM timestamp relates to the GNSS fix
//! its position comes from (Paper 1 item B, research/synthesis/paper-plan-2026-10.md §1.4).
//!
//! On public CV-pilot BSMs (research/synthesis/check-realdata-and-vesnos.md §2.3) honest
//! vehicles trip SM-MBR RandomPosition because of the **timing** of the message, not its
//! position error: the reported position comes from one GNSS fix epoch and the timestamp
//! says another (an *epoch slip*), or the position is repeated from the previous message
//! while the vehicle moves (a *frozen repeat*), and the 1609.2 `generationTime` jitters
//! against `secMark`. Whether a device does this is a property of the device: IDs whose
//! `secMark` sits on the fix epoch ("locked") trip it about 0 times per ID-minute at speed,
//! IDs that do not ("free") 20–195 times.
//!
//! This module gives each vehicle OBU a **timing class** (keyed draw, its own RNG domain)
//! and applies the class to the **transmitted** BSM only:
//!
//! * fix epochs are every `fix_interval_ms` on the GNSS (simulation) timeline; the
//!   message's base epoch `E` is the latest one at or before the instant its belief is for;
//! * `secMark` is the timestamp epoch plus the class's `secmark_phase_ms` (drawn once per
//!   device, or once per message when the class says `phase_scope: message`);
//! * with `epoch_slip_prob` the position and the timestamp come from different epochs:
//!   a slip of `k < 0` reports the fix `|k|` epochs older than the timestamp says, a slip of
//!   `k > 0` stamps the timestamp `k` epochs older than the fix (both causal: the reported
//!   position is always a fix the receiver already had);
//! * with `frozen_repeat_prob` the previous message's position is sent again while moving;
//! * the 1609.2 `generationTime` is the device's own time plus a normal jitter of
//!   `header_jitter_ms.sd`.
//!
//! Ground truth, the mobility model and the GNSS model are untouched: the slip is between
//! the reported position and the reported time. The position of an earlier fix epoch is
//! the node's own belief carried back along its own believed velocity, which is exactly the
//! fix when the GNSS model samples on the fix grid (the engine's 100 ms mobility step).
//! It replaces the raw-vs-smoothed GNSS factor as the honest-trigger knob; the GNSS
//! presets stay as they are.
//!
//! Off unless `actors.vehicles.bsm_timing.enabled` is true. The calibration is the shared
//! `v2xw.bsm_timing.v1` file (`data/calibration/bsm-timing-cvpilot.yaml`, written by the
//! real-data pipeline); [`BsmTimingCalibration::provisional`] is the built-in stand-in,
//! marked provisional, taken from the two research checks.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use v2xw_core::belief::PositionEstimate;
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::Vec3;
use v2xw_core::rng::{RngDomain, RngStream};
use v2xw_core::time::SimTime;

/// The model id, which is also the name of its RNG domain.
pub const MODEL_ID: &str = "node/bsm-timing/per-device";

/// The calibration schema this module reads.
pub const SCHEMA: &str = "v2xw.bsm_timing.v1";

/// The RNG domain every draw of the model comes from: one stream per node.
#[must_use]
pub fn rng_domain() -> RngDomain {
    RngDomain::plugin(MODEL_ID)
}

/// What a BSM says beyond position, speed and heading that a receiver's SM-MBR checks
/// read: its `secMark` and its `accelSet` longitudinal acceleration.
pub use v2xw_core::bsm_fields::BsmExtras;

/// A distribution of `secMark` minus the fix epoch, milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "dist", rename_all = "lowercase")]
pub enum PhaseDist {
    /// Normal, `mean` and `sd` in ms.
    Normal {
        /// Mean, ms.
        mean: f64,
        /// Standard deviation, ms.
        sd: f64,
    },
    /// Uniform on `[lo, hi)`, ms.
    Uniform {
        /// Lower bound, ms.
        lo: f64,
        /// Upper bound, ms.
        hi: f64,
    },
    /// Empirical: `p` over `bins_ms`. With one more bin value than probabilities the
    /// values are bin **edges** and a draw is uniform inside its bin; with as many, they
    /// are point masses.
    Empirical {
        /// Bin edges or point values, ms, ascending.
        bins_ms: Vec<f64>,
        /// The probability of each bin or point; sums to 1.
        p: Vec<f64>,
    },
}

impl PhaseDist {
    /// One draw, ms. Consumes exactly two 64-bit draws whatever the distribution, so a
    /// change of distribution does not shift the device's later draws.
    fn draw(&self, rng: &mut RngStream) -> f64 {
        let n = rng.normal(0.0, 1.0);
        let u = rng.uniform(0.0, 1.0);
        match self {
            Self::Normal { mean, sd } => mean + sd * n,
            Self::Uniform { lo, hi } => lo + (hi - lo) * u,
            Self::Empirical { bins_ms, p } => {
                // The bin comes from `u`, the place inside it from Φ(n), a second uniform.
                let mut acc = 0.0;
                let mut i = p.len().saturating_sub(1);
                for (j, pj) in p.iter().enumerate() {
                    acc += pj;
                    if u < acc {
                        i = j;
                        break;
                    }
                }
                if bins_ms.len() == p.len() + 1 {
                    let inner = 0.5 * (1.0 + erf(n / core::f64::consts::SQRT_2));
                    bins_ms[i] + (bins_ms[i + 1] - bins_ms[i]) * inner.clamp(0.0, 1.0)
                } else {
                    bins_ms.get(i).copied().unwrap_or(0.0)
                }
            }
        }
    }

    fn check(&self, at: &str) -> Result<(), String> {
        let finite = |v: f64| v.is_finite();
        match self {
            Self::Normal { mean, sd } if !finite(*mean) || !finite(*sd) || *sd < 0.0 => {
                Err(format!("{at}: a normal phase needs a finite mean and an sd of 0 or more"))
            }
            Self::Uniform { lo, hi } if !finite(*lo) || !finite(*hi) || lo > hi => {
                Err(format!("{at}: a uniform phase needs finite lo <= hi"))
            }
            Self::Empirical { bins_ms, p } => {
                let sum: f64 = p.iter().sum();
                if p.is_empty()
                    || p.iter().any(|v| !v.is_finite() || *v < 0.0)
                    || (sum - 1.0).abs() > 1e-6
                {
                    return Err(format!("{at}: empirical p must be non-negative and sum to 1"));
                }
                if bins_ms.len() != p.len() && bins_ms.len() != p.len() + 1 {
                    return Err(format!(
                        "{at}: empirical bins_ms needs as many values as p (points) or one \
                         more (edges)"
                    ));
                }
                if bins_ms.iter().any(|v| !v.is_finite()) || bins_ms.windows(2).any(|w| w[0] > w[1])
                {
                    return Err(format!("{at}: empirical bins_ms must be finite and ascending"));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// The error function, Abramowitz and Stegun 7.1.26 (|error| < 1.5e-7): only the place of a
/// draw inside an empirical bin uses it.
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * v2xw_core::math::exp(-x * x);
    if x < 0.0 { -y } else { y }
}

/// Whether a device's `secMark` phase is drawn once per device or once per message.
///
/// A v2xw extension of the shared format (absent = `device`). On Tampa the residual's p50
/// is 0.019 m while 4.5 % of fast pairs exceed 1 m (check-realdata §2.2–2.3): the phase of
/// a free device is steady and its triggers are discrete epoch slips, which a per-message
/// phase draw would not reproduce.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhaseScope {
    /// One phase per device for the run.
    #[default]
    Device,
    /// A fresh phase on every message.
    Message,
}

/// The `header_jitter_ms` block.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Jitter {
    /// Standard deviation of `generationTime` minus the device's time, ms.
    pub sd: f64,
}

/// One device timing class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingClass {
    /// The class name (`locked`, `free`, ...).
    pub name: String,
    /// The share of devices in this class, 0..1; shares sum to 1.
    pub share: f64,
    /// `secMark` minus the fix epoch, ms.
    pub secmark_phase_ms: PhaseDist,
    /// Whether the phase is per device or per message (extension; default device).
    #[serde(default)]
    pub phase_scope: PhaseScope,
    /// Per-message P(position from a different fix epoch than the timestamp says).
    pub epoch_slip_prob: f64,
    /// The slip size in fix intervals, given a slip: `{k: p}`.
    #[serde(default, deserialize_with = "slip_map")]
    pub slip_epochs: BTreeMap<i32, f64>,
    /// Per-message P(position repeated from the previous message while moving).
    pub frozen_repeat_prob: f64,
    /// `generationTime` minus true time.
    #[serde(default)]
    pub header_jitter_ms: Jitter,
}

fn slip_map<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<i32, f64>, D::Error> {
    use serde::de::Error;
    // Through a YAML value, whose keys may be integers (`-1:`) or strings (`"-1":`).
    let m = match serde_yml::Value::deserialize(d)? {
        serde_yml::Value::Mapping(m) => m,
        serde_yml::Value::Null => return Ok(BTreeMap::new()),
        _ => return Err(D::Error::custom("slip_epochs must be a map of slip size to probability")),
    };
    let mut out = BTreeMap::new();
    for (k, v) in m {
        let key = k
            .as_i64()
            .or_else(|| k.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
            .and_then(|i| i32::try_from(i).ok())
            .ok_or_else(|| D::Error::custom(format!("slip_epochs key {k:?} is not an integer")))?;
        let p = v
            .as_f64()
            .ok_or_else(|| D::Error::custom(format!("slip_epochs[{key}] is not a number")))?;
        out.insert(key, p);
    }
    Ok(out)
}

/// A `v2xw.bsm_timing.v1` calibration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BsmTimingCalibration {
    /// `v2xw.bsm_timing.v1`.
    pub schema: String,
    /// Where the numbers came from (kept as written; not interpreted).
    #[serde(default)]
    pub provenance: serde_json::Value,
    /// True for the built-in stand-in values (extension; default false).
    #[serde(default)]
    pub provisional: bool,
    /// The GNSS fix interval, ms (100 for a 10 Hz receiver).
    pub fix_interval_ms: f64,
    /// The device classes.
    pub classes: Vec<TimingClass>,
}

impl BsmTimingCalibration {
    /// Parses and checks a calibration file's text (YAML or JSON).
    ///
    /// # Errors
    /// A message naming what is wrong.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let c: Self = serde_yml::from_str(text).map_err(|e| e.to_string())?;
        c.check()?;
        Ok(c)
    }

    /// Checks every field's range.
    ///
    /// # Errors
    /// The first problem, as a sentence.
    pub fn check(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("schema is {:?}; this build reads {SCHEMA}", self.schema));
        }
        if !(self.fix_interval_ms.is_finite() && self.fix_interval_ms >= 1.0) {
            return Err("fix_interval_ms must be at least 1".to_string());
        }
        if self.classes.is_empty() {
            return Err("classes is empty".to_string());
        }
        let prob = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
        let mut total = 0.0;
        for (i, c) in self.classes.iter().enumerate() {
            let at = format!("classes[{i}] ({})", c.name);
            if !prob(c.share) || !prob(c.epoch_slip_prob) || !prob(c.frozen_repeat_prob) {
                return Err(format!(
                    "{at}: share, epoch_slip_prob and frozen_repeat_prob must be in 0..1"
                ));
            }
            total += c.share;
            c.secmark_phase_ms.check(&at)?;
            if !(c.header_jitter_ms.sd.is_finite() && c.header_jitter_ms.sd >= 0.0) {
                return Err(format!("{at}: header_jitter_ms.sd must be 0 or more"));
            }
            let slip_sum: f64 = c.slip_epochs.values().sum();
            if c.slip_epochs.iter().any(|(k, p)| *k == 0 || k.abs() > 10 || !prob(*p)) {
                return Err(format!(
                    "{at}: slip_epochs keys are non-zero fix counts within ±10, values in 0..1"
                ));
            }
            if c.epoch_slip_prob > 0.0 && (slip_sum - 1.0).abs() > 1e-6 {
                return Err(format!("{at}: slip_epochs must sum to 1 (it sums to {slip_sum})"));
            }
        }
        if (total - 1.0).abs() > 1e-6 {
            return Err(format!("class shares sum to {total}, not 1"));
        }
        Ok(())
    }

    /// The calibration with the `locked` class at `share` and every other class scaled to
    /// fill the rest in proportion to its own share (equally when they are all 0).
    ///
    /// # Errors
    /// When `share` is outside 0..1 or there is no class named `locked`.
    pub fn with_locked_share(mut self, share: f64) -> Result<Self, String> {
        if !(share.is_finite() && (0.0..=1.0).contains(&share)) {
            return Err(format!("locked_share is {share}; it must be in 0..1"));
        }
        let Some(li) = self.classes.iter().position(|c| c.name == "locked") else {
            return Err("locked_share is set and the calibration has no class named locked".into());
        };
        let others: f64 = self
            .classes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != li)
            .map(|(_, c)| c.share)
            .sum();
        let n_others = (self.classes.len() - 1).max(1) as f64;
        for (i, c) in self.classes.iter_mut().enumerate() {
            c.share = if i == li {
                share
            } else if others > 0.0 {
                (1.0 - share) * c.share / others
            } else {
                (1.0 - share) / n_others
            };
        }
        if self.classes.len() == 1 && share < 1.0 {
            return Err("locked_share below 1 needs a second class to fill the rest".into());
        }
        Ok(self)
    }

    /// The built-in **provisional** values, until the real-data pipeline writes
    /// `data/calibration/bsm-timing-cvpilot.yaml`. Sources:
    /// research/synthesis/check-realdata-and-vesnos.md §2.2–2.4 and
    /// check-paper1-preemption.md §2.1.
    ///
    /// * shares: Tampa's classified IDs, 48 locked and 142 free;
    /// * locked phase: `secMark` on the epoch ±1 ms (Wyoming's xx00/xx50 modes);
    /// * locked slips: 1 of 2,803 fast Tampa pairs over 1 m, about two pairs per slip;
    /// * free slips: 4.5 % of fast Tampa pairs over 1 m, the triggers being epoch slips
    ///   (e.g. 3.04 m in 102 ms at 15.16 m/s), two pairs per slip;
    /// * free frozen repeats: Tampa's ConstantPosition share (447 of 32,291 pairs, all
    ///   speeds); Orem's 13 % is the high end;
    /// * header jitter: Wyoming's header Δt minus secMark Δt, p10 −46 / p90 +53 ms per
    ///   pair, i.e. about 27 ms per message.
    #[must_use]
    pub fn provisional() -> Self {
        let slips = BTreeMap::from([(-1, 0.45), (1, 0.45), (2, 0.10)]);
        Self {
            schema: SCHEMA.to_string(),
            provenance: serde_json::json!({
                "status": "provisional",
                "sources": [
                    "research/synthesis/check-realdata-and-vesnos.md §2.2-2.4",
                    "research/synthesis/check-paper1-preemption.md §2.1"
                ],
                "note": "hand-derived from published sample statistics; replace with the \
                         real-data pipeline's fit (data/calibration/bsm-timing-cvpilot.yaml)"
            }),
            provisional: true,
            fix_interval_ms: 100.0,
            classes: vec![
                TimingClass {
                    name: "locked".to_string(),
                    share: 0.25,
                    secmark_phase_ms: PhaseDist::Normal { mean: 0.0, sd: 0.5 },
                    phase_scope: PhaseScope::Device,
                    epoch_slip_prob: 0.000_2,
                    slip_epochs: slips.clone(),
                    frozen_repeat_prob: 0.0,
                    header_jitter_ms: Jitter { sd: 27.0 },
                },
                TimingClass {
                    name: "free".to_string(),
                    share: 0.75,
                    secmark_phase_ms: PhaseDist::Uniform { lo: 0.0, hi: 100.0 },
                    phase_scope: PhaseScope::Device,
                    epoch_slip_prob: 0.022,
                    slip_epochs: slips,
                    frozen_repeat_prob: 0.005,
                    header_jitter_ms: Jitter { sd: 27.0 },
                },
            ],
        }
    }

    fn interval_ns(&self) -> u64 {
        (self.fix_interval_ms * 1e6).round().max(1.0) as u64
    }
}

/// What the model did to one message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BsmReport {
    /// The position the message reports, world ENU metres.
    pub pos: Vec3,
    /// The instant, on the simulation (GNSS) timeline, the message's `secMark` encodes.
    pub sec_mark_at: SimTime,
    /// `generationTime` minus the device's own time, ns.
    pub header_jitter_ns: i64,
    /// The slip in fix intervals (0 for none).
    pub slip: i32,
    /// Whether the position was repeated from the previous message.
    pub frozen: bool,
}

/// One device's class and memory.
#[derive(Debug, Clone, PartialEq)]
pub struct BsmTimingDevice {
    class: usize,
    phase_ms: f64,
    last_pos: Option<Vec3>,
}

impl BsmTimingDevice {
    /// Assigns a device its class (one uniform draw against the cumulative shares) and,
    /// for a per-device phase, its phase.
    pub fn assign(cal: &BsmTimingCalibration, rng: &mut RngStream) -> Self {
        let u = rng.uniform(0.0, 1.0);
        // The `locked` class first, then the rest in file order: a higher locked share
        // then only ever moves a device into the locked class.
        let locked = cal.classes.iter().position(|c| c.name == "locked");
        let order = locked
            .into_iter()
            .chain((0..cal.classes.len()).filter(|i| Some(*i) != locked));
        let mut acc = 0.0;
        let mut class = cal.classes.len() - 1;
        for i in order {
            acc += cal.classes[i].share;
            class = i;
            if u < acc {
                break;
            }
        }
        let phase_ms = cal.classes[class].secmark_phase_ms.draw(rng);
        Self {
            class,
            phase_ms,
            last_pos: None,
        }
    }

    /// The device's class.
    #[must_use]
    pub fn class(&self) -> usize {
        self.class
    }

    /// Applies the class to one message built from `belief` (on the simulation timeline at
    /// `belief.time_ns`). `None` when the belief has no position: nothing is sent.
    ///
    /// Every call consumes the same seven 64-bit draws, whatever the class's numbers.
    pub fn report(
        &mut self,
        cal: &BsmTimingCalibration,
        rng: &mut RngStream,
        belief: &PositionEstimate,
    ) -> Option<BsmReport> {
        let c = &cal.classes[self.class];
        let phase_draw = c.secmark_phase_ms.draw(rng); // 2 draws
        let u_slip = rng.uniform(0.0, 1.0);
        let u_size = rng.uniform(0.0, 1.0);
        let u_frozen = rng.uniform(0.0, 1.0);
        let jitter = rng.normal(0.0, 1.0) * c.header_jitter_ms.sd; // 2 draws
        if !belief.fix.has_position() {
            return None;
        }
        let phase_ms = match c.phase_scope {
            PhaseScope::Device => self.phase_ms,
            PhaseScope::Message => phase_draw,
        };
        let iv = cal.interval_ns();
        let t = belief.time_ns;
        let epoch = t - t % iv;
        let mut slip = 0;
        if u_slip < c.epoch_slip_prob {
            let mut acc = 0.0;
            for (k, p) in &c.slip_epochs {
                acc += p;
                slip = *k;
                if u_size < acc {
                    break;
                }
            }
        }
        let back = |k: i32| u64::from(k.unsigned_abs()).saturating_mul(iv);
        let (pos_epoch, time_epoch) = match slip {
            k if k < 0 => (epoch.saturating_sub(back(k)), epoch),
            k if k > 0 => (epoch, epoch.saturating_sub(back(k))),
            _ => (epoch, epoch),
        };
        let age_s = t.saturating_sub(pos_epoch) as f64 * 1e-9;
        let mut pos = Vec3::new(
            belief.pos.x - belief.vel.x * age_s,
            belief.pos.y - belief.vel.y * age_s,
            belief.pos.z - belief.vel.z * age_s,
        );
        // J2735 Speed LSB: below it the vehicle is not moving and a repeat is no repeat.
        let moving = belief.ground_speed_mps() >= 0.02;
        let frozen = moving && u_frozen < c.frozen_repeat_prob && self.last_pos.is_some();
        if frozen && let Some(p) = self.last_pos {
            pos = p;
        }
        self.last_pos = Some(pos);
        let sec_mark_at =
            (i128::from(time_epoch) + (phase_ms * 1e6).round() as i128).max(0) as SimTime;
        Some(BsmReport {
            pos,
            sec_mark_at,
            header_jitter_ns: (jitter * 1e6).round() as i64,
            slip,
            frozen,
        })
    }
}

/// The model card for a calibration.
#[must_use]
pub fn card(cal: &BsmTimingCalibration) -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Gnss,
        "1.0.0",
        "Per-device BSM timing: each vehicle OBU's timing class sets the transmitted BSM's \
         secMark phase against the GNSS fix epoch, its position-epoch slips, its frozen \
         repeats while moving and its 1609.2 generationTime jitter.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new("epoch", "E = ⌊t / T_fix⌋·T_fix"),
        Equation::new("secMark", "secMark = E_time + φ_class"),
        Equation::new("position", "p = p̂(t) − v̂·(t − E_pos)"),
        Equation::new("generationTime", "t_gen = t_device + N(0, σ_hdr)"),
    ];
    let src = |r: &str| Source::new(SourceKind::Dataset, r);
    card.parameters = vec![Parameter::new(
        "fix_interval_ms",
        "ms",
        serde_json::json!(cal.fix_interval_ms),
        src("v2xw.bsm_timing.v1 calibration"),
    )];
    for c in &cal.classes {
        let n = &c.name;
        for (name, unit, v) in [
            ("share", "-", c.share),
            ("epoch_slip_prob", "-", c.epoch_slip_prob),
            ("frozen_repeat_prob", "-", c.frozen_repeat_prob),
            ("header_jitter_ms.sd", "ms", c.header_jitter_ms.sd),
        ] {
            card.parameters.push(Parameter::new(
                format!("{n}.{name}"),
                unit,
                serde_json::json!(v),
                src(if cal.provisional {
                    "PROVISIONAL: research/synthesis/check-realdata-and-vesnos.md §2.2-2.4"
                } else {
                    "v2xw.bsm_timing.v1 calibration (provenance in the file)"
                }),
            ));
        }
    }
    card.sources = vec![
        Source::new(
            SourceKind::Dataset,
            "USDOT ITS JPO CV Pilot BSMs (THEA Tampa, Wyoming, Utah Orem), \
             research/synthesis/check-realdata-and-vesnos.md",
        ),
        Source::new(SourceKind::Code, "research/synthesis/paper-plan-2026-10.md §1.4 item B"),
    ];
    card.assumptions = vec![
        "Fix epochs are on the simulation (GPS) timeline; an earlier epoch's position is the \
         belief carried back along the believed velocity."
            .to_string(),
        "A slip keeps the reported position causal: k < 0 reports an older fix, k > 0 an \
         older timestamp."
            .to_string(),
        "phase_scope device (default) draws the secMark phase once per device.".to_string(),
    ];
    card.limitations = vec![
        "The phase does not drift with the device oscillator; a slip stands in for the \
         epoch crossings that drift causes."
            .to_string(),
    ];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![rng_domain().as_str().to_string()],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![src("research/synthesis/check-realdata-and-vesnos.md §2.3")],
        tests: vec!["bsm_timing::tests".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::belief::FixQuality;
    use v2xw_core::rng::EntityRef;

    fn belief(t_ms: u64, v: f64) -> PositionEstimate {
        let mut b = PositionEstimate::no_fix(t_ms * 1_000_000);
        b.fix = FixQuality::Rtk;
        b.pos = Vec3::new(v * t_ms as f64 * 1e-3, 0.0, 0.0);
        b.vel = Vec3::new(v, 0.0, 0.0);
        b
    }

    fn rng(n: u32) -> RngStream {
        RngStream::derive(7, rng_domain(), EntityRef::Node(v2xw_core::ids::NodeId::new(n)))
    }

    #[test]
    fn the_provisional_file_round_trips_and_checks() {
        let p = BsmTimingCalibration::provisional();
        p.check().expect("provisional values are valid");
        let text = serde_yml::to_string(&p).expect("serialises");
        assert_eq!(BsmTimingCalibration::from_text(&text).expect("parses"), p);
    }

    #[test]
    fn the_shared_format_parses_with_string_slip_keys() {
        let text = r#"
schema: v2xw.bsm_timing.v1
provenance: {sources: [{dataset: thea, rows: 10, query: q}], manifest_sha256: ab, fitted_on: [thea], held_out: [wy], tool: tools/realdata 0.1}
fix_interval_ms: 100
classes:
  - name: locked
    share: 0.4
    secmark_phase_ms: {dist: normal, mean: 0.2, sd: 0.4}
    epoch_slip_prob: 0.001
    slip_epochs: {"-1": 0.5, "1": 0.4, "2": 0.1}
    frozen_repeat_prob: 0
    header_jitter_ms: {sd: 25}
  - name: free
    share: 0.6
    secmark_phase_ms: {dist: empirical, bins_ms: [0, 50, 100], p: [0.5, 0.5]}
    epoch_slip_prob: 0.02
    slip_epochs: {-1: 0.5, 1: 0.5}
    frozen_repeat_prob: 0.01
    header_jitter_ms: {sd: 25}
"#;
        let c = BsmTimingCalibration::from_text(text).expect("the shared shape parses");
        assert_eq!(c.classes[0].slip_epochs.get(&-1), Some(&0.5));
        assert_eq!(c.classes[1].slip_epochs.get(&1), Some(&0.5));
        // Shown red: shares that do not sum to one are refused.
        let bad = text.replace("share: 0.6", "share: 0.7");
        assert!(BsmTimingCalibration::from_text(&bad).is_err());
    }

    #[test]
    fn locked_share_moves_devices_monotonically() {
        let base = BsmTimingCalibration::provisional();
        let mut prev = 0;
        for share in [0.0, 0.5, 1.0] {
            let cal = base.clone().with_locked_share(share).expect("valid");
            cal.check().expect("rescaled shares check");
            let locked = (0..400)
                .filter(|n| BsmTimingDevice::assign(&cal, &mut rng(*n)).class() == 0)
                .count();
            assert!(locked >= prev, "share {share}: {locked} < {prev}");
            prev = locked;
        }
        assert_eq!(prev, 400);
    }

    #[test]
    fn a_forced_slip_moves_the_position_by_whole_fixes() {
        let mut cal = BsmTimingCalibration::provisional().with_locked_share(1.0).unwrap();
        cal.classes[0].epoch_slip_prob = 1.0;
        cal.classes[0].slip_epochs = BTreeMap::from([(-1, 1.0)]);
        let mut r = rng(1);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let rep = d.report(&cal, &mut r, &belief(1_230, 20.0)).unwrap();
        assert_eq!(rep.slip, -1);
        // The base epoch is 1.2 s; the fix reported is 1.1 s's while the time says 1.2 s.
        assert!((rep.pos.x - 22.0).abs() < 1e-9, "{}", rep.pos.x);
        assert!((rep.sec_mark_at as f64 * 1e-6 - 1_200.0).abs() < 3.0);
    }

    #[test]
    fn a_frozen_repeat_sends_the_previous_position() {
        let mut cal = BsmTimingCalibration::provisional().with_locked_share(1.0).unwrap();
        cal.classes[0].frozen_repeat_prob = 1.0;
        let mut r = rng(2);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let a = d.report(&cal, &mut r, &belief(1_000, 20.0)).unwrap();
        let b = d.report(&cal, &mut r, &belief(1_100, 20.0)).unwrap();
        assert!(!a.frozen && b.frozen);
        assert_eq!(a.pos, b.pos);
    }

    #[test]
    fn the_same_seed_gives_the_same_reports() {
        let cal = BsmTimingCalibration::provisional();
        let run = |seed_node: u32| {
            let mut r = rng(seed_node);
            let mut d = BsmTimingDevice::assign(&cal, &mut r);
            (0..200)
                .map(|i| d.report(&cal, &mut r, &belief(1_000 + 100 * i, 15.0)).unwrap())
                .map(|x| (x.pos.x.to_bits(), x.sec_mark_at, x.header_jitter_ns))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(3), run(3));
        assert_ne!(run(3), run(4), "a different key gives a different sequence");
    }
}
