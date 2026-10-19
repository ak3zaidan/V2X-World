//! Red team, PKI side (docs/redteam/pki-privacy.md): what insiders and a compromised
//! roadside unit can do to the real SCMS flows, and how long revocation takes on the real
//! calendar. Every number the document quotes is printed here as an `RT ` line:
//!
//!     cargo test -p v2xw-proto --test redteam_pki -- --nocapture --test-threads=1
//!
//! Two of these tests pinned weaknesses (R2: a replayed older CRL; B4: a second
//! provisioning chain that outlived the revocation of the first). Both are fixed and the
//! tests are now regressions for the fixes (docs/design/scms-real/validation/protocols.md).
//! Each was shown to fail with its fix removed.

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use common::{DEVICE_A, DEVICE_B, DEVICE_C, deployment, provisioned};
use v2xw_core::ids::NodeId;
use v2xw_proto::scms::calendar::{
    civil_from_days, days_from_civil, eastern_date, eastern_to_unix, is_business_day,
    next_publication_slot, publication_deadline,
};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_sec::linkage::{
    LinkageValue, crl_contains, linkage_seed_at, linkage_value, pre_linkage_value,
};

const JMAX: u32 = 2;
const DEVICE_X: NodeId = NodeId::new(1_003);

// -----------------------------------------------------------------------------------------
// 1. Collusion: what each coalition of backend authorities learns
// -----------------------------------------------------------------------------------------

/// A certificate as an eavesdropper sees it on the air: its period and linkage value.
type AirCert = (u32, [u8; 9]);

/// Three devices, two periods, two certificates a period: twelve certificates on the air.
fn fleet() -> (ScmsRun, BTreeMap<AirCert, NodeId>) {
    let mut run = deployment(3);
    for d in [DEVICE_A, DEVICE_B, DEVICE_C] {
        provisioned(&mut run, d, 0, 2, JMAX);
    }
    // Ground truth, from the devices (never from an authority's state).
    let mut truth = BTreeMap::new();
    for d in [DEVICE_A, DEVICE_B, DEVICE_C] {
        for (&(i, _), c) in &run.state.devices[&d].credentials {
            truth.insert((i, *c.lv.as_bytes()), d);
        }
    }
    (run, truth)
}

/// What a coalition concluded: certificate groups it can say belong to one device, and the
/// device it can name for a certificate.
#[derive(Default)]
struct Learned {
    groups: Vec<BTreeSet<AirCert>>,
    named: BTreeMap<AirCert, NodeId>,
}

/// Scores a coalition against the truth: (fraction of same-device certificate pairs it
/// links, fraction of certificates it names correctly, wrong links + wrong names).
fn score(l: &Learned, truth: &BTreeMap<AirCert, NodeId>) -> (f64, f64, usize) {
    let mut by_dev: BTreeMap<NodeId, usize> = BTreeMap::new();
    for d in truth.values() {
        *by_dev.entry(*d).or_default() += 1;
    }
    let pairs_total: usize = by_dev.values().map(|n| n * (n - 1) / 2).sum();
    let mut linked = BTreeSet::new();
    let mut wrong = 0usize;
    for g in &l.groups {
        let v: Vec<_> = g.iter().collect();
        for a in 0..v.len() {
            for b in a + 1..v.len() {
                if truth.get(v[a]) == truth.get(v[b]) {
                    linked.insert((*v[a], *v[b]));
                } else {
                    wrong += 1;
                }
            }
        }
    }
    let named_ok = l
        .named
        .iter()
        .filter(|(c, d)| truth.get(*c) == Some(*d))
        .count();
    wrong += l.named.len() - named_ok;
    (
        linked.len() as f64 / pairs_total as f64,
        named_ok as f64 / truth.len() as f64,
        wrong,
    )
}

/// Groups certificates that share a key.
fn group_by<K: Ord>(certs: impl Iterator<Item = (K, AirCert)>) -> Vec<BTreeSet<AirCert>> {
    let mut m: BTreeMap<K, BTreeSet<AirCert>> = BTreeMap::new();
    for (k, c) in certs {
        m.entry(k).or_default().insert(c);
    }
    m.into_values().filter(|g| g.len() > 1).collect()
}

/// LA1 and LA2 together, with nothing but their own seeds and the air: for every period
/// and index they expand every chain, and a certificate whose linkage value is
/// `plv1 ⊕ plv2` for one chain of each is that chain pair's. A hash set makes the pairing
/// linear in the chains (no `n1 × n2` search), so this is what a real pair of colluding
/// LAs can do at deployment scale.
fn la_pair_links(run: &ScmsRun, air: &BTreeSet<AirCert>) -> Vec<BTreeSet<AirCert>> {
    let la1 = &run.state.la[0];
    let la2 = &run.state.la[1];
    let mut pair_of: BTreeMap<AirCert, (u64, u64)> = BTreeMap::new();
    let periods: BTreeSet<u32> = air.iter().map(|c| c.0).collect();
    for &i in &periods {
        for j in 0..JMAX {
            let plv2: HashMap<[u8; 9], u64> = la2
                .chains
                .iter()
                .map(|(lci, ls0)| {
                    let p = pre_linkage_value(la2.la_id, linkage_seed_at(la2.la_id, *ls0, i), j);
                    (*p.as_bytes(), lci.0)
                })
                .collect();
            for (lci1, ls0) in &la1.chains {
                let p1 = pre_linkage_value(la1.la_id, linkage_seed_at(la1.la_id, *ls0, i), j);
                for c in air.iter().filter(|c| c.0 == i) {
                    let mut want = [0u8; 9];
                    for k in 0..9 {
                        want[k] = c.1[k] ^ p1.as_bytes()[k];
                    }
                    if let Some(lci2) = plv2.get(&want) {
                        // Confirm through the real combiner, not the XOR above.
                        let p2 = v2xw_sec::linkage::PreLinkageValue::from_bytes(want);
                        assert_eq!(linkage_value(p1, p2), LinkageValue::new(c.1));
                        pair_of.insert(*c, (lci1.0, *lci2));
                    }
                }
            }
        }
    }
    group_by(pair_of.into_iter().map(|(c, p)| (p, c)))
}

#[test]
fn coalitions_learn_what_the_separation_rules_predict() {
    let (run, truth) = fleet();
    let air: BTreeSet<AirCert> = truth.keys().copied().collect();
    assert_eq!(
        air.len(),
        12,
        "every certificate's linkage value is distinct on the air"
    );
    let st = &run.state;
    let device_of_handle = |h: &[u8; 32]| {
        st.ra
            .handles
            .get(h)
            .and_then(|req| st.ra.requests.get(req))
            .map(|r| r.device)
    };

    // The eavesdropper alone: certificates share nothing.
    let eaves = Learned {
        groups: group_by(air.iter().map(|c| (c.1, *c))),
        ..Learned::default()
    };
    // The ACA alone: its per-certificate handle (the hash of one RA→ACA request).
    let aca = Learned {
        groups: group_by(
            st.pca
                .issued
                .iter()
                .map(|(&(i, lv), l)| (l.handle, (i, lv))),
        ),
        ..Learned::default()
    };
    // One LA alone: a pre-linkage value is not a linkage value, so nothing on the air is
    // one of its values.
    let la1 = &st.la[0];
    let mut la1_hits = 0usize;
    for (_, ls0) in &la1.chains {
        for c in &air {
            for j in 0..JMAX {
                let p = pre_linkage_value(la1.la_id, linkage_seed_at(la1.la_id, *ls0, c.0), j);
                la1_hits += usize::from(p.as_bytes() == &c.1);
            }
        }
    }
    // LA1 + LA2: every certificate grouped by chain pair, nobody named.
    let la_pair = Learned {
        groups: la_pair_links(&run, &air),
        ..Learned::default()
    };
    // RA + LA1 + LA2: the RA's request record holds the chain pair and the device.
    let mut ra_la = Learned {
        groups: la_pair.groups.clone(),
        ..Learned::default()
    };
    let lci_owner: BTreeMap<(u64, u64), NodeId> = st
        .ra
        .requests
        .values()
        .filter_map(|r| Some(((r.lci1?.0, r.lci2?.0), r.device)))
        .collect();
    for (c, d) in la_pair_named(&run, &air, &lci_owner) {
        ra_la.named.insert(c, d);
    }
    // ACA + RA: handle → RA request → device, for every certificate the ACA issued.
    let mut aca_ra = Learned::default();
    for (&(i, lv), l) in &st.pca.issued {
        if let Some(d) = device_of_handle(&l.handle) {
            aca_ra.named.insert((i, lv), d);
        }
    }
    aca_ra.groups = group_by(aca_ra.named.iter().map(|(c, d)| (*d, *c)));

    let rows = [
        ("eavesdropper", &eaves),
        ("ACA", &aca),
        ("LA1+LA2", &la_pair),
        ("RA+LA1+LA2", &ra_la),
        ("ACA+RA", &aca_ra),
    ];
    let mut got = BTreeMap::new();
    for (name, l) in rows {
        let (link, name_frac, wrong) = score(l, &truth);
        println!(
            "RT collusion {name:<13} linked_pairs={link:.2} named={name_frac:.2} wrong={wrong}"
        );
        got.insert(name, (link, name_frac, wrong));
    }
    println!("RT collusion LA1-alone      plv_equal_to_an_lv={la1_hits}");
    println!(
        "RT collusion RA-alone       holds_lv={} (requests={})",
        format!("{:?}", st.ra).contains(&hex(air.iter().next().unwrap().1)),
        st.ra.requests.len()
    );

    assert_eq!(
        got["eavesdropper"],
        (0.0, 0.0, 0),
        "linkage values unlinkable on the air"
    );
    assert_eq!(
        got["ACA"],
        (0.0, 0.0, 0),
        "the ACA alone cannot group a device's certificates"
    );
    assert_eq!(la1_hits, 0, "one LA alone recognises no certificate");
    assert_eq!(
        got["LA1+LA2"],
        (1.0, 0.0, 0),
        "two LAs link everything and name nobody"
    );
    assert_eq!(
        got["RA+LA1+LA2"],
        (1.0, 1.0, 0),
        "with the RA they name every device"
    );
    assert_eq!(
        got["ACA+RA"],
        (1.0, 1.0, 0),
        "the ACA and the RA name every device"
    );
}

/// The device the RA's chain-pair record names for each certificate LA1+LA2 can place.
fn la_pair_named(
    run: &ScmsRun,
    air: &BTreeSet<AirCert>,
    lci_owner: &BTreeMap<(u64, u64), NodeId>,
) -> Vec<(AirCert, NodeId)> {
    let la1 = &run.state.la[0];
    let la2 = &run.state.la[1];
    let mut out = Vec::new();
    for c in air {
        'found: for (l1, s1) in &la1.chains {
            for (l2, s2) in &la2.chains {
                for j in 0..JMAX {
                    let p1 = pre_linkage_value(la1.la_id, linkage_seed_at(la1.la_id, *s1, c.0), j);
                    let p2 = pre_linkage_value(la2.la_id, linkage_seed_at(la2.la_id, *s2, c.0), j);
                    if linkage_value(p1, p2) == LinkageValue::new(c.1) {
                        if let Some(d) = lci_owner.get(&(l1.0, l2.0)) {
                            out.push((*c, *d));
                        }
                        break 'found;
                    }
                }
            }
        }
    }
    out
}

fn hex(b: [u8; 9]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

// -----------------------------------------------------------------------------------------
// 2. A compromised roadside unit and the CRL
// -----------------------------------------------------------------------------------------

/// X is revoked (CRL v1 = [X]), then A (v2 = [X, A]). C and A fetch v2 over cellular.
/// Returns the run and v1 as the CRL Generator signed it, which any RSU that carried it
/// still holds.
#[allow(clippy::type_complexity)]
fn two_revocations() -> (
    ScmsRun,
    (
        Vec<v2xw_sec::linkage::CrlLinkageEntry>,
        Option<([u8; 8], [u8; 64])>,
        Option<v2xw_proto::scms::pdu::Pdu>,
        Option<v2xw_proto::scms::pdu::Pdu>,
    ),
) {
    let mut run = deployment(4);
    for d in [DEVICE_X, DEVICE_A, DEVICE_C, DEVICE_B] {
        provisioned(&mut run, d, 0, 1, JMAX);
    }
    let lvx = run.state.devices[&DEVICE_X].credentials[&(0, 0)].lv;
    run.submit_report(DEVICE_B, 0, lvx);
    run.run().expect("report");
    run.revoke(0, 0, JMAX).expect("held");
    run.run().expect("revocation of X");
    let b = &run.state.crl_broadcast;
    let v1 = (
        b.entries.clone(),
        b.signature,
        b.secured.clone(),
        b.delta.clone(),
    );
    let lva = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    run.submit_report(DEVICE_B, 0, lva);
    run.run().expect("report");
    run.revoke(1, 0, JMAX).expect("held");
    run.run().expect("revocation of A");
    run.distribute_crl(DEVICE_C);
    run.distribute_crl(DEVICE_A);
    run.run().expect("cellular CRL fetch");
    (run, v1)
}

/// R2 (fixed): an RSU that replays the CRL Generator's own earlier, validly signed list
/// once erased a later revocation at every vehicle that heard it, and a revoked device
/// that checked itself against it resumed transmitting. Nothing is forged, so no signature
/// check can catch it; the device now refuses a CRL whose 1609.2 counter (`iRev`,
/// `indexWithinI`, then `issueDate`) is not above the one it holds for that CRACA and
/// series, and the same list heard again is still installed.
#[test]
fn a_replayed_older_crl_is_refused_and_the_newer_revocation_holds() {
    let (mut run, v1) = two_revocations();
    let lva = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    let lvx = run.state.devices[&DEVICE_X].credentials[&(0, 0)].lv;
    assert!(crl_contains(&run.state.devices[&DEVICE_C].crl, 0, 0, lva));
    assert!(run.state.devices[&DEVICE_A].silenced);
    // The compromised RSU puts v1 back on the air.
    let b = &mut run.state.crl_broadcast;
    (b.entries, b.signature, b.secured, b.delta) = v1;
    for d in [DEVICE_C, DEVICE_A] {
        run.attach_rsu(d);
        run.broadcast_crl_to(d);
    }
    run.run().expect("replayed broadcast");
    let c = &run.state.devices[&DEVICE_C];
    let rolled_back = !crl_contains(&c.crl, 0, 0, lva);
    println!(
        "RT crl-replay stale_refusals_at_C={} A_revoked_at_C={} X_revoked_at_C={} A_silenced={}",
        c.crls_stale,
        !rolled_back,
        crl_contains(&c.crl, 0, 0, lvx),
        run.state.devices[&DEVICE_A].silenced
    );
    assert_eq!(c.crls_rejected, 0, "v1 is genuine: its signature verifies");
    assert_eq!(c.crls_stale, 1, "v1 is refused as older than v2");
    assert_eq!(run.state.devices[&DEVICE_A].crls_stale, 1);
    assert!(!rolled_back, "A stays revoked at C");
    assert!(crl_contains(&c.crl, 0, 0, lvx), "and X too");
    assert!(run.state.devices[&DEVICE_A].silenced, "A stays silent");
    assert!(run.state.pdu_rejections.iter().any(|r| {
        r.kind == v2xw_proto::scms::pdu::SpduKind::SecuredCrl
            && r.reason.to_string().contains("stale CRL")
    }));
    // The current list heard again (an RSU repeating v2) is not a replay: it is installed
    // again, which changes nothing and refuses nothing.
    run.distribute_crl(DEVICE_C);
    run.run().expect("v2 again");
    let c = &run.state.devices[&DEVICE_C];
    assert_eq!((c.crls_stale, c.crls_rejected), (1, 0));
    assert!(crl_contains(&c.crl, 0, 0, lva));
}

/// A compromised RSU that edits the list it carries (adds an honest vehicle, or drops a
/// revoked one) is refused whole: the device acts only on the signed bytes.
#[test]
fn an_rsu_edited_crl_is_refused_whole() {
    let (mut run, _) = two_revocations();
    let lva = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    let lvb = run.state.devices[&DEVICE_B].credentials[&(0, 0)].lv;
    // Frame B: an entry built from B's own context is what a forger would need, and even
    // that does not help without the CRL Generator's key. Here: drop A instead, and add a
    // copy of X's entry under B's period (the list differs from the signed bytes).
    let b = &mut run.state.crl_broadcast;
    let x_entry = b.entries[0];
    b.entries.retain(|e| *e == x_entry);
    b.entries.push(x_entry);
    run.attach_rsu(DEVICE_C);
    run.broadcast_crl_to(DEVICE_C);
    run.run().expect("edited broadcast");
    let c = &run.state.devices[&DEVICE_C];
    println!(
        "RT crl-edit rejected={} A_still_revoked_at_C={} B_revoked_at_C={}",
        c.crls_rejected,
        crl_contains(&c.crl, 0, 0, lva),
        crl_contains(&c.crl, 0, 0, lvb)
    );
    assert_eq!(c.crls_rejected, 1, "the edited list is refused");
    assert!(
        crl_contains(&c.crl, 0, 0, lva),
        "the device keeps the list it had"
    );
}

// -----------------------------------------------------------------------------------------
// 3. A second provisioning chain (a stolen enrolment key, or a device hoarding)
// -----------------------------------------------------------------------------------------

/// B4 (fixed): after any policy change the RA accepts a second butterfly request from the
/// same enrolment certificate ([CAMP122] SCMS-520), and that request gets its own LA chain
/// pair. Revocation once published only the reported chain's seeds, so every certificate
/// of the other chain stayed valid for as long as it was issued for (a thief holding the
/// enrolment key kept an unlinked batch; a misbehaving device kept one through its own
/// revocation). The RA blocklists the enrolment, so it now returns the chain identifiers
/// of every request it accepted from it ([BRECHT §VI-D] per request), and the MA has the
/// seeds of each released: one CRL entry per chain, every certificate covered.
#[test]
fn every_chain_of_a_revoked_enrolment_is_revoked() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    provisioned(&mut run, DEVICE_B, 0, 1, JMAX);
    let reported = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    let mut policy = run.state.gov.gpf.policy;
    policy.certs_per_period = 1;
    run.decide_policy_at(policy, run.kernel.now());
    run.run().expect("policy");
    run.provision(DEVICE_A, 0, 2, JMAX);
    run.run().expect("second request");
    let chains = run
        .state
        .ra
        .requests
        .values()
        .filter(|r| r.device == DEVICE_A)
        .count();
    run.submit_report(DEVICE_B, 0, LinkageValue::new(*reported.as_bytes()));
    run.run().expect("report");
    run.revoke(0, 0, JMAX).expect("held");
    run.run().expect("revocation");
    let st = &run.state;
    let crl = &st.crl_store.entries;
    let (mut covered, mut escaped) = (0, 0);
    for (&(i, lv), l) in &st.pca.issued {
        let owner = st
            .ra
            .handles
            .get(&l.handle)
            .and_then(|h| st.ra.requests.get(h))
            .map(|r| r.device);
        if owner == Some(DEVICE_A) {
            if crl
                .iter()
                .any(|e| e.matches_any_index(i, LinkageValue::new(lv)))
            {
                covered += 1;
            } else {
                escaped += 1;
            }
        }
    }
    println!(
        "RT second-chain accepted_requests={chains} crl_entries={} certs_revoked={covered} certs_escaped={escaped} enrolment_blocklisted={}",
        crl.len(),
        st.ra.blocklist.contains(&DEVICE_A)
    );
    assert_eq!(chains, 2, "the RA accepted the second request");
    assert!(st.ra.blocklist.contains(&DEVICE_A));
    assert_eq!(crl.len(), 2, "one entry per chain pair");
    assert!(covered > 0, "{covered}");
    assert_eq!(escaped, 0, "no certificate of the device escapes");
    // Device B, never reported, stays valid: the second entry is A's own chain.
    for (&(i, lv), l) in &st.pca.issued {
        let owner = st
            .ra
            .handles
            .get(&l.handle)
            .and_then(|h| st.ra.requests.get(h))
            .map(|r| r.device);
        if owner == Some(DEVICE_B) {
            assert!(
                !crl.iter()
                    .any(|e| e.matches_any_index(i, LinkageValue::new(lv))),
                "B is not revoked"
            );
        }
    }
    let case = st.ma.cases.values().find(|c| c.done).expect("a case");
    assert_eq!(case.chains.len(), 2);
    assert!(case.chain_seeds.iter().flatten().all(Option::is_some));
}

// -----------------------------------------------------------------------------------------
// 4. Detection to publication and enforcement on the real calendar
// -----------------------------------------------------------------------------------------

/// The end of `work_s` seconds of an operator's time who works 09:00-17:00 Eastern on
/// business days only (the CV-pilot "minimum viable" MA is a person, [PRIMER] pp. 6, 8).
fn business_hours_done(mut t: i64, mut work_s: i64) -> i64 {
    loop {
        let (y, m, d) = eastern_date(t);
        let day = days_from_civil(y, m, d);
        if is_business_day(day) {
            let open = eastern_to_unix(y, m, d, 9, 0);
            let close = eastern_to_unix(y, m, d, 17, 0);
            t = t.max(open);
            if t < close {
                if work_s <= close - t {
                    return t + work_s;
                }
                work_s -= close - t;
            }
        }
        let (y2, m2, d2) = civil_from_days(day + 1);
        t = eastern_to_unix(y2, m2, d2, 0, 0);
    }
}

fn percentiles(mut v: Vec<i64>) -> (f64, f64, f64) {
    v.sort_unstable();
    let h = |x: i64| x as f64 / 3600.0;
    (
        h(v[v.len() / 2]),
        h(v[v.len() * 95 / 100]),
        h(*v.last().unwrap()),
    )
}

#[test]
fn the_revocation_window_on_the_real_calendar() {
    const H: i64 = 3600;
    let start = eastern_to_unix(2026, 1, 1, 0, 0);
    let end = eastern_to_unix(2027, 1, 1, 0, 0);
    // Pipelines: (name, RA→MA shuffle flush, MA decision).
    type Ma = fn(i64) -> i64;
    let pipelines: [(&str, i64, Ma); 4] = [
        ("auto/flush-0", 0, |t| t),
        ("auto/flush-24h", 24 * H, |t| t),
        ("manual-8h-flat/flush-24h", 24 * H, |t| t + 8 * H),
        ("manual-8h-business/flush-24h", 24 * H, |t| {
            business_hours_done(t, 8 * H)
        }),
    ];
    let mut pinned = Vec::new();
    for (name, flush, ma) in pipelines {
        let mut publish = Vec::new();
        let mut worst = (0i64, 0i64);
        let mut t = start;
        while t < end {
            let decided = ma(t + flush);
            let published = next_publication_slot(decided, 10, 0);
            assert!(
                published <= publication_deadline(decided),
                "{name}: the 1-business-day rule is broken for a decision at {decided}"
            );
            let d = published - t;
            if d > worst.0 {
                worst = (d, t);
            }
            publish.push(d);
            t += 15 * 60;
        }
        let (p50, p95, max) = percentiles(publish);
        let (y, m, d) = eastern_date(worst.1);
        println!(
            "RT window {name:<30} detect->published p50={p50:.1}h p95={p95:.1}h max={max:.1}h (worst detection {y}-{m:02}-{d:02}); +weekly-fetch worst max={:.1}h",
            max + 168.0
        );
        pinned.push((p50.round() as i64, p95.round() as i64, max.round() as i64));
    }
    // Regression pins (whole hours), from the run that the document quotes.
    assert_eq!(
        pinned, PINNED_WINDOWS,
        "the calendar results moved: re-derive docs/redteam"
    );
}

const PINNED_WINDOWS: [(i64, i64, i64); 4] =
    [(18, 68, 96), (42, 92, 120), (50, 100, 128), (71, 121, 144)];
