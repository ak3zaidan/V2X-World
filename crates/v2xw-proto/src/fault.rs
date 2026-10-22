//! Backend entities that misbehave: offline, slow, or lying.
//!
//! A fault is a property of a node over a window of simulated time, applied where the
//! kernel already is, so it reaches every protocol the same way:
//!
//! * **offline** — the server is down. A device's request to it is dropped: nothing tells
//!   the device, which finds out by timing out (SCMS-522). Two exceptions, both what a real
//!   service does (edge case 3d of the configuration testing, `ScmsRun::step`):
//!   - a message one SCMS component sends another (an LA's PLVs or the ACA's certificates
//!     to a down RA) waits in the sender's outbound queue, which retries every
//!     [`BACKEND_RETRY`] until the receiver is back ([`FaultPlan::held`]);
//!   - a down component's own scheduled jobs (the RA's shuffle release, its weekly
//!     pre-generation) are persisted and run when it restarts
//!     ([`FaultPlan::deferred_timers`]).
//!
//!   An outage that never ends (`until` = `u64::MAX`) loses both, as before.
//! * **slow** — every delivery the node handles in the window costs it `extra` more
//!   service time: an overloaded or distant server.
//! * **malicious** — the node answers, but a threshold participant's share is invalid
//!   (`crate::threshold::session`). Protocols that are not threshold ones ignore it.
//!
//! Faults are scenario data (`actors.backend.failures`), keyed by node, applied in id
//! order; nothing here reads a clock or a random number.

use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};

/// How often an SCMS component retries a message to a component that is down.
///
/// **ASSUMPTION.** No SCMS document specifies inter-component retry: 1609.2.1 leaves the
/// RA↔LA flows `NULL` and CAMP specifies only the EE's 10 s retry (SCMS-522), which is
/// reused here. A real deployment's job queue would retry on a schedule of its own.
pub const BACKEND_RETRY: Duration = Duration::from_secs(10);

/// What is wrong with a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// Down: deliveries are dropped.
    Offline,
    /// Every message costs this much more service time.
    Slow {
        /// The extra time.
        extra: Duration,
    },
    /// Returns invalid threshold shares.
    Malicious,
}

impl FaultKind {
    /// The kind's stable name.
    pub const fn as_str(self) -> &'static str {
        match self {
            FaultKind::Offline => "offline",
            FaultKind::Slow { .. } => "slow",
            FaultKind::Malicious => "malicious",
        }
    }
}

/// One node's fault over `[from, until)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fault {
    /// The node.
    pub node: NodeId,
    /// What is wrong with it.
    pub kind: FaultKind,
    /// From when.
    pub from: SimTime,
    /// Until when (exclusive); `SimTime::MAX`-like `u64::MAX` for the rest of the run.
    pub until: SimTime,
}

impl Fault {
    /// Whether the fault holds at `at`.
    pub const fn active(&self, at: SimTime) -> bool {
        at >= self.from && at < self.until
    }
}

/// Every fault of a run, and what they did.
#[derive(Debug, Clone, Default)]
pub struct FaultPlan {
    /// The faults.
    pub faults: Vec<Fault>,
    /// Deliveries dropped because their receiver was offline.
    pub dropped: u64,
    /// Deliveries that a slow node handled, and so took longer.
    pub slowed: u64,
    /// Component-to-component messages held in the sender's queue while their receiver
    /// was down, and delivered on a retry after it came back.
    pub held: u64,
    /// A down component's own timers, run when it restarted.
    pub deferred_timers: u64,
}

impl FaultPlan {
    /// Whether `node` is offline at `at`.
    pub fn offline(&self, node: NodeId, at: SimTime) -> bool {
        self.faults
            .iter()
            .any(|f| f.node == node && f.kind == FaultKind::Offline && f.active(at))
    }

    /// The first instant at or after `at` at which `node` is up: `at` itself when no
    /// outage holds then, else the end of the outages that cover it back to back. `None`
    /// when it never comes back.
    pub fn back_online(&self, node: NodeId, at: SimTime) -> Option<SimTime> {
        let mut t = at;
        loop {
            let end = self
                .faults
                .iter()
                .filter(|f| f.node == node && f.kind == FaultKind::Offline && f.active(t))
                .map(|f| f.until)
                .max();
            match end {
                None => return Some(t),
                Some(u64::MAX) => return None,
                Some(u) => t = u,
            }
        }
    }

    /// When the outage that holds `node` down at `at` began (the earliest start of the
    /// offline faults active then); `None` when it is up.
    pub fn outage_start(&self, node: NodeId, at: SimTime) -> Option<SimTime> {
        self.faults
            .iter()
            .filter(|f| f.node == node && f.kind == FaultKind::Offline && f.active(at))
            .map(|f| f.from)
            .min()
    }

    /// The first retry, `BACKEND_RETRY` apart from `at`, that finds `node` up; `None`
    /// when it never comes back.
    pub fn retry_reaches(&self, node: NodeId, at: SimTime) -> Option<SimTime> {
        let up = self.back_online(node, at)?;
        let step = BACKEND_RETRY.as_nanos();
        let tries = (up - at).div_ceil(step);
        Some(at.saturating_add(tries.saturating_mul(step)))
    }

    /// The extra service time `node` pays at `at`, summed over its active slow faults.
    pub fn slow_extra(&self, node: NodeId, at: SimTime) -> Duration {
        let ns = self
            .faults
            .iter()
            .filter(|f| f.node == node && f.active(at))
            .map(|f| match f.kind {
                FaultKind::Slow { extra } => extra.as_nanos(),
                _ => 0,
            })
            .fold(0u64, u64::saturating_add);
        Duration::from_nanos(ns)
    }

    /// Whether `node` lies at `at`.
    pub fn malicious(&self, node: NodeId, at: SimTime) -> bool {
        self.faults
            .iter()
            .any(|f| f.node == node && f.kind == FaultKind::Malicious && f.active(at))
    }

    /// Whether any fault is declared at all.
    pub fn is_empty(&self) -> bool {
        self.faults.is_empty()
    }
}
