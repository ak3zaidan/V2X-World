//! Backend entities that misbehave: offline, slow, or lying.
//!
//! A fault is a property of a node over a window of simulated time, applied where the
//! kernel already is, so it reaches every protocol the same way:
//!
//! * **offline** — every delivery to the node in the window is dropped, timers included:
//!   the server is down. Nothing tells the sender; it finds out by timing out, which is
//!   what a real coordinator does.
//! * **slow** — every delivery the node handles in the window costs it `extra` more
//!   service time: an overloaded or distant server.
//! * **malicious** — the node answers, but a threshold participant's share is invalid
//!   (`crate::threshold::session`). Protocols that are not threshold ones ignore it.
//!
//! Faults are scenario data (`actors.backend.failures`), keyed by node, applied in id
//! order; nothing here reads a clock or a random number.

use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};

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
}

impl FaultPlan {
    /// Whether `node` is offline at `at`.
    pub fn offline(&self, node: NodeId, at: SimTime) -> bool {
        self.faults
            .iter()
            .any(|f| f.node == node && f.kind == FaultKind::Offline && f.active(at))
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
