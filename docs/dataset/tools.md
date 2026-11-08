# Dataset release tools

The Python tools in `tools/dataset/` turn per-run exporter outputs into a release that
follows [schema.md](schema.md), check it, and publish it. They share no code with the Rust
writer, so they are an independent check of it. Python 3.10+ with pyarrow, pandas,
scikit-learn and PyYAML; `huggingface_hub` only for the upload; `mlcroissant` optional.
Run them with `python -I`.

| Tool | Does |
|---|---|
| `make_fixture.py` | writes a tiny synthetic two-run release (or per-run folders) that follows schema.md column for column |
| `assemble_release.py` | merges per-run `dataset/` folders into one release root |
| `validate_release.py` | the release checks, one verdict line each, exit 1 on any failure |
| `hf_upload.py` | dry run by default; uploads to Hugging Face only with `--yes` |
| `tests/` | pytest: every check fails on its own injected fault |

## 1. Export the runs

Each run must name the dataset exporter in its scenario, so the run writes
`<run-dir>/dataset/` (tables partitioned `run=<id>/`, `runs.parquet`, `schema/`,
`compat/veremi/`, `scenario.yaml`, `manifest.json`). Without the engine, make the same shape
from the fixture:

```sh
python -I tools/dataset/make_fixture.py --out /tmp/fx-runs --layout runs
```

## 2. Assemble

```sh
python -I tools/dataset/assemble_release.py /tmp/fx-runs/r0001 /tmp/fx-runs/r0002 \
    --out /tmp/v2x-scms-mbd-mini --name v2x-scms-mbd-mini --repo <org>/v2x-scms-mbd-mini
```

- Every input file is checked against its run's `manifest.json` first (`--no-verify` skips).
- The split comes from each run's `runs.parquet`; `--split r0002=validation` overrides it.
  Splits are always by run.
- Two runs whose `schema/<table>.json` differ are refused: re-export them with one engine.
- Writes the partitions under `split=<s>/run=<id>/`, `runs.parquet`, `ml/` (the joined
  training copies; `--no-ml` leaves them out), `compat/veremi/<run>.zip`,
  `compat/dare/<run>.zip` (reports with observer ids; real ids in `real_ids.json`),
  `scenarios/`, `README.md` (the Hugging Face card), `croissant.json` and `manifest.json`.
- `--date YYYY-MM-DD` sets Croissant's `datePublished`; the tool reads no clock, so the same
  inputs give the same bytes.

## 3. Validate

```sh
python -I tools/dataset/validate_release.py /tmp/v2x-scms-mbd-mini
python -I tools/dataset/validate_release.py /tmp/v2x-scms-mbd-mini --only joins,leakage --json out.json
```

| Check | Fails when |
|---|---|
| `schema` | a table, column, type or non-null rule differs from `schema/<table>.json` (schema.md names when absent), or a column is undeclared |
| `joins` | an evidence `spdu_digest` is not in `truth/transmissions`, an `rx_id` is not in `truth/receptions`, or a `report_id` is not in local, global and truth |
| `leakage` | a truth key column, or a truth-only id value (`device_id`, `tx_id`), appears in `local/` or `global/` |
| `labels` | a malicious transmission lacks `attack_id`, a benign one has it, or attackers per run do not match `runs.attack_mix` |
| `time` | a time is outside `0..duration`, a receiver's rows go back in time, or causal order breaks |
| `units` | own or true speed outside 0..90 m/s (claimed: the J2735 range), headings, radio values, positions outside the run's area |
| `splits` | a run sits in two splits, partitions disagree with `runs.parquet`, or a held-out attack is in train |
| `manifest` | a SHA-256 or size differs, or a file is missing or unlisted |
| `shortcuts` | a metadata-only classifier (size, `msg_cnt`, signer kind, per-pseudonym rate) scores ROC-AUC above 0.7 on an attack that is not a rate or size attack |

To check the Croissant file with the reference library, install `mlcroissant` in a venv
and load it: `python -c "import mlcroissant as m; m.Dataset(jsonld='croissant.json')"`
(this validates the metadata offline; iterating records would fetch from Hugging Face).

## 4. Test the tools

```sh
python -I -m pytest tools/dataset/tests -q
```

`test_validate.py` injects one fault per check into a copy of the fixture and asserts that
check goes red. `test_release_tools.py` covers assembly (determinism across processes,
tampered input, schema drift, the DARE files, the card and Croissant) and the upload tool
with a fake `huggingface_hub`, so no test touches the network.

## 5. Package and upload

```sh
python -I tools/dataset/hf_upload.py --root /tmp/v2x-scms-mbd-mini --repo <org>/v2x-scms-mbd-mini
```

The dry run validates the release, lists files and sizes per folder, and uploads nothing.
After review, add `--yes` (and `--private` for a private repo, `--large` for releases of
many GB). The token is `HF_TOKEN` from the repo's `.env` (or `--env FILE`); it is never
printed, and the upload is refused if any release file contains it or any check fails.
For the frozen Zenodo copy, archive the same release root; `manifest.json` pins every file.
