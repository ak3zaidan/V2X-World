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

use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};
use v2xw_metrics::channels::ByteBucket;

use super::Engine;
use crate::ctx::{EngineCtx, RunRecorder};

impl Engine {
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
        let (answer, req) = {
            let p = self.phase2.as_mut()?;
            (p.info_status_answer(node, now), p.crl_request_size())
        };
        let size = answer.unwrap_or(crate::phase2::CRL_NOT_MODIFIED_BYTES);
        let (ul, dl) = {
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
            let a = phase2.as_mut()?.access_mut();
            let ul = a.uu_send(
                &mut ctx,
                node,
                pos,
                v2xw_radio::cellular::Direction::Uplink,
                req,
            );
            let dl = if matches!(ul, crate::backend::AccessOutcome::Arrives(_)) {
                a.uu_send(
                    &mut ctx,
                    node,
                    pos,
                    v2xw_radio::cellular::Direction::Downlink,
                    size,
                )
            } else {
                ul
            };
            (ul, dl)
        };
        let (
            crate::backend::AccessOutcome::Arrives(t1),
            crate::backend::AccessOutcome::Arrives(t2),
        ) = (ul, dl)
        else {
            if changed && let Some(p) = self.phase2.as_mut() {
                p.crl_poll_failed(node);
            }
            return None;
        };
        for (bucket, bytes) in [
            (ByteBucket::CellularUl, u64::from(req)),
            (ByteBucket::CellularDl, u64::from(size)),
        ] {
            self.emit_backend_bytes(recorder, now, bucket, bytes, Some(node));
        }
        if answer.is_none()
            && let Some(p) = self.phase2.as_mut()
        {
            p.note_crl_not_modified();
        }
        if !changed {
            return None;
        }
        let service = self
            .phase2
            .as_ref()
            .map_or(Duration::ZERO, |p| p.params().scms.backend_overhead);
        Some(
            service
                .after(t1)
                .saturating_add(t2.saturating_sub(now))
                .saturating_sub(now),
        )
    }
}
