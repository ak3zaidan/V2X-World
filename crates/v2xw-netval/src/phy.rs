//! Layer 1: propagation, fading, the error model, noise, sensitivity and the link budget.
//!
//! The references are written here from the published equations with the platform's own
//! `f64` functions, never by calling back into `v2xw-radio`.

use v2xw_core::card::Tier;
use v2xw_core::geom::Vec3;
use v2xw_core::ids::{LinkKey, NodeId};
use v2xw_core::weather::WeatherState;
use v2xw_radio::fading::{NakagamiFading, NakagamiPreset};
use v2xw_radio::obstacle::{NlosvCase, knife_edge_loss_db, knife_edge_loss_exact_db};
use v2xw_radio::phy::{OfdmPhy, air_time};
use v2xw_radio::prop::{
    self, FreeSpace, LogDistancePreset, LogDistanceShadowing, Polarization, Tr37885State,
};
use v2xw_radio::traits::{Fading, Propagation};
use v2xw_radio::types::{ActorClass, LosResult, Mcs, RadioEndpoint};
use v2xw_radio::{NoFading, cv2x};
use v2xw_world::model::EnvClass;

use crate::ctx::TestCtx;
use crate::stats;
use crate::{Check, Cost, Layer, Mode, Outcome};

/// The defined speed of light, m/s.
const C: f64 = 299_792_458.0;
/// The SI-exact Boltzmann constant, J/K.
const BOLTZMANN: f64 = 1.380_649e-23;

fn log10(x: f64) -> f64 {
    x.log10()
}

/// The checks of this layer.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "PHY-01",
            layer: Layer::Physical,
            title: "Free-space loss equals 20·log10(4π·d·f/c) at every distance and carrier",
            reference: "Friis transmission equation; ITU-R P.525-4 Eq. 4 (Lbf = 20 log(4πd/λ))",
            tolerance: "1e-9 dB (both sides are exact; only rounding separates them)",
            fault: "The textbook-rounded constant 32.44 dB in place of 32.4478 dB",
            cost: Cost::Fast,
            run: friis,
        },
        Check {
            id: "PHY-02",
            layer: Layer::Physical,
            title: "Two-ray ground: Friis below the crossover 4π·ht·hr/λ, 40·log10(d) − 20·log10(ht·hr) above, continuous at it",
            reference: "Rappaport, Wireless Communications 2nd ed. Eq. 4.52; ns-3 TwoRayGroundPropagationLossModel",
            tolerance: "1e-9 dB pointwise; 1e-6 dB jump at the crossover",
            fault: "The receiver antenna height read as 1.4 m instead of 1.5 m",
            cost: Cost::Fast,
            run: two_ray,
        },
        Check {
            id: "PHY-03",
            layer: Layer::Physical,
            title: "TR 37.885 sidelink path loss (highway LOS, urban LOS, NLOS) and shadowing σ per state",
            reference: "3GPP TR 37.885 V15.3.0 Table 6.2.1-1 (σ: LOS 3, NLOSv 3, NLOS 4 dB)",
            tolerance: "1e-9 dB",
            fault: "Urban LOS distance slope 17.6 (a transposed digit) instead of 16.7",
            cost: Cost::Fast,
            run: tr37885_pathloss,
        },
        Check {
            id: "PHY-04",
            layer: Layer::Physical,
            title: "TR 37.885 line-of-sight probability, highway and urban",
            reference: "3GPP TR 37.885 Table 6.2.1-2: highway min(1, 2.1013e-6 d² − 0.002 d + 1.0193) to 475 m, max(0, 0.54 − 0.001(d − 475)) beyond; urban min(1, 1.05·exp(−0.0114 d))",
            tolerance: "1e-12",
            fault: "Urban decay constant 0.0141 instead of 0.0114",
            cost: Cost::Fast,
            run: tr37885_plos,
        },
        Check {
            id: "PHY-05",
            layer: Layer::Physical,
            title: "TR 37.885 NLOSv vehicle-blockage loss: mean and σ per antenna-height case",
            reference: "3GPP TR 37.885 §6.2.1: case 1 0 dB; case 2 μ = 9 + max(0, 15·log10(d) − 41), σ 4.5; case 3 μ = 5 + max(0, 15·log10(d) − 41), σ 4",
            tolerance: "1e-9 dB",
            fault: "Ramp offset 14 instead of 41 (the loss grows from 9 m instead of 545 m)",
            cost: Cost::Fast,
            run: nlosv,
        },
        Check {
            id: "PHY-06",
            layer: Layer::Physical,
            title: "Nakagami-m fading: the power gain is Gamma(m, 1/m) with unit mean, for m = 1, 3, 5",
            reference: "Nakagami 1960; Torrent-Moreno et al. IEEE TVT 2009 Eq. 2. One-sample Kolmogorov-Smirnov test of 20,000 draws per m against the Gamma CDF",
            tolerance: "KS p-value > 0.001 for each m; mean power gain within 4 standard errors of 1",
            fault: "The model built with the m = 1 (Rayleigh) preset where m = 3 is configured",
            cost: Cost::Fast,
            run: nakagami,
        },
        Check {
            id: "PHY-07",
            layer: Layer::Physical,
            title: "Log-normal shadowing: N(0, σ) marginal across links, and Gudmundson autocorrelation exp(−Δ/d_corr) along a link",
            reference: "Gudmundson, Electronics Letters 27(23) 1991; Abbas et al. 2015 Table II (σ 3.95 dB highway LOS); TR 36.885 Annex A.1.4. KS test on 5,000 links; lag correlation over 20,000 steps",
            tolerance: "KS p > 0.001; |ρ̂ − exp(−5/25)| < 0.02 (5σ of the estimator)",
            fault: "Decorrelation distance 50 m where 25 m is configured",
            cost: Cost::Fast,
            run: shadowing,
        },
        Check {
            id: "PHY-08",
            layer: Layer::Physical,
            title: "Single knife-edge diffraction: the P.526 approximation as printed, and the exact Fresnel-integral loss",
            reference: "ITU-R P.526-15 Eq. 31 (J(ν) = 6.9 + 20 log10(√((ν−0.1)²+1) + ν − 0.1), ν > −0.78) and Eqs. 26-27 with independent Fresnel series; J(0) = 6.02 dB",
            tolerance: "1e-9 dB for Eq. 31; 0.01 dB for the exact loss",
            fault: "The exact loss evaluated at ν + 0.05 (an obstacle height a few cm high)",
            cost: Cost::Fast,
            run: knife_edge,
        },
        Check {
            id: "PHY-09",
            layer: Layer::Physical,
            title: "Noise floor: kTB at 290 K over 10 MHz plus the noise figure; the sidelink's kT density",
            reference: "kT = −173.975 dBm/Hz at 290 K (k = 1.380649e-23 J/K, SI 2019); IEEE 802.11 10 MHz channel; NF 6 dB hardware (04-models.md §4.7), 9 dB UE (TR 37.885 Table 6.1.1-1)",
            tolerance: "0.05 dB (the 802.11p floor carries −104 dBm, a 0.02 dB rounding of −103.98)",
            fault: "Noise bandwidth taken as 20 MHz (the 802.11a channel) instead of 10 MHz",
            cost: Cost::Fast,
            run: noise_floor,
        },
        Check {
            id: "PHY-10",
            layer: Layer::Physical,
            title: "Receiver sensitivity per MCS against the ETSI table; dynamic sensitivity at 6 Mbit/s",
            reference: "ETSI EN 302 663 V1.3.1 (2020-01) Table 1 (static: −91, −90, −88, −86, −83, −79, −75, −74 dBm) and Table 2 (dynamic, 6 Mbit/s: −85 dBm), read from the published PDF",
            tolerance: "exact",
            fault: "The IEEE 802.11-2016 Table 17-18 10 MHz figures (−85 ... −68 dBm) in place of EN 302 663's",
            cost: Cost::Fast,
            run: sensitivity,
        },
        Check {
            id: "PHY-11",
            layer: Layer::Physical,
            title: "PPDU air time for every MCS and frame length from 1 to 2,304 bytes",
            reference: "IEEE 802.11-2016 Eq. 17-29: TXTIME = T_PREAMBLE + T_SIGNAL + T_SYM·⌈(16 + 8·L + 6)/N_DBPS⌉, with 32, 8 and 8 µs at 10 MHz (EN 302 663 Table C.2) and N_DBPS from EN 302 663 Table C.1",
            tolerance: "exact, in nanoseconds",
            fault: "The 6 tail bits left out of the symbol count",
            cost: Cost::Fast,
            run: airtime,
        },
        Check {
            id: "PHY-12",
            layer: Layer::Physical,
            title: "Link-budget arithmetic: received power = Pt + Gt + Gr − FSPL for an unobstructed free-space link",
            reference: "Hand calculation of the Friis link budget; antenna gain 3 dBi per vehicle (TR 36.885 Table A.1.1-1)",
            tolerance: "1e-9 dB",
            fault: "The receive antenna gain counted twice",
            cost: Cost::Fast,
            run: link_budget,
        },
        Check {
            id: "PHY-13",
            layer: Layer::Physical,
            title: "Rain specific attenuation: the P.838 regression reproduces the recommendation's own 6 GHz table row",
            reference: "ITU-R P.838-3 Table 5, 6 GHz: kH 0.0007056, αH 1.5900, kV 0.0004878, αV 1.5728",
            tolerance: "0.05 % on k, 0.0005 on α (the table's last printed digit)",
            fault: "Horizontal and vertical coefficient sets swapped",
            cost: Cost::Fast,
            run: rain,
        },
    ]
}

fn friis(mode: Mode) -> Outcome {
    let mut worst: f64 = 0.0;
    for &f in &[5.860e9, 5.890e9, 5.900e9, 5.920e9] {
        for &d in &[1.0, 2.0, 5.0, 10.0, 31.6, 100.0, 300.0, 1_000.0, 3_000.0] {
            let reference = 20.0 * log10(4.0 * core::f64::consts::PI * d * f / C);
            let mut model = prop::friis_loss_db(d, f);
            if mode.faulted() {
                model -= prop::FRIIS_CONST_DB - 32.44;
            }
            worst = worst.max((model - reference).abs());
        }
    }
    Outcome::judge(worst < 1e-9, format!("max |error| {worst:.3e} dB over 36 points"))
}

fn two_ray(mode: Mode) -> Outcome {
    let (ht, hr, f) = (1.5, 1.5, 5.9e9);
    let lambda = C / f;
    let dc = 4.0 * core::f64::consts::PI * ht * hr / lambda;
    let reference = |d: f64| {
        if d <= dc {
            20.0 * log10(4.0 * core::f64::consts::PI * d / lambda)
        } else {
            40.0 * log10(d) - 20.0 * log10(ht * hr)
        }
    };
    let hr_model = if mode.faulted() { 1.4 } else { hr };
    let mut worst: f64 = 0.0;
    let mut d = 1.0;
    while d < 5_000.0 {
        let model = prop::two_ray_ground_loss_db(d, ht, hr_model, f, 1.0);
        worst = worst.max((model - reference(d)).abs());
        d *= 1.07;
    }
    let jump = (prop::two_ray_ground_loss_db(dc * (1.0 + 1e-12), ht, hr_model, f, 1.0)
        - prop::two_ray_ground_loss_db(dc * (1.0 - 1e-12), ht, hr_model, f, 1.0))
    .abs();
    Outcome::judge(
        worst < 1e-9 && jump < 1e-6,
        format!("crossover {dc:.1} m; max |error| {worst:.3e} dB; jump at crossover {jump:.3e} dB"),
    )
}

fn tr37885_pathloss(mode: Mode) -> Outcome {
    let fc = 5.9;
    let mut worst: f64 = 0.0;
    for &d in &[3.0, 10.0, 50.0, 100.0, 320.0, 1_000.0] {
        let hw = 32.4 + 20.0 * log10(d) + 20.0 * log10(fc);
        let ur = 38.77 + 16.7 * log10(d) + 18.2 * log10(fc);
        let nl = 36.85 + 30.0 * log10(d) + 18.9 * log10(fc);
        let mut ur_model = prop::tr37885_urban_los_db(d, fc);
        if mode.faulted() {
            ur_model += (17.6 - 16.7) * log10(d);
        }
        worst = worst
            .max((prop::tr37885_highway_los_db(d, fc) - hw).abs())
            .max((ur_model - ur).abs())
            .max((prop::tr37885_nlos_db(d, fc) - nl).abs());
    }
    let sigma_ok = prop::tr37885_sigma_db(Tr37885State::Los) == 3.0
        && prop::tr37885_sigma_db(Tr37885State::Nlosv) == 3.0
        && prop::tr37885_sigma_db(Tr37885State::Nlos) == 4.0;
    Outcome::judge(
        worst < 1e-9 && sigma_ok,
        format!("max |error| {worst:.3e} dB over 18 points; σ table {}", if sigma_ok { "matches" } else { "differs" }),
    )
}

fn tr37885_plos(mode: Mode) -> Outcome {
    let mut worst: f64 = 0.0;
    let mut d = 0.0;
    while d <= 1_200.0 {
        let hw = if d <= 475.0 {
            (2.1013e-6 * d * d - 0.002 * d + 1.0193).min(1.0)
        } else {
            (0.54 - 0.001 * (d - 475.0)).max(0.0)
        };
        let ur = (1.05 * (-0.0114 * d).exp()).min(1.0);
        let ur_model = if mode.faulted() {
            (1.05 * (-0.0141 * d).exp()).min(1.0)
        } else {
            prop::p_los_urban(d)
        };
        worst = worst
            .max((prop::p_los_highway(d) - hw).abs())
            .max((ur_model - ur).abs());
        d += 2.5;
    }
    Outcome::judge(worst < 1e-12, format!("max |error| {worst:.3e} over 481 distances"))
}

fn nlosv(mode: Mode) -> Outcome {
    let mut worst: f64 = 0.0;
    for &d in &[5.0, 100.0, 545.0, 600.0, 1_000.0] {
        let ramp = (15.0 * log10(d) - 41.0).max(0.0);
        let model_ramp = |case: NlosvCase| {
            let m = case.mean_db(d);
            if mode.faulted() && case != NlosvCase::MinAntennaAboveBlocker {
                let base = if case == NlosvCase::MaxAntennaBelowBlocker { 9.0 } else { 5.0 };
                base + (15.0 * log10(d) - 14.0).max(0.0)
            } else {
                m
            }
        };
        worst = worst
            .max(model_ramp(NlosvCase::MinAntennaAboveBlocker).abs())
            .max((model_ramp(NlosvCase::MaxAntennaBelowBlocker) - (9.0 + ramp)).abs())
            .max((model_ramp(NlosvCase::Between) - (5.0 + ramp)).abs());
    }
    let sig = NlosvCase::MinAntennaAboveBlocker.sigma_db() == 0.0
        && NlosvCase::MaxAntennaBelowBlocker.sigma_db() == 4.5
        && NlosvCase::Between.sigma_db() == 4.0;
    let classify = NlosvCase::classify(1.5, 1.5, 3.0) == NlosvCase::MaxAntennaBelowBlocker
        && NlosvCase::classify(3.5, 3.5, 3.0) == NlosvCase::MinAntennaAboveBlocker
        && NlosvCase::classify(1.5, 3.5, 3.0) == NlosvCase::Between;
    Outcome::judge(
        worst < 1e-9 && sig && classify,
        format!("max |mean error| {worst:.3e} dB; σ {}; case classification {}", if sig { "ok" } else { "wrong" }, if classify { "ok" } else { "wrong" }),
    )
}

fn nakagami(mode: Mode) -> Outcome {
    let n = 20_000usize;
    let mut parts = Vec::new();
    for (preset, m) in [
        (NakagamiPreset::FixedSevere, 1.0),
        (NakagamiPreset::FixedMedium, 3.0),
        (NakagamiPreset::FixedLow, 5.0),
    ] {
        let built = if mode.faulted() && m == 3.0 { NakagamiPreset::FixedSevere } else { preset };
        let mut model = NakagamiFading::new(built);
        let mut ctx = TestCtx::new(0x6e61_6b61);
        let link = LinkKey(NodeId::new(1), NodeId::new(2));
        let gains: Vec<f64> = (0..n)
            .map(|i| {
                let db = Fading::sample_db(&mut model, &mut ctx, link, 50.0, 1_000 + i as u64 * 100_000);
                10f64.powf(db / 10.0)
            })
            .collect();
        let d = stats::ks_statistic(&gains, |x| stats::gamma_cdf(x, m, 1.0 / m));
        let p = stats::ks_p_value(d, n);
        let mean = stats::mean(&gains);
        let se = (1.0 / m).sqrt() / (n as f64).sqrt();
        let ok = p > 1e-3 && (mean - 1.0).abs() < 4.0 * se;
        parts.push((ok, format!("m={m}: KS D={d:.4} p={p:.3}, mean gain {mean:.4}")));
    }
    Outcome::all(parts)
}

fn endpoint(node: u32, x: f64, y: f64) -> RadioEndpoint {
    RadioEndpoint::isotropic(NodeId::new(node), Vec3::new(x, y, 1.5), ActorClass::Car, 0)
}

fn shadowing(mode: Mode) -> Outcome {
    let preset = LogDistancePreset::AbbasLosHighway;
    let sigma = preset.params().sigma_db;
    let los = LosResult::default();
    let w = WeatherState::CLEAR;
    // Marginal: 5,000 independent links at 50 m.
    let mut ctx = TestCtx::new(0x5348_4144);
    let mut model = LogDistanceShadowing::new(Tier::High, preset, EnvClass::Highway);
    let mut samples = Vec::with_capacity(5_000);
    for i in 0..5_000u32 {
        let tx = endpoint(2 * i, 0.0, 0.0);
        let rx = endpoint(2 * i + 1, 50.0, 0.0);
        let b = Propagation::loss_db(&mut model, &mut ctx, &tx, &rx, 5.9e9, &los, &w);
        samples.push(b.shadow_db);
    }
    let d = stats::ks_statistic(&samples, |x| stats::normal_cdf(x / sigma));
    let p = stats::ks_p_value(d, samples.len());
    // Correlation: one link, the transmitter moving 5 m per evaluation.
    let d_corr = 25.0;
    let configured = if mode.faulted() { 50.0 } else { d_corr };
    let mut model = LogDistanceShadowing::new(Tier::High, preset, EnvClass::Highway)
        .with_decorrelation_m(configured);
    let mut ctx = TestCtx::new(0x434f_5252);
    let rx = endpoint(1, 0.0, 0.0);
    let mut prev = None;
    let mut pairs = Vec::new();
    for step in 0..20_000u32 {
        let tx = endpoint(0, 60.0 + 5.0 * f64::from(step), 0.0);
        let s = Propagation::loss_db(&mut model, &mut ctx, &tx, &rx, 5.9e9, &los, &w).shadow_db;
        if let Some(p) = prev {
            pairs.push((p, s));
        }
        prev = Some(s);
    }
    let rho = stats::correlation(&pairs);
    let expected = (-5.0f64 / d_corr).exp();
    Outcome::judge(
        p > 1e-3 && (rho - expected).abs() < 0.02,
        format!("marginal KS D={d:.4} p={p:.3} (σ {sigma} dB); lag-5 m correlation {rho:.4} against {expected:.4}"),
    )
}

/// The Fresnel integrals `C(x)`, `S(x)` by their power series, for `|x| ≤ 4`.
fn fresnel(x: f64) -> (f64, f64) {
    let h = core::f64::consts::FRAC_PI_2;
    let mut c = 0.0;
    let mut s = 0.0;
    let mut fact = 1.0; // (2n)! then (2n+1)!
    for n in 0..60 {
        let n2 = 2 * n;
        if n > 0 {
            fact *= (n2 - 1) as f64 * n2 as f64;
        }
        let sign = if n % 2 == 0 { 1.0 } else { -1.0 };
        c += sign * h.powi(n2 as i32) * x.powi(4 * n as i32 + 1) / (fact * (4 * n + 1) as f64);
        let fact1 = fact * (n2 + 1) as f64;
        s += sign * h.powi(n2 as i32 + 1) * x.powi(4 * n as i32 + 3) / (fact1 * (4 * n + 3) as f64);
    }
    (c, s)
}

fn knife_edge(mode: Mode) -> Outcome {
    let mut worst_approx: f64 = 0.0;
    let mut worst_exact: f64 = 0.0;
    let mut nu = -0.7;
    while nu <= 3.0 {
        let eq31 = 6.9 + 20.0 * log10(((nu - 0.1) * (nu - 0.1) + 1.0).sqrt() + nu - 0.1);
        worst_approx = worst_approx.max((knife_edge_loss_db(nu) - eq31).abs());
        let (c, s) = fresnel(nu);
        let exact = -10.0 * log10(((0.5 - c).powi(2) + (0.5 - s).powi(2)) / 2.0);
        let model = knife_edge_loss_exact_db(if mode.faulted() { nu + 0.05 } else { nu });
        worst_exact = worst_exact.max((model - exact).abs());
        nu += 0.05;
    }
    let j0 = knife_edge_loss_exact_db(0.0);
    Outcome::judge(
        worst_approx < 1e-9 && worst_exact < 0.01,
        format!("Eq. 31 max |error| {worst_approx:.2e} dB; exact max |error| {worst_exact:.4} dB; J(0) = {j0:.4} dB"),
    )
}

fn noise_floor(mode: Mode) -> Outcome {
    let kt = 10.0 * log10(BOLTZMANN * 290.0 * 1_000.0);
    let bw = if mode.faulted() { 20e6 } else { 10e6 };
    let phy = OfdmPhy::new(Tier::High).with_noise_figure_db(6.0);
    let reference = kt + 10.0 * log10(10e6) + 6.0;
    let model = if mode.faulted() {
        phy.noise_floor() + 10.0 * log10(bw / 10e6)
    } else {
        phy.noise_floor()
    };
    let sl_err = (cv2x::THERMAL_NOISE_DBM_PER_HZ - kt).abs();
    Outcome::judge(
        (model - reference).abs() < 0.05 && sl_err < 1e-6 && cv2x::UE_NOISE_FIGURE_DB == 9.0,
        format!("802.11p floor {model:.3} dBm vs kTB+NF {reference:.3} dBm; sidelink kT {:.6} vs {kt:.6} dBm/Hz", cv2x::THERMAL_NOISE_DBM_PER_HZ),
    )
}

fn sensitivity(mode: Mode) -> Outcome {
    let etsi = [-91.0, -90.0, -88.0, -86.0, -83.0, -79.0, -75.0, -74.0];
    let ieee = [-85.0, -84.0, -82.0, -80.0, -77.0, -73.0, -69.0, -68.0];
    let mut mismatches = Vec::new();
    for (i, mcs) in Mcs::ALL.iter().enumerate() {
        let model = if mode.faulted() { ieee[i] } else { mcs.sensitivity_static_dbm() };
        if model != etsi[i] {
            mismatches.push(format!("{}: {model} vs {}", mcs.label(), etsi[i]));
        }
    }
    let dynamic = Mcs::R6Qpsk12.sensitivity_dynamic_dbm();
    if dynamic != -85.0 {
        mismatches.push(format!("dynamic 6 Mbit/s {dynamic} vs -85"));
    }
    Outcome::judge(
        mismatches.is_empty(),
        if mismatches.is_empty() {
            "all 8 static rows and the dynamic row match".to_string()
        } else {
            mismatches.join("; ")
        },
    )
}

fn airtime(mode: Mode) -> Outcome {
    let mut wrong = 0usize;
    let mut first = None;
    let mut n = 0usize;
    for mcs in Mcs::ALL {
        for len in 1..=2_304u32 {
            n += 1;
            let tail = if mode.faulted() { 0 } else { 6 };
            let symbols = (16 + 8 * u64::from(len) + 6).div_ceil(u64::from(mcs.data_bits_per_symbol()));
            let reference_ns = (32 + 8 + 8 * symbols) * 1_000;
            let model_ns = if mode.faulted() {
                let s = (16 + 8 * u64::from(len) + tail).div_ceil(u64::from(mcs.data_bits_per_symbol()));
                (32 + 8 + 8 * s) * 1_000
            } else {
                air_time(len, mcs).as_nanos()
            };
            if model_ns != reference_ns {
                wrong += 1;
                first.get_or_insert(format!("{} {len} B: {model_ns} ns vs {reference_ns} ns", mcs.label()));
            }
        }
    }
    let example = air_time(300, Mcs::R6Qpsk12).as_nanos();
    Outcome::judge(
        wrong == 0,
        format!(
            "{wrong} of {n} wrong{}; 300 B at 6 Mbit/s = {} µs",
            first.map(|f| format!(" (first: {f})")).unwrap_or_default(),
            example / 1_000
        ),
    )
}

fn link_budget(mode: Mode) -> Outcome {
    let mut ctx = TestCtx::new(7);
    let mut propagation = FreeSpace::new(Tier::High);
    let mut fading = NoFading::new();
    let los = LosResult::default();
    let w = WeatherState::CLEAR;
    let mut worst: f64 = 0.0;
    for &d in &[10.0, 87.0, 250.0, 640.0] {
        let tx = endpoint(0, 0.0, 0.0);
        let rx = endpoint(1, d, 0.0);
        let budget = v2xw_radio::budget::evaluate(
            &mut ctx, &mut propagation, &mut [], &mut fading, &tx, &rx, 5.9e9, &los, &w, 20.0, 0,
        );
        let fspl = 20.0 * log10(4.0 * core::f64::consts::PI * d * 5.9e9 / C);
        let reference = 20.0 + tx.gain_dbi + rx.gain_dbi - fspl;
        let model = budget.rx_power_dbm + if mode.faulted() { rx.gain_dbi } else { 0.0 };
        worst = worst.max((model - reference).abs());
    }
    Outcome::judge(worst < 1e-9, format!("max |error| {worst:.3e} dB over 4 links (Pt 20 dBm, 3 dBi each end)"))
}

fn rain(mode: Mode) -> Outcome {
    let table = [
        (Polarization::Horizontal, 0.000_705_6, 1.5900),
        (Polarization::Vertical, 0.000_487_8, 1.5728),
    ];
    let mut parts = Vec::new();
    for (pol, k_t, a_t) in table {
        let asked = if mode.faulted() {
            match pol {
                Polarization::Horizontal => Polarization::Vertical,
                Polarization::Vertical => Polarization::Horizontal,
            }
        } else {
            pol
        };
        let (k, a) = prop::p838_coefficients(6e9, asked);
        let ok = crate::rel(k, k_t) < 5e-4 && (a - a_t).abs() < 5e-4;
        parts.push((ok, format!("{pol:?}: k {k:.7} (table {k_t}), α {a:.4} (table {a_t})")));
    }
    Outcome::all(parts)
}
