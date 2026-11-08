# `v2xw-py`

The native half of the `v2xw` Python package. The Python half is in `python/v2xw/`; the
package documentation is `python/README.md`.

```
cargo test  -p v2xw-py                                    # no wheel needed
cargo clippy -p v2xw-py --all-targets --no-deps -- -D warnings
cd python && maturin develop && python -m pytest tests    # the Python half
```

## Two version pins, and why they are load-bearing

* **`pyo3 = 0.25`**, not the newest release. `arrow`'s `pyarrow` feature — the zero-copy
  hand-off — depends on `arrow-pyarrow`, which requires `pyo3 ^0.25.1`. Two pyo3 versions in
  one binary do not interoperate: a `Bound<'py, PyAny>` from one is a different type from
  the other's. Raising this means waiting for `arrow-pyarrow` to follow, or giving up the
  zero-copy path for Arrow IPC bytes.
* **`extension-module` is not a default feature.** It tells pyo3 not to link libpython,
  which is right for a wheel and wrong for `cargo test`; maturin turns it on
  (`python/pyproject.toml`).

## Known limitations

1. **Metric providers do not reach the run manifest.** `Engine` owns its registry
   privately, so `run` registers metric providers in a registry of its own; the manifest
   pins the engine's models but not the providers that measured it.
2. **A run holds the interpreter lock.** `Engine` is not `Send`, so `Python::allow_threads`
   cannot wrap a run. Use one process per run, as the experiment system does.
3. **A `python-hot-path` run is not marked as one** in the manifest (the SDK contract
   rule P2), because the manifest is assembled before a plug-in can be attached.
4. **`plugins::Detector` is declared in this crate**, with
   `plugins::SpeedPlausibilityDetector` as a reference implementation, against the surface
   the SDK contract publishes.
5. **A Python plug-in cannot be named in a scenario.** `plugins::PyCarFollowing` works
   through `dyn CarFollowing`, and the conformance kit drives it directly, but the scenario
   schema has no way to select a Python object. The one wired family is the MA decision
   rule (`ma_policy::PyMaPolicy`): `v2xw.run(..., ma_policy=obj)` attaches it through
   `Engine::attach_ma_policy`, and the manifest pins it
   (`ma_policy:python:<module>.<class>`, source SHA-256).

## Ground truth in a node-only read

`Recording.arrow(channel, ground_truth=False)` and `Recording.table(...)` drop the
columns `v2xw_record::export::schema::ground_truth_fields` declares for the channel:
`tx` and `dist_m` on `phy.rx` and `node.rx`, `origin` and `dest` on `msg.latency`,
`subject`, `outcome`, `ttc_truth_s` and `delay_s` on `app.outcome`, and `truth` on
`pki.knowledge` and `pki.report_fate`. A channel that is ground truth whole (`gt.*`) has
no tagged columns; leave it out of a node-only read altogether
(`Recording.ground_truth_channels()` lists them).

The projection matches by exact name, so a declared name that no record carries withholds
nothing. That happened once: the list named `phy.rx`'s oracle columns `tx_node`,
`distance_m` and `los_class` while the record carries `tx` and `dist_m`, and a node-only
read carried the true transmitter id and distance. `tests/ground_truth_fields.rs` now
serialises the engine's own record type for every channel with a list and fails if a
declared name is not one of its fields, or if a channel tagged node-and-ground-truth
declares none; `python/tests/test_sdk.py` checks a real run's `phy.rx` read without
ground truth has no `tx` or `dist_m` column.
