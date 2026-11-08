"""An independent link-budget and packet-error-rate model, for the differential test.

Written from the published equations, not from the Rust code:

* path loss: Friis (ITU-R P.525), TR 37.885 Table 6.2.1-1 (highway LOS, urban LOS, NLOS),
  and the Abbas et al. 2015 dual-slope log-distance law with its Table II rows;
* received power: P_t + G_t + G_r - L;
* noise: the 10 MHz thermal floor of -104 dBm plus the receiver noise figure;
* frame error: the NIST OFDM model of Pei and Henderson (2010) as ns-3's
  NistErrorRateModel evaluates it: uncoded BER per modulation, the Chernoff union bound
  over the convolutional code's distance spectrum (Miller, NIST report, Tables 3.1.1-3.1.3),
  and PER = 1 - (1 - Pe_SIGNAL)^24 (1 - Pe_DATA)^(16 + 8L + 6), the SIGNAL field at
  BPSK 1/2.

Input on stdin: a JSON array of links. Output: a JSON array of
{"rx_dbm", "sinr_db", "per"}.
"""

import json
import math
import sys

ABBAS = {
    "abbas-los-highway": (1.66, 2.88, 66.1, 104.0),
    "abbas-los-urban": (1.81, 2.85, 63.9, 104.0),
    "abbas-olos-urban": (1.93, 2.74, 72.3, 104.0),
}

# (coefficient, exponent of D) per code rate; leading factor 1/(2·bValue).
SPECTRUM = {
    "1/2": (1, [(36, 10), (211, 12), (1404, 14), (11633, 16), (77433, 18), (502690, 20),
                (3322763, 22), (21292910, 24), (134365911, 26)]),
    "2/3": (2, [(3, 6), (70, 7), (285, 8), (1276, 9), (6160, 10), (27128, 11), (117019, 12),
                (498860, 13), (2103891, 14), (8784123, 15)]),
    "3/4": (3, [(42, 5), (201, 6), (1492, 7), (10469, 8), (62935, 9), (379644, 10),
                (2253373, 11), (13073811, 12), (75152755, 13), (428005675, 14)]),
}


def path_loss(model, d, f_hz):
    d = max(d, 1.0)
    fc = f_hz / 1e9
    if model == "friis":
        return 20 * math.log10(4 * math.pi * d * f_hz / 299792458.0)
    if model == "tr37885-highway-los":
        return 32.4 + 20 * math.log10(d) + 20 * math.log10(fc)
    if model == "tr37885-urban-los":
        return 38.77 + 16.7 * math.log10(d) + 18.2 * math.log10(fc)
    if model == "tr37885-nlos":
        return 36.85 + 30 * math.log10(d) + 18.9 * math.log10(fc)
    n1, n2, pl0, db = ABBAS[model]
    if d <= db:
        return pl0 + 10 * n1 * math.log10(d / 10.0)
    return pl0 + 10 * n1 * math.log10(db / 10.0) + 10 * n2 * math.log10(d / db)


def ber(modulation, snr):
    if snr <= 0:
        return 0.5
    if modulation == "bpsk":
        return 0.5 * math.erfc(math.sqrt(snr))
    if modulation == "qpsk":
        return 0.5 * math.erfc(math.sqrt(snr / 2.0))
    if modulation == "16qam":
        return 0.75 * 0.5 * math.erfc(math.sqrt(snr / 10.0))
    return 7.0 / 12.0 * 0.5 * math.erfc(math.sqrt(snr / 42.0))


def coded(p, rate):
    if p <= 0:
        return 0.0
    b, terms = SPECTRUM[rate]
    d = math.sqrt(4 * p * (1 - p))
    pe = sum(c * d ** e for c, e in terms) / (2 * b)
    return min(max(pe, 0.0), 1.0)


def per(sinr_db, modulation, rate, nbytes):
    snr = 10 ** (sinr_db / 10.0)
    pe_sig = coded(ber("bpsk", snr), "1/2")
    pe_dat = coded(ber(modulation, snr), rate)
    psr = (1 - pe_sig) ** 24 * (1 - pe_dat) ** (16 + 8 * nbytes + 6)
    return min(max(1 - psr, 0.0), 1.0)


def main():
    links = json.load(sys.stdin)
    out = []
    for l in links:
        rx = l["pt"] + l["gt"] + l["gr"] - path_loss(l["model"], l["d"], l["f"])
        sinr = rx - (-104.0 + l["nf"])
        out.append({"rx_dbm": rx, "sinr_db": sinr, "per": per(sinr, l["mod"], l["rate"], l["bytes"])})
    json.dump(out, sys.stdout)


if __name__ == "__main__":
    main()
