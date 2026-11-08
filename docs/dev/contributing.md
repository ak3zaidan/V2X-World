# Contributing

How to set up, what a change must pass, and how to work on the documentation. Code rules
are on [Conventions](conventions.md); building and test layout on
[Building and testing](../testing.md).

## Setup

| Need | Version / source |
|---|---|
| Rust | pinned in `rust-toolchain.toml` (`1.98.1`, with `rustfmt`, `clippy` and the `wasm32-unknown-unknown` target); `rustup` installs it on first use |
| `just` | the task runner (`just --list`) |
| Python and `uv` | the `python/` package (optional) |
| Node and `pnpm` | the Studio UI under `ui/` (optional) |
| SUMO | optional: the high mobility tier and cross-validation (`just sumo-install` prints how) |

```sh
just setup          # cargo fetch; uv sync --project python; pnpm --dir ui install
just build          # cargo build --workspace
just check          # cargo check --workspace --all-targets
```

Tests that import the Manhattan extract need `worlds/cache/manhattan.osm.xml`
(`worlds/fetch.sh manhattan`; git-ignored). On a machine with little memory, build the
release binaries one job at a time (`just build-cli`, `just build-server`).

## Making a change

1. Branch from `main`; never commit to `main` directly.
2. Keep the change to one concern. A new model, setting, detector or metric follows its
   recipe under [Extending](../extending.md).
3. Run the gates below locally, scoped to the crates you touched while iterating
   (`cargo test -p <crate>`, `cargo test -p <crate> --test <file>`); the full workspace
   suite is long.
4. If the change moves a run's output on purpose, re-bless the golden digests and say so
   in the commit:
   `V2XW_CONFORMANCE_BLESS=1 cargo test -p v2xw-conformance --test kit golden`.
5. If the change adds or edits a setting, run `just docs-gen` and commit the regenerated
   reference.

### Commit messages

Subject `<area>: <what changed>` in plain descriptive language (`v2xw-sec: ...`,
`scenarios: ...`, `docs: ...`), body explaining why where it is not obvious. No internal
tracker ids or narrative.

## The gates

| Gate | Command | Runs in CI |
|---|---|---|
| formatting | `cargo fmt --all -- --check` (`just fmt-check`) | `ci.yml` job `rust`, on Ubuntu, macOS and Windows |
| lints | `cargo clippy --workspace --all-targets -- -D warnings` (`just lint`) | `rust` |
| tests, including the conformance kit (`tests/conformance` is a workspace member) | `cargo test --workspace` (`just test`) | `rust` |
| toolchain pin | the installed `rustc` must equal `rust-toolchain.toml` | `rust` |
| cross-platform determinism | each platform imports `tests/fixtures/midtown-6block.osm.xml` and prints the world and payload digests (`just world-hash`); all three must agree and no value may be off its grid | `determinism`, `determinism-compare` |
| UI | `pnpm --dir ui typecheck && pnpm --dir ui test` (`just ui-test`) | local only |
| validation suites | `just validation-report [--quick]`, `just cross-validate` (needs SUMO) | local only ([tools](tools.md)) |

`just ci` runs fmt-check, lint and test in one go.

## Documentation

The site is built with [Zensical](https://zensical.org) from `mkdocs.yml` and `docs/`,
and deployed to GitHub Pages by `.github/workflows/docs.yml` on every push to `main` that
touches `docs/`, `mkdocs.yml`, `crates/`, `tools/docs/` or the workflow.

```sh
pip install -r tools/docs/requirements.txt   # pins zensical
just docs-gen       # regenerate docs/configuration/reference/ from the engine
just docs-serve     # live-reload preview at http://localhost:8000
just docs-build     # build into site/ (zensical build --clean); warns on broken links
cargo doc --workspace --no-deps --open        # the Rust API docs
```

The CI workflow does the same: generate the reference
(`dump_settings` → `tools/docs/gen_config_reference.py`), `zensical build --clean`, then
`cargo doc --workspace --no-deps` copied into `site/api/`.

| Where | What |
|---|---|
| `mkdocs.yml` `nav` | the site structure; a new page needs an entry, and the generator fails if a generated page is missing from it |
| `docs/configuration/reference/` | generated; never edit by hand |
| `docs/README.md` | the map for readers browsing the repository; excluded from the site |
| `docs/reference/` | curated tables, some embedded by the assistant |
| `docs/protocol/vwp-v1.md` | the normative wire specification |

Rules for documentation:

- **The code is the truth.** Every claim is checked against the code it describes; leave
  out what cannot be verified. Describe what is implemented, not what is planned, and say
  where a model is simplified.
- **Cite** the standards and papers the code implements, with sections, as the code does
  ([Standards and references](../about/standards.md)). Do not quote licensed standard
  text (SAE J2735, IEEE 1609) at length; field names are fine.
- **Link instead of repeating.** One page owns each topic.
- **Real commands only**, and paths relative to the repository root.
- Pages use admonitions for notes and warnings, tables for parameters, mermaid for data
  flow and `$...$` / `$$...$$` (MathJax) for equations.

## Notes for coding agents

`AGENTS.md` at the repository root is the entry point: the map of the workspace, the
commands, and the invariants that must not break (determinism, the canonical digest,
scenario hashes, belief versus ground truth, model cards, no invented defaults). Then:

- read the crate's `src/lib.rs` module table before the module;
- prefer `cargo check -p <crate> --all-targets` and scoped tests over workspace builds;
- prove a new check can fail before trusting it (inject the fault, see it go red,
  restore; `tools/mutate/mutate.py` restores the file's mtime so cargo rebuilds);
- never let a setting reach the Studio without a range or menu, units and a status
  ([Extending: settings](extending/settings.md));
- keep `AGENTS.md`'s links consistent when moving a page.
