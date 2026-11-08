# Python

The `v2xw` package drives the engine from Python: load a scenario, run it, read a metric
as an Arrow table, open the recording, and write a model in Python that the engine calls.
It is the compiled module `crates/v2xw-py` (PyO3) plus a thin Python layer in
`python/v2xw/`. Every class and function: [Python API](../api/python.md).

## Install

Python 3.9 or later, in a virtualenv, from the repository:

```sh
pip install maturin
cd python && maturin develop            # editable install into the active virtualenv
cd python && maturin build --release    # or a wheel, in ../target/wheels
pip install 'v2xw[arrow]'               # optional: pyarrow, for zero-copy tables
```

`pyarrow` is optional. Without it every table still crosses as an Arrow IPC buffer, which
`polars.read_ipc_stream` reads. With it, `Metrics.arrow()` and `Recording.arrow()` hand
Python the engine's own buffers through the Arrow C data interface, with no copy.

## Run a scenario and read a metric

```python
import v2xw

scenario = v2xw.Scenario.load("scenarios/phase1-grid.yaml")   # load, merge overlays, validate
print(scenario.name, hex(scenario.seed), scenario.content_hash[:16])

run = v2xw.run(scenario, recording="run.mcap", metric_window_s=1.0)
print(run.records, "records;", run.report["frames_transmitted"], "frames")

for t_ns, value in run.metrics.series("pdr"):                   # (t_ns, value) pairs
    print(t_ns / 1e9, value)

table = v2xw.read_ipc(run.metrics.ipc())                        # the whole table, via pyarrow

rec = v2xw.Recording.open("run.mcap")
print(rec.verify())                                             # records, frames, integrity
print(rec.records("node.tx", limit=2))                          # one channel's first records
assert rec.manifest["scenario_hash"] == scenario.content_hash
```

| Call | What it does |
|---|---|
| `Scenario.load(path)` | parse YAML or JSON, merge `meta.base`, migrate, validate; raises `ScenarioError` on the first problem |
| `Scenario.problems()` | every problem as a list of strings, without raising |
| `Scenario.from_dict(doc)`, `Scenario.minimal()`, `as_dict()`, `to_yaml()`, `write(path)` | build or edit a scenario as a document |
| `v2xw.run(scenario, *, build_utc=None, recording=None, metrics=True, metric_window_s=1.0, ma_policy=None)` | run it; `scenario` may be a path. `build_utc` defaults to now, read in Python, because the engine reads no clock |
| `Run.metrics.series(name)` | a metric's time series; windows with too few samples for an estimate are left out, not zeroed |
| `Run.metrics.arrow()` / `ipc()` / `summary()` / `digest` | the table zero-copy, as IPC bytes, the manifest's summary, its digest |
| `Recording.open(path)`, `verify()`, `channels`, `records(channel, limit)`, `arrow(channel)`, `content_digest()` | read a recording back |

`python/examples/run_and_read_metric.py` is this walk-through as a script. With no
argument it runs the built-in minimal scenario with a short Poisson demand, so it works in
a fresh checkout:

```sh
python python/examples/run_and_read_metric.py                       # built-in minimal scenario
python python/examples/run_and_read_metric.py scenarios/phase1-grid.yaml
```

!!! note "Short runs report insufficiency"
    On a five-second run with two vehicles, `pdr` has too few receptions to estimate, and
    the example prints "no point estimate" for it. That is the statistics refusing to
    report a number it cannot support ([Metrics and outputs](../recording-and-metrics.md#statistics)).

## Check determinism

```python
a, b = v2xw.run_twice(scenario, build_utc="2026-01-01T00:00:00Z")
assert a == b      # digests over every record's instant, channel, visibility and bytes, in order
```

## Write a model in Python

`v2xw.plugins` provides base classes to subclass, `card()` to build a model card that
validates, and handles that put a Python object behind the engine's trait.

| Base class | Called | Notes |
|---|---|---|
| `CarFollowing` | once per vehicle per mobility step: `accel(ego, leader, lane, weather) -> m/s²` | pure; abstract tier only; each call crosses the interpreter lock, so expect a large slowdown |
| `Detector` | once per window: `on_messages(ctx, batch) -> [Observation]` | `batch` is a `pyarrow.RecordBatch` of the window's received messages |
| `MaPolicy` | once per MA step with new evidence: `decide(ctx, batch)` | the batch is the `ma.received` columns, never ground truth |

`python/examples/python_idm.py` writes the Intelligent Driver Model as a plug-in (Treiber,
Hennecke and Helbing 2000), runs the conformance kit over it, then shows the kit failing a
copy that adds `random.gauss` jitter:

```python
import v2xw
from v2xw.plugins import CarFollowing, card, equation, parameter, source

m = v2xw.math

class Idm(CarFollowing):
    card = card(
        id="mobility/car-following/python-idm", family="mobility", version="1.0.0",
        tier=["abstract"],
        purpose="Treiber's Intelligent Driver Model, as a Python plug-in.",
        sources=[source("paper", "Treiber, Hennecke & Helbing (2000), Phys. Rev. E 62(2) 1805")],
        parameters=[parameter("delta", "-", 4.0, source("paper", "Treiber et al. (2000) §II"))],
        # equations, assumptions, limitations and ignores as in the example file
    )
    DELTA = 4.0

    def accel(self, ego, leader, lane, weather) -> float:
        v0 = min(ego.desired_speed_mps, lane.speed_limit_mps)
        a_max = ego.max_accel_mps2
        free = a_max * (1.0 - m.pow(ego.speed_mps / v0, self.DELTA))
        if leader is None:
            return free
        dv = ego.speed_mps - leader.speed_mps
        s_star = (ego.min_gap_m + ego.speed_mps * ego.time_headway_s
                  + ego.speed_mps * dv / (2.0 * m.sqrt(a_max * ego.comfort_decel_mps2)))
        return free - a_max * m.pow(max(s_star, 0.0) / max(leader.gap_m, 0.01), 2.0)

handle = Idm().attach()                     # the model behind the engine's trait
print(handle.accel(13.888889, gap_m=50.0, leader_speed_mps=13.888889))
v2xw.conformance.check(Idm()).raise_for_failures()
```

The driver parameters (`v0`, `a`, `b`, `T`, `s0`) arrive on `ego` because the engine draws
them per driver; the model holds no state. The equation it implements:

$$
a = a_{max}\left[1 - \left(\frac{v}{v_0}\right)^{\delta} - \left(\frac{s^*}{s}\right)^2\right],\qquad
s^* = s_0 + vT + \frac{v\,\Delta v}{2\sqrt{a_{max}\, b}}
$$

### The rules a plug-in must keep

| Rule | Why | Checked by |
|---|---|---|
| no `random`, `numpy.random`, `secrets`, `os.urandom` | randomness comes from streams keyed by `(domain, entity)` | `purity_guard` replaces them with functions that raise, then calls the model |
| no wall clock (`time`, `datetime.now`) | simulated time arrives as an argument | `purity_guard` |
| no `math`, no `**` on floats | the platform libm differs between machines in the low bits; use `v2xw.math` | `scan_source` (advisory) |
| no iteration over a `set` or hashed `dict` where order reaches a result | Python randomises string hashing per process | `scan_source` |
| no hidden state between calls | the engine reorders and parallelises phases | not decidable: `v2xw.run_twice` over a real scenario catches it |

`v2xw.conformance.check(model)` runs both halves: the Rust-side checks through the engine's
call path (determinism, thread-count independence, card validation and completeness,
quantisation, finiteness) and the Python-side purity checks. Passing it is necessary, not
sufficient.

### A Misbehaviour Authority rule in Python

A rule that [`security.scms.ma_policy`](../configuration/reference/security.md) cannot
express can be a `MaPolicy` passed to `v2xw.run(..., ma_policy=MyRule())` (a scenario file
cannot name code). It decides for every provider MA that would run the deployment-wide
rule, so the scenario must not set `detection.ma` or `security.scms.ma_policy`. A revoke
goes through the provider's decision mode and the CRL like a built-in policy's. Define the
class in a module file: the manifest pins it as `ma_policy:python:<module>.<class>` with
the SHA-256 of the module's source. `run_twice(..., ma_policy_factory=...)` takes a
factory so each run starts from a fresh rule. A complete example is in `python/README.md`.

!!! note "What `v2xw.run` accepts"
    In this build `v2xw.run` takes a Python plug-in only as `ma_policy`. A
    `CarFollowing` or `Detector` subclass is attached as an engine trait object
    (`attach()`), which is what the conformance kit and direct calls such as
    `handle.accel(...)` use.

## Tests

```sh
cd python && maturin develop && python -m pytest
cargo test -p v2xw-py           # the same surface from Rust, without a wheel
```
