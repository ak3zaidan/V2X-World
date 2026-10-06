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
}

impl EventServices {
    /// Installs the payload the unit's controller feed produced for `ty` (SPaT or MAP).
    /// Any other type is ignored.
    pub fn set_infra_payload(&mut self, ty: MsgType, bytes: Vec<u8>) {
        match ty {
            MsgType::Spat => self.spat = Some(bytes),
            MsgType::Map => self.map = Some(bytes),
            _ => {}
        }
    }

    /// The installed payload for `ty`, when there is one.
    pub fn infra_payload(&self, ty: MsgType) -> Option<&[u8]> {
        match ty {
            MsgType::Spat => self.spat.as_deref(),
            MsgType::Map => self.map.as_deref(),
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

    /// The hazard switch.
    pub fn set_hazard_lights(&mut self, on: bool) {
        self.hazard_lights = on;
    }

    /// The vehicle's own bus reading: its wheel speed and its hazard switch.
    pub fn set_vehicle_bus(&mut self, bus: &crate::vehicle::VehicleBus) {
        self.own_speed = Some(bus.speed_mps);
        self.hazard_lights = bus.hazard_lights;
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
        let repetition = Repetition::new(STATIONARY_REPETITION, STATIONARY_REPETITION_DURATION);
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
