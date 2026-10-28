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
        explain: "How the traffic lights decide when to change: on a fixed clock, or actuated by detectors in the road that hold a green while cars keep coming and end it early when the street goes quiet, as most US suburban and many city signals do. nema runs the US eight-phase dual-ring controller: both directions' throughs and lefts time side by side, with pedestrian push buttons, left-turn arrows, coordination and emergency-vehicle priority.",
        recommended: "fixed-time for a repeatable baseline; actuated to study signal timing or SPaT that changes; nema for US controller behaviour (FHWA Signal Timing Manual 2nd ed.).",
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
        path: "traffic.reroute_share",
        explain: "The share of drivers who change route on information (a navigation app or a received V2X warning); the rest keep the route they chose and only detour round a closure they can see.",
        recommended: "1 (every driver, today); about 0.3-0.6 for today's navigation-app use in a US city.",
    },
    Basic {
        path: "traffic.reroute_source",
        explain: "Where re-routing drivers learn that the road has changed: a navigation app, which sees the whole network a few minutes late, or only the V2X hazard warnings their own car received, which are local and immediate.",
        recommended: "navigation (today); v2x to measure what V2X hazard warnings are worth to routing.",
    },
    Basic {
        path: "traffic.depart_at_speed",
        explain: "Whether cars entering the map from outside it are already moving, as real traffic crossing the map's edge is, instead of starting from rest at the edge; cars starting inside the map still start from rest.",
        recommended: "Off (today); on for realistic map edges (no false halts or stopped-vehicle warnings there).",
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
        explain: "Which received messages a vehicle checks the signature of. Checking everything is safest but can overload the security chip in dense traffic. Misbehaviour detection only judges messages whose signature was checked (a report's evidence is the accused's signed messages, SCMS Manager SM-MBR v1.0 §2.3.1.2.1), so verifying on demand or nearest first also blinds detection: in testing, on-demand cut reports from 897 to 79 and no attacker was revoked.",
        recommended: "Verify every message, unless you study verification load. With detection on, expect a warning for any other choice.",
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
        recommended: "None to explore; Parquet to analyse in Python or R; veremi (codebook veremi-ext, hourly slices) to run a VeReMi-trained detector.",
    }, // traffic audit
    Basic {
        path: "actors.vehicles.drivers.speed_factor",
        explain: "How fast drivers go on an empty street compared with the speed limit: 1.1 means the average driver goes 10 % over.",
        recommended: "1.0 (SUMO's default) to 1.1; 0.9 for cautious traffic.",
    },
    Basic {
        path: "actors.vehicles.drivers.queue_start_delay_s",
        explain: "How long each driver in a queue waits after the car in front moves before moving off. It decides how many cars get through each green.",
        recommended: "0.5 s (calibrated to measured discharge headways); 0.3 to 1.0 s.",
    },
    Basic {
        path: "traffic.critical_gap_factor",
        explain: "How big a gap in the main-road traffic drivers wait for before pulling out at a junction without lights, or turning right on red where that is allowed: below 1 they squeeze in, above 1 they wait longer.",
        recommended: "1 (the Highway Capacity Manual's gaps); 0.8 to 1.2.",
    },
    Basic {
        path: "traffic.kerbside.enabled",
        explain: "Whether buses stop at stops, taxis pick up and delivery vans double-park, briefly blocking a lane as they do in a real city.",
        recommended: "On for city realism; off for a clean baseline.",
    },
    Basic {
        path: "world.signals.cycle_s",
        explain: "How long a traffic light takes to go through all its phases and come back to green on an imported city map.",
        recommended: "60 to 120 s; 90 s is the default.",
    },
    Basic {
        path: "actors.vehicles.gnss.rtk",
        explain: "Whether vehicles position themselves with survey-grade corrected GPS (about 0.3 m) instead of an ordinary receiver (about 1.5 m). Every safety message carries this position.",
        recommended: "Off for today's fleet; on to study what precise positioning buys.",
    },
    // end traffic audit
    // SCMS settings audit (docs/guides/config-inventory-scms.md)
    Basic {
        path: "security.scms.lifecycle.certs_per_period",
        explain: "How many pseudonym certificates a vehicle may use in one week, all valid at once. More lets it change identity more often without reusing one, at the cost of bigger downloads, and one revocation then covers more certificates. The US profile issues 20 a week (CAMP-EE Table 2.1.2.6.2).",
        recommended: "20 (the US profile); 5 to 60 (60 for vehicles that change often, such as taxis).",
    },
    Basic {
        path: "security.scms.lifecycle.crl_cadence_s",
        explain: "A fixed schedule for publishing the revocation list. Left unset, each provider follows its own rule: the US providers publish within one business day of a revocation decision (SCMS Manager Provider Requirements 3.4.9). 0 publishes the moment a decision is made.",
        recommended: "Unset (the business-day rule); 0 to 86,400 s (a daily list, the USDOT 2013 working assumption).",
    },
    Basic {
        path: "security.scms.lifecycle.crl_fetch_interval_s",
        explain: "How often a vehicle with a cellular link asks for a newer revocation list. Shorter means revoked senders are refused sooner, for more cellular traffic; CAMP requires at least one fetch a week.",
        recommended: "3,600 s (one hour, an engine assumption inside CAMP's weekly bound); 60 to 86,400 s.",
    },
    Basic {
        path: "security.scms.lifecycle.ma_review_s",
        explain: "How long the Misbehaviour Authority's operator takes over a case before the revocation is carried out; 0 is a fully automatic authority. The US pilots reviewed every case by hand and published no review time.",
        recommended: "28,800 s (one 8-hour shift, an assumption) on the real calendar; 0 (automatic) to 259,200 s (three days).",
    },
    Basic {
        path: "security.scms.lifecycle.download_poll_interval_s",
        explain: "download-minWait: how long a vehicle waits before asking again for a certificate batch that is not ready yet. Shorter gets certificates sooner once the mix releases them, for more requests to the Registration Authority (SCMS Manager EE profile, Table 3).",
        recommended: "300 s (the US profile); 1 to 3,600 s.",
    },
];

/// The explanation and recommended range of advanced fields that have them: the
/// researcher-grade radio numbers (`radio.phy`, `radio.mac`, `radio.channel_model`,
/// `radio.dcc`), which a newcomer does not need but should still be able to read.
const ADVANCED: &[Basic] = &[
    Basic {
        path: "actors.vehicles.drivers.red_runner_share",
        explain: "The share of drivers who keep going on amber even when they will reach the line after it turns red, as red-light runners do; 0 makes every driver clear on amber.",
        recommended: "0 (the default); 0.05 to 0.2 to study red-light-violation warnings. Field studies count roughly 1 to 5 red entries per 1,000 vehicles at urban signals (Bonneson, Zimmerman & Brewer, TTI 0-4027, 2002, as FHWA's red-light-running guidance quotes it).",
    },
    Basic {
        path: "actors.vehicles.drivers.violations.red_run_share",
        explain: "With violations enabled, the share of drivers who will still enter after the light turns red, each up to their own lateness (exponential, mean red_run_tolerance_mean_s).",
        recommended: "0.6 (the default) with the 0.6 s mean tolerance reproduces 1.0 red-light violation per 10,000 vehicle-cycles and 3 to 6 per 1,000 vehicles (Bonneson, Zimmerman & Quiroga, TTI 0-4196-P1, Table 1); 0 for none.",
    },
    Basic {
        path: "traffic.lc_strategic",
        explain: "How early drivers move into the lane their next turn needs: higher values change lane further before the junction.",
        recommended: "Unset (today's MOBIL); 1 (SUMO default); 0.5 to 4.",
    },
    Basic {
        path: "traffic.lc_cooperative",
        explain: "How willing drivers are to move over to let in a car that must merge into their lane.",
        recommended: "Unset (today's MOBIL); 1 (SUMO default); 0 to 1.",
    },
    Basic {
        path: "apps.glosa_compliance",
        explain: "The share of drivers who slow down to the speed a green-light advisory tells them, so they arrive on green instead of stopping at red.",
        recommended: "0 (advice shown, not followed: no field rate is published); 0.25 to 1 to study the benefit.",
    },
    Basic {
        path: "apps.fcw_ttc_s",
        explain: "How many seconds before a rear-end collision the forward collision warning sounds: earlier warns sooner but more often for nothing.",
        recommended: "2.4 s (NHTSA's FCW confirmation test); 2.0 to 3.0 s.",
    },
    Basic {
        path: "apps.ima_tti_s",
        explain: "How far ahead, in seconds of driving, the intersection movement assist looks for a crossing vehicle that would reach the same point.",
        recommended: "4 s (this build's choice); 3 to 6 s.",
    },
    Basic {
        path: "apps.lta_gap_s",
        explain: "The gap in oncoming traffic the left-turn assist requires before it stays quiet: a smaller gap means fewer warnings.",
        recommended: "4.1 s (HCM 6th ed. Exhibit 20-11 critical gap); 3.5 to 5.5 s.",
    },
    Basic {
        path: "apps.pcw_ttc_s",
        explain: "How many seconds before a pedestrian or cyclist would be in the vehicle's path the pedestrian collision warning sounds.",
        recommended: "3 s (this build's choice); 2 to 4 s.",
    },
    Basic {
        path: "apps.rlvw_decel_mps2",
        explain: "The braking a driver can do comfortably: the red-light violation warning sounds once stopping for the red would need harder braking than this.",
        recommended: "3.4 m/s² (AASHTO Green Book comfortable deceleration); 2.5 to 4.5 m/s².",
    },
    Basic {
        path: "scale.traffic.model",
        explain: "City scale: drive a map box vehicle by vehicle and everything outside it as queues (tiered), or the whole map as queues; the queues carry a whole city's traffic at a fraction of the cost.",
        recommended: "mobility/tiered/meso-micro with a box around the area you study; leave the section out for a single microscopic map.",
    },
    Basic {
        path: "scale.traffic.meso.segment_length_m",
        explain: "The longest stretch of road one queue covers; shorter segments place vehicles more precisely and cost more.",
        recommended: "98 m (SUMO MESO --meso-edgelength default); 50 to 200 m.",
    },
    Basic {
        path: "scale.traffic.meso.tau_ff_s",
        explain: "Time between vehicles leaving a free-flowing queue into another free-flowing one.",
        recommended: "1.13 s (SUMO MESO --meso-tauff, Eissfeldt 2003); 0.9 to 1.5 s.",
    },
    Basic {
        path: "scale.traffic.meso.tau_fj_s",
        explain: "Time between vehicles leaving a free-flowing queue into a jammed one.",
        recommended: "1.13 s (SUMO MESO --meso-taufj); 0.9 to 1.5 s.",
    },
    Basic {
        path: "scale.traffic.meso.tau_jf_s",
        explain: "Time between vehicles leaving a jammed queue into a free-flowing one: the start-up loss of a jam.",
        recommended: "1.73 s (SUMO MESO --meso-taujf); 1.4 to 2.2 s.",
    },
    Basic {
        path: "scale.traffic.meso.tau_jj_s",
        explain: "Time between vehicles leaving a jammed queue into another jammed one.",
        recommended: "1.4 s (SUMO MESO --meso-taujj); 1.1 to 1.8 s.",
    },
    Basic {
        path: "scale.traffic.meso.jam_threshold",
        explain: "How full a queue must be before it counts as jammed; left out, SUMO's speed rule decides.",
        recommended: "Leave unset (SUMO's default); 0.2 to 0.8 when set.",
    },
    Basic {
        path: "scale.traffic.meso.junction_control",
        explain: "Whether a red signal holds the head of a queue, as it would a real car.",
        recommended: "On (SUMO MESO --meso-junction-control); off only to compare with an unsignalled queue network.",
    },
    Basic {
        path: "scale.radio.follows",
        explain: "Whether the radio's detailed region is the traffic's detailed box, so radios outside it use the cheap abstract model.",
        recommended: "On, unless radio.tiers.focus names its own region.",
    },
    Basic {
        path: "scale.radio.margin_m",
        explain: "How far outside the detailed box a queued vehicle's radio stays on; beyond it, it neither sends nor hears.",
        recommended: "Leave unset (the radio's own range, about 300 m for DSRC); 100 to 1000 m.",
    },
    Basic {
        path: "scale.radio.receive_in_margin",
        explain: "Whether queued vehicles near the box also receive messages, not only send them; on costs most of a city's reception work.",
        recommended: "Off for speed; on when receivers outside the box matter to the study.",
    },
    Basic {
        path: "security.on_demand_threshold",
        explain: "Under on-demand verification, how threatening a known vehicle's message must look (from its time to collision, 0 to 1) before it is checked; lower checks more messages and costs more processor time.",
        recommended: "0.5 (a time to collision of 1 s); 0.05 to 0.9. No published operating point (Krishnan & Weimerskirch 2011); sweep it.",
    },
    Basic {
        path: "security.prioritized_horizon_m",
        explain: "Under prioritized verification, how far away a sender can be and still be checked ahead of farther ones; senders beyond it get no head start for being near.",
        recommended: "300 m (the DSRC/C-V2X awareness range); 100 to 1000 m.",
    },
    Basic {
        path: "nodes.hsm_sign_lower_us",
        explain: "The fastest a hardware security module signs a message, when its maker only publishes a worst case; each signature then takes a random time between this and the worst case instead of always the worst case.",
        recommended: "Unset (every signature at the CRATON2's published <9 ms bound; no typical value is published); 613 us (software ECDSA P-256 on a Cortex-A53, R7 §D8) to 9,000 us.",
    },
    Basic {
        path: "radio.channel_model.fading_coherence",
        explain: "Whether a link's fade carries over from one message to the next: with doppler, vehicles standing still (a queue at a red light) keep a bad fade for many messages in a row, while fast vehicles get a fresh fade with every message.",
        recommended: "independent (today's behaviour); doppler for burst-loss studies (Akki & Haber 1986; Rappaport 2002 Eq. 5.40c).",
    },
    Basic {
        path: "nodes.peer_cert_cache",
        explain: "How many other stations' certificates each vehicle or roadside radio unit remembers; when it forgets one, that vehicle's next short-form message cannot be checked until its certificate is requested again.",
        recommended: "128 (engine default; no device publishes one); 16 to 1024.",
    },
    Basic {
        path: "security.signer_id_policy.on_new_neighbour",
        explain: "Send the full certificate in the next safety message whenever a vehicle it has not heard before comes into range, so the newcomer can check its signatures at once.",
        recommended: "Off (the US cadence alone); on for the ETSI rule (TS 103 097 §7.1.1).",
    },
    Basic {
        path: "world.source.params.ew_green_share",
        explain: "On a generated grid with traffic lights, the share of each light's green time the east-west street gets; the north-south street gets the rest. Give more to the busier street.",
        recommended: "0.5 for equal demand (Webster 1958); 0.3 to 0.7, in proportion to the two streets' flows.",
    },
    Basic {
        path: "actors.vru.walking_speed_mps",
        explain: "How fast pedestrians under 65 walk on average; older walkers are slower in proportion. Slower walkers need longer gaps and longer crossing time.",
        recommended: "1.51 m/s (Knoblauch 1996 crosswalk study); 1.2 to 1.6 m/s.",
    },
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
        path: "radio.phy.energy_detect_dbm",
        explain: "The level at which another channel's frame, leaking through the filter, makes the radio hear its own channel as busy.",
        recommended: "-65 dBm (IEEE 802.11-2020 §17.3.10.6, 10 MHz); -62 (ns-3); -85 reproduces the older single-threshold carrier sense.",
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
        path: "radio.channel_model.foliage",
        explain: "Whether trees between two antennas weaken the signal: about 2.3 dB per metre of trees at 5.9 GHz, up to what bending over the treetops would cost. Needs world.vegetation.",
        recommended: "Off (today); on with world.vegetation for suburban and rural maps with woods and tree-lined streets.",
    },
    Basic {
        path: "radio.channel_model.density_sigma",
        explain: "Whether the signal's random variation grows with the traffic and buildings around each link, as measured in Porto (GEMV2): calmer on an empty road, rougher downtown.",
        recommended: "Off (today, a fixed spread); on for the measured dependence on surroundings.",
    },
    Basic {
        path: "world.vegetation",
        explain: "Whether the map import keeps woods, tree rows and single trees, so the radio can charge for signal through foliage.",
        recommended: "Off (today); on with radio.channel_model.foliage where trees line the roads.",
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
    }, // traffic audit
    Basic {
        path: "actors.vehicles.drivers.speed_spread",
        explain: "How much drivers differ in the speed they choose: 0 makes everyone drive the same speed.",
        recommended: "1 (SUMO's spread, about ±10 % for cars); 0 to 2.",
    },
    // network audit
    Basic {
        path: "radio.sidelink.noise_figure_db",
        explain: "How much noise a C-V2X receiver's own electronics add; every extra dB shortens the range at which a transport block still decodes.",
        recommended: "9 dB (3GPP TR 36.885 UE); 6 dB for a good automotive module.",
    },
    Basic {
        path: "radio.sidelink.cbr_threshold_dbm",
        explain: "The signal level above which a C-V2X sub-channel counts as busy in the channel busy ratio, which drives congestion control.",
        recommended: "-94 dBm (this build's default); -112 to -80 dBm in 2 dB steps (TS 36.331).",
    },
    Basic {
        path: "radio.dcc.reactive_table",
        explain: "Which ETSI reactive table sets the message rate in each congestion state: A.1 for frames up to 1 ms on air, A.2 (twice the rates) for frames up to 500 µs.",
        recommended: "A.1 for secured CAMs at 6 Mbit/s; A.2 only for short frames.",
    },
    Basic {
        path: "radio.dcc.reactive_hold_ms",
        explain: "How long reactive congestion control stays in a state before it may step again; longer holds damp oscillation.",
        recommended: "0 (the tables give none) to 1,000 ms.",
    },
    Basic {
        path: "net.geonet.cbf_min_time_ms",
        explain: "How long the farthest receiver waits before re-broadcasting a geo-broadcast warning.",
        recommended: "1 ms (EN 302 636-4-1 Annex H).",
    },
    Basic {
        path: "net.geonet.cbf_max_time_ms",
        explain: "How long the nearest receiver waits before re-broadcasting; the gap between the two timers decides how cleanly the farthest vehicle wins.",
        recommended: "100 ms (Annex H); 20 to 100 ms in studies.",
    },
    Basic {
        path: "net.geonet.max_comm_range_m",
        explain: "The progress at which the contention timer reaches its minimum: the forwarding range the protocol assumes.",
        recommended: "1,000 m (Annex H); about the real radio range in dense traffic, 300 to 500 m.",
    },
    Basic {
        path: "messages.spat_interval_ms",
        explain: "How often a traffic light's roadside unit broadcasts its signal phase and timing.",
        recommended: "100 ms (10 Hz, CTI 4501).",
    },
    Basic {
        path: "messages.map_interval_ms",
        explain: "How often a roadside unit broadcasts the intersection's lane map.",
        recommended: "1,000 ms (1 Hz, CTI 4501).",
    },
    Basic {
        path: "messages.cpm_interval_ms",
        explain: "How often a vehicle checks whether to share what its sensors see; a longer interval means fewer, larger collective perception messages.",
        recommended: "100 ms (ETSI TS 103 324) to 1,000 ms.",
    },
    Basic {
        path: "messages.cpm_dcc_limited",
        explain: "Whether vehicles send collective perception messages less often when the channel is congested, following the congestion control as the standard says, instead of at their own rate whatever the load.",
        recommended: "Off (today); on for a realistic CPM load in dense traffic (ETSI TS 103 324).",
    },
    Basic {
        path: "messages.cpm_quality_threshold",
        explain: "The lowest perception quality (0 to 15) an object must exceed to be shared in a collective perception message; tracks the sensors have lost for a while fall below it.",
        recommended: "Empty (not applied, today); 3 as ETSI TS 103 324 recommends.",
    },
    Basic {
        path: "messages.cpm_max_objects",
        explain: "The most objects one collective perception message carries; when more are due, the most informative go first and the rest wait for the next message.",
        recommended: "Empty (255, the format's limit); lower to study a size budget.",
    },
    Basic {
        path: "messages.denm_traffic_condition",
        explain: "Whether vehicles warn the traffic behind them of the end of a queue and of slow or stopped traffic ahead, on non-urban roads, as deployed C-ITS cars do.",
        recommended: "Off (today); on for highway studies (C2C-CC triggering conditions).",
    },
    Basic {
        path: "messages.denm_traction_loss",
        explain: "Whether a vehicle whose anti-lock brakes find a slippery road (ice, snow) warns the vehicles around it.",
        recommended: "Off (today); on for winter-weather studies (C2C-CC triggering conditions).",
    },
    Basic {
        path: "messages.path_history_points",
        explain: "How many points of its recent track a vehicle puts in each safety message; more points let receivers follow its path but make every message bigger.",
        recommended: "15 (SAE J2945/1); 0 to 23 (the J2735 limit).",
    },
    Basic {
        path: "nodes.verify_queue_depth",
        explain: "How many received messages can wait for signature verification; when the processor falls behind, messages beyond it are dropped.",
        recommended: "16 to 256; 64 is one second of 6 neighbours at 10 Hz.",
    },
    Basic {
        path: "actors.vehicles.drivers.start_reaction_s",
        explain: "How long the first driver in line takes to react when the light turns green.",
        recommended: "1.08 s (Bonneson 1992, a 1.22 s mean); 0.8 to 1.5 s.",
    },
    Basic {
        path: "traffic.permitted_left_gap_s",
        explain: "The gap in oncoming traffic a driver waits for before turning left across it on a green light.",
        recommended: "4.5 s (HCM); 4 to 5.5 s.",
    },
    Basic {
        path: "traffic.turn_lateral_accel_mps2",
        explain: "How fast drivers take corners through a junction: higher values let them turn faster.",
        recommended: "5.5 m/s² (SUMO); 3 to 6 m/s².",
    },
    Basic {
        path: "traffic.heavy_vehicle_gap_terms",
        explain: "Whether trucks and buses, which accelerate slowly, wait for longer gaps in traffic than cars before pulling out or turning left across it.",
        recommended: "Off (today); on for HCM heavy-vehicle gaps (+1 s, +2 s on wide roads).",
    },
    Basic {
        path: "traffic.critical_gap_cv",
        explain: "How much drivers differ in the gap they wait for before pulling out or turning across traffic: 0 gives everyone the same gap.",
        recommended: "0 (today); 0.18 for realistic drivers (field spreads 0.15 to 0.2).",
    },
    Basic {
        path: "traffic.time_to_impatience_s",
        explain: "How long a driver waiting to pull out or turn across traffic stands before they accept the shortest gap they ever will; the longer they wait, the smaller the gap they take.",
        recommended: "0 (off, today); 180 s (SUMO) for realistic waiting.",
    },
    Basic {
        path: "traffic.impatience_gap_floor",
        explain: "How short a gap a driver who has waited a long time accepts, as a share of the gap they wanted at first.",
        recommended: "0.7; 0.5 to 0.9.",
    },
    Basic {
        path: "traffic.impatience_foe_braking",
        explain: "Whether an impatient driver pulls out counting on oncoming cars to brake for them.",
        recommended: "Off; on for SUMO-like assertive drivers.",
    },
    Basic {
        path: "traffic.permissive_sneakers",
        explain: "How many drivers waiting to turn left on a green may finish the turn as the light turns yellow, when oncoming traffic stops; without them a left turn into steady oncoming traffic can wait for minutes.",
        recommended: "0 (today); 2 (HCM) for realistic left turns.",
    },
    Basic {
        path: "traffic.standing_leader_stop",
        explain: "Whether drivers coming up behind a stopped car brake smoothly to stop at their usual gap, as they do at a stop line, rather than braking late and stopping abruptly (vans and trucks, whose following model brakes gently, were stopped in one step).",
        recommended: "Off (today); on for smooth stops behind queues (the realistic benchmark sets it).",
    },
    Basic {
        path: "traffic.depart_speed_max",
        explain: "Whether cars enter the road already moving, at the fastest speed that is safe there, as traffic joining a freeway does, rather than appearing parked in the lane and pulling away from rest.",
        recommended: "Off (today, SUMO's default); on for freeways and other roads entered at speed (SUMO departSpeed=max).",
    },
    Basic {
        path: "traffic.lane_bend_speed",
        explain: "Whether drivers slow for sharp bends in a street's mapped centreline as they do for corners at junctions; without it a car follows a kink in the map at full speed.",
        recommended: "Off (today); on with turn_curve_window_m 6 for kinked city streets (the realistic benchmark sets it).",
    },
    Basic {
        path: "traffic.vehicle_dynamics.grade",
        explain: "Whether hills slow vehicles down: on a climb a driver gets less acceleration from the same pedal, and on a descent the car holds its speed. Needs a map with terrain heights; flat maps are unchanged.",
        recommended: "Off (today); on for hilly maps (SUMO's KraussPS slope term).",
    },
    Basic {
        path: "traffic.vehicle_dynamics.power_limit",
        explain: "Whether an engine's power limits how fast a vehicle can speed up, so a loaded tractor-trailer crawls up a long climb at about 46 km/h on a 5% grade while a car keeps the limit. Applies to cars and tractor-trailers; rigid trucks, buses, vans and motorcycles have no sourced figures yet.",
        recommended: "Off (today); on with grade for hilly maps and freight corridors.",
    },
    Basic {
        path: "traffic.lc_speed_gain",
        explain: "How readily drivers change lane to get past slower traffic; setting any of the lane-change eagerness settings switches to SUMO's LC2013 lane-changing.",
        recommended: "Unset (today's MOBIL); 1 (SUMO default); 0.5 to 2.",
    },
    Basic {
        path: "traffic.lc_keep_right",
        explain: "How readily drivers move back into the right lane when it is free, as the keep-right rule asks.",
        recommended: "Unset (today's MOBIL); 1 (SUMO default); 0 to 2.",
    },
    Basic {
        path: "traffic.turn_curve_window_m",
        explain: "Whether drivers slow for the sharpest part of a corner (a few metres) or only for its average curve, which is what SUMO does.",
        recommended: "0 m (SUMO, average); 6 m to slow for the apex.",
    },
    Basic {
        path: "traffic.kerbside.bus_stop_spacing_m",
        explain: "How far apart bus stops are along a bus route.",
        recommended: "200 to 400 m for local buses.",
    },
    Basic {
        path: "traffic.kerbside.bus_dwell_mean_s",
        explain: "How long a bus stands at each stop on average.",
        recommended: "15 to 60 s (TCQSM).",
    },
    Basic {
        path: "traffic.kerbside.taxi_share",
        explain: "The share of cars that are taxis or ride-hail cars making one kerbside stop.",
        recommended: "0 outside a downtown; 0.2 to 0.3 in Midtown Manhattan.",
    },
    Basic {
        path: "traffic.kerbside.taxi_dwell_median_s",
        explain: "How long a taxi stands at the kerb for a pick-up or drop-off.",
        recommended: "20 to 60 s.",
    },
    Basic {
        path: "traffic.kerbside.delivery_stop_share",
        explain: "The share of vans and trucks that stop once on their trip to deliver.",
        recommended: "0.3 to 0.6.",
    },
    Basic {
        path: "traffic.kerbside.delivery_dwell_median_s",
        explain: "How long a delivery stop lasts.",
        recommended: "5 to 15 minutes.",
    },
    Basic {
        path: "world.signals.all_red_s",
        explain: "The shortest time all directions see red between phases, so the junction clears.",
        recommended: "0 to 2 s; the ITE formula may add more.",
    },
    Basic {
        path: "world.signals.pedestrian_walk_s",
        explain: "How long the walk signal shows; each green is made long enough for it and the crossing.",
        recommended: "7 s (MUTCD minimum); 4 to 10 s.",
    },
    Basic {
        path: "world.signals.join_m",
        explain: "Two traffic lights this close together on an imported map run as one junction with one plan, so cars are never fed into the short gap between them faster than it empties.",
        recommended: "Off (0) as imported; 20 m (SUMO's netconvert default); 10 to 30 m.",
    },
    // FHWA Traffic Signal Timing Manual 2e (FHWA-HOP-08-024), tables 5-3 and 5-4: minimum
    // green 5-15 s, maximum green 30-60 s on an arterial through phase. The two ranges do
    // not overlap, so a sweep across either one never crosses the other's default (7 / 45).
    Basic {
        path: "traffic.signals.min_green_s",
        explain: "Under actuated control, the shortest a green runs once it starts, even if no more cars arrive, so the queue stored at the stop line can start up and clear.",
        recommended: "7 s; 5 to 15 s (FHWA Signal Timing Manual).",
    },
    Basic {
        path: "traffic.signals.max_green_s",
        explain: "Under actuated control, the longest a green may be held by arriving cars while another approach waits.",
        recommended: "45 s; 30 to 60 s (FHWA Signal Timing Manual).",
    },
    Basic {
        path: "traffic.signals.detector_m",
        explain: "Under actuated control, how far back from the stop line the detector loop reaches; cars inside it hold the green.",
        recommended: "12.2 m (40 ft stop-line loop, FHWA); 6 to 30 m.",
    },
    Basic {
        path: "traffic.signals.short_link_clearance_m",
        explain: "Where two traffic lights are this close, the downstream light turns green for the short street between them just before the upstream one does, so cars are not held at a green light by a full block ahead (closely spaced signals, FHWA Signal Timing Manual ch. 6).",
        recommended: "Off for the plans as imported; 30 to 90 m (4 to 12 queued cars at the HCM's 7.62 m each).",
    },
    Basic {
        path: "traffic.signals.detector_placement",
        explain: "Under actuated control, where the vehicle detectors sit: a loop at the stop line, or SUMO's single loop per lane about two seconds of travel upstream, which ends the green sooner once a platoon has passed.",
        recommended: "stop-line (FHWA) for US practice; sumo to compare with a SUMO actuated run.",
    },
    // WP-17: FHWA Signal Timing Manual 2nd ed. (NCHRP 812), Exhibits 6-4, 6-9 and 6-22.
    Basic {
        path: "traffic.signals.nema.left_min_green_s",
        explain: "Under the NEMA dual-ring controller, the shortest a protected left-turn arrow runs once it starts.",
        recommended: "4 s; 2 to 5 s (FHWA Signal Timing Manual 2nd ed., Exhibit 6-4).",
    },
    Basic {
        path: "traffic.signals.nema.left_max_green_s",
        explain: "Under the NEMA dual-ring controller, the longest a protected left-turn arrow may be held by arriving cars while another phase waits.",
        recommended: "25 s; 15 to 30 s (FHWA Signal Timing Manual 2nd ed., Exhibit 6-9).",
    },
    Basic {
        path: "traffic.signals.nema.soft_recall",
        explain: "Phases the controller returns to when nobody else is waiting, so the main street rests in green.",
        recommended: "2 and 6, the main-street throughs (FHWA Signal Timing Manual 2nd ed., Exhibit 6-22).",
    },
    Basic {
        path: "traffic.signals.nema.lefts",
        explain: "Whether the left-turn arrows come before or after the opposing through traffic's green.",
        recommended: "lead-lead, the most common US sequence (FHWA Signal Timing Manual 2nd ed. §5.1.3.6).",
    },
    Basic {
        path: "world.signals.pedestrian_speed_mps",
        explain: "The walking speed the flashing don't-walk time is timed for: slower gives pedestrians longer and drivers shorter greens.",
        recommended: "1.067 m/s (MUTCD 3.5 ft/s); 0.91 m/s near schools or seniors.",
    },
    Basic {
        path: "actors.vehicles.drivers.truck.max_accel_mps2",
        explain: "How hard truck drivers accelerate when the road ahead is clear.",
        recommended: "0.5 to 1.3 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.truck.comfort_decel_mps2",
        explain: "How hard truck drivers brake in normal driving.",
        recommended: "1.5 to 4 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.truck.time_headway_s",
        explain: "The time gap truck drivers keep to the vehicle in front.",
        recommended: "1.5 to 2.5 s.",
    },
    Basic {
        path: "actors.vehicles.drivers.truck.min_gap_m",
        explain: "The distance truck drivers leave to the vehicle in front when queued.",
        recommended: "2.5 to 4 m.",
    },
    Basic {
        path: "actors.vehicles.drivers.bus.max_accel_mps2",
        explain: "How hard bus drivers accelerate when the road ahead is clear.",
        recommended: "0.5 to 1.2 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.bus.comfort_decel_mps2",
        explain: "How hard bus drivers brake in normal driving.",
        recommended: "1.5 to 4 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.bus.time_headway_s",
        explain: "The time gap bus drivers keep to the vehicle in front.",
        recommended: "1.5 to 2.5 s.",
    },
    Basic {
        path: "actors.vehicles.drivers.bus.min_gap_m",
        explain: "The distance bus drivers leave to the vehicle in front when queued.",
        recommended: "2.5 to 4 m.",
    },
    Basic {
        path: "actors.vehicles.drivers.motorcycle.max_accel_mps2",
        explain: "How hard motorcyclists accelerate when the road ahead is clear.",
        recommended: "2 to 5 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.motorcycle.comfort_decel_mps2",
        explain: "How hard motorcyclists brake in normal driving.",
        recommended: "2.5 to 4 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.motorcycle.time_headway_s",
        explain: "The time gap motorcyclists keep to the vehicle in front.",
        recommended: "0.8 to 1.5 s.",
    },
    Basic {
        path: "actors.vehicles.drivers.motorcycle.min_gap_m",
        explain: "The distance motorcyclists leave to the vehicle in front when queued.",
        recommended: "1 to 2 m.",
    },
    Basic {
        path: "actors.vehicles.drivers.bicycle.max_accel_mps2",
        explain: "How hard cyclists accelerate when the road ahead is clear.",
        recommended: "0.8 to 1.5 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.bicycle.comfort_decel_mps2",
        explain: "How hard cyclists brake in normal driving.",
        recommended: "1.5 to 3 m/s².",
    },
    Basic {
        path: "actors.vehicles.drivers.bicycle.time_headway_s",
        explain: "The time gap cyclists keep to the vehicle in front.",
        recommended: "1 to 2 s.",
    },
    Basic {
        path: "actors.vehicles.drivers.bicycle.min_gap_m",
        explain: "The distance cyclists leave to the vehicle in front when queued.",
        recommended: "0.5 to 1.5 m.",
    },
    Basic {
        path: "actors.vehicles.gnss.outlier_rate_hz",
        explain: "How often a vehicle's GPS fix jumps away for a moment.",
        recommended: "0.01 per second (default); 0 for clean positions.",
    },
    Basic {
        path: "actors.vehicles.gnss.outlier_magnitude_m",
        explain: "How far such a jump puts the reported position.",
        recommended: "5 to 30 m.",
    },
    Basic {
        path: "actors.vehicles.gnss.burst_rate_hz",
        explain: "How often a stretch of poor positioning starts, as among tall buildings.",
        recommended: "0.006 per second (default); higher for a dense downtown.",
    },
    Basic {
        path: "actors.vehicles.gnss.outage_probability",
        explain: "The chance each position fix is lost altogether, as in a tunnel.",
        recommended: "0 (default) to 0.05.",
    },
    Basic {
        path: "actors.vehicles.gnss.smoothing_tau_s",
        explain: "How slowly the GPS receiver blends new fixes into the position it reports, steering by its own measured speed in between, as real receivers do. 0 reports every raw fix, which jumps by metres between messages.",
        recommended: "100 s (the US real-world presets, RTCA DO-229's smoothing constant); 0 for raw fixes.",
    },
    Basic {
        path: "actors.vehicles.gnss.static_hold_mps",
        explain: "Below this speed the GPS receiver treats the vehicle as stopped: it holds the position and reports zero speed. Needs the smoothing.",
        recommended: "0.3 m/s (the US real-world presets); 0 for off.",
    },
    Basic {
        path: "actors.vehicles.gnss.velocity_noise_mps",
        explain: "How much the GPS receiver's reported speed jitters from one fix to the next. Unset uses the GPS service's 95 % accuracy figure, which makes consecutive messages' speeds differ about three times more than real CV-pilot units' do.",
        recommended: "Unset (default) to compare with earlier runs; 0 with the BSM timing model on, whose fitted output noise then carries the message-level speed and heading noise.",
    },
    Basic {
        path: "actors.vehicles.gnss.stated_sigma_m",
        explain: "The position accuracy each car's radio claims in its messages (one standard deviation). Unset claims the receiver's true error statistics; real deployed units claim much less than their real error, which makes detectors that trust the claim stricter.",
        recommended: "Unset (default) to compare with earlier runs; 0.3 m (Tampa CV pilot) or 2.0 m (Orem) to match a deployment.",
    },
    Basic {
        path: "actors.vehicles.gnss.stated_sigma_log_sd",
        explain: "How much the claimed accuracy differs from one car to the next.",
        recommended: "0.81 with 0.3 m (Tampa's spread); 0 with 2.0 m (Orem's units all claim the same).",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.enabled",
        explain: "Gives each car's radio its own message timing, as real units have: some stamp each message with the instant of the GPS fix (locked), others do not (free), and those now and then send a position one fix too old or too new, or the last one again. Real CV-pilot data shows that is what makes honest cars trip the US position-consistency check.",
        recommended: "Off (default) to compare with earlier runs; on for honest-trigger studies.",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.locked_share",
        explain: "The share of cars whose message time is locked to the GPS fix.",
        recommended: "0.25 (Tampa CV pilot, built in); sweep 0 to 1.",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.calibration",
        explain: "A timing calibration file fitted to real CV-pilot messages.",
        recommended: "data/calibration/bsm-timing-cvpilot.yaml when present; unset uses provisional built-in values.",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.enabled",
        explain: "Locked units report the position of the GPS fix itself and stamp the message on the fix or one millisecond early, switching between the two as real units do; when a unit switches to early its position often takes a small lasting step.",
        recommended: "Off (default) keeps earlier runs; on for the most realistic locked-unit timing (CV-pilot Tampa and Wyoming units).",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.profile",
        explain: "Which real units' switching rates to use: Tampa's locked units (stamped early on about 10 % of messages), the Wyoming unit (46 %), or Tampa's switching units alone (tampa_chain, 26 %, whose positions step only while stamped on the fix; use it with the chain-device timing file).",
        recommended: "tampa_chain with data/calibration/bsm-timing-cvpilot-chain-device.yaml for the closest out-of-sample match to Wyoming; tampa_locked is the default.",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.stay_minus1",
        explain: "The chance that a message stamped early is followed by another early one.",
        recommended: "Unset (the profile's: 0.235 Tampa, 0.566 Wyoming).",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.stay_zero",
        explain: "The chance that a message stamped on the fix is followed by another on the fix.",
        recommended: "Unset (the profile's: 0.918 Tampa, 0.630 Wyoming).",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.steps",
        explain: "Whether a switch to early stamping can carry a lasting position step of a few centimetres, as the real units' positioning filter does.",
        recommended: "On (default) with the chain.",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.speed_per_m",
        explain: "How much the reported speed jumps, in m/s per metre, when a unit's position takes a step; the jump then fades. Real units' speed steps with their position.",
        recommended: "Unset (no speed change; it barely moves the position check but matters for speed-consistency detectors); 0.499 for Tampa's switching units.",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.speed_tau_s",
        explain: "How quickly the speed jump that comes with a step fades.",
        recommended: "Unset (Tampa's 0.357 s).",
    },
    Basic {
        path: "actors.vehicles.bsm_timing.stamp_chain.position_follows",
        explain: "How much of the speed jump the reported position follows: 1 moves the position with the reported speed, 0 leaves it on the old speed.",
        recommended: "Unset (Tampa's 0.759).",
    },
    // end traffic audit
    // SCMS settings audit (docs/guides/config-inventory-scms.md)
    Basic {
        path: "security.scms.lifecycle.i_period_s",
        explain: "How long one set of pseudonym certificates is current. The US system uses one week from its 2015-01-06 epoch; a shorter period compresses the calendar so a short run sees weekly events, and the run is then labelled compressed.",
        recommended: "604,800 s (one week, CAMP-EE) on the real calendar; 60 to 3,600 s only with time.calendar compressed.",
    },
    Basic {
        path: "security.scms.lifecycle.cert_lifetime_s",
        explain: "How long each pseudonym certificate is valid: the period plus an overlap, so a vehicle always holds a valid one across the change of week.",
        recommended: "608,400 s (the week plus one hour, CAMP-EE); at least the i-period.",
    },
    Basic {
        path: "security.scms.lifecycle.pool_periods",
        explain: "How many weeks of certificates a vehicle holds when the run starts.",
        recommended: "2 (engine default); 1 to 8 (a THEA device is pre-loaded with three years, more than any run uses).",
    },
    Basic {
        path: "security.scms.lifecycle.topup_below_periods",
        explain: "A vehicle asks for its next week of certificates when fewer than this many weeks, the current one counted, remain; 0 turns top-ups off, so the pool only drains.",
        recommended: "1 (engine default); 0 to 4.",
    },
    Basic {
        path: "security.scms.lifecycle.max_periods_ahead",
        explain: "The most weeks ahead the Registration Authority provisions a device; a request for more is cut to this.",
        recommended: "156 weeks (three years, CAMP Primer); 1 to 156.",
    },
    Basic {
        path: "security.scms.lifecycle.report_shuffle_threshold",
        explain: "How many misbehaviour reports the Registration Authority mixes before passing them to the Misbehaviour Authority, so the authority cannot tell which vehicle reported when (CAMP-EE SCMS-765).",
        recommended: "10,000 (CAMP); 10 to 10,000.",
    },
    Basic {
        path: "security.scms.lifecycle.report_shuffle_window_s",
        explain: "The longest a misbehaviour report waits in the Registration Authority's mix; it delays every revocation by up to this long.",
        recommended: "86,400 s (one day, CAMP-EE SCMS-765); 5 to 86,400 s.",
    },
    Basic {
        path: "security.scms.lifecycle.first_batch_delay_s",
        explain: "How long after the Registration Authority acknowledges a request the vehicle first asks for its batch.",
        recommended: "60 s (engine default); 1 to 3,600 s.",
    },
    Basic {
        path: "security.scms.lifecycle.max_download_polls",
        explain: "download-maxReqs: how many times a vehicle asks for a batch that is not ready before it gives up on it (SCMS Manager EE profile, Table 3).",
        recommended: "100 (the US profile); 1 to 1,000.",
    },
    Basic {
        path: "security.scms.lifecycle.download_request_tolerance_s",
        explain: "How far a download request's time stamp may be from the Registration Authority's clock before the request is refused as stale or replayed (CAMP release 1.2.2, SCMS-1203).",
        recommended: "5 s (CAMP); 1 to 60 s.",
    },
    Basic {
        path: "security.scms.lifecycle.enrolment_lifetime_s",
        explain: "How long a vehicle's enrolment certificate, its long-term identity with the system, stays valid. It must be renewed before it ends or the vehicle can get no more pseudonyms.",
        recommended: "189,216,000 s (six years, CAMP release 1.2.2); 600 s to 220,752,000 s (seven years, CAMP release 1.1).",
    },
    Basic {
        path: "security.scms.lifecycle.reenrol_lead_s",
        explain: "How long before the enrolment certificate expires the vehicle asks for its successor; the successor starts when the current one ends, so an early request costs no validity.",
        recommended: "47,347,200 s (548 days, SCMS Manager's 12 to 24 months); 60 s to 63,072,000 s.",
    },
    Basic {
        path: "security.scms.lifecycle.reenrol_download_delay_s",
        explain: "How long after a re-enrolment request the vehicle downloads its new enrolment certificate.",
        recommended: "300 s (the profile's download-minWait); 1 to 3,600 s.",
    },
    Basic {
        path: "security.scms.lifecycle.crl_broadcast_interval_s",
        explain: "How often a roadside unit carrying the revocation list repeats it over the air to vehicles in range; only roadside units with the crl role do this, and the US deployment does not use it.",
        recommended: "5 s (the longest USDOT RSU v4.1 store-and-repeat interval); 1 to 60 s.",
    },
    Basic {
        path: "security.scms.lifecycle.ma_max_open_cases",
        explain: "The most investigations the Misbehaviour Authority keeps open at once; a case beyond it waits for one to close. No authority publishes its capacity.",
        recommended: "1,000 (an assumption); 1 to 10,000 to study an overloaded authority.",
    },
    Basic {
        path: "security.scms.lifecycle.report_store_quota_bytes",
        explain: "How much space a vehicle keeps for misbehaviour reports it has not uploaded yet. When a new report would overfill it, the vehicle deletes its lowest-weight report (the older one on a tie), possibly the new one. SCMS Manager MBR v1.0 §2.2 leaves the size to each device.",
        recommended: "65,536 B (an assumption, 58 reference reports); 0 (keeps nothing) to 10 MB to study evidence crowd-out.",
    },
    Basic {
        path: "security.scms.lifecycle.report_daily_multiplier",
        explain: "Every day at the re-weighting hour each stored report's weight is multiplied by this, so older evidence is uploaded later and deleted sooner. SCMS Manager MBR v1.0 §2.3.1 DailyMultiplier.",
        recommended: "0.9 (MBR v1.0, every subtype); 0 to 1. 1 never ages a report.",
    },
    Basic {
        path: "security.scms.lifecycle.report_decay_hour_et",
        explain: "The Eastern-time hour at which stored reports are re-weighted each day. SCMS Manager MBR v1.0 §2.2: 5 am Eastern.",
        recommended: "5 (MBR v1.0); 0 to 23.",
    },
    Basic {
        path: "security.scms.lifecycle.report_weight_random_position",
        explain: "The weight a new RandomPosition report starts with: higher weights upload first and are deleted last. SCMS Manager MBR v1.0 §2.3.1.2.2 initialWeight.",
        recommended: "1.0 (MBR v1.0); 0 to 10.",
    },
    Basic {
        path: "security.scms.lifecycle.report_weight_constant_position",
        explain: "The weight a new ConstantPosition report starts with. SCMS Manager MBR v1.0 §2.3.1.2.3 initialWeight.",
        recommended: "1.0 (MBR v1.0); 0 to 10.",
    },
    Basic {
        path: "security.scms.lifecycle.report_weight_max_speed",
        explain: "The weight a new MaxSpeed report starts with. SCMS Manager MBR v1.0 §2.3.1.3.1 initialWeight.",
        recommended: "1.0 (MBR v1.0); 0 to 10.",
    },
    Basic {
        path: "security.scms.lifecycle.report_weight_max_acceleration",
        explain: "The weight a new MaxAcceleration report starts with. SCMS Manager MBR v1.0 §2.3.1.3.2 initialWeight.",
        recommended: "1.0 (MBR v1.0); 0 to 10.",
    },
    Basic {
        path: "security.scms.lifecycle.report_weight_brake_acceleration",
        explain: "The weight a new BrakeAcceleration report starts with. SCMS Manager MBR v1.0 §2.3.1.3.4 initialWeight.",
        recommended: "1.0 (MBR v1.0); 0 to 10.",
    },
    Basic {
        path: "security.scms.lifecycle.cert_shuffle_threshold",
        explain: "How many certificate requests the Registration Authority collects and mixes before passing them on, so the authority that signs the certificates cannot tell which vehicle asked when; a batch also leaves when its oldest request reaches the shuffle window. CAMP SCMS release 1.2.2, Table 67.",
        recommended: "1,000 (CAMP's production value); 10 to 10,000 (Brecht et al. 2018 used 10,000). It acts on a device's first request only: every vehicle in a run is provisioned before it joins, and its weekly top-ups are the RA's pre-generation, which does not wait in this mix, so a run's results do not change.",
    },
    Basic {
        path: "security.scms.lifecycle.cert_shuffle_window_s",
        explain: "The longest a certificate request may wait in the Registration Authority's mix before it is passed on, full batch or not. A longer wait hides each request among more others and delays a vehicle's certificates.",
        recommended: "172,800 s (two days, CAMP release 1.2.2); 3,600 to 172,800 s. Like the batch size, it acts on a device's first request, which a run makes before the vehicle joins.",
    },
    Basic {
        path: "security.scms.rx_beacon_max_age_s",
        explain: "How old a received safety beacon (BSM, CAM, PSM, VAM) may be, by the receiver's own clock, before it is thrown away unchecked (IEEE 1609.2 relevance check). It makes a replayed message harmless, and it is why a vehicle whose clock is badly wrong is ignored rather than reported for misbehaviour.",
        recommended: "2 s (CAR 2 CAR Communication Consortium Basic System Profile RS_BSP_168; the US J2945/1 value is not public, so this is an assumption); a year turns the check off for a study.",
    },
    Basic {
        path: "security.scms.rx_max_age_s",
        explain: "The same age limit for every other application message: signal phase, intersection maps, hazard warnings, collective perception.",
        recommended: "600 s (RS_BSP_168).",
    },
    Basic {
        path: "security.scms.rx_max_future_s",
        explain: "How far ahead of the receiver's own clock a message's time stamp may be. A sender whose clock runs ahead is ignored past it; the misbehaviour authority applies the same rule to report evidence.",
        recommended: "0.04 s (RS_BSP_168); a year turns the check off for a study.",
    },
    Basic {
        path: "security.scms.crl_overdue_tolerance_s",
        explain: "How long after a revocation list said the next one would be out a vehicle keeps trusting the certificates that list covers. Past it, the vehicle refuses every message those certificates sign, so a vehicle cut off from new lists (by an attacker or a dead link) stops trusting its neighbours. CAMP SCMS release 1.2.2, Table 67, overdue_CRL_tolerance.",
        recommended: "1,209,600 s (two weeks, CAMP's value); 0 to 2,419,200 s to study a delayed or blocked revocation list. The next list is due a week after each one (nextCrl), so the rule acts only in runs that span weeks of calendar time.",
    },
    Basic {
        path: "nodes.validated_cert_cache",
        explain: "How many already-checked certificates of other vehicles each device remembers, so a known sender costs only its signature check; a forgotten one is checked from scratch the next time it is heard. A small cache is what a certificate-flooding attack exhausts.",
        recommended: "4,096 (engine default; no device publishes one); 16 to 65,536. The peer certificate cache (128) usually forgets first.",
    },
    Basic {
        path: "security.scms.tls",
        explain: "The TLS version a vehicle uses for its secure session with the Registration Authority over a cellular link. TLS 1.2 needs one more round trip to set up each session than TLS 1.3, so certificate requests and downloads start later. The US profile admits both (SCMS Manager EE profile 2.1.18).",
        recommended: "1.3 (default); 1.2 to compare with older devices.",
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
                // The backend runs the same protocol, or the run refuses to start on a
                // scenario that names the other one (base/city.yaml names CAMP).
                set(
                    "actors.backend.protocol",
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
                set("actors.backend.protocol", json!(crate::phase2::ETSI_PKI)),
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
            "veremi" => (
                "VeReMi traces",
                "Per-receiver traces and a ground-truth log in the VeReMi Extension format, for detectors written against VeReMi.",
            ),
            "sumo-fcd" => (
                "SUMO FCD trace",
                "Every vehicle's position, speed and lane each step, as SUMO's fcd-output.",
            ),
            "sumo-tripinfo" => (
                "SUMO trip info",
                "One line per finished trip (duration, waiting time, time loss), as SUMO's tripinfo-output.",
            ),
            "sumo-edgedata" => (
                "SUMO edge data",
                "Per-road flow, speed, density and occupancy, as SUMO's edgeData.",
            ),
            "sumo-lanedata" => (
                "SUMO lane data",
                "The same measures per lane, as SUMO's laneData.",
            ),
            "sumo-summary" => (
                "SUMO summary",
                "Vehicles inserted, running and arrived each step, as SUMO's summary-output.",
            ),
            // Detectors need positions, so they are not one-click choices.
            "sumo-e1" | "sumo-e2" => continue,
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
        let mut patch = vec![set("messages.rate_hz", json!(hz))];
        // A generator runs once a mobility step, so above 10 Hz the step must shrink with
        // the interval (validate refuses the rate otherwise): 50 ms for 20 Hz, which also
        // doubles the mobility work.
        let note = if hz > 10 {
            patch.push(set("time.mobility_step_ms", json!(1000 / hz)));
            format!(
                " Vehicles also move in {} ms steps, so the run is slower.",
                1000 / hz
            )
        } else {
            String::new()
        };
        out.push(choice(
            &format!("{hz} Hz"),
            &format!(
                "Every vehicle broadcasts its awareness message (BSM or CAM) {hz} times a \
                 second; congestion control may still lengthen the interval.{note}"
            ),
            None,
            patch,
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
        g(
            field(
                "traffic",
                "Traffic",
                "actors.vehicles.demand.rate_veh_per_h",
                "How many vehicles enter the map each hour."
            ),
            "Traffic"
        ),
        g(
            field(
                "equipped",
                "Equipped share",
                "actors.vehicles.equipped_fraction",
                "The share of vehicles that carry a V2X radio (market penetration)."
            ),
            "Traffic"
        ),
        g(
            field(
                "pedestrians",
                "Pedestrians",
                "actors.vru.pedestrians",
                "People walking on the sidewalks and crossings."
            ),
            "Traffic"
        ),
        g(
            field(
                "cyclists",
                "Cyclists",
                "actors.vru.cyclists",
                "People cycling on the roads and bike lanes."
            ),
            "Traffic"
        ),
        g(
            field(
                "drivers",
                "Drivers",
                "actors.vehicles.drivers.realism",
                "How human the drivers are: calibrated, identical, heterogeneous or human (stop-and-go waves)."
            ),
            "Traffic"
        ),
        // Shares from scenarios/midtown-street-life.yaml's fleet (a stated choice there:
        // no classified count of Midtown traffic was read).
        g(
            class_toggle(
                "motorcycles",
                "Motorcycles",
                "motorcycle",
                0.02,
                "2 % of vehicles are motorcycles, taken from the cars."
            ),
            "Traffic"
        ),
        g(
            class_toggle(
                "trucks",
                "Trucks",
                "truck",
                0.03,
                "3 % of vehicles are trucks, taken from the cars."
            ),
            "Traffic"
        ),
        g(
            field(
                "rsus",
                "Roadside units",
                "actors.rsu_placement.count",
                "Radios at the busiest signalised junctions."
            ),
            "Traffic"
        ),
        g(
            field(
                "radio",
                "Radio",
                "radio.rat",
                "Which V2X radio technology vehicles use."
            ),
            "Radio"
        ),
        g(
            field(
                "tx_power",
                "Transmit power",
                "radio.devices.obu.tx_power_dbm",
                "A vehicle radio's conducted power; congestion control may lower it."
            ),
            "Radio"
        ),
        g(
            json!({"id": "message_rate", "label": "Message rate", "kind": "choice",
                 "explain": "How often each vehicle broadcasts its safety message.",
                 "options": message_rate_choices()}),
            "Radio"
        ),
        g(
            json!({"id": "security", "label": "Security", "kind": "choice",
                 "explain": "Which credential system signs and checks messages.",
                 "options": security_choices()}),
            "Security"
        ),
        g(
            json!({"id": "signature", "label": "Signature", "kind": "choice",
                 "explain": "The signature on every message: classical, hybrid or post-quantum. A larger signature is sent in pieces and checked on hardware with figures for it.",
                 "options": signature_choices()}),
            "Security"
        ),
        g(
            field(
                "pseudonyms",
                "Pseudonym change",
                "security.pseudonym_change.strategy",
                "When a vehicle switches to a fresh certificate to stay untrackable."
            ),
            "Security"
        ),
        g(
            json!({"id": "attackers", "label": "Attackers", "kind": "choice",
                 "explain": "Whether some vehicles send false messages, and which kind.",
                 "options": attacker_choices()}),
            "Security"
        ),
        // The share of every attacker population at once; the Studio greys it out while
        // `threats.attackers` is empty. Bounded [0, 1] by validate's BOUNDS.
        g(
            json!({"id": "attacker_share", "label": "Attacker share", "kind": "share",
                 "path": "threats.attackers", "pointer": pointer_for("threats.attackers"),
                 "key": "fraction", "values": [0.01, 0.05, 0.1, 0.2, 0.3],
                 "explain": "The share of equipped vehicles that attack."}),
            "Security"
        ),
        g(
            field(
                "duration",
                "Run length",
                "time.duration_s",
                "How much simulated time the run covers."
            ),
            "Run"
        ),
        g(
            field(
                "seed",
                "Seed",
                "seed",
                "The random seed: the same seed and settings give the same run."
            ),
            "Run"
        ),
        g(
            json!({"id": "save", "label": "Save run data", "kind": "choice",
                 "explain": "Which files the run writes when it finishes, beside its recording.",
                 "options": export_choices(), "shows_files": true}),
            "Run"
        ),
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
