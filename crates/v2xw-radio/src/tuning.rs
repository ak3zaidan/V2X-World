//! Researcher overrides of the radio models' cited constants.
//!
//! Every model in this crate ships its constants from a cited source. A protocol study
//! often needs to move one of them — a harsher fading channel, no shadowing at all to
//! isolate the MAC, a lighter building material, a different carrier-sense threshold —
//! without writing a new model. These types carry such an override from the scenario
//! (`radio.tuning`, see the engine's wiring) into the model, and every field is
//! `Option`: unset keeps the model's own cited value, so an empty override is the shipped
//! model bit for bit.

use serde::{Deserialize, Serialize};

/// Overrides of a propagation law's log-normal shadowing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ShadowingOverride {
    /// The shadowing standard deviation, dB, in every link state. `Some(0.0)` turns
    /// shadowing off.
    pub sigma_db: Option<f64>,
    /// The decorrelation distance of the AR(1) process, metres.
    pub d_corr_m: Option<f64>,
}

impl ShadowingOverride {
    /// No override.
    pub const NONE: ShadowingOverride = ShadowingOverride {
        sigma_db: None,
        d_corr_m: None,
    };

    /// The σ in force where the model's own is `own`.
    #[must_use]
    pub fn sigma(&self, own: f64) -> f64 {
        self.sigma_db.unwrap_or(own)
    }

    /// The decorrelation distance in force where the model's own is `own`.
    #[must_use]
    pub fn d_corr(&self, own: f64) -> f64 {
        self.d_corr_m.unwrap_or(own)
    }

    /// Whether anything is overridden.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.sigma_db.is_none() && self.d_corr_m.is_none()
    }
}

/// A caller-chosen Nakagami-m profile: `m_near` below `breakpoint_m`, `m_far` at and
/// beyond it. A single `m` everywhere is `m_near == m_far`.
///
/// This is the shape of every distance-dependent Nakagami model in the V2V literature
/// (Cheng et al. 2007; Torrent-Moreno 2009; the Veins `NakagamiFading` with its distance
/// thresholds), so a study can reproduce any of them from their printed values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NakagamiProfile {
    /// The shape parameter below the breakpoint.
    pub m_near: f64,
    /// The shape parameter at and beyond the breakpoint.
    pub m_far: f64,
    /// The distance at which the shape changes, metres.
    pub breakpoint_m: f64,
}

impl NakagamiProfile {
    /// One `m` at every distance.
    #[must_use]
    pub const fn constant(m: f64) -> Self {
        Self {
            m_near: m,
            m_far: m,
            breakpoint_m: f64::INFINITY,
        }
    }

    /// The shape parameter at a distance.
    #[must_use]
    pub fn m_at(&self, d_m: f64) -> f64 {
        if d_m < self.breakpoint_m {
            self.m_near
        } else {
            self.m_far
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_override_keeps_the_models_value() {
        let o = ShadowingOverride::NONE;
        assert!(o.is_empty());
        assert_eq!(o.sigma(3.0), 3.0);
        assert_eq!(o.d_corr(25.0), 25.0);
        let o = ShadowingOverride {
            sigma_db: Some(0.0),
            d_corr_m: Some(50.0),
        };
        assert_eq!(o.sigma(3.0), 0.0);
        assert_eq!(o.d_corr(25.0), 50.0);
    }

    #[test]
    fn a_nakagami_profile_switches_at_its_breakpoint() {
        let p = NakagamiProfile {
            m_near: 1.5,
            m_far: 0.75,
            breakpoint_m: 80.0,
        };
        assert_eq!(p.m_at(79.9), 1.5);
        assert_eq!(p.m_at(80.0), 0.75);
        assert_eq!(NakagamiProfile::constant(3.0).m_at(1e6), 3.0);
    }
}
