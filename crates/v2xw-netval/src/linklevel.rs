//! Layer 1, link level: the 802.11p error model's three stages each held to a reference
//! computed here from first principles, sharing no code with `v2xw-radio`.
//!
//! * **PHY-14** — the convolutional code's distance spectrum, recomputed from the 802.11
//!   generator polynomials and puncturing patterns by a trellis search, against the
//!   coefficients the NIST model's union bound carries.
//! * **PHY-15** — the uncoded bit-error formulas against the exact Gray-mapped BER of each
//!   constellation over AWGN, computed by integrating the Gaussian over every decision
//!   region.
//! * **PHY-16** — the packet error rate against a Monte-Carlo link: the frame (SIGNAL and
//!   DATA fields) convolutionally encoded, punctured, sent through the binary symmetric
//!   channel the modulation's exact BER defines, and decoded by a hard-decision Viterbi
//!   decoder written here.

use v2xw_core::math;
use v2xw_radio::types::{CodeRate, Mcs, Modulation};
use v2xw_radio::{PerModel, coded_error_probability, uncoded_ber};

use crate::{Check, Cost, Layer, Mode, Outcome};

/// The checks of this module.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "PHY-14",
            layer: Layer::Physical,
            title: "Coded error probability: the union bound's distance spectrum recomputed from the 802.11 code (133, 171)₈ and its puncturing patterns",
            reference: "IEEE 802.11-2016 §17.3.5.6 (K = 7, g0 = 133₈, g1 = 171₈; rate 2/3 and 3/4 by the bit-stealing patterns of Figures 17-8/17-9); the bound P_b ≤ (1/(2P))·Σ c_d·D^d, D = √(4p(1−p)) (Viterbi 1971; Pei & Henderson 2010 Eq. 3); spectrum by trellis search here, cross-checked by its free distances 10, 6 and 5 (Haccoun & Bégin, IEEE Trans. Commun. 37(11) 1989)",
            tolerance: "relative 1e-3 in the coded error probability at 12 values of p per rate (1e-3 relative is under 0.001 dB of SNR); free distances exact. Rate 1/2 agrees to 1e-15; the published punctured spectra differ from the exhaustive search in their high-order terms (see the findings)",
            fault: "One coefficient transposed in transcription: c_16 = 11 363 for 11 633 at rate 1/2",
            cost: Cost::Fast,
            run: distance_spectrum,
        },
        Check {
            id: "PHY-15",
            layer: Layer::Physical,
            title: "Uncoded bit-error probability per constellation against the exact Gray-mapped BER over AWGN",
            reference: "Exact BER of Gray-coded square M-QAM as two √M-PAM, every decision region integrated (Cho & Yoon, IEEE Trans. Commun. 50(7) 2002); the model's formulas are the nearest-neighbour forms of Pei & Henderson 2010 Table I",
            tolerance: "relative 2 % wherever the exact BER is at most 0.1 (the nearest-neighbour form drops terms that matter only above that, where every coded frame fails); BPSK and QPSK 1e-5 (exact formulas; the limit is the reference erfc's accuracy in the far tail)",
            fault: "16-QAM's symbol energy normalised as 5·d² instead of 10·d² (the argument SNR/5 for SNR/10): a 3 dB error",
            cost: Cost::Fast,
            run: uncoded,
        },
        Check {
            id: "PHY-16",
            layer: Layer::Physical,
            title: "Packet error rate against a Monte-Carlo hard-decision Viterbi link: the SNR of 10 % PER for 100-byte frames at the four BPSK and QPSK rates",
            reference: "The NIST model is a union upper bound on a hard-decision Viterbi decoder (Pei & Henderson 2010 §II; Viterbi 1971), so it must sit at or above the simulated decoder's PER, and Pei & Henderson found it within about 1 dB of measurement. Frame as IEEE 802.11-2016 §17.3.5: SIGNAL 24 bits BPSK 1/2, DATA 16 + 8·bytes + 6 bits; decoder: 64-state hard-decision Viterbi written here, erasures at stolen bits",
            tolerance: "simulated SNR at 10 % PER between 2.0 dB below and 0.3 dB above the model's (the bound is pessimistic by construction; 0.3 dB covers the 400-frame estimate's spread)",
            fault: "The model evaluated with 3 dB of implementation loss (the SNR of 10 % PER moved up 3 dB)",
            cost: Cost::Slow,
            run: viterbi_per,
        },
    ]
}

// ---------------------------------------------------------------------------------------
// The code
// ---------------------------------------------------------------------------------------

const G0: u32 = 0o133;
const G1: u32 = 0o171;

fn parity(x: u32) -> u32 {
    x.count_ones() & 1
}

/// The two coded bits of input `u` in encoder state `state` (the last six inputs, newest
/// in bit 5), and the next state.
fn encode_step(state: u32, u: u32) -> (u32, u32, u32) {
    let reg = (u << 6) | state;
    (parity(reg & G0), parity(reg & G1), reg >> 1)
}

/// Which of (A, B) each phase of the puncturing period transmits.
fn pattern(rate: CodeRate) -> &'static [(bool, bool)] {
    match rate {
        CodeRate::R1_2 => &[(true, true)],
        // A0 B0 A1 (B1 stolen).
        CodeRate::R2_3 => &[(true, true), (true, false)],
        // A0 B0 A1 B2 (B1 and A2 stolen).
        CodeRate::R3_4 => &[(true, true), (true, false), (false, true)],
    }
}

/// The bit-weight distance spectrum `c_d` for `d ≤ d_max`, summed over the error events
/// that start at each phase of the puncturing period: `c[d]` is the total information
/// weight of every path that leaves the all-zero state at some phase and returns to it
/// with output weight `d`.
fn spectrum(rate: CodeRate, d_max: usize) -> Vec<f64> {
    let pat = pattern(rate);
    let period = pat.len();
    let mut c = vec![0.0f64; d_max + 1];
    for phase0 in 0..period {
        // active[(state, w)] = (paths, total information weight), at the current phase.
        let mut active: std::collections::BTreeMap<(u32, usize), (f64, f64)> =
            std::collections::BTreeMap::new();
        // The diverging step: input 1 from state 0.
        let (a, b, next) = encode_step(0, 1);
        let (ta, tb) = pat[phase0];
        let w = usize::from(ta) * a as usize + usize::from(tb) * b as usize;
        if w <= d_max {
            active.insert((next, w), (1.0, 1.0));
        }
        let mut phase = (phase0 + 1) % period;
        for _ in 0..400 {
            if active.is_empty() {
                break;
            }
            let mut nxt: std::collections::BTreeMap<(u32, usize), (f64, f64)> =
                std::collections::BTreeMap::new();
            let (ta, tb) = pat[phase];
            for (&(state, w), &(n, info)) in &active {
                for u in 0..2u32 {
                    let (a, b, ns) = encode_step(state, u);
                    let w2 = w + usize::from(ta) * a as usize + usize::from(tb) * b as usize;
                    if w2 > d_max {
                        continue;
                    }
                    let info2 = info + f64::from(u) * n;
                    if ns == 0 {
                        c[w2] += info2;
                    } else {
                        let e = nxt.entry((ns, w2)).or_insert((0.0, 0.0));
                        e.0 += n;
                        e.1 += info2;
                    }
                }
            }
            active = nxt;
            phase = (phase + 1) % period;
        }
    }
    c
}

fn distance_spectrum(mode: Mode) -> Outcome {
    let mut parts = Vec::new();
    // The bound keeps the first terms from the free distance: nine for rate 1/2 (d to
    // 26), ten for 2/3 (to 15) and 3/4 (to 14).
    for (rate, d_max, d_free_want) in [
        (CodeRate::R1_2, 26usize, 10usize),
        (CodeRate::R2_3, 15, 6),
        (CodeRate::R3_4, 14, 5),
    ] {
        let c = spectrum(rate, d_max);
        let d_free = c.iter().position(|x| *x > 0.0).unwrap_or(0);
        let p_div = 2.0 * f64::from(rate.b_value());
        let mut worst = 0.0f64;
        for k in 0..12 {
            let p = 1e-6 * math::pow(10.0, f64::from(k) * 0.4);
            let d = math::sqrt(4.0 * p * (1.0 - p));
            let mut want = 0.0;
            for (w, cw) in c.iter().enumerate().rev() {
                want += cw * math::pow(d, w as f64);
            }
            let want = (want / p_div).clamp(0.0, 1.0);
            let mut got = coded_error_probability(p, rate);
            if mode.faulted() && rate == CodeRate::R1_2 {
                got = (got + (11_363.0 - 11_633.0) * math::pow(d, 16.0) / p_div).clamp(0.0, 1.0);
            }
            if want > 0.0 && want < 1.0 {
                worst = worst.max(crate::rel(got, want));
            }
        }
        let (n, dn) = rate.as_fraction();
        let shown: Vec<String> = c
            .iter()
            .enumerate()
            .filter(|x| *x.1 > 0.0)
            .take(4)
            .map(|(w, x)| format!("c{w}={x}"))
            .collect();
        parts.push((
            worst < 1e-3 && d_free == d_free_want,
            format!(
                "rate {n}/{dn}: d_free {d_free}, {} …; max rel. error {worst:.1e}",
                shown.join(" ")
            ),
        ));
    }
    Outcome::all(parts)
}

// ---------------------------------------------------------------------------------------
// Uncoded BER
// ---------------------------------------------------------------------------------------

/// `erfc` to about 1.2e-7 relative everywhere (Numerical Recipes 2nd ed. §6.2, `erfcc`,
/// a Chebyshev fit) — an implementation independent of the radio crate's.
fn erfc_nr(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let poly = -z * z - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18
                    + t * (-0.186_288_06
                        + t * (0.278_868_07
                            + t * (-1.135_203_98
                                + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77))))))));
    let r = t * math::exp(poly);
    if x >= 0.0 { r } else { 2.0 - r }
}

fn q(x: f64) -> f64 {
    0.5 * erfc_nr(x / core::f64::consts::SQRT_2)
}

/// The exact BER of Gray-coded square `m²`-QAM (or `m`-PAM per dimension) at symbol SNR
/// `snr` (linear): each dimension is a Gray-coded `m`-PAM, and the probability of landing
/// in every other decision region is integrated exactly.
fn exact_qam_ber(m: usize, snr: f64) -> f64 {
    let gray = |n: usize| n ^ (n >> 1);
    let k = m.trailing_zeros() as f64;
    let levels: Vec<f64> = (0..m).map(|i| (2 * i) as f64 - (m as f64 - 1.0)).collect();
    let es_dim = levels.iter().map(|l| l * l).sum::<f64>() / m as f64;
    // QAM symbol energy 2·es_dim, noise N0/2 per dimension: σ² = es_dim / snr.
    let sigma = math::sqrt(es_dim / snr);
    let mut total = 0.0;
    for (i, l) in levels.iter().enumerate() {
        for (j, c) in levels.iter().enumerate() {
            if i == j {
                continue;
            }
            let upper = if j == m - 1 { f64::INFINITY } else { c + 1.0 };
            let lower = if j == 0 { f64::NEG_INFINITY } else { c - 1.0 };
            let p_lo = if lower.is_infinite() {
                1.0
            } else {
                q((lower - l) / sigma)
            };
            let p_hi = if upper.is_infinite() {
                0.0
            } else {
                q((upper - l) / sigma)
            };
            total += (p_lo - p_hi) * f64::from((gray(i) ^ gray(j)).count_ones());
        }
    }
    total / (m as f64 * k)
}

fn uncoded(mode: Mode) -> Outcome {
    let mut parts = Vec::new();
    for (modulation, m, label) in [
        (Modulation::Bpsk, 2usize, "BPSK"),
        (Modulation::Qpsk, 2, "QPSK"),
        (Modulation::Qam16, 4, "16-QAM"),
        (Modulation::Qam64, 8, "64-QAM"),
    ] {
        let mut worst = 0.0f64;
        let mut at = 0.0;
        for k in 0..=60 {
            let db = -3.0 + f64::from(k) * 0.5;
            let snr = math::pow(10.0, db / 10.0);
            // BPSK carries one bit on one dimension at the full symbol energy: a 2-PAM at
            // twice the per-dimension SNR of QPSK.
            let exact = match modulation {
                Modulation::Bpsk => exact_qam_ber(2, 2.0 * snr),
                _ => exact_qam_ber(m, snr),
            };
            if !(1e-12..=0.1).contains(&exact) {
                continue;
            }
            let model = if mode.faulted() && modulation == Modulation::Qam16 {
                0.75 * 0.5 * erfc_nr(math::sqrt(snr / 5.0))
            } else {
                uncoded_ber(modulation, snr)
            };
            let e = crate::rel(model, exact);
            if e > worst {
                worst = e;
                at = db;
            }
        }
        let tol = if m == 2 { 1e-5 } else { 0.02 };
        parts.push((
            worst <= tol,
            format!("{label}: max rel. error {worst:.2e} (at {at:.1} dB)"),
        ));
    }
    Outcome::all(parts)
}

// ---------------------------------------------------------------------------------------
// Monte-Carlo Viterbi link
// ---------------------------------------------------------------------------------------

/// SplitMix64: the check's own stream, seeded per run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Encodes `bits` (tail included), punctures by `rate`, flips each sent bit with
/// probability `p`, and decodes with a hard-decision Viterbi decoder. True when the
/// decoded block equals the sent one.
fn block_survives(bits: &[u8], rate: CodeRate, p: f64, rng: &mut Rng) -> bool {
    let pat = pattern(rate);
    // Received symbols: per step (A, B) as Some(bit) or None (stolen).
    let mut rx: Vec<(Option<u8>, Option<u8>)> = Vec::with_capacity(bits.len());
    let mut state = 0u32;
    for (i, &u) in bits.iter().enumerate() {
        let (a, b, ns) = encode_step(state, u32::from(u));
        state = ns;
        let (ta, tb) = pat[i % pat.len()];
        let mut send = |bit: u32, sent: bool| {
            sent.then(|| {
                let flip = rng.uniform() < p;
                (bit as u8) ^ u8::from(flip)
            })
        };
        let ra = send(a, ta);
        let rb = send(b, tb);
        rx.push((ra, rb));
    }
    // Viterbi, 64 states, Hamming metric, survivors as one decision bit per state-step.
    const INF: u32 = u32::MAX / 4;
    let mut metric = [INF; 64];
    metric[0] = 0;
    let mut decisions: Vec<[u8; 64]> = Vec::with_capacity(rx.len());
    for &(ra, rb) in &rx {
        let mut next = [INF; 64];
        let mut dec = [0u8; 64];
        for (s, &m) in metric.iter().enumerate() {
            if m >= INF {
                continue;
            }
            for u in 0..2u32 {
                let (a, b, ns) = encode_step(s as u32, u);
                let cost = ra.map_or(0, |r| u32::from(u32::from(r) != a))
                    + rb.map_or(0, |r| u32::from(u32::from(r) != b));
                let cand = m + cost;
                let ns = ns as usize;
                // The predecessor's oldest bit is what the next state drops: record it.
                if cand < next[ns] {
                    next[ns] = cand;
                    dec[ns] = (s & 1) as u8;
                }
            }
        }
        metric = next;
        decisions.push(dec);
    }
    // The tail forces state 0; trace back from it.
    let mut s = 0usize;
    let mut decoded = vec![0u8; bits.len()];
    for i in (0..bits.len()).rev() {
        // The state after step i holds input i in bit 5.
        decoded[i] = ((s >> 5) & 1) as u8;
        let dropped = decisions[i][s] as usize;
        s = ((s << 1) & 0x3F) | dropped;
    }
    decoded == bits
}

/// The simulated PER of `frames` frames of `bytes` at `sinr_db`.
fn simulated_per(mcs: Mcs, bytes: u32, sinr_db: f64, frames: u32, seed: u64) -> f64 {
    let snr = math::pow(10.0, sinr_db / 10.0);
    let ber = |m: Modulation| match m {
        Modulation::Bpsk => exact_qam_ber(2, 2.0 * snr),
        Modulation::Qpsk => exact_qam_ber(2, snr),
        Modulation::Qam16 => exact_qam_ber(4, snr),
        Modulation::Qam64 => exact_qam_ber(8, snr),
    };
    let p_signal = ber(Modulation::Bpsk);
    let p_data = ber(mcs.modulation());
    let mut rng = Rng(seed);
    let n_data = 16 + 8 * bytes as usize + 6;
    let mut failed = 0u32;
    for _ in 0..frames {
        // SIGNAL: 18 information bits and 6 tail zeros.
        let mut signal: Vec<u8> = (0..18).map(|_| (rng.next() & 1) as u8).collect();
        signal.extend([0u8; 6]);
        let mut data: Vec<u8> = (0..n_data - 6).map(|_| (rng.next() & 1) as u8).collect();
        data.extend([0u8; 6]);
        let ok = block_survives(&signal, CodeRate::R1_2, p_signal, &mut rng)
            && block_survives(&data, mcs.code_rate(), p_data, &mut rng);
        if !ok {
            failed += 1;
        }
    }
    f64::from(failed) / f64::from(frames)
}

/// The SNR at which `per(snr)` crosses `target`, by bisection on `[lo, hi]`.
fn crossing(mut lo: f64, mut hi: f64, target: f64, per: impl Fn(f64) -> f64) -> f64 {
    for _ in 0..10 {
        let mid = 0.5 * (lo + hi);
        if per(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn viterbi_per(mode: Mode) -> Outcome {
    let bytes = 100;
    let model = if mode.faulted() {
        PerModel::with_impl_loss_db(3.0)
    } else {
        PerModel::default()
    };
    let mut parts = Vec::new();
    for (k, mcs) in [Mcs::R3Bpsk12, Mcs::R4p5Bpsk34, Mcs::R6Qpsk12, Mcs::R9Qpsk34]
        .into_iter()
        .enumerate()
    {
        let model_snr = model.snr_for_per(bytes, mcs, 0.1);
        // The same frames at every SNR (common random numbers), so the bisection walks a
        // monotone estimate.
        let seed = 0xF1D0 + k as u64;
        let sim_snr = crossing(model_snr - 7.0, model_snr + 2.0, 0.1, |s| {
            simulated_per(mcs, bytes, s, 400, seed)
        });
        let d = sim_snr - model_snr;
        parts.push((
            (-2.0..=0.3).contains(&d),
            format!(
                "{}: 10 % PER at {sim_snr:.2} dB simulated, {model_snr:.2} dB model ({d:+.2} dB)",
                mcs.label()
            ),
        ));
    }
    Outcome::all(parts)
}
