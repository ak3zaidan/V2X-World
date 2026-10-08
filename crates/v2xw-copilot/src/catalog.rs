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
    let words: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.' && c != '-')
        .filter(|w| w.len() > 1)
        .map(str::to_string)
        .collect();
    let mut scored: Vec<(usize, &Setting)> = settings()
        .iter()
        .map(|s| {
            let path = s.path.to_lowercase();
            let note = s.note.to_lowercase();
            let values = s.values.join(" ").to_lowercase();
            let score = words
                .iter()
                .map(|w| {
                    3 * usize::from(path.contains(w.as_str()))
                        + 2 * usize::from(values.contains(w.as_str()))
                        + usize::from(note.contains(w.as_str()))
                })
                .sum::<usize>();
            (score, s)
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    let rows: Vec<Value> = scored
        .iter()
        .take(limit.max(1))
        .map(|(_, s)| s.to_json())
        .collect();
    let mut out = json!({"settings": rows});
    let q = query.to_lowercase();
    if q.contains("attack") || q.contains("threat") || q.contains("misbehav") {
        out["attackers"] = json!({
            "id": "threat/attacker/legacy/<Kind>",
            "kinds": RENDERED_ATTACKERS.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            "refused_by_this_engine": REFUSED_ATTACKERS,
        });
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
            "Settings (dotted keys for `changes`; find_settings has the other ~160):\n\
             - time.duration_s (s); seed (keep it to compare like with like)\n\
             - actors.vehicles.demand.rate_veh_per_h (veh/h; {{\"multiply\": 2}} doubles it); \
             actors.vehicles.equipped_fraction (0-1, market penetration)\n\
             - actors.vru.pedestrians, actors.vru.cyclists (people; the vru-grid and \
             midtown-street-life scenarios have them)\n\
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
             Array items are addressed by index (threats.attackers.0.fraction).",
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
