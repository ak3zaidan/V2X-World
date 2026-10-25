//! The BSM fields beyond position, speed and heading that a receiver's SM-MBR checks read.
//!
//! Shared by the sender (`v2xw-node`, which fills them from what it encodes) and the
//! receiver's detectors (`v2xw-threat`), which do not depend on each other.

/// A BSM's `secMark` and `accelSet.long`, carried from the sender's encoder to the
/// receiver's observation; both `None` for anything that is not a BSM.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BsmExtras {
    /// `secMark`, ms within the UTC minute (0..=59,999).
    pub sec_mark_ms: Option<u16>,
    /// `accelSet.long`, m/s², at the J2735 0.01 m/s² resolution.
    pub accel_long_mps2: Option<f64>,
    /// The `speed` the BSM carries when the sender's output differs from its belief (the
    /// device's receiver output noise, `v2xw_node::bsm_timing`), m/s; `None`: the belief's.
    pub speed_mps: Option<f64>,
    /// The `heading` the BSM carries when it differs from the belief, radians.
    pub heading_rad: Option<f64>,
}
