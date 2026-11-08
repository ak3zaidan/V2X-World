# Hugging Face release: v2x-scms-mbd-mini

**Repository**: https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini (public, CC BY 4.0).

| Hub tag | what | Hub commit |
|---|---|---|
| `v1.1` (current `main`) | the one-run sample `s01`: 42 files, 33.95 MB, uploaded 2026-11-05 | `73ff965bf5` |
| `v1.0` | the seven mini runs r01..r07: 162 files, 114.7 MB, uploaded 2026-11-04 | `a4e9ed9a8e` |

Load an older version with `load_dataset("akzaidan/v2x-scms-mbd-mini", <config>, revision="v1.0")`.

## v1.1 (Hub tag `v1.1`, uploaded 2026-11-05)

### What was uploaded

A **one-run sample**: run `s01` (`scenarios/dataset/v11-sample.yaml`, 60 vehicles, 75 s, split
`train`), assembled with `--version 1.1 --notes tools/dataset/notes/v1.1.json` and no
`--repair-v1-truth`. Release digest (manifest.json) `83dd1d7a3cccbcaa...`. The full multi-run
release (`scenarios/dataset/r1..r11`) follows.

| path | files | MB |
|---|---|---|
| `README.md` | 1 | 0.06 |
| `compat/` | 2 | 18.96 |
| `croissant.json` | 1 | 0.11 |
| `global/` | 3 | 0.19 |
| `local/` | 5 | 5.39 |
| `ml/` | 2 | 5.32 |
| `runs.parquet` | 1 | 0.01 |
| `scenarios/` | 1 | 0.01 |
| `schema/` | 18 | 0.06 |
| `truth/` | 7 | 3.85 |
| `manifest.json` | 1 | 0.01 |

Uploaded with `tools/dataset/hf_upload.py --root <release> --repo akzaidan/v2x-scms-mbd-mini --sync
--no-validate --yes` (one Hub commit; `--sync` deleted the 139 v1.0 files the release does not
hold, so the Hub tree is the release plus the Hub's `.gitattributes`). The token scan ran;
validate_release.py ran beforehand on the same bytes. Checked after upload: every Hub file's size
and LFS SHA-256 match the local release, and the live `README.md` is byte-identical to the
release's (5 `jsonc` blocks, 64 `//` comments in the annotated example rows).

### The card

`tools/dataset/card.py` writes the card. Since v1.1 it writes the example rows in the annotated
style of the hand-edited v1.0 card (one bold line per table, ```jsonc rows with short `//`
comments on the non-obvious fields), and takes a one-line version note and extra limitations
from `tools/dataset/notes/v1.1.json` (`version_note`, `limitations`; an optional `changes` list
becomes a short changelog at the very end). The card is kept short for readers new to the data
(a test holds it under 450 lines): a summary, the three views, how it was generated, one line per
table (the columns are in `schema/<table>.json`), the example rows, the attacks with their
crosswalk codes, the labels, how to load it, the validation result in one line, at most six
limitations, licence and citation. The v1.1 card says it is a one-run sample, names exactly the
run's attacks (ConstPos, DataReplay, DoSDisruptive, EventualStop, FeignedBraking, RandomPos,
RandomSpeed, SlowDrift, Sybil, 3 vehicles each, and the PoisonForwardedReports compromised RSU),
and lists as limitations that the compromised RSU poisons nothing (no report is relayed), that
FeignedBraking and Sybil are never detected in this run, and that one RandomSpeed attacker left
before its window.

2026-11-05: the v1.1 card was trimmed from 967 to 422 lines (YAML header and example rows
byte-identical) and only `README.md` was re-uploaded (Hub commit `1ac0921`; tags not moved, the
`v1.1` tag still names the long card). The release's `manifest.json` still pins the previous
README's SHA-256 and size, so `validate_release.py` on the release now reports a manifest
mismatch on `README.md` until the manifest's README entry and `release_digest_sha256` are
refreshed.

### Validation of the uploaded release

`validate_release.py`: 9 of 9 checks pass (schema, joins, leakage, labels, time, units, splits,
manifest, shortcuts; splits says the held-out and cross-split comparisons are not applicable to a
one-split release). Shortcut audit, limit 0.7: ConstPos 0.54, DataReplay 0.39, EventualStop 0.59,
FeignedBraking 0.51, RandomPos 0.30, RandomSpeed 0.55, SlowDrift 0.50; the rate attacks Sybil
(0.92) and DoSDisruptive (0.98) are exempt. The offline viewer check loads 18/18 config splits.

### Viewer status (from the Hub's datasets-server)

Checked 2026-11-05 18:25 PST on https://datasets-server.huggingface.co: `/splits` lists
18 config/split pairs (one `train` split per config) and 18 of 18 return
`/first-rows`.

- `is-valid`: `{"preview": false, "viewer": true, "search": false, "filter": false, "statistics": false}`

| config | first-rows |
|---|---|
| `local_rx` | ok (100 rows, 38 columns, split train) |
| `global_crl` | ok (18 rows, 9 columns, split train) |
| `global_decisions` | ok (18 rows, 11 columns, split train) |
| `global_reports` | ok (100 rows, 18 columns, split train) |
| `local_checks` | ok (100 rows, 10 columns, split train) |
| `local_ego` | ok (100 rows, 12 columns, split train) |
| `local_pki` | ok (100 rows, 10 columns, split train) |
| `local_reports` | ok (100 rows, 13 columns, split train) |
| `ml_global` | ok (100 rows, 20 columns, split train) |
| `ml_local` | ok (100 rows, 42 columns, split train) |
| `runs` | ok (1 rows, 28 columns, split train) |
| `truth_app_effects` | ok (100 rows, 10 columns, split train) |
| `truth_devices` | ok (65 rows, 16 columns, split train) |
| `truth_pseudonyms` | ok (100 rows, 6 columns, split train) |
| `truth_receptions` | ok (100 rows, 8 columns, split train) |
| `truth_reports` | ok (100 rows, 12 columns, split train) |
| `truth_timeline` | ok (32 rows, 17 columns, split train) |
| `truth_transmissions` | ok (100 rows, 23 columns, split train) |

### Loaded from the Hub with `datasets.load_dataset`

```
global_reports {'train': 1679} 18 cols
truth_timeline {'train': 32} 17 cols
local_rx {'train': 89237} 38 cols
```

### Rebuilding it

```sh
python -I tools/dataset/assemble_release.py <run s01> --out <root> --name v2x-scms-mbd-mini --repo akzaidan/v2x-scms-mbd-mini --version 1.1 --date 2026-11-05 --notes tools/dataset/notes/v1.1.json --validation <validate_release --json output>
python -I tools/dataset/validate_release.py <root>
python tools/dataset/hf_upload.py --root <root> --repo akzaidan/v2x-scms-mbd-mini --sync --yes
```

## v1.0 (Hub tag `v1.0`, uploaded 2026-11-04)

### What was uploaded

The release root assembled by `tools/dataset/assemble_release.py` from the seven mini runs (scenarios/dataset/r1..r7, 75 s, 40 vehicles each): 161 files, 114.7 MB. Release digest (manifest.json) `1f75f07e4a91efe9...`.

| path | files | MB |
|---|---|---|
| `README.md` | 1 | 0.06 |
| `compat/` | 14 | 54.23 |
| `croissant.json` | 1 | 0.11 |
| `global/` | 21 | 0.20 |
| `local/` | 35 | 22.71 |
| `ml/` | 13 | 21.92 |
| `runs.parquet` | 1 | 0.01 |
| `scenarios/` | 7 | 0.03 |
| `schema/` | 18 | 0.06 |
| `truth/` | 49 | 15.29 |
| `manifest.json` | 1 | 0.03 |

Only files under the release root were uploaded (`tools/dataset/hf_upload.py --root <release> --repo akzaidan/v2x-scms-mbd-mini --no-validate --yes`; its token scan ran, and validate_release.py was run separately on the same bytes beforehand).

### Viewer status (from the Hub's datasets-server)

Checked 2026-11-04 10:32 PST on https://datasets-server.huggingface.co: all 18 configs and all 52 config/split
pairs return `/first-rows`; `/splits` lists train/validation/test for every table config (`runs`: train).
Column statistics were still being computed (`statistics: false`) at the time of the check.

- `is-valid`: `{"preview": true, "viewer": true, "search": true, "filter": true, "statistics": false}`

| config | first-rows |
|---|---|
| `local_rx` | ok (100 rows, 38 columns, split train) |
| `local_checks` | ok (100 rows, 10 columns, split train) |
| `local_ego` | ok (100 rows, 12 columns, split train) |
| `local_pki` | ok (100 rows, 10 columns, split train) |
| `local_reports` | ok (100 rows, 13 columns, split train) |
| `global_crl` | ok (9 rows, 9 columns, split train) |
| `global_decisions` | ok (9 rows, 11 columns, split train) |
| `global_reports` | ok (100 rows, 18 columns, split train) |
| `truth_app_effects` | ok (100 rows, 10 columns, split train) |
| `truth_devices` | ok (100 rows, 16 columns, split train) |
| `truth_pseudonyms` | ok (100 rows, 6 columns, split train) |
| `truth_receptions` | ok (100 rows, 8 columns, split train) |
| `truth_reports` | ok (100 rows, 12 columns, split train) |
| `truth_timeline` | ok (34 rows, 17 columns, split train) |
| `truth_transmissions` | ok (100 rows, 23 columns, split train) |
| `ml_global` | ok (100 rows, 20 columns, split train) |
| `ml_local` | ok (100 rows, 42 columns, split train) |
| `runs` | ok (7 rows, 28 columns, split train) |

### Loaded from the Hub with `datasets.load_dataset`

```
global_reports {'train': 523, 'validation': 69, 'test': 195} 18 cols
truth_timeline {'train': 34, 'validation': 7, 'test': 18} 17 cols
local_rx {'train': 187859, 'validation': 59125, 'test': 100632} 38 cols
```

### Rebuilding it

```sh
python -I tools/dataset/assemble_release.py <runs r01..r07> --out <root> --name v2x-scms-mbd-mini --repo akzaidan/v2x-scms-mbd-mini --date 2026-11-04 --validation <validate_release --json output>
python -I tools/dataset/validate_release.py <root>
python tools/dataset/hf_upload.py --root <root> --repo akzaidan/v2x-scms-mbd-mini --yes
```
### Validation of the uploaded release

`validate_release.py`: 8 of 9 checks pass (schema, joins, leakage, labels, time, units, splits, manifest).
The shortcut audit fails: EventualStop ROC-AUC 0.73 and PosHeadingInconsistent ROC-AUC 0.74 from metadata alone
(limit 0.7); the card states it. The offline viewer check (`load_dataset("parquet", ...)` per config and split)
loads 52/52 config splits.

### Repairs made at assembly

`assemble_release.repair_truth` repaired three v1 writer defects in `truth/` (r03 NotYetValid duplicate
tx_ids, 24 reception labels, device attack windows ending ~9 ms before the last falsified message's air time);
the card lists them run by run. The empty `ml/global` partition of r06 (no MA reports) is not written, and was
deleted from the Hub after the first upload.

## From v1.0 to v1.1: the writer fixed at the source (2026-11-04)

The three repairs above, the empty `local/reports.evidence_spdu_digests`, `evidence_start_ns ==
evidence_end_ns` on every report, the 1024 VeReMi sentinel in `truth/` and `ml/`, and the stale
`runs.git_sha` (worktree builds) are fixed in the exporter itself (before the v1.1 recording; tests in
`crates/v2xw-record/src/dataset/release/tests.rs` and `crates/v2xw-engine/tests/dataset_records.rs`).
`assemble_release.py` now only checks for the v1.0 defects and refuses an input that has them, unless
`--repair-v1-truth`.

**v1.1 recorded (2026-11-05): a one-run sample.** v1.1 is published as one run,
`s01` (`scenarios/dataset/v11-sample.yaml`, split `train`), and the eleven-run design
(`scenarios/dataset/r1-...` to `r11-...`) is kept for the full release. `s01` is the dataset base (60
vehicles, 75 s) with the ten main attacks on three vehicles each, at varied start times, and faulty
honest devices; DoS, FakeHazard and ReportPoisoning run as their nearest runnable stand-ins, and the
ReportPoisoning stand-in (a compromised roadside unit) poisons nothing because no report is relayed.
Recorded with a release build of the fixed exporter, assembled with `--version 1.1` and without
`--repair-v1-truth`: `validate_release.py` passes 9 of 9 (splits reports the held-out and
cross-split comparisons as not applicable), the shortcut audit's highest non-rate score is
EventualStop 0.59, and the offline viewer check loads 18/18 config splits.

**Shortcut audit, looked at (v1.0 release).** The 0.73 / 0.74 are not an engine artefact: the
attackers' rate (`gap_ms` alone AUC 0.49), `msg_cnt` (0.48) and certificate/digest signer mix (0.43; 80/20
as for honest vehicles) match the honest fleet. The model leans on the per-pseudonym exposure features
(`cert_span_s`, `cert_rows`, `cert_rate_hz`: 0.93 of the importance for EventualStop; `cert_rows` alone 0.57, `cert_span_s` 0.56) and `size_bytes`
(alone 0.81 for EventualStop). The size is the BSM's own Part II path history, which grows over a
vehicle's life (196 B just after spawn; 204-238 B later); every attacker's falsified messages come after
its attack start, late in its life, while the honest set includes newly joined vehicles. The audited
classes are tiny: EventualStop is 2 attacker devices (4 pseudonyms) in r02 and r05, PosHeadingInconsistent
1 device in r02, so a 5-fold grouped AUC has 2-4 positive groups. Small-sample and time-in-life confound;
the check is left as is (limit 0.7). A larger tier with more attackers per class, or matching the honest
set on time since spawn, is the way to settle it.
