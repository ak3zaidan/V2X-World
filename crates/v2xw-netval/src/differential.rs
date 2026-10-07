//! The differential test: the radio crate's link budget and frame-error model against an
//! independent Python implementation (`reference/linkbudget.py`) on random links.

use serde_json::{Value, json};
use v2xw_core::card::Tier;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_radio::per::PerModel;
use v2xw_radio::phy::OfdmPhy;
use v2xw_radio::prop::{self, LogDistancePreset};
use v2xw_radio::types::{CodeRate, Mcs, Modulation, timing};

use crate::{Check, Cost, Layer, Mode, Outcome};

/// The checks of this module.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![Check {
        id: "E2E-06",
        layer: Layer::EndToEnd,
        title: "Differential: received power, SINR and packet error rate on 600 random links against an independent Python link-budget and PER model",
        reference: "reference/linkbudget.py, written from Friis, TR 37.885 Table 6.2.1-1, Abbas et al. 2015 Table II, and the NIST OFDM error model as ns-3's NistErrorRateModel evaluates it (Pei and Henderson 2010)",
        tolerance: "received power and SINR 1e-9 dB; PER 1e-9 absolute + 1e-7 relative",
        fault: "The PLCP SIGNAL field decoded at the data MCS instead of BPSK 1/2",
        cost: Cost::Fast,
        run: differential,
    }]
}

const MODELS: [&str; 7] = [
    "friis",
    "tr37885-highway-los",
    "tr37885-urban-los",
    "tr37885-nlos",
    "abbas-los-highway",
    "abbas-los-urban",
    "abbas-olos-urban",
];

fn path_loss(model: &str, d: f64, f: f64) -> f64 {
    let fc = f / 1e9;
    match model {
        "friis" => prop::friis_loss_db(d, f),
        "tr37885-highway-los" => prop::tr37885_highway_los_db(d, fc),
        "tr37885-urban-los" => prop::tr37885_urban_los_db(d, fc),
        "tr37885-nlos" => prop::tr37885_nlos_db(d, fc),
        "abbas-los-highway" => LogDistancePreset::AbbasLosHighway.params().path_loss_db(d),
        "abbas-los-urban" => LogDistancePreset::AbbasLosUrban.params().path_loss_db(d),
        _ => LogDistancePreset::AbbasOlosUrban.params().path_loss_db(d),
    }
}

fn names(mcs: Mcs) -> (&'static str, &'static str) {
    let m = match mcs.modulation() {
        Modulation::Bpsk => "bpsk",
        Modulation::Qpsk => "qpsk",
        Modulation::Qam16 => "16qam",
        Modulation::Qam64 => "64qam",
    };
    let r = match mcs.code_rate() {
        CodeRate::R1_2 => "1/2",
        CodeRate::R2_3 => "2/3",
        CodeRate::R3_4 => "3/4",
    };
    (m, r)
}

fn differential(mode: Mode) -> Outcome {
    let mut rng = RngStream::derive(0xD1FF, RngDomain::plugin("netval"), EntityRef::Global);
    let mut links = Vec::new();
    let mut ours = Vec::new();
    for _ in 0..600 {
        let model = MODELS[rng.below(MODELS.len() as u64) as usize];
        let d = 10f64.powf(rng.uniform(0.0, 3.3));
        let f = [5.860e9, 5.890e9, 5.900e9, 5.920e9][rng.below(4) as usize];
        let pt = rng.uniform(0.0, 33.0);
        let (gt, gr) = (rng.uniform(0.0, 6.0), rng.uniform(0.0, 6.0));
        let nf = [6.0, 9.0][rng.below(2) as usize];
        let mcs = Mcs::ALL[rng.below(8) as usize];
        let bytes = 20 + rng.below(1_500) as u32;
        let rx = pt + gt + gr - path_loss(model, d, f);
        let sinr = rx - OfdmPhy::new(Tier::High).with_noise_figure_db(nf).noise_floor();
        let per_model = PerModel::default();
        let per = if mode.faulted() {
            let bits = (timing::SIGNAL_BITS as u64 + 16 + 8 * u64::from(bytes) + 6) as f64;
            1.0 - PerModel::survival(per_model.pe_data(mcs, sinr), bits)
        } else {
            per_model.per(bytes, mcs, sinr)
        };
        let (m, r) = names(mcs);
        links.push(json!({"model": model, "d": d, "f": f, "pt": pt, "gt": gt, "gr": gr, "nf": nf, "mod": m, "rate": r, "bytes": bytes}));
        ours.push((rx, sinr, per));
    }
    let theirs = match crate::python("linkbudget.py", &Value::Array(links)) {
        Ok(v) => v,
        Err(e) => return Outcome::skip(e),
    };
    let rows = theirs.as_array().cloned().unwrap_or_default();
    let (mut worst_rx, mut worst_per, mut bad) = (0f64, 0f64, 0usize);
    let mut first = None;
    let mut graded = 0;
    for (i, (r, (rx, sinr, per))) in rows.iter().zip(&ours).enumerate() {
        let (trx, tsinr, tper) = (
            r["rx_dbm"].as_f64().unwrap_or(f64::NAN),
            r["sinr_db"].as_f64().unwrap_or(f64::NAN),
            r["per"].as_f64().unwrap_or(f64::NAN),
        );
        worst_rx = worst_rx.max((rx - trx).abs()).max((sinr - tsinr).abs());
        let dp = (per - tper).abs();
        worst_per = worst_per.max(dp);
        if *per > 1e-6 && *per < 1.0 - 1e-6 {
            graded += 1;
        }
        if (rx - trx).abs() > 1e-9 || (sinr - tsinr).abs() > 1e-9 || dp > 1e-9 + 1e-7 * tper.abs() {
            bad += 1;
            first.get_or_insert(format!("link {i}: PER {per:.6e} vs {tper:.6e} at {sinr:.2} dB"));
        }
    }
    Outcome::judge(
        bad == 0 && rows.len() == ours.len(),
        format!(
            "{} links ({graded} in the PER transition region): max |Δ power| {worst_rx:.2e} dB, max |Δ PER| {worst_per:.2e}{}",
            rows.len(),
            first.map(|f| format!("; {bad} out of tolerance, first {f}")).unwrap_or_default()
        ),
    )
}
