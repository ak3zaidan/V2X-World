//! Local policy and chain files (LPF/LCCF) and policy decisions.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

impl ScmsRun {
    /// The SCMS Manager decides a new policy at `at`: the Policy Generator signs a new
    /// Global Policy File and the Registration Authority issues the Local Policy File its
    /// devices fetch on their next connection.
    pub fn decide_policy_at(&mut self, policy: Policy, at: SimTime) -> FlowRun {
        let run = self.new_run();
        let manager = self.state.gov.nodes.manager;
        let at = at.max(self.kernel.now());
        self.inject_at(
            at,
            manager,
            ScmsMsg::PolicyUpdate(Box::new(policy)),
            FlowId::PolicyDistribution,
            run,
        );
        run
    }
}

impl ScmsState {
    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables)]
    pub(super) fn handle_gov(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
            ScmsMsg::FileRequest { device, lpf, lccf } if to == n.lop => {
                self.lop.upstream += 1;
                out.send(
                    n.ra,
                    ScmsMsg::FileRequest { device, lpf, lccf },
                    "file-request-proxied",
                    self.sizes.file_request(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::FileRequest { device, lpf, lccf } => {
                // The RA serves what it has signed: the Local Policy File and the Local
                // Certificate Chain File, each only if newer than the device's.
                let newer_lpf = lpf
                    .is_none_or(|v| v < self.gov.lpf.version)
                    .then(|| Box::new(self.gov.lpf.clone()));
                let newer_lccf = lccf
                    .is_none_or(|v| v < self.gov.lccf.version)
                    .then(|| Box::new(self.gov.lccf.clone()));
                self.ra.files_served +=
                    u32::from(newer_lpf.is_some()) + u32::from(newer_lccf.is_some());
                Self::sign(out, 1);
                let size = self.sizes.file_response(
                    newer_lpf.is_some(),
                    newer_lccf
                        .as_ref()
                        .map(|c| u32::try_from(c.certs.len()).unwrap_or(0)),
                );
                out.send(
                    n.lop,
                    ScmsMsg::FileResponse {
                        device,
                        lpf: newer_lpf,
                        lccf: newer_lccf,
                    },
                    "file-response",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::FileResponse { device, lpf, lccf } if to == n.lop => {
                self.lop.downstream += 1;
                let size = self.sizes.file_response(
                    lpf.is_some(),
                    lccf.as_ref()
                        .map(|c| u32::try_from(c.certs.len()).unwrap_or(0)),
                );
                out.send(
                    device,
                    ScmsMsg::FileResponse { device, lpf, lccf },
                    "file-response-proxied",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::FileResponse { device, lpf, lccf } => {
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                let before = dev.trust.verifications;
                if let Some(chain) = lccf {
                    let _ = dev.trust.install_chain(&chain, at);
                }
                if let Some(policy) = lpf {
                    let _ = dev.trust.install_policy(&policy);
                }
                let checks = dev.trust.verifications.saturating_sub(before);
                Self::verify(out, checks);
            }
            ScmsMsg::PolicyUpdate(policy) if to == self.gov.nodes.manager => {
                self.gov.counters.manager_decisions += 1;
                Self::sign(out, 1);
                out.stage_at(StageId::Decision, to, None, flow, run);
                out.send(
                    self.gov.nodes.pg,
                    ScmsMsg::PolicyUpdate(policy),
                    "policy-update",
                    self.sizes.policy_file(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PolicyUpdate(policy) if to == self.gov.nodes.pg => {
                Self::verify(out, 1);
                Self::sign(out, 2);
                self.gov.sign_global(*policy);
                out.stage_at(StageId::Issued, to, None, flow, run);
                out.send(
                    n.ra,
                    ScmsMsg::PolicyUpdate(policy),
                    "policy-publish",
                    self.sizes.policy_file(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PolicyUpdate(_) => {
                Self::verify(out, 1);
                Self::sign(out, 2);
                self.gov.sign_local();
                out.stage_at(StageId::Published, to, None, flow, run);
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
