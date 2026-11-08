//! Signal priority for emergency vehicles, served by the junction's controller: the
//! roadside unit's priority request server hands each SRM to the controller, which
//! extends a green the vehicle is about to need or ends early the green that stands in
//! its way (NTCIP 1211 Signal Control and Prioritization), and afterwards walks the
//! plan back to its coordinated offset.
//!
//! # How the controller changes its plan
//!
//! A fixed-time plan is a clock: the phase running at `t` is the plan's phase at
//! `(t − offset) mod cycle`. A controller serving priority moves that clock, and only in
//! the ways a real one does:
//!
//! * **Green extension.** While the requested movement's green would end before the
//!   vehicle arrives (its ETA plus [`PriorityParams::clearance_margin_s`]), the clock is
//!   held — the green lasts longer — for at most [`PriorityParams::max_extension_s`].
//! * **Early green (red truncation).** While the requested movement is red, a conflicting
//!   green that has run its minimum ([`PriorityParams::min_green_s`]) is ended at once:
//!   the clock jumps to that phase's end, so its amber, its all-red and every clearance
//!   interval after it run in full. A phase that clears pedestrians (a crossing that walked
//!   in the phase before and does not now) is never cut, and neither is a walk shorter
//!   than the minimum — MUTCD 2009 §4E.06's 7 s walk.
//! * **Recovery.** Once no request stands, the offset the service moved is walked back
//!   during greens only, at most [`PriorityParams::recovery_rate`] of real time, so a
//!   green runs between 80 % and 120 % of its length until the junction is back in its
//!   green wave — the "smooth" transition a coordinated controller uses.
//!
//! Every mobility and signal consumer reads the plan's offset from the world, so the
//! drivers, the SPaT and the page's lamps all see the moved plan at once.
//!
//! # What is cited and what is chosen
//!
//! NTCIP 1211 defines the request, its server and the strategies (early green, green
//! extension) but leaves their limits to the operator. The limits here are this build's
//! defaults: a 10 s extension (FHWA's TSP handbook, 2005, reports 5–15 s in practice,
//! recalled), a 7 s minimum green (MUTCD §4E.06's minimum walk), a 20 % recovery rate
//! (the size of the cycle-length changes NEMA TS 2 transitions use, recalled), and a
//! request that lapses 2 s after its last repetition (the vehicle repeats its SRM every
//! second while it approaches, `v2xw_node::events::SRM_INTERVAL`).

use std::collections::BTreeMap;

use serde::Serialize;
use v2xw_core::ctx::{Record, Visibility};
use v2xw_core::time::SimTime;
use v2xw_world::model::LaneKind;
use v2xw_world::{SignalPlan, SignalState, World};

/// The controller's priority settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriorityParams {
    /// The longest a green is extended for one service, seconds.
    pub max_extension_s: f64,
    /// A green is never ended before this, seconds.
    pub min_green_s: f64,
    /// The fraction of real time the clock may run fast or slow while recovering.
    pub recovery_rate: f64,
    /// A request not repeated for this long has lapsed, seconds.
    pub request_timeout_s: f64,
    /// The green is held until this long after the vehicle's arrival, seconds.
    pub clearance_margin_s: f64,
}

impl Default for PriorityParams {
    fn default() -> Self {
        Self {
            max_extension_s: 10.0,
            min_green_s: 7.0,
            recovery_rate: 0.2,
            request_timeout_s: 2.0,
            clearance_margin_s: 2.0,
        }
    }
}

/// `signal.priority` — what a controller did for a priority request (PUBLIC: it is the
/// controller's own log).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriorityRecord {
    /// When.
    pub t: SimTime,
    /// The junction.
    pub junction: u32,
    /// The signal group served.
    pub group: u16,
    /// The requesting vehicle's pseudonym (its signer digest, lowercase hex): what the
    /// unit knows it by.
    pub requester: String,
    /// `request`, `extend`, `early-green`, `served` or `recovered`.
    pub action: &'static str,
    /// For `extend`: the extension this hold asks for, at most what the green has left of
    /// [`PriorityParams::max_extension_s`]; for `served`: the extension the service spent; for
    /// `early-green`: the green cut; for `recovered`: the offset walked back. Seconds.
    pub seconds: f64,
}

impl Record for PriorityRecord {
    const CHANNEL: &'static str = "signal.priority";
    const VISIBILITY: Visibility = Visibility::Public;
}

#[derive(Debug, Clone, Copy)]
struct Request {
    group: u16,
    /// Arrival at the stop line, simulation seconds.
    eta_s: f64,
    last_s: f64,
}

#[derive(Debug, Clone, Default)]
struct PlanState {
    /// The coordinated offset the plan had before any service.
    base_offset: Option<f64>,
    /// Extension spent on the current service, seconds.
    extended_s: f64,
    /// Extension spent on the green now running, whichever service asked, seconds: the limit
    /// is per green, or two services in a row would hold one green past every stated bound.
    green_extended_s: f64,
    requests: BTreeMap<[u8; 8], Request>,
    /// Movements per signal group (indices into `controlled`), and which movements are
    /// pedestrian crossings.
    groups: BTreeMap<u16, Vec<usize>>,
    crossings: Vec<usize>,
    announced_extend: bool,
    /// The phase the clock showed at the last step, and since when in real time: the minimum
    /// green is a time the lamp shows, and a clock run fast or held says nothing of it.
    phase: Option<usize>,
    phase_since_s: f64,
}

impl PlanState {
    /// The seconds the lamps have shown phase `i`, at `now_s`, had it just started if the clock
    /// was last elsewhere.
    fn shown_s(&self, i: usize, into: f64, now_s: f64) -> f64 {
        if self.phase == Some(i) {
            now_s - self.phase_since_s
        } else {
            into
        }
    }
}

/// One junction controller's view of a request, for the SPaT (`crate::infra`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outlook {
    /// The group being served.
    pub group: u16,
    /// Extension still available to it, seconds.
    pub extension_left_s: f64,
    /// Seconds until its arrival plus the margin, from now.
    pub needed_s: f64,
    /// Seconds before any other green could be ended (its minimum green), from now.
    pub earliest_cut_s: f64,
}

/// Every junction's priority service.
#[derive(Debug, Clone, Default)]
pub struct PriorityControllers {
    params: PriorityParams,
    plans: BTreeMap<usize, PlanState>,
    served: u64,
    /// Plans whose requests the NEMA rings serve: logged and lapsed here,
    /// never served by moving the clock.
    through_rings: std::collections::BTreeSet<usize>,
    /// Whether requests can reach these controllers at all (`srm` on the air): the SPaT then
    /// states what a request could do ([`Self::window`]).
    enabled: bool,
}

impl PriorityControllers {
    /// Controllers with `params`.
    pub fn new(params: PriorityParams) -> Self {
        Self {
            params,
            plans: BTreeMap::new(),
            served: 0,
            through_rings: std::collections::BTreeSet::new(),
            enabled: false,
        }
    }

    /// Whether this run's vehicles can send requests.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// How far the service may still move the end of each phase of `plan`, whatever request
    /// comes next: `None` when no request can come, or when the rings serve them. A green
    /// runs on by what is left of its extension, or ends once its minimum has run, unless it
    /// is clearing pedestrians; recovery may then run its clock at `1 ± recovery_rate`, which
    /// makes what is left of it `1 / (1 - rate)` as long or `1 / (1 + rate)`.
    pub fn window(
        &self,
        world: &World,
        plan: usize,
        now_s: f64,
    ) -> Option<Vec<crate::actuated::PhaseBound>> {
        if !self.enabled || self.through_rings.contains(&plan) {
            return None;
        }
        let st = self.plans.get(&plan)?;
        let p = world.signals.get(plan)?;
        let (i, into) = p.phase_at(now_s)?;
        let q = self.params;
        let slow = q.recovery_rate / (1.0 - q.recovery_rate).max(0.1);
        Some(
            (0..p.phases.len())
                .map(|k| {
                    let duration = p.phases[k].duration_s;
                    if !is_vehicle_green(p, k) {
                        return crate::actuated::PhaseBound::FIXED;
                    }
                    let (left, gone, spent) = if k == i {
                        (
                            (duration - into).max(0.0),
                            st.shown_s(i, into, now_s),
                            st.green_extended_s,
                        )
                    } else {
                        (duration, 0.0, 0.0)
                    };
                    let cut_s = if clears_pedestrians(p, st, k) {
                        0.0
                    } else {
                        (left - (q.min_green_s - gone).max(0.0)).clamp(0.0, left)
                    };
                    crate::actuated::PhaseBound {
                        cut_s,
                        extend_s: Some((q.max_extension_s - spent).max(0.0) + left * slow),
                    }
                })
                .collect(),
        )
    }

    /// The settings.
    pub fn params(&self) -> PriorityParams {
        self.params
    }

    /// Marks `plans` as served through the NEMA rings: their requests are kept, logged and
    /// lapsed here, and [`Self::requests`] hands them to the rings.
    pub fn serve_through_rings(&mut self, plans: impl Iterator<Item = usize>) {
        self.through_rings.extend(plans);
    }

    /// The standing requests at `plan`: `(signal group, arrival at the stop line in
    /// simulation seconds)`, earliest first.
    pub fn requests(&self, plan: usize) -> Vec<(u16, f64)> {
        let mut v: Vec<(u16, f64)> = self
            .plans
            .get(&plan)
            .map(|st| st.requests.values().map(|r| (r.group, r.eta_s)).collect())
            .unwrap_or_default();
        v.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        v
    }

    /// How many requests were served to completion.
    pub fn served(&self) -> u64 {
        self.served
    }

    /// A priority request for `group` of `world.signals[plan]` from `requester`, arriving
    /// in `eta_s` seconds, heard at `now_s`.
    pub fn request(
        &mut self,
        world: &World,
        plan: usize,
        requester: [u8; 8],
        group: u16,
        eta_s: f64,
        now_s: f64,
    ) -> Option<PriorityRecord> {
        let p = world.signals.get(plan)?;
        let st = self
            .plans
            .entry(plan)
            .or_insert_with(|| plan_state(world, p));
        st.base_offset.get_or_insert(p.offset_s);
        let new = !st.requests.contains_key(&requester);
        st.requests.insert(
            requester,
            Request {
                group,
                eta_s: now_s + eta_s.max(0.0),
                last_s: now_s,
            },
        );
        new.then(|| PriorityRecord {
            t: (now_s * 1e9) as SimTime,
            junction: p.junction.index(),
            group,
            requester: hex(&requester),
            action: "request",
            seconds: v2xw_core::math::q3(eta_s),
        })
    }

    /// The service's outlook at `plan`, for the SPaT's max and likely end times.
    pub fn outlook(&self, world: &World, plan: usize, now_s: f64) -> Option<Outlook> {
        // A NEMA junction's SPaT window comes from its rings (`crate::nema::control`).
        if self.through_rings.contains(&plan) {
            return None;
        }
        let st = self.plans.get(&plan)?;
        let r = st
            .requests
            .values()
            .min_by(|a, b| a.eta_s.total_cmp(&b.eta_s))?;
        let p = world.signals.get(plan)?;
        let (_, into) = p.phase_at(now_s)?;
        Some(Outlook {
            group: r.group,
            extension_left_s: (self.params.max_extension_s - st.extended_s).max(0.0),
            needed_s: (r.eta_s - now_s + self.params.clearance_margin_s).max(0.0),
            earliest_cut_s: (self.params.min_green_s - into).max(0.0),
        })
    }

    /// Advances every controller by `dt_s` to `now_s`: holds, cuts and recovers, moving
    /// the plans' offsets in `world`.
    pub fn step(&mut self, world: &mut World, now_s: f64, dt_s: f64) -> Vec<PriorityRecord> {
        let mut out = Vec::new();
        let params = self.params;
        let mut finished = 0u64;
        // With requests possible, every junction's SPaT states what one could do.
        if self.enabled && self.plans.len() < world.signals.len() {
            for idx in 0..world.signals.len() {
                if let std::collections::btree_map::Entry::Vacant(e) = self.plans.entry(idx) {
                    e.insert(plan_state(world, &world.signals[idx]));
                }
            }
        }
        for (&idx, st) in &mut self.plans {
            let Some(plan) = world.signals.get(idx) else {
                continue;
            };
            let junction = plan.junction.index();
            // Lapse requests that stopped repeating: the vehicle has passed or turned.
            let before = st.requests.len();
            let lapsed: Vec<([u8; 8], Request)> = st
                .requests
                .iter()
                .filter(|(_, r)| now_s - r.last_s > params.request_timeout_s)
                .map(|(n, r)| (*n, *r))
                .collect();
            for (n, r) in &lapsed {
                st.requests.remove(n);
                out.push(PriorityRecord {
                    t: (now_s * 1e9) as SimTime,
                    junction,
                    group: r.group,
                    requester: hex(n),
                    action: "served",
                    seconds: v2xw_core::math::q3(st.extended_s),
                });
            }
            finished += (before - st.requests.len()) as u64;
            // The extension limit starts again with the next green.
            if let Some((i, _)) = plan.phase_at(now_s) {
                if !is_vehicle_green(plan, i) {
                    st.green_extended_s = 0.0;
                }
                if st.phase != Some(i) {
                    st.phase = Some(i);
                    st.phase_since_s = now_s;
                }
            }
            // A NEMA dual-ring junction serves its requests through its rings
            // (`crate::nema`): its clock is not moved here.
            if self.through_rings.contains(&idx) {
                continue;
            }
            let Some((&requester, &req)) = st
                .requests
                .iter()
                .min_by(|a, b| a.1.eta_s.total_cmp(&b.1.eta_s))
            else {
                // No request: walk the offset back to the coordinated one, in greens.
                st.extended_s = 0.0;
                st.announced_extend = false;
                if let Some(base) = st.base_offset
                    && let Some(d) = recover(plan, st, base, now_s, dt_s, &params)
                {
                    let done = {
                        let p = &mut world.signals[idx];
                        p.offset_s += d;
                        wrap_error(p.offset_s - base, p.cycle_s).abs() < 1e-6
                    };
                    if done {
                        world.signals[idx].offset_s = base;
                        st.base_offset = None;
                        out.push(PriorityRecord {
                            t: (now_s * 1e9) as SimTime,
                            junction,
                            group: 0,
                            requester: String::new(),
                            action: "recovered",
                            seconds: 0.0,
                        });
                    }
                }
                continue;
            };
            let Some(moves) = st.groups.get(&req.group) else {
                continue;
            };
            let Some((i, into)) = plan.phase_at(now_s) else {
                continue;
            };
            let green_in = |k: usize| {
                moves.iter().any(|m| {
                    matches!(
                        plan.phases[k].states.get(*m),
                        Some(SignalState::Green | SignalState::GreenYield)
                    )
                })
            };
            let needed = req.eta_s - now_s + params.clearance_margin_s;
            if green_in(i) {
                // How long the green has left, through the phases it continues in.
                let mut left = plan.phases[i].duration_s - into;
                let n = plan.phases.len();
                let mut k = i;
                for _ in 1..n {
                    k = (k + 1) % n;
                    if !green_in(k) {
                        break;
                    }
                    left += plan.phases[k].duration_s;
                }
                let available = params.max_extension_s - st.green_extended_s;
                if left < needed && available > 0.0 {
                    // Held, the clock stands still; it never goes back past the phase's start,
                    // or the lamps would show the phase before again (a hold begun the step a
                    // green came up parked the clock on the red before it).
                    let hold = dt_s.min(available).min(back_room(into));
                    world.signals[idx].offset_s += hold;
                    st.extended_s += hold;
                    st.green_extended_s += hold;
                    if !st.announced_extend {
                        st.announced_extend = true;
                        out.push(PriorityRecord {
                            t: (now_s * 1e9) as SimTime,
                            junction,
                            group: req.group,
                            requester: hex(&requester),
                            action: "extend",
                            seconds: v2xw_core::math::q3((needed - left).min(available)),
                        });
                    }
                }
            } else if st.shown_s(i, into, now_s) >= params.min_green_s
                && is_vehicle_green(plan, i)
                && !clears_pedestrians(plan, st, i)
            {
                // Early green: end this conflicting green now.
                let cut = plan.phases[i].duration_s - into;
                if cut > 1e-6 {
                    world.signals[idx].offset_s -= cut;
                    out.push(PriorityRecord {
                        t: (now_s * 1e9) as SimTime,
                        junction,
                        group: req.group,
                        requester: hex(&requester),
                        action: "early-green",
                        seconds: v2xw_core::math::q3(cut),
                    });
                }
            }
        }
        self.served += finished;
        out
    }
}

fn hex(d: &[u8; 8]) -> String {
    let mut s = String::with_capacity(16);
    for b in d {
        s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        s.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
    }
    s
}

/// Wraps an offset error into `(−cycle/2, cycle/2]`.
fn wrap_error(e: f64, cycle: f64) -> f64 {
    if cycle <= 0.0 {
        return e;
    }
    let mut x = e % cycle;
    if x > cycle / 2.0 {
        x -= cycle;
    } else if x <= -cycle / 2.0 {
        x += cycle;
    }
    x
}

/// The offset change recovery makes this step, or `None` outside a green.
fn recover(
    plan: &SignalPlan,
    st: &PlanState,
    base: f64,
    now_s: f64,
    dt_s: f64,
    p: &PriorityParams,
) -> Option<f64> {
    let e = wrap_error(plan.offset_s - base, plan.cycle_s);
    if e.abs() < 1e-9 {
        return Some(-e);
    }
    let (i, into) = plan.phase_at(now_s)?;
    if !is_vehicle_green(plan, i) || clears_pedestrians(plan, st, i) {
        return None;
    }
    let step = p.recovery_rate * dt_s;
    // A positive error (offset too large) is a clock running behind: run it faster by
    // lowering the offset — but never so far that the green ends before its minimum, counted
    // in what the lamp has shown and not on the clock it is speeding up.
    if e > 0.0 {
        let left = plan.phases[i].duration_s - into;
        let room = (st.shown_s(i, into, now_s) + left - p.min_green_s)
            .max(0.0)
            .min(left);
        Some(-(e.min(step).min(room)))
    } else {
        // Slowed, the clock never goes back past the phase's start.
        Some((-e).min(step).min(back_room(into)))
    }
}

/// How far a clock `into` a phase may be set back and stay in it, seconds: the lamps never
/// go back to the phase before.
fn back_room(into: f64) -> f64 {
    (into - 1e-6).max(0.0)
}

/// Whether phase `i` gives some vehicle movement a green.
fn is_vehicle_green(plan: &SignalPlan, i: usize) -> bool {
    plan.phases[i]
        .states
        .iter()
        .any(|s| matches!(s, SignalState::Green | SignalState::GreenYield))
}

/// Whether phase `i` is clearing pedestrians: a crossing that showed walk in the phase
/// before it no longer does.
fn clears_pedestrians(plan: &SignalPlan, st: &PlanState, i: usize) -> bool {
    let n = plan.phases.len();
    if n < 2 {
        return false;
    }
    let prev = (i + n - 1) % n;
    st.crossings.iter().any(|c| {
        let was = matches!(plan.phases[prev].states.get(*c), Some(SignalState::Green));
        let now = matches!(plan.phases[i].states.get(*c), Some(SignalState::Green));
        was && !now
    })
}

/// The per-plan bookkeeping: each signal group's movements and the crossings.
fn plan_state(world: &World, plan: &SignalPlan) -> PlanState {
    // A movement belongs to the group of the head over the lane that approaches it.
    let mut approach_of: BTreeMap<v2xw_core::ids::LaneId, v2xw_core::ids::LaneId> = BTreeMap::new();
    for c in world.roads.connections() {
        if let Some(via) = c.via {
            approach_of.entry(via).or_insert(c.from_lane);
        }
    }
    let mut groups: BTreeMap<u16, Vec<usize>> = BTreeMap::new();
    let mut crossings = Vec::new();
    for (k, lane) in plan.controlled.iter().enumerate() {
        if world
            .roads
            .try_lane(*lane)
            .is_some_and(|l| l.kind == LaneKind::Crossing)
        {
            crossings.push(k);
        }
        let approach = approach_of.get(lane).copied().unwrap_or(*lane);
        for h in plan
            .heads
            .iter()
            .filter(|h| h.lane == approach || h.lane == *lane)
        {
            groups.entry(h.group).or_default().push(k);
        }
    }
    for v in groups.values_mut() {
        v.sort_unstable();
        v.dedup();
    }
    PlanState {
        groups,
        crossings,
        ..PlanState::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_world::ImportOptions;
    use v2xw_world::procedural::GridParams;

    /// One head group's greens over a run: the lengths of the complete ones, by group.
    #[derive(Default)]
    struct Greens {
        on: BTreeMap<u32, (bool, f64)>,
        done: Vec<(u32, f64, f64)>,
    }

    impl Greens {
        fn sample(&mut self, world: &World, t: f64) {
            for g in world.group_signals() {
                if g.wire_id < 65536 {
                    continue;
                }
                let Some((state, _)) = g.at(t) else { continue };
                let green = matches!(state, SignalState::Green | SignalState::GreenYield);
                let e = self.on.entry(g.wire_id).or_insert((green, f64::NAN));
                if green && !e.0 {
                    e.1 = t;
                } else if !green && e.0 && e.1.is_finite() {
                    self.done.push((g.wire_id, e.1, t - e.1));
                }
                e.0 = green;
            }
        }
    }

    /// A service kept busy with requests from every group, on and off, at the junctions of a
    /// grid: no green the lamps show is shorter than the minimum, whatever the requests do
    /// (session 2's probe saw 0.2 s on the grid).
    #[test]
    fn no_green_is_shorter_than_the_minimum_under_any_requests() {
        let mut world = v2xw_world::procedural::grid(
            &GridParams::legacy().with_size(2, 2).with_signals(true),
            &ImportOptions::default(),
        )
        .expect("grid");
        let mut c = PriorityControllers::new(PriorityParams::default());
        c.set_enabled(true);
        let plans = world.signals.len();
        assert!(plans > 0);
        let groups: Vec<Vec<u16>> = world
            .signals
            .iter()
            .map(|p| {
                let mut g: Vec<u16> = p.heads.iter().map(|h| h.group).collect();
                g.sort_unstable();
                g.dedup();
                g
            })
            .collect();
        let mut rng = 0x5eed_u64;
        let mut next = move |n: u64| {
            rng = rng
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (rng >> 33) % n
        };
        let mut active: Vec<(usize, u16, [u8; 8], f64, f64)> = Vec::new();
        let mut greens = Greens::default();
        let dt = 0.1;
        for step in 0..60_000u64 {
            let t = step as f64 * dt;
            if next(150) == 0 {
                let plan = next(plans as u64) as usize;
                let group = groups[plan][next(groups[plan].len() as u64) as usize];
                let eta = next(30) as f64 + 1.0;
                let mut id = [0u8; 8];
                id[0] = next(255) as u8;
                id[1] = next(255) as u8;
                active.push((plan, group, id, t + eta, t + eta));
            }
            active.retain(|a| a.4 > t);
            for a in &active {
                c.request(&world, a.0, a.2, a.1, (a.3 - t).max(0.0), t);
            }
            let _ = c.step(&mut world, t, dt);
            greens.sample(&world, t);
        }
        assert!(!greens.done.is_empty());
        let shortest = greens
            .done
            .iter()
            .cloned()
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .expect("greens");
        assert!(
            shortest.2 >= PriorityParams::default().min_green_s - 2.0 * dt,
            "group {} green from {:.1} s lasted {:.1} s of {} greens",
            shortest.0,
            shortest.1,
            shortest.2,
            greens.done.len()
        );
    }

    /// The `extend` record says how long the hold asks for, not how much green was left.
    #[test]
    fn an_extend_record_logs_the_extension() {
        let mut world = v2xw_world::procedural::grid(
            &GridParams::legacy().with_size(2, 2).with_signals(true),
            &ImportOptions::default(),
        )
        .expect("grid");
        let params = PriorityParams::default();
        let mut c = PriorityControllers::new(params);
        let plan = world.signals[0].clone();
        let green = plan
            .phases
            .iter()
            .position(|p| p.states.contains(&SignalState::Green))
            .expect("a green");
        let st = plan_state(&world, &plan);
        let (&group, moves) = st
            .groups
            .iter()
            .find(|(_, m)| {
                m.iter()
                    .any(|k| plan.phases[green].states[*k] == SignalState::Green)
                    && !m.iter().any(|k| {
                        plan.phases[(green + 1) % plan.phases.len()].states[*k]
                            == SignalState::Green
                    })
            })
            .expect("a group whose green ends with this phase");
        assert!(!moves.is_empty());
        // 2 s of green left; the car needs 5 s + the 2 s margin: a 5 s extension.
        let start: f64 = plan.phases[..green].iter().map(|p| p.duration_s).sum();
        let t = plan.offset_s + start + plan.phases[green].duration_s - 2.0;
        c.request(&world, 0, [1; 8], group, 5.0, t);
        let recs = c.step(&mut world, t, 0.1);
        let e = recs
            .iter()
            .find(|r| r.action == "extend")
            .expect("an extend");
        assert!((e.seconds - 5.0).abs() < 1e-2, "{e:?}");
    }

    /// Recovery slowing a clock that has just entered a green never sets it back into the
    /// phase before: the lamps cannot go back (the 0.1 s greens the service flashed at a
    /// phase start). Fault: `back_room` left out of the slowing arm -> red.
    #[test]
    fn a_recovering_clock_never_goes_back_a_phase() {
        let world = v2xw_world::procedural::grid(
            &GridParams::legacy().with_size(2, 2).with_signals(true),
            &ImportOptions::default(),
        )
        .expect("grid");
        let mut plan = world.signals[0].clone();
        let st = plan_state(&world, &plan);
        let green = (0..plan.phases.len())
            .find(|&k| is_vehicle_green(&plan, k) && !clears_pedestrians(&plan, &st, k))
            .expect("a green");
        let start: f64 = plan.phases[..green].iter().map(|p| p.duration_s).sum();
        for into in [0.004, 0.01, 0.015] {
            let now = plan.offset_s + start + into;
            assert_eq!(plan.phase_at(now).map(|p| p.0), Some(green));
            // The clock is ahead of its base: recovery slows it.
            let base = plan.offset_s + 5.0;
            let d = recover(&plan, &st, base, now, 0.1, &PriorityParams::default())
                .expect("a green recovers");
            assert!(d >= 0.0);
            plan.offset_s += d;
            assert_eq!(
                plan.phase_at(now).map(|p| p.0),
                Some(green),
                "{into} s into the green, slowed by {d} s"
            );
            plan.offset_s -= d;
        }
    }

    #[test]
    fn an_offset_error_wraps_to_the_nearer_way_round() {
        assert!((wrap_error(80.0, 90.0) + 10.0).abs() < 1e-9);
        assert!((wrap_error(-80.0, 90.0) - 10.0).abs() < 1e-9);
        assert!((wrap_error(5.0, 90.0) - 5.0).abs() < 1e-9);
    }
}
