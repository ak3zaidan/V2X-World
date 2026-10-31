//! WP-35 (A16): a cellular vehicle's CRL poll through the RA's information status.
//!
//! Under `security.scms.crl_poll: info-status` the hourly poll asks first for the RA's
//! `CertificateManagementInformationStatusSpdu` (IEEE 1609.2.1 §6.3.5.14), conditionally,
//! as the US profile has it ([SMP] §4.4: `If-Modified-Since`, via
//! `docs/design/scms-real/03-protocols-and-operations.md` §4.1): HTTP 412 when the status
//! the vehicle last received is still the RA's, the signed status otherwise. Only a
//! vehicle that holds an older list than the RA serves goes on to fetch the composite CRL
//! (6.3.5.8, which the profile prefers over individual CRLs), in a second exchange. The
//! default, `conditional`, is the one-exchange conditional composite poll.
//!
//! The order is the protocol's: the RA answers only a request that reached it (and logs
//! the signed status it sends then), and the vehicle keeps a status, its next
//! `If-Modified-Since`, only once the answer reached it. A vehicle answered 412 that still
//! holds an older list than its kept status names (its last download was lost) fetches the
//! composite again: the status it kept already says what is newer.

use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};
use v2xw_metrics::channels::ByteBucket;

use super::Engine;
use crate::backend::AccessOutcome;
use crate::ctx::{EngineCtx, RunRecorder};

impl Engine {
    /// One leg of the information-status exchange on the cellular link, at `now`.
    fn info_status_leg(
        &mut self,
        node: NodeId,
        pos: Vec3,
        dir: v2xw_radio::cellular::Direction,
        bytes: u32,
    ) -> Option<AccessOutcome> {
        let Engine {
            scheduler,
            rng,
            world,
            snapshot,
            provenance,
            params,
            phase2,
            ..
        } = self;
        let mut null = crate::ctx::NullRecorder::new();
        let mut ctx = EngineCtx::new(
            scheduler, rng, world, snapshot, provenance, params, &mut null,
        );
        Some(
            phase2
                .as_mut()?
                .access_mut()
                .uu_send(&mut ctx, node, pos, dir, bytes),
        )
    }

    /// The first exchange of an information-status poll: the request, and the RA's 412
    /// or its signed status. `Some(delay)` when the vehicle goes on to fetch the composite
    /// CRL (it holds an older list), the time from `now` until the status reached it;
    /// `None` when the poll ends here (current, or the exchange was lost).
    pub(super) fn info_status_exchange(
        &mut self,
        recorder: &mut dyn RunRecorder,
        node: NodeId,
        pos: Vec3,
        now: SimTime,
        changed: bool,
    ) -> Option<u64> {
        let req = {
            let p = self.phase2.as_mut()?;
            p.note_info_status_poll();
            p.crl_request_size()
        };
        let ul = self.info_status_leg(node, pos, v2xw_radio::cellular::Direction::Uplink, req)?;
        let lost = |e: &mut Self| {
            if changed && let Some(p) = e.phase2.as_mut() {
                p.crl_poll_failed(node);
            }
            None
        };
        let AccessOutcome::Arrives(t1) = ul else {
            return lost(self);
        };
        // The RA answers the request it received, after its service time.
        let service = self
            .phase2
            .as_ref()
            .map_or(Duration::ZERO, |p| p.params().scms.backend_overhead);
        let answered = service.after(t1);
        let (answer, seen) = self.phase2.as_mut()?.info_status_answer(node, answered);
        let size = answer.unwrap_or(crate::phase2::CRL_NOT_MODIFIED_BYTES);
        let dl = self.info_status_leg(node, pos, v2xw_radio::cellular::Direction::Downlink, size)?;
        let AccessOutcome::Arrives(t2) = dl else {
            return lost(self);
        };
        // Bytes are counted for a completed exchange, as the composite poll counts them.
        for (bucket, bytes) in [
            (ByteBucket::CellularUl, u64::from(req)),
            (ByteBucket::CellularDl, u64::from(size)),
        ] {
            self.emit_backend_bytes(recorder, now, bucket, bytes, Some(node));
        }
        let p = self.phase2.as_mut()?;
        if answer.is_some() {
            // The vehicle keeps the status it received: its next If-Modified-Since.
            p.info_status_received(node, seen);
        } else {
            p.note_crl_not_modified();
        }
        if !changed {
            return None;
        }
        Some(
            answered
                .saturating_add(t2.saturating_sub(now))
                .saturating_sub(now),
        )
    }
}
