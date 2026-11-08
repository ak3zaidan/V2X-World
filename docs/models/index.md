# Models overview

Every behaviour the engine simulates (a car-following law, a path-loss model, a codec, a
detector, an emission model) is a **model**: a Rust implementation of a family trait plus a
**model card** that states what it computes, from which parameters, with which sources,
and where it is known to be wrong. A model without a card cannot be registered, and a
number a model reads without declaring it on its card is a conformance failure.

Code: `crates/v2xw-core/src/{model,card,registry}.rs`,
`crates/v2xw-engine/src/scenario/publish.rs` (`slots()`, `models()`),
`crates/v2xw-engine/src/wiring.rs` (what a run registers). Every card in the published
catalogue, with every parameter: [Model parameters](../configuration/reference/models.md).

## Families and slots

A **family** (`v2xw_core::card::Family`) is one seam of the architecture: a trait that
extends `Model`, a conformance suite and a Python base class. The set is closed; adding a
family is an engine change.

| Area | Families |
|---|---|
| world and traffic | `world`, `mobility`, `vru`, `weather`, `gnss`, `clock` |
| radio | `propagation`, `fading`, `obstacle`, `phy`, `mac`, `dcc` |
| network | `net`, `fragmenter`, `backhaul`, `cellular`, `backend-net` |
| messages | `codec`, `generator` |
| security | `envelope`, `primitive`, `crypto-backend`, `verification-policy`, `hardware-profile` |
| applications and sensing | `safety-app`, `perception` |
| credential system | `protocol`, `service-model`, `responder` |
| threats | `attacker`, `detector`, `ma-pipeline` |
| measurement | `metric`, `exporter` |
| energy | `emission`, `energy` |

A **slot** is a place in the scenario where a model of one family is chosen. Most slots
take a `ModelChoice`:

```yaml
net:
  fragmenter:
    id: fragmenter/generic-sdu        # the card id
    params: {}                        # overrides of the card's defaults
energy:
  ev_model: energy/ev/vt-cpem         # a slot that takes a bare id
```

`ModelChoice { id, params }` denies unknown fields. Anything not named in `params` keeps
the card's default, so the resolved parameter set is a pure function of the card and the
overrides and can be content-addressed. The slot table (path, family, selectable ids and
whether the engine acts on it) is generated: [Slots](../configuration/reference/models.md#slots).

Some models are chosen by named settings rather than an id slot: the radio stack by
`radio.rat`, `radio.tiers` and `radio.models` ([Radio and network](network.md)), the
signature scheme by `security.signature` ([Security](security.md)), the pseudonym policy by
`security.pseudonym_change.strategy` ([Privacy](privacy.md)).

## Resolving parameters

`ParamSet::resolve(card, overrides)` builds the parameter set a model runs on:

1. start from every parameter's card default;
2. `null` overrides keep the defaults; anything but a JSON object is refused;
3. each override must name a declared parameter (a typo is an error listing the declared
   names, never a silent fallback), have the default's JSON type, and lie inside the
   declared `range` when there is one.

Resolved sets are interned in a `ParamSetStore` by SHA-256 of their canonical JSON, so
provenance records point at a parameter set by id instead of copying it.

## The registry

`v2xw_core::Registry` holds one registration per model id:

| Field | Meaning |
|---|---|
| `card` | the card as registered |
| `licence` | Apache-2.0, MIT, BSD, MPL-2.0 may be linked in process; GPL, LGPL or an unknown licence must run out of process |
| `hosting` | `in-process` (Rust trait object, or batched Python via PyO3) or `out-of-process` (a separate process; its replies are hashed into the run digest instead of its code being linked) |
| `content_hash` | SHA-256 of the card's canonical JSON |

A `ModelRef` is a dense `u32` handle in registration order, cheap to copy into
provenance. Registering a different card under an existing id is refused. A hosted (out-of-tree)
plug-in's card must target the engine's plug-in API version (same major; same minor while
the major is 0).

The run's manifest pins every registered model as `id@version` with its card hash, so a
changed default is a different run. A run registers what it composes
(`wiring::register_all`, plus the radio models it builds, `wiring::register_radio`); the
published catalogue (`publish::catalogue`) registers every crate that offers cards, so the
documentation can list what a scenario could name.

## The model card

`v2xw_core::card::ModelCard`:

| Field | Content |
|---|---|
| `id` | stable id, `^[a-z0-9][a-z0-9-]*(/[a-z0-9-]+)*$`, e.g. `mobility/car-following/idm` |
| `family`, `version`, `api_version` | the seam, the model's semver, the plug-in API it targets |
| `tier` | the fidelity tiers it implements (never empty) |
| `purpose` | what it is for |
| `equations` | each with a name, LaTeX or text, and notes on validity and units |
| `parameters` | every number it reads: name, unit, default, optional range, source, calibration plan |
| `assumptions` | what it assumes of the world or its inputs |
| `limitations` | where it is known to be wrong or inapplicable |
| `ignores` | what this tier leaves out relative to the next one up |
| `sources` | citations for the model as a whole |
| `validation` | status (`unvalidated`, `unit-tested`, `literature-checked`, `field-checked`), references, test names |
| `determinism` | whether it draws random numbers and from which RNG domains |
| `cost` | optional per-call cost estimate |

### Sources and `todo: calibrate`

Every parameter carries a `Source` of one kind: `standard` (a clause), `paper`,
`datasheet`, `dataset`, `code` (another implementation), or **`todo-calibrate`**: a value
the implementer chose that still needs calibration. A `todo-calibrate` parameter must
carry a calibration plan (registry rule R1), and the generated reference lists them.

`ModelCard::validate` refuses, at registration, a card whose id is malformed, whose
`todo-calibrate` parameter has no plan, whose parameter names repeat, whose default lies
outside its own range (otherwise a scenario that never mentions the parameter would run on
an impossible value), or whose version, API version, purpose or tier is empty.

### How to read a card

On the [generated page](../configuration/reference/models.md) each card shows its purpose,
version, tiers, hosting and whether it draws random numbers, then:

1. **Parameters**: the table to override in `params`. Check the source column: `standard`
   and `paper` values are cited; `todo-calibrate` values are choices to vary in a
   sensitivity study.
2. **Equations**: what the code computes, to check against the paper.
3. **Assumptions, Limitations, Ignores**: what not to conclude from the model.
4. **Sources**: what to cite.

## Fidelity tiers

| Tier | Meaning | Typical use |
|---|---|---|
| `abstract` | closed-form or probabilistic stand-ins | 10,000-node runs |
| `medium` | the models the literature agrees on, without frame-level detail (the default) | most studies |
| `high` | frame-level or physically detailed | a focus region or a small scenario |

A scenario picks one tier per family (`radio.tiers` for the radio stack); a model that
offers several branches on the tier selected. The validator refuses combinations that
cannot compose, for example `phy: high` with `mac: medium`. At city scale, a microscopic
focus region runs inside a mesoscopic network (`scale`, `mobility/tiered/meso-micro`).

## Writing a model

```rust
use v2xw_core::card::{Family, ModelCard, Tier};
use v2xw_core::model::Model;

struct FreeSpace { card: ModelCard }

impl Model for FreeSpace {
    fn card(&self) -> &ModelCard { &self.card }
}
```

The family trait (`pub trait Propagation: Model { … }`) supplies the behaviour; `Model`
supplies `id()`, `version()`, `tiers()` and `family()` from the card. Models reach the
engine only through a narrowed context (`v2xw_core::ctx::Ctx`) that gives the clock, keyed
RNG streams and the reads the family is allowed. Walk-through:
[Extending: models](../dev/extending/models.md).

## Registered models

The published catalogue, by family; each id links to its card, and the generated page is
the authoritative list. Radio
propagation, fading and PHY models are registered by the run that composes them and are
documented on [Radio and network](network.md).

| Family | Registered models | Explained on |
|---|---|---|
| [`attacker`](../configuration/reference/models.md#attacker) | [`attacker/jammer/constant`](../configuration/reference/models.md#attackerjammerconstant), [`attacker/jammer/pulsed`](../configuration/reference/models.md#attackerjammerpulsed), [`attacker/jammer/reactive`](../configuration/reference/models.md#attackerjammerreactive) | [Threats](threats.md#jammers-threatsjammers) |
| [`backhaul`](../configuration/reference/models.md#backhaul) | [`backhaul/dsl`](../configuration/reference/models.md#backhauldsl), [`backhaul/fibre`](../configuration/reference/models.md#backhaulfibre), [`backhaul/fixed`](../configuration/reference/models.md#backhaulfixed), [`backhaul/microwave`](../configuration/reference/models.md#backhaulmicrowave) | [Radio and network](network.md#cellular-and-backhaul) |
| [`cellular`](../configuration/reference/models.md#cellular) | [`cellular/uu/cell-capacity-mm1`](../configuration/reference/models.md#cellularuucell-capacity-mm1), [`cellular/uu/fixed-latency`](../configuration/reference/models.md#cellularuufixed-latency), [`cellular/uu/handover-outage`](../configuration/reference/models.md#cellularuuhandover-outage), [`cellular/uu/packet-pf`](../configuration/reference/models.md#cellularuupacket-pf) | [Radio and network](network.md#cellular-and-backhaul) |
| [`clock`](../configuration/reference/models.md#clock) | [`clock/drift/none`](../configuration/reference/models.md#clockdriftnone), [`clock/drift/tcxo-ocxo`](../configuration/reference/models.md#clockdrifttcxo-ocxo) | [Determinism and time](determinism-and-time.md#node-clocks-and-gnss-time) |
| [`crypto-backend`](../configuration/reference/models.md#crypto-backend) | [`crypto-backend/modeled`](../configuration/reference/models.md#crypto-backendmodeled), [`crypto-backend/real`](../configuration/reference/models.md#crypto-backendreal) | [Security](security.md#crypto-backends-securitycrypto_mode) |
| [`dcc`](../configuration/reference/models.md#dcc) | [`dcc/etsi/adaptive-ts102687`](../configuration/reference/models.md#dccetsiadaptive-ts102687), [`dcc/etsi/reactive-ts102687`](../configuration/reference/models.md#dccetsireactive-ts102687), [`dcc/sae/j2945-1-rate-power`](../configuration/reference/models.md#dccsaej2945-1-rate-power), [`dcc/sae/j3161-1-rate`](../configuration/reference/models.md#dccsaej3161-1-rate) | [Radio and network](network.md#congestion-control-dccrs) |
| [`detector`](../configuration/reference/models.md#detector) | [`threat/detector/ts103759-observations`](../configuration/reference/models.md#threatdetectorts103759-observations) | [Threats](threats.md#local-detectors-detectionlocal) |
| [`emission`](../configuration/reference/models.md#emission) | [`energy/emission/hbefa3`](../configuration/reference/models.md#energyemissionhbefa3), [`energy/emission/moves-opmode`](../configuration/reference/models.md#energyemissionmoves-opmode), [`energy/emission/phemlight`](../configuration/reference/models.md#energyemissionphemlight) | [Energy](energy.md#emission-models) |
| [`energy`](../configuration/reference/models.md#energy) | [`energy/ev/kurczveil`](../configuration/reference/models.md#energyevkurczveil), [`energy/ev/vt-cpem`](../configuration/reference/models.md#energyevvt-cpem) | [Energy](energy.md#electric-vehicle-models) |
| [`envelope`](../configuration/reference/models.md#envelope) | [`envelope/etsi-ts103097`](../configuration/reference/models.md#envelopeetsi-ts103097), [`envelope/ieee-1609-2`](../configuration/reference/models.md#envelopeieee-1609-2) | [Security](security.md#the-envelope) |
| [`fragmenter`](../configuration/reference/models.md#fragmenter) | [`fragmenter/cert-cycle-partial-hybrid`](../configuration/reference/models.md#fragmentercert-cycle-partial-hybrid), [`fragmenter/facilities-segmentation`](../configuration/reference/models.md#fragmenterfacilities-segmentation), [`fragmenter/generic-sdu`](../configuration/reference/models.md#fragmentergeneric-sdu), [`fragmenter/none`](../configuration/reference/models.md#fragmenternone) | [Radio and network](network.md#fragmentation-frag) |
| [`generator`](../configuration/reference/models.md#generator) | [`generator/bsm-j2945-1`](../configuration/reference/models.md#generatorbsm-j2945-1), [`generator/cam-en302637-2`](../configuration/reference/models.md#generatorcam-en302637-2) | [Messages](messages.md#generation-rules) |
| [`gnss`](../configuration/reference/models.md#gnss) | [`gnss/error/gauss-markov`](../configuration/reference/models.md#gnsserrorgauss-markov), [`gnss/error/ou-bias-legacy`](../configuration/reference/models.md#gnsserrorou-bias-legacy) | [Traffic](traffic.md#node-beliefs-gnss-and-clock) |
| [`hardware-profile`](../configuration/reference/models.md#hardware-profile) | [`backend/cloud-vm-x86`](../configuration/reference/models.md#backendcloud-vm-x86), [`backend/hsm-appliance-luna7`](../configuration/reference/models.md#backendhsm-appliance-luna7), [`bs/lte-macro-generic`](../configuration/reference/models.md#bslte-macro-generic), [`obu/cohda-mk5`](../configuration/reference/models.md#obucohda-mk5), [`obu/cohda-mk6c-qualcomm-9150`](../configuration/reference/models.md#obucohda-mk6c-qualcomm-9150), [`obu/generic-automotive-soc-no-hsm`](../configuration/reference/models.md#obugeneric-automotive-soc-no-hsm), [`obu/pq-capable-hypothetical`](../configuration/reference/models.md#obupq-capable-hypothetical), [`obu/unex-obu-301-craton2`](../configuration/reference/models.md#obuunex-obu-301-craton2), [`obu/unex-obu-301-craton2-software`](../configuration/reference/models.md#obuunex-obu-301-craton2-software), [`rsu/cohda-mk5-rsu`](../configuration/reference/models.md#rsucohda-mk5-rsu), [`rsu/commsignia-its-rs4`](../configuration/reference/models.md#rsucommsignia-its-rs4), [`vru-device/handset-generic`](../configuration/reference/models.md#vru-devicehandset-generic) | [Security](security.md#the-nodes-security-stack-cratesv2xw-node) |
| [`ma-pipeline`](../configuration/reference/models.md#ma-pipeline) | [`threat/ma/legacy-window`](../configuration/reference/models.md#threatmalegacy-window) | [Threats](threats.md#the-misbehaviour-authority-detectionma) |
| [`metric`](../configuration/reference/models.md#metric) | [`metrics/energy`](../configuration/reference/models.md#metricsenergy) | [Energy](energy.md#metrics) |
| [`mobility`](../configuration/reference/models.md#mobility) | [`mobility/car-following/acc`](../configuration/reference/models.md#mobilitycar-followingacc), [`mobility/car-following/cacc-milanes`](../configuration/reference/models.md#mobilitycar-followingcacc-milanes), [`mobility/car-following/cacc-path`](../configuration/reference/models.md#mobilitycar-followingcacc-path), [`mobility/car-following/cacc-ploeg`](../configuration/reference/models.md#mobilitycar-followingcacc-ploeg), [`mobility/car-following/eidm`](../configuration/reference/models.md#mobilitycar-followingeidm), [`mobility/car-following/gipps`](../configuration/reference/models.md#mobilitycar-followinggipps), [`mobility/car-following/idm`](../configuration/reference/models.md#mobilitycar-followingidm), [`mobility/car-following/krauss`](../configuration/reference/models.md#mobilitycar-followingkrauss), [`mobility/car-following/w99`](../configuration/reference/models.md#mobilitycar-followingw99), [`mobility/classes/legacy-fleet`](../configuration/reference/models.md#mobilityclasseslegacy-fleet), [`mobility/classes/sumo-vtypes`](../configuration/reference/models.md#mobilityclassessumo-vtypes), [`mobility/classes/tr37885-types`](../configuration/reference/models.md#mobilityclassestr37885-types), [`mobility/demand/od-gravity`](../configuration/reference/models.md#mobilitydemandod-gravity), [`mobility/demand/poisson-thinned`](../configuration/reference/models.md#mobilitydemandpoisson-thinned), [`mobility/demand/tr36885-drop`](../configuration/reference/models.md#mobilitydemandtr36885-drop), [`mobility/intersection/gap-acceptance-hcm`](../configuration/reference/models.md#mobilityintersectiongap-acceptance-hcm), [`mobility/intersection/signal-fixed-time`](../configuration/reference/models.md#mobilityintersectionsignal-fixed-time), [`mobility/intersection/two-coloring-legacy`](../configuration/reference/models.md#mobilityintersectiontwo-coloring-legacy), [`mobility/kinematic/lane-follow`](../configuration/reference/models.md#mobilitykinematiclane-follow), [`mobility/lane-change/mobil`](../configuration/reference/models.md#mobilitylane-changemobil), [`mobility/lateral/sublane-sl2015`](../configuration/reference/models.md#mobilitylateralsublane-sl2015), [`mobility/meso/queue`](../configuration/reference/models.md#mobilitymesoqueue), [`mobility/native/medium`](../configuration/reference/models.md#mobilitynativemedium), [`mobility/routing/dijkstra`](../configuration/reference/models.md#mobilityroutingdijkstra), [`mobility/routing/dynamic-reroute`](../configuration/reference/models.md#mobilityroutingdynamic-reroute), [`mobility/tiered/meso-micro`](../configuration/reference/models.md#mobilitytieredmeso-micro), [`signals/nema-dual-ring`](../configuration/reference/models.md#signalsnema-dual-ring) | [Traffic](traffic.md) |
| [`net`](../configuration/reference/models.md#net) | [`net/gn-btp/en302636`](../configuration/reference/models.md#netgn-btpen302636), [`net/wsmp/1609-3`](../configuration/reference/models.md#netwsmp1609-3) | [Radio and network](network.md#network-layer-cratesv2xw-net) |
| [`primitive`](../configuration/reference/models.md#primitive) | [`primitive/aes-128`](../configuration/reference/models.md#primitiveaes-128), [`primitive/aes-128-ccm`](../configuration/reference/models.md#primitiveaes-128-ccm), [`primitive/butterfly-expand-p256`](../configuration/reference/models.md#primitivebutterfly-expand-p256), [`primitive/ecdh-p256`](../configuration/reference/models.md#primitiveecdh-p256), [`primitive/ecdsa-brainpoolp256r1`](../configuration/reference/models.md#primitiveecdsa-brainpoolp256r1), [`primitive/ecdsa-p256-sha256`](../configuration/reference/models.md#primitiveecdsa-p256-sha256), [`primitive/ecdsa-p384`](../configuration/reference/models.md#primitiveecdsa-p384), [`primitive/ecies-p256`](../configuration/reference/models.md#primitiveecies-p256), [`primitive/ecqv-p256`](../configuration/reference/models.md#primitiveecqv-p256), [`primitive/ed25519`](../configuration/reference/models.md#primitiveed25519), [`primitive/falcon-1024`](../configuration/reference/models.md#primitivefalcon-1024), [`primitive/falcon-512`](../configuration/reference/models.md#primitivefalcon-512), [`primitive/hybrid-falcon512-ecdsa-p256`](../configuration/reference/models.md#primitivehybrid-falcon512-ecdsa-p256), [`primitive/hybrid-mldsa44-ecdsa-p256`](../configuration/reference/models.md#primitivehybrid-mldsa44-ecdsa-p256), [`primitive/hybrid-mldsa65-ecdsa-p256`](../configuration/reference/models.md#primitivehybrid-mldsa65-ecdsa-p256), [`primitive/linkage-value`](../configuration/reference/models.md#primitivelinkage-value), [`primitive/ml-dsa-44`](../configuration/reference/models.md#primitiveml-dsa-44), [`primitive/ml-dsa-65`](../configuration/reference/models.md#primitiveml-dsa-65), [`primitive/ml-dsa-87`](../configuration/reference/models.md#primitiveml-dsa-87), [`primitive/ml-kem-1024`](../configuration/reference/models.md#primitiveml-kem-1024), [`primitive/ml-kem-512`](../configuration/reference/models.md#primitiveml-kem-512), [`primitive/ml-kem-768`](../configuration/reference/models.md#primitiveml-kem-768), [`primitive/sha-256`](../configuration/reference/models.md#primitivesha-256), [`primitive/sha-384`](../configuration/reference/models.md#primitivesha-384), [`primitive/slh-dsa-sha2-128f`](../configuration/reference/models.md#primitiveslh-dsa-sha2-128f), [`primitive/slh-dsa-sha2-128s`](../configuration/reference/models.md#primitiveslh-dsa-sha2-128s), [`primitive/slh-dsa-sha2-192f`](../configuration/reference/models.md#primitiveslh-dsa-sha2-192f), [`primitive/slh-dsa-sha2-192s`](../configuration/reference/models.md#primitiveslh-dsa-sha2-192s), [`primitive/slh-dsa-sha2-256f`](../configuration/reference/models.md#primitiveslh-dsa-sha2-256f), [`primitive/slh-dsa-sha2-256s`](../configuration/reference/models.md#primitiveslh-dsa-sha2-256s) | [Security](security.md#signature-and-kem-schemes-securitysignature-securitykem) |
| [`propagation`](../configuration/reference/models.md#propagation) | [`antenna/vehicle/tr37885-option1`](../configuration/reference/models.md#antennavehicletr37885-option1) | [Radio and network](network.md#link-budget) |
| [`protocol`](../configuration/reference/models.md#protocol) | [`protocol/etsi/ts102941`](../configuration/reference/models.md#protocoletsits102941), [`protocol/scms/camp`](../configuration/reference/models.md#protocolscmscamp) | [SCMS](scms.md) |
| [`safety-app`](../configuration/reference/models.md#safety-app) | [`safety-app/eebl-vsca`](../configuration/reference/models.md#safety-appeebl-vsca), [`safety-app/fcw-vsca`](../configuration/reference/models.md#safety-appfcw-vsca), [`safety-app/ima-vsca`](../configuration/reference/models.md#safety-appima-vsca) | [Applications](applications.md#surrogate-measures) |
| [`verification-policy`](../configuration/reference/models.md#verification-policy) | [`verification-policy/on-demand`](../configuration/reference/models.md#verification-policyon-demand), [`verification-policy/prioritized`](../configuration/reference/models.md#verification-policyprioritized), [`verification-policy/verify-all`](../configuration/reference/models.md#verification-policyverify-all) | [Security](security.md#the-nodes-security-stack-cratesv2xw-node) |
| [`vru`](../configuration/reference/models.md#vru) | [`vru/pedestrian/social-force`](../configuration/reference/models.md#vrupedestriansocial-force) | [Traffic](traffic.md#vulnerable-road-users) |
| [`world`](../configuration/reference/models.md#world) | [`world/source/opendrive`](../configuration/reference/models.md#worldsourceopendrive), [`world/source/osm`](../configuration/reference/models.md#worldsourceosm), [`world/source/procedural-grid`](../configuration/reference/models.md#worldsourceprocedural-grid), [`world/source/procedural-radial`](../configuration/reference/models.md#worldsourceprocedural-radial), [`world/source/procedural-random`](../configuration/reference/models.md#worldsourceprocedural-random), [`world/source/sumo-net`](../configuration/reference/models.md#worldsourcesumo-net), [`world/terrain/copernicus-glo-30`](../configuration/reference/models.md#worldterraincopernicus-glo-30), [`world/terrain/los-profile`](../configuration/reference/models.md#worldterrainlos-profile), [`world/terrain/srtm-30`](../configuration/reference/models.md#worldterrainsrtm-30) | [Traffic](traffic.md#world) |

## Model pages

| Page | Covers |
|---|---|
| [Traffic and mobility](traffic.md) | world import, car-following, lane change, intersections and signals, VRUs, routing and demand, GNSS |
| [Radio and network](network.md) | link budget, 802.11p, C-V2X sidelink, congestion control, cellular, network layer, fragmentation |
| [Messages](messages.md) | BSM, CAM, DENM, VAM, CPM, SPaT, MAP, PSM, WSA codecs; generation rules; sizes |
| [V2X applications](applications.md) | FCW, EEBL, IMA, LTA, BSW, PCW, RLVW, GLOSA; truth labelling; signal priority; perception |
| [Energy and emissions](energy.md) | HBEFA 3, PHEMlight, MOVES, EV energy, health bands |
| [SCMS and credentials](scms.md) | CAMP SCMS, ETSI PKI, threshold protocols, calendar |
| [Security and crypto](security.md) | IEEE 1609.2 envelope, crypto backends, schemes, the node's security stack |
| [Threats and misbehaviour detection](threats.md) | attackers, jammers, detectors, the misbehaviour authority |
| [Privacy](privacy.md) | pseudonym change, the passive observer, linkability |
| [Determinism and time](determinism-and-time.md) | time, the event queue, RNG streams, digests, checkpoints |
