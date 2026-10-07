//! The statistics the goodness-of-fit checks need, written from the textbook definitions.
//!
//! Deliberately independent of the crates under test: every transcendental here is the
//! platform's `f64` method (the radio crate uses the `libm` crate through
//! `v2xw_core::math`), and the special functions are the classical series and continued
//! fractions [Press et al., *Numerical Recipes*, 3rd ed., §6.1-6.2, §6.14, §14.3]. A
//! reference that shared code with the model it judges would agree with it by
//! construction.

/// `ln Γ(x)` for `x > 0`, by the Lanczos approximation (g = 7, n = 9), accurate to about
/// 1e-15 relative over the range the tests use.
#[must_use]
pub fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflection.
        let pi = core::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + G + 0.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * core::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularised lower incomplete gamma function `P(a, x)`.
#[must_use]
pub fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        // Series.
        let mut sum = 1.0 / a;
        let mut del = sum;
        let mut ap = a;
        for _ in 0..10_000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        (sum.ln() - x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        1.0 - gamma_q_cf(a, x)
    }
}

/// `Q(a, x) = 1 − P(a, x)` by Lentz's continued fraction (valid for `x ≥ a + 1`).
fn gamma_q_cf(a: f64, x: f64) -> f64 {
    let tiny = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / tiny;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..10_000 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < tiny {
            d = tiny;
        }
        c = b + an / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x + a * x.ln() - ln_gamma(a)).exp() * h
}

/// The error function, as `sign(x)·P(1/2, x²)`.
#[must_use]
pub fn erf(x: f64) -> f64 {
    let p = gamma_p(0.5, x * x);
    if x < 0.0 { -p } else { p }
}

/// The complementary error function. For large positive `x` it is computed from the
/// continued fraction directly, so it keeps its relative precision in the tail where
/// `1 − erf(x)` would cancel to zero.
#[must_use]
pub fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    let x2 = x * x;
    if x2 < 1.5 {
        1.0 - gamma_p(0.5, x2)
    } else {
        gamma_q_cf(0.5, x2)
    }
}

/// The standard normal CDF.
#[must_use]
pub fn normal_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / core::f64::consts::SQRT_2)
}

/// The Gamma(shape `k`, scale `θ`) CDF.
#[must_use]
pub fn gamma_cdf(x: f64, k: f64, theta: f64) -> f64 {
    gamma_p(k, x / theta)
}

/// The one-sample Kolmogorov-Smirnov statistic `D` of `samples` against `cdf`.
#[must_use]
pub fn ks_statistic(samples: &[f64], cdf: impl Fn(f64) -> f64) -> f64 {
    let mut s: Vec<f64> = samples.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len() as f64;
    let mut d: f64 = 0.0;
    for (i, x) in s.iter().enumerate() {
        let f = cdf(*x);
        let lo = i as f64 / n;
        let hi = (i + 1) as f64 / n;
        d = d.max((f - lo).abs()).max((hi - f).abs());
    }
    d
}

/// The asymptotic p-value of a KS statistic `d` from `n` samples, with Stephens'
/// small-sample correction `λ = (√n + 0.12 + 0.11/√n)·d`
/// [Numerical Recipes §14.3.3].
#[must_use]
pub fn ks_p_value(d: f64, n: usize) -> f64 {
    let sn = (n as f64).sqrt();
    let lambda = (sn + 0.12 + 0.11 / sn) * d;
    if lambda < 1e-3 {
        return 1.0;
    }
    let mut sum = 0.0;
    let mut sign = 1.0;
    for j in 1..=200 {
        let jf = j as f64;
        let term = sign * 2.0 * (-2.0 * jf * jf * lambda * lambda).exp();
        sum += term;
        if term.abs() < 1e-14 {
            break;
        }
        sign = -sign;
    }
    sum.clamp(0.0, 1.0)
}

/// Pearson's χ² statistic of observed counts against expected counts, and its p-value
/// for `bins − 1 − fitted` degrees of freedom.
#[must_use]
pub fn chi_square(observed: &[u64], expected: &[f64], fitted: usize) -> (f64, f64) {
    let chi2: f64 = observed
        .iter()
        .zip(expected)
        .map(|(o, e)| {
            let d = *o as f64 - e;
            d * d / e
        })
        .sum();
    let dof = (observed.len() - 1 - fitted) as f64;
    let p = 1.0 - gamma_p(dof / 2.0, chi2 / 2.0);
    (chi2, p)
}

/// The sample mean.
#[must_use]
pub fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// The sample standard deviation (n − 1).
#[must_use]
pub fn std_dev(xs: &[f64]) -> f64 {
    let m = mean(xs);
    let v = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (xs.len() as f64 - 1.0);
    v.sqrt()
}

/// The lag-1 sample autocorrelation of paired observations `(x_t, x_{t+1})`.
#[must_use]
pub fn correlation(pairs: &[(f64, f64)]) -> f64 {
    let n = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for (x, y) in pairs {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
    }
    sxy / (sxx * syy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_functions_match_tabulated_values() {
        // Abramowitz and Stegun Table 7.1: erf(0.5) = 0.5204998778, erf(1) = 0.8427007929.
        assert!((erf(0.5) - 0.520_499_877_8).abs() < 1e-9);
        assert!((erf(1.0) - 0.842_700_792_9).abs() < 1e-9);
        // erfc(3) = 2.209049699858544e-5.
        assert!((erfc(3.0) / 2.209_049_699_858_544e-5 - 1.0).abs() < 1e-9);
        // Γ(5) = 24.
        assert!((ln_gamma(5.0) - 24f64.ln()).abs() < 1e-12);
        // P(1, x) = 1 − e^{−x}.
        assert!((gamma_p(1.0, 2.0) - (1.0 - (-2f64).exp())).abs() < 1e-12);
        // χ²(1 dof) at 3.841 has p = 0.05.
        assert!(((1.0 - gamma_p(0.5, 3.841_458_820_694_124 / 2.0)) - 0.05).abs() < 1e-9);
    }
}
