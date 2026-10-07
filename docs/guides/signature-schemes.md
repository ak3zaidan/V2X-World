# Signature schemes: add one and see what it costs

A protocol study often asks one question: *what if the messages were signed differently?*
The simulator answers it in three steps — register the scheme, run the comparison, read the
signing-overhead metrics.

## 1. Compare schemes that already exist

```sh
v2xw compare-signatures scenarios/compare-signature-schemes.yaml \
    --schemes ecdsa-p256,ml-dsa-44,falcon-512,hybrid-mldsa44-ecdsa-p256 \
    --duration 60 --out runs/signatures
```

Every scheme runs the **same scenario with the same seed**: the same traffic, the same channel
draws, the same credential calendar. Only `security.signature` changes. ECDSA P-256 is
always included, because every difference is measured against it. The command writes
these files to `--out`:

| File | What it holds |
|---|---|
| `comparison.md` | the side-by-side table: differences against ECDSA P-256 in brackets, large ones in bold |
| `comparison.csv` | the same table in long form, one line per (quantity, scheme), for a plotting script |
| `comparison.json` | everything, including health levels; the Studio opens this file |
| `runs/c000N-…/` | each run's metrics, report and manifest, so any number can be traced back |

Any scenario works. `v2xw compare-signatures` turns on the signing-overhead group in every
run and refuses a scheme that the scenario's OBU hardware profile has no cost for. It does
not invent a cost. `obu/generic-automotive-soc-no-hsm` prices every registered scheme.

**In the Studio:** open the metrics dashboard (press M) and go to **Compare signature
schemes**. Tick the schemes, set the simulated seconds, and press **Run comparison**. The
engine must be started with `v2xw serve`, which hosts `/agent/compare-signatures`. The
comparison runs the scenario that is on screen. When it finishes, the page shows:

* the schemes' sizes and where each one runs;
* bar charts of the main quantities;
* the full table, with differences against ECDSA P-256 coloured as better or worse.

**Export CSV** and **Export Markdown** save the same files the CLI writes. **Open
comparison.json** loads a comparison that was run from the command line.

## 2. The signing-overhead metrics

To turn the group on in an ordinary run, use `metrics: [all, signing]`. A bare `all` does not
include it, so runs recorded before the group existed keep their digests. The definitions
live in `crates/v2xw-metrics/src/signing.rs`. Each metric has a unit, a definition, a source
and what it leaves out, and the main ones have a health band.

| Question | Metrics |
|---|---|
| How many bytes does signing add? | `sig_signature_bytes`, `sig_signature_rate`, `sig_signer_id_bytes`, `sig_cert_bytes` |
| How much of the air is it? | `sig_cert_airtime_share`, `sig_signer_id_airtime_share`, `sig_security_airtime_share` (with certificates), `sig_security_airtime_share_nocert` (as if no certificate were attached) |
| How long do signing and verifying take, and on which chip? | `sig_sign_latency`, `sig_verify_latency` (p50, p95; labelled `scheme@hsm`, `@accelerator` or `@cpu`), `sig_sign_wait` |
| Can the devices keep up? | `sig_signer_util(_max)`, `sig_verifier_util(_max)`, `sig_verify_capacity`, `sig_neighbour_capacity` (how many neighbours a device can verify before its queue grows), `sig_verify_headroom` |
| What is lost or late because of it? | `sig_verify_dropped` (verification queue full), `sig_verify_late` (over 100 ms) |
| Do large signatures fragment? | `sig_fragments_per_msg`, `sig_fragmented_share`, `sig_reassembly_loss` |
| What does it do to latency and the channel? | `sig_e2e_added`, `sig_e2e_added_share`, `sig_cbr_extra` (an estimate; the comparison's measured CBR row is the observed difference) |
| What does the backend pay? | `sig_backend_bytes` (credential bytes to and from devices), `sig_ca_sign_rate`, `sig_ca_busy` |

## 3. Add a scheme

`docs/design/extending-crypto.md` §2 is the full worked example. In short:

1. Copy `crates/v2xw-sec/src/scheme/template.rs` to a new module and fill in the `const`: the
   parts (primitives) in wire order, the mode, the standard and the notes.
2. If the primitive is new, the module also declares its sizes and its cited sign, verify
   and keygen costs (`SignatureScheme::defines`).
3. Add one line to `SIGNATURE_SCHEMES` in `crates/v2xw-sec/src/scheme/mod.rs`.
4. Run it: `v2xw compare-signatures <scenario> --schemes ecdsa-p256,<your-id>`.

The scenario loader, the settings window, the comparison command and the signing-overhead
metrics all read the registry, so nothing else needs to change. The Studio's tick-list is
`SIGNATURE_SCHEMES` in `ui/apps/studio/src/metrics/sigCompareModel.ts`. Add the new id there
as well so the button can offer it.

## How to read the numbers

* A comparison is **one realisation per scheme**. The seed is shared, so a difference is the
  scheme's effect on that traffic. No confidence interval is claimed. For intervals, sweep
  `security.signature` with several seeds through `v2xw experiment run`.
* Percentiles are the sample-weighted mean of each metric window's percentile, not the
  percentile of every message in the run.
* Sizes come from the scheme registry and costs from the hardware profile. Post-quantum
  arithmetic is modelled, not executed (`crypto_mode: real` runs ECDSA P-256 only).
* Where an operation runs matters. On an HSM profile, a slow secure element signs and a fast
  accelerator verifies, so verifying can be cheaper than signing. On a software-only
  profile, verifying ECDSA costs about twice as much as signing. Every comparison table lists
  the engine for each scheme.
