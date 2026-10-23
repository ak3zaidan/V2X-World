//! CPU and HSM servers: the modelled time a node spends doing cryptography
//! (06-node-models.md §2.1, 03-interfaces.md §8).
//!
//! This is the module the simulator exists for. A node that signs 10 BSMs a second on an
//! HSM whose datasheet says `<9 ms signing latency` is using 9 % of one engine; the same
//! node on a secure element that takes 60 ms is using 60 %, and a pseudonym change that
//! needs a second signature inside the same 100 ms window will not fit. A node that has to
//! verify 100 neighbours at 10 Hz needs a kilohertz of verification throughput, and the
//! profile set of §7 contains a real part that offers one hundred. None of that is visible
//! unless service time is real and the servers are finite, so here they are.
//!
//! The rule that keeps it honest is the same as everywhere else in the crate: a service
//! time comes from the profile or it does not exist. [`ProfileServiceModel::service_time`]
//! returns `None` for an operation the profile does not cost, and the caller has to decide
//! what to do about it, because the alternative — a plausible default — would silently set
//! the load of every run that forgot to configure it.

use v2xw_core::card::{
    Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::model::Model;
use v2xw_core::time::{Duration, SimTime};

use crate::ctx::NodeCtx;
use crate::profile::{HardwareProfile, RunsOn};

/// What kind of work a task is, for cost lookup and for the utilisation split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpClass {
    /// Producing a signature.
    Sign,
    /// Checking a signature.
    Verify,
    /// Hashing.
    Hash,
    /// Decoding an SPDU and applying the policy.
    Parse,
    /// Running a detector or a safety application.
    Application,
    /// Expanding a linkage-CRL i-period or sweeping an expired entry.
    CrlProcessing,
}

/// One unit of work, named so that its cost is a table lookup
/// (03-interfaces.md §8: `Task{queue, op, deadline}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpDescriptor {
    /// The operation id the profiles key their cost tables by, e.g. `ecdsa-p256-verify`.
    pub op: &'static str,
    /// What kind of work it is.
    pub class: OpClass,
    /// How many bytes it covers, where that matters.
    pub bytes: u32,
}

impl OpDescriptor {
    /// A signature verification over `bytes` bytes with the named primitive.
    pub const fn verify(op: &'static str, bytes: u32) -> Self {
        OpDescriptor {
            op,
            class: OpClass::Verify,
            bytes,
        }
    }

    /// A signature generation over `bytes` bytes.
    pub const fn sign(op: &'static str, bytes: u32) -> Self {
        OpDescriptor {
            op,
            class: OpClass::Sign,
            bytes,
        }
    }

    /// Non-cryptographic node work.
    pub const fn task(op: &'static str, class: OpClass) -> Self {
        OpDescriptor {
            op,
            class,
            bytes: 0,
        }
    }
}

/// How long an entity takes to do one thing, and how many at once
/// (03-interfaces.md §8).
///
/// Narrowed to [`NodeCtx`] rather than `Ctx` per build decision D12.2, and returning a
/// [`Duration`] rather than a `SimTime` per D11.1: a service time is a span, not an
/// instant.
pub trait ServiceModel: Model {
    /// How long `op` takes on this entity, or `None` when the profile costs no such
    /// operation.
    fn service_time(&mut self, ctx: &mut dyn NodeCtx, op: &OpDescriptor) -> Option<Duration>;

    /// How many of these run in parallel.
    fn servers(&self) -> u32;

    /// Where the work runs, which decides whether it charges the CPU or the HSM.
    fn runs_on(&self, op: &OpDescriptor) -> RunsOn;
}

/// Model id of the profile-driven service model.
pub const PROFILE_SERVICE_MODEL_ID: &str = "service-model/hardware-profile";

/// A [`ServiceModel`] that reads its costs out of a [`HardwareProfile`].
#[derive(Debug, Clone)]
pub struct ProfileServiceModel {
    /// Shared: every node built on one profile reads the same one (a full-scale run has
    /// thousands of nodes on a handful of profiles).
    profile: std::sync::Arc<HardwareProfile>,
    /// Cost of the non-cryptographic classes, when a scenario supplies one.
    app_task: Option<Duration>,
    /// The abstract compute tier: every operation the profile costs takes
    /// [`ProfileServiceModel::UNLIMITED_COST`] instead of its profiled time.
    unlimited: bool,
    /// The node whose `ServiceTime` stream a ranged cost draws from; without one a ranged
    /// cost is charged at its midpoint.
    node: Option<v2xw_core::ids::NodeId>,
    card: ModelCard,
}

impl ProfileServiceModel {
    /// A service model over `profile`, with no cost for application tasks.
    ///
    /// Application task costs are `TODO: calibrate` in 06-node-models §2.1 ("per-model
    /// cost classes declared in their model cards with `TODO: calibrate` defaults measured
    /// on the reference laptop and scaled by DMIPS ratio"), and none of the ten profiles
    /// carries a DMIPS rating for the reference laptop to scale against, so the default is
    /// "not modelled" rather than a made-up microsecond count.
    pub fn new(profile: impl Into<std::sync::Arc<HardwareProfile>>) -> Self {
        let profile = profile.into();
        let card = service_card(&profile);
        ProfileServiceModel {
            profile,
            app_task: None,
            unlimited: false,
            node: None,
            card,
        }
    }

    /// The service time of an operation at the abstract compute tier: one microsecond.
    ///
    /// Not zero, because a signature that completed at the instant the node decided to
    /// send would put the frame's MAC timer at the instant of the node phase that produced
    /// it, which the kernel refuses (02-architecture.md §5.1); one microsecond is the
    /// smallest interval that keeps it strictly after, and is what the engine already
    /// charges a frame whose profile publishes no signing rate.
    pub const UNLIMITED_COST: Duration = Duration::from_micros(1);

    /// The same model at the abstract compute tier (`nodes.compute_tier: abstract`):
    /// cryptography costs [`ProfileServiceModel::UNLIMITED_COST`], so no node is ever
    /// compute-bound and the verification queue never builds. An operation the profile
    /// does not cost still has no service time — the tier removes the bottleneck, it does
    /// not invent a primitive the device lacks.
    #[must_use]
    pub fn unlimited(mut self) -> Self {
        self.unlimited = true;
        self
    }

    /// The same model drawing ranged costs from `node`'s `ServiceTime` stream.
    #[must_use]
    pub fn for_node(mut self, node: v2xw_core::ids::NodeId) -> Self {
        self.node = Some(node);
        self
    }

    /// Whether this model runs at the abstract compute tier.
    pub fn is_unlimited(&self) -> bool {
        self.unlimited
    }

    /// The same model with a scenario-supplied cost for application tasks.
    #[must_use]
    pub fn with_app_task_cost(mut self, cost: Duration) -> Self {
        self.app_task = Some(cost);
        self
    }

    /// The profile behind the costs.
    pub fn profile(&self) -> &HardwareProfile {
        &self.profile
    }
}

impl Model for ProfileServiceModel {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl ServiceModel for ProfileServiceModel {
    fn service_time(&mut self, ctx: &mut dyn NodeCtx, op: &OpDescriptor) -> Option<Duration> {
        let cost = match op.class {
            OpClass::Sign | OpClass::Verify | OpClass::Hash => {
                // An upper-bound latency with a stated lower edge is drawn per call from
                // the node's own stream, so a run is still a function of its seed.
                let ranged = (!self.unlimited)
                    .then(|| self.profile.hsm.ops.get(op.op))
                    .flatten()
                    .filter(|_| self.profile.hsm.kind != crate::profile::HsmKind::None)
                    .and_then(crate::profile::HsmOp::range_us);
                match (ranged, self.node) {
                    (Some([lo, hi]), Some(node)) => {
                        let us = ctx
                            .rng(
                                v2xw_core::rng::RngDomain::ServiceTime,
                                v2xw_core::rng::EntityRef::Node(node),
                            )
                            .uniform(lo, hi);
                        Some(Duration::from_nanos((us * 1e3) as u64))
                    }
                    (Some([lo, hi]), None) => {
                        Some(Duration::from_nanos((0.5 * (lo + hi) * 1e3) as u64))
                    }
                    (None, _) => self.profile.op_cost(op.op).map(|(d, _)| d),
                }
            }
            OpClass::Parse | OpClass::Application | OpClass::CrlProcessing => self.app_task,
        };
        if self.unlimited {
            cost.map(|_| Self::UNLIMITED_COST)
        } else {
            cost
        }
    }

    fn servers(&self) -> u32 {
        // The CPU's core count is the only server count any profile publishes; the HSM's
        // is `hsm.servers`, which every profile carries as uncalibrated except
        // obu/cohda-mk5, where §7.1 states the single-server assumption explicitly.
        self.profile.cpu.cores.or(1).max(1)
    }

    fn runs_on(&self, op: &OpDescriptor) -> RunsOn {
        self.profile
            .op_cost(op.op)
            .map_or(RunsOn::Cpu, |(_, where_)| where_)
    }
}

fn service_card(profile: &HardwareProfile) -> ModelCard {
    let mut card = ModelCard::new(
        PROFILE_SERVICE_MODEL_ID,
        Family::ServiceModel,
        "0.1.0",
        format!(
            "Service times for one node, read out of the hardware profile `{}`; \
             an operation the profile does not cost has no service time here either.",
            profile.id
        ),
    );
    card.tier = vec![Tier::Medium, Tier::High];
    let mut p = Parameter::new(
        "app_task_us",
        "us",
        serde_json::Value::Null,
        Source::todo_calibrate("cost of a parse, detector or neighbour-table task"),
    );
    p.calibration = Some(
        "06-node-models §2.1 specifies these as per-model cost classes with TODO: calibrate \
         defaults 'measured on the reference laptop and scaled by DMIPS ratio'. Neither \
         the reference laptop nor a DMIPS rating for it exists yet, and six of the ten \
         shipped profiles publish no DMIPS either, so no default is asserted. Measure a \
         parse and a detector pass on the reference laptop with a cycle counter, record \
         its DMIPS, and scale."
            .to_string(),
    );
    card.parameters.push(p);
    card.sources.push(Source::new(
        SourceKind::Datasheet,
        format!("hardware profile {}@{}", profile.id, profile.version),
    ));
    card.assumptions.push(
        "An operation with no published cost has no modelled service time, rather than a \
         default: a default would set the load of every run that forgot to configure it, \
         invisibly."
            .to_string(),
    );
    card.validation = Validation::new(ValidationStatus::UnitTested);
    card
}

/// A bank of `c` identical FIFO servers.
///
/// The `medium` tier of 06-node-models §2.1 is one CPU server and one HSM server; the
/// `high` tier is `c` cores. One type serves both because the only difference is `c`.
#[derive(Debug, Clone)]
pub struct ServerBank {
    name: &'static str,
    free_at: Vec<SimTime>,
    busy_ns: u64,
    completed: u32,
    window_start: SimTime,
}

/// When a submitted task starts and finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scheduled {
    /// When a server picks it up.
    pub start: SimTime,
    /// When it is done.
    pub finish: SimTime,
    /// How long it waited between being offered and being started.
    pub wait: Duration,
}

impl ServerBank {
    /// A bank of `servers` servers, all free at `at`.
    ///
    /// A `servers` of zero is raised to one: a bank with no servers would accept work and
    /// never complete it, which models nothing and hides the configuration mistake.
    pub fn new(name: &'static str, servers: u32, at: SimTime) -> Self {
        ServerBank {
            name,
            free_at: vec![at; servers.max(1) as usize],
            busy_ns: 0,
            completed: 0,
            window_start: at,
        }
    }

    /// The same bank with `servers` servers, each free when the earliest of the old ones
    /// was. Zero is raised to one, as in [`ServerBank::new`].
    #[must_use]
    pub fn resized(mut self, servers: u32) -> Self {
        let at = self
            .free_at
            .iter()
            .copied()
            .min()
            .unwrap_or(self.window_start);
        self.free_at = vec![at; servers.max(1) as usize];
        self
    }

    /// The bank's name, for records.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// How many servers.
    pub fn servers(&self) -> usize {
        self.free_at.len()
    }

    /// The instant the first server frees up — when a task offered now would start, if it
    /// arrived no later than this.
    ///
    /// What a FIFO waiting line in front of the bank needs to simulate itself in continuous
    /// time: the head of the line starts at `max(its arrival, earliest_free())`, and it is
    /// in the queue (not in service) until then.
    pub fn earliest_free(&self) -> SimTime {
        self.free_at.iter().copied().min().unwrap_or(0)
    }

    /// Offers one task of length `duration`, arriving at `arrival`.
    ///
    /// The task goes to the server that frees up first; ties go to the lowest-indexed
    /// server, so the assignment is a function of the arrival sequence alone and does not
    /// depend on iteration order or on the thread the phase ran on.
    pub fn submit(&mut self, arrival: SimTime, duration: Duration) -> Scheduled {
        let mut best = 0usize;
        for i in 1..self.free_at.len() {
            if self.free_at[i] < self.free_at[best] {
                best = i;
            }
        }
        let start = arrival.max(self.free_at[best]);
        let finish = duration.after(start);
        self.free_at[best] = finish;
        self.busy_ns = self.busy_ns.saturating_add(duration.as_nanos());
        self.completed = self.completed.saturating_add(1);
        Scheduled {
            start,
            finish,
            wait: Duration::between(arrival, start),
        }
    }

    /// The fraction of the window every server was busy, in per mille, averaged over the
    /// servers and saturated at 1000.
    ///
    /// Per mille because that is the unit the telemetry record uses (`cpu_util_pm`,
    /// `hsm_util_pm`, §3.5.2 offsets 148 and 150).
    pub fn utilisation_pm(&self, now: SimTime) -> u16 {
        let window = now.saturating_sub(self.window_start);
        if window == 0 {
            return 0;
        }
        let capacity = (window as f64) * self.free_at.len() as f64;
        let frac = (self.busy_ns as f64) / capacity;
        // Quantised before the comparison and the cast, per build decision D10: the value
        // reaches an exported artefact and a threshold in the HUD.
        let pm = v2xw_core::math::quantize_to(frac * 1000.0, 1.0);
        if pm <= 0.0 {
            0
        } else if pm >= 1000.0 {
            1000
        } else {
            pm as u16
        }
    }

    /// How many tasks completed in this window.
    pub fn completed(&self) -> u32 {
        self.completed
    }

    /// Total busy time in this window.
    pub fn busy(&self) -> Duration {
        Duration::from_nanos(self.busy_ns)
    }

    /// Whether every server is idle at `now`.
    pub fn is_idle(&self, now: SimTime) -> bool {
        self.free_at.iter().all(|&t| t <= now)
    }

    /// Clears the window counters. The servers keep whatever work they are mid-way
    /// through, because a reporting boundary is not a reset of the hardware.
    pub fn reset_window(&mut self, now: SimTime) {
        self.busy_ns = 0;
        self.completed = 0;
        self.window_start = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::NodeRuntimeCtx;
    use crate::profile::HsmKind;
    use v2xw_core::rng::RngRegistry;
    use v2xw_core::time::NS_PER_MS;

    fn profile(id: &str) -> HardwareProfile {
        crate::profiles::get(id).expect("shipped profile").clone()
    }

    /// The reference OBU's published sign latency is 9 ms and its verify rate is a
    /// published lower bound with no latency, so the verify service time is the derived
    /// reciprocal. Both come from the profile; neither is written here.
    #[test]
    fn service_times_come_from_the_profile() {
        let reg = RngRegistry::new(0);
        let mut ctx = NodeRuntimeCtx::new(0, &reg);
        let mut sm = ProfileServiceModel::new(profile(crate::profiles::REFERENCE_OBU));

        let sign = sm
            .service_time(&mut ctx, &OpDescriptor::sign("ecdsa-p256-sign", 64))
            .expect("published");
        assert_eq!(sign, Duration::from_millis(9));
        assert_eq!(
            sm.runs_on(&OpDescriptor::sign("ecdsa-p256-sign", 64)),
            RunsOn::Hsm
        );

        let verify = sm
            .service_time(&mut ctx, &OpDescriptor::verify("ecdsa-p256-verify", 300))
            .expect("derived from the published rate");
        assert_eq!(verify, Duration::from_micros(400));
        assert_eq!(
            sm.runs_on(&OpDescriptor::verify("ecdsa-p256-verify", 300)),
            RunsOn::Accelerator
        );
    }

    /// The abstract compute tier: everything the profile costs takes a microsecond, and
    /// what it does not cost still has no service time.
    #[test]
    fn the_unlimited_tier_costs_a_microsecond_and_invents_nothing() {
        let reg = RngRegistry::new(0);
        let mut ctx = NodeRuntimeCtx::new(0, &reg);
        let mut sm = ProfileServiceModel::new(profile(crate::profiles::REFERENCE_OBU)).unlimited();
        assert!(sm.is_unlimited());
        let sign = sm.service_time(&mut ctx, &OpDescriptor::sign("ecdsa-p256-sign", 64));
        assert_eq!(sign, Some(ProfileServiceModel::UNLIMITED_COST));
        let verify = sm.service_time(&mut ctx, &OpDescriptor::verify("ecdsa-p256-verify", 300));
        assert_eq!(verify, Some(ProfileServiceModel::UNLIMITED_COST));
        assert_eq!(
            sm.service_time(&mut ctx, &OpDescriptor::sign("no-such-primitive", 64)),
            None
        );
    }

    /// Rule H2 in the cost path: a profile with no security hardware charges the software
    /// table, and the work lands on the CPU.
    #[test]
    fn a_profile_without_an_hsm_charges_the_cpu() {
        let reg = RngRegistry::new(0);
        let mut ctx = NodeRuntimeCtx::new(0, &reg);
        let p = profile("obu/generic-automotive-soc-no-hsm");
        assert_eq!(p.hsm.kind, HsmKind::None);
        let mut sm = ProfileServiceModel::new(p);
        let op = OpDescriptor::verify("ecdsa-p256-verify", 300);
        assert_eq!(
            sm.service_time(&mut ctx, &op),
            Some(Duration::from_micros(645))
        );
        assert_eq!(sm.runs_on(&op), RunsOn::Cpu);
        assert_eq!(sm.servers(), 4);
    }

    /// An operation nobody costed has no service time. This is the behaviour that keeps
    /// an unconfigured run visibly unconfigured.
    #[test]
    fn an_uncosted_operation_has_no_service_time() {
        let reg = RngRegistry::new(0);
        let mut ctx = NodeRuntimeCtx::new(0, &reg);
        let mut sm = ProfileServiceModel::new(profile("obu/cohda-mk5"));
        // The MK5's SXF1700 sign figures are NOT PUBLISHED and no software fallback is
        // measured for the i.MX6 DL either, so there is nothing to charge.
        assert_eq!(
            sm.service_time(&mut ctx, &OpDescriptor::sign("ecdsa-p256-sign", 64)),
            None
        );
        // Nor is an application task costed until a scenario supplies one.
        assert_eq!(
            sm.service_time(&mut ctx, &OpDescriptor::task("parse", OpClass::Parse)),
            None
        );
        let mut sm = sm.with_app_task_cost(Duration::from_micros(50));
        assert_eq!(
            sm.service_time(&mut ctx, &OpDescriptor::task("parse", OpClass::Parse)),
            Some(Duration::from_micros(50))
        );
    }

    /// One server, three back-to-back 9 ms signatures: the third waits 18 ms, which is the
    /// queueing delay a node that cannot keep up actually experiences.
    #[test]
    fn a_single_server_makes_work_queue() {
        let mut bank = ServerBank::new("hsm", 1, 0);
        let d = Duration::from_millis(9);
        let a = bank.submit(0, d);
        let b = bank.submit(0, d);
        let c = bank.submit(0, d);
        assert_eq!(a.wait, Duration::ZERO);
        assert_eq!(b.wait, Duration::from_millis(9));
        assert_eq!(c.wait, Duration::from_millis(18));
        assert_eq!(c.finish, 27 * NS_PER_MS);
        assert_eq!(bank.completed(), 3);
    }

    /// Four servers take four simultaneous tasks without queueing, and the utilisation is
    /// the average over the bank rather than over one server.
    #[test]
    fn a_bank_spreads_work_and_averages_its_utilisation() {
        let mut bank = ServerBank::new("cpu", 4, 0);
        for _ in 0..4 {
            assert_eq!(
                bank.submit(0, Duration::from_millis(100)).wait,
                Duration::ZERO
            );
        }
        // 4 x 100 ms of work over a 1 s window across 4 servers is 10 %.
        assert_eq!(bank.utilisation_pm(v2xw_core::time::NS_PER_S), 100);
        // The same work on one server is 40 %.
        let mut one = ServerBank::new("cpu", 1, 0);
        for _ in 0..4 {
            one.submit(0, Duration::from_millis(100));
        }
        assert_eq!(one.utilisation_pm(v2xw_core::time::NS_PER_S), 400);
    }

    /// Utilisation saturates rather than reporting more than 100 %, which a bank given
    /// more work than the window can hold would otherwise do.
    #[test]
    fn utilisation_saturates() {
        let mut bank = ServerBank::new("hsm", 1, 0);
        for _ in 0..20 {
            bank.submit(0, Duration::from_millis(100));
        }
        assert_eq!(bank.utilisation_pm(v2xw_core::time::NS_PER_S), 1000);
    }

    /// The professor's case: on the reference OBU sign reads 9 ms and verify 0.4 ms. The
    /// placement must say they ran on different hardware, that 9 ms is a vendor bound and
    /// that 0.4 ms is derived from a rate; the software variant must have verify > sign.
    #[test]
    fn crypto_placement_names_the_hardware_and_the_kind_of_figure() {
        use crate::profile::CostBasis;
        let hw = profile(crate::profiles::REFERENCE_OBU)
            .crypto_placement("ecdsa-p256-sign", "ecdsa-p256-verify");
        let sign = hw.sign.expect("costed");
        let verify = hw.verify.expect("costed");
        assert_eq!(sign.facts.basis, CostBasis::UpperBound);
        assert!(!sign.facts.basis.is_exact());
        assert_eq!(sign.label, "signed in the secure element");
        assert!(
            sign.basis_text.starts_with("at most 9.000 ms"),
            "{}",
            sign.basis_text
        );
        assert_eq!(verify.facts.basis, CostBasis::DerivedFromThroughput);
        assert_eq!(verify.label, "verified by the hardware accelerator");
        assert!(
            verify.basis_text.contains("derived"),
            "{}",
            verify.basis_text
        );
        assert!(
            hw.explanation.contains("different hardware"),
            "{}",
            hw.explanation
        );

        let sw = profile("obu/unex-obu-301-craton2-software")
            .crypto_placement("ecdsa-p256-sign", "ecdsa-p256-verify");
        let (s, v) = (sw.sign.expect("costed"), sw.verify.expect("costed"));
        assert_eq!(s.facts.runs_on, RunsOn::Cpu);
        assert_eq!(v.facts.runs_on, RunsOn::Cpu);
        assert_eq!(s.facts.basis, CostBasis::SoftwareProxy);
        assert!(
            v.modelled_ms > s.modelled_ms,
            "software verify must cost more than sign"
        );
        assert!(
            sw.explanation
                .contains("verification costs more than signing"),
            "{}",
            sw.explanation
        );
    }

    /// Every shipped OBU profile states where it signs and verifies.
    #[test]
    fn every_obu_profile_places_sign_and_verify() {
        for p in crate::profiles::all()
            .iter()
            .filter(|p| p.id.starts_with("obu/"))
        {
            let c = p.crypto_placement("ecdsa-p256-sign", "ecdsa-p256-verify");
            assert!(
                c.sign.is_some() || c.verify.is_some(),
                "{} places neither",
                p.id
            );
        }
    }

    /// An upper-bound latency with a lower edge is drawn per call inside the range, from
    /// the node's own stream: the same seed gives the same draws.
    #[test]
    fn a_bounded_sign_latency_with_a_lower_edge_is_drawn_inside_it() {
        let mut p = profile(crate::profiles::REFERENCE_OBU);
        let op = p.hsm.ops.get_mut("ecdsa-p256-sign").expect("costed");
        op.latency_lower_us.value = Some(3000.0);
        let draw = |seed: u64| {
            let reg = RngRegistry::new(seed);
            let mut ctx = NodeRuntimeCtx::new(0, &reg);
            let mut sm =
                ProfileServiceModel::new(p.clone()).for_node(v2xw_core::ids::NodeId::new(4));
            (0..20)
                .map(|_| {
                    sm.service_time(&mut ctx, &OpDescriptor::sign("ecdsa-p256-sign", 64))
                        .expect("costed")
                        .as_nanos()
                })
                .collect::<Vec<_>>()
        };
        let a = draw(7);
        assert_eq!(a, draw(7));
        assert!(a.iter().all(|&ns| (3_000_000..=9_000_000).contains(&ns)));
        assert!(a.iter().any(|&ns| ns != a[0]), "the draws vary");
        let facts = p.op_cost_facts("ecdsa-p256-sign").expect("costed");
        assert_eq!(facts.basis, crate::profile::CostBasis::WithinBound);
        assert_eq!(facts.range_us, Some([3000.0, 9000.0]));
    }
}
