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
        path: "exporters",
        explain: "Which files the run writes when it finishes, for analysis outside the Studio.",
        recommended: "None to explore; Parquet to analyse in Python or R.",
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

/// The real-world sentence for a basic field.
pub fn explain(path: &str) -> Option<&'static str> {
    basic(path).map(|b| b.explain)
}

/// The recommended range, in words, for a basic field.
pub fn recommended(path: &str) -> Option<&'static str> {
    basic(path).map(|b| b.recommended)
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

/// The quick settings, in the order the Studio shows them.
pub fn quick() -> Value {
    json!([
        {"id": "scenario", "label": "Scenario", "kind": "scenario",
         "explain": "A ready-made situation: the traffic, the radio, the security and what goes wrong."},
        {"id": "city", "label": "City", "kind": "city",
         "explain": "Where it happens: any scenario runs on any city in the library."},
        field("traffic", "Traffic", "actors.vehicles.demand.rate_veh_per_h",
              "How many vehicles enter the map each hour."),
        field("pedestrians", "Pedestrians", "actors.vru.pedestrians",
              "People walking on the sidewalks and crossings."),
        // Shares from scenarios/midtown-street-life.yaml's fleet (a stated choice there:
        // no classified count of Midtown traffic was read).
        class_toggle("motorcycles", "Motorcycles", "motorcycle", 0.02,
                     "2 % of vehicles are motorcycles, taken from the cars."),
        class_toggle("trucks", "Trucks", "truck", 0.03,
                     "3 % of vehicles are trucks, taken from the cars."),
        field("radio", "Radio", "radio.rat", "Which V2X radio technology vehicles use."),
        {"id": "security", "label": "Security", "kind": "choice",
         "explain": "Which credential system signs and checks messages.",
         "options": security_choices()},
        {"id": "signature", "label": "Signature", "kind": "choice",
         "explain": "The signature on every message: classical, hybrid or post-quantum. A larger signature is sent in pieces and checked on hardware with figures for it.",
         "options": signature_choices()},
        {"id": "attackers", "label": "Attackers", "kind": "choice",
         "explain": "Whether some vehicles send false messages, and which kind.",
         "options": attacker_choices()},
        field("duration", "Run length", "time.duration_s", "How much simulated time the run covers."),
        field("rsus", "Roadside units", "actors.rsu_placement.count",
              "Radios at the busiest signalised junctions."),
        {"id": "save", "label": "Save run data", "kind": "choice",
         "explain": "Which files the run writes when it finishes, beside its recording.",
         "options": export_choices(), "shows_files": true},
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
