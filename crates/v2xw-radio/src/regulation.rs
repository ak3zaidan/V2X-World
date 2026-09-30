//! Spectrum regulation per region: which channels each radio technology may use, the
//! radiated-power limits of on-board and roadside units there, the transmitter spectrum
//! masks, and the adjacent-channel leakage and selectivity that decide how much one
//! technology interferes with its neighbour.
//!
//! # The three regions and what they rest on
//!
//! | Region | Band plan | Primary text read |
//! |---|---|---|
//! | [`Region::Us`] | 5.895-5.925 GHz for C-V2X; DSRC's licences end | FCC 24-123 (Second Report and Order, 2024) Appendix A: 47 CFR §90.386-§90.392 (RSU) and §95.3201-§95.3205 (OBU); ¶82 on DSRC |
//! | [`Region::Us2016`] | 5.850-5.925 GHz DSRC, BSM on channel 172 | 47 CFR §90.377 and §95.1511 as in force on 3 January 2017 (eCFR point-in-time); SAE J2945/1's channel 172 through the FMVSS 150 NPRM (82 FR 3854) S5.3.2 |
//! | [`Region::Eu`] | 5.855-5.925 GHz, 10 MHz channels, technology neutral | ETSI EN 302 571 V2.1.1 (2017) §4.2; ETSI EN 303 613 V1.1.1 (2020) Annex B for LTE-V2X; the 5GAA deployment band configurations of 2021 and 2024 for which technology sits where |
//!
//! The United States moved ITS in two steps. The FCC's First Report and Order (FCC 20-164,
//! 2020) gave the lower 45 MHz (5.850-5.895 GHz) to unlicensed use and kept the upper
//! 30 MHz for ITS; the Second Report and Order (FCC 24-123, 2024) wrote the C-V2X rules
//! and ordered that no new DSRC licence issue and that existing ones cancel two years
//! after the rules' publication — the Federal Register published them on 13 December
//! 2024 (Federal Register document 2024-28980), so DSRC ends on 13 December 2026. Channel
//! 172, where SAE J2945/1 put the BSM, is now U-NII-4 Wi-Fi spectrum. A DSRC study of the
//! deployment era runs [`Region::Us2016`]; [`Region::Us`] carries C-V2X only.
//!
//! In Europe the band is technology neutral and industry, not the regulator, decides who
//! sits where: ITS-G5 at 5.895-5.905 GHz today (5GAA 2024: "current deployments of ITS-G5
//! OBUs operate in the 5895-5905 MHz block"), LTE-V2X proposed at 5.905-5.915 GHz (5GAA
//! 2021), and NR-V2X in 5.875-5.895 GHz (5GAA 2024). EN 302 571 allows only 10 MHz
//! channels, so a European LTE-V2X or NR-V2X carrier is 10 MHz wide. The two
//! technologies then occupy *adjacent* 10 MHz channels, which is exactly the case
//! [`acir_db`] prices.
//!
//! # Power
//!
//! Every limit here is an EIRP — conducted power plus antenna gain less cable loss —
//! because that is what every one of these rules limits. [`max_eirp_dbm`] folds in the
//! two geometric rules: a US roadside unit above 8 m loses `20·log10(h/8)` dB and may not
//! exceed 15 m (§90.391(b)); a US C-V2X on-board unit without a geofence may radiate
//! 33 dBm but only 27 dBm within ±5° of the horizon on the 5.905-5.925 GHz channels
//! (§95.3204(a)) — and a vehicle's link to another vehicle *is* within ±5° of the horizon,
//! so 27 dBm is the operative figure.

use serde::{Deserialize, Serialize};

use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::math;

/// A regulatory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Region {
    /// The United States under FCC 24-123: C-V2X in 5.895-5.925 GHz.
    Us,
    /// The United States' DSRC band plan before FCC 20-164: 5.850-5.925 GHz, seven
    /// 10 MHz channels, the BSM on channel 172.
    #[serde(rename = "us-2016")]
    Us2016,
    /// The European Union and CEPT under ETSI EN 302 571: 5.855-5.925 GHz, 10 MHz
    /// channels, technology neutral.
    Eu,
}

impl Region {
    /// Every region, in a fixed order.
    pub const ALL: [Region; 3] = [Region::Us, Region::Us2016, Region::Eu];

    /// The scenario's name for it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Region::Us => "us",
            Region::Us2016 => "us-2016",
            Region::Eu => "eu",
        }
    }

    /// The region a scenario names.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.id() == id)
    }

    /// Every channel the region's rules name, with its technologies and limits.
    #[must_use]
    pub const fn rules(self) -> &'static [ChannelRule] {
        match self {
            Region::Us => US_RULES,
            Region::Us2016 => US_2016_RULES,
            Region::Eu => EU_RULES,
        }
    }

    /// The channel a technology deploys on in this region, or why it may not.
    ///
    /// # Errors
    /// The reason, in words a scenario author can act on, when the region has no channel
    /// for the technology.
    pub fn default_channel(self, tech: Technology) -> Result<&'static ChannelRule, &'static str> {
        let number = match (self, tech) {
            // SAE J3161/1's LTE-V2X profile: the 20 MHz channel 5.905-5.925 GHz.
            (Region::Us, Technology::LteV2x) => 183,
            // No US NR-V2X profile exists; the same 20 MHz C-V2X channel.
            (Region::Us, Technology::NrV2x) => 183,
            (Region::Us, Technology::Ieee80211p) => {
                return Err("the United States ended DSRC: FCC 20-164 (2020) gave \
                            5.850-5.895 GHz, channel 172 included, to unlicensed use, \
                            and FCC 24-123 (2024) issues no new DSRC licence and cancels \
                            the existing ones on 13 December 2026. Run 802.11p under \
                            radio.region: us-2016 (the deployment-era band plan) or eu \
                            (ITS-G5), or C-V2X under us");
            }
            // SAE J2945/1: the BSM on channel 172 (FMVSS 150 NPRM S5.3.2).
            (Region::Us2016, Technology::Ieee80211p) => 172,
            (Region::Us2016, _) => {
                return Err("the 2016 US band plan is DSRC's (47 CFR §90.377); C-V2X \
                            has no channel in it. Run LTE-V2X or NR-V2X under \
                            radio.region: us or eu");
            }
            // ITS-G5's control channel, where current deployments operate (5GAA 2024).
            (Region::Eu, Technology::Ieee80211p) => 180,
            // 5.905-5.915 GHz, the LTE-V2X day-1 channel 5GAA proposed (5GAA 2021).
            (Region::Eu, Technology::LteV2x) => 182,
            // 5.885-5.895 GHz: 5GAA 2024 puts 5G-V2X at 5.875-5.895 GHz, which the
            // current 10 MHz channelisation splits into two; the upper one.
            (Region::Eu, Technology::NrV2x) => 178,
        };
        Ok(self
            .rule(tech, number)
            .expect("every default channel is in its region's table"))
    }

    /// The rule for a technology on a channel number, when the region allows it there.
    #[must_use]
    pub fn rule(self, tech: Technology, number: u16) -> Option<&'static ChannelRule> {
        self.rules()
            .iter()
            .find(|r| r.channel.number == number && r.technologies.contains(&tech))
    }

    /// The channel numbers a technology may use in this region.
    #[must_use]
    pub fn channels_for(self, tech: Technology) -> Vec<u16> {
        self.rules()
            .iter()
            .filter(|r| r.technologies.contains(&tech))
            .map(|r| r.channel.number)
            .collect()
    }

    /// The transmitter spectrum mask this region imposes on a technology.
    #[must_use]
    pub const fn mask(self, tech: Technology) -> &'static SpectrumMask {
        match (self, tech) {
            (_, Technology::Ieee80211p) => &EN302571_10MHZ_MASK,
            (Region::Us, _) => &FCC_CV2X_OOBE,
            _ => &TS36101_ACLR_MASK,
        }
    }
}

/// A radio technology as the regulation sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Technology {
    /// IEEE 802.11p: DSRC in the US, ITS-G5 in Europe.
    Ieee80211p,
    /// LTE-V2X PC5.
    LteV2x,
    /// NR-V2X PC5.
    NrV2x,
}

/// What kind of station transmits: the rules limit them differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Station {
    /// A vehicle's on-board unit.
    Obu,
    /// A roadside unit, with its antenna's height above the roadway.
    Rsu,
    /// A portable unit carried by a person (a pedestrian's or cyclist's device).
    Portable,
}

/// One channel: its number and its edges.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ChannelSpec {
    /// The IEEE 802.11 channel number of the centre frequency, `(f_c − 5000 MHz)/5`,
    /// which every band plan here uses to name its channels.
    pub number: u16,
    /// Lower edge, MHz.
    pub lower_mhz: f64,
    /// Upper edge, MHz.
    pub upper_mhz: f64,
}

impl ChannelSpec {
    /// The centre frequency, Hz.
    #[must_use]
    pub fn centre_hz(&self) -> f64 {
        (self.lower_mhz + self.upper_mhz) * 0.5e6
    }

    /// The bandwidth, MHz.
    #[must_use]
    pub fn bandwidth_mhz(&self) -> f64 {
        self.upper_mhz - self.lower_mhz
    }

    /// Whether two channels overlap in frequency.
    #[must_use]
    pub fn overlaps(&self, other: &ChannelSpec) -> bool {
        self.lower_mhz < other.upper_mhz && other.lower_mhz < self.upper_mhz
    }

    /// The gap between the channels' facing edges, MHz; zero when adjacent, negative
    /// when they overlap.
    #[must_use]
    pub fn edge_gap_mhz(&self, other: &ChannelSpec) -> f64 {
        if self.upper_mhz <= other.lower_mhz {
            other.lower_mhz - self.upper_mhz
        } else if other.upper_mhz <= self.lower_mhz {
            self.lower_mhz - other.upper_mhz
        } else {
            -(self.upper_mhz.min(other.upper_mhz) - self.lower_mhz.max(other.lower_mhz))
        }
    }
}

/// What a region allows on one channel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ChannelRule {
    /// The channel.
    pub channel: ChannelSpec,
    /// The technologies the rule lets transmit there.
    pub technologies: &'static [Technology],
    /// A roadside unit's EIRP limit, dBm, at an antenna no higher than the region's
    /// reference height.
    pub rsu_eirp_dbm: f64,
    /// An on-board unit's EIRP limit toward the horizon, dBm.
    pub obu_eirp_dbm: f64,
    /// An on-board unit's EIRP limit away from the horizon, dBm, where it differs (the
    /// US C-V2X 33 dBm with 27 dBm within ±5° of horizontal).
    pub obu_eirp_off_horizon_dbm: f64,
    /// A portable unit's limit, dBm, where the rule sets one.
    pub portable_dbm: Option<f64>,
    /// The mean EIRP spectral density limit, dBm/MHz, where the rule sets one.
    pub psd_dbm_per_mhz: Option<f64>,
    /// What the channel is for.
    pub usage: &'static str,
    /// The clause.
    pub clause: &'static str,
}

const fn ch(number: u16, lower_mhz: f64, upper_mhz: f64) -> ChannelSpec {
    ChannelSpec {
        number,
        lower_mhz,
        upper_mhz,
    }
}

const P: &[Technology] = &[Technology::Ieee80211p];
const CV2X: &[Technology] = &[Technology::LteV2x, Technology::NrV2x];
const ANY: &[Technology] = &[
    Technology::Ieee80211p,
    Technology::LteV2x,
    Technology::NrV2x,
];

/// 47 CFR §90.390(a) (C-V2X band segments), §90.391(a) (RSU 33 dBm per 10, 20 and
/// 30 MHz), §95.3204(a) (OBU without geofencing, or inside a coordination zone), as
/// adopted by FCC 24-123 Appendix A.
const US_RULES: &[ChannelRule] = &[
    ChannelRule {
        channel: ch(180, 5895.0, 5905.0),
        technologies: CV2X,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 23.0,
        obu_eirp_off_horizon_dbm: 23.0,
        portable_dbm: None,
        psd_dbm_per_mhz: None,
        usage: "C-V2X 10 MHz; 23 dBm for an OBU without geofencing (federal radar)",
        clause: "47 CFR §90.390(a), §90.391(a), §95.3204(a)(1) (FCC 24-123)",
    },
    ChannelRule {
        channel: ch(182, 5905.0, 5915.0),
        technologies: CV2X,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 27.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: None,
        usage: "C-V2X 10 MHz",
        clause: "47 CFR §90.390(a), §90.391(a), §95.3204(a)(2) (FCC 24-123)",
    },
    ChannelRule {
        channel: ch(184, 5915.0, 5925.0),
        technologies: CV2X,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 27.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: None,
        usage: "C-V2X 10 MHz",
        clause: "47 CFR §90.390(a), §90.391(a), §95.3204(a)(3) (FCC 24-123)",
    },
    ChannelRule {
        channel: ch(181, 5895.0, 5915.0),
        technologies: CV2X,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 23.0,
        obu_eirp_off_horizon_dbm: 23.0,
        portable_dbm: None,
        psd_dbm_per_mhz: None,
        usage: "C-V2X 20 MHz including 5.895-5.905 GHz",
        clause: "47 CFR §90.390(a), §90.391(a), §95.3204(a)(4) (FCC 24-123)",
    },
    ChannelRule {
        channel: ch(183, 5905.0, 5925.0),
        technologies: CV2X,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 27.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: None,
        usage: "C-V2X 20 MHz: the SAE J3161/1 LTE-V2X deployment channel",
        clause: "47 CFR §90.390(a), §90.391(a), §95.3204(a)(5) (FCC 24-123)",
    },
];

/// 47 CFR §90.377(b) Table and §95.1511 as in force on 3 January 2017: the DSRC band plan
/// J2945/1 was written for. The RSU EIRP limits are the table's; where it gives two, the
/// higher is for state or local government, and the lower is carried. An OBU's
/// radiated power is SAE J2945/1's `vRPMax`, 20 dBm (FMVSS 150 NPRM S5.5.2.2), on every
/// channel: the OBU rules defer to ASTM E2213-03, which is not openly available.
const US_2016_RULES: &[ChannelRule] = &[
    ChannelRule {
        channel: ch(172, 5855.0, 5865.0),
        technologies: P,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 20.0,
        obu_eirp_off_horizon_dbm: 20.0,
        portable_dbm: Some(0.0),
        psd_dbm_per_mhz: None,
        usage: "Service channel for safety of life and property: the J2945/1 BSM channel",
        clause: "47 CFR §90.377(b) and note 2, §95.1511(a) (2017); FMVSS 150 NPRM S5.3.2",
    },
    ChannelRule {
        channel: ch(174, 5865.0, 5875.0),
        technologies: P,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 20.0,
        obu_eirp_off_horizon_dbm: 20.0,
        portable_dbm: Some(0.0),
        psd_dbm_per_mhz: None,
        usage: "Service channel",
        clause: "47 CFR §90.377(b) (2017)",
    },
    ChannelRule {
        channel: ch(176, 5875.0, 5885.0),
        technologies: P,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 20.0,
        obu_eirp_off_horizon_dbm: 20.0,
        portable_dbm: Some(0.0),
        psd_dbm_per_mhz: None,
        usage: "Service channel",
        clause: "47 CFR §90.377(b) (2017)",
    },
    ChannelRule {
        channel: ch(178, 5885.0, 5895.0),
        technologies: P,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 20.0,
        obu_eirp_off_horizon_dbm: 20.0,
        portable_dbm: Some(0.0),
        psd_dbm_per_mhz: None,
        usage: "Control channel (44.8 dBm for state or local government)",
        clause: "47 CFR §90.377(b) (2017)",
    },
    ChannelRule {
        channel: ch(180, 5895.0, 5905.0),
        technologies: P,
        rsu_eirp_dbm: 23.0,
        obu_eirp_dbm: 20.0,
        obu_eirp_off_horizon_dbm: 20.0,
        portable_dbm: Some(0.0),
        psd_dbm_per_mhz: None,
        usage: "Service channel",
        clause: "47 CFR §90.377(b) (2017)",
    },
    ChannelRule {
        channel: ch(182, 5905.0, 5915.0),
        technologies: P,
        rsu_eirp_dbm: 23.0,
        obu_eirp_dbm: 20.0,
        obu_eirp_off_horizon_dbm: 20.0,
        portable_dbm: Some(0.0),
        psd_dbm_per_mhz: None,
        usage: "Service channel",
        clause: "47 CFR §90.377(b) (2017)",
    },
    ChannelRule {
        channel: ch(184, 5915.0, 5925.0),
        technologies: P,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 20.0,
        obu_eirp_off_horizon_dbm: 20.0,
        portable_dbm: Some(0.0),
        psd_dbm_per_mhz: None,
        usage: "Public safety, safety of life and property (40 dBm for public safety)",
        clause: "47 CFR §90.377(b) and note 4 (2017)",
    },
];

/// ETSI EN 302 571 V2.1.1 Table 2 (carriers, 10 MHz maximum), §4.2.2.2 (33 dBm EIRP) and
/// §4.2.3.2 (23 dBm/MHz), for OBUs and RSUs alike; ETSI EN 303 613 V1.1.1 Table B.1
/// (LTE-V2X `maxTxPower` 23 dBm conducted, `sl-bandwidth` n50 = 10 MHz in Europe).
/// 5.855-5.875 GHz is non-safety ITS (ECC Rec (08)01); 5.915-5.925 GHz is prioritised for
/// rail and allows road I2V only with national authorisation (5GAA 2024 §1.1), so it
/// carries no road V2V technology here.
const EU_RULES: &[ChannelRule] = &[
    ChannelRule {
        channel: ch(172, 5855.0, 5865.0),
        technologies: ANY,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 33.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: Some(23.0),
        usage: "ITS-G5B, non-safety ITS",
        clause: "ETSI EN 302 571 V2.1.1 Table 1-2, §4.2.2.2, §4.2.3.2",
    },
    ChannelRule {
        channel: ch(174, 5865.0, 5875.0),
        technologies: ANY,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 33.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: Some(23.0),
        usage: "ITS-G5B, non-safety ITS",
        clause: "ETSI EN 302 571 V2.1.1 Table 1-2, §4.2.2.2, §4.2.3.2",
    },
    ChannelRule {
        channel: ch(176, 5875.0, 5885.0),
        technologies: ANY,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 33.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: Some(23.0),
        usage: "ITS-G5A road safety (SCH1); 5GAA's 5G-V2X block",
        clause: "ETSI EN 302 571 V2.1.1 Table 1-2, §4.2.2.2, §4.2.3.2; 5GAA 2024 §2.3",
    },
    ChannelRule {
        channel: ch(178, 5885.0, 5895.0),
        technologies: ANY,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 33.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: Some(23.0),
        usage: "ITS-G5A road safety (SCH2); 5GAA's 5G-V2X block",
        clause: "ETSI EN 302 571 V2.1.1 Table 1-2, §4.2.2.2, §4.2.3.2; 5GAA 2024 §2.3",
    },
    ChannelRule {
        channel: ch(180, 5895.0, 5905.0),
        technologies: ANY,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 33.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: Some(23.0),
        usage: "ITS-G5A road safety, the control channel where ITS-G5 deploys",
        clause: "ETSI EN 302 571 V2.1.1 Table 1-2, §4.2.2.2, §4.2.3.2; 5GAA 2024 §2.1",
    },
    ChannelRule {
        channel: ch(182, 5905.0, 5915.0),
        technologies: ANY,
        rsu_eirp_dbm: 33.0,
        obu_eirp_dbm: 33.0,
        obu_eirp_off_horizon_dbm: 33.0,
        portable_dbm: None,
        psd_dbm_per_mhz: Some(23.0),
        usage: "Road safety ITS; the LTE-V2X day-1 channel 5GAA proposed",
        clause: "ETSI EN 302 571 V2.1.1 Table 1-2, §4.2.2.2, §4.2.3.2; 5GAA 2021",
    },
];

/// A transmitter's out-of-channel emission, as a piecewise-linear level against the
/// offset from the carrier, relative to the in-channel level (dBc, per the same
/// reference bandwidth) or, for a regulator's band-edge limits, as the leakage ratio it
/// implies into an adjacent channel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SpectrumMask {
    /// What it is.
    pub label: &'static str,
    /// `(offset from the carrier MHz, dBc)`, ascending; the level is flat at 0 dBc inside
    /// the first point and interpolated linearly in dB between points.
    pub points: &'static [(f64, f64)],
    /// The channel bandwidth the mask is for, MHz.
    pub bandwidth_mhz: f64,
    /// Where it comes from.
    pub clause: &'static str,
}

/// ETSI EN 302 571 V2.1.1 §4.2.5.2.2 Table 6: the 10 MHz transmitter spectrum mask,
/// 0 dBc to ±4.5 MHz, −26 at ±5, −32 at ±5.5, −40 at ±10, −50 at ±15 MHz (IEEE 802.11
/// Annex D class C's shape). It binds 802.11p everywhere in this crate: the US rules
/// defer to ASTM E2213-03, whose 802.11p class C mask is the same shape.
pub const EN302571_10MHZ_MASK: SpectrumMask = SpectrumMask {
    label: "en302571-10mhz",
    points: &[
        (4.5, 0.0),
        (5.0, -26.0),
        (5.5, -32.0),
        (10.0, -40.0),
        (15.0, -50.0),
    ],
    bandwidth_mhz: 10.0,
    clause: "ETSI EN 302 571 V2.1.1 §4.2.5.2.2 Table 6",
};

/// An LTE-V2X or NR-V2X UE's adjacent-channel leakage as a mask: 3GPP TS 36.101
/// §6.6.2.3.1 sets E-UTRA ACLR at 30 dB for every channel bandwidth, and TS 38.101-1
/// §6.5.2.4 NR ACLR at 30 dB. The mask is flat at −30 dBc across the adjacent channel,
/// which is what an ACLR requirement means when it is met with no margin.
pub const TS36101_ACLR_MASK: SpectrumMask = SpectrumMask {
    label: "3gpp-ue-aclr-30db",
    points: &[(5.0, 0.0), (5.01, -30.0), (15.0, -30.0), (15.01, -43.0)],
    bandwidth_mhz: 10.0,
    clause: "3GPP TS 36.101 §6.6.2.3.1 (E-UTRA ACLR 30 dB); TS 38.101-1 §6.5.2.4",
};

/// The US C-V2X unwanted-emission limits outside 5.895-5.925 GHz, 47 CFR §95.3205(a) and
/// §90.392(a) (FCC 24-123): −16 dBm/100 kHz within ±1 MHz of the band edges, −13 dBm/MHz
/// from 1 to 5 MHz, −16 dBm/MHz from 5 to 30 MHz and −28 dBm/MHz beyond. Inside the band
/// the FCC sets no mask, so an adjacent C-V2X carrier sees the 3GPP ACLR: the mask
/// carried for adjacent-channel purposes is [`TS36101_ACLR_MASK`]'s, and the band-edge
/// limits are in [`fcc_cv2x_oobe_dbm_per_mhz`].
pub const FCC_CV2X_OOBE: SpectrumMask = SpectrumMask {
    label: "fcc-24-123-cv2x",
    points: TS36101_ACLR_MASK.points,
    bandwidth_mhz: 10.0,
    clause: "47 CFR §95.3205(a), §90.392(a) (FCC 24-123); in-band: 3GPP TS 36.101 §6.6.2.3.1",
};

/// The conducted out-of-band emission limit a US C-V2X unit must meet `offset_mhz`
/// outside the 5.895-5.925 GHz band edge, dBm/MHz (47 CFR §95.3205(a); the ±1 MHz limit
/// is per 100 kHz and is scaled to a megahertz here, +10 dB).
#[must_use]
pub fn fcc_cv2x_oobe_dbm_per_mhz(offset_mhz: f64) -> f64 {
    let o = offset_mhz.abs();
    if o <= 1.0 {
        -16.0 + 10.0
    } else if o <= 5.0 {
        -13.0
    } else if o <= 30.0 {
        -16.0
    } else {
        -28.0
    }
}

impl SpectrumMask {
    /// The mask's level at an offset from the carrier, dBc.
    #[must_use]
    pub fn level_dbc(&self, offset_mhz: f64) -> f64 {
        let o = offset_mhz.abs();
        let pts = self.points;
        if pts.is_empty() || o <= pts[0].0 {
            return pts.first().map_or(0.0, |p| p.1);
        }
        for w in pts.windows(2) {
            let (x0, y0) = w[0];
            let (x1, y1) = w[1];
            if o <= x1 {
                return y0 + (o - x0) / (x1 - x0) * (y1 - y0);
            }
        }
        pts[pts.len() - 1].1
    }

    /// The adjacent-channel leakage ratio the mask allows, dB: the power it lets into a
    /// channel of `victim_bw_mhz` whose near edge is `edge_gap_mhz` above this carrier's
    /// upper edge, against the power in this carrier's own channel, both integrated in
    /// 50 kHz steps of the linear level.
    #[must_use]
    pub fn aclr_db(&self, victim_bw_mhz: f64, edge_gap_mhz: f64) -> f64 {
        let step = 0.05;
        let half = self.bandwidth_mhz * 0.5;
        let integrate = |from: f64, to: f64| {
            let n = ((to - from) / step).round().max(1.0) as usize;
            let dx = (to - from) / n as f64;
            let terms = (0..n).map(|i| {
                let f = from + (i as f64 + 0.5) * dx;
                math::pow(10.0, self.level_dbc(f) / 10.0) * dx
            });
            math::sum_ordered(terms)
        };
        let own = integrate(-half, half);
        let lo = half + edge_gap_mhz.max(0.0);
        let leak = integrate(lo, lo + victim_bw_mhz);
        10.0 * math::log10(own / leak.max(1e-30))
    }
}

/// A receiver's adjacent-channel selectivity: how far above the wanted signal an
/// adjacent-channel signal may stand before the receiver fails, dB.
///
/// * 802.11p: ETSI EN 302 571 V2.1.1 §4.2.7.2 Table 8 (from IEEE 802.11-2012 Table 18-14)
///   adjacent-channel rejection per MCS — 16 dB at BPSK 1/2 down to −1 dB at 64-QAM 3/4 —
///   and 32 to 15 dB for the alternate channel. The rejection is measured with an 802.11
///   interferer, so it already includes that interferer's own leakage; it is used as the
///   selectivity against any technology, which is conservative.
/// * LTE-V2X: TS 36.101 §7.5.1 Table 7.5.1-1, ACS 33 dB in a 10 MHz channel, 27 dB in
///   20 MHz.
/// * NR-V2X: TS 38.101-1 §7.5, ACS 33 dB for 5-20 MHz channels at 30 kHz (the 3GPP UE
///   minimum; NR-V2X sidelink reuses it).
#[must_use]
pub fn acs_db(
    victim: Technology,
    victim_bw_mhz: f64,
    victim_mcs_index: u8,
    alternate: bool,
) -> f64 {
    match victim {
        Technology::Ieee80211p => {
            // Table 8 by the 802.11p rate index: 0 BPSK 1/2 (3 Mb/s) to 7 64-QAM 3/4.
            const ADJ: [f64; 8] = [16.0, 15.0, 13.0, 11.0, 8.0, 4.0, 0.0, -1.0];
            const ALT: [f64; 8] = [32.0, 31.0, 29.0, 27.0, 24.0, 20.0, 16.0, 15.0];
            let i = usize::from(victim_mcs_index.min(7));
            if alternate { ALT[i] } else { ADJ[i] }
        }
        Technology::LteV2x => {
            if victim_bw_mhz > 15.0 {
                27.0
            } else {
                33.0
            }
        }
        Technology::NrV2x => 33.0,
    }
}

/// The adjacent-channel interference ratio between an aggressor and a victim, dB:
/// `ACIR = −10·log10(10^(−ACLR/10) + 10^(−ACS/10))` (3GPP TR 36.942 §5.1.1.3; the same
/// combination ECC reports use). An interferer received at `P` dBm on the adjacent channel
/// adds `P − ACIR` dBm of in-channel interference at the victim.
#[must_use]
pub fn acir_db(aclr_db: f64, acs_db: f64) -> f64 {
    -10.0 * math::log10(math::pow(10.0, -aclr_db / 10.0) + math::pow(10.0, -acs_db / 10.0))
}

/// The ACIR between a transmitter of `aggressor` on channel `aggressor_ch` and a receiver
/// of `victim` on channel `victim_ch`, both where `region` allows them: the aggressor's
/// mask leakage into the victim's channel ([`SpectrumMask::aclr_db`]) combined with the
/// victim's selectivity ([`acs_db`], the alternate-channel figure once the channels are a
/// whole channel apart) by [`acir_db`]. `victim_mcs_index` is the 802.11p rate index
/// (0-7); a sidelink ignores it.
///
/// These are the rules' and the standards' *minimum* figures, so the ratio is the worst
/// a conformant pair of radios may show: fielded hardware does better (the 5GAA P-190033
/// adjacent-channel field test is the reference, `sweep::field`).
///
/// # Errors
/// Why the pair is not an adjacent-channel pair: the region does not allow a technology
/// on its channel, or the channels overlap, which is co-channel interference.
pub fn adjacent_acir_db(
    region: Region,
    aggressor: Technology,
    aggressor_ch: u16,
    victim: Technology,
    victim_ch: u16,
    victim_mcs_index: u8,
) -> Result<f64, String> {
    let a = region.rule(aggressor, aggressor_ch).ok_or_else(|| {
        format!(
            "{} does not allow {aggressor:?} on channel {aggressor_ch} (it allows it on {:?})",
            region.id(),
            region.channels_for(aggressor)
        )
    })?;
    let v = region.rule(victim, victim_ch).ok_or_else(|| {
        format!(
            "{} does not allow {victim:?} on channel {victim_ch}",
            region.id()
        )
    })?;
    let gap = v.channel.edge_gap_mhz(&a.channel);
    if gap < 0.0 {
        return Err(format!(
            "channel {aggressor_ch} overlaps the run's channel {victim_ch}: that is \
             co-channel interference, which threats.jammers models"
        ));
    }
    let victim_bw = v.channel.bandwidth_mhz();
    let aclr = region.mask(aggressor).aclr_db(victim_bw, gap);
    let alternate = gap >= a.channel.bandwidth_mhz() - 1e-9;
    let acs = acs_db(victim, victim_bw, victim_mcs_index, alternate);
    Ok(acir_db(aclr, acs))
}

/// The most a station may radiate on a channel, dBm EIRP.
///
/// * An RSU gets the channel's limit, less `20·log10(h/8)` dB when its antenna is above
///   8 m in the US (47 CFR §90.391(b); §90.377(b) note 1 in the 2016 plan). An antenna
///   above 15 m is not allowed there; this returns the 15 m figure and
///   [`rsu_height_allowed`] says so.
/// * An OBU gets the limit toward the horizon, where every V2V link lies.
/// * A portable unit gets the portable limit where the rule has one, else the OBU's.
#[must_use]
pub fn max_eirp_dbm(region: Region, rule: &ChannelRule, station: Station, antenna_m: f64) -> f64 {
    match station {
        Station::Rsu => {
            let us = matches!(region, Region::Us | Region::Us2016);
            if us && antenna_m > 8.0 {
                rule.rsu_eirp_dbm - 20.0 * math::log10(antenna_m.min(15.0) / 8.0)
            } else {
                rule.rsu_eirp_dbm
            }
        }
        Station::Obu => rule.obu_eirp_dbm,
        Station::Portable => rule.portable_dbm.unwrap_or(rule.obu_eirp_dbm),
    }
}

/// Whether a roadside antenna may stand this high: 15 m above the roadway at most in the
/// US (47 CFR §90.391(b)); no height rule in EN 302 571.
#[must_use]
pub fn rsu_height_allowed(region: Region, antenna_m: f64) -> bool {
    !matches!(region, Region::Us | Region::Us2016) || antenna_m <= 15.0
}

/// `radio/regulation` — the card a run registers for its region.
#[must_use]
pub fn card(region: Region) -> ModelCard {
    let mut card = ModelCard::new(
        REGULATION_ID,
        Family::Phy,
        "1.0.0",
        "Spectrum regulation per region: the channel each technology deploys on, the EIRP \
         limits of on-board and roadside units, the transmitter masks, and the \
         adjacent-channel interference ratio between technologies on neighbouring \
         channels.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new(
            "RSU antenna-height reduction (US)",
            "EIRP_max = EIRP_table − 20·log10(h/8) for 8 m < h ≤ 15 m",
        ),
        Equation::new(
            "adjacent-channel interference ratio",
            "ACIR = −10·log10(10^(−ACLR/10) + 10^(−ACS/10))",
        ),
    ];
    let fcc = Source {
        kind: SourceKind::Standard,
        reference: "FCC 24-123, Second Report and Order, Use of the 5.850-5.925 GHz Band \
                    (adopted 21 November 2024), Appendix A: 47 CFR §90.386-§90.392 and \
                    §95.3201-§95.3205; ¶82 (DSRC licences cancel two years after \
                    publication, 13 December 2024)"
            .to_string(),
        accessed: Some("2026-09-29".to_string()),
        note: None,
    };
    let cfr2016 = Source {
        kind: SourceKind::Standard,
        reference: "47 CFR §90.377(b) and §95.1511 as in force on 3 January 2017 (eCFR \
                    point-in-time); FMVSS 150 NPRM, 82 FR 3854 (12 January 2017), S5.3.2 \
                    (BSM on channel 172) and S5.5.2.2 (20 dBm maximum)"
            .to_string(),
        accessed: Some("2026-09-29".to_string()),
        note: None,
    };
    let etsi = Source {
        kind: SourceKind::Standard,
        reference: "ETSI EN 302 571 V2.1.1 (2017-02) §4.2.1-§4.2.7 (carriers, 33 dBm EIRP, \
                    23 dBm/MHz, mask Table 6, rejection Table 8); ETSI EN 303 613 V1.1.1 \
                    (2020-01) Annex B; 5GAA position papers on the European deployment band \
                    configuration (June 2021, March 2024)"
            .to_string(),
        accessed: Some("2026-09-29".to_string()),
        note: None,
    };
    let aclr = Source::new(
        SourceKind::Standard,
        "3GPP TS 36.101 §6.6.2.3.1 (E-UTRA ACLR 30 dB) and §7.5.1 (ACS 33 dB at 10 MHz, 27 dB \
         at 20 MHz); TS 38.101-1 §6.5.2.4 and §7.5; TR 36.942 §5.1.1.3 (ACIR)",
    );
    card.parameters = vec![Parameter::new(
        "region",
        "-",
        serde_json::json!(region.id()),
        match region {
            Region::Us => fcc.clone(),
            Region::Us2016 => cfr2016.clone(),
            Region::Eu => etsi.clone(),
        },
    )];
    card.assumptions = vec![
        "A US C-V2X OBU has no geofence (§95.3204(a)): 27 dBm EIRP toward the horizon on \
         5.905-5.925 GHz, 23 dBm on any channel that includes 5.895-5.905 GHz."
            .to_string(),
        "Where a limit is given for state or local government only (44.8 dBm on channel \
         178, 40 dBm on 184 in the 2016 plan), the general limit is carried."
            .to_string(),
        "The 3GPP ACLR and ACS minimum requirements are met with no margin; real \
         equipment usually does better, so the adjacent-channel interference is an upper \
         bound."
            .to_string(),
    ];
    card.limitations = vec![
        "A DSRC OBU's power in the 2016 plan is SAE J2945/1's 20 dBm, not ASTM E2213-03's \
         class table, which is not openly available."
            .to_string(),
        "Europe's CEN DSRC tolling protection (EN 302 571 §4.2.9, 10 dBm inside a \
         protected zone) is not modelled: no world carries toll-plaza zones."
            .to_string(),
    ];
    card.sources = vec![fcc, cfr2016, etsi, aclr];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "the_us_band_plan_is_fcc_24_123s".to_string(),
            "the_2016_band_plan_is_the_cfr_table".to_string(),
            "europe_allows_every_technology_in_10_mhz_channels".to_string(),
            "the_en302571_mask_leaks_what_its_table_implies".to_string(),
            "acir_combines_leakage_and_selectivity".to_string(),
        ],
    };
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card
}

/// The regulation card's id.
pub const REGULATION_ID: &str = "radio/regulation";

#[cfg(test)]
mod tests {
    use super::*;

    /// Europe's side-by-side deployment: ITS-G5 on 180 beside LTE-V2X on 182. The
    /// ITS-G5 receiver's 6 Mbit/s adjacent-channel rejection (13 dB, EN 302 571 Table 8)
    /// dominates the LTE-V2X transmitter's 30 dB ACLR, so ACIR is just under 13 dB; the
    /// LTE-V2X receiver's 33 dB ACS and the 802.11p mask's leakage give about 30 dB the
    /// other way. Overlapping channels and channels a region forbids are refused.
    #[test]
    fn the_adjacent_channel_ratio_is_the_masks_and_the_selectivity_combined() {
        use Technology::{Ieee80211p, LteV2x, NrV2x};
        let g5_victim = adjacent_acir_db(Region::Eu, LteV2x, 182, Ieee80211p, 180, 2).unwrap();
        let want = acir_db(30.0, 13.0);
        assert!((g5_victim - want).abs() < 0.2, "{g5_victim} against {want}");
        assert!(g5_victim < 13.0 && g5_victim > 12.5, "{g5_victim}");
        let lte_victim = adjacent_acir_db(Region::Eu, Ieee80211p, 180, LteV2x, 182, 0).unwrap();
        assert!(lte_victim > 25.0 && lte_victim < 33.0, "{lte_victim}");
        // One channel further out, the alternate-channel figures apply and it grows.
        let alt = adjacent_acir_db(Region::Eu, NrV2x, 178, Ieee80211p, 182, 2);
        assert!(
            alt.is_err() || alt.unwrap() > g5_victim,
            "alternate channel"
        );
        let g5_alt = adjacent_acir_db(Region::Eu, LteV2x, 182, Ieee80211p, 178, 2);
        assert!(g5_alt.is_err() || g5_alt.unwrap() > g5_victim);
        // Co-channel is not adjacent; the US gives DSRC no channel.
        assert!(adjacent_acir_db(Region::Eu, LteV2x, 180, Ieee80211p, 180, 2).is_err());
        assert!(adjacent_acir_db(Region::Us, Ieee80211p, 180, LteV2x, 183, 0).is_err());
    }

    #[test]
    fn the_us_band_plan_is_fcc_24_123s() {
        let us = Region::Us;
        // C-V2X only, 5.895-5.925 GHz, and DSRC refused with the reason.
        assert!(us.rules().iter().all(|r| r.channel.lower_mhz >= 5895.0
            && r.channel.upper_mhz <= 5925.0
            && !r.technologies.contains(&Technology::Ieee80211p)));
        let err = us.default_channel(Technology::Ieee80211p).unwrap_err();
        assert!(err.contains("FCC 24-123") && err.contains("13 December 2026"));
        // J3161/1's 20 MHz channel 183, centre 5.915 GHz.
        let lte = us.default_channel(Technology::LteV2x).unwrap();
        assert_eq!(lte.channel.number, 183);
        assert!((lte.channel.centre_hz() - 5.915e9).abs() < 1.0);
        assert_eq!(lte.channel.bandwidth_mhz(), 20.0);
        // §95.3204(a): 33 dBm but 27 dBm within ±5° of horizontal; 23 dBm with 5895-5905.
        assert_eq!(max_eirp_dbm(us, lte, Station::Obu, 1.5), 27.0);
        assert_eq!(lte.obu_eirp_off_horizon_dbm, 33.0);
        assert_eq!(us.rule(Technology::LteV2x, 181).unwrap().obu_eirp_dbm, 23.0);
        assert_eq!(us.rule(Technology::LteV2x, 180).unwrap().obu_eirp_dbm, 23.0);
        // §90.391: RSU 33 dBm up to 8 m, then −20·log10(h/8), 15 m at most.
        assert_eq!(max_eirp_dbm(us, lte, Station::Rsu, 8.0), 33.0);
        let at_12 = max_eirp_dbm(us, lte, Station::Rsu, 12.0);
        assert!((at_12 - (33.0 - 20.0 * math::log10(12.0 / 8.0))).abs() < 1e-9);
        assert!(rsu_height_allowed(us, 15.0) && !rsu_height_allowed(us, 15.5));
    }

    #[test]
    fn the_2016_band_plan_is_the_cfr_table() {
        let r = Region::Us2016;
        let p = r.default_channel(Technology::Ieee80211p).unwrap();
        assert_eq!(p.channel.number, 172);
        assert!((p.channel.centre_hz() - 5.860e9).abs() < 1.0);
        // The §90.377(b) RSU column, general limits.
        let rsu: Vec<(u16, f64)> = r
            .rules()
            .iter()
            .map(|x| (x.channel.number, x.rsu_eirp_dbm))
            .collect();
        assert_eq!(
            rsu,
            vec![
                (172, 33.0),
                (174, 33.0),
                (176, 33.0),
                (178, 33.0),
                (180, 23.0),
                (182, 23.0),
                (184, 33.0)
            ]
        );
        assert!(r.default_channel(Technology::LteV2x).is_err());
        assert_eq!(max_eirp_dbm(r, p, Station::Portable, 1.5), 0.0);
    }

    #[test]
    fn europe_allows_every_technology_in_10_mhz_channels() {
        let eu = Region::Eu;
        for rule in eu.rules() {
            assert_eq!(rule.channel.bandwidth_mhz(), 10.0);
            assert_eq!(rule.obu_eirp_dbm, 33.0);
            assert_eq!(rule.psd_dbm_per_mhz, Some(23.0));
        }
        let g5 = eu.default_channel(Technology::Ieee80211p).unwrap().channel;
        let lte = eu.default_channel(Technology::LteV2x).unwrap().channel;
        let nr = eu.default_channel(Technology::NrV2x).unwrap().channel;
        assert_eq!((g5.number, lte.number, nr.number), (180, 182, 178));
        // ITS-G5 at 5.895-5.905 and LTE-V2X at 5.905-5.915 GHz share an edge.
        assert_eq!(g5.edge_gap_mhz(&lte), 0.0);
        assert!(!g5.overlaps(&lte));
        assert_eq!(g5.edge_gap_mhz(&nr), 0.0);
    }

    /// Integrating EN 302 571 Table 6 over the adjacent 10 MHz channel gives an ACLR in
    /// the mid-30s of dB (a mask is a limit, not a measurement), and the alternate channel
    /// much more.
    #[test]
    fn the_en302571_mask_leaks_what_its_table_implies() {
        let m = &EN302571_10MHZ_MASK;
        assert_eq!(m.level_dbc(0.0), 0.0);
        assert_eq!(m.level_dbc(5.0), -26.0);
        assert!((m.level_dbc(7.75) - -36.0).abs() < 1e-9);
        assert_eq!(m.level_dbc(20.0), -50.0);
        let adj = m.aclr_db(10.0, 0.0);
        let alt = m.aclr_db(10.0, 10.0);
        assert!((34.0..40.0).contains(&adj), "adjacent ACLR {adj}");
        assert!(alt > adj + 10.0, "alternate {alt} vs adjacent {adj}");
        let lte = TS36101_ACLR_MASK.aclr_db(10.0, 0.0);
        assert!((lte - 30.0).abs() < 0.5, "3GPP mask ACLR {lte}");
        assert_eq!(fcc_cv2x_oobe_dbm_per_mhz(10.0), -16.0);
        assert_eq!(fcc_cv2x_oobe_dbm_per_mhz(40.0), -28.0);
    }

    #[test]
    fn acir_combines_leakage_and_selectivity() {
        // Equal ACLR and ACS lose 3 dB; a much better one leaves the worse.
        assert!((acir_db(30.0, 30.0) - (30.0 - 10.0 * math::log10(2.0))).abs() < 1e-9);
        assert!((acir_db(60.0, 13.0) - 13.0).abs() < 0.01);
        // LTE-V2X next to an ITS-G5 receiver at QPSK 1/2: the receiver's 13 dB rules.
        let into_g5 = acir_db(30.0, acs_db(Technology::Ieee80211p, 10.0, 2, false));
        assert!((12.0..13.0).contains(&into_g5), "{into_g5}");
        // ITS-G5 next to an LTE-V2X receiver: the 802.11p mask's leakage and ACS 33 dB.
        let aclr = EN302571_10MHZ_MASK.aclr_db(10.0, 0.0);
        let into_lte = acir_db(aclr, acs_db(Technology::LteV2x, 10.0, 0, false));
        assert!(into_lte > 30.0 && into_lte < 33.0, "{into_lte}");
    }

    #[test]
    fn every_region_resolves_by_id_and_carries_a_valid_card() {
        for r in Region::ALL {
            assert_eq!(Region::from_id(r.id()), Some(r));
            let c = card(r);
            assert!(c.validate().is_ok(), "{:?}", c.validate());
        }
        assert_eq!(Region::from_id("mars"), None);
    }
}
