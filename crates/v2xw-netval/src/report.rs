//! The generated report, `docs/validation/network.md`.

use crate::{Check, Layer, Status, Verdict};

/// Defects this suite found, what was done about them, and where. Maintained by hand
/// beside the checks that found them: the checks prove the current state, this list
/// records the history a reader of the report needs.
pub const FINDINGS: &[(&str, &str, &str)] = &[
    (
        "ACC-03",
        "The EDCA backoff countdown restarted from the full draw after every busy period instead of resuming with the slots still owed (EN 302 663 Annex C.4.2 'suspend the countdown'). `EdcaOcbMac::on_cca(Busy)` froze without counting the slots already elapsed, and the engine reported the busy medium only when the node's own timer fired, so the MAC could not know when the medium had gone busy. Effect: longer channel-access delay and a contention process unlike 802.11's under load.",
        "Fixed: `on_cca(Busy)` counts the whole slots elapsed before it freezes, a new `EdcaOcbMac::freeze_at` applies the instant the medium actually went busy, and the engine computes that instant from the arrivals in flight (`Engine::medium_busy_since`).",
    ),
    (
        "ACC-06",
        "The reactive DCC state machine jumped straight to the state the CBR mapped to; ETSI TS 102 687 V1.2.1 §5.3 says 'one state can only be reached by a neighbouring state'.",
        "Fixed: `ReactiveDcc::on_cbr` moves one state towards the target per evaluation.",
    ),
    (
        "ACC-09",
        "NR-V2X's reselection-counter interval used a floor where TS 38.321 §5.22.1.1 prints a ceiling, ⌈100/max(20, RRI)⌉: at RRI 30, 40, 60-90 ms the counter range was too short (e.g. [5, 15] instead of [10, 30] at 60 ms).",
        "Fixed in `Rri::c_resel_range`.",
    ),
    (
        "PHY-09",
        "The sidelink thermal-noise density `cv2x::THERMAL_NOISE_DBM_PER_HZ` was −173.9750935 dBm/Hz; its own documented formula, 10·log10(k·290·1000) with the SI-exact k, gives −173.9751872. A 9.4e-5 dB error, physically negligible, but a constant that disagrees with the formula printed above it.",
        "Fixed: the constant is the formula's value to nine decimals.",
    ),
    (
        "SEC-01",
        "The real crypto backend refuses high-s ECDSA signatures (`(r, n − s)`), which FIPS 186-4 and IEEE 1609.2 both accept; OpenSSL emits high s about half the time, so 14 of 32 OpenSSL signatures were refused. This is a deliberate policy (crates/v2xw-sec/src/crypto.rs: refusing the malleated form keeps verdicts independent of crypto mode, invariant I-S1), so it is not changed here; it does mean a simulated receiver would reject a real third-party OBU that signs with high s.",
        "Recorded, not changed. SEC-01 now sends OpenSSL's signatures in their low-s form (the same signature to any ECDSA verifier) and separately confirms every raw high-s form is refused, so the policy is visible in the report.",
    ),
    (
        "ACC-04 (suite)",
        "The suite's own reference was wrong at high load: Bianchi's Eq. 7 rests on a decoupling approximation that overstates broadcast collisions at small W and large N (0.907 against 0.871 for the exact slotted process at N = 20, W = 16), so the check failed a MAC that was right.",
        "Fixed in the check: the slotted process Eq. 7 models is simulated exactly, independently of the MAC, and held at every N within 0.02; Eq. 7 itself is held where it is accurate (N ≤ 10).",
    ),
    (
        "SEC-03 (suite)",
        "The check signed period 8's SPDU with a fresh backend that did not hold the signing key, so it panicked before it judged anything.",
        "Fixed in the check: period 8's PKI keeps its own backend.",
    ),
    (
        "PHY-14",
        "The union bound's distance-spectrum coefficients for the punctured rates (copied from ns-3's `NistErrorRateModel`, after Haccoun & Bégin 1989) differ from an exhaustive trellis search of the 802.11 code with its 802.11 puncturing patterns in their high-order terms: rate 2/3 c13–c15 498 860 / 2 103 891 / 8 784 123 against 498 835 / 2 103 480 / 8 781 268, rate 3/4 c10–c14 379 644 … 428 005 675 against 379 546 … 427 474 864. The search reproduces every rate-1/2 coefficient exactly and every lower-order punctured one. The effect on the coded error probability is at most 6e-4 relative, under 0.001 dB.",
        "Recorded, not changed: below any physical significance, and the published values are what other simulators use. PHY-14 holds the model within 1e-3 relative.",
    ),
    (
        "PHY-16",
        "Not a defect: the NIST model is 1.2–1.8 dB pessimistic against a hard-decision Viterbi decoder at 10 % PER (100-byte frames, BPSK and QPSK), as a union bound must be. A soft-decision receiver gains about 2 dB more, so against real hardware the model's ideal-receiver optimism (Sjöberg et al., ~5 dB) and the bound's pessimism partly cancel.",
        "Recorded for readers choosing `rx_impl_loss_db`.",
    ),
    (
        "NET-03",
        "Every message is signed under one PSID, 0x20 (`v2xw_node::secure::PSID_SAFETY`), including ETSI CAMs and DENMs (ITS-AID 36 and 37 in TS 102 965 Table A.1) and SPATEM/MAPEM (137, 138). A receiver filtering on ITS-AID, and the SPDU size (a 2-octet ITS-AID for 137/138), differ from a real stack.",
        "Open, recorded for the V2X-applications track that owns `crates/v2xw-node` this round: the PSID must follow the message type and the envelope profile.",
    ),
];

fn status(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Fail => "fail",
        Status::Skip => "skip",
    }
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// Renders the report.
#[must_use]
pub fn render(results: &[(Check, Verdict)], commit: &str, quick: bool) -> String {
    let mut out = String::new();
    let total = results.len();
    let valid = results.iter().filter(|r| r.1.valid()).count();
    let defects = results.iter().filter(|r| r.1.defect()).count();
    let toothless = results.iter().filter(|r| r.1.toothless()).count();
    let skipped = results.iter().filter(|r| r.1.skipped()).count();
    out.push_str("# Network validation\n\n");
    out.push_str(
        "Generated by `cargo run -p v2xw-netval --bin netval-report` — do not edit by hand.\n\n",
    );
    out.push_str(&format!("Commit: `{commit}`{}.\n\n", if quick { " (quick run: whole-run checks left out)" } else { "" }));
    out.push_str(
        "Every check runs twice. The **honest** run measures the model as shipped against the reference \
         and must pass. The **faulted** run injects the named fault — a misread table row, a rounded \
         constant, a wrong parameter — and must fail; a check that passes with its fault in place cannot \
         tell a right model from a wrong one and is reported as `CANNOT FAIL`. The independent \
         references in `crates/v2xw-netval/reference/` are Python written from the standards, sharing \
         no code with the crates; OpenSSL is reached through Python's `cryptography` package.\n\n",
    );
    out.push_str(&format!(
        "**{valid} of {total} checks pass and are proved able to fail.** {defects} fail (a defect in the model), \
         {toothless} cannot fail, {skipped} skipped.\n\n"
    ));
    for layer in Layer::ALL {
        let rows: Vec<&(Check, Verdict)> = results.iter().filter(|r| r.0.layer == layer).collect();
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!("## {}\n\n", layer.title()));
        out.push_str("| ID | Result | Check | Reference | Tolerance | Measured | Fault injected | Faulted run |\n");
        out.push_str("|---|---|---|---|---|---|---|---|\n");
        for (c, v) in rows {
            out.push_str(&format!(
                "| {} | **{}** | {} | {} | {} | {} | {} | {}: {} |\n",
                c.id,
                v.label(),
                cell(c.title),
                cell(c.reference),
                cell(c.tolerance),
                cell(&v.honest.measured),
                cell(c.fault),
                status(v.faulted.status),
                cell(&v.faulted.measured),
            ));
        }
        out.push('\n');
    }
    out.push_str("## Defects found by this suite\n\n");
    out.push_str("| Found by | Defect | Resolution |\n|---|---|---|\n");
    for (id, what, done) in FINDINGS {
        out.push_str(&format!("| {id} | {} | {} |\n", cell(what), cell(done)));
    }
    out.push_str(
        "\n## What this suite does not yet cover\n\n\
         - IEEE 1609.2.1 linkage-value test vectors: the standard's own vectors were not obtainable; the \
           linkage arithmetic is pinned instead to the CAMP reference implementation's outputs \
           (`crates/v2xw-sec/tests/legacy_vectors.rs`), and SEC-03 checks the revocation decisions it drives.\n\
         - The J2735 SPaT and MAP encoders against an independent decoder: the BSM's Part I has one \
           (NET-05) and the ETSI CAM, DENM and VAM have asn1tools (NET-06, which needs `pip install \
           asn1tools` or `NETVAL_PYTHON` naming an interpreter that has it, and otherwise reports a skip); \
           the J2735 ASN.1 modules are not in this repository.\n\
         - The SAE J2945/1 CAMP worked examples: the CAMP report was not obtainable; ACC-08 holds the \
           equations to J2945/1's published constants (Ahmad et al. 2019).\n\
         - LTE-V2X and NR-V2X block-error curves against the 3GPP link-level results: the crate pins its \
           lookup tables to the published WiLabV2Xsim and Lusvarghi et al. curves in its own tests; this \
           suite does not re-derive them.\n\
         - A revoked sender's messages refused after CRL installation in a whole run: covered by \
           `crates/v2xw-engine/tests/security_lifecycle.rs` and `phase2.rs`, too long for this suite's \
           short runs.\n",
    );
    out
}
