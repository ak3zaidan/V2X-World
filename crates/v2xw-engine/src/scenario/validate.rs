//! Scenario validation: 03-interfaces.md §13's "actionable errors".
//!
//! The requirement is specific, and it is the reason this module is not a pile of
//! `assert!`s: an error must name **the field** and **the conflict**, in the author's own
//! vocabulary, as in §13's own example —
//!
//! ```text
//! radio.tiers.phy: 'high' requires mac 'high' (mac is 'medium')
//! ```
//!
//! Every rule below produces that shape: [`ScenarioError::Conflict`] with a dotted
//! `field` a UI can highlight and a `conflict` that states the offending value, the rule
//! and the *other* value that the rule conflicts with. An error that says only "invalid
//! tier combination" sends the author back to the specification; this one does not.
//!
//! # All of them, not the first
//!
//! [`validate`] returns **every** problem, in a fixed order (the order of the schema's own
//! fields), because an author fixing a scenario wants the list rather than one round trip
//! per mistake. [`crate::scenario::Scenario::validate`] returns the first for callers that
//! only need pass or fail.

use std::collections::BTreeSet;

use serde_json::Value;
use v2xw_core::card::Tier;
use v2xw_core::time::WallClock;
use v2xw_core::weather::WeatherKind;

use crate::error::ScenarioError;
use crate::scenario::schema::{Scenario, TimelineKind};

/// The DES resolutions a scenario may promise its models.
const RESOLUTIONS: [&str; 3] = ["1ns", "1us", "1ms"];
/// The network layers `v2xw-net` implements.
const NET_LAYERS: [&str; 2] = ["wsmp", "gn-btp"];
const GN_AREA_FORWARDING: [&str; 2] = ["cbf", "simple"];
const GN_NON_AREA_FORWARDING: [&str; 2] = ["greedy", "cbf"];
const GN_CBF_VARIANTS: [&str; 3] = ["etsi", "dpd", "gpc"];
const CHANNEL_ACCESS: [&str; 2] = ["continuous", "alternating"];
/// The EDCA access categories, as `radio.mac` spells them (EN 302 663 Table C.3).
const ACCESS_CATEGORIES: [&str; 4] = ["vo", "vi", "be", "bk"];
/// The small-scale fading laws `radio.channel_model.fading` names.
const FADING_LAWS: [&str; 4] = ["none", "rayleigh", "rice", "nakagami"];
const FADING_COHERENCE: [&str; 2] = ["independent", "doppler"];
/// The eight 10 MHz OFDM data rates, Mbit/s (IEEE 802.11-2020 Table 17-4).
pub const DATA_RATES_MBPS: [f64; 8] = [3.0, 4.5, 6.0, 9.0, 12.0, 18.0, 24.0, 27.0];
const GN_FORWARDING: [&str; 2] = ["on-air", "overlay"];
/// How `net.access_categories` maps messages onto EDCA queues.
const NET_ACCESS_CATEGORIES: [&str; 2] = ["single", "etsi-profiles"];
/// `net.rsu_ip.air`: the constant air link, or IEEE 1609.3 WSA-gated service (WP-19).
const RSU_IP_AIR: [&str; 2] = ["constant", "sch"];
const DCC_CBR: [&str; 2] = ["local", "global"];
/// ETSI reactive DCC's tables, TS 102 687 V1.2.1 Annex A (network audit).
const DCC_REACTIVE_TABLES: [&str; 2] = ["a1", "a2"];
/// The envelope profiles `v2xw-sec` implements.
const ENVELOPES: [&str; 2] = ["ieee1609.2", "etsi103097"];
/// `security.scms.tls`: the TLS versions the US profile admits for `session-scmsAuth`
/// (SCMS Manager EE profile §2.1.18).
const SCMS_TLS: [&str; 2] = ["1.3", "1.2"];
/// `security.scms.crl_overdue_scope`: the two readings of CAMP 1.2.2 Table 67's
/// "rejecting all of them" (`v2xw_node::rxval::CrlOverdueScope`).
const SCMS_CRL_OVERDUE_SCOPES: [&str; 2] = v2xw_node::rxval::CrlOverdueScope::NAMES;
/// The verification policies `v2xw-node` registers.
const POLICIES: [&str; 3] = ["verify-all", "on-demand", "prioritized"];
/// The codec tiers build decision D2 settled.
const CODEC_TIERS: [&str; 2] = ["uper", "size-model"];
/// The message sets `v2xw-msg` can generate or size.
const MESSAGE_SETS: [&str; 10] = [
    "bsm", "cam", "denm", "spat", "map", "srm", "ssm", "psm", "vam", "cpm",
];
/// The signature primitives that exist in `real` crypto mode.
///
/// `modeled` mode costs any primitive the profile prices; `real` mode has to do the
/// mathematics, and P-256 is the one `v2xw-sec` implements (the post-quantum primitives of
/// 05-protocols.md are sized, not computed).
const REAL_SIGNATURES: [&str; 1] = ["ecdsa-p256"];
/// The pseudonym-change strategies 05-protocols.md §2.6 names.
const STRATEGIES: [&str; 7] = [
    "time",
    "distance",
    "j2945-1",
    "nyc-pilot",
    "c2c-cc",
    "mix-zone",
    "silent",
];

/// The on-board-unit hardware profiles that ship with `v2xw-node`.
///
/// Built from [`v2xw_node::profiles::PROFILE_SOURCES`] rather than listed, so a profile
/// added to that crate becomes selectable without an edit here. The filter is the id
/// prefix, which is how 06-node-models.md §7 spells a device's kind.
pub fn obu_profile_ids() -> Vec<&'static str> {
    v2xw_node::profiles::PROFILE_SOURCES
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| id.starts_with("obu/"))
        .collect()
}

/// Every hardware profile id that ships, for the roadside and backend slots.
pub fn all_profile_ids() -> Vec<&'static str> {
    v2xw_node::profiles::PROFILE_SOURCES
        .iter()
        .map(|(id, _)| *id)
        .collect()
}

// ---------------------------------------------------------------------------
// The machine-readable half of this module (13-product-direction.md §2)
// ---------------------------------------------------------------------------
//
// The page's settings form is generated, and its inline validation has to be the same
// validation the loader performs or the two disagree about what is valid. The three
// tables below are how: every numeric range and every closed value set is stated once,
// the rules further down read them, and `crate::scenario::publish` publishes them. A
// number that appears in a form and a different number in the loader is not possible,
// because there is only one number.

/// One numeric range the loader enforces and the published schema states.
#[derive(Debug, Clone, Copy)]
pub struct Bound {
    /// The dotted scenario path, with `[]` for a list element and `*` for a map key.
    pub path: &'static str,
    /// The lower limit.
    pub lo: f64,
    /// The upper limit; [`f64::INFINITY`] for "no upper limit", which publishes none.
    pub hi: f64,
    /// Whether `lo` itself is excluded.
    pub exclusive_lo: bool,
    /// What the quantity is, for the error message: "the equipped fraction is 1.5, …".
    pub what: &'static str,
}

impl Bound {
    /// The interval in mathematical notation, for an error message.
    pub fn describe(&self) -> String {
        let open = if self.exclusive_lo { '(' } else { '[' };
        if self.hi.is_infinite() {
            format!("{open}{}, ∞)", self.lo)
        } else {
            format!("{open}{}, {}]", self.lo, self.hi)
        }
    }

    /// Whether `value` satisfies it.
    pub fn admits(&self, value: f64) -> bool {
        let low = if self.exclusive_lo {
            value > self.lo
        } else {
            value >= self.lo
        };
        value.is_finite() && low && value <= self.hi
    }
}

/// Every numeric range in the schema.
///
/// A number that belongs in a range belongs here and nowhere else. `world.imported_at`,
/// `time.t0` and the cross-field rules are not ranges and stay as rules below.
pub static BOUNDS: &[Bound] = &[
    Bound {
        path: "time.duration_s",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: true,
        what: "the run length",
    },
    // WP-21: condition-triggered timeline items. Syntax, not physics: the bounds keep a
    // clause meaningful (a TTC threshold of a minute is no longer a near miss).
    Bound {
        path: "events[].max_fires",
        lo: 1.0,
        hi: 1_000_000.0,
        exclusive_lo: false,
        what: "how many times the item may fire",
    },
    Bound {
        path: "events[].cooldown_s",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: false,
        what: "the least time between two firings",
    },
    Bound {
        path: "events[].when.ttc_s",
        lo: 0.0,
        hi: 60.0,
        exclusive_lo: true,
        what: "the time-to-collision threshold",
    },
    Bound {
        path: "events[].when.of[].ttc_s",
        lo: 0.0,
        hi: 60.0,
        exclusive_lo: true,
        what: "the time-to-collision threshold",
    },
    Bound {
        path: "events[].when.speed_diff_mps",
        lo: 0.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "the closing speed",
    },
    Bound {
        path: "events[].when.of[].speed_diff_mps",
        lo: 0.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "the closing speed",
    },
    Bound {
        path: "events[].when.p",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the probability per step",
    },
    Bound {
        path: "events[].when.of[].p",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the probability per step",
    },
    // ADR 0004 decision 2: below 10 ms no mobility provider is calibrated for the step,
    // and above 100 ms the constant-velocity extrapolation between steps stops being
    // accurate enough for frame-level radio.
    Bound {
        path: "time.mobility_step_ms",
        lo: 10.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "the mobility period",
    },
    // From a tenth of a second (a mobility step at most) to an hour: a sample is a
    // window's summary, and an hour is the coarsest a day's chart still has a shape at.
    Bound {
        path: "time.warmup_s",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: false,
        what: "the warm-up period",
    },
    Bound {
        path: "time.metric_period_s",
        lo: 0.1,
        hi: 3600.0,
        exclusive_lo: false,
        what: "the metric period",
    },
    Bound {
        path: "world.buildings.metres_per_level",
        lo: 1.5,
        hi: 10.0,
        exclusive_lo: false,
        what: "the storey height",
    },
    // A walking pace to a motorway's: slower is no wave, faster is no street.
    Bound {
        path: "world.signals.progression_speed_mps",
        lo: 1.0,
        hi: 40.0,
        exclusive_lo: false,
        what: "the green-wave speed",
    },
    Bound {
        path: "actors.vehicles.equipped_fraction",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the equipped fraction",
    },
    Bound {
        path: "actors.vehicles.classes.*.fraction",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the class share",
    },
    Bound {
        path: "energy.fleet.*.ev_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the segment's battery-electric share",
    },
    Bound {
        path: "energy.fleet.*.diesel_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the segment's diesel share among combustion vehicles",
    },
    Bound {
        path: "energy.fleet.*.euro",
        lo: 0.0,
        hi: 6.0,
        exclusive_lo: false,
        what: "the Euro emission standard",
    },
    Bound {
        path: "energy.ev.initial_soc",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the initial state of charge",
    },
    Bound {
        path: "energy.stop_speed_mps",
        lo: 0.0,
        hi: 5.0,
        exclusive_lo: true,
        what: "the stop speed",
    },
    // The ceiling is a guard, not a traffic figure. `scenarios/scale/` offers 3,600,000
    // veh/h on purpose — a thousand candidates a second, so the grid fills as fast as lane
    // capacity admits them — and the guard sits well above that: 100,000,000 veh/h is
    // 2,778 candidates per 100 ms step, still bounded work. Far above it the thinned-Poisson
    // process draws a candidate per nanosecond and one step allocates millions of trips,
    // which took the server's memory rather than being refused (`rate_veh_per_h: 1e308` was
    // accepted).
    Bound {
        path: "actors.vehicles.demand.rate_veh_per_h",
        lo: 0.0,
        hi: MAX_ARRIVAL_RATE_VEH_PER_H,
        exclusive_lo: false,
        what: "the arrival rate",
    },
    // A guard, not a traffic figure: 50,000 vehicles held on the road is well past what the
    // all-microscopic engine carries in real time on this machine class (BUILD-STATUS,
    // city scale: about 250 at 0.2x real time) and what the tiered city carries.
    Bound {
        path: "actors.vehicles.demand.fleet_size",
        lo: 1.0,
        hi: 50_000.0,
        exclusive_lo: false,
        what: "the number of vehicles held on the road",
    },
    // Gridlock timeout: 0 is off; an hour is past any signal cycle or incident a run
    // would want to wait out (SUMO's default is 300 s).
    Bound {
        path: "traffic.gridlock_timeout_s",
        lo: 0.0,
        hi: 3600.0,
        exclusive_lo: false,
        what: "the gridlock timeout",
    },
    // FHWA Traffic Signal Timing Manual (2008) ch. 5: minimum greens of 2-15 s, maximum
    // greens up to about 120 s on a major arterial, passage times of 0.5-5 s; the bounds
    // keep inside those, with room either side.
    // 0 is off; an hour between re-plans is as good as none, and more often than every
    // 10 s re-routes faster than a lane's smoothed speed can change (SUMO's adaptation is
    // 180 one-second samples).
    Bound {
        path: "traffic.reroute_period_s",
        lo: 0.0,
        hi: 3600.0,
        exclusive_lo: false,
        what: "the rerouting period",
    },
    // A share: 0 is nobody, 1 everybody (SUMO device.rerouting.probability).
    Bound {
        path: "traffic.reroute_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share of drivers who re-route",
    },
    Bound {
        path: "traffic.signals.min_green_s",
        lo: 2.0,
        hi: 60.0,
        exclusive_lo: false,
        what: "the minimum green",
    },
    Bound {
        path: "traffic.signals.max_green_s",
        lo: 5.0,
        hi: 180.0,
        exclusive_lo: false,
        what: "the maximum green",
    },
    Bound {
        path: "traffic.signals.passage_s",
        lo: 0.2,
        hi: 10.0,
        exclusive_lo: false,
        what: "the passage time",
    },
    Bound {
        path: "traffic.signals.detector_m",
        lo: 1.0,
        hi: 150.0,
        exclusive_lo: false,
        what: "the detector length",
    },
    Bound {
        path: "traffic.signals.green_wave_speed_mps",
        lo: 1.0,
        hi: 40.0,
        exclusive_lo: false,
        what: "the green-wave speed",
    },
    // A link of 10 m stores one car; 200 m stores 26 at the HCM's 7.62 m per queued car,
    // beyond which a link is not "short" in any timing practice.
    Bound {
        path: "traffic.signals.short_link_clearance_m",
        lo: 10.0,
        hi: 200.0,
        exclusive_lo: false,
        what: "the short-link length",
    },
    Bound {
        path: "traffic.signals.green_wave_heading_deg",
        lo: 0.0,
        hi: 360.0,
        exclusive_lo: false,
        what: "the green-wave heading",
    },
    // WP-17 NEMA: the left-turn phase's timing, bounded as the through's is. STM 2nd ed.
    // Exhibit 6-4 (left minimum 2-5 s) and Exhibit 6-9 (left maximum 15-30 s) sit inside.
    Bound {
        path: "traffic.signals.nema.left_min_green_s",
        lo: 2.0,
        hi: 60.0,
        exclusive_lo: false,
        what: "the left-turn minimum green",
    },
    Bound {
        path: "traffic.signals.nema.left_max_green_s",
        lo: 5.0,
        hi: 180.0,
        exclusive_lo: false,
        what: "the left-turn maximum green",
    },
    Bound {
        path: "traffic.signals.nema.left_passage_s",
        lo: 0.2,
        hi: 10.0,
        exclusive_lo: false,
        what: "the left-turn passage time",
    },
    // MUTCD 2009 §4D.26 ¶03: a yellow change interval should be 3 to 6 s.
    Bound {
        path: "traffic.signals.nema.yellow_s",
        lo: 3.0,
        hi: 6.0,
        exclusive_lo: false,
        what: "the yellow change interval",
    },
    // MUTCD 2009 §4D.26: a red clearance shall not exceed 6 s.
    Bound {
        path: "traffic.signals.nema.red_clear_s",
        lo: 0.0,
        hi: 6.0,
        exclusive_lo: false,
        what: "the red clearance",
    },
    // The same MUTCD 2009 §4E.06 limits as world.signals.pedestrian_walk_s and
    // world.signals.pedestrian_speed_mps.
    Bound {
        path: "traffic.signals.nema.walk_s",
        lo: 4.0,
        hi: 30.0,
        exclusive_lo: false,
        what: "the pedestrian walk interval",
    },
    Bound {
        path: "traffic.signals.nema.ped_speed_mps",
        lo: 0.8,
        hi: 1.5,
        exclusive_lo: false,
        what: "the pedestrian clearance walking speed",
    },
    // 1,000 veh/h per lane-km is several times a jammed lane's density at any speed; the
    // ceiling is a guard against a typo, as the rate's is.
    Bound {
        path: "actors.vehicles.demand.rate_veh_per_h_per_lane_km",
        lo: 0.0,
        hi: 1000.0,
        exclusive_lo: false,
        what: "the arrival rate per kilometre of driving lane",
    },
    // IEEE 1609.3 RepeatRate is WSAs per 5 s in 0..=255 (RSU Spec v4.1 App. D), so the
    // shortest WSA period it can say is 5 000 / 255 ms and the longest 5 s.
    Bound {
        path: "net.rsu_ip.wsa_interval_ms",
        lo: 20.0,
        hi: 5000.0,
        exclusive_lo: false,
        what: "the WSA period (RepeatRate counts WSAs per 5 s, at most 255)",
    },
    // EN 302 636-4-1 Annex H: the hop limit is an 8-bit field, default 10.
    Bound {
        path: "net.geonet.hop_limit",
        lo: 1.0,
        hi: 255.0,
        exclusive_lo: false,
        what: "the GeoNetworking hop limit",
    },
    // Annex H: itsGnMaxGeoAreaSize is 10 km², a circle of about 1 784 m radius.
    Bound {
        path: "net.geonet.area_radius_m",
        lo: 0.0,
        hi: 1784.0,
        exclusive_lo: true,
        what: "the warning area's radius",
    },
    Bound {
        path: "net.geonet.range_m",
        lo: 0.0,
        hi: 3000.0,
        exclusive_lo: true,
        what: "the single-hop range the forwarding sees",
    },
    Bound {
        path: "net.geonet.repetition_interval_ms",
        lo: 0.0,
        hi: 60000.0,
        exclusive_lo: false,
        what: "the warning's repetition interval",
    },
    Bound {
        path: "net.geonet.repetition_duration_ms",
        lo: 0.0,
        hi: 600000.0,
        exclusive_lo: false,
        what: "how long the warning repeats",
    },
    Bound {
        path: "net.geonet.window_s",
        lo: 0.0,
        hi: 600.0,
        exclusive_lo: true,
        what: "how long each warning is followed",
    },
    Bound {
        path: "actors.rsu_placement.count",
        lo: 0.0,
        hi: 256.0,
        exclusive_lo: false,
        what: "the number of placed roadside units",
    },
    Bound {
        path: "actors.rsu_placement.min_spacing_m",
        lo: 0.0,
        hi: 5000.0,
        exclusive_lo: false,
        what: "the spacing of placed roadside units",
    },
    Bound {
        path: "actors.vru.device_fraction",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the device fraction",
    },
    Bound {
        path: "actors.vru.midblock_rate_per_100m",
        lo: 0.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "the mid-block crossing rate",
    },
    Bound {
        path: "actors.vru.red_crossing_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share who cross against the signal",
    },
    Bound {
        path: "actors.vehicles.drivers.parameter_spread",
        lo: 0.0,
        hi: 2.0,
        exclusive_lo: false,
        what: "the driver parameter spread",
    },
    Bound {
        path: "actors.vehicles.drivers.reaction_time_s",
        lo: 0.0,
        hi: v2xw_mobility::drivers::MAX_REACTION_S,
        exclusive_lo: false,
        what: "the drivers' reaction time",
    },
    Bound {
        path: "actors.vehicles.drivers.perception_error",
        lo: 0.0,
        hi: 3.0,
        exclusive_lo: false,
        what: "the perception-error multiplier",
    },
    Bound {
        path: "actors.vru.crossing_gap_margin_s",
        lo: 0.5,
        hi: 6.0,
        exclusive_lo: false,
        what: "the pedestrian crossing gap margin",
    },
    Bound {
        path: "actors.vru.walking_speed_mps",
        lo: 0.5,
        hi: 2.5,
        exclusive_lo: false,
        what: "the pedestrian walking speed",
    },
    // The applications' thresholds. The loader checked them by hand (positive, at most
    // 30) with no row here, so the settings window and the sensitivity catalogue saw no
    // range and the catalogue could not sweep any of them.
    Bound {
        path: "apps.glosa_compliance",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share of drivers who follow GLOSA advice",
    },
    Bound {
        path: "apps.fcw_ttc_s",
        lo: 0.0,
        hi: 30.0,
        exclusive_lo: true,
        what: "the FCW time-to-collision threshold",
    },
    Bound {
        path: "apps.ima_tti_s",
        lo: 0.0,
        hi: 30.0,
        exclusive_lo: true,
        what: "the IMA look-ahead",
    },
    Bound {
        path: "apps.lta_gap_s",
        lo: 0.0,
        hi: 30.0,
        exclusive_lo: true,
        what: "the LTA critical gap",
    },
    Bound {
        path: "apps.pcw_ttc_s",
        lo: 0.0,
        hi: 30.0,
        exclusive_lo: true,
        what: "the PCW time threshold",
    },
    Bound {
        path: "apps.rlvw_decel_mps2",
        lo: 0.0,
        hi: 30.0,
        exclusive_lo: true,
        what: "the RLVW comfortable-stop deceleration",
    },
    Bound {
        path: "actors.vru.ebike_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the e-bike share",
    },
    Bound {
        path: "actors.backend.links[].latency_ms",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: false,
        what: "the one-way latency",
    },
    Bound {
        path: "actors.backend.links[].capacity_mbps",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: true,
        what: "the link capacity",
    },
    // The same `[0, 1]` scale `v2xw_core::WeatherState::intensity` uses; each model's
    // card declares what its own 1.0 means.
    Bound {
        path: "weather.intensity",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the weather intensity",
    },
    Bound {
        path: "weather.visibility_m",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: true,
        what: "the meteorological visibility",
    },
    Bound {
        path: "scale.traffic.meso.segment_length_m",
        lo: 1.0,
        hi: 10_000.0,
        exclusive_lo: false,
        what: "the queue segment length",
    },
    Bound {
        path: "scale.traffic.meso.tau_ff_s",
        lo: 0.0,
        hi: 60.0,
        exclusive_lo: true,
        what: "the free-free headway",
    },
    Bound {
        path: "scale.traffic.meso.tau_fj_s",
        lo: 0.0,
        hi: 60.0,
        exclusive_lo: true,
        what: "the free-jam headway",
    },
    Bound {
        path: "scale.traffic.meso.tau_jf_s",
        lo: 0.0,
        hi: 60.0,
        exclusive_lo: true,
        what: "the jam-free headway",
    },
    Bound {
        path: "scale.traffic.meso.tau_jj_s",
        lo: 0.0,
        hi: 60.0,
        exclusive_lo: true,
        what: "the jam-jam headway",
    },
    Bound {
        path: "scale.traffic.meso.jam_threshold",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the jam occupancy share",
    },
    Bound {
        path: "scale.radio.margin_m",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: false,
        what: "the radio margin around the microscopic region",
    },
    Bound {
        path: "radio.tiers.focus.region.radius_m",
        lo: 0.0,
        hi: f64::INFINITY,
        exclusive_lo: true,
        what: "the follow radius",
    },
    // A conducted power the FCC's 33 dBm EIRP ceiling for C-V2X on-board and roadside
    // units (FCC 24-123 §95.3204, §90.391) could not be radiated at with any antenna, and
    // a floor well below any J2945/1 or ETSI setting.
    Bound {
        path: "radio.devices.obu.tx_power_dbm",
        lo: -20.0,
        hi: 33.0,
        exclusive_lo: false,
        what: "the on-board unit's transmit power",
    },
    Bound {
        path: "radio.devices.rsu.off_channel.probability",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share of sync intervals a roadside unit leaves the safety channel in",
    },
    Bound {
        path: "radio.devices.rsu.off_channel.from_ms",
        lo: 0.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "when a roadside unit leaves the safety channel within the sync interval",
    },
    Bound {
        path: "radio.devices.rsu.off_channel.min_duration_ms",
        lo: 0.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "a roadside unit's shortest time off the safety channel",
    },
    Bound {
        path: "radio.devices.rsu.off_channel.max_duration_ms",
        lo: 0.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "a roadside unit's longest time off the safety channel",
    },
    Bound {
        path: "radio.devices.rsu.tx_power_dbm",
        lo: -20.0,
        hi: 33.0,
        exclusive_lo: false,
        what: "the roadside unit's transmit power",
    },
    Bound {
        path: "radio.devices.vru.tx_power_dbm",
        lo: -20.0,
        hi: 33.0,
        exclusive_lo: false,
        what: "the VRU device's transmit power",
    },
    Bound {
        path: "radio.devices.obu.antenna_gain_dbi",
        lo: -10.0,
        hi: 20.0,
        exclusive_lo: false,
        what: "the on-board unit's antenna gain",
    },
    Bound {
        path: "radio.devices.rsu.antenna_gain_dbi",
        lo: -10.0,
        hi: 20.0,
        exclusive_lo: false,
        what: "the roadside unit's antenna gain",
    },
    Bound {
        path: "radio.devices.vru.antenna_gain_dbi",
        lo: -10.0,
        hi: 20.0,
        exclusive_lo: false,
        what: "the VRU device's antenna gain",
    },
    Bound {
        path: "radio.devices.obu.cable_loss_db",
        lo: 0.0,
        hi: 20.0,
        exclusive_lo: false,
        what: "the on-board unit's cable loss",
    },
    Bound {
        path: "radio.devices.rsu.cable_loss_db",
        lo: 0.0,
        hi: 20.0,
        exclusive_lo: false,
        what: "the roadside unit's cable loss",
    },
    Bound {
        path: "radio.devices.vru.cable_loss_db",
        lo: 0.0,
        hi: 20.0,
        exclusive_lo: false,
        what: "the VRU device's cable loss",
    },
    // Above ground; the FCC caps a C-V2X roadside antenna at 15 m (FCC 24-123 §90.391(b)).
    Bound {
        path: "radio.devices.obu.antenna_height_m",
        lo: 0.1,
        hi: 15.0,
        exclusive_lo: false,
        what: "the on-board unit's antenna height",
    },
    Bound {
        path: "radio.devices.rsu.antenna_height_m",
        lo: 0.1,
        hi: 15.0,
        exclusive_lo: false,
        what: "the roadside unit's antenna height",
    },
    Bound {
        path: "radio.devices.vru.antenna_height_m",
        lo: 0.1,
        hi: 15.0,
        exclusive_lo: false,
        what: "the VRU device's antenna height",
    },
    // A measured antenna's mount rotation (WP-09; Veins SampledAntenna1D's rotation).
    Bound {
        path: "radio.devices.obu.sampled_pattern.rotation_deg",
        lo: -360.0,
        hi: 360.0,
        exclusive_lo: false,
        what: "the sampled antenna's mount rotation",
    },
    Bound {
        path: "radio.devices.obu.sampled_pattern.rotation_jitter_deg",
        lo: 0.0,
        hi: 180.0,
        exclusive_lo: false,
        what: "the sampled antenna's per-vehicle rotation spread",
    },
    Bound {
        path: "radio.range.margin_db",
        lo: 0.0,
        hi: 40.0,
        exclusive_lo: false,
        what: "the candidate-range margin",
    },
    Bound {
        path: "radio.range.max_m",
        lo: 50.0,
        hi: 100_000.0,
        exclusive_lo: false,
        what: "the candidate-range cap",
    },
    Bound {
        path: "radio.channel_model.nakagami_shape_far",
        lo: 0.5,
        hi: 20.0,
        exclusive_lo: false,
        what: "the far Nakagami m",
    },
    Bound {
        path: "radio.channel_model.nakagami_breakpoint_m",
        lo: 0.0,
        hi: 5000.0,
        exclusive_lo: true,
        what: "the Nakagami breakpoint",
    },
    // Sommer 2011's fitted rows run 2.4-9.6 dB per wall; 40 dB is a reinforced-concrete ceiling.
    Bound {
        path: "radio.channel_model.building_wall_loss_db",
        lo: 0.0,
        hi: 40.0,
        exclusive_lo: false,
        what: "the per-wall building loss",
    },
    // Sommer 2011's fitted rows run 0.10-0.63 dB/m.
    Bound {
        path: "radio.channel_model.building_loss_db_per_m",
        lo: 0.0,
        hi: 10.0,
        exclusive_lo: false,
        what: "the per-metre building loss",
    },
    Bound {
        path: "radio.sidelink.candidate_percent",
        lo: 20.0,
        hi: 50.0,
        exclusive_lo: false,
        what: "the candidate percentage",
    },
    // TS 36.213 §14.1.1.6: T2 in [20, 100] ms on LTE; TS 38.214 §8.1.4: up to the 100 ms packet delay budget on NR.
    Bound {
        path: "radio.sidelink.selection_window_ms",
        lo: 1.0,
        hi: 100.0,
        exclusive_lo: false,
        what: "the selection window end T2",
    },
    Bound {
        path: "radio.channel",
        lo: 170.0,
        hi: 184.0,
        exclusive_lo: false,
        what: "the ITS channel number",
    },
    Bound {
        path: "security.pseudonym_change.period_s",
        lo: 1.0,
        hi: 86_400.0,
        exclusive_lo: false,
        what: "the rotation period",
    },
    // A rotation distance below a metre is not a distance and above a hundred kilometres
    // is longer than any trip this simulator places, so either is an author error rather
    // than a study.
    Bound {
        path: "security.pseudonym_change.distance_m",
        lo: 1.0,
        hi: 100_000.0,
        exclusive_lo: false,
        what: "the rotation distance",
    },
    // `radio.phy`. Noise figure: 0 dB is an ideal receiver; 15 dB is worse than any
    // automotive front end in the 3GPP TR 37.885 Table 6.1.2-1 assumptions (9 dB UE).
    Bound {
        path: "radio.phy.data_rate_mbps",
        lo: 3.0,
        hi: 27.0,
        exclusive_lo: false,
        what: "the 802.11p data rate",
    },
    Bound {
        path: "radio.phy.noise_figure_db",
        lo: 0.0,
        hi: 15.0,
        exclusive_lo: false,
        what: "the receiver noise figure",
    },
    // Between the thermal floor less any useful SNR (-105 dBm) and a deafened receiver.
    Bound {
        path: "radio.phy.sensitivity_dbm",
        lo: -105.0,
        hi: -60.0,
        exclusive_lo: false,
        what: "the receiver sensitivity",
    },
    // -100 dBm is below the noise floor; -50 dBm is above Veins' -65 dBm by 15 dB.
    Bound {
        path: "radio.phy.cca_threshold_dbm",
        lo: -100.0,
        hi: -50.0,
        exclusive_lo: false,
        what: "the carrier-sense threshold",
    },
    // The same span: -85 dBm reproduces the pre-radio-fix4 carrier sense, -62 is ns-3's.
    Bound {
        path: "radio.phy.energy_detect_dbm",
        lo: -100.0,
        hi: -50.0,
        exclusive_lo: false,
        what: "the energy-detect level",
    },
    Bound {
        path: "radio.phy.capture_threshold_db",
        lo: 0.0,
        hi: 30.0,
        exclusive_lo: false,
        what: "the capture threshold",
    },
    // `radio.mac`. CWmin is 2^k - 1 up to aCWmax = 1023 and AIFSN is 2-15 for a non-AP
    // station (IEEE 802.11-2020 §9.4.2.28, Table 9-155).
    Bound {
        path: "radio.mac.cw_min_vo",
        lo: 1.0,
        hi: 1023.0,
        exclusive_lo: false,
        what: "the voice contention window",
    },
    Bound {
        path: "radio.mac.cw_min_vi",
        lo: 1.0,
        hi: 1023.0,
        exclusive_lo: false,
        what: "the video contention window",
    },
    Bound {
        path: "radio.mac.aifsn_vo",
        lo: 2.0,
        hi: 15.0,
        exclusive_lo: false,
        what: "the voice AIFSN",
    },
    Bound {
        path: "radio.mac.aifsn_vi",
        lo: 2.0,
        hi: 15.0,
        exclusive_lo: false,
        what: "the video AIFSN",
    },
    Bound {
        path: "radio.mac.queue_length",
        lo: 1.0,
        hi: 1024.0,
        exclusive_lo: false,
        what: "the queue length",
    },
    // `radio.channel_model`. Exponents from 1.5 (waveguiding street, Kunisch 1.61) to 6
    // (dense obstruction); shadowing 0-15 dB; Nakagami m >= 0.5 by definition.
    Bound {
        path: "radio.channel_model.path_loss_exponent",
        lo: 1.5,
        hi: 6.0,
        exclusive_lo: false,
        what: "the path-loss exponent",
    },
    Bound {
        path: "radio.channel_model.shadowing_sigma_db",
        lo: 0.0,
        hi: 15.0,
        exclusive_lo: false,
        what: "the shadowing spread",
    },
    Bound {
        path: "radio.channel_model.shadowing_decorrelation_m",
        lo: 0.0,
        hi: 500.0,
        exclusive_lo: true,
        what: "the shadowing decorrelation distance",
    },
    Bound {
        path: "radio.channel_model.nakagami_shape",
        lo: 0.5,
        hi: 20.0,
        exclusive_lo: false,
        what: "the Nakagami shape",
    },
    Bound {
        path: "radio.channel_model.rice_k_db",
        lo: -10.0,
        hi: 30.0,
        exclusive_lo: false,
        what: "the Rice factor",
    },
    // `radio.dcc`. A target load below 0.1 throttles an empty channel; above 0.95 the
    // channel is saturated before the loop reacts. α and β are TS 102 687 Table 3's
    // 0.016 and 0.0012, open a decade either way.
    Bound {
        path: "radio.dcc.cbr_target",
        lo: 0.1,
        hi: 0.95,
        exclusive_lo: false,
        what: "the target channel load",
    },
    Bound {
        path: "radio.dcc.adaptive_alpha",
        lo: 0.001,
        hi: 0.5,
        exclusive_lo: false,
        what: "the adaptive smoothing factor",
    },
    Bound {
        path: "radio.dcc.adaptive_beta",
        lo: 0.0001,
        hi: 0.05,
        exclusive_lo: false,
        what: "the adaptive gain",
    },
    // vMax_ITT must not undercut vMin_ITT (100 ms); 2 s is a fifth of the CAM floor rate.
    Bound {
        path: "radio.dcc.max_itt_ms",
        lo: 100.0,
        hi: 2000.0,
        exclusive_lo: false,
        what: "the longest BSM interval",
    },
    Bound {
        path: "radio.dcc.density_coefficient",
        lo: 1.0,
        hi: 200.0,
        exclusive_lo: false,
        what: "the density coefficient",
    },
    Bound {
        path: "radio.dcc.min_power_dbm",
        lo: -10.0,
        hi: 20.0,
        exclusive_lo: false,
        what: "the lowest controlled power",
    },
    // `radio.sidelink`: TS 36.331's RRI range, probResourceKeep's 0-0.8, and the RSRP
    // threshold's -128 dBm floor (TS 36.213 §14.1.1.6), capped at -60 dBm.
    Bound {
        path: "radio.sidelink.reservation_interval_ms",
        lo: 1.0,
        hi: 1000.0,
        exclusive_lo: false,
        what: "the resource reservation interval",
    },
    Bound {
        path: "radio.sidelink.resource_keep_probability",
        lo: 0.0,
        hi: 0.8,
        exclusive_lo: false,
        what: "the resource keep probability",
    },
    Bound {
        path: "radio.sidelink.rsrp_threshold_dbm",
        lo: -128.0,
        hi: -60.0,
        exclusive_lo: false,
        what: "the sensing RSRP threshold",
    },
    // `actors.vehicles.drivers`: the ranges of the IDM calibrations in Treiber and
    // Kesting, *Traffic Flow Dynamics* (2013) Table 11.2 and of Kesting 2007's MOBIL
    // politeness, opened a little either way.
    Bound {
        path: "actors.vehicles.drivers.max_accel_mps2",
        lo: 0.3,
        hi: 5.0,
        exclusive_lo: false,
        what: "the comfortable acceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.comfort_decel_mps2",
        lo: 0.5,
        hi: 6.0,
        exclusive_lo: false,
        what: "the comfortable deceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.time_headway_s",
        lo: 0.3,
        hi: 4.0,
        exclusive_lo: false,
        what: "the desired time headway",
    },
    Bound {
        path: "actors.vehicles.drivers.min_gap_m",
        lo: 0.5,
        hi: 10.0,
        exclusive_lo: false,
        what: "the standstill gap",
    },
    Bound {
        path: "actors.vehicles.drivers.politeness",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the MOBIL politeness",
    },
    Bound {
        path: "actors.vehicles.drivers.lane_change_threshold_mps2",
        lo: 0.0,
        hi: 2.0,
        exclusive_lo: false,
        what: "the lane-change threshold",
    },
    // `messages.rate_hz`: 1 Hz is the CAM floor (T_GenCamMax 1 s, EN 302 637-2 §6.1.3);
    // 50 Hz is five times J2945/1's rate, past which a 10 MHz channel cannot carry a
    // city fleet.
    Bound {
        path: "messages.rate_hz",
        lo: 1.0,
        hi: 50.0,
        exclusive_lo: false,
        what: "the message rate",
    },
    Bound {
        path: "threats.attackers[].fraction",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the attacker fraction",
    },
    // network audit: bounds of the fields the 2026-10-11 network-parameter audit exposed
    // (docs/guides/config-inventory-network.md). Sidelink noise figure as radio.phy's;
    // sl-ThresS-RSSI-CBR is -112 + 2n dBm, n = 0..45 (TS 36.331 SL-CBR-CommonTxConfig);
    // the GN MIB's timers and range are 16-bit in EN 302 636-4-1 Annex H, bounded here to
    // what a study uses; SPaT and MAP no faster than the 100 ms generation check.
    Bound {
        path: "radio.sidelink.noise_figure_db",
        lo: 0.0,
        hi: 15.0,
        exclusive_lo: false,
        what: "the sidelink noise figure",
    },
    Bound {
        path: "radio.sidelink.cbr_threshold_dbm",
        lo: -112.0,
        hi: -8.0,
        exclusive_lo: false,
        what: "the sidelink CBR threshold",
    },
    Bound {
        path: "radio.dcc.reactive_hold_ms",
        lo: 0.0,
        hi: 5_000.0,
        exclusive_lo: false,
        what: "the reactive DCC state-hold time",
    },
    Bound {
        path: "net.geonet.cbf_min_time_ms",
        lo: 0.0,
        hi: 1_000.0,
        exclusive_lo: false,
        what: "the CBF minimum time",
    },
    Bound {
        path: "net.geonet.cbf_max_time_ms",
        lo: 0.0,
        hi: 1_000.0,
        exclusive_lo: true,
        what: "the CBF maximum time",
    },
    Bound {
        path: "net.geonet.max_comm_range_m",
        lo: 0.0,
        hi: 5_000.0,
        exclusive_lo: true,
        what: "the GeoNetworking DIST_MAX",
    },
    Bound {
        path: "messages.spat_interval_ms",
        lo: 100.0,
        hi: 10_000.0,
        exclusive_lo: false,
        what: "the SPaT interval",
    },
    Bound {
        path: "messages.map_interval_ms",
        lo: 100.0,
        hi: 60_000.0,
        exclusive_lo: false,
        what: "the MAP interval",
    },
    Bound {
        path: "messages.cpm_interval_ms",
        lo: 100.0,
        hi: 1_000.0,
        exclusive_lo: false,
        what: "the CPM generation interval",
    },
    Bound {
        path: "messages.cpm_quality_threshold",
        lo: 0.0,
        hi: 15.0,
        exclusive_lo: false,
        what: "the object perception quality threshold",
    },
    Bound {
        path: "messages.cpm_max_objects",
        lo: 1.0,
        hi: 255.0,
        exclusive_lo: false,
        what: "the objects per CPM",
    },
    Bound {
        path: "messages.path_history_points",
        lo: 0.0,
        hi: 23.0,
        exclusive_lo: false,
        what: "the path-history points per message (J2735 PathHistoryPointList, 1..23)",
    },
    Bound {
        path: "nodes.verify_queue_depth",
        lo: 1.0,
        hi: 4_096.0,
        exclusive_lo: false,
        what: "the verification queue depth",
    },
    // --- SCMS settings audit (docs/guides/config-inventory-scms.md) ----------
    Bound {
        path: "security.scms.lifecycle.i_period_s",
        lo: 1.0,
        hi: 31_536_000.0,
        exclusive_lo: false,
        what: "the i-period",
    },
    Bound {
        path: "security.scms.lifecycle.cert_lifetime_s",
        lo: 1.0,
        hi: 31_536_000.0,
        exclusive_lo: false,
        what: "the pseudonym certificate lifetime",
    },
    Bound {
        path: "security.scms.lifecycle.certs_per_period",
        lo: 1.0,
        hi: 60.0,
        exclusive_lo: false,
        what: "the certificates per i-period",
    },
    Bound {
        path: "security.scms.lifecycle.pool_periods",
        lo: 1.0,
        hi: 64.0,
        exclusive_lo: false,
        what: "the i-periods held at the start",
    },
    Bound {
        path: "security.scms.lifecycle.topup_below_periods",
        lo: 0.0,
        hi: 64.0,
        exclusive_lo: false,
        what: "the top-up threshold",
    },
    Bound {
        path: "security.scms.lifecycle.max_periods_ahead",
        lo: 1.0,
        hi: 520.0,
        exclusive_lo: false,
        what: "the RA's provisioning horizon",
    },
    Bound {
        path: "security.scms.lifecycle.cert_shuffle_threshold",
        lo: 1.0,
        hi: 1_000_000.0,
        exclusive_lo: false,
        what: "the certificate-request shuffle threshold",
    },
    Bound {
        path: "security.scms.lifecycle.cert_shuffle_window_s",
        lo: 0.0,
        hi: 604_800.0,
        exclusive_lo: false,
        what: "the certificate-request shuffle window",
    },
    Bound {
        path: "security.scms.lifecycle.report_shuffle_threshold",
        lo: 1.0,
        hi: 1_000_000.0,
        exclusive_lo: false,
        what: "the report shuffle threshold",
    },
    Bound {
        path: "security.scms.lifecycle.report_shuffle_window_s",
        lo: 0.0,
        hi: 604_800.0,
        exclusive_lo: false,
        what: "the report shuffle window",
    },
    Bound {
        path: "security.scms.lifecycle.first_batch_delay_s",
        lo: 0.0,
        hi: 86_400.0,
        exclusive_lo: false,
        what: "the first download delay",
    },
    Bound {
        path: "security.scms.lifecycle.download_poll_interval_s",
        lo: 0.1,
        hi: 86_400.0,
        exclusive_lo: false,
        what: "download-minWait",
    },
    Bound {
        path: "security.scms.lifecycle.max_download_polls",
        lo: 1.0,
        hi: 10_000.0,
        exclusive_lo: false,
        what: "download-maxReqs",
    },
    Bound {
        path: "security.scms.lifecycle.download_request_tolerance_s",
        lo: 0.0,
        hi: 3_600.0,
        exclusive_lo: false,
        what: "the download request's clock tolerance",
    },
    Bound {
        path: "security.scms.lifecycle.enrolment_lifetime_s",
        lo: 1.0,
        hi: 315_360_000.0,
        exclusive_lo: false,
        what: "the enrolment certificate lifetime",
    },
    Bound {
        path: "security.scms.lifecycle.reenrol_lead_s",
        lo: 0.0,
        hi: 315_360_000.0,
        exclusive_lo: false,
        what: "the re-enrolment lead",
    },
    Bound {
        path: "security.scms.lifecycle.reenrol_download_delay_s",
        lo: 0.0,
        hi: 86_400.0,
        exclusive_lo: false,
        what: "the re-enrolment download delay",
    },
    Bound {
        path: "security.scms.lifecycle.crl_cadence_s",
        lo: 0.0,
        hi: 2_592_000.0,
        exclusive_lo: false,
        what: "the CRL publication cadence",
    },
    Bound {
        path: "security.scms.lifecycle.crl_fetch_interval_s",
        lo: 0.1,
        hi: 1_209_600.0,
        exclusive_lo: false,
        what: "the cellular CRL poll interval",
    },
    Bound {
        path: "security.scms.lifecycle.crl_broadcast_interval_s",
        lo: 0.1,
        hi: 3_600.0,
        exclusive_lo: false,
        what: "the roadside CRL repeat interval",
    },
    Bound {
        path: "security.scms.lifecycle.ma_review_s",
        lo: 0.0,
        hi: 2_592_000.0,
        exclusive_lo: false,
        what: "the MA operator's review",
    },
    Bound {
        path: "security.scms.lifecycle.ma_max_open_cases",
        lo: 1.0,
        hi: 1_000_000.0,
        exclusive_lo: false,
        what: "the MA's open-case capacity",
    },
    // SCMS Manager MBR v1.0 §2.2 / §2.3.1: the device report store.
    Bound {
        path: "security.scms.lifecycle.report_store_quota_bytes",
        lo: 0.0,
        hi: 1_000_000_000.0,
        exclusive_lo: false,
        what: "the report store's quota",
    },
    Bound {
        path: "security.scms.lifecycle.report_daily_multiplier",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the report store's daily multiplier",
    },
    Bound {
        path: "security.scms.lifecycle.report_decay_hour_et",
        lo: 0.0,
        hi: 23.0,
        exclusive_lo: false,
        what: "the report store's re-weighting hour",
    },
    Bound {
        path: "security.scms.lifecycle.report_weight_random_position",
        lo: 0.0,
        hi: 1_000.0,
        exclusive_lo: false,
        what: "a report subtype's initial weight",
    },
    Bound {
        path: "security.scms.lifecycle.report_weight_constant_position",
        lo: 0.0,
        hi: 1_000.0,
        exclusive_lo: false,
        what: "a report subtype's initial weight",
    },
    Bound {
        path: "security.scms.lifecycle.report_weight_max_speed",
        lo: 0.0,
        hi: 1_000.0,
        exclusive_lo: false,
        what: "a report subtype's initial weight",
    },
    Bound {
        path: "security.scms.lifecycle.report_weight_max_acceleration",
        lo: 0.0,
        hi: 1_000.0,
        exclusive_lo: false,
        what: "a report subtype's initial weight",
    },
    Bound {
        path: "security.scms.lifecycle.report_weight_brake_acceleration",
        lo: 0.0,
        hi: 1_000.0,
        exclusive_lo: false,
        what: "a report subtype's initial weight",
    },
    // Zero is the strictest device (refuse the moment `nextCrl` passes); a year is far
    // past any profile (CAMP 1.2.2 Table 67: two weeks).
    Bound {
        path: "security.scms.crl_overdue_tolerance_s",
        lo: 0.0,
        hi: 31_536_000.0,
        exclusive_lo: false,
        what: "the overdue-CRL tolerance",
    },
    // The generationTime relevance window (IEEE 1609.2 §5.2; C2C-CC BSP RS_BSP_168:
    // 2 s, 10 min, 40 ms). Zero is the strictest receiver; a year turns a bound off in
    // effect, for a study of the system without it.
    Bound {
        path: "security.scms.rx_beacon_max_age_s",
        lo: 0.0,
        hi: 31_536_000.0,
        exclusive_lo: false,
        what: "the oldest safety beacon a receiver accepts",
    },
    Bound {
        path: "security.scms.rx_max_age_s",
        lo: 0.0,
        hi: 31_536_000.0,
        exclusive_lo: false,
        what: "the oldest application message a receiver accepts",
    },
    Bound {
        path: "security.scms.rx_max_future_s",
        lo: 0.0,
        hi: 31_536_000.0,
        exclusive_lo: false,
        what: "how far in the future a receiver accepts a generation time",
    },
    Bound {
        path: "nodes.validated_cert_cache",
        lo: 1.0,
        hi: 1_048_576.0,
        exclusive_lo: false,
        what: "the validated-certificate cache size",
    },
    // --- security engine fixes (2026-10-12) ---------------------------------
    // A relevance score is a ratio in [0, 1] (v2xw-node `relevance_from_ttc`); 0 would
    // verify everything and be `verify-all`, so it is excluded.
    Bound {
        path: "security.on_demand_threshold",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: true,
        what: "the on-demand relevance threshold (a score in (0, 1])",
    },
    // 06-node-models.md §2.1 scores proximity over the awareness range; J2945/1 and ETSI
    // CAM receivers care out to about 300 m (the DSRC/C-V2X design range), so 10 m to
    // 2 km brackets any useful horizon.
    Bound {
        path: "security.prioritized_horizon_m",
        lo: 10.0,
        hi: 2_000.0,
        exclusive_lo: false,
        what: "the prioritized proximity horizon, metres",
    },
    // A unit must hold at least one certificate; 65 536 is far above any neighbourhood
    // a 1 km radio range holds.
    // A lower edge of the HSM signing time; the upper edge is the profile's own bound,
    // checked against it in `check_hsm_sign_lower`.
    Bound {
        path: "nodes.hsm_sign_lower_us",
        lo: 0.0,
        hi: 1_000_000.0,
        exclusive_lo: true,
        what: "the lower edge of the HSM signing time",
    },
    Bound {
        path: "nodes.peer_cert_cache",
        lo: 1.0,
        hi: 65_536.0,
        exclusive_lo: false,
        what: "the peer certificate cache size",
    },
    // --- traffic audit (2026-10-11) ----------------------------------------
    // HCM 6th ed. (2016) ch. 20's two-way-stop critical gaps run 4.1-7.5 s and field
    // re-measurements (e.g. LTRC Report 715, 2025) fall within about +-40 % of them; a
    // factor of 0.5-2 covers that with room either side.
    Bound {
        path: "traffic.critical_gap_factor",
        lo: 0.5,
        hi: 2.0,
        exclusive_lo: false,
        what: "the critical-gap factor",
    },
    // A gap shorter than the 2.5 s follow-up headway is not a gap; 8 s is past any HCM
    // critical gap (HCM 6th ed. ch. 31 uses 4.5 s).
    Bound {
        path: "traffic.permitted_left_gap_s",
        lo: 2.5,
        hi: 8.0,
        exclusive_lo: false,
        what: "the permitted left-turn critical gap",
    },
    // WP-04. Field critical-gap spreads run cv 0.15-0.2 (SUMO P1); 0.5 is past any.
    Bound {
        path: "traffic.critical_gap_cv",
        lo: 0.0,
        hi: 0.5,
        exclusive_lo: false,
        what: "the critical-gap coefficient of variation",
    },
    // 0 is off; SUMO's default is 180 s, and an hour is past any junction wait.
    Bound {
        path: "traffic.time_to_impatience_s",
        lo: 0.0,
        hi: 3600.0,
        exclusive_lo: false,
        what: "the time to full impatience",
    },
    // A floor of 1 is no impatience; below 0.3 the HCM 4.5 s headway would fall under the
    // 1.4 s a car needs to clear a lane.
    Bound {
        path: "traffic.impatience_gap_floor",
        lo: 0.3,
        hi: 1.0,
        exclusive_lo: false,
        what: "the critical-gap floor at full impatience",
    },
    // The HCM credits 2 sneakers per cycle; 4 is past what a junction mouth holds.
    Bound {
        path: "traffic.permissive_sneakers",
        lo: 0.0,
        hi: 4.0,
        exclusive_lo: false,
        what: "the sneakers per cycle",
    },
    // 0 turns the turn-speed limit off; 10 m/s² is about 1 g, past any tyre's grip
    // (SUMO's default is 5.5).
    Bound {
        path: "traffic.turn_lateral_accel_mps2",
        lo: 0.0,
        hi: 10.0,
        exclusive_lo: false,
        what: "the turning lateral acceleration",
    },
    // 0 is the whole connector (SUMO's average radius); 30 m is longer than any urban
    // junction connector, where the window is the connector again.
    Bound {
        path: "traffic.turn_curve_window_m",
        lo: 0.0,
        hi: 30.0,
        exclusive_lo: false,
        what: "the turn-speed curve window",
    },
    // SUMO LC2013 eagerness factors (sumo.dlr.de "Definition of Vehicles, Vehicle Types,
    // and Routes", lane-changing models): default 1, documented in [0, inf); 0 turns a
    // motivation off. lcCooperative is a probability-like weight in [0, 1]. 10 is ten
    // times SUMO's eagerness, beyond any published calibration (Erdmann 2015).
    Bound {
        path: "traffic.lc_strategic",
        lo: 0.0,
        hi: 10.0,
        exclusive_lo: false,
        what: "the LC2013 strategic eagerness",
    },
    Bound {
        path: "traffic.lc_cooperative",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the LC2013 cooperativeness",
    },
    Bound {
        path: "traffic.lc_speed_gain",
        lo: 0.0,
        hi: 10.0,
        exclusive_lo: false,
        what: "the LC2013 speed-gain eagerness",
    },
    Bound {
        path: "traffic.lc_keep_right",
        lo: 0.0,
        hi: 10.0,
        exclusive_lo: false,
        what: "the LC2013 keep-right eagerness",
    },
    // TCRP Report 19 (1996): urban local-bus stops 150-400 m apart, limited-stop and BRT
    // up to about 1 km; 100-2,000 m covers both with room.
    Bound {
        path: "traffic.kerbside.bus_stop_spacing_m",
        lo: 100.0,
        hi: 2000.0,
        exclusive_lo: false,
        what: "the bus stop spacing",
    },
    // TCQSM 3rd ed. (TCRP Report 165) ch. 6: dwell times of 15-60 s typical, up to a few
    // minutes at a major stop.
    Bound {
        path: "traffic.kerbside.bus_dwell_mean_s",
        lo: 5.0,
        hi: 300.0,
        exclusive_lo: false,
        what: "the mean bus dwell",
    },
    Bound {
        path: "traffic.kerbside.taxi_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the taxi share",
    },
    // A pick-up or drop-off: seconds to a few minutes; 10 minutes is a guard.
    Bound {
        path: "traffic.kerbside.taxi_dwell_median_s",
        lo: 5.0,
        hi: 600.0,
        exclusive_lo: false,
        what: "the median taxi dwell",
    },
    Bound {
        path: "traffic.kerbside.delivery_stop_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the delivery stop share",
    },
    // Urban freight double-parking studies report dwells of a few minutes to about half
    // an hour; an hour is a guard.
    Bound {
        path: "traffic.kerbside.delivery_dwell_median_s",
        lo: 30.0,
        hi: 3600.0,
        exclusive_lo: false,
        what: "the median delivery dwell",
    },
    // SUMO's bluelight device reacts at 25 m by default (`reactiondist`); a siren is heard
    // and a lightbar seen over a few hundred metres in a quiet street, much less in
    // traffic (S-P5: calibrate against siren audibility). 1-500 m covers both.
    Bound {
        path: "traffic.emergency_yield.reaction_distance_m",
        lo: 1.0,
        hi: 500.0,
        exclusive_lo: false,
        what: "the emergency-vehicle reaction distance",
    },
    // UVC 11-106(b)2 gives no figure; 0 (a full stop) to 10 m/s (36 km/h) covers a stop,
    // a crawl and the slow roll some ambulance services allow.
    Bound {
        path: "traffic.emergency_yield.red_pass_speed_mps",
        lo: 0.0,
        hi: 10.0,
        exclusive_lo: false,
        what: "the emergency-vehicle red-pass speed",
    },
    // SUMO's speedFactor: observed urban free-flow means are about 0.9-1.2 of the limit;
    // 0.5-1.5 covers it with room either side. The spread multiplies SUMO's speedDev
    // (0.1 for a car): 3 is a 30 % deviation.
    Bound {
        path: "actors.vehicles.drivers.speed_factor",
        lo: 0.5,
        hi: 1.5,
        exclusive_lo: false,
        what: "the mean speed factor",
    },
    Bound {
        path: "actors.vehicles.drivers.speed_spread",
        lo: 0.0,
        hi: 3.0,
        exclusive_lo: false,
        what: "the speed spread",
    },
    Bound {
        path: "actors.vehicles.drivers.red_runner_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share of drivers who run reds",
    },
    Bound {
        path: "actors.vehicles.drivers.violations.red_run_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share of red-light runners",
    },
    Bound {
        path: "actors.vehicles.drivers.violations.red_run_tolerance_mean_s",
        lo: 0.0,
        hi: 4.0,
        exclusive_lo: false,
        what: "the mean red-light tolerance",
    },
    Bound {
        path: "actors.vehicles.drivers.violations.red_run_tolerance_max_s",
        lo: 0.0,
        hi: 16.0,
        exclusive_lo: false,
        what: "the longest red-light tolerance",
    },
    Bound {
        path: "actors.vehicles.drivers.violations.amber_tail_max_s",
        lo: 5.5,
        hi: 12.0,
        exclusive_lo: false,
        what: "the longest amber threshold",
    },
    Bound {
        path: "actors.vehicles.drivers.violations.fail_to_yield_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the failure-to-yield probability",
    },
    Bound {
        path: "actors.vehicles.drivers.violations.ignore_foe_speed_mps",
        lo: 0.0,
        hi: 40.0,
        exclusive_lo: false,
        what: "the fastest foe ignored",
    },
    Bound {
        path: "actors.vehicles.drivers.violations.crash_clearance_s",
        lo: 0.0,
        hi: 86400.0,
        exclusive_lo: false,
        what: "the crash clearance time",
    },
    // The engine clamps each driver's draw to 0.3-3 s (Taoka 1989's lognormal: median
    // 1.1-1.2 s); a median outside that would be clamped flat.
    Bound {
        path: "actors.vehicles.drivers.start_reaction_s",
        lo: 0.3,
        hi: 3.0,
        exclusive_lo: false,
        what: "the median start-up reaction",
    },
    // Greenshields 1947 discharge headways imply a delay under 1 s; the engine clamps
    // each driver to 0.2-3 s.
    Bound {
        path: "actors.vehicles.drivers.queue_start_delay_s",
        lo: 0.05,
        hi: 3.0,
        exclusive_lo: false,
        what: "the median queue start-up delay",
    },
    // Per-class car-following numbers: the passenger rows' ranges (Treiber and Kesting
    // 2013 Table 11.2), which take in SUMO's truck (accel 1.3, decel 4.0, minGap 2.5),
    // bus (1.2, 4.0, 2.5), motorcycle (6.0 accel is capped at 5) and bicycle (1.2, 3.0,
    // 0.5) vTypes.
    Bound {
        path: "actors.vehicles.drivers.truck.max_accel_mps2",
        lo: 0.3,
        hi: 5.0,
        exclusive_lo: false,
        what: "the truck acceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.truck.comfort_decel_mps2",
        lo: 0.5,
        hi: 6.0,
        exclusive_lo: false,
        what: "the truck comfortable deceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.truck.time_headway_s",
        lo: 0.3,
        hi: 4.0,
        exclusive_lo: false,
        what: "the truck time headway",
    },
    Bound {
        path: "actors.vehicles.drivers.truck.min_gap_m",
        lo: 0.3,
        hi: 10.0,
        exclusive_lo: false,
        what: "the truck standstill gap",
    },
    Bound {
        path: "actors.vehicles.drivers.bus.max_accel_mps2",
        lo: 0.3,
        hi: 5.0,
        exclusive_lo: false,
        what: "the bus acceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.bus.comfort_decel_mps2",
        lo: 0.5,
        hi: 6.0,
        exclusive_lo: false,
        what: "the bus comfortable deceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.bus.time_headway_s",
        lo: 0.3,
        hi: 4.0,
        exclusive_lo: false,
        what: "the bus time headway",
    },
    Bound {
        path: "actors.vehicles.drivers.bus.min_gap_m",
        lo: 0.3,
        hi: 10.0,
        exclusive_lo: false,
        what: "the bus standstill gap",
    },
    Bound {
        path: "actors.vehicles.drivers.motorcycle.max_accel_mps2",
        lo: 0.3,
        hi: 5.0,
        exclusive_lo: false,
        what: "the motorcycle acceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.motorcycle.comfort_decel_mps2",
        lo: 0.5,
        hi: 6.0,
        exclusive_lo: false,
        what: "the motorcycle comfortable deceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.motorcycle.time_headway_s",
        lo: 0.3,
        hi: 4.0,
        exclusive_lo: false,
        what: "the motorcycle time headway",
    },
    Bound {
        path: "actors.vehicles.drivers.motorcycle.min_gap_m",
        lo: 0.3,
        hi: 10.0,
        exclusive_lo: false,
        what: "the motorcycle standstill gap",
    },
    Bound {
        path: "actors.vehicles.drivers.bicycle.max_accel_mps2",
        lo: 0.3,
        hi: 5.0,
        exclusive_lo: false,
        what: "the bicycle acceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.bicycle.comfort_decel_mps2",
        lo: 0.5,
        hi: 6.0,
        exclusive_lo: false,
        what: "the bicycle comfortable deceleration",
    },
    Bound {
        path: "actors.vehicles.drivers.bicycle.time_headway_s",
        lo: 0.3,
        hi: 4.0,
        exclusive_lo: false,
        what: "the bicycle time headway",
    },
    Bound {
        path: "actors.vehicles.drivers.bicycle.min_gap_m",
        lo: 0.3,
        hi: 10.0,
        exclusive_lo: false,
        what: "the bicycle standstill gap",
    },
    // FHWA Signal Timing Manual (2008) ch. 5: cycles of 60-150 s typical, rarely above
    // 180 s; 30 s is a two-phase minimum with short greens.
    Bound {
        path: "world.signals.cycle_s",
        lo: 30.0,
        hi: 180.0,
        exclusive_lo: false,
        what: "the signal cycle length",
    },
    // Webster (1958) and HCM 6th ed. ch. 19 split green by the critical flow ratios; the
    // procedural grid's own check (`GridParams::validate`) holds the same 0.1-0.9, which
    // leaves each street at least 5.4 s of green at the 60 s default cycle.
    Bound {
        path: "world.source.params.ew_green_share",
        lo: 0.1,
        hi: 0.9,
        exclusive_lo: false,
        what: "the east-west share of a generated grid's green",
    },
    // MUTCD 2009 §4D.26: a red clearance shall not exceed 6 s.
    Bound {
        path: "world.signals.all_red_s",
        lo: 0.0,
        hi: 6.0,
        exclusive_lo: false,
        what: "the all-red floor",
    },
    // MUTCD 2009 §4E.06: walk at least 7 s, 4 s where pedestrian volumes are low; 30 s
    // is a guard.
    Bound {
        path: "world.signals.pedestrian_walk_s",
        lo: 4.0,
        hi: 30.0,
        exclusive_lo: false,
        what: "the pedestrian walk interval",
    },
    // MUTCD 2009 §4E.06: 3.5 ft/s (1.067 m/s); 3.0 ft/s (0.91 m/s) where slower
    // pedestrians cross; 0.8-1.5 m/s brackets observed walking speeds.
    Bound {
        path: "world.signals.pedestrian_speed_mps",
        lo: 0.8,
        hi: 1.5,
        exclusive_lo: false,
        what: "the pedestrian clearance walking speed",
    },
    // SUMO netconvert `--tls.join-dist` defaults to 20 m. Above 40 m two junctions a
    // short block apart would merge, and the importer refuses any cluster wider than
    // 45 m (`v2xw_world::osm::JOIN_MAX_SPREAD_M`) anyway.
    Bound {
        path: "world.signals.join_m",
        lo: 0.0,
        hi: 40.0,
        exclusive_lo: false,
        what: "the signalised-junction join distance",
    },
    // GNSS: 0.01 outliers/s of 12 m and 0.006 bursts/s are the legacy engine's draws
    // (04-models.md §3.8); the bounds are guards around them.
    Bound {
        path: "actors.vehicles.gnss.outlier_rate_hz",
        lo: 0.0,
        hi: 0.5,
        exclusive_lo: false,
        what: "the GNSS outlier rate",
    },
    Bound {
        path: "actors.vehicles.gnss.outlier_magnitude_m",
        lo: 1.0,
        hi: 200.0,
        exclusive_lo: false,
        what: "the GNSS outlier magnitude",
    },
    Bound {
        path: "actors.vehicles.gnss.burst_rate_hz",
        lo: 0.0,
        hi: 0.5,
        exclusive_lo: false,
        what: "the GNSS degraded-burst rate",
    },
    Bound {
        path: "actors.vehicles.gnss.outage_probability",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the GNSS outage probability",
    },
    // Receiver smoothing (R-USFP): RTCA DO-229's carrier-smoothing constant is 100 s; the
    // bounds are guards around it. The static hold is a receiver setting, at most a
    // walking pace.
    Bound {
        path: "actors.vehicles.gnss.smoothing_tau_s",
        lo: 0.0,
        hi: 600.0,
        exclusive_lo: false,
        what: "the GNSS receiver smoothing time constant",
    },
    Bound {
        path: "actors.vehicles.gnss.static_hold_mps",
        lo: 0.0,
        hi: 1.5,
        exclusive_lo: false,
        what: "the GNSS receiver static-hold threshold",
    },
    // The receiver's white velocity output noise: 0 to five times the SPS committed sd.
    Bound {
        path: "actors.vehicles.gnss.velocity_noise_mps",
        lo: 0.0,
        hi: 0.5,
        exclusive_lo: false,
        what: "the GNSS receiver velocity output noise",
    },
    // The stated one-sigma accuracy: J2735 SemiMajorAxisAccuracy tops out at 12.70 m.
    Bound {
        path: "actors.vehicles.gnss.stated_sigma_m",
        lo: 0.0,
        hi: 12.7,
        exclusive_lo: false,
        what: "the GNSS receiver's stated one-sigma accuracy",
    },
    Bound {
        path: "actors.vehicles.gnss.stated_sigma_log_sd",
        lo: 0.0,
        hi: 3.0,
        exclusive_lo: false,
        what: "the per-device spread of the stated accuracy",
    },
    // The per-device BSM timing model (Paper 1 item B): a share.
    Bound {
        path: "actors.vehicles.bsm_timing.locked_share",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share of OBUs whose BSM timestamp is locked to the GNSS fix epoch",
    },
    Bound {
        path: "actors.vehicles.bsm_timing.stamp_chain.stay_minus1",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the chance that a BSM stamped 1 ms early is followed by another",
    },
    Bound {
        path: "actors.vehicles.bsm_timing.stamp_chain.stay_zero",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the chance that a BSM stamped on the fix epoch is followed by another",
    },
    // p1-realdata-baseline.md §7.5 Finding 6: Tampa chain devices 0.499, fixed-stamp
    // devices 0.92, the Wyoming OBU 1.02 m/s per m; 5 is far past every one.
    Bound {
        path: "actors.vehicles.bsm_timing.stamp_chain.speed_per_m",
        lo: -5.0,
        hi: 5.0,
        exclusive_lo: false,
        what: "the reported speed's change per metre of a stamp-chain position step",
    },
    Bound {
        path: "actors.vehicles.bsm_timing.stamp_chain.speed_tau_s",
        lo: 0.0,
        hi: 60.0,
        exclusive_lo: true,
        what: "the decay time of a stamp-chain step's speed change",
    },
    Bound {
        path: "actors.vehicles.bsm_timing.stamp_chain.position_follows",
        lo: 0.0,
        hi: 1.0,
        exclusive_lo: false,
        what: "the share of a step's speed change the reported position moves with",
    },
    // --- end traffic audit ----------------------------------------------------
];

/// One closed set of values the loader accepts and the published schema offers.
#[derive(Debug, Clone, Copy)]
pub struct Choices {
    /// The dotted scenario path.
    pub path: &'static str,
    /// The accepted values, in a stable order.
    pub values: &'static [&'static str],
    /// A path whose value narrows this set further, and the narrowing value; empty when
    /// the set is unconditional.
    ///
    /// `security.signature` is the case: `modeled` cryptography costs any primitive the
    /// hardware profile prices, and `real` has to do the mathematics. A form can offer
    /// the wide set and grey out what the current mode cannot do, which is what the
    /// loader enforces.
    pub narrowed_by: &'static str,
    /// The value of `narrowed_by` that applies the narrowing.
    pub narrowed_when: &'static str,
    /// The narrower set, when `narrowed_by` is set.
    pub narrowed_to: &'static [&'static str],
}

/// A choice set with no conditional narrowing.
const fn choices(path: &'static str, values: &'static [&'static str]) -> Choices {
    Choices {
        path,
        values,
        narrowed_by: "",
        narrowed_when: "",
        narrowed_to: &[],
    }
}

/// Every closed value set in the schema whose members are fixed strings.
///
/// Sets whose members come from a registry — model ids, hardware profile ids — are
/// published as slots by `crate::scenario::publish` instead, because their membership is
/// a property of the build rather than a constant.
pub static CHOICES: &[Choices] = &[
    choices("time.des_resolution", &RESOLUTIONS),
    choices("net.layer", &NET_LAYERS),
    choices("world.sumo.signal_programmes", &["as-file", "static"]),
    choices("net.geonet.area_forwarding", &GN_AREA_FORWARDING),
    choices("net.geonet.non_area_forwarding", &GN_NON_AREA_FORWARDING),
    choices("net.geonet.cbf_variant", &GN_CBF_VARIANTS),
    choices("net.channel_access", &CHANNEL_ACCESS),
    choices("radio.mac.periodic_access_category", &ACCESS_CATEGORIES),
    choices("radio.mac.event_access_category", &ACCESS_CATEGORIES),
    choices("radio.channel_model.fading", &FADING_LAWS),
    choices("radio.channel_model.fading_coherence", &FADING_COHERENCE),
    choices("net.geonet.forwarding", &GN_FORWARDING),
    choices("net.access_categories", &NET_ACCESS_CATEGORIES),
    choices("net.dcc_cbr", &DCC_CBR),
    choices("net.rsu_ip.air", &RSU_IP_AIR),
    choices(
        "actors.vehicles.bsm_timing.stamp_chain.profile",
        &v2xw_node::bsm_timing::StampChain::PROFILES,
    ),
    choices("messages.codec_tier", &CODEC_TIERS),
    choices("messages.sets[]", &MESSAGE_SETS),
    choices("security.envelope", &ENVELOPES),
    choices("security.scms.tls", &SCMS_TLS),
    choices("security.scms.crl_overdue_scope", &SCMS_CRL_OVERDUE_SCOPES),
    choices("security.verification_policy", &POLICIES),
    choices("security.pseudonym_change.strategy", &STRATEGIES),
    choices("scale.traffic.model", &crate::scale::TRAFFIC_MODELS),
    Choices {
        path: "security.signature",
        values: &crate::signature::SIGNATURES,
        narrowed_by: "security.crypto_mode",
        narrowed_when: "real",
        narrowed_to: &REAL_SIGNATURES,
    },
    choices("security.kem", &crate::signature::KEMS),
    // network audit
    choices("radio.dcc.reactive_table", &DCC_REACTIVE_TABLES),
];

/// How much of a scenario key this build actually acts on.
///
/// The distinction the page needs is not "valid or invalid" — the loader answers that —
/// but "will editing this change the run". 13-product-direction.md §2 is explicit that a
/// field the engine does not act on must not be offered as though it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// The engine reads it and it changes the run.
    Wired,
    /// The engine reads it and acts on part of it, or acts on it only under conditions
    /// the note states. Editing it may do less than it appears to.
    Partial,
    /// Validated, hashed, and read by nothing. Editing it changes the scenario hash and
    /// nothing else.
    NotImplemented,
    /// Validated and refused: the loader rejects any value this build cannot act on, so
    /// the field exists but only its implemented values load.
    Refused,
    /// Describes the scenario rather than configuring the run: shown in the page and kept
    /// with the scenario in the run's record, and by design it changes nothing computed.
    /// Distinct from [`Status::NotImplemented`], which is a setting the engine *should*
    /// act on and does not.
    Descriptive,
}

/// One key's implementation status.
#[derive(Debug, Clone, Copy)]
pub struct KeyStatus {
    /// The dotted path, or a prefix of one. The longest matching entry wins, so a
    /// section can be classified once and an exception stated beneath it.
    pub path: &'static str,
    /// How much of it is real.
    pub status: Status,
    /// One line a non-specialist can read, saying what happens if they edit it.
    pub note: &'static str,
}

/// The implementation status of every scenario key.
///
/// Written from the 2026-09-22 wiring audit, which traced every leaf of the schema to
/// the code that reads it. Entries are prefixes, longest match wins, and
/// `crate::scenario::publish`'s `every_leaf_has_a_status` test fails the build if a leaf
/// of the reflected schema matches none of them — so a field added to the schema cannot
/// reach the page unclassified.
pub static KEY_STATUS: &[KeyStatus] = &[
    // --- the run -----------------------------------------------------------
    KeyStatus {
        path: "schema",
        status: Status::Wired,
        note: "The schema version. The loader migrates by it.",
    },
    KeyStatus {
        path: "meta",
        status: Status::Descriptive,
        note: "Describes the scenario: shown with it in the page's scenario list and kept \
               with the scenario in every recording and in the scenario digest. By design \
               it changes nothing the run computes.",
    },
    KeyStatus {
        path: "meta.name",
        status: Status::Wired,
        note: "Names the output directory, the recording's run label and the manifest.",
    },
    KeyStatus {
        path: "meta.base",
        status: Status::Wired,
        note: "The scenario this one overlays; resolved by the loader before anything \
               else.",
    },
    KeyStatus {
        path: "seed",
        status: Status::Wired,
        note: "The master seed. Every random draw in the run derives from it.",
    },
    KeyStatus {
        path: "time.t0",
        status: Status::Wired,
        note: "The civil instant simulated time zero is; every 1609.2 generationTime is \
               stamped from it.",
    },
    KeyStatus {
        path: "time.duration_s",
        status: Status::Wired,
        note: "The run horizon.",
    },
    KeyStatus {
        path: "time.mobility_step_ms",
        status: Status::Wired,
        note: "The mobility period, and the step the stream and run.seek are quantised \
               to.",
    },
    KeyStatus {
        path: "time.warmup_s",
        status: Status::Wired,
        note: "Metric samples whose window ends inside the warm-up are not recorded, so \
               charts and summaries start once traffic has built up. The run report's \
               whole-run counters still count it.",
    },
    KeyStatus {
        path: "time.metric_period_s",
        status: Status::Wired,
        note: "How often the metric providers flush a sample; one second by default. A \
               run of days at 60 s writes a sixtieth of the samples.",
    },
    KeyStatus {
        path: "time.des_resolution",
        status: Status::Refused,
        note: "The resolution the run promises its models. The kernel keeps nanoseconds \
               whatever this says; the loader refuses a resolution the run cannot keep — \
               '1ms' with a PHY or MAC that times frames in microseconds, or a \
               time-dilation window off the promised grid.",
    },
    KeyStatus {
        path: "time.calendar",
        status: Status::Wired,
        note: "real (the default): one-week i-periods from the US epoch, 2015-01-06 04:00 \
               Eastern, the deployment's CRL publication rule and the CV-pilot manual MA. \
               compressed: the only way to state a shorter security.protocol.params.\
               i_period_s; the i-period counts from the run's start, the MA is automatic \
               unless ma_review_s says otherwise, and the manifest and run report label it.",
    },
    KeyStatus {
        path: "time.fast_forward",
        status: Status::Wired,
        note: "Windows in which the traffic is frozen, no radio frame is generated, and \
               the run jumps from one credential-system event to the next on the \
               mobility-step grid (top-ups, shuffles, CRL publication and fetches, \
               re-enrolment, root rollovers), so weeks of PKI behaviour fit in a short \
               run. Requires a credential backend. Recorded in the manifest.",
    },
    KeyStatus {
        path: "time.time_dilation",
        status: Status::Partial,
        note: "Inside a window no radio frame is generated: the frames are counted as \
               suppressed in the run report and in run.status. The mobility tier is not \
               lowered inside a window, and no metric is marked not-observed for one.",
    },
    // --- the world ---------------------------------------------------------
    KeyStatus {
        path: "world.source",
        status: Status::Wired,
        note: "Where the world comes from. Procedural grids and OpenStreetMap XML are \
               built; the other source kinds return an unsupported-source error.",
    },
    KeyStatus {
        path: "world.imported_at",
        status: Status::Wired,
        note: "The import date the world's provenance records. Supplied here because no \
               part of the engine may read a clock.",
    },
    KeyStatus {
        path: "world.buildings.enabled",
        status: Status::Wired,
        note: "Whether buildings obstruct radio links. On, at the medium tier a link \
               through a building loses 9 dB per wall and 0.4 dB per metre inside (Sommer \
               2011), capped at 3GPP TR 37.885's urban NLOS loss; at the high tier the \
               street corner the link turns round is traced and priced by the Mangel 2011 \
               intersection model (TR 37.885 NLOS where no single corner connects), or \
               the path through the building when that is stronger. Off, every link is \
               line of sight. Buildings are imported and drawn either way.",
    },
    KeyStatus {
        path: "world.buildings.keep_holes",
        status: Status::Wired,
        note: "Whether interior courtyards stay holes in a footprint.",
    },
    KeyStatus {
        path: "world.buildings.metres_per_level",
        status: Status::Wired,
        note: "Overrides the importer's storey height for buildings tagged with levels \
               rather than a height.",
    },
    KeyStatus {
        path: "world.terrain",
        status: Status::Wired,
        note: "A digital elevation model (an SRTM .hgt tile or a geographic ESRI ASCII \
               grid) is read, resampled onto the world, and obstructs radio links by \
               ITU-R P.526 knife-edge diffraction over the ground profile. Roads and \
               buildings are not lifted onto it. No file: the world is flat.",
    },
    KeyStatus {
        path: "world.cache",
        status: Status::Wired,
        note: "A directory: the imported world is kept there, keyed by the world section \
               and the source file's contents, and read back exactly on the next run \
               instead of importing again.",
    },
    KeyStatus {
        path: "world.highway_preset",
        status: Status::Wired,
        note: "Which jurisdiction's fallback speed limits (and, for urban-us-nyc, lane \
               widths) the OpenStreetMap importer uses: urban-us-nyc, urban-us-portland, \
               urban-de or sumo-german; it also selects the rules of the road the traffic \
               follows (right turn on red: prohibited in New York City, which is also the \
               rule without a preset, and in Germany; permitted after a stop in Portland). \
               An OSM import is refused without it.",
    },
    KeyStatus {
        path: "world.terrain.drape",
        status: Status::Wired,
        note: "Lays lanes, junctions, buildings, sites and signal heads on world.terrain.dem \
               (z += the DEM height), so traffic.vehicle_dynamics.grade has a slope to read. \
               Off by default (the DEM is then only the radio's ground); needs a DEM.",
    },
    KeyStatus {
        path: "world.sumo.signal_programmes",
        status: Status::Wired,
        note: "A SUMO net.xml's actuated, delay-based and NEMA tlLogics run as live \
               controllers (as-file, the default) with their phases' minDur/maxDur and \
               their params, every attribute and param counted in the import report's \
               tl_ledger; delay-based runs as actuated, counted. static flattens every \
               programme to its phase list, as before WP-23. SUMO-network worlds only.",
    },
    KeyStatus {
        path: "world.vegetation",
        status: Status::Wired,
        note: "The OSM importer keeps woods, forests, tree rows and single trees as a \
               vegetation layer for radio.channel_model.foliage. Off by default; OSM \
               imports only.",
    },
    KeyStatus {
        path: "radio.channel_model.foliage",
        status: Status::Wired,
        note: "Foliage excess on paths through world.vegetation: depth times 2.3 dB/m at \
               5.9 GHz (GEMV2 eq. 4), capped at the knife-edge loss over a 15 m canopy \
               (P.833 s3.1).",
    },
    KeyStatus {
        path: "radio.channel_model.density_sigma",
        status: Status::Wired,
        note: "GEMV2 eq. 10: the log-distance law's shadowing sigma from the vehicles and \
               building footprints inside each link's ellipse (Table V bounds).",
    },
    KeyStatus {
        path: "radio.channel_model.reflection",
        status: Status::Wired,
        note: "GEMV2 single-bounce building reflection: the strongest specular ray off a \
               footprint wall with clear legs (free-space spreading plus the TE Fresnel \
               loss, P.2040-3 Table 3 walls), power-summed with a building-blocked path.",
    },
    KeyStatus {
        path: "world.service_roads",
        status: Status::Wired,
        note: "Whether highway=service ways (alleys, driveways, parking aisles) become \
               motor-traffic lanes. On by default; a city of the library turns it off, \
               because downtown alleys run hard against building walls and a vehicle routed \
               through one clips the buildings.",
    },
    KeyStatus {
        path: "world.signals.coordinate",
        status: Status::Wired,
        note: "Whether the OpenStreetMap importer offsets its synthesised signal plans for \
               a green wave along each major road (each plan's major green starts one \
               travel time after the one upstream). Off, every plan starts at t0, as \
               netconvert's do.",
    },
    KeyStatus {
        path: "world.signals.progression_speed_mps",
        status: Status::Wired,
        note: "The green wave's speed. Absent, each link's speed limit: 25 mph in \
               Manhattan, NYC DOT's retimed progression. NYC DOT's cyclist Green Wave \
               avenues run at 15 mph (6.7 m/s); downtown Portland's signals reward 11-13.5 \
               mph.",
    },
    // --- what moves --------------------------------------------------------
    KeyStatus {
        path: "actors.vehicles.demand.kind",
        status: Status::Wired,
        note: "Which demand model runs: 'mobility/demand/none', 'mobility/demand/poisson' \
               (the thinned-Poisson model) or 'mobility/demand/tr36885-drop' (the 3GPP \
               TR 36.885 vehicle drop). Any other id is refused.",
    },
    KeyStatus {
        path: "actors.vehicles.demand.rate_veh_per_h",
        status: Status::Wired,
        note: "Vehicles per hour offered to the network.",
    },
    KeyStatus {
        path: "perception",
        status: Status::Wired,
        note: "The sensors of the vehicles and roadside units that send CPMs, and how a \
               road user is hidden from them (WP-14); needs 'cpm' in messages.sets.",
    },
    KeyStatus {
        path: "perception.occlusion",
        status: Status::Wired,
        note: "centre-line (the line to the reference point, vehicles as discs; the \
               default) or outline (any visible corner of the target's outline, vehicles \
               as their body rectangles).",
    },
    KeyStatus {
        path: "perception.vehicle",
        status: Status::Wired,
        note: "Every CPM-sending vehicle's suite: each sensor's kind, range, opening, \
               boresight azimuth and mount; absent is the default front suite.",
    },
    KeyStatus {
        path: "perception.classes",
        status: Status::Wired,
        note: "A suite per vehicle class (a truck's own radars, a bus's), over \
               perception.vehicle.",
    },
    KeyStatus {
        path: "perception.rsu",
        status: Status::Wired,
        note: "The suite of every roadside unit with the 'cpm' role, east-north-up from \
               its antenna; its objects go out in infrastructure CPMs.",
    },
    KeyStatus {
        path: "traffic",
        status: Status::Wired,
        note: "How the traffic behaves at junctions and in jams; every field changes the run.",
    },
    KeyStatus {
        path: "traffic.right_turn_on_red",
        status: Status::Wired,
        note: "Overrides the jurisdiction's right-turn-on-red rule.",
    },
    KeyStatus {
        path: "traffic.dynamic_rerouting",
        status: Status::Wired,
        note: "Drivers re-plan on travel-time changes and closures.",
    },
    KeyStatus {
        path: "traffic.lane_changes",
        status: Status::Wired,
        note: "Discretionary and route lane changes (MOBIL).",
    },
    KeyStatus {
        path: "traffic.reroute_period_s",
        status: Status::Wired,
        note: "Congestion-aware rerouting on smoothed lane speeds (SUMO's rerouting device); \
               0 is off.",
    },
    KeyStatus {
        path: "traffic.strategic_lane_choice",
        status: Status::Wired,
        note: "Routes are planned from every lane of the road and the driver changes lane \
               ahead of the turn; gives up after 8 s at the lane end without a gap.",
    },
    KeyStatus {
        path: "traffic.keep_junctions_clear",
        status: Status::Wired,
        note: "Do not enter a junction without room beyond it; yield to vehicles inside it.",
    },
    KeyStatus {
        path: "traffic.gridlock_timeout_s",
        status: Status::Wired,
        note: "A vehicle standing this long is moved on along its route and counted on \
               traffic.control (SUMO's time-to-teleport); 0 is off.",
    },
    KeyStatus {
        path: "traffic.signals",
        status: Status::Wired,
        note: "Fixed-time or NEMA-style actuated control, and a green wave; the SPaT shows \
               what the controller does.",
    },
    KeyStatus {
        path: "traffic.signals.nema",
        status: Status::Partial,
        note: "The NEMA dual-ring eight-phase controller (control: nema): rings, barriers, \
               lead/lag, the three left-turn modes and both permissive faces, left-turn \
               timing, recalls, yellow, red, pedestrian calls, recall and timing, and \
               coordination run. Priority through the rings is a later WP-17 piece; a \
               NEMA junction does not serve priority requests yet.",
    },
    KeyStatus {
        path: "traffic.signals.nema.permissive_display",
        status: Status::Wired,
        note: "When a permissive left may turn: under a flashing yellow arrow while the \
               opposing through is green, under a circular green while its own approach is \
               green (the lead-lag yellow trap). A left sharing the through's lane always \
               has the circular green.",
    },
    KeyStatus {
        path: "traffic.signals.nema.coordination",
        status: Status::Wired,
        note: "Keeps every junction on its plan's cycle and offset (a green wave's, when \
               one is set): the coordinated phases hold to their yield point, the others \
               end at their force-offs, and unused time returns to the coordinated phases.",
    },
    KeyStatus {
        path: "traffic.signals.nema.force_off",
        status: Status::Wired,
        note: "Under coordination: floating limits each phase to its split; fixed ends it \
               at its point in the cycle, so a later phase may use time an earlier one left.",
    },
    KeyStatus {
        path: "traffic.signals.nema.controller_type",
        status: Status::Wired,
        note: "Under coordination, what the offset references: the start of the \
               coordinated green (ts2) or of its yellow (type170).",
    },
    KeyStatus {
        path: "traffic.signals.nema.coordinate_phases",
        status: Status::Wired,
        note: "Under coordination, the phases that hold to the yield point.",
    },
    KeyStatus {
        path: "traffic.signals.nema.ped_recall",
        status: Status::Wired,
        note: "These phases serve their walk every cycle; the others only when a \
               pedestrian standing at the crosswalk has pressed the button.",
    },
    KeyStatus {
        path: "actors.vehicles.demand.fleet_size",
        status: Status::Wired,
        note: "Holds exactly this many vehicles on the road: a finished trip is replaced, \
               and a trip that cannot enter or has no route is redrawn. Needs the Poisson \
               model; its arrival rate is then not used.",
    },
    KeyStatus {
        path: "actors.vehicles.demand.rate_veh_per_h_per_lane_km",
        status: Status::Wired,
        note: "Vehicles per hour per kilometre of driving lane, so one scenario offers the \
               same density on any city. Once the world is built it becomes \
               rate_veh_per_h (times the network's driving-lane length) and wins over it.",
    },
    KeyStatus {
        path: "actors.rsu_placement",
        status: Status::Wired,
        note: "Roadside units the engine stands at the city's busiest signalised \
               intersections (most approach lanes first, nearest the middle of the map, \
               no two closer than min_spacing_m), with the given roles, profile and \
               backhaul; added to actors.rsus once the world is built. A city with fewer \
               signals gets fewer units.",
    },
    KeyStatus {
        path: "city",
        status: Status::Wired,
        note: "The city the scenario runs on, by its id in worlds/cities.yaml. With no \
               world block the world is the city's extract, box and jurisdiction; set by \
               the city picker, `v2xw run --city` and run.start, it replaces the world the \
               file names and moves what was placed in the old city (roadside units, a \
               fixed rate, a street closure, the local time of day) to the new one.",
    },
    KeyStatus {
        path: "actors.vehicles.demand.params",
        status: Status::Wired,
        note: "The named model's own parameters: the Poisson model's (with an optional \
               'od' object for the origin-destination law) or the drop model's. \
               'max_total_vehicles' asks the Poisson model for an exact fleet size.",
    },
    KeyStatus {
        path: "actors.vehicles.equipped_fraction",
        status: Status::Wired,
        note: "What share of vehicles carry a radio. 0 is a legal pure-traffic run.",
    },
    KeyStatus {
        path: "actors.vehicles.driving",
        status: Status::Wired,
        note: "The driving-model library: 'car_following' (every class) and 'by_class' \
               each pick a model — idm, eidm, krauss, gipps, w99, acc, or the cooperative \
               cacc-milanes, cacc-ploeg and cacc-path, which drive on the BSMs and CAMs the \
               network actually delivered and fall back to ACC when they are lost or late \
               — with optional 'params' and a 'share' of the class; 'cooperative' sets the \
               message age limit and association gate; 'lateral' turns on the SL2015 \
               sub-lane model. Left out, the engine's own IDM drives. See \
               docs/models/driving-model-library.md.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers",
        status: Status::Wired,
        note: "How human the drivers are. 'realism' picks the population: calibrated (the \
               default: own free speed, reaction at a green, three following styles), \
               identical (the textbook model), heterogeneous (every driver their own time \
               gap, acceleration, braking and standstill gap, spread as fitted to real \
               drivers) or human (heterogeneous plus a 0.8 s reaction time and the Human \
               Driver Model's perception errors, Treiber, Kesting & Helbing 2006). The other \
               keys override the preset. See docs/models/driver-heterogeneity.md. The \
               passenger-car numbers max_accel_mps2, comfort_decel_mps2, time_headway_s \
               and min_gap_m replace the car-following model's calibration for cars (each \
               driver keeps a style and spread around them); politeness and \
               lane_change_threshold_mps2 set MOBIL for every class. Read by the native \
               mobility engine; a SUMO run takes its drivers from SUMO.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.reaction_time_s",
        status: Status::Wired,
        note: "Each driver applies the car-following decision they made this long ago, \
               except when braking harder than comfortable, which acts at once. 0.7-0.75 s \
               is an expected event's brake reaction, 1.25 s an unexpected one's (Green \
               2000). Above about 0.6 s homogeneous traffic forms stop-and-go waves.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.truck_lane_restriction",
        status: Status::Wired,
        note: "Trucks and trailers make no discretionary change into the leftmost lane of a \
               motorway or trunk road with three or more lanes (StVO §18(11); many US \
               states' left-lane truck restrictions).",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.motorcycle_filtering",
        status: Status::Wired,
        note: "Overrides the jurisdiction's rule: true lets motorcycles and mopeds ride \
               between stopped cars (and turns on the SL2015 sub-lane model, which filtering \
               needs); false keeps them in the queue.",
    },
    KeyStatus {
        path: "actors.vehicles.classes",
        status: Status::Wired,
        note: "The fleet mix: each vehicle's class is drawn with these shares, and its \
               size, driver, hardware profile and CAM station type follow from the class. \
               Only motorised classes; cyclists and pedestrians are actors.vru. A \
               'motorcycle' or 'moped' rides as one: its own acceleration and braking, \
               junction turns within a 25° lean, the left tyre track of its lane; it \
               queues behind stopped traffic, because every jurisdiction with rules here \
               (New York VTL §1252(c) among them) prohibits filtering between lanes.",
    },
    KeyStatus {
        path: "actors.vru",
        status: Status::Partial,
        note: "Pedestrians walk sidewalks and cross streets on crosswalk lanes \
               (social-force model), and away from them: with the default 'observed' \
               behaviour they walk at speeds by age group (Knoblauch et al. 1996), step \
               off after a start-up time when walk comes on, cross against flashing or \
               steady don't-walk when they are among the share who do and the gap passes \
               the HCM critical headway (calibrated to Midtown counts, Basch et al. 2015), \
               walk in groups, and cross mid-block (jaywalk, straight or diagonal) where \
               blocks are long, more often beside stopped traffic, with a lane-by-lane \
               gap. Vehicles yield to anyone on a crosswalk, stop for anyone in or about \
               to enter their lane mid-block, a few yield to a pedestrian waiting at the \
               kerb, and never stop inside a crosswalk. Signalised crosswalks get MUTCD \
               walk, flashing don't-walk (3.5 ft/s clearance) and don't-walk intervals. \
               Cyclists ride the lanes that admit bicycles at naturalistic speeds \
               (conventional and e-bike, Schleinitz et al. 2017). Partial because: \
               crossing nodes with no crossing way get no crosswalk, the grid has no \
               separate cycle lanes, and a cyclist rides the lane centre.",
    },
    KeyStatus {
        path: "actors.vru.behaviour",
        status: Status::Wired,
        note: "'observed' (default) or 'document': the field-study behaviour described \
               under actors.vru, or the social-force document defaults of 04-models.md \
               §2.5 (one speed law, everyone waits for walk, nobody crosses mid-block, \
               no groups).",
    },
    KeyStatus {
        path: "actors.vru.midblock_rate_per_100m",
        status: Status::Wired,
        note: "Mid-block crossing decisions per 100 m of eligible sidewalk walked where \
               traffic moves (four times as many beside a queue); 0 turns jaywalking off. \
               Default 0.25 under 'observed', a choice: no observed rate per metre of \
               sidewalk could be read, and the run reports the share of crossings made \
               mid-block.",
    },
    KeyStatus {
        path: "actors.vru.red_crossing_share",
        status: Status::Wired,
        note: "The share of pedestrians who cross on flashing or steady don't-walk when \
               the traffic leaves a gap. Default 0.10 under 'observed', calibrated so that \
               about a tenth of signalised crossings begin on don't-walk, as Basch et al. \
               2015 counted at five Midtown intersections.",
    },
    KeyStatus {
        path: "actors.vru.crossing_gap_margin_s",
        status: Status::Wired,
        note: "The gap a pedestrian accepts: the HCM critical headway t_c = L/S_p + t_s, \
               where this is t_s (start-up and clearance), the median of a lognormal \
               across pedestrians. Default 2 s (HCM, not re-verified locally); smaller \
               values make pedestrians accept tighter gaps against traffic.",
    },
    KeyStatus {
        path: "actors.vru.walking_speed_mps",
        status: Status::Wired,
        note: "The mean walking speed of pedestrians under 65. Default 1.51 m/s under \
               'observed' (Knoblauch, Pietrucha & Nitzburg 1996, TRR 1538: 4.95 ft/s at \
               crosswalks; the 65-and-over mean, 1.25 m/s, scales with it) and 1.34 m/s \
               under 'document' (Helbing & Molnar 1995). The bound 0.5-2.5 m/s spans a \
               frail walker to a brisk one; the MUTCD clearance speed is 1.07 m/s.",
    },
    KeyStatus {
        path: "actors.vru.ebike_share",
        status: Status::Wired,
        note: "The share of cyclists on e-bikes (17.4 km/h mean against 15.3 km/h on a \
               conventional bicycle, Schleinitz et al. 2017). Default 0.3, a choice.",
    },
    KeyStatus {
        path: "actors.vru.device_fraction",
        status: Status::Wired,
        note: "The share of pedestrians and cyclists carrying a V2X handset. Each one \
               equipped is hosted in the node phase as a VRU device: it sends SAE J2735 \
               PSMs (the SAE stack) or ETSI TS 103 300-3 VAMs (the ETSI stack) under the \
               EN 302 571 duty cycle, signed and pseudonymous like a vehicle's, and its \
               messages appear in node.tx, node.rx and every metric.",
    },
    KeyStatus {
        path: "actors.rsus",
        status: Status::Wired,
        note: "Roadside units. Placed, given a profile and a role set, and they transmit. \
               A unit with the 'spat' or 'map' role must stand within 60 m of a signalised \
               junction: it broadcasts that junction's SPaT and MAP. Under the SCMS each \
               unit is in its provider's RSU application-certificate cycle (WP-B3b): \
               enrolled and issued this week's certificate (169 h, no butterfly, CRL series \
               3) before the run, and next week's when its backhaul is up; renewed weekly \
               over its own backhaul line; it signs nothing once none is valid (counted as \
               rsus_starved, rsu_starved_ns, rsu_messages_withheld). It installs its RA's \
               composite CRL over the backhaul after verifying it.",
    },
    KeyStatus {
        path: "actors.rsus[].backhaul",
        status: Status::Wired,
        note: "This unit's backhaul: 'backhaul/fixed' (net.backhaul's latency and \
               capacity), 'backhaul/fibre', 'backhaul/dsl' or 'backhaul/microwave' (a wired \
               or radio line with its medium's rate, asymmetry, propagation and loss), \
               'backhaul/cellular' (the unit's own modem; under cellular/uu/packet-pf its \
               messages ride a TCP connection through the PF scheduler) or 'backhaul/none' \
               (an isolated unit: it relays nothing and broadcasts no CRL). Every message \
               the unit carries — relayed reports, its own reports, the CRL it broadcasts \
               and each message of a relayed vehicle's top-up — queues on its line, in the \
               backhaul bucket.",
    },
    KeyStatus {
        path: "actors.rsus[].backhaul_params",
        status: Status::Wired,
        note: "This unit's own backhaul numbers: rate_mbps (towards the backend), \
               down_mbps, latency_ms (the fixed one-way part), distance_km, hops, loss \
               (per packet; a lost packet is sent again after Linux's 200 ms minimum RTO). \
               Over net.backhaul.params when the unit names the same medium, over the \
               medium's card defaults otherwise.",
    },
    KeyStatus {
        path: "actors.rsu_placement.backhaul_params",
        status: Status::Wired,
        note: "The backhaul numbers of every placed unit (as actors.rsus[].backhaul_params).",
    },
    KeyStatus {
        path: "actors.backend.protocol",
        status: Status::Wired,
        note: "The credential-management protocol. Naming the CAMP SCMS is what turns the \
               whole backend on.",
    },
    KeyStatus {
        path: "actors.backend.entities",
        status: Status::Partial,
        note: "Keyed by CAMP SCMS role (ra, pca, la1, la2, ma, crlg, lop, crl_store, \
               crl_broadcast, eca, dcm). 'profile' sets the hardware the entity's \
               cryptography is costed on and 'service_model' its queue ('service/mmc', \
               the default M/M/c, or 'service/mm1'). 'net' is not read: every entity sits \
               on net.backend_net.",
    },
    KeyStatus {
        path: "actors.backend.links",
        status: Status::Wired,
        note: "Overrides the latency and capacity of one link between two backend roles; \
               every flow crossing it pays the new figures.",
    },
    // --- environment -------------------------------------------------------
    KeyStatus {
        path: "weather.initial",
        status: Status::Wired,
        note: "The weather drivers, the radio and the GNSS model start in. Rain, snow and \
               fog lower desired speeds and stretch headways by the FHWA Road Weather \
               Management bands (arterial rows on city streets, freeway rows at 50 mph \
               and over); a weather-front event changes it and its end restores it.",
    },
    KeyStatus {
        path: "weather.intensity",
        status: Status::Wired,
        note: "Heavy (0.5 and over) takes the FHWA heavy-rain and heavy-snow rows for \
               drivers; the 0.5 threshold is an uncalibrated choice, stated on the card. \
               For the radio, rain and sleet at intensity 1 stand for 50 mm/h, which ITU-R \
               P.838-3 prices at 0.21 dB per km at 5.9 GHz: 0.06 dB on a 300 m link. That \
               is the real size of rain at this carrier, and it changes no delivery \
               measurably. Fog and snow cost the radio nothing.",
    },
    KeyStatus {
        path: "weather.visibility_m",
        status: Status::Wired,
        note: "Drivers keep to a speed they can stop from within what they can see: the \
               AASHTO stopping sight distance (2.5 s reaction, 3.4 m/s^2) solved for \
               speed.",
    },
    KeyStatus {
        path: "weather.surface",
        status: Status::Partial,
        note: "Caps braking at the surface's grip, mu*g (wet 0.5, snow 0.25, ice 0.1; \
               secondary friction figures), which also shrinks the comfortable braking \
               the car-following model plans with. A dry surface is not capped.",
    },
    // --- radio -------------------------------------------------------------
    KeyStatus {
        path: "radio.rat",
        status: Status::Wired,
        note: "The radio access technology. dsrc-80211p runs CSMA/CA on the region's \
               channel (172 in the 2016 US plan, 180 in Europe) with its congestion \
               control: SAE J2945/1 in the US, ETSI TS 102 687's adaptive gatekeeper in \
               Europe. lte-v2x-pc5 runs Mode 4 sensing-based \
               semi-persistent scheduling on the SAE J3161/1 US profile: channel 183, \
               20 MHz, ten 10-PRB sub-channels, MCS 7, probResourceKeep 0.8, one blind \
               retransmission (as every deployed-profile test ran it, 5GAA P-190033), the \
               J3161/1 CR limits enforced per CBR zone (values second-hand, via Abrar et \
               al. 2026) and J3161/1's density-driven BSM interval. Receivers are \
               fielded ones: an 802.11p unit's sensitivity is the lab-measured OBU's \
               (-92 dBm at 6 Mbit/s, 4 dB better than EN 302 663's minimum), and a \
               sidelink block is decided by its error curve with no cutoff at TS 36.101's \
               conformance sensitivity, which fielded receivers beat by about 13 dB. nr-v2x-pc5 runs Mode 2 at 30 kHz with re-evaluation, \
               pre-emption and the ETSI TS 103 574 CR limits, on the ETSI EN 303 798 \
               pool (20 MHz, four 12-PRB sub-channels, 16QAM-490), each block error read \
               from Lusvarghi et al. 2024's link-level curve for the link's environment, \
               line of sight or vehicle or building blockage, and relative speed; no US \
               NR-V2X deployment profile exists. Sensing records only decoded SCIs. 'hybrid' is refused: the radio crate's hybrid selector \
               arbitrates a direct radio against the cellular Uu link, not 802.11p \
               against a sidelink, and the Uu backend path is not wired.",
    },
    KeyStatus {
        path: "radio.tiers.propagation",
        status: Status::Wired,
        note: "Path-loss fidelity. Abstract is free space. Medium is the Abbas 2015 \
               dual-slope law with correlated shadowing and Sommer building loss capped \
               at TR 37.885's NLOS law. High is priced from the geometry: TR 37.885 line \
               of sight, TR 37.885 blockage from the vehicles actually on the path, the \
               Mangel 2011 model round a traced street corner, TR 37.885 NLOS where no \
               single corner connects. Both take Nakagami fading unless radio.models \
               says otherwise, except on an LTE-V2X or NR-V2X run, whose block-error \
               curves were measured over fading channels and already contain it. High \
               is the default: on a street grid it is the law best \
               supported by the intersection measurements it was fitted to, at about 1.8 \
               times medium's cost. Rain applies at medium and high.",
    },
    KeyStatus {
        path: "radio.tiers.phy",
        status: Status::Wired,
        note: "Physical-layer fidelity. Medium decides each frame from its SINR over time \
               with the 802.11p error model; high adds preamble capture, which only \
               changes frames that overlap another at the receiver, so a sparse run gives \
               the same result at either. Abstract takes effect together with an abstract \
               MAC (reception from a table); with a medium or high MAC the frame is \
               decided by the medium PHY, so abstract and medium give the same run.",
    },
    KeyStatus {
        path: "radio.tiers.mac",
        status: Status::Wired,
        note: "Medium-access fidelity. Abstract has no MAC (reception comes from a \
               table); medium and high run the same 802.11p EDCA/OCB CSMA model with the \
               region's congestion control, so high adds nothing over medium.",
    },
    KeyStatus {
        path: "scale",
        status: Status::Wired,
        note: "City scale: microscopic traffic inside a map box and a queue network (SUMO \
               MESO's model) everywhere else, vehicles handed across under one identity, \
               and the radio's focus region matched to the box.",
    },
    KeyStatus {
        path: "scale.traffic.model",
        status: Status::Wired,
        note: "mobility/tiered/meso-micro drives the box microscopically and the rest as \
               queues; mobility/meso/queue drives the whole map as queues.",
    },
    KeyStatus {
        path: "scale.traffic.region",
        status: Status::Wired,
        note: "The map box driven microscopically. A lane belongs to it when its midpoint \
               is inside.",
    },
    KeyStatus {
        path: "scale.traffic.meso",
        status: Status::Wired,
        note: "The queue network: segment length, the four headways, the jam threshold and \
               whether signals hold the queue head.",
    },
    KeyStatus {
        path: "scale.radio.follows",
        status: Status::Wired,
        note: "On, the radio's focus region is the microscopic box: the declared tiers \
               inside it, abstract propagation outside.",
    },
    KeyStatus {
        path: "scale.radio.receive_in_margin",
        status: Status::Wired,
        note: "On, a queued vehicle inside the margin also receives; off (the default), it \
               only transmits, so the box hears it and its own receptions are skipped.",
    },
    KeyStatus {
        path: "scale.radio.margin_m",
        status: Status::Wired,
        note: "How far outside the box a queued vehicle's radio stays on; past it the \
               vehicle neither transmits nor receives.",
    },
    KeyStatus {
        path: "radio.tiers.focus",
        status: Status::Partial,
        note: "A region — a disc following one node, or a map box — whose links run the \
               propagation and the receiver at the focus tier (at high: weather \
               attenuation, and 802.11p preamble capture or the sidelink's SCI stage). \
               Links entering it use the surrounding propagation with no fading draw. \
               Every phy.rx record is tagged inside, outside, inbound or outbound; \
               outbound links carry the region's stated bias. Medium access stays one \
               model for the whole world.",
    },
    KeyStatus {
        path: "radio.models",
        status: Status::Wired,
        note: "Picks a model per radio family, overriding the tier's default: \
               propagation (free-space, two-ray-ground, log-distance with a named preset, \
               tr37885, v2v-urban-geometric, winner-plus-b1 with heights effective, the \
               default, as WINNER+ and TR 36.885 print it, or actual, as the LTE-V2X study \
               simulators evaluate it, and nlos sommer, the default, or manhattan, the \
               WINNER+ B1 two-leg law round the traced street corner), fading (none, nakagami-m with a preset), per (the \
               802.11p error model's implementation loss), phy (the 802.11p sensitivity \
               table: measured-obu by default, etsi-static, etsi-dynamic or cohda-mk5), \
               obstacle (the Sommer building row) and sidelink \
               (access/sidelink/engine-coupling: profile sae-j3161, etsi-en303613 or \
               molina-masegosa-2017 for LTE, etsi-en303798 or todisco-2021 for NR; mcs, \
               an index into the profile's table (NR etsi-en303798: TS 38.214 Table \
               5.1.3.1-2, 0-27); max_transmissions for blind \
               HARQ retransmissions, 1-2 LTE, 1-3 NR; congestion_control \
               etsi-ts-103-574, sae-j3161 or off; rate_control sae-j3161 or off, SAE \
               J3161/1's density-driven BSM interval on LTE-V2X, on by default under \
               the sae-j3161 profile; sensitivity measured, the default, where the \
               block-error curve decides, or ts-36-101, which loses every copy under the \
               conformance sensitivity; sizing code-rate, the default, the profile's \
               Qm·R estimate of a transport block, or ts-36-213, LTE only, TS 36.213 \
               Table 7.1.7.2.1-1's block sizes at the MCS, as OpenCV2X sizes them), and \
               dcc for 802.11p \
               (dcc/sae/j2945-1-rate-power, dcc/etsi/adaptive-ts102687 or \
               dcc/etsi/reactive-ts102687; the region's by default). J2945/1 sets the \
               interval from the neighbours within 100 m and the power from the busy \
               ratio, and sends a BSM early and at full power on hard braking or when \
               its neighbours' coasted estimate of it drifts 0.2-0.5 m; each BSM is \
               staggered by 0-5 ms. Unknown families, ids and values are refused.",
    },
    KeyStatus {
        path: "radio.devices.obu.sampled_pattern",
        status: Status::Wired,
        note: "A measured antenna pattern every vehicle carries instead of obu.antenna_pattern: \
               a CSV table of azimuth (optionally elevation) gains in dB relative to the \
               antenna gain, interpolated between samples, with an optional fixed and \
               per-vehicle random mount rotation (Eckhoff et al., IEEE VNC 2016). Unset, \
               the default, changes nothing; a table of zeros is the isotropic antenna.",
    },
    KeyStatus {
        path: "radio.devices",
        status: Status::Wired,
        note: "The radio each kind of node carries. Transmit power is the conducted power \
               at the antenna port; the link budget applies antenna gain less cable loss at \
               both ends. Defaults are 3GPP TR 36.885's: 23 dBm with a 3 dBi antenna on a \
               vehicle or a roadside unit, 23 dBm and 0 dBi on a pedestrian's device, \
               no cable loss, antennas 1.5 m high (3 m on a truck). obu.antenna_pattern \
               shapes each vehicle's gain by direction (TR 37.885 Option 1): tr37885, the \
               default, puts a rooftop antenna on a car or van (the same all round, down \
               toward a high mast) and front and rear panels on a truck or bus, 6.75 dB \
               down to its side; rooftop gives every vehicle the roof antenna; isotropic \
               none. On 802.11p a vehicle's \
               J2945/1 congestion control sets the radiated power (20 dBm at most) and \
               the unit transmits at the lesser of that less its net gain and this power.",
    },
    KeyStatus {
        path: "radio.range",
        status: Status::Wired,
        note: "How far each transmission is followed: to where, in line of sight, a unit \
               radiating the 33 dBm regulatory maximum would still arrive at no less than \
               the noise floor minus the margin. A link too lossy for even that to clear \
               it is counted as interference, not a reception attempt; the reference is \
               fixed, so the links a run attempts do not move with the transmit power. An optional cap bounds the fully evaluated range; beyond it, \
               line-of-sight receivers still get the frame's energy as interference.",
    },
    KeyStatus {
        path: "radio.region",
        status: Status::Wired,
        note: "The regulation the radios transmit under: the channel each technology \
               deploys on, and the EIRP each unit may radiate there. us is FCC 24-123 \
               (2024): LTE-V2X and NR-V2X on the 20 MHz channel 5.905-5.925 GHz, an OBU \
               without a geofence at 27 dBm EIRP toward the horizon, a roadside unit at \
               33 dBm less 20 log10(h/8) above 8 m; 802.11p is refused, because FCC \
               20-164 gave channel 172 to Wi-Fi and FCC 24-123 cancels the last DSRC \
               licences on 13 December 2026. us-2016 is the DSRC band plan SAE J2945/1 was \
               written for (47 CFR 90.377, 2017): 802.11p on channel 172, roadside units \
               at 33 dBm, portable units at 1 mW. eu is ETSI EN 302 571: 10 MHz channels, \
               33 dBm EIRP; ITS-G5 on 5.895-5.905 GHz, LTE-V2X on 5.905-5.915 GHz, NR-V2X \
               on 5.885-5.895 GHz. Unset, us-2016 for 802.11p and us otherwise. A \
               configured power above the limit is lowered to it.",
    },
    KeyStatus {
        path: "radio.adjacent_channel",
        status: Status::Wired,
        note: "Transmitters of another technology on a neighbouring channel, where the \
               region lets the two operate side by side (Europe: ITS-G5 on 180 beside \
               LTE-V2X on 182 and NR-V2X on 178). Each is placed and timed as a jammer is \
               (position_m, follow_node or path_m; power_dbm, 23 by default; period_ms and \
               duty; from_s, to_s) and its power reaches this run's receivers through the \
               same link budget, less the adjacent-channel interference ratio: by default \
               the region's spectrum mask for its technology combined with the minimum \
               selectivity of this run's receivers (ETSI EN 302 571 Table 8, 3GPP TS \
               36.101 and 38.101-1), the worst a conformant pair may show; acir_db sets \
               a measured one. It raises the noise a frame is decoded against, clear-channel \
               assessment and the busy ratio, and a frame it alone killed is lost to \
               adjacent-channel. A channel that overlaps the run's is refused (that is a \
               jammer), as is one the region does not allow the technology on.",
    },
    KeyStatus {
        path: "radio.channels",
        status: Status::Wired,
        note: "The run's own channel map (WP-13): each entry names a channel of the \
               region's band plan and the message sets that go out on it (spat, map, wsa, \
               srm, ssm, crl, mbr, denm, cpm, ...); everything else stays on radio.channel, \
               which the safety broadcasts (bsm, cam, psm, vam) never leave. One service \
               channel beside the safety channel (IEEE 1609.4-2016 §6.2.5 service \
               channels; SAE J2945/1 keeps the BSM on 172; ETSI EN 302 663 / TS 103 141 in \
               Europe). A unit sends and hears the service channel only with a second \
               radio (radio.devices.*.radios: 2), or under net.channel_access: \
               alternating in the SCH intervals; a message for a channel it has no radio \
               on is not sent (untuned_tx_drops). Each channel has its own EDCA queues, \
               carrier sense and busy ratio (channel_busy_ns / channel_window_ns in the \
               report). Empty, the default: the single-channel run, byte for byte.",
    },
    KeyStatus {
        path: "radio.channel_leakage",
        status: Status::Wired,
        note: "Whether the run's own channels leak into each other (WP-13): a frame on \
               one is energy on the other at its received power less the ACIR of \
               v2xw_radio::regulation (the EN 302 571 V2.1.1 mask's ACLR with the Table 8 \
               selectivity at the run's rate, TR 36.942 §5.1.1.3). It enters the SINR, \
               carrier sense and the busy ratio. Unset is on; false is ideal channel \
               filters. Only acts with radio.channels.",
    },
    KeyStatus {
        path: "radio.devices.obu.radios",
        status: Status::Wired,
        note: "Radios on a vehicle's unit (WP-13): 1 (default; SAE J2945/1's single \
               continuous safety-channel radio) or 2 (adds one on radio.channels' service \
               channel). Acts only with radio.channels.",
    },
    KeyStatus {
        path: "radio.devices.rsu.radios",
        status: Status::Wired,
        note: "Radios on a roadside unit (WP-13): 2 (default; the USDOT DSRC RSU \
               Specification v4.1 calls for two) or 1, which keeps the unit on the safety \
               channel and drops its service-channel messages. Acts only with \
               radio.channels.",
    },
    KeyStatus {
        path: "radio.devices.rsu.off_channel",
        status: Status::Wired,
        note: "A roadside unit that leaves the 802.11p safety channel early in some IEEE \
               1609.4 sync intervals (UTC 100 ms) and hears nothing while away: in each \
               interval with `probability`, from `from_ms` for a duration uniform in \
               [min_duration_ms, max_duration_ms]; a frame overlapping the absence is lost \
               as off-channel. Unset, the default, keeps the unit on the channel. Fitted \
               on Tampa's RSU log, where losses double in the interval's first half \
               (data/calibration/rsu-off-channel-tampa.yaml). Sidelink runs ignore it.",
    },
    KeyStatus {
        path: "radio.devices.vru.radios",
        status: Status::Wired,
        note: "Radios on a pedestrian's or cyclist's device (WP-13): 1 (default) or 2. \
               Acts only with radio.channels.",
    },
    KeyStatus {
        path: "radio.channel",
        status: Status::Wired,
        note: "The IEEE channel number within the region's band plan (170-184); unset, the \
               region's deployment channel. Only channels the region opens to the \
               technology are accepted, and the sidelink's resource pool is sized to the \
               channel's width.",
    },
    // --- network -----------------------------------------------------------
    KeyStatus {
        path: "net.layer",
        status: Status::Wired,
        note: "The network and transport header every frame carries: 'wsmp' (IEEE 1609.3, \
               5 octets for a BSM) or 'gn-btp' (ETSI GeoNetworking with BTP, 44 octets for a \
               CAM). LLC/SNAP, the 802.11 MAC header and the FCS are added below either, \
               and the air time and the overhead metrics are computed over the whole \
               frame.",
    },
    KeyStatus {
        path: "radio.phy",
        status: Status::Wired,
        note: "802.11p receiver and transmitter numbers. The data rate sets every safety \
               frame's MCS and air time; the noise figure, sensitivity, carrier-sense and \
               capture thresholds replace the PHY's defaults. A sidelink run (LTE-V2X, \
               NR-V2X) refuses them: its numbers are radio.models.sidelink's.",
    },
    KeyStatus {
        path: "radio.mac",
        status: Status::Wired,
        note: "802.11p EDCA at the medium and high MAC tiers: the access category of \
               periodic and event messages, CWmin and AIFSN of the voice and video \
               categories, and the per-category queue length.",
    },
    KeyStatus {
        path: "messages.rate_hz",
        status: Status::Wired,
        note: "A fixed BSM and CAM rate in place of J2945/1's 10 Hz and the ETSI CAM \
               triggers; congestion control may still stretch it.",
    },
    KeyStatus {
        path: "radio.sidelink",
        status: Status::Wired,
        note: "LTE-V2X and NR-V2X semi-persistent scheduling: reservation interval, keep \
               probability, sensing threshold, sensing on or off, candidate percentage \
               (20; 35 or 50 on NR) and selection-window end T2, over the profile's \
               values. Refused on an 802.11p run.",
    },
    KeyStatus {
        path: "radio.dcc",
        status: Status::Wired,
        note: "Congestion control on or off, the ETSI adaptive target load and gains, and \
               SAE J2945/1's longest interval, density coefficient and power control. A \
               number for an algorithm the run does not use is refused.",
    },
    KeyStatus {
        path: "radio.channel_model",
        status: Status::Wired,
        note: "The channel as plain numbers. The exponent runs the log-distance law with \
               one slope; the shadowing spread and decorrelation reach every law with \
               shadowing (log-distance, tr37885, v2v-urban-geometric, winner-plus-b1); \
               the fading law replaces the tier's Nakagami draw (Rice through Nakagami's \
               m = (K+1)^2/(2K+1); Nakagami near and far m with a breakpoint); the \
               building wall and per-metre losses are Sommer 2011's beta and gamma \
               wherever buildings are charged; vehicle_blockage turns TR 37.885 NLOSv on \
               under any law, or off.",
    },
    KeyStatus {
        path: "net.channel_access",
        status: Status::Wired,
        note: "IEEE 1609.4 on wsmp. 'continuous' (default): every message goes out when \
               generated. 'alternating': a single radio spends 50 ms of every 100 ms on the \
               control channel, less a 4 ms guard at each switch, so a BSM generated in \
               the service-channel interval waits for the next control-channel interval; \
               the wait shows in the end-to-end latency. The EDCA MAC holds the grant \
               (WP-16): no frame starts in a guard or crosses its interval's end \
               (mac_gate_deferrals, mac_guard_deferrals, straddling_frames = 0 in the \
               report). A one-radio unit hears the safety channel only in CCH intervals \
               and the service channel of radio.channels only in SCH intervals, and can \
               send there; a two-radio unit (radio.devices.*.radios: 2, the roadside \
               default) keeps one radio on each and never switches. At the abstract MAC \
               tier and on a sidelink, which have no EDCA MAC, the hand-off waits \
               instead. Interval lengths UNVERIFIED at the primary level.",
    },
    KeyStatus {
        path: "net.geonet",
        status: Status::Wired,
        note: "Multi-hop hazard warnings (gn-btp only). When a hard-brake or breakdown \
               fires, its DENM is GeoBroadcast to a circle around the vehicle and forwarded \
               hop by hop by the GeoNetworking router of EN 302 636-4-1 (beacons, location \
               table, duplicate detection, hop limit, greedy and contention-based \
               forwarding) with the DEN service's repetition and keep-alive forwarding. \
               forwarding 'on-air' (default): every router runs inside the run, a CBF \
               contender's copy is a real frame queued in its MAC, so forwards contend, \
               collide, are delayed, count in the channel busy ratio and pass the DCC gate; \
               the run's link budget decides who hears each copy, and CAMs refresh the \
               location table. forwarding 'overlay': a protocol-only run over the vehicles' \
               positions at the hazard instant, hop range range_m, no MAC. The run summary \
               says how many vehicles in the area it reached, how fast, over how many hops \
               and at how many transmissions.",
    },
    KeyStatus {
        path: "net.access_categories",
        status: Status::Wired,
        note: "'single' (default): every frame waits in the AC_VO queue. 'etsi-profiles': \
               the DCC profiles of ETSI TS 102 724 mapped to EDCA queues as EN 302 663 \
               maps them (DP0 hazard DENM -> AC_VO, DP2 CAM -> AC_BE, DP3 forwarded \
               GeoNetworking copies and other traffic -> AC_BK), each with its own AIFS \
               and contention window (IEEE 802.11-2020 Table 9-155, OCB).",
    },
    KeyStatus {
        path: "net.dcc_cbr",
        status: Status::Wired,
        note: "gn-btp only. 'local' (default): the DCC algorithm acts on the station's \
               own 100 ms CBR. 'global': ETSI TS 102 636-4-2 - every frame's GeoNetworking \
               header carries the sender's CBR and the highest its neighbours reported, \
               and the DCC acts on the maximum over two hops, so stations back off for \
               hidden-terminal congestion. The run summary's gcbr counts how often it \
               raised the input.",
    },
    KeyStatus {
        path: "net.rsu_ip",
        status: Status::Partial,
        note: "SCMS traffic through roadside units (WP-19). air 'constant' (default): a \
               vehicle with no modem uses any relaying unit within 300 m and the hop is a \
               fixed latency and rate. air 'sch' (P19.2): each unit with the \
               provisioning-proxy or report-forward role signs and sends an IEEE 1609.3 \
               WSA every wsa_interval_ms (PSID 0x23, IPv6 Routing, the WAVE Routing \
               Advertisement; v2xw_msg::wsa), and a vehicle binds to a unit only while it \
               holds a verified WSA from it no older than 1 s. A relayed misbehaviour \
               report goes on the radio.channels service channel when the run has one \
               (rule gap M11; a one-radio unit then needs alternating access). Downloads \
               (top-up, re-enrolment, trust fetch) ride TCP/TLS on a service-channel \
               session (transport/80211p-sch: the vehicle's contact window, EDCA AC_BE on \
               the unit's measured CBR, 1609.4 intervals, link-budget loss) and the unit's \
               line; a transfer that fails cuts its flow off, and the vehicle starts it again \
               while a verified unit is in range (relayed_flows_cut_off). Under constant the air hop is the fixed link: the service-channel path (contact window, PER, SCH load) \
               is P19.3-P19.4.",
    },
    KeyStatus {
        path: "net.fragmenter",
        status: Status::Wired,
        note: "All four run. 'fragmenter/none' (the default) refuses a signed message \
               above the network layer's MTU (1,400 octets WSMP, 1,398 GeoNetworking) \
               before the MAC. 'fragmenter/generic-sdu' sends it as pieces with a 4-octet \
               header, reassembled per sender with a timeout; the message reaches the \
               node only whole. 'fragmenter/facilities-segmentation' (gn-btp) sends \
               independently interpretable segments, each a message of its own. \
               'fragmenter/cert-cycle-partial-hybrid' carries the hybrid certificate in \
               the first α SPDUs of each 5-SPDU cycle. Every fragmented SDU is followed \
               per receiver to net.reassembly beside the loss 1 − Π(1 − p_i) its \
               fragments' PHY success probabilities predict. The post-quantum security.signature schemes produce messages \
               that need it (an ML-DSA-44 BSM is several frames); \
               params.sdu_padding_bytes still pads every signed message (counted as \
               payload) to study a size no registered scheme has. The certificate cycle \
               carries the selected post-quantum scheme's certificate unless \
               params.hybrid_cert_bytes is set. \
               802.11p only.",
    },
    KeyStatus {
        path: "net.backhaul",
        status: Status::Wired,
        note: "The roadside units' default backhaul: 'backhaul/fixed' with latency_ms \
               (default 10, an ASSUMPTION), capacity_mbps (default 1000, an ASSUMPTION) and down_mbps; \
               'backhaul/fibre' (1 Gbit/s metro fibre, about 0.3 ms), 'backhaul/dsl' \
               (VDSL2 20/80 Mbit/s with 8 ms interleaving) or 'backhaul/microwave' \
               (400 Mbit/s radio hop, 1e-5 outage), each with rate_mbps, down_mbps, \
               latency_ms, distance_km, hops and loss; 'backhaul/cellular' or \
               'backhaul/none'. Each unit's line is a FIFO per direction.",
    },
    KeyStatus {
        path: "net.uu",
        status: Status::Wired,
        note: "Gives vehicles a cellular modem for backend traffic (misbehaviour reports, \
               CRL downloads, certificate top-ups); safety messages stay on the sidelink. \
               'cellular/uu/fixed-latency' (params.preset, a measured one-way latency; \
               default 4g-east-coast), 'cellular/uu/cell-capacity-mm1' (per-cell capacity, \
               an M/M/1 queue per UE, isd_m, uplink_mbps, downlink_mbps) or \
               'cellular/uu/handover-outage' (adds handover interruption and loss, \
               loss_percentile) or 'cellular/uu/packet-pf' (packet level: base stations \
               per city from params.sites or params.city_bbox, else a lattice at isd_m; a \
               proportional-fair scheduler per cell, HARQ, RLC AM, measured handover gaps \
               and every backend message over TCP CUBIC with TLS 1.3; params.rat lte|nr, \
               background_load; the run report says when it differs from the simple \
               model; every message of a top-up, re-enrolment or trust-list fetch rides \
               it too). params.crl_delivery 'unicast' (each vehicle polls the CRL Store) \
               or 'mbms' (eMBMS broadcast once per cell on a new list and every \
               mbms_carousel_s, default 10; mbms_subframes 1-6 MBSFN subframes per frame, \
               default 1, about 1.4 Mbit/s on LTE 20 MHz; mbms_period_ms the MCH \
               scheduling period, default 320). params.penetration is the share of vehicles with a modem \
               (default 1); the rest relay through roadside units or are offline. Without \
               net.uu no vehicle has a modem.",
    },
    KeyStatus {
        path: "net.backend_net",
        status: Status::Wired,
        note: "The links between backend entities: 'backend-net/fixed' with latency_ms \
               (default 10) and capacity_mbps (default 1000); both uncited defaults of the \
               SCMS deployment's card.",
    },
    // --- messages ----------------------------------------------------------
    KeyStatus {
        path: "messages.sets",
        status: Status::Partial,
        note: "Which message sets are generated, each by the station that sends it in a \
               deployment. bsm and cam: every equipped vehicle; a BSM carries J2945/1 path \
               history and path prediction, and its event flags and lights when set; a CAM \
               its low-frequency container every 500 ms. denm (needs gn-btp), at the C2C-CC \
               triggering conditions: a vehicle whose emergency stop signal comes on (6 \
               m/s2, UN R48) raises an emergency-brake-light DENM updated every 100 ms \
               while it stays on, and a broken-down vehicle with its hazards on, once it \
               has stood 10 s, a stationary-vehicle DENM repeated every second, updated \
               every 15 s and cancelled (the cancellation repeated for 15 s) when the \
               hazards go out. spat and map: roadside \
               units with that role, from the signal plan the drivers obey (min, max and \
               likely end times) and the junction's own lanes, at 10 Hz and 1 Hz. srm and \
               ssm (need codec_tier size-model): emergency vehicles ask the junction whose \
               MAP they heard for priority, its unit answers, and its controller extends or \
               ends greens for it (NTCIP 1211, signal.priority). cpm (needs gn-btp): each \
               vehicle's radar and camera perception, shared at the TS 103 324 rules. psm \
               and vam: VRU devices.",
    },
    KeyStatus {
        path: "messages.generator",
        status: Status::Wired,
        note: "Tunes the generators. Under any id, params.phase_window_ms (each node's \
               phase is uniform over it; default 100) and params.max_jitter_ms (a \
               per-message delay before the radio; default 10) place every node's \
               generator in time; 0 and 0 put every vehicle on one grid. \
               'generator/bsm-j2945-1' also takes the J2945/1 nominal, minimum and maximum \
               time between BSMs, and 'generator/cam-en302637-2' the EN 302 637-2 CAM \
               triggering intervals and thresholds; the other generator keeps the \
               standard's defaults. Nodes run at the mobility step, so an interval shorter \
               than it is refused.",
    },
    KeyStatus {
        path: "messages.codec_tier",
        status: Status::Wired,
        note: "'uper': every message is encoded for real — BSM, SPaT, MAP and PSM by the \
               hand-written J2735 encoders (each checked byte for byte against pycrate), \
               CAM, DENM, CPM and VAM by the generated ETSI ones — and a set \
               with no real encoder (srm, ssm) is refused. 'size-model': those sets are \
               carried as payloads of the validated modelled length (build decision D2), \
               and every message that has a real encoder is still encoded for real.",
    },
    // --- security ----------------------------------------------------------
    KeyStatus {
        path: "security.envelope",
        status: Status::Wired,
        note: "Which secured-message envelope the nodes use. 'etsi103097' is the ETSI \
               TS 103 097 profile of the same IEEE 1609.2 structure: it enforces that \
               profile's rules when a node signs (a DENM must carry its generation \
               location, for one), and the ETSI credential protocol requires it. A BSM \
               signed under either profile has the same octets, so on a BSM run the two \
               give the same result.",
    },
    KeyStatus {
        path: "security.protocol",
        status: Status::Partial,
        note: "'protocol/scms/camp' runs the credential lifecycle; its params set \
               i_period_s (default one week), cert_lifetime_s (the period plus an hour), \
               certs_per_period (20), pool_periods (2 held at the start), \
               topup_below_periods (1), cert_shuffle_threshold (1,000) and \
               cert_shuffle_window_s (2 days, CAMP 1.2.2 Table 67; Brecht's 10,000 or one \
               day as a setting), report_shuffle_threshold (10,000) and \
               report_shuffle_window_s (one day, SCMS-765): a batch closes at the count or \
               the window, whichever comes first; crl_cadence_s (one day; 0 publishes on \
               decision), crl_fetch_interval_s (3600, an ASSUMPTION inside CAMP's weekly \
               bound) and crl_broadcast_interval_s (5, the longest USDOT RSU v4.1 Store \
               and Repeat interval). Every default's source: \
               docs/design/scms-real/constants.md. Vehicles top up over their backend link when the pool runs low \
               and cannot sign once it is empty. 'protocol/etsi/ts102941' runs the ETSI \
               ITS PKI: authorization tickets (certs_per_period a period, no linkage), \
               TS 103 759 reports straight to the MA, and passive revocation only — the \
               authority's decision blocklists the enrolment credential at the EA, the \
               vehicle keeps its tickets until they expire and no reception is refused \
               (TS 102 941 §6.1.4 NOTE 4). It needs security.envelope 'etsi103097'; \
               etsi_butterfly: 1 tops up with one butterfly authorization and a batch \
               download (TS 102 941 V2 §6.2.3.5) instead of a request per ticket, and the \
               AA never sees the enrolment identity. SCMS only: every vehicle is \
               bootstrapped through the DCM with the electors' trust list and the RA's \
               policy and chain files; enrolment_lifetime_s (six years, CAMP 1.2.2) with \
               reenrol_lead_s (18 months, SM-REENR) renews it through the RA, no overlap; \
               max_periods_ahead (156) caps what the RA provisions; a blocklisted or expired \
               enrolment is refused its top-up. ETSI only: the TLM signs one ECTL and the \
               Root CA one CA-CRL at the start, and every station fetches both from the \
               Distribution Centre (the CPOC) when it joins and then every \
               crl_fetch_interval_s, over its own access, receiving only what is newer \
               (TS 102 941 §6.3.3); nothing re-issues either list during a run.",
    },
    KeyStatus {
        path: "security.scms.rx_beacon_max_age_s",
        status: Status::Wired,
        note: "The oldest a received BSM, CAM, PSM or VAM may be on the receiver's own \
               clock (IEEE 1609.2 §5.2 relevance check; 2 s unless set, C2C-CC BSP \
               RS_BSP_168). An older one is discarded unverified (node.rx \
               generation-time-window, rxval generation-time-too-old) and reaches no \
               detector.",
    },
    KeyStatus {
        path: "security.scms.rx_max_age_s",
        status: Status::Wired,
        note: "The oldest any other application message (SPaT, MAP, DENM, CPM, SRM, SSM) \
               may be on the receiver's clock (600 s unless set, RS_BSP_168). Reports, \
               CRLs, P2PCD and WSAs are not checked.",
    },
    KeyStatus {
        path: "security.scms.rx_max_future_s",
        status: Status::Wired,
        note: "How far ahead of the receiver's clock a generation time may be (0.04 s \
               unless set, RS_BSP_168); later ones are discarded unverified (rxval \
               generation-time-in-future). The misbehaviour authority's evidence rule \
               uses the RS_BSP_168 values whatever a scenario sets here.",
    },
    KeyStatus {
        path: "security.scms.crl_overdue_tolerance_s",
        status: Status::Wired,
        note: "How long past a CRL's nextCrl every device keeps trusting the certificates it \
               governs (CAMP 1.2.2 Table 67 overdue_CRL_tolerance, two weeks unless set); \
               after it the device refuses them (rxval `crl-overdue`).",
    },
    KeyStatus {
        path: "security.scms.crl_overdue_scope",
        status: Status::Wired,
        note: "Which messages an overdue pseudonym CRL makes every device refuse: craca \
               (default), the signers whose CRACA is the overdue list's; all, every message \
               while any held series-1 list is past nextCrl + the tolerance (CAMP 1.2.2 \
               Table 67: the EE is \"rejecting all of them\"; rxval `crl-overdue`).",
    },
    KeyStatus {
        path: "nodes.validated_cert_cache",
        status: Status::Wired,
        note: "Validated end-entity certificates each device remembers (4,096 unless set; \
               least recently used evicted, counted in ee_evictions); a forgotten signer's \
               next message pays first-sight validation again.",
    },
    KeyStatus {
        path: "security.scms.tls",
        status: Status::Wired,
        note: "The TLS version of every device-to-RA session over a packet-level cellular \
               link: 1.3 (default) or 1.2, the two the US profile admits; 1.2's handshake \
               flights are the measured US-profile sizes (constants.md, TLS) and cost one \
               more round trip before the first request.",
    },
    KeyStatus {
        path: "security.scms.lifecycle.cert_shuffle_threshold",
        status: Status::Partial,
        note: "The RA's request shuffle acts on a device's first certificate request \
               (CAMP 1.2.2 Table 67). Every vehicle in a run is provisioned before it \
               joins (Phase2::provision, ScmsRun::preload), and its weekly top-ups are the \
               RA's pre-generation, which goes to the ACA at the period boundary without \
               waiting in this shuffle (CAMP Primer p.6), so a same-seed run is identical \
               (docs/design/scms-real/testing/scms-knobs.md). Kept as the RA's policy.",
    },
    KeyStatus {
        path: "security.scms.lifecycle.cert_shuffle_window_s",
        status: Status::Partial,
        note: "As cert_shuffle_threshold: the request shuffle's longest wait, on a device's \
               first certificate request, which a run makes before the vehicle joins (the \
               pre-run provisioning collapses the window); in-run top-ups do not wait in \
               it.",
    },
    KeyStatus {
        path: "security.scms.lifecycle",
        status: Status::Wired,
        note: "The credential lifecycle's knobs as fields of their own, read by the same \
               model as security.protocol.params (a field set here wins over the same \
               key there). Every field left out keeps the US deployment's cited default \
               (docs/design/scms-real/constants.md; the inventory and the same-seed proof \
               of each: docs/guides/config-inventory-scms.md). An i_period_s other than \
               one week needs time.calendar: compressed; cert_lifetime_s must not be \
               shorter than the i-period; crl_cadence_s replaces each provider's \
               business-day publication rule with a fixed cadence (0 publishes on \
               decision).",
    },
    KeyStatus {
        path: "security.scms",
        status: Status::Partial,
        note: "The SCMS deployment (protocol/scms/camp only; refused otherwise). \
               deployment.preset: us-production-2025 (default: two providers with neutral \
               labels on one CTL signed by the five shared electors, quorum 3; each with \
               its own offline root and ICA and its own ECA, RA, ACA (provider-b two), LA1, \
               LA2, MA, CRL Generator and PG; no LOP, no CRL broadcast), us-test-ctl-2025 \
               (the 2025-07-17 test CTL: six roots, five approved), camp-reference (the \
               single SCMS with LOP and a central MA) or \
               custom. providers[]: components, region (an ASSUMPTION; no provider \
               publishes one), fleet_share (roadside units are split by it), on_ctl (off: a \
               provider nobody trusts), ma_mode (manual-review with ma_review_s, or \
               automatic) and crl_publication (sla-1-business-day, cadence, on-decision) \
               for that provider's run only. latency region (great-circle distance at \
               fibre_fraction of c plus per_link_ms, ASSUMPTION) or flat; options.lop \
               (off: the proxy hop is transparent) and options.butterfly (original only). \
               Every node trusts every root on the CTL and holds every such provider's \
               chain. Each provider runs its own backend: vehicles and roadside units are \
               split by fleet_share, every RA serves every provider's signed CRL, MAs share \
               reports, the Backend view shows each provider's entities as \
               <provider>/<role>, and a home root rollover re-certifies every peer's copy \
               of the home hierarchy. Partial: extra ACA instances take no load.",
    },
    KeyStatus {
        path: "security.signature",
        status: Status::Partial,
        note: "Any scheme in the registry (v2xw_sec::scheme::SIGNATURE_IDS; adding one is one \
               module, docs/design/extending-crypto.md): classical ecdsa-p256, \
               ecdsa-brainpoolp256r1, ecdsa-p384; hybrids (ECDSA P-256 with Falcon-512, \
               ML-DSA-44 or ML-DSA-65 concatenated); pure ML-DSA-44/65/87, Falcon-512/1024 \
               and SLH-DSA-SHA2-128s/128f. Each changes the octets of every signed message \
               and attached certificate on the air by its parts' published sizes (FIPS 204, \
               FIPS 205, Falcon, SEC 1; a pure scheme replaces the ECDSA fields); an SPDU \
               above the network MTU is refused and counted unless a fragmenter is selected. \
               Each is charged its time: every signature and verification costs the sum of \
               its parts on the hardware profile, a profile with no figure for a part falls \
               back to the catalogue anchor it names (crypto_anchors; derived by cycle ratio \
               where nothing is published, and flagged), and a profile that prices none is \
               refused by name (the reference OBU publishes no post-quantum figure). \
               brainpool and P-384 are charged the P-256 time. Only ecdsa-p256 runs in real \
               crypto mode. A post-quantum scheme is the credential system's too (SCMS and \
               CCMS): every certificate carries the post-quantum key and issuer signature, \
               every signed backend message the post-quantum signature, authorities pay at \
               the Raspberry Pi 5 liboqs figure (an upper bound for a server) and devices at \
               the Cohda MK6's; there is no post-quantum butterfly, so a device generates one \
               post-quantum key per pseudonym certificate or ticket and uploads each \
               encrypted to the PCA or AA (v2xw_proto::hybrid).",
    },
    KeyStatus {
        path: "security.kem",
        status: Status::Wired,
        note: "The key-encapsulation scheme wherever the credential system encrypts to the \
               PCA or to a device (v2xw_sec::scheme::KEM_IDS): ecies-p256 (IEEE 1609.2's \
               65-byte EciesP256EncryptedKey), ml-kem-512/768/1024 (FIPS 203: the ML-KEM \
               ciphertext replaces the ephemeral point) or hybrid-ecies-p256-ml-kem-768 \
               (both travel). Every encrypted-key structure grows by the ciphertext and every \
               encryption and decryption pays an encapsulation or decapsulation (liboqs on a \
               Raspberry Pi 5 for authorities, the Cohda MK6 or a cycle-ratio derivation for \
               devices).",
    },
    KeyStatus {
        path: "security.threshold",
        status: Status::Wired,
        note: "Committees replace single authorities (v2xw_proto::threshold): 'roles' picks \
               the PCA (no single server can issue a certificate), the CRL Generator (the list \
               is threshold-signed) and the linkage authorities (any quorum of a t-of-n \
               committee releases a revoked device's seeds, instead of both of two). \
               'protocol' is any registered one: frost-p256 (RFC 9591), cggmp21-ecdsa \
               (threshold ECDSA, 1609.2-compatible output) or umbrella (one committee \
               signature per umbrella_leaves derived pseudonyms; 05-protocols.md §5.4's \
               placeholder binding). Every round crosses a member link (member_latency_ms) \
               with its bytes and is charged per member; a member that misses \
               round_timeout_ms is excluded and the session restarts, a malicious one is \
               caught by its share, and fewer than a quorum fail the session until members \
               return (retry_after_s). ETSI is refused.",
    },
    KeyStatus {
        path: "actors.backend.failures",
        status: Status::Wired,
        note: "Backend servers that are 'offline' (deliveries dropped), 'slow' (extra_ms \
               more service time per message) or 'malicious' (invalid threshold shares) \
               between from_s and to_s. 'node' is a role (pca, la1, la2, ma, crlg, …) or a \
               committee member, '<role>-member-<k>', the home provider's unless qualified \
               by its provider, '<provider>/<role>' ('provider-b/crlg'). Two-of-two linkage \
               with an LA down cannot revoke; a t-of-n committee with fewer than n − t + 1 \
               down can.",
    },
    KeyStatus {
        path: "security.crypto_mode",
        status: Status::Wired,
        note: "Whether signing and verification are costed or actually computed. Both \
               produce the same event log; only the manifest and the timing differ.",
    },
    KeyStatus {
        path: "security.verification_policy",
        status: Status::Wired,
        note: "Which received messages a unit verifies: all, the relevant ones \
               (on-demand), or all in priority order (prioritized).",
    },
    KeyStatus {
        path: "security.on_demand_threshold",
        status: Status::Wired,
        note: "The relevance score (time-to-collision based, 0 to 1) at or above which an \
               on-demand unit verifies a known signer (0.5 unless set). Refused under any \
               other policy.",
    },
    KeyStatus {
        path: "security.prioritized_horizon_m",
        status: Status::Wired,
        note: "The range over which proximity raises a prioritized unit's verification \
               priority (300 m unless set). Refused under any other policy.",
    },
    KeyStatus {
        path: "nodes.hsm_sign_lower_us",
        status: Status::Wired,
        note: "Lower edge of the HSM signing time where the profile publishes only an upper \
               bound (CRATON2 <9 ms); each signature is drawn uniformly between it and the \
               bound. Unset, every signature takes the bound. Refused when the default OBU \
               publishes no upper-bound signing time, or at or above that bound.",
    },
    KeyStatus {
        path: "nodes.peer_cert_cache",
        status: Status::Wired,
        note: "Peer certificates each vehicle or roadside unit holds (128 unless set); an evicted \
               peer's next digest-signed message needs a P2PCD request.",
    },
    KeyStatus {
        path: "security.signer_id_policy",
        status: Status::Wired,
        note: "How often a full certificate is attached instead of an eight-byte digest.",
    },
    KeyStatus {
        path: "security.signer_id_policy.on_new_neighbour",
        status: Status::Wired,
        note: "Also attach the certificate to a vehicle's next BSM or CAM after it hears a \
               station it did not know (TS 103 097 V1.3.1 §7.1.1). Off unless set; refused \
               when every message already carries one or no BSM/CAM runs.",
    },
    KeyStatus {
        path: "security.pseudonym_change.strategy",
        status: Status::Wired,
        note: "time changes pseudonym at period_s of age (SAE J2945/1 CERTCHG, 300 s) and \
               distance after distance_m of travel (NYC pilot, 2 km). c2c-cc is the C2C-CC \
               Basic System Profile's RS_BSP_521-524: a first change after a random \
               800-1,500 m, the second at least 800 m on plus a random 120-360 s, the third \
               after a random 10-20 km, then every 25-35 km. mix-zone changes inside a \
               30 m zone around a signalised intersection once the pseudonym is \
               mix_zone_min_age_s old (default 60 s), and frames sent inside a zone are encrypted to the \
               eavesdropper (CMIX, Freudiger et al. 2007). silent makes no scheduled change. \
               Every change moves the certificate, the BSM TemporaryID and the link-layer \
               address together; expiry and revocation force a change under every strategy.",
    },
    KeyStatus {
        path: "security.pseudonym_change.silent_period_s",
        status: Status::Wired,
        note: "[min, max] seconds: after every change the vehicle sends no safety message \
               for a uniform draw from the range (Huang et al. 2005; PRESERVE's 3-13 s). \
               Absent, no silent period.",
    },
    KeyStatus {
        path: "security.pseudonym_change.mix_zone_min_age_s",
        status: Status::Wired,
        note: "The age a pseudonym must reach before a mix zone changes it, seconds \
               (mix-zone; default 60).",
    },
    KeyStatus {
        path: "security.pseudonym_change.mix_zone_radius_m",
        status: Status::Wired,
        note: "The mix zone's radius around each signalised intersection's centre for the \
               mix-zone strategy, metres (default 30).",
    },
    KeyStatus {
        path: "security.pseudonym_change.period_s",
        status: Status::Wired,
        note: "The age at which the time strategy changes pseudonym (default 300 s, the \
               J2945/1 CERTCHG interval). Each vehicle holds a batch of 20 pseudonyms and \
               uses every one before reusing any.",
    },
    KeyStatus {
        path: "security.pseudonym_change.distance_m",
        status: Status::Wired,
        note: "The travel after which the distance strategy changes pseudonym (default \
               2 km, the NYC pilot's rule).",
    },
    // --- nodes -------------------------------------------------------------
    KeyStatus {
        path: "nodes.default_obu",
        status: Status::Wired,
        note: "The hardware profile every equipped vehicle runs on: its compute, its \
               security module and its radio.",
    },
    KeyStatus {
        path: "nodes.per_class",
        status: Status::Wired,
        note: "Per-vehicle-class overrides of the profile above.",
    },
    KeyStatus {
        path: "nodes.compute_tier",
        status: Status::Partial,
        note: "abstract: signing and verification cost a microsecond and no node is ever \
               compute-bound. medium: every operation costs the hardware profile's \
               service time and queues FIFO behind one CPU server and the profile's HSM \
               (06-node-models 2.1). high: the CPU runs as the profile's cores, so \
               software cryptography and application tasks are served in parallel; it \
               changes nothing for a profile whose cryptography runs on its HSM. \
               Processor sharing, priority classes and HSM latency distributions are not \
               built.",
    },
    KeyStatus {
        path: "nodes.backend_tier",
        status: Status::Partial,
        note: "abstract: backend entities never queue (64 servers, no fixed overhead). \
               medium and high: each entity is an M/M/c with the deployment's servers and \
               overhead, and the RA shuffles; high adds nothing over medium yet.",
    },
    // --- threats and detection --------------------------------------------
    KeyStatus {
        path: "threats.attackers",
        status: Status::Wired,
        note: "Attacker populations: which model, how many, and when they are active. \
               On the air the engine renders an attacker's claimed position, speed, \
               heading and generation time, a forged signature (the SPDU's signature \
               octets are corrupted, so a receiver checking the bytes finds it) and a \
               certificate outside its validity window, and a Sybil attacker's ghost \
               frames (genuine SPDUs under its concurrent pseudonyms, certificate \
               attached). DoS, DoSRandom, \
               VruImpersonation, VruPositionSpoof, FakeHazard and SelectiveDrop change \
               none of those and are refused by name rather than counted as lies nobody \
               could hear.",
    },
    KeyStatus {
        path: "threats.attackers[].params",
        status: Status::Wired,
        note: "Every key is read or refused: intensity, dt_s, magnitude_scale, dos_burst, \
               delay_s, expired_cert_lag_s, not_yet_valid_lead_s and each legacy magnitude \
               by name (const_pos_offset_m, teleport_dx_m, heading_offset_deg, …).",
    },
    KeyStatus {
        path: "threats.jammers",
        status: Status::Wired,
        note: "Jammers: constant, pulsed (period_ms, duty) or reactive (trigger_dbm), \
               with power_dbm and an active window, placed one way: position_m (fixed), \
               follow_node (riding a vehicle or unit) or path_m with speed_mps and \
               loop_path (driving a polyline). Their energy raises the noise at every \
               receiver in range, holds 802.11p carrier sense busy, counts as channel \
               load, enters a sidelink UE's S-RSSI and CBR (and so its CR limit), and a \
               frame they kill is reported 'jammed'.",
    },
    KeyStatus {
        path: "threats.compromised_rsus",
        status: Status::Wired,
        note: "Roadside units under an attacker's control, by their position in actors.rsus \
               (0 is the first). The unit keeps its trusted credentials and its backhaul, \
               and does what threats.compromised_rsu_attack says to the misbehaviour \
               reports vehicles hand it to forward, or to the CRL it repeats on the air.",
    },
    KeyStatus {
        path: "threats.compromised_rsu_attack",
        status: Status::Partial,
        note: "'threat/attacker/compromised-rsu'. params.kind: PoisonForwardedReports \
               (default: a relayed report is replaced, with poison_prob, by a forgery \
               framing a vehicle the unit heard, and the authority trusts it as \
               infrastructure), SuppressForwardedReports (dropped with suppress_prob) or \
               FalseCrl (fabricated_entries added to each CRL frame; the unit has no CRL \
               Generator key, so every vehicle discards the frame). from_s and to_s bound \
               it. FalseSpat, FalseMap and FalseCtl are refused: nothing acts on SPaT or \
               MAP content yet and units do not broadcast the trust list.",
    },
    KeyStatus {
        path: "threats.eavesdropper",
        status: Status::Wired,
        note: "'threat/observer/passive-privacy': passive sniffers at an evenly spread \
               params.sniffer_fraction of the signalised intersections (default 1), each \
               reading every safety frame sent within params.range_m (default 200 m) and \
               linking pseudonyms across changes by kinematics (Wiedersheim et al. 2010). \
               Absent, the observer hears every frame (a global eavesdropper). Its \
               linkability, anonymity set and tracking duration are in the run report.",
    },
    KeyStatus {
        path: "detection.local",
        status: Status::Wired,
        note: "'detect/legacy-12' on every honest vehicle and every roadside unit (a \
               unit is trusted infrastructure to the authority). Each message is judged \
               against the positional accuracy it states on the air (a BSM's J2735 \
               PositionalAccuracy, one sigma, scaled to 95 %; a CAM's 95 % confidence \
               ellipse); use_stated_accuracy: 0 restores the legacy constant 5 m. The \
               heading check allows for the bearing error those stated accuracies imply \
               over the longest straight baseline the sender's own headings describe \
               (CaTch, Kamel et al. 2019); heading_bearing_bound: 0 restores the legacy \
               one-step check, heading_straight_tol_deg (10) sets 'straight'. Each \
               receiver holds a map of the motor-vehicle lanes (F2MD's position \
               plausibility check): a claim whose whole stated confidence disc lies more \
               than offroad_tol_m (15 m) beyond the carriageway edge is off the road; \
               use_map: 0 is a receiver with no map, offroad_confidence_bound: 0 the \
               legacy check on the point. A PSM sender is checked as a vulnerable road \
               user (no vehicle kinematic or map check). params \
               override the suite's thresholds by name (consistency_threshold_m, \
               heading_threshold_deg, detector_lag_s, z_threshold, min_consecutive, \
               sybil_min_certs, art_max_m, max_accel_mps2, stale_max_s, …) and \
               report_interval_s (default 1: a reporter files about one subject at most \
               once a second). 'detect/ts103759' (listed alone) runs the ETSI TS 103 759 \
               observation suite on every message instead: params take its card's \
               parameters by name (threat/detector/ts103759-observations, each with its \
               default, unit and source), the labels environment, beacon_profile, fusion \
               and scoring, and the host keys above; each frame's J2735 VehicleSize (BSM, \
               centre reference) or CAM size (front-centre reference) and the receiver's \
               own class footprint feed its box check, and the run report tallies every \
               check per sender class (ts_checks, ts_messages). Only speedPlausibility \
               and accelerationPlausibility have an SM-MBR v1.1 subtype to file under.",
    },
    KeyStatus {
        path: "detection.ma",
        status: Status::Wired,
        note: "'threat/ma/corroborated' (the default when the key is absent): reports \
               about one pseudonym are correlated into events (every report observed \
               within event_window_s, 5 s, of an event's first is that one event, however \
               many reporters), an event counts when event_min_reporters (2) distinct \
               trusted reporters witnessed it, and the authority revokes on min_events (3) \
               such events inside window_s (60 s) from min_reporters (3) trusted \
               reporters. 'threat/ma/legacy-window', the legacy persistence gate, is kept \
               for parity with the legacy corpus: report_threshold_k trusted reporters (3) \
               in revoke_min_seconds distinct seconds (4) spanning revoke_persist_s (3 s) \
               inside revoke_window_s (15 s); it counts one event heard by many as many \
               and revokes honest vehicles in dense traffic. Both take defence, \
               reputation_max (40) and report_budget (30).",
    },
    KeyStatus {
        path: "detection.responder",
        status: Status::Refused,
        note: "Refused when set: the authority's only response in this build is \
               revocation through the credential protocol, which detection.ma decides.",
    },
    KeyStatus {
        path: "detection.perception_tier",
        status: Status::Refused,
        note: "Refused when set: there is no perception model in this build, so no \
               detector reads sensor data.",
    },
    // --- measurement -------------------------------------------------------
    KeyStatus {
        path: "metrics",
        status: Status::Wired,
        note: "Which metric providers to install; 'all' selects every registered one. \
               Opt-in groups beside it: 'signing', 'traffic', and 'detection-scores' (MCC, \
               informedness, markedness and Cohen's kappa at the report, vehicle and \
               per-message levels; it also records the local detectors' det.observation), \
               and 'ssm' (the full surrogate safety measures: crossing, merging and \
               oncoming TTC by trajectory extrapolation within 50 m, DRAC, MDRAC with a 1 s \
               reaction time, predicted PET, conflicts by type; SUMO SSM-device default \
               thresholds TTC 3.0 s, DRAC 3.0 m/s², MDRAC 3.4 m/s², PPET 2.0 s; Gettman et \
               al. 2008, Laureshyn et al. 2010, Fazekas et al. 2017; off by default).",
    },
    KeyStatus {
        path: "exporters",
        status: Status::Wired,
        note: "Written after the run, over its recording: 'recording' keeps the MCAP, \
               'jsonl', 'parquet' and 'arrow' write one table per recorded channel. \
               opts.profile 'node' drops every ground-truth channel and column. 'veremi' \
               writes VeReMi traces: opts.codebook 'veremi-ext' (default; F2MD attack \
               enum 0-19, Kamel et al. ICC 2020) or 'veremi-1' (bit codes 1/2/4/8/16, \
               Van der Heijden et al. 2018), opts.slice_s the slice length in seconds \
               (default 3600, F2MD VeReMiSliceTime; 1 to 604800); naming it records the \
               BSM's position accuracy and acceleration on node.tx. The SUMO-format \
               exporters (WP-06) write SUMO's own output files from a full recording: \
               'sumo-fcd', 'sumo-tripinfo' (opts.write_unfinished, default false), \
               'sumo-edgedata', 'sumo-lanedata', 'sumo-summary', 'sumo-e1' \
               (opts.detectors [{id, lane, pos_m}], lane a SUMO lane id) and 'sumo-e2' \
               ([{id, lane, pos_m, length_m}], opts.time_threshold_s 1 s, \
               speed_threshold_mps 1.39 m/s, jam_threshold_m 10 m: SUMO's defaults); \
               each takes opts.period_s (seconds; every step for fcd and summary, the \
               whole run otherwise) and opts.precision (decimals, default 2, SUMO \
               --precision). Naming one records gt.spawn and gt.despawn. Naming any \
               exporter makes the run record.",
    },
    KeyStatus {
        path: "events",
        status: Status::Wired,
        note: "Every kind acts, at control priority so everything at that instant sees it, \
               and writes a scenario.event record of what it did. WP-21: an item with a \
               when clause (ttc_below, in_polygon, speed_diff_above, probability, or one \
               all/any of them) is armed at t and fires at the first mobility step the \
               clause holds on ground truth, up to max_fires times (1) at least cooldown_s \
               apart (0); a safety.* target may name what the clause bound (when.leader, \
               when.follower, when.actor). weather.front: the \
               weather drivers and links see. outage: the node goes off. \
               demand.multiplier: the Poisson arrival rate is scaled (the candidate process \
               is sized to the timeline's peak). closure: the lanes of a lane, an edge or a \
               named street cost infinity to every router and every vehicle re-plans; one \
               already on a closed lane finishes it, and one that cannot avoid it leaves \
               the run at the barrier. param.change: weather.*, \
               actors.vehicles.demand.rate_veh_per_h (now), and \
               actors.vehicles.equipped_fraction, actors.vru.device_fraction, \
               security.verification_policy, security.pseudonym_change.*, \
               nodes.default_obu (vehicles that enter after the change); any other path is \
               refused. attack.wave: the named attacker populations act only inside the \
               wave, and a population may be in one wave. safety.hard-brake: a vehicle \
               (target: a node id, or auto for the first moving equipped vehicle with a car \
               close behind) brakes at decel_mps2 (default 0.5 g) to a stop and stands \
               hold_s (default 2). safety.breakdown: a vehicle stops at decel_mps2 (default \
               3) with its hazard lights on until 'until'. safety.cut-in: a vehicle changes \
               lane to side (left or right) at once into the gap ahead of that lane's \
               follower. An auto safety event that finds no vehicle fitting waits up to \
               within_s (default 10) and fires at the first step one does.",
    },
    KeyStatus {
        path: "apps",
        status: Status::Wired,
        note: "The V2X applications every equipped vehicle runs over the messages it \
               heard, decoded from their own octets, and its own vehicle's state; each \
               warning is recorded on app.warning and labelled true, false or missed \
               against ground truth (app.outcome).",
    },
    KeyStatus {
        path: "apps.enabled",
        status: Status::Wired,
        note: "Which applications run: fcw (forward collision, TTC 2.4 s — NHTSA's FCW test \
               timing), eebl (a vehicle ahead braking past 0.4 g, from its BSM or DENM), \
               ima (crossing traffic), lta (oncoming traffic inside the 4.1 s critical gap \
               when turning left, HCM), bsw (blind spot, ISO 17387; a lane-change warning \
               when signalling), pcw (pedestrians and cyclists from their PSM or VAM), rlvw \
               (running a red, from SPaT and MAP), glosa (the speed that meets the green, \
               from SPaT and MAP, recorded on app.advice).",
    },
    KeyStatus {
        path: "apps.glosa_compliance",
        status: Status::Wired,
        note: "The fraction of equipped drivers who follow GLOSA's advice: each driver \
               decides once, and a follower slows to the advised speed as it approaches. \
               No field compliance figure could be read for this build (the field \
               evaluations found, e.g. Stahlmann et al., IEEE VNC 2016, report that \
               simulations are too optimistic but give no rate), so the default is 0 \
               (advice shown, not followed); a study sets it and sweeps it.",
    },
    KeyStatus {
        path: "apps.fcw_ttc_s",
        status: Status::Wired,
        note: "FCW warns at or below this time to collision, seconds (default 2.4, NHTSA's \
               FCW confirmation test for a decelerating lead).",
    },
    KeyStatus {
        path: "apps.ima_tti_s",
        status: Status::Wired,
        note: "How far ahead IMA looks, as the ego's time to the conflict point, seconds \
               (default 4, this build's choice).",
    },
    KeyStatus {
        path: "apps.lta_gap_s",
        status: Status::Wired,
        note: "The gap LTA requires before a left turn, seconds (default 4.1, HCM 6th ed. \
               Exhibit 20-11).",
    },
    KeyStatus {
        path: "apps.pcw_ttc_s",
        status: Status::Wired,
        note: "PCW warns when a pedestrian or cyclist would be in the path within this \
               time, seconds (default 3, this build's choice).",
    },
    KeyStatus {
        path: "apps.rlvw_decel_mps2",
        status: Status::Wired,
        note: "RLVW warns once stopping would need more than this deceleration after a 1 s \
               reaction, m/s² (default 3.4, AASHTO's comfortable deceleration).",
    },
    KeyStatus {
        path: "mobility",
        status: Status::Wired,
        note: "Who drives the traffic. provider: sumo hands every vehicle to SUMO over \
               TraCI (needs SUMO installed and a sumo-net world); absent or native is this \
               engine's own driving.",
    },
    KeyStatus {
        path: "energy",
        status: Status::Wired,
        note: "Emissions, fuel and electric-vehicle energy of the traffic, integrated over \
               every vehicle's true trajectory and reported per window, class, lane and \
               vehicle (metrics/energy). Absent: no energy metrics. It changes no driving.",
    },
    KeyStatus {
        path: "energy.emission_model",
        status: Status::Wired,
        note: "The tailpipe model for combustion vehicles: hbefa3 (HBEFA 3.1 fit, 45 \
               classes, SUMO's HBEFA3), phemlight (power-based, the two published Euro 4 \
               cars) or moves-opmode (US EPA MOVES operating modes, light-duty).",
    },
    KeyStatus {
        path: "energy.emission_model_by_segment",
        status: Status::Wired,
        note: "A different emission model for some segments, e.g. passenger-car: phemlight \
               with hbefa3 for the rest; a segment not listed uses emission_model.",
    },
    KeyStatus {
        path: "energy.ev_model",
        status: Status::Wired,
        note: "The battery model for electric vehicles: kurczveil (SUMO's battery device) \
               or vt-cpem (Fiori et al. 2016, deceleration-dependent regeneration).",
    },
    KeyStatus {
        path: "energy.fleet",
        status: Status::Wired,
        note: "Per segment (passenger-car, light-commercial, heavy-truck, urban-bus, coach, \
               motorcycle): the electric share, the diesel share of the rest and the Euro \
               class. Each vehicle draws its powertrain once, keyed by its id.",
    },
    KeyStatus {
        path: "energy.ev",
        status: Status::Wired,
        note: "Electric vehicles' starting state of charge and overrides of the EV model's \
               card parameters (mass_kg, capacity_wh, ...); an unknown name is refused.",
    },
    KeyStatus {
        path: "energy.stop_speed_mps",
        status: Status::Wired,
        note: "Below this speed a vehicle counts as stopped (default 0.1 m/s, SUMO's \
               halting threshold); the stops metrics read it.",
    },
    KeyStatus {
        path: "experiment",
        status: Status::Wired,
        note: "The parameter sweep. Expanded by the experiment runner, not by a single \
               run: the engine clears it before running a cell.",
    },
    // network audit
    KeyStatus {
        path: "messages.spat_interval_ms",
        status: Status::Wired,
        note: "A roadside unit's SPaT period (CTI 4501's 100 ms unless set). Refused \
               unless messages.sets has spat.",
    },
    KeyStatus {
        path: "messages.map_interval_ms",
        status: Status::Wired,
        note: "A roadside unit's MAP period (CTI 4501's 1 s unless set). Refused unless \
               messages.sets has map.",
    },
    KeyStatus {
        path: "messages.cpm_interval_ms",
        status: Status::Wired,
        note: "T_GenCpm, how often a vehicle checks whether a CPM is due (TS 103 324's \
               100 ms unless set). Refused unless messages.sets has cpm.",
    },
    KeyStatus {
        path: "messages.cpm_dcc_limited",
        status: Status::Wired,
        note: "T_GenCpm follows the DCC's T_off, clamped to [100, 1000] ms (TS 103 324 \
               §6.1.2.1, Annex D). Refused unless messages.sets has cpm.",
    },
    KeyStatus {
        path: "messages.cpm_quality_threshold",
        status: Status::Wired,
        note: "ObjectPerceptionQualityThreshold: objects whose §7.1.8.6 quality does not \
               exceed it are not sent, and the computed quality is encoded (TS 103 324 \
               §6.1.2.3, Table F.1's 3). Refused unless messages.sets has cpm.",
    },
    KeyStatus {
        path: "messages.cpm_max_objects",
        status: Status::Wired,
        note: "Objects per CPM: the selection is ordered by the §6.1.3.2 utility and the \
               lowest-utility objects past it wait for the next event. Refused unless \
               messages.sets has cpm.",
    },
    KeyStatus {
        path: "messages.denm_traffic_condition",
        status: Status::Wired,
        note: "The C2C-CC RS 2007 sudden-speed-drop (dangerousEndOfQueue 27) and \
               local-slow-down (trafficCondition 1) DENMs. Refused unless messages.sets \
               has denm.",
    },
    KeyStatus {
        path: "messages.denm_traction_loss",
        status: Status::Wired,
        note: "The C2C-CC RS 2002 traction-loss DENM (adhesion 6) from the own ABS's \
               friction estimate. Refused unless messages.sets has denm.",
    },
    KeyStatus {
        path: "messages.path_history_points",
        status: Status::Wired,
        note: "The most path-history points a BSM's Part II or a CAM carries (J2945/1's 15 \
               unless set; 0 to 23). Refused unless messages.sets has bsm or cam.",
    },
    KeyStatus {
        path: "radio.channel_model.fading_coherence",
        status: Status::Wired,
        note: "independent (unset) draws each frame's fade on its own; doppler keys it by \
               the link's coherence block, 0.423/f_D with f_D = (|v_tx|+|v_rx|)·f_c/c. \
               Refused when the run draws no fading.",
    },
    KeyStatus {
        path: "nodes.verify_queue_depth",
        status: Status::Wired,
        note: "Each node's verification queue depth (64 unless set); a full queue drops \
               (or, under prioritized, sheds the oldest).",
    },
    // --- traffic audit (2026-10-11) ----------------------------------------
    KeyStatus {
        path: "traffic.critical_gap_factor",
        status: Status::Wired,
        note: "Scales the HCM critical gaps and follow-up times at junctions without signals; below 1 drivers squeeze into shorter gaps.",
    },
    KeyStatus {
        path: "traffic.permitted_left_gap_s",
        status: Status::Wired,
        note: "The gap in oncoming traffic a left turn on a green waits for.",
    },
    KeyStatus {
        path: "traffic.turn_lateral_accel_mps2",
        status: Status::Wired,
        note: "Caps the speed through a turn at sqrt(a times the corner radius); 0 is no cap.",
    },
    KeyStatus {
        path: "traffic.turn_curve_window_m",
        status: Status::Wired,
        note: "Judges a corner's radius over its tightest arc of this length instead of its average.",
    },
    KeyStatus {
        path: "traffic.heavy_vehicle_gap_terms",
        status: Status::Wired,
        note: "Heavy vehicles add the HCM 6th ed. Exhibit 20-11 heavy-vehicle critical gap and follow-up terms.",
    },
    KeyStatus {
        path: "traffic.critical_gap_cv",
        status: Status::Wired,
        note: "Each driver's critical gap is lognormal around the HCM value with this coefficient of variation.",
    },
    KeyStatus {
        path: "traffic.time_to_impatience_s",
        status: Status::Wired,
        note: "A yielding driver's critical gap falls with standing time, to its floor after this many seconds (SUMO 180 s).",
    },
    KeyStatus {
        path: "traffic.impatience_gap_floor",
        status: Status::Wired,
        note: "The critical gap at full impatience as a share of the driver's own.",
    },
    KeyStatus {
        path: "traffic.impatience_foe_braking",
        status: Status::Wired,
        note: "An impatient driver judges a foe as if it would brake at impatience times 4.5 m/s².",
    },
    KeyStatus {
        path: "traffic.permissive_sneakers",
        status: Status::Wired,
        note: "Permitted left turners per lane that complete the turn in the amber ending a permissive green (HCM: 2).",
    },
    KeyStatus {
        path: "traffic.standing_leader_stop",
        status: Status::Wired,
        note: "A driver closing on a standing vehicle brakes to stop at its standstill gap at constant deceleration, as at a stop line.",
    },
    KeyStatus {
        path: "traffic.depart_speed_max",
        status: Status::Wired,
        note: "A trip enters the road at the highest safe speed there (SUMO departSpeed=max) instead of at rest.",
    },
    KeyStatus {
        path: "traffic.lane_bend_speed",
        status: Status::Wired,
        note: "The bends of a through lane's polyline get the turn-speed cap a junction connector has.",
    },
    KeyStatus {
        path: "traffic.vehicle_dynamics",
        status: Status::Wired,
        note: "Grade-aware, power-limited acceleration: grade subtracts g sin(theta) of the draped lane from the driver's acceleration (KraussPS); power_limit caps it at eta P/(m v) less rolling and air resistance for passenger cars (PHEMlight PC_D_EU4) and tractor-trailers (Green Book 120 kg/kW). Native engine only.",
    },
    KeyStatus {
        path: "traffic.emergency_yield",
        status: Status::Wired,
        note: "Drivers with an emergency vehicle running its beacons within reaction_distance_m behind on the same carriageway yield by the jurisdiction's rule (rules::EmergencyYield): in the US they move to the right-hand lane and stop clear of the junction (NY VTL 1144(a), UVC 11-405). The emergency vehicle passes a red after stopping (or slowing to red_pass_speed_mps) once the junction is clear (UVC 11-106(b)2). Native engine only.",
    },
    KeyStatus {
        path: "traffic.depart_at_speed",
        status: Status::Wired,
        note: "Vehicles entering at the map's edge are put on at the lane's start at their desired speed, capped by the safe speed to the leader and the lane end (SUMO departSpeed).",
    },
    KeyStatus {
        path: "traffic.reroute_share",
        status: Status::Wired,
        note: "The share of drivers who re-route on information, drawn per driver (SUMO device.rerouting.probability).",
    },
    KeyStatus {
        path: "traffic.reroute_source",
        status: Status::Wired,
        note: "navigation: network-wide smoothed speeds and every closure; v2x: only received and verified DENM/TIM hazard warnings on the route.",
    },
    KeyStatus {
        path: "traffic.lc_strategic",
        status: Status::Wired,
        note: "SUMO LC2013 look-ahead for route lane changes; any lc_* switches discretionary changes from MOBIL to LC2013.",
    },
    KeyStatus {
        path: "traffic.lc_cooperative",
        status: Status::Wired,
        note: "SUMO LC2013: moves aside for a neighbour that must merge in.",
    },
    KeyStatus {
        path: "traffic.lc_speed_gain",
        status: Status::Wired,
        note: "SUMO LC2013 speed-gain accumulator threshold 0.2 / value.",
    },
    KeyStatus {
        path: "traffic.lc_keep_right",
        status: Status::Wired,
        note: "SUMO LC2013 keep-right accumulator.",
    },
    KeyStatus {
        path: "traffic.kerbside",
        status: Status::Wired,
        note: "Overrides the jurisdiction's kerbside stops: bus stops and dwell, taxi share and dwell, delivery share and dwell. Not applied to SUMO runs.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.speed_factor",
        status: Status::Wired,
        note: "Mean free speed as a multiple of the limit (SUMO speedFactor). Needs realism other than identical.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.speed_spread",
        status: Status::Wired,
        note: "Multiplies each class's speed deviation (SUMO speedDev). Needs realism other than identical.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.red_runner_share",
        status: Status::Wired,
        note: "The share of drivers who go on an amber inside their dilemma-zone threshold \
               without checking they reach the line before the red, so some enter on red. \
               Microscopic engine only. Needs realism other than identical.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.violations",
        status: Status::Wired,
        note: "Rule violations and crash states (WP-08): red-light running after the red onset \
               (Bonneson et al., TTI 0-4196-P1), amber running beyond the dilemma zone, failure \
               to yield (SUMO jmIgnoreFoeProb/Speed semantics) and collisions that stop both \
               vehicles and block the lane until cleared. Off by default. Microscopic engine \
               only; needs realism other than identical.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.start_reaction_s",
        status: Status::Wired,
        note: "Median reaction to a green or a cleared crosswalk. Needs realism other than identical.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.queue_start_delay_s",
        status: Status::Wired,
        note: "Median start-up delay behind the car in front: sets start-up lost time and saturation flow. Needs realism other than identical.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.truck",
        status: Status::Wired,
        note: "Replaces the car-following a, b, T and s0 of trucks and trailers; each driver keeps their own spread and style around them.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.bus",
        status: Status::Wired,
        note: "Replaces the car-following a, b, T and s0 of buses and coaches; each driver keeps their own spread and style around them.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.motorcycle",
        status: Status::Wired,
        note: "Replaces the car-following a, b, T and s0 of motorcycles and mopeds; each driver keeps their own spread and style around them.",
    },
    KeyStatus {
        path: "actors.vehicles.drivers.bicycle",
        status: Status::Wired,
        note: "Replaces the car-following a, b, T and s0 of cyclists; each driver keeps their own spread and style around them.",
    },
    KeyStatus {
        path: "world.signals.cycle_s",
        status: Status::Wired,
        note: "Cycle length of the plans synthesised for an imported (OSM) world.",
    },
    KeyStatus {
        path: "world.source.params.ew_green_share",
        status: Status::Wired,
        note: "The share of each signalised junction's green given to the east-west street \
               of a generated grid (world/source/procedural-grid with signalised: true); \
               the north-south street gets the rest. Default 0.5, an even split (Webster \
               1958: green in proportion to the critical flow ratios, equal on a symmetric \
               grid). An imported city's plans come from the map, not from this.",
    },
    KeyStatus {
        path: "world.signals.join_m",
        status: Status::Wired,
        note: "Signalised junctions joined into one, with one plan, when the road between \
               them is at most this long (netconvert --tls.join; SUMO's --tls.join-dist \
               default is 20 m). Unset joins none, so an imported world builds as before. \
               Stops a 12-16 m link between two plans filling on one plan's turns and \
               holding cars on the other's green.",
    },
    KeyStatus {
        path: "world.signals.all_red_s",
        status: Status::Wired,
        note: "Floor under each phase's all-red in an imported world's plans.",
    },
    KeyStatus {
        path: "world.signals.pedestrian_walk_s",
        status: Status::Wired,
        note: "Walk interval an imported world's greens leave room for.",
    },
    KeyStatus {
        path: "actors.vehicles.gnss",
        status: Status::Wired,
        note: "The positioning error every equipped vehicle's BSM carries: receiver grade, outliers, degraded bursts and outages; and the accuracy its PositionalAccuracy states (stated_sigma_m, stated_sigma_log_sd: unset states the model's honest radius, set states a deployed OBU's, CV-pilot medians 0.30-2.00 m).",
    },
    KeyStatus {
        path: "actors.vehicles.bsm_timing",
        status: Status::Wired,
        note: "Per-device BSM timing (locked / free classes): secMark phase, position-epoch slips, frozen repeats and generationTime jitter on the transmitted BSM.",
    },
    KeyStatus {
        path: "world.signals.pedestrian_speed_mps",
        status: Status::Wired,
        note: "Walking speed an imported world's pedestrian clearance is timed for.",
    },
    // --- end traffic audit ----------------------------------------------------
];

/// The status entry that governs `path`: the longest matching prefix.
pub fn status_of(path: &str) -> Option<&'static KeyStatus> {
    KEY_STATUS
        .iter()
        .filter(|k| covers(k.path, path))
        .max_by_key(|k| k.path.len())
}

/// Whether the status entry `entry` governs the leaf `path`.
///
/// A prefix match is on a path segment boundary, so `net.layer` does not govern
/// `net.layerx` and `meta` governs `meta.tags`.
fn covers(entry: &str, path: &str) -> bool {
    if path == entry {
        return true;
    }
    let Some(rest) = path.strip_prefix(entry) else {
        return false;
    };
    rest.starts_with('.') || rest.starts_with('[')
}

/// The bound declared for `path`, if there is one.
pub fn bound_of(path: &str) -> Option<&'static Bound> {
    BOUNDS.iter().find(|b| b.path == path)
}

/// The choice set declared for `path`, if there is one.
pub fn choices_of(path: &str) -> Option<&'static Choices> {
    CHOICES.iter().find(|c| c.path == path)
}

/// Every problem with `s`, in schema order. Empty means the scenario is loadable.
pub fn validate(s: &Scenario) -> Vec<ScenarioError> {
    let mut e = Vec::new();
    time(s, &mut e);
    calendar(s, &mut e);
    fast_forward(s, &mut e);
    world(s, &mut e);
    actors(s, &mut e);
    weather(s, &mut e);
    radio(s, &mut e);
    net(s, &mut e);
    messages(s, &mut e);
    security(s, &mut e);
    nodes(s, &mut e);
    threats(s, &mut e);
    metrics_and_exporters(s, &mut e);
    timeline(s, &mut e);
    apps(s, &mut e);
    energy(s, &mut e);
    experiment(s, &mut e);
    scale(s, &mut e);
    security_backend(s, &mut e);
    e.extend(crate::driving::validate(s));
    network_audit(s, &mut e);
    scms_lifecycle(s, &mut e);
    unreachable_keys(s, &mut e);
    absent_radio_holders(s, &mut e);
    perception(s, &mut e);
    e
}

/// Roadside and handset radio settings do nothing when the scenario has no roadside unit
/// or no pedestrian or cyclist carrying a device, the same rule the per-class driver
/// settings follow.
fn absent_radio_holders(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let d = &s.radio.devices;
    let no_rsu = s.actors.rsus.is_empty() && s.actors.rsu_placement.is_none();
    if no_rsu && d.rsu != crate::scenario::schema::RsuRadio::default() {
        e.push(conflict(
            "radio.devices.rsu",
            "sets the roadside units' radio, and the scenario has none; add actors.rsus or \
             actors.rsu_placement, or unset it"
                .to_string(),
        ));
    }
    if let Some(o) = &d.rsu.off_channel
        && (o.min_duration_ms > o.max_duration_ms || o.from_ms + o.max_duration_ms > 100.0)
    {
        e.push(conflict(
            "radio.devices.rsu.off_channel",
            format!(
                "leaves at {} ms for {}..{} ms: the shortest absence must not exceed the \
                 longest, and the unit must be back within its 100 ms sync interval",
                o.from_ms, o.min_duration_ms, o.max_duration_ms
            ),
        ));
    }
    let v = &s.actors.vru;
    let no_handset = v.pedestrians + v.cyclists == 0 || v.device_fraction == 0.0;
    if no_handset && d.vru != crate::scenario::schema::VruRadio::default() {
        e.push(conflict(
            "radio.devices.vru",
            "sets pedestrians' and cyclists' handset radio, and none carries one; add \
             actors.vru.pedestrians or actors.vru.cyclists with a device_fraction above 0, \
             or unset it"
                .to_string(),
        ));
    }
}

/// The security lifecycle's and the backend's keys, checked by the very functions that
/// build them (`crate::backend`, `crate::phase2`), so a value the loader accepts is a
/// value the run can act on and the page hears about a bad one at Apply rather than at
/// run start.
fn security_backend(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let mut take = |r: crate::error::Result<()>| {
        if let Err(crate::EngineError::Scenario(inner)) = r {
            e.push(inner);
        }
    };
    take(
        crate::backend::BackendAccess::from_scenario(s).and_then(|access| {
            for (i, rsu) in s.actors.rsus.iter().enumerate() {
                access
                    .backhaul_with(rsu.backhaul.as_deref(), rsu.backhaul_params.as_ref())
                    .map_err(|_| {
                        crate::EngineError::Scenario(ScenarioError::conflict(
                            &format!("actors.rsus[{i}].backhaul"),
                            format!(
                                "'{}' is not a backhaul model; allowed: {}",
                                rsu.backhaul.as_deref().unwrap_or(""),
                                crate::backend::BACKHAUL_MODELS.join(", ")
                            ),
                        ))
                    })?;
            }
            Ok(())
        }),
    );
    take(crate::phase2::LifecycleParams::from_scenario(s).map(|_| ()));
    // The detectors', the authority's and the backend network's parameters, read by the
    // very functions the run builds them with. Before these were here, an unknown
    // `detection.ma.params` key or a wrong `net.backend_net` id passed Check and came back
    // from Run as an "internal error".
    take(crate::phase2::detection_params(s).map(|_| ()));
    take(crate::phase2::MaHost::from_scenario(s).map(|_| ()));
    take(crate::phase2::apply_backend_net(
        s,
        &mut v2xw_proto::ScmsParams::default(),
    ));
    take(crate::crypto_config::check(s));
    for (i, a) in s.threats.attackers.iter().enumerate() {
        if let Err(crate::EngineError::Scenario(ScenarioError::Conflict {
            field,
            conflict: why,
        })) = crate::phase2::evasion_params(&a.id, &a.params)
        {
            e.push(conflict(
                &field.replace("threats.attackers[]", &format!("threats.attackers[{i}]")),
                why,
            ));
        }
        if let Some(kind) =
            a.id.strip_prefix("threat/attacker/legacy/")
                .and_then(v2xw_threat::AttackKind::parse)
            && let Err(crate::EngineError::Scenario(ScenarioError::Conflict {
                field,
                conflict: why,
            })) = crate::phase2::attacker_params(kind, &a.params)
        {
            e.push(conflict(
                &field.replace("threats.attackers[]", &format!("threats.attackers[{i}]")),
                why,
            ));
        }
    }
    // The same rule `phase2::Phase2::build` enforces at run start, here so the settings
    // window marks the field when the settings are applied rather than the run refusing
    // to start (found in the quick-settings sweep: Security -> European C-ITS on a
    // scenario built on base/city.yaml, which names the CAMP backend).
    if let (Some(a), Some(b)) = (
        s.actors.backend.protocol.as_ref(),
        s.security.protocol.as_ref(),
    ) && *a != b.id
    {
        e.push(conflict(
            "security.protocol",
            format!(
                "names {} but actors.backend.protocol names {a}; one deployment runs one \
                 credential protocol, so set both to the same one",
                b.id
            ),
        ));
    }
    // `security.scms` describes an SCMS deployment: on any other protocol it would do
    // nothing (the settings contract refuses it), and a deployment that does not build is
    // refused here as the run would refuse it.
    if s.security.scms.is_some() {
        let protocol = s
            .actors
            .backend
            .protocol
            .clone()
            .or_else(|| s.security.protocol.as_ref().map(|c| c.id.clone()));
        if protocol.as_deref() != Some(crate::phase2::CAMP_SCMS) {
            e.push(conflict(
                "security.scms",
                format!(
                    "describes an SCMS deployment, and the credential protocol is {}; set \
                     security.protocol to {} or remove security.scms",
                    protocol.as_deref().unwrap_or("none"),
                    crate::phase2::CAMP_SCMS
                ),
            ));
        } else if let Err(crate::EngineError::Scenario(inner)) =
            crate::phase2::deployment_of(s, true)
        {
            e.push(inner);
        } else if let Err(crate::EngineError::Scenario(inner)) =
            crate::phase2::home_chain_file_of(s)
        {
            e.push(inner);
        }
    }
    let etsi = s.actors.backend.protocol.as_deref() == Some(crate::phase2::ETSI_PKI)
        || s.security
            .protocol
            .as_ref()
            .is_some_and(|p| p.id == crate::phase2::ETSI_PKI);
    if etsi && s.security.envelope != "etsi103097" {
        e.push(conflict(
            "security.envelope",
            format!(
                "is '{}', and the ETSI ITS PKI issues TS 103 097 certificates and \
                 authorization tickets; a station on it signs 'etsi103097' envelopes",
                s.security.envelope
            ),
        ));
    }
    if !crate::signature::KEMS.contains(&s.security.kem.as_str()) {
        e.push(conflict(
            "security.kem",
            format!(
                "'{}' is not a key-encapsulation scheme this build models; allowed: {}",
                s.security.kem,
                crate::signature::KEMS.join(", ")
            ),
        ));
    }
    if !crate::signature::SIGNATURES.contains(&s.security.signature.as_str()) {
        e.push(conflict(
            "security.signature",
            format!(
                "'{}' is not a signature scheme this build models; allowed: {}",
                s.security.signature,
                crate::signature::SIGNATURES.join(", ")
            ),
        ));
    }
    let declared_rsus = s.actors.rsus.len()
        + s.actors
            .rsu_placement
            .as_ref()
            .map_or(0, |p| p.count as usize);
    for &unit in &s.threats.compromised_rsus {
        if unit as usize >= declared_rsus {
            e.push(conflict(
                "threats.compromised_rsus",
                format!(
                    "names roadside unit {unit}, and actors.rsus declares {}; a unit is \
                     named by its position in that list, from 0",
                    declared_rsus
                ),
            ));
        }
    }
    // The lifecycle refuses this too, but only a scenario that runs the lifecycle reaches
    // it: on a run without a credential backend the attack was accepted and did nothing
    // (the security-track catalogue sweep found it dead on the DSRC family).
    if s.threats.compromised_rsu_attack.is_some() && s.threats.compromised_rsus.is_empty() {
        e.push(conflict(
            "threats.compromised_rsu_attack",
            "is set but threats.compromised_rsus names no roadside unit, so no unit \
             carries it out; name the units by their position in actors.rsus or remove it"
                .to_string(),
        ));
    }
    if s.detection.responder.is_some() {
        e.push(conflict(
            "detection.responder",
            "is set, and this build's only response to the authority's decision is \
             revocation through the credential protocol; remove it"
                .to_string(),
        ));
    }
    if s.detection.perception_tier.is_some() {
        e.push(conflict(
            "detection.perception_tier",
            "is set, and there is no perception model in this build; remove it".to_string(),
        ));
    }
}

/// Keys this build validates, documents and hashes but **cannot act on**.
///
/// The vertical-slice audit's finding, in one function: a scenario that says something the
/// engine never reads is a scenario that overstates what it controls, and a run that
/// accepted it produced a result the file appears to explain and does not. Everything
/// here is refused rather than warned about, because a warning in a log is a warning
/// nobody reads and the whole point of §13's validation is that the author finds out at
/// load rather than at analysis.
///
/// Each message names the *seam* — the model or the field that would have to exist — so
/// that removing a rule from here is a one-line change the day the seam is filled, and so
/// that a reader can tell "not implemented" from "not allowed".
///
/// The six keys the audit found are wired rather than refused and are **not** here:
/// `messages.sets` reaches [`crate::wiring::service_set`], `security.crypto_mode`
/// reaches the node's crypto backend, `security.envelope` and
/// `security.signer_id_policy` reach its security stack, `nodes.per_class` reaches its
/// hardware profile, and `time.t0` reaches its wall clock through
/// [`crate::wiring::NodeEnv`].
fn unreachable_keys(s: &Scenario, e: &mut Vec<ScenarioError>) {
    // `actors.vru.device_fraction` used to be refused here: the node phase held vehicle
    // OBUs only. It now hosts VRU devices (`crate::hosted`), so any fraction in [0, 1] is
    // run as written.

    // Exporters run after the run, over its recording (`crate::export`). An id this build
    // does not implement is refused rather than skipped, so a list never promises a file
    // that will not be written.
    for (i, x) in s.exporters.iter().enumerate() {
        if !crate::export::EXPORTERS.contains(&x.id.as_str()) && !x.id.trim().is_empty() {
            e.push(conflict(
                &format!("exporters[{i}].id"),
                format!(
                    "'{}' is not an exporter this build has; one of {}",
                    x.id,
                    crate::export::EXPORTERS.join(", ")
                ),
            ));
        }
        if let Err(ScenarioError::Conflict {
            field,
            conflict: why,
        }) = crate::export::profile_of(x, i).map_err(|err| match err {
            crate::EngineError::Scenario(inner) => inner,
            other => ScenarioError::conflict("exporters", other.to_string()),
        }) {
            e.push(conflict(&field, why));
        }
        if x.id == "veremi"
            && let Err(crate::EngineError::Scenario(ScenarioError::Conflict {
                field,
                conflict: why,
            })) = crate::export::veremi_options_of(x, i)
        {
            e.push(conflict(&field, why));
        }
        if crate::sumo_outputs::is_sumo(&x.id)
            && let Err(crate::EngineError::Scenario(ScenarioError::Conflict {
                field,
                conflict: why,
            })) = crate::export::sumo_options_of(x, i)
        {
            e.push(conflict(&field, why));
        }
    }

    // The fragmenter (`crate::frag`): the model and its parameters resolve against its
    // card, and the padding knob is the engine's.
    if let Some(f) = s.net.fragmenter.as_ref() {
        match crate::frag::FragPlan::from_choice(f) {
            Err(why) => e.push(conflict("net.fragmenter", why)),
            Ok(plan) if plan.strategy.is_some() => {
                if s.radio.rat != crate::scenario::schema::Rat::Dsrc80211p {
                    e.push(conflict(
                        "net.fragmenter",
                        format!(
                            "is '{}', and fragmentation runs over 802.11p only in this build: \
                             a sidelink transport block takes its size in sub-channels \
                             (04-models.md §5.1) and its error model does not expose the \
                             per-block success probability the loss prediction is built from",
                            f.id
                        ),
                    ));
                }
                if f.id == v2xw_net::FRAGMENTER_FACILITIES_ID && s.net.layer != "gn-btp" {
                    e.push(conflict(
                        "net.fragmenter",
                        "is facilities-layer segmentation, which is the ETSI stack's \
                         (TS 103 301 MAPEM layerID, TS 103 324 CPM messageSegmentInfo); \
                         set net.layer to 'gn-btp', or use fragmenter/generic-sdu on wsmp"
                            .to_string(),
                    ));
                }
                if f.id == v2xw_net::FRAGMENTER_CERT_CYCLE_ID {
                    cert_cycle_conflicts(s, f, plan, e);
                }
            }
            Ok(plan) if plan.padding > 0 => e.push(conflict(
                "net.fragmenter.params.sdu_padding_bytes",
                "pads every signed message past what fragmenter/none can send; with no \
                 splitting fragmenter every padded message above the MTU would be refused"
                    .to_string(),
            )),
            Ok(_) => {}
        }
    }

    // Message generators: which node decides to send each set (`v2xw_node::ServiceSet` and
    // `crate::wiring`'s vehicle_services / rsu_services), and what that decision needs from
    // the rest of the scenario.
    let has = |name: &str| s.messages.sets.iter().any(|x| x == name);
    let role = |name: &str| {
        s.actors
            .rsus
            .iter()
            .map(|r| &r.roles)
            .chain(s.actors.rsu_placement.iter().map(|p| &p.roles))
            .any(|roles| roles.iter().any(|x| x == name || x == "spat-map"))
    };
    for (i, set) in s.messages.sets.iter().enumerate() {
        let field = format!("messages.sets[{i}]");
        match set.as_str() {
            "bsm" | "cam" => {}
            "denm" if s.net.layer != "gn-btp" => e.push(conflict(
                &field,
                "'denm' is the ETSI stack's event message (EN 302 637-3) and goes out over \
                 GeoNetworking/BTP; with net.layer 'wsmp' it would be a DENM on a US channel \
                 under the BSM's PSID. Set net.layer to 'gn-btp' (the US stack's hard-braking \
                 signal is the BSM's event flag, not a separate message)"
                    .to_string(),
            )),
            "denm" => {}
            "spat" | "map" if !role(set) => e.push(conflict(
                &field,
                format!(
                    "'{set}' is broadcast by a roadside unit wired to a signal controller, \
                     and no actors.rsus entry has the '{set}' role; add one standing at a \
                     signalised junction"
                ),
            )),
            "spat" | "map" => {}
            "srm" if !has("map") => e.push(conflict(
                &field,
                "'srm' is a vehicle's request to the junction whose MAP it heard; without \
                 'map' in messages.sets no vehicle knows a junction to ask"
                    .to_string(),
            )),
            "srm"
                if !s
                    .actors
                    .vehicles
                    .classes
                    .get("emergency")
                    .is_some_and(|c| c.fraction > 0.0) =>
            {
                e.push(conflict(
                    &field,
                    "'srm' is sent by vehicles entitled to signal priority, which in this build \
                     is the 'emergency' class, and actors.vehicles.classes gives it no share"
                        .to_string(),
                ));
            }
            "srm" => {}
            "ssm" if !(has("srm") && role("spat")) => e.push(conflict(
                &field,
                "'ssm' is a signal controller's answer to a request: it needs 'srm' in \
                 messages.sets and a roadside unit with the 'spat' role to answer from"
                    .to_string(),
            )),
            "ssm" => {}
            // The perception model (`crate::perception`) fills it now; the CPM is an ETSI
            // message and rides GeoNetworking/BTP (port 2009).
            "cpm" if s.net.layer != "gn-btp" => e.push(conflict(
                &field,
                "'cpm' (ETSI TS 103 324) is an ETSI facilities message carried over \
                 GeoNetworking/BTP: set net.layer to gn-btp"
                    .to_string(),
            )),
            "cpm" => {}
            _ => e.push(conflict(
                &field,
                format!(
                    "'{set}' has no generator in this build: a node would never decide to \
                     send one"
                ),
            )),
        }
    }
}

/// `radio.channel_model` and `radio.sidelink` against the run's law and technology: every
/// override the chosen law or radio would ignore is refused rather than silently accepted.
/// The ranges and closed sets of the fields both share are [`radio_tuning`]'s.
fn radio_law_checks(s: &Scenario, e: &mut Vec<ScenarioError>) {
    use crate::scenario::schema::Rat;
    use crate::wiring::PropagationChoice;
    let c = &s.radio.channel_model;
    for (leaf, v) in [
        ("nakagami_shape_far", c.nakagami_shape_far),
        ("nakagami_breakpoint_m", c.nakagami_breakpoint_m),
        ("building_wall_loss_db", c.building_wall_loss_db),
        ("building_loss_db_per_m", c.building_loss_db_per_m),
    ] {
        if let Some(v) = v {
            let path = format!("radio.channel_model.{leaf}");
            bounded_at(&path, &path, v, e);
        }
    }
    // WP-09: a measured antenna table must parse, and its rotations sit in their bounds.
    if let Some(sp) = &s.radio.devices.obu.sampled_pattern {
        if let Err(why) = v2xw_radio::antenna::SampledAntenna::from_csv(&sp.csv) {
            e.push(conflict(
                "radio.devices.obu.sampled_pattern.csv",
                format!("is not an antenna pattern table: {why}"),
            ));
        }
        for (leaf, v) in [
            ("rotation_deg", sp.rotation_deg),
            ("rotation_jitter_deg", sp.rotation_jitter_deg),
        ] {
            let path = format!("radio.devices.obu.sampled_pattern.{leaf}");
            bounded_at(&path, &path, v, e);
        }
    }
    let sl = &s.radio.sidelink;
    for (leaf, v) in [
        ("candidate_percent", sl.candidate_percent.map(f64::from)),
        ("selection_window_ms", sl.selection_window_ms.map(f64::from)),
    ] {
        if let Some(v) = v {
            let path = format!("radio.sidelink.{leaf}");
            bounded_at(&path, &path, v, e);
        }
    }

    let tier = s.radio.tiers.propagation;
    let law = crate::wiring::propagation_choice_at(s, tier);
    let abstract_tier = tier == v2xw_core::card::Tier::Abstract;
    let law_name = match law {
        PropagationChoice::FreeSpace => "propagation/free-space",
        PropagationChoice::TwoRayGround => "propagation/two-ray-ground",
        PropagationChoice::LogDistance(..) => "propagation/log-distance-shadowing",
        PropagationChoice::Tr37885 => "propagation/tr37885",
        PropagationChoice::Geometric => "propagation/v2v-urban-geometric",
        PropagationChoice::WinnerB1(..) => "propagation/winner-plus-b1",
        PropagationChoice::TwoRayInterference(_) => "propagation/two-ray-interference",
    };

    if c.path_loss_exponent.is_some() && !matches!(law, PropagationChoice::LogDistance(..)) {
        e.push(conflict(
            "radio.channel_model.path_loss_exponent",
            format!(
                "sets the log-distance law's slope, and radio.models.propagation names                  {law_name}, which has its own; choose propagation/log-distance-shadowing                  or remove the named law"
            ),
        ));
    }
    let no_shadow = matches!(
        law,
        PropagationChoice::FreeSpace
            | PropagationChoice::TwoRayGround
            | PropagationChoice::TwoRayInterference(_)
    );
    for (leaf, set) in [
        ("shadowing_sigma_db", c.shadowing_sigma_db.is_some()),
        (
            "shadowing_decorrelation_m",
            c.shadowing_decorrelation_m.is_some(),
        ),
    ] {
        if set && no_shadow {
            e.push(conflict(
                &format!("radio.channel_model.{leaf}"),
                format!("tunes shadowing, and {law_name} is a deterministic law with none"),
            ));
        }
    }
    if (c.nakagami_shape_far.is_some() || c.nakagami_breakpoint_m.is_some())
        && c.fading.as_deref() != Some("nakagami")
    {
        e.push(conflict(
            "radio.channel_model.nakagami_shape_far",
            "and nakagami_breakpoint_m shape a Nakagami channel; set              radio.channel_model.fading to 'nakagami'"
                .to_string(),
        ));
    }
    if c.nakagami_shape_far.is_some() != c.nakagami_breakpoint_m.is_some() {
        e.push(conflict(
            "radio.channel_model.nakagami_breakpoint_m",
            "and nakagami_shape_far go together: a distance-dependent channel needs both              the far m and where it starts"
                .to_string(),
        ));
    }
    if abstract_tier && c.fading.as_deref().is_some_and(|f| f != "none") {
        e.push(conflict(
            "radio.channel_model.fading",
            "would change nothing: the abstract propagation tier draws no fading".to_string(),
        ));
    }
    // Doppler coherence correlates a fading draw; a run that draws none has nothing to
    // correlate (wiring::draws_fading).
    if c.fading_coherence.as_deref() == Some("doppler") && !crate::wiring::draws_fading(s) {
        e.push(conflict(
            "radio.channel_model.fading_coherence",
            "would change nothing: this run draws no small-scale fading (the abstract \
             tier, a sidelink run without radio.channel_model.fading, or fading 'none')"
                .to_string(),
        ));
    }
    let buildings_charged = !matches!(law, PropagationChoice::Tr37885);
    for (leaf, set) in [
        ("building_wall_loss_db", c.building_wall_loss_db.is_some()),
        ("building_loss_db_per_m", c.building_loss_db_per_m.is_some()),
    ] {
        if !set {
            continue;
        }
        let why = if abstract_tier {
            Some("the abstract propagation tier composes no obstacle".to_string())
        } else if !s.world.buildings.enabled {
            Some("world.buildings.enabled is false".to_string())
        } else if !buildings_charged {
            Some(format!(
                "{law_name} prices a building-blocked link with its own NLOS law, not the \
                 Sommer term"
            ))
        } else {
            None
        };
        if let Some(why) = why {
            e.push(conflict(
                &format!("radio.channel_model.{leaf}"),
                format!("would change nothing: {why}"),
            ));
        }
    }
    for (path, set) in [
        ("radio.channel_model.foliage", c.foliage.is_some()),
        (
            "radio.channel_model.density_sigma",
            c.density_sigma.is_some(),
        ),
        ("radio.channel_model.reflection", c.reflection.is_some()),
    ] {
        if set && abstract_tier {
            e.push(conflict(
                path,
                "would change nothing: the abstract propagation tier composes no obstacle"
                    .to_string(),
            ));
        }
    }
    if c.foliage == Some(true) && !s.world.vegetation {
        e.push(conflict(
            "radio.channel_model.foliage",
            "would change nothing: world.vegetation is false, so the world has no trees"
                .to_string(),
        ));
    }
    // The ray is power-summed with the building term's shadowed path, so it needs that
    // term charged: buildings on, under a law that leaves them to the Sommer term.
    if c.reflection == Some(true) && !abstract_tier {
        let why = if !s.world.buildings.enabled {
            Some("world.buildings.enabled is false, so there is no wall to reflect off".to_string())
        } else if law.owns_buildings() {
            Some(format!(
                "{law_name} prices a building-blocked link with its own NLOS law, so there \
                 is no shadowed path to add the reflection to"
            ))
        } else {
            None
        };
        if let Some(why) = why {
            e.push(conflict(
                "radio.channel_model.reflection",
                format!("would change nothing: {why}"),
            ));
        }
    }
    if c.vehicle_blockage.is_some() && abstract_tier {
        e.push(conflict(
            "radio.channel_model.vehicle_blockage",
            "would change nothing: the abstract propagation tier composes no obstacle".to_string(),
        ));
    }

    let lte = s.radio.rat == Rat::LteV2xPc5;
    if let Some(p) = sl.candidate_percent {
        if !matches!(p, 20 | 35 | 50) {
            e.push(conflict(
                "radio.sidelink.candidate_percent",
                format!("{p} is not one of 20, 35, 50 (TS 38.331 sl-TxPercentageList)"),
            ));
        } else if lte && p != 20 {
            e.push(conflict(
                "radio.sidelink.candidate_percent",
                format!("{p} % is NR-only: LTE-V2X fixes it at 20 % (TS 36.213 §14.1.1.6 step 7)"),
            ));
        }
    }
    if let Some(ms) = sl.selection_window_ms
        && lte
        && ms < 20
    {
        e.push(conflict(
            "radio.sidelink.selection_window_ms",
            format!("{ms} ms is under LTE's 20 ms minimum T2 (TS 36.213 §14.1.1.6)"),
        ));
    }
}

/// The sensor suites (`perception`, WP-14): each sensor's id is a CPM `sensorId` bit the
/// object list can name (1 to 7, distinct within a suite), its range fits the CPM's
/// `StandardLength12b` (409.5 m), its opening is a real one, its probabilities are
/// probabilities, and its offsets fit the shape's `CartesianCoordinate` (±327.67 m). A
/// block, a class suite or a roadside suite that nothing would read is refused, as is a
/// unit given the `cpm` role in a scenario that sends no CPM.
fn perception(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let sends_cpm = s.messages.sets.iter().any(|m| m == "cpm");
    let rsu_roles = s
        .actors
        .rsus
        .iter()
        .enumerate()
        .map(|(i, r)| (format!("actors.rsus[{i}].roles"), &r.roles))
        .chain(
            s.actors
                .rsu_placement
                .iter()
                .map(|p| ("actors.rsu_placement.roles".to_string(), &p.roles)),
        );
    let mut cpm_units = false;
    for (field, roles) in rsu_roles {
        if roles.iter().any(|r| r == "cpm") {
            cpm_units = true;
            if !sends_cpm {
                e.push(conflict(
                    &field,
                    "gives a roadside unit the 'cpm' role, and messages.sets does not carry \
                     'cpm', so the unit would send no infrastructure CPM; add 'cpm' to \
                     messages.sets or drop the role"
                        .to_string(),
                ));
            }
        }
    }
    let Some(p) = &s.perception else {
        return;
    };
    if !sends_cpm {
        e.push(conflict(
            "perception",
            "would change nothing: only a station that sends CPMs runs the perception \
             model; add 'cpm' to messages.sets"
                .to_string(),
        ));
    }
    if p.rsu.is_some() && !cpm_units {
        e.push(conflict(
            "perception.rsu",
            "would change nothing: no roadside unit has the 'cpm' role".to_string(),
        ));
    }
    let suite = |field: &str,
                 list: &[crate::scenario::schema::SensorConf],
                 e: &mut Vec<ScenarioError>| {
        if list.is_empty() || list.len() > 7 {
            e.push(conflict(
                field,
                format!(
                    "has {} sensors; a suite has 1 to 7 (a CPM object names its sensors by \
                     a bit per id)",
                    list.len()
                ),
            ));
        }
        let mut ids = BTreeSet::new();
        for (i, c) in list.iter().enumerate() {
            let f = |leaf: &str| format!("{field}[{i}].{leaf}");
            if !(1..=7).contains(&c.id) || !ids.insert(c.id) {
                e.push(conflict(
                    &f("id"),
                    format!("{} is not a distinct sensor id from 1 to 7", c.id),
                ));
            }
            if !(c.range_m > 0.0 && c.range_m <= 409.5) {
                e.push(conflict(
                    &f("range_m"),
                    format!(
                        "{} m is outside (0, 409.5]: the CPM's sensor range is a \
                         StandardLength12b in decimetres",
                        c.range_m
                    ),
                ));
            }
            if !(c.fov_deg > 0.0 && c.fov_deg <= 360.0) {
                e.push(conflict(
                    &f("fov_deg"),
                    format!("{}° is not an opening in (0, 360]", c.fov_deg),
                ));
            }
            if !c.azimuth_deg.is_finite() || c.azimuth_deg.abs() > 360.0 {
                e.push(conflict(
                    &f("azimuth_deg"),
                    format!("{}° is outside [-360, 360]", c.azimuth_deg),
                ));
            }
            for (leaf, v) in [("pd_near", c.pd_near), ("pd_far", c.pd_far)] {
                if !(0.0..=1.0).contains(&v) {
                    e.push(conflict(&f(leaf), format!("{v} is not a probability")));
                }
            }
            for (leaf, v) in [
                ("ahead_m", c.ahead_m),
                ("left_m", c.left_m),
                ("height_m", c.height_m),
            ] {
                if !v.is_finite() || v.abs() > 327.67 {
                    e.push(conflict(
                        &f(leaf),
                        format!("{v} m is outside ±327.67 m, the CPM shape's CartesianCoordinate"),
                    ));
                }
            }
            for (leaf, v) in [
                ("sigma_base_m", c.sigma_base_m),
                ("sigma_range_fraction", c.sigma_range_fraction),
            ] {
                if let Some(v) = v
                    && !(v.is_finite() && v >= 0.0)
                {
                    e.push(conflict(
                        &f(leaf),
                        format!("{v} is not a non-negative error"),
                    ));
                }
            }
        }
    };
    if let Some(v) = &p.vehicle {
        suite("perception.vehicle", v, e);
    }
    if let Some(v) = &p.rsu {
        suite("perception.rsu", v, e);
    }
    for (class, v) in &p.classes {
        let field = format!("perception.classes.{class}");
        let known = v2xw_mobility::VehicleClass::ALL
            .iter()
            .find(|c| c.as_str() == class);
        match known {
            None => e.push(conflict(
                &field,
                format!(
                    "'{class}' is not a vehicle class; one of {}",
                    v2xw_mobility::VehicleClass::ALL
                        .iter()
                        .filter(|c| !c.is_vru())
                        .map(|c| c.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
            Some(c) if c.is_vru() => e.push(conflict(
                &field,
                format!("would change nothing: a {class} sends no CPM"),
            )),
            Some(_) => suite(&field, v, e),
        }
    }
}

fn conflict(field: &str, conflict: String) -> ScenarioError {
    ScenarioError::conflict(field, conflict)
}

/// `traffic.signals.control: nema` and the `traffic.signals.nema` block (WP-17).
///
/// The block is checked in full so that a scenario written now still loads when the
/// controller lands. The phase layout follows the STM 2nd ed. numbering (§5.1.1, Exhibit
/// 5-1): ring 1 holds phases from 1-4 and ring 2 from 5-8, each in ascending order (the
/// lead-lead order of Exhibit 5-3); lead or lag is `nema.lefts`, never a ring reordering,
/// so one scenario cannot state the sequence twice and disagree with itself.
pub(crate) fn nema_checks(g: &super::schema::SignalControl, e: &mut Vec<ScenarioError>) {
    use super::schema::{
        ForceOffMode, LeftTurnMode, NemaControllerType, NemaLeftSequence, NemaSettings,
        PermissiveDisplay, SignalControlKind,
    };
    if g.control == SignalControlKind::Nema {
        let coordinated = g.nema.as_ref().is_some_and(|n| n.coordination);
        // The dual ring rewrites each plan as it runs (`crate::nema::control`). A green
        // wave's offsets reach it only through coordination, which keeps the cycle on
        // them; free-running rings would ignore them. The short-link rewrite of the
        // plans' greens has no counterpart in the rings.
        if g.green_wave_speed_mps.is_some() && !coordinated {
            e.push(conflict(
                "traffic.signals.green_wave_speed_mps",
                "sets the plans' offsets, and the NEMA dual ring runs free unless \
                 traffic.signals.nema.coordination is true; turn coordination on or remove \
                 the wave"
                    .to_string(),
            ));
        }
        if g.short_link_clearance_m.is_some() {
            e.push(conflict(
                "traffic.signals.short_link_clearance_m",
                "rewrites the fixed plans' greens at load, and the NEMA dual ring \
                 (control: nema) times its own; remove this or choose fixed-time or \
                 actuated"
                    .to_string(),
            ));
        }
    }
    let Some(n) = &g.nema else {
        return;
    };
    if g.control != SignalControlKind::Nema {
        e.push(conflict(
            "traffic.signals.nema",
            "configures the NEMA dual-ring controller, and traffic.signals.control is not \
             nema; set control to nema or remove the block"
                .to_string(),
        ));
    }
    for (path, v) in [
        ("traffic.signals.nema.left_min_green_s", n.left_min_green_s),
        ("traffic.signals.nema.left_max_green_s", n.left_max_green_s),
        ("traffic.signals.nema.left_passage_s", n.left_passage_s),
    ] {
        bounded_at(path, path, v, e);
    }
    for (path, v) in [
        ("traffic.signals.nema.yellow_s", n.yellow_s),
        ("traffic.signals.nema.red_clear_s", n.red_clear_s),
        ("traffic.signals.nema.walk_s", n.walk_s),
        ("traffic.signals.nema.ped_speed_mps", n.ped_speed_mps),
    ] {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
        }
    }
    if n.left_max_green_s < n.left_min_green_s {
        e.push(conflict(
            "traffic.signals.nema.left_max_green_s",
            format!(
                "is {} s, below the left-turn minimum green of {} s",
                n.left_max_green_s, n.left_min_green_s
            ),
        ));
    }
    // The rings: STM numbers, ascending, no phase twice.
    let mut seen = [false; 9];
    for (path, ring, range) in [
        ("traffic.signals.nema.ring1", &n.ring1, 1u8..=4),
        ("traffic.signals.nema.ring2", &n.ring2, 5u8..=8),
    ] {
        if ring.is_empty() {
            e.push(conflict(
                path,
                "is empty; a ring needs at least one phase".to_string(),
            ));
        }
        for &p in ring {
            if !range.contains(&p) {
                e.push(conflict(
                    path,
                    format!(
                        "names phase {p}; this ring holds NEMA phases {} to {} (FHWA Signal \
                         Timing Manual 2nd ed., Exhibit 5-3)",
                        range.start(),
                        range.end()
                    ),
                ));
            } else if seen[usize::from(p)] {
                e.push(conflict(path, format!("names phase {p} twice")));
            } else {
                seen[usize::from(p)] = true;
            }
        }
        if ring.windows(2).any(|w| w[0] >= w[1]) {
            e.push(conflict(
                path,
                format!(
                    "is {ring:?}; write a ring's phases in ascending order and set lead or \
                     lag with traffic.signals.nema.lefts"
                ),
            ));
        }
    }
    let present = |p: u8| usize::from(p) < seen.len() && seen[usize::from(p)];
    // The barriers: one phase of each ring, both in the barrier's group.
    for (path, pair, group) in [
        (
            "traffic.signals.nema.barrier_phases",
            &n.barrier_phases,
            [3u8, 4, 7, 8],
        ),
        (
            "traffic.signals.nema.barrier2_phases",
            &n.barrier2_phases,
            [1u8, 2, 5, 6],
        ),
    ] {
        let ok = pair.len() == 2
            && pair.iter().all(|&p| group.contains(&p) && present(p))
            && (1..=4).contains(&pair[0])
            && (5..=8).contains(&pair[1]);
        if !ok {
            e.push(conflict(
                path,
                format!(
                    "is {pair:?}; a barrier is a ring-1 phase then a ring-2 phase, both in the \
                     rings and both from {group:?}"
                ),
            ));
        }
    }
    // Recalls name phases that exist; a pedestrian phase goes with a through (STM 2nd ed.
    // §5.1.1: pedestrian phases take the parallel through's number).
    for (path, list) in [
        ("traffic.signals.nema.min_recall", &n.min_recall),
        ("traffic.signals.nema.soft_recall", &n.soft_recall),
        ("traffic.signals.nema.max_recall", &n.max_recall),
        ("traffic.signals.nema.ped_recall", &n.ped_recall),
        (
            "traffic.signals.nema.coordinate_phases",
            &n.coordinate_phases,
        ),
    ] {
        for &p in list {
            if !present(p) {
                e.push(conflict(
                    path,
                    format!("names phase {p}, which neither ring holds"),
                ));
            }
        }
    }
    for &p in &n.ped_recall {
        if p % 2 == 1 {
            e.push(conflict(
                "traffic.signals.nema.ped_recall",
                format!(
                    "names phase {p}, a left turn; a pedestrian phase takes its parallel \
                     through's number (2, 4, 6 or 8)"
                ),
            ));
        }
    }
    let c = &n.coordinate_phases;
    let concurrent = match c.as_slice() {
        [a] => (1..=8).contains(a),
        [a, b] => (1..=4).contains(a) && (5..=8).contains(b) && ((*a <= 2) == (*b <= 6)),
        _ => false,
    };
    if !concurrent {
        e.push(conflict(
            "traffic.signals.nema.coordinate_phases",
            format!(
                "is {c:?}; coordinate one phase, or a ring-1 and a ring-2 phase on the same \
                 side of the barrier"
            ),
        ));
    }
    // Settings that only coordination or only a left phase reads would do nothing.
    let d = NemaSettings::default();
    if !n.coordination {
        for (path, moved) in [
            (
                "traffic.signals.nema.force_off",
                n.force_off != ForceOffMode::default(),
            ),
            (
                "traffic.signals.nema.controller_type",
                n.controller_type != NemaControllerType::default(),
            ),
            (
                "traffic.signals.nema.coordinate_phases",
                n.coordinate_phases != d.coordinate_phases,
            ),
        ] {
            if moved {
                e.push(conflict(
                    path,
                    "acts only under coordination, and traffic.signals.nema.coordination is \
                     false; set it true or remove this"
                        .to_string(),
                ));
            }
        }
    }
    if n.left_turn_mode == LeftTurnMode::Permissive {
        for (path, moved) in [
            (
                "traffic.signals.nema.lefts",
                n.lefts != NemaLeftSequence::default(),
            ),
            (
                "traffic.signals.nema.left_min_green_s",
                n.left_min_green_s != d.left_min_green_s,
            ),
            (
                "traffic.signals.nema.left_max_green_s",
                n.left_max_green_s != d.left_max_green_s,
            ),
            (
                "traffic.signals.nema.left_passage_s",
                n.left_passage_s != d.left_passage_s,
            ),
        ] {
            if moved {
                e.push(conflict(
                    path,
                    "times a protected left-turn phase, and traffic.signals.nema.left_turn_mode \
                     is permissive, which has none; remove this or choose a protected mode"
                        .to_string(),
                ));
            }
        }
    }
    if n.left_turn_mode == LeftTurnMode::Protected
        && n.permissive_display != PermissiveDisplay::default()
    {
        e.push(conflict(
            "traffic.signals.nema.permissive_display",
            "is the face of a permissive left, and traffic.signals.nema.left_turn_mode is \
             protected, which has none; remove this or choose a permissive mode"
                .to_string(),
        ));
    }
}

/// `value` satisfies the bound [`BOUNDS`] declares for `path`, or an error saying so.
///
/// `path` is the *canonical* path — `threats.attackers[].fraction` — and `field` is the
/// concrete one the author wrote — `threats.attackers[2].fraction` — so the table has one
/// row per rule and the message names the author's own field.
// traffic audit
/// The ranges and the do-nothing combinations of the traffic-audit fields
/// (docs/guides/config-inventory-traffic.md).
fn traffic_audit_checks(s: &Scenario, e: &mut Vec<ScenarioError>) {
    use crate::scenario::schema::{ClassDriver, DriverRealism};
    let dv = &s.actors.vehicles.drivers;
    let trait_fields = [
        ("actors.vehicles.drivers.speed_factor", dv.speed_factor),
        ("actors.vehicles.drivers.speed_spread", dv.speed_spread),
        (
            "actors.vehicles.drivers.red_runner_share",
            dv.red_runner_share,
        ),
        (
            "actors.vehicles.drivers.start_reaction_s",
            dv.start_reaction_s,
        ),
        (
            "actors.vehicles.drivers.queue_start_delay_s",
            dv.queue_start_delay_s,
        ),
    ];
    for (path, v) in trait_fields {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
            if dv.realism == DriverRealism::Identical {
                e.push(conflict(
                    path,
                    "is set, and actors.vehicles.drivers.realism is identical, whose drivers \
                     all drive the limit and react at once; choose another realism or unset it"
                        .to_string(),
                ));
            }
        }
    }
    // A class's numbers do nothing when the fleet has none of that class.
    let classes = &s.actors.vehicles.classes;
    // With no `classes`, the Poisson models draw from their `fleet` preset, whose default
    // is cars only; the other demand kinds (GTFS buses, OD tables, composites) may bring
    // any class, so they are given the benefit of the doubt.
    let demand = &s.actors.vehicles.demand;
    let poisson = matches!(
        demand.kind.as_str(),
        "mobility/demand/poisson" | v2xw_mobility::demand::poisson::MODEL_ID
    );
    let fleet = demand
        .params
        .get("fleet")
        .and_then(|v| v.as_str())
        .unwrap_or("cars-only");
    let preset_has = |names: &[&str]| {
        !poisson
            || match fleet {
                "cars-only" => false,
                "kesting2007" => names.contains(&"truck"),
                _ => true,
            }
    };
    let has = |names: &[&str]| {
        if classes.is_empty() {
            preset_has(names)
        } else {
            names.iter().any(|n| classes.contains_key(*n))
        }
    };
    let per_class: [(&str, &Option<ClassDriver>, bool); 4] = [
        ("truck", &dv.truck, has(&["truck", "trailer"])),
        ("bus", &dv.bus, has(&["bus", "coach"])),
        ("motorcycle", &dv.motorcycle, has(&["motorcycle", "moped"])),
        ("bicycle", &dv.bicycle, s.actors.vru.cyclists > 0),
    ];
    for (name, spec, present) in per_class {
        let Some(cd) = spec else { continue };
        for (leaf, v) in [
            ("max_accel_mps2", cd.max_accel_mps2),
            ("comfort_decel_mps2", cd.comfort_decel_mps2),
            ("time_headway_s", cd.time_headway_s),
            ("min_gap_m", cd.min_gap_m),
        ] {
            if let Some(v) = v {
                let path = format!("actors.vehicles.drivers.{name}.{leaf}");
                bounded_at(&path, &path, v, e);
            }
        }
        if !present {
            let why = if name == "bicycle" {
                "sets cyclists' driving, and actors.vru.cyclists is 0".to_string()
            } else {
                format!(
                    "sets {name} driving, and the fleet has no {name}; add a {name} share to \
                     actors.vehicles.classes (for example {name}: {{fraction: 0.1}}) or unset it"
                )
            };
            e.push(conflict(&format!("actors.vehicles.drivers.{name}"), why));
        }
    }
    if let Some(t) = &s.traffic {
        for (path, v) in [
            ("traffic.critical_gap_factor", t.critical_gap_factor),
            ("traffic.permitted_left_gap_s", t.permitted_left_gap_s),
            ("traffic.critical_gap_cv", t.critical_gap_cv),
            ("traffic.time_to_impatience_s", t.time_to_impatience_s),
            ("traffic.impatience_gap_floor", t.impatience_gap_floor),
            (
                "traffic.permissive_sneakers",
                t.permissive_sneakers.map(f64::from),
            ),
            ("traffic.turn_lateral_accel_mps2", t.turn_lateral_accel_mps2),
            ("traffic.turn_curve_window_m", t.turn_curve_window_m),
            ("traffic.lc_strategic", t.lc_strategic),
            ("traffic.lc_cooperative", t.lc_cooperative),
            ("traffic.lc_speed_gain", t.lc_speed_gain),
            ("traffic.lc_keep_right", t.lc_keep_right),
        ] {
            if let Some(v) = v {
                bounded_at(path, path, v, e);
            }
        }
        if let Some(y) = &t.emergency_yield {
            for (path, v) in [
                (
                    "traffic.emergency_yield.reaction_distance_m",
                    y.reaction_distance_m,
                ),
                (
                    "traffic.emergency_yield.red_pass_speed_mps",
                    y.red_pass_speed_mps,
                ),
            ] {
                let Some(v) = v else { continue };
                bounded_at(path, path, v, e);
                if y.enabled != Some(true) {
                    e.push(conflict(
                        path,
                        "is set, and traffic.emergency_yield.enabled is not true".to_string(),
                    ));
                }
            }
        }
        if let Some(k) = &t.kerbside {
            for (path, v) in [
                ("traffic.kerbside.bus_stop_spacing_m", k.bus_stop_spacing_m),
                ("traffic.kerbside.bus_dwell_mean_s", k.bus_dwell_mean_s),
                ("traffic.kerbside.taxi_share", k.taxi_share),
                (
                    "traffic.kerbside.taxi_dwell_median_s",
                    k.taxi_dwell_median_s,
                ),
                (
                    "traffic.kerbside.delivery_stop_share",
                    k.delivery_stop_share,
                ),
                (
                    "traffic.kerbside.delivery_dwell_median_s",
                    k.delivery_dwell_median_s,
                ),
            ] {
                if let Some(v) = v {
                    bounded_at(path, path, v, e);
                    if k.enabled == Some(false) {
                        e.push(conflict(
                            path,
                            "is set, and traffic.kerbside.enabled is false".to_string(),
                        ));
                    }
                }
            }
            if k.taxi_share == Some(0.0) && k.taxi_dwell_median_s.is_some() {
                e.push(conflict(
                    "traffic.kerbside.taxi_dwell_median_s",
                    "is set, and traffic.kerbside.taxi_share is 0: no taxi stops".to_string(),
                ));
            }
            if k.delivery_stop_share == Some(0.0) && k.delivery_dwell_median_s.is_some() {
                e.push(conflict(
                    "traffic.kerbside.delivery_dwell_median_s",
                    "is set, and traffic.kerbside.delivery_stop_share is 0: no delivery stops"
                        .to_string(),
                ));
            }
        }
        let native_only = t.critical_gap_factor.is_some()
            || t.permitted_left_gap_s.is_some()
            || t.turn_lateral_accel_mps2.is_some()
            || t.turn_curve_window_m.is_some()
            || t.lc_strategic.is_some()
            || t.lc_cooperative.is_some()
            || t.lc_speed_gain.is_some()
            || t.lc_keep_right.is_some()
            || t.heavy_vehicle_gap_terms.is_some()
            || t.critical_gap_cv.is_some()
            || t.time_to_impatience_s.is_some()
            || t.impatience_gap_floor.is_some()
            || t.impatience_foe_braking.is_some()
            || t.permissive_sneakers.is_some()
            || t.standing_leader_stop.is_some()
            || t.depart_speed_max.is_some()
            || t.lane_bend_speed.is_some()
            || t.depart_at_speed.is_some()
            || t.reroute_share.is_some()
            || t.reroute_source.is_some()
            || t.vehicle_dynamics.is_some()
            || t.emergency_yield.is_some()
            || t.kerbside.is_some();
        // The LC2013 factors act only on lane changes, and the strategic one only on the
        // changes a route needs.
        for (path, v) in [
            ("traffic.lc_strategic", t.lc_strategic),
            ("traffic.lc_cooperative", t.lc_cooperative),
            ("traffic.lc_speed_gain", t.lc_speed_gain),
            ("traffic.lc_keep_right", t.lc_keep_right),
        ] {
            if v.is_some() && !t.lane_changes {
                e.push(conflict(
                    path,
                    "tunes lane changing, and traffic.lane_changes is false".to_string(),
                ));
            }
        }
        if t.lc_strategic.is_some() && t.lane_changes && !t.strategic_lane_choice {
            e.push(conflict(
                "traffic.lc_strategic",
                "sets when route lane changes are made, and traffic.strategic_lane_choice is \
                 false"
                    .to_string(),
            ));
        }
        if t.lc_cooperative.is_some_and(|c| c > 0.0) && t.lane_changes && !t.strategic_lane_choice {
            e.push(conflict(
                "traffic.lc_cooperative",
                "makes room for route lane changes, and traffic.strategic_lane_choice is false"
                    .to_string(),
            ));
        }
        if t.turn_curve_window_m.is_some_and(|w| w > 0.0) && t.turn_lateral_accel_mps2 == Some(0.0)
        {
            e.push(conflict(
                "traffic.turn_curve_window_m",
                "sets how a corner's radius is judged, and traffic.turn_lateral_accel_mps2 is \
                 0, which turns the turn-speed limit off"
                    .to_string(),
            ));
        }
        if native_only && s.mobility.as_ref().is_some_and(|m| m.provider == "sumo") {
            e.push(conflict(
                "traffic",
                "sets the native engine's junction or kerbside behaviour, and \
                 mobility.provider is sumo"
                    .to_string(),
            ));
        }
    }
    if let Some(t) = &s.actors.vehicles.bsm_timing {
        if let Some(v) = t.locked_share {
            bounded_at(
                "actors.vehicles.bsm_timing.locked_share",
                "actors.vehicles.bsm_timing.locked_share",
                v,
                e,
            );
        }
        if let Some(ch) = &t.stamp_chain {
            for (path, v) in [
                (
                    "actors.vehicles.bsm_timing.stamp_chain.stay_minus1",
                    ch.stay_minus1,
                ),
                (
                    "actors.vehicles.bsm_timing.stamp_chain.stay_zero",
                    ch.stay_zero,
                ),
                (
                    "actors.vehicles.bsm_timing.stamp_chain.speed_per_m",
                    ch.speed_per_m,
                ),
                (
                    "actors.vehicles.bsm_timing.stamp_chain.speed_tau_s",
                    ch.speed_tau_s,
                ),
                (
                    "actors.vehicles.bsm_timing.stamp_chain.position_follows",
                    ch.position_follows,
                ),
            ] {
                if let Some(v) = v {
                    bounded_at(path, path, v, e);
                }
            }
            if let Some(p) = &ch.profile {
                one_of(
                    "actors.vehicles.bsm_timing.stamp_chain.profile",
                    p,
                    &v2xw_node::bsm_timing::StampChain::PROFILES,
                    e,
                );
            }
        }
        if t.is_on()
            && let Err((path, why)) = t.calibration()
        {
            e.push(conflict(path, why));
        }
    }
    if let Some(g) = &s.actors.vehicles.gnss {
        for (path, v) in [
            ("actors.vehicles.gnss.outlier_rate_hz", g.outlier_rate_hz),
            (
                "actors.vehicles.gnss.outlier_magnitude_m",
                g.outlier_magnitude_m,
            ),
            ("actors.vehicles.gnss.burst_rate_hz", g.burst_rate_hz),
            (
                "actors.vehicles.gnss.outage_probability",
                g.outage_probability,
            ),
            ("actors.vehicles.gnss.smoothing_tau_s", g.smoothing_tau_s),
            ("actors.vehicles.gnss.static_hold_mps", g.static_hold_mps),
            (
                "actors.vehicles.gnss.velocity_noise_mps",
                g.velocity_noise_mps,
            ),
            ("actors.vehicles.gnss.stated_sigma_m", g.stated_sigma_m),
            (
                "actors.vehicles.gnss.stated_sigma_log_sd",
                g.stated_sigma_log_sd,
            ),
        ] {
            if let Some(v) = v {
                bounded_at(path, path, v, e);
            }
        }
        if g.static_hold_mps.is_some_and(|v| v > 0.0) && !g.smoothing_tau_s.is_some_and(|v| v > 0.0)
        {
            e.push(conflict(
                "actors.vehicles.gnss.static_hold_mps",
                "is set, and actors.vehicles.gnss.smoothing_tau_s is not: the hold is a \
                 smoothing receiver's, and raw fixes ignore it"
                    .to_string(),
            ));
        }
        if g.outlier_rate_hz == Some(0.0) && g.outlier_magnitude_m.is_some() {
            e.push(conflict(
                "actors.vehicles.gnss.outlier_magnitude_m",
                "is set, and actors.vehicles.gnss.outlier_rate_hz is 0: no outliers".to_string(),
            ));
        }
        if s.actors.vehicles.equipped_fraction == 0.0
            && (s.actors.vru.device_fraction == 0.0
                || s.actors.vru.pedestrians + s.actors.vru.cyclists == 0)
        {
            e.push(conflict(
                "actors.vehicles.gnss",
                "is set, and nothing carries a radio (actors.vehicles.equipped_fraction is 0), \
                 so no position is ever reported"
                    .to_string(),
            ));
        }
    }
    let ws = &s.world.signals;
    let osm = matches!(s.world.source, v2xw_world::WorldSourceSpec::OsmXml { .. });
    for (path, v) in [
        ("world.signals.cycle_s", ws.cycle_s),
        ("world.signals.all_red_s", ws.all_red_s),
        ("world.signals.pedestrian_walk_s", ws.pedestrian_walk_s),
        (
            "world.signals.pedestrian_speed_mps",
            ws.pedestrian_speed_mps,
        ),
        ("world.signals.join_m", ws.join_m),
    ] {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
            if !osm {
                e.push(conflict(
                    path,
                    "times the plans synthesised for an imported (osm-xml) world; this world \
                     brings its own plans (a procedural grid takes params.cycle_s)"
                        .to_string(),
                ));
            }
        }
    }
}

fn bounded_at(path: &str, field: &str, value: f64, e: &mut Vec<ScenarioError>) {
    let Some(b) = bound_of(path) else {
        // A field checked through this helper with no row in the table is a defect in
        // this module: the page would offer an unbounded control for a bounded field.
        // It is reported rather than skipped, because a check that cannot fail is the
        // defect class this project has already found four times.
        e.push(conflict(
            field,
            format!(
                "cannot be range-checked: `{path}` has no row in `validate::BOUNDS`, which is a defect in the engine rather than in this scenario"
            ),
        ));
        return;
    };
    if !b.admits(value) {
        e.push(conflict(
            field,
            format!(
                "{} is {value}, which is outside the allowed range {}",
                b.what,
                b.describe()
            ),
        ));
    }
}

/// `scale`: the model is one this build has, the tiered model has a region, and every
/// number is in range (`crate::scale`).
fn scale(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let Some(scale) = s.scale.as_ref() else {
        return;
    };
    let t = &scale.traffic;
    one_of(
        "scale.traffic.model",
        &t.model,
        &crate::scale::TRAFFIC_MODELS,
        e,
    );
    if t.model == v2xw_mobility::meso::tiered::MODEL_ID && t.region.is_none() {
        e.push(conflict(
            "scale.traffic.region",
            "the tiered model needs the box it drives microscopically".to_string(),
        ));
    }
    // The queue tier reads the drivers' speed factor and spread (each vehicle's free
    // speed) but not their reactions: a queue discharges at its headway law
    // (`scale.traffic.meso.tau_jf_s`), so a reaction set here would do nothing.
    if t.model == v2xw_mobility::meso::MODEL_ID {
        let d = &s.actors.vehicles.drivers;
        for (leaf, set) in [
            ("start_reaction_s", d.start_reaction_s.is_some()),
            ("queue_start_delay_s", d.queue_start_delay_s.is_some()),
        ] {
            if set {
                e.push(conflict(
                    &format!("actors.vehicles.drivers.{leaf}"),
                    "the queue tier (scale.traffic.model: mobility/meso/queue) discharges a queue \
                     at its headway law, not at drivers' reactions; set \
                     scale.traffic.meso.tau_jf_s instead, or use the tiered model"
                        .to_string(),
                ));
            }
        }
    }
    let m = &t.meso;
    for (leaf, value) in [
        ("segment_length_m", Some(m.segment_length_m)),
        ("tau_ff_s", Some(m.tau_ff_s)),
        ("tau_fj_s", Some(m.tau_fj_s)),
        ("tau_jf_s", Some(m.tau_jf_s)),
        ("tau_jj_s", Some(m.tau_jj_s)),
        ("jam_threshold", m.jam_threshold),
    ] {
        if let Some(v) = value {
            let path = format!("scale.traffic.meso.{leaf}");
            bounded_at(&path, &path, v, e);
        }
    }
    if let Some(margin) = scale.radio.margin_m {
        bounded_at("scale.radio.margin_m", "scale.radio.margin_m", margin, e);
    }
}

/// One of a fixed set, or an error listing the set.
fn one_of(field: &str, value: &str, allowed: &[&str], e: &mut Vec<ScenarioError>) {
    if !allowed.contains(&value) {
        e.push(conflict(
            field,
            format!(
                "'{value}' is not one this build implements; allowed: {}",
                allowed.join(", ")
            ),
        ));
    }
}

fn time(s: &Scenario, e: &mut Vec<ScenarioError>) {
    if WallClock::parse_rfc3339(&s.time.t0).is_err() {
        e.push(conflict(
            "time.t0",
            format!(
                "'{}' is not an RFC 3339 instant of the form 2027-03-04T07:00:00Z; it is what \
                 every 1609.2 generationTime is stamped from, so it cannot be guessed",
                s.time.t0
            ),
        ));
    }
    bounded_at("time.duration_s", "time.duration_s", s.time.duration_s, e);
    if let Some(p) = s.time.metric_period_s {
        bounded_at("time.metric_period_s", "time.metric_period_s", p, e);
    }
    if let Some(w) = s.time.warmup_s {
        bounded_at("time.warmup_s", "time.warmup_s", w, e);
        if w >= s.time.duration_s {
            e.push(conflict(
                "time.warmup_s",
                format!(
                    "is {w} s, which leaves nothing of the {} s run to measure; make it \
                     shorter than time.duration_s",
                    s.time.duration_s
                ),
            ));
        }
    }
    // ADR 0004 decision 2's window, read from `BOUNDS` so the page's slider and this
    // check cannot disagree about it.
    if let Some(b) = bound_of("time.mobility_step_ms")
        && !b.admits(s.time.mobility_step_ms as f64)
    {
        e.push(conflict(
            "time.mobility_step_ms",
            format!(
                "is {}, and ADR 0004 decision 2 allows {} ms: below the floor no mobility \
                 provider is calibrated for the step, and above the ceiling the published \
                 constant-velocity extrapolation between steps stops being accurate enough \
                 for frame-level radio",
                s.time.mobility_step_ms,
                b.describe()
            ),
        ));
    }
    one_of(
        "time.des_resolution",
        &s.time.des_resolution,
        &RESOLUTIONS,
        e,
    );
    // `des_resolution` is a guarantee the run makes, not a knob on the clock: the kernel keeps
    // nanoseconds whatever it says. A resolution is therefore accepted only when everything
    // the run times is representable at it. The binding case is the radio: at the medium and
    // high PHY/MAC tiers a frame is timed in microseconds — an 802.11p OFDM symbol is 8 µs and
    // SIFS 32 µs in a 10 MHz channel (IEEE 802.11-2020 §17.4, Table 17-21) — and a millisecond
    // grid cannot hold either.
    if s.time.des_resolution == "1ms"
        && (s.radio.tiers.phy != Tier::Abstract || s.radio.tiers.mac != Tier::Abstract)
    {
        e.push(conflict(
            "time.des_resolution",
            "is '1ms', and the PHY/MAC tiers time frames in microseconds (an 802.11p OFDM \
             symbol is 8 µs, SIFS 32 µs; IEEE 802.11-2020 §17.4): a run at this tier cannot \
             promise millisecond resolution. Use '1us', or the abstract PHY and MAC"
                .to_string(),
        ));
    }
    let grid_s = match s.time.des_resolution.as_str() {
        "1us" => Some(1e-6),
        "1ms" => Some(1e-3),
        _ => None,
    };
    if let Some(grid) = grid_s {
        for (i, w) in s.time.time_dilation.iter().enumerate() {
            for (name, at) in [("from_s", w.from_s), ("to_s", w.to_s)] {
                let ticks = at / grid;
                if (ticks - ticks.round()).abs() > 1e-6 {
                    e.push(conflict(
                        &format!("time.time_dilation[{i}].{name}"),
                        format!(
                            "is {at} s, which is not on the {} grid time.des_resolution \
                             promises",
                            s.time.des_resolution
                        ),
                    ));
                }
            }
        }
    }

    let horizon = s.time.duration_s;
    let mut windows: Vec<(f64, f64)> = Vec::new();
    for (i, w) in s.time.time_dilation.iter().enumerate() {
        let field = format!("time.time_dilation[{i}]");
        if w.to_s <= w.from_s {
            e.push(conflict(
                &field,
                format!(
                    "to_s is {} but from_s is {}; a window ends after it starts",
                    w.to_s, w.from_s
                ),
            ));
            continue;
        }
        if w.from_s < 0.0 || w.to_s > horizon {
            e.push(conflict(
                &field,
                format!(
                    "spans [{}, {}] s but the run is [0, {}] s (time.duration_s); a window \
                     outside the run silently disables nothing",
                    w.from_s, w.to_s, horizon
                ),
            ));
        }
        for (j, prev) in windows.iter().enumerate() {
            if w.from_s < prev.1 && prev.0 < w.to_s {
                e.push(conflict(
                    &field,
                    format!(
                        "overlaps time.time_dilation[{j}] ([{}, {}] s): the manifest records \
                         windows as disjoint intervals and metrics are marked not-observed \
                         per window, so an overlap has no single answer",
                        prev.0, prev.1
                    ),
                ));
            }
        }
        windows.push((w.from_s, w.to_s));
    }
}

/// The US i-period, seconds: one week [CAMP-EE §2.1.5.3.2; SMP §2.1.17 Table 2].
const REAL_I_PERIOD_S: f64 = 604_800.0;

/// The calendar lint (WP-D2, gap row 43): a credential system on anything but the
/// deployment's weekly grid must say so with `time.calendar: compressed`, so no output of
/// a compressed run can be read as the deployment's; and a run that says compressed must
/// be.
fn calendar(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let i_period = crate::phase2::lifecycle_param(s, "i_period_s");
    let compressed = i_period.is_some_and(|v| (v - REAL_I_PERIOD_S).abs() > 1e-9);
    // The field the author wrote: the typed one wins over the protocol param.
    let typed = s
        .security
        .scms
        .as_ref()
        .and_then(|x| x.lifecycle.as_ref())
        .is_some_and(|l| l.i_period_s.is_some());
    let field = if typed {
        "security.scms.lifecycle.i_period_s"
    } else {
        "security.protocol.params.i_period_s"
    };
    match (s.time.calendar, compressed) {
        (crate::scenario::Calendar::Real, true) => e.push(conflict(
            field,
            format!(
                "is {} s, and the deployment's i-period is one week (604800 s, CAMP-EE \
                 §2.1.5.3.2): a shorter grid is a compressed calendar whose i-period \
                 numbers, validity windows and calendar waits are not the deployment's. \
                 Declare it with time.calendar: compressed, or remove i_period_s and use \
                 time.fast_forward to make the weeks pass",
                i_period.unwrap_or_default()
            ),
        )),
        (crate::scenario::Calendar::Compressed, false) => e.push(conflict(
            "time.calendar",
            "is compressed, and the run's i-period is the deployment's week (no shorter \
             security.protocol.params.i_period_s): remove the flag, or state the \
             compressed i_period_s"
                .to_string(),
        )),
        _ => {}
    }
}

/// `time.fast_forward`: each window inside the run, ordered, disjoint, on the promised
/// grid, and with a credential backend to advance.
fn fast_forward(s: &Scenario, e: &mut Vec<ScenarioError>) {
    if s.time.fast_forward.is_empty() {
        return;
    }
    let backend = s.actors.backend.protocol.is_some() || s.security.protocol.is_some();
    if !backend {
        e.push(conflict(
            "time.fast_forward",
            "has windows, and the run has no credential backend (security.protocol or \
             actors.backend.protocol): a fast-forward window freezes the traffic and the \
             radio and advances only the backend, so here it would skip everything"
                .to_string(),
        ));
    }
    let grid = match s.time.des_resolution.as_str() {
        "1us" => Some(1e-6),
        "1ms" => Some(1e-3),
        _ => None,
    };
    let horizon = s.time.duration_s;
    let mut windows: Vec<(f64, f64)> = Vec::new();
    for (i, w) in s.time.fast_forward.iter().enumerate() {
        let field = format!("time.fast_forward[{i}]");
        if let Some(g) = grid {
            for (name, at) in [("from_s", w.from_s), ("to_s", w.to_s)] {
                let ticks = at / g;
                if (ticks - ticks.round()).abs() > 1e-6 {
                    e.push(conflict(
                        &format!("{field}.{name}"),
                        format!(
                            "is {at} s, which is not on the {} grid time.des_resolution \
                             promises",
                            s.time.des_resolution
                        ),
                    ));
                }
            }
        }
        if !(w.from_s.is_finite() && w.to_s.is_finite()) || w.to_s <= w.from_s {
            e.push(conflict(
                &field,
                format!(
                    "to_s is {} but from_s is {}; a window ends after it starts",
                    w.to_s, w.from_s
                ),
            ));
            continue;
        }
        if w.from_s < 0.0 || w.to_s > horizon {
            e.push(conflict(
                &field,
                format!(
                    "spans [{}, {}] s but the run is [0, {}] s (time.duration_s); a window \
                     outside the run fast-forwards nothing",
                    w.from_s, w.to_s, horizon
                ),
            ));
        }
        for (j, prev) in windows.iter().enumerate() {
            if w.from_s < prev.1 && prev.0 < w.to_s {
                e.push(conflict(
                    &field,
                    format!(
                        "overlaps time.fast_forward[{j}] ([{}, {}] s); merge the two",
                        prev.0, prev.1
                    ),
                ));
            }
        }
        windows.push((w.from_s, w.to_s));
    }
}

fn world(s: &Scenario, e: &mut Vec<ScenarioError>) {
    // 02-architecture.md §6.1: nothing in the engine reads a wall clock, so an importer
    // cannot date its own work. A generated world has no import to date.
    let needs_date = !matches!(
        s.world.source,
        v2xw_world::WorldSourceSpec::Procedural { .. }
    );
    if needs_date && s.world.imported_at.trim().is_empty() {
        e.push(conflict(
            "world.imported_at",
            format!(
                "is empty, but world.source is {} — an imported world records the import \
                 date in its provenance and no part of the engine may read a wall clock \
                 (02-architecture.md §6.1), so the scenario supplies it",
                s.world.source.label()
            ),
        ));
    }
    match &s.world.source {
        v2xw_world::WorldSourceSpec::Procedural { generator, .. } => {
            // The generator is a choice, not a label: the engine used to build a grid
            // whatever it named, so choosing the radial city in the settings did nothing.
            let built = match generator.as_str() {
                v2xw_world::procedural::MODEL_ID => grid_params(s).map(|_| ()),
                v2xw_world::procedural::radial::MODEL_ID => radial_params(s).map(|_| ()),
                other => Err(conflict(
                    "world.source.generator",
                    format!(
                        "'{other}' is not a world generator this build ships; use {} or {}",
                        v2xw_world::procedural::MODEL_ID,
                        v2xw_world::procedural::radial::MODEL_ID
                    ),
                )),
            };
            if let Err(why) = built {
                e.push(why);
            }
        }
        v2xw_world::WorldSourceSpec::OsmXml { path, .. } => {
            if path.trim().is_empty() {
                e.push(conflict(
                    "world.source.path",
                    "is empty; name the OpenStreetMap XML extract to import, e.g. \
                     worlds/cache/manhattan.osm.xml"
                        .to_string(),
                ));
            }
            if s.world.highway_preset.is_none() {
                e.push(conflict(
                    "world.highway_preset",
                    "is not set, and an osm-xml world needs one: the importer's fallback \
                     speed limits and lane counts are a statement about a jurisdiction. \
                     Choose sumo-german, urban-us-nyc, urban-us-portland or urban-de"
                        .to_string(),
                ));
            }
        }
        v2xw_world::WorldSourceSpec::SumoNet { path } => {
            if path.trim().is_empty() {
                e.push(conflict(
                    "world.source.path",
                    "is empty; name the SUMO network to import, e.g. worlds/sumo/grid3.net.xml"
                        .to_string(),
                ));
            }
        }
        other => e.push(conflict(
            "world.source",
            format!(
                "is {}, and this build builds worlds only from a procedural grid \
                 (kind: procedural), an OpenStreetMap XML extract (kind: osm-xml) or a \
                 SUMO network (kind: sumo-net); convert the network or use one of those",
                other.label()
            ),
        )),
    }
    if !s.world.sumo.is_default()
        && !matches!(s.world.source, v2xw_world::WorldSourceSpec::SumoNet { .. })
    {
        e.push(conflict(
            "world.sumo.signal_programmes",
            "would change nothing: the world is not imported from a SUMO network, so there \
             are no tlLogic programmes to read"
                .to_string(),
        ));
    }
    if let Some(m) = &s.mobility {
        match m.provider.as_str() {
            "native" => {}
            "sumo" => {
                if !matches!(s.world.source, v2xw_world::WorldSourceSpec::SumoNet { .. }) {
                    e.push(conflict(
                        "mobility.provider",
                        "is sumo, which drives traffic only on the network SUMO runs: set \
                         world.source to { kind: sumo-net, path: <the .net.xml> }"
                            .to_string(),
                    ));
                }
            }
            other => e.push(conflict(
                "mobility.provider",
                format!("is {other}; choose native (this engine) or sumo (SUMO over TraCI)"),
            )),
        }
    }
    if let Some(mpl) = s.world.buildings.metres_per_level {
        bounded_at(
            "world.buildings.metres_per_level",
            "world.buildings.metres_per_level",
            mpl,
            e,
        );
    }
    if let Some(v) = s.world.signals.progression_speed_mps {
        bounded_at(
            "world.signals.progression_speed_mps",
            "world.signals.progression_speed_mps",
            v,
            e,
        );
    }
}

/// The highest arrival rate the loader accepts, vehicles per hour, including a rate a
/// `demand.multiplier` event raises it to. See the bound on
/// `actors.vehicles.demand.rate_veh_per_h`.
pub const MAX_ARRIVAL_RATE_VEH_PER_H: f64 = 100_000_000.0;

/// The most junctions a procedural grid may have.
///
/// A guard, not a model parameter: a lattice's memory grows with its junction count, and a
/// typo in `cols` (1300 for 13) would otherwise take the machine's memory rather than be
/// refused. 10,000 junctions is a 100 × 100 grid, about 27 km × 8 km at Midtown spacing,
/// 23 times the shipped `phase1-grid`'s 442.
pub const MAX_GRID_JUNCTIONS: u64 = 10_000;

/// The procedural grid's parameters as the world builder reads them
/// (`crate::wiring::build_world`), checked by the generator's own rules.
///
/// # Errors
/// A conflict at `world.source.params` (or one of its keys) the page can mark.
/// The radial generator's parameters, as `world.source.params` gives them (`null` is its
/// legacy preset).
pub(crate) fn radial_params(
    s: &Scenario,
) -> core::result::Result<v2xw_world::procedural::radial::RadialParams, ScenarioError> {
    let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &s.world.source else {
        return Err(conflict(
            "world.source",
            "is not a procedural world".to_string(),
        ));
    };
    let radial: v2xw_world::procedural::radial::RadialParams = if params.is_null() {
        v2xw_world::procedural::radial::RadialParams::default()
    } else {
        serde_json::from_value(params.clone()).map_err(|why| {
            conflict(
                "world.source.params",
                format!(
                    "does not fit the radial generator: {why}. Its parameters are listed on \
                     the world/source/procedural-radial model card"
                ),
            )
        })?
    };
    if let Err(v2xw_world::WorldError::InvalidParameter { parameter, problem }) = radial.validate()
    {
        return Err(conflict(
            &format!("world.source.params.{parameter}"),
            problem,
        ));
    }
    Ok(radial)
}

pub(crate) fn grid_params(
    s: &Scenario,
) -> core::result::Result<v2xw_world::procedural::GridParams, ScenarioError> {
    let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &s.world.source else {
        return Err(conflict(
            "world.source",
            "is not a procedural grid".to_string(),
        ));
    };
    let grid: v2xw_world::procedural::GridParams = if params.is_null() {
        v2xw_world::procedural::GridParams::legacy()
    } else {
        serde_json::from_value(params.clone()).map_err(|why| {
            conflict(
                "world.source.params",
                format!(
                    "does not fit the procedural grid generator: {why}. Its parameters are \
                     listed on the world/source/procedural-grid model card"
                ),
            )
        })?
    };
    // The green split is checked against the published bound (the settings window's
    // range), and refused on a grid without signals, where it would do nothing.
    if params.get("ew_green_share").is_some() {
        const SHARE: &str = "world.source.params.ew_green_share";
        let mut out = Vec::new();
        bounded_at(SHARE, SHARE, grid.ew_green_share, &mut out);
        if let Some(why) = out.pop() {
            return Err(why);
        }
        if !grid.signalised {
            return Err(conflict(
                SHARE,
                "splits the green of a signalised grid, but this grid has no signals; set \
                 world.source.params.signalised to true"
                    .to_string(),
            ));
        }
    }
    if let Err(v2xw_world::WorldError::InvalidParameter { parameter, problem }) = grid.validate() {
        return Err(conflict(
            &format!("world.source.params.{parameter}"),
            problem,
        ));
    }
    let junctions = u64::from(grid.cols) * u64::from(grid.rows);
    if junctions > MAX_GRID_JUNCTIONS {
        return Err(conflict(
            "world.source.params.cols",
            format!(
                "{} × {} is {junctions} junctions, over this build's limit of \
                 {MAX_GRID_JUNCTIONS} for one procedural grid; use fewer columns or rows",
                grid.cols, grid.rows
            ),
        ));
    }
    Ok(grid)
}

/// How many infrastructure sites the scenario's world will have, when that is known before
/// the world is built: one per junction on a grid with `rsu_at_junctions`, none on any
/// imported city (OpenStreetMap has no mast inventory). `None` when the grid's parameters
/// are themselves wrong, which is reported on its own.
fn world_site_count(s: &Scenario) -> Option<usize> {
    match &s.world.source {
        v2xw_world::WorldSourceSpec::Procedural { generator, .. }
            if generator == v2xw_world::procedural::radial::MODEL_ID =>
        {
            None
        }
        v2xw_world::WorldSourceSpec::Procedural { .. } => {
            let grid = grid_params(s).ok()?;
            Some(if grid.rsu_at_junctions {
                (u64::from(grid.cols) * u64::from(grid.rows)) as usize
            } else {
                0
            })
        }
        _ => Some(0),
    }
}

/// One warning per roadside-unit hardware profile whose post-quantum signature costs come
/// from its flagged software proxy (`pq_software_proxy` in the profile YAML) rather than
/// a figure for that device. No roadside unit publishes a post-quantum figure, so under a
/// post-quantum or hybrid `security.signature` every RSU verifies (and signs) at a
/// comparable board's benchmark. Not an error — the run is legitimate and the alternative
/// was an RSU that dropped every post-quantum message unverified — but a result that turns
/// on RSU verification load must say so. `v2xw validate` prints these as notes and the run
/// manifest carries them in `warnings`.
pub fn pq_rsu_proxy_warnings(s: &Scenario) -> Vec<String> {
    let pq = v2xw_sec::scheme::signature(&s.security.signature)
        .is_some_and(|x| x.mode != v2xw_sec::scheme::SchemeMode::Classical);
    if !pq {
        return Vec::new();
    }
    let default = || crate::phase2::DEFAULT_RSU_PROFILE.to_string();
    let mut ids = BTreeSet::new();
    for r in &s.actors.rsus {
        ids.insert(r.profile.clone().unwrap_or_else(default));
    }
    if let Some(p) = &s.actors.rsu_placement {
        ids.insert(p.profile.clone().unwrap_or_else(default));
    }
    let (sign_op, verify_op) = v2xw_node::profile::signature_ops(&s.security.signature);
    ids.into_iter()
        .filter_map(|id| {
            let p = v2xw_node::profiles::get(&id)?;
            let ops: Vec<&str> = [sign_op, verify_op]
                .into_iter()
                .filter(|op| p.op_cost_is_pq_proxy(op))
                .collect();
            (!ops.is_empty()).then(|| {
                format!(
                    "roadside profile '{id}' prices {} for security.signature '{}' from a \
                     post-quantum SOFTWARE PROXY ({}): no roadside unit publishes a \
                     post-quantum figure, so these are another board's benchmarks, likely \
                     optimistic for this unit; treat RSU verification load and latency \
                     under this scheme as indicative",
                    ops.join(" and "),
                    s.security.signature,
                    p.pq_software_proxy.join(", ")
                )
            })
        })
        .collect()
}

/// A warning when a verification policy other than `verify-all` runs with local
/// misbehaviour detection.
///
/// Both detector suites judge only messages the receiver verified: a report's evidence is
/// the accused's *signed* BSMs ([SM-MBR] v1.0 §2.3.1.2.1, §2.3.1.3.1), and a message whose
/// signature was never checked is not evidence about anyone (`detect::Legacy12` zeroes
/// it, `us_mbr::UsSmMbr` skips it). Verify-on-demand checks only what a safety application
/// is about to act on (Krishnan & Weimerskirch, SAE Int. J. Passenger Cars 4:536-546,
/// 2011), so detection sees a sliver of the traffic: in the effects study (row
/// `ond-reports`, docs/design/scms-real/testing/effects.md) reports fell from 897 to 79 and
/// no attacker was revoked in 54 h. Not a refusal: that blindness is a weakness worth
/// studying, and the run still says so in its manifest.
pub fn detection_policy_warnings(s: &Scenario) -> Vec<String> {
    let policy = s.security.verification_policy.as_str();
    if policy == "verify-all" || s.detection.local.is_empty() {
        return Vec::new();
    }
    let what = match policy {
        "on-demand" => "verifies only the messages a safety application is about to act on",
        "prioritized" => {
            "sheds the oldest and least relevant messages unverified when the \
                          verifier falls behind"
        }
        _ => "leaves some messages unverified",
    };
    vec![format!(
        "security.verification_policy: '{policy}' {what}, and misbehaviour detection \
         (detection.local) judges only verified messages, because a report's evidence must \
         be the accused's signed messages (SCMS Manager SM-MBR v1.0 §2.3.1.2.1). Detection \
         is then nearly blind: with on-demand, reports fell from 897 to 79 and no attacker \
         was revoked in 54 h (docs/design/scms-real/testing/effects.md, ond-reports). Use \
         verify-all to measure detection, or keep this to study the blindness."
    )]
}

/// The NDSS 2024 certificate cycle (`fragmenter/cert-cycle-partial-hybrid`) against the
/// signature scheme (R-CCSIG). The Partially-Hybrid design signs every message with the
/// classical half of a hybrid and carries the hybrid certificate in the first α SPDUs of
/// each τ-SPDU cycle, so two combinations cannot run as it specifies, and each would send
/// nothing useful while looking like a cheap scheme:
///
/// * a pure post-quantum scheme has no classical half to sign a message with: every
///   message would carry the post-quantum signature (ML-DSA-44's 2,420 B is over the
///   1,400 B WSMP MTU, so nothing is sent at all);
/// * a certificate the cycle cannot carry in τ fragments under the MTU: every SPDU would
///   carry only its digest and no receiver could ever learn a signer.
fn cert_cycle_conflicts(
    s: &Scenario,
    f: &crate::scenario::ModelChoice,
    plan: crate::frag::FragPlan,
    e: &mut Vec<ScenarioError>,
) {
    if let Some(scheme) = v2xw_sec::scheme::signature(&s.security.signature)
        && scheme.mode == v2xw_sec::scheme::SchemeMode::PostQuantum
    {
        e.push(conflict(
            "net.fragmenter",
            format!(
                "is the NDSS 2024 Partially-Hybrid certificate cycle, which signs each message \
                 with the classical half of a hybrid scheme and carries only the hybrid \
                 certificate in fragments; security.signature '{}' is purely post-quantum, so \
                 every message would carry its {}-octet signature. Choose a hybrid \
                 (hybrid-falcon512-ecdsa-p256, hybrid-mldsa44-ecdsa-p256) or \
                 fragmenter/generic-sdu",
                s.security.signature,
                scheme.sig_octets()
            ),
        ));
        return;
    }
    let plan = plan.for_signature(
        &s.security.signature,
        f.params.get("hybrid_cert_bytes").is_some(),
    );
    let Some(cycle) = plan.cert_cycle() else {
        return;
    };
    let mtu = if s.net.layer == "gn-btp" {
        v2xw_net::gn::ITS_GN_MAX_SDU_SIZE
    } else {
        v2xw_net::wsmp::WSM_MAX_PAYLOAD_MIB_DEFAULT
    };
    if let Err(why) = cycle.alpha(mtu) {
        e.push(conflict(
            "net.fragmenter",
            format!(
                "is the certificate cycle, and its {}-octet hybrid certificate cannot travel \
                 in the {} SPDUs of a cycle under the {mtu}-octet MTU ({why:?}): every SPDU \
                 would carry only a digest no receiver could resolve",
                cycle.params().hybrid_cert_bytes,
                cycle.params().tau_spdus,
            ),
        ));
    }
}

/// A warning when `security.signature` makes every signed message larger than the network
/// layer's MTU and no fragmenter splits it: each SPDU is then refused before the MAC
/// (`net_mtu_refusals`), so the run sends no frames and costs nothing, which reads as a
/// cheap scheme rather than an impossible one. Not a refusal, so that a study of exactly
/// that impossibility still runs. The bound is the signature alone (FIPS 204: ML-DSA-44
/// 2,420 B, ML-DSA-65 3,309 B, ML-DSA-87 4,627 B; FIPS 205 SLH-DSA-128s 7,856 B) against
/// the WSM payload of 1,400 octets (IEEE 1609.3-2020 WsmMaxLength default) or the
/// GeoNetworking SDU of 1,398 (ETSI EN 302 636-4-1 Annex H), so it never warns on a
/// scheme some message could fit under.
pub fn signature_mtu_warnings(s: &Scenario) -> Vec<String> {
    let Some(scheme) = v2xw_sec::scheme::signature(&s.security.signature) else {
        return Vec::new();
    };
    if s.messages.sets.is_empty() {
        return Vec::new();
    }
    let splits = s
        .net
        .fragmenter
        .as_ref()
        .is_some_and(|f| f.id != v2xw_net::FRAGMENTER_NONE_ID);
    if splits {
        return Vec::new();
    }
    let gn = s.net.layer == "gn-btp";
    let mtu = if gn {
        v2xw_net::gn::ITS_GN_MAX_SDU_SIZE
    } else {
        v2xw_net::wsmp::WSM_MAX_PAYLOAD_MIB_DEFAULT
    };
    let sig = scheme.sig_octets();
    if sig <= mtu {
        return Vec::new();
    }
    let dsrc = s.radio.rat == crate::scenario::schema::Rat::Dsrc80211p;
    let offer = if !dsrc {
        "fragmentation runs over 802.11p only in this build, so choose a scheme whose \
         signature fits (falcon-512, falcon-1024 or hybrid-falcon512-ecdsa-p256) or set \
         radio.rat to DSRC with a fragmenter"
            .to_string()
    } else if gn {
        format!(
            "set net.fragmenter to '{}' (ETSI facilities-layer segmentation) or '{}' to \
             send it in pieces",
            v2xw_net::FRAGMENTER_FACILITIES_ID,
            v2xw_net::FRAGMENTER_GENERIC_ID
        )
    } else {
        format!(
            "set net.fragmenter to '{}' to send it in pieces, reassembled at the receiver",
            v2xw_net::FRAGMENTER_GENERIC_ID
        )
    };
    vec![format!(
        "security.signature '{}' signs with {sig} octets, more than the {} MTU of {mtu} \
         octets, and net.fragmenter does not split messages: every signed message will be \
         refused before the radio (counted in net_mtu_refusals) and the run will send no \
         frames; {offer}",
        s.security.signature,
        if gn { "GeoNetworking" } else { "WSMP" },
    )]
}

/// The inputs the scenario names **on disk**, checked before anything is built: the map
/// extract and the terrain raster.
///
/// Separate from [`validate`] on purpose. `validate` is a function of the document, which
/// is what makes a scenario's verdict the same on every machine; this is a function of the
/// machine it will run on. The server runs both at Check, Apply and Run, the command line
/// at `v2xw validate`, and [`crate::Engine::build`] before it imports, so a missing file
/// is named with its setting rather than reported by the importer as an internal error.
/// A relative path is resolved against the process's working directory, exactly as the
/// importer resolves it.
pub fn preflight(s: &Scenario) -> Vec<ScenarioError> {
    let mut e = Vec::new();
    let readable = |path: &str| -> core::result::Result<(), String> {
        let p = std::path::Path::new(path);
        match std::fs::metadata(p) {
            Ok(m) if m.is_file() => std::fs::File::open(p)
                .map(|_| ())
                .map_err(|why| format!("cannot be opened: {why}")),
            Ok(_) => Err("is a directory, not a file".to_string()),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
                let cwd = std::env::current_dir()
                    .map(|d| d.display().to_string())
                    .unwrap_or_else(|_| "the working directory".to_string());
                Err(format!(
                    "does not exist (a relative path is read from the directory the \
                     engine was started in, {cwd})"
                ))
            }
            Err(why) => Err(format!("cannot be read: {why}")),
        }
    };
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &s.world.source
        && !path.trim().is_empty()
        && let Err(why) = readable(path)
    {
        e.push(conflict(
            "world.source.path",
            format!(
                "names the map {path}, which {why}. Put the OpenStreetMap extract there, or \
                 point world.source.path at one that exists"
            ),
        ));
    }
    if s.world.terrain.drape && s.world.terrain.dem.is_none() {
        e.push(conflict(
            "world.terrain.drape",
            "would change nothing: world.terrain.dem is null, so there is no terrain to lay the \
             roads on. Name a DEM raster, or set drape to false"
                .to_string(),
        ));
    }
    if let Some(dem) = s.world.terrain.dem.as_deref()
        && let Err(why) = readable(dem)
    {
        e.push(conflict(
            "world.terrain.dem",
            format!(
                "names the terrain raster {dem}, which {why}. Point it at an SRTM .hgt tile \
                 or an ESRI ASCII grid, or set it to null for flat ground"
            ),
        ));
    }
    e
}

fn actors(s: &Scenario, e: &mut Vec<ScenarioError>) {
    bounded_at(
        "actors.vehicles.equipped_fraction",
        "actors.vehicles.equipped_fraction",
        s.actors.vehicles.equipped_fraction,
        e,
    );
    bounded_at(
        "actors.vru.device_fraction",
        "actors.vru.device_fraction",
        s.actors.vru.device_fraction,
        e,
    );
    for (path, value) in [
        (
            "actors.vru.midblock_rate_per_100m",
            s.actors.vru.midblock_rate_per_100m,
        ),
        (
            "actors.vru.red_crossing_share",
            s.actors.vru.red_crossing_share,
        ),
        ("actors.vru.ebike_share", s.actors.vru.ebike_share),
        (
            "actors.vru.crossing_gap_margin_s",
            s.actors.vru.crossing_gap_margin_s,
        ),
        (
            "actors.vru.walking_speed_mps",
            s.actors.vru.walking_speed_mps,
        ),
        (
            "actors.vehicles.drivers.parameter_spread",
            s.actors.vehicles.drivers.parameter_spread,
        ),
        (
            "actors.vehicles.drivers.reaction_time_s",
            s.actors.vehicles.drivers.reaction_time_s,
        ),
        (
            "actors.vehicles.drivers.perception_error",
            s.actors.vehicles.drivers.perception_error,
        ),
    ] {
        if let Some(v) = value {
            bounded_at(path, path, v, e);
        }
    }

    if !s.actors.vehicles.classes.is_empty() {
        let mut total = 0.0;
        for (name, c) in &s.actors.vehicles.classes {
            bounded_at(
                "actors.vehicles.classes.*.fraction",
                &format!("actors.vehicles.classes.{name}.fraction"),
                c.fraction,
                e,
            );
            total += c.fraction;
            if crate::wiring::vehicle_class_named(name).is_none() {
                e.push(conflict(
                    &format!("actors.vehicles.classes.{name}"),
                    format!(
                        "'{name}' is not a vehicle class this engine drives; the classes are \
                         {}. A cyclist or a pedestrian belongs in actors.vru",
                        v2xw_mobility::VehicleClass::ALL
                            .iter()
                            .filter(|c| crate::wiring::vehicle_class_named(c.as_str()).is_some())
                            .map(|c| c.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
        // 1e-9 is the project's cross-engine float tolerance (D9); anything looser would
        // let a fleet quietly lose vehicles.
        if (total - 1.0).abs() > 1e-9 {
            e.push(conflict(
                "actors.vehicles.classes",
                format!(
                    "the class shares sum to {total}, not 1.0; every vehicle belongs to \
                     exactly one class, so a sum below 1 leaves vehicles with no class and \
                     a sum above 1 makes the mix unreachable"
                ),
            ));
        }
    }

    // A demand model this build does not have is refused by name, rather than silently
    // running the Poisson model under another model's id.
    if !crate::wiring::DEMAND_KINDS.contains(&s.actors.vehicles.demand.kind.as_str()) {
        e.push(conflict(
            "actors.vehicles.demand.kind",
            format!(
                "'{}' is not a demand model this build ships; use one of {}",
                s.actors.vehicles.demand.kind,
                crate::wiring::DEMAND_KINDS.join(", ")
            ),
        ));
    }

    if let Some(per_km) = s.actors.vehicles.demand.rate_veh_per_h_per_lane_km {
        bounded_at(
            "actors.vehicles.demand.rate_veh_per_h_per_lane_km",
            "actors.vehicles.demand.rate_veh_per_h_per_lane_km",
            per_km,
            e,
        );
    }
    if let Some(p) = &s.actors.rsu_placement {
        bounded_at(
            "actors.rsu_placement.count",
            "actors.rsu_placement.count",
            f64::from(p.count),
            e,
        );
        bounded_at(
            "actors.rsu_placement.min_spacing_m",
            "actors.rsu_placement.min_spacing_m",
            p.min_spacing_m,
            e,
        );
        if let Some(profile) = &p.profile
            && !all_profile_ids().contains(&profile.as_str())
        {
            e.push(conflict(
                "actors.rsu_placement.profile",
                format!(
                    "'{profile}' is not a hardware profile this build ships; shipped \
                     profiles: {}",
                    all_profile_ids().join(", ")
                ),
            ));
        }
    }
    if let Some(city) = &s.city
        && crate::city::find(city).is_none()
    {
        e.push(conflict(
            "city",
            format!("'{city}' is not a city in worlds/cities.yaml; `v2xw cities` lists them"),
        ));
    }

    if let Some(t) = &s.traffic {
        bounded_at(
            "traffic.gridlock_timeout_s",
            "traffic.gridlock_timeout_s",
            t.gridlock_timeout_s,
            e,
        );
        bounded_at(
            "traffic.reroute_period_s",
            "traffic.reroute_period_s",
            t.reroute_period_s,
            e,
        );
        if let Some(share) = t.reroute_share {
            bounded_at("traffic.reroute_share", "traffic.reroute_share", share, e);
        }
        for (path, set) in [
            ("traffic.reroute_share", t.reroute_share.is_some()),
            ("traffic.reroute_source", t.reroute_source.is_some()),
        ] {
            if set && !t.dynamic_rerouting {
                e.push(conflict(
                    path,
                    "decides who re-routes and on what, and traffic.dynamic_rerouting is \
                     false"
                        .to_string(),
                ));
            }
        }
        if t.reroute_period_s > 0.0 && t.reroute_period_s < 10.0 {
            e.push(conflict(
                "traffic.reroute_period_s",
                format!(
                    "is {} s; re-planning more often than every 10 s only repeats the same \
                     route (lane speeds are smoothed over 180 s); use 0 for off or at least 10",
                    t.reroute_period_s
                ),
            ));
        }
        let g = &t.signals;
        for (path, v) in [
            ("traffic.signals.min_green_s", g.min_green_s),
            ("traffic.signals.max_green_s", g.max_green_s),
            ("traffic.signals.passage_s", g.passage_s),
            ("traffic.signals.detector_m", g.detector_m),
            (
                "traffic.signals.green_wave_heading_deg",
                g.green_wave_heading_deg,
            ),
        ] {
            bounded_at(path, path, v, e);
        }
        if let Some(v) = g.short_link_clearance_m {
            bounded_at(
                "traffic.signals.short_link_clearance_m",
                "traffic.signals.short_link_clearance_m",
                v,
                e,
            );
        }
        if let Some(v) = g.green_wave_speed_mps {
            bounded_at(
                "traffic.signals.green_wave_speed_mps",
                "traffic.signals.green_wave_speed_mps",
                v,
                e,
            );
        }
        if g.detector_placement != super::schema::DetectorPlacement::StopLine
            && !matches!(
                g.control,
                super::schema::SignalControlKind::Actuated | super::schema::SignalControlKind::Nema
            )
        {
            e.push(conflict(
                "traffic.signals.detector_placement",
                "places the detectors of actuated control, and traffic.signals.control is \
                 fixed-time; set control to actuated or remove it"
                    .to_string(),
            ));
        }
        // The heading only orients a green wave (crate::actuated::coordinate is a no-op
        // without a speed), so a heading moved off its default with no speed would do
        // nothing (the round-15 test found it dead on every family).
        if g.green_wave_speed_mps.is_none()
            && g.green_wave_heading_deg
                != super::schema::SignalControl::default().green_wave_heading_deg
        {
            e.push(conflict(
                "traffic.signals.green_wave_heading_deg",
                "orients a green wave, and traffic.signals.green_wave_speed_mps is not set; \
                 set the wave speed or remove the heading"
                    .to_string(),
            ));
        }
        if g.max_green_s < g.min_green_s {
            e.push(conflict(
                "traffic.signals.max_green_s",
                format!(
                    "is {} s, below the minimum green of {} s",
                    g.max_green_s, g.min_green_s
                ),
            ));
        }
        nema_checks(g, e);
    }
    if let Some(n) = s.actors.vehicles.demand.fleet_size {
        bounded_at(
            "actors.vehicles.demand.fleet_size",
            "actors.vehicles.demand.fleet_size",
            f64::from(n),
            e,
        );
    }
    if s.actors.vehicles.demand.fleet_size.is_some() && s.scale.is_some() {
        e.push(conflict(
            "actors.vehicles.demand.fleet_size",
            "holds a fleet in the microscopic engine only, and this scenario runs the \
             city-scale tiers (scale); remove one of the two"
                .to_string(),
        ));
    }
    if s.actors.vehicles.demand.fleet_size.is_some()
        && !matches!(
            s.actors.vehicles.demand.kind.as_str(),
            "mobility/demand/poisson" | v2xw_mobility::demand::poisson::MODEL_ID
        )
    {
        e.push(conflict(
            "actors.vehicles.demand.fleet_size",
            format!(
                "holds a fleet only with the Poisson demand model, and the demand is '{}'; \
                 set actors.vehicles.demand.kind to mobility/demand/poisson",
                s.actors.vehicles.demand.kind
            ),
        ));
    }
    if let Some(rate) = s.actors.vehicles.demand.rate_veh_per_h {
        if !(rate.is_finite() && rate >= 0.0) {
            e.push(conflict(
                "actors.vehicles.demand.rate_veh_per_h",
                format!("is {rate}, and an arrival rate is a finite non-negative number"),
            ));
        } else if rate > MAX_ARRIVAL_RATE_VEH_PER_H {
            bounded_at(
                "actors.vehicles.demand.rate_veh_per_h",
                "actors.vehicles.demand.rate_veh_per_h",
                rate,
                e,
            );
        } else {
            // Demand multipliers compose by product where they overlap, so the product of
            // every multiplier above 1 bounds the peak the arrival process is sized to.
            let peak: f64 = s
                .events
                .iter()
                .filter(|i| i.kind == TimelineKind::DemandMultiplier)
                .filter_map(|i| i.params.get("value").and_then(Value::as_f64))
                .filter(|v| v.is_finite() && *v > 1.0)
                .product();
            if rate * peak > MAX_ARRIVAL_RATE_VEH_PER_H {
                e.push(conflict(
                    "events",
                    format!(
                        "the demand multipliers raise the arrival rate of {rate} veh/h to as \
                         much as {:.0} veh/h, over the {MAX_ARRIVAL_RATE_VEH_PER_H} veh/h the \
                         loader accepts; lower the rate or the multipliers",
                        rate * peak
                    ),
                ));
            }
        }
    }

    // The reverse of the messages.sets rule above: a unit given the SPaT or MAP role
    // broadcasts it only when the set is on, so the role alone was a setting that did
    // nothing (a unit at a signalised junction with roles [spat, map] and sets [bsm] sent
    // no message at all, and nothing said why).
    let roled = s
        .actors
        .rsus
        .iter()
        .enumerate()
        .map(|(i, r)| (format!("actors.rsus[{i}].roles"), &r.roles))
        .chain(
            s.actors
                .rsu_placement
                .iter()
                .map(|p| ("actors.rsu_placement.roles".to_string(), &p.roles)),
        );
    for (field, roles) in roled {
        for set in ["spat", "map"] {
            let named = roles.iter().any(|r| r == set || r == "spat-map");
            if named && !s.messages.sets.iter().any(|m| m == set) {
                e.push(conflict(
                    &field,
                    format!(
                        "gives a roadside unit the '{set}' role, and messages.sets does not \
                         carry '{set}', so the unit would broadcast nothing for it; add \
                         '{set}' to messages.sets or drop the role"
                    ),
                ));
            }
        }
    }

    let profiles = all_profile_ids();
    let mut seen_sites = BTreeSet::new();
    let sites = world_site_count(s);
    for (i, r) in s.actors.rsus.iter().enumerate() {
        if let (Some(site), Some(count)) = (r.site, sites)
            && site as usize >= count
        {
            e.push(conflict(
                &format!("actors.rsus[{i}].site"),
                match &s.world.source {
                    v2xw_world::WorldSourceSpec::Procedural { .. } if count == 0 => format!(
                        "names site {site}, and this grid has no roadside sites: set \
                         world.source.params.rsu_at_junctions to true for one site per \
                         junction, or give this unit a position_m [x, y, z] instead"
                    ),
                    v2xw_world::WorldSourceSpec::Procedural { .. } => format!(
                        "names site {site}, and this grid has {count} (one per junction, \
                         numbered from 0); pick one below {count}, or give a position_m"
                    ),
                    other => format!(
                        "names site {site}, and an imported city ({}) has no roadside sites: \
                         OpenStreetMap carries no mast inventory. Remove `site` and give this \
                         unit a position_m [x, y, z] in world metres instead",
                        other.label()
                    ),
                },
            ));
        }
        if let Some(profile) = &r.profile
            && !profiles.contains(&profile.as_str())
        {
            e.push(conflict(
                &format!("actors.rsus[{i}].profile"),
                format!(
                    "'{profile}' is not a hardware profile this build ships, and an \
                     unknown one silently becomes the reference on-board unit; shipped \
                     profiles: {}",
                    profiles.join(", ")
                ),
            ));
        }
        match (r.site, r.position_m) {
            (Some(site), None) => {
                if !seen_sites.insert(site) {
                    e.push(conflict(
                        &format!("actors.rsus[{i}].site"),
                        format!(
                            "site {site} already carries a roadside unit; two units at one \
                             site would share a position and the node ids would be assigned \
                             by list order"
                        ),
                    ));
                }
            }
            (None, Some(p)) => {
                if p.iter().any(|v| !v.is_finite()) {
                    e.push(conflict(
                        &format!("actors.rsus[{i}].position_m"),
                        "is not a finite world-local position".to_string(),
                    ));
                }
            }
            (Some(_), Some(_)) => e.push(conflict(
                &format!("actors.rsus[{i}]"),
                "names both a world `site` and an explicit `position_m`; give exactly one, \
                 because two answers to where the mast stands is one answer too many"
                    .to_string(),
            )),
            (None, None) => e.push(conflict(
                &format!("actors.rsus[{i}]"),
                "says where it stands neither by world `site` nor by `position_m`; an OSM \
                 import carries no mast inventory, so a scenario on one states the position"
                    .to_string(),
            )),
        }
    }

    let entities: BTreeSet<&str> = s
        .actors
        .backend
        .entities
        .keys()
        .map(String::as_str)
        .collect();
    for (i, l) in s.actors.backend.links.iter().enumerate() {
        if let Some(latency) = l.latency_ms {
            bounded_at(
                "actors.backend.links[].latency_ms",
                &format!("actors.backend.links[{i}].latency_ms"),
                latency,
                e,
            );
        }
        if let Some(capacity) = l.capacity_mbps {
            bounded_at(
                "actors.backend.links[].capacity_mbps",
                &format!("actors.backend.links[{i}].capacity_mbps"),
                capacity,
                e,
            );
        }
        for (end, name) in [("from", &l.from), ("to", &l.to)] {
            if !entities.contains(name.as_str())
                && !crate::phase2::SCMS_ENTITIES.contains(&name.as_str())
            {
                e.push(conflict(
                    &format!("actors.backend.links[{i}].{end}"),
                    format!(
                        "'{name}' is not in actors.backend.entities (which has {})",
                        if entities.is_empty() {
                            "no entries".to_string()
                        } else {
                            entities.iter().copied().collect::<Vec<_>>().join(", ")
                        }
                    ),
                ));
            }
        }
    }
}

/// Weather at `t0`.
///
/// The two ranges here were the gap the schema's own header forbids: `intensity` and
/// `visibility_m` were defaulted and unchecked, so a scenario could state an intensity of
/// 40 and the propagation model would be handed it. A generated form would have offered
/// an unbounded number box for a `[0, 1]` quantity.
fn weather(s: &Scenario, e: &mut Vec<ScenarioError>) {
    bounded_at(
        "weather.intensity",
        "weather.intensity",
        s.weather.intensity,
        e,
    );
    if let Some(v) = s.weather.visibility_m {
        bounded_at("weather.visibility_m", "weather.visibility_m", v, e);
    }
}

fn radio(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let t = &s.radio.tiers;

    // `radio.devices` and `radio.range`: every number the link budget reads.
    let d = &s.radio.devices;
    for (class, power, gain, loss, height) in [
        (
            "obu",
            d.obu.tx_power_dbm,
            d.obu.antenna_gain_dbi,
            d.obu.cable_loss_db,
            d.obu.antenna_height_m,
        ),
        (
            "rsu",
            d.rsu.tx_power_dbm,
            d.rsu.antenna_gain_dbi,
            d.rsu.cable_loss_db,
            d.rsu.antenna_height_m,
        ),
        (
            "vru",
            d.vru.tx_power_dbm,
            d.vru.antenna_gain_dbi,
            d.vru.cable_loss_db,
            Some(d.vru.antenna_height_m),
        ),
    ] {
        for (leaf, value) in [
            ("tx_power_dbm", Some(power)),
            ("antenna_gain_dbi", Some(gain)),
            ("cable_loss_db", Some(loss)),
            ("antenna_height_m", height),
        ] {
            if let Some(v) = value {
                let path = format!("radio.devices.{class}.{leaf}");
                bounded_at(&path, &path, v, e);
            }
        }
    }
    bounded_at(
        "radio.range.margin_db",
        "radio.range.margin_db",
        s.radio.range.margin_db,
        e,
    );
    if let Some(cap) = s.radio.range.max_m {
        bounded_at("radio.range.max_m", "radio.range.max_m", cap, e);
    }

    radio_tuning(s, e);
    if let Some(r) = s.messages.rate_hz {
        bounded_at("messages.rate_hz", "messages.rate_hz", r, e);
        // Each vehicle's message generator is asked once per mobility step, so the step
        // caps the rate: measured 2026-10-08 on midtown-normal, 10 s, 13 vehicles, a 100 ms
        // step: 20 Hz sent 788 frames, exactly what 10 Hz sent; with a 50 ms step, 1,569.
        // A setting that silently does nothing is refused with the way out.
        let step_ms = s.time.mobility_step_ms.max(1) as f64;
        let ceiling_hz = 1_000.0 / step_ms;
        if r.is_finite() && r > ceiling_hz + 1e-9 {
            e.push(conflict(
                "messages.rate_hz",
                format!(
                    "is {r} Hz, but vehicles can send at most {ceiling_hz} Hz with a {step_ms} ms \
                     mobility step (each vehicle's message generator runs once a step): set \
                     time.mobility_step_ms to {} or less, or choose {ceiling_hz} Hz or less",
                    (1_000.0 / r).floor()
                ),
            ));
        }
        let timing_keys = [
            "nominal_itt_ms",
            "min_itt_ms",
            "t_gen_cam_min_ms",
            "t_gen_cam_max_ms",
        ];
        if s.messages
            .generator
            .as_ref()
            .is_some_and(|g| timing_keys.iter().any(|k| g.params.get(*k).is_some()))
        {
            e.push(conflict(
                "messages.rate_hz",
                "and messages.generator.params both set the message interval; keep one".to_string(),
            ));
        }
    }
    let dv = &s.actors.vehicles.drivers;
    for (path, v) in [
        ("actors.vehicles.drivers.max_accel_mps2", dv.max_accel_mps2),
        (
            "actors.vehicles.drivers.comfort_decel_mps2",
            dv.comfort_decel_mps2,
        ),
        ("actors.vehicles.drivers.time_headway_s", dv.time_headway_s),
        ("actors.vehicles.drivers.min_gap_m", dv.min_gap_m),
        ("actors.vehicles.drivers.politeness", dv.politeness),
        (
            "actors.vehicles.drivers.lane_change_threshold_mps2",
            dv.lane_change_threshold_mps2,
        ),
    ] {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
        }
    }
    traffic_audit_checks(s, e);
    if !dv.is_default() && s.mobility.as_ref().is_some_and(|m| m.provider == "sumo") {
        e.push(conflict(
            "actors.vehicles.drivers",
            "sets the native engine's drivers, and mobility.provider is sumo, whose drivers \
             are SUMO's vType parameters"
                .to_string(),
        ));
    }

    // `radio.models`: every family and id must be one `wiring::build_radio`,
    // `build_phy` and `build_obstacles` act on, with parameters that fit the model. The
    // parse is the wiring's own, so the loader and the run cannot disagree about it.
    if let Err(problems) = crate::wiring::radio_models(s) {
        for (path, why) in problems {
            e.push(conflict(&path, why));
        }
    }
    radio_law_checks(s, e);

    // `hybrid` names two radios on one node and a policy choosing between them per
    // message. `v2xw_radio::hybrid` has the selector; nothing states the policy, so the
    // engine would have to invent one.
    if s.radio.rat == crate::scenario::schema::Rat::Hybrid {
        e.push(conflict(
            "radio.rat",
            "is 'hybrid', which needs a per-message arbitration policy between the \
             802.11p and the sidelink stack that no scenario key states; choose \
             dsrc-80211p, lte-v2x-pc5 or nr-v2x-pc5"
                .to_string(),
        ));
    } else {
        // `radio.region` and `radio.channel`: the technology must have a channel in the
        // region, a named channel must be one the region opens to it, and a sidelink
        // profile fixed to one width needs a channel of that width.
        if let Some(ch) = s.radio.channel {
            bounded_at("radio.channel", "radio.channel", f64::from(ch), e);
        }
        if let Err((path, why)) = crate::wiring::radio_regulation(s) {
            e.push(conflict(path, why));
        }
        // `radio.channels` (WP-13): the map's channels must be the region's, one service
        // channel beside the safety one, real message sets, and one or two radios.
        if let Err(errors) = crate::run::channels::plan(s) {
            for (path, why) in errors {
                e.push(conflict(&path, why));
            }
        }
    }

    // 03-interfaces.md §13's own example, and 02-architecture.md §7.1's ladder: a
    // frame-level PHY decides receptions per frame, and a MAC below `high` does not
    // produce frames at that granularity, so the PHY would be computing outcomes for
    // arrivals the MAC never scheduled.
    if t.phy == Tier::High && t.mac != Tier::High {
        e.push(conflict(
            "radio.tiers.phy",
            format!(
                "'high' requires mac 'high' (mac is '{}'): a frame-level PHY decides an \
                 outcome per frame, and a '{}' MAC does not schedule frames at that \
                 granularity",
                t.mac, t.mac
            ),
        ));
    }
    if t.propagation == Tier::Abstract && t.phy != Tier::Abstract {
        e.push(conflict(
            "radio.tiers.propagation",
            format!(
                "'abstract' conflicts with phy '{}': an abstract propagation model returns a \
                 reception probability rather than a received power, and a '{}' PHY needs a \
                 power to accumulate SINR from",
                t.phy, t.phy
            ),
        ));
    }
    if let Some(f) = &t.focus {
        let base = t.phy.max(t.mac).max(t.propagation);
        if f.tier <= base {
            e.push(conflict(
                "radio.tiers.focus.tier",
                format!(
                    "is '{}' but the surrounding world already runs at '{base}' \
                     (radio.tiers): a focus region exists to run *higher* than its \
                     surroundings (02-architecture.md §7.3), so this one costs the mixed-tier \
                     boundary and buys nothing",
                    f.tier
                ),
            ));
        }
        if let crate::scenario::schema::FocusRegion::Follow { radius_m, .. } = f.region {
            bounded_at(
                "radio.tiers.focus.region.radius_m",
                "radio.tiers.focus.region.radius_m",
                radius_m,
                e,
            );
        }
    }
}

/// `radio.phy`, `radio.mac` and `radio.channel_model`: ranges, closed sets, and the
/// combinations that would leave a number with nothing to act on.
/// Refuses an EDCA number for a queue no frame of the run waits in, and an event queue on
/// a run with no event traffic: each would change nothing.
///
/// Without `net.access_categories: etsi-profiles` periodic messages wait in
/// `periodic_access_category` (AC_VO by default) and DENMs and forwarded copies, which only
/// a GeoNetworking stack sends, in `event_access_category` (`Engine::access_category`).
/// `etsi-profiles` uses AC_VO for DENMs alone and never AC_VI (EN 302 663 Annex B).
fn mac_queue_in_use(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let m = &s.radio.mac;
    let events = s.net.layer == "gn-btp";
    if m.event_access_category.is_some() && !events {
        e.push(conflict(
            "radio.mac.event_access_category",
            "chooses the queue of DENMs and forwarded copies, which only a GeoNetworking \
             stack sends; set net.layer to 'gn-btp' or remove it"
                .to_string(),
        ));
    }
    let etsi = s.net.access_categories.as_deref() == Some("etsi-profiles");
    let periodic = m.periodic_access_category.as_deref().unwrap_or("vo");
    let event = m.event_access_category.as_deref().unwrap_or("vo");
    let has_denm = s.messages.sets.iter().any(|x| x == "denm");
    let used = |ac: &str| {
        if etsi {
            ac == "vo" && has_denm
        } else {
            periodic == ac || (events && event == ac)
        }
    };
    for (ac, fields) in [
        (
            "vo",
            [
                ("radio.mac.cw_min_vo", m.cw_min_vo),
                ("radio.mac.aifsn_vo", m.aifsn_vo),
            ],
        ),
        (
            "vi",
            [
                ("radio.mac.cw_min_vi", m.cw_min_vi),
                ("radio.mac.aifsn_vi", m.aifsn_vi),
            ],
        ),
    ] {
        if used(ac) {
            continue;
        }
        for (path, v) in fields {
            if v.is_some() {
                e.push(conflict(
                    path,
                    format!(
                        "tunes the AC_{} queue, and no frame of this run waits in it; map a \
                         message there with radio.mac.periodic_access_category (or \
                         event_access_category on a gn-btp stack) or remove it",
                        ac.to_uppercase()
                    ),
                ));
            }
        }
    }
}

fn radio_tuning(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let p = &s.radio.phy;
    let m = &s.radio.mac;
    let c = &s.radio.channel_model;
    for (path, v) in [
        ("radio.phy.data_rate_mbps", p.data_rate_mbps),
        ("radio.phy.noise_figure_db", p.noise_figure_db),
        ("radio.phy.sensitivity_dbm", p.sensitivity_dbm),
        ("radio.phy.cca_threshold_dbm", p.cca_threshold_dbm),
        ("radio.phy.energy_detect_dbm", p.energy_detect_dbm),
        ("radio.phy.capture_threshold_db", p.capture_threshold_db),
        ("radio.mac.cw_min_vo", m.cw_min_vo.map(f64::from)),
        ("radio.mac.cw_min_vi", m.cw_min_vi.map(f64::from)),
        ("radio.mac.aifsn_vo", m.aifsn_vo.map(f64::from)),
        ("radio.mac.aifsn_vi", m.aifsn_vi.map(f64::from)),
        ("radio.mac.queue_length", m.queue_length.map(f64::from)),
        (
            "radio.channel_model.path_loss_exponent",
            c.path_loss_exponent,
        ),
        (
            "radio.channel_model.shadowing_sigma_db",
            c.shadowing_sigma_db,
        ),
        (
            "radio.channel_model.shadowing_decorrelation_m",
            c.shadowing_decorrelation_m,
        ),
        ("radio.channel_model.nakagami_shape", c.nakagami_shape),
        ("radio.channel_model.rice_k_db", c.rice_k_db),
    ] {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
        }
    }
    if let Some(r) = p.data_rate_mbps
        && !DATA_RATES_MBPS.iter().any(|x| (x - r).abs() < 1e-9)
    {
        e.push(conflict(
            "radio.phy.data_rate_mbps",
            format!("{r} Mbit/s is not a 10 MHz 802.11p rate; choose one of 3, 4.5, 6, 9, 12, 18, 24 or 27"),
        ));
    }
    for (path, cw) in [
        ("radio.mac.cw_min_vo", m.cw_min_vo),
        ("radio.mac.cw_min_vi", m.cw_min_vi),
    ] {
        if let Some(cw) = cw
            && !(cw + 1).is_power_of_two()
        {
            e.push(conflict(
                path,
                format!("{cw} is not 2^k - 1; EDCA windows are 1, 3, 7, 15, 31, 63, 127, 255, 511 or 1023 slots (IEEE 802.11-2020 §9.4.2.28)"),
            ));
        }
    }
    for (path, ac) in [
        (
            "radio.mac.periodic_access_category",
            &m.periodic_access_category,
        ),
        ("radio.mac.event_access_category", &m.event_access_category),
    ] {
        if let Some(ac) = ac {
            one_of(path, ac, &ACCESS_CATEGORIES, e);
        }
    }
    mac_queue_in_use(s, e);
    let dsrc = s.radio.rat == crate::scenario::schema::Rat::Dsrc80211p;
    if !dsrc && (!p.is_default() || !m.is_default()) {
        let path = if p.is_default() {
            "radio.mac"
        } else {
            "radio.phy"
        };
        e.push(conflict(
            path,
            "configures the 802.11p radio, and radio.rat is not dsrc-80211p; a sidelink's \
             MCS, sensitivity and resource settings are radio.models.sidelink's"
                .to_string(),
        ));
    }
    if !m.is_default() && s.radio.tiers.mac == v2xw_core::card::Tier::Abstract {
        e.push(conflict(
            "radio.mac",
            "sets EDCA numbers, and radio.tiers.mac is 'abstract', which models no medium \
             access; set it to 'medium' or 'high'"
                .to_string(),
        ));
    }
    let sl = &s.radio.sidelink;
    for (path, v) in [
        (
            "radio.sidelink.reservation_interval_ms",
            sl.reservation_interval_ms.map(f64::from),
        ),
        (
            "radio.sidelink.resource_keep_probability",
            sl.resource_keep_probability,
        ),
        ("radio.sidelink.rsrp_threshold_dbm", sl.rsrp_threshold_dbm),
    ] {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
        }
    }
    if !sl.is_default() && dsrc {
        e.push(conflict(
            "radio.sidelink",
            "configures C-V2X sidelink scheduling, and radio.rat is dsrc-80211p".to_string(),
        ));
    }
    if let Some(p) = sl.resource_keep_probability
        && v2xw_radio::ProbResourceKeep::from_probability(p).is_none()
    {
        e.push(conflict(
            "radio.sidelink.resource_keep_probability",
            format!("{p} is not a probResourceKeep value; choose 0, 0.2, 0.4, 0.6 or 0.8"),
        ));
    }
    if let Some(ms) = sl.reservation_interval_ms {
        let rri = v2xw_radio::Rri(ms);
        let legal = match s.radio.rat {
            crate::scenario::schema::Rat::LteV2xPc5 => rri.is_lte_legal(),
            _ => rri.is_nr_legal(),
        };
        if !legal {
            e.push(conflict(
                "radio.sidelink.reservation_interval_ms",
                format!(
                    "{ms} ms is not a reservation interval this sidelink allows; LTE: 20, \
                     50, 100, 200 ... 1000; NR: 1-99 or a multiple of 100 up to 1000"
                ),
            ));
        }
    }
    let d = &s.radio.dcc;
    for (path, v) in [
        ("radio.dcc.cbr_target", d.cbr_target),
        ("radio.dcc.adaptive_alpha", d.adaptive_alpha),
        ("radio.dcc.adaptive_beta", d.adaptive_beta),
        ("radio.dcc.max_itt_ms", d.max_itt_ms),
        ("radio.dcc.density_coefficient", d.density_coefficient),
        ("radio.dcc.min_power_dbm", d.min_power_dbm),
    ] {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
        }
    }
    if !d.is_default() {
        let running = crate::wiring::build_dcc(s);
        let adaptive = matches!(running, Some(crate::wiring::EngineDcc::Adaptive(_)));
        let sae = running
            .as_ref()
            .is_some_and(crate::wiring::EngineDcc::is_sae);
        let off = d.enabled == Some(false);
        for (path, set, applies, algo) in [
            (
                "radio.dcc.cbr_target",
                d.cbr_target.is_some(),
                adaptive,
                "the ETSI adaptive",
            ),
            (
                "radio.dcc.adaptive_alpha",
                d.adaptive_alpha.is_some(),
                adaptive,
                "the ETSI adaptive",
            ),
            (
                "radio.dcc.adaptive_beta",
                d.adaptive_beta.is_some(),
                adaptive,
                "the ETSI adaptive",
            ),
            (
                "radio.dcc.max_itt_ms",
                d.max_itt_ms.is_some(),
                sae,
                "the SAE J2945/1",
            ),
            (
                "radio.dcc.density_coefficient",
                d.density_coefficient.is_some(),
                sae,
                "the SAE J2945/1",
            ),
            (
                "radio.dcc.power_control",
                d.power_control.is_some(),
                sae,
                "the SAE J2945/1",
            ),
            (
                "radio.dcc.min_power_dbm",
                d.min_power_dbm.is_some(),
                sae,
                "the SAE J2945/1",
            ),
        ] {
            if set && !applies {
                e.push(conflict(
                    path,
                    if off {
                        "is a congestion-control number, and radio.dcc.enabled is false"
                            .to_string()
                    } else {
                        format!(
                            "configures {algo} congestion control, which this run does not \
                             use (radio.models.dcc, the region and radio.tiers.mac decide which runs)"
                        )
                    },
                ));
            }
        }
    }
    if let Some(f) = &c.fading_coherence {
        one_of(
            "radio.channel_model.fading_coherence",
            f,
            &FADING_COHERENCE,
            e,
        );
    }
    if let Some(f) = &c.fading {
        one_of("radio.channel_model.fading", f, &FADING_LAWS, e);
        if s.radio.models.contains_key("fading") {
            e.push(conflict(
                "radio.channel_model.fading",
                "and radio.models.fading both choose the fading law; keep one".to_string(),
            ));
        }
    }
    if c.nakagami_shape.is_some() && c.fading.as_deref() != Some("nakagami") {
        e.push(conflict(
            "radio.channel_model.nakagami_shape",
            "is the Nakagami shape; set radio.channel_model.fading to 'nakagami'".to_string(),
        ));
    }
    if c.rice_k_db.is_some() && c.fading.as_deref() != Some("rice") {
        e.push(conflict(
            "radio.channel_model.rice_k_db",
            "is the Rice factor; set radio.channel_model.fading to 'rice'".to_string(),
        ));
    }
}

fn net(s: &Scenario, e: &mut Vec<ScenarioError>) {
    one_of("net.layer", &s.net.layer, &NET_LAYERS, e);
    if let Some(a) = &s.net.channel_access {
        one_of("net.channel_access", a, &CHANNEL_ACCESS, e);
        if a == "alternating" && s.net.layer != "wsmp" {
            e.push(conflict(
                "net.channel_access",
                "channel switching is IEEE 1609.4, the US stack's; set net.layer to 'wsmp'"
                    .to_string(),
            ));
        }
    }
    if let Some(a) = &s.net.access_categories {
        one_of("net.access_categories", a, &NET_ACCESS_CATEGORIES, e);
    }
    if let Some(r) = &s.net.rsu_ip {
        one_of("net.rsu_ip.air", &r.air, &RSU_IP_AIR, e);
        bounded_at(
            "net.rsu_ip.wsa_interval_ms",
            "net.rsu_ip.wsa_interval_ms",
            f64::from(r.wsa_interval_ms),
            e,
        );
        if r.air == "sch" && s.net.layer != "wsmp" {
            e.push(conflict(
                "net.rsu_ip.air",
                "a WSA is IEEE 1609.3, the US stack's; set net.layer to 'wsmp'".to_string(),
            ));
        }
    }
    if let Some(c) = &s.net.dcc_cbr {
        one_of("net.dcc_cbr", c, &DCC_CBR, e);
        if c == "global" && s.net.layer != "gn-btp" {
            e.push(conflict(
                "net.dcc_cbr",
                "the global CBR is carried in GeoNetworking headers; set net.layer to \
                 'gn-btp', or use 'local'"
                    .to_string(),
            ));
        }
    }
    if s.net.access_categories.as_deref() == Some("etsi-profiles")
        && (s.radio.mac.periodic_access_category.is_some()
            || s.radio.mac.event_access_category.is_some())
    {
        e.push(conflict(
            "net.access_categories",
            "is 'etsi-profiles', which maps every message to its ETSI DCC profile's queue; \
             radio.mac.periodic_access_category and event_access_category choose the \
             queues by hand. Keep one"
                .to_string(),
        ));
    }
    if let Some(g) = &s.net.geonet {
        one_of("net.geonet.forwarding", &g.forwarding, &GN_FORWARDING, e);
        if s.net.layer != "gn-btp" {
            e.push(conflict(
                "net.geonet",
                "multi-hop forwarding is GeoNetworking's; WSMP has no forwarding. Set \
                 net.layer to 'gn-btp', or remove net.geonet"
                    .to_string(),
            ));
        }
        one_of(
            "net.geonet.area_forwarding",
            &g.area_forwarding,
            &GN_AREA_FORWARDING,
            e,
        );
        one_of(
            "net.geonet.non_area_forwarding",
            &g.non_area_forwarding,
            &GN_NON_AREA_FORWARDING,
            e,
        );
        one_of(
            "net.geonet.cbf_variant",
            &g.cbf_variant,
            &GN_CBF_VARIANTS,
            e,
        );
        bounded_at(
            "net.geonet.hop_limit",
            "net.geonet.hop_limit",
            f64::from(g.hop_limit),
            e,
        );
        bounded_at(
            "net.geonet.area_radius_m",
            "net.geonet.area_radius_m",
            g.area_radius_m,
            e,
        );
        bounded_at("net.geonet.range_m", "net.geonet.range_m", g.range_m, e);
        bounded_at(
            "net.geonet.repetition_interval_ms",
            "net.geonet.repetition_interval_ms",
            g.repetition_interval_ms as f64,
            e,
        );
        if let Some(ms) = g.repetition_duration_ms {
            bounded_at(
                "net.geonet.repetition_duration_ms",
                "net.geonet.repetition_duration_ms",
                ms as f64,
                e,
            );
        }
        bounded_at("net.geonet.window_s", "net.geonet.window_s", g.window_s, e);
        // The on-air routers (run/gn_air.rs) have no keep-alive forwarding: only the
        // overlay's DEN model re-sends a warning a vehicle stops hearing.
        if g.keep_alive_forwarding && g.forwarding == "on-air" {
            e.push(conflict(
                "net.geonet.keep_alive_forwarding",
                "is modelled by the overlay forwarding only; on the air it would do \
                 nothing. Set net.geonet.forwarding: overlay, or unset it"
                    .to_string(),
            ));
        }
    }
}

fn messages(s: &Scenario, e: &mut Vec<ScenarioError>) {
    one_of(
        "messages.codec_tier",
        &s.messages.codec_tier,
        &CODEC_TIERS,
        e,
    );
    if let Some(g) = s.messages.generator.as_ref() {
        generator(s, g, e);
    }
    if s.messages.sets.is_empty() {
        e.push(conflict(
            "messages.sets",
            "is empty; a run with no message set generates no traffic, which is a scenario \
             with radio configured and nothing to carry — say so with actors.vehicles.\
             equipped_fraction: 0 instead"
                .to_string(),
        ));
    }
    let mut seen = BTreeSet::new();
    for (i, set) in s.messages.sets.iter().enumerate() {
        one_of(&format!("messages.sets[{i}]"), set, &MESSAGE_SETS, e);
        if !seen.insert(set.as_str()) {
            e.push(conflict(
                &format!("messages.sets[{i}]"),
                format!("'{set}' is listed twice, so its generator would be installed twice"),
            ));
        }
    }
    // D2: the BSM, SPaT and MAP have hand-written J2735 encoders and the CAM and DENM
    // generated ETSI ones; PSM, SRM and SSM have a validated size model and no real
    // encoder, so `uper` cannot be honoured for them.
    if s.messages.codec_tier == "uper" {
        for (i, set) in s.messages.sets.iter().enumerate() {
            if matches!(set.as_str(), "srm" | "ssm" | "psm") {
                e.push(conflict(
                    &format!("messages.sets[{i}]"),
                    format!(
                        "'{set}' has no real UPER encoder (build decision D2: it ships as a \
                         validated size model), but messages.codec_tier is 'uper'; set \
                         codec_tier to 'size-model' or drop this set"
                    ),
                ));
            }
        }
    }
}

/// The generation-timing parameters (`v2xw_msg::GenerationTiming`), read by
/// `wiring::generation_timing` whichever generator the key names, with their upper bounds
/// in milliseconds.
const TIMING_PARAMS: [(&str, f64); 2] = [("phase_window_ms", 1_000.0), ("max_jitter_ms", 100.0)];

/// `messages.generator`: a known generator, its own parameter names, and values a node
/// stepped at the mobility step can honour.
///
/// Two tracks gave this key parameters, and both are honoured. The generation-timing pair
/// (`phase_window_ms`, `max_jitter_ms`; `wiring::generation_timing`) places every node's
/// generator in time and is accepted under any of the three ids. The BSM and CAM ids also
/// take their own generator's rule parameters (`wiring::generator_params`).
/// `generator/timing-phase-jitter` takes the timing pair only.
fn generator(s: &Scenario, g: &crate::scenario::ModelChoice, e: &mut Vec<ScenarioError>) {
    use v2xw_msg::generator::{BSM_GENERATOR_ID, CAM_GENERATOR_ID};
    let names: &[&str] = match g.id.as_str() {
        v2xw_msg::GENERATION_TIMING_ID => &[],
        BSM_GENERATOR_ID => &["nominal_itt_ms", "min_itt_ms", "max_itt_ms"],
        CAM_GENERATOR_ID => &[
            "t_gen_cam_min_ms",
            "t_gen_cam_max_ms",
            "t_check_cam_gen_ms",
            "n_gen_cam",
            "heading_threshold_deg",
            "position_threshold_m",
            "speed_threshold_mps",
            "low_frequency_interval_ms",
        ],
        other => {
            e.push(conflict(
                "messages.generator.id",
                format!(
                    "is '{other}', and the generators a node runs are '{BSM_GENERATOR_ID}' and \
                     '{CAM_GENERATOR_ID}'; '{}' sets the generation timing alone",
                    v2xw_msg::GENERATION_TIMING_ID
                ),
            ));
            return;
        }
    };
    let params = match &g.params {
        serde_json::Value::Null => return,
        serde_json::Value::Object(m) => m,
        _ => {
            e.push(conflict(
                "messages.generator.params",
                "must be an object of the generator's parameters".to_string(),
            ));
            return;
        }
    };
    let step_ms = s.time.mobility_step_ms as f64;
    let mut read = |name: &str| -> Option<f64> {
        let v = params.get(name)?;
        match v.as_f64() {
            Some(x) if x.is_finite() && x > 0.0 => Some(x),
            _ => {
                e.push(conflict(
                    &format!("messages.generator.params.{name}"),
                    format!("is {v}, and must be a positive number"),
                ));
                None
            }
        }
    };
    let values: Vec<(String, Option<f64>)> =
        names.iter().map(|n| (n.to_string(), read(n))).collect();
    for (key, v) in params {
        if let Some((_, limit)) = TIMING_PARAMS.iter().find(|(n, _)| n == key) {
            match v.as_f64() {
                Some(x) if x.is_finite() && (0.0..=*limit).contains(&x) => {}
                _ => e.push(conflict(
                    &format!("messages.generator.params.{key}"),
                    format!("is {v}, and it must be a number of milliseconds in [0, {limit}]"),
                )),
            }
        } else if !names.contains(&key.as_str()) {
            let all: Vec<&str> = names
                .iter()
                .copied()
                .chain(TIMING_PARAMS.iter().map(|(n, _)| *n))
                .collect();
            e.push(conflict(
                &format!("messages.generator.params.{key}"),
                format!(
                    "is not a parameter of '{}'; it has {}",
                    g.id,
                    all.join(", ")
                ),
            ));
        }
    }
    let get = |n: &str| values.iter().find(|(k, _)| k == n).and_then(|(_, v)| *v);
    // A node is stepped once per mobility step, so it cannot send more often than that.
    for n in [
        "nominal_itt_ms",
        "min_itt_ms",
        "t_gen_cam_min_ms",
        "t_check_cam_gen_ms",
    ] {
        if let Some(v) = get(n)
            && v < step_ms
        {
            e.push(conflict(
                &format!("messages.generator.params.{n}"),
                format!(
                    "is {v} ms, shorter than the {step_ms} ms mobility step the nodes are \
                     stepped at, so no node could send that often"
                ),
            ));
        }
    }
    let ordered = |lo: &str, hi: &str, e: &mut Vec<ScenarioError>| {
        if let (Some(a), Some(b)) = (get(lo), get(hi))
            && a > b
        {
            e.push(conflict(
                &format!("messages.generator.params.{lo}"),
                format!("is {a}, above {hi} = {b}"),
            ));
        }
    };
    ordered("min_itt_ms", "nominal_itt_ms", e);
    ordered("nominal_itt_ms", "max_itt_ms", e);
    ordered("t_gen_cam_min_ms", "t_gen_cam_max_ms", e);
}

fn security(s: &Scenario, e: &mut Vec<ScenarioError>) {
    one_of("security.envelope", &s.security.envelope, &ENVELOPES, e);
    one_of(
        "security.verification_policy",
        &s.security.verification_policy,
        &POLICIES,
        e,
    );
    if s.security.crypto_mode == crate::scenario::schema::CryptoModeSpec::Real
        && !REAL_SIGNATURES.contains(&s.security.signature.as_str())
    {
        e.push(conflict(
            "security.signature",
            format!(
                "'{}' has no real implementation, but security.crypto_mode is 'real'; \
                 'modeled' costs any primitive the hardware profile prices, 'real' does the \
                 mathematics and only {} is implemented",
                s.security.signature,
                REAL_SIGNATURES.join(", ")
            ),
        ));
    }
    if s.security.signer_id_policy.full_cert_every_ms == 0
        && s.security.signer_id_policy.digest_otherwise
    {
        e.push(conflict(
            "security.signer_id_policy.full_cert_every_ms",
            "is 0 (attach the certificate on every message) while digest_otherwise is true; \
             the two say opposite things about what goes in the signer identifier"
                .to_string(),
        ));
    }
    let sid = &s.security.signer_id_policy;
    if s.security.on_demand_threshold.is_some() && s.security.verification_policy != "on-demand" {
        e.push(conflict(
            "security.on_demand_threshold",
            "is read only by the on-demand verification policy; set \
             security.verification_policy: on-demand or remove it"
                .to_string(),
        ));
    }
    if s.security.prioritized_horizon_m.is_some() && s.security.verification_policy != "prioritized"
    {
        e.push(conflict(
            "security.prioritized_horizon_m",
            "is read only by the prioritized verification policy; set \
             security.verification_policy: prioritized or remove it"
                .to_string(),
        ));
    }
    if sid.on_new_neighbour && (sid.full_cert_every_ms == 0 || !sid.digest_otherwise) {
        e.push(conflict(
            "security.signer_id_policy.on_new_neighbour",
            "is set while every message already carries the certificate \
             (full_cert_every_ms 0 or digest_otherwise false), so it would change nothing"
                .to_string(),
        ));
    }
    if sid.on_new_neighbour && !s.messages.sets.iter().any(|m| m == "bsm" || m == "cam") {
        e.push(conflict(
            "security.signer_id_policy.on_new_neighbour",
            "is set but messages.sets has neither bsm nor cam, the awareness messages it \
             attaches the certificate to"
                .to_string(),
        ));
    }
    let p = &s.security.pseudonym_change;
    match p.strategy.as_str() {
        // `time` without `period_s` and `distance` without `distance_m` run on the rule's
        // cited default (`PseudonymStrategy::from_scenario`: J2945/1 CERTCHG 300 s, the
        // NYC CV pilot's 2 km). Refusing them here made the Studio's Pseudonym change
        // quick setting unusable: picking Distance on a scenario with no distance_m was
        // refused, and the quick panel has no distance field (quick-settings sweep).
        "time" | "distance" | "j2945-1" | "nyc-pilot" | "c2c-cc" | "mix-zone" | "silent" => {}
        other => e.push(conflict(
            "security.pseudonym_change.strategy",
            format!(
                "'{other}' is not one this build implements; allowed: {}",
                STRATEGIES.join(", ")
            ),
        )),
    }
    if let Some(range) = &p.silent_period_s
        && !(range.len() == 2
            && range.iter().all(|v| v.is_finite() && *v >= 0.0)
            && range[0] <= range[1])
    {
        e.push(conflict(
            "security.pseudonym_change.silent_period_s",
            format!("must be [min, max] seconds with 0 ≤ min ≤ max, got {range:?}"),
        ));
    }
    if let Some(period) = p.period_s {
        bounded_at(
            "security.pseudonym_change.period_s",
            "security.pseudonym_change.period_s",
            period,
            e,
        );
    }
    if let Some(distance) = p.distance_m {
        bounded_at(
            "security.pseudonym_change.distance_m",
            "security.pseudonym_change.distance_m",
            distance,
            e,
        );
    }
}

fn nodes(s: &Scenario, e: &mut Vec<ScenarioError>) {
    if s.nodes.default_obu.trim().is_empty() {
        e.push(conflict(
            "nodes.default_obu",
            "is empty; every equipped vehicle runs on a hardware profile and there is no \
             default profile to fall back on, because a defaulted profile would set the \
             service times of a whole run invisibly (06-node-models.md §1)"
                .to_string(),
        ));
    }
    let obus = obu_profile_ids();
    if !s.nodes.default_obu.trim().is_empty() && !obus.contains(&s.nodes.default_obu.as_str()) {
        // Until this rule existed an unknown id fell through to the reference profile
        // without a word, so a scenario could name a device that does not ship and get a
        // different one's service times. The shipped default was itself such an id.
        e.push(conflict(
            "nodes.default_obu",
            format!(
                "'{}' is not a hardware profile this build ships, and an unknown profile \
                 silently becomes the reference on-board unit — which would set the \
                 service times of the whole run to a device the scenario did not name. \
                 Shipped on-board units: {}",
                s.nodes.default_obu,
                obus.join(", ")
            ),
        ));
    }
    // A vehicle signs every message it sends on its own hardware, and a profile that
    // publishes no cost for the signing primitive signs nothing (`ObuRuntime` never signs
    // for free): every frame is dropped before the air and the run is silent. Found in QA:
    // `nodes.default_obu: obu/cohda-mk5`, offered by the page's list, ran 20 s of
    // Manhattan with 22 vehicles and put no frame on the air. Refused here by name, as a
    // roadside unit without one already is (`run.rs`, SPaT/MAP broadcasters).
    let vehicles_send = s
        .messages
        .sets
        .iter()
        .any(|m| matches!(m.as_str(), "bsm" | "cam" | "denm" | "srm" | "cpm"));
    if vehicles_send {
        // Under a hybrid signature the node makes and checks both signatures, so the
        // profile must publish both halves' costs (`v2xw_node::profile::signature_ops`).
        let (op, verify_op) = v2xw_node::profile::signature_ops(&s.security.signature);
        let hybrid = v2xw_sec::scheme::op_components(op).is_some();
        // A post-quantum scheme, hybrid or pure, must be priced both ways: no classical
        // fallback exists for its verification.
        let pq = v2xw_sec::scheme::signature(&s.security.signature)
            .is_some_and(|x| x.mode != v2xw_sec::scheme::SchemeMode::Classical);
        let signs = |id: &str| {
            v2xw_node::profiles::get(id)
                .is_none_or(|p| p.op_cost(op).is_some() && (!pq || p.op_cost(verify_op).is_some()))
        };
        let signing: Vec<&str> = obus.iter().copied().filter(|id| signs(id)).collect();
        let unsigned = |field: &str, id: &str, e: &mut Vec<ScenarioError>| {
            e.push(conflict(
                field,
                if hybrid {
                    format!(
                        "'{id}' does not publish both halves of {}: a hybrid signature is \
                         an ECDSA P-256 signature and a post-quantum one, made and checked \
                         on the vehicle's own hardware, and its cost is the two published \
                         costs together; on-board units that publish both: {}",
                        s.security.signature,
                        signing.join(", ")
                    )
                } else if pq {
                    format!(
                        "'{id}' prices neither {op} nor {verify_op} (no published figure, and \
                         no crypto_anchors entry the catalogue prices it on); on-board units \
                         that price both: {}",
                        signing.join(", ")
                    )
                } else {
                    format!(
                        "'{id}' publishes no {op} cost (its sources give no signing rate or \
                         latency for it), and a vehicle signs every message it sends on its \
                         own hardware, so no vehicle on it could send anything; on-board \
                         units that publish one: {}",
                        signing.join(", ")
                    )
                },
            ));
        };
        if obus.contains(&s.nodes.default_obu.as_str()) && !signs(&s.nodes.default_obu) {
            unsigned("nodes.default_obu", &s.nodes.default_obu, e);
        }
        for (name, profile) in &s.nodes.per_class {
            if obus.contains(&profile.as_str()) && !signs(profile) {
                unsigned(&format!("nodes.per_class.{name}"), profile, e);
            }
        }
    }
    for (name, profile) in &s.nodes.per_class {
        if !obus.contains(&profile.as_str()) {
            e.push(conflict(
                &format!("nodes.per_class.{name}"),
                format!(
                    "'{profile}' is not a hardware profile this build ships; shipped \
                     on-board units: {}",
                    obus.join(", ")
                ),
            ));
        }
    }
    for name in s.nodes.per_class.keys() {
        if !s.actors.vehicles.classes.is_empty() && !s.actors.vehicles.classes.contains_key(name) {
            e.push(conflict(
                &format!("nodes.per_class.{name}"),
                format!(
                    "names a vehicle class '{name}' that actors.vehicles.classes does not \
                     define (it defines {})",
                    s.actors
                        .vehicles
                        .classes
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
}

fn threats(s: &Scenario, e: &mut Vec<ScenarioError>) {
    if let Err(problems) = crate::run::jamming::jammer_specs(s) {
        for (path, why) in problems {
            e.push(conflict(&path, why));
        }
    }
    for (i, a) in s.threats.attackers.iter().enumerate() {
        let named = u8::from(a.fraction.is_some())
            + u8::from(a.count.is_some())
            + u8::from(!a.actor_ids.is_empty());
        if named != 1 {
            e.push(conflict(
                &format!("threats.attackers[{i}]"),
                format!(
                    "names {} of fraction, count and actor_ids; exactly one selects the \
                     attacker population, and two would need a rule for which wins",
                    if named == 0 {
                        "none".to_string()
                    } else {
                        named.to_string()
                    }
                ),
            ));
        }
        if let Some(f) = a.fraction {
            bounded_at(
                "threats.attackers[].fraction",
                &format!("threats.attackers[{i}].fraction"),
                f,
                e,
            );
        }
        if a.id.trim().is_empty() {
            e.push(conflict(
                &format!("threats.attackers[{i}].id"),
                "is empty; an attacker is a registered model and the id is how it is \
                 resolved and how the manifest pins it"
                    .to_string(),
            ));
        }
        // An end past the run's end means "until the run ends": shortening the run (the
        // Studio's Run length) must not make an attack scenario unstartable. The attack is
        // active over [from, min(to, duration)); `active_at` never sees the later instants.
        // A start at or past the end is refused: no attack would happen, and the run would
        // silently measure an honest fleet.
        if let Some(w) = a.schedule {
            if w.from_s < 0.0 || w.to_s <= w.from_s {
                e.push(conflict(
                    &format!("threats.attackers[{i}].schedule"),
                    format!(
                        "spans [{}, {}] s, which is not a non-empty interval of time",
                        w.from_s, w.to_s
                    ),
                ));
            } else if w.from_s >= s.time.duration_s {
                e.push(conflict(
                    &format!("threats.attackers[{i}].schedule"),
                    format!(
                        "starts the attack at {} s, but the run ends at {} s, so no attack \
                         would happen: lengthen the run past {} s or start the attack earlier",
                        w.from_s, s.time.duration_s, w.from_s
                    ),
                ));
            }
        }
    }
}

fn metrics_and_exporters(s: &Scenario, e: &mut Vec<ScenarioError>) {
    // An opt-in group (`signing`) is not selected by `all`, so it may stand beside it.
    let opt_in = |m: &String| {
        m == v2xw_metrics::signing::GROUP
            || m == v2xw_metrics::traffic::GROUP
            || m == v2xw_metrics::detection::SCORES_GROUP
            || m == v2xw_metrics::safety::SSM_GROUP
    };
    if s.metrics.iter().any(|m| m == "all") && s.metrics.iter().filter(|m| !opt_in(m)).count() > 1 {
        e.push(conflict(
            "metrics",
            format!(
                "lists 'all' beside {} other entries; 'all' already selects every registered \
                 metric, so the list says two different things",
                s.metrics.len() - 1
            ),
        ));
    }
    let mut seen = BTreeSet::new();
    for (i, x) in s.exporters.iter().enumerate() {
        if x.id.trim().is_empty() {
            e.push(conflict(
                &format!("exporters[{i}].id"),
                "is empty; an exporter is resolved by id".to_string(),
            ));
        } else if !seen.insert(x.id.as_str()) {
            e.push(conflict(
                &format!("exporters[{i}].id"),
                format!(
                    "'{}' is listed twice; the second would overwrite the first's output \
                     files",
                    x.id
                ),
            ));
        }
    }
}

fn timeline(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let doc = match serde_json::to_value(s) {
        Ok(v) => v,
        // Unreachable for a `Scenario` (every field is plain data), so it is reported
        // rather than unwrapped: a panic in a validator is worse than a missed rule.
        Err(err) => {
            e.push(conflict(
                "events",
                format!("cannot be checked because the scenario does not serialise: {err}"),
            ));
            return;
        }
    };
    for (i, item) in s.events.iter().enumerate() {
        let field = format!("events[{i}]");
        if !(item.t.is_finite() && (0.0..=s.time.duration_s).contains(&item.t)) {
            e.push(conflict(
                &format!("{field}.t"),
                format!(
                    "is {} s, which is outside the run [0, {}] s (time.duration_s), so the \
                     event would never happen: lengthen the run past {} s or move the event \
                     earlier",
                    item.t, s.time.duration_s, item.t
                ),
            ));
        }
        match item.until {
            Some(until) if !item.kind.takes_until() => e.push(conflict(
                &format!("{field}.until"),
                format!(
                    "is {until} s, but a '{}' has no end: it replaces the previous value \
                     rather than being undone",
                    serde_json::to_value(item.kind)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                ),
            )),
            // An end past the run's end means "until the run ends" (the run loop schedules
            // only the ends inside the horizon), so shortening the run keeps it startable.
            Some(until) if until <= item.t => e.push(conflict(
                &format!("{field}.until"),
                format!(
                    "is {until} s, which is not after the event's start t ({} s)",
                    item.t
                ),
            )),
            _ => {}
        }
        for key in item.kind.required_params() {
            if !item.params.contains_key(*key) {
                e.push(conflict(
                    &format!("{field}.{key}"),
                    format!(
                        "is missing, and a '{}' event needs it",
                        serde_json::to_value(item.kind)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_default()
                    ),
                ));
            }
        }
        if item.kind == TimelineKind::ParamChange
            && let Some(Value::String(path)) = item.params.get("path")
            && resolve_path(&doc, path).is_none()
        {
            e.push(conflict(
                &format!("{field}.path"),
                format!(
                    "'{path}' does not name a field of this scenario, so the change would \
                     be applied to nothing and the run would silently not do what the \
                     timeline says"
                ),
            ));
        }
        if item.kind == TimelineKind::WeatherFront
            && let Some(v) = item.params.get("value")
            && serde_json::from_value::<WeatherKind>(v.clone()).is_err()
        {
            e.push(conflict(
                &format!("{field}.value"),
                format!(
                    "{v} is not a weather kind; allowed: {}",
                    WeatherKind::ALL
                        .iter()
                        .filter_map(|k| serde_json::to_value(k).ok())
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        if item.kind == TimelineKind::DemandMultiplier
            && let Some(v) = item.params.get("value")
            && !v.as_f64().is_some_and(|x| x.is_finite() && x >= 0.0)
        {
            e.push(conflict(
                &format!("{field}.value"),
                format!("{v} is not a non-negative demand multiplier"),
            ));
        }
        live_timeline_item(s, i, item, e);
        trigger_checks(i, item, e);
    }
    // One window per population: the threat crate's `AttackSchedule` carries a single
    // `[from, to)`, so a population two waves name has no schedule that is both.
    let mut named: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for (i, item) in s.events.iter().enumerate() {
        if item.kind != TimelineKind::AttackWave {
            continue;
        }
        for p in crate::timeline::wave_populations(s, item.params.get("ids")).unwrap_or_default() {
            if let Some(first) = named.insert(p, i) {
                e.push(conflict(
                    &format!("events[{i}].ids"),
                    format!(
                        "names attacker population {p}, which events[{first}] already names; \
                         a population acts in one wave (the threat model's schedule is one \
                         window), so give the second wave its own population"
                    ),
                ));
            }
        }
    }
}

/// WP-21: the rules of a condition-triggered item. `max_fires` and `cooldown_s` belong to
/// a `when` item; every clause is in range and an area is a real polygon; a `target` that
/// names a binding (`when.leader`, `when.follower`, `when.actor`) is on a safety kind and
/// names one the clause always makes.
fn trigger_checks(
    i: usize,
    item: &crate::scenario::schema::TimelineItem,
    e: &mut Vec<ScenarioError>,
) {
    use crate::scenario::schema::Clause;
    use crate::timeline::trigger::Role;
    let field = format!("events[{i}]");
    if item.when.is_none() {
        for (key, set) in [
            ("max_fires", item.max_fires.is_some()),
            ("cooldown_s", item.cooldown_s.is_some()),
        ] {
            if set {
                e.push(conflict(
                    &format!("{field}.{key}"),
                    "applies to an item with a when clause, and this one fires once at t"
                        .to_string(),
                ));
            }
        }
    }
    if let Some(n) = item.max_fires {
        bounded_at(
            "events[].max_fires",
            &format!("{field}.max_fires"),
            f64::from(n),
            e,
        );
    }
    if let Some(c) = item.cooldown_s {
        bounded_at("events[].cooldown_s", &format!("{field}.cooldown_s"), c, e);
    }
    let role = item
        .params
        .get("target")
        .and_then(Value::as_str)
        .and_then(Role::parse);
    let Some(when) = &item.when else {
        if let Some(r) = role {
            e.push(conflict(
                &format!("{field}.target"),
                format!(
                    "is '{}', a binding of a when clause, and the item has no when clause",
                    r.name()
                ),
            ));
        }
        return;
    };
    if item.kind == TimelineKind::AttackWave {
        e.push(conflict(
            &format!("{field}.when"),
            "cannot trigger an attack.wave: the attacker populations' windows are fixed \
             before the run starts (threat schedule), so give the wave a t and an until"
                .to_string(),
        ));
    }
    let (_, clauses) = when.clauses();
    let composite = matches!(
        when,
        crate::scenario::schema::Condition::All { .. }
            | crate::scenario::schema::Condition::Any { .. }
    );
    if composite && clauses.is_empty() {
        e.push(conflict(
            &format!("{field}.when.of"),
            "names no clause; an empty all always holds and an empty any never does, so \
             say what should fire the item"
                .to_string(),
        ));
    }
    for (k, c) in clauses.iter().enumerate() {
        let (canon, at) = if composite {
            ("events[].when.of[]", format!("{field}.when.of[{k}]"))
        } else {
            ("events[].when", format!("{field}.when"))
        };
        match c {
            Clause::TtcBelow { ttc_s } => {
                bounded_at(&format!("{canon}.ttc_s"), &format!("{at}.ttc_s"), *ttc_s, e);
            }
            Clause::SpeedDiffAbove { speed_diff_mps } => bounded_at(
                &format!("{canon}.speed_diff_mps"),
                &format!("{at}.speed_diff_mps"),
                *speed_diff_mps,
                e,
            ),
            Clause::Probability { p } => {
                bounded_at(&format!("{canon}.p"), &format!("{at}.p"), *p, e);
            }
            Clause::InPolygon { points_m } => {
                let finite = points_m.iter().flatten().all(|v| v.is_finite());
                let n = points_m.len();
                // The shoelace area: zero for a degenerate polygon (a line or a point).
                let area = (0..n)
                    .map(|j| {
                        let [x0, y0] = points_m[j];
                        let [x1, y1] = points_m[(j + 1) % n];
                        x0 * y1 - x1 * y0
                    })
                    .sum::<f64>()
                    / 2.0;
                if n < 3 || !finite || area.abs() < 1e-6 {
                    e.push(conflict(
                        &format!("{at}.points_m"),
                        format!(
                            "has {n} corner(s){}, which encloses no area; give at least three \
                             finite [x, y] corners in metres of the world's local frame (the \
                             polygon closes itself)",
                            if finite {
                                ""
                            } else {
                                " and a non-finite value"
                            }
                        ),
                    ));
                }
            }
        }
    }
    if let Some(r) = role {
        if !matches!(
            item.kind,
            TimelineKind::HardBrake | TimelineKind::Breakdown | TimelineKind::CutIn
        ) {
            e.push(conflict(
                &format!("{field}.target"),
                format!(
                    "is '{}'; only the safety.* kinds act on a vehicle a when clause binds",
                    r.name()
                ),
            ));
        } else if !when.always_binds(r) {
            e.push(conflict(
                &format!("{field}.target"),
                format!(
                    "is '{}', which this when clause does not always bind: ttc_below and \
                     speed_diff_above bind when.leader and when.follower, in_polygon binds \
                     when.actor; an all binds what any of its clauses binds, an any only \
                     what every one does",
                    r.name()
                ),
            ));
        }
    }
}

/// The rules the four kinds that act on a running kernel add: a `param.change` names a
/// parameter that can change mid-run and a value that fits it, a `closure` names a target in
/// a form the engine reads, a `demand.multiplier` has an arrival process to scale, and an
/// `attack.wave` names populations that exist.
fn live_timeline_item(
    s: &Scenario,
    i: usize,
    item: &crate::scenario::schema::TimelineItem,
    e: &mut Vec<ScenarioError>,
) {
    let field = format!("events[{i}]");
    match item.kind {
        TimelineKind::ParamChange => {
            let Some(Value::String(path)) = item.params.get("path") else {
                return;
            };
            if crate::timeline::live_param(path).is_none() {
                e.push(conflict(
                    &format!("{field}.path"),
                    format!(
                        "'{path}' cannot change during a run: the model that reads it is built \
                         when the run starts, so a change at t = {} s would be accepted and do \
                         nothing. A param.change may set {}",
                        item.t,
                        crate::timeline::live_param_list()
                    ),
                ));
                return;
            }
            let Some(value) = item.params.get("value") else {
                return;
            };
            match crate::timeline::with_param(s, path, value) {
                Err(why) => e.push(conflict(&format!("{field}.value"), why)),
                Ok(next) => {
                    // The changed scenario is held to every rule the loaded one is, less its
                    // own timeline (which is this one, and would recurse).
                    let mut probe = next;
                    probe.events.clear();
                    for err in validate(&probe) {
                        e.push(conflict(
                            &format!("{field}.value"),
                            format!("{value} for '{path}' would make the scenario invalid: {err}"),
                        ));
                    }
                }
            }
        }
        TimelineKind::Closure => {
            if let Some(target) = item.params.get("target")
                && let Err(why) = crate::timeline::ClosureTarget::parse(target)
            {
                e.push(conflict(&format!("{field}.target"), why));
            }
        }
        TimelineKind::DemandMultiplier => {
            let kind = s.actors.vehicles.demand.kind.as_str();
            let poisson = kind == "mobility/demand/poisson"
                || kind == v2xw_mobility::demand::poisson::MODEL_ID;
            if !poisson {
                e.push(conflict(
                    &format!("{field}.type"),
                    format!(
                        "is a demand multiplier, and actors.vehicles.demand.kind is '{kind}', \
                         which has no arrival process to scale; use mobility/demand/poisson"
                    ),
                ));
            }
        }
        TimelineKind::AttackWave => {
            if let Err(why) = crate::timeline::wave_populations(s, item.params.get("ids")) {
                e.push(conflict(&format!("{field}.ids"), why));
            }
        }
        TimelineKind::HardBrake | TimelineKind::Breakdown | TimelineKind::CutIn => {
            use crate::safety_events as se;
            if let Err(why) = se::Pick::parse(item.params.get("target")) {
                e.push(conflict(&format!("{field}.target"), why));
            }
            if item.kind == TimelineKind::CutIn
                && let Err(why) = se::parse_side(item.params.get("side"))
            {
                e.push(conflict(&format!("{field}.side"), why));
            }
            for key in ["decel_mps2", "hold_s", "within_s"] {
                if let Err(why) = se::positive(item.params.get(key), 1.0) {
                    e.push(conflict(&format!("{field}.{key}"), why));
                }
            }
        }
        _ => {}
    }
    if item.kind == TimelineKind::ParamChange
        && let Some(Value::String(path)) = item.params.get("path")
        && path == "actors.vehicles.demand.rate_veh_per_h"
        && crate::timeline::base_rate_veh_per_h(s).is_none_or(|r| r <= 0.0)
    {
        e.push(conflict(
            &format!("{field}.path"),
            "changes the arrival rate, and the scenario states none to change it from: set \
             actors.vehicles.demand.rate_veh_per_h"
                .to_string(),
        ));
    }
}

/// The applications: known names, a compliance fraction, positive thresholds.
fn apps(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let a = &s.apps;
    for (i, name) in a.enabled.iter().enumerate() {
        one_of(
            &format!("apps.enabled[{i}]"),
            name,
            &crate::scenario::Apps::ALL,
            e,
        );
    }
    for (field, v) in [
        ("apps.glosa_compliance", a.glosa_compliance),
        ("apps.fcw_ttc_s", a.fcw_ttc_s),
        ("apps.ima_tti_s", a.ima_tti_s),
        ("apps.lta_gap_s", a.lta_gap_s),
        ("apps.pcw_ttc_s", a.pcw_ttc_s),
        ("apps.rlvw_decel_mps2", a.rlvw_decel_mps2),
    ] {
        bounded_at(field, field, v, e);
    }
    // A GLOSA compliance with GLOSA not running would do nothing.
    if a.glosa_compliance > 0.0 && !a.runs("glosa") {
        e.push(conflict(
            "apps.glosa_compliance",
            "is set, but `glosa` is not in apps.enabled, so no driver gets advice to follow"
                .to_string(),
        ));
    }
}

/// The energy section: the ranges from [`BOUNDS`], then the models and their parameter
/// overrides, checked by the very registry that builds them (`v2xw_energy::registry`).
fn energy(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let Some(en) = &s.energy else { return };
    for (seg, m) in &en.fleet {
        let at = |leaf: &str| format!("energy.fleet.{}.{leaf}", seg.as_str());
        bounded_at("energy.fleet.*.ev_share", &at("ev_share"), m.ev_share, e);
        bounded_at(
            "energy.fleet.*.diesel_share",
            &at("diesel_share"),
            m.diesel_share,
            e,
        );
        bounded_at("energy.fleet.*.euro", &at("euro"), f64::from(m.euro), e);
    }
    bounded_at(
        "energy.ev.initial_soc",
        "energy.ev.initial_soc",
        en.ev.initial_soc,
        e,
    );
    bounded_at(
        "energy.stop_speed_mps",
        "energy.stop_speed_mps",
        en.stop_speed_mps,
        e,
    );
    if let Err(why) = v2xw_energy::registry::emission_model(&en.emission_model) {
        e.push(conflict("energy.emission_model", why));
    }
    for (seg, model) in &en.emission_model_by_segment {
        if let Err(why) = v2xw_energy::registry::emission_model(model) {
            e.push(conflict(
                &format!("energy.emission_model_by_segment.{}", seg.as_str()),
                why,
            ));
        }
    }
    if let Err(why) = v2xw_energy::registry::ev_model(&en.ev_model, &en.ev.params) {
        let field = if v2xw_energy::registry::ev_model(&en.ev_model, &Default::default()).is_ok() {
            "energy.ev.params"
        } else {
            "energy.ev_model"
        };
        e.push(conflict(field, why));
    }
}

fn experiment(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let Some(x) = &s.experiment else { return };
    let doc = match serde_json::to_value(s) {
        Ok(v) => v,
        Err(_) => return,
    };
    if x.replications == 0 && x.seeds.is_empty() {
        e.push(conflict(
            "experiment.replications",
            "is 0 and experiment.seeds is empty, so the experiment has no runs in it".to_string(),
        ));
    }
    for (path, values) in &x.sweep {
        if resolve_path(&doc, path).is_none() && !names_a_default_section(&doc, path, values) {
            e.push(conflict(
                &format!("experiment.sweep.{path}"),
                format!("'{path}' does not name a field of this scenario"),
            ));
        }
        if values.is_empty() {
            e.push(conflict(
                &format!("experiment.sweep.{path}"),
                "has no values, so the sweep over it is empty and the whole experiment \
                 collapses to nothing"
                    .to_string(),
            ));
        }
    }
}

/// Whether `path` names a field inside a section the serialised scenario leaves out because
/// it is at its default (`apps` when every application setting is the default: then
/// `apps.glosa_compliance` is a field of the scenario that its document does not show, and
/// `glosa-energy.yaml`'s own sweep was refused on a variant whose `apps` was the default,
/// live-rsu §3). The path's first missing segment must be a whole section (not the last
/// segment), and written with one of the swept values the document must still read as a
/// scenario and give the value back at that path, inside a section that came back with its
/// other fields: a free-form map (which comes back holding only the written key) and a key
/// the schema does not have (refused on read) are still refused.
pub fn names_a_default_section(doc: &Value, path: &str, values: &[Value]) -> bool {
    default_section(doc, path, values).is_some()
}

/// Writes into `doc` the section [`names_a_default_section`] finds missing for `path`, with
/// every field it holds (the swept field at one of `values`, to be replaced), so a sweep can
/// replace `path` in it. Returns whether it wrote one.
pub fn fill_default_section(doc: &mut Value, path: &str, values: &[Value]) -> bool {
    if resolve_path(doc, path).is_some() {
        return false;
    }
    let Some((keys, section)) = default_section(doc, path, values) else {
        return false;
    };
    let mut at = doc;
    for k in &keys[..keys.len() - 1] {
        let Some(next) = at.get_mut(k.as_str()) else {
            return false;
        };
        at = next;
    }
    let Some(obj) = at.as_object_mut() else {
        return false;
    };
    obj.insert(keys[keys.len() - 1].clone(), section);
    true
}

/// The missing section's key path and its full serialised form, or `None`.
fn default_section(doc: &Value, path: &str, values: &[Value]) -> Option<(Vec<String>, Value)> {
    if path.contains('[') {
        return None;
    }
    let keys: Vec<&str> = path.split('.').collect();
    let mut cur = doc;
    let mut missing = None;
    for (i, k) in keys.iter().enumerate() {
        match cur.get(*k) {
            Some(v) => cur = v,
            None => {
                missing = Some(i);
                break;
            }
        }
    }
    let first = missing?;
    if first + 1 >= keys.len() {
        return None;
    }
    let section: Vec<String> = keys[..=first].iter().map(|k| (*k).to_string()).collect();
    let section_path = section.join(".");
    values.iter().find_map(|value| {
        let mut d = doc.clone();
        let mut at = &mut d;
        for (i, k) in keys.iter().enumerate() {
            let obj = at.as_object_mut()?;
            if i + 1 == keys.len() {
                obj.insert((*k).to_string(), value.clone());
                break;
            }
            at = obj
                .entry((*k).to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }
        let scenario = serde_json::from_value::<crate::scenario::Scenario>(d).ok()?;
        let back = serde_json::to_value(&scenario).ok()?;
        let same = match (resolve_path(&back, path), value) {
            (Some(Value::Number(a)), Value::Number(b)) => a.as_f64() == b.as_f64(),
            (Some(a), b) => a == b,
            (None, _) => false,
        };
        let full = resolve_path(&back, &section_path)
            .filter(|v| v.as_object().is_some_and(|o| o.len() > 1))?;
        same.then(|| (section.clone(), full.clone()))
    })
}

/// Walks a dotted path into a document, `a.b[2].c`.
///
/// Used by two rules — `param.change` paths and sweep paths — which is the whole reason
/// the scenario is re-serialised during validation: a path is checked against the
/// document the author wrote, not against a list of paths kept in step by hand.
pub fn resolve_path<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = doc;
    for segment in path.split('.') {
        let (name, indices) = split_indices(segment)?;
        if !name.is_empty() {
            cur = cur.get(name)?;
        }
        for i in indices {
            cur = cur.get(i)?;
        }
    }
    Some(cur)
}

/// `foo[1][2]` into `("foo", [1, 2])`; `None` if the brackets are malformed.
fn split_indices(segment: &str) -> Option<(&str, Vec<usize>)> {
    let Some(open) = segment.find('[') else {
        return Some((segment, Vec::new()));
    };
    let (name, rest) = segment.split_at(open);
    let mut indices = Vec::new();
    let mut rest = rest;
    while !rest.is_empty() {
        let close = rest.find(']')?;
        if !rest.starts_with('[') {
            return None;
        }
        indices.push(rest[1..close].parse::<usize>().ok()?);
        rest = &rest[close + 1..];
    }
    Some((name, indices))
}

// network audit ---------------------------------------------------------------------------

/// The fields the network-parameter audit exposed: ranges, and the refusal of a value the
/// run would not read (docs/guides/config-inventory-network.md).
/// The ranges and cross-field rules of `security.scms.lifecycle` (SCMS settings audit,
/// docs/guides/config-inventory-scms.md).
fn scms_lifecycle(s: &Scenario, e: &mut Vec<ScenarioError>) {
    if let Some(t) = s.security.scms.as_ref().and_then(|x| x.tls.as_deref())
        && !SCMS_TLS.contains(&t)
    {
        e.push(conflict(
            "security.scms.tls",
            format!(
                "is '{t}'; the US profile's session-scmsAuth is TLS 1.2 or 1.3: one of {}",
                SCMS_TLS.join(", ")
            ),
        ));
    }
    if let Some(t) = s
        .security
        .scms
        .as_ref()
        .and_then(|x| x.crl_overdue_scope.as_deref())
        && v2xw_node::rxval::CrlOverdueScope::parse(t).is_none()
    {
        e.push(conflict(
            "security.scms.crl_overdue_scope",
            format!(
                "is '{t}'; an overdue CRL refuses its CRACA's signers or every signer: one of {}",
                SCMS_CRL_OVERDUE_SCOPES.join(", ")
            ),
        ));
    }
    if let Some(t) = s
        .security
        .scms
        .as_ref()
        .and_then(|x| x.crl_overdue_tolerance_s)
    {
        bounded_at(
            "security.scms.crl_overdue_tolerance_s",
            "security.scms.crl_overdue_tolerance_s",
            f64::from(t),
            e,
        );
    }
    if let Some(x) = s.security.scms.as_ref() {
        for (path, v) in [
            ("security.scms.rx_beacon_max_age_s", x.rx_beacon_max_age_s),
            ("security.scms.rx_max_age_s", x.rx_max_age_s),
            ("security.scms.rx_max_future_s", x.rx_max_future_s),
        ] {
            if let Some(v) = v {
                bounded_at(path, path, v, e);
            }
        }
    }
    let Some(l) = s.security.scms.as_ref().and_then(|x| x.lifecycle.as_ref()) else {
        return;
    };
    for (key, v) in l.entries() {
        let path = format!("security.scms.lifecycle.{key}");
        bounded_at(&path, &path, v, e);
    }
    // The model refuses this at build time too; said here so the page marks the field.
    let i = crate::phase2::lifecycle_param(s, "i_period_s").unwrap_or(REAL_I_PERIOD_S);
    if let Some(life) = l.cert_lifetime_s
        && life < i
    {
        e.push(conflict(
            "security.scms.lifecycle.cert_lifetime_s",
            format!(
                "is {life} s, shorter than the {i} s i-period: between two periods' \
                 certificates a vehicle would hold nothing valid to sign with (CAMP-EE \
                 §2.1.5.3.2 makes the lifetime an hour longer than the period)"
            ),
        ));
    }
    if let (Some(lead), Some(life)) = (l.reenrol_lead_s, l.enrolment_lifetime_s)
        && lead >= life
    {
        e.push(conflict(
            "security.scms.lifecycle.reenrol_lead_s",
            format!(
                "is {lead} s, not shorter than the {life} s enrolment lifetime: the device \
                 would re-enrol the moment it enrols"
            ),
        ));
    }
}

fn network_audit(s: &Scenario, e: &mut Vec<ScenarioError>) {
    let sl = &s.radio.sidelink;
    let d = &s.radio.dcc;
    let g = s.net.geonet.as_ref();
    let m = &s.messages;
    let n = &s.nodes;
    for (path, v) in [
        ("radio.sidelink.noise_figure_db", sl.noise_figure_db),
        ("radio.sidelink.cbr_threshold_dbm", sl.cbr_threshold_dbm),
        ("radio.dcc.reactive_hold_ms", d.reactive_hold_ms),
        (
            "net.geonet.cbf_min_time_ms",
            g.and_then(|g| g.cbf_min_time_ms).map(|v| v as f64),
        ),
        (
            "net.geonet.cbf_max_time_ms",
            g.and_then(|g| g.cbf_max_time_ms).map(|v| v as f64),
        ),
        (
            "net.geonet.max_comm_range_m",
            g.and_then(|g| g.max_comm_range_m),
        ),
        (
            "messages.spat_interval_ms",
            m.spat_interval_ms.map(|v| v as f64),
        ),
        (
            "messages.map_interval_ms",
            m.map_interval_ms.map(|v| v as f64),
        ),
        (
            "messages.cpm_interval_ms",
            m.cpm_interval_ms.map(|v| v as f64),
        ),
        (
            "messages.cpm_quality_threshold",
            m.cpm_quality_threshold.map(f64::from),
        ),
        ("messages.cpm_max_objects", m.cpm_max_objects.map(f64::from)),
        (
            "messages.path_history_points",
            m.path_history_points.map(f64::from),
        ),
        (
            "nodes.verify_queue_depth",
            n.verify_queue_depth.map(f64::from),
        ),
        ("nodes.peer_cert_cache", n.peer_cert_cache.map(f64::from)),
        (
            "nodes.validated_cert_cache",
            n.validated_cert_cache.map(f64::from),
        ),
        ("nodes.hsm_sign_lower_us", n.hsm_sign_lower_us),
        (
            "security.on_demand_threshold",
            s.security.on_demand_threshold,
        ),
        (
            "security.prioritized_horizon_m",
            s.security.prioritized_horizon_m,
        ),
    ] {
        if let Some(v) = v {
            bounded_at(path, path, v, e);
        }
    }
    // A signing lower edge only acts on a profile whose signing time is a published upper
    // bound (wiring::with_sign_lower_edge, v2xw_node HsmOp::range_us), and only below it.
    if let Some(lo) = n.hsm_sign_lower_us {
        let (sign_op, _) =
            v2xw_node::profile::signature_ops(crate::signature::message_signature(s));
        let bound = v2xw_node::profiles::get(&n.default_obu)
            .and_then(|p| p.hsm.ops.get(sign_op))
            .filter(|op| {
                matches!(
                    op.latency_us.status,
                    Some(v2xw_node::profile::FieldStatus::UpperBound)
                )
            })
            .and_then(|op| op.latency_us.value);
        match bound {
            None => e.push(conflict(
                "nodes.hsm_sign_lower_us",
                format!(
                    "{} publishes no upper-bound HSM time for {sign_op}, so a lower edge \
                     would do nothing",
                    n.default_obu
                ),
            )),
            Some(hi) if lo >= hi => e.push(conflict(
                "nodes.hsm_sign_lower_us",
                format!("{lo} us is not below the profile's {hi} us signing bound"),
            )),
            Some(_) => {}
        }
    }
    // The sidelink pair rides on `radio.sidelink`, which `radio_tuning` already refuses on
    // an 802.11p run.
    if let Some(t) = sl.cbr_threshold_dbm
        && ((t + 112.0) / 2.0).fract().abs() > 1e-9
    {
        e.push(conflict(
            "radio.sidelink.cbr_threshold_dbm",
            format!("{t} dBm is not -112 + 2n dBm (TS 36.331 sl-ThresS-RSSI-CBR)"),
        ));
    }
    if let Some(t) = &d.reactive_table {
        one_of("radio.dcc.reactive_table", t, &DCC_REACTIVE_TABLES, e);
    }
    if d.reactive_table.is_some() || d.reactive_hold_ms.is_some() {
        let reactive = matches!(
            crate::wiring::build_dcc(s),
            Some(crate::wiring::EngineDcc::Reactive(_))
        );
        if !reactive {
            let path = if d.reactive_table.is_some() {
                "radio.dcc.reactive_table"
            } else {
                "radio.dcc.reactive_hold_ms"
            };
            e.push(conflict(
                path,
                "configures ETSI reactive congestion control, which this run does not use; \
                 set radio.models.dcc to dcc/etsi/reactive-ts102687"
                    .to_string(),
            ));
        }
    }
    if let Some(g) = g {
        let tuned = g.cbf_min_time_ms.is_some()
            || g.cbf_max_time_ms.is_some()
            || g.max_comm_range_m.is_some();
        if tuned && g.area_forwarding != "cbf" && g.non_area_forwarding != "cbf" {
            e.push(conflict(
                "net.geonet",
                "sets CBF timers, and neither area_forwarding nor non_area_forwarding is \
                 'cbf', so no contention timer runs"
                    .to_string(),
            ));
        }
        let lo = g
            .cbf_min_time_ms
            .unwrap_or(v2xw_net::stack::mib::CBF_MIN_TIME_MS);
        let hi = g
            .cbf_max_time_ms
            .unwrap_or(v2xw_net::stack::mib::CBF_MAX_TIME_MS);
        if tuned && lo >= hi {
            e.push(conflict(
                "net.geonet.cbf_min_time_ms",
                format!("{lo} ms is not below the CBF maximum time {hi} ms"),
            ));
        }
    }
    for (path, set, ty) in [
        (
            "messages.spat_interval_ms",
            m.spat_interval_ms.is_some(),
            "spat",
        ),
        (
            "messages.map_interval_ms",
            m.map_interval_ms.is_some(),
            "map",
        ),
        (
            "messages.cpm_interval_ms",
            m.cpm_interval_ms.is_some(),
            "cpm",
        ),
    ] {
        if set && !m.sets.iter().any(|x| x == ty) {
            e.push(conflict(
                path,
                format!("sets the {ty} period, and messages.sets does not include {ty}"),
            ));
        }
    }
    for (path, set, ty) in [
        (
            "messages.cpm_dcc_limited",
            m.cpm_dcc_limited.is_some(),
            "cpm",
        ),
        (
            "messages.cpm_quality_threshold",
            m.cpm_quality_threshold.is_some(),
            "cpm",
        ),
        (
            "messages.cpm_max_objects",
            m.cpm_max_objects.is_some(),
            "cpm",
        ),
        (
            "messages.denm_traffic_condition",
            m.denm_traffic_condition.is_some(),
            "denm",
        ),
        (
            "messages.denm_traction_loss",
            m.denm_traction_loss.is_some(),
            "denm",
        ),
    ] {
        if set && !m.sets.iter().any(|x| x == ty) {
            e.push(conflict(
                path,
                format!("configures the {ty} service, and messages.sets does not include {ty}"),
            ));
        }
    }
    if m.path_history_points.is_some() && !m.sets.iter().any(|x| x == "bsm" || x == "cam") {
        e.push(conflict(
            "messages.path_history_points",
            "sets the path history's length, and messages.sets has neither bsm nor cam, the \
             messages that carry it"
                .to_string(),
        ));
    }
}
