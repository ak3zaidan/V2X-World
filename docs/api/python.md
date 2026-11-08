# Python API

The `v2xw` package (`python/v2xw/`) drives the engine from Python: load and validate a
scenario, run it, read its metrics and its recording, and write plug-ins. It is a binding,
not a second implementation: every call delegates to the native module `v2xw._v2xw`,
built from `crates/v2xw-py`. Typed stubs ship in `_v2xw.pyi`; `python/tests/test_stubs.py`
holds them equal to the module. A walkthrough is on [Python](../getting-started/python.md).

## Install

```sh
pip install maturin
cd python && maturin develop                 # editable, into the active virtualenv
cd python && maturin build --release         # a wheel in ../target/wheels
pip install 'v2xw[arrow]'                    # optional pyarrow for the zero-copy table paths
```

`requires-python >= 3.9`. Without `pyarrow`, every table still crosses as Arrow IPC bytes.

```python
import v2xw

scenario = v2xw.Scenario.load("scenarios/midtown-normal.yaml")
run = v2xw.run(scenario, recording="run.mcap", build_utc="2026-01-01T00:00:00Z")
pdr = run.metrics.series("pdr")
rec = v2xw.Recording.open("run.mcap")
table = rec.arrow("node.tx")
```

## Module `v2xw`

| Name | Signature | Meaning |
|---|---|---|
| `run` | `run(scenario, *, build_utc=None, recording=None, metrics=True, metric_window_s=1.0, ma_policy=None) -> Run` | runs a `Scenario` (or a path to one). `build_utc` is the manifest timestamp; left out, it is read here, the package's only wall-clock read, and excluded from every digest. `recording` is the MCAP path, or `None` for no file. `metrics` registers the metric providers. `metric_window_s` is the aggregation window (0: one sample per metric). `ma_policy` is a [`MaPolicy`](#families) for every provider MA that would run the deployment-wide rule |
| `run_twice` | `run_twice(scenario, *, build_utc, ma_policy_factory=None) -> (str, str)` | runs twice and returns both record-stream digests: the determinism gate. `ma_policy_factory` builds a fresh policy per run |
| `read_ipc` | `read_ipc(buffer: bytes) -> pyarrow.Table` | reads an Arrow IPC stream buffer (from `Metrics.ipc` or `Recording.table`); raises `ImportError` naming the extra if `pyarrow` is missing |
| `math` | class of static functions | the engine's deterministic transcendentals ([below](#v2xwmath)) |
| `plugins`, `conformance` | modules | the plug-in seam and its checks |
| `__version__` | `str` | the package version |

Errors: `V2xwError` (base), `ScenarioError` (the scenario will not build or validate),
`RecordingError` (a recording cannot be written or read), `MetricError` (a provider refused
its definition), `DeterminismError` (a determinism or conformance check failed).

### `Scenario`

| Member | Meaning |
|---|---|
| `Scenario.load(path)` | loads a file, resolving `meta.base` relative to its directory, migrating and validating |
| `Scenario.parse(text, base_dir=None)` | parses YAML or JSON text |
| `Scenario.from_dict(doc)` | builds from a `dict` of the file's shape |
| `Scenario.minimal()` | the smallest scenario that runs: a procedural grid world and the schema defaults |
| `schema`, `name`, `seed`, `duration_s`, `path` | the schema version after migration, `meta.name`, the master seed, the run length in seconds, the source file |
| `content_hash` | SHA-256 of the canonical JSON: the identity the manifest pins |
| `problems()` | every validation conflict as messages, without raising |
| `validate()` | raises `ScenarioError` if invalid; returns `self` |
| `to_yaml()`, `to_json()`, `as_dict()` | the document |
| `write(path)` | writes it as YAML |

### `Run`

| Member | Meaning |
|---|---|
| `manifest` | the run manifest (`dict`): scenario hash, seed, platform, crate versions, model cards ([format](../dev/recording-format.md#the-manifest)) |
| `report` | the run's counters: events by class, actors, nodes, frames, records |
| `metrics` | a `Metrics` |
| `recording` | the recording path, if one was written |
| `end_ns` | the instant the loop stopped, ns |
| `records`, `records_refused` | records emitted; records the recording refused (non-zero: the file is not the whole run) |
| `scenario_hash` | the scenario hash this run is identified by |

### `Metrics`

| Member | Meaning |
|---|---|
| `len(m)` | how many samples |
| `names()` | every metric name present, sorted |
| `series(metric) -> [(t_ns, value)]` | one metric in time order, skipping insufficient samples |
| `last(metric)` | the last point value, or `None` |
| `rows(metric=None)` | samples as dictionaries (the slow path, for tests) |
| `arrow()` | the samples as a `pyarrow.RecordBatch` sharing the native buffers |
| `ipc() -> bytes` | the samples as an Arrow IPC stream, no `pyarrow` needed |
| `write_arrow(path) -> int` | writes an Arrow IPC stream file, returns its size in bytes |
| `summary()` | the run summary the manifest carries: the digested metrics and their digest |
| `digest` | SHA-256 over the digested metrics: what two runs are compared on |
| `diagnostics()` | machine-dependent runtime diagnostics, kept out of the summary and its digest |

### `Recording`

| Member | Meaning |
|---|---|
| `Recording.open(path)`, `Recording.open_bytes(data)` | opens a recording from a file or from memory |
| `path`, `manifest`, `metadata` | the source file; the run manifest (`dict`, empty for an interrupted run); the MCAP metadata verbatim |
| `channels` | every channel: name, visibility and message count |
| `ground_truth_channels` | the channel names that carry ground truth |
| `require_chunk_checksums(require)` | make a chunk without a stored checksum a refusal rather than a caveat |
| `verify()` | walks the whole file and reports what it checked ([`VerifyReport`](../dev/recording-format.md#reading-a-recording)) |
| `content_digest()` | SHA-256 over the recording's content, excluding the container's framing |
| `records(channel=None, limit=None)` | records as dictionaries (the slow path) |
| `arrow(channel, ground_truth=True)` | one channel as a `pyarrow.RecordBatch` sharing the native buffers; `ground_truth=False` drops the ground-truth columns |
| `table(channel, ground_truth=True) -> bytes` | the same as Arrow IPC bytes, no `pyarrow` needed |
| `table_schema(channel)` | the inferred column schema of a channel |
| `frame_count()` | how many VWP snapshot frames it holds |

Pass `ground_truth=False` for a node-visible view of a mixed channel; `ground_truth_channels` lists the channels that are ground truth throughout.

### `v2xw.math`

The same pure-Rust functions the engine uses (`v2xw_core::math`), bit-identical on every
platform. A plug-in uses these instead of `math`, `**` on floats or numpy ufuncs.

| Group | Functions |
|---|---|
| trigonometric | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2(y, x)`, `sin_cos(x) -> (sin, cos)` |
| exponential | `exp`, `exp2`, `ln`, `log10`, `log2`, `pow(base, exponent)` |
| roots and others | `sqrt`, `cbrt`, `hypot(x, y)`, `sinh`, `cosh`, `tanh` |
| quantisation | `quantize_to(x, quantum)`, `quantize(x, decimals)`, `is_on_grid(x, quantum)`, `grid_index(x, quantum)` |
| ordered sums | `sum_ordered(values)`, `sum_sorted_by_key(pairs)` |

## Module `v2xw.plugins`

Writing a model in Python: base classes, a card builder that validates, and handles that
put a Python object behind an engine trait. The rules a plug-in keeps and what each family
can do are on [Extending: plugins](../dev/extending/plugins.md#python-plug-ins).

### Card builders

| Function | Signature | Returns |
|---|---|---|
| `source` | `source(kind, ref, *, accessed=None, note=None)` | a parameter's source; `kind` is `standard`, `paper`, `datasheet`, `dataset`, `code` or `todo-calibrate` |
| `equation` | `equation(name, latex_or_text, *, notes=None)` | one `equations` entry |
| `parameter` | `parameter(name, unit, default, src, *, calibration=None, value_range=None)` | one `parameters` entry; a `todo-calibrate` source needs `calibration` |
| `card` | `card(*, id, family, version, purpose, tier=("abstract",), equations=(), parameters=(), assumptions=(), limitations=(), ignores=(), sources=(), api_version=API_VERSION)` | a validated card `dict` (`determinism.uses_rng` is always `False`); raises `V2xwError` naming the field |
| `validate_card` | `validate_card(card) -> dict` | the registry's own card validation |
| `uncited_parameters` | `uncited_parameters(card) -> list[str]` | parameters whose default cites nothing |
| `API_VERSION` | `"1.0.0"` | the plug-in API version targeted |
| `PROBABILITY_QUANTUM` | `float` | the grid a probability output is quantised to |

### Families

| Class | Implement | Called | Attach |
|---|---|---|---|
| `CarFollowing` | `accel(ego, leader, lane, weather) -> float` (m/s²); optional `profile(vehicle_class) -> dict` with `desired_speed_mps`, `max_accel_mps2`, `comfort_decel_mps2`, `time_headway_s`, `min_gap_m` | once per vehicle per mobility step; abstract tier only | `.attach() -> CarFollowingModel` |
| `Detector` | `on_messages(ctx, batch) -> list[Observation]`; `ctx` is a `DetectorCtx`, `batch` a `pyarrow.RecordBatch` of the window's received messages | once per window | `.attach() -> DetectorModel` |
| `MaPolicy` | `decide(ctx, batch) -> list[(subject, revoke)` or `(subject, revoke, row)]`; `ctx` is a `MaCtx`, `batch` a `MaBatch` | once per step with that step's accepted reports | pass to `v2xw.run(..., ma_policy=obj)` |

Each class needs a `card` class attribute built with `card(...)` and `family="mobility"`,
`"detector"` or `"ma-pipeline"`.

### Views and handles

| Type | Fields / members |
|---|---|
| `Ego` | `actor`, `vehicle_class`, `lane`, `lane_index`, `s_m`, `lateral_m`, `speed_mps`, `accel_mps2`, `heading_rad`, `length_m`, `width_m`, `height_m`, and the driver's `desired_speed_mps`, `max_accel_mps2`, `comfort_decel_mps2`, `time_headway_s`, `min_gap_m` |
| `Leader` | `gap_m`, `speed_mps`, `accel_mps2`, `length_m`, `is_vehicle` (`False` for a stop line, red signal, junction or curve-speed cap: a virtual leader) |
| `Lane` | `id`, `kind`, `speed_limit_mps`, `width_m`, `length_m` |
| `Weather` | `kind`, `intensity`, `visibility_m`, `surface` |
| `DetectorCtx` | `t_ns`, `node` (no RNG accessor, by design) |
| `Observation` | `Observation(subject, kind, confidence, t_ns)` |
| `CarFollowingModel` | `CarFollowingModel(obj)`; `id`, `card`, `last_error` (the stored traceback; a raising `accel` returns `NaN`), `accel(ego_speed_mps, gap_m=None, leader_speed_mps=..., speed_limit_mps=..., is_vehicle=...)` for direct calls |
| `DetectorModel` | `DetectorModel(obj)`; `id`, `card`, `last_error`, `on_messages(t_ns, node, batch)` |
| `ReferenceDetector` | `ReferenceDetector(max_speed_mps=...)`; a built-in reference detector with `card` and `on_messages(t_ns, node, batch)` |
| `MaCtx` | `t_ns`, `ma` (the deciding MA's provider index; keep state per `ma`), `provider` |
| `MaBatch` | `num_rows`, `columns`, `len()`, `column(name)`, `rows()`, `to_pyarrow()`; columns `MA_BATCH_COLUMNS` (the `ma.received` columns) plus `observed_ns` and `reporter_infrastructure`; never ground truth |

## Module `v2xw.conformance`

The plug-in checks, split between the engine's call path and Python-side facts.

| Name | Signature | Meaning |
|---|---|---|
| `check` | `check(model, *, quantum=1e-3) -> Report` | the whole suite over a car-following plug-in (an instance or an attached handle): `native_checks`, a purity sweep under `purity_guard`, and `scan_source` |
| `native_checks` | `native_checks(model, *, quantum=1e-3) -> list[Check]` | through the engine's call path: determinism, thread-count independence, dyn-compatibility, finiteness, quantisation to `quantum`, card validation and completeness |
| `purity_guard` | context manager yielding a list | replaces `random.random`, `time.time`, `datetime.now` and relatives with functions that raise, recording each call |
| `scan_source` | `scan_source(obj) -> Check` | advisory: imports of banned modules in the plug-in's source, read with `ast` |
| `Check` | frozen dataclass `(check, passed, detail, advisory=False)` | one check's outcome |
| `Report` | frozen dataclass `(checks)` | properties `passed` (no non-advisory failure), `failures`, `warnings`; `raise_for_failures()` raises `DeterminismError` or returns `self` |

Passing the suite is necessary, not sufficient: hidden state between calls is caught only
by comparing two real runs (`v2xw.run_twice`).
