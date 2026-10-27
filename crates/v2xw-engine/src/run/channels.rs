//! The run's own channel map, second radios and own-channel leakage (WP-13).
//!
//! `radio.channels` moves message sets off the safety channel (`radio.channel`) onto a
//! service channel of the region's band plan, as a multi-channel deployment does: SAE
//! J2945/1 keeps the BSM on channel 172 and IEEE 1609.4-2016 §6.2.5 lets the other
//! services run on a service channel; in Europe ETSI EN 302 663 and TS 103 141 assign the
//! control and service channels. A unit sends and hears on the service channel only when
//! it carries a second radio (`radio.devices.*.radios: 2`); with one radio it stays on the
//! safety channel (alternating access inside the MAC is WP-16). The two channels leak into
//! each other through the transmitter mask and the receiver's selectivity, the ACIR of
//! [`v2xw_radio::regulation::adjacent_acir_db`].
//!
//! Re-implemented from IEEE 1609.4-2016, SAE J2945/1 and 3GPP TR 36.942 §5.1.1.3; Artery's
//! `MultiChannelPolicy` and Veins were read for behaviour only and no code was copied.
//!
//! An empty map builds no plan, and every engine path that consults one is then the
//! single-channel path it always was.

use std::collections::BTreeMap;

use v2xw_msg::MsgType;
use v2xw_radio::{ActorClass, ChannelId};

use crate::scenario::Scenario;

/// The safety broadcasts that stay on `radio.channel`: moving them is changing that.
const SAFETY_BROADCASTS: [MsgType; 4] = [MsgType::Bsm, MsgType::Cam, MsgType::Psm, MsgType::Vam];

/// The resolved channel map of one run.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChannelPlan {
    /// The safety channel, every unit's first radio.
    pub(crate) primary: ChannelId,
    /// The service channel the map names, a second radio's.
    pub(crate) service: Option<ChannelId>,
    by_msg: BTreeMap<MsgType, ChannelId>,
    radios_obu: u8,
    radios_rsu: u8,
    radios_vru: u8,
    /// `(aggressor, victim)` → ACIR dB; empty when leakage is off or there is one channel.
    pub(crate) leakage: BTreeMap<(u16, u16), f64>,
}

impl ChannelPlan {
    /// The channel a message goes out on.
    pub(crate) fn channel_for(&self, m: MsgType) -> ChannelId {
        self.by_msg.get(&m).copied().unwrap_or(self.primary)
    }

    /// How many radios a unit of `class` carries.
    pub(crate) fn radios(&self, class: ActorClass) -> u8 {
        match class {
            ActorClass::Rsu | ActorClass::BaseStation => self.radios_rsu,
            ActorClass::Pedestrian | ActorClass::Bicycle => self.radios_vru,
            _ => self.radios_obu,
        }
    }

    /// Whether a unit of `class` has a radio on `ch`.
    pub(crate) fn tuned(&self, class: ActorClass, ch: ChannelId) -> bool {
        ch == self.primary || (self.service == Some(ch) && self.radios(class) >= 2)
    }

    /// The channels a unit of `class` listens on, the safety channel first.
    pub(crate) fn channels_of(&self, class: ActorClass) -> Vec<ChannelId> {
        let mut v = vec![self.primary];
        if let Some(s) = self.service
            && self.radios(class) >= 2
        {
            v.push(s);
        }
        v
    }
}

/// The run's channel plan, `None` for a single-channel run (an empty `radio.channels`),
/// or every reason the map is refused, as `(path, why)`.
pub(crate) fn plan(s: &Scenario) -> Result<Option<ChannelPlan>, Vec<(String, String)>> {
    if s.radio.channels.is_empty() {
        return Ok(None);
    }
    let mut errors: Vec<(String, String)> = Vec::new();
    let Ok(reg) = crate::wiring::radio_regulation(s) else {
        // The regulation's own refusal is reported where it is checked.
        return Ok(None);
    };
    if reg.technology != v2xw_radio::regulation::Technology::Ieee80211p {
        return Err(vec![(
            "radio.channels".to_string(),
            "maps messages onto 802.11p channels; a C-V2X sidelink run has one resource \
             pool, and multi-channel sidelink is not modelled"
                .to_string(),
        )]);
    }
    let region = reg.region;
    let tech = reg.technology;
    let primary = reg.rule.channel;
    let mut by_msg: BTreeMap<MsgType, ChannelId> = BTreeMap::new();
    let mut service: Option<u16> = None;
    for (i, entry) in s.radio.channels.iter().enumerate() {
        let path = format!("radio.channels[{i}]");
        let Some(rule) = region.rule(tech, entry.channel) else {
            errors.push((
                format!("{path}.channel"),
                format!(
                    "is {}; {} opens 802.11p on {:?}",
                    entry.channel,
                    region.id(),
                    region.channels_for(tech)
                ),
            ));
            continue;
        };
        if rule.channel.number != primary.number {
            if rule.channel.overlaps(&primary) {
                errors.push((
                    format!("{path}.channel"),
                    format!(
                        "{} overlaps the safety channel {}",
                        entry.channel, primary.number
                    ),
                ));
                continue;
            }
            match service {
                Some(other) if other != entry.channel => errors.push((
                    format!("{path}.channel"),
                    format!(
                        "names a second service channel ({} beside {other}); one service \
                         channel, the second radio's, is modelled — more needs IEEE \
                         1609.4 channel switching in the MAC",
                        entry.channel
                    ),
                )),
                _ => service = Some(entry.channel),
            }
        }
        if entry.messages.is_empty() {
            errors.push((
                format!("{path}.messages"),
                "names no message set".to_string(),
            ));
        }
        for name in &entry.messages {
            let Ok(m) = serde_json::from_value::<MsgType>(serde_json::json!(name)) else {
                errors.push((
                    format!("{path}.messages"),
                    format!(
                        "names '{name}', which is not a message set (one of {:?})",
                        MsgType::ALL.map(MsgType::as_str)
                    ),
                ));
                continue;
            };
            if SAFETY_BROADCASTS.contains(&m) && entry.channel != primary.number {
                errors.push((
                    format!("{path}.messages"),
                    format!(
                        "moves '{name}' off the safety channel; set radio.channel to move \
                         the safety broadcast"
                    ),
                ));
                continue;
            }
            if by_msg.insert(m, ChannelId(entry.channel)).is_some() {
                errors.push((
                    format!("{path}.messages"),
                    format!("maps '{name}' a second time"),
                ));
            }
        }
    }
    let d = &s.radio.devices;
    let radios = [
        ("radio.devices.obu.radios", d.obu.radios.unwrap_or(1)),
        ("radio.devices.rsu.radios", d.rsu.radios.unwrap_or(2)),
        ("radio.devices.vru.radios", d.vru.radios.unwrap_or(1)),
    ];
    for (path, n) in radios {
        if !(1..=2).contains(&n) {
            errors.push((
                path.to_string(),
                format!("is {n}; a unit carries one radio or two"),
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let primary_id = ChannelId(primary.number);
    let service_id = service.map(ChannelId);
    let leakage = match service {
        Some(sc) if s.radio.channel_leakage != Some(false) => {
            let mcs = crate::wiring::safety_mcs(s);
            let index = v2xw_radio::Mcs::ALL
                .iter()
                .position(|m| *m == mcs)
                .unwrap_or(2) as u8;
            v2xw_radio::regulation::own_channel_leakage(
                region,
                tech,
                &[primary.number, sc],
                index,
            )
        }
        _ => BTreeMap::new(),
    };
    Ok(Some(ChannelPlan {
        primary: primary_id,
        service: service_id,
        by_msg,
        radios_obu: radios[0].1,
        radios_rsu: radios[1].1,
        radios_vru: radios[2].1,
        leakage,
    }))
}
