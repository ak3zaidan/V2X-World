//! The PKI view's inspect queries (WP-E1, 00-gap-analysis §3.3): `inspect.pdu`,
//! `inspect.cert` and `inspect.knowledge`, answered from the records a run publishes.
//!
//! The store reads `pki.state`, `pki.flow`, `node.credentials` and `pki.knowledge` as
//! they arrive (live) or as a recording replays them, and keeps what the three queries
//! need: every recorded SPDU by id, every certificate by HashedId8, every CRL version by
//! HashedId8, the CTL's roots over time and the newest knowledge record per entity. The
//! answers themselves are `v2xw_proto::scms::pki_view`'s, so a fixture, a recording and a
//! live run answer through one function and a test of the proto crate is a test of these.
//!
//! Every store is bounded ([`PDU_CAP`], [`CERT_CAP`]): the oldest entries go first, and a
//! query for one that went answers "unknown id" rather than something invented.
//!
//! The Studio's PKI view (WP-E2) reads the same store through three more queries, because
//! none of these channels is on the event stream: `inspect.trust` (the newest `pki.state`
//! at or before an instant, with which components carried traffic in the
//! [`ACTIVITY_WINDOW_NS`] before it), `inspect.flows` (the flow instances, newest first,
//! filtered by flow, device or provider) and `inspect.flow` (one instance's messages and
//! stage stamps in time order). Flow records are kept without SPDU bytes or plaintext:
//! those are `inspect.pdu`'s, which applies the per-entity view.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::{Value, json};
use v2xw_proto::scms::pki_view::{self, CrlHead, Knowledge, PduRef};

/// SPDUs kept, newest last.
pub const PDU_CAP: usize = 200_000;
/// Certificates kept.
pub const CERT_CAP: usize = 400_000;
/// `pki.state` snapshots kept for `inspect.trust`: five minutes at one a second.
pub const STATE_HISTORY: usize = 300;
/// Flow instances kept for `inspect.flows`.
pub const FLOW_CAP: usize = 20_000;
/// Records kept per flow instance (the pre-run `preload` instance holds every vehicle's
/// enrolment and provisioning SPDUs); the rest are counted as `dropped`.
pub const FLOW_RECORDS_CAP: usize = 4_000;
/// How far back `inspect.trust` looks for a component's traffic: two simulated seconds
/// (00 §3.2: a component lights while it has traffic in the last 2 s).
pub const ACTIVITY_WINDOW_NS: u64 = 2_000_000_000;

/// One flow instance, as `pki.flow` delivered it.
#[derive(Debug, Default)]
struct FlowLog {
    provider: String,
    flow: String,
    first_t: u64,
    last_t: u64,
    messages: u32,
    stages: u32,
    bytes: u64,
    pdus: u32,
    devices: BTreeSet<String>,
    last_stage: Option<String>,
    /// The records, without SPDU bytes or plaintext, in arrival order.
    records: Vec<Value>,
    dropped: u32,
}

/// What the PKI view's queries are answered from.
#[derive(Debug, Default)]
pub struct PkiStore {
    pdus: BTreeMap<String, PduRef>,
    pdu_order: VecDeque<String>,
    certs: BTreeMap<String, String>,
    cert_order: VecDeque<String>,
    crl_hex: BTreeMap<String, String>,
    /// CRL heads, newest last, as `pki.state` listed them.
    crls: Vec<CrlHead>,
    /// `(t, time32, roots)` of every `pki.state`, the newest few.
    ctl: VecDeque<(u64, u32, Vec<String>)>,
    knowledge: BTreeMap<String, Knowledge>,
    /// `(t, pki.state without its certificates and CRL bytes)`, newest last.
    states: VecDeque<(u64, Value)>,
    /// Flow instances by id.
    flows: BTreeMap<String, FlowLog>,
    /// Flow instance ids in the order they first appeared.
    flow_order: VecDeque<String>,
    /// Records it could not read, by channel.
    pub undecodable: BTreeMap<String, u64>,
}

/// One query.
#[derive(Debug, Clone, PartialEq)]
pub enum PkiQuery {
    /// `inspect.pdu {id, view}`.
    Pdu {
        /// `provider/pdu/<n>`.
        id: String,
        /// `ground-truth`, or a component id or role.
        view: String,
    },
    /// `inspect.cert {hashedid8, t_ns}`.
    Cert {
        /// HashedId8, hex.
        id8: String,
        /// The instant to validate at; the newest state's when absent.
        t_ns: Option<u64>,
    },
    /// `inspect.knowledge {entity, view}`.
    Knowledge {
        /// `provider/role/0`, or a role (the home provider's).
        entity: String,
        /// `ground-truth`, or the entity itself.
        view: String,
    },
    /// `inspect.trust {t_ns}`.
    Trust {
        /// The instant; the newest snapshot when absent.
        t_ns: Option<u64>,
    },
    /// `inspect.flows {flow, device, provider, limit, t_ns}`.
    Flows {
        /// Only instances of this flow (`provisioning`, `report`, ...).
        flow: Option<String>,
        /// Only instances a device took part in (`ee/1000` or `1000`).
        device: Option<String>,
        /// Only one provider's.
        provider: Option<String>,
        /// At most this many, newest first.
        limit: usize,
        /// Only what had started by this instant.
        t_ns: Option<u64>,
    },
    /// `inspect.flow {instance, t_ns}`.
    Flow {
        /// `provider/flow/run`.
        instance: String,
        /// Only records up to this instant.
        t_ns: Option<u64>,
    },
}

impl PkiQuery {
    /// What an unknown id names, for the error.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Pdu { .. } => "pdu",
            Self::Cert { .. } => "certificate",
            Self::Knowledge { .. } => "entity",
            Self::Trust { .. } => "pki.state",
            Self::Flows { .. } | Self::Flow { .. } => "flow",
        }
    }

    /// The id the query names, for the error.
    #[must_use]
    pub fn subject(&self) -> String {
        match self {
            Self::Pdu { id, .. } => id.clone(),
            Self::Cert { id8, .. } => id8.clone(),
            Self::Knowledge { entity, .. } => entity.clone(),
            Self::Trust { t_ns } => {
                format!(
                    "t={}",
                    t_ns.map_or_else(|| "newest".to_string(), |t| t.to_string())
                )
            }
            Self::Flows { flow, .. } => flow.clone().unwrap_or_else(|| "any flow".into()),
            Self::Flow { instance, .. } => instance.clone(),
        }
    }
}

/// A device id as flow records spell it (`ee/<id>`), from either spelling.
fn device_id(d: &str) -> String {
    if d.starts_with("ee/") {
        d.to_string()
    } else {
        format!("ee/{d}")
    }
}

/// One component's message and byte counts in a `pki.state` value.
fn traffic_of(c: &Value) -> (u64, u64) {
    let t = &c["traffic"];
    let n = |k: &str| t[k].as_u64().unwrap_or(0);
    (n("sent") + n("received"), n("bytes_in") + n("bytes_out"))
}

/// Every component of a `pki.state` value, governance first: `(id, component)`.
fn components_of(state: &Value) -> Vec<(&str, &Value)> {
    let providers = state["providers"].as_array().into_iter().flatten();
    std::iter::once(&state["governance"])
        .chain(providers.map(|p| &p["components"]))
        .flat_map(|list| list.as_array().into_iter().flatten())
        .filter_map(|c| c["id"].as_str().map(|id| (id, c)))
        .collect()
}

impl PkiStore {
    /// Whether any PKI record has been read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ctl.is_empty()
            && self.pdus.is_empty()
            && self.knowledge.is_empty()
            && self.flows.is_empty()
    }

    fn put_cert(&mut self, id8: String, hex: String) {
        if self.certs.insert(id8.clone(), hex).is_none() {
            self.cert_order.push_back(id8);
            while self.cert_order.len() > CERT_CAP {
                if let Some(old) = self.cert_order.pop_front() {
                    self.certs.remove(&old);
                }
            }
        }
    }

    /// Reads one record, if it is on a PKI channel. Returns whether it was.
    pub fn absorb(&mut self, channel: &str, json: &[u8]) -> bool {
        let ok = match channel {
            "pki.state" => serde_json::from_slice::<Value>(json)
                .map(|v| self.absorb_state(&v))
                .is_ok(),
            "pki.flow" => serde_json::from_slice::<pki_view::PkiFlow>(json)
                .map(|f| {
                    self.absorb_flow(&f);
                    for p in f.pdus {
                        let id = p.id.clone();
                        if self.pdus.insert(id.clone(), p).is_none() {
                            self.pdu_order.push_back(id);
                        }
                    }
                    while self.pdu_order.len() > PDU_CAP {
                        if let Some(old) = self.pdu_order.pop_front() {
                            self.pdus.remove(&old);
                        }
                    }
                })
                .is_ok(),
            "node.credentials" => serde_json::from_slice::<pki_view::NodeCredentials>(json)
                .map(|c| {
                    for (id, hex) in c.certificates {
                        self.put_cert(id, hex);
                    }
                })
                .is_ok(),
            "pki.knowledge" => serde_json::from_slice::<Knowledge>(json)
                .map(|k| {
                    self.knowledge.insert(k.entity.clone(), k);
                })
                .is_ok(),
            _ => return false,
        };
        if !ok {
            *self.undecodable.entry(channel.to_string()).or_insert(0) += 1;
        }
        true
    }

    /// Files one `pki.flow` record under its instance, without bytes or plaintext.
    fn absorb_flow(&mut self, f: &pki_view::PkiFlow) {
        if !self.flows.contains_key(&f.instance) {
            self.flow_order.push_back(f.instance.clone());
            while self.flow_order.len() > FLOW_CAP {
                if let Some(old) = self.flow_order.pop_front() {
                    self.flows.remove(&old);
                }
            }
        }
        let log = self
            .flows
            .entry(f.instance.clone())
            .or_insert_with(|| FlowLog {
                provider: f.provider.clone(),
                flow: f.flow.clone(),
                first_t: f.t,
                last_t: f.t,
                ..FlowLog::default()
            });
        log.first_t = log.first_t.min(f.t);
        log.last_t = log.last_t.max(f.t);
        match f.kind.as_str() {
            "msg" => {
                log.messages += 1;
                log.bytes += u64::from(f.bytes.unwrap_or(0));
            }
            "stage" => {
                log.stages += 1;
                log.last_stage = Some(f.name.clone());
            }
            _ => {}
        }
        log.pdus += u32::try_from(f.pdus.len()).unwrap_or(u32::MAX);
        for end in [f.from.as_deref(), f.to.as_deref()].into_iter().flatten() {
            if end.starts_with("ee/") {
                log.devices.insert(end.to_string());
            }
        }
        if log.records.len() >= FLOW_RECORDS_CAP {
            log.dropped += 1;
            return;
        }
        let mut lean = f.clone();
        for p in &mut lean.pdus {
            p.hex = None;
            if let Some(o) = p.opened.as_mut() {
                o.hex = None;
            }
        }
        if let Ok(v) = serde_json::to_value(&lean) {
            log.records.push(v);
        }
    }

    fn absorb_state(&mut self, v: &Value) {
        if let Some(certs) = v.get("certificates").and_then(Value::as_object) {
            for (id, hex) in certs {
                if let Some(h) = hex.as_str() {
                    self.put_cert(id.clone(), h.to_string());
                }
            }
        }
        if let Some(crls) = v.get("crls").and_then(Value::as_array) {
            for c in crls {
                if let Ok(head) = serde_json::from_value::<CrlHead>(c.clone()) {
                    if let Some(h) = head.hex.clone() {
                        self.crl_hex.insert(head.id8.clone(), h);
                    }
                    if !self.crls.iter().any(|x| x.id8 == head.id8) {
                        self.crls.push(head);
                    }
                }
            }
        }
        let roots: Vec<String> = v["ctl"]["roots"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|r| r.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let t = v["t"].as_u64().unwrap_or(0);
        let time32 = v["time32"]
            .as_u64()
            .and_then(|x| u32::try_from(x).ok())
            .unwrap_or(0);
        self.ctl.push_back((t, time32, roots));
        while self.ctl.len() > 64 {
            self.ctl.pop_front();
        }
        let mut lean = v.clone();
        if let Some(o) = lean.as_object_mut() {
            o.remove("certificates");
        }
        for c in lean["crls"].as_array_mut().into_iter().flatten() {
            if let Some(o) = c.as_object_mut() {
                o.remove("hex");
            }
        }
        self.states.push_back((t, lean));
        while self.states.len() > STATE_HISTORY {
            self.states.pop_front();
        }
    }

    /// `inspect.trust`: the newest `pki.state` at or before `t_ns` (the oldest when every
    /// one is later), and each component's traffic over the [`ACTIVITY_WINDOW_NS`] before
    /// it, against the newest earlier snapshot at least that old (or the oldest earlier
    /// one when the history is shorter; none when it is the first).
    fn trust(&self, t_ns: Option<u64>) -> Option<Value> {
        let at = match t_ns {
            Some(t) => self.states.iter().rposition(|(s, _)| *s <= t).unwrap_or(0),
            None => self.states.len().checked_sub(1)?,
        };
        let (t, state) = self.states.get(at)?;
        let base = self
            .states
            .iter()
            .take(at)
            .rev()
            .find(|(s, _)| s.saturating_add(ACTIVITY_WINDOW_NS) <= *t)
            .or_else(|| self.states.iter().take(at).next());
        let mut activity = serde_json::Map::new();
        if let Some((_, before)) = base {
            let prior: BTreeMap<&str, (u64, u64)> = components_of(before)
                .into_iter()
                .map(|(id, c)| (id, traffic_of(c)))
                .collect();
            for (id, c) in components_of(state) {
                let (m, b) = traffic_of(c);
                let (m0, b0) = prior.get(id).copied().unwrap_or((0, 0));
                if m > m0 {
                    activity.insert(
                        id.to_string(),
                        json!({"messages": m - m0, "bytes": b.saturating_sub(b0)}),
                    );
                }
            }
        }
        Some(json!({
            "t_ns": t,
            "state": state,
            "window_ns": base.map(|(s, _)| t - s),
            "activity": activity,
            "certificates_known": self.certs.len(),
            "history": self.states.len(),
        }))
    }

    /// One flow instance's summary row.
    fn flow_row(id: &str, log: &FlowLog) -> Value {
        json!({
            "instance": id,
            "provider": log.provider,
            "flow": log.flow,
            "first_t": log.first_t,
            "last_t": log.last_t,
            "messages": log.messages,
            "stages": log.stages,
            "bytes": log.bytes,
            "pdus": log.pdus,
            "devices": log.devices.iter().take(8).collect::<Vec<_>>(),
            "device_count": log.devices.len(),
            "last_stage": log.last_stage,
        })
    }

    /// `inspect.flows`: matching instances, newest (latest start) first, and the count of
    /// every flow under the device and provider filters (for the type filter's labels).
    fn flow_list(
        &self,
        flow: Option<&str>,
        device: Option<&str>,
        provider: Option<&str>,
        limit: usize,
        t_ns: Option<u64>,
    ) -> Value {
        let device = device.map(device_id);
        let mut by_flow: BTreeMap<&str, u64> = BTreeMap::new();
        let mut rows: Vec<(&str, &FlowLog)> = Vec::new();
        for (id, log) in &self.flows {
            if t_ns.is_some_and(|t| log.first_t > t)
                || provider.is_some_and(|p| p != log.provider)
                || device.as_ref().is_some_and(|d| !log.devices.contains(d))
            {
                continue;
            }
            *by_flow.entry(log.flow.as_str()).or_insert(0) += 1;
            if flow.is_some_and(|f| f != log.flow) {
                continue;
            }
            rows.push((id.as_str(), log));
        }
        let total = rows.len();
        rows.sort_by(|a, b| b.1.first_t.cmp(&a.1.first_t).then(a.0.cmp(b.0)));
        let instances: Vec<Value> = rows
            .iter()
            .take(limit)
            .map(|(id, log)| Self::flow_row(id, log))
            .collect();
        json!({"instances": instances, "total": total, "flows": by_flow})
    }

    /// `inspect.flow`: one instance's records in time order (ties in arrival order).
    fn flow_detail(&self, instance: &str, t_ns: Option<u64>) -> Option<Value> {
        let log = self.flows.get(instance)?;
        let mut records: Vec<&Value> = log
            .records
            .iter()
            .filter(|r| t_ns.is_none_or(|t| r["t"].as_u64().unwrap_or(0) <= t))
            .collect();
        records.sort_by_key(|r| r["t"].as_u64().unwrap_or(0));
        let mut row = Self::flow_row(instance, log);
        row["records"] = json!(records);
        row["dropped"] = json!(log.dropped);
        Some(row)
    }

    /// The CTL and clock as of `t_ns` (the newest at or before it, else the oldest).
    fn ctl_at(&self, t_ns: Option<u64>) -> Option<&(u64, u32, Vec<String>)> {
        match t_ns {
            Some(t) => self
                .ctl
                .iter()
                .rev()
                .find(|(at, ..)| *at <= t)
                .or_else(|| self.ctl.front()),
            None => self.ctl.back(),
        }
    }

    /// Answers one query; `None` for an id the store does not hold.
    #[must_use]
    pub fn answer(&self, q: &PkiQuery) -> Option<Value> {
        match q {
            PkiQuery::Pdu { id, view } => self.pdus.get(id).map(|p| pki_view::inspect_pdu(p, view)),
            PkiQuery::Cert { id8, t_ns } => {
                let (t, time32, roots) = self.ctl_at(*t_ns)?;
                // A t_ns between snapshots validates at that instant on the same clock.
                let time32 = match t_ns {
                    Some(at) if at > t => {
                        time32.saturating_add(u32::try_from((at - t) / 1_000_000_000).unwrap_or(0))
                    }
                    _ => *time32,
                };
                let crls: Vec<CrlHead> = self
                    .crls
                    .iter()
                    .filter(|c| t_ns.is_none_or(|at| c.issued_at <= at))
                    .cloned()
                    .collect();
                let mut v =
                    pki_view::inspect_cert(id8, &self.certs, roots, &crls, &self.crl_hex, time32)?;
                v["t_ns"] = json!(t_ns.unwrap_or(*t));
                Some(v)
            }
            PkiQuery::Knowledge { entity, view } => {
                let k = self.knowledge.get(entity).or_else(|| {
                    self.knowledge
                        .values()
                        .find(|k| k.role == *entity && k.entity.starts_with("provider-a/"))
                        .or_else(|| self.knowledge.values().find(|k| k.role == *entity))
                })?;
                Some(pki_view::inspect_knowledge(k, view))
            }
            PkiQuery::Trust { t_ns } => self.trust(*t_ns),
            PkiQuery::Flows {
                flow,
                device,
                provider,
                limit,
                t_ns,
            } => Some(self.flow_list(
                flow.as_deref(),
                device.as_deref(),
                provider.as_deref(),
                *limit,
                *t_ns,
            )),
            PkiQuery::Flow { instance, t_ns } => self.flow_detail(instance, *t_ns),
        }
    }

    /// The ids it can answer for, for an "unknown id" error's hint.
    #[must_use]
    pub fn known(&self, q: &PkiQuery) -> String {
        match q {
            PkiQuery::Pdu { .. } => format!("{} PDUs recorded", self.pdus.len()),
            PkiQuery::Cert { .. } => format!("{} certificates recorded", self.certs.len()),
            PkiQuery::Knowledge { .. } => {
                let ids: Vec<&str> = self.knowledge.keys().map(String::as_str).collect();
                format!("entities: {}", ids.join(", "))
            }
            PkiQuery::Trust { .. } => {
                format!("{} pki.state snapshots recorded", self.states.len())
            }
            PkiQuery::Flows { .. } | PkiQuery::Flow { .. } => {
                format!("{} flow instances recorded", self.flows.len())
            }
        }
    }
}
