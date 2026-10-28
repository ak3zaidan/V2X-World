//! Evasion of the Misbehaviour Authority's device-class features (paper plan 2026-10
//! §1.3, experiment P1-E8; §1.4 item F).
//!
//! # Why this attack exists
//!
//! On public CV-pilot BSMs honest devices whose `secMark` is not locked to the GNSS fix
//! epoch ("free") trip SM-MBR RandomPosition 20–195 times per ID-minute at speed, and
//! locked ones about 0 (research/synthesis/p1-realdata-baseline.md T2/T7). An MA that
//! wants to avoid revoking that benign population can read the device class from fields
//! it already receives: the secMark phase of the evidence BSMs, the accused's stated
//! `PositionalAccuracy`, and the issuing CA (`v2xw ma-replay`'s `cluster` and `catch`
//! rules). Every one of those fields except the issuer is written by the accused itself.
//! This attacker writes them to look benign:
//!
//! * **secMark phase.** It adds a constant shift to its BSM `secMark`, so a locked device's
//!   phase moves off the 50 ms epoch into the free class. A constant shift leaves every
//!   consecutive-BSM `Δt` (secMark or generationTime time base) as it was, so the disguise
//!   itself costs no RandomPosition trigger. The signature still verifies: the attacker
//!   signs what it sends.
//! * **Poor accuracy.** It states a larger 95 % position radius (J2735
//!   `PositionalAccuracy`) than its receiver's own. SM-MBR's checks ignore the stated
//!   accuracy ([SM-MBR] §2.3: fixed `gpsDrift`), so this moves only what an
//!   accuracy-aware MA rule reads.
//!
//! The issuer is the one class feature the attacker cannot choose: it is the ACA that
//! signed its pseudonym certificate.
//!
//! # Shape
//!
//! The payload (what is falsified) is a ported legacy rendering, unchanged: the attack id
//! `threat/attacker/evasion/BenignClassMimic` takes the legacy kind as its `payload`
//! parameter, so a run with and a run without the disguise differ in the disguise only.
//! The legacy catalogue ([`crate::attack::AttackKind::LEGACY_CATALOG`]) is untouched.
//! With neither disguise field set the attacker is its payload, message for message
//! (`no_disguise_is_the_payload_exactly`).

use crate::attack::{AttackAction, AttackFamily, Attacker, AttackerView, Emission};
use crate::attack_legacy::LegacyAttacker;
use crate::capability::{AttackSchedule, Capabilities};
use crate::cards::{design, paper, ranged, standard};
use crate::ctx::ThreatCtx;
use v2xw_core::card::{Family, ModelCard, Parameter, Tier};
use v2xw_core::model::Model;

/// The model id this module's card carries.
pub const MODEL_ID: &str = "threat/attacker/evasion";

/// The scenario id prefix of this module's attacks.
pub const ID_PREFIX: &str = "threat/attacker/evasion/";

/// One evasion attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvasionKind {
    /// A legacy payload behind the benign device class's secMark phase and/or a poor
    /// stated accuracy.
    BenignClassMimic,
}

impl EvasionKind {
    /// Every evasion attack.
    pub const ALL: [EvasionKind; 1] = [EvasionKind::BenignClassMimic];

    /// The name a scenario carries after [`ID_PREFIX`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            EvasionKind::BenignClassMimic => "BenignClassMimic",
        }
    }

    /// Parses a name. `None` for anything not in [`EvasionKind::ALL`].
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        EvasionKind::ALL.into_iter().find(|k| k.as_str() == name)
    }

    /// The family: it falsifies the sender's apparent device class.
    #[must_use]
    pub const fn family(self) -> AttackFamily {
        match self {
            EvasionKind::BenignClassMimic => AttackFamily::Identity,
        }
    }
}

impl core::fmt::Display for EvasionKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the disguise writes. Both `None` is no disguise.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Disguise {
    /// The constant `secMark` shift, ms (1..=99).
    pub secmark_shift_ms: Option<u16>,
    /// The stated 95 % position radius, m (> 0).
    pub stated_accuracy_m: Option<f64>,
}

/// `BenignClassMimic`: a legacy payload under a [`Disguise`].
pub struct BenignClassMimic {
    inner: LegacyAttacker,
    disguise: Disguise,
    card: ModelCard,
}

impl BenignClassMimic {
    /// `inner` is the payload, `disguise` what it writes on top.
    #[must_use]
    pub fn new(inner: LegacyAttacker, disguise: Disguise) -> Self {
        Self {
            card: card(&disguise),
            inner,
            disguise,
        }
    }

    /// The disguise.
    #[must_use]
    pub fn disguise(&self) -> Disguise {
        self.disguise
    }
}

impl Model for BenignClassMimic {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Attacker for BenignClassMimic {
    fn capabilities(&self) -> &Capabilities {
        self.inner.capabilities()
    }

    fn schedule(&self) -> &AttackSchedule {
        self.inner.schedule()
    }

    fn observe(&mut self, ctx: &mut dyn ThreatCtx, view: &AttackerView<'_>) {
        self.inner.observe(ctx, view);
    }

    fn act(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        view: &AttackerView<'_>,
        out: &mut Emission,
    ) -> Vec<AttackAction> {
        let mut actions = self.inner.act(ctx, view, out);
        // The disguise is worn whenever the payload's schedule is on: a device whose class
        // changed only while it lied would be its own tell.
        let on = self.inner.schedule().active_at(
            view.believed_time,
            view.own_belief.x_m,
            view.own_belief.y_m,
        );
        if !on {
            return actions;
        }
        if let Some(shift) = self.disguise.secmark_shift_ms {
            out.sec_mark_shift_ms = Some(shift);
            actions.push(AttackAction::FalsifyOutgoing {
                fields: vec!["secMark".to_string()],
                magnitude: f64::from(shift),
            });
        }
        if let Some(a) = self.disguise.stated_accuracy_m {
            out.stated_accuracy_m = Some(a);
            actions.push(AttackAction::FalsifyOutgoing {
                fields: vec!["positionalAccuracy".to_string()],
                magnitude: a,
            });
        }
        actions
    }
}

/// The card.
#[must_use]
pub fn card(d: &Disguise) -> ModelCard {
    use serde_json::json;
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Attacker,
        "1.0.0",
        "A legacy payload that hides inside the benign device class the Misbehaviour \
         Authority can read from the evidence it receives: a constant secMark shift off the \
         GNSS fix epoch (the CV-pilot free class) and/or a poor stated PositionalAccuracy."
            .to_string(),
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    let mut shift = ranged(
        "secmark_shift_ms",
        "ms",
        json!(d.secmark_shift_ms.unwrap_or(0)),
        json!(0),
        json!(99),
        paper(
            "research/synthesis/p1-realdata-baseline.md T7: locked > 80 % of BSMs within 1 ms \
             of a 50 ms epoch, free < 30 %",
        ),
    );
    shift.calibration = Some(
        "0 is no shift; any shift whose residue modulo 50 ms is outside ±1 ms reads as free; \
         25 ms is the farthest from both epochs"
            .to_string(),
    );
    let acc: Parameter = Parameter::new(
        "stated_accuracy_m",
        "m",
        json!(d.stated_accuracy_m.unwrap_or(0.0)),
        standard(
            "SAE J2735 PositionalAccuracy (semiMajor, 0.05 m LSB, 95 % radius here); 0 is the \
             sender's own estimate",
        ),
    );
    card.parameters = vec![shift, acc];
    card.sources = vec![
        design("../../research/synthesis/paper-plan-2026-10.md §1.3 P1-E8"),
        standard("SCMS Manager CV-Pilot MBR v1.0 §2.3 (gpsDrift ignores stated accuracy)"),
    ];
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attack::{AttackKind, HonestClaim};
    use crate::attack_legacy::LegacyAttackerParams;
    use crate::ctx::CollectingCtx;
    use crate::obs::SelfBelief;
    use v2xw_core::ids::NodeId;
    use v2xw_core::time::NS_PER_S;

    fn legacy(kind: AttackKind) -> LegacyAttacker {
        LegacyAttacker::new(
            NodeId::new(7),
            LegacyAttackerParams::new(kind),
            Capabilities::insider(1),
            AttackSchedule {
                from: 10 * NS_PER_S,
                to: 100 * NS_PER_S,
                ..AttackSchedule::default()
            },
            Vec::new(),
        )
    }

    fn step(a: &mut dyn Attacker, t: SimTime, x: f64) -> (Emission, Vec<AttackAction>) {
        let honest = HonestClaim {
            x_m: x,
            y_m: 0.0,
            speed_mps: 15.0,
            heading_rad: 0.0,
        };
        let view = AttackerView {
            own_rx: &[],
            own_credentials: &[],
            crl_revocations_seen: None,
            own_belief: SelfBelief {
                node: NodeId::new(7),
                believed_time: t,
                x_m: x,
                y_m: 0.0,
                radio_range_m: 300.0,
            },
            honest,
            believed_time: t,
        };
        let mut out = Emission::honest([1; 8], honest, t, 0, u64::MAX);
        let mut ctx = CollectingCtx::new(42);
        let actions = a.act(&mut ctx, &view, &mut out);
        (out, actions)
    }

    use v2xw_core::time::SimTime;

    #[test]
    fn no_disguise_is_the_payload_exactly() {
        let mut plain = legacy(AttackKind::ConstPos);
        let mut mimic = BenignClassMimic::new(legacy(AttackKind::ConstPos), Disguise::default());
        for i in 0..300u64 {
            let t = i * NS_PER_S / 10;
            let x = 1.5 * i as f64;
            assert_eq!(
                step(&mut plain, t, x),
                step(&mut mimic, t, x),
                "message {i}"
            );
        }
    }

    #[test]
    fn the_disguise_is_written_only_inside_the_window_and_logged() {
        let d = Disguise {
            secmark_shift_ms: Some(25),
            stated_accuracy_m: Some(9.0),
        };
        let mut plain = legacy(AttackKind::ConstPos);
        let mut mimic = BenignClassMimic::new(legacy(AttackKind::ConstPos), d);
        let (before, a0) = step(&mut mimic, 5 * NS_PER_S, 75.0);
        let _ = step(&mut plain, 5 * NS_PER_S, 75.0);
        assert_eq!(
            (before.sec_mark_shift_ms, before.stated_accuracy_m),
            (None, None)
        );
        assert!(a0.is_empty());
        let (p, pa) = step(&mut plain, 20 * NS_PER_S, 300.0);
        let (m, ma) = step(&mut mimic, 20 * NS_PER_S, 300.0);
        assert_eq!(m.sec_mark_shift_ms, Some(25));
        assert_eq!(m.stated_accuracy_m, Some(9.0));
        // The payload's claim is the payload's.
        assert_eq!((m.x_m, m.y_m, m.speed_mps), (p.x_m, p.y_m, p.speed_mps));
        let fields: Vec<&str> = ma
            .iter()
            .skip(pa.len())
            .filter_map(|a| match a {
                AttackAction::FalsifyOutgoing { fields, .. } => Some(fields[0].as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(fields, ["secMark", "positionalAccuracy"]);
    }

    #[test]
    fn the_names_parse_back_and_the_card_validates() {
        for k in EvasionKind::ALL {
            assert_eq!(EvasionKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(EvasionKind::parse("ConstPos"), None);
        card(&Disguise::default())
            .validate()
            .expect("card validates");
    }
}
