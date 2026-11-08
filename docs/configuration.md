# Configuration

Every setting is a scenario field ([scenarios.md](scenarios.md)). There are no
configuration files or environment variables that change a run's results; the only
environment variables read are for optional tooling (SUMO location, the assistant's
provider keys, test switches).

How to read the generated reference, and the status vocabulary in full:
[Configuration overview](configuration/overview.md). Adding a setting:
[Extending: settings](dev/extending/settings.md).

## Where a setting lives

All under `crates/v2xw-engine/src/scenario/`:

| File | Role |
|---|---|
| `schema.rs` | the typed scenario: every field, its type, its documented default and unit; `deny_unknown_fields`, so a misspelt key is an error |
| `validate.rs` | `BOUNDS` (numeric ranges), `CHOICES` (closed value sets), `KEY_STATUS` (implementation status of every key), and the cross-field rules |
| `options.rs` | labels and one-line explanations for each value of a closed set; named presets for numbers; the model ids a slot accepts, taken from the registry |
| `guide.rs` | a `basic`/`advanced` level per field, a plain-language description and recommended range for basic fields, the Studio's quick settings and study starting points |
| `publish.rs` | the published settings surface, generated from the above and from the registry's model cards |
| `module_keys.rs` | fallback for keys whose tables live beside the engine module that implements them (for example `ma_policy.rs`, `scms_topology.rs`) |

Model parameters are not scenario fields of their own: a slot holds a
`ModelChoice { id, params }` (for example `security.protocol`, `net.uu`, entries of
`radio.models`), and the parameters a model accepts, with units, defaults, ranges and
sources, come from its model card.

## Implementation status

`KEY_STATUS` classifies every key, and the published surface tags every field with it
(`x-status`), so it is always visible whether editing a field changes a run:

| Status | Meaning |
|---|---|
| `wired` | the engine reads it and it changes the run |
| `partial` | the engine acts on part of it, or only under conditions the row's note states |
| `not-implemented` | validated and hashed, read by nothing |
| `refused` | the loader rejects every value this build cannot act on; only implemented values load |
| `descriptive` | describes the scenario (name, tags) and by design changes nothing computed |

A test (`every_leaf_has_a_status`) requires every published leaf to match a row.

## The published surface

`v2xw_engine::scenario::publish::surface()` returns, as JSON:

| Key | Contents |
|---|---|
| `schema` | JSON Schema 2020-12 over the whole scenario, with `unit`, `default`, `x-status`, `x-path` per node |
| `fields` | the same tree flattened to one row per editable leaf |
| `groups` | the sections in display order |
| `slots` | every place the scenario selects a model, with the selectable ids |
| `models` | every registered model's card and parameters |
| `statuses` | the status vocabulary |

The server returns it from the `scenario.get` and `scenario.schema` RPCs; the Studio's settings window and
the assistant are generated from it rather than hand-written. Tests require that every
key in every shipped scenario is described, that a document built from the published
defaults loads, and that no unclassified field is offered.

## Validation rules worth knowing

- Tier combinations must compose (`phy: high` needs `mac: high`; abstract propagation
  needs an abstract PHY).
- `time.mobility_step_ms` is bounded to the range the mobility models are calibrated for.
- An attacker population names exactly one of `fraction`, `count` or `actor_ids`.
- `security.crypto_mode: real` narrows `security.signature` to the schemes with a real
  implementation (`ecdsa-p256`).
- `security.protocol` and `actors.backend.protocol` must name the same credential protocol.
- A shorter credential i-period than the deployment's needs `time.calendar: compressed`,
  which every output then labels.

The same validator runs in the CLI (`v2xw validate`), behind the `scenario.validate` RPC,
and when a run is built.

## Reference tables

Generated or curated per-domain listings, with paths, defaults, ranges and sources:

- [reference/configuration-reference.md](reference/configuration-reference.md)
- [reference/config-inventory-traffic.md](reference/config-inventory-traffic.md)
- [reference/config-inventory-network.md](reference/config-inventory-network.md)
- [reference/config-inventory-scms.md](reference/config-inventory-scms.md)

How each published setting moves the metrics, run one at a time on a fixed seed with
`v2xw sensitivity --catalogue` and checked against expected directions with
`v2xw sensitivity --expect`:
[reference/sensitivity-catalogue.md](reference/sensitivity-catalogue.md),
[reference/sensitivity-expectations.md](reference/sensitivity-expectations.md). The
expectation rows live in `crates/v2xw-engine/tests/expectations/`.
