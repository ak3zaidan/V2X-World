//! The dataset release writer (docs/dataset/schema.md): one run as two observer views, the
//! local one (what each OBU saw) and the global one (what the Misbehavior Authority saw),
//! plus the answers in `truth/` and joined training copies in `ml/`.
//!
//! | Concern | Module |
//! |---|---|
//! | Schema column to recording field, the one mapping table | [`sources`] |
//! | `local/{ego,rx,checks,reports,pki}` | [`local`] |
//! | `global/{reports,decisions,crl}` | [`global`] |
//! | `truth/*` | [`truth`] |
//! | `ml/{local,global}` | [`ml`] |
//! | Attack name to family and VeReMi / NextGen / DARE codes | [`crosswalk`] |
//!
//! # Streaming
//!
//! The recording is read twice, record by record, and never held: the first pass keeps the
//! small channels (reports, decisions, CRL, PKI events, per-device ground truth), the
//! second streams transmissions and receptions through bounded time windows and writes row
//! batches as it goes. What stays in memory is per device, per certificate or per report,
//! plus a few seconds of in-flight receptions.
//!
//! # No clock, no RNG
//!
//! Nothing here reads a clock or draws a random number. The instrumented receivers are
//! the ones whose hash of the run seed and node id sorts first; `receiver_ref` and `rx_id`
//! are hashes too.

pub mod crosswalk;
pub mod global;
pub mod local;
pub mod ml;
pub mod sources;
pub mod truth;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Fields, Schema, SchemaRef};
use parquet::arrow::ArrowWriter;
use serde_json::{Map, Value, json};

use crate::dataset::leakage::{LeakageReport, LeakageViolation, ViolationKind};
use crate::error::{RecordError, Result};
use sources::{as_int, as_str, float, int, pick, string};

/// The schema version this writer produces.
pub const RELEASE_SCHEMA: &str = "v2x-scms-mbd/release/1";

// ---------------------------------------------------------------------------------------
// Column model
// ---------------------------------------------------------------------------------------

/// A column's type. Lists and structs are native Parquet types.
#[derive(Debug, Clone, Copy)]
pub enum Ty {
    /// `int64`.
    I64,
    /// `float64`.
    F64,
    /// `utf8`.
    Str,
    /// `bool`.
    Bool,
    /// `list<utf8>`.
    ListStr,
    /// `struct<..>`.
    Struct(&'static [Col]),
    /// `list<struct<..>>`.
    ListStruct(&'static [Col]),
}

/// One column.
#[derive(Debug, Clone, Copy)]
pub struct Col {
    /// Name, with its unit in it.
    pub name: &'static str,
    /// Type.
    pub ty: Ty,
    /// Unit, or empty.
    pub unit: &'static str,
    /// What it is.
    pub desc: &'static str,
}

/// A column, for the const tables.
#[must_use]
pub const fn col(name: &'static str, ty: Ty, unit: &'static str, desc: &'static str) -> Col {
    Col {
        name,
        ty,
        unit,
        desc,
    }
}

/// One table.
#[derive(Debug, Clone, Copy)]
pub struct TableDef {
    /// `local/rx`, ...
    pub name: &'static str,
    /// `NODE`, `MA`, `ORACLE` or `JOINED`.
    pub visibility: &'static str,
    /// One line on what a row is.
    pub doc: &'static str,
    /// Columns.
    pub cols: &'static [Col],
    /// Columns appended after `cols` (the `ml/` labels).
    pub extra: &'static [Col],
}

impl TableDef {
    /// Every column, in order.
    pub fn columns(&self) -> impl Iterator<Item = &'static Col> {
        self.cols.iter().chain(self.extra.iter())
    }

    /// The Arrow schema.
    #[must_use]
    pub fn schema(&self) -> SchemaRef {
        Arc::new(Schema::new(self.columns().map(field).collect::<Vec<_>>()))
    }

    /// `schema/<file>.json`'s name: `local/rx` is `local_rx`.
    #[must_use]
    pub fn file_stem(&self) -> String {
        self.name.replace('/', "_")
    }

    /// The table's `schema/<table>.json` document.
    #[must_use]
    pub fn schema_json(&self) -> Value {
        json!({
            "table": self.name,
            "visibility": self.visibility,
            "description": self.doc,
            "release_schema": RELEASE_SCHEMA,
            "columns": self.columns().map(col_json).collect::<Vec<_>>(),
        })
    }
}

fn col_json(c: &Col) -> Value {
    let mut m = json!({
        "name": c.name,
        "arrow_type": arrow_type(&c.ty).to_string(),
        "unit": c.unit,
        "description": c.desc,
    });
    match c.ty {
        Ty::Struct(cols) | Ty::ListStruct(cols) => {
            m["fields"] = Value::Array(cols.iter().map(col_json).collect());
        }
        _ => {}
    }
    m
}

fn field(c: &Col) -> Field {
    Field::new(c.name, arrow_type(&c.ty), true)
}

fn arrow_type(ty: &Ty) -> DataType {
    match ty {
        Ty::I64 => DataType::Int64,
        Ty::F64 => DataType::Float64,
        Ty::Str => DataType::Utf8,
        Ty::Bool => DataType::Boolean,
        Ty::ListStr => DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
        Ty::Struct(cols) => {
            DataType::Struct(Fields::from(cols.iter().map(field).collect::<Vec<_>>()))
        }
        Ty::ListStruct(cols) => DataType::List(Arc::new(Field::new(
            "item",
            DataType::Struct(Fields::from(cols.iter().map(field).collect::<Vec<_>>())),
            true,
        ))),
    }
}

/// Puts a JSON value into the shape `ty` declares, or null. A recorded field whose shape
/// differs from the declared column is nulled rather than allowed to fail the export.
fn coerce(ty: &Ty, v: &Value) -> Value {
    if v.is_null() {
        return Value::Null;
    }
    match ty {
        Ty::I64 => as_int(v).map_or(Value::Null, Value::from),
        Ty::F64 => v
            .as_f64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            .and_then(serde_json::Number::from_f64)
            .map_or(Value::Null, Value::Number),
        Ty::Str => match v {
            Value::Object(_) | Value::Array(_) => Value::String(v.to_string()),
            other => as_str(other).map_or(Value::Null, Value::String),
        },
        Ty::Bool => match v {
            Value::Bool(b) => Value::Bool(*b),
            Value::Number(n) => Value::Bool(n.as_f64().is_some_and(|f| f != 0.0)),
            _ => Value::Null,
        },
        Ty::ListStr => match v {
            Value::Array(a) => Value::Array(a.iter().map(|x| coerce(&Ty::Str, x)).collect()),
            Value::String(_) => Value::Array(vec![v.clone()]),
            Value::Object(m) => Value::Array(
                m.iter()
                    .map(|(k, x)| {
                        Value::String(format!(
                            "{k}={}",
                            as_str(x).unwrap_or_else(|| x.to_string())
                        ))
                    })
                    .collect(),
            ),
            _ => Value::Null,
        },
        Ty::Struct(cols) => match v {
            Value::Object(m) => {
                let mut out = Map::new();
                for c in *cols {
                    out.insert(
                        c.name.into(),
                        coerce(&c.ty, m.get(c.name).unwrap_or(&Value::Null)),
                    );
                }
                Value::Object(out)
            }
            _ => Value::Null,
        },
        Ty::ListStruct(cols) => match v {
            Value::Array(a) => {
                Value::Array(a.iter().map(|x| coerce(&Ty::Struct(cols), x)).collect())
            }
            _ => Value::Null,
        },
    }
}

// ---------------------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------------------

const BATCH_ROWS: usize = 4096;

/// One Parquet part being written, a row batch at a time.
struct PartWriter {
    def: &'static TableDef,
    rel: String,
    path: PathBuf,
    writer: ArrowWriter<File>,
    decoder: arrow::json::reader::Decoder,
    buf: Vec<Value>,
    rows: u64,
}

impl PartWriter {
    fn create(root: &Path, rel: String, def: &'static TableDef) -> Result<Self> {
        let path = root.join(&rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| RecordError::io(dir, e))?;
        }
        let file = File::create(&path).map_err(|e| RecordError::io(&path, e))?;
        let schema = def.schema();
        let writer = ArrowWriter::try_new(
            file,
            schema.clone(),
            Some(crate::export::parquet::writer_properties()?),
        )?;
        let decoder = arrow::json::ReaderBuilder::new(schema)
            .with_batch_size(BATCH_ROWS)
            .build_decoder()?;
        Ok(Self {
            def,
            rel,
            path,
            writer,
            decoder,
            buf: Vec::with_capacity(BATCH_ROWS),
            rows: 0,
        })
    }

    fn push(&mut self, row: &Map<String, Value>) -> Result<()> {
        let mut out = Map::new();
        for c in self.def.columns() {
            out.insert(
                c.name.into(),
                coerce(&c.ty, row.get(c.name).unwrap_or(&Value::Null)),
            );
        }
        self.buf.push(Value::Object(out));
        self.rows += 1;
        if self.buf.len() >= BATCH_ROWS {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.decoder.serialize(&self.buf)?;
        self.buf.clear();
        if let Some(batch) = self.decoder.flush()? {
            self.writer.write(&batch)?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<WrittenFile> {
        self.flush()?;
        self.writer.close()?;
        Ok(WrittenFile {
            rel: self.rel,
            table: Some(self.def.name.to_string()),
            rows: Some(self.rows),
            path: self.path,
        })
    }
}

/// One file the release wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenFile {
    /// Path relative to the release root.
    pub rel: String,
    /// Absolute path.
    pub path: PathBuf,
    /// The table it holds, for a Parquet part.
    pub table: Option<String>,
    /// Rows, for a Parquet part.
    pub rows: Option<u64>,
}

/// What [`write`] produced.
#[derive(Debug, Clone)]
pub struct WrittenRelease {
    /// The release root (`<out>/dataset`).
    pub root: PathBuf,
    /// Every file, `manifest.json` last.
    pub files: Vec<WrittenFile>,
    /// The leakage verdict over `local/` and `global/`.
    pub lint: LeakageReport,
    /// The instrumented receivers' count.
    pub instrumented: usize,
}

impl WrittenRelease {
    /// Rows written to `table`.
    #[must_use]
    pub fn rows(&self, table: &str) -> u64 {
        self.files
            .iter()
            .filter(|f| f.table.as_deref() == Some(table))
            .filter_map(|f| f.rows)
            .sum()
    }
}

// ---------------------------------------------------------------------------------------
// Options and run metadata
// ---------------------------------------------------------------------------------------

/// The splits a run may be assigned to.
pub const SPLITS: &[&str] = &["train", "validation", "test", "unassigned"];
/// The tiers.
pub const TIERS: &[&str] = &["mini", "standard", "full"];
/// The default number of instrumented receivers.
pub const DEFAULT_INSTRUMENTED: u32 = 25;

/// The `dataset` exporter's options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseOptions {
    /// Partition key; `None` derives it from the scenario name and hash.
    pub run_id: Option<String>,
    /// `train`, `validation`, `test` or `unassigned`.
    pub split: String,
    /// `mini`, `standard` or `full`.
    pub tier: String,
    /// How many OBUs get `local/rx` and `local/ego` rows; 0 means all.
    pub instrumented_receivers: u32,
    /// Attacks held out of `train`.
    pub heldout_attacks: Vec<String>,
    /// Raw signed bytes (full tier). Not built yet; the validator refuses `true`.
    pub wire: bool,
    /// Also run the VeReMi writer into `compat/veremi/<run_id>/`.
    pub compat_veremi: bool,
}

impl Default for ReleaseOptions {
    fn default() -> Self {
        Self {
            run_id: None,
            split: "unassigned".into(),
            tier: "standard".into(),
            instrumented_receivers: DEFAULT_INSTRUMENTED,
            heldout_attacks: Vec::new(),
            wire: false,
            compat_veremi: true,
        }
    }
}

impl ReleaseOptions {
    /// Parses `exporters[i].opts`. The error is `(field, why)`.
    ///
    /// # Errors
    /// A key this exporter does not have, or a value outside its menu.
    pub fn from_opts(opts: &Value) -> core::result::Result<Self, (String, String)> {
        let mut out = Self::default();
        let Some(map) = opts.as_object() else {
            if opts.is_null() {
                return Ok(out);
            }
            return Err(("opts".into(), "must be an object".into()));
        };
        let menu =
            |k: &str, v: &Value, list: &[&str]| -> core::result::Result<String, (String, String)> {
                v.as_str()
                    .filter(|s| list.contains(s))
                    .map(str::to_string)
                    .ok_or_else(|| {
                        (
                            format!("opts.{k}"),
                            format!("is {v}; one of {}", list.join(", ")),
                        )
                    })
            };
        for (k, v) in map {
            match k.as_str() {
                "run_id" => {
                    let s = v.as_str().unwrap_or_default();
                    if s.is_empty()
                        || s.len() > 64
                        || !s
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    {
                        return Err((
                            "opts.run_id".into(),
                            format!("is {v}; 1 to 64 characters of A-Z, a-z, 0-9, '-' and '_'"),
                        ));
                    }
                    out.run_id = Some(s.to_string());
                }
                "split" => out.split = menu(k, v, SPLITS)?,
                "tier" => out.tier = menu(k, v, TIERS)?,
                "instrumented_receivers" => {
                    out.instrumented_receivers = v
                        .as_u64()
                        .filter(|n| *n <= 100_000)
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| {
                            (
                                "opts.instrumented_receivers".to_string(),
                                format!("is {v}; a count of OBUs in [0, 100000], 0 for all"),
                            )
                        })?;
                }
                "heldout_attacks" => {
                    let a = v.as_array().ok_or_else(|| {
                        (
                            "opts.heldout_attacks".to_string(),
                            format!("is {v}; a list of attack names"),
                        )
                    })?;
                    out.heldout_attacks = a
                        .iter()
                        .map(|x| {
                            x.as_str()
                                .map(|s| crosswalk::short_name(s).to_string())
                                .ok_or_else(|| {
                                    (
                                        "opts.heldout_attacks".to_string(),
                                        format!("holds {x}; an attack name"),
                                    )
                                })
                        })
                        .collect::<core::result::Result<_, _>>()?;
                }
                "wire" => {
                    out.wire = v.as_bool().ok_or_else(|| {
                        ("opts.wire".to_string(), format!("is {v}; true or false"))
                    })?;
                }
                "compat_veremi" => {
                    out.compat_veremi = v.as_bool().ok_or_else(|| {
                        (
                            "opts.compat_veremi".to_string(),
                            format!("is {v}; true or false"),
                        )
                    })?;
                }
                other => {
                    return Err((
                        format!("opts.{other}"),
                        "is not a dataset option; run_id, split, tier, instrumented_receivers, \
                         heldout_attacks, wire, compat_veremi"
                            .into(),
                    ));
                }
            }
        }
        if out.wire {
            return Err((
                "opts.wire".into(),
                "is true; the wire/ tables (raw signed bytes) are not built yet, so a release \
                 cannot promise them"
                    .into(),
            ));
        }
        Ok(out)
    }
}

/// One `attack_mix` entry of `runs.parquet`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttackMix {
    /// Attack short name.
    pub id: String,
    /// Fraction of vehicles.
    pub fraction: Option<f64>,
    /// How many vehicles the scenario names for it (`actor_ids` or `count`), when it
    /// names them rather than a fraction.
    pub count: Option<i64>,
    /// Schedule start.
    pub from_s: Option<f64>,
    /// Schedule end.
    pub to_s: Option<f64>,
    /// The scenario's parameters for it, `name=value`, sorted by name.
    pub params: Vec<String>,
}

/// What the run was: `runs.parquet`'s row, filled by the engine from the scenario and the
/// recording's manifest.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunMeta {
    /// Resolved partition key.
    pub run_id: String,
    /// The run's master seed, as the manifest spells it. Every derived id hashes it.
    pub seed: String,
    /// Scenario name.
    pub name: Option<String>,
    /// Scenario hash.
    pub scenario_hash: Option<String>,
    /// Engine version.
    pub engine_version: Option<String>,
    /// Git commit.
    pub git_sha: Option<String>,
    /// City or world name.
    pub city: Option<String>,
    /// `procedural`, `osm`, ...
    pub world_source: Option<String>,
    /// `time.t0`.
    pub t0_utc: Option<String>,
    /// Duration.
    pub duration_s: Option<f64>,
    /// Origin latitude.
    pub origin_lat_deg: Option<f64>,
    /// Origin longitude.
    pub origin_lon_deg: Option<f64>,
    /// Fleet size.
    pub vehicles: Option<i64>,
    /// Equipped fraction.
    pub equipped_fraction: Option<f64>,
    /// Demand.
    pub demand_veh_per_h: Option<f64>,
    /// Radio access technology.
    pub rat: Option<String>,
    /// `in-loop`, `abstracted` or `off`.
    pub scms_mode: Option<String>,
    /// `real` or `compressed`.
    pub calendar: Option<String>,
    /// Local detector ids.
    pub local_suite: Option<String>,
    /// MA policy id.
    pub ma_policy: Option<String>,
    /// Sum of the attackers' fractions.
    pub attacker_fraction: Option<f64>,
    /// The attack mix.
    pub attack_mix: Vec<AttackMix>,
}

impl RunMeta {
    /// The default `run_id`: the scenario name as a slug plus the hash's first 8 characters.
    #[must_use]
    pub fn default_run_id(name: Option<&str>, scenario_hash: Option<&str>) -> String {
        let slug: String = name
            .unwrap_or("run")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let slug = slug
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let slug: String = slug.chars().take(40).collect();
        let hash: String = scenario_hash.unwrap_or("").chars().take(8).collect();
        if hash.is_empty() {
            slug
        } else {
            format!("{slug}-{hash}")
        }
    }
}

/// `runs.parquet`.
pub const RUNS: TableDef = TableDef {
    name: "runs",
    visibility: "PUBLIC",
    doc: "One row per run: the only place experiment settings appear.",
    cols: &[
        col("run_id", Ty::Str, "", "partition key everywhere"),
        col(
            "split",
            Ty::Str,
            "",
            "train, validation, test or unassigned",
        ),
        col("tier", Ty::Str, "", "mini, standard or full"),
        col(
            "heldout_attacks",
            Ty::ListStr,
            "",
            "attacks absent from train",
        ),
        col("scenario_name", Ty::Str, "", "scenario meta.name"),
        col("city", Ty::Str, "", "world name"),
        col("world_source", Ty::Str, "", "procedural, osm, ..."),
        col("t0_utc", Ty::Str, "", "UTC instant of t = 0"),
        col("duration_s", Ty::F64, "s", "simulated duration"),
        col("origin_lat_deg", Ty::F64, "deg", "local ENU origin"),
        col("origin_lon_deg", Ty::F64, "deg", "local ENU origin"),
        col("vehicles", Ty::I64, "", "fleet size"),
        col(
            "equipped_fraction",
            Ty::F64,
            "",
            "share of vehicles with an OBU",
        ),
        col("demand_veh_per_h", Ty::F64, "veh/h", "traffic demand"),
        col("rat", Ty::Str, "", "radio access technology"),
        col(
            "bsm_rate_hz",
            Ty::F64,
            "Hz",
            "observed BSM rate per transmitting device",
        ),
        col("scms_mode", Ty::Str, "", "in-loop, abstracted or off"),
        col("calendar", Ty::Str, "", "real or compressed"),
        col("local_suite", Ty::Str, "", "local detector ids"),
        col("ma_policy", Ty::Str, "", "MA policy id"),
        col(
            "attacker_fraction",
            Ty::F64,
            "",
            "share of vehicles that attack",
        ),
        col(
            "attack_mix",
            Ty::ListStruct(ATTACK_MIX),
            "",
            "attacks, fractions and schedules",
        ),
        col(
            "instrumented_receivers",
            Ty::I64,
            "",
            "OBUs with local/rx and local/ego rows",
        ),
        col("engine_version", Ty::Str, "", "engine version"),
        col("git_sha", Ty::Str, "", "engine commit"),
        col(
            "scenario_hash",
            Ty::Str,
            "",
            "SHA-256 of the resolved scenario",
        ),
        col("seed", Ty::Str, "", "master seed"),
        col("release_schema", Ty::Str, "", "this layout's version"),
    ],
    extra: &[],
};

const ATTACK_MIX: &[Col] = &[
    col("id", Ty::Str, "", "attack short name"),
    col("fraction", Ty::F64, "", "share of vehicles"),
    col(
        "count",
        Ty::I64,
        "",
        "vehicles named (actor_ids or count), instead of a fraction",
    ),
    col("from_s", Ty::F64, "s", "schedule start"),
    col("to_s", Ty::F64, "s", "schedule end"),
    col(
        "params",
        Ty::ListStr,
        "",
        "the scenario's parameters, name=value",
    ),
];

/// Every table the release writes.
#[must_use]
pub fn all_tables() -> Vec<&'static TableDef> {
    let mut v: Vec<&'static TableDef> = vec![&RUNS];
    v.extend(local::TABLES.iter().copied());
    v.extend(global::TABLES.iter().copied());
    v.extend(truth::TABLES.iter().copied());
    v.extend(ml::TABLES.iter().copied());
    v
}

// ---------------------------------------------------------------------------------------
// Sources of records
// ---------------------------------------------------------------------------------------

/// What [`RecordSource::scan`] calls for each record: `(channel, log_time_ns, json)`.
pub type RecordVisitor<'a> = dyn FnMut(&str, u64, &[u8]) -> Result<()> + 'a;

/// Something the writer can walk record by record, in emission order. Each call is one
/// pass over the whole recording.
pub trait RecordSource {
    /// Calls `f(channel, log_time_ns, json)` for every serde record.
    ///
    /// # Errors
    /// The reader's, or `f`'s.
    fn scan(&mut self, f: &mut RecordVisitor<'_>) -> Result<()>;
}

/// Records already in memory (tests, the fixture).
pub struct Records<'a>(
    /// The records, in emission order.
    pub &'a [crate::reader::RecordedRecord],
);

impl RecordSource for Records<'_> {
    fn scan(&mut self, f: &mut RecordVisitor<'_>) -> Result<()> {
        for r in self.0 {
            f(&r.channel, r.sim_time, &r.json)?;
        }
        Ok(())
    }
}

impl<S: crate::index::Source> RecordSource for crate::reader::Reader<S> {
    fn scan(&mut self, f: &mut RecordVisitor<'_>) -> Result<()> {
        self.for_each_message(|_| Ok(()), |r| f(&r.channel, r.sim_time, &r.json))
    }
}

// ---------------------------------------------------------------------------------------
// Identifiers (schema.md §5)
// ---------------------------------------------------------------------------------------

fn hex16(s: &str) -> String {
    v2xw_core::hash::sha256_hex(s.as_bytes())[..16].to_string()
}

/// The opaque id of an OBU's local rows.
#[must_use]
pub fn receiver_ref(seed: &str, node: i64) -> String {
    hex16(&format!("v2x-scms-mbd/receiver_ref|{seed}|{node}"))
}

/// The opaque id of one reception row.
#[must_use]
pub fn rx_id(seed: &str, rx: i64, msg: i64) -> String {
    hex16(&format!("v2x-scms-mbd/rx_id|{seed}|{rx}|{msg}"))
}

fn instrument_rank(seed: &str, node: i64) -> String {
    v2xw_core::hash::sha256_hex(format!("v2x-scms-mbd/instrumented|{seed}|{node}").as_bytes())
}

/// The instrumented OBUs: the `n` whose hash of seed and node id sorts first (all if 0).
#[must_use]
pub fn choose_instrumented(seed: &str, obus: &BTreeSet<i64>, n: u32) -> BTreeSet<i64> {
    if n == 0 || obus.len() <= n as usize {
        return obus.clone();
    }
    let mut ranked: Vec<(String, i64)> = obus
        .iter()
        .map(|&o| (instrument_rank(seed, o), o))
        .collect();
    ranked.sort();
    ranked
        .into_iter()
        .take(n as usize)
        .map(|(_, o)| o)
        .collect()
}

const RSU_NODE_BASE: i64 = 1 << 30;

// ---------------------------------------------------------------------------------------
// Pass 1: the small channels
// ---------------------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct Device {
    class: Option<String>,
    provider: Option<String>,
    gnss_class: Option<String>,
    faulty: bool,
    fault_kind: Option<String>,
    la_link: Option<String>,
    joined: Option<i64>,
    left: Option<i64>,
    attacker: bool,
    attack_start: Option<i64>,
    attack: Option<String>,
    /// The attack a vehicle attacker was scheduled to run (`gt.device.scheduled_attack`):
    /// its `attack_id` when it logged no falsification at all.
    scheduled: Option<String>,
    first_atk: Option<i64>,
    last_atk: Option<i64>,
}

impl Device {
    /// When the device's attack began: its first logged falsification
    /// (`gt.attack.action`) or the engine's first lying message
    /// (`gt.device_exposure.attack_start`), whichever is earlier. The latter alone can
    /// follow messages the attack already rewrote (a frozen position that has not yet
    /// drifted from the truth), which would put labelled-malicious messages before it.
    fn attack_began(&self) -> Option<i64> {
        match (self.first_atk, self.attack_start) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn label(&self) -> &'static str {
        if self.attacker || self.attack.is_some() {
            "malicious"
        } else if self.faulty {
            "faulty"
        } else {
            "benign"
        }
    }
}

#[derive(Default)]
struct Pass1 {
    devices: BTreeMap<i64, Device>,
    actor_node: BTreeMap<i64, i64>,
    cert_actor: BTreeMap<String, i64>,
    received: Vec<Value>,
    fates: Vec<Value>,
    decisions: Vec<Value>,
    crl_entries: Vec<Value>,
    revocations: Vec<Value>,
    pki_events: Vec<Value>,
    warnings: Vec<Value>,
    /// Each pseudonym certificate's issuer HashedId8 (hex), read from the certificate
    /// octets an OBU holds (`node.credentials`): what any receiver of a message signed
    /// with it reads from the certificate (1609.2 `IssuerIdentifier`).
    cert_issuer: BTreeMap<String, String>,
    /// The `spdu_digest` of every message some report names as evidence.
    evidence_digests: BTreeSet<String>,
    /// Each actor's falsifications (`gt.attack.action`), by actor id, until resolved.
    attack_log: BTreeMap<i64, Device>,
}

impl Pass1 {
    fn node_of_actor(&self, actor: i64) -> i64 {
        self.actor_node.get(&actor).copied().unwrap_or(actor)
    }

    fn take(&mut self, channel: &str, v: Value) {
        match channel {
            sources::DEVICE => {
                if let Some(n) = int("dev.node", &v) {
                    let d = self.devices.entry(n).or_default();
                    d.class = string("dev.class", &v).or(d.class.take());
                    d.provider = string("dev.provider", &v).or(d.provider.take());
                    d.gnss_class = string("dev.gnss_class", &v).or(d.gnss_class.take());
                    d.faulty |= pick("dev.faulty", &v).as_bool().unwrap_or(false);
                    d.attacker |= pick("dev.attacker", &v).as_bool().unwrap_or(false);
                    d.fault_kind = string("dev.fault_kind", &v).or(d.fault_kind.take());
                    if d.attack.is_none() {
                        d.attack = string("dev.attack_id", &v);
                    }
                    d.scheduled = string("dev.scheduled_attack", &v).or(d.scheduled.take());
                }
            }
            sources::EXPOSURE => {
                if let Some(n) = int("exp.node", &v) {
                    let d = self.devices.entry(n).or_default();
                    d.la_link = string("exp.la_link", &v).or(d.la_link.take());
                    d.joined = int("exp.joined_ns", &v).or(d.joined);
                    d.left = int("exp.left_ns", &v).or(d.left);
                    d.attacker |= pick("exp.attacker", &v).as_bool().unwrap_or(false);
                    d.attack_start = int("exp.attack_start_ns", &v).or(d.attack_start);
                }
            }
            sources::SUBJECT => {
                if let (Some(c), Some(a)) = (string("subj.cert_id", &v), int("subj.actor", &v)) {
                    self.cert_actor.entry(c).or_insert(a);
                }
            }
            sources::KINEMATICS => {
                if let (Some(a), Some(n)) = (int("kin.actor", &v), int("kin.node", &v)) {
                    self.actor_node.entry(a).or_insert(n);
                }
            }
            sources::ATTACK => {
                // Resolved to a node after the pass (`resolve_attacks`): an attacker that
                // joins mid-run can falsify before its first kinematics record names its
                // node, and its actor id is not a node id.
                if let Some(actor) = int("atk.actor", &v) {
                    let a = self.attack_log.entry(actor).or_default();
                    if a.attack.is_none() {
                        a.attack = string("atk.attacker", &v)
                            .map(|x| crosswalk::short_name(&x).to_string());
                    }
                    if let Some(t) = int("atk.t_ns", &v) {
                        a.first_atk = Some(a.first_atk.map_or(t, |f| f.min(t)));
                        a.last_atk = Some(a.last_atk.map_or(t, |l| l.max(t)));
                    }
                }
            }
            sources::MA_RECEIVED => self.received.push(v),
            // A build record is one more step of the report's life to the writer: it
            // carries the octets an upload still in the RA at the run's end has nowhere
            // else.
            sources::REPORT_FATE | sources::REPORT_BUILT => {
                for d in pick("fate.evidence_spdu_digests", &v)
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    self.evidence_digests.insert(d.to_string());
                }
                self.fates.push(v);
            }
            sources::CREDENTIALS => {
                if let Some(certs) = pick("cred.certificates", &v).as_object() {
                    for (id8, hex) in certs {
                        if let Some(issuer) = hex.as_str().and_then(cert_issuer_of) {
                            self.cert_issuer.entry(id8.clone()).or_insert(issuer);
                        }
                    }
                }
            }
            sources::DECISION => self.decisions.push(v),
            sources::CRL_ENTRY => self.crl_entries.push(v),
            sources::REVOCATION => {
                if string("rev.stage", &v).as_deref() == Some("issued") {
                    self.revocations.push(v);
                }
            }
            sources::PKI_EVENT => self.pki_events.push(v),
            sources::APP_WARNING => self.warnings.push(v),
            _ => {}
        }
    }

    /// Moves each actor's logged falsifications onto its node, once every actor-to-node
    /// record has been read.
    fn resolve_attacks(&mut self) {
        for (actor, a) in std::mem::take(&mut self.attack_log) {
            let node = self.node_of_actor(actor);
            let d = self.devices.entry(node).or_default();
            if d.attack.is_none() {
                d.attack = a.attack;
            }
            d.first_atk = match (d.first_atk, a.first_atk) {
                (Some(x), Some(y)) => Some(x.min(y)),
                (x, y) => x.or(y),
            };
            d.last_atk = match (d.last_atk, a.last_atk) {
                (Some(x), Some(y)) => Some(x.max(y)),
                (x, y) => x.or(y),
            };
        }
        // An attacker that left before its window opened logged nothing; it is still
        // malicious, and it still names the attack it was given.
        for d in self.devices.values_mut() {
            if d.attack.is_none() {
                d.attack = d.scheduled.take();
            }
        }
    }

    fn label_of(&self, node: Option<i64>) -> Option<&'static str> {
        node.map(|n| self.devices.get(&n).map_or("benign", Device::label))
    }

    fn is_obu(&self, node: i64) -> bool {
        match self.devices.get(&node).and_then(|d| d.class.as_deref()) {
            Some(c) => !c.eq_ignore_ascii_case("rsu"),
            None => node < RSU_NODE_BASE,
        }
    }
}

const PASS1: &[&str] = &[
    sources::DEVICE,
    sources::EXPOSURE,
    sources::SUBJECT,
    sources::KINEMATICS,
    sources::ATTACK,
    sources::MA_RECEIVED,
    sources::REPORT_FATE,
    sources::REPORT_BUILT,
    sources::DECISION,
    sources::CRL_ENTRY,
    sources::REVOCATION,
    sources::PKI_EVENT,
    sources::APP_WARNING,
    sources::CREDENTIALS,
];

/// The issuer HashedId8 (hex) of a COER 1609.2 `Certificate`: `version` 3, the
/// `CertificateType`, then the `IssuerIdentifier` choice, whose `sha256AndDigest` (0x80)
/// and `sha384AndDigest` (0x82) alternatives carry the issuer's HashedId8. `None` for a
/// self-signed certificate or octets that are not one.
fn cert_issuer_of(hex: &str) -> Option<String> {
    let b = hex.as_bytes();
    let byte = |i: usize| -> Option<u8> {
        let s = std::str::from_utf8(b.get(2 * i..2 * i + 2)?).ok()?;
        u8::from_str_radix(s, 16).ok()
    };
    if byte(1)? != 3 || !matches!(byte(3)?, 0x80 | 0x82) {
        return None;
    }
    let id = hex.get(8..24)?;
    id.bytes()
        .all(|c| c.is_ascii_hexdigit())
        .then(|| id.to_ascii_lowercase())
}

// ---------------------------------------------------------------------------------------
// Pass 2: transmissions and receptions, through bounded windows
// ---------------------------------------------------------------------------------------

/// How long a reception waits for its security record before it is written.
const RX_WINDOW_NS: u64 = 2_000_000_000;
/// How long a transmission waits for its truth and receptions, from its air record (or
/// its first record, while it has none). Longer than [`RX_WINDOW_NS`], so a reception is
/// always written while its transmission is held.
const TX_WINDOW_NS: u64 = 3_000_000_000;

/// How long a falsification (`gt.attack.action`) waits for its message's air record
/// (`node.tx`) once [`TX_WINDOW_NS`] has passed. A message can wait seconds between its
/// falsification and the air (a device without a usable certificate, a full queue: 4.6 s
/// for a NotYetValid attacker in the v1.0 sample), and written without it the
/// falsification became a second `truth/transmissions` row under the same `tx_id` and
/// the air row and its receptions were labelled with the sender's device label instead.
/// Only attackers' messages wait, so the set stays small.
const ATK_WAIT_NS: u64 = 60_000_000_000;

/// A VeReMi code as the truth tables write it: null for an attack the codebook does not
/// name ([`crate::dataset::veremi::UNNAMED_CODE`] is the trace files' sentinel, not a
/// code), else the code (0 for benign).
/// Two attack actions on one message (a flood's beacon is falsified and repeated, a
/// stacked attacker edits it twice): the message keeps the first action's attacker, time
/// and magnitude (its primary attack, the one `truth/devices.attack_id` names) and the
/// union of the fields every action changed, sorted. Before, the last action replaced the
/// first, and a DoSDisruptive beacon was labelled `[repetitions]` alone.
fn merge_actions(mut first: Value, next: &Value) -> Value {
    let mut fields: Vec<String> = [&first, next]
        .iter()
        .flat_map(|a| {
            pick("atk.fields", a)
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    fields.sort_unstable();
    fields.dedup();
    if let Some(o) = first.as_object_mut() {
        o.insert(
            "fields".into(),
            Value::Array(fields.into_iter().map(Value::from).collect()),
        );
    }
    first
}

fn veremi_code(code: i64) -> Value {
    if code == crate::dataset::veremi::UNNAMED_CODE {
        Value::Null
    } else {
        code.into()
    }
}

#[derive(Default)]
struct TxEntry {
    tx: Option<Value>,
    truth: Option<Value>,
    atk: Option<Value>,
    /// The record time of the air record (`node.tx`): the entry's window runs from it.
    air: Option<u64>,
}

#[derive(Clone, Copy, Default)]
struct Ego {
    x: Option<f64>,
    y: Option<f64>,
    speed: Option<f64>,
    heading: Option<f64>,
}

struct RxEntry {
    t_rx: i64,
    tx_node: Option<i64>,
    rssi: Value,
    dist: Value,
    cbr: Option<f64>,
    ego: Ego,
    sec: Option<Map<String, Value>>,
}

struct Writers {
    root: PathBuf,
    part: String,
    open: BTreeMap<&'static str, PartWriter>,
}

impl Writers {
    fn rel(&self, def: &TableDef) -> String {
        if def.name == "runs" {
            "runs.parquet".into()
        } else {
            format!("{}/{}/part-00000.parquet", def.name, self.part)
        }
    }

    fn push(&mut self, def: &'static TableDef, row: &Map<String, Value>) -> Result<()> {
        if !self.open.contains_key(def.name) {
            let rel = self.rel(def);
            self.open
                .insert(def.name, PartWriter::create(&self.root, rel, def)?);
        }
        self.open.get_mut(def.name).expect("opened above").push(row)
    }

    fn finish(mut self) -> Result<Vec<WrittenFile>> {
        // A table with no rows is still written, with its schema, so the layout is the
        // same for every run.
        for def in all_tables() {
            if !self.open.contains_key(def.name) {
                let rel = self.rel(def);
                self.open
                    .insert(def.name, PartWriter::create(&self.root, rel, def)?);
            }
        }
        let mut out = Vec::new();
        for (_, w) in std::mem::take(&mut self.open) {
            out.push(w.finish()?);
        }
        Ok(out)
    }
}

fn obj(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn opt<T: Into<Value>>(v: Option<T>) -> Value {
    v.map_or(Value::Null, Into::into)
}

fn verify_result(raw: Option<String>) -> Value {
    match raw.as_deref() {
        None => Value::Null,
        Some("valid" | "verified" | "ok") => "ok".into(),
        Some("skipped" | "not-checked" | "not_verified") => "not_checked".into(),
        Some(other) => other.replace('-', "_").into(),
    }
}

struct Pass2<'a> {
    p1: &'a Pass1,
    meta: &'a RunMeta,
    instrumented: &'a BTreeSet<i64>,
    tx: BTreeMap<i64, TxEntry>,
    tx_order: VecDeque<(u64, i64)>,
    rx: BTreeMap<(i64, i64), RxEntry>,
    rx_order: VecDeque<(u64, i64, i64)>,
    sec_wait: BTreeMap<(i64, i64), (i64, i64)>,
    sec_order: VecDeque<(u64, i64, i64)>,
    saw_rx_sec: bool,
    saw_any_sec: bool,
    cbr: BTreeMap<i64, f64>,
    ego: BTreeMap<i64, Ego>,
    node_cert: BTreeMap<i64, String>,
    cert_use: BTreeMap<String, (i64, i64, i64, i64)>,
    pseudonym_changes: Vec<(i64, i64, String)>,
    neighbours: BTreeMap<i64, BTreeSet<i64>>,
    last_mal_accepted: BTreeMap<i64, i64>,
    bsm_tx: u64,
    bsm_senders: BTreeSet<i64>,
    t_last: u64,
    /// `(t_rx_ns, rx_id)` of each `local/rx` row whose message some report names as
    /// evidence, by receiver and `spdu_digest`, in reception order: the reception a filed
    /// report's checks ran on. A replayed or flooded SPDU is received more than once.
    evidence_rx: BTreeMap<(i64, String), Vec<(i64, String)>>,
    /// Each non-benign sender's transmissions by certificate, `(t_ns, tx_id)` in time
    /// order: which message an application warning about that certificate followed.
    suspect_tx: BTreeMap<String, Vec<(i64, i64)>>,
    /// Each attacker's first falsified transmission time (`truth/transmissions.t_ns`).
    first_falsified: BTreeMap<i64, i64>,
    /// Each attacker's last falsified transmission time.
    last_falsified: BTreeMap<i64, i64>,
    /// Falsified messages past [`TX_WINDOW_NS`] still waiting for their air record
    /// ([`ATK_WAIT_NS`]), and the order they wait in.
    held: BTreeMap<i64, TxEntry>,
    held_order: VecDeque<(u64, i64)>,
}

impl Pass2<'_> {
    fn take(&mut self, w: &mut Writers, channel: &str, t: u64, v: Value) -> Result<()> {
        self.t_last = self.t_last.max(t);
        self.drain(w, t, false)?;
        match channel {
            sources::ATTACK => {
                if let Some(m) = int("atk.msg", &v) {
                    let e = self.tx_entry(t, m);
                    e.atk = Some(match e.atk.take() {
                        Some(first) => merge_actions(first, &v),
                        None => v,
                    });
                }
            }
            sources::TX_TRUTH => {
                if let Some(m) = int("txt.msg", &v) {
                    self.tx_entry(t, m).truth = Some(v);
                }
            }
            sources::TX => self.on_tx(w, t, v)?,
            sources::PHY_RX => self.on_rx(t, &v),
            sources::RX_SEC => {
                self.saw_rx_sec = true;
                self.on_sec("sec", &v);
            }
            sources::VERIFY => {
                if !self.saw_rx_sec {
                    self.on_sec("verify", &v);
                }
            }
            sources::CBR => {
                if let (Some(n), Some(c)) = (int("cbr.node", &v), float("cbr.cbr", &v)) {
                    self.cbr.insert(n, c);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn tx_entry(&mut self, t: u64, msg: i64) -> &mut TxEntry {
        if !self.tx.contains_key(&msg) {
            // A held falsification takes its air record (and what follows it) back into
            // the window.
            if let Some(e) = self.held.remove(&msg) {
                self.tx.insert(msg, e);
            }
            self.tx_order.push_back((t, msg));
        }
        self.tx.entry(msg).or_default()
    }

    fn on_tx(&mut self, w: &mut Writers, t: u64, v: Value) -> Result<()> {
        let Some(msg) = int("tx.msg", &v) else {
            return Ok(());
        };
        let node = int("tx.node", &v);
        let t_tx = int("tx.t_ns", &v).unwrap_or(t as i64);
        let cert = string("tx.cert_id", &v);
        let msg_type = string("tx.msg_type", &v);
        if let Some(n) = node {
            if msg_type.as_deref().is_none_or(|m| m == "bsm") {
                self.bsm_tx += 1;
                self.bsm_senders.insert(n);
            }
            if let Some(c) = &cert
                && self.p1.label_of(Some(n)).is_some_and(|l| l != "benign")
            {
                self.suspect_tx
                    .entry(c.clone())
                    .or_default()
                    .push((t_tx, msg));
            }
            if let Some(c) = &cert {
                let e = self.cert_use.entry(c.clone()).or_insert((n, t_tx, t_tx, 0));
                e.1 = e.1.min(t_tx);
                e.2 = e.2.max(t_tx);
                e.3 += 1;
                if self.node_cert.get(&n) != Some(c) {
                    if self.node_cert.contains_key(&n) {
                        self.pseudonym_changes.push((n, t_tx, c.clone()));
                    }
                    self.node_cert.insert(n, c.clone());
                }
            }
            // `local/ego` is the device's own state as its own state messages carried it,
            // one row per message. Its other frames state no position or speed of their
            // own: a misbehaviour report a car without a cellular modem sends over the air
            // to a roadside unit went out at the same instant as one of its BSMs, and was
            // written as a second, empty ego row (and blanked the receiver context of what
            // it heard next).
            if self.instrumented.contains(&n)
                && msg_type.as_deref().is_none_or(states_own_kinematics)
            {
                let ego = Ego {
                    x: float("tx.x_m", &v),
                    y: float("tx.y_m", &v),
                    speed: float("tx.speed_mps", &v),
                    heading: float("tx.heading_deg", &v),
                };
                self.ego.insert(n, ego);
                let pa = pos_accuracy(&v);
                let acc = accel(&v);
                w.push(
                    &local::EGO,
                    &obj(vec![
                        ("run_id", self.meta.run_id.clone().into()),
                        ("receiver_ref", receiver_ref(&self.meta.seed, n).into()),
                        ("t_ns", t_tx.into()),
                        ("x_m", opt(ego.x)),
                        ("y_m", opt(ego.y)),
                        ("lat_deg", pick("tx.lat_deg", &v)),
                        ("lon_deg", pick("tx.lon_deg", &v)),
                        ("speed_mps", opt(ego.speed)),
                        ("heading_deg", opt(ego.heading)),
                        (
                            "accel_mps2",
                            acc.get("long_mps2").cloned().unwrap_or(Value::Null),
                        ),
                        (
                            "pos_accuracy_m",
                            pa.get("semi_major_m").cloned().unwrap_or(Value::Null),
                        ),
                        ("own_cert_id", opt(cert.clone())),
                    ]),
                )?;
            }
        }
        // A message whose falsification came before its air record (a queued message)
        // restarts its window at the air time, so its receptions, which wait
        // [`RX_WINDOW_NS`] from the air, are written while it is still held.
        let restart = self.tx.get(&msg).is_some_and(|e| e.air.is_none());
        let e = self.tx_entry(t, msg);
        e.tx = Some(v);
        e.air = Some(t);
        if restart {
            self.tx_order.push_back((t, msg));
        }
        Ok(())
    }

    fn on_rx(&mut self, t: u64, v: &Value) {
        if string("rx.outcome", v).as_deref() != Some("ok") {
            return;
        }
        let (Some(rx), Some(msg)) = (int("rx.rx", v), int("rx.msg", v)) else {
            return;
        };
        let t_rx = int("rx.t_ns", v).unwrap_or(t as i64);
        let tx_node = int("rx.tx", v);
        let sender_bad = self.p1.label_of(tx_node).is_some_and(|l| l != "benign");
        if sender_bad && let Some(s) = tx_node {
            self.neighbours.entry(s).or_default().insert(rx);
        }
        let falsified = self
            .tx
            .get(&msg)
            .or_else(|| self.held.get(&msg))
            .is_some_and(|e| e.atk.is_some());
        if falsified && let Some(s) = tx_node {
            self.sec_wait.insert((rx, msg), (t_rx, s));
            self.sec_order.push_back((t + RX_WINDOW_NS, rx, msg));
        }
        if self.instrumented.contains(&rx) {
            self.rx.insert(
                (rx, msg),
                RxEntry {
                    t_rx,
                    tx_node,
                    rssi: pick("rx.rssi_dbm", v),
                    dist: pick("rx.dist_m", v),
                    cbr: self.cbr.get(&rx).copied(),
                    ego: self.ego.get(&rx).copied().unwrap_or_default(),
                    sec: None,
                },
            );
            self.rx_order.push_back((t + RX_WINDOW_NS, rx, msg));
        }
    }

    fn on_sec(&mut self, kind: &str, v: &Value) {
        let k = |c: &str| format!("{kind}.{c}");
        let (Some(rx), Some(msg)) = (int(&k("rx"), v), int(&k("msg"), v)) else {
            return;
        };
        self.saw_any_sec = true;
        let result = verify_result(string(&k("verify_result"), v));
        if let Some((t_rx, sender)) = self.sec_wait.remove(&(rx, msg))
            && result == "ok"
        {
            let e = self.last_mal_accepted.entry(sender).or_insert(t_rx);
            *e = (*e).max(t_rx);
        }
        if let Some(e) = self.rx.get_mut(&(rx, msg)) {
            let mut m = Map::new();
            m.insert("verify_result".into(), result);
            m.insert("verify_latency_us".into(), pick(&k("verify_latency_us"), v));
            m.insert("cbr".into(), pick(&k("cbr"), v));
            if kind == "sec" {
                m.insert("spdu_digest".into(), pick("sec.spdu_digest", v));
                m.insert("cert_id".into(), pick("sec.cert_id", v));
            }
            e.sec = Some(m);
        }
    }

    /// Writes everything whose window has closed by `now` (or everything, at the end).
    fn drain(&mut self, w: &mut Writers, now: u64, all: bool) -> Result<()> {
        while let Some(&(due, rx, msg)) = self.rx_order.front() {
            if !all && due > now {
                break;
            }
            self.rx_order.pop_front();
            if let Some(e) = self.rx.remove(&(rx, msg)) {
                self.write_rx(w, rx, msg, &e)?;
            }
        }
        while let Some(&(due, rx, msg)) = self.sec_order.front() {
            if !all && due > now {
                break;
            }
            self.sec_order.pop_front();
            // No security record arrived: with no security channel at all the decode is
            // the acceptance; with one, a missing record is not an acceptance.
            if let Some((t_rx, sender)) = self.sec_wait.remove(&(rx, msg))
                && !self.saw_any_sec
            {
                let e = self.last_mal_accepted.entry(sender).or_insert(t_rx);
                *e = (*e).max(t_rx);
            }
        }
        while let Some(&(first, msg)) = self.tx_order.front() {
            if !all && first + TX_WINDOW_NS > now {
                break;
            }
            self.tx_order.pop_front();
            // Queued before its air record: it waits from the air time (a later entry).
            if !all
                && self
                    .tx
                    .get(&msg)
                    .and_then(|e| e.air)
                    .is_some_and(|a| a > first)
            {
                continue;
            }
            if let Some(e) = self.tx.remove(&msg) {
                if !all && e.tx.is_none() && e.atk.is_some() {
                    self.held.insert(msg, e);
                    self.held_order.push_back((first, msg));
                } else {
                    self.write_tx(w, msg, &e)?;
                }
            }
        }
        while let Some(&(first, msg)) = self.held_order.front() {
            if !all && first + ATK_WAIT_NS > now {
                break;
            }
            self.held_order.pop_front();
            // Gone when its air record came and took it back into the window.
            if let Some(e) = self.held.remove(&msg) {
                self.write_tx(w, msg, &e)?;
            }
        }
        Ok(())
    }

    fn labels_of(&self, msg: i64, sender: Option<i64>) -> (Value, Option<crosswalk::Crosswalk>) {
        let atk = self
            .tx
            .get(&msg)
            .or_else(|| self.held.get(&msg))
            .and_then(|e| e.atk.as_ref());
        if let Some(a) = atk {
            let name = string("atk.attacker", a).unwrap_or_else(|| "unknown".into());
            return ("malicious".into(), Some(crosswalk::of(&name)));
        }
        let label = match self.p1.label_of(sender) {
            Some("faulty") => "faulty",
            Some(_) => "benign",
            None => return (Value::Null, None),
        };
        (label.into(), None)
    }

    fn write_rx(&mut self, w: &mut Writers, rx: i64, msg: i64, e: &RxEntry) -> Result<()> {
        let seed = &self.meta.seed;
        let entry = self.tx.get(&msg);
        let tx = entry
            .and_then(|t| t.tx.as_ref())
            .cloned()
            .unwrap_or(Value::Null);
        let sec = e.sec.clone().unwrap_or_default();
        let sec_of = |k: &str| sec.get(k).cloned().filter(|v| !v.is_null());
        let x = float("tx.x_m", &tx);
        let y = float("tx.y_m", &tx);
        let range = match (x, y, e.ego.x, e.ego.y) {
            (Some(x), Some(y), Some(ex), Some(ey)) => {
                Some(((x - ex).powi(2) + (y - ey).powi(2)).sqrt())
            }
            _ => None,
        };
        let rid = rx_id(seed, rx, msg);
        let digest = sec_of("spdu_digest").unwrap_or_else(|| pick("tx.spdu_digest", &tx));
        let cert_id = sec_of("cert_id").unwrap_or_else(|| pick("tx.cert_id", &tx));
        if let Some(d) = digest.as_str()
            && self.p1.evidence_digests.contains(d)
        {
            self.evidence_rx
                .entry((rx, d.to_string()))
                .or_default()
                .push((e.t_rx, rid.clone()));
        }
        // The signer's issuer: the recording's field, else the certificate's own
        // `IssuerIdentifier` (a receiver holds the certificate, inline or cached).
        let issuer = match pick("tx.issuer_id", &tx) {
            Value::Null => opt(cert_id
                .as_str()
                .and_then(|c| self.p1.cert_issuer.get(c).cloned())),
            v => v,
        };
        let mut row = obj(vec![
            ("run_id", self.meta.run_id.clone().into()),
            ("receiver_ref", receiver_ref(seed, rx).into()),
            ("rx_id", rid.clone().into()),
            ("t_rx_ns", e.t_rx.into()),
            ("spdu_digest", digest),
            ("rssi_dbm", e.rssi.clone()),
            ("cbr", sec_of("cbr").unwrap_or_else(|| opt(e.cbr))),
            ("size_bytes", pick("tx.size_bytes", &tx)),
            ("psid", pick("tx.psid", &tx)),
            ("cert_id", cert_id),
            ("signer_kind", pick("tx.signer_kind", &tx)),
            ("issuer_id", issuer),
            (
                "verify_result",
                sec_of("verify_result").unwrap_or(Value::Null),
            ),
            (
                "verify_latency_us",
                sec_of("verify_latency_us").unwrap_or(Value::Null),
            ),
            ("generation_time_us", pick("tx.generation_time_us", &tx)),
            ("msg_type", pick("tx.msg_type", &tx)),
            ("msg_cnt", pick("tx.msg_cnt", &tx)),
            ("temp_id", pick("tx.temp_id", &tx)),
            ("sec_mark_ms", pick("tx.sec_mark_ms", &tx)),
            ("lat_deg", pick("tx.lat_deg", &tx)),
            ("lon_deg", pick("tx.lon_deg", &tx)),
            ("x_m", opt(x)),
            ("y_m", opt(y)),
            ("elevation_m", pick("tx.elevation_m", &tx)),
            ("pos_accuracy", pos_accuracy(&tx)),
            ("speed_mps", pick("tx.speed_mps", &tx)),
            ("heading_deg", pick("tx.heading_deg", &tx)),
            ("accel", accel(&tx)),
            ("brakes", pick("tx.brakes", &tx)),
            ("transmission", pick("tx.transmission", &tx)),
            ("length_m", pick("tx.length_m", &tx)),
            ("width_m", pick("tx.width_m", &tx)),
            ("event_flags", pick("tx.event_flags", &tx)),
            ("ego_x_m", opt(e.ego.x)),
            ("ego_y_m", opt(e.ego.y)),
            ("ego_speed_mps", opt(e.ego.speed)),
            ("ego_heading_deg", opt(e.ego.heading)),
            ("range_m", opt(range)),
        ]);
        w.push(&local::RX, &row)?;
        let sender = int("tx.node", &tx).or(e.tx_node);
        let (label, cw) = self.labels_of(msg, sender);
        let attack = cw.as_ref().map(|c| c.attack.clone());
        w.push(
            &truth::RECEPTIONS,
            &obj(vec![
                ("run_id", self.meta.run_id.clone().into()),
                ("rx_id", rid.into()),
                ("tx_id", msg.into()),
                ("rx_device_id", rx.into()),
                ("tx_device_id", opt(sender)),
                ("label", label.clone()),
                ("attack_id", opt(attack.clone())),
                ("true_distance_m", e.dist.clone()),
            ]),
        )?;
        row.insert("label".into(), label);
        row.insert("attack_id".into(), opt(attack));
        row.insert("family".into(), opt(cw.as_ref().map(|c| c.family)));
        row.insert(
            "veremi_ext_code".into(),
            veremi_code(cw.as_ref().map_or(0, |c| c.veremi_ext_code)),
        );
        w.push(&ml::LOCAL, &row)
    }

    fn write_tx(&mut self, w: &mut Writers, msg: i64, e: &TxEntry) -> Result<()> {
        let null = Value::Null;
        let tx = e.tx.as_ref().unwrap_or(&null);
        let tr = e.truth.as_ref().unwrap_or(&null);
        let atk = e.atk.as_ref();
        let node = int("tx.node", tx)
            .or_else(|| int("txt.node", tr))
            .or_else(|| {
                atk.and_then(|a| int("atk.actor", a))
                    .map(|a| self.p1.node_of_actor(a))
            });
        let device_label = self.p1.label_of(node);
        let cw = atk
            .map(|a| crosswalk::of(&string("atk.attacker", a).unwrap_or_else(|| "unknown".into())));
        let label = if atk.is_some() {
            Some("malicious")
        } else {
            device_label.map(|l| if l == "faulty" { "faulty" } else { "benign" })
        };
        let t_tx = int("tx.t_ns", tx).or_else(|| int("txt.t_ns", tr));
        if let (Some(n), Some(t), Some(_)) = (node, t_tx, atk) {
            let f = self.first_falsified.entry(n).or_insert(t);
            *f = (*f).min(t);
            let l = self.last_falsified.entry(n).or_insert(t);
            *l = (*l).max(t);
        }
        w.push(
            &truth::TRANSMISSIONS,
            &obj(vec![
                ("run_id", self.meta.run_id.clone().into()),
                ("tx_id", msg.into()),
                ("device_id", opt(node)),
                (
                    "t_ns",
                    int("tx.t_ns", tx).map_or_else(|| pick("txt.t_ns", tr), Value::from),
                ),
                ("spdu_digest", pick("tx.spdu_digest", tx)),
                ("cert_id", pick("tx.cert_id", tx)),
                ("label", opt(label)),
                ("device_label", opt(device_label)),
                ("attack_id", opt(cw.as_ref().map(|c| c.attack.clone()))),
                ("family", opt(cw.as_ref().map(|c| c.family))),
                (
                    "veremi_ext_code",
                    veremi_code(cw.as_ref().map_or(0, |c| c.veremi_ext_code)),
                ),
                (
                    "veremi1_code",
                    veremi_code(cw.as_ref().map_or(0, |c| c.veremi1_code)),
                ),
                (
                    "nextgen_name",
                    opt(cw.as_ref().and_then(|c| c.nextgen_name)),
                ),
                ("dare_name", opt(cw.as_ref().and_then(|c| c.dare_name))),
                (
                    "fields_falsified",
                    atk.map_or(Value::Null, |a| pick("atk.fields", a)),
                ),
                (
                    "magnitude",
                    atk.map_or(Value::Null, |a| pick("atk.magnitude", a)),
                ),
                ("x_m", pick("txt.x_m", tr)),
                ("y_m", pick("txt.y_m", tr)),
                ("speed_mps", pick("txt.speed_mps", tr)),
                ("heading_deg", pick("txt.heading_deg", tr)),
                ("accel_mps2", pick("txt.accel_mps2", tr)),
                ("gnss_error_m", pick("txt.gnss_error_m", tr)),
                ("fix_to_stamp_ms", pick("txt.fix_to_stamp_ms", tr)),
            ]),
        )
    }
}

const PASS2: &[&str] = &[
    sources::ATTACK,
    sources::TX_TRUTH,
    sources::TX,
    sources::PHY_RX,
    sources::RX_SEC,
    sources::VERIFY,
    sources::CBR,
];

// ---------------------------------------------------------------------------------------
// The writer
// ---------------------------------------------------------------------------------------

/// One report, from its store and upload records and the MA's decryption.
#[derive(Default)]
struct Report {
    reporter: Option<i64>,
    report_id: Option<String>,
    subject_cert: Option<String>,
    subject_node: Option<i64>,
    t_created: Option<i64>,
    t_uploaded: Option<i64>,
    fate: Option<&'static str>,
    weight: Option<f64>,
    size: Option<i64>,
    path: Option<String>,
    evidence: Value,
    received: Option<usize>,
    /// The reporter's own fired checks, from the report's creation.
    checks: Value,
    /// The reporter's detector score.
    score: Option<f64>,
    /// A forgery an attacker filed on purpose (`pki.report_fate.truth.forged`).
    forged: bool,
}

/// Writes one run's release under `root` (normally `<out>/dataset`).
///
/// # Errors
/// I/O, Arrow or Parquet errors; and [`RecordError::Inconsistent`] when the leakage
/// linter finds a truth column in `local/` or `global/` — the release is written, then
/// inspected, and refused rather than declared clean.
#[allow(clippy::too_many_lines)]
pub fn write(
    src: &mut dyn RecordSource,
    root: &Path,
    opts: &ReleaseOptions,
    meta: &RunMeta,
) -> Result<WrittenRelease> {
    std::fs::create_dir_all(root).map_err(|e| RecordError::io(root, e))?;
    let run_id = meta.run_id.clone();
    let seed = meta.seed.clone();
    let mut w = Writers {
        root: root.to_path_buf(),
        part: format!("split={}/run={run_id}", opts.split),
        open: BTreeMap::new(),
    };

    // ---- pass 1 ----
    let mut p1 = Pass1::default();
    src.scan(&mut |channel: &str, _t: u64, bytes: &[u8]| {
        if PASS1.contains(&channel) {
            let v: Value = serde_json::from_slice(bytes)
                .map_err(|e| RecordError::malformed("release record", format!("{channel}: {e}")))?;
            p1.take(channel, v);
        }
        Ok(())
    })?;
    p1.resolve_attacks();
    let obus: BTreeSet<i64> = p1
        .devices
        .keys()
        .copied()
        .filter(|&n| p1.is_obu(n))
        .collect();
    let instrumented = choose_instrumented(&seed, &obus, opts.instrumented_receivers);

    // ---- pass 2 ----
    let mut p2 = Pass2 {
        p1: &p1,
        meta,
        instrumented: &instrumented,
        tx: BTreeMap::new(),
        tx_order: VecDeque::new(),
        rx: BTreeMap::new(),
        rx_order: VecDeque::new(),
        sec_wait: BTreeMap::new(),
        sec_order: VecDeque::new(),
        saw_rx_sec: false,
        saw_any_sec: false,
        cbr: BTreeMap::new(),
        ego: BTreeMap::new(),
        node_cert: BTreeMap::new(),
        cert_use: BTreeMap::new(),
        pseudonym_changes: Vec::new(),
        neighbours: BTreeMap::new(),
        last_mal_accepted: BTreeMap::new(),
        bsm_tx: 0,
        bsm_senders: BTreeSet::new(),
        t_last: 0,
        evidence_rx: BTreeMap::new(),
        suspect_tx: BTreeMap::new(),
        first_falsified: BTreeMap::new(),
        last_falsified: BTreeMap::new(),
        held: BTreeMap::new(),
        held_order: VecDeque::new(),
    };
    src.scan(&mut |channel: &str, t: u64, bytes: &[u8]| {
        // Most reception attempts are losses, which no table keeps: skip them before
        // parsing. Only a record that names an outcome other than "ok" is skipped, so a
        // recording that spells the outcome differently is parsed, not dropped.
        let lost = channel == sources::PHY_RX
            && contains(bytes, br#""outcome":"#)
            && !contains(bytes, br#""outcome":"ok""#);
        if PASS2.contains(&channel) && !lost {
            let v: Value = serde_json::from_slice(bytes)
                .map_err(|e| RecordError::malformed("release record", format!("{channel}: {e}")))?;
            p2.take(&mut w, channel, t, v)?;
        }
        Ok(())
    })?;
    let end = p2.t_last;
    p2.drain(&mut w, end, true)?;
    let Pass2 {
        cert_use,
        pseudonym_changes,
        neighbours,
        last_mal_accepted,
        bsm_tx,
        bsm_senders,
        evidence_rx,
        suspect_tx,
        first_falsified,
        last_falsified,
        ..
    } = p2;
    // A device's attack window covers its falsifications as the attack model logged them
    // and the air times of its falsified messages: a message is falsified a few ms (or,
    // queued, seconds) before it is on the air, and an attack whose first falsified
    // values equal the truth logs its start after the first such message.
    let attack_window = |n: i64, d: &Device| -> (Option<i64>, Option<i64>) {
        let lo = match (d.attack_began(), first_falsified.get(&n).copied()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let hi = match (d.last_atk, last_falsified.get(&n).copied()) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        (lo, hi)
    };

    let cert_device = |c: &str| -> Option<i64> {
        cert_use
            .get(c)
            .map(|u| u.0)
            .or_else(|| p1.cert_actor.get(c).map(|&a| p1.node_of_actor(a)))
    };
    let link_device: BTreeMap<String, i64> = p1
        .devices
        .iter()
        .filter_map(|(n, d)| d.la_link.clone().map(|l| (l, *n)))
        .collect();

    // ---- reports: store, upload, MA ----
    let mut reports: BTreeMap<String, Report> = BTreeMap::new();
    for f in &p1.fates {
        let Some(id) = string("fate.internal_id", f) else {
            continue;
        };
        let r = reports.entry(id).or_default();
        r.reporter = r.reporter.or_else(|| int("fate.reporter", f));
        r.report_id = r.report_id.take().or_else(|| string("fate.report_id", f));
        r.subject_cert = r
            .subject_cert
            .take()
            .or_else(|| string("fate.subject_cert_id", f));
        r.subject_node = r.subject_node.or_else(|| int("fate.truth_subject_node", f));
        r.size = r.size.or_else(|| int("fate.size_bytes", f));
        r.path = r.path.take().or_else(|| string("fate.path", f));
        // The first non-empty evidence list (a fate written before the report was built
        // carries an empty one).
        if r.evidence.as_array().is_none_or(Vec::is_empty) {
            let e = pick("fate.evidence_spdu_digests", f);
            if !e.is_null() {
                r.evidence = e;
            }
        }
        if r.checks.is_null() {
            let c = pick("fate.checks", f);
            if c.as_array().is_some_and(|a| !a.is_empty()) {
                r.checks = c;
            }
        }
        r.score = r.score.or_else(|| float("fate.score", f));
        r.forged |= pick("fate.truth_forged", f).as_bool().unwrap_or(false);
        let t = int("fate.t_ns", f);
        match string("fate.fate", f).as_deref() {
            Some("stored" | "created") => {
                r.t_created = r.t_created.or(t);
                r.weight = r.weight.or_else(|| float("fate.weight", f));
                r.fate = r.fate.or(Some("pending_at_end"));
            }
            Some("uploaded" | "delivered") => {
                if r.t_uploaded.is_none() {
                    r.t_uploaded = t;
                    r.weight = float("fate.weight", f).or(r.weight);
                }
                r.fate = Some("uploaded");
            }
            // The engine's store fates (`pki.report_fate`): a quota eviction and a decay
            // below the store's floor both drop the report unsent. Nothing overrides an
            // upload; `held_at_end` and `left_with_device` stay `pending_at_end`.
            Some("evicted" | "evicted_quota" | "decayed_out") if r.fate != Some("uploaded") => {
                r.fate = Some("evicted");
            }
            Some("expired") if r.fate != Some("uploaded") => r.fate = Some("expired"),
            _ => {}
        }
    }
    for (i, m) in p1.received.iter().enumerate() {
        let Some(id) = string("mar.internal_id", m) else {
            continue;
        };
        let r = reports.entry(id).or_default();
        r.received.get_or_insert(i);
        r.report_id = r.report_id.take().or_else(|| string("mar.report_id", m));
        r.subject_cert = r
            .subject_cert
            .take()
            .or_else(|| string("mar.subject_cert_id", m));
    }
    for r in reports.values_mut() {
        if r.subject_node.is_none() {
            r.subject_node = r.subject_cert.as_deref().and_then(cert_device);
        }
    }
    let rref = |n: i64| receiver_ref(&seed, n);
    let run = || Value::from(run_id.clone());

    // local/reports and local/checks, in the order the reporters created them: a file is
    // its observer's log, and the map's key order would follow the engine's report ids
    // (which carry the reporter's node id).
    let mut by_created: Vec<(&String, &Report)> = reports.iter().collect();
    by_created.sort_by(|a, b| {
        (a.1.t_created.unwrap_or(i64::MAX), a.0).cmp(&(b.1.t_created.unwrap_or(i64::MAX), b.0))
    });
    for (_, r) in by_created {
        let Some(reporter) = r.reporter else { continue };
        let mar = r.received.map(|i| &p1.received[i]);
        // The reporter's own checks (known from creation); a recording without them falls
        // back to the names the MA decrypted.
        let subtypes = if r.checks.is_null() {
            mar.map_or(Value::Null, |m| pick("mar.subtype_names", m))
        } else {
            r.checks.clone()
        };
        w.push(
            &local::REPORTS,
            &obj(vec![
                ("run_id", run()),
                ("receiver_ref", rref(reporter).into()),
                ("report_id", opt(r.report_id.clone())),
                ("t_created_ns", opt(r.t_created)),
                ("subject_cert_id", opt(r.subject_cert.clone())),
                (
                    "mbr_type",
                    mar.map_or(Value::Null, |m| pick("mar.mbr_type", m)),
                ),
                ("subtypes", subtypes.clone()),
                ("evidence_spdu_digests", r.evidence.clone()),
                ("size_bytes", opt(r.size)),
                ("fate", opt(r.fate)),
                ("t_uploaded_ns", opt(r.t_uploaded)),
                ("path", opt(r.path.clone())),
                ("weight_at_upload", opt(r.weight)),
            ]),
        )?;
        let excess = match r.score {
            Some(x) => x.into(),
            None => mar.map_or(Value::Null, |m| pick("mar.excess", m)),
        };
        // The reception the checks ran on: the last evidence message the reporter itself
        // decoded and wrote to local/rx (an instrumented reporter), else null; of an SPDU
        // received more than once, its last reception by the time the report was created.
        let rx_of_check = r
            .evidence
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .filter_map(Value::as_str)
            .find_map(|d| {
                evidence_rx
                    .get(&(reporter, d.to_string()))?
                    .iter()
                    .rev()
                    .find(|(t, _)| r.t_created.is_none_or(|c| *t <= c))
                    .map(|(_, id)| id.clone())
            });
        for check in subtypes
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            w.push(
                &local::CHECKS,
                &obj(vec![
                    ("run_id", run()),
                    ("receiver_ref", rref(reporter).into()),
                    ("t_ns", opt(r.t_created)),
                    ("rx_id", opt(rx_of_check.clone())),
                    ("cert_id", opt(r.subject_cert.clone())),
                    ("suite", local::suite_of(check).into()),
                    ("check", check.into()),
                    ("fired", true.into()),
                    ("score", excess.clone()),
                    ("excess", excess.clone()),
                ]),
            )?;
        }
    }

    // local/pki
    if p1.pki_events.is_empty() {
        for (n, t, c) in &pseudonym_changes {
            if !p1.is_obu(*n) {
                continue;
            }
            w.push(
                &local::PKI,
                &obj(vec![
                    ("run_id", run()),
                    ("receiver_ref", rref(*n).into()),
                    ("t_ns", (*t).into()),
                    ("event", "pseudonym_change".into()),
                    ("cert_id", c.clone().into()),
                ]),
            )?;
        }
    } else {
        for e in &p1.pki_events {
            let Some(n) = int("pki.node", e) else {
                continue;
            };
            w.push(
                &local::PKI,
                &obj(vec![
                    ("run_id", run()),
                    ("receiver_ref", rref(n).into()),
                    ("t_ns", pick("pki.t_ns", e)),
                    ("event", pick("pki.event", e)),
                    ("cert_id", pick("pki.cert_id", e)),
                    ("crl_series", pick("pki.crl_series", e)),
                    ("crl_number", pick("pki.crl_number", e)),
                    ("crl_entries", pick("pki.crl_entries", e)),
                    ("bytes", pick("pki.bytes", e)),
                    ("latency_ms", pick("pki.latency_ms", e)),
                ]),
            )?;
        }
    }

    // global/reports and ml/global, in the order the MA received them (the recording logs
    // them in per-second batches)
    // The MA links a report's subject to a device only when a case asks the Linkage
    // Authorities: here, the decision whose case named the report. Before (and without)
    // one, `la_link` is null, as the MA cannot link pseudonyms on its own.
    let mut linked: BTreeMap<String, (Value, Value)> = BTreeMap::new();
    for d in &p1.decisions {
        for id in pick("dec.report_ids", d)
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            linked
                .entry(id.to_string())
                .or_insert_with(|| (pick("dec.la_link", d), pick("dec.t_ns", d)));
        }
    }
    let mut by_received: Vec<usize> = (0..p1.received.len()).collect();
    by_received.sort_by_key(|&i| (int("mar.t_ns", &p1.received[i]).unwrap_or(i64::MAX), i));
    for m in by_received.iter().map(|&i| &p1.received[i]) {
        let id = string("mar.internal_id", m);
        let r = id.as_ref().and_then(|i| reports.get(i));
        let link = string("mar.report_id", m).and_then(|d| linked.get(&d));
        let subject_label = r.and_then(|r| p1.label_of(r.subject_node));
        let verdict = subject_label.map(|l| {
            if l == "benign" {
                "false_accusation"
            } else {
                "true_accusation"
            }
        });
        let mut row = obj(vec![
            ("run_id", run()),
            ("ma_id", pick("mar.ma_id", m)),
            ("report_id", pick("mar.report_id", m)),
            ("t_received_ns", pick("mar.t_ns", m)),
            ("generation_time_us", pick("mar.generation_time_us", m)),
            ("generation_location", pick("mar.generation_location", m)),
            ("psid", pick("mar.psid", m)),
            ("reporter_cert_id", pick("mar.reporter_cert_id", m)),
            ("subject_cert_id", pick("mar.subject_cert_id", m)),
            (
                "subject_issuer",
                opt(string("mar.subject_cert_id", m).and_then(|c| p1.cert_issuer.get(&c).cloned())),
            ),
            ("mbr_type", pick("mar.mbr_type", m)),
            ("subtypes", pick("mar.subtype_codes", m)),
            ("evidence", pick("mar.evidence", m)),
            ("evidence_start_ns", pick("mar.evidence_start_ns", m)),
            ("evidence_end_ns", pick("mar.evidence_end_ns", m)),
            ("set_aside", pick("mar.set_aside", m)),
            ("la_link", link.map_or(Value::Null, |l| l.0.clone())),
            ("la_link_t_ns", link.map_or(Value::Null, |l| l.1.clone())),
        ]);
        w.push(&global::REPORTS, &row)?;
        row.insert("verdict".into(), opt(verdict));
        row.insert("subject_label".into(), opt(subject_label));
        w.push(&ml::GLOBAL, &row)?;
    }

    // global/decisions
    for d in &p1.decisions {
        w.push(
            &global::DECISIONS,
            &obj(vec![
                ("run_id", run()),
                ("ma_id", pick("dec.ma_id", d)),
                ("t_ns", pick("dec.t_ns", d)),
                ("subject_kind", pick("dec.subject_kind", d)),
                ("subject_cert_id", pick("dec.subject_cert_id", d)),
                ("la_link", pick("dec.la_link", d)),
                ("decision", pick("dec.decision", d)),
                ("rule", pick("dec.rule", d)),
                ("report_ids", pick("dec.report_ids", d)),
                ("n_reports", pick("dec.n_reports", d)),
                ("n_distinct_reporters", pick("dec.n_distinct_reporters", d)),
            ]),
        )?;
    }

    // global/crl: ma.crl_entry, else the revocation pipeline's `issued` stage
    let (crl_rows, k): (&Vec<Value>, &str) = if p1.crl_entries.is_empty() {
        (&p1.revocations, "rev")
    } else {
        (&p1.crl_entries, "crl")
    };
    let kk = |c: &str| format!("{k}.{c}");
    // (device, t_issued, crl_number) per CRL entry, for the timeline
    let mut crl_of_device: BTreeMap<i64, (i64, Option<i64>)> = BTreeMap::new();
    for c in crl_rows {
        let t = int(&kk("t_issued_ns"), c);
        let link = string(&kk("la_link"), c);
        let hash = string(&kk("hash_id"), c);
        let dev = link
            .as_ref()
            .and_then(|l| link_device.get(l).copied())
            .or_else(|| hash.as_deref().and_then(cert_device));
        if let (Some(d), Some(t)) = (dev, t) {
            let e = crl_of_device
                .entry(d)
                .or_insert((t, int(&kk("crl_number"), c)));
            if t < e.0 {
                *e = (t, int(&kk("crl_number"), c));
            }
        }
        w.push(
            &global::CRL,
            &obj(vec![
                ("run_id", run()),
                ("t_issued_ns", opt(t)),
                ("crl_series", pick(&kk("crl_series"), c)),
                ("crl_number", pick(&kk("crl_number"), c)),
                ("entry_kind", pick(&kk("entry_kind"), c)),
                ("la_link", opt(link)),
                ("hash_id", opt(hash)),
                ("i_period", pick(&kk("i_period"), c)),
                ("t_effective_ns", pick(&kk("t_effective_ns"), c)),
            ]),
        )?;
    }

    // truth/devices
    for (n, d) in &p1.devices {
        let class = d.class.clone().unwrap_or_else(|| {
            if *n >= RSU_NODE_BASE {
                "rsu".into()
            } else {
                "vehicle".into()
            }
        });
        w.push(
            &truth::DEVICES,
            &obj(vec![
                ("run_id", run()),
                ("device_id", (*n).into()),
                ("class", class.into()),
                ("provider", opt(d.provider.clone())),
                ("gnss_class", opt(d.gnss_class.clone())),
                ("label", d.label().into()),
                ("fault_kind", opt(d.fault_kind.clone())),
                ("attack_id", opt(d.attack.clone())),
                (
                    "attack_params",
                    d.attack.as_ref().map_or(Value::Null, |a| {
                        meta.attack_mix
                            .iter()
                            .find(|m| &m.id == a)
                            .map_or(Value::Null, |m| json!(m.params))
                    }),
                ),
                ("attack_start_ns", opt(attack_window(*n, d).0)),
                ("attack_end_ns", opt(attack_window(*n, d).1)),
                ("joined_ns", opt(d.joined)),
                ("left_ns", opt(d.left)),
                ("la_link", opt(d.la_link.clone())),
                (
                    "receiver_ref",
                    if p1.is_obu(*n) {
                        rref(*n).into()
                    } else {
                        Value::Null
                    },
                ),
                ("instrumented", instrumented.contains(n).into()),
            ]),
        )?;
    }

    // truth/pseudonyms
    let mut certs: BTreeSet<&String> = cert_use.keys().collect();
    certs.extend(p1.cert_actor.keys());
    for c in certs {
        let u = cert_use.get(c);
        w.push(
            &truth::PSEUDONYMS,
            &obj(vec![
                ("run_id", run()),
                ("cert_id", c.clone().into()),
                ("device_id", opt(cert_device(c.as_str()))),
                ("first_use_ns", opt(u.map(|u| u.1))),
                ("last_use_ns", opt(u.map(|u| u.2))),
                ("n_tx", u.map_or(0, |u| u.3).into()),
            ]),
        )?;
    }

    // truth/reports
    let mut at_ma: BTreeMap<i64, i64> = BTreeMap::new();
    for r in reports.values() {
        let t_ma = r.received.and_then(|i| int("mar.t_ns", &p1.received[i]));
        if let (Some(s), Some(t)) = (r.subject_node, t_ma) {
            let e = at_ma.entry(s).or_insert(t);
            *e = (*e).min(t);
        }
        let subject_label = p1.label_of(r.subject_node);
        w.push(
            &truth::REPORTS,
            &obj(vec![
                ("run_id", run()),
                ("report_id", opt(r.report_id.clone())),
                ("reporter_device_id", opt(r.reporter)),
                ("subject_cert_id", opt(r.subject_cert.clone())),
                ("subject_device_id", opt(r.subject_node)),
                ("subject_label", opt(subject_label)),
                (
                    "verdict",
                    opt(subject_label.map(|l| {
                        if l == "benign" {
                            "false_accusation"
                        } else {
                            "true_accusation"
                        }
                    })),
                ),
                ("poisoned", r.forged.into()),
                ("fate", opt(r.fate)),
                ("t_created_ns", opt(r.t_created)),
                ("t_at_ma_ns", opt(t_ma)),
                (
                    "reporter_checks",
                    if r.checks.is_null() {
                        r.received
                            .map_or(Value::Null, |i| pick("mar.subtype_names", &p1.received[i]))
                    } else {
                        r.checks.clone()
                    },
                ),
                (
                    "nonstandard_triggers",
                    r.received.map_or(Value::Null, |i| {
                        pick("mar.nonstandard_triggers", &p1.received[i])
                    }),
                ),
            ]),
        )?;
    }

    // truth/timeline (schema.md §8): per misbehaving or falsely accused device
    let mut first_created: BTreeMap<i64, i64> = BTreeMap::new();
    let mut first_uploaded: BTreeMap<i64, i64> = BTreeMap::new();
    for r in reports.values() {
        let Some(s) = r.subject_node else { continue };
        if let Some(t) = r.t_created {
            let e = first_created.entry(s).or_insert(t);
            *e = (*e).min(t);
        }
        if let Some(t) = r.t_uploaded {
            let e = first_uploaded.entry(s).or_insert(t);
            *e = (*e).min(t);
        }
    }
    let mut decided: BTreeMap<i64, i64> = BTreeMap::new();
    for d in &p1.decisions {
        if string("dec.decision", d).as_deref() != Some("revoke") {
            continue;
        }
        let dev = string("dec.la_link", d)
            .and_then(|l| link_device.get(&l).copied())
            .or_else(|| {
                string("dec.subject_cert_id", d)
                    .as_deref()
                    .and_then(cert_device)
            });
        if let (Some(dev), Some(t)) = (dev, int("dec.t_ns", d)) {
            let e = decided.entry(dev).or_insert(t);
            *e = (*e).min(t);
        }
    }
    let mut crl_downloads: BTreeMap<i64, Vec<(i64, Option<i64>)>> = BTreeMap::new();
    for e in &p1.pki_events {
        if string("pki.event", e).as_deref() == Some("crl_downloaded")
            && let (Some(n), Some(t)) = (int("pki.node", e), int("pki.t_ns", e))
        {
            crl_downloads
                .entry(n)
                .or_default()
                .push((t, int("pki.crl_number", e)));
        }
    }
    let mut subjects: BTreeSet<i64> = p1
        .devices
        .iter()
        .filter(|(_, d)| d.label() != "benign")
        .map(|(n, _)| *n)
        .collect();
    subjects.extend(first_created.keys().copied());
    for dev in subjects {
        let d = p1.devices.get(&dev).cloned().unwrap_or_default();
        let crl = crl_of_device.get(&dev).copied();
        let nbrs = neighbours.get(&dev);
        let mut reached: Vec<i64> = Vec::new();
        if let Some((t_pub, number)) = crl {
            for n in nbrs.into_iter().flatten() {
                let first = crl_downloads.get(n).and_then(|v| {
                    v.iter()
                        .filter(|(t, k)| {
                            *t >= t_pub && number.is_none_or(|num| k.is_none_or(|k| k >= num))
                        })
                        .map(|(t, _)| *t)
                        .min()
                });
                if let Some(t) = first {
                    reached.push(t);
                }
            }
        }
        reached.sort_unstable();
        w.push(
            &truth::TIMELINE,
            &obj(vec![
                ("run_id", run()),
                ("device_id", dev.into()),
                ("label", d.label().into()),
                ("falsely_accused", (d.label() == "benign").into()),
                ("attack_start_ns", opt(attack_window(dev, &d).0)),
                // The first falsified message on the air (its truth/transmissions row),
                // else the first falsification the attack model logged.
                (
                    "first_falsified_ns",
                    opt(first_falsified.get(&dev).copied().or(d.first_atk)),
                ),
                ("first_detection_ns", opt(first_created.get(&dev).copied())),
                (
                    "first_report_created_ns",
                    opt(first_created.get(&dev).copied()),
                ),
                (
                    "first_report_uploaded_ns",
                    opt(first_uploaded.get(&dev).copied()),
                ),
                ("first_report_at_ma_ns", opt(at_ma.get(&dev).copied())),
                ("decision_ns", opt(decided.get(&dev).copied())),
                ("crl_published_ns", opt(crl.map(|c| c.0))),
                ("crl_first_neighbour_ns", opt(reached.first().copied())),
                (
                    "crl_median_neighbour_ns",
                    opt(truth::percentile(&reached, 50.0)),
                ),
                (
                    "crl_p95_neighbour_ns",
                    opt(truth::percentile(&reached, 95.0)),
                ),
                (
                    "n_neighbours",
                    (nbrs.map_or(0, BTreeSet::len) as i64).into(),
                ),
                (
                    "last_malicious_accepted_ns",
                    opt(last_mal_accepted.get(&dev).copied()),
                ),
            ]),
        )?;
    }

    // truth/app_effects: warnings caused by a falsified or faulty sender
    for a in &p1.warnings {
        if pick("app.fired", a).as_bool() == Some(false) {
            continue;
        }
        let subject_cert = string("app.subject_cert_id", a);
        let subject = subject_cert.as_deref().and_then(cert_device);
        let label = p1.label_of(subject);
        if label.is_none_or(|l| l == "benign") {
            continue;
        }
        w.push(
            &truth::APP_EFFECTS,
            &obj(vec![
                ("run_id", run()),
                ("t_ns", pick("app.t_ns", a)),
                ("app", pick("app.app", a)),
                ("receiver_device_id", pick("app.node", a)),
                (
                    "tx_id",
                    match pick("app.msg", a) {
                        Value::Null => {
                            let t = int("app.t_ns", a).unwrap_or(i64::MAX);
                            opt(subject_cert
                                .as_ref()
                                .and_then(|c| suspect_tx.get(c))
                                .and_then(|v| {
                                    let k = v.partition_point(|x| x.0 <= t);
                                    k.checked_sub(1).map(|k| v[k].1)
                                }))
                        }
                        v => v,
                    },
                ),
                ("subject_cert_id", opt(subject_cert)),
                ("subject_device_id", opt(subject)),
                ("subject_label", opt(label)),
                ("severity", pick("app.severity", a)),
                ("driver_reaction", pick("app.reaction", a)),
            ]),
        )?;
    }

    // runs.parquet
    #[allow(clippy::cast_precision_loss)]
    let bsm_rate = match (meta.duration_s, bsm_senders.len()) {
        (Some(d), n) if d > 0.0 && n > 0 => Some(bsm_tx as f64 / n as f64 / d),
        _ => None,
    };
    let mix: Vec<Value> = meta
        .attack_mix
        .iter()
        .map(|m| json!({"id": m.id, "fraction": m.fraction, "count": m.count, "from_s": m.from_s, "to_s": m.to_s, "params": m.params}))
        .collect();
    w.push(
        &RUNS,
        &obj(vec![
            ("run_id", run()),
            ("split", opts.split.clone().into()),
            ("tier", opts.tier.clone().into()),
            ("heldout_attacks", opts.heldout_attacks.clone().into()),
            ("scenario_name", opt(meta.name.clone())),
            ("city", opt(meta.city.clone())),
            ("world_source", opt(meta.world_source.clone())),
            ("t0_utc", opt(meta.t0_utc.clone())),
            ("duration_s", opt(meta.duration_s)),
            ("origin_lat_deg", opt(meta.origin_lat_deg)),
            ("origin_lon_deg", opt(meta.origin_lon_deg)),
            ("vehicles", opt(meta.vehicles)),
            ("equipped_fraction", opt(meta.equipped_fraction)),
            ("demand_veh_per_h", opt(meta.demand_veh_per_h)),
            ("rat", opt(meta.rat.clone())),
            ("bsm_rate_hz", opt(bsm_rate)),
            ("scms_mode", opt(meta.scms_mode.clone())),
            ("calendar", opt(meta.calendar.clone())),
            ("local_suite", opt(meta.local_suite.clone())),
            ("ma_policy", opt(meta.ma_policy.clone())),
            ("attacker_fraction", opt(meta.attacker_fraction)),
            ("attack_mix", Value::Array(mix)),
            ("instrumented_receivers", (instrumented.len() as i64).into()),
            ("engine_version", opt(meta.engine_version.clone())),
            ("git_sha", opt(meta.git_sha.clone())),
            ("scenario_hash", opt(meta.scenario_hash.clone())),
            ("seed", seed.clone().into()),
            ("release_schema", RELEASE_SCHEMA.into()),
        ]),
    )?;

    let mut files = w.finish()?;

    // schema/<table>.json
    for def in all_tables() {
        let rel = format!("schema/{}.json", def.file_stem());
        let path = root.join(&rel);
        write_bytes(&path, &pretty(&def.schema_json())?)?;
        files.push(WrittenFile {
            rel,
            path,
            table: None,
            rows: None,
        });
    }

    // the leakage gate, over the bytes on disk
    let mut identities: BTreeSet<String> = reports.keys().cloned().collect();
    identities.extend(
        p1.devices
            .values()
            .filter_map(|d| d.attack.clone())
            .filter(|a| a.len() > 3),
    );
    let lint = lint_release(root, &identities)?;

    // DATASHEET.md
    let counts: Vec<(String, u64)> = files
        .iter()
        .filter_map(|f| Some((f.table.clone()?, f.rows?)))
        .collect();
    let sheet =
        crate::dataset::datasheet::render_release(meta, opts, &counts, instrumented.len(), &lint);
    let rel = "DATASHEET.md".to_string();
    let path = root.join(&rel);
    write_bytes(&path, sheet.as_bytes())?;
    files.push(WrittenFile {
        rel,
        path,
        table: None,
        rows: None,
    });

    if !lint.is_clean() {
        return Err(RecordError::malformed(
            "dataset release",
            format!(
                "the release under {} failed the leakage linter: {}",
                root.display(),
                lint.summary()
            ),
        ));
    }

    // manifest.json: every file under the root, compat/ included
    let manifest = manifest_of(root, &files, meta, opts)?;
    let rel = "manifest.json".to_string();
    let path = root.join(&rel);
    write_bytes(&path, &pretty(&manifest)?)?;
    files.push(WrittenFile {
        rel,
        path,
        table: None,
        rows: None,
    });
    Ok(WrittenRelease {
        root: root.to_path_buf(),
        files,
        lint,
        instrumented: instrumented.len(),
    })
}

/// Whether a message of this type states its sender's own position and motion: the
/// safety messages (BSM, PSM; CAM and VAM on the ETSI stack). A misbehaviour report, a
/// CRL, a WSA or a P2PCD frame does not.
fn states_own_kinematics(msg_type: &str) -> bool {
    matches!(
        msg_type.to_ascii_lowercase().as_str(),
        "bsm" | "psm" | "cam" | "vam"
    )
}

/// One written Parquet part's rows as JSON objects, in file order: how a test, or a quick
/// look at a release, reads a table without the Arrow API.
///
/// # Errors
/// I/O, Parquet or Arrow errors reading the part.
pub fn read_rows(path: &Path) -> Result<Vec<Value>> {
    let batches = crate::export::parquet::read(path)?;
    let mut w = arrow::json::ArrayWriter::new(Vec::new());
    let refs: Vec<&arrow::array::RecordBatch> = batches.iter().collect();
    w.write_batches(&refs)?;
    w.finish()?;
    let bytes = w.into_inner();
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| RecordError::malformed("release part", format!("{}: {e}", path.display())))
}

/// J2735 `PositionalAccuracy` from the transmission's flat content fields.
fn pos_accuracy(tx: &Value) -> Value {
    let m = obj(vec![
        ("semi_major_m", pick("tx.pos_semi_major_m", tx)),
        ("semi_minor_m", pick("tx.pos_semi_minor_m", tx)),
        ("orientation_deg", pick("tx.pos_orientation_deg", tx)),
    ]);
    if m.values().all(Value::is_null) {
        Value::Null
    } else {
        Value::Object(m)
    }
}

/// J2735 `AccelerationSet4Way` from the transmission's flat content fields.
fn accel(tx: &Value) -> Value {
    let m = obj(vec![
        ("long_mps2", pick("tx.accel_long_mps2", tx)),
        ("lat_mps2", pick("tx.accel_lat_mps2", tx)),
        ("vert_mps2", pick("tx.accel_vert_mps2", tx)),
        ("yaw_rate_dps", pick("tx.yaw_rate_dps", tx)),
    ]);
    if m.values().all(Value::is_null) {
        Value::Null
    } else {
        Value::Object(m)
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn pretty(v: &Value) -> Result<Vec<u8>> {
    let mut b = serde_json::to_vec_pretty(v)
        .map_err(|e| RecordError::malformed("release json", e.to_string()))?;
    b.push(b'\n');
    Ok(b)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| RecordError::io(d, e))?;
    }
    std::fs::write(path, bytes).map_err(|e| RecordError::io(path, e))
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk_files(&p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

fn manifest_of(
    root: &Path,
    files: &[WrittenFile],
    meta: &RunMeta,
    opts: &ReleaseOptions,
) -> Result<Value> {
    let rows: BTreeMap<&str, u64> = files
        .iter()
        .filter_map(|f| Some((f.rel.as_str(), f.rows?)))
        .collect();
    let mut all = Vec::new();
    walk_files(root, &mut all)?;
    let mut out = Vec::new();
    for p in all {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "manifest.json" {
            continue;
        }
        let bytes = std::fs::read(&p).map_err(|e| RecordError::io(&p, e))?;
        out.push(json!({
            "path": rel,
            "sha256": v2xw_core::hash::sha256_hex(&bytes),
            "bytes": bytes.len(),
            "rows": rows.get(rel.as_str()),
        }));
    }
    Ok(json!({
        "release_schema": RELEASE_SCHEMA,
        "run_id": meta.run_id,
        "split": opts.split,
        "tier": opts.tier,
        "scenario_hash": meta.scenario_hash,
        "seed": meta.seed,
        "engine_version": meta.engine_version,
        "git_sha": meta.git_sha,
        "files": out,
    }))
}

// ---------------------------------------------------------------------------------------
// Leakage gate over the written layout
// ---------------------------------------------------------------------------------------

fn field_names(prefix: &str, f: &Field, out: &mut Vec<String>) {
    let name = if f.name() == "item" {
        prefix.to_string()
    } else if prefix.is_empty() {
        f.name().clone()
    } else {
        format!("{prefix}.{}", f.name())
    };
    match f.data_type() {
        DataType::Struct(fields) => {
            for c in fields {
                field_names(&name, c, out);
            }
        }
        DataType::List(item) => field_names(&name, item, out),
        _ => {}
    }
    if f.name() != "item" {
        out.push(name);
    }
}

/// Lints every Parquet file under `root/local` and `root/global`, read back from disk: no
/// column (at any nesting depth) may be a truth key ([`crate::dataset::leakage::is_release_truth_key`]),
/// and no string value may be one of `identities` (the engine's report ids, attack names).
/// `ml/` is the declared joined exception and is not linted.
///
/// # Errors
/// I/O or Parquet errors reading the files back.
pub fn lint_release(root: &Path, identities: &BTreeSet<String>) -> Result<LeakageReport> {
    use arrow::array::Array as _;
    let mut report = LeakageReport::default();
    for view in ["local", "global"] {
        let mut paths = Vec::new();
        walk_files(&root.join(view), &mut paths)?;
        for p in paths
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "parquet"))
        {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .into_owned();
            let file = File::open(&p).map_err(|e| RecordError::io(&p, e))?;
            let reader =
                parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?;
            let schema = reader.schema().clone();
            let mut names = Vec::new();
            for f in schema.fields() {
                field_names("", f, &mut names);
            }
            report.merge(crate::dataset::leakage::lint_release_columns(&rel, &names));
            if identities.is_empty() {
                continue;
            }
            for batch in reader.build()? {
                let batch = batch?;
                for (ci, f) in schema.fields().iter().enumerate() {
                    if f.data_type() != &DataType::Utf8 {
                        continue;
                    }
                    let Some(a) = batch
                        .column(ci)
                        .as_any()
                        .downcast_ref::<arrow::array::StringArray>()
                    else {
                        continue;
                    };
                    for (ri, s) in a.iter().enumerate() {
                        if let Some(s) = s
                            && identities.contains(s)
                        {
                            report.violations.push(LeakageViolation {
                                file: rel.clone(),
                                row: ri,
                                path: f.name().clone(),
                                kind: ViolationKind::IdentityValue,
                                detail: format!("{s:?} is a ground-truth identity"),
                            });
                        }
                    }
                }
            }
        }
    }
    report.violations.sort();
    report.violations.dedup();
    Ok(report)
}

#[cfg(test)]
mod tests;
