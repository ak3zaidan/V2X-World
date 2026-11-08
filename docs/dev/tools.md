# Validation and conformance tools

Two kinds of independent checking sit beside the engine's own tests
([Building and testing](../testing.md)):

- **Rust validation suites** (`crates/v2xw-netval`, `crates/v2xw-trafficval`) that hold each
  model to a reference outside the crate that implements it, and prove each check can
  fail by running it again with a named fault injected.
- **Python tools** under `tools/` that check the engine's wire output with code that
  shares nothing with the Rust: their own ASN.1 compiler and codec, their own elliptic
  curve and certificate arithmetic, written from the standards.

!!! note "The rule every tool keeps"
    A check is trusted only once it has been seen to fail. Each suite or tool below has a
    faulted mode (`Mode::Faulted`, `--mutate`, `--inject`, `oracle.py mutate`), or is
    driven by `tools/mutate/mutate.py`.

## Python environment

The tools run with `python3 -I` (isolated mode, so nothing in the working directory is
imported). The ones that decode PDUs need the oracle's pinned packages:

```sh
python3 -m venv .venv-oracle
.venv-oracle/bin/pip install -r tools/oracle/requirements.txt   # asn1tools, cryptography, mcap, PyYAML
```

The tools that need no packages say "standard library" below.

## The Rust validation suites

### `v2xw-netval`: the network stack

Every layer of the network simulation against its reference: the physical layer against
published equations, distributions and receiver tables; access against IEEE 802.11 /
ETSI EN 302 663, ETSI TS 102 687, SAE J2945/1 and the 3GPP sidelink procedures; the
network layer and messages against their header tables and an independent decoder; the
security envelope against OpenSSL; whole runs against conservation, metamorphic and
determinism properties and an independent reference implementation.

| Layer (`Layer`) | Modules |
|---|---|
| Physical | `phy`, `linklevel` |
| Access | `access`, `sps`, `etsi` |
| Network | `net` |
| Security | `sec` |
| End to end | `e2e`, `differential` |

Each `Check` runs in `Mode::Honest` (must pass) and `Mode::Faulted` (a named, realistic
fault such as a rounded constant, a misread table row or a backoff counter that restarts
instead of freezing; must fail). The fault is injected at the boundary between model and
check, so no rebuild per fault is needed. A check that passes with its fault in place is
reported as unable to fail.

```sh
cargo run --release -p v2xw-netval --bin netval-report             # writes target/validation/network.md
cargo run --release -p v2xw-netval --bin netval-report -- --quick  # leaves out the checks that run the engine
cargo run --release -p v2xw-netval --bin netval-report -- --only <ID,...>   # prints, writes nothing
```

The exit status is non-zero when any check fails or cannot fail. A check that needs a
missing external tool reports `Skip` with the reason.

### `v2xw-trafficval`: traffic and mobility

| Group | Module | Holds |
|---|---|---|
| model | `model` | IDM, MOBIL, gap acceptance and social force against their papers; the ring fundamental diagram; string stability; the Sugiyama ring |
| calibration | `calib` | saturation flow, start-up lost time, speeds and accelerations, turning, critical gaps, pedestrians and motorcycles against published measurements |
| safety | `safety` | time-to-collision and post-encroachment time, SSAM thresholds, conflicts per vehicle-kilometre |
| invariants | `invariants`, `nema_safety` | the traffic auditor's classes at zero on every shipped scenario and at peak density; metamorphic properties; no conflicting greens and no red entry under the NEMA dual ring |
| cities | `cities` | Manhattan, Portland and Berlin import and run with the auditor clean |

Each check runs as shipped (its tested rows inside their bands) and with a fault
injected (at least one row must leave its band). A row no fault turned red is reported
as *not proved*. Wall-clock time goes to the terminal only, so two runs write the same
report.

```sh
cargo run --release -p v2xw-trafficval --bin traffic_validation -- [--quick] [--only SUBSTR] \
    [--out target/validation/traffic.md] [--json FILE] [--list]
cargo run --release -p v2xw-trafficval --bin traffic_calibration -- [--quick] [--out FILE]
```

`traffic_calibration` writes `target/validation/traffic-calibration.md`: fundamental
diagrams on a city ring and a freeway ring (Greenshields and triangular fits, capacity
against HCM 6th ed. Exhibit 12-6), stop-and-go wave speed, headways, the human-driver
population, the IDM rest gap and string stability against theory, the Sugiyama ring, and
each figure beside SUMO's default driver (Krauss, σ = 0.5). It exits 1 when a held
figure leaves its band.

`just validation-report [--quick]` runs both report binaries. The city checks need the
city extracts (`worlds/fetch.sh <id>`).

## The SCMS oracle (`tools/oracle`)

An independent implementation of the IEEE 1609.2 / 1609.2.1 codec and cryptography,
written from the standards without reading the Rust. Everything else in this page that
decodes a PDU uses it.

| Part | Contents |
|---|---|
| `asn1/` | the committed ASN.1 modules: `ieee1609.2-2022`, `ieee1609.2.1-2022`, `ieee1609.3` |
| `scms_oracle/coer.py` | `asn1tools` COER over those modules |
| `scms_oracle/{ec,prims,ecqv,butterfly,linkage}.py` | ECDSA, ECQV as profiled by 1609.2, butterfly key expansion, linkage values |
| `scms_oracle/{certs,chain}.py` | certificate construction and the chain rules |
| `scms_oracle/{ctlfiles,real}.py` | the published SCMS Manager CTL and chain files |
| `scms_oracle/{faults,emit,corpus,records,time1609}.py` | named faults, vector emission, corpora, MCAP records, 1609.2 time |
| `transcribe/` | run once by hand: transcribes the IEEE 1609.2a, CAMP, NIST CAVP and RFC 4231 vectors into `crates/v2xw-sec/tests/vectors`, each value with its citation and the source's sha256 (values only; the source texts are not redistributable) |

```sh
.venv-oracle/bin/python -I tools/oracle/oracle.py selftest        # reproduce every vector in crates/v2xw-sec/tests/vectors
.venv-oracle/bin/python -I tools/oracle/oracle.py mutate          # each named fault must turn its target checks red
.venv-oracle/bin/python -I tools/oracle/oracle.py emit [--out DIR]   # write oracle vectors for the Rust tests
.venv-oracle/bin/python -I tools/oracle/oracle.py verify-emitted  # recompute the emitted files byte for byte
.venv-oracle/bin/python -I tools/oracle/oracle.py coer TYPE HEX   # decode and re-encode one value (canonical check)
```

## Codec and certificate cross-checks

| Tool | Proves | Run |
|---|---|---|
| `asn1-xcheck/xcheck.py` | the generated `rasn` bindings in `v2xw-msg` and an independent codec (`asn1tools`, its own parser and X.696 OER encoder) agree byte for byte in both directions on random values of every 1609.2 / 1609.2.1 type | `xcheck.py gen OUT [--n 200] [--seed N]` writes `TypeName HEX` lines that `crates/v2xw-msg/tests/scms_coer_xcheck.rs` decodes and re-encodes; `xcheck.py verify IN` checks lines Rust wrote; `xcheck.py diff-editions` prints the code-level differences between the 2022 and 2025 module editions |
| `chain-xcheck/chain_xcheck.py` | the Rust chain validator (`v2xw_sec::chain::validate`) gives the oracle's verdict on random certificate chains, and re-issues byte-identical certificates | `chain_xcheck.py gen OUT [--n 500] [--seed N]` writes `crates/v2xw-sec/tests/vectors/xcheck/chains.json`, read by `crates/v2xw-sec/tests/chain_xcheck.rs`; `verify OUT` regenerates and compares |
| `scms-artefacts/xcheck_crl.py` | the Rust-emitted CRL and CTL vectors (`crates/v2xw-sec/tests/crl_ctl_vectors.rs`) decode, verify and match under the oracle's own COER, hashing, ECDSA and linkage code | `xcheck_crl.py [DIR]` |
| `pki_view/check_jer.py` | the PKI view's JER rendering of each SPDU layer (`inspect.pdu`) equals `asn1tools`' JER of an independent OER decode | `V2XW_PKI_OUT=<dir> cargo test -p v2xw-proto --test pki_view`, then `check_jer.py <dir>` |

Limits stated by the tools: `asn1tools` implements BASIC-OER, so canonical rules are
checked by byte equality in both directions; open types are encoded per X.696 §30 by a
patch in `xcheck.py`; `WITH COMPONENTS` and `CONTAINING` constraints are enforced by
neither codec. `chain_xcheck.py` generates chains only where the oracle's rules are
complete; the rest (chains longer than 8, bitmap SSPs, sub-region containment,
`certRequestPermissions`, duplicate PSIDs) is covered by Rust negative tests.

## PDU checkers

Each Rust test writes a corpus when its environment variable is set; the Python driver
decodes and verifies every PDU in it with the oracle and prints one verdict line.

| Driver | Corpus | Checks |
|---|---|---|
| `pdu_check/check_pdus.py` | `V2XW_PDU_OUT=<dir> cargo test -p v2xw-proto --test pdu_layer` | every 1609.2.1 SPDU of the provisioning flows: canonical COER of each layer, protocol and PDU versions, PSID 35 header, signer chains (explicit and ECQV-implicit) to the emitted PKI, signatures, ECIES and AES-CCM to the RA |
| `pdu_check/check_appcerts.py` | `V2XW_APPCERT_OUT=<dir> cargo test -p v2xw-proto --test appcert` | the RSU application and OBU identification certificate exchange, including re-derived certificate keys |
| `pdu_check/check_mbr.py` | `V2XW_PDU_OUT=<dir> cargo test -p v2xw-proto --test mbr corpus_for_the_oracle` | US misbehaviour reports: encrypted to the MA certificate, signed by the reporter's pseudonym certificate, PSID 0x26, payload decoded with `pdu_check/asn1/CvpdMbdWire.asn` |
| `pdu_check/check_run_crl.py` | `V2XW_PDU_OUT=<dir> cargo test -p v2xw-proto --test crl_issuance a_revocation_publishes` | issued SecuredCrls: signer, CrlSsp, CRACA, series, and linkage-CRL matching revokes exactly the revoked certificates |
| `pdu_check/mutate_rules.py` | as `check_pdus.py` / `check_appcerts.py` | fault injection for the rule checks those drivers added: each fault, applied after the signature checks, must turn the verdict red |
| `conformance/check_run.py` | `V2XW_CONFORMANCE_OUT=<dir> cargo test -p v2xw-engine --test conformance` | every PDU of a whole engine run (the capture is the recording plus the recipients' private keys): canonical COER, signed and encrypted layers, chains to a CTL the tool verified itself, signer roles. `--mutate` runs its faults; `--json OUT` writes the findings |
| `pki_view/aca_blindness.py` | a run directory | the RA strips the enrolment identity before the ACA (IEEE 1609.2.1 §4.1.3): no ACA-bound PDU contains an enrolment certificate or its HashedId8. `--inject` plants one and the leak count must rise |

!!! info "Independence"
    The primitives in every driver are the oracle's. `check_run.py`'s rules were derived
    from the ASN.1 modules and the design specification only, without reading the Rust
    that builds the PDUs. The `pdu_check/` drivers' rules were written alongside the Rust
    they test, so only their primitives are independent; their headers say so.

## Comparison with published artefacts

| Tool | Compares | Run |
|---|---|---|
| `scms-artefacts/fetch.py` | downloads the published SCMS Manager CTL, chain and elector files into `third_party/scms-artefacts/files/` (not committed: no redistribution licence) and checks each against SCMS Manager's SHA-384 fingerprint and the sha256 in `PROVENANCE.md`. Standard library plus `curl` | `python3 -I tools/scms-artefacts/fetch.py`, then `cargo test -p v2xw-sec --test real_artefacts` |
| `realworld/sizes.py` | the engine's certificates and SPDUs against CAMP SCMS Release 1.2.2 Table 3 sizes, field by field, and against the published certificates when fetched | `V2XW_SIZES_OUT=f.json cargo test -p v2xw-proto --test camp_sizes`, then `sizes.py f.json [--real third_party/scms-artefacts/files]` |
| `realworld/profiles.py` | each engine component certificate's `ToBeSignedCertificate` profile against the published US certificate of the same role | `profiles.py f.json third_party/scms-artefacts/files` |
| `realworld/component_certs.py` | a run's provisioning certificates (from `pki.flow` records) per provider against the published chains | a run directory with `recording.mcap` |
| `realworld/crl_sizes.py` | a 10,000-entry linkage CRL decoded by the oracle against Brecht et al., IEEE T-ITS 2018 §VI-F (about 40 B per entry) | `V2XW_CRL_OUT=<file> cargo test -p v2xw-sec --test crl_size_brecht`, then `crl_sizes.py <file>` |
| `scms_constants/tls_handshake_sizes.py` | the TLS handshake flight sizes of an EE to SCMS session under the US profile (TLS 1.2/1.3, server-only X.509, ECDHE-ECDSA, no resumption), measured between two OpenSSL endpoints in memory; feeds [SCMS constants](../reference/scms-constants.md) | `python -I tools/scms_constants/tls_handshake_sizes.py` |

## Real-data calibration

| Tool | Produces | Consumed by |
|---|---|---|
| `realdata/rdtool.py` | the BSM timing model fit from public US connected-vehicle pilot data: Tampa CV Pilot and UDOT/Panasonic Orem BSMs (fit), Wyoming CV Pilot (held out), NYC CV Pilot events (separate). `fetch --raw DIR` runs the pinned SODA queries (`sources.py`) and writes a manifest; `fit --raw DIR` writes the frozen classifier `realdata/frozen/classifier-v1.json`; `all --raw DIR` verifies the manifest, refits, runs the falsification gate (`fgate.py`: an injected position jump and epoch slip must be detected, shuffled labels must erase the effect, a deliberately broken check must fail) and scores the held-out sets | `v2xw_node::bsm_timing` (`actors.vehicles.bsm_timing`) |
| `realdata/bsm_clock_drift.py`, `fit_off_channel.py` | the free-running BSM clock statistics; the RSU off-channel loss fit (`radio.devices.rsu.off_channel`) | `bsm_timing`, the radio device model |
| `calibration/nyc-dot-hourly-profile.py` | Manhattan's hourly weekday and weekend factors from NYC DOT Automated Traffic Volume Counts (NYC Open Data `7ym2-wayt`). Standard library | `NYC_DOT_MANHATTAN_WEEKDAY`, `NYC_DOT_MANHATTAN_WEEKEND`, `WEEKEND_VOLUME_RATIO` in `crates/v2xw-mobility/src/demand/daily.rs` |
| `regions/build.py` | `crates/v2xw-node/data/regions.bin`: the integer rings and grid the receiver uses to check a 1609.2 `identifiedRegion` against `generationLocation`, from Natural Earth admin-0 (Canada, Mexico) and US Census 1:20m states (country 840 as the union of the 50 states and DC) | `v2xw_node::regions` |

The raw rows of the real-data pipeline are never placed inside the repository; only
derived statistics are committed. `realdata/test_*.py` are its unit tests
(`python3 -I -m unittest`).

## VeReMi Extension support

| Tool | Does |
|---|---|
| `realworld/lust_cut.py` | `edges` cuts a box of the LuST network keeping only edges fully inside it; `stamp` makes netconvert's output byte-reproducible; `measure` reports the cut's extent |
| `realworld/veremi_baseline.py` | a reference machine-learning baseline on a VeReMi Extension export: `features` reads only what a receiver sees (ground truth only for the label), `train` and `repro` fit and reproduce it. Needs numpy and scikit-learn |

## `tools/mutate/mutate.py`

Injects one fault into a source file, runs a command, restores the file and sets its
modification time to now, so the next cargo build recompiles the restored source (a
restored copy with an old mtime would leave cargo linking the faulted build).

```sh
python3 -I tools/mutate/mutate.py <file> '<old>' '<new>' -- cargo test -p <crate> --test <t> <filter>
```

`<old>` must occur exactly once. It prints `RED` (the check failed, so it can fail),
`GREEN` (the fault was not caught) or `RESTORE FAILED`, and exits 0 only for `RED`.

## `tools/docs`

`gen_config_reference.py` turns the engine's published settings surface into
`docs/configuration/reference/` ([Configuration overview](../configuration/overview.md));
`requirements.txt` pins the site generator. See [Contributing](contributing.md#documentation).
