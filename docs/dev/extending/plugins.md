# Extending: plugins

What can be written outside the engine's own crates, how it registers, and what a
scenario can then do with it. The short answer: an out-of-tree Rust or Python model can be
written, carded, registered, tested and conformance-checked against the engine's own
traits; a scenario can select only the models compiled into the engine's slots, and a
Python Misbehaviour Authority policy is the one plug-in that runs inside a full run.

## What is supported

| Extension | Where it lives | Registers | Runs inside `v2xw run` / the Studio | Selected by |
|---|---|---|---|---|
| a model in any family (Rust, out of tree) | your crate, depending on `v2xw-core` and one family crate | `Registry::register_model` | no, until a slot builder in the engine names its id ([models](models.md#5-make-it-selectable)) | n/a |
| a model added to an engine slot (Rust, in tree) | a model crate plus the slot's builder in `v2xw-engine` | `wiring::register_all` / the slot's catalogue | yes | its id in the slot |
| a signature or KEM scheme | `crates/v2xw-sec/src/scheme/` (in tree) | one line in `SIGNATURE_SCHEMES` / `KEM_SCHEMES` | yes | `security.signature`, `security.kem` |
| a threshold protocol | `crates/v2xw-proto/src/threshold/` (in tree) | one line in `THRESHOLD_PROTOCOLS` | yes | `security.threshold.protocol` |
| a Python car-following model | your module, subclassing `v2xw.plugins.CarFollowing` | `.attach()` gives a `CarFollowingModel` handle | no: callable directly and through the conformance kit | n/a |
| a Python detector | `v2xw.plugins.Detector` | `.attach()` gives a `DetectorModel` handle | no: callable directly and through the conformance kit | n/a |
| a Python MA decision rule | `v2xw.plugins.MaPolicy` | passed to `v2xw.run(scenario, ma_policy=obj)` | yes, in a run started from Python | the `ma_policy` argument (a scenario file cannot name it) |

## Rust plug-ins: the `examples/` workspace

`examples/` is a separate cargo workspace of four crates, each a real plug-in built the
way an out-of-tree crate is: every dependency spelled out with a version
(`v2xw-core = { path = "../crates/v2xw-core", version = "0.1.0" }`), nothing inherited from
the root workspace, its own lock file and target directory. Building them never changes
the engine's build.

| Crate (package) | Family | Trait | Walkthrough |
|---|---|---|---|
| `propagation-log-distance` (`v2xw-example-propagation`) | propagation | `v2xw_radio::traits::Propagation` | [models](models.md#the-propagation-example) |
| `carfollowing-constant-time-gap` (`v2xw-example-carfollowing`) | mobility | `v2xw_mobility::traits::CarFollowing` | [models](models.md#the-car-following-example) |
| `detector-heading-rate` (`v2xw-example-detector`) | detector | `v2xw_threat::Detector` | [detectors](detectors.md) |
| `metric-detector-load` (`v2xw-example-metric`) | metric | `v2xw_metrics::MetricProvider` | [metrics](metrics.md) |

```sh
cd examples
cargo test                               # all four
cargo test -p v2xw-example-detector      # one, including the conformance suite
```

To start your own, copy the closest example to a new crate, change `MODEL_ID` (keep an
`example/` or your own prefix so it cannot be mistaken for a shipped model), and keep the
dependency table explicit. Add `v2xw-conformance` (path `tests/conformance`) as a
dev-dependency to run the plug-in suite.

### Registration and licences

`v2xw_core::registry::Registry` validates the card, checks `api_version` against
`v2xw_core::card::API_VERSION` (`1.0.0`), refuses a duplicate id with a different card,
hashes the card into the manifest's content hash, and applies a licence gate:

| `Licence` | In process |
|---|---|
| `Apache2`, `Mit`, `Bsd`, `Mpl2`, `Unspecified` | allowed (`register_model_with_licence`) |
| `Gpl`, `Lgpl`, `Other(..)` | refused in process (`LicenceRequiresOutOfProcess`); register with `register_out_of_process` / `register_model_out_of_process` |

The engine itself is Apache-2.0 and links only Apache-compatible code. Registering out of
process records the card and the hosting; no out-of-process host for running such a model
ships with the engine.

## A signature or KEM scheme

The scheme registry in `crates/v2xw-sec/src/scheme/mod.rs` is read by the scenario
loader, the published schema, the node's signing cost, the over-the-air octets, the
fragmenter, the credential system's sizes and charges, and the recorded run, so one entry
is the whole change.

1. Copy `crates/v2xw-sec/src/scheme/template.rs` (compiled, deliberately unregistered) to
   `scheme/<your_scheme>.rs`.
2. Fill in the `SignatureScheme`:

    ```rust
    pub const MY_SCHEME: SignatureScheme = SignatureScheme {
        id: "my-scheme",                       // what security.signature names; stable, unique
        label: "My scheme",                    // what the settings window shows
        mode: SchemeMode::PostQuantum,         // Classical, Hybrid or PostQuantum; must match the parts
        parts: &[Part {                        // wire order; sizes and costs add
            primitive: PrimitiveId::ML_DSA_44,
            op_prefix: "ml-dsa-44",            // hardware-profile ops <prefix>-sign/-verify/-keygen
            post_quantum: true,
        }],
        standard: "FIPS 204 (2024)",           // the standard or draft, with clause
        real_crypto: false,                    // true only if crypto::Real executes every part
        defines: None,                         // Some(fn) when a part's primitive is new
        notes: "...",
    };
    ```

3. A primitive new to the catalogue gets a `fn` returning its `PrimitiveDescriptor`: sizes
   from the specification, one `CostRow` per published benchmark with its citation, or a
   `cost_ratio` to a sibling with a shared reference platform rather than an invented
   number. Set `defines: Some(that_fn)`.
4. In `scheme/mod.rs`: `pub mod <your_scheme>;` and one line in `SIGNATURE_SCHEMES` (or
   `KEM_SCHEMES` for a KEM).
5. `cargo test -p v2xw-sec scheme`: the registry tests check the id is unique, the mode
   matches the parts, every part is in the catalogue and every post-quantum part is priced
   on the V2X hardware profiles.

`hybrid_mldsa65.rs` is a worked example. Under `security.crypto_mode: real` the loader
accepts only schemes with `real_crypto: true`.

## A threshold protocol

Copy `crates/v2xw-proto/src/threshold/template.rs`: define `sign_rounds` (one `StarRound`
per online round, with request and response payload sizes, each cited, per-member and
coordinator computation, and whether shares are verified), `dkg_rounds` and
`refresh_rounds`, and the guarantees (`identifiable_abort`, `verifies_as`, `knowledge`).
Add `ENTRY` to `THRESHOLD_PROTOCOLS` in `threshold/mod.rs` and run
`cargo test -p v2xw-proto threshold`. `security.threshold.protocol` then accepts the id.

## Python plug-ins

`python/v2xw/plugins.py` (Python half) and `crates/v2xw-py/src/plugins.rs` (Rust half) put
a Python object behind an engine trait. The full API is on [Python API](../../api/python.md).

### The rules

A plug-in is a function of its inputs; the engine's reproducibility depends on it:

1. no randomness of its own (`random`, `numpy.random`, `secrets`, `os.urandom`): neither
   family is given a stream, and the card's `determinism.uses_rng` is always `False`;
2. no wall clock (`time.time`, `time.perf_counter`, `datetime.now`); simulated time
   arrives as an argument;
3. no platform libm (`math.exp`, `**` on floats, numpy ufuncs): use `v2xw.math`, the
   engine's pure-Rust implementation;
4. no iteration over a `set`, or a `dict` keyed by hashed values, where the order reaches
   a result;
5. no hidden state between calls (the engine reorders and parallelises).

### Car-following

```python
import v2xw
from v2xw.plugins import CarFollowing, card, parameter, source, equation

m = v2xw.math

class Idm(CarFollowing):
    card = card(
        id="mobility/car-following/python-idm", family="mobility", version="1.0.0",
        purpose="Treiber's IDM as a Python plug-in.",
        tier=("abstract",),
        equations=[equation("idm", "a = a_max [1 - (v/v0)^delta - (s*/s)^2]")],
        parameters=[parameter("delta", "-", 4.0,
                              source("paper", "Treiber, Hennecke, Helbing 2000, Eq. 1"))],
    )

    def accel(self, ego, leader, lane, weather):
        # driver parameters arrive on ego: desired_speed_mps, max_accel_mps2,
        # comfort_decel_mps2, time_headway_s, min_gap_m
        ...

model = Idm()
handle = model.attach()                 # a CarFollowingModel: the engine trait object
v2xw.conformance.check(model).raise_for_failures()
```

`card(...)` validates immediately, so a bad card fails at its definition. `accel` is
called once per vehicle per mobility step (abstract tier only; one interpreter-lock
acquisition per call). A Python exception cannot cross `accel`'s `f64` return, so the
adapter stores the traceback in `last_error` and returns `NaN`, which fails the writer's
quantisation check at the first record it reaches. `python/examples/python_idm.py` is the
full example, including a deliberately broken model the kit rejects.

### Detector

```python
class SpeedCap(v2xw.plugins.Detector):
    card = v2xw.plugins.card(id="detector/python-speed-cap", family="detector", ...)

    def on_messages(self, ctx, batch):        # ctx: DetectorCtx(t_ns, node)
        # batch: a pyarrow.RecordBatch, one row per received message; read it columnwise
        return [v2xw.plugins.Observation(subject, "speed", confidence, ctx.t_ns), ...]
```

Called once per window with the whole window, so the per-call cost is amortised.
`v2xw.plugins.ReferenceDetector` is a built-in reference implementation with the same
interface.

### MA policy

```python
class TwoReports(v2xw.plugins.MaPolicy):
    card = v2xw.plugins.card(id="ma/python-two-reports", family="ma-pipeline", ...)

    def decide(self, ctx, batch):              # ctx: MaCtx(t_ns, ma, provider)
        # batch: MaBatch of this step's accepted reports (MA_BATCH_COLUMNS + observed_ns,
        # reporter_infrastructure); never ground truth
        return [(subject_digest, True)]         # (subject, revoke) or (subject, revoke, row)

run = v2xw.run("scenarios/manhattan-5min.yaml", ma_policy=TwoReports())
```

It decides for every provider MA that would run the deployment-wide rule; a scenario that
already sets `detection.ma` or `security.scms.ma_policy` is refused. Define the class in a
module file: the manifest pins it as `ma_policy:python:<module>.<class>` with the SHA-256
of the module's source. Unlike the other families an MA keeps evidence between calls;
its state must be a function of the calls it received. Keep separate state per `ctx.ma`.

### Checking a Python plug-in

| Call | Checks |
|---|---|
| `v2xw.conformance.check(model, quantum=1e-3)` | the whole suite over a car-following plug-in; returns a `Report` (properties `passed`, `failures`, `warnings`; `raise_for_failures()` raises `DeterminismError`) |
| `native_checks(model)` | through the engine's own call path: determinism, thread-count independence, dyn-compatibility, card validation and completeness, quantisation, finiteness |
| `purity_guard()` | replaces `random.random`, `time.time`, `datetime.now` and relatives with functions that raise while the plug-in is called; a call is a failure naming the function |
| `scan_source(model)` | advisory: imports of the banned modules in the plug-in's source (`ast`) |
| `v2xw.run_twice(scenario, build_utc=..., ma_policy_factory=...)` | two full runs' record digests; the only check for hidden state between calls (rule 5) |

Passing the kit is necessary, not sufficient; `raise_for_failures()` says so.
