//! What each backend entity is doing, as an inspector reads it.
//!
//! One shape for both credential systems, because the question a researcher asks of either
//! is the same: which authorities exist, which are online, how busy each one is, what it
//! has issued, and which messages are flowing between it, the other authorities and the
//! devices. Every number here is read from the entity's own state or from the kernel's
//! logs; none is estimated.
//!
//! Entities are identified by a short role id (`ra`, `pca`, `la1`, `ea`, `aa`, …) and
//! devices are pooled as one `ee` node, so a diagram of the backend has a fixed set of
//! boxes whatever the fleet size.

use std::collections::{BTreeMap, VecDeque};

use serde_json::Value;
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;

use crate::kernel::{Kernel, NodeTraffic};
use crate::stage::WireStep;

/// One entity's queue at an instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct QueueView {
    /// Requests in service or waiting.
    pub depth: u64,
    /// Servers (the `c` of the M/M/c).
    pub servers: u32,
    /// Requests served since the run began.
    pub served: u64,
    /// Total service time, ns.
    pub busy_ns: u64,
    /// Total time requests waited for a server, ns.
    pub waited_ns: u64,
    /// Work already admitted that is not finished yet, ns.
    pub backlog_ns: u64,
    /// Deliveries on their way to it (messages on links, timers it set).
    pub inbound: u64,
}

/// One authority, or the pooled devices.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EntityView {
    /// Its role id: `ra`, `pca`, `la1`, `root`, `electors`, `ee`, …
    pub id: String,
    /// Its name as the standard spells it.
    pub name: String,
    /// `scms` or `ccms`.
    pub system: &'static str,
    /// Which tier of the diagram it belongs in: `governance`, `ca`, `ra`, `privacy`,
    /// `revocation`, `distribution`, `device`.
    pub tier: &'static str,
    /// Whether it is online during a run (offline authorities act at setup only).
    pub online: bool,
    /// Its node id in the backend, if it has one.
    pub node: Option<u32>,
    /// What it does, in one line.
    pub role: &'static str,
    /// Its queue, if it is hosted.
    pub queue: Option<QueueView>,
    /// Messages and bytes in and out.
    pub traffic: NodeTraffic,
    /// Cryptographic operations it performed, by kind (`sign`, `verify`, `keygen`).
    pub ops: BTreeMap<String, u64>,
    /// Its own counts: what it issued, served, refused, holds.
    pub state: BTreeMap<String, Value>,
}

impl EntityView {
    /// Sets one state field.
    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.state.insert(key.to_string(), value.into());
    }
}

/// The traffic between two entities (or an entity and the devices) since the run began.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EdgeView {
    /// Sending role id.
    pub from: String,
    /// Receiving role id.
    pub to: String,
    /// Messages.
    pub messages: u64,
    /// Bytes.
    pub bytes: u64,
    /// The last message's instant, ns.
    pub last_t: SimTime,
    /// The last message's step name (`provisioning-request`, `crl-download`, …).
    pub last_step: String,
    /// The transport the last message used (`cellular-uu`, `backend-net`, …).
    pub transport: String,
    /// Message counts by step name.
    pub steps: BTreeMap<String, u64>,
}

/// One recent message, for a live feed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StepView {
    /// When it left, ns.
    pub t: SimTime,
    /// Sending role id.
    pub from: String,
    /// Receiving role id.
    pub to: String,
    /// The flow it belongs to.
    pub flow: String,
    /// Its step name.
    pub step: String,
    /// Its size.
    pub bytes: u32,
    /// Its transport.
    pub transport: String,
}

/// The whole backend at an instant.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BackendView {
    /// `scms` or `ccms`.
    pub system: &'static str,
    /// The protocol's id.
    pub protocol: &'static str,
    /// The signature scheme every certificate and signed message uses: `ecdsa-p256`, or
    /// the hybrid post-quantum scheme's id (`crate::hybrid`).
    pub signature: &'static str,
    /// The instant, ns.
    pub t: SimTime,
    /// Every entity, governance first.
    pub entities: Vec<EntityView>,
    /// Every pair that has exchanged a message.
    pub edges: Vec<EdgeView>,
    /// The newest messages, newest last.
    pub recent: Vec<StepView>,
    /// Flow runs started, by flow name.
    pub flows: BTreeMap<String, u64>,
}

/// How many recent messages a view carries.
pub const RECENT: usize = 40;

/// Folds the kernel's wire log into edges, incrementally.
///
/// The log is append-only, so a cursor is enough: each call reads only what was logged
/// since the last one.
#[derive(Debug, Clone, Default)]
pub struct EdgeTracker {
    cursor: usize,
    edges: BTreeMap<(String, String), EdgeView>,
    recent: VecDeque<StepView>,
    flows: BTreeMap<String, std::collections::BTreeSet<u32>>,
}

impl EdgeTracker {
    /// Reads every step logged since the last call, naming nodes with `role`.
    pub fn absorb<M>(&mut self, kernel: &Kernel<M>, role: impl Fn(NodeId) -> String) {
        let (steps, cursor) = kernel.steps_since(self.cursor);
        self.cursor = cursor;
        for s in steps {
            self.note(s, &role);
        }
    }

    fn note(&mut self, s: &WireStep, role: &impl Fn(NodeId) -> String) {
        let (from, to) = (role(s.from), role(s.to));
        let transport = transport_name(s.transport).to_string();
        let e = self
            .edges
            .entry((from.clone(), to.clone()))
            .or_insert_with(|| EdgeView {
                from: from.clone(),
                to: to.clone(),
                messages: 0,
                bytes: 0,
                last_t: 0,
                last_step: String::new(),
                transport: String::new(),
                steps: BTreeMap::new(),
            });
        e.messages += 1;
        e.bytes = e.bytes.saturating_add(u64::from(s.bytes));
        e.last_t = e.last_t.max(s.t);
        e.last_step = s.step.to_string();
        e.transport.clone_from(&transport);
        *e.steps.entry(s.step.to_string()).or_insert(0) += 1;
        self.flows
            .entry(s.flow.as_str().to_string())
            .or_default()
            .insert(s.run.0);
        if self.recent.len() >= RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(StepView {
            t: s.t,
            from,
            to,
            flow: s.flow.as_str().to_string(),
            step: s.step.to_string(),
            bytes: s.bytes,
            transport,
        });
    }

    /// The edges, in `(from, to)` order.
    #[must_use]
    pub fn edges(&self) -> Vec<EdgeView> {
        self.edges.values().cloned().collect()
    }

    /// The newest messages, oldest first.
    #[must_use]
    pub fn recent(&self) -> Vec<StepView> {
        self.recent.iter().cloned().collect()
    }

    /// Flow runs that have moved a message, by flow name.
    #[must_use]
    pub fn flows(&self) -> BTreeMap<String, u64> {
        self.flows
            .iter()
            .map(|(k, v)| (k.clone(), v.len() as u64))
            .collect()
    }
}

/// A transport's stable name.
#[must_use]
pub const fn transport_name(t: crate::net::Transport) -> &'static str {
    t.as_str()
}

/// An entity's queue as the kernel holds it.
#[must_use]
pub fn queue_of<M>(kernel: &Kernel<M>, node: NodeId, now: SimTime) -> Option<QueueView> {
    let q = kernel.queue(node)?;
    Some(QueueView {
        depth: kernel.depth_at(node, now) as u64,
        servers: q.servers(),
        served: q.served(),
        busy_ns: q.busy().as_nanos(),
        waited_ns: q.waiting().as_nanos(),
        backlog_ns: q.busy_until().saturating_sub(now),
        inbound: kernel.pending_to(node) as u64,
    })
}

/// An entity's cryptographic operations, by primitive and kind: `ecdsa-p256-sha256 sign`,
/// `falcon-512 verify`, `ml-dsa-44 keygen`. Kept apart by primitive so a hybrid
/// scheme's post-quantum half is visible beside its ECDSA half. The kernel counts the
/// uncosted AES and SHA-256 work under a nominal kind; here they read as blocks and hashes,
/// and an ECQV "sign" is the scalar multiplication it stands for.
#[must_use]
pub fn ops_of<M>(kernel: &Kernel<M>, node: NodeId) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    for ((n, primitive, kind), count) in &kernel.ops {
        if *n == node {
            let short = primitive.strip_prefix("primitive/").unwrap_or(primitive);
            let label = match short {
                "aes-128" => "aes-128 block".to_string(),
                "sha-256" => "sha-256 hash".to_string(),
                "ecqv-p256" if *kind == "sign" => "ec scalar-mult".to_string(),
                "ecqv-p256" => "ecqv-p256 reconstruct".to_string(),
                _ => format!("{short} {kind}"),
            };
            *out.entry(label).or_insert(0) += count;
        }
    }
    out
}

/// A skeleton entity with its kernel-side numbers filled.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn entity<M>(
    kernel: &Kernel<M>,
    now: SimTime,
    system: &'static str,
    id: &str,
    name: &str,
    tier: &'static str,
    role: &'static str,
    node: Option<NodeId>,
    online: bool,
) -> EntityView {
    EntityView {
        id: id.to_string(),
        name: name.to_string(),
        system,
        tier,
        online,
        node: node.map(NodeId::index),
        role,
        queue: node.and_then(|n| queue_of(kernel, n, now)),
        traffic: node.map(|n| kernel.traffic(n)).unwrap_or_default(),
        ops: node.map(|n| ops_of(kernel, n)).unwrap_or_default(),
        state: BTreeMap::new(),
    }
}
