//! The words a settings form puts on a closed set, and the presets it offers beside a
//! number.
//!
//! The owner's review of 2026-10-06: "a lot of fields the user needs to manually fill out
//! … the user doesn't really know what is supposed to go there". The published schema
//! already said *which* values a field admits (`enum`, from `validate::CHOICES`, the
//! reflected unit enums and the registry's slots); what it did not say is what each value
//! *means* to someone who has not read the standard. This module is that half:
//!
//! * [`option_row`] — one selectable value with a label a person reads and one sentence
//!   saying what choosing it does. The labels live here, next to the engine that defines
//!   the values, so the page never hard-codes a model id.
//! * [`slot_values`] — the ids a model slot may hold, taken from the registry (or from the
//!   loader's own table where the loader is narrower than the registry), so a model added
//!   to a crate appears in the form without an edit here.
//! * [`extra_values`] — closed sets that are neither a `CHOICES` row nor a reflected enum:
//!   the applications, the roadside-unit roles, the region's channels.
//! * [`presets`] — named values for a number ("rush hour", "the J2945/1 five minutes"),
//!   each with its source; the form keeps a custom number beside them.
//! * [`title`] — a plain-language title for the fields whose Rust name reads badly as one.
//!
//! Nothing here changes what the loader accepts. A value missing a label falls back to a
//! humanised id and the model card's purpose, so a missing row costs polish, never a
//! choice.

use serde_json::{Value, json};
use v2xw_core::card::Family;
use v2xw_core::registry::Registry;

use crate::scenario::validate;

/// A label and a one-sentence description for `value` at `path`, and whether this build
/// accepts it (`false` for a value the schema names and the loader refuses).
fn words(path: &str, value: &str) -> Option<(&'static str, &'static str, bool)> {
    let tier = path.starts_with("radio.tiers.") || path.ends_with("_tier");
    if tier {
        return match value {
            // The 802.11p PHY distinguishes only high from the rest, and the sidelink maps
            // abstract to medium (`run::sidelink`): the settings audit found choosing it
            // changed nothing, so the words say what it does.
            "abstract" if path == "radio.tiers.phy" => Some((
                "Fast (abstract) — runs as Standard",
                "This build has no separate abstract PHY: the Standard (medium) reception \
                 model runs. Only propagation and channel access have an abstract tier.",
                true,
            )),
            // The run-cost audit (docs/validation/test-run-cost.md) found `mac: high` beside
            // a medium PHY identical to `medium` frame for frame: no MAC model reads the
            // high tier; it only unlocks the frame-level PHY, which requires it.
            "high" if path == "radio.tiers.mac" => Some((
                "Detailed (high) — runs as Standard",
                "Channel access has no separate high tier in this build: the Standard EDCA \
                 model runs. Choosing it is what allows the Detailed physical layer.",
                true,
            )),
            // The run-cost audit (docs/validation/test-run-cost.md, finding 4) measured all
            // three radio tiers at abstract only 11 % cheaper than medium on Manhattan:
            // each link is cheaper to price, but free space with no buildings lets 49 %
            // more links reach a receiver, and receptions (records, metrics, a node wake
            // and a verification each), not the radio model, are most of the cost. The
            // words for the two radio paths say so instead of promising a cheap run.
            "abstract" if path == "radio.tiers.propagation" => Some((
                "Simplified (abstract) — free space, no buildings",
                "Free-space loss with no buildings, shadowing or fading: each link is cheaper \
                 to price, but with nothing in the way more links reach each receiver, so a \
                 city run costs about as much as Standard; cap radio.range.max_m to make a \
                 large fleet cheap.",
                true,
            )),
            "abstract" if path == "radio.tiers.mac" => Some((
                "Simplified (abstract) — no channel access",
                "Every frame goes on air when it is queued, with no carrier sense, backoff \
                 or channel busy ratio (so no congestion control); it saves little run \
                 time, because receptions, not channel access, are most of the cost.",
                true,
            )),
            "abstract" => Some((
                "Fast (abstract)",
                "Closed-form stand-ins: cheapest, for very large fleets; least faithful.",
                true,
            )),
            "medium" => Some((
                "Standard (medium)",
                "The models the literature agrees on, without frame-level detail. The usual choice.",
                true,
            )),
            "high" => Some((
                "Detailed (high)",
                "Frame-level or geometry-traced detail: the most faithful and the slowest.",
                true,
            )),
            _ => None,
        };
    }
    let w = |l, d| Some((l, d, true));
    match (path, value) {
        // --- radio ----------------------------------------------------------------
        ("radio.rat" | "radio.adjacent_channel[].rat", "dsrc-80211p") => w(
            "DSRC / ITS-G5 (IEEE 802.11p)",
            "Wi-Fi-based V2X: listen-before-talk on a 10 MHz channel. The US pilots' radio and Europe's ITS-G5.",
        ),
        ("radio.rat" | "radio.adjacent_channel[].rat", "lte-v2x-pc5") => w(
            "C-V2X, LTE sidelink (PC5 Mode 4)",
            "3GPP Release 14 cellular V2X without a network: sensing-based semi-persistent scheduling on the SAE J3161/1 profile.",
        ),
        ("radio.rat" | "radio.adjacent_channel[].rat", "nr-v2x-pc5") => w(
            "C-V2X, 5G NR sidelink (PC5 Mode 2)",
            "3GPP Release 16 sidelink at 30 kHz on the ETSI EN 303 798 pool. No US deployment profile exists yet.",
        ),
        ("radio.rat" | "radio.adjacent_channel[].rat", "hybrid") => Some((
            "Hybrid (not available)",
            "Refused by this build: the hybrid selector arbitrates a direct radio against cellular, which is not wired.",
            false,
        )),
        ("radio.region", "us") => w(
            "United States, 2024 rules (FCC 24-123)",
            "C-V2X only on 5.905-5.925 GHz; DSRC is refused because its licences end in December 2026.",
        ),
        ("radio.region", "us-2016") => w(
            "United States, DSRC band plan (2016)",
            "802.11p on channel 172, the plan SAE J2945/1 was written for (47 CFR 90.377).",
        ),
        ("radio.region", "eu") => w(
            "Europe (ETSI EN 302 571)",
            "10 MHz channels at up to 33 dBm EIRP; ITS-G5, LTE-V2X and NR-V2X side by side.",
        ),
        ("radio.devices.obu.antenna_pattern", "tr37885") => w(
            "By vehicle type (3GPP TR 37.885)",
            "A roof antenna on cars and vans, front and rear panels on trucks and buses. The default.",
        ),
        ("radio.devices.obu.antenna_pattern", "rooftop") => w(
            "Roof antenna on every vehicle",
            "The same all-round roof antenna on every vehicle, trucks included.",
        ),
        ("radio.devices.obu.antenna_pattern", "isotropic") => w(
            "No pattern (isotropic)",
            "The same gain in every direction: the textbook simplification.",
        ),
        // network audit
        ("radio.dcc.reactive_table", "a1") => w(
            "Table A.1 (frames up to 1 ms)",
            "10 Hz relaxed down to 1 Hz restrictive (TS 102 687 V1.2.1 Table A.1). The default.",
        ),
        ("radio.dcc.reactive_table", "a2") => w(
            "Table A.2 (frames up to 500 µs)",
            "20 Hz relaxed down to 1 Hz restrictive, for short frames (Table A.2).",
        ),
        ("radio.mac.periodic_access_category" | "radio.mac.event_access_category", "vo") => w(
            "Voice (AC_VO, highest)",
            "The shortest wait (AIFSN 2, CWmin 3): where SAE J2945/1 puts the BSM and EN 302 663 the DENM.",
        ),
        ("radio.mac.periodic_access_category" | "radio.mac.event_access_category", "vi") => w(
            "Video (AC_VI)",
            "AIFSN 3, CWmin 7: where EN 302 663 Annex C puts the CAM.",
        ),
        ("radio.mac.periodic_access_category" | "radio.mac.event_access_category", "be") => w(
            "Best effort (AC_BE)",
            "AIFSN 6, CWmin 15: ordinary data, yields to every safety message.",
        ),
        ("radio.mac.periodic_access_category" | "radio.mac.event_access_category", "bk") => w(
            "Background (AC_BK, lowest)",
            "AIFSN 9, CWmin 15: the longest wait.",
        ),
        ("radio.channel_model.fading_coherence", "independent") => w(
            "Independent per message",
            "Every message on a link gets its own fade, whatever the speeds.",
        ),
        ("radio.channel_model.fading_coherence", "doppler") => w(
            "Doppler (speed-dependent)",
            "A fade lasts one coherence time: long for vehicles standing still, a fraction of a millisecond at highway speed.",
        ),
        ("radio.channel_model.fading", "none") => w(
            "None",
            "No small-scale fading: the received power is the path loss and shadowing only.",
        ),
        ("radio.channel_model.fading", "rayleigh") => w(
            "Rayleigh",
            "No direct path, only scattered copies: the harshest common fading (Nakagami m = 1).",
        ),
        ("radio.channel_model.fading", "rice") => w(
            "Rice",
            "A direct path plus scatter; its strength over the scatter is the Rice factor K.",
        ),
        ("radio.channel_model.fading", "nakagami") => w(
            "Nakagami-m",
            "The V2V literature's usual fit; its shape m runs from 1 (Rayleigh) up to milder channels.",
        ),
        // --- network and messages -------------------------------------------------
        ("net.layer", "wsmp") => w(
            "WSMP (IEEE 1609.3, US)",
            "The US short message protocol: a 5-octet header on a BSM.",
        ),
        ("net.layer", "gn-btp") => w(
            "GeoNetworking + BTP (ETSI, Europe)",
            "The European network and transport layers: 44 octets of headers on a CAM.",
        ),
        ("net.channel_access", "continuous") => w(
            "Continuous (one channel)",
            "The deployed US profile: safety messages go out on one channel as soon as they are made.",
        ),
        ("net.channel_access", "alternating") => w(
            "Alternating (IEEE 1609.4)",
            "One radio switches channel every 50 ms; a message made on the wrong half waits up to 54 ms.",
        ),
        ("net.geonet.forwarding", "on-air") => w(
            "On the air (recommended)",
            "Each forwarded copy is a real frame: it queues, contends, can collide and loads the channel.",
        ),
        ("net.geonet.forwarding", "overlay") => w(
            "Protocol only (no MAC)",
            "Forwarding runs over a fixed radio range with no contention or collisions: optimistic reach.",
        ),
        ("net.dcc_cbr", "local") => w(
            "Own measurement",
            "Each radio throttles for the channel load it hears itself.",
        ),
        ("net.dcc_cbr", "global") => w(
            "Two-hop maximum (ETSI GeoNetworking)",
            "Radios share their load in every header and throttle for congestion up to two hops away.",
        ),
        ("net.access_categories", "single") => w(
            "One queue (AC_VO)",
            "Every message waits in the same top-priority queue, as deployed US radios do.",
        ),
        ("net.access_categories", "etsi-profiles") => w(
            "ETSI DCC profiles",
            "Hazard warnings jump the queue; CAMs and forwarded copies wait longer when the channel is busy.",
        ),
        ("net.geonet.area_forwarding", "cbf") => w(
            "Contention-based (CBF)",
            "The farthest receiver rebroadcasts first and the nearer ones stand down: few transmissions, the standard's choice.",
        ),
        ("net.geonet.area_forwarding", "simple") => w(
            "Flooding (simple)",
            "Every vehicle rebroadcasts the warning once: fast and robust, many transmissions.",
        ),
        ("net.geonet.non_area_forwarding", "greedy") => w(
            "Greedy",
            "Hand the warning to the neighbour nearest the area (the standard's default).",
        ),
        ("net.geonet.non_area_forwarding", "cbf") => w(
            "Contention-based",
            "Broadcast towards the area; the receiver that gets closest rebroadcasts first.",
        ),
        ("net.geonet.cbf_variant", "etsi") => w(
            "As evaluated in Artery (no duplicate check)",
            "A copy heard after forwarding is taken as new: very many transmissions (Amador et al. 2022).",
        ),
        ("net.geonet.cbf_variant", "dpd") => w(
            "With duplicate detection",
            "Each vehicle forwards a warning at most once; any copy makes a contender stand down, which can stall the spread.",
        ),
        ("net.geonet.cbf_variant", "gpc") => w(
            "Geographic cancellation (recommended)",
            "Stand down only when the copy came from farther along: near-full reach at low overhead (Amador et al. 2022).",
        ),
        ("messages.sets[]", "bsm") => w(
            "BSM: Basic Safety Message",
            "Every equipped vehicle's position, speed and heading, ten times a second (SAE J2735/J2945/1).",
        ),
        ("messages.sets[]", "cam") => w(
            "CAM: Cooperative Awareness Message",
            "The European counterpart of the BSM, sent on change of position, speed or heading (ETSI EN 302 637-2).",
        ),
        ("messages.sets[]", "denm") => w(
            "DENM: hazard warning",
            "An event message raised by a hazard such as hard braking or a breakdown (ETSI EN 302 637-3).",
        ),
        ("messages.sets[]", "spat") => w(
            "SPaT: signal phase and timing",
            "Roadside units broadcast each signal's state and time to change (needs units with the spat role).",
        ),
        ("messages.sets[]", "map") => w(
            "MAP: intersection geometry",
            "Roadside units broadcast the lanes and stop lines of their junction (needs the map role).",
        ),
        ("messages.sets[]", "srm") => w(
            "SRM: signal request (emergency priority)",
            "Emergency vehicles ask the junction for a green; needs SPaT and MAP and the size-model codec.",
        ),
        ("messages.sets[]", "ssm") => w(
            "SSM: signal status",
            "The junction's answer to a signal request.",
        ),
        ("messages.sets[]", "psm") => w(
            "PSM: Personal Safety Message",
            "Pedestrians' and cyclists' devices announce where they are (SAE J2735).",
        ),
        ("messages.sets[]", "vam") => w(
            "VAM: vulnerable road user awareness",
            "The European counterpart of the PSM (ETSI TS 103 300-3).",
        ),
        ("messages.sets[]", "cpm") => w(
            "CPM: collective perception",
            "Vehicles share what their sensors detect (ETSI TS 103 324).",
        ),
        ("messages.codec_tier", "uper") => w(
            "Real encoding (UPER)",
            "Every message is encoded to its real bytes; a set with no encoder is refused.",
        ),
        ("messages.codec_tier", "size-model") => w(
            "Modelled sizes",
            "Messages without an encoder travel as payloads of their validated length; the rest are still encoded.",
        ),
        // --- security -------------------------------------------------------------
        ("security.envelope", "ieee1609.2") => w(
            "IEEE 1609.2 (US)",
            "The secured-message format of the US SCMS.",
        ),
        ("security.envelope", "etsi103097") => w(
            "ETSI TS 103 097 (Europe)",
            "The European profile of the same structure; required by the ETSI credential system.",
        ),
        ("security.signature", "ecdsa-p256") => w(
            "ECDSA P-256",
            "The signature every deployed V2X system uses today: 64-octet signatures.",
        ),
        ("security.signature", "ecdsa-brainpoolp256r1") => w(
            "ECDSA brainpool P-256r1",
            "The European alternative curve; same size, charged the P-256 time.",
        ),
        ("security.signature", "ecdsa-p384") => w(
            "ECDSA P-384",
            "A longer curve: 32 more octets per signature.",
        ),
        ("security.signature", "hybrid-falcon512-ecdsa-p256") => w(
            "Post-quantum hybrid: Falcon-512 + P-256",
            "Adds 666 octets per signature and 1,563 per certificate; needs a fragmenter and a profile with a Falcon figure.",
        ),
        ("security.signature", "hybrid-mldsa44-ecdsa-p256") => w(
            "Post-quantum hybrid: ML-DSA-44 + P-256",
            "FIPS 204 Dilithium beside P-256: the largest messages; needs a fragmenter and a profile with an ML-DSA figure.",
        ),
        ("security.crypto_mode", "modeled") => w(
            "Costed (fast)",
            "Signing and verification take the hardware's measured time; the outcome is modelled.",
        ),
        ("security.crypto_mode", "real") => w(
            "Computed (exact, slow)",
            "Real ECDSA over real bytes. Only P-256 is available in this mode.",
        ),
        ("security.verification_policy", "verify-all") => w(
            "Verify every message",
            "Every received message is verified, whatever it costs the security module.",
        ),
        ("security.verification_policy", "on-demand") => w(
            "Verify on demand",
            "Only messages an application is about to act on are verified.",
        ),
        ("security.verification_policy", "prioritized") => w(
            "Verify nearest first",
            "When the module cannot keep up, the closest and most relevant senders are verified first.",
        ),
        ("security.pseudonym_change.strategy", "time") => w(
            "Every few minutes (SAE J2945/1)",
            "Change certificate after period_s of use; the US rule is 300 s.",
        ),
        ("security.pseudonym_change.strategy", "distance") => w(
            "Every few kilometres (NYC pilot)",
            "Change after distance_m of driving; the NYC Connected Vehicle Pilot used 2 km.",
        ),
        ("security.pseudonym_change.strategy", "j2945-1") => w(
            "SAE J2945/1 CERTCHG (5 min and 2 km)",
            "Change once 5 minutes have passed and at least 2 km have been driven since the last change.",
        ),
        ("security.pseudonym_change.strategy", "nyc-pilot") => w(
            "NYC CV pilot (2 km or 5 min)",
            "Change after 2 km of driving or 5 minutes, whichever comes first.",
        ),
        ("security.pseudonym_change.strategy", "c2c-cc") => w(
            "European car makers' rule (C2C-CC)",
            "After 0.8-1.5 km, then 0.8 km plus 2-6 minutes, then every 10-35 km.",
        ),
        ("security.pseudonym_change.strategy", "mix-zone") => w(
            "In mix zones at junctions",
            "Change inside a zone around each signalised junction, where many vehicles mingle.",
        ),
        ("security.pseudonym_change.strategy", "silent") => w(
            "Never (keep one identity)",
            "No change: the baseline a tracker finds easiest.",
        ),
        // --- time -----------------------------------------------------------------
        ("time.des_resolution", "1ns") => w(
            "1 nanosecond",
            "The finest promise; models that time frames in microseconds are content.",
        ),
        ("time.des_resolution", "1us") => w(
            "1 microsecond",
            "What the radio models time frames in. The usual choice.",
        ),
        ("time.des_resolution", "1ms") => w(
            "1 millisecond",
            "Only for runs without a frame-level radio; refused with a medium or high PHY or MAC.",
        ),
        // --- traffic --------------------------------------------------------------
        ("actors.vehicles.demand.kind", "mobility/demand/poisson") => w(
            "Steady arrivals (Poisson)",
            "Vehicles enter at random at the rate below, each with its own trip across the map.",
        ),
        ("actors.vehicles.demand.kind", "mobility/demand/tr36885-drop") => w(
            "3GPP drop (TR 36.885)",
            "Every vehicle placed at once at the 3GPP evaluation spacing; for radio benchmarks.",
        ),
        ("actors.vehicles.demand.kind", "mobility/demand/none") => w(
            "No vehicles",
            "No vehicle demand: pedestrians, roadside units and the backend only.",
        ),
        ("actors.vehicles.drivers.realism", "calibrated") => w(
            "Calibrated city drivers",
            "Each driver their own free speed, reaction at a green and following style; tuned to HCM saturation flow.",
        ),
        ("actors.vehicles.drivers.realism", "identical") => w(
            "Identical (textbook model)",
            "Every driver the same ideal model: the limit exactly, instant reactions. For model studies.",
        ),
        ("actors.vehicles.drivers.realism", "heterogeneous") => w(
            "Heterogeneous",
            "Calibrated, and each driver keeps their own gap and accelerates and brakes their own way.",
        ),
        ("actors.vehicles.drivers.realism", "human") => w(
            "Human (reaction time and misjudgement)",
            "Heterogeneous, with a 0.8 s reaction and misjudged gaps: dense traffic forms stop-and-go waves.",
        ),
        ("actors.vru.behaviour", "observed") => w(
            "As observed in Midtown",
            "Walking speeds by age, crossing against the signal and mid-block as field studies measured.",
        ),
        ("actors.vru.behaviour", "document") => w(
            "Textbook (everyone obeys)",
            "One walking speed, everyone waits for WALK, nobody crosses mid-block.",
        ),
        ("actors.rsus[].roles[]", "report-forward") => w(
            "Relay misbehaviour reports",
            "Forwards vehicles' reports to the backend when they have no cellular link.",
        ),
        ("actors.rsus[].roles[]", "crl") => w(
            "Broadcast the revocation list",
            "Repeats the certificate revocation list on the air.",
        ),
        ("actors.rsus[].roles[]", "provisioning-proxy") => w(
            "Deliver certificate top-ups",
            "Carries pseudonym-certificate downloads for vehicles without cellular.",
        ),
        ("actors.rsus[].roles[]", "spat") => w(
            "Signal timing (SPaT) and priority",
            "Broadcasts the junction's signal state and answers emergency-vehicle requests.",
        ),
        ("actors.rsus[].roles[]", "map") => w(
            "Intersection map (MAP)",
            "Broadcasts the junction's lane geometry.",
        ),
        ("actors.rsus[].roles[]", "spat-map") => w("Signal timing and map", "Both of the above."),
        // --- world ----------------------------------------------------------------
        ("world.highway_preset", "urban-us-nyc") => w(
            "New York City",
            "25 mph default on every street; no right turn on red.",
        ),
        ("world.highway_preset", "urban-us-portland") => w(
            "Portland, Oregon",
            "20 mph in the business district; right on red after a stop.",
        ),
        ("world.highway_preset", "urban-de") => w(
            "German city",
            "50 km/h in built-up areas (StVO); no right on red.",
        ),
        ("world.highway_preset", "sumo-german") => w(
            "SUMO defaults (German design speeds)",
            "netconvert's type map, for comparison with SUMO; not an urban legal limit.",
        ),
        ("weather.initial", "clear") => w("Clear", "No precipitation, unlimited visibility."),
        ("weather.initial", "rain") => w(
            "Rain",
            "Attenuates 5.9 GHz slightly and changes how people drive.",
        ),
        ("weather.initial", "snow") => w("Snow", "Lower visibility and a slippery surface."),
        ("weather.initial", "sleet") => w("Sleet", "Freezing rain: the worst surface."),
        ("weather.initial", "fog") => {
            w("Fog", "Visibility collapses; the radio is barely affected.")
        }
        ("weather.initial", "wind") => w("Wind", "Strong wind with no precipitation."),
        // --- applications ---------------------------------------------------------
        ("apps.enabled[]", "fcw") => w(
            "Forward collision warning",
            "Warns when the car ahead is closing at a time to collision of 2.4 s or less.",
        ),
        ("apps.enabled[]", "eebl") => w(
            "Emergency electronic brake light",
            "Warns when a vehicle ahead brakes harder than 0.4 g.",
        ),
        ("apps.enabled[]", "ima") => w(
            "Intersection movement assist",
            "Warns of a crossing vehicle about to reach the same conflict point.",
        ),
        ("apps.enabled[]", "lta") => w(
            "Left turn assist",
            "Warns a left turner when the oncoming gap is too short.",
        ),
        ("apps.enabled[]", "bsw") => w(
            "Blind spot warning",
            "Warns of a vehicle in the blind spot before a lane change.",
        ),
        ("apps.enabled[]", "pcw") => w(
            "Pedestrian collision warning",
            "Warns when a pedestrian or cyclist will be in the path within 3 s.",
        ),
        ("apps.enabled[]", "rlvw") => w(
            "Red light violation warning",
            "Warns when the driver can no longer stop comfortably for a red.",
        ),
        ("apps.enabled[]", "glosa") => w(
            "Green light speed advisory",
            "Advises the speed that arrives on green; drivers follow it at the compliance share.",
        ),
        // --- detection and threats: the ids whose card purpose reads poorly --------
        (_, "detect/legacy-12") => w(
            "Standard 12-check suite",
            "Plausibility and consistency checks on every message (the VeReMi detector set).",
        ),
        (_, "detect/us-sm-mbr") => w(
            "Strict US profile (SM-MBR)",
            "Only the SCMS Manager BSM report subtypes at their published thresholds (1 m drift, 90 m/s, 10 m/s²), every BSM checked, no streak gate.",
        ),
        (_, "threat/ma/corroborated") => w(
            "Corroborating authority",
            "Revokes only on independent, corroborated evidence sustained over time.",
        ),
        (_, "threat/ma/legacy-window") => w(
            "Legacy window authority",
            "Revokes once three trusted reporters agree within a window: the legacy engine's rule, kept for parity.",
        ),
        (_, "threat/observer/passive-privacy") => w(
            "Roadside eavesdropper",
            "Passive sniffers at a share of the signalised junctions try to link each car's pseudonyms.",
        ),
        (_, "threat/attacker/compromised-rsu") => w(
            "Compromised roadside unit",
            "A trusted unit under an attacker's control poisons or drops what it relays.",
        ),
        (_, "backhaul/fixed") => w(
            "Fixed line",
            "A wired link with a set latency and capacity.",
        ),
        (_, "backhaul/cellular") => w(
            "Cellular",
            "The unit reaches the backend over the mobile network.",
        ),
        (_, "backhaul/fibre") => w(
            "Fibre",
            "Metro fibre to the backend: 1 Gbit/s and well under a millisecond.",
        ),
        (_, "backhaul/dsl") => w(
            "DSL (copper)",
            "A VDSL2 phone line: 20 Mbit/s up, 80 down, about 9 ms each way.",
        ),
        (_, "backhaul/microwave") => w(
            "Microwave",
            "A point-to-point radio hop: 400 Mbit/s, about 0.2 ms, rare outages.",
        ),
        (_, "backhaul/none") => w("None (offline)", "The unit has no backend link."),
        (_, "backend-net/fixed") => w(
            "Fixed links",
            "Links between backend authorities with a set latency and capacity.",
        ),
        (_, "cellular/uu/cell-capacity-mm1") => w(
            "Cellular with cell capacity",
            "Each cell's capacity shared by its users (an M/M/1 queue per device).",
        ),
        (_, "cellular/uu/fixed-latency") => w(
            "Cellular, measured latency",
            "A fixed one-way latency measured on a real network.",
        ),
        (_, "cellular/uu/handover-outage") => w(
            "Cellular with handovers",
            "Adds the interruption and loss of handovers between cells.",
        ),
        (_, "cellular/uu/packet-pf") => w(
            "Cellular, packet level",
            "Real base stations, a shared scheduler, retransmissions and TCP/TLS for backend traffic; slower to run.",
        ),
        (_, "fragmenter/none") => w(
            "No fragmentation",
            "A signed message larger than the network's limit is refused.",
        ),
        (_, "fragmenter/generic-sdu") => w(
            "Split into pieces",
            "A large message is sent in pieces and reassembled; it arrives only if every piece does.",
        ),
        (_, "fragmenter/facilities-segmentation") => w(
            "Independent segments (ETSI)",
            "Each segment is a message of its own (GeoNetworking only).",
        ),
        (_, "fragmenter/cert-cycle-partial-hybrid") => w(
            "Post-quantum certificate spread over a cycle",
            "The hybrid certificate rides in the first messages of each five-message cycle.",
        ),
        (_, "generator/bsm-j2945-1") => w(
            "BSM timing (SAE J2945/1)",
            "Tunes when Basic Safety Messages are sent.",
        ),
        (_, "generator/cam-en302637-2") => w(
            "CAM triggering (ETSI EN 302 637-2)",
            "Tunes the CAM triggering intervals and thresholds.",
        ),
        (_, "protocol/scms/camp") => w(
            "US SCMS (CAMP)",
            "The US credential system: butterfly keys, two linkage authorities, linkage-value CRL.",
        ),
        (_, "protocol/etsi/ts102941") => w(
            "European CCMS (ETSI TS 102 941)",
            "Authorization tickets, an enrolment authority and passive revocation.",
        ),
        _ => None,
    }
}

/// A label and a description for one legacy attack type (`v2xw_threat::AttackKind`),
/// written from the type's own documentation in that crate.
fn attack_words(kind: &str) -> Option<(&'static str, &'static str)> {
    Some(match kind {
        "ConstPos" => (
            "Fake position: frozen",
            "Keeps claiming the first position it sent while it drives on.",
        ),
        "ConstPosOffset" => (
            "Fake position: shifted",
            "Claims a position a constant distance from where it is.",
        ),
        "RandomPos" => (
            "Fake position: random",
            "Claims a random position near itself, redrawn every message.",
        ),
        "Teleport" => (
            "Fake position: teleporting",
            "Jumps its claimed position every fourth second.",
        ),
        "SineWavePos" => (
            "Fake position: weaving",
            "Swings its claimed position from side to side across its lane.",
        ),
        "ConstSpeedOffset" => (
            "Fake speed: shifted",
            "Adds a constant offset to the speed it claims.",
        ),
        "RandomSpeed" => ("Fake speed: random", "Claims a random speed."),
        "StopAndGo" => (
            "Fake speed: stop-and-go",
            "Alternates its claimed speed between stopped and fast.",
        ),
        "ReversedHeading" => (
            "Fake heading: reversed",
            "Claims to be driving the opposite way.",
        ),
        "HeadingOffset" => (
            "Fake heading: shifted",
            "Adds a constant offset to the heading it claims.",
        ),
        "DataReplay" => (
            "Replay attack",
            "Re-sends its own earlier messages with their old timestamps.",
        ),
        "SlowDrift" => (
            "Fake position: slow drift",
            "A position error that grows slowly from nothing, hard to notice.",
        ),
        "AlongRoadOffset" => (
            "Fake position: along the road",
            "Shifts its claimed position along the road, so it stays on the map.",
        ),
        "Sybil" => (
            "Ghost vehicles (Sybil)",
            "Sends extra signed messages for ghost vehicles that do not exist, each under its own valid pseudonym.",
        ),
        "DoS" => (
            "Message flood (DoS)",
            "Not runnable yet: a flood needs extra transmissions per interval.",
        ),
        "DelayedMessages" => (
            "Delayed messages",
            "Stamps its messages with a stale generation time.",
        ),
        "InvalidSignature" => (
            "Invalid signature",
            "Signs with a key that does not match its certificate.",
        ),
        "ExpiredCert" => (
            "Expired certificate",
            "Presents a certificate past its validity.",
        ),
        "NotYetValid" => (
            "Certificate not yet valid",
            "Presents a certificate whose validity has not begun.",
        ),
        "OutOfOrder" => (
            "Out-of-order timestamps",
            "Stamps messages with times that go backwards.",
        ),
        "DoSRandom" => (
            "Random flood (DoS)",
            "Not runnable yet: a flood needs extra transmissions per interval.",
        ),
        "Disruptive" => (
            "Everything inconsistent",
            "Falsifies position, speed and heading at once, inconsistently.",
        ),
        "PosSpeedInconsistent" => (
            "Moving while claiming to stop",
            "Its position advances while it claims to be almost stopped.",
        ),
        "PosHeadingInconsistent" => (
            "Sideways while claiming straight",
            "Its position swings sideways while it claims a straight heading.",
        ),
        "EventualStop" => (
            "Fake stop",
            "Drives honestly, then freezes its claimed position while claiming to creep.",
        ),
        "VruImpersonation" => (
            "Posing as a pedestrian",
            "Not runnable yet: a BSM does not carry a station type.",
        ),
        "VruPositionSpoof" => (
            "Fake pedestrian",
            "Not runnable yet: a BSM does not carry a station type.",
        ),
        "FakeHazard" => (
            "Fake hazard warning",
            "Not runnable yet: the host does not send an attacker's event messages.",
        ),
        "SelectiveDrop" => (
            "Drops relayed messages",
            "Not runnable yet: vehicles here relay nothing.",
        ),
        _ => return None,
    })
}

/// Every attack type a scenario can name, `threat/attacker/legacy/<Kind>`, with the ones
/// the engine refuses to run (`phase2::unrendered`) marked unavailable and listed last.
fn attacker_rows() -> Vec<Value> {
    let mut rows: Vec<(bool, Value)> = v2xw_threat::AttackKind::ALL
        .iter()
        .map(|&kind| {
            let name = kind.as_str();
            let value = format!("threat/attacker/legacy/{name}");
            let available = crate::phase2::unrendered(kind).is_none();
            let (label, description) = attack_words(name).unwrap_or((name, ""));

            (
                available,
                json!({"value": value, "label": label, "description": description,
                       "available": available}),
            )
        })
        .collect();
    rows.sort_by_key(|(available, _)| !*available);
    rows.into_iter().map(|(_, r)| r).collect()
}

/// Words for the abbreviations that appear in model ids, so a humanised id reads.
const ID_WORDS: &[(&str, &str)] = &[
    ("Pos", "position"),
    ("Const", "constant"),
    ("DoS", "denial of service"),
    ("Sybil", "Sybil"),
    ("Rand", "random"),
    ("obu", "OBU"),
    ("rsu", "RSU"),
    ("uu", "cellular"),
    ("mm1", "M/M/1"),
];

/// A model id in words: its last segment, CamelCase and dashes split, abbreviations
/// spelled out. `threat/attacker/legacy/ConstPosOffset` → "Constant position offset".
pub fn humanise_id(id: &str) -> String {
    let last = id.rsplit('/').next().unwrap_or(id).replace("DoS", "-DoS-");
    let mut words: Vec<String> = Vec::new();
    for chunk in last.split(['-', '_']).filter(|c| !c.is_empty()) {
        if chunk == "DoS" {
            words.push(chunk.to_string());
            continue;
        }
        // Split CamelCase at a lower-to-upper change: "ConstPosOffset" → Const Pos Offset.
        let mut cur = String::new();
        let mut prev_lower = false;
        for c in chunk.chars() {
            if c.is_uppercase() && prev_lower && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = c.is_lowercase() || c.is_ascii_digit();
            cur.push(c);
        }
        if !cur.is_empty() {
            words.push(cur);
        }
    }
    let spelled: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let word = ID_WORDS
                .iter()
                .find(|(k, _)| k == w)
                .map_or_else(|| w.clone(), |(_, v)| (*v).to_string());
            let keep = word.len() > 1 && word.chars().all(|c| !c.is_lowercase());
            if keep || word == "Sybil" {
                word
            } else if i == 0 {
                let mut c = word.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase())
                    .unwrap_or_default()
            } else {
                word.to_lowercase()
            }
        })
        .collect();
    spelled.join(" ")
}

/// The first sentence of a card's purpose, short enough for an option's description.
fn first_sentence(text: &str) -> String {
    let t = text.trim();
    let cut = t
        .char_indices()
        .find(|&(i, c)| c == '.' && t[i + 1..].starts_with(' '))
        .map_or(t.len(), |(i, _)| i + 1);
    let s = &t[..cut];
    if s.chars().count() > 200 {
        let mut short: String = s.chars().take(197).collect();
        short.push('…');
        short
    } else {
        s.to_string()
    }
}

/// One selectable value: `{value, label, description, available}`.
///
/// `fallback` is the description to use when this module has no words for the value —
/// a reflected variant's doc comment or a model card's purpose.
pub fn option_row(path: &str, value: &str, fallback: &str) -> Value {
    // A state of the city library: its name, and its default limit and turn rule in words.
    if path == "world.highway_preset"
        && let Some(j) =
            v2xw_world::osm::HighwayPreset::parse(value).and_then(|p| p.us_jurisdiction())
    {
        return json!({
            "value": value,
            "label": j.name,
            "description": format!(
                "{} mph default on untagged streets; right on red after a stop{}.",
                j.urban_mph,
                if j.cited { "" } else { " (generic US default, not this state's statute)" }
            ),
            "available": true,
        });
    }
    // An on-board unit whose profile publishes no ECDSA signing cost is refused by the
    // loader for every vehicle (a vehicle signs every message on its own hardware), so the
    // dropdown says so instead of offering a choice that cannot run.
    if matches!(path, "nodes.default_obu" | "nodes.per_class.*")
        && let Some(p) = v2xw_node::profiles::get(value)
        && p.op_cost(v2xw_node::profile::signature_ops("ecdsa-p256").0)
            .is_none()
    {
        return json!({
            "value": value,
            "label": format!("{} (cannot sign)", humanise_id(value)),
            "description": "Its sources publish no ECDSA signing rate, so a vehicle on it \
                            could not sign a message; choose a unit that publishes one.",
            "available": false,
        });
    }
    match words(path, value) {
        Some((label, description, available)) => json!({
            "value": value, "label": label, "description": description,
            "available": available,
        }),
        None => json!({
            "value": value,
            "label": if value.contains('/') { humanise_id(value) } else { plain(value) },
            "description": first_sentence(fallback),
            "available": true,
        }),
    }
}

/// A short value in words: `us-2016` → "Us 2016".
fn plain(value: &str) -> String {
    let spaced = value.replace(['-', '_'], " ");
    let mut c = spaced.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => spaced,
    }
}

/// The ids a model slot at `path` offers, with whether the set is closed (the loader
/// refuses anything else) — `None` when `path` is not a slot's id.
///
/// The registry is the source; where the loader is narrower than the registry (the
/// demand models, the hardware profiles) the loader's own table is used, so the form
/// never offers what Check would refuse.
pub fn slot_values(path: &str, registry: &Registry) -> Option<(Vec<Value>, bool)> {
    let slot = super::publish::SLOTS.iter().find(|s| {
        if s.bare_id {
            s.path == path
        } else {
            path.strip_suffix(".id") == Some(s.path)
        }
    })?;
    let purpose = |id: &str| -> String {
        registry
            .resolve(id)
            .and_then(|r| registry.iter_by_id().find(|(m, _)| *m == r))
            .map(|(_, m)| m.card.purpose.clone())
            .unwrap_or_default()
    };
    // The loader's own tables, where it enforces one.
    let closed: Option<Vec<String>> = match slot.path {
        "actors.vehicles.demand.kind" => Some(
            [
                "mobility/demand/poisson",
                "mobility/demand/tr36885-drop",
                "mobility/demand/none",
            ]
            .iter()
            .filter(|id| crate::wiring::DEMAND_KINDS.contains(id))
            .map(|s| (*s).to_string())
            .collect(),
        ),
        "nodes.default_obu" | "nodes.per_class.*" => Some(
            validate::obu_profile_ids()
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        // The three jammers `run::jamming::jammer_specs` builds; anything else is refused.
        "threats.jammers[]" => Some(
            [
                v2xw_radio::jamming::ConstantJammer::ID,
                v2xw_radio::jamming::PulsedJammer::ID,
                v2xw_radio::jamming::ReactiveJammer::ID,
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        ),
        // The ids the security path builds by name (`phase2`, `backend`,
        // `pseudonym_policy`); the catalogue registers few of these as cards, so the
        // registry alone offered an empty or wrong list for them.
        "detection.local[]" => Some(vec![
            crate::phase2::LEGACY_12.to_string(),
            crate::phase2::US_SM_MBR.to_string(),
        ]),
        "detection.ma" => Some(vec![
            crate::phase2::MA_CORROBORATED.to_string(),
            crate::phase2::MA_LEGACY_WINDOW.to_string(),
        ]),
        "net.backhaul" | "actors.rsus[].backhaul" => Some(
            crate::backend::BACKHAUL_MODELS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        ),
        "net.backend_net" | "actors.backend.entities.*.net" => Some(
            crate::backend::BACKEND_NET_MODELS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        ),
        "threats.eavesdropper" => Some(vec![crate::pseudonym_policy::EAVESDROPPER_ID.to_string()]),
        "threats.compromised_rsu_attack" => {
            Some(vec![v2xw_threat::attack_rsu::MODEL_ID.to_string()])
        }
        // Each slot offers only the hardware that belongs there: a roadside unit's list
        // holds the RSU profiles, a backend entity's the server and base-station ones.
        "actors.rsus[].profile" => Some(
            validate::all_profile_ids()
                .into_iter()
                .filter(|id| id.starts_with("rsu/"))
                .map(str::to_string)
                .collect(),
        ),
        "actors.backend.entities.*.profile" => Some(
            validate::all_profile_ids()
                .into_iter()
                .filter(|id| id.starts_with("backend/") || id.starts_with("bs/"))
                .map(str::to_string)
                .collect(),
        ),
        _ => None,
    };
    // The attacker slot's ids are not registry cards: the legacy family is one card and
    // its types are named `threat/attacker/legacy/<Kind>` (`phase2`, `validate`).
    if slot.path == "threats.attackers[].id" {
        return Some((attacker_rows(), true));
    }
    if let Some(ids) = closed {
        let rows = ids
            .iter()
            .map(|id| option_row(path, id, &purpose(id)))
            .collect();
        return Some((rows, true));
    }
    let mut rows: Vec<(bool, Value)> = registry
        .iter_by_id()
        .filter(|(_, m)| m.card.family == slot.family)
        // `generator` names a procedural world's generator; the world family also holds
        // the importers and the terrain models, which are not chosen here.
        .filter(|(_, m)| {
            slot.path != "world.source.generator"
                || m.card.id.starts_with("world/source/procedural")
        })
        // A jammer is an attacker-family card, chosen in `threats.jammers`, not here.
        .filter(|(_, m)| {
            slot.family != Family::Attacker || !m.card.id.starts_with("attacker/jammer/")
        })
        .map(|(r, m)| {
            let implemented = registry.has_implementation(r);
            let mut row = option_row(path, &m.card.id, &m.card.purpose);
            if !implemented && let Value::Object(map) = &mut row {
                map.insert("implemented".into(), json!(false));
            }
            (implemented, row)
        })
        .collect();
    // Implemented first, then by id (the registry's order), so the list opens on what runs.
    rows.sort_by_key(|(implemented, _)| !*implemented);
    Some((rows.into_iter().map(|(_, r)| r).collect(), false))
}

/// The RSU roles the engine reads (`phase2`, `wiring::rsu_services`).
const RSU_ROLES: [&str; 6] = [
    "report-forward",
    "crl",
    "provisioning-proxy",
    "spat",
    "map",
    "spat-map",
];

/// Closed sets that are not a `CHOICES` row, a reflected enum or a slot, with whether the
/// loader refuses anything outside them.
pub fn extra_values(path: &str) -> Option<(Vec<Value>, bool)> {
    match path {
        "apps.enabled[]" => Some((
            crate::scenario::Apps::ALL
                .iter()
                .map(|v| option_row(path, v, ""))
                .collect(),
            true,
        )),
        "actors.rsus[].roles[]" => Some((
            RSU_ROLES.iter().map(|v| option_row(path, v, "")).collect(),
            false,
        )),
        "metrics[]" => Some((
            vec![
                json!({"value": "all", "label": "Everything this build measures",
                        "description": "Every registered metric provider.",
                        "available": true}),
            ],
            false,
        )),
        "radio.channel" | "radio.adjacent_channel[].channel" => Some((channel_rows(), true)),
        "actors.rsu_placement.profile" => Some((
            validate::all_profile_ids()
                .into_iter()
                .filter(|id| id.starts_with("rsu/"))
                .map(|v| option_row(path, v, ""))
                .collect(),
            true,
        )),
        "actors.vehicles.classes.*.obu" => Some((
            validate::obu_profile_ids()
                .into_iter()
                .map(|v| option_row(path, v, ""))
                .collect(),
            true,
        )),
        _ => None,
    }
}

/// Every channel any region's band plan opens, from `v2xw_radio::regulation`, with which
/// region and technology it serves.
fn channel_rows() -> Vec<Value> {
    use std::collections::BTreeMap;
    use v2xw_radio::regulation::Region;
    let mut by_number: BTreeMap<u16, (f64, f64, Vec<String>)> = BTreeMap::new();
    for region in Region::ALL {
        for rule in region.rules() {
            let entry = by_number.entry(rule.channel.number).or_insert((
                rule.channel.lower_mhz,
                rule.channel.upper_mhz,
                Vec::new(),
            ));
            entry
                .2
                .push(format!("{}: {}", region.id(), first_sentence(rule.usage)));
        }
    }
    by_number
        .into_iter()
        .map(|(n, (lo, hi, uses))| {
            json!({
                "value": n,
                "label": format!("Channel {n} ({:.3} GHz, {:.0} MHz)", (lo + hi) / 2000.0, hi - lo),
                "description": uses.join("; "),
                "available": true,
            })
        })
        .collect()
}

/// What "Add" puts in a list whose elements are model choices: a complete element the
/// loader accepts as it stands, which the user then edits.
pub fn item_template(path: &str) -> Option<Value> {
    Some(match path {
        // One of fraction, count and actor_ids is required (validate::threats).
        "threats.attackers" => json!({"id": "threat/attacker/legacy/ConstPos", "fraction": 0.05}),
        "detection.local" => json!({"id": "detect/legacy-12"}),
        // Placed exactly one way (run::jamming::parse_emitter); 23 dBm is TR 36.885's
        // vehicle power. The position is Fifth Avenue at 42nd Street on the Midtown
        // extract, a busy junction to start from.
        "threats.jammers" => json!({"id": v2xw_radio::jamming::ConstantJammer::ID,
                                    "params": {"position_m": [770.3, 1052.8], "power_dbm": 23}}),
        "actors.rsus" => json!({"position_m": [770.3, 1052.8, 0.0],
                                "roles": ["report-forward", "crl", "provisioning-proxy"]}),
        _ => return None,
    })
}

/// One named value for a number.
#[derive(Debug, Clone, Copy)]
pub struct Preset {
    /// The dotted path.
    pub path: &'static str,
    /// What to call it.
    pub label: &'static str,
    /// The value.
    pub value: f64,
    /// Why this number: its source, or that it is this build's choice.
    pub note: &'static str,
}

/// Named values offered beside a number, each with its source.
///
/// The demand rates are network-wide arrival rates on the Midtown Manhattan extract. Their
/// *ratios* are cited: the HCM's design-hour factor puts the peak hour at about 9 % of a
/// day's traffic (HCM 6th ed., K-factor), FHWA's Traffic Monitoring Guide (2016, urban
/// hourly distribution) puts a weekday midday hour near 6 % and the small hours near 1 %.
/// Their *level* is scaled: NYC DOT's Midtown counts (avenues 970-1,890 veh/h per
/// approach) would need about forty times "Normal" on this extract
/// (`docs/design/BUILD-STATUS.md`, traffic calibration, open item 1), thousands of
/// vehicles at once, which this machine does not run interactively.
pub static PRESETS: &[Preset] = &[
    Preset {
        path: "radio.phy.data_rate_mbps",
        label: "3 Mbit/s (BPSK 1/2, longest range)",
        value: 3.0,
        note: "The lowest 10 MHz rate (IEEE 802.11-2020 Table 17-4); sensitivity -85 dBm by EN 302 663 Table 1.",
    },
    Preset {
        path: "radio.phy.data_rate_mbps",
        label: "6 Mbit/s (QPSK 1/2, J2945/1 default)",
        value: 6.0,
        note: "SAE J2945/1's vDataRate and the rate every published 802.11p PDR curve this engine is validated against.",
    },
    Preset {
        path: "radio.phy.data_rate_mbps",
        label: "12 Mbit/s (16-QAM 1/2)",
        value: 12.0,
        note: "Half the air time of 6 Mbit/s; studied as a congestion remedy (Jiang, Chen, Delgrossi 2008).",
    },
    Preset {
        path: "radio.phy.sensitivity_dbm",
        label: "Veins default (-89 dBm)",
        value: -89.0,
        note: "Veins' Decider80211p minPowerLevel default.",
    },
    Preset {
        path: "radio.phy.sensitivity_dbm",
        label: "ETSI static minimum at 6 Mbit/s (-88 dBm)",
        value: -88.0,
        note: "The EN 302 663 Table 1 figure the PHY's etsi-static table uses at QPSK 1/2.",
    },
    Preset {
        path: "radio.phy.cca_threshold_dbm",
        label: "ETSI (-85 dBm)",
        value: -85.0,
        note: "EN 302 571 §4.2.10.1, the CBR busy threshold.",
    },
    Preset {
        path: "radio.phy.cca_threshold_dbm",
        label: "ns-3 (-82 dBm)",
        value: -82.0,
        note: "ns-3 WifiPhy CcaSensitivity default.",
    },
    Preset {
        path: "radio.phy.cca_threshold_dbm",
        label: "Veins (-65 dBm)",
        value: -65.0,
        note: "Veins' Mac1609_4 ccaThreshold default.",
    },
    Preset {
        path: "radio.phy.noise_figure_db",
        label: "3GPP UE (9 dB)",
        value: 9.0,
        note: "3GPP TR 37.885 Table 6.1.2-1.",
    },
    Preset {
        path: "radio.channel_model.path_loss_exponent",
        label: "Free space (2.0)",
        value: 2.0,
        note: "Friis.",
    },
    Preset {
        path: "radio.channel_model.path_loss_exponent",
        label: "Urban street (2.75)",
        value: 2.75,
        note: "Cheng et al. 2007's suburban fits and the ns-3 LogDistance urban examples sit at 2.7-2.8.",
    },
    Preset {
        path: "radio.channel_model.shadowing_sigma_db",
        label: "Line of sight (4 dB)",
        value: 4.0,
        note: "Abbas et al. 2015, urban LOS sigma 4.15 dB.",
    },
    Preset {
        path: "radio.channel_model.shadowing_sigma_db",
        label: "Obstructed (6.7 dB)",
        value: 6.7,
        note: "Abbas et al. 2015, urban OLOS sigma 6.67 dB.",
    },
    Preset {
        path: "actors.vehicles.demand.rate_veh_per_h",
        label: "Night",
        value: 500.0,
        note: "About a sixth of midday: FHWA TMG urban hourly share near 1 % of daily against 6 % at midday.",
    },
    Preset {
        path: "actors.vehicles.demand.rate_veh_per_h",
        label: "Light",
        value: 1500.0,
        note: "Half of midday; the fleet the default Midtown scenario runs in about two minutes of wall time.",
    },
    Preset {
        path: "actors.vehicles.demand.rate_veh_per_h",
        label: "Normal (midday)",
        value: 3000.0,
        note: "This build's interactive midday level on the Midtown extract (scaled; see the note on rush hour).",
    },
    Preset {
        path: "actors.vehicles.demand.rate_veh_per_h",
        label: "Rush hour",
        value: 4500.0,
        note: "1.5 × midday: the HCM peak-hour factor (about 9 % of daily) over FHWA TMG's midday hour (about 6 %). Real Midtown volumes are about forty times this on the extract.",
    },
    Preset {
        path: "actors.vehicles.demand.rate_veh_per_h",
        label: "Gridlock (stress test)",
        value: 9000.0,
        note: "Twice rush hour: past the point where queues spill back through junctions. Slow to run.",
    },
    Preset {
        path: "time.duration_s",
        label: "1 minute",
        value: 60.0,
        note: "A quick look.",
    },
    Preset {
        path: "time.duration_s",
        label: "5 minutes",
        value: 300.0,
        note: "One SAE J2945/1 pseudonym period: long enough for certificate changes.",
    },
    Preset {
        path: "time.duration_s",
        label: "15 minutes",
        value: 900.0,
        note: "Long enough for revocation to reach every vehicle on a compressed calendar.",
    },
    Preset {
        path: "time.duration_s",
        label: "1 hour",
        value: 3600.0,
        note: "A full peak hour; a batch run, not an interactive one.",
    },
    Preset {
        path: "time.duration_s",
        label: "6 hours",
        value: 21_600.0,
        note: "A morning or an evening: a batch run (checkpointed; see docs/design/BUILD-STATUS.md, long runs).",
    },
    Preset {
        path: "time.duration_s",
        label: "1 day",
        value: 86_400.0,
        note: "A whole day: the SCMS revocation list is published daily (CAMP), so one day sees one CRL.",
    },
    Preset {
        path: "time.duration_s",
        label: "1 week",
        value: 604_800.0,
        note: "One SCMS certificate period (CAMP i_period, one week): pseudonym top-ups and expiry.",
    },
    Preset {
        path: "actors.rsu_placement.count",
        label: "None",
        value: 0.0,
        note: "Vehicles only: no infrastructure.",
    },
    Preset {
        path: "actors.rsu_placement.count",
        label: "A few (4)",
        value: 4.0,
        note: "This build's choice: a pilot corridor's handful of instrumented junctions.",
    },
    Preset {
        path: "actors.rsu_placement.count",
        label: "Many (16)",
        value: 16.0,
        note: "This build's choice: most signalised junctions of a downtown extract.",
    },
    Preset {
        path: "actors.vehicles.equipped_fraction",
        label: "Early deployment (5 %)",
        value: 0.05,
        note: "A pilot-scale share of the fleet; this build's choice for illustration.",
    },
    Preset {
        path: "actors.vehicles.equipped_fraction",
        label: "Half the fleet",
        value: 0.5,
        note: "NHTSA's proposed FMVSS 150 (2017) phase-in reached 50 % of new vehicles in year one.",
    },
    Preset {
        path: "actors.vehicles.equipped_fraction",
        label: "Every vehicle",
        value: 1.0,
        note: "Full penetration: the case V2X safety benefits are usually quoted for.",
    },
    Preset {
        path: "security.pseudonym_change.period_s",
        label: "5 minutes (SAE J2945/1)",
        value: 300.0,
        note: "SAE J2945/1's CERTCHG interval.",
    },
    Preset {
        path: "security.pseudonym_change.period_s",
        label: "1 minute",
        value: 60.0,
        note: "Aggressive: more privacy, more certificates and more verification of full certificates.",
    },
    Preset {
        path: "security.pseudonym_change.distance_m",
        label: "2 km (NYC pilot)",
        value: 2000.0,
        note: "The NYC Connected Vehicle Pilot's rule.",
    },
    Preset {
        path: "radio.devices.obu.tx_power_dbm",
        label: "20 dBm (J2945/1 cap)",
        value: 20.0,
        note: "SAE J2945/1's congestion control radiates at most 20 dBm on 802.11p.",
    },
    Preset {
        path: "radio.devices.obu.tx_power_dbm",
        label: "23 dBm (3GPP default)",
        value: 23.0,
        note: "3GPP TR 36.885's vehicle transmit power.",
    },
    Preset {
        path: "radio.devices.rsu.tx_power_dbm",
        label: "23 dBm (3GPP default)",
        value: 23.0,
        note: "3GPP TR 36.885's roadside-unit transmit power.",
    },
    Preset {
        path: "radio.devices.rsu.tx_power_dbm",
        label: "33 dBm (regulatory maximum)",
        value: 33.0,
        note: "The EIRP ceiling of 47 CFR 90.377 and ETSI EN 302 571 (before antenna gain).",
    },
    Preset {
        path: "actors.vru.pedestrians",
        label: "None",
        value: 0.0,
        note: "Vehicles only.",
    },
    Preset {
        path: "actors.vru.pedestrians",
        label: "Quiet street",
        value: 100.0,
        note: "This build's choice: enough to see crossings without a crowd.",
    },
    Preset {
        path: "actors.vru.pedestrians",
        label: "Busy Midtown",
        value: 400.0,
        note: "This build's choice for a Midtown afternoon at interactive cost; real sidewalks carry far more.",
    },
    Preset {
        path: "weather.intensity",
        label: "Light",
        value: 0.3,
        note: "On each weather model's own 0-1 scale.",
    },
    Preset {
        path: "weather.intensity",
        label: "Heavy",
        value: 0.8,
        note: "On each weather model's own 0-1 scale.",
    },
];

/// The presets for `path`, as `[{label, value, note}]`, or `None`.
pub fn presets(path: &str) -> Option<Value> {
    let rows: Vec<Value> = PRESETS
        .iter()
        .filter(|p| p.path == path)
        .map(|p| json!({"label": p.label, "value": p.value, "note": p.note}))
        .collect();
    (!rows.is_empty()).then(|| Value::Array(rows))
}

/// A plain-language title for a field whose name reads badly as one, or `None` to keep
/// the humanised name.
pub fn title(path: &str) -> Option<&'static str> {
    Some(match path {
        "actors.vehicles.demand.rate_veh_per_h" => "Vehicles entering per hour",
        "actors.vehicles.demand.kind" => "How vehicles arrive",
        "actors.vehicles.demand.fleet_size" => "Vehicles on the road (fixed fleet)",
        "traffic.signals.control" => "Signal control",
        "traffic.reroute_period_s" => "Reroute around congestion every",
        "traffic.gridlock_timeout_s" => "Gridlock timeout",
        "traffic.keep_junctions_clear" => "Keep junctions clear",
        "traffic.signals.passage_s" => "Passage time (gap-out)",
        "traffic.signals.green_wave_speed_mps" => "Green-wave speed",
        "actors.vehicles.equipped_fraction" => "Share of vehicles with a V2X radio",
        "actors.vru.device_fraction" => "Share of pedestrians and cyclists with a device",
        "actors.vru.crossing_gap_margin_s" => "Pedestrian gap acceptance margin",
        "actors.vru.walking_speed_mps" => "Pedestrian walking speed",
        "world.source.params.ew_green_share" => "East-west green share (generated grid)",
        "actors.vehicles.drivers" => "Drivers",
        "actors.vehicles.drivers.realism" => "Driver realism",
        "actors.vehicles.drivers.parameter_spread" => "How much drivers differ",
        "actors.vehicles.drivers.reaction_time_s" => "Reaction time",
        "actors.vehicles.drivers.perception_error" => "Misjudgement of gaps",
        "actors.vehicles.drivers.truck_lane_restriction" => "Trucks out of the left lane",
        "actors.vehicles.drivers.motorcycle_filtering" => "Motorcycles filter between cars",
        "radio.rat" => "Radio technology",
        "radio.region" => "Regulatory region",
        "radio.channel" => "Channel",
        "radio.channel_model.nakagami_shape_far" => "Fading severity far away (Nakagami m)",
        "radio.channel_model.nakagami_breakpoint_m" => "Fading breakpoint distance",
        "radio.channel_model.building_wall_loss_db" => "Loss per building wall",
        "radio.channel_model.building_loss_db_per_m" => "Loss per metre inside buildings",
        "radio.channel_model.vehicle_blockage" => "Vehicles block the signal",
        "radio.sidelink.candidate_percent" => "Candidate resources kept",
        "radio.sidelink.selection_window_ms" => "Selection window end (T2)",
        // network audit
        "radio.sidelink.noise_figure_db" => "Receiver noise figure (C-V2X)",
        "radio.sidelink.cbr_threshold_dbm" => "Busy threshold for the CBR (C-V2X)",
        "radio.dcc.reactive_table" => "Reactive state table (ETSI)",
        "radio.dcc.reactive_hold_ms" => "Reactive state hold time (ETSI)",
        "net.geonet.cbf_min_time_ms" => "Shortest forwarding wait (CBF)",
        "net.geonet.cbf_max_time_ms" => "Longest forwarding wait (CBF)",
        "net.geonet.max_comm_range_m" => "Assumed forwarding range (DIST_MAX)",
        "messages.spat_interval_ms" => "Signal phase (SPaT) interval",
        "messages.map_interval_ms" => "Intersection map (MAP) interval",
        "messages.cpm_interval_ms" => "Collective perception (CPM) check interval",
        "nodes.verify_queue_depth" => "Verification queue length",
        "nodes.peer_cert_cache" => "Peer certificates remembered",
        "nodes.hsm_sign_lower_us" => "Fastest HSM signing time",
        "security.on_demand_threshold" => "On-demand relevance threshold",
        "security.prioritized_horizon_m" => "Prioritized verification range",
        "messages.path_history_points" => "Path history points per message",
        "security.signer_id_policy.on_new_neighbour" => "Certificate for new neighbours",
        "radio.tiers.propagation" => "Signal propagation detail",
        "radio.tiers.phy" => "Physical layer detail",
        "radio.tiers.mac" => "Channel access detail",
        "nodes.default_obu" => "On-board unit hardware",
        "nodes.compute_tier" => "Compute detail",
        "nodes.backend_tier" => "Backend detail",
        "security.envelope" => "Message security format",
        "security.signature" => "Signature algorithm",
        "security.crypto_mode" => "Cryptography",
        "security.verification_policy" => "Which messages are verified",
        "security.pseudonym_change.strategy" => "When vehicles change pseudonym",
        "security.protocol.id" => "Credential system",
        // SCMS settings audit (docs/guides/config-inventory-scms.md)
        "security.scms.tls" => "Vehicle-to-RA TLS version",
        "security.scms.crl_overdue_tolerance_s" => "Overdue revocation list tolerance",
        "nodes.validated_cert_cache" => "Validated certificates remembered",
        "security.scms.lifecycle.i_period_s" => "Certificate period (i-period)",
        "security.scms.lifecycle.cert_lifetime_s" => "Pseudonym certificate lifetime",
        "security.scms.lifecycle.certs_per_period" => "Pseudonym certificates per week",
        "security.scms.lifecycle.pool_periods" => "Weeks of certificates held at the start",
        "security.scms.lifecycle.topup_below_periods" => "Top up when fewer weeks remain than",
        "security.scms.lifecycle.max_periods_ahead" => "RA provisioning horizon",
        "security.scms.lifecycle.cert_shuffle_threshold" => "RA request shuffle: batch size",
        "security.scms.lifecycle.cert_shuffle_window_s" => "RA request shuffle: longest wait",
        "security.scms.lifecycle.report_shuffle_threshold" => "RA report shuffle: batch size",
        "security.scms.lifecycle.report_shuffle_window_s" => "RA report shuffle: longest wait",
        "security.scms.lifecycle.first_batch_delay_s" => "First download after the request",
        "security.scms.lifecycle.download_poll_interval_s" => "Download retry wait (minWait)",
        "security.scms.lifecycle.max_download_polls" => {
            "Download retries before giving up (maxReqs)"
        }
        "security.scms.lifecycle.download_request_tolerance_s" => {
            "Download request clock tolerance"
        }
        "security.scms.lifecycle.enrolment_lifetime_s" => "Enrolment certificate lifetime",
        "security.scms.lifecycle.reenrol_lead_s" => "Re-enrol this long before expiry",
        "security.scms.lifecycle.reenrol_download_delay_s" => "Re-enrolment download wait",
        "security.scms.lifecycle.crl_cadence_s" => "Revocation list: fixed publication cadence",
        "security.scms.lifecycle.crl_fetch_interval_s" => {
            "Revocation list: cellular fetch interval"
        }
        "security.scms.lifecycle.crl_broadcast_interval_s" => {
            "Revocation list: roadside repeat interval"
        }
        "security.scms.lifecycle.ma_review_s" => "Misbehaviour Authority review time",
        "security.scms.lifecycle.ma_max_open_cases" => "Misbehaviour Authority open-case capacity",
        "actors.backend.protocol" => "Credential system (backend)",
        "messages.sets" => "Messages sent",
        "messages.codec_tier" => "Message encoding",
        "net.layer" => "Network layer",
        "net.geonet" => "Multi-hop hazard warnings",
        "net.channel_access" => "Channel switching",
        "radio.phy" => "Radio receiver and data rate",
        "radio.phy.data_rate_mbps" => "Data rate (MCS)",
        "radio.phy.noise_figure_db" => "Receiver noise figure",
        "radio.phy.sensitivity_dbm" => "Receiver sensitivity",
        "radio.phy.cca_threshold_dbm" => "Carrier-sense threshold",
        "radio.phy.capture_threshold_db" => "Capture threshold",
        "radio.mac" => "Channel access (EDCA)",
        "radio.mac.periodic_access_category" => "Priority of periodic messages",
        "radio.mac.event_access_category" => "Priority of hazard warnings",
        "radio.mac.cw_min_vo" => "Contention window, voice (CWmin)",
        "radio.mac.aifsn_vo" => "Arbitration spacing, voice (AIFSN)",
        "radio.mac.cw_min_vi" => "Contention window, video (CWmin)",
        "radio.mac.aifsn_vi" => "Arbitration spacing, video (AIFSN)",
        "radio.mac.queue_length" => "Queue length per category",
        "radio.channel_model" => "Channel model numbers",
        "actors.vehicles.drivers.max_accel_mps2" => "Comfortable acceleration (a)",
        "actors.vehicles.drivers.comfort_decel_mps2" => "Comfortable braking (b)",
        "actors.vehicles.drivers.time_headway_s" => "Time gap to the car ahead (T)",
        "actors.vehicles.drivers.min_gap_m" => "Gap when stopped (s0)",
        "actors.vehicles.drivers.politeness" => "Lane-change politeness (MOBIL p)",
        "actors.vehicles.drivers.lane_change_threshold_mps2" => "Lane-change threshold",
        "messages.rate_hz" => "Fixed message rate",
        "radio.sidelink" => "C-V2X scheduling",
        "radio.sidelink.reservation_interval_ms" => "Resource reservation interval",
        "radio.sidelink.resource_keep_probability" => "Probability of keeping a resource",
        "radio.sidelink.rsrp_threshold_dbm" => "Sensing exclusion threshold (RSRP)",
        "radio.sidelink.sensing" => "Sense before selecting",
        "radio.dcc" => "Congestion control",
        "radio.dcc.enabled" => "Congestion control on",
        "radio.dcc.cbr_target" => "Target channel load (ETSI)",
        "radio.dcc.adaptive_alpha" => "Load smoothing α (ETSI)",
        "radio.dcc.adaptive_beta" => "Control gain β (ETSI)",
        "radio.dcc.max_itt_ms" => "Longest message interval (J2945/1)",
        "radio.dcc.density_coefficient" => "Density threshold (J2945/1)",
        "radio.dcc.power_control" => "Power control (J2945/1)",
        "radio.dcc.min_power_dbm" => "Lowest controlled power (J2945/1)",
        "time.warmup_s" => "Warm-up before measuring",
        "radio.channel_model.path_loss_exponent" => "Path-loss exponent",
        "radio.channel_model.shadowing_sigma_db" => "Shadowing spread (sigma)",
        "radio.channel_model.shadowing_decorrelation_m" => "Shadowing decorrelation distance",
        "radio.channel_model.fading_coherence" => "Fading over time (Doppler)",
        "radio.channel_model.fading" => "Fading law",
        "radio.channel_model.nakagami_shape" => "Nakagami shape (m)",
        "radio.channel_model.rice_k_db" => "Rice factor (K)",
        "net.geonet.area_forwarding" => "Forwarding inside the area",
        "net.geonet.forwarding" => "How forwarded copies travel",
        "net.access_categories" => "Message priority queues",
        "net.dcc_cbr" => "Load the congestion control sees",
        "net.geonet.non_area_forwarding" => "Forwarding towards the area",
        "net.geonet.cbf_variant" => "When a contender stands down",
        "net.geonet.keep_alive_forwarding" => "Keep the warning alive",
        "world.highway_preset" => "Road rules and speed limits",
        "weather.initial" => "Weather",
        "time.duration_s" => "Run length",
        "time.mobility_step_ms" => "Mobility step",
        "time.des_resolution" => "Time resolution",
        "threats.attackers" => "Attackers",
        "threats.jammers" => "Jammers",
        "detection.local" => "Detectors on each vehicle",
        "detection.ma.id" => "Misbehaviour authority",
        "apps.enabled" => "Safety applications",
        // The fields of one element of a list, shown in the row-by-row editor.
        "threats.attackers[].id" => "Attack type",
        "threats.attackers[].fraction" => "Share of vehicles",
        "threats.attackers[].count" => "Number of vehicles",
        "threats.attackers[].schedule.from_s" => "Active from",
        "threats.attackers[].schedule.to_s" => "Active until",
        "threats.attackers[].params" => "Parameters (advanced)",
        "threats.jammers[].id" => "Jammer type",
        "threats.jammers[].params" => "Where and when (position_m, power_dbm, from_s, to_s)",
        "detection.local[].id" => "Detector suite",
        "detection.local[].params" => "Parameters (advanced)",
        "radio.adjacent_channel" => "Neighbouring-channel transmitters",
        "radio.adjacent_channel[].rat" => "Technology",
        "radio.adjacent_channel[].params" => "Where and when (position_m, power_dbm)",
        "actors.rsus[].profile" => "Hardware",
        "actors.rsus[].backhaul" => "Backhaul",
        "actors.rsus[].site" => "Site",
        "actors.rsu_placement.count" => "Number of roadside units",
        "actors.rsu_placement.profile" => "Roadside unit hardware",
        "actors.vehicles.classes" => "Fleet mix",
        "actors.vru.pedestrians" => "Pedestrians",
        "actors.vru.cyclists" => "Cyclists",
        "exporters" => "Files written after the run",
        "seed" => "Random seed",
        "meta.name" => "Run name",
        "city" => "City",
        // traffic audit
        "traffic.critical_gap_factor" => "Gap acceptance at unsignalised junctions (factor)",
        "traffic.permitted_left_gap_s" => "Left turn on green: gap accepted",
        "traffic.turn_lateral_accel_mps2" => "Cornering acceleration through junctions",
        "traffic.turn_curve_window_m" => "Corner radius judged over",
        "traffic.heavy_vehicle_gap_terms" => "Trucks and buses wait for longer gaps (HCM)",
        "traffic.lc_strategic" => "Lane change for the turn: eagerness (LC2013)",
        "traffic.lc_cooperative" => "Lane change to let others in (LC2013)",
        "traffic.lc_speed_gain" => "Lane change for speed: eagerness (LC2013)",
        "traffic.lc_keep_right" => "Keep right: eagerness (LC2013)",
        "traffic.kerbside" => "Kerbside stops",
        "traffic.kerbside.taxi_share" => "Share of cars that are taxis",
        "traffic.kerbside.delivery_stop_share" => "Share of vans making a delivery stop",
        "actors.vehicles.drivers.speed_factor" => "Free speed (multiple of the limit)",
        "actors.vehicles.drivers.speed_spread" => "Spread of free speeds",
        "actors.vehicles.drivers.red_runner_share" => "Share of drivers who run reds",
        "actors.vehicles.drivers.start_reaction_s" => "Reaction to a green light",
        "actors.vehicles.drivers.queue_start_delay_s" => "Queue start-up delay",
        "actors.vehicles.drivers.truck" => "Truck drivers",
        "actors.vehicles.drivers.bus" => "Bus drivers",
        "actors.vehicles.drivers.motorcycle" => "Motorcyclists",
        "actors.vehicles.drivers.bicycle" => "Cyclist riding",
        "world.signals.cycle_s" => "Signal cycle length",
        "world.signals.all_red_s" => "All-red between phases (minimum)",
        "world.signals.join_m" => "Join traffic lights closer than",
        "world.signals.pedestrian_walk_s" => "Pedestrian walk interval",
        "world.signals.pedestrian_speed_mps" => "Pedestrian clearance walking speed",
        "actors.vehicles.gnss" => "GNSS receivers",
        "actors.vehicles.gnss.rtk" => "RTK-corrected receivers",
        // end traffic audit
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    /// The published row for `path`.
    fn row(path: &str) -> Value {
        crate::scenario::publish::fields()
            .into_iter()
            .find(|f| f.get("x-path").and_then(Value::as_str) == Some(path))
            .unwrap_or_else(|| panic!("{path} is published"))
    }

    fn options(path: &str) -> Vec<(String, bool)> {
        row(path)["x-options"]
            .as_array()
            .unwrap_or_else(|| panic!("{path} publishes x-options"))
            .iter()
            .map(|o| {
                (
                    o["value"]
                        .as_str()
                        .map_or_else(|| o["value"].to_string(), str::to_string),
                    o["available"].as_bool().unwrap_or(true),
                )
            })
            .collect()
    }

    /// The fields the owner had to type are dropdowns, offering what the loader accepts.
    #[test]
    fn the_typed_fields_are_choices() {
        // A model slot with no `enum`: the loader's own demand table.
        let demand: Vec<String> = options("actors.vehicles.demand.kind")
            .into_iter()
            .map(|o| o.0)
            .collect();
        assert_eq!(
            demand,
            [
                "mobility/demand/poisson",
                "mobility/demand/tr36885-drop",
                "mobility/demand/none"
            ]
        );
        assert_eq!(row("actors.vehicles.demand.kind")["kind"], "enum");
        // Hardware profiles from the shipped table.
        assert!(
            options("nodes.default_obu")
                .iter()
                .any(|o| o.0 == "obu/unex-obu-301-craton2")
        );
        // Attack types: runnable ones offered, refused ones listed and marked unavailable.
        let attacks = options("threats.attackers[].id");
        assert!(attacks.contains(&("threat/attacker/legacy/ConstPos".to_string(), true)));
        assert!(attacks.contains(&("threat/attacker/legacy/Sybil".to_string(), true)));
        assert!(attacks.contains(&("threat/attacker/legacy/DoS".to_string(), false)));
        // A reflected enum keeps its values and gains words; 'hybrid' is refused by the loader.
        assert!(options("radio.rat").contains(&("hybrid".to_string(), false)));
        // Closed string sets inside a list.
        assert!(options("messages.sets[]").iter().any(|o| o.0 == "cam"));
        assert_eq!(
            options("apps.enabled[]").len(),
            crate::scenario::Apps::ALL.len()
        );
        // Every channel any region opens, as numbers.
        assert!(options("radio.channel").iter().any(|o| o.0 == "172"));
        // A number with named values.
        assert!(
            row("actors.vehicles.demand.rate_veh_per_h")["x-presets"]
                .as_array()
                .is_some_and(|p| p.len() >= 3)
        );
    }

    #[test]
    fn a_model_id_reads_as_words() {
        assert_eq!(
            super::humanise_id("threat/attacker/legacy/ConstPosOffset"),
            "Constant position offset"
        );
        assert_eq!(
            super::humanise_id("cellular/uu/cell-capacity-mm1"),
            "Cell capacity M/M/1"
        );
    }
}
