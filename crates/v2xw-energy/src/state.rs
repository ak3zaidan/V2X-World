//! What every model is given — one vehicle over one integration interval — and what an
//! emission model gives back.
//!
//! # One interval, not one instant
//!
//! The provider integrates over the interval between two consecutive `gt.kinematics`
//! records of the same actor, `[t0, t1]`. Every model sees the same kinematically
//! consistent description of that interval:
//!
//! * `v_mps` is the **mean** speed `(v0 + v1) / 2`, and `distance_m = v_mps · dt`;
//! * `a_mps2` is the **interval** acceleration `(v1 − v0) / dt`.
//!
//! With those two, `m · a · v · dt` is exactly `½ m (v1² − v0²)`: the kinetic energy a
//! power-based model charges for the interval is the kinetic energy the vehicle actually
//! gained, whatever acceleration the mobility model happened to record at either end. An
//! instant-based reading (the recorded `acc_mps2` at `t0`, say) would charge a different
//! amount whenever the acceleration changed inside the interval, and a vehicle that
//! accelerated and braked back to its starting speed could come out with a net energy
//! gain. SUMO evaluates its models at the step's end speed with the step's acceleration
//! (`HelpersEnergy::compute` reconstructs the previous speed as `v − a·TS`), which is the
//! same consistency obtained the other way round.

/// Standard gravity, m/s² (CGPM 1901; ISO 80000-3).
pub const GRAVITY_MPS2: f64 = 9.80665;

/// One vehicle over one integration interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriveState {
    /// Mean speed over the interval, m/s, never negative.
    pub v_mps: f64,
    /// Speed at the start of the interval, m/s.
    pub v0_mps: f64,
    /// Speed at the end of the interval, m/s.
    pub v1_mps: f64,
    /// Interval acceleration `(v1 − v0) / dt`, m/s², signed.
    pub a_mps2: f64,
    /// Road grade, rise over run (0.05 is a 5 % climb), signed.
    pub grade: f64,
    /// Interval length, s.
    pub dt_s: f64,
    /// Distance covered, m.
    pub distance_m: f64,
    /// Heading change over the interval, radians, signed — what a curvature loss reads.
    pub heading_change_rad: f64,
    /// How long, up to and including this interval, the vehicle has decelerated harder than
    /// 1 mph/s without a break, s — what the MOVES braking rule reads.
    pub decel_run_s: f64,
}

impl DriveState {
    /// The state of a vehicle going from `v0` to `v1` over `dt` on a level, straight road.
    ///
    /// # Panics
    /// Never; a non-positive `dt` gives a zero-length interval with zero acceleration.
    #[must_use]
    pub fn between(v0_mps: f64, v1_mps: f64, dt_s: f64) -> Self {
        let v0 = v0_mps.max(0.0);
        let v1 = v1_mps.max(0.0);
        let dt = dt_s.max(0.0);
        let v = 0.5 * (v0 + v1);
        Self {
            v_mps: v,
            v0_mps: v0,
            v1_mps: v1,
            a_mps2: if dt > 0.0 { (v1 - v0) / dt } else { 0.0 },
            grade: 0.0,
            dt_s: dt,
            distance_m: v * dt,
            heading_change_rad: 0.0,
            decel_run_s: 0.0,
        }
    }

    /// The same interval on a grade (rise over run).
    #[must_use]
    pub const fn with_grade(mut self, grade: f64) -> Self {
        self.grade = grade;
        self
    }

    /// The same interval with a heading change.
    #[must_use]
    pub const fn with_heading_change(mut self, rad: f64) -> Self {
        self.heading_change_rad = rad;
        self
    }

    /// The same interval with the deceleration run the MOVES braking rule reads.
    #[must_use]
    pub const fn with_decel_run(mut self, s: f64) -> Self {
        self.decel_run_s = s;
        self
    }

    /// `sin θ` of the road's inclination angle `θ = atan(grade)`.
    #[must_use]
    pub fn sin_incline(&self) -> f64 {
        self.grade / v2xw_core::math::sqrt(1.0 + self.grade * self.grade)
    }

    /// `cos θ` of the road's inclination angle.
    #[must_use]
    pub fn cos_incline(&self) -> f64 {
        1.0 / v2xw_core::math::sqrt(1.0 + self.grade * self.grade)
    }
}

/// Tailpipe emission and fuel rates over an interval, grams per second.
///
/// A pollutant a model does not compute is zero and the model's card says so in
/// `ignores`; there is no `NaN` to propagate into a sum.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rates {
    /// Carbon dioxide, g/s.
    pub co2_g_s: f64,
    /// Fuel, by mass, g/s.
    pub fuel_g_s: f64,
    /// Nitrogen oxides (as NO₂ mass), g/s.
    pub nox_g_s: f64,
    /// Particulate matter, g/s.
    pub pm_g_s: f64,
    /// Carbon monoxide, g/s.
    pub co_g_s: f64,
    /// Hydrocarbons, g/s.
    pub hc_g_s: f64,
}

impl Rates {
    /// Every rate times `s` seconds, as grams.
    #[must_use]
    pub fn times(self, s: f64) -> Self {
        Self {
            co2_g_s: self.co2_g_s * s,
            fuel_g_s: self.fuel_g_s * s,
            nox_g_s: self.nox_g_s * s,
            pm_g_s: self.pm_g_s * s,
            co_g_s: self.co_g_s * s,
            hc_g_s: self.hc_g_s * s,
        }
    }

    /// True if every rate is finite and non-negative — what every model must return.
    #[must_use]
    pub fn is_physical(&self) -> bool {
        [
            self.co2_g_s,
            self.fuel_g_s,
            self.nox_g_s,
            self.pm_g_s,
            self.co_g_s,
            self.hc_g_s,
        ]
        .iter()
        .all(|x| x.is_finite() && *x >= 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interval_charges_exactly_the_kinetic_energy_gained() {
        let s = DriveState::between(5.0, 9.0, 2.0);
        let m = 1500.0;
        let charged = m * s.a_mps2 * s.v_mps * s.dt_s;
        let gained = 0.5 * m * (9.0_f64 * 9.0 - 5.0 * 5.0);
        assert!((charged - gained).abs() < 1e-9, "{charged} vs {gained}");
        assert_eq!(s.distance_m, 14.0);
    }

    #[test]
    fn grade_angle_functions_are_consistent() {
        let s = DriveState::between(10.0, 10.0, 1.0).with_grade(0.1);
        let (sn, cs) = (s.sin_incline(), s.cos_incline());
        assert!((sn * sn + cs * cs - 1.0).abs() < 1e-12);
        assert!((sn / cs - 0.1).abs() < 1e-12);
    }
}
