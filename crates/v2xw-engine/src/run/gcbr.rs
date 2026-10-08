//! GeoNetworking-layer DCC: the global channel busy ratio, `net.dcc_cbr: global`.
//!
//! A station's own CBR measures only what it hears. Two stations a few hundred metres
//! apart can each see a moderate channel while the receiver between them is saturated by
//! both — the hidden-terminal congestion that access-layer DCC alone cannot see. ETSI TS
//! 102 636-4-2 V1.1.1 §5 (GeoNetworking media-dependent functionality for ITS-G5) closes
//! that loop at the network layer: every single-hop broadcast's DCC-MCO field carries the
//! sender's own CBR (`CBR_R_0_Hop`) and the highest CBR its neighbours reported
//! (`CBR_R_1_Hop`), and a station computes
//!
//! ```text
//! CBR_L_0_Hop = its own measurement
//! CBR_L_1_Hop = max over neighbours of their CBR_R_0_Hop
//! CBR_L_2_Hop = max over neighbours of their CBR_R_1_Hop
//! CBR_G       = max(CBR_L_0_Hop, CBR_L_1_Hop, CBR_L_2_Hop)
//! ```
//!
//! and hands `CBR_G` to its DCC algorithm instead of its local value, so a station
//! throttles for congestion up to two hops away. The clause is cited from memory of the
//! published TS; the averaging of the local CBR over two measurement periods that it
//! also describes is not applied (the MAC's CBR is already a 100 ms window).
//!
//! A neighbour's report is kept for [`REPORT_LIFETIME_NS`] (this build's choice: ten CAM
//! periods at the 10 Hz maximum, one at the 1 Hz minimum), so a station that drove away
//! stops counting.
//!
//! # Determinism
//!
//! Every table is a `BTreeMap` by [`NodeId`]; reports are written in the PHY's receiver
//! order and read as a max, which does not depend on order.

use std::collections::BTreeMap;

use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;

/// How long a neighbour's CBR report counts, ns.
pub const REPORT_LIFETIME_NS: u64 = 1_000_000_000;

/// What the global CBR did over the run, for [`super::RunReport::gcbr`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct GlobalCbrTally {
    /// DCC updates fed the global CBR.
    pub updates: u64,
    /// Of those, the ones where a neighbour's report raised it above the local value.
    pub raised: u64,
    /// The largest rise over the local value, thousandths of the channel.
    pub max_rise_permille: u64,
}

/// The run's network-layer CBR tables.
#[derive(Debug, Clone, Default)]
pub(super) struct GlobalCbr {
    /// Each station's latest `(CBR_L_0_Hop, CBR_L_1_Hop)`: what its next SHB carries.
    own: BTreeMap<NodeId, (f64, f64)>,
    /// What each station heard: receiver → sender → `(CBR_R_0_Hop, CBR_R_1_Hop, when)`.
    heard: BTreeMap<NodeId, BTreeMap<NodeId, (f64, f64, SimTime)>>,
    pub(super) tally: GlobalCbrTally,
}

impl GlobalCbr {
    /// The layer, when the scenario asks for it on GeoNetworking.
    pub(super) fn for_scenario(s: &crate::scenario::schema::Scenario) -> Option<Self> {
        (s.net.layer == "gn-btp" && s.net.dcc_cbr.as_deref() == Some("global"))
            .then(GlobalCbr::default)
    }

    /// `rx` decoded a frame from `tx` at `now`: the frame's DCC-MCO field is `tx`'s
    /// latest own values.
    pub(super) fn on_receive(&mut self, rx: NodeId, tx: NodeId, now: SimTime) {
        if let Some(&(r0, r1)) = self.own.get(&tx) {
            self.heard.entry(rx).or_default().insert(tx, (r0, r1, now));
        }
    }

    /// `node` measured `local` at `now`: the CBR its DCC algorithm should act on.
    pub(super) fn global(&mut self, node: NodeId, local: f64, now: SimTime) -> f64 {
        let mut l1 = 0.0_f64;
        let mut l2 = 0.0_f64;
        if let Some(h) = self.heard.get_mut(&node) {
            h.retain(|_, (_, _, t)| now.saturating_sub(*t) <= REPORT_LIFETIME_NS);
            for (r0, r1, _) in h.values() {
                l1 = l1.max(*r0);
                l2 = l2.max(*r1);
            }
        }
        self.own.insert(node, (local, l1));
        let g = local.max(l1).max(l2);
        self.tally.updates += 1;
        if g > local {
            self.tally.raised += 1;
            let rise = ((g - local) * 1_000.0).round() as u64;
            self.tally.max_rise_permille = self.tally.max_rise_permille.max(rise);
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A congested station two hops away raises a quiet station's CBR: A hears B, B hears
    /// C, C is saturated; A's global CBR reaches C's within two report rounds, and falls
    /// back once the reports age out.
    #[test]
    fn congestion_two_hops_away_reaches_the_dcc_input() {
        let (a, b, c) = (NodeId::new(1), NodeId::new(2), NodeId::new(3));
        let mut g = GlobalCbr::default();
        let ms = 1_000_000;
        // Round 1: everyone measures; C is busy.
        assert_eq!(g.global(c, 0.7, 0), 0.7);
        assert_eq!(g.global(b, 0.2, 0), 0.2);
        assert_eq!(g.global(a, 0.1, 0), 0.1);
        // B hears C; A hears B.
        g.on_receive(b, c, 10 * ms);
        g.on_receive(a, b, 10 * ms);
        assert!((g.global(b, 0.2, 100 * ms) - 0.7).abs() < 1e-12, "one hop");
        // A's last report from B predates B learning of C: B's own value only.
        assert_eq!(g.global(a, 0.1, 100 * ms), 0.2);
        g.on_receive(a, b, 110 * ms);
        assert!((g.global(a, 0.1, 200 * ms) - 0.7).abs() < 1e-12, "two hops");
        assert!(g.tally.raised >= 2 && g.tally.max_rise_permille == 600);
        // Reports age out.
        assert_eq!(g.global(a, 0.1, 200 * ms + REPORT_LIFETIME_NS), 0.1);
    }
}
