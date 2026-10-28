//! The event-driven and infrastructure message services: DENM, SPaT, MAP, SRM and SSM.
//!
//! The awareness messages (CAM, BSM) are periodic or dynamics-triggered and live in
//! [`crate::generate`]. The messages here are sent because something happened or because a
//! roadside unit's controller said so, and each one's trigger is stated below with where it
//! comes from and how sure it is.
//!
//! | Message | Who sends it | Trigger | Source |
//! |---|---|---|---|
//! | DENM `dangerousSituation(99)` / `emergencyElectronicBrakeLights(1)` | a vehicle running `denm` | its emergency stop signal comes on: deceleration of at least [`ESS_ON_MPS2`] (6 m/s²), off again below [`ESS_OFF_MPS2`] (2.5 m/s²) | C2C-CC RS 2003 *Dangerous situation* R1.6.10 (2026-07-24) RS_tcDaSi_167 a): the trigger is the electronic emergency brake light request of UN R48 / R13-H, whose emergency stop signal is not activated below 6 m/s² for M1 and N1 vehicles and is deactivated below 2.5 m/s² (UN ECE GRRF documents) |
//! | DENM `stationaryVehicle(94)` / `vehicleBreakdown(2)` | a vehicle running `denm` | its hazard lights are on, it stands, and the Triggering Timer — 30 s, less 10 s for each of automatic transmission in park and parking brake on, so [`STATIONARY_TRIGGER_TIMER`] — has run out; cancelled when the hazards go out or it has moved for 5 s | C2C-CC RS 2006 *Stationary vehicle* R1.6.10 §3.2 (RS_tcStVe_139–155) |
//! | DENM `dangerousEndOfQueue(27)` (sudden speed drop) and `trafficCondition(1)` (local slow down) | a vehicle running `denm` with `messages.denm_traffic_condition` (off by default) | see [`crate::denm_triggers`] | C2C-CC RS 2007 *Traffic condition* R1.6.10 (2026-07-24) |
//! | DENM `adhesion(6)` (traction loss) | a vehicle running `denm` with `messages.denm_traction_loss` (off by default) | see [`crate::denm_triggers`] | C2C-CC RS 2002 *Adverse weather* R1.6.10 §3.3 (2026-07-24) |
//! | SPaT / SPATEM | a roadside unit running `spat` | every [`crate::generate::SPAT_INTERVAL`] | CTI 4501 via 04-models.md §8.1 |
//! | MAP / MAPEM | a roadside unit running `map` | every [`crate::generate::MAP_INTERVAL`] | likewise |
//! | SRM / SREM | a vehicle running `srm` (the wiring gives it to emergency vehicles) | a junction whose MAP it heard is within [`SRM_RANGE_M`] ahead; repeated at [`SRM_INTERVAL`] | J2735's signal request; the range and interval are this build's choice |
//! | SSM / SSEM | a roadside unit running `ssm` | a signal request it received; at most one per [`SSM_MIN_INTERVAL`] | J2735's signal status |
//!
//! # What a unit is told, and what it decides
//!
//! A roadside unit's SPaT and MAP payloads are installed by its controller feed
//! ([`EventServices::set_infra_payload`]): the unit signs and sends what the traffic signal
//! controller and the operator's survey gave it, which is what a deployed RSU does. A
//! vehicle's own deceleration arrives the same way ([`EventServices::set_own_acceleration`])
//! — it is the vehicle's own accelerometer, not a view of anyone else.
//!
//! # What is not triggered, and why
//!
//! * A queued vehicle raises no `stationaryVehicle`: without its hazard lights on it is
//!   traffic, not a hazard. The scenario timeline's `safety.breakdown` is what switches a
//!   vehicle's hazards on.
//! * The emergency-brake-light DENM, as C2C-CC RS 2003 §3.1 sets it (read 2026-10-06 from
//!   car-2-car.org): no repetition; instead an **update every 100 ms** while the emergency
//!   stop signal is on, each refreshing `detectionTime` and the event position; validity
//!   2 s; `relevanceDistance` lessThan500m; no cancellation and no negation — when the
//!   signal goes off the updates simply stop and receivers let it lapse.
//! * The broken-down-vehicle DENM, as RS 2006 §3.2 sets it: new, updated and cancelled
//!   DENMs are each **repeated every 1 s for 15 s**; an update every 15 s while the vehicle
//!   stands; validity 30 s (ignition on); `relevanceDistance` lessThan1000m.
//! * Relevance direction: both documents ask for `allTrafficDirections` unless the road
//!   type is a structurally separated one (then upstream); every road this build's worlds
//!   mark is urban without separation, so both send `allTrafficDirections`. Both carry the
//!   path the vehicle drove to the event as their trace — its own path history.
//! * The US stack has no DENM: there the same braking sets the BSM's `eventHardBraking`
//!   flag at J2735's 0.4 g ([`crate::vehicle`]).
//! * SRM and SSM have no real encoder (build decision D2): their payloads are the validated
//!   size model's placeholder of the modelled length (`codec/size-model/j2735`), which is
//!   why `messages.codec_tier` must be `size-model` to select them. A unit acknowledges a
//!   request here and hands it to its junction's controller, which serves it in the
//!   engine (`v2xw_engine::priority`, NTCIP 1211).

use std::collections::BTreeMap;

use v2xw_core::belief::PositionEstimate;
use v2xw_core::geom::Vec3;
use v2xw_core::time::{Duration, SimTime};
use v2xw_msg::MsgType;
use v2xw_msg::codec::{Message, MessageCodec};
use v2xw_msg::denm::{self, DenmAction, DenmCause, DenmInput, DenmService, EventId, Repetition};
use v2xw_msg::size_model::{ContentProfile, J2735SizeCodec, SizeRequest};

pub use crate::denm_triggers::DenmTriggers;
use crate::denm_triggers::{TractionAction, TriggerState};
use crate::generate::ServiceSet;
use crate::runtime::{NodeConfig, VerifiedMessage};
use crate::stores::{CredentialHandle, VerificationState};

/// How far ahead a junction whose MAP a vehicle heard may be for it to request priority,
/// metres. This build's choice: about twelve seconds at an emergency vehicle's urban speed.
pub const SRM_RANGE_M: f64 = 300.0;

/// How often a vehicle repeats its signal request while it approaches. This build's choice.
pub const SRM_INTERVAL: Duration = Duration::from_secs(1);

/// The closest together two signal statuses from one unit may be. This build's choice.
pub const SSM_MIN_INTERVAL: Duration = Duration::from_millis(100);

/// How long a heard MAP stays usable for a priority request without being heard again.
pub const MAP_MEMORY: Duration = Duration::from_secs(5);

/// The emergency stop signal comes on at this deceleration, m/s²: UN R48 / R13-H's 6 m/s²
/// for M1 and N1 vehicles, below which it shall not be activated.
pub const ESS_ON_MPS2: f64 = 6.0;
/// ... and goes off, at the latest, when the deceleration falls below this, m/s².
pub const ESS_OFF_MPS2: f64 = 2.5;

/// The emergency-brake-light DENM's update interval while the signal is on (C2C-CC
/// RS_tcDaSi_174). Also its `transmissionInterval`.
pub const DENM_REPETITION: Duration = Duration::from_millis(100);
/// The emergency-brake-light DENM's validity (C2C-CC RS_tcDaSi_177).
pub const DENM_VALIDITY: Duration = Duration::from_secs(2);

/// The broken-down-vehicle Triggering Timer: 30 s, less 10 s for the automatic
/// transmission in park and 10 s for the parking brake (C2C-CC RS_tcStVe_142 a and c) —
/// what a driver whose car has broken down does on stopping. Opening a door would set it
/// to 0 s; the simulated drivers stay in the car.
pub const STATIONARY_TRIGGER_TIMER: Duration = Duration::from_secs(10);
/// Former name of [`STATIONARY_TRIGGER_TIMER`].
pub const STATIONARY_DWELL: Duration = STATIONARY_TRIGGER_TIMER;

/// New, updated and cancelled broken-down-vehicle DENMs are repeated at this interval
/// (C2C-CC RS_tcStVe_154) ...
pub const STATIONARY_REPETITION: Duration = Duration::from_secs(1);
/// ... for this long.
pub const STATIONARY_REPETITION_DURATION: Duration = Duration::from_secs(15);
/// The broken-down-vehicle DENM is updated this often while the vehicle stands.
pub const STATIONARY_UPDATE: Duration = Duration::from_secs(15);
/// The broken-down-vehicle DENM's validity with the ignition on (C2C-CC RS_tcStVe_155).
pub const STATIONARY_VALIDITY: Duration = Duration::from_secs(30);
/// A vehicle that has moved this long is no longer a broken-down vehicle (RS_tcStVe_126 a).
pub const STATIONARY_MOVING_CANCEL: Duration = Duration::from_secs(5);

/// `DangerousSituationSubCauseCode` `emergencyElectronicBrakeEngaged(1)`.
pub const SUB_CAUSE_EEBL: u8 = 1;
/// `StationaryVehicleSubCauseCode` `vehicleBreakdown(2)`.
pub const SUB_CAUSE_BREAKDOWN: u8 = 2;

/// ETSI `MessageId` of a SREM, `srem(9)` (ETSI TS 102 894-2).
pub const SREM_MESSAGE_ID: u8 = 9;
/// ETSI `MessageId` of a SSEM, `ssem(10)` (ETSI TS 102 894-2).
pub const SSEM_MESSAGE_ID: u8 = 10;

/// The own vehicle as the traffic-condition relevance tests see it: its believed position
/// and heading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ego {
    /// Believed position, world ENU metres.
    pub pos: Vec3,
    /// Believed heading, ENU radians.
    pub heading: f64,
}

/// The DENM fields a WP-12 trigger sets for its event ([`crate::denm_triggers`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct DenmParams {
    validity: Duration,
    interval: Duration,
    distance: denm::AwarenessDistance,
    direction: denm::RelevanceDirection,
    quality: u8,
}

/// One event message due now: what to build.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventRequest {
    /// A DENM for this lifecycle action.
    Denm(DenmAction),
    /// A signal request toward the junction whose reference point this is.
    Srm(Vec3),
    /// A signal status answering this many requests.
    Ssm(u32),
}

impl EventRequest {
    /// The message type it becomes.
    pub const fn msg_type(&self) -> MsgType {
        match self {
            EventRequest::Denm(_) => MsgType::Denm,
            EventRequest::Srm(_) => MsgType::Srm,
            EventRequest::Ssm(_) => MsgType::Ssm,
        }
    }
}

/// The node's event and infrastructure services' state.
#[derive(Debug, Clone, Default)]
pub struct EventServices {
    /// The SPaT and MAP payloads the unit's controller feed installed.
    spat: Option<Vec<u8>>,
    map: Option<Vec<u8>>,
    /// The WSA a roadside unit advertises its services in (IEEE 1609.3; WP-19).
    wsa: Option<Vec<u8>>,
    /// The vehicle's own longitudinal acceleration, m/s², from its own accelerometer.
    own_accel: Option<f64>,
    /// Whether the emergency stop signal is on: on at [`ESS_ON_MPS2`], off below
    /// [`ESS_OFF_MPS2`].
    braking: bool,
    /// The live emergency-brake-light event and when it was last updated.
    eebl_event: Option<(EventId, SimTime)>,
    /// When the live stationary event was last updated, and since when the vehicle moves.
    stationary_updated: Option<SimTime>,
    moving_since: Option<SimTime>,
    /// The DEN basic service, created for the station id the first event is raised under.
    denm: Option<DenmService>,
    /// The detection instant and position of each live event, for building its DENM.
    denm_events: BTreeMap<EventId, (SimTime, PositionEstimate)>,
    /// Junctions whose MAP this vehicle heard: reference point → when last heard.
    maps_heard: BTreeMap<(i64, i64), (Vec3, SimTime)>,
    last_srm: Option<SimTime>,
    /// Signal requests received and not yet answered, and when the last status went.
    requests_pending: u32,
    last_ssm: Option<SimTime>,
    /// DENMs raised, for a test and a report.
    denm_raised: u64,
    /// The hazard switch.
    hazard_lights: bool,
    /// The vehicle's own wheel speed, m/s.
    own_speed: Option<f64>,
    /// When the vehicle came to a stand, while it stands.
    standing_since: Option<SimTime>,
    /// The live stationary-vehicle event, if one is raised.
    stationary_event: Option<EventId>,
    /// The cause of each live event.
    denm_causes: BTreeMap<EventId, (DenmCause, u8)>,
    /// The stationary-vehicle DENM's repetition instruction when the scenario sets one
    /// (`net.geonet.repetition_*`); [`STATIONARY_REPETITION`] for
    /// [`STATIONARY_REPETITION_DURATION`] otherwise.
    stationary_repetition: Option<Repetition>,
    /// The optional WP-12 triggers, and their detectors' state.
    triggers: DenmTriggers,
    trigger_state: TriggerState,
    /// The world anchor, for a received DENM's event position.
    origin: Option<v2xw_core::geo::GeoOrigin>,
    /// What the navigation map says of the road now (urban or not).
    own_non_urban: Option<bool>,
    /// The live traction-loss event.
    traction_event: Option<EventId>,
    /// The fields of the WP-12 triggers' events.
    denm_params: BTreeMap<EventId, DenmParams>,
}

impl EventServices {
    /// Installs the payload the unit's controller feed produced for `ty` (SPaT or MAP),
    /// or the unit's encoded WSA. Any other type is ignored.
    pub fn set_infra_payload(&mut self, ty: MsgType, bytes: Vec<u8>) {
        match ty {
            MsgType::Spat => self.spat = Some(bytes),
            MsgType::Map => self.map = Some(bytes),
            MsgType::Wsa => self.wsa = Some(bytes),
            _ => {}
        }
    }

    /// The installed payload for `ty`, when there is one.
    pub fn infra_payload(&self, ty: MsgType) -> Option<&[u8]> {
        match ty {
            MsgType::Spat => self.spat.as_deref(),
            MsgType::Map => self.map.as_deref(),
            MsgType::Wsa => self.wsa.as_deref(),
            _ => None,
        }
    }

    /// The vehicle's own longitudinal acceleration, m/s² (negative when braking).
    pub fn set_own_acceleration(&mut self, a_mps2: f64) {
        self.own_accel = Some(a_mps2);
    }

    /// How many DENM events this node has raised.
    pub fn denm_raised(&self) -> u64 {
        self.denm_raised
    }

    /// The stationary-vehicle DENM's repetition (EN 302 637-3 6.1.4.2): `interval`
    /// for `duration`; a zero interval sends each DENM once.
    pub fn set_stationary_repetition(&mut self, interval: Duration, duration: Duration) {
        self.stationary_repetition = Some(Repetition::new(interval, duration));
    }

    /// Switches the WP-12 DENM triggers on (`messages.denm_traffic_condition`,
    /// `messages.denm_traction_loss`) for a world anchored at `origin`.
    pub fn set_triggers(&mut self, triggers: DenmTriggers, origin: v2xw_core::geo::GeoOrigin) {
        self.triggers = triggers;
        self.origin = Some(origin);
    }

    /// The WP-12 DENM triggers this node runs.
    pub fn triggers(&self) -> DenmTriggers {
        self.triggers
    }

    /// The vehicles the own sensors report, for the traffic-condition sensor conditions.
    pub fn set_perceived(&mut self, objects: &[crate::cpm::ObjectIn]) {
        if self.triggers.traffic_condition {
            self.trigger_state.set_perceived(
                objects
                    .iter()
                    .filter(|o| o.kind == crate::cpm::ObjectKind::Vehicle)
                    .map(|o| (o.pos, o.vel, o.heading_rad))
                    .collect(),
            );
        }
    }

    /// The own vehicle's friction estimate, when its ABS has given it one.
    pub fn friction_estimate(&self) -> Option<f64> {
        self.trigger_state.friction_estimate()
    }

    /// The hazard switch.
    pub fn set_hazard_lights(&mut self, on: bool) {
        self.hazard_lights = on;
    }

    /// The vehicle's own bus reading: its wheel speed and its hazard switch.
    pub fn set_vehicle_bus(&mut self, bus: &crate::vehicle::VehicleBus) {
        self.own_speed = Some(bus.speed_mps);
        self.hazard_lights = bus.hazard_lights;
        self.own_non_urban = bus.non_urban;
        if self.triggers.traffic_condition || self.triggers.traction_loss {
            self.trigger_state
                .on_bus(bus, self.triggers.traffic_condition);
        }
    }

    /// Learns from a message the applications received: a MAP is a junction a priority
    /// request can be addressed to, and a signal request is one to answer.
    pub fn on_delivered(&mut self, m: &VerifiedMessage) {
        if matches!(
            m.verification,
            VerificationState::Invalid | VerificationState::Revoked
        ) {
            return;
        }
        match m.msg_type {
            MsgType::Map => {
                if let Some(p) = m.claimed_pos {
                    let key = (p.x.round() as i64, p.y.round() as i64);
                    self.maps_heard.insert(key, (p, m.received_at));
                }
            }
            MsgType::Srm => self.requests_pending = self.requests_pending.saturating_add(1),
            _ => {}
        }
        if self.triggers.traffic_condition
            && let Some(signer) = m.signer.as_ref()
        {
            let key = crate::safety::digest_key(signer);
            match m.msg_type {
                MsgType::Cam => {
                    if let Some(p) = m.claimed_pos {
                        self.trigger_state.on_cam(
                            key,
                            p,
                            m.claimed_speed_mps,
                            m.claimed_heading_rad,
                            m.received_at,
                        );
                    }
                }
                MsgType::Denm => {
                    if let (Some(payload), Some(origin)) = (m.payload.as_deref(), self.origin)
                        && let Some((cause, pos)) = traffic_denm(payload, origin)
                    {
                        self.trigger_state.on_denm(key, cause, pos, m.received_at);
                    }
                }
                _ => {}
            }
        }
    }

    /// Every event message due at `now` on this node's clock. `station_id` is the identifier
    /// the node is transmitting under now, the one a new event's `actionId` carries.
    pub fn due(
        &mut self,
        now: SimTime,
        belief: &PositionEstimate,
        services: ServiceSet,
        station_id: Option<u32>,
    ) -> Vec<EventRequest> {
        let mut out = Vec::new();
        if services.denm {
            // A new pseudonym is a new station id for new events; an event already raised
            // keeps its `actionId` to the end (EN 302 637-3 §8.2.1.5), so the service is
            // replaced only when it has nothing alive.
            if let Some(sid) = station_id
                && self
                    .denm
                    .as_ref()
                    .is_none_or(|s| s.station_id() != sid && s.active_len() == 0)
            {
                self.denm = Some(DenmService::new(sid));
            }
            self.raise_on_hard_braking(now, belief);
            self.raise_on_stationary(now, belief);
            if self.triggers.traffic_condition {
                self.raise_on_traffic_condition(now, belief);
            }
            if self.triggers.traction_loss {
                self.raise_on_traction_loss(now, belief);
            }
            if let Some(service) = self.denm.as_mut() {
                for action in service.poll(now) {
                    out.push(EventRequest::Denm(action));
                }
                // Forget the events the service has dropped — but only once their last
                // action (a termination included) has been built.
                let service = &*service;
                let polled: Vec<EventId> = out
                    .iter()
                    .filter_map(|r| match r {
                        EventRequest::Denm(a) => Some(a.event()),
                        _ => None,
                    })
                    .collect();
                self.denm_events
                    .retain(|id, _| service.is_active(*id) || polled.contains(id));
                self.denm_causes
                    .retain(|id, _| service.is_active(*id) || polled.contains(id));
                self.denm_params
                    .retain(|id, _| service.is_active(*id) || polled.contains(id));
            }
        }
        if services.srm
            && let Some(target) = self.priority_target(now, belief)
            && self
                .last_srm
                .is_none_or(|t| now.saturating_sub(t) >= SRM_INTERVAL.as_nanos())
        {
            self.last_srm = Some(now);
            out.push(EventRequest::Srm(target));
        }
        if services.ssm
            && self.requests_pending > 0
            && self
                .last_ssm
                .is_none_or(|t| now.saturating_sub(t) >= SSM_MIN_INTERVAL.as_nanos())
        {
            self.last_ssm = Some(now);
            out.push(EventRequest::Ssm(core::mem::take(
                &mut self.requests_pending,
            )));
        }
        out
    }

    /// The electronic emergency brake light (C2C-CC RS 2003 §3.1): a new DENM when the
    /// emergency stop signal comes on, an update every [`DENM_REPETITION`] while it stays
    /// on (each refreshing the detection instant and the position), and nothing — no
    /// cancellation — when it goes off.
    fn raise_on_hard_braking(&mut self, now: SimTime, belief: &PositionEstimate) {
        let Some(a) = self.own_accel else {
            return;
        };
        let on = if self.braking {
            a <= -ESS_OFF_MPS2
        } else {
            a <= -ESS_ON_MPS2
        };
        let Some(service) = self.denm.as_mut() else {
            self.braking = on;
            return;
        };
        if let Some((id, _)) = self.eebl_event
            && !service.is_active(id)
        {
            self.eebl_event = None;
        }
        match (on, self.eebl_event) {
            (true, None) if belief.fix.has_position() => {
                let id = service.create(now, DENM_VALIDITY, None);
                self.denm_events.insert(id, (now, *belief));
                self.denm_causes
                    .insert(id, (DenmCause::DangerousSituation, SUB_CAUSE_EEBL));
                self.eebl_event = Some((id, now));
                self.denm_raised += 1;
            }
            (true, Some((id, last)))
                if now.saturating_sub(last) >= DENM_REPETITION.as_nanos()
                    && belief.fix.has_position() =>
            {
                if service.refresh(id, now) {
                    self.denm_events.insert(id, (now, *belief));
                    self.eebl_event = Some((id, now));
                }
            }
            (false, Some((id, _))) => {
                service.stop(id);
                self.eebl_event = None;
            }
            _ => {}
        }
        self.braking = on;
    }

    /// Raises a `stationaryVehicle` / `vehicleBreakdown` event once the vehicle has stood
    /// [`STATIONARY_DWELL`] with its hazards on, and cancels it when it moves off or the
    /// hazards go out (EN 302 637-3: the originator terminates its own event).
    ///
    /// The C2C-CC broken-down-vehicle service (RS 2006 §3.2): raised when the hazards are
    /// on, the vehicle stands and the [`STATIONARY_TRIGGER_TIMER`] has run; updated every
    /// [`STATIONARY_UPDATE`]; cancelled when the hazards go out or the vehicle has moved
    /// for [`STATIONARY_MOVING_CANCEL`]; new, updated and cancelled DENMs each repeated
    /// every [`STATIONARY_REPETITION`] for [`STATIONARY_REPETITION_DURATION`].
    fn raise_on_stationary(&mut self, now: SimTime, belief: &PositionEstimate) {
        let standing = self.own_speed.is_some_and(|v| v < 0.1);
        if standing {
            self.standing_since.get_or_insert(now);
            self.moving_since = None;
        } else {
            self.standing_since = None;
            self.moving_since.get_or_insert(now);
        }
        let Some(service) = self.denm.as_mut() else {
            return;
        };
        // An event that ran out its validity is gone; forget it so a breakdown that
        // outlasts it is raised again.
        if let Some(id) = self.stationary_event
            && !service.is_active(id)
        {
            self.stationary_event = None;
        }
        let repetition = self.stationary_repetition.unwrap_or(Repetition::new(
            STATIONARY_REPETITION,
            STATIONARY_REPETITION_DURATION,
        ));
        let moved_long = self
            .moving_since
            .is_some_and(|t| now.saturating_sub(t) >= STATIONARY_MOVING_CANCEL.as_nanos());
        match self.stationary_event {
            Some(id) if !self.hazard_lights || moved_long => {
                service.cancel_repeated(id, now, repetition);
                self.stationary_event = None;
                self.stationary_updated = None;
            }
            Some(id)
                if standing
                    && self
                        .stationary_updated
                        .is_some_and(|t| now.saturating_sub(t) >= STATIONARY_UPDATE.as_nanos()) =>
            {
                if service.refresh(id, now) {
                    self.denm_events.insert(id, (now, *belief));
                    self.stationary_updated = Some(now);
                }
            }
            None if self.hazard_lights
                && standing
                && belief.fix.has_position()
                && self.standing_since.is_some_and(|t| {
                    now.saturating_sub(t) >= STATIONARY_TRIGGER_TIMER.as_nanos()
                }) =>
            {
                let id = service.create(now, STATIONARY_VALIDITY, Some(repetition));
                self.denm_events.insert(id, (now, *belief));
                self.denm_causes
                    .insert(id, (DenmCause::StationaryVehicle, SUB_CAUSE_BREAKDOWN));
                self.stationary_event = Some(id);
                self.stationary_updated = Some(now);
                self.denm_raised += 1;
            }
            _ => {}
        }
    }

    /// Raises a new event of `cause` with the WP-12 `params` and `repetition`.
    fn raise_with(
        &mut self,
        now: SimTime,
        belief: &PositionEstimate,
        cause: DenmCause,
        params: DenmParams,
        repetition: Repetition,
    ) -> Option<EventId> {
        let service = self.denm.as_mut()?;
        let id = service.create(now, params.validity, Some(repetition));
        self.denm_events.insert(id, (now, *belief));
        self.denm_causes.insert(id, (cause, 0));
        self.denm_params.insert(id, params);
        self.denm_raised += 1;
        Some(id)
    }

    /// The C2C-CC traffic-condition services, sudden speed drop and local slow down
    /// ([`crate::denm_triggers`]): each a new DENM, never updated or cancelled, at most one
    /// per detection blocking time.
    fn raise_on_traffic_condition(&mut self, now: SimTime, belief: &PositionEstimate) {
        if self.denm.is_none() || !belief.fix.has_position() {
            return;
        }
        let ego = Ego {
            pos: belief.pos,
            heading: belief.heading_rad,
        };
        if let Some(quality) = self.trigger_state.sudden_speed_drop(now, &ego) {
            self.raise_with(
                now,
                belief,
                DenmCause::DangerousEndOfQueue,
                DenmParams {
                    validity: crate::denm_triggers::DROP_VALIDITY,
                    interval: crate::denm_triggers::DROP_REPETITION,
                    distance: denm::AwarenessDistance::LessThan1000m,
                    direction: denm::RelevanceDirection::Upstream,
                    quality,
                },
                Repetition::new(
                    crate::denm_triggers::DROP_REPETITION,
                    crate::denm_triggers::DROP_REPETITION_DURATION,
                ),
            );
        }
        let stationary = self.stationary_event.is_some();
        if let Some(quality) = self.trigger_state.local_slow_down(now, &ego, stationary) {
            self.raise_with(
                now,
                belief,
                DenmCause::TrafficCondition,
                DenmParams {
                    validity: crate::denm_triggers::SLOW_VALIDITY,
                    interval: crate::denm_triggers::SLOW_REPETITION,
                    distance: denm::AwarenessDistance::LessThan1000m,
                    direction: denm::RelevanceDirection::Upstream,
                    quality,
                },
                Repetition::new(
                    crate::denm_triggers::SLOW_REPETITION,
                    crate::denm_triggers::SLOW_REPETITION_DURATION,
                ),
            );
        }
    }

    /// The C2C-CC traction-loss service ([`crate::denm_triggers`]): a new DENM, then an
    /// update at least every 0.1 s while the low friction persists and one when it ends.
    fn raise_on_traction_loss(&mut self, now: SimTime, belief: &PositionEstimate) {
        let Some(service) = self.denm.as_ref() else {
            return;
        };
        if let Some(id) = self.traction_event
            && !service.is_active(id)
        {
            self.traction_event = None;
        }
        let urban = self.own_non_urban == Some(false);
        let (validity, interval, duration) = if urban {
            (
                crate::denm_triggers::TRACTION_VALIDITY_URBAN,
                crate::denm_triggers::TRACTION_REPETITION_URBAN,
                crate::denm_triggers::TRACTION_REPETITION_DURATION_URBAN,
            )
        } else {
            (
                crate::denm_triggers::TRACTION_VALIDITY,
                crate::denm_triggers::TRACTION_REPETITION,
                crate::denm_triggers::TRACTION_REPETITION_DURATION,
            )
        };
        let live = self.traction_event.is_some();
        let Some(action) = self.trigger_state.traction_loss(now, live) else {
            return;
        };
        if !belief.fix.has_position() {
            // RS_tcAdWe_169: an update that cannot be built ends the updates.
            return;
        }
        let params = DenmParams {
            validity,
            interval,
            distance: denm::AwarenessDistance::LessThan1000m,
            direction: denm::RelevanceDirection::All,
            quality: match action {
                TractionAction::New(q) | TractionAction::Update(q) => q,
            },
        };
        match action {
            TractionAction::New(_) => {
                self.traction_event = self.raise_with(
                    now,
                    belief,
                    DenmCause::Adhesion,
                    params,
                    Repetition::new(interval, duration),
                );
            }
            TractionAction::Update(_) => {
                if let Some(id) = self.traction_event
                    && let Some(service) = self.denm.as_mut()
                    && service.refresh(id, now)
                {
                    // detectionTime, eventPosition and informationQuality are those of
                    // the current event point (RS_tcAdWe_177).
                    self.denm_events.insert(id, (now, *belief));
                    self.denm_params.insert(id, params);
                }
            }
        }
    }

    /// The nearest junction ahead whose MAP was heard recently, within [`SRM_RANGE_M`].
    fn priority_target(&mut self, now: SimTime, belief: &PositionEstimate) -> Option<Vec3> {
        self.maps_heard
            .retain(|_, (_, heard)| now.saturating_sub(*heard) <= MAP_MEMORY.as_nanos());
        if !belief.fix.has_position() {
            return None;
        }
        let (hx, hy) = (
            v2xw_core::math::cos(belief.heading_rad),
            v2xw_core::math::sin(belief.heading_rad),
        );
        let mut best: Option<(f64, Vec3)> = None;
        for (p, _) in self.maps_heard.values() {
            let (dx, dy) = (p.x - belief.pos.x, p.y - belief.pos.y);
            let d2 = dx * dx + dy * dy;
            let ahead = dx * hx + dy * hy > 0.0;
            if ahead && d2 <= SRM_RANGE_M * SRM_RANGE_M && best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, *p));
            }
        }
        best.map(|(_, p)| p)
    }

    /// The payload for one event request, or `None` when it cannot be built.
    ///
    /// The station id is the first four octets of the active pseudonym's digest, as for a
    /// CAM, so the identifier on the air changes with the pseudonym.
    pub fn encode(
        &self,
        request: &EventRequest,
        now: SimTime,
        cred: &CredentialHandle,
        config: &NodeConfig,
        own: &crate::vehicle::OwnVehicle,
    ) -> Option<Vec<u8>> {
        let mut id = [0u8; 4];
        id.copy_from_slice(&cred.digest.0[..4]);
        let station_id = u32::from_be_bytes(id);
        match request {
            EventRequest::Denm(action) => {
                let event = action.event();
                let (detected_at, position) = self.denm_events.get(&event).copied()?;
                let (cause, sub) = self
                    .denm_causes
                    .get(&event)
                    .copied()
                    .unwrap_or((DenmCause::DangerousSituation, 0));
                let mut input = DenmInput::new(
                    event,
                    station_id,
                    config.station_type,
                    v2xw_msg::cam::timestamp_its(config.wall, detected_at).ok()?,
                    position,
                    config.origin,
                    cause,
                );
                input.sub_cause = sub;
                input.reference_time = v2xw_msg::cam::timestamp_its(config.wall, now).ok()?;
                let stationary = cause == DenmCause::StationaryVehicle;
                input.validity = if stationary {
                    STATIONARY_VALIDITY
                } else {
                    DENM_VALIDITY
                };
                input.transmission_interval = Some(if stationary {
                    STATIONARY_REPETITION
                } else {
                    DENM_REPETITION
                });
                // C2C-CC: lessThan1000m for a broken-down vehicle (RS_tcStVe_150 table),
                // lessThan500m for the emergency brake light (RS_tcDaSi_177); all traffic
                // directions on a road not structurally separated, which every road here is.
                input.awareness_distance = Some(if stationary {
                    denm::AwarenessDistance::LessThan1000m
                } else {
                    denm::AwarenessDistance::LessThan500m
                });
                input.traffic_direction = Some(denm::RelevanceDirection::All);
                // C2C-CC RS_tcDaSi_169: the request detected and a filtered deceleration
                // past 4 m/s² is quality 2; RS_tcStVe_145: hazard lights and a standstill,
                // with park and the parking brake (conditions a and c), is quality 2.
                input.information_quality = 2;
                // The WP-12 triggers set their own fields ([`crate::denm_triggers`]).
                if let Some(p) = self.denm_params.get(&event) {
                    input.validity = p.validity;
                    input.transmission_interval = Some(p.interval);
                    input.awareness_distance = Some(p.distance);
                    input.traffic_direction = Some(p.direction);
                    input.information_quality = p.quality;
                }
                // The trace: the vehicle's own path up to the event.
                let points = own.history_points(&position);
                input.trace = v2xw_msg::j2945::etsi_path_deltas(
                    &points,
                    position.pos,
                    position.time_ns,
                    config.origin,
                    v2xw_msg::cam::MAX_PATH_POINTS,
                );
                let message = match action {
                    DenmAction::Termination(_, kind) => denm::build_termination_denm(&input, *kind),
                    _ => denm::build_denm(&input),
                }
                .ok()?;
                Some(denm::encode_denm(&message).ok()?.bytes)
            }
            EventRequest::Srm(_) => sized(MsgType::Srm, 1, config, station_id),
            EventRequest::Ssm(n) => sized(MsgType::Ssm, (*n).clamp(1, 8), config, station_id),
        }
    }
}

/// A received DENM's traffic-condition cause code (1, or 27) and event position, when it
/// is one and is not a termination.
fn traffic_denm(payload: &[u8], origin: v2xw_core::geo::GeoOrigin) -> Option<(u8, Vec3)> {
    let d = v2xw_msg::denm::decode_denm(payload).ok()?;
    let m = &d.denm.management;
    if m.termination.is_some() {
        return None;
    }
    let cause = match d.denm.situation.as_ref()?.event_type.cc_and_scc {
        v2xw_msg::asn1::cdd::CauseCodeChoice::trafficCondition1(_) => {
            crate::denm_triggers::CAUSE_TRAFFIC_CONDITION
        }
        v2xw_msg::asn1::cdd::CauseCodeChoice::dangerousEndOfQueue27(_) => {
            crate::denm_triggers::CAUSE_DANGEROUS_END_OF_QUEUE
        }
        _ => return None,
    };
    let lat = f64::from(m.event_position.latitude.0) * 1e-7;
    let lon = f64::from(m.event_position.longitude.0) * 1e-7;
    Some((cause, origin.to_enu(lat, lon, 0.0)))
}

/// A size-model payload: the validated modelled length of a J2735 SRM or SSM, with the ETSI
/// `ItsPduHeader` in front on the ETSI stack.
fn sized(ty: MsgType, elements: u32, config: &NodeConfig, station_id: u32) -> Option<Vec<u8>> {
    let encoded = J2735SizeCodec::new()
        .encode(&Message::Modeled(SizeRequest {
            ty,
            profile: ContentProfile::Typical,
            elements,
        }))
        .ok()?;
    if !config.etsi_facilities {
        return Some(encoded.bytes);
    }
    let message_id = if ty == MsgType::Srm {
        SREM_MESSAGE_ID
    } else {
        SSEM_MESSAGE_ID
    };
    v2xw_msg::j2735::infra::its_wrap(ty, message_id, station_id, &encoded.bytes).ok()
}

#[cfg(test)]
mod wp12_tests {
    //! The WP-12 DENM triggers through the event services, each with the fault that
    //! turns it red listed in `docs/design/frameworks/00-adoption-plan.md` (WP-12).

    use super::*;
    use crate::vehicle::VehicleBus;

    const MS: u64 = 1_000_000;
    const STEP: u64 = 100 * MS;

    fn bus(t: SimTime, v: f64, a: f64) -> VehicleBus {
        VehicleBus {
            t,
            speed_mps: v,
            a_long_mps2: a,
            a_lat_mps2: 0.0,
            yaw_rate_rad_s: 0.0,
            hazard_lights: false,
            low_beam: false,
            intent: None,
            heavy: false,
            speed_limit_mps: 29.0,
            non_urban: None,
            abs_active: false,
        }
    }

    fn belief(t: SimTime, x: f64, v: f64) -> PositionEstimate {
        let mut p = PositionEstimate::no_fix(t);
        p.pos = Vec3::new(x, 0.0, 0.0);
        p.vel = Vec3::new(v, 0.0, 0.0);
        p.heading_rad = 0.0;
        p.fix = v2xw_core::belief::FixQuality::ThreeD;
        p
    }

    fn services() -> ServiceSet {
        ServiceSet {
            cam: true,
            denm: true,
            ..ServiceSet::NONE
        }
    }

    fn with(triggers: DenmTriggers) -> EventServices {
        let mut e = EventServices::default();
        e.set_triggers(triggers, v2xw_core::geo::GeoOrigin::new(48.0, 11.0, 0.0));
        e
    }

    const TC: DenmTriggers = DenmTriggers {
        traffic_condition: true,
        traction_loss: false,
    };
    const TL: DenmTriggers = DenmTriggers {
        traffic_condition: false,
        traction_loss: true,
    };

    /// Drives `e` along a speed profile `(v, a)` per 100 ms step, with `each` run before
    /// every step; returns every new DENM as (time, cause, information quality) and the
    /// count of updates.
    fn drive(
        e: &mut EventServices,
        profile: &[(f64, f64)],
        mut each: impl FnMut(&mut EventServices, SimTime, f64),
    ) -> (Vec<(SimTime, DenmCause, u8)>, usize) {
        let mut x = 0.0;
        let mut raised = Vec::new();
        let mut updates = 0;
        for (k, (v, a)) in profile.iter().enumerate() {
            let t = k as u64 * STEP;
            x += v * 0.1;
            each(e, t, x);
            e.set_vehicle_bus(&bus(t, *v, *a));
            for r in e.due(t, &belief(t, x, *v), services(), Some(7)) {
                match r {
                    EventRequest::Denm(DenmAction::Original(id)) => {
                        let cause = e.denm_causes.get(&id).map(|c| c.0);
                        let q = e.denm_params.get(&id).map_or(0, |p| p.quality);
                        raised.push((t, cause.expect("a cause"), q));
                    }
                    EventRequest::Denm(DenmAction::Update(_)) => updates += 1,
                    _ => {}
                }
            }
        }
        (raised, updates)
    }

    /// `n` steps at `v` m/s.
    fn steady(v: f64, n: usize) -> Vec<(f64, f64)> {
        vec![(v, 0.0); n]
    }

    /// Braking at `a` m/s² from `v0` down to `v1`.
    fn brake(v0: f64, v1: f64, a: f64) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        let mut v = v0;
        while v > v1 {
            v = (v - a * 0.1).max(v1);
            out.push((v, -a));
        }
        out
    }

    /// Free flow at 110 km/h for five minutes, among other fast cars, raises nothing.
    #[test]
    fn free_flow_raises_no_traffic_condition_denm() {
        let mut e = with(TC);
        let (raised, _) = drive(&mut e, &steady(30.6, 3_000), |e, t, x| {
            for k in 0..8u8 {
                e.trigger_state.on_cam(
                    [k; 8],
                    Vec3::new(x + 30.0 * f64::from(k), 0.0, 0.0),
                    30.0,
                    0.0,
                    t,
                );
            }
        });
        assert!(raised.is_empty(), "{raised:?}");
    }

    /// RS 2007 local slow down condition 1: a minute at 110 km/h, then a gentle slow-down
    /// into 14 km/h traffic. The 120 s average falls to 30 km/h at about 165 s and the
    /// trafficCondition DENM goes then, once (detection blocking 180 s, and the fast block
    /// has left the 180 s window by the time that ends); a 3 m/s² slow-down is no sudden
    /// speed drop.
    #[test]
    fn a_queue_raises_the_local_slow_down_denm_once() {
        let mut e = with(TC);
        let mut profile = steady(30.6, 600);
        profile.extend(brake(30.6, 4.0, 3.0));
        let fill = 3_600 - profile.len();
        profile.extend(steady(4.0, fill));
        let (raised, _) = drive(&mut e, &profile, |_, _, _| {});
        assert_eq!(raised.len(), 1, "{raised:?}");
        let (t, cause, q) = raised[0];
        assert_eq!(cause, DenmCause::TrafficCondition);
        assert_eq!(q, 1, "vehicle dynamics only (Table 8)");
        let t_s = t as f64 * 1e-9;
        assert!((160.0..=170.0).contains(&t_s), "raised at {t_s} s");
    }

    /// The same queue on a road the map calls urban, without the fast block: nothing.
    #[test]
    fn a_slow_city_street_raises_no_traffic_condition_denm() {
        let mut e = with(TC);
        let mut profile = steady(13.0, 600);
        profile.extend(brake(13.0, 4.0, 3.0));
        let fill = 3_600 - profile.len();
        profile.extend(steady(4.0, fill));
        let (raised, _) = drive(&mut e, &profile, |_, _, _| {});
        assert!(raised.is_empty(), "{raised:?}");
    }

    /// RS 2007 local slow down condition 2: stationary for 30 s among five slow cars whose
    /// CAMs are relevant (same heading, within 500 m): quality 2. Four cars are not enough.
    #[test]
    fn a_standing_queue_among_five_slow_cams_raises_the_denm() {
        let run = |cars: u8| {
            let mut e = with(TC);
            let mut profile = steady(30.6, 400);
            profile.extend(brake(30.6, 0.0, 3.0));
            let fill = 1_200 - profile.len();
            profile.extend(steady(0.0, fill));
            drive(&mut e, &profile, |e, t, x| {
                for k in 0..cars {
                    e.trigger_state.on_cam(
                        [k; 8],
                        Vec3::new(x + 10.0 * f64::from(k + 1), 0.0, 0.0),
                        1.0,
                        0.0,
                        t,
                    );
                }
            })
            .0
        };
        let raised = run(5);
        assert_eq!(raised.len(), 1, "{raised:?}");
        assert_eq!(raised[0].1, DenmCause::TrafficCondition);
        assert_eq!(raised[0].2, 2, "dynamics and environment (Table 8)");
        assert!(run(4).is_empty(), "four slow cars are not five");
    }

    /// RS 2007 sudden speed drop: a minute at 110 km/h, then a 5 m/s² stop behind a car
    /// the own radar sees standing in the lane: dangerousEndOfQueue, quality 2 (driver
    /// reaction and on-board sensors). Without the car ahead the drop alone is not enough.
    #[test]
    fn a_hard_stop_at_the_end_of_a_queue_raises_the_end_of_queue_denm() {
        let run = |sees_queue: bool| {
            let mut e = with(TC);
            let mut profile = steady(30.6, 600);
            profile.extend(brake(30.6, 0.0, 5.0));
            profile.extend(steady(0.0, 50));
            drive(&mut e, &profile, |e, _, x| {
                let objects = if sees_queue {
                    vec![(Vec3::new(x + 30.0, 0.5, 0.0), Vec3::ZERO, 0.0)]
                } else {
                    vec![]
                };
                e.trigger_state.set_perceived(objects);
            })
            .0
        };
        let raised = run(true);
        let drops: Vec<_> = raised
            .iter()
            .filter(|r| r.1 == DenmCause::DangerousEndOfQueue)
            .collect();
        assert_eq!(drops.len(), 1, "{raised:?}");
        assert_eq!(drops[0].2, 2);
        assert!(
            run(false)
                .iter()
                .all(|r| r.1 != DenmCause::DangerousEndOfQueue),
            "a drop with no second condition must not fire"
        );
    }

    /// RS 2002 traction loss: an ABS stop on ice (μ ≈ 0.1) gives a friction estimate under
    /// 0.2; held 5 s it raises the adhesion DENM with quality 7 (condition j), updated at
    /// least 0.1 s apart while it holds. ABS on a wet road (μ ≈ 0.5) and a dry stop raise
    /// nothing.
    #[test]
    fn low_friction_with_wheel_slip_raises_the_traction_loss_denm() {
        let run = |decel: f64, abs: bool| {
            let mut e = with(TL);
            let mut x = 0.0;
            let mut raised = Vec::new();
            let mut updates = 0;
            for k in 0..200u64 {
                let t = k * STEP;
                // Braking for the first 2 s, then rolling on at 5 m/s.
                let braking = k < 20;
                let a = if braking { -decel } else { 0.0 };
                let v = 5.0;
                x += v * 0.1;
                let mut b = bus(t, v, a);
                b.abs_active = braking && abs;
                e.set_vehicle_bus(&b);
                for r in e.due(t, &belief(t, x, v), services(), Some(7)) {
                    match r {
                        EventRequest::Denm(DenmAction::Original(id)) => {
                            raised.push((t, e.denm_causes[&id].0, e.denm_params[&id].quality));
                        }
                        EventRequest::Denm(DenmAction::Update(_)) => updates += 1,
                        _ => {}
                    }
                }
            }
            (raised, updates, e.friction_estimate())
        };
        let (raised, updates, mu) = run(0.96, true);
        assert!(mu.is_some_and(|m| (m - 0.0979).abs() < 1e-3), "{mu:?}");
        assert_eq!(raised.len(), 1, "{raised:?}");
        assert_eq!(raised[0].1, DenmCause::Adhesion);
        assert_eq!(raised[0].2, 7, "condition j");
        // The estimate starts 200 ms into the ABS stop; held 5 s: about 5.2 s.
        let t_s = raised[0].0 as f64 * 1e-9;
        assert!((5.1..=5.4).contains(&t_s), "raised at {t_s} s");
        // Updates every 0.1 s while it holds (the estimate is kept 30 s).
        assert!(updates >= 100, "{updates} updates");
        assert!(run(4.9, true).0.is_empty(), "wet: μ 0.5 is not low");
        assert!(run(0.96, false).0.is_empty(), "no slip, no estimate");
    }

    /// With every trigger off (the default) none of the three scenarios raises anything.
    #[test]
    fn the_triggers_are_off_by_default() {
        let mut e = EventServices::default();
        let mut profile = steady(30.6, 600);
        profile.extend(brake(30.6, 0.0, 5.0));
        profile.extend(steady(0.0, 600));
        let (raised, _) = drive(&mut e, &profile, |e, _, x| {
            e.trigger_state
                .set_perceived(vec![(Vec3::new(x + 30.0, 0.0, 0.0), Vec3::ZERO, 0.0)]);
        });
        assert!(raised.is_empty());
    }
}
