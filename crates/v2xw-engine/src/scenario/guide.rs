//! The settings surface for a person who is not a V2X engineer.
//!
//! `publish` says what every field *is*; this module says what a newcomer needs to use it:
//!
//! * a **level** for every field — `basic` for the few dozen that decide what a run models,
//!   `advanced` for model internals, fidelity tiers and developer switches — so the window
//!   can open on a page a newcomer can read top to bottom;
//! * for each basic field, one sentence on **what it is in the real world** and a
//!   **recommended range** in words;
//! * the **quick settings**: the dozen controls the Studio shows one click from the map,
//!   each bound to a field (whose choices and presets the page reads from the field's own
//!   row) or to a set of coherent choices, each choice a patch the loader accepts;
//! * the **studies**: "what would you like to study?" starting points, each a patch that
//!   sets a coherent group of settings.
//!
//! Every choice here is built from the engine's own tables (`validate`, `options`,
//! `v2xw_sec::scheme`, `crate::export`), so a scheme, an attack or an exporter added to
//! the engine is offered here without an edit. The tests apply every patch to the minimal
//! scenario and run the loader's validator over it: a quick setting the loader would refuse
//! cannot ship.

use serde_json::{Value, json};

use super::options;
use super::publish::pointer_for;

/// One basic field: its path, what it is in the real world, and the range to start in.
struct Basic {
    path: &'static str,
    explain: &'static str,
    recommended: &'static str,
}

/// The basic fields, in no particular order (the window keeps the engine's group order).
///
/// Everything not listed is `advanced`. The recommended ranges are this build's advice for
/// an interactive run on this machine class, with the source of any number that has one.
const BASIC: &[Basic] = &[
    Basic {
        path: "meta.name",
        explain: "The name this run is saved and listed under.",
        recommended: "Anything that tells you which experiment it was.",
    },
    Basic {
        path: "seed",
        explain: "The random seed: the same settings and seed always give exactly the same run; change it to see another, equally likely, day.",
        recommended: "Any number. Keep it fixed while you compare settings.",
    },
    Basic {
        path: "time.duration_s",
        explain: "How much simulated time the run covers. A vehicle changes its pseudonym about every 5 minutes (SAE J2945/1), so privacy and credential studies need longer runs than radio studies.",
        recommended: "1 to 15 minutes to watch; 1 hour or more as a batch run.",
    },
    Basic {
        path: "actors.vehicles.demand.rate_veh_per_h",
        explain: "How many vehicles drive into the map each hour. More vehicles means more radios sharing one channel, which is what makes V2X congest.",
        recommended: "500 (night) to 4,500 (rush hour) on a city extract.",
    },
    Basic {
        path: "actors.vehicles.demand.fleet_size",
        explain: "Keeps exactly this many vehicles driving for the whole run: when one finishes its trip a new one sets off, as in a field trial with a fixed test fleet. Leave it empty to let vehicles arrive at the hourly rate instead.",
        recommended: "2 to 50 to watch individual vehicles; 100 to 300 for channel load on this machine.",
    },
    Basic {
        path: "traffic.signals.control",
        explain: "How the traffic lights decide when to change: on a fixed clock, or actuated by detectors in the road that hold a green while cars keep coming and end it early when the street goes quiet, as most US suburban and many city signals do.",
        recommended: "fixed-time for a repeatable baseline; actuated to study signal timing or SPaT that changes.",
    },
    Basic {
        path: "traffic.gridlock_timeout_s",
        explain: "When a vehicle has been stuck this long in a jam the network cannot clear, it is moved on along its route and counted, as SUMO does. A run that counts many has unrealistic demand or a network fault.",
        recommended: "300 (SUMO's default); 0 to see raw gridlock.",
    },
    Basic {
        path: "traffic.reroute_period_s",
        explain: "How often drivers check for a faster way round the traffic, as a navigation app does. Off, every driver keeps the route it chose on free-flowing roads; on, jams push traffic onto parallel streets.",
        recommended: "0 (off) for a baseline; 60 to 300 to model navigation-app drivers.",
    },
    Basic {
        path: "traffic.right_turn_on_red",
        explain: "Whether drivers may turn right on a red light after stopping. Legal in most of the US, banned in New York City.",
        recommended: "Leave empty to follow the city's rule.",
    },
    Basic {
        path: "actors.vehicles.equipped_fraction",
        explain: "The share of vehicles that carry a V2X radio. Today's deployments are a few percent; a mandate would equip every new vehicle.",
        recommended: "0.05 (early deployment) to 1 (every vehicle).",
    },
    Basic {
        path: "actors.vehicles.classes",
        explain: "The fleet mix: the share of cars, vans, trucks, buses, motorcycles and mopeds. Larger vehicles block the radio more and drive differently.",
        recommended: "Leave empty for all cars, or shares that add up to 1.",
    },
    Basic {
        path: "actors.vehicles.drivers.realism",
        explain: "How human the drivers are. Real drivers differ from one another, react late and misjudge gaps, which is what turns dense traffic into stop-and-go waves (Treiber, Kesting & Helbing 2006).",
        recommended: "Calibrated for city runs; human for congestion and shockwave studies; identical to test a model in isolation.",
    },
    Basic {
        path: "actors.vehicles.drivers.reaction_time_s",
        explain: "How long a driver takes to respond to the car ahead. 0.7 s is a prepared driver's brake reaction, 1.25 s a surprised one's (Green 2000).",
        recommended: "0.6 to 1.2 s; leave unset to use the realism preset's.",
    },
    Basic {
        path: "actors.vru.pedestrians",
        explain: "People walking on the sidewalks and crossings. Those with a phone or wearable can broadcast their own safety messages (PSM).",
        recommended: "0 to 400 at once.",
    },
    Basic {
        path: "actors.vru.cyclists",
        explain: "People riding bicycles and e-bikes among the traffic.",
        recommended: "0 to 150 at once.",
    },
    Basic {
        path: "actors.vru.device_fraction",
        explain: "The share of pedestrians and cyclists carrying a device that sends V2X messages.",
        recommended: "0 to 0.5; few phones broadcast V2X today.",
    },
    Basic {
        path: "actors.rsu_placement.count",
        explain: "Roadside units are radios mounted at intersections: they forward misbehaviour reports, hand out revocation lists and broadcast signal timing. The engine places this many at the busiest signalised junctions.",
        recommended: "0 to 20 on a city extract.",
    },
    Basic {
        path: "actors.rsus",
        explain: "Roadside units you place yourself, each at a position or a junction, with its own hardware and backhaul.",
        recommended: "Usually leave empty and set the number of roadside units instead.",
    },
    Basic {
        path: "weather.initial",
        explain: "The weather at the start of the run: rain and fog slow drivers down and weaken radio signals.",
        recommended: "Clear, unless weather is what you study.",
    },
    Basic {
        path: "weather.intensity",
        explain: "How strong the weather is, on its model's 0 to 1 scale.",
        recommended: "0.3 (light) to 0.8 (heavy).",
    },
    Basic {
        path: "radio.rat",
        explain: "Which V2X radio vehicles use: DSRC (IEEE 802.11p, the original Wi-Fi-based standard), LTE-V2X (4G sidelink, 3GPP Release 14) or NR-V2X (5G sidelink, Release 16).",
        recommended: "DSRC or LTE-V2X: the two deployed today.",
    },
    Basic {
        path: "radio.devices.obu.tx_power_dbm",
        explain: "How loudly each vehicle transmits. Louder reaches farther but interferes with more neighbours.",
        recommended: "20 dBm (SAE J2945/1) to 23 dBm (3GPP TR 36.885).",
    },
    Basic {
        path: "messages.sets",
        explain: "Which messages are broadcast: BSM is the US basic safety message (position, speed, heading, 10 times a second); CAM is the European one; SPaT and MAP are traffic-light timing and junction maps from roadside units.",
        recommended: "BSM for US studies, CAM for European ones.",
    },
    Basic {
        path: "security.protocol.id",
        explain: "Which credential system issues the certificates vehicles sign with: the US Security Credential Management System (SCMS) or the European C-ITS trust model (ETSI TS 102 941).",
        recommended: "The one that matches the message format.",
    },
    Basic {
        path: "security.signature",
        explain: "The digital signature on every message. ECDSA is what is deployed today; post-quantum signatures resist a quantum computer but are many times larger.",
        recommended: "ECDSA P-256, or a hybrid to study the cost of post-quantum security.",
    },
    Basic {
        path: "security.verification_policy",
        explain: "Which received messages a vehicle checks the signature of. Checking everything is safest but can overload the security chip in dense traffic.",
        recommended: "Verify every message, unless you study verification load.",
    },
    Basic {
        path: "security.pseudonym_change.strategy",
        explain: "When a vehicle switches to a new anonymous certificate, so an eavesdropper cannot follow it across the city.",
        recommended: "Time-based every 5 minutes (SAE J2945/1).",
    },
    Basic {
        path: "security.pseudonym_change.period_s",
        explain: "How often a vehicle changes pseudonym under the time-based strategy.",
        recommended: "60 to 300 s; 300 s is SAE J2945/1's interval.",
    },
    Basic {
        path: "nodes.default_obu",
        explain: "The on-board unit hardware vehicles carry. It decides how long signing and verifying a message take.",
        recommended: "A shipped automotive profile; a post-quantum signature needs one with figures for it.",
    },
    Basic {
        path: "threats.attackers",
        explain: "Misbehaving vehicles that send false messages, such as fake positions or speeds, while holding valid certificates.",
        recommended: "Empty, or one attack on 1 to 10 % of vehicles.",
    },
    Basic {
        path: "threats.jammers",
        explain: "Radio transmitters that drown the V2X channel in noise near a position.",
        recommended: "Empty, or one jammer to study denial of service.",
    },
    Basic {
        path: "detection.local",
        explain: "Plausibility checks each vehicle runs on what it hears, which flag and report suspicious senders.",
        recommended: "The standard detector suite when attackers are on.",
    },
    Basic {
        path: "detection.ma.id",
        explain: "The misbehaviour authority: the backend service that reads reports and decides whom to revoke.",
        recommended: "Corroborated, which needs reports from several vehicles.",
    },
    Basic {
        path: "apps.enabled",
        explain: "The safety and efficiency applications vehicles run on what they hear, such as forward collision warning or green-light speed advice.",
        recommended: "The applications whose benefit you want to measure.",
    },
    Basic {
        path: "time.warmup_s",
        explain: "How long the run fills with traffic before it is measured: charts and metric summaries skip it, so an empty map filling up does not drag the averages down.",
        recommended: "10 to 20 % of the run; 30 to 60 s on a city extract.",
    },
    Basic {
        path: "radio.phy.data_rate_mbps",
        explain: "The 802.11p data rate (modulation and coding). Faster rates finish each frame sooner, so more radios fit on the channel, but they need a stronger signal, so range shrinks.",
        recommended: "6 Mbit/s (the US default); 3 for range, 12 to 18 for crowded channels.",
    },
    Basic {
        path: "radio.channel_model.fading",
        explain: "The fast signal fluctuation as vehicles move a few centimetres: Rayleigh has no direct path, Rice and Nakagami have one. It is the main cause of losses well inside range.",
        recommended: "Leave unset (Nakagami m = 3); Rayleigh for a worst case.",
    },
    Basic {
        path: "radio.dcc.enabled",
        explain: "Congestion control: radios slow down or turn their power down when the channel is busy, so the channel stays usable. Turn it off to measure what it buys.",
        recommended: "On, except as the baseline of a congestion study.",
    },
    Basic {
        path: "actors.vehicles.drivers.time_headway_s",
        explain: "The time gap drivers keep to the car ahead. Shorter gaps fit more cars on the road and leave less time to react to a warning.",
        recommended: "1.0 s (aggressive) to 2.0 s (cautious); about 1.5 s typical.",
    },
    Basic {
        path: "messages.rate_hz",
        explain: "How many awareness messages each vehicle sends per second, at a fixed rate. Unset, vehicles follow the standards: 10 per second in the US, and in Europe as often as their motion changes.",
        recommended: "Unset; 10 Hz to match ns-3 and Veins; 1 to 20 Hz to study load.",
    },
    Basic {
        path: "exporters",
        explain: "Which files the run writes when it finishes, for analysis outside the Studio.",
        recommended: "None to explore; Parquet to analyse in Python or R.",
    },
];

/// The explanation and recommended range of advanced fields that have them: the
/// researcher-grade radio numbers (`radio.phy`, `radio.mac`, `radio.channel_model`,
/// `radio.dcc`), which a newcomer does not need but should still be able to read.
const ADVANCED: &[Basic] = &[
    Basic {
        path: "actors.vehicles.drivers.max_accel_mps2",
        explain: "How hard a car driver accelerates when the road ahead is clear.",
        recommended: "1.0 to 2.0 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.comfort_decel_mps2",
        explain: "How hard a car driver brakes in normal driving (emergency braking is harder).",
        recommended: "1.5 to 3.0 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.min_gap_m",
        explain: "The distance drivers leave to the car in front when queued at a light.",
        recommended: "1.5 to 3 m.",
    },
    Basic {
        path: "actors.vehicles.drivers.politeness",
        explain: "How much drivers care about the cars behind them when changing lanes: 0 is selfish, 1 fully considerate.",
        recommended: "0.2 to 0.5.",
    },
    Basic {
        path: "actors.vehicles.drivers.lane_change_threshold_mps2",
        explain: "The least advantage a lane change must bring before a driver bothers.",
        recommended: "0.1 to 0.3 m/s².",
    },
    Basic {
        path: "radio.sidelink.reservation_interval_ms",
        explain: "How often a C-V2X radio's booked slot repeats; it should match how often the radio has a message to send.",
        recommended: "100 ms for 10 Hz BSMs; 20 or 50 ms for faster traffic.",
    },
    Basic {
        path: "radio.sidelink.resource_keep_probability",
        explain: "The chance a radio keeps its booked slot when its counter runs out, rather than picking a new one.",
        recommended: "0.8 (J3161/1); 0 to study fast reselection.",
    },
    Basic {
        path: "radio.sidelink.rsrp_threshold_dbm",
        explain: "How strong another radio's booking must be for a radio to avoid that slot.",
        recommended: "-128 dBm (NR studies) to -110 dBm (LTE studies).",
    },
    Basic {
        path: "radio.sidelink.sensing",
        explain: "Whether radios listen before booking a slot. Off is the random-selection baseline.",
        recommended: "On; off only as a baseline.",
    },
    Basic {
        path: "radio.phy.noise_figure_db",
        explain: "How much noise the receiver's own electronics add; every extra dB costs about 10 % of range in free space.",
        recommended: "6 dB (automotive module) to 9 dB (3GPP UE).",
    },
    Basic {
        path: "radio.phy.sensitivity_dbm",
        explain: "The weakest frame the receiver decodes, at every data rate (Veins' minPowerLevel).",
        recommended: "-92 dBm (fielded units) to -85 dBm (ETSI minimum).",
    },
    Basic {
        path: "radio.phy.cca_threshold_dbm",
        explain: "The signal level at which a radio hears the channel as busy and holds back; it also sets what the channel busy ratio counts.",
        recommended: "-85 dBm (ETSI); -82 (ns-3); -65 (Veins).",
    },
    Basic {
        path: "radio.phy.capture_threshold_db",
        explain: "Replaces the error model with a hard rule: a frame survives only if it stays this many dB above interference plus noise.",
        recommended: "Unset (error model); 4 to 10 dB to compare with threshold-based simulators.",
    },
    Basic {
        path: "radio.mac.periodic_access_category",
        explain: "The priority queue awareness messages wait in; higher priorities wait less before sending.",
        recommended: "Voice (US BSM) or video (European CAM).",
    },
    Basic {
        path: "radio.mac.event_access_category",
        explain: "The priority queue hazard warnings (DENM) wait in.",
        recommended: "Voice.",
    },
    Basic {
        path: "radio.mac.cw_min_vo",
        explain: "The random back-off window of the voice queue: a wider window spreads transmissions out, which means fewer collisions and more delay.",
        recommended: "3 (standard) to 15 in dense traffic.",
    },
    Basic {
        path: "radio.mac.aifsn_vo",
        explain: "The fixed idle wait of the voice queue before its back-off countdown.",
        recommended: "2 (standard).",
    },
    Basic {
        path: "radio.mac.cw_min_vi",
        explain: "The random back-off window of the video queue.",
        recommended: "7 (standard).",
    },
    Basic {
        path: "radio.mac.aifsn_vi",
        explain: "The fixed idle wait of the video queue.",
        recommended: "3 (standard).",
    },
    Basic {
        path: "radio.mac.queue_length",
        explain: "How many frames wait in each priority queue before new ones are dropped.",
        recommended: "16 to 64.",
    },
    Basic {
        path: "radio.channel_model.path_loss_exponent",
        explain: "How fast the signal weakens with distance: 2 in free space, near 3 in a built-up street. Setting it runs the log-distance law.",
        recommended: "1.8 (highway) to 3.5 (dense city).",
    },
    Basic {
        path: "radio.channel_model.shadowing_sigma_db",
        explain: "How much the signal wanders around its average as buildings and trucks block it.",
        recommended: "3 to 4 dB in line of sight, 6 to 7 dB obstructed.",
    },
    Basic {
        path: "radio.channel_model.shadowing_decorrelation_m",
        explain: "How far a vehicle travels before its shadowing changes completely.",
        recommended: "10 m (city) to 50 m (highway).",
    },
    Basic {
        path: "radio.channel_model.nakagami_shape",
        explain: "The Nakagami shape m: 1 is Rayleigh, larger values are milder fading.",
        recommended: "1 to 5.",
    },
    Basic {
        path: "radio.channel_model.rice_k_db",
        explain: "The strength of the direct path over the scattered ones in a Rice channel.",
        recommended: "3 to 10 dB.",
    },
    Basic {
        path: "radio.dcc.cbr_target",
        explain: "The share of time busy the ETSI adaptive algorithm aims the channel at.",
        recommended: "0.6 to 0.7 (standard 0.68).",
    },
    Basic {
        path: "radio.dcc.adaptive_alpha",
        explain: "How quickly the ETSI adaptive algorithm's load average follows new measurements.",
        recommended: "0.016 (standard).",
    },
    Basic {
        path: "radio.dcc.adaptive_beta",
        explain: "How strongly the ETSI adaptive algorithm changes the allowed airtime when the load is off target.",
        recommended: "0.0012 (standard).",
    },
    Basic {
        path: "radio.dcc.max_itt_ms",
        explain: "The longest gap J2945/1 allows between one vehicle's BSMs under congestion.",
        recommended: "600 ms (standard).",
    },
    Basic {
        path: "radio.dcc.density_coefficient",
        explain: "How many neighbours within 100 m J2945/1 tolerates before it stretches the BSM interval.",
        recommended: "25 (standard).",
    },
    Basic {
        path: "radio.dcc.power_control",
        explain: "Whether J2945/1 also turns the radiated power down as the channel fills.",
        recommended: "On (J2945/1); off for rate-only control.",
    },
    Basic {
        path: "radio.dcc.min_power_dbm",
        explain: "The lowest power J2945/1's power control may reach.",
        recommended: "10 dBm (standard).",
    },
    Basic {
        path: "radio.channel_model.nakagami_shape_far",
        explain: "The fading severity beyond the breakpoint distance, for a channel that gets harsher with range.",
        recommended: "0.75 to 1 beyond about 80-100 m.",
    },
    Basic {
        path: "radio.channel_model.nakagami_breakpoint_m",
        explain: "The distance at which the fading switches to its far-away severity.",
        recommended: "80 to 100 m.",
    },
    Basic {
        path: "radio.channel_model.building_wall_loss_db",
        explain: "How much each building wall between two cars costs the signal (Sommer 2011's β, as Veins models it).",
        recommended: "9 dB (Sommer 2011 default); 2.4 dB for light construction.",
    },
    Basic {
        path: "radio.channel_model.building_loss_db_per_m",
        explain: "How much each metre of the path inside a building costs (Sommer 2011's γ).",
        recommended: "0.4 dB/m (Sommer 2011 default); 0.1-0.63 dB/m across the fitted rows.",
    },
    Basic {
        path: "radio.channel_model.vehicle_blockage",
        explain: "Whether trucks and cars between two antennas weaken the signal (3GPP TR 37.885 NLOSv).",
        recommended: "On for realism; off to measure what blockage costs.",
    },
    Basic {
        path: "radio.sidelink.candidate_percent",
        explain: "The share of resources that must remain after exclusion before a radio picks one.",
        recommended: "20 % (LTE's fixed value); 35 or 50 % on NR.",
    },
    Basic {
        path: "radio.sidelink.selection_window_ms",
        explain: "How late after a message is ready its new resource may be. Shorter cuts latency but leaves fewer resources to pick from.",
        recommended: "100 ms at 10 Hz; at most the message interval.",
    },
];

/// `basic` or `advanced`.
pub fn level_of(path: &str) -> &'static str {
    if basic(path).is_some() {
        "basic"
    } else {
        "advanced"
    }
}

fn basic(path: &str) -> Option<&'static Basic> {
    BASIC.iter().find(|b| b.path == path)
}

fn hint(path: &str) -> Option<&'static Basic> {
    basic(path).or_else(|| ADVANCED.iter().find(|b| b.path == path))
}

/// The real-world sentence for a basic field, or an advanced one that carries one.
pub fn explain(path: &str) -> Option<&'static str> {
    hint(path).map(|b| b.explain)
}

/// The recommended range, in words, for a basic field, or an advanced one that carries
/// one.
pub fn recommended(path: &str) -> Option<&'static str> {
    hint(path).map(|b| b.recommended)
}

/// One write of a patch: the pointer and the value it puts there.
fn set(path: &str, value: Value) -> Value {
    json!({"path": path, "pointer": pointer_for(path), "value": value})
}

/// A choice of a quick setting or a study: its words and its patch.
fn choice(label: &str, description: &str, group: Option<&str>, patch: Vec<Value>) -> Value {
    let mut v = json!({"label": label, "description": description, "patch": patch});
    if let Some(g) = group {
        v["group"] = json!(g);
    }
    v
}

/// The published label and description of `value` at `path`.
fn words(path: &str, value: &str) -> (String, String) {
    let row = options::option_row(path, value, "");
    (
        row["label"].as_str().unwrap_or(value).to_string(),
        row["description"].as_str().unwrap_or("").to_string(),
    )
}

/// The hardware and transport a signature larger than an ECDSA one needs: a fragmenter, so
/// an SPDU above the MTU is sent in pieces, and a profile with post-quantum figures (the
/// pair `scenarios/pq-signatures.yaml` runs every scheme with).
fn pq_support() -> Vec<Value> {
    vec![
        set(
            "net.fragmenter",
            json!({"id": v2xw_net::FRAGMENTER_GENERIC_ID}),
        ),
        set(
            "nodes.default_obu",
            json!("obu/generic-automotive-soc-no-hsm"),
        ),
        set("security.crypto_mode", json!("modeled")),
    ]
}

fn signature_choices() -> Vec<Value> {
    use v2xw_sec::scheme::{SIGNATURE_SCHEMES, SchemeMode};
    SIGNATURE_SCHEMES
        .iter()
        .map(|s| {
            let (label, description) = words("security.signature", s.id);
            let label = if label == s.id {
                s.label.to_string()
            } else {
                label
            };
            let (group, mut patch) = match s.mode {
                SchemeMode::Classical => ("Classical (deployed today)", Vec::new()),
                SchemeMode::Hybrid => ("Hybrid post-quantum", pq_support()),
                SchemeMode::PostQuantum => ("Post-quantum", pq_support()),
            };
            patch.insert(0, set("security.signature", json!(s.id)));
            choice(&label, &description, Some(group), patch)
        })
        .collect()
}

fn security_choices() -> Vec<Value> {
    vec![
        choice(
            "US SCMS (IEEE 1609.2)",
            "The US Security Credential Management System: pseudonym certificates in batches, linkage values and certificate revocation lists. Vehicles send BSMs over WSMP.",
            None,
            vec![
                set(
                    "security.protocol.id",
                    json!(v2xw_proto::scms::CAMP_SCMS_ID),
                ),
                set("security.envelope", json!("ieee1609.2")),
                set("net.layer", json!("wsmp")),
                set("messages.sets", json!(["bsm"])),
            ],
        ),
        choice(
            "European C-ITS (ETSI TS 102 941)",
            "The European trust model: authorization tickets, a trust list and revocation at the enrolment authority. Vehicles send CAMs over GeoNetworking.",
            None,
            vec![
                set("security.protocol.id", json!(crate::phase2::ETSI_PKI)),
                set("security.envelope", json!("etsi103097")),
                set("net.layer", json!("gn-btp")),
                set("messages.sets", json!(["cam"])),
            ],
        ),
    ]
}

/// The attack share a quick attacker setting uses: 5 %, the share `options::item_template`
/// adds and the attack scenarios run with (this build's choice, not a measured share).
const ATTACK_SHARE: f64 = 0.05;

fn attacker_choices() -> Vec<Value> {
    let mut out = vec![choice(
        "Off",
        "Every vehicle is honest.",
        None,
        vec![set("threats.attackers", json!([]))],
    )];
    let path = "threats.attackers[].id";
    let rows = options::slot_values(path, &super::publish::catalogue_for_guide())
        .map(|(rows, _)| rows)
        .unwrap_or_default();
    for row in rows {
        if row["available"].as_bool() == Some(false) {
            continue;
        }
        let Some(id) = row["value"].as_str() else {
            continue;
        };
        let label = row["label"].as_str().unwrap_or(id);
        let description = format!(
            "{} 5 % of vehicles attack; each vehicle runs the standard detectors and reports them.",
            row["description"].as_str().unwrap_or("")
        );
        out.push(choice(
            label,
            description.trim(),
            None,
            vec![
                set(
                    "threats.attackers",
                    json!([{"id": id, "fraction": ATTACK_SHARE}]),
                ),
                set("detection.local", json!([{"id": crate::phase2::LEGACY_12}])),
            ],
        ));
    }
    out
}

fn export_choices() -> Vec<Value> {
    let mut out = vec![choice(
        "Don't save",
        "Nothing is written; the run lives in the Studio only.",
        None,
        vec![set("exporters", json!([]))],
    )];
    for id in crate::export::EXPORTERS {
        let (label, description) = match id {
            "recording" => (
                "Recording (MCAP)",
                "The full recording, replayable in the Studio and readable with Foxglove.",
            ),
            "jsonl" => (
                "Tables as JSON Lines",
                "One table per recorded channel, as text: easy to read, large.",
            ),
            "parquet" => (
                "Tables as Parquet",
                "One table per recorded channel, compressed: the format for pandas, R or DuckDB.",
            ),
            "arrow" => (
                "Tables as Arrow",
                "One table per recorded channel as Arrow IPC files.",
            ),
            other => (other, ""),
        };
        out.push(choice(
            label,
            description,
            None,
            vec![set("exporters", json!([{"id": id}]))],
        ));
    }
    out
}

/// A fleet-mix toggle: a class's share switched on (taken from passenger cars) or off.
fn class_toggle(id: &str, label: &str, class: &str, share: f64, explain: &str) -> Value {
    json!({
        "id": id, "label": label, "kind": "class",
        "path": "actors.vehicles.classes", "pointer": pointer_for("actors.vehicles.classes"),
        "class": class, "share": share, "absorb": "passenger",
        "explain": explain,
    })
}

fn field(id: &str, label: &str, path: &str, explain: &str) -> Value {
    json!({"id": id, "label": label, "kind": "field", "path": path,
           "pointer": pointer_for(path), "explain": explain})
}

/// The awareness-message rate choices: each message set's own standard timing, and the
/// fixed rates a channel-load study steps through. A fixed rate is `messages.rate_hz`,
/// which sets the BSM's nominal interval and pins the CAM's shortest and longest interval
/// to it (`wiring::generator_params`); congestion control may still lengthen it.
fn message_rate_choices() -> Vec<Value> {
    let mut out = vec![choice(
        "Standard",
        "Each message set's own standard timing: the J2945/1 BSM at 10 Hz, the EN 302 637-2 \
         CAM triggered by the vehicle's movement.",
        None,
        vec![set("messages.rate_hz", Value::Null)],
    )];
    for hz in [20u32, 10, 5, 2, 1] {
        out.push(choice(
            &format!("{hz} Hz"),
            &format!(
                "Every vehicle broadcasts its awareness message (BSM or CAM) {hz} times a \
                 second; congestion control may still lengthen the interval."
            ),
            None,
            vec![set("messages.rate_hz", json!(hz))],
        ));
    }
    out
}

/// The quick settings, in the order the Studio shows them, each with the heading it sits
/// under. The scenario and the city are not here: the Studio's header picks those.
pub fn quick() -> Value {
    let g = |mut v: Value, group: &str| {
        v["group"] = json!(group);
        v
    };
    json!([
        g(field("traffic", "Traffic", "actors.vehicles.demand.rate_veh_per_h",
                "How many vehicles enter the map each hour."), "Traffic"),
        g(field("equipped", "Equipped share", "actors.vehicles.equipped_fraction",
                "The share of vehicles that carry a V2X radio (market penetration)."), "Traffic"),
        g(field("pedestrians", "Pedestrians", "actors.vru.pedestrians",
                "People walking on the sidewalks and crossings."), "Traffic"),
        g(field("cyclists", "Cyclists", "actors.vru.cyclists",
                "People cycling on the roads and bike lanes."), "Traffic"),
        g(field("drivers", "Drivers", "actors.vehicles.drivers.realism",
                "How human the drivers are: calibrated, identical, heterogeneous or human (stop-and-go waves)."), "Traffic"),
        // Shares from scenarios/midtown-street-life.yaml's fleet (a stated choice there:
        // no classified count of Midtown traffic was read).
        g(class_toggle("motorcycles", "Motorcycles", "motorcycle", 0.02,
                       "2 % of vehicles are motorcycles, taken from the cars."), "Traffic"),
        g(class_toggle("trucks", "Trucks", "truck", 0.03,
                       "3 % of vehicles are trucks, taken from the cars."), "Traffic"),
        g(field("rsus", "Roadside units", "actors.rsu_placement.count",
                "Radios at the busiest signalised junctions."), "Traffic"),
        g(field("radio", "Radio", "radio.rat", "Which V2X radio technology vehicles use."),
          "Radio"),
        g(field("tx_power", "Transmit power", "radio.devices.obu.tx_power_dbm",
                "A vehicle radio's conducted power; congestion control may lower it."), "Radio"),
        g(json!({"id": "message_rate", "label": "Message rate", "kind": "choice",
                 "explain": "How often each vehicle broadcasts its safety message.",
                 "options": message_rate_choices()}), "Radio"),
        g(json!({"id": "security", "label": "Security", "kind": "choice",
                 "explain": "Which credential system signs and checks messages.",
                 "options": security_choices()}), "Security"),
        g(json!({"id": "signature", "label": "Signature", "kind": "choice",
                 "explain": "The signature on every message: classical, hybrid or post-quantum. A larger signature is sent in pieces and checked on hardware with figures for it.",
                 "options": signature_choices()}), "Security"),
        g(field("pseudonyms", "Pseudonym change", "security.pseudonym_change.strategy",
                "When a vehicle switches to a fresh certificate to stay untrackable."), "Security"),
        g(json!({"id": "attackers", "label": "Attackers", "kind": "choice",
                 "explain": "Whether some vehicles send false messages, and which kind.",
                 "options": attacker_choices()}), "Security"),
        // The share of every attacker population at once; the Studio greys it out while
        // `threats.attackers` is empty. Bounded [0, 1] by validate's BOUNDS.
        g(json!({"id": "attacker_share", "label": "Attacker share", "kind": "share",
                 "path": "threats.attackers", "pointer": pointer_for("threats.attackers"),
                 "key": "fraction", "values": [0.01, 0.05, 0.1, 0.2, 0.3],
                 "explain": "The share of equipped vehicles that attack."}), "Security"),
        g(field("duration", "Run length", "time.duration_s",
                "How much simulated time the run covers."), "Run"),
        g(field("seed", "Seed", "seed",
                "The random seed: the same seed and settings give the same run."), "Run"),
        g(json!({"id": "save", "label": "Save run data", "kind": "choice",
                 "explain": "Which files the run writes when it finishes, beside its recording.",
                 "options": export_choices(), "shows_files": true}), "Run"),
    ])
}

/// The study starting points.
pub fn studies() -> Value {
    let study = |id: &str, label: &str, question: &str, look_at: &str, patch: Vec<Value>| {
        json!({"id": id, "label": label, "question": question, "look_at": look_at,
               "patch": patch})
    };
    json!([
        study(
            "network-load",
            "Network load",
            "How busy does the radio channel get, and when do messages start being lost?",
            "Channel busy ratio, packet delivery ratio against distance, latency.",
            vec![
                set("actors.vehicles.demand.rate_veh_per_h", json!(4500.0)),
                set("actors.vehicles.equipped_fraction", json!(1.0)),
                set("threats.attackers", json!([])),
                set("time.duration_s", json!(300.0)),
            ],
        ),
        study(
            "security-overhead",
            "Security overhead",
            "What do signatures and certificates cost in bytes, airtime and processing time?",
            "Message sizes, sign and verify latency, verification queue, delivery ratio.",
            {
                let mut p = vec![
                    set("security.signature", json!("hybrid-falcon512-ecdsa-p256")),
                    set("security.verification_policy", json!("verify-all")),
                    set("time.duration_s", json!(120.0)),
                ];
                p.extend(pq_support());
                p
            },
        ),
        study(
            "attacks",
            "Attacks and detection",
            "Can misbehaving vehicles be detected, reported and revoked, and how fast?",
            "Detections, reports, revocation latency, false positives.",
            vec![
                set(
                    "threats.attackers",
                    json!([{"id": "threat/attacker/legacy/ConstPos", "fraction": ATTACK_SHARE}])
                ),
                set("detection.local", json!([{"id": crate::phase2::LEGACY_12}])),
                set(
                    "detection.ma",
                    json!({"id": crate::phase2::MA_CORROBORATED})
                ),
                set("time.duration_s", json!(300.0)),
            ],
        ),
        study(
            "privacy",
            "Privacy",
            "Can an eavesdropper follow a vehicle across pseudonym changes?",
            "Tracking success, pseudonym changes, linkability.",
            vec![
                set("security.pseudonym_change.strategy", json!("time")),
                set("security.pseudonym_change.period_s", json!(60.0)),
                set(
                    "threats.eavesdropper",
                    json!({"id": crate::pseudonym_policy::EAVESDROPPER_ID})
                ),
                set("threats.attackers", json!([])),
                set("time.duration_s", json!(600.0)),
            ],
        ),
        study(
            "traffic",
            "Traffic",
            "How do vehicles, pedestrians and cyclists move, and what do safety applications change?",
            "Speeds, queues, travel times, warnings issued.",
            vec![
                set("actors.vehicles.demand.rate_veh_per_h", json!(3000.0)),
                set("actors.vru.pedestrians", json!(100)),
                set("actors.vru.cyclists", json!(50)),
                set("threats.attackers", json!([])),
                set("time.duration_s", json!(900.0)),
            ],
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies a patch to a scenario document the way the Studio does: each write at its
    /// pointer, creating objects on the way.
    fn apply(doc: &mut Value, patch: &Value) {
        for w in patch.as_array().expect("patch is a list") {
            let pointer = w["pointer"].as_str().expect("pointer");
            let mut cur = &mut *doc;
            let parts: Vec<&str> = pointer.split('/').skip(1).collect();
            for (i, part) in parts.iter().enumerate() {
                if i + 1 == parts.len() {
                    cur[*part] = w["value"].clone();
                } else {
                    if !cur[*part].is_object() {
                        cur[*part] = json!({});
                    }
                    cur = &mut cur[*part];
                }
            }
        }
    }

    fn errors_after(patch: &Value) -> Vec<String> {
        let mut doc = serde_json::to_value(crate::Scenario::minimal()).expect("serialises");
        apply(&mut doc, patch);
        let s: crate::Scenario = serde_json::from_value(doc).expect("patched scenario parses");
        crate::scenario::validate(&s)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn every_quick_choice_and_study_loads() {
        let mut checked = 0;
        for q in quick().as_array().expect("list") {
            for o in q["options"].as_array().into_iter().flatten() {
                let e = errors_after(&o["patch"]);
                assert!(e.is_empty(), "{} / {}: {e:?}", q["id"], o["label"]);
                checked += 1;
            }
        }
        for s in studies().as_array().expect("list") {
            let e = errors_after(&s["patch"]);
            assert!(e.is_empty(), "study {}: {e:?}", s["id"]);
            checked += 1;
        }
        assert!(checked > 10, "{checked}");
    }

    #[test]
    fn every_quick_field_and_basic_path_is_published() {
        let published: Vec<String> = crate::scenario::publish::fields()
            .iter()
            .filter_map(|f| f["x-path"].as_str().map(str::to_string))
            .collect();
        let has = |p: &str| {
            published.iter().any(|x| {
                x == p || x.starts_with(&format!("{p}[]")) || x.starts_with(&format!("{p}."))
            })
        };
        for q in quick().as_array().expect("list") {
            if let Some(p) = q["path"].as_str() {
                assert!(has(p), "quick setting {} names unpublished {p}", q["id"]);
            }
        }
        for b in BASIC {
            assert!(has(b.path), "basic {} is not published", b.path);
        }
    }

    #[test]
    fn signatures_cover_every_mode() {
        let groups: Vec<String> = signature_choices()
            .iter()
            .filter_map(|c| c["group"].as_str().map(str::to_string))
            .collect();
        for g in [
            "Classical (deployed today)",
            "Hybrid post-quantum",
            "Post-quantum",
        ] {
            assert!(groups.iter().any(|x| x == g), "{g}");
        }
    }
}
