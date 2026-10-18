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

use std::collections::{BTreeMap, VecDeque};

use serde_json::{Value, json};
use v2xw_proto::scms::pki_view::{self, CrlHead, Knowledge, PduRef};

/// SPDUs kept, newest last.
pub const PDU_CAP: usize = 200_000;
/// Certificates kept.
pub const CERT_CAP: usize = 400_000;

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
}

impl PkiStore {
    /// Whether any PKI record has been read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ctl.is_empty() && self.pdus.is_empty() && self.knowledge.is_empty()
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
        }
    }
}
