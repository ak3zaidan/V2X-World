//! The umbrella threshold authority: a t-of-n committee that issues *umbrella* credentials,
//! each covering `k` leaf pseudonyms the device derives itself.
//!
//! # What "umbrella" means here — an assumption, stated
//!
//! The design brief (§8.3) leaves the owner's umbrella-thresholding specification as
//! `[FILL IN]`, and 05-protocols.md §5.4 gives only a placeholder binding: "issuing
//! committee `n` of which `t+1` sign … umbrella credential (long-lived, PQ-signed, covers
//! `k` leaf pseudonyms), leaf pseudonym (short-lived, derived) … umbrella issuance (t-of-n
//! signing over WAN), leaf derivation (local or 1 round), revocation of an umbrella (list
//! entry …)". This module implements exactly that binding and nothing more:
//!
//! * the committee signs **one umbrella credential per `k` leaves**, with whichever
//!   registered protocol is named as the inner one (FROST by default), so a batch of `B`
//!   pseudonyms costs `⌈B / k⌉` threshold signatures instead of `B`;
//! * the device derives each leaf **locally** and certifies it with its umbrella key — one
//!   signature per leaf on the device, no round trip;
//! * a verifier that has not cached the umbrella checks two signatures (leaf, umbrella);
//! * revocation is one entry per umbrella, which revokes its `k` leaves together.
//!
//! The privacy cost is the obvious one and is reported, not hidden: the `k` leaves under
//! one umbrella are linkable to anyone who sees the umbrella travel with them, so `k`
//! trades committee load against unlinkability. When the owner's specification arrives it
//! replaces this module; nothing else changes.

use super::{
    Committee, Knowledge, ProtocolEntry, ProtocolParams, RoundPlan, StarRound, ThresholdProtocol,
};
use crate::sizes::WireSize;

/// The registry id.
pub const ID: &str = "umbrella";

/// The registry entry.
pub const ENTRY: ProtocolEntry = ProtocolEntry {
    id: ID,
    label: "Umbrella authority: t-of-n umbrella credentials over k derived leaves",
    build: build_umbrella,
};

fn build_umbrella(p: &ProtocolParams) -> Box<dyn ThresholdProtocol> {
    // The inner protocol is any registered one except the umbrella itself.
    let inner_id = if p.umbrella_inner == ID {
        super::frost::ID
    } else {
        p.umbrella_inner
    };
    let inner =
        super::build(inner_id, p).unwrap_or_else(|| Box::new(super::frost::Frost::default()));
    Box::new(Umbrella {
        inner,
        leaves: p.umbrella_leaves.max(1),
    })
}

/// The umbrella authority over an inner threshold protocol.
#[derive(Debug)]
pub struct Umbrella {
    /// The protocol the committee signs each umbrella with.
    pub inner: Box<dyn ThresholdProtocol>,
    /// Leaf pseudonyms per umbrella, `k`.
    pub leaves: u32,
}

impl Umbrella {
    /// How many umbrellas cover `batch` leaves.
    pub const fn umbrellas_for(&self, batch: u32) -> u32 {
        batch.div_ceil(self.leaves)
    }
}

impl ThresholdProtocol for Umbrella {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        ENTRY.label
    }

    fn standard(&self) -> &'static str {
        "05-protocols.md §5.4 placeholder binding (owner's specification pending, brief §8.3)"
    }

    fn sign_rounds(&self, signers: u16, batch: u32) -> Vec<StarRound> {
        self.inner.sign_rounds(signers, self.umbrellas_for(batch))
    }

    fn dkg_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        self.inner.dkg_rounds(committee)
    }

    fn refresh_rounds(&self, committee: &Committee) -> Vec<RoundPlan> {
        self.inner.refresh_rounds(committee)
    }

    fn signature_bytes(&self) -> WireSize {
        self.inner.signature_bytes()
    }

    fn verification_key_bytes(&self) -> WireSize {
        self.inner.verification_key_bytes()
    }

    fn verifies_as(&self) -> &'static str {
        "a leaf signature by the device plus the umbrella credential's committee signature"
    }

    fn identifiable_abort(&self) -> bool {
        self.inner.identifiable_abort()
    }

    fn knowledge(&self) -> Vec<Knowledge> {
        let mut k = vec![
            Knowledge {
                party: "committee",
                learns: "one umbrella per k leaves; it never sees the leaves, so it cannot \
                         link them to each other or to the device beyond the umbrella",
            },
            Knowledge {
                party: "observer on the air",
                learns: "the k leaves under one umbrella are linkable whenever the umbrella \
                         (or its digest) travels with them — the price of 1/k of the \
                         committee load",
            },
            Knowledge {
                party: "revocation",
                learns: "one list entry revokes an umbrella and all k of its leaves",
            },
        ];
        k.extend(self.inner.knowledge());
        k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_umbrella_signs_one_inner_signature_per_k_leaves() {
        let p = ProtocolParams {
            umbrella_leaves: 20,
            ..ProtocolParams::default()
        };
        let u = build_umbrella(&p);
        let f = super::super::frost::Frost::default();
        // 40 leaves = 2 umbrellas.
        assert_eq!(u.sign_bytes(3, 40, 93), f.sign_bytes(3, 2, 93));
        assert_eq!(u.sign_bytes(3, 41, 93), f.sign_bytes(3, 3, 93));
    }
}
