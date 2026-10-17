//! RSU application and OBU identification certificates in the run (WP-B3b): the
//! non-butterfly branch of provisioning, the weekly RSU cycle over its backhaul, and
//! certificate starvation when that backhaul is down (the Wyoming pilot, 01 §6).
//!
//! The PDUs and the ACA's ECQV issuance are [`crate::scms::appcert`]; this module is who
//! sends what, when, over which link, and what each side records.
//!
//! # The cycle (one per end entity)
//!
//! For local period `p` the EE holds at most one certificate whose validity starts at
//! `start(p)` and lasts the kind's validity (RSU: 169 h, so consecutive certificates
//! overlap by the cited hour). It asks for `p + 1` at `start(p + 1) − lead`, where `lead`
//! is at most the RA's `ra-maxGenDelay` of one week ([SMP] §2.1.21 Table 3; "generated
//! at most a week ahead", 01 §4): an RSU therefore needs its backhaul at least once a week.
//! The RA refuses a request whose validity starts more than that ahead, and a second,
//! different request for a period it already answered (1 (+1) concurrent, 03 §7), so an
//! EE cannot stockpile certificates; each refusal is recorded.
//!
//! When the request cannot go out (the backhaul is down) the EE retries after
//! `ra-minWait` (1 h, [SMP] §4.3.1 "retry after *-minWait"); it polls the download every
//! `download-minWait` (5 min, Table 3) until the file is there. A lost request or ack is
//! retried with the *same* SPDU, which the RA recognises by its HashedId8 and answers
//! again; a different request for the same period is refused. When no installed
//! certificate covers the current instant the EE is **starving**: an RSU cannot sign
//! SPaT/MAP validly (00 gap row 24). Starvation is a property of the installed
//! certificates' validity windows, so [`ScmsRun::app_starvation`] computes it exactly.
//!
//! The flow needs the real PDU layer (the PKI); under a hybrid or KEM scheme, which
//! 1609.2.1 does not define, starting it is refused with
//! [`AppEventKind::NoPki`] recorded.

use super::*;

use crate::scms::appcert::{self, AppCertKind, AppCredential, RequestKeys};
use crate::scms::pdu::{Drbg, Pdu, PduError, SpduKind};

/// The cycle's parameters (cited where a source exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppCertParams {
    /// The identification certificate's local period: 30 days (assumption: CAMP says
    /// "1 month"). The RSU certificate's period is the run's i-period (1 week).
    pub id_period: Duration,
    /// `ra-maxGenDelay`: how far ahead of its validity start a certificate may be
    /// generated (1 week, [SMP] Table 3).
    pub max_gen_ahead: Duration,
    /// How long before the next period starts the EE asks for it; `None` = as early as
    /// the RA allows (`max_gen_ahead`), the most robust firmware.
    pub request_lead: Option<Duration>,
    /// `ra-minWait`: the wait before a failed request is tried again (1 h, Table 3).
    pub retry_wait: Duration,
    /// `download-minWait`: the wait between download polls (5 min, Table 3).
    pub download_poll: Duration,
    /// The RA's `nextDlTime` after an ack: the ACA's turnaround it promises (the run's
    /// `first_batch_delay`, 60 s, unless set).
    pub download_delay: Duration,
    /// RSU backhaul one-way latency (assumption: 20 ms fibre/DSL to the provider).
    pub backhaul_latency: Duration,
    /// RSU backhaul bandwidth (assumption: 10 Mbit/s).
    pub backhaul_bandwidth_bps: u64,
}

impl Default for AppCertParams {
    fn default() -> AppCertParams {
        AppCertParams {
            id_period: Duration::from_secs(30 * 86_400),
            max_gen_ahead: Duration::from_secs(7 * 86_400),
            request_lead: None,
            retry_wait: Duration::from_secs(3_600),
            download_poll: Duration::from_secs(300),
            download_delay: Duration::from_secs(60),
            backhaul_latency: Duration::from_millis(20),
            backhaul_bandwidth_bps: 10_000_000,
        }
    }
}

/// The messages of the flow, carried as [`ScmsMsg::AppCert`].
#[derive(Debug, Clone)]
pub enum AppMsg {
    /// EE timer: time to ask for period `period` (or to try again).
    Cycle {
        /// The EE.
        device: NodeId,
        /// The period.
        period: u32,
        /// The timer generation (only the newest acts).
        seq: u32,
    },
    /// EE → RA: `EeRaCertRequestSpdu`.
    Request {
        /// The EE.
        device: NodeId,
        /// The period.
        period: u32,
        /// The SPDU.
        pdu: Pdu,
    },
    /// RA → EE: `RaEeCertAckSpdu`.
    Ack {
        /// The EE.
        device: NodeId,
        /// The period.
        period: u32,
        /// The SPDU.
        pdu: Pdu,
    },
    /// RA → EE: the RA will not issue (blocklisted, enrolment expired, too early, a
    /// second request for one period, or the request did not verify).
    Refused {
        /// The EE.
        device: NodeId,
        /// The period.
        period: u32,
        /// HashedId8 of the refused request: the EE acts only on a refusal of its own.
        hash8: [u8; 8],
        /// Why.
        reason: &'static str,
    },
    /// RA → ACA: `RaAcaCertRequestSpdu`.
    AcaRequest {
        /// The SPDU.
        pdu: Pdu,
    },
    /// ACA → RA: `AcaRaCertResponseSpdu`.
    AcaResponse {
        /// The SPDU.
        pdu: Pdu,
    },
    /// EE timer: poll the download.
    Poll {
        /// The EE.
        device: NodeId,
        /// The period.
        period: u32,
        /// The timer generation.
        seq: u32,
    },
    /// EE → RA: `EeRaDownloadRequestSpdu`.
    DownloadRequest {
        /// The EE.
        device: NodeId,
        /// The period.
        period: u32,
        /// The SPDU.
        pdu: Pdu,
    },
    /// RA → EE: the stored `AcaEeCertResponsePrivateSpdu`, or `None` (not ready yet).
    Download {
        /// The EE.
        device: NodeId,
        /// The period.
        period: u32,
        /// The file.
        pdu: Option<Pdu>,
    },
}

/// What happened to an EE, in order: the record a study reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEvent {
    /// When.
    pub at: SimTime,
    /// For which period.
    pub period: u32,
    /// What.
    pub kind: AppEventKind,
}

/// An [`AppEvent`]'s kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEventKind {
    /// The request went out.
    Requested,
    /// The request or poll could not go out: the backhaul was down.
    BackhaulDown,
    /// A message for the EE arrived while its backhaul was down and was lost.
    LostInbound,
    /// The RA refused.
    Refused(&'static str),
    /// The EE refused what it received (ack or certificate response).
    Rejected(PduError),
    /// A certificate was installed (its HashedId10).
    Installed([u8; 10]),
    /// Not enrolled yet; tried again later.
    NotEnrolled,
    /// No PDU layer: the flow cannot run.
    NoPki,
}

/// An installed certificate with its validity in simulated time.
#[derive(Debug, Clone)]
pub struct InstalledApp {
    /// The credential.
    pub cred: AppCredential,
    /// Validity start.
    pub from: SimTime,
    /// Validity end (exclusive).
    pub until: SimTime,
    /// When it was installed.
    pub at: SimTime,
    /// The request's secrets (kept for [`AppCertState::debug_secrets`]).
    keys: RequestKeys,
}

#[derive(Debug, Clone)]
struct PendingReq {
    period: u32,
    keys: RequestKeys,
    pdu: Pdu,
    hash8: [u8; 8],
    valid_from: u32,
    acked: bool,
    polls: u32,
}

/// One EE in the cycle.
#[derive(Debug, Clone)]
pub struct AppDevice {
    /// Its certificate kind.
    pub kind: AppCertKind,
    /// Installed certificates, oldest first.
    pub installed: Vec<InstalledApp>,
    /// Its record.
    pub events: Vec<AppEvent>,
    /// Backhaul outages `[from, until)`.
    pub outages: Vec<(SimTime, SimTime)>,
    /// Whether the cycle has stopped for good (refused for a reason renewal cannot fix).
    pub stopped: bool,
    pending: Option<PendingReq>,
    cycle_seq: u32,
    poll_seq: u32,
}

impl AppDevice {
    fn down(&self, at: SimTime) -> bool {
        self.outages.iter().any(|&(a, b)| at >= a && at < b)
    }

    /// The certificate it holds and can sign with at `t`, if any (the newest): installed
    /// by `t` and inside its validity.
    pub fn valid_at(&self, t: SimTime) -> Option<&InstalledApp> {
        self.installed
            .iter()
            .rev()
            .find(|c| t >= c.at && t >= c.from && t < c.until)
    }

    fn has(&self, period: u32) -> bool {
        self.installed.iter().any(|c| c.cred.period == period)
    }
}

/// What the RA keeps per request it forwarded.
#[derive(Debug, Clone)]
struct RaJob {
    device: NodeId,
    period: u32,
    filename: String,
    flow: FlowId,
    run: FlowRun,
}

/// What the RA keeps per request it answered.
#[derive(Debug, Clone)]
struct Answered {
    /// HashedId8 of the request SPDU.
    hash8: [u8; 8],
    /// The download file it names.
    filename: String,
    /// The ack, sent again if the same request comes again.
    ack: Pdu,
    /// HashedId8 of the enrolment certificate that signed it: only it may download.
    enrolment_id8: [u8; 8],
}

/// The ACA's record of an issued certificate: what a series-3 hash CRL revokes it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedApp {
    /// The holder (the RA's knowledge, not the ACA's).
    pub device: NodeId,
    /// Its kind.
    pub kind: AppCertKind,
    /// Its period.
    pub period: u32,
    /// HashedId10.
    pub id10: [u8; 10],
    /// When the ACA issued it.
    pub at: SimTime,
}

/// The flow's state.
#[derive(Debug, Clone)]
pub struct AppCertState {
    /// Parameters.
    pub params: AppCertParams,
    /// The EEs in the cycle.
    pub devices: BTreeMap<NodeId, AppDevice>,
    /// Every certificate the ACA issued.
    pub issued: Vec<IssuedApp>,
    /// The RA's download store: filename → `AcaEeCertResponsePrivateSpdu`.
    pub ra_store: BTreeMap<String, Pdu>,
    /// Every refusal the RA answered: `(when, device, period, why)`.
    pub ra_refusals: Vec<(SimTime, NodeId, u32, &'static str)>,
    /// The RA's answered requests: `(device, period)` → the request's HashedId8.
    ra_answered: BTreeMap<(NodeId, u32), Answered>,
    /// RA → ACA requests in flight, by the HashedId8 of the `RaAcaCertRequestSpdu`.
    ra_jobs: BTreeMap<[u8; 8], RaJob>,
    rng: Drbg,
}

impl AppCertState {
    /// Empty, with the cited defaults; randomness from its own stream so it shifts no
    /// draw of any other flow.
    pub fn new(seed: u64) -> AppCertState {
        AppCertState {
            params: AppCertParams::default(),
            devices: BTreeMap::new(),
            issued: Vec::new(),
            ra_store: BTreeMap::new(),
            ra_refusals: Vec::new(),
            ra_answered: BTreeMap::new(),
            ra_jobs: BTreeMap::new(),
            rng: Drbg::new(seed, b"appcert"),
        }
    }
}

/// One installed certificate's request secrets, for an independent verifier.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppSecrets {
    /// The holder.
    pub device: u32,
    /// The period.
    pub period: u32,
    /// The kind.
    pub kind: &'static str,
    /// ECQV `k_U` (`b_i0` for butterfly), 32 bytes hex.
    pub k_u: String,
    /// The response decryption key (`u`, or `h_i0`), hex.
    pub response: String,
    /// Butterfly only: the caterpillar `(a, ck, h, ek)`, hex, for re-expansion.
    pub caterpillar: Option<[String; 4]>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl AppCertState {
    /// Every installed certificate's request secrets. **Test and oracle use only**: an
    /// EE's private keys, exported so `tools/pdu_check/check_appcerts.py` can re-derive
    /// what the EE derived.
    pub fn debug_secrets(&self) -> Vec<AppSecrets> {
        let s = |x: &p256::Scalar| hex(&v2xw_sec::ec::scalar_to_be32(x));
        self.devices
            .iter()
            .flat_map(|(n, d)| {
                d.installed.iter().map(move |c| AppSecrets {
                    device: n.index(),
                    period: c.cred.period,
                    kind: c.cred.kind.name(),
                    k_u: s(&c.keys.ecqv_private()),
                    response: s(&c.keys.response_private()),
                    caterpillar: match &c.keys {
                        RequestKeys::Butterfly { cat, .. } => {
                            Some([s(cat.a()), hex(cat.ck()), s(cat.p()), hex(cat.ek())])
                        }
                        RequestKeys::Plain { .. } => None,
                    },
                })
            })
            .collect()
    }
}

const fn flow_of(kind: AppCertKind) -> FlowId {
    match kind {
        AppCertKind::RsuApplication => FlowId::RsuApplicationCert,
        AppCertKind::ObuIdentification => FlowId::IdentificationCert,
    }
}

const fn transport_of(kind: AppCertKind) -> Transport {
    match kind {
        AppCertKind::RsuApplication => Transport::RsuBackhaul,
        AppCertKind::ObuIdentification => Transport::CellularUu,
    }
}

fn wrap(m: AppMsg) -> ScmsMsg {
    ScmsMsg::AppCert(Box::new(m))
}

impl ScmsRun {
    /// Adds a roadside unit: a device whose link to the backend is its backhaul
    /// ([`Transport::RsuBackhaul`], the [`AppCertParams`] latency and bandwidth).
    pub fn add_rsu(&mut self, node: NodeId) {
        self.add_device(node);
        let a = self.state.apps.params;
        self.set_access(
            node,
            Link {
                latency: a.backhaul_latency,
                bandwidth_bps: a.backhaul_bandwidth_bps,
                transport: Transport::RsuBackhaul,
            },
        );
    }

    /// The backhaul of `device` is down over `[from, until)`: nothing it sends gets out and
    /// nothing addressed to it arrives.
    pub fn set_backhaul_outage(&mut self, device: NodeId, from: SimTime, until: SimTime) {
        if let Some(d) = self.state.apps.devices.get_mut(&device) {
            d.outages.push((from, until));
        }
    }

    /// Starts the certificate cycle of `device` (an RSU's application certificate, or an
    /// emergency OBU's identification certificate) at `at`: the first request is for the
    /// period containing `at`. The device must be enrolled ([`ScmsRun::enrol`]) or
    /// enrolling; outages set before or after this call both apply.
    pub fn start_app_certs(&mut self, device: NodeId, kind: AppCertKind, at: SimTime) {
        if !self.state.devices.contains_key(&device) {
            self.add_device(device);
        }
        let run = self.new_run();
        let period = self.state.app_period_at(kind, at);
        let st = &mut self.state.apps;
        let outages = st
            .devices
            .get(&device)
            .map(|d| d.outages.clone())
            .unwrap_or_default();
        st.devices.insert(
            device,
            AppDevice {
                kind,
                installed: Vec::new(),
                events: Vec::new(),
                outages,
                stopped: false,
                pending: None,
                cycle_seq: 0,
                poll_seq: 0,
            },
        );
        if self.state.pki.is_none() {
            if let Some(d) = self.state.apps.devices.get_mut(&device) {
                d.events.push(AppEvent {
                    at,
                    period,
                    kind: AppEventKind::NoPki,
                });
                d.stopped = true;
            }
            return;
        }
        let at = at.max(self.kernel.now());
        self.inject_at(
            at,
            device,
            wrap(AppMsg::Cycle {
                device,
                period,
                seq: 0,
            }),
            flow_of(kind),
            run,
        );
    }

    /// Registers `device` for the cycle without starting it, so outages can be declared
    /// first. [`ScmsRun::start_app_certs`] keeps them.
    pub fn declare_app_device(&mut self, device: NodeId, kind: AppCertKind) {
        self.state
            .apps
            .devices
            .entry(device)
            .or_insert_with(|| AppDevice {
                kind,
                installed: Vec::new(),
                events: Vec::new(),
                outages: Vec::new(),
                stopped: false,
                pending: None,
                cycle_seq: 0,
                poll_seq: 0,
            });
    }

    /// The certificate `device` can sign with at `t`, if any.
    pub fn app_credential_at(&self, device: NodeId, t: SimTime) -> Option<&InstalledApp> {
        self.state.apps.devices.get(&device)?.valid_at(t)
    }

    /// The intervals in `[from, to)` during which `device` held no valid certificate:
    /// starvation (an RSU signs no valid SPaT/MAP).
    pub fn app_starvation(
        &self,
        device: NodeId,
        from: SimTime,
        to: SimTime,
    ) -> Vec<(SimTime, SimTime)> {
        let Some(d) = self.state.apps.devices.get(&device) else {
            return vec![(from, to)];
        };
        let mut w: Vec<(SimTime, SimTime)> = d
            .installed
            .iter()
            // A certificate covers only from when it was installed.
            .map(|c| (c.from.max(c.at), c.until))
            .filter(|(a, b)| a < b)
            .collect();
        w.sort_unstable();
        let mut gaps = Vec::new();
        let mut t = from;
        for (a, b) in w {
            if b <= t {
                continue;
            }
            if a > t {
                gaps.push((t, a.min(to)));
            }
            t = t.max(b);
            if t >= to {
                break;
            }
        }
        if t < to {
            gaps.push((t, to));
        }
        gaps.retain(|(a, b)| a < b);
        gaps
    }
}

impl ScmsState {
    /// The kind's period length.
    fn app_period_len(&self, kind: AppCertKind) -> Duration {
        match kind {
            AppCertKind::RsuApplication => self.params.i_period,
            AppCertKind::ObuIdentification => self.apps.params.id_period,
        }
    }

    /// When local period `p` starts.
    fn app_period_start(&self, kind: AppCertKind, p: u32) -> SimTime {
        self.params.period_start_of(self.app_period_len(kind), p)
    }

    /// The period containing `at`.
    fn app_period_at(&self, kind: AppCertKind, at: SimTime) -> u32 {
        self.params.period_at_of(self.app_period_len(kind), at)
    }

    /// When the EE asks for `p`: `start(p) − lead`, never earlier than the RA allows.
    fn app_request_time(&self, kind: AppCertKind, p: u32) -> SimTime {
        let a = self.apps.params;
        let lead = a
            .request_lead
            .unwrap_or(a.max_gen_ahead)
            .min(a.max_gen_ahead);
        self.app_period_start(kind, p)
            .saturating_sub(lead.as_nanos())
    }

    fn app_event(&mut self, device: NodeId, at: SimTime, period: u32, kind: AppEventKind) {
        if let Some(d) = self.apps.devices.get_mut(&device) {
            d.events.push(AppEvent { at, period, kind });
        }
    }

    fn app_cycle_timer(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        device: NodeId,
        period: u32,
        delay: Duration,
        flow: FlowId,
        run: FlowRun,
    ) {
        let Some(d) = self.apps.devices.get_mut(&device) else {
            return;
        };
        d.cycle_seq += 1;
        let seq = d.cycle_seq;
        out.start_timer(
            delay,
            wrap(AppMsg::Cycle {
                device,
                period,
                seq,
            }),
            flow,
            run,
        );
    }

    fn app_poll_timer(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        device: NodeId,
        period: u32,
        delay: Duration,
        flow: FlowId,
        run: FlowRun,
    ) {
        let Some(d) = self.apps.devices.get_mut(&device) else {
            return;
        };
        d.poll_seq += 1;
        let seq = d.poll_seq;
        out.start_timer(
            delay,
            wrap(AppMsg::Poll {
                device,
                period,
                seq,
            }),
            flow,
            run,
        );
    }

    /// Handles every message of this flow.
    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_appcert(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at) = (d.flow, d.run, d.to, d.at);
        let ScmsMsg::AppCert(m) = d.msg else {
            return Ok(());
        };
        let Some(pki) = self.pki.clone() else {
            return Ok(());
        };
        let wait = self.apps.params.retry_wait;
        match *m {
            AppMsg::Cycle {
                device,
                period,
                seq,
            } => {
                let Some(dev) = self.apps.devices.get(&device) else {
                    return Ok(());
                };
                if seq != dev.cycle_seq || dev.stopped || dev.has(period) {
                    return Ok(());
                }
                let kind = dev.kind;
                if dev.down(at) {
                    self.app_event(device, at, period, AppEventKind::BackhaulDown);
                    self.app_cycle_timer(out, device, period, wait, flow, run);
                    return Ok(());
                }
                let Some(enrol) = self
                    .devices
                    .get(&device)
                    .and_then(|x| x.enrolment_credential.clone())
                else {
                    self.app_event(device, at, period, AppEventKind::NotEnrolled);
                    self.app_cycle_timer(out, device, period, wait, flow, run);
                    return Ok(());
                };
                // Re-send the same SPDU if this period's request is outstanding (a lost
                // request or ack), so the RA can recognise it; otherwise a new one.
                let pending = self.apps.devices[&device]
                    .pending
                    .clone()
                    .filter(|p| p.period == period);
                let pdu = match pending {
                    Some(p) => p.pdu,
                    None => {
                        let valid_from = self
                            .params
                            .period_start_time32_of(self.app_period_len(kind), period);
                        let t = self.time32(at);
                        let built =
                            RequestKeys::fresh(kind, period, &mut self.apps.rng).and_then(|keys| {
                                appcert::ee_request_spdu(
                                    &pki,
                                    &enrol,
                                    kind,
                                    t,
                                    &keys,
                                    valid_from,
                                    &mut self.apps.rng,
                                )
                                .map(|b| (keys, b))
                            });
                        let (keys, bytes) = match built {
                            Ok(x) => x,
                            Err(e) => {
                                self.app_event(device, at, period, AppEventKind::Rejected(e));
                                return Ok(());
                            }
                        };
                        // A fresh key pair (and, for identification, a caterpillar)
                        // per certificate: one key generation, and the butterfly
                        // request's second.
                        Self::scalar_mults(out, 2);
                        let pdu = Pdu::new(bytes);
                        let hash8 = pdu.hashed_id8();
                        if let Some(dv) = self.apps.devices.get_mut(&device) {
                            dv.pending = Some(PendingReq {
                                period,
                                keys,
                                pdu: pdu.clone(),
                                hash8,
                                valid_from,
                                acked: false,
                                polls: 0,
                            });
                        }
                        pdu
                    }
                };
                Self::sign(out, 1);
                Self::encrypt(out, 1);
                out.stage_at(StageId::Requested, device, None, flow, run);
                self.app_event(device, at, period, AppEventKind::Requested);
                let size = self.emit_pdu(SpduKind::EeRaCertRequest, device, n.ra, at, &pdu);
                out.send(
                    n.ra,
                    wrap(AppMsg::Request {
                        device,
                        period,
                        pdu,
                    }),
                    "app-cert-request",
                    size,
                    transport_of(kind),
                    flow,
                    run,
                );
                // The guard: no ack by then and the same request goes again.
                self.app_cycle_timer(out, device, period, wait, flow, run);
            }
            AppMsg::Request {
                device,
                period,
                pdu,
            } => {
                Self::decrypt(out, 1);
                Self::verify(out, 1);
                let Some(kind) = self.apps.devices.get(&device).map(|x| x.kind) else {
                    return Ok(());
                };
                let hash8 = pdu.hashed_id8();
                let transport = transport_of(kind);
                let refuse =
                    |st: &mut ScmsState, out: &mut Outbox<ScmsMsg>, reason: &'static str| {
                        st.apps.ra_refusals.push((at, device, period, reason));
                        Self::sign(out, 1);
                        out.send(
                            device,
                            wrap(AppMsg::Refused {
                                device,
                                period,
                                hash8,
                                reason,
                            }),
                            "app-cert-refused",
                            st.sizes.refusal(),
                            transport,
                            flow,
                            run,
                        );
                    };
                // The same request again (its ack was lost): answer it again.
                if let Some(Answered { hash8: h, ack, .. }) = self.ra_answered_for(device, period) {
                    if h == hash8 {
                        let size = self.emit_pdu(SpduKind::RaEeCertAck, to, device, at, &ack);
                        out.send(
                            device,
                            wrap(AppMsg::Ack {
                                device,
                                period,
                                pdu: ack,
                            }),
                            "app-cert-ack",
                            size,
                            transport,
                            flow,
                            run,
                        );
                    } else {
                        self.ra.refused += 1;
                        refuse(self, out, "a second request for a period already answered");
                    }
                    return Ok(());
                }
                let opened = match appcert::ra_open_request(&pki, pdu.bytes()) {
                    Ok(o) => o,
                    Err(e) => {
                        self.reject_pdu(SpduKind::EeRaCertRequest, to, at, e);
                        refuse(self, out, "the request does not verify");
                        return Ok(());
                    }
                };
                let enrolment_ok = self
                    .devices
                    .get(&device)
                    .and_then(|x| x.enrolment)
                    .is_some_and(|ec| ec.valid_at(at));
                let start = opened.tbs.validity_period.start.0.0;
                let horizon = self.time32(self.apps.params.max_gen_ahead.after(at));
                let reason = if self.ra.blocklist.contains(&device) {
                    Some("blocklisted")
                } else if !enrolment_ok {
                    Some("enrolment certificate expired or unknown")
                } else if opened.kind() != kind {
                    Some("request shape is not the device's certificate kind")
                } else if start > horizon {
                    Some("validity starts beyond ra-maxGenDelay")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    self.ra.refused += 1;
                    refuse(self, out, reason);
                    return Ok(());
                }
                out.stage_at(StageId::Acknowledged, to, None, flow, run);
                let (r_pub, enc) = match opened.aca_keys(period) {
                    Ok(x) => x,
                    Err(e) => {
                        self.reject_pdu(SpduKind::EeRaCertRequest, to, at, e);
                        return Ok(());
                    }
                };
                if kind.butterfly() {
                    // Cocoon expansion of (i, 0): two scalar multiplications.
                    Self::scalar_mults(out, 2);
                    out.stage_at(StageId::Expanded, to, None, flow, run);
                }
                let t = self.time32(at);
                let aca_req =
                    match appcert::ra_aca_cert_request_spdu(&pki, t, &opened.tbs, &r_pub, &enc) {
                        Ok(b) => Pdu::new(b),
                        Err(e) => {
                            self.reject_pdu(SpduKind::RaAcaCertRequest, to, at, e);
                            return Ok(());
                        }
                    };
                Self::sign(out, 1);
                let filename = crate::scms::pdu::batch_filename(hash8, period);
                self.apps.ra_jobs.insert(
                    aca_req.hashed_id8(),
                    RaJob {
                        device,
                        period,
                        filename: filename.clone(),
                        flow,
                        run,
                    },
                );
                let first_i = kind
                    .butterfly()
                    .then(|| u16::try_from(period).unwrap_or(u16::MAX));
                let next = self.time32(self.apps.params.download_delay.after(at));
                let ack = match crate::scms::pdu::signed(
                    &appcert::ra_ee_cert_ack(t, hash8, first_i, next),
                    &pki.ra.cert,
                    &pki.ra.key,
                ) {
                    Ok(b) => Pdu::new(b),
                    Err(e) => {
                        self.reject_pdu(SpduKind::RaEeCertAck, to, at, e);
                        return Ok(());
                    }
                };
                Self::sign(out, 1);
                self.apps.ra_answered.insert(
                    (device, period),
                    Answered {
                        hash8,
                        filename,
                        ack: ack.clone(),
                        enrolment_id8: crate::scms::pdu::cert_id8(&opened.enrolment),
                    },
                );
                let size = self.emit_pdu(SpduKind::RaAcaCertRequest, to, n.pca, at, &aca_req);
                out.send(
                    n.pca,
                    wrap(AppMsg::AcaRequest { pdu: aca_req }),
                    "app-cert-aca-request",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
                let size = self.emit_pdu(SpduKind::RaEeCertAck, to, device, at, &ack);
                out.send(
                    device,
                    wrap(AppMsg::Ack {
                        device,
                        period,
                        pdu: ack,
                    }),
                    "app-cert-ack",
                    size,
                    transport,
                    flow,
                    run,
                );
            }
            AppMsg::AcaRequest { pdu } => {
                // Verify the RA, ECQV issuance, encrypt to the response key, sign twice.
                Self::verify(out, 1);
                out.charge(ECQV, PrimitiveOpKind::Sign, 1);
                Self::encrypt(out, 1);
                Self::sign(out, 2);
                let t = self.time32(at);
                let issued = match appcert::aca_issue(&pki, pdu.bytes(), t, &mut self.apps.rng) {
                    Ok(x) => x,
                    Err(e) => {
                        self.reject_pdu(SpduKind::RaAcaCertRequest, to, at, e);
                        return Ok(());
                    }
                };
                out.stage_at(StageId::Certified, to, None, flow, run);
                if let Some(job) = self.apps.ra_jobs.get(&pdu.hashed_id8()) {
                    let kind = self
                        .apps
                        .devices
                        .get(&job.device)
                        .map_or(AppCertKind::RsuApplication, |x| x.kind);
                    let id10 = v2xw_sec::cert::cert_id10(&issued.cert).unwrap_or([0; 10]);
                    self.apps.issued.push(IssuedApp {
                        device: job.device,
                        kind,
                        period: job.period,
                        id10,
                        at,
                    });
                }
                self.pca.issued_count += 1;
                let resp = Pdu::new(issued.aca_ra_spdu);
                let size = self.emit_pdu(SpduKind::AcaRaCertResponse, to, n.ra, at, &resp);
                out.send(
                    n.ra,
                    wrap(AppMsg::AcaResponse { pdu: resp }),
                    "app-cert-aca-response",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            AppMsg::AcaResponse { pdu } => {
                Self::verify(out, 1);
                let opened = crate::scms::pdu::open_signed_pdu(pdu.bytes(), &pki)
                    .ok()
                    .and_then(|(p, _)| match p.content {
                        v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPduContent::aca_ra(
                            v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ra_interface::AcaRaInterfacePdu::acaRaCertResponse(r),
                        ) => Some(v2xw_sec::hashedid::id8_bytes(&r.request_hash)),
                        _ => None,
                    });
                let Some(job) = opened.and_then(|h| self.apps.ra_jobs.remove(&h).map(|j| (h, j)))
                else {
                    self.reject_pdu(
                        SpduKind::AcaRaCertResponse,
                        to,
                        at,
                        PduError::Shape("an ACA response to no request this RA sent"),
                    );
                    return Ok(());
                };
                let (h, job) = job;
                match appcert::ra_open_aca_response(&pki, pdu.bytes(), h) {
                    Ok(private) => {
                        out.stage_at(StageId::BatchReady, to, None, job.flow, job.run);
                        self.apps.ra_store.insert(job.filename, Pdu::new(private));
                    }
                    Err(e) => self.reject_pdu(SpduKind::AcaRaCertResponse, to, at, e),
                }
            }
            AppMsg::Ack {
                device,
                period,
                pdu,
            } => {
                if self.app_inbound_lost(device, at, period) {
                    return Ok(());
                }
                Self::verify(out, 1);
                let Some(dev) = self.apps.devices.get(&device) else {
                    return Ok(());
                };
                let kind = dev.kind;
                let Some(p) = dev.pending.clone().filter(|p| p.period == period) else {
                    return Ok(());
                };
                match appcert::ee_check_ack(&pki, pdu.bytes(), p.hash8, kind.butterfly()) {
                    Ok(next) => {
                        if p.acked {
                            return Ok(());
                        }
                        if let Some(dv) = self.apps.devices.get_mut(&device) {
                            if let Some(pp) = dv.pending.as_mut() {
                                pp.acked = true;
                            }
                            // The request guard has done its job.
                            dv.cycle_seq += 1;
                        }
                        let now = self.time32(at);
                        let delay = Duration::from_secs(u64::from(next.saturating_sub(now)));
                        self.app_poll_timer(out, device, period, delay, flow, run);
                    }
                    Err(e) => {
                        self.reject_pdu(SpduKind::RaEeCertAck, device, at, e.clone());
                        self.app_event(device, at, period, AppEventKind::Rejected(e));
                    }
                }
            }
            AppMsg::Refused {
                device,
                period,
                hash8,
                reason,
            } => {
                if self.app_inbound_lost(device, at, period) {
                    return Ok(());
                }
                // A refusal of a request this EE did not send (another party's, on its
                // own connection) is not about this EE.
                let mine = self
                    .apps
                    .devices
                    .get(&device)
                    .is_some_and(|d| d.pending.as_ref().is_some_and(|p| p.hash8 == hash8));
                if !mine {
                    return Ok(());
                }
                self.app_event(device, at, period, AppEventKind::Refused(reason));
                if let Some(dv) = self.apps.devices.get_mut(&device) {
                    dv.cycle_seq += 1;
                    dv.pending = None;
                    // Blocklisting and an expired enrolment are final until re-enrolment;
                    // a request that came too early is tried again later.
                    if reason != "validity starts beyond ra-maxGenDelay" {
                        dv.stopped = true;
                    }
                }
                if reason == "validity starts beyond ra-maxGenDelay" {
                    self.app_cycle_timer(out, device, period, wait, flow, run);
                }
            }
            AppMsg::Poll {
                device,
                period,
                seq,
            } => {
                let Some(dev) = self.apps.devices.get(&device) else {
                    return Ok(());
                };
                if seq != dev.poll_seq || dev.has(period) {
                    return Ok(());
                }
                let kind = dev.kind;
                let poll = self.apps.params.download_poll;
                if dev.down(at) {
                    self.app_event(device, at, period, AppEventKind::BackhaulDown);
                    self.app_poll_timer(out, device, period, wait, flow, run);
                    return Ok(());
                }
                let Some(p) = dev.pending.clone().filter(|p| p.period == period) else {
                    return Ok(());
                };
                // `download-maxReqs` (100, [SMP] Table 3): then the request goes again.
                if p.polls >= 100 {
                    if let Some(pp) = self
                        .apps
                        .devices
                        .get_mut(&device)
                        .and_then(|d| d.pending.as_mut())
                    {
                        pp.acked = false;
                        pp.polls = 0;
                    }
                    self.app_cycle_timer(out, device, period, wait, flow, run);
                    return Ok(());
                }
                if let Some(pp) = self
                    .apps
                    .devices
                    .get_mut(&device)
                    .and_then(|d| d.pending.as_mut())
                {
                    pp.polls += 1;
                }
                let Some(enrol) = self
                    .devices
                    .get(&device)
                    .and_then(|x| x.enrolment_credential.clone())
                else {
                    return Ok(());
                };
                let filename = crate::scms::pdu::batch_filename(p.hash8, period);
                let t = self.time32(at);
                let bytes = match appcert::ee_download_request_spdu(
                    &pki,
                    &enrol,
                    t,
                    &filename,
                    &mut self.apps.rng,
                ) {
                    Ok(b) => Pdu::new(b),
                    Err(e) => {
                        self.app_event(device, at, period, AppEventKind::Rejected(e));
                        return Ok(());
                    }
                };
                Self::sign(out, 1);
                Self::encrypt(out, 1);
                let size = self.emit_pdu(SpduKind::EeRaDownloadRequest, device, n.ra, at, &bytes);
                out.send(
                    n.ra,
                    wrap(AppMsg::DownloadRequest {
                        device,
                        period,
                        pdu: bytes,
                    }),
                    "app-cert-download-request",
                    size,
                    transport_of(kind),
                    flow,
                    run,
                );
                // The next poll, unless this one installs the certificate.
                self.app_poll_timer(out, device, period, poll, flow, run);
            }
            AppMsg::DownloadRequest {
                device,
                period,
                pdu,
            } => {
                Self::decrypt(out, 1);
                Self::verify(out, 1);
                let Some(kind) = self.apps.devices.get(&device).map(|x| x.kind) else {
                    return Ok(());
                };
                let file = match appcert::ra_open_download_request(&pki, pdu.bytes()) {
                    Ok((name, signer)) => {
                        // Only the EE that asked may fetch the file: the download request
                        // is signed by the enrolment certificate that signed the request.
                        let owner = self.ra_answered_for(device, period).is_some_and(|a| {
                            a.filename == name
                                && a.enrolment_id8 == crate::scms::pdu::cert_id8(&signer)
                        });
                        if owner {
                            self.apps.ra_store.get(&name).cloned()
                        } else {
                            self.reject_pdu(
                                SpduKind::EeRaDownloadRequest,
                                to,
                                at,
                                PduError::Signer("download of another device's file"),
                            );
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        self.reject_pdu(SpduKind::EeRaDownloadRequest, to, at, e);
                        return Ok(());
                    }
                };
                let size = match &file {
                    Some(f) => self.emit_pdu(SpduKind::AcaEeCertResponsePrivate, to, device, at, f),
                    None => self.sizes.refusal(),
                };
                out.send(
                    device,
                    wrap(AppMsg::Download {
                        device,
                        period,
                        pdu: file,
                    }),
                    "app-cert-download",
                    size,
                    transport_of(kind),
                    flow,
                    run,
                );
            }
            AppMsg::Download {
                device,
                period,
                pdu,
            } => {
                if self.app_inbound_lost(device, at, period) {
                    return Ok(());
                }
                let Some(file) = pdu else {
                    return Ok(());
                };
                let Some(dev) = self.apps.devices.get(&device) else {
                    return Ok(());
                };
                let kind = dev.kind;
                let Some(p) = dev.pending.clone().filter(|p| p.period == period) else {
                    return Ok(());
                };
                if dev.has(period) {
                    return Ok(());
                }
                // Verify the ACA, decrypt, ECQV reconstruction and its check.
                Self::verify(out, 1);
                Self::decrypt(out, 1);
                out.charge(ECQV, PrimitiveOpKind::Verify, 1);
                out.stage_at(StageId::Downloaded, device, None, flow, run);
                match appcert::ee_open_response(
                    &pki,
                    file.bytes(),
                    &p.keys,
                    kind,
                    period,
                    p.valid_from,
                ) {
                    Ok(cred) => {
                        let from = self.app_period_start(kind, period);
                        // The end the certificate itself states (exact on the real
                        // calendar even when its period began before the run).
                        let until = Duration::from_secs(u64::from(
                            cred.valid_until.saturating_sub(self.time32(0)),
                        ))
                        .after(0);
                        out.stage_at(StageId::Installed, device, None, flow, run);
                        self.app_event(device, at, period, AppEventKind::Installed(cred.id10));
                        let next_at = self.app_request_time(kind, period + 1).max(at);
                        if let Some(dv) = self.apps.devices.get_mut(&device) {
                            dv.installed.push(InstalledApp {
                                cred,
                                from,
                                until,
                                at,
                                keys: p.keys.clone(),
                            });
                            dv.pending = None;
                            dv.poll_seq += 1;
                        }
                        let delay = Duration::from_nanos(next_at - at);
                        self.app_cycle_timer(out, device, period + 1, delay, flow, run);
                    }
                    Err(e) => {
                        self.reject_pdu(SpduKind::AcaEeCertResponsePrivate, device, at, e.clone());
                        self.app_event(device, at, period, AppEventKind::Rejected(e));
                    }
                }
            }
        }
        Ok(())
    }

    fn ra_answered_for(&self, device: NodeId, period: u32) -> Option<Answered> {
        self.apps.ra_answered.get(&(device, period)).cloned()
    }

    /// A message for `device` while its backhaul is down is lost.
    fn app_inbound_lost(&mut self, device: NodeId, at: SimTime, period: u32) -> bool {
        let down = self.apps.devices.get(&device).is_some_and(|d| d.down(at));
        if down {
            self.app_event(device, at, period, AppEventKind::LostInbound);
        }
        down
    }
}
