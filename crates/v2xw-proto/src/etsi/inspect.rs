//! The European CCMS as an inspector sees it: the Trust List Manager, the Central Point of
//! Contact, the Root CA, the Enrolment and Authorization Authorities and the Misbehaviour
//! Authority, with the devices pooled as one node.

use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;

use crate::etsi::ts102941::{ETSI_TS102941_ID, EtsiRun};
use crate::view::{BackendView, EdgeTracker, entity};

impl EtsiRun {
    /// The role id `node` is known by in a view.
    #[must_use]
    pub fn role_of(&self, node: NodeId) -> String {
        let n = self.nodes;
        [
            (n.ea, "ea"),
            (n.aa, "aa"),
            (n.rca, "rca"),
            (n.tlm, "tlm"),
            (n.cpoc, "cpoc"),
            (n.ma, "ma"),
        ]
        .iter()
        .find(|(id, _)| *id == node)
        .map_or_else(|| "ee".to_string(), |(_, r)| (*r).to_string())
    }

    /// Every entity at `now`, and the traffic between them since the run began.
    pub fn backend_view(&self, now: SimTime, tracker: &mut EdgeTracker) -> BackendView {
        tracker.absorb(&self.kernel, |node| self.role_of(node));
        let k = &self.kernel;
        let n = self.nodes;
        let sys = "ccms";
        let mut out = Vec::new();

        let mut tlm = entity(
            k,
            now,
            sys,
            "tlm",
            "Trust List Manager",
            "governance",
            "Signs the European Certificate Trust List of approved Root CAs.",
            Some(n.tlm),
            true,
        );
        tlm.set("ectl_sequence", self.ctl_sequence);
        tlm.set("ectl_entries", self.params.ctl_entries);
        out.push(tlm);

        let mut cpoc = entity(
            k,
            now,
            sys,
            "cpoc",
            "Central Point of Contact",
            "distribution",
            "Publishes the ECTL and the Root CAs' CA-CRLs and CTLs to stations.",
            Some(n.cpoc),
            true,
        );
        cpoc.set("stations_with_ectl", self.installed_ctl.len() as u64);
        cpoc.set(
            "stations_with_ca_crl",
            self.installed_ca_crl.len() as u64,
        );
        cpoc.set("ectl_served", self.dc_ectl.map_or(0, |(s, _)| s));
        cpoc.set("ca_crl_served", self.dc_ca_crl.map_or(0, |(s, _)| s));
        cpoc.set("fetches_answered", self.dc_fetches);
        cpoc.set("answered_current", self.dc_not_modified);
        out.push(cpoc);

        let mut rca = entity(
            k,
            now,
            sys,
            "rca",
            "Root CA",
            "ca",
            "Certifies the EA and the AA; signs its CTL and the CA-only CRL.",
            Some(n.rca),
            true,
        );
        rca.set("ca_crl_sequence", self.ca_crl_sequence);
        rca.set("ca_crl_entries", self.params.ca_crl_entries);
        out.push(rca);

        let mut ea = entity(
            k,
            now,
            sys,
            "ea",
            "Enrolment Authority",
            "ca",
            "Issues enrolment credentials, validates authorization requests for the AA, \
             keeps the blocklist (passive revocation).",
            Some(n.ea),
            true,
        );
        ea.set("enrolled", self.enrolled.len() as u64);
        ea.set("blocklisted", self.blocklist.len() as u64);
        ea.set("requests_refused", self.refused);
        ea.set("butterfly_batches_waiting", self.pending_batches.len() as u64);
        ea.set("current_i", self.current_i);
        out.push(ea);

        let mut aa = entity(
            k,
            now,
            sys,
            "aa",
            "Authorization Authority",
            "ra",
            "Issues authorization tickets after the EA vouches for an enrolment it cannot \
             see.",
            Some(n.aa),
            true,
        );
        aa.set("tickets_issued", self.aa.tickets_issued);
        aa.set("validations_requested", self.aa.validations_requested);
        aa.set("refused", self.aa.refused);
        aa.set("butterfly_batches", self.aa.butterfly_batches);
        out.push(aa);

        let mut ma = entity(
            k,
            now,
            sys,
            "ma",
            "Misbehaviour Authority",
            "revocation",
            "Receives TS 103 759 reports and has the EA block an enrolment credential.",
            Some(n.ma),
            true,
        );
        ma.set(
            "reports_received",
            self.reports.values().map(|v| u64::from(*v)).sum::<u64>(),
        );
        ma.set("reports_pre_processed", self.pre_processed);
        out.push(ma);

        let mut ee = entity(
            k,
            now,
            sys,
            "ee",
            "Stations",
            "device",
            "Vehicles and roadside units: enrolled, holding authorization tickets, reporting.",
            None,
            true,
        );
        ee.set("stations", self.tickets.len() as u64);
        ee.set(
            "authorization_tickets",
            self.tickets.values().map(|v| u64::from(*v)).sum::<u64>(),
        );
        let mut traffic = crate::kernel::NodeTraffic::default();
        for s in self.tickets.keys() {
            let t = k.traffic(*s);
            traffic.received += t.received;
            traffic.sent += t.sent;
            traffic.bytes_in += t.bytes_in;
            traffic.bytes_out += t.bytes_out;
        }
        ee.traffic = traffic;
        out.push(ee);

        BackendView {
            system: sys,
            protocol: ETSI_TS102941_ID,
            t: now,
            entities: out,
            edges: tracker.edges(),
            recent: tracker.recent(),
            flows: tracker.flows(),
        }
    }
}
