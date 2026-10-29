//! Scenario keys whose tables live in the engine module that implements them.
//!
//! The settings of docs/design/scms-real/08-settings-ux-and-gaps.md §5–§8 (server sites,
//! server capacity, the MA policy, deeper CA hierarchies) are built by two agents who do
//! not edit the scenario tables (§11). So each such key's status, range, title, unit, step
//! and menu sit beside its code, in [`crate::ma_policy`] and [`crate::scms_topology`], and
//! the tables here fall back to them: [`crate::scenario::validate::status_of`] and
//! [`crate::scenario::validate::bound_of`], [`crate::scenario::options::title`] and
//! `options::extra_values`, and `publish`'s units and steps. A key becomes live by its
//! module flipping its own `KEY_STATUS` row and refusal, not by an edit here.

use serde_json::{Value, json};

use crate::error::ScenarioError;
use crate::scenario::validate::{Bound, KeyStatus};

/// A key's title, unit and step, as the form shows them.
#[derive(Debug, Clone, Copy)]
pub struct KeyWords {
    /// The key's path, `[]` for a list element and `*` for a map key. A module whose
    /// keys repeat under several roots states the path relative to them.
    pub path: &'static str,
    /// A plain-English title.
    pub title: &'static str,
    /// The unit a number is in; empty for a text key.
    pub unit: &'static str,
    /// The step a number's control moves by; 0 for none stated.
    pub step: f64,
}

/// One value a menu offers.
#[derive(Debug, Clone, Copy)]
pub struct Opt {
    /// The value the document holds.
    pub value: &'static str,
    /// What a person reads.
    pub label: &'static str,
    /// One sentence on what choosing it does; for an unavailable value, why.
    pub description: &'static str,
    /// Whether this build loads it.
    pub available: bool,
}

/// `Opt` rows as the published `x-options`.
pub fn rows(opts: &[Opt]) -> Vec<Value> {
    opts.iter()
        .map(|o| {
            json!({"value": o.value, "label": o.label, "description": o.description,
                   "available": o.available})
        })
        .collect()
}

/// Every status row a module states, for [`crate::scenario::validate::status_of`].
pub fn key_status() -> impl Iterator<Item = &'static KeyStatus> {
    crate::ma_policy::KEY_STATUS
        .iter()
        .chain(crate::scms_topology::KEY_STATUS)
}

/// The range a module states for `path`.
pub fn bound(path: &str) -> Option<&'static Bound> {
    crate::ma_policy::bound(path).or_else(|| crate::scms_topology::bound(path))
}

/// The title, unit and step a module states for `path`.
pub fn words(path: &str) -> Option<&'static KeyWords> {
    crate::ma_policy::words(path).or_else(|| crate::scms_topology::words(path))
}

/// The menu a module states for `path`, and whether it is closed.
pub fn options(path: &str) -> Option<(Vec<Value>, bool)> {
    crate::ma_policy::options(path).or_else(|| crate::scms_topology::options(path))
}

/// Every path the modules' tables name, for the test that each is a published field.
pub fn table_paths() -> Vec<String> {
    let mut out = crate::ma_policy::table_paths();
    out.extend(crate::scms_topology::table_paths());
    out
}

/// One leaf of a module's settings as written: its concrete path, its template path (a
/// map key as `*`) and its value.
pub struct Leaf {
    /// `security.scms.deployment.providers.provider-a.capacity.ra.workers`.
    pub at: String,
    /// `security.scms.deployment.providers[].capacity.*.workers`.
    pub template: String,
    /// The value written.
    pub value: Value,
}

/// The leaves of `value`, a block serialised from the schema (absent fields are already
/// skipped). `maps` names the template paths whose children are map entries, and
/// `keys(template)` the closed key set of such a map, if it has one: a key outside it is
/// reported in `e`.
pub fn leaves(
    value: &Value,
    at: &str,
    template: &str,
    maps: &[&str],
    keys: &dyn Fn(&str) -> Option<&'static [&'static str]>,
    e: &mut Vec<ScenarioError>,
) -> Vec<Leaf> {
    let mut out = Vec::new();
    walk(value, at, template, maps, keys, e, &mut out);
    out
}

fn walk(
    value: &Value,
    at: &str,
    template: &str,
    maps: &[&str],
    keys: &dyn Fn(&str) -> Option<&'static [&'static str]>,
    e: &mut Vec<ScenarioError>,
    out: &mut Vec<Leaf>,
) {
    match value {
        Value::Null => {}
        Value::Object(map) => {
            let is_map = maps.contains(&template);
            let allowed = if is_map { keys(template) } else { None };
            for (k, v) in map {
                if let Some(allowed) = allowed
                    && !allowed.contains(&k.as_str())
                {
                    e.push(ScenarioError::conflict(
                        at,
                        format!(
                            "'{k}' is not a key it takes; allowed: {}",
                            allowed.join(", ")
                        ),
                    ));
                    continue;
                }
                let t = if is_map {
                    format!("{template}.*")
                } else {
                    format!("{template}.{k}")
                };
                walk(v, &format!("{at}.{k}"), &t, maps, keys, e, out);
            }
        }
        leaf => out.push(Leaf {
            at: at.to_string(),
            template: template.to_string(),
            value: leaf.clone(),
        }),
    }
}

/// The checks every module key gets before its module's own: a number inside its
/// range, a value of a closed menu inside it. `None` when the value passes both.
pub fn malformed(leaf: &Leaf) -> Option<ScenarioError> {
    if let Some(b) = bound(&leaf.template) {
        let Some(n) = leaf.value.as_f64() else {
            return Some(ScenarioError::conflict(
                &leaf.at,
                format!("is {}, and {} is a number", leaf.value, b.what),
            ));
        };
        if !b.admits(n) {
            return Some(ScenarioError::conflict(
                &leaf.at,
                format!(
                    "{} is {n}, which is outside the allowed range {}",
                    b.what,
                    b.describe()
                ),
            ));
        }
        return None;
    }
    if let Some((menu, true)) = options(&leaf.template)
        && let Some(v) = leaf.value.as_str()
        && !menu
            .iter()
            .any(|r| r.get("value").and_then(Value::as_str) == Some(v))
    {
        let names: Vec<&str> = menu
            .iter()
            .filter_map(|r| r.get("value").and_then(Value::as_str))
            .collect();
        return Some(ScenarioError::conflict(
            &leaf.at,
            format!(
                "'{v}' is not one of its values; allowed: {}",
                names.join(", ")
            ),
        ));
    }
    None
}

/// The refusal of a well-formed value whose behaviour is not built yet.
pub fn not_built(leaf: &Leaf, piece: &str, instead: &str) -> ScenarioError {
    ScenarioError::conflict(
        &leaf.at,
        format!("is {}: not built yet ({piece}); {instead}", leaf.value),
    )
}
