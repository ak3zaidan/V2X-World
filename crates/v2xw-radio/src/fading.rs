//! Small-scale fading: `fading/nakagami-m` and `fading/none` (04-models.md §3.4).
//!
//! The power gain of a Nakagami-m channel, sampled once per link and per frame from the
//! `Fading` RNG domain through the core's own Nakagami sampler
//! (`RngStream::nakagami`, a documented algorithm with a fixed draw count).
//!
//! # The key, and why it is single-use
//!
//! A fading draw is scoped to one frame on one directed link, so its key is
//! `EntityRef::LinkFrame { link, frame }`. That scope is `is_single_use` in the contract
//! crate: the key already embeds the frame counter, so `checkout` derives the stream,
//! uses it and drops it instead of interning a generator per frame per link that nothing
//! would ever free (03-interfaces.md §1.1). The consequence a caller can see is that the
//! *same* `(link, frame)` always yields the *same* sample, whatever else the run did in
//! between — which is what `the_same_link_and_frame_always_draws_the_same_sample`
//! asserts, and what makes a receiver's outcome independent of the order receivers are
//! evaluated in (invariant I-R2).
//!
//! `frame` is the frame's start instant on the link, in nanoseconds. Two frames cannot
//! start at the same nanosecond on one directed link, so the instant identifies the frame
//! without a counter that a reordering could disturb.
//!
//! # Presets, and the one that is not here
//!
//! `fixed-severe`, `fixed-medium` and `fixed-low` (m = 1, 3, 5) are the labels the
//! D-FPAV and EMDV studies use [Torrent-Moreno 2009]. `yin-dsrc-freeway` is the
//! distance-dependent preset: the cited source gives *ranges* per 10 m bin (1.0-1.8 below
//! 100 m, 0.7-1.0 above), not per-bin values, so the two shipped numbers are the range
//! midpoints and are recorded as `todo-calibrate` with the range on the parameter.
//!
//! **`taliwal-ns2` is not shipped.** 04-models.md §3.4 marks its thresholds UNVERIFIED —
//! "Torrent-Moreno 2009 credits Taliwal's ns-2.28 port but does not reproduce the
//! thresholds; VANET '04 text not retrieved" — and says "not shipped until confirmed".
//! Shipping it would mean inventing the 50 m and 150 m breakpoints. The Cheng 2007
//! distance-binned m is likewise unrecoverable, and `fading/rician` does not exist
//! because no numeric Rician K for 5.9 GHz V2V was found in any cached source.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::ctx::Ctx;
use v2xw_core::ids::LinkKey;
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::SimTime;
use v2xw_world::model::EnvClass;

use crate::traits::Fading;

/// A Nakagami-m shape preset (04-models.md §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NakagamiPreset {
    /// m = 1 everywhere: Rayleigh, the severest of the three fixed labels.
    FixedSevere,
    /// m = 3 everywhere. The default outside highways.
    FixedMedium,
    /// m = 5 everywhere: the most line-of-sight-like.
    FixedLow,
    /// Distance-dependent, Yin et al. as quoted in the α-μ DSRC paper: more
    /// line-of-sight-like below 100 m, Rayleigh-or-worse beyond. The default on highways.
    YinDsrcFreeway,
}

impl NakagamiPreset {
    /// Every shipped preset, in a fixed order.
    pub const ALL: [NakagamiPreset; 4] = [
        NakagamiPreset::FixedSevere,
        NakagamiPreset::FixedMedium,
        NakagamiPreset::FixedLow,
        NakagamiPreset::YinDsrcFreeway,
    ];

    /// The preset's id as a scenario spells it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            NakagamiPreset::FixedSevere => "fixed-severe",
            NakagamiPreset::FixedMedium => "fixed-medium",
            NakagamiPreset::FixedLow => "fixed-low",
            NakagamiPreset::YinDsrcFreeway => "yin-dsrc-freeway",
        }
    }

    /// The shape parameter `m` at a distance.
    #[must_use]
    pub fn m_at(self, d_m: f64) -> f64 {
        match self {
            NakagamiPreset::FixedSevere => 1.0,
            NakagamiPreset::FixedMedium => 3.0,
            NakagamiPreset::FixedLow => 5.0,
            NakagamiPreset::YinDsrcFreeway => {
                if d_m < 100.0 {
                    // Cited range 1.0-1.8; midpoint, todo-calibrate.
                    1.4
                } else {
                    // Cited range 0.7-1.0; midpoint, todo-calibrate.
                    0.85
                }
            }
        }
    }

    /// True when every constant of the preset is a printed value rather than the midpoint
    /// of a printed range.
    #[must_use]
    pub const fn is_fully_cited(self) -> bool {
        !matches!(self, NakagamiPreset::YinDsrcFreeway)
    }

    /// The preset an environment selects by default (04-models.md §3.4: the Yin freeway
    /// preset on highways, m = 3 elsewhere).
    #[must_use]
    pub const fn for_environment(env: EnvClass) -> NakagamiPreset {
        match env {
            EnvClass::Highway => NakagamiPreset::YinDsrcFreeway,
            EnvClass::Urban | EnvClass::Suburban | EnvClass::Rural => NakagamiPreset::FixedMedium,
        }
    }
}

/// `fading/nakagami-m` — per-link, per-frame Nakagami fading (04-models.md §3.4).
#[derive(Debug, Clone)]
pub struct NakagamiFading {
    card: ModelCard,
    preset: NakagamiPreset,
    /// A caller-chosen shape profile replacing the preset's, when a study sets one
    /// (`radio.channel_model.fading: nakagami`).
    profile: Option<crate::tuning::NakagamiProfile>,
    /// `Ω`, the mean power gain. One, so the fading is a 0 dB-mean multiplicative gain
    /// and the link budget's path loss is not silently rescaled by it.
    omega: f64,
}

impl NakagamiFading {
    /// The model's id.
    pub const ID: &'static str = "fading/nakagami-m";

    /// The model with one preset.
    #[must_use]
    pub fn new(preset: NakagamiPreset) -> Self {
        Self {
            card: nakagami_card(preset),
            preset,
            profile: None,
            omega: 1.0,
        }
    }

    /// The model with the preset this environment selects.
    #[must_use]
    pub fn for_environment(env: EnvClass) -> Self {
        Self::new(NakagamiPreset::for_environment(env))
    }

    /// The preset in use.
    #[must_use]
    pub const fn preset(&self) -> NakagamiPreset {
        self.preset
    }

    /// The same model with a caller-chosen shape profile in place of the preset's. `m`
    /// is clamped to the Nakagami domain `m >= 0.5`.
    #[must_use]
    pub fn with_profile(mut self, p: crate::tuning::NakagamiProfile) -> Self {
        self.profile = Some(crate::tuning::NakagamiProfile {
            m_near: p.m_near.max(0.5),
            m_far: p.m_far.max(0.5),
            breakpoint_m: p.breakpoint_m,
        });
        self
    }

    /// The model with one fixed shape parameter `m` (≥ 0.5) at every distance.
    #[must_use]
    pub fn with_m(self, m: f64) -> Self {
        self.with_profile(crate::tuning::NakagamiProfile::constant(m))
    }

    /// The Nakagami `m` that approximates a Rice channel of factor `k_db`:
    /// `m = (K+1)²/(2K+1)`, `K` linear (Nakagami 1960; Stüber, *Principles of Mobile
    /// Communication*, 4th ed., Eq. 2.57).
    #[must_use]
    pub fn rice_m(k_db: f64) -> f64 {
        let k = math::pow(10.0, k_db / 10.0);
        (k + 1.0) * (k + 1.0) / (2.0 * k + 1.0)
    }

    /// The shape parameter this model uses at a distance.
    #[must_use]
    pub fn m_at(&self, d_m: f64) -> f64 {
        match &self.profile {
            Some(p) => p.m_at(d_m),
            None => self.preset.m_at(d_m),
        }
    }
}

impl Default for NakagamiFading {
    fn default() -> Self {
        Self::new(NakagamiPreset::FixedMedium)
    }
}

impl Model for NakagamiFading {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl NakagamiFading {
    /// The gain for one frame on one link, dB, drawn from that frame's own keyed stream
    /// (`(RngDomain::Fading, EntityRef::LinkFrame { link, frame: t })`, single-use).
    ///
    /// What [`Fading::sample_db`] returns, through a shared borrow: the draw reads nothing
    /// but the model's shape and its own stream, so a frame's receivers can be faded in
    /// parallel and get the very numbers the sequential call would give them.
    #[must_use]
    pub fn sample_shared_db(
        &self,
        rng: &v2xw_core::rng::RngRegistry,
        link: LinkKey,
        d_m: f64,
        t: SimTime,
    ) -> f64 {
        let mut stream = rng.checkout(RngDomain::Fading, EntityRef::LinkFrame { link, frame: t });
        self.gain_db(&mut stream, d_m)
    }

    fn gain_db(&self, stream: &mut v2xw_core::rng::RngStream, d_m: f64) -> f64 {
        let m = self.m_at(d_m);
        // `nakagami` returns an amplitude; the power gain is its square, and the gain in
        // dB is 10·log10 of that, i.e. 20·log10 of the amplitude.
        let amplitude = stream.nakagami(m, self.omega);
        if amplitude <= 0.0 {
            // A zero amplitude is a total fade. Reporting −inf would poison every sum it
            // reaches; −200 dB is below any receiver's sensitivity by 100 dB and keeps
            // the arithmetic finite.
            return -200.0;
        }
        20.0 * math::log10(amplitude)
    }
}

impl<C: Ctx + ?Sized> Fading<C> for NakagamiFading {
    fn sample_db(&mut self, ctx: &mut C, link: LinkKey, d_m: f64, t: SimTime) -> f64 {
        let mut stream = ctx.rng(RngDomain::Fading, EntityRef::LinkFrame { link, frame: t });
        let gain = self.gain_db(&mut stream, d_m);
        drop(stream);
        gain
    }
}

fn nakagami_card(preset: NakagamiPreset) -> ModelCard {
    let torrent = Source::new(
        SourceKind::Paper,
        "M. Torrent-Moreno et al., IEEE TVT 2009, Eq. 2 (R3 §B), via 04-models.md §3.4",
    );
    let mut card = ModelCard::new(
        NakagamiFading::ID,
        Family::Fading,
        "1.0.0",
        "Nakagami-m small-scale fading, sampled per link and per frame from the core's \
         Nakagami sampler.",
    );
    card.tier = vec![Tier::High];
    card.equations = vec![Equation {
        name: "Nakagami-m density".to_string(),
        latex_or_text: "f(x; m, Ω) = 2m^m/(Γ(m)Ω^m)·x^(2m−1)·exp(−m·x²/Ω), m >= 1/2".to_string(),
        notes: Some(
            "m = 1 is Rayleigh, larger m is more line-of-sight-like. The sample is an \
             amplitude; the gain reported is 20·log10(amplitude), i.e. 10·log10 of the \
             power gain, with Ω = 1 so the mean power gain is 0 dB."
                .to_string(),
        ),
    }];
    let m_source = if preset.is_fully_cited() {
        torrent.clone()
    } else {
        Source {
            kind: SourceKind::TodoCalibrate,
            reference: "Yin et al. as quoted in the α-μ DSRC paper (R3 §B): m in 1.0-1.8 \
                        below 100 m and 0.7-1.0 above, in 10 m bins; the per-bin values \
                        are not printed, so the midpoints are shipped"
                .to_string(),
            accessed: None,
            note: Some("Secondary source; not Cheng 2007.".to_string()),
        }
    };
    card.parameters = vec![
        Parameter {
            name: "preset".to_string(),
            unit: "-".to_string(),
            default: serde_json::json!(preset.label()),
            range: Some(
                NakagamiPreset::ALL
                    .iter()
                    .map(|p| serde_json::json!(p.label()))
                    .collect(),
            ),
            source: m_source.clone(),
            calibration: if preset.is_fully_cited() {
                None
            } else {
                Some(
                    "Obtain the Yin et al. per-10 m-bin table (or a 5.9 GHz V2V \
                     measurement of equal standing) and replace the two midpoints with \
                     the binned values."
                        .to_string(),
                )
            },
        },
        Parameter {
            name: "m_near".to_string(),
            unit: "-".to_string(),
            default: serde_json::json!(preset.m_at(0.0)),
            range: Some(vec![serde_json::json!(0.5), serde_json::json!(10.0)]),
            source: m_source.clone(),
            calibration: if preset.is_fully_cited() {
                None
            } else {
                Some("As for `preset`; the cited range is 1.0-1.8.".to_string())
            },
        },
        Parameter {
            name: "m_far".to_string(),
            unit: "-".to_string(),
            default: serde_json::json!(preset.m_at(1_000.0)),
            range: Some(vec![serde_json::json!(0.5), serde_json::json!(10.0)]),
            source: m_source,
            calibration: if preset.is_fully_cited() {
                None
            } else {
                Some("As for `preset`; the cited range is 0.7-1.0.".to_string())
            },
        },
        Parameter {
            name: "distance_threshold_m".to_string(),
            unit: "m".to_string(),
            default: serde_json::json!(100.0),
            range: Some(vec![serde_json::json!(10.0), serde_json::json!(1_000.0)]),
            source: Source::new(
                SourceKind::Paper,
                "Yin et al. via R3 §B: the 100 m boundary between the two m bands",
            ),
            calibration: None,
        },
        Parameter {
            name: "omega".to_string(),
            unit: "-".to_string(),
            default: serde_json::json!(1.0),
            range: Some(vec![serde_json::json!(0.0), serde_json::json!(10.0)]),
            source: torrent.clone(),
            calibration: None,
        },
    ];
    card.assumptions = vec![
        "Ω = 1, so fading is a multiplicative gain with unit mean power and does not \
         rescale the path loss."
            .to_string(),
        "One independent draw per frame per directed link, keyed by the frame's start \
         instant."
            .to_string(),
    ];
    card.limitations = vec![
        "No temporal correlation between successive frames on the same link: each frame \
         draws independently (04-models.md §3.4 records this as a TODO: calibrate pending \
         a cited 5.9 GHz Doppler measurement)."
            .to_string(),
        "No frequency selectivity within the 10 MHz channel.".to_string(),
        "taliwal-ns2 is not shipped: its distance thresholds are UNVERIFIED.".to_string(),
    ];
    card.ignores = vec![
        "Doppler spectrum and coherence time; the medium tier ignores fast fading \
         altogether (fading/none)."
            .to_string(),
    ];
    card.sources = vec![torrent];
    card.validation = Validation {
        status: if preset.is_fully_cited() {
            ValidationStatus::LiteratureChecked
        } else {
            // The shipped m values are range midpoints, not printed values.
            ValidationStatus::Unvalidated
        },
        references: Vec::new(),
        tests: vec![
            "the_same_link_and_frame_always_draws_the_same_sample".to_string(),
            "the_mean_power_gain_is_unity".to_string(),
            "a_smaller_m_fades_deeper".to_string(),
        ],
    };
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec!["fading".to_string()],
    };
    card
}

/// Time correlation of the small-scale fade from the link's Doppler (Clarke/Jakes, and
/// Akki-Haber for a link whose two ends both move).
///
/// With static scatterers and both ends moving, the fade's autocorrelation is
/// `J0(2π f_1 τ)·J0(2π f_2 τ)` with `f_i = v_i·f_c/c` (Akki & Haber, IEEE TVT 35(1),
/// 1986), so it decorrelates at the rate of the sum of the two Doppler shifts; the model
/// takes `f_D = (|v_tx| + |v_rx|)·f_c/c` and the coherence time `T_c = 0.423 / f_D`
/// (Rappaport, *Wireless Communications*, 2nd ed., 2002, Eq. 5.40c: the geometric mean of
/// the Clarke-model `9/(16π f_D)` and `1/f_D`). The fade is block-constant: one draw per
/// link per coherence block, so frames on a slow link inside one block share a fade and
/// frames on a fast link decorrelate. The block length is rounded down to a power of two
/// nanoseconds, so blocks nest and a small change of speed does not re-key every frame.
///
/// At 5.9 GHz two cars at 140 km/h (TR 37.885 Table 6.1.2-1's highest relative speed is
/// 280 km/h) have `f_D` ≈ 1.5 kHz and `T_c` ≈ 0.3 ms, so every 100 ms beacon fades
/// independently; cars queued at a red light keep one fade for many beacons, the burst
/// loss stationary traffic sees. Moving scatterers keep a link from freezing when both
/// ends stand still: the speed is floored at [`DopplerCoherence::SPEED_FLOOR_MPS`] (an
/// engine assumption, walking-pace scatterers: about 1 Hz and a 0.4 s block at 5.9 GHz).
/// What it does not model: the 802.11p receiver's channel estimate ageing within one
/// frame (a per-frame PER loss at high Doppler), for which no cited curve is shipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DopplerCoherence {
    /// The carrier frequency, Hz.
    pub carrier_hz: f64,
}

impl DopplerCoherence {
    /// The Doppler speed below which the link is treated as moving at this speed, m/s.
    pub const SPEED_FLOOR_MPS: f64 = 0.05;
    /// Rappaport (2002) Eq. 5.40c's constant.
    pub const COHERENCE_CONSTANT: f64 = 0.423;

    /// The Doppler spread, Hz, for `v_rel_mps` = `|v_tx| + |v_rx|`, m/s.
    #[must_use]
    pub fn doppler_hz(&self, v_rel_mps: f64) -> f64 {
        let v = if v_rel_mps.is_finite() {
            v_rel_mps.abs().max(Self::SPEED_FLOOR_MPS)
        } else {
            Self::SPEED_FLOOR_MPS
        };
        v * self.carrier_hz / 299_792_458.0
    }

    /// The coherence time, seconds.
    #[must_use]
    pub fn coherence_time_s(&self, v_rel_mps: f64) -> f64 {
        Self::COHERENCE_CONSTANT / self.doppler_hz(v_rel_mps)
    }

    /// The time the fade of a frame sent at `t` is keyed by: the start of its coherence
    /// block, the block length rounded down to a power of two nanoseconds.
    #[must_use]
    pub fn block_start(&self, t: SimTime, v_rel_mps: f64) -> SimTime {
        let tc_ns = (self.coherence_time_s(v_rel_mps) * 1e9).max(1.0) as u64;
        let q = 1u64 << (63 - tc_ns.max(1).leading_zeros());
        (t / q) * q
    }
}

/// `fading/none` — the medium tier's fading model, which is no fading at all
/// (04-models.md §3.4, and the tier table of §3: "medium ignores fast fading").
#[derive(Debug, Clone)]
pub struct NoFading {
    card: ModelCard,
}

impl NoFading {
    /// The model's id.
    pub const ID: &'static str = "fading/none";

    /// The model.
    #[must_use]
    pub fn new() -> Self {
        let mut card = ModelCard::new(
            Self::ID,
            Family::Fading,
            "1.0.0",
            "No small-scale fading: the medium tier's choice, and the way to turn fading \
             off in a study that varies one thing at a time.",
        );
        card.tier = vec![Tier::Abstract, Tier::Medium];
        card.equations = vec![Equation::new("fading gain", "0 dB")];
        card.assumptions =
            vec!["The channel gain is its large-scale mean at every instant.".to_string()];
        card.ignores =
            vec!["Everything fading/nakagami-m models (04-models.md §3 tier table).".to_string()];
        card.sources = vec![Source::new(
            SourceKind::Standard,
            "02-architecture.md §7.1 tier table, via 04-models.md §3.4",
        )];
        card.validation = Validation::new(ValidationStatus::UnitTested);
        Self { card }
    }
}

impl Default for NoFading {
    fn default() -> Self {
        Self::new()
    }
}

impl Model for NoFading {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl<C: Ctx + ?Sized> Fading<C> for NoFading {
    fn sample_db(&mut self, _ctx: &mut C, _link: LinkKey, _d_m: f64, _t: SimTime) -> f64 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numeric;
    use crate::testctx::TestCtx;
    use v2xw_core::ids::NodeId;

    fn link(a: u32, b: u32) -> LinkKey {
        LinkKey(NodeId::new(a), NodeId::new(b))
    }

    #[test]
    fn the_same_link_and_frame_always_draws_the_same_sample() {
        let mut ctx = TestCtx::new(1234);
        let mut model = NakagamiFading::new(NakagamiPreset::FixedMedium);
        let l = link(3, 9);
        let first = model.sample_db(&mut ctx, l, 150.0, 4_200_000);
        // Draw a few unrelated samples in between, on other links and other frames.
        for i in 0..20 {
            model.sample_db(&mut ctx, link(1, 2), 40.0, 1_000 * i);
            model.sample_db(&mut ctx, l, 150.0, 9_000_000 + i);
        }
        let again = model.sample_db(&mut ctx, l, 150.0, 4_200_000);
        assert_eq!(first.to_bits(), again.to_bits(), "{first} vs {again}");
        // A different frame on the same link is a different sample; a different link at
        // the same instant is a different sample.
        assert_ne!(first, model.sample_db(&mut ctx, l, 150.0, 4_200_001));
        assert_ne!(
            first,
            model.sample_db(&mut ctx, link(9, 3), 150.0, 4_200_000)
        );
    }

    #[test]
    fn the_draw_order_across_links_does_not_change_any_sample() {
        // Invariant I-R2: two receivers of the same transmission are independent, so the
        // engine may evaluate them in any order.
        let mut ctx = TestCtx::new(77);
        let mut model = NakagamiFading::new(NakagamiPreset::FixedSevere);
        let links: Vec<LinkKey> = (0..8).map(|i| link(0, i + 1)).collect();
        let forward: Vec<f64> = links
            .iter()
            .map(|l| model.sample_db(&mut ctx, *l, 100.0, 7_000_000))
            .collect();
        let backward: Vec<f64> = links
            .iter()
            .rev()
            .map(|l| model.sample_db(&mut ctx, *l, 100.0, 7_000_000))
            .collect();
        let backward: Vec<f64> = backward.into_iter().rev().collect();
        assert_eq!(forward, backward);
    }

    #[test]
    fn the_mean_power_gain_is_unity() {
        // Ω = 1: the fading is a unit-mean multiplicative gain, so it does not silently
        // rescale the path loss.
        let mut ctx = TestCtx::new(2024);
        for preset in NakagamiPreset::ALL {
            let mut model = NakagamiFading::new(preset);
            let n = 20_000;
            let mean_power = math::sum_ordered((0..n).map(|i| {
                let db = model.sample_db(&mut ctx, link(1, 2), 50.0, i as SimTime);
                numeric::db_to_linear(db)
            })) / f64::from(n);
            assert!(
                (mean_power - 1.0).abs() < 0.05,
                "{}: mean power gain {mean_power}",
                preset.label()
            );
        }
    }

    #[test]
    fn a_smaller_m_fades_deeper() {
        // m = 1 (Rayleigh) must spend more time deep in a fade than m = 5.
        let mut ctx = TestCtx::new(31337);
        let n = 20_000;
        let deep_fraction = |model: &mut NakagamiFading, ctx: &mut TestCtx| {
            let count = (0..n)
                .filter(|i| model.sample_db(ctx, link(4, 5), 200.0, *i as SimTime) < -6.0)
                .count();
            count as f64 / f64::from(n)
        };
        let severe = deep_fraction(
            &mut NakagamiFading::new(NakagamiPreset::FixedSevere),
            &mut ctx,
        );
        let low = deep_fraction(&mut NakagamiFading::new(NakagamiPreset::FixedLow), &mut ctx);
        assert!(severe > low, "m=1 {severe} should exceed m=5 {low}");
        // Rayleigh is below −6 dB about 1 − exp(−10^−0.6) = 22 % of the time.
        assert!((severe - 0.221).abs() < 0.02, "{severe}");
    }

    #[test]
    fn the_yin_preset_switches_bands_at_a_hundred_metres() {
        let m = NakagamiFading::new(NakagamiPreset::YinDsrcFreeway);
        assert_eq!(m.m_at(99.9), 1.4);
        assert_eq!(m.m_at(100.0), 0.85);
        // And the preset is registered unvalidated, because both numbers are midpoints of
        // a cited range rather than printed values.
        assert_eq!(m.card().validation.status, ValidationStatus::Unvalidated);
        assert_eq!(
            NakagamiFading::new(NakagamiPreset::FixedMedium)
                .card()
                .validation
                .status,
            ValidationStatus::LiteratureChecked
        );
    }

    #[test]
    fn taliwal_is_not_shippable_and_no_preset_claims_to_be_it() {
        for p in NakagamiPreset::ALL {
            assert_ne!(p.label(), "taliwal-ns2");
        }
    }

    #[test]
    fn no_fading_returns_zero_and_the_environment_default_follows_the_document() {
        let mut ctx = TestCtx::new(5);
        let mut none = NoFading::new();
        assert_eq!(none.sample_db(&mut ctx, link(1, 2), 10.0, 0), 0.0);
        assert_eq!(
            NakagamiPreset::for_environment(EnvClass::Highway),
            NakagamiPreset::YinDsrcFreeway
        );
        assert_eq!(
            NakagamiPreset::for_environment(EnvClass::Urban),
            NakagamiPreset::FixedMedium
        );
    }

    #[test]
    fn the_cards_validate_and_register() {
        let mut registry = v2xw_core::registry::Registry::new();
        registry
            .register(NakagamiFading::default().card().clone())
            .expect("registers");
        registry
            .register(NoFading::new().card().clone())
            .expect("registers");
        assert!(registry.contains(NakagamiFading::ID));
        assert!(registry.contains(NoFading::ID));
        for preset in NakagamiPreset::ALL {
            NakagamiFading::new(preset)
                .card()
                .validate()
                .expect("card validates");
        }
    }

    /// Faster ends decorrelate the fade faster: at 5.9 GHz a queue at a red light keeps
    /// one block across 100 ms beacons, two cars at 140 km/h get a new one per frame.
    #[test]
    fn doppler_coherence_shrinks_with_speed() {
        let c = super::DopplerCoherence { carrier_hz: 5.9e9 };
        let fast = c.coherence_time_s(2.0 * 140.0 / 3.6);
        assert!((fast - 0.423 / (77.78 * 5.9e9 / 299_792_458.0)).abs() < 1e-6, "{fast}");
        assert!(fast < 3e-4);
        assert!(c.coherence_time_s(0.0) > 0.4);
        let beacons = [0u64, 100_000_000, 200_000_000];
        let key = |v: f64| beacons.map(|t| c.block_start(t + 3_000_000_000, v));
        let still = key(0.0);
        assert_eq!(still[0], still[1], "a stationary pair shares a fade across beacons");
        let moving = key(77.78);
        assert!(moving[0] != moving[1] && moving[1] != moving[2]);
        for (t, k) in beacons.iter().zip(moving) {
            assert!(k <= t + 3_000_000_000 && t + 3_000_000_000 - k < 300_000);
        }
    }
}
