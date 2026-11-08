//! The DEN basic service [EN 302 637-3 V1.3.1] as a state machine: trigger, update and
//! terminate at the originator (§6.1.2-6.1.4), DENM repetition (§6.1.3, `repetitionInterval`
//! and `repetitionDuration`), the receiver's new/update/repetition/termination decision
//! (§8.1.2), and **keep-alive forwarding** (KAF, §8.2.2): a receiver inside the
//! relevance area that stops hearing a still-valid DENM for `transmissionInterval` plus a
//! jitter retransmits it, so a hazard keeps reaching vehicles that arrive after its
//! originator stopped repeating — or drove away.
//!
//! The service never reads a clock: `now_ms` comes in, timers go out as
//! [`DenAction::ArmTimer`], and stale timers are recognised by their due time.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::area::Area;

/// `validityDuration` default: 600 s [EN 302 637-3 V1.3.1 Annex A, DE_ValidityDuration].
pub const DEFAULT_VALIDITY_MS: u64 = 600_000;

/// A DENM's identity [§6.1.1]: the originating station and its sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActionId {
    /// Originating station id.
    pub station: u32,
    /// Sequence number.
    pub sn: u16,
}

impl ActionId {
    /// Pack into a 64-bit payload tag.
    pub fn tag(self) -> u64 {
        (u64::from(self.station) << 16) | u64::from(self.sn)
    }

    /// Unpack a payload tag.
    pub fn from_tag(tag: u64) -> Self {
        ActionId {
            station: (tag >> 16) as u32,
            sn: (tag & 0xFFFF) as u16,
        }
    }
}

/// How a DENM ends [§6.1.4].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Termination {
    /// The originator cancels its own event.
    Cancel,
    /// Another station negates it.
    Negate,
}

/// The fields of a DENM the service acts on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Denm {
    /// Identity.
    pub action: ActionId,
    /// The relevance (and GeoBroadcast destination) area.
    pub area: Area,
    /// When the event was detected, ms.
    pub detection_ms: u64,
    /// The reference time: set at trigger, advanced by every update, ms.
    pub reference_ms: u64,
    /// `validityDuration`, ms.
    pub validity_ms: u64,
    /// `transmissionInterval`, ms: present means KAF may forward it.
    pub transmission_interval_ms: Option<u64>,
    /// Terminating DENM?
    pub termination: Option<Termination>,
    /// Encoded size, octets.
    pub bytes: u32,
}

impl Denm {
    /// When the DENM stops being valid, ms.
    pub fn expires_ms(&self) -> u64 {
        self.detection_ms + self.validity_ms
    }
}

/// The parameters of an application's trigger request [§6.1.2, AppDENM_trigger].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TriggerRequest {
    /// Relevance area.
    pub area: Area,
    /// `validityDuration`, ms.
    pub validity_ms: u64,
    /// `repetitionInterval`, ms: none sends once.
    pub repetition_interval_ms: Option<u64>,
    /// `repetitionDuration`, ms (defaults to the validity).
    pub repetition_duration_ms: Option<u64>,
    /// `transmissionInterval`, ms (enables KAF at receivers).
    pub transmission_interval_ms: Option<u64>,
    /// Encoded size, octets.
    pub bytes: u32,
}

/// A timer the service asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DenTimer {
    /// The originator's repetition.
    Repetition(ActionId),
    /// A receiver's keep-alive forwarding.
    Kaf(ActionId),
}

/// What a received DENM was to the receiver [§8.1.2].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RxKind {
    /// A new event: pass to the application.
    New,
    /// A newer version of a known event.
    Update,
    /// A repetition of the version already held.
    Repetition,
    /// The event ended.
    Terminated,
    /// Older than the version held, or expired: discarded.
    Discarded,
}

/// What the caller must do.
#[derive(Debug, Clone, PartialEq)]
pub enum DenAction {
    /// Hand this DENM to BTP/GeoNetworking as a GeoBroadcast to its area.
    Send {
        /// The DENM.
        denm: Denm,
        /// True for a keep-alive forward by a non-originator.
        kaf: bool,
    },
    /// Call [`DenService::on_timer`] with `timer` at `at_ms`.
    ArmTimer {
        /// Which.
        timer: DenTimer,
        /// When, ms.
        at_ms: u64,
    },
    /// Tell the application.
    Indicate {
        /// The DENM.
        denm: Denm,
        /// What it was.
        kind: RxKind,
    },
}

#[derive(Debug, Clone)]
struct Originated {
    denm: Denm,
    interval_ms: Option<u64>,
    repeat_until_ms: u64,
}

#[derive(Debug, Clone)]
struct Received {
    denm: Denm,
    kaf_due_ms: Option<u64>,
}

/// One station's DEN basic service.
#[derive(Debug, Clone)]
pub struct DenService {
    station: u32,
    next_sn: u16,
    kaf: bool,
    originated: BTreeMap<ActionId, Originated>,
    received: BTreeMap<ActionId, Received>,
    /// KAF forwards sent.
    pub kaf_sent: u64,
}

impl DenService {
    /// The service of `station`; `kaf` enables keep-alive forwarding.
    pub fn new(station: u32, kaf: bool) -> Self {
        DenService {
            station,
            next_sn: 0,
            kaf,
            originated: BTreeMap::new(),
            received: BTreeMap::new(),
            kaf_sent: 0,
        }
    }

    /// AppDENM_trigger [§6.1.2]: a new event, sent now and repeated if asked.
    pub fn trigger(&mut self, req: TriggerRequest, now_ms: u64) -> (ActionId, Vec<DenAction>) {
        self.next_sn = self.next_sn.wrapping_add(1);
        let action = ActionId {
            station: self.station,
            sn: self.next_sn,
        };
        let denm = Denm {
            action,
            area: req.area,
            detection_ms: now_ms,
            reference_ms: now_ms,
            validity_ms: req.validity_ms,
            transmission_interval_ms: req.transmission_interval_ms,
            termination: None,
            bytes: req.bytes,
        };
        let until = now_ms
            + req
                .repetition_duration_ms
                .unwrap_or(req.validity_ms)
                .min(req.validity_ms);
        self.originated.insert(
            action,
            Originated {
                denm,
                interval_ms: req.repetition_interval_ms,
                repeat_until_ms: until,
            },
        );
        let mut out = vec![DenAction::Send { denm, kaf: false }];
        if let Some(iv) = req.repetition_interval_ms.filter(|&i| i > 0) {
            out.push(DenAction::ArmTimer {
                timer: DenTimer::Repetition(action),
                at_ms: now_ms + iv,
            });
        }
        (action, out)
    }

    /// AppDENM_update [§6.1.3]: a new reference time, sent at once.
    pub fn update(&mut self, action: ActionId, now_ms: u64) -> Vec<DenAction> {
        match self.originated.get_mut(&action) {
            Some(o) if o.denm.termination.is_none() => {
                o.denm.reference_ms = now_ms;
                vec![DenAction::Send {
                    denm: o.denm,
                    kaf: false,
                }]
            }
            _ => vec![],
        }
    }

    /// AppDENM_termination [§6.1.4]: a cancellation DENM; repetition stops.
    pub fn terminate(&mut self, action: ActionId, now_ms: u64) -> Vec<DenAction> {
        match self.originated.get_mut(&action) {
            Some(o) if o.denm.termination.is_none() => {
                o.denm.termination = Some(Termination::Cancel);
                o.denm.reference_ms = now_ms;
                vec![DenAction::Send {
                    denm: o.denm,
                    kaf: false,
                }]
            }
            _ => vec![],
        }
    }

    /// A timer fired. `jitter_ms` is the caller's draw for a re-armed KAF timer.
    pub fn on_timer(&mut self, timer: DenTimer, now_ms: u64, jitter_ms: u64) -> Vec<DenAction> {
        match timer {
            DenTimer::Repetition(a) => {
                let Some(o) = self.originated.get(&a) else {
                    return vec![];
                };
                if o.denm.termination.is_some()
                    || now_ms > o.repeat_until_ms
                    || now_ms >= o.denm.expires_ms()
                {
                    return vec![];
                }
                let mut out = vec![DenAction::Send {
                    denm: o.denm,
                    kaf: false,
                }];
                if let Some(iv) = o.interval_ms {
                    out.push(DenAction::ArmTimer {
                        timer,
                        at_ms: now_ms + iv,
                    });
                }
                out
            }
            DenTimer::Kaf(a) => {
                let Some(r) = self.received.get_mut(&a) else {
                    return vec![];
                };
                if r.kaf_due_ms != Some(now_ms)
                    || r.denm.termination.is_some()
                    || now_ms >= r.denm.expires_ms()
                {
                    return vec![];
                }
                let iv = r.denm.transmission_interval_ms.unwrap_or(0);
                if iv == 0 {
                    return vec![];
                }
                let due = now_ms + iv + jitter_ms;
                r.kaf_due_ms = Some(due);
                self.kaf_sent += 1;
                vec![
                    DenAction::Send {
                        denm: r.denm,
                        kaf: true,
                    },
                    DenAction::ArmTimer { timer, at_ms: due },
                ]
            }
        }
    }

    /// A DENM arrived [§8.1.2]. `inside` is whether this station is in its relevance
    /// area; `jitter_ms` is the caller's KAF jitter draw.
    pub fn on_receive(
        &mut self,
        denm: Denm,
        now_ms: u64,
        inside: bool,
        jitter_ms: u64,
    ) -> Vec<DenAction> {
        if denm.action.station == self.station || now_ms >= denm.expires_ms() {
            return vec![DenAction::Indicate {
                denm,
                kind: RxKind::Discarded,
            }];
        }
        let kind = match self.received.get(&denm.action) {
            None if denm.termination.is_some() => RxKind::Terminated,
            None => RxKind::New,
            Some(r) if denm.reference_ms < r.denm.reference_ms => RxKind::Discarded,
            Some(r) if r.denm.termination.is_some() => RxKind::Discarded,
            Some(_) if denm.termination.is_some() => RxKind::Terminated,
            Some(r) if denm.reference_ms > r.denm.reference_ms => RxKind::Update,
            Some(_) => RxKind::Repetition,
        };
        let mut out = Vec::new();
        if kind == RxKind::Discarded {
            out.push(DenAction::Indicate { denm, kind });
            return out;
        }
        let kaf_due = match (
            self.kaf,
            inside,
            denm.termination,
            denm.transmission_interval_ms,
        ) {
            (true, true, None, Some(iv)) if iv > 0 => Some(now_ms + iv + jitter_ms),
            _ => None,
        };
        self.received.insert(
            denm.action,
            Received {
                denm,
                kaf_due_ms: kaf_due,
            },
        );
        if let Some(at) = kaf_due {
            out.push(DenAction::ArmTimer {
                timer: DenTimer::Kaf(denm.action),
                at_ms: at,
            });
        }
        out.push(DenAction::Indicate { denm, kind });
        out
    }

    /// Events this station holds as a receiver.
    pub fn known(&self) -> usize {
        self.received.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> TriggerRequest {
        TriggerRequest {
            area: Area::circle(0.0, 0.0, 1_000.0),
            validity_ms: 10_000,
            repetition_interval_ms: Some(1_000),
            repetition_duration_ms: Some(3_000),
            transmission_interval_ms: Some(1_000),
            bytes: 200,
        }
    }

    #[test]
    fn originator_repeats_until_the_repetition_duration() {
        let mut s = DenService::new(1, false);
        let (a, first) = s.trigger(req(), 0);
        assert!(matches!(first[0], DenAction::Send { .. }));
        let mut sends = 1;
        let mut t = 1_000;
        loop {
            let out = s.on_timer(DenTimer::Repetition(a), t, 0);
            if out.is_empty() {
                break;
            }
            sends += 1;
            t += 1_000;
        }
        assert_eq!(sends, 4); // 0, 1, 2, 3 s
    }

    #[test]
    fn receiver_classifies_and_kaf_forwards_after_silence() {
        let mut o = DenService::new(1, false);
        let (_, acts) = o.trigger(req(), 0);
        let DenAction::Send { denm, .. } = acts[0] else {
            panic!()
        };
        let mut r = DenService::new(2, true);
        let a = r.on_receive(denm, 10, true, 50);
        assert!(a.contains(&DenAction::Indicate {
            denm,
            kind: RxKind::New
        }));
        assert!(a.contains(&DenAction::ArmTimer {
            timer: DenTimer::Kaf(denm.action),
            at_ms: 1_060
        }));
        // A repetition at 900 ms pushes the KAF timer back, so the old one is stale.
        let b = r.on_receive(denm, 900, true, 50);
        assert!(b.contains(&DenAction::Indicate {
            denm,
            kind: RxKind::Repetition
        }));
        assert!(r.on_timer(DenTimer::Kaf(denm.action), 1_060, 0).is_empty());
        let fwd = r.on_timer(DenTimer::Kaf(denm.action), 1_950, 0);
        assert!(matches!(fwd[0], DenAction::Send { kaf: true, .. }));
        let mut cancel = denm;
        cancel.termination = Some(Termination::Cancel);
        cancel.reference_ms = 2_000;
        let c = r.on_receive(cancel, 2_000, true, 0);
        assert!(c.contains(&DenAction::Indicate {
            denm: cancel,
            kind: RxKind::Terminated
        }));
        assert!(r.on_timer(DenTimer::Kaf(denm.action), 2_950, 0).is_empty());
    }
}
