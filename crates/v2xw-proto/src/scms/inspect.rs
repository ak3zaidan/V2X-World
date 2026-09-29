//! The SCMS deployment as an inspector sees it: every entity's queue, traffic, keys and
//! counts, and the messages flowing between them and the devices.

use serde_json::json;
use v2xw_core::hash::hex_encode;
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;

use crate::scms::run::ScmsRun;
use crate::view::{BackendView, EdgeTracker, entity};

impl ScmsRun {
    /// The role id `node` is known by in a view: an authority's short name, or `ee` for
    /// any device.
    #[must_use]
    pub fn role_of(&self, node: NodeId) -> String {
        let n = self.state.nodes;
        let g = &self.state.gov.nodes;
        let named = [
            (n.ra, "ra"),
            (n.pca, "pca"),
            (n.la1, "la1"),
            (n.la2, "la2"),
            (n.ma, "ma"),
            (n.crlg, "crlg"),
            (n.lop, "lop"),
            (n.crl_store, "crl-store"),
            (n.crl_broadcast, "crl-broadcast"),
            (n.eca, "eca"),
            (n.dcm, "dcm"),
            (g.manager, "manager"),
            (g.pg, "pg"),
            (g.root, "root"),
            (g.ica, "ica"),
        ];
        named
            .iter()
            .find(|(id, _)| *id == node)
            .map(|(_, r)| (*r).to_string())
            .or_else(|| g.electors.contains(&node).then(|| "electors".to_string()))
            .unwrap_or_else(|| "ee".to_string())
    }

    /// Every entity at `now`, and the traffic between them since the run began.
    ///
    /// `tracker` carries the fold of the wire log between calls; pass the same one each
    /// time.
    #[allow(clippy::too_many_lines)]
    pub fn backend_view(&self, now: SimTime, tracker: &mut EdgeTracker) -> BackendView {
        tracker.absorb(&self.kernel, |node| self.role_of(node));
        let k = &self.kernel;
        let st = &self.state;
        let n = st.nodes;
        let g = &st.gov;
        let sys = "scms";
        let key = |role: &str| {
            g.certs
                .get(role)
                .map(|c| hex_encode(&c.digest()))
                .unwrap_or_default()
        };
        let mut out = Vec::new();

        let mut manager = entity(
            k,
            now,
            sys,
            "manager",
            "SCMS Manager",
            "governance",
            "Sets policy for the whole system; its decisions reach devices as newer policy files.",
            Some(g.nodes.manager),
            true,
        );
        manager.set("policy_decisions", g.counters.manager_decisions);
        manager.set("gpf_version", g.gpf.version);
        out.push(manager);

        let mut pg = entity(
            k,
            now,
            sys,
            "pg",
            "Policy Generator",
            "governance",
            "Signs the Global Policy File and the Global Certificate Chain File.",
            Some(g.nodes.pg),
            true,
        );
        pg.set("gpf_version", g.gpf.version);
        pg.set("gccf_version", g.gccf.version);
        pg.set("files_signed", g.counters.pg_files_signed);
        pg.set("certs_per_period", g.gpf.policy.certs_per_period);
        pg.set("i_period_s", g.gpf.policy.i_period_s);
        pg.set("certificate", key("pg"));
        out.push(pg);

        let mut electors = entity(
            k,
            now,
            sys,
            "electors",
            "Electors",
            "governance",
            "Endorse the certificate trust list; a device accepts it only with a quorum.",
            None,
            false,
        );
        electors.set("electors", g.nodes.electors.len() as u64);
        electors.set("quorum", g.ctl.quorum);
        electors.set("endorsements", g.counters.elector_endorsements);
        electors.set("ctl_sequence", g.ctl.sequence);
        out.push(electors);

        let mut root = entity(
            k,
            now,
            sys,
            "root",
            "Root CA",
            "ca",
            "Offline trust anchor; certifies the ICA, PG, MA and CRL Generator.",
            None,
            false,
        );
        root.set("certs_issued", g.counters.root_certs_issued);
        root.set("certificate", key("root"));
        out.push(root);

        let mut ica = entity(
            k,
            now,
            sys,
            "ica",
            "Intermediate CA",
            "ca",
            "Certifies the online authorities: ECA, PCA, RA, LAs, LOP, DCM, CRL Store.",
            None,
            false,
        );
        ica.set("certs_issued", g.counters.ica_certs_issued);
        ica.set("certificate", key("ica"));
        out.push(ica);

        let mut dcm = entity(
            k,
            now,
            sys,
            "dcm",
            "Device Configuration Manager",
            "ca",
            "Bootstraps devices: forwards enrolment to the ECA and installs the trust bundle.",
            Some(n.dcm),
            true,
        );
        dcm.set("devices_bootstrapped", st.dcm.bootstrapped);
        dcm.set("trust_bundles", st.dcm.bundles);
        out.push(dcm);

        let mut eca = entity(
            k,
            now,
            sys,
            "eca",
            "Enrolment CA",
            "ca",
            "Issues enrolment certificates, and successors before they expire.",
            Some(n.eca),
            true,
        );
        eca.set("enrolment_certs_issued", st.eca.issued);
        eca.set("successors_issued", st.eca.successors);
        eca.set("successors_refused", st.eca.refused);
        eca.set("blocklisted", st.eca.blocklist.len() as u64);
        eca.set("certificate", key("eca"));
        out.push(eca);

        let mut lop = entity(
            k,
            now,
            sys,
            "lop",
            "Location Obscurer Proxy",
            "privacy",
            "Relays every device-to-RA message with its network address stripped.",
            Some(n.lop),
            true,
        );
        lop.set("relayed_to_ra", st.lop.upstream);
        lop.set("relayed_to_devices", st.lop.downstream);
        out.push(lop);

        let mut ra = entity(
            k,
            now,
            sys,
            "ra",
            "Registration Authority",
            "ra",
            "Checks enrolment, expands butterfly keys, shuffles requests, serves batches and \
             local policy; keeps the blocklist.",
            Some(n.ra),
            true,
        );
        ra.set("requests_accepted", st.ra.accepted);
        ra.set("refused_blocklisted", st.ra.refused);
        ra.set("refused_enrolment", st.ra.refused_enrolment);
        ra.set("clipped_to_policy", st.ra.clipped);
        ra.set("files_served", st.ra.files_served);
        ra.set("blocklist", st.ra.blocklist.len() as u64);
        ra.set("enrolled_devices", st.ra.enrolled.len() as u64);
        ra.set("jobs_pending", st.ra.pending_jobs() as u64);
        ra.set("reports_in_shuffle", st.ra.reports_waiting() as u64);
        ra.set("batches_held", st.ra.batches_held() as u64);
        ra.set("lpf_version", g.lpf.version);
        ra.set(
            "shuffle_window_s",
            st.params.shuffle_window.as_nanos() / 1_000_000_000,
        );
        out.push(ra);

        for (idx, id, name, node) in [
            (0usize, "la1", "Linkage Authority 1", n.la1),
            (1, "la2", "Linkage Authority 2", n.la2),
        ] {
            let la = &st.la[idx];
            let mut e = entity(
                k,
                now,
                sys,
                id,
                name,
                "privacy",
                "Holds one half of every device's linkage seed chain; seals pre-linkage \
                 values for the PCA and releases ls(i) only for a revocation.",
                Some(node),
                true,
            );
            e.set("chains", la.chains.len() as u64);
            e.set("plv_issued", la.plv_issued);
            e.set("seeds_released", la.seeds_released);
            out.push(e);
        }

        let mut pca = entity(
            k,
            now,
            sys,
            "pca",
            "Pseudonym CA",
            "ra",
            "Certifies cocoon keys as pseudonym certificates, encrypted to the device; never \
             learns whose they are.",
            Some(n.pca),
            true,
        );
        pca.set("certs_issued", st.pca.issued_count);
        pca.set("lookups_answered", st.pca.lookups);
        pca.set("certificate", key("pca"));
        out.push(pca);

        let mut ma = entity(
            k,
            now,
            sys,
            "ma",
            "Misbehaviour Authority",
            "revocation",
            "Receives reports, decides, and resolves a certificate to its device through the \
             PCA, the RA and both LAs.",
            Some(n.ma),
            true,
        );
        ma.set("reports_received", st.ma.reports.len() as u64);
        ma.set("revocations", st.ma.decisions.len() as u64);
        ma.set(
            "case_open",
            st.ma.case.as_ref().is_some_and(|c| !c.done),
        );
        out.push(ma);

        let mut crlg = entity(
            k,
            now,
            sys,
            "crlg",
            "CRL Generator",
            "revocation",
            "Signs the linked CRL on its cadence.",
            Some(n.crlg),
            true,
        );
        crlg.set("entries", st.crlg.entries.len() as u64);
        crlg.set("versions_signed", st.crlg.versions);
        crlg.set(
            "cadence_s",
            st.params.crl_cadence.as_nanos() / 1_000_000_000,
        );
        out.push(crlg);

        let mut store = entity(
            k,
            now,
            sys,
            "crl-store",
            "CRL Store",
            "distribution",
            "Serves the signed CRL to devices that download it over cellular.",
            Some(n.crl_store),
            true,
        );
        store.set("entries", st.crl_store.entries.len() as u64);
        store.set("versions", st.crl_store.versions);
        out.push(store);

        let mut bcast = entity(
            k,
            now,
            sys,
            "crl-broadcast",
            "CRL Broadcast",
            "distribution",
            "Hands the signed CRL to roadside units that repeat it on the air.",
            Some(n.crl_broadcast),
            true,
        );
        bcast.set("entries", st.crl_broadcast.entries.len() as u64);
        bcast.set("versions", st.crl_broadcast.versions);
        out.push(bcast);

        let mut ee = entity(
            k,
            now,
            sys,
            "ee",
            "Devices",
            "device",
            "Vehicles and roadside units: enrolled, holding pseudonym batches, reporting, \
             enforcing the CRL.",
            None,
            true,
        );
        let mut enrolled = 0u64;
        let mut holding = 0u64;
        let mut credentials = 0u64;
        let mut refused = 0u64;
        let mut reenrolled = 0u64;
        let mut silenced = 0u64;
        let mut trusted = 0u64;
        let mut crls_rejected = 0u64;
        let mut trust_rejections = 0u64;
        for d in st.devices.values() {
            enrolled += u64::from(d.enrolment.is_some());
            holding += u64::from(!d.credentials.is_empty());
            credentials += d.credentials.len() as u64;
            refused += u64::from(d.refused.is_some());
            reenrolled += u64::from(d.reenrolments);
            silenced += u64::from(d.silenced);
            trusted += u64::from(d.trust.ctl_sequence.is_some() && d.trust.lpf.is_some());
            crls_rejected += u64::from(d.crls_rejected);
            trust_rejections += u64::from(d.trust.rejected);
        }
        ee.set("devices", st.devices.len() as u64);
        ee.set("enrolled", enrolled);
        ee.set("trust_installed", trusted);
        ee.set("holding_pseudonyms", holding);
        ee.set("pseudonym_certificates", credentials);
        ee.set("refused_now", refused);
        ee.set("re_enrolments", reenrolled);
        ee.set("self_revoked", silenced);
        ee.set("crls_rejected", crls_rejected);
        ee.set("trust_artefacts_rejected", trust_rejections);
        let mut traffic = crate::kernel::NodeTraffic::default();
        let mut ops = std::collections::BTreeMap::new();
        for d in st.devices.keys() {
            let t = k.traffic(*d);
            traffic.received += t.received;
            traffic.sent += t.sent;
            traffic.bytes_in += t.bytes_in;
            traffic.bytes_out += t.bytes_out;
            for (op, c) in crate::view::ops_of(k, *d) {
                *ops.entry(op).or_insert(0) += c;
            }
        }
        ee.traffic = traffic;
        ee.ops = ops;
        out.push(ee);

        let mut flows = tracker.flows();
        flows.retain(|_, v| *v > 0);
        BackendView {
            system: sys,
            protocol: crate::scms::CAMP_SCMS_ID,
            t: now,
            entities: out,
            edges: tracker.edges(),
            recent: tracker.recent(),
            flows,
        }
    }
}

/// The policy a device holds, as JSON, for an inspector.
#[must_use]
pub fn device_policy_json(run: &ScmsRun, device: NodeId) -> serde_json::Value {
    run.state
        .devices
        .get(&device)
        .map_or(serde_json::Value::Null, |d| {
            json!({
                "enrolment": d.enrolment,
                "ctl_sequence": d.trust.ctl_sequence,
                "lccf_version": d.trust.lccf_version,
                "lpf_version": d.trust.lpf.as_ref().map(|l| l.version),
                "policy": d.trust.policy(),
                "refused": d.refused,
                "re_enrolments": d.reenrolments,
            })
        })
}
