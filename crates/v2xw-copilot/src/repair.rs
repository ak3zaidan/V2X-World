//! A change as a model writes it, made into a change the engine's loader reads.
//!
//! A model asked to "use C-V2X mode 4" or to set a "2 s" truck headway writes what the person
//! said: a value with its unit, a synonym of one of the allowed choices, a key without its
//! first segments. Before this module every such change went to the loader as written, the
//! loader refused it (`unknown variant`, `invalid type: string "2 s", expected f64`,
//! `unknown field`), and the model spent a round trip, or gave up and said it was done when
//! nothing had changed. [`repair`] reads the change against the build's own settings index
//! ([`crate::catalog::settings`], the same reflection of the schema the Studio's settings
//! window is built from) and:
//!
//! * **finds the key**: a JSON Pointer or dotted key, any case; a key that is the tail of
//!   exactly one published path (`drivers.truck.time_headway_s`, `shadowing_sigma_db`); for a
//!   vehicle/roadside/pedestrian device triple, the vehicle's (`tx_power_dbm` →
//!   `radio.devices.obu.tx_power_dbm`, the radio "the radio" means). Anything else is an
//!   error that names the closest published keys, never a guess.
//! * **reads the value** in the field's own terms: a choice by its exact id, any case, any
//!   punctuation, or by a name people use for it (`C-V2X mode 4`, `802.11p`, `5G NR`);
//!   `on`/`off`, `yes`/`no` for a switch; a number with its unit converted to the field's
//!   unit (`2 min` → 120 s, `200 ms` → 5 Hz for a rate, `30%` → 0.3 for a 0–1 share,
//!   `50 km/h` → m/s).
//! * **adds the companion** a setting needs to mean what was asked: a certificate on every
//!   message (`full_cert_every_ms: 0`) also turns `digest_otherwise` off, which the loader
//!   otherwise refuses as a contradiction.
//!
//! Every repair is written into the change's note, so the model and the person see that
//! "2 min" was read as 120 s. The loader still validates the result: this module never
//! decides a value is acceptable, only what the request meant.

use serde_json::{Value, json};

use crate::catalog::{self, Setting};

/// A change after repair.
#[derive(Debug, Clone, PartialEq)]
pub struct Repaired {
    /// The published key it lands on (dotted).
    pub key: String,
    /// The value in the field's own terms.
    pub value: Value,
    /// What was read differently from how it was written, one line each.
    pub notes: Vec<String>,
    /// Companion changes the request implies (`key`, `value`).
    pub companions: Vec<(String, Value)>,
}

/// Names people use for the radio technologies, as `(normalised name, choice)`.
const RAT_NAMES: &[(&str, &str)] = &[
    ("cv2x", "lte-v2x-pc5"),
    ("cv2xmode4", "lte-v2x-pc5"),
    ("mode4", "lte-v2x-pc5"),
    ("ltev2x", "lte-v2x-pc5"),
    ("lte", "lte-v2x-pc5"),
    ("ltepc5", "lte-v2x-pc5"),
    ("ltesidelink", "lte-v2x-pc5"),
    ("pc5", "lte-v2x-pc5"),
    ("5g", "nr-v2x-pc5"),
    ("5gnr", "nr-v2x-pc5"),
    ("5gv2x", "nr-v2x-pc5"),
    ("5gnrv2x", "nr-v2x-pc5"),
    ("nr", "nr-v2x-pc5"),
    ("nrv2x", "nr-v2x-pc5"),
    ("nrsidelink", "nr-v2x-pc5"),
    ("5gsidelink", "nr-v2x-pc5"),
    ("5gnrsidelink", "nr-v2x-pc5"),
    ("mode2", "nr-v2x-pc5"),
    ("dsrc", "dsrc-80211p"),
    ("80211p", "dsrc-80211p"),
    ("wave", "dsrc-80211p"),
    ("itsg5", "dsrc-80211p"),
    ("80211pdsrc", "dsrc-80211p"),
    ("dsrc80211p", "dsrc-80211p"),
];

/// Lower case, letters and digits only: `C-V2X Mode 4` → `cv2xmode4`.
fn squash(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Path segments with `[]` dropped, so `threats.attackers[].fraction` and
/// `threats.attackers.0.fraction` line up (an index segment matches the dropped one).
fn segments(path: &str) -> Vec<String> {
    path.replace("[]", "")
        .split('.')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn index(seg: &str) -> bool {
    !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit())
}

/// `key` segments against a published path's: equal, `*` (a map key), or an array index.
fn seg_match(k: &str, p: &str) -> bool {
    k == p || p == "*"
}

/// Whether `key` is `setting`'s path (`exact`), or lies under it or above it.
fn relation(key: &[String], path: &[String]) -> Option<std::cmp::Ordering> {
    let k: Vec<&String> = key.iter().filter(|s| !index(s)).collect();
    let n = k.len().min(path.len());
    if !k.iter().zip(path).take(n).all(|(a, b)| seg_match(a, b)) {
        return None;
    }
    Some(k.len().cmp(&path.len()))
}

/// The setting a key names exactly, if any.
#[must_use]
pub fn setting_for(key: &str) -> Option<&'static Setting> {
    let k = segments(key);
    catalog::settings()
        .iter()
        .find(|s| relation(&k, &segments(&s.path)) == Some(std::cmp::Ordering::Equal))
}

/// Whether the engine publishes `key`, a parent of a published key, or a key inside a
/// free-form (`json`) or list setting.
fn known(key: &str) -> bool {
    let k = segments(key);
    catalog::settings().iter().any(|s| {
        let p = segments(&s.path);
        match relation(&k, &p) {
            Some(std::cmp::Ordering::Equal | std::cmp::Ordering::Less) => true,
            Some(std::cmp::Ordering::Greater) => {
                s.kind == "json" || s.kind == "object" || s.kind == "array" || s.path.ends_with("[]")
            }
            None => false,
        }
    })
}

/// The published key `key` most plausibly means, with a note when it is not `key` itself.
fn resolve_key(raw: &str) -> Result<(String, Option<String>), String> {
    let trimmed = raw.trim();
    let mut key = trimmed
        .trim_start_matches('/')
        .replace('/', ".")
        .trim_start_matches("scenario.")
        .to_string();
    if key.is_empty() {
        return Err("a change has no key".to_string());
    }
    if !known(&key) && known(&key.to_ascii_lowercase()) {
        key = key.to_ascii_lowercase();
    }
    if known(&key) {
        let note = (key != trimmed && !trimmed.starts_with('/'))
            .then(|| format!("key `{trimmed}` read as `{key}`"));
        return Ok((key, note));
    }
    let lower = key.to_ascii_lowercase();
    let k = segments(&lower);
    // A tail of published paths: `drivers.truck.time_headway_s`, `tx_power_dbm`.
    let mut tails: Vec<String> = Vec::new();
    for s in catalog::settings() {
        let p = segments(&s.path);
        if p.len() <= k.len() {
            continue;
        }
        let head = p.len() - k.len();
        // The field's own name must be written out: a map's `*` stands for a key, never
        // for the field (`nodes.per_class.*` is not a tail match for `reaction_time_s`).
        if p.last() != k.last() {
            continue;
        }
        if k.iter().zip(&p[head..]).all(|(a, b)| seg_match(a, b)) {
            let mut full: Vec<String> = p[..head].to_vec();
            full.extend(k.iter().cloned());
            let full = full.join(".");
            if !tails.contains(&full) {
                tails.push(full);
            }
        }
    }
    if tails.len() > 1 {
        // The radio "the radio" means is the vehicle's.
        if let Some(obu) = tails.iter().find(|t| t.contains(".obu.")) {
            if tails
                .iter()
                .all(|t| t.contains(".obu.") || t.contains(".rsu.") || t.contains(".vru."))
            {
                tails = vec![obu.clone()];
            }
        }
        // The fleet-wide value rather than one class's.
        if tails.len() > 1 {
            let shortest = tails.iter().map(|t| t.split('.').count()).min().unwrap_or(0);
            let short: Vec<&String> = tails
                .iter()
                .filter(|t| t.split('.').count() == shortest)
                .collect();
            if short.len() == 1 {
                tails = vec![short[0].clone()];
            }
        }
    }
    match tails.len() {
        1 => {
            let full = tails.remove(0);
            Ok((full.clone(), Some(format!("key `{trimmed}` read as `{full}`"))))
        }
        0 => {
            let words = lower.replace(['.', '_'], " ");
            let near: Vec<String> = catalog::find_settings(&words, 5)["settings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r["path"].as_str().map(str::to_string))
                .collect();
            // Not in the index: it may still be a key inside a free-form block the index
            // does not reach, so the loader decides; the note says what is close.
            Ok((
                key.clone(),
                Some(format!(
                    "`{trimmed}` is not in the settings index{}",
                    if near.is_empty() {
                        String::new()
                    } else {
                        format!("; if the loader refuses it, the closest are {}", near.join(", "))
                    }
                )),
            ))
        }
        _ => Err(format!(
            "`{trimmed}` could mean {}; give the full key",
            tails.iter().take(6).cloned().collect::<Vec<_>>().join(", ")
        )),
    }
}

/// A number and the unit written after it: `-95 dBm`, `1,500 veh/h`, `30%`, `4k`.
fn number_and_unit(s: &str) -> Option<(f64, String)> {
    let t = s.trim();
    let mut end = 0;
    for (i, c) in t.char_indices() {
        if c.is_ascii_digit() || c == '.' || c == ',' || ((c == '-' || c == '+') && i == 0) {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let mut x: f64 = t[..end].replace(',', "").parse().ok()?;
    let mut unit = t[end..].trim().to_ascii_lowercase();
    if unit == "k" || unit.starts_with("k ") {
        x *= 1000.0;
        unit = unit[1..].trim().to_string();
    }
    Some((x, unit))
}

/// Seconds per unit of time, for the units people write.
fn seconds_per(unit: &str) -> Option<f64> {
    Some(match unit {
        "ms" | "msec" | "millisecond" | "milliseconds" => 1e-3,
        "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
        "min" | "mins" | "minute" | "minutes" => 60.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
        _ => return None,
    })
}

/// Whether a setting is a 0–1 share.
fn is_share(s: &Setting) -> bool {
    let p = s.path.to_ascii_lowercase();
    s.range_max == Some(1.0)
        || p.ends_with("fraction")
        || p.ends_with("_share")
        || p.ends_with("probability")
        || p.ends_with("_fraction")
}

fn read_number(s: &Setting, raw: &Value, notes: &mut Vec<String>) -> Value {
    let text = match raw {
        Value::String(t) => t.clone(),
        Value::Number(n) => {
            // A whole percentage given for a 0–1 share (30 for 30 %); 1.5 is left for the
            // loader to refuse, since it is no plausible percentage.
            if let Some(x) = n.as_f64() {
                if is_share(s) && x >= 2.0 && x <= 100.0 && x.fract() == 0.0 {
                    notes.push(format!("{x} read as {x} % = {}", x / 100.0));
                    return json!(x / 100.0);
                }
            }
            return raw.clone();
        }
        _ => return raw.clone(),
    };
    let Some((x, unit)) = number_and_unit(&text) else {
        return raw.clone();
    };
    let field_unit = s.unit.trim().to_ascii_lowercase();
    let mut y = x;
    if unit == "%" || unit == "percent" {
        if is_share(s) || field_unit.is_empty() || field_unit == "1" {
            y = x / 100.0;
        }
    } else if let (Some(have), Some(want)) = (seconds_per(&unit), seconds_per(&field_unit)) {
        y = x * have / want;
    } else if field_unit == "hz" {
        if let Some(have) = seconds_per(&unit) {
            // A period for a rate: 200 ms between messages is 5 Hz.
            if x > 0.0 {
                y = 1.0 / (x * have);
            }
        }
    } else if field_unit == "m/s" && (unit == "km/h" || unit == "kph" || unit == "kmh") {
        y = x / 3.6;
    } else if field_unit == "m/s" && unit == "mph" {
        y = x * 0.447_04;
    } else if field_unit == "m" && (unit == "km") {
        y = x * 1000.0;
    }
    let y = (y * 1e9).round() / 1e9;
    let out = if s.kind == "integer" && (y - y.round()).abs() < 1e-9 {
        json!(y.round() as i64)
    } else {
        json!(y)
    };
    notes.push(format!("\"{text}\" read as {out}{}", if field_unit.is_empty() { String::new() } else { format!(" {}", s.unit) }));
    out
}

fn read_bool(raw: &Value, notes: &mut Vec<String>) -> Value {
    let Value::String(t) = raw else {
        if let Some(n) = raw.as_f64() {
            return json!(n != 0.0);
        }
        return raw.clone();
    };
    let b = match squash(t).as_str() {
        "true" | "on" | "yes" | "enable" | "enabled" | "1" | "y" => true,
        "false" | "off" | "no" | "disable" | "disabled" | "0" | "n" | "none" => false,
        _ => return raw.clone(),
    };
    notes.push(format!("\"{t}\" read as {b}"));
    json!(b)
}

fn read_choice(s: &Setting, raw: &Value, notes: &mut Vec<String>) -> Value {
    let Value::String(t) = raw else {
        return raw.clone();
    };
    if s.values.iter().any(|v| v == t) {
        return raw.clone();
    }
    let q = squash(t);
    let pick = s
        .values
        .iter()
        .find(|v| squash(v) == q)
        .cloned()
        .or_else(|| {
            RAT_NAMES
                .iter()
                .find(|(name, choice)| *name == q && s.values.iter().any(|v| v == choice))
                .map(|(_, c)| (*c).to_string())
        })
        .or_else(|| {
            // Every word of the value in exactly one choice: `ML-DSA-44 hybrid`.
            let words: Vec<String> = t
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(str::to_ascii_lowercase)
                .collect();
            let hits: Vec<&String> = s
                .values
                .iter()
                .filter(|v| {
                    let sv = squash(v);
                    !words.is_empty() && words.iter().all(|w| sv.contains(w.as_str()))
                })
                .collect();
            match hits.len() {
                1 => Some(hits[0].clone()),
                // Several contain it: the shortest is the plain one (`rain`, not `rain-heavy`).
                n if n > 1 => hits.iter().min_by_key(|h| h.len()).map(|h| (*h).clone()),
                _ => None,
            }
        });
    match pick {
        Some(v) => {
            notes.push(format!("\"{t}\" read as `{v}`"));
            json!(v)
        }
        None => raw.clone(),
    }
}

/// The change a model wrote, as the engine reads it. See the module documentation.
///
/// # Errors
/// When the key is no published setting, or could mean several; the message names the
/// candidates.
pub fn repair(key: &str, value: &Value) -> Result<Repaired, String> {
    let (key, key_note) = resolve_key(key)?;
    let mut notes: Vec<String> = key_note.into_iter().collect();
    let op = value
        .as_object()
        .filter(|o| o.len() == 1)
        .and_then(|o| o.keys().next())
        .is_some_and(|k| matches!(k.as_str(), "multiply" | "add" | "append" | "remove"));
    let mut out = value.clone();
    if !op {
        if let Some(s) = setting_for(&key) {
            out = if !s.values.is_empty() {
                read_choice(s, value, &mut notes)
            } else {
                match s.kind.as_str() {
                    "number" | "integer" => read_number(s, value, &mut notes),
                    "boolean" => read_bool(value, &mut notes),
                    _ => value.clone(),
                }
            };
        }
    }
    let mut companions = Vec::new();
    if key == "security.signer_id_policy.full_cert_every_ms" && out.as_f64() == Some(0.0) {
        companions.push((
            "security.signer_id_policy.digest_otherwise".to_string(),
            json!(false),
        ));
        notes.push(
            "a certificate on every message also sets digest_otherwise to false".to_string(),
        );
    }
    Ok(Repaired {
        key,
        value: out,
        notes,
        companions,
    })
}

/// What a refused setting allows, in words: its choices, range, unit and recommended range.
#[must_use]
pub fn allowed(field: &str) -> Option<String> {
    let s = setting_for(field)?;
    let mut parts = Vec::new();
    if !s.values.is_empty() {
        parts.push(format!("choices: {}", s.values.join(", ")));
    }
    match (s.range_min, s.range_max) {
        (Some(a), Some(b)) => parts.push(format!("range {a} to {b}")),
        (Some(a), None) => parts.push(format!("at least {a}")),
        (None, Some(b)) => parts.push(format!("at most {b}")),
        _ => {}
    }
    if !s.unit.is_empty() {
        parts.push(format!("unit {}", s.unit));
    }
    if !s.recommended.is_empty() {
        parts.push(format!("recommended: {}", s.recommended));
    }
    (!parts.is_empty()).then(|| format!("`{}` takes {}", s.path, parts.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(key: &str, v: Value) -> Repaired {
        repair(key, &v).unwrap_or_else(|e| panic!("{key}: {e}"))
    }

    #[test]
    fn keys_values_and_companions_are_read_as_meant() {
        assert_eq!(r("radio.rat", json!("C-V2X mode 4")).value, json!("lte-v2x-pc5"));
        assert_eq!(r("/radio/rat", json!("DSRC")).value, json!("dsrc-80211p"));
        let t = r("drivers.truck.time_headway_s", json!("2 s"));
        assert_eq!(t.key, "actors.vehicles.drivers.truck.time_headway_s");
        assert_eq!(t.value, json!(2.0));
        assert_eq!(r("tx_power_dbm", json!(15)).key, "radio.devices.obu.tx_power_dbm");
        assert_eq!(r("messages.rate_hz", json!("200 ms")).value, json!(5.0));
        assert_eq!(
            r("actors.vehicles.equipped_fraction", json!("30%")).value,
            json!(0.3)
        );
        let c = r("security.signer_id_policy.full_cert_every_ms", json!(0));
        assert_eq!(c.companions.len(), 1);
        // An operation is passed through untouched.
        assert_eq!(
            r("actors.vehicles.demand.rate_veh_per_h", json!({"multiply": 2})).value,
            json!({"multiply": 2})
        );
    }

    #[test]
    fn an_unknown_key_is_never_guessed_and_names_the_closest_ones() {
        let u = r("radio.loudness", json!(3));
        assert_eq!(u.key, "radio.loudness", "left for the loader to refuse");
        assert!(u.notes[0].contains("not in the settings index"), "{:?}", u.notes);
        // A tail the fleet-wide value shares with each class's lands on the fleet-wide one.
        let e = repair("min_gap_m", &json!(3));
        assert!(
            e.as_ref().is_ok_and(|r| r.key == "actors.vehicles.drivers.min_gap_m"),
            "the fleet-wide value: {e:?}"
        );
    }
}
