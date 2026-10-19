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
//!
//! `inspect.trust` also lists the messages themselves that crossed a link in its activity
//! window (`messages`: sender, receiver, name, flow, instance, bytes, time), so the
//! Studio's topology can draw each one along its link as it happens. They come from a
//! bounded log of the newest [`RECENT_CAP`] messages; an answer lists at most
//! [`RECENT_PER_ANSWER`] of them (the newest) and counts the rest in `messages_omitted`,
//! and `messages_complete` is false when the log no longer reaches back to the window's
//! start (an instant long past after a backward seek).

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
/// Messages kept for `inspect.trust`'s `messages`, newest last.
pub const RECENT_CAP: usize = 8_192;
/// Messages one `inspect.trust` answer lists at most (the newest in its window).
pub const RECENT_PER_ANSWER: usize = 400;

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
    /// The newest [`RECENT_CAP`] messages (`kind: "msg"` flow records, lean), in arrival
    /// order, for `inspect.trust`'s `messages`.
    recent: VecDeque<Value>,
    /// Whether a message was ever evicted from `recent`.
    recent_evicted: bool,
    /// The newest `pki.state`, without its certificates (WP-E3).
    state: Option<Value>,
    /// Per vehicle, its newest `node.credentials` and pseudonym history (WP-E3).
    vehicles: BTreeMap<String, Vehicle>,
    /// Which vehicle holds each certificate, from the grids (ground truth).
    cert_owner: BTreeMap<String, String>,
    /// Ground truth: each vehicle certificate a published CRL revokes, with the CRLs.
    revoked_by: BTreeMap<String, BTreeSet<String>>,
    /// Per node, the newest `node.security` facts the Vehicles table shows.
    security: BTreeMap<u64, Value>,
    /// Every `pki.case` transition, per case, oldest case first.
    cases: BTreeMap<String, Vec<pki_view::PkiCase>>,
    case_order: VecDeque<String>,
    /// The messages and stage stamps of every linkage-resolution and CRL-issuance
    /// instance, per instance (the investigation steps of a case).
    steps: BTreeMap<String, Vec<Value>>,
    step_order: VecDeque<String>,
    /// Records it could not read, by channel.
    pub undecodable: BTreeMap<String, u64>,
}

/// Cases kept.
pub const CASE_CAP: usize = 8_192;
/// Flow instances whose steps are kept.
pub const STEP_CAP: usize = 16_384;

/// One vehicle as the PKI view's Vehicles tab shows it (WP-E3).
#[derive(Debug, Clone)]
struct Vehicle {
    /// The newest `node.credentials`, its grid reduced to the cells the vehicle holds
    /// (`not-yet-available` cells dropped; the grid's bounds are kept) and its
    /// certificates moved to the store.
    last: pki_view::NodeCredentials,
    /// The grid's i-periods, first and last.
    i_min: u32,
    i_max: u32,
    /// Cells in the full grid.
    cells: usize,
    /// Each pseudonym it signed with, with the first record that showed it active.
    pseudonyms: Vec<(String, u64)>,
    /// `node.credentials` records read.
    records: u64,
    /// First record's time.
    first_t: u64,
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
    /// `inspect.pki {topic: "vehicles"}` (WP-E3): every vehicle's pool, enrolment and
    /// provisioning state.
    Vehicles {
        /// `ground-truth`, or one vehicle (`ee/<n>`), which sees only itself.
        view: String,
    },
    /// `inspect.pki {topic: "vehicle", device}`: one vehicle's i × j grid and pseudonym
    /// history.
    Vehicle {
        /// `ee/<n>`, or the engine node id in decimal.
        device: String,
        /// As for [`PkiQuery::Vehicles`].
        view: String,
    },
    /// `inspect.pki {topic: "cases"}`: every misbehaviour case with its stages, its
    /// investigation steps and who learned what.
    Cases {
        /// `ground-truth`, or a component (`provider/role/n`), which sees the steps it
        /// took part in and no identity.
        view: String,
    },
    /// `inspect.pki {topic: "crls"}`: every CRL version with its SLA clock, and the CTL
    /// with its history.
    Crls,
    /// `inspect.pki {topic: "crl", hashedid8}`: one CRL's entries as the real decoder
    /// reads them, and in ground truth the run's certificates each entry revokes.
    Crl {
        /// The CRL's HashedId8, hex.
        id8: String,
        /// `ground-truth`, or anything else (no matches).
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
            Self::Vehicle { .. } => "device",
            Self::Crl { .. } => "crl",
            Self::Vehicles { .. } | Self::Cases { .. } | Self::Crls => "pki topic",
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
            Self::Crl { id8, .. } => id8.clone(),
            Self::Vehicle { device, .. } => device.clone(),
            Self::Vehicles { .. } | Self::Cases { .. } | Self::Crls => String::new(),
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

/// The ground-truth view's name.
pub const GROUND_TRUTH: &str = "ground-truth";

impl PkiStore {
    /// Whether any PKI record has been read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ctl.is_empty()
            && self.pdus.is_empty()
            && self.knowledge.is_empty()
            && self.flows.is_empty()
            && self.vehicles.is_empty()
            && self.cases.is_empty()
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
                    self.absorb_step(&f);
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
                .map(|c| self.absorb_credentials(c))
                .is_ok(),
            "pki.case" => serde_json::from_slice::<pki_view::PkiCase>(json)
                .map(|c| {
                    if !self.cases.contains_key(&c.case) {
                        self.case_order.push_back(c.case.clone());
                        while self.case_order.len() > CASE_CAP {
                            if let Some(old) = self.case_order.pop_front() {
                                self.cases.remove(&old);
                            }
                        }
                    }
                    self.cases.entry(c.case.clone()).or_default().push(c);
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
                self.recent.push_back(json!({
                    "id": f.id,
                    "t": f.t,
                    "from": f.from,
                    "to": f.to,
                    "name": f.name,
                    "flow": f.flow,
                    "instance": f.instance,
                    "provider": f.provider,
                    "bytes": f.bytes,
                }));
                while self.recent.len() > RECENT_CAP {
                    self.recent.pop_front();
                    self.recent_evicted = true;
                }
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
                        if !self.crl_hex.contains_key(&head.id8) {
                            self.match_crl(&head.id8, &h);
                        }
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
        let mut kept = v.clone();
        if let Some(o) = kept.as_object_mut() {
            o.remove("certificates");
        }
        self.state = Some(kept);
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
        // The messages of the same window: after the base snapshot (or, for the first, in
        // the ACTIVITY_WINDOW_NS before it) and at or before this one, in time order.
        let lo = base.map_or_else(|| t.saturating_sub(ACTIVITY_WINDOW_NS), |(s, _)| *s);
        let mut messages: Vec<&Value> = self
            .recent
            .iter()
            .filter(|m| {
                let mt = m["t"].as_u64().unwrap_or(0);
                mt > lo && mt <= *t
            })
            .collect();
        messages.sort_by_key(|m| m["t"].as_u64().unwrap_or(0));
        let omitted = messages.len().saturating_sub(RECENT_PER_ANSWER);
        let messages = &messages[omitted..];
        let complete = !self.recent_evicted
            || self
                .recent
                .front()
                .is_some_and(|m| m["t"].as_u64().unwrap_or(0) <= lo);
        Some(json!({
            "t_ns": t,
            "state": state,
            "window_ns": base.map(|(s, _)| t - s),
            "activity": activity,
            "messages": messages,
            "messages_omitted": omitted,
            "messages_complete": complete,
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

    /// Ground truth: which held certificates the CRL `id8` revokes.
    fn match_crl(&mut self, id8: &str, crl_hex: &str) {
        let held = self
            .cert_owner
            .keys()
            .filter_map(|c| self.certs.get(c).map(|h| (c.as_str(), h.as_str())));
        for c in pki_view::crl_matches(crl_hex, held) {
            self.revoked_by
                .entry(c)
                .or_default()
                .insert(id8.to_string());
        }
    }

    fn absorb_credentials(&mut self, mut c: pki_view::NodeCredentials) {
        let fresh: Vec<(String, String)> =
            std::mem::take(&mut c.certificates).into_iter().collect();
        for (id, hex) in &fresh {
            self.put_cert(id.clone(), hex.clone());
        }
        for cell in &c.cells {
            if let Some(id) = &cell.id8 {
                self.cert_owner.insert(id.clone(), c.device.clone());
            }
        }
        if let Some(e) = &c.enrolment {
            self.cert_owner.insert(e.clone(), c.device.clone());
        }
        // A certificate first seen now against every CRL already published.
        let crls: Vec<(String, String)> = self
            .crl_hex
            .iter()
            .map(|(a, b)| (a.clone(), b.clone()))
            .collect();
        for (crl, h) in &crls {
            for m in pki_view::crl_matches(h, fresh.iter().map(|(a, b)| (a.as_str(), b.as_str()))) {
                self.revoked_by.entry(m).or_default().insert(crl.clone());
            }
        }
        let i_min = c.cells.iter().map(|x| x.i).min().unwrap_or(c.current_i);
        let i_max = c.cells.iter().map(|x| x.i).max().unwrap_or(c.current_i);
        let cells = c.cells.len();
        c.cells
            .retain(|x| x.state != "not-yet-available" || x.id8.is_some());
        let v = self
            .vehicles
            .entry(c.device.clone())
            .or_insert_with(|| Vehicle {
                last: c.clone(),
                i_min,
                i_max,
                cells,
                pseudonyms: Vec::new(),
                records: 0,
                first_t: c.t,
            });
        if let Some(a) = &c.active
            && v.pseudonyms.last().is_none_or(|(p, _)| p != a)
        {
            v.pseudonyms.push((a.clone(), c.t));
        }
        v.records += 1;
        v.i_min = i_min;
        v.i_max = i_max;
        v.cells = cells;
        v.last = c;
    }

    /// The facts of a `node.security` row the Vehicles table shows (next top-up, the
    /// CRL it installed, its backend link).
    pub fn absorb_security(&mut self, row: &Value) {
        let Some(node) = row["node"].as_u64() else {
            return;
        };
        let mut keep = serde_json::Map::new();
        for k in [
            "t",
            "next_topup",
            "topup_in_flight",
            "crl_version",
            "crl_entries",
            "self_revoked",
            "link",
            "link_up",
            "pool_valid",
            "changes",
            "pseudonym",
        ] {
            if let Some(x) = row.get(k) {
                keep.insert(k.to_string(), x.clone());
            }
        }
        self.security.insert(node, Value::Object(keep));
    }

    fn absorb_step(&mut self, f: &pki_view::PkiFlow) {
        if f.kind == "pdu" || !matches!(f.flow.as_str(), "linkage-resolution" | "crl-issuance") {
            return;
        }
        if !self.steps.contains_key(&f.instance) {
            self.step_order.push_back(f.instance.clone());
            while self.step_order.len() > STEP_CAP {
                if let Some(old) = self.step_order.pop_front() {
                    self.steps.remove(&old);
                }
            }
        }
        let pdus: Vec<Value> = f
            .pdus
            .iter()
            .map(|p| {
                json!({"id": p.id, "type": p.kind, "len": p.len, "sealed_to": p.sealed_to,
                            "opened_by": p.opened.as_ref().map(|o| o.by.clone())})
            })
            .collect();
        self.steps.entry(f.instance.clone()).or_default().push(json!({
            "kind": f.kind, "id": f.id, "t": f.t, "flow": f.flow, "name": f.name,
            "from": f.from, "to": f.to, "bytes": f.bytes, "transport": f.transport, "pdus": pdus,
        }));
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
            PkiQuery::Vehicles { view } => {
                let gt = view == GROUND_TRUTH;
                let rows: Vec<Value> = self
                    .vehicles
                    .values()
                    .filter(|v| gt || v.last.device == *view)
                    .map(|v| self.vehicle_row(v, gt))
                    .collect();
                Some(json!({
                    "t_ns": self.now().0, "time32": self.now().1, "view": view,
                    "vehicles": rows,
                }))
            }
            PkiQuery::Vehicle { device, view } => {
                let v = self
                    .vehicles
                    .values()
                    .find(|v| v.last.device == *device || v.last.node.to_string() == *device)?;
                let gt = view == GROUND_TRUTH;
                if !gt && v.last.device != *view {
                    return None;
                }
                let mut row = self.vehicle_row(v, gt);
                row["grid"] = json!({"i_min": v.i_min, "i_max": v.i_max, "jmax": v.last.jmax,
                                     "cells": v.cells, "held": v.last.cells});
                row["pseudonyms"] = v
                    .pseudonyms
                    .iter()
                    .map(|(id, t)| json!({"id8": id, "since": t}))
                    .collect();
                row["history"] = json!(v.last.history);
                row["records"] = json!(v.records);
                row["first_t"] = json!(v.first_t);
                row["view"] = json!(view);
                if gt {
                    let rev: BTreeMap<&String, &BTreeSet<String>> = v
                        .last
                        .cells
                        .iter()
                        .filter_map(|c| c.id8.as_ref())
                        .filter_map(|id| self.revoked_by.get(id).map(|s| (id, s)))
                        .collect();
                    row["truth"]["revoked_by"] = json!(rev);
                }
                Some(row)
            }
            PkiQuery::Cases { view } => Some(self.cases_answer(view)),
            PkiQuery::Crls => Some(self.crls_answer()),
            PkiQuery::Crl { id8, view } => {
                let head = self.crls.iter().find(|c| c.id8 == *id8)?;
                let hex = self.crl_hex.get(id8)?;
                let gt = view == GROUND_TRUTH;
                let cand: Vec<(&str, &str)> = if gt {
                    self.revoked_by
                        .iter()
                        .filter(|(_, crls)| crls.contains(id8))
                        .filter_map(|(c, _)| self.certs.get(c).map(|h| (c.as_str(), h.as_str())))
                        .collect()
                } else {
                    Vec::new()
                };
                let mut v = pki_view::crl_entries(hex, &cand)?;
                let mut head = serde_json::to_value(head).ok()?;
                if let Some(o) = head.as_object_mut() {
                    o.remove("hex");
                }
                v["head"] = head;
                v["view"] = json!(view);
                if gt {
                    let owners: BTreeMap<&str, Value> =
                        cand.iter().map(|(c, _)| (*c, self.owner_of(c))).collect();
                    v["owners"] = json!(owners);
                } else {
                    for e in v["linkage"].as_array_mut().into_iter().flatten() {
                        e.as_object_mut().map(|o| o.remove("matches"));
                    }
                    for e in v["hash"].as_array_mut().into_iter().flatten() {
                        e.as_object_mut().map(|o| o.remove("matches"));
                    }
                }
                Some(v)
            }
        }
    }

    /// The newest `pki.state`'s `(t, time32)`.
    fn now(&self) -> (u64, u32) {
        self.ctl.back().map_or((0, 0), |(t, s, _)| (*t, *s))
    }

    /// Who holds certificate `id8` and where in its grid (ground truth).
    fn owner_of(&self, id8: &str) -> Value {
        let Some(dev) = self.cert_owner.get(id8) else {
            return Value::Null;
        };
        let v = self.vehicles.get(dev);
        let cell = v.and_then(|v| v.last.cells.iter().find(|c| c.id8.as_deref() == Some(id8)));
        json!({
            "device": dev,
            "node": v.map(|v| v.last.node),
            "provider": v.map(|v| v.last.provider.clone()),
            "i": cell.map(|c| c.i),
            "j": cell.map(|c| c.j),
            "enrolment": v.and_then(|v| v.last.enrolment.as_deref()) == Some(id8),
        })
    }

    /// One row of the Vehicles table.
    fn vehicle_row(&self, v: &Vehicle, gt: bool) -> Value {
        let c = &v.last;
        let mut by_state: BTreeMap<&str, u32> = BTreeMap::new();
        for cell in &c.cells {
            *by_state.entry(cell.state.as_str()).or_insert(0) += 1;
        }
        let usable = ["active", "valid", "downloaded"]
            .iter()
            .map(|s| by_state.get(s).copied().unwrap_or(0))
            .sum::<u32>();
        let last_period = c
            .cells
            .iter()
            .filter(|x| matches!(x.state.as_str(), "active" | "valid" | "downloaded"))
            .map(|x| x.i)
            .max();
        let mut row = json!({
            "device": c.device, "node": c.node, "provider": c.provider, "t": c.t,
            "enrolled": c.enrolled, "enrolment": c.enrolment,
            "provisioned": usable > 0,
            "active": c.active, "current_i": c.current_i, "jmax": c.jmax,
            "pool": by_state, "usable": usable, "last_period": last_period,
            "periods_ahead": last_period.map(|p| p.saturating_sub(c.current_i)),
            "pseudonym_changes": v.pseudonyms.len().saturating_sub(1),
            "used": c.history.len(),
            "ctl_sequence": c.ctl_sequence, "revoked": c.revoked,
            "security": self.security.get(&u64::from(c.node)),
        });
        if gt {
            let mut crls: BTreeSet<&String> = BTreeSet::new();
            let mut n = 0;
            for id in c.cells.iter().filter_map(|x| x.id8.as_ref()) {
                if let Some(s) = self.revoked_by.get(id) {
                    n += 1;
                    crls.extend(s.iter());
                }
            }
            row["truth"] = json!({"revoked_certs": n, "revoked_by_crls": crls});
        }
        row
    }

    /// Every CRL version with its SLA clock at the newest state, the CTL and its history,
    /// and each provider's publication rule.
    fn crls_answer(&self) -> Value {
        let (t, now32) = self.now();
        let heads: Vec<Value> = self
            .crls
            .iter()
            .map(|h| {
                let mut v = serde_json::to_value(h).unwrap_or(Value::Null);
                if let Some(o) = v.as_object_mut() {
                    o.remove("hex");
                    o.insert(
                        "sla_now_s".into(),
                        json!(i64::from(h.next_crl) - i64::from(now32)),
                    );
                    o.insert("decodable".into(), json!(self.crl_hex.contains_key(&h.id8)));
                }
                v
            })
            .collect();
        let st = self.state.as_ref();
        let providers: Vec<Value> = st
            .and_then(|s| s["providers"].as_array())
            .map(|a| {
                a.iter()
                    .map(|p| {
                        json!({"label": p["label"], "on_ctl": p["on_ctl"], "home": p["home"],
                               "crl_publication": p["crl_publication"], "ma_mode": p["ma_mode"],
                               "ma_review_s": p["ma_review_s"], "devices": p["devices"],
                               "region": p["region"], "fleet_share": p["fleet_share"]})
                    })
                    .collect()
            })
            .unwrap_or_default();
        json!({
            "t_ns": t, "time32": now32,
            "crls": heads,
            "ctl": st.map(|s| s["ctl"].clone()),
            "providers": providers,
            "edition": st.map(|s| s["edition"].clone()),
            "assumptions": st.map(|s| s["assumptions"].clone()),
        })
    }

    /// The case board as `view` sees it.
    fn cases_answer(&self, view: &str) -> Value {
        let gt = view == GROUND_TRUTH;
        let truth: BTreeMap<&str, &Value> = self
            .knowledge
            .values()
            .filter(|k| k.role == "ma")
            .filter_map(|k| k.truth["cases"].as_array())
            .flatten()
            .filter_map(|c| c["case"].as_str().map(|id| (id, c)))
            .collect();
        let cases: Vec<Value> = self
            .case_order
            .iter()
            .filter_map(|id| self.cases.get(id).map(|s| (id, s)))
            .filter_map(|(id, stages)| {
                let first = stages.first()?;
                let crl_instance = stages
                    .iter()
                    .find_map(|s| s.detail["crl_instance"].as_str().map(str::to_string));
                let owns = gt
                    || view
                        .strip_prefix(&format!("{}/ma/", first.provider))
                        .is_some();
                let mut steps: Vec<Value> = [Some(first.instance.clone()), crl_instance.clone()]
                    .iter()
                    .flatten()
                    .filter_map(|i| self.steps.get(i))
                    .flatten()
                    .filter(|s| {
                        gt || s["from"].as_str() == Some(view) || s["to"].as_str() == Some(view)
                    })
                    .cloned()
                    .collect();
                steps.sort_by_key(|s| s["t"].as_u64().unwrap_or(0));
                if !owns && steps.is_empty() {
                    return None;
                }
                let mut v = json!({
                    "case": id, "provider": first.provider, "instance": first.instance,
                    "crl_instance": crl_instance,
                    "stages": if owns { json!(stages) } else { json!([]) },
                    "steps": steps,
                });
                if gt && let Some(t) = truth.get(id.as_str()) {
                    let party = |d: &Value| {
                        let dev = d.as_str().unwrap_or("");
                        let veh = self.vehicles.get(dev);
                        json!({"device": dev, "node": veh.map(|x| x.last.node),
                               "provider": veh.map(|x| x.last.provider.clone())})
                    };
                    let accused: Vec<Value> = t["accused"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(party)
                        .collect();
                    let reporters: Vec<Value> = t["reporters"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|r| {
                            let mut p = party(&r["device"]);
                            p["reports"] = r["reports"].clone();
                            p
                        })
                        .collect();
                    v["truth"] = json!({"accused": accused, "reporters": reporters});
                }
                Some(v)
            })
            .collect();
        json!({"t_ns": self.now().0, "view": view, "cases": cases})
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
            PkiQuery::Vehicles { .. } | PkiQuery::Vehicle { .. } => {
                format!("{} vehicles recorded", self.vehicles.len())
            }
            PkiQuery::Cases { .. } => format!("{} cases recorded", self.cases.len()),
            PkiQuery::Crls | PkiQuery::Crl { .. } => {
                format!("{} CRL versions recorded", self.crls.len())
            }
        }
    }
}
