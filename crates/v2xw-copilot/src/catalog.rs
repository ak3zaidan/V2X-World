//! What the agent can set, read once from this build's own tables and cached.
//!
//! The settings index is [`v2xw_engine::scenario::publish::fields`] — the same reflection of
//! the scenario schema the Studio's settings window is generated from — cut down to what a
//! model needs to write a change: the dotted path, the type, the allowed values, the unit,
//! the default, whether the engine acts on it and the one-line note. It is a property of
//! the build, so it is computed once per process ([`settings`]) and every tool result and
//! prompt built from it is byte-stable, which is what lets a provider's prompt cache hold.
//!
//! [`RENDERED_ATTACKERS`] and [`JAMMERS`] are the two lists the reflection cannot give: the
//! attacker catalogue is resolved by name inside the engine (`threat/attacker/legacy/<Kind>`)
//! and seven of its kinds are refused by name because the engine does not put them on the
//! air (the `threats.attackers` row of `validate::KEY_STATUS` names them). The eval suite
//! runs one of each family live.

use std::sync::OnceLock;

use serde_json::{Map, Value, json};

/// One editable setting, compact.
#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    /// The dotted path (`radio.rat`, `threats.attackers[].fraction`).
    pub path: String,
    /// `number`, `integer`, `string`, `enum`, `boolean` or `json`.
    pub kind: String,
    /// The allowed values, for a closed set.
    pub values: Vec<String>,
    /// The unit, when there is one.
    pub unit: String,
    /// The default, `null` when none.
    pub default: Value,
    /// `wired`, `partial`, `not-implemented`, `refused`, `descriptive`.
    pub status: String,
    /// What the engine does with it, one line.
    pub note: String,
    /// The plain-language label the settings window shows (`Radio technology`).
    pub title: String,
    /// `basic` or `advanced`, as the settings window files it.
    pub level: String,
    /// The recommended range, in words, when the engine publishes one.
    pub recommended: String,
    /// The loader's lower bound, when it has one (the published row's `minimum`).
    pub range_min: Option<f64>,
    /// The loader's upper bound (`maximum`).
    pub range_max: Option<f64>,
}

impl Setting {
    /// The row as a tool returns it: empty fields left out.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("path".into(), json!(self.path));
        m.insert("type".into(), json!(self.kind));
        if !self.values.is_empty() {
            m.insert("values".into(), json!(self.values));
        }
        if !self.unit.is_empty() {
            m.insert("unit".into(), json!(self.unit));
        }
        if !self.default.is_null() {
            m.insert("default".into(), self.default.clone());
        }
        if self.status != "wired" {
            m.insert("status".into(), json!(self.status));
        }
        if !self.title.is_empty() {
            m.insert("label".into(), json!(self.title));
        }
        if self.range_min.is_some() || self.range_max.is_some() {
            m.insert("range".into(), json!([self.range_min, self.range_max]));
        }
        if !self.recommended.is_empty() {
            m.insert("recommended".into(), json!(self.recommended));
        }
        if !self.note.is_empty() {
            m.insert("note".into(), json!(self.note));
        }
        Value::Object(m)
    }
}

/// The attacker kinds the engine renders on the air, as
/// `threat/attacker/legacy/<Kind>` names them, with the family each belongs to.
pub const RENDERED_ATTACKERS: [(&str, &str); 22] = [
    ("ConstPos", "position: freezes its claimed position"),
    ("ConstPosOffset", "position: a constant offset"),
    ("RandomPos", "position: a random position each message"),
    ("Teleport", "position: jumps"),
    ("SineWavePos", "position: oscillates"),
    ("ConstSpeedOffset", "speed: a constant offset"),
    ("RandomSpeed", "speed: random"),
    ("StopAndGo", "speed: claims to stop and go"),
    ("ReversedHeading", "heading: flipped 180°"),
    ("HeadingOffset", "heading: a constant offset"),
    (
        "DataReplay",
        "timing: replays another vehicle's past messages",
    ),
    ("DelayedMessages", "timing: sends its messages late"),
    ("OutOfOrder", "timing: out-of-order generation times"),
    ("SlowDrift", "stealth: a slowly growing position error"),
    ("AlongRoadOffset", "stealth: an offset along the road"),
    ("InvalidSignature", "credential: corrupted signature octets"),
    ("ExpiredCert", "credential: a certificate past its validity"),
    ("NotYetValid", "credential: a certificate not yet valid"),
    ("Disruptive", "combined: replays neighbours' data"),
    (
        "PosSpeedInconsistent",
        "combined: position and speed disagree",
    ),
    (
        "PosHeadingInconsistent",
        "combined: position and heading disagree",
    ),
    (
        "EventualStop",
        "combined: claims a stop that does not happen",
    ),
];

/// Attacker kinds in the catalogue the engine refuses by name.
pub const REFUSED_ATTACKERS: [&str; 7] = [
    "DoS",
    "DoSRandom",
    "Sybil",
    "VruImpersonation",
    "VruPositionSpoof",
    "FakeHazard",
    "SelectiveDrop",
];

/// The jammer models (`threats.jammers[].id`).
pub const JAMMERS: [(&str, &str); 3] = [
    ("attacker/jammer/constant", "always on"),
    (
        "attacker/jammer/pulsed",
        "on for `duty` of every `period_ms`",
    ),
    (
        "attacker/jammer/reactive",
        "fires when it hears a frame above `trigger_dbm`",
    ),
];

/// Words that say nothing about which setting is meant.
const STOP_WORDS: &[&str] = &[
    "the", "to", "a", "an", "make", "use", "set", "turn", "on", "off", "of", "more", "less",
    "please", "every", "with", "and", "for", "at", "in", "only", "have", "has", "than", "instead",
    "go", "back", "plain", "give", "so", "are", "is", "it", "its", "this", "that", "into", "after",
    "be", "can", "how", "what", "which", "my", "our", "all", "each", "do", "lower", "higher",
    "raise", "change", "switch", "slower", "faster", "allow", "add",
];

/// The words people use for a setting (each phrase is matched inside the lower-cased query
/// padded with spaces), and fragments of the paths or choices they mean. A fragment in a
/// path weighs more than a query word in one, so "make the radio weaker" finds the
/// transmit power before every other `radio.*` setting.
const SYNONYMS: &[(&[&str], &[&str])] = &[
    (
        &[
            "weak",
            "strong",
            "louder",
            "quieter",
            "transmit",
            "tx power",
            "power level",
        ],
        &["tx_power_dbm"],
    ),
    (
        &[
            "c-v2x",
            "cv2x",
            "mode 4",
            " lte",
            "5g",
            " nr ",
            " nr-",
            "802.11p",
            "dsrc",
            "its-g5",
            "radio technology",
            "access technology",
            " rat ",
            "wifi",
            "wi-fi",
        ],
        &["radio.rat"],
    ),
    (&["certificate", " cert"], &["full_cert_every_ms"]),
    (
        &["headway", "following distance", "time gap"],
        &["time_headway_s"],
    ),
    (&[" idm", "car-following", "car following"], &["drivers."]),
    (&["truck", "lorr", "hgv"], &[".truck."]),
    (&[" bus", "buses"], &[".bus."]),
    (&["motorcycl", "motorbike"], &[".motorcycle."]),
    (&["congestion control", " dcc"], &["radio.dcc.enabled"]),
    (
        &[
            "penetration",
            "equipped",
            "v2x radio",
            "have v2x",
            "of the cars",
            "of cars",
            "of vehicles",
            "market share",
        ],
        &["equipped_fraction"],
    ),
    (
        &[
            "quantum",
            "ml-dsa",
            "mldsa",
            "falcon",
            "dilithium",
            "signature",
            "sign with",
        ],
        &["security.signature"],
    ),
    (
        &[
            " rain", " snow", " fog", "sleet", " wind", "weather", "storm",
        ],
        &["weather.initial", "weather.intensity"],
    ),
    (
        &[
            "jaywalk",
            "against the red",
            "red light",
            "cross against",
            "on red",
        ],
        &["red_crossing_share"],
    ),
    (
        &["pedestrian", "walker", "people walking"],
        &["actors.vru."],
    ),
    (&[" walk"], &["walking_speed_mps"]),
    (
        &[
            "europ",
            " eu ",
            "frequency",
            "regulat",
            " etsi",
            " fcc",
            "channel plan",
        ],
        &["radio.region"],
    ),
    (&["sensitivity"], &["sensitivity_dbm"]),
    (&["pseudonym"], &["pseudonym_change.period_s"]),
    (
        &[
            " bsm",
            "beacon",
            "message rate",
            " hz",
            "messages per second",
        ],
        &["messages.rate_hz"],
    ),
    (&["building"], &["world.buildings.enabled"]),
    (
        &["fading", "nakagami", "rayleigh", "rician"],
        &["channel_model.fading"],
    ),
    (&["shadowing"], &["shadowing_sigma_db"]),
    (&["roadside", " rsu"], &["rsu_placement.count"]),
    (
        &["right turn on red", "right on red"],
        &["right_turn_on_red"],
    ),
    (&[" rtk", " gps", "gnss"], &["gnss.rtk"]),
    (&["react"], &["reaction_time_s"]),
    (
        &["green wave", "coordinat"],
        &["world.signals.coordinate", "green_wave"],
    ),
    (&["path loss", "path-loss"], &["path_loss_exponent"]),
    (&["gridlock", "stuck", "deadlock"], &["gridlock_timeout_s"]),
    (&["noise figure"], &["noise_figure_db"]),
    (&["antenna"], &["antenna_gain_dbi"]),
    (
        &[
            " long",
            "duration",
            " seconds ",
            " minutes ",
            "how long",
            "run length",
        ],
        &["time.duration_s"],
    ),
    (
        &[
            "traffic volume",
            "veh/h",
            "vehicles per hour",
            "cars per hour",
            "demand",
            "rush hour",
            "busier",
            "more cars",
            "more traffic",
            "less traffic",
        ],
        &["demand.rate_veh_per_h"],
    ),
    (
        &[
            "lane change",
            "changing lanes",
            "change lanes",
            "aggressive",
            "polite",
        ],
        &["politeness", "lane_change_threshold"],
    ),
    (&[" jam"], &["threats.jammers"]),
    (&["attack"], &["threats.attackers"]),
    (&["detector", "misbehavio", " mbd"], &["detection.local"]),
    (&["verif"], &["verification_policy"]),
];

/// Every editable setting of the scenario schema, built once.
pub fn settings() -> &'static [Setting] {
    static CELL: OnceLock<Vec<Setting>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut out: Vec<Setting> = Vec::new();
        for row in v2xw_engine::scenario::publish::fields() {
            let s = |k: &str| row.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let path = s("x-path");
            if path.is_empty() || out.iter().any(|r| r.path == path) {
                continue;
            }
            let mut note = s("x-status-note");
            if note.is_empty() {
                note = s("description");
            }
            out.push(Setting {
                path,
                kind: s("kind"),
                values: row
                    .get("enum")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                unit: s("unit"),
                default: row.get("default").cloned().unwrap_or(Value::Null),
                status: s("x-status"),
                note: shorten(&note, 220),
                title: s("title"),
                level: s("x-level"),
                recommended: shorten(&s("x-recommended"), 120),
                range_min: row.get("minimum").and_then(Value::as_f64),
                range_max: row.get("maximum").and_then(Value::as_f64),
            });
        }
        out
    })
}

/// `text` cut to about `n` characters at a word boundary.
#[must_use]
pub fn shorten(text: &str, n: usize) -> String {
    let t: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= n {
        return t;
    }
    let cut: String = t.chars().take(n).collect();
    match cut.rfind(' ') {
        Some(i) => format!("{}…", &cut[..i]),
        None => format!("{cut}…"),
    }
}

/// The settings that best match `query` (words, case-insensitive), at most `limit`, best
/// first. A word matching the path counts three times a word matching the note.
#[must_use]
pub fn find_settings(query: &str, limit: usize) -> Value {
    let q = format!(" {} ", query.to_lowercase());
    let words: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.' && c != '-')
        .map(|w| w.trim_matches('.'))
        .filter(|w| w.len() > 1 && !STOP_WORDS.contains(w))
        .map(|w| {
            // Plurals: `trucks` finds `truck`.
            if w.len() > 4 && w.ends_with('s') && !w.ends_with("ss") {
                w[..w.len() - 1].to_string()
            } else {
                w.to_string()
            }
        })
        .collect();
    // The words people use for a setting, as fragments of its path or of its choices.
    let terms: Vec<&str> = SYNONYMS
        .iter()
        .filter(|(phrases, _)| phrases.iter().any(|p| q.contains(p)))
        .flat_map(|(_, t)| t.iter().copied())
        .collect();
    let mut scored: Vec<(usize, &Setting)> = settings()
        .iter()
        .map(|s| {
            let path = s.path.to_lowercase();
            let note = format!("{} {}", s.title, s.note).to_lowercase();
            let values = s.values.join(" ").to_lowercase();
            let score = words
                .iter()
                .map(|w| {
                    3 * usize::from(path.contains(w.as_str()))
                        + 2 * usize::from(values.contains(w.as_str()))
                        + usize::from(note.contains(w.as_str()))
                })
                .sum::<usize>()
                + terms
                    .iter()
                    .map(|t| 4 * usize::from(path.contains(t)) + usize::from(values.contains(t)))
                    .sum::<usize>();
            (score, s)
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    // Ties: the settings window's basic page first, the vehicle's radio before the
    // roadside unit's and the pedestrian's, the fleet-wide value before one class's.
    let rank = |s: &Setting| {
        (
            usize::from(s.level != "basic"),
            usize::from(s.path.contains(".rsu.") || s.path.contains(".vru.")),
            s.path.matches('.').count(),
        )
    };
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| rank(a.1).cmp(&rank(b.1)))
            .then_with(|| a.1.path.cmp(&b.1.path))
    });
    let rows: Vec<Value> = scored
        .iter()
        .take(limit.max(1))
        .map(|(_, s)| s.to_json())
        .collect();
    let mut out = json!({"settings": rows});
    if q.contains("attack") || q.contains("threat") || q.contains("misbehav") {
        out["attackers"] = json!({
            "id": "threat/attacker/legacy/<Kind>",
            "kinds": RENDERED_ATTACKERS.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            "refused_by_this_engine": REFUSED_ATTACKERS,
        });
    }
    if q.contains("city") || q.contains("place") || q.contains("map") || q.contains("where") {
        out["cities"] = cities(query);
    }
    if q.contains("jam") {
        out["jammers"] = json!(
            JAMMERS
                .iter()
                .map(|(id, what)| json!({"id": id, "what": what}))
                .collect::<Vec<_>>()
        );
    }
    out
}

/// The plain-language label of a setting (`radio.rat` → `Radio technology`), from the
/// engine's published settings; a key it does not publish is humanised.
#[must_use]
pub fn label_of(key: &str) -> String {
    let path = key.trim_start_matches('/').replace('/', ".");
    let generic = path
        .split('.')
        .map(|p| {
            if p.chars().all(|c| c.is_ascii_digit()) {
                "[]"
            } else {
                p
            }
        })
        .collect::<Vec<_>>()
        .join(".")
        .replace(".[]", "[]");
    match generic.as_str() {
        "city" => return "City".into(),
        "threats.attackers" => return "Attackers".into(),
        "threats.jammers" => return "Jammers".into(),
        "detection.local" => return "Detectors".into(),
        "events" => return "Timeline".into(),
        "time.duration_s" => return "Duration".into(),
        "actors.vehicles.classes" => return "Vehicle mix".into(),
        _ => {}
    }
    if let Some(s) = settings()
        .iter()
        .find(|s| s.path == generic || s.path == path)
        .filter(|s| !s.title.is_empty())
    {
        return s.title.clone();
    }
    let last = generic.rsplit('.').next().unwrap_or(&generic);
    let words = last.replace("[]", "").replace('_', " ");
    let mut cs = words.chars();
    cs.next()
        .map(|f| f.to_uppercase().chain(cs).collect())
        .unwrap_or_default()
}

/// A city's short name for a chip or a header: `Portland, OR`, `Midtown Manhattan, NY`.
#[must_use]
pub fn city_short_name(id: &str) -> Option<String> {
    let c = v2xw_engine::city::find(id)?;
    let name = c
        .name
        .split_once(" (")
        .map(|(_, inner)| inner.trim_end_matches(')').to_string())
        .unwrap_or_else(|| c.name.clone());
    Some(if c.region == "europe" || c.state.is_empty() {
        name
    } else {
        format!("{name}, {}", c.state)
    })
}

/// The city library, matched against `query` (an id, a name or a state; an empty or
/// unmatched query lists them all): id, short name, the area, and whether its map is on
/// this machine. Read from the engine's own registry (`worlds/cities.yaml`), so the agent
/// never says a city is missing without having looked.
#[must_use]
pub fn cities(query: &str) -> Value {
    let reg = &v2xw_engine::city::registry().cities;
    let words: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| w.len() > 1 && !matches!(*w, "city" | "cities" | "the" | "on" | "in" | "map"))
        .map(str::to_string)
        .collect();
    let hit = |c: &v2xw_engine::city::City| {
        let hay = format!("{} {} {} {}", c.id, c.name, c.state, c.state_name).to_lowercase();
        words.iter().any(|w| hay.contains(w.as_str()))
    };
    let mut picked: Vec<&v2xw_engine::city::City> = reg.iter().filter(|c| hit(c)).collect();
    if picked.is_empty() {
        picked = reg.iter().collect();
    }
    json!({
        "how": "a change {\"key\": \"city\", \"value\": \"<id>\"}; any scenario runs on any city",
        "cities": picked.iter().map(|c| {
            let mut v = json!({"id": c.id,
                               "name": city_short_name(&c.id).unwrap_or_else(|| c.name.clone()),
                               "area": shorten(&c.description, 90)});
            if !c.extract_present() {
                v["map_missing"] = json!(format!("its map is not downloaded: worlds/fetch.sh {}", c.id));
            }
            v
        }).collect::<Vec<_>>(),
    })
}

/// Every city id with its short name, one line, for the agent's instructions.
pub fn cities_line() -> &'static str {
    static CELL: OnceLock<String> = OnceLock::new();
    CELL.get_or_init(|| {
        v2xw_engine::city::registry()
            .cities
            .iter()
            .map(|c| {
                format!(
                    "{} ({})",
                    c.id,
                    city_short_name(&c.id).unwrap_or_else(|| c.name.clone())
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// The quick reference the agent's instructions carry: the settings most requests touch,
/// with their allowed values read from the index, and how to write the compound ones.
/// Built once; byte-stable for the build.
pub fn quick_reference() -> &'static str {
    static CELL: OnceLock<String> = OnceLock::new();
    CELL.get_or_init(|| {
        let values = |path: &str| -> String {
            settings()
                .iter()
                .find(|s| s.path == path)
                .map(|s| s.values.join(" | "))
                .unwrap_or_default()
        };
        let attackers = RENDERED_ATTACKERS
            .iter()
            .map(|(k, _)| *k)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Settings (dotted keys for `changes`; find_settings searches all {n} by plain words or synonyms; values may carry units, e.g. \"2 min\", \"30%\"):\n\
             - city: a city id from the list below (top level; any scenario runs on any \
             city; it replaces the base's map)\n\
             - time.duration_s (s); seed (keep it to compare like with like)\n\
             - actors.vehicles.demand.rate_veh_per_h (veh/h; {{\"multiply\": 2}} doubles it; \
             busy downtown 2400-4500, light 500-1000) needs demand.kind \
             mobility/demand/poisson; actors.vehicles.equipped_fraction (0-1, market \
             penetration)\n\
             - actors.vehicles.classes: the fleet mix, a map of class to {{\"fraction\": x}} \
             whose fractions sum to 1, e.g. {{\"passenger\": {{\"fraction\": 0.8}}, \
             \"truck\": {{\"fraction\": 0.1}}, \"motorcycle\": {{\"fraction\": 0.1}}}}. \
             Classes: passenger, delivery, truck, trailer, bus, coach, motorcycle, moped, \
             emergency\n\
             - actors.vru.pedestrians, actors.vru.cyclists (people); \
             actors.vru.device_fraction (0-1 carrying a V2X device)\n\
             - actors.rsu_placement: {{\"count\": n}} roadside units at the busiest signals\n\
             - security.protocol.id: protocol/scms/camp (US SCMS) | protocol/etsi/ts102941 \
             (EU CCMS); a base with a backend (credential-lifecycle, ccms-lifecycle) carries \
             the entities it needs\n\
             - radio.rat: {rat}; radio.region: {region} (unset = the technology's own: \
             us-2016 for DSRC, us for LTE/NR); radio.devices.obu.tx_power_dbm\n\
             - weather.initial: {weather}; weather.intensity (0-1); weather.visibility_m\n\
             - security.signature: {sig} (the hybrids are post-quantum); \
             security.signer_id_policy.full_cert_every_ms (1000 J2945/1, 450 NYC pilot); \
             security.verification_policy: {verify}\n\
             - security.pseudonym_change.strategy: {pseud}; security.pseudonym_change.period_s\n\
             - threats.attackers: {{\"append\": {{\"id\": \"threat/attacker/legacy/<Kind>\", \
             \"fraction\": 0.05}}}} (or \"count\": n; optional \"schedule\": {{\"from_s\", \
             \"to_s\"}}). Kinds: {attackers}. Refused by this engine (say so, do not \
             substitute silently): {refused}.\n\
             - detection.local: {{\"append\": {{\"id\": \"detect/legacy-12\"}}}} turns on \
             the misbehaviour detectors (det_* metrics, time_to_detect).\n\
             - threats.jammers: {{\"append\": {{\"id\": \"attacker/jammer/constant\" | \
             \".../pulsed\" (params period_ms, duty) | \".../reactive\" (trigger_dbm), \
             \"params\": {{\"position_m\": \"centre\", \"power_dbm\": 23}}}}}} — \"centre\" is \
             the middle of a procedural grid; else [x, y] m or \"follow_node\": id.\n\
             - threats.eavesdropper: {{\"id\": \"threat/observer/passive-privacy\", \"params\": \
             {{\"sniffer_fraction\": 0.25, \"range_m\": 100}}}}\n\
             - events (timeline): {{\"append\": {{\"t\": 20, \"type\": \"weather.front\", \
             \"value\": \"fog\"}}}}; closure {{\"t\", \"until\", \"type\": \"closure\", \
             \"target\": \"edge:12\" (grid) or \"street:<name>\" (city)}}; \
             {{\"type\": \"safety.hard-brake\" | \"safety.breakdown\" | \"safety.cut-in\", \
             \"target\": \"auto\"}}; {{\"type\": \"demand.multiplier\", \"value\": 2, \"until\"}}; \
             {{\"type\": \"attack.wave\", \"ids\": [0], \"until\"}} (attacker population \
             indexes); {{\"type\": \"outage\", \"target\": \"<entity>\"}}; \
             {{\"type\": \"param.change\", \"path\", \"value\"}}\n\
             - apps.enabled: fcw, eebl, ima, lta, bsw, pcw, rlvw, glosa (all on by default)\n\
             Change values: a plain value sets the key; {{\"multiply\": k}} scales a number; \
             {{\"append\": item}} adds to a list; {{\"remove\": true}} deletes the key. \
             Array items are addressed by index (threats.attackers.0.fraction).\n\
             \n\
             Cities (`city` ids; every one is supported; list_cities says whether its map \
             is downloaded): {cities}",
            cities = cities_line(),
            n = settings().len(),
            rat = values("radio.rat"),
            region = values("radio.region"),
            weather = values("weather.initial"),
            sig = values("security.signature"),
            verify = values("security.verification_policy"),
            pseud = values("security.pseudonym_change.strategy"),
            refused = REFUSED_ATTACKERS.join(", "),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_has_the_settings_the_agent_writes_and_their_closed_sets() {
        let rat = settings()
            .iter()
            .find(|s| s.path == "radio.rat")
            .expect("radio.rat");
        assert!(rat.values.iter().any(|v| v == "lte-v2x-pc5"));
        assert!(
            settings()
                .iter()
                .any(|s| s.path == "actors.vehicles.demand.rate_veh_per_h")
        );
        let sig = settings()
            .iter()
            .find(|s| s.path == "security.signature")
            .expect("signature");
        assert!(
            sig.values.iter().any(|v| v.contains("mldsa")),
            "{:?}",
            sig.values
        );
    }

    #[test]
    fn a_search_ranks_the_path_match_first_and_adds_the_attacker_list_when_asked() {
        let r = find_settings("pseudonym period", 5);
        assert_eq!(
            r["settings"][0]["path"],
            "security.pseudonym_change.period_s"
        );
        let a = find_settings("attacker fraction", 5);
        assert!(
            a["attackers"]["kinds"]
                .as_array()
                .is_some_and(|k| k.len() == RENDERED_ATTACKERS.len())
        );
        assert!(
            find_settings("zzzz-nothing", 5)["settings"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
    }

    #[test]
    fn portland_is_in_the_agents_city_list_and_its_search() {
        assert!(
            quick_reference().contains("portland (Portland, OR)"),
            "the instructions name every city"
        );
        assert!(quick_reference().contains("manhattan (Midtown Manhattan, NY)"));
        let found = cities("Portland");
        assert!(
            found["cities"]
                .as_array()
                .is_some_and(|c| c.iter().any(|c| c["id"] == "portland")),
            "{found}"
        );
        assert!(find_settings("which city", 3)["cities"].is_object());
        assert_eq!(label_of("city"), "City");
        assert_eq!(label_of("threats.attackers"), "Attackers");
    }

    #[test]
    fn the_quick_reference_is_filled_from_the_index() {
        let q = quick_reference();
        assert!(q.contains("dsrc-80211p | lte-v2x-pc5"), "{q}");
        assert!(q.contains("hybrid-mldsa44-ecdsa-p256"));
        assert!(
            !q.contains("radio.rat: ;"),
            "an empty value list means a path moved"
        );
    }
}
