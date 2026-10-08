//! The session engine: runs any [`ThresholdProtocol`]'s online rounds over the modelled
//! backend, with slow, offline and malicious participants.
//!
//! Protocol-agnostic. A host protocol (the SCMS) owns a [`Committees`] value, wraps
//! [`CommitteeMsg`] in its own message type, starts a session from the coordinator's
//! handler with a *continuation* — what to do once the committee has signed — and gets
//! the continuation back as an [`Outcome`] when the session ends. Every message crosses a
//! link with its bytes, every computation is charged to the participant's queue, and a
//! round ends only when the coordinator holds the quorum's responses.
//!
//! # Behaviour under failure
//!
//! * **Selection.** The coordinator asks every live member it has not excluded
//!   (first-responders, the default) or exactly a quorum of them (`fanout_all = false`),
//!   and fixes the signing set to the first `quorum` to answer round one.
//! * **Slow.** A slow member answers late (`crate::fault`); with first-responders the
//!   others carry the round, and the session pays only if fewer than `quorum` are fast.
//! * **Offline.** Its requests are dropped. When a round's timer fires short of quorum,
//!   the silent members are excluded and the session restarts from round one with the
//!   rest (a new attempt) — unless excluding them would leave fewer than a quorum, in
//!   which case the coordinator cannot tell a slow member from a dead one and waits one
//!   more timeout instead, up to `max_attempts` times.
//! * **Malicious.** It returns an invalid share. In the round that verifies shares the
//!   coordinator detects it; with identifiable abort the member is excluded — from this
//!   session and from every later one of the committee, since identifying a cheater is
//!   what identifiable abort is for — and the session restarts without it; without
//!   attribution the coordinator cannot tell who lied and restarts with the signing set
//!   rotated.
//! * **Too few.** When fewer than `quorum` members remain, the attempt fails; the session
//!   waits `retry_after` (members may come back) and tries again with the timed-out
//!   members readmitted (never a caught cheater), up to `max_attempts`, then fails for
//!   good and hands the continuation back as [`Outcome::Failed`].

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};

use super::{StarRound, ThresholdProtocol};
use crate::fault::FaultPlan;
use crate::kernel::Outbox;
use crate::net::Transport;
use crate::sizes::WireSize;
use crate::stage::{FlowId, FlowRun};

/// A committee message, wrapped by the host in its own message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitteeMsg {
    /// Coordinator → member: do round `round` of attempt `attempt`.
    Request {
        /// The session.
        session: u64,
        /// The round, from 0.
        round: u16,
        /// The attempt, from 0.
        attempt: u16,
    },
    /// Member → coordinator.
    Response {
        /// The session.
        session: u64,
        /// The round.
        round: u16,
        /// The attempt.
        attempt: u16,
        /// The member's committee index.
        member: u16,
        /// Whether its share is valid.
        valid: bool,
    },
    /// The coordinator's round timer.
    Timeout {
        /// The session.
        session: u64,
        /// The round.
        round: u16,
        /// The attempt.
        attempt: u16,
    },
    /// The coordinator's retry timer after a failed attempt.
    Retry {
        /// The session.
        session: u64,
    },
}

/// One committee: a role played by `n` servers instead of one.
#[derive(Debug)]
pub struct RoleSpec {
    /// The role, e.g. `pca`, `crl`, `linkage`.
    pub role: &'static str,
    /// The node that coordinates.
    pub coordinator: NodeId,
    /// The members, in committee-index order.
    pub members: Vec<NodeId>,
    /// How many must act.
    pub quorum: u16,
    /// The protocol.
    pub protocol: Box<dyn ThresholdProtocol>,
    /// How long the coordinator waits for a round before excluding the silent.
    pub round_timeout: Duration,
    /// Ask every live member and take the first `quorum` (true), or ask exactly `quorum`.
    pub fanout_all: bool,
    /// Attempts before the session fails for good.
    pub max_attempts: u16,
    /// Wait before retrying an attempt that ran out of members.
    pub retry_after: Duration,
}

/// What a role's sessions cost and how they went.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct CommitteeStats {
    /// Sessions started.
    pub sessions: u64,
    /// Sessions that produced their output.
    pub completed: u64,
    /// Sessions that failed for good.
    pub failed: u64,
    /// Restarts: attempts beyond the first.
    pub restarts: u64,
    /// Round timers that fired short of quorum.
    pub timeouts: u64,
    /// Invalid shares detected.
    pub invalid_shares: u64,
    /// Members excluded from a session (timed out or caught lying).
    pub exclusions: u64,
    /// Committee messages sent.
    pub messages: u64,
    /// Committee bytes sent.
    pub bytes: u64,
    /// Rounds completed.
    pub rounds: u64,
    /// Items signed (certificates, CRLs, seeds).
    pub items: u64,
    /// Sum of completed sessions' latency, ns.
    pub latency_ns_sum: u64,
    /// Largest completed session latency, ns.
    pub latency_ns_max: u64,
}

impl CommitteeStats {
    /// Mean completed-session latency, ns.
    pub const fn mean_latency_ns(&self) -> u64 {
        if self.completed == 0 {
            0
        } else {
            self.latency_ns_sum / self.completed
        }
    }
}

/// How a session ended.
#[derive(Debug)]
pub enum Outcome<C> {
    /// The committee produced its output.
    Done {
        /// What the host asked to do next.
        cont: C,
        /// From start to the last round's completion.
        latency: Duration,
        /// The members whose shares made the output.
        signers: Vec<u16>,
    },
    /// It could not.
    Failed {
        /// What the host asked to do next, to be abandoned or retried.
        cont: C,
    },
}

#[derive(Debug)]
struct Session<C> {
    role: usize,
    batch: u32,
    flow: FlowId,
    run: FlowRun,
    rounds: Vec<StarRound>,
    round: u16,
    attempt: u16,
    tries: u16,
    asked: Vec<u16>,
    responded: Vec<u16>,
    excluded: BTreeSet<u16>,
    rotation: u16,
    waits: u16,
    started: SimTime,
    cont: Option<C>,
}

/// Every committee of a deployment, and its sessions.
#[derive(Debug)]
pub struct Committees<C> {
    /// The committees.
    pub roles: Vec<RoleSpec>,
    sessions: BTreeMap<u64, Session<C>>,
    next: u64,
    /// The envelope every committee message carries, bytes.
    pub envelope: u32,
    /// Per-role statistics, by role name.
    pub stats: BTreeMap<&'static str, CommitteeStats>,
    /// Members each committee has caught returning an invalid share, by role index:
    /// excluded from every later session.
    pub caught: BTreeMap<usize, BTreeSet<u16>>,
}

impl<C> Default for Committees<C> {
    fn default() -> Committees<C> {
        Committees {
            roles: Vec::new(),
            sessions: BTreeMap::new(),
            next: 0,
            envelope: 0,
            stats: BTreeMap::new(),
            caught: BTreeMap::new(),
        }
    }
}

/// How the host wraps a committee message.
pub type Wrap<M> = fn(CommitteeMsg) -> M;

impl<C> Committees<C> {
    /// Committees whose messages carry `envelope` bytes of framing.
    pub fn new(roles: Vec<RoleSpec>, envelope: u32) -> Committees<C> {
        Committees {
            roles,
            envelope,
            ..Committees::default()
        }
    }

    /// The committee playing `role`, if the deployment has one.
    pub fn role(&self, role: &str) -> Option<usize> {
        self.roles.iter().position(|r| r.role == role)
    }

    /// Whether `node` is a member of any committee.
    pub fn is_member(&self, node: NodeId) -> bool {
        self.roles.iter().any(|r| r.members.contains(&node))
    }

    /// Sessions still running.
    pub fn open_sessions(&self) -> usize {
        self.sessions.len()
    }

    fn stats_mut(&mut self, role: usize) -> &mut CommitteeStats {
        self.stats.entry(self.roles[role].role).or_default()
    }

    /// Starts a session from the coordinator's handler: `batch` items to sign, `cont` to
    /// hand back when it ends. Sends round one's requests on `out`.
    #[allow(clippy::too_many_arguments)]
    pub fn start<M>(
        &mut self,
        role: usize,
        batch: u32,
        cont: C,
        now: SimTime,
        flow: FlowId,
        run: FlowRun,
        out: &mut Outbox<M>,
        wrap: Wrap<M>,
    ) -> u64 {
        let id = self.next;
        self.next += 1;
        let r = &self.roles[role];
        // The plan is sized for the quorum that will sign; asking more members in round
        // one costs their extra requests and responses, which are charged as sent.
        let rounds = r.protocol.sign_rounds(r.quorum.max(1), batch.max(1));
        self.sessions.insert(
            id,
            Session {
                role,
                batch,
                flow,
                run,
                rounds,
                round: 0,
                attempt: 0,
                tries: 0,
                asked: Vec::new(),
                responded: Vec::new(),
                excluded: BTreeSet::new(),
                rotation: 0,
                waits: 0,
                started: now,
                cont: Some(cont),
            },
        );
        let s = self.stats_mut(role);
        s.sessions += 1;
        s.items += u64::from(batch);
        self.begin_attempt(id, out, wrap);
        id
    }

    /// Sends the requests of round one of the session's current attempt.
    fn begin_attempt<M>(&mut self, id: u64, out: &mut Outbox<M>, wrap: Wrap<M>) {
        let Some(s) = self.sessions.get_mut(&id) else {
            return;
        };
        let r = &self.roles[s.role];
        let n = u16::try_from(r.members.len()).unwrap_or(u16::MAX);
        let caught = self.caught.get(&s.role);
        let live: Vec<u16> = (0..n)
            .map(|k| (k + s.rotation) % n.max(1))
            .filter(|k| !s.excluded.contains(k) && !caught.is_some_and(|c| c.contains(k)))
            .collect();
        s.waits = 0;
        s.round = 0;
        s.responded.clear();
        s.asked = if r.fanout_all {
            live
        } else {
            live.into_iter().take(usize::from(r.quorum)).collect()
        };
        self.send_round(id, out, wrap);
    }

    fn send_round<M>(&mut self, id: u64, out: &mut Outbox<M>, wrap: Wrap<M>) {
        let envelope = self.envelope;
        let Some(s) = self.sessions.get_mut(&id) else {
            return;
        };
        let role = s.role;
        let r = &self.roles[role];
        let round = &s.rounds[usize::from(s.round)];
        let size = WireSize::derived(
            round.request.bytes() + envelope,
            "envelope + round request",
            "crate::threshold::session; sizes per the protocol module's citations",
        );
        let mut sent = 0u64;
        for &k in &s.asked {
            out.send(
                r.members[usize::from(k)],
                wrap(CommitteeMsg::Request {
                    session: id,
                    round: s.round,
                    attempt: s.attempt,
                }),
                round.name,
                size,
                Transport::BackendNet,
                s.flow,
                s.run,
            );
            sent += 1;
        }
        // The timeout runs from when a member could have answered: a protocol whose
        // members compute for seconds (CGGMP21's presigning) is not timed out for doing
        // its work.
        let expected = round.member_time.map_or(Duration::ZERO, |(t, _)| t);
        out.start_timer(
            Duration::from_nanos(
                r.round_timeout
                    .as_nanos()
                    .saturating_add(expected.as_nanos()),
            ),
            wrap(CommitteeMsg::Timeout {
                session: id,
                round: s.round,
                attempt: s.attempt,
            }),
            s.flow,
            s.run,
        );
        let st = self.stats.entry(self.roles[role].role).or_default();
        st.messages += sent;
        st.bytes += sent * u64::from(size.bytes());
    }

    /// Handles a committee message delivered to `to` at `at`. Returns the session's
    /// outcome when this message ended it.
    #[allow(clippy::too_many_arguments)]
    pub fn handle<M>(
        &mut self,
        msg: CommitteeMsg,
        at: SimTime,
        to: NodeId,
        faults: &FaultPlan,
        out: &mut Outbox<M>,
        wrap: Wrap<M>,
    ) -> Option<Outcome<C>> {
        match msg {
            CommitteeMsg::Request {
                session,
                round,
                attempt,
            } => {
                self.member_round(session, round, attempt, at, to, faults, out, wrap);
                None
            }
            CommitteeMsg::Response {
                session,
                round,
                attempt,
                member,
                valid,
            } => self.on_response(session, round, attempt, member, valid, at, out, wrap),
            CommitteeMsg::Timeout {
                session,
                round,
                attempt,
            } => self.on_timeout(session, round, attempt, out, wrap),
            CommitteeMsg::Retry { session } => {
                if self.sessions.contains_key(&session) {
                    self.begin_attempt(session, out, wrap);
                }
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn member_round<M>(
        &mut self,
        id: u64,
        round: u16,
        attempt: u16,
        at: SimTime,
        to: NodeId,
        faults: &FaultPlan,
        out: &mut Outbox<M>,
        wrap: Wrap<M>,
    ) {
        let envelope = self.envelope;
        let Some(s) = self.sessions.get(&id) else {
            return;
        };
        let r = &self.roles[s.role];
        let Some(member) = r.members.iter().position(|m| *m == to) else {
            return;
        };
        let member = u16::try_from(member).unwrap_or(u16::MAX);
        let plan = &s.rounds[usize::from(round)];
        for op in &plan.member_ops {
            out.compute(*op);
        }
        if let Some((t, _)) = plan.member_time {
            out.charge_time("threshold/protocol-time", t);
        }
        // A malicious member's lie is in its share, so it only shows in the round where
        // shares are verified; elsewhere its messages are well formed.
        let valid = !(plan.verifies_shares && faults.malicious(to, at));
        let size = WireSize::derived(
            plan.response.bytes() + envelope,
            "envelope + round response",
            "crate::threshold::session; sizes per the protocol module's citations",
        );
        out.send(
            r.coordinator,
            wrap(CommitteeMsg::Response {
                session: id,
                round,
                attempt,
                member,
                valid,
            }),
            plan.name,
            size,
            Transport::BackendNet,
            s.flow,
            s.run,
        );
        let st = self.stats.entry(r.role).or_default();
        st.messages += 1;
        st.bytes += u64::from(size.bytes());
    }

    #[allow(clippy::too_many_arguments)]
    fn on_response<M>(
        &mut self,
        id: u64,
        round: u16,
        attempt: u16,
        member: u16,
        valid: bool,
        at: SimTime,
        out: &mut Outbox<M>,
        wrap: Wrap<M>,
    ) -> Option<Outcome<C>> {
        let s = self.sessions.get_mut(&id)?;
        if s.attempt != attempt || s.round != round || s.responded.contains(&member) {
            // A late answer to a round already decided: it cost its bytes, nothing more.
            return None;
        }
        if !s.asked.contains(&member) {
            return None;
        }
        let role = s.role;
        let quorum = self.roles[role].quorum;
        let identifiable = self.roles[role].protocol.identifiable_abort();
        let plan = s.rounds[usize::from(round)].clone();
        if plan.verifies_shares {
            // The coordinator checks each share as it arrives; its cost is part of the
            // round's coordinator work, charged once per share here.
            for op in &plan.coordinator_ops {
                let mut one = *op;
                one.count = one.count.div_ceil(u32::from(quorum.max(1)));
                out.compute(one);
            }
        }
        if !valid {
            let st = self.stats.entry(self.roles[role].role).or_default();
            st.invalid_shares += 1;
            let s = self.sessions.get_mut(&id)?;
            if identifiable {
                s.excluded.insert(member);
                st.exclusions += 1;
                self.caught.entry(role).or_default().insert(member);
            } else {
                s.rotation = s.rotation.wrapping_add(1);
            }
            return self.restart(id, out, wrap);
        }
        s.responded.push(member);
        if s.responded.len() < usize::from(quorum) {
            return None;
        }
        // The round is in.
        if !plan.verifies_shares {
            for op in &plan.coordinator_ops {
                out.compute(*op);
            }
        }
        if s.waits > 0 {
            // The round needed extra timeouts: whoever is still silent missed a whole one,
            // and is presumed down for the rest of the session.
            let silent: Vec<u16> = s
                .asked
                .iter()
                .copied()
                .filter(|k| !s.responded.contains(k))
                .collect();
            s.excluded.extend(silent);
            s.waits = 0;
        }
        if round == 0 {
            // The first `quorum` to answer are the signing set from here on.
            s.asked = s.responded.clone();
        }
        let st = self.stats.entry(self.roles[role].role).or_default();
        st.rounds += 1;
        let s = self.sessions.get_mut(&id)?;
        if usize::from(round) + 1 < s.rounds.len() {
            s.round += 1;
            s.responded.clear();
            self.send_round(id, out, wrap);
            return None;
        }
        let s = self.sessions.remove(&id)?;
        let latency = Duration::from_nanos(at.saturating_sub(s.started));
        let st = self.stats.entry(self.roles[role].role).or_default();
        st.completed += 1;
        st.latency_ns_sum = st.latency_ns_sum.saturating_add(latency.as_nanos());
        st.latency_ns_max = st.latency_ns_max.max(latency.as_nanos());
        Some(Outcome::Done {
            cont: s.cont?,
            latency,
            signers: s.responded,
        })
    }

    fn on_timeout<M>(
        &mut self,
        id: u64,
        round: u16,
        attempt: u16,
        out: &mut Outbox<M>,
        wrap: Wrap<M>,
    ) -> Option<Outcome<C>> {
        let s = self.sessions.get_mut(&id)?;
        if s.attempt != attempt || s.round != round {
            return None;
        }
        let silent: Vec<u16> = s
            .asked
            .iter()
            .copied()
            .filter(|k| !s.responded.contains(k))
            .collect();
        let role = s.role;
        let r = &self.roles[role];
        let caught = self
            .caught
            .get(&role)
            .map_or(0, |c| c.iter().filter(|k| !s.excluded.contains(k)).count());
        let left = r
            .members
            .len()
            .saturating_sub(s.excluded.len() + caught + silent.len());
        if left < usize::from(r.quorum) && s.waits < r.max_attempts {
            // Excluding the silent would leave no quorum, and a slow member cannot be told
            // from a dead one: wait one more timeout for this round.
            s.waits += 1;
            let (flow, run, wait, round, attempt) =
                (s.flow, s.run, r.round_timeout, s.round, s.attempt);
            out.start_timer(
                wait,
                wrap(CommitteeMsg::Timeout {
                    session: id,
                    round,
                    attempt,
                }),
                flow,
                run,
            );
            self.stats
                .entry(self.roles[role].role)
                .or_default()
                .timeouts += 1;
            return None;
        }
        for k in &silent {
            s.excluded.insert(*k);
        }
        let st = self.stats.entry(self.roles[role].role).or_default();
        st.timeouts += 1;
        st.exclusions += silent.len() as u64;
        self.restart(id, out, wrap)
    }

    /// A new attempt without the excluded members, or — if too few remain — a wait and
    /// a fresh try, or failure.
    fn restart<M>(&mut self, id: u64, out: &mut Outbox<M>, wrap: Wrap<M>) -> Option<Outcome<C>> {
        let s = self.sessions.get_mut(&id)?;
        let r = &self.roles[s.role];
        let n = r.members.len();
        s.attempt = s.attempt.wrapping_add(1);
        let role = s.role;
        let caught = self.caught.get(&role);
        let remaining = n.saturating_sub(
            s.excluded.len()
                + caught.map_or(0, |c| c.iter().filter(|k| !s.excluded.contains(k)).count()),
        );
        // A bound on restarts that exclude nobody (a lie that cannot be attributed rotates
        // the signing set instead): after `max_attempts` passes over the committee the
        // session fails rather than spinning.
        let cap = u32::from(r.max_attempts.max(1)) * u32::try_from(n.max(1)).unwrap_or(u32::MAX);
        if u32::from(s.attempt) > cap {
            let s = self.sessions.remove(&id)?;
            self.stats.entry(self.roles[role].role).or_default().failed += 1;
            return Some(Outcome::Failed { cont: s.cont? });
        }
        if remaining >= usize::from(r.quorum) {
            self.stats
                .entry(self.roles[role].role)
                .or_default()
                .restarts += 1;
            self.begin_attempt(id, out, wrap);
            return None;
        }
        // Too few: wait for members to come back, and forget who was excluded.
        s.tries += 1;
        if s.tries >= r.max_attempts {
            let s = self.sessions.remove(&id)?;
            self.stats.entry(self.roles[role].role).or_default().failed += 1;
            return Some(Outcome::Failed { cont: s.cont? });
        }
        s.excluded.clear();
        let (flow, run, wait) = (s.flow, s.run, r.retry_after);
        self.stats
            .entry(self.roles[role].role)
            .or_default()
            .restarts += 1;
        out.start_timer(wait, wrap(CommitteeMsg::Retry { session: id }), flow, run);
        None
    }

    /// The batch a session signs, for a host that wants it while the session runs.
    pub fn batch_of(&self, id: u64) -> Option<u32> {
        self.sessions.get(&id).map(|s| s.batch)
    }
}
