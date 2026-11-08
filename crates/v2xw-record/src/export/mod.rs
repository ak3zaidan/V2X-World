//! Exporters.
//!
//! | Format | Used for | Module |
//! |---|---|---|
//! | Parquet | every tabular record channel; "the primary format" | [`parquet`] |
//! | Arrow IPC | metric batches and Python plug-in exchange | [`arrow_ipc`] |
//! | JSONL | the legacy MA dataset profile | [`jsonl`] |
//!
//! Every one of them writes through [`table::build_batch`] or [`jsonl::write`], and both
//! of those quantise each float to the grid its column declares (D9). The declared grids
//! are written next to the data as `schema.json`, and [`scan`] reads them back and checks
//! every value — the output-scanning test.
//!
//! # Visibility
//!
//! [`ExportProfile::NodeOnly`] drops the ground-truth channels entirely and projects the
//! ground-truth *columns* out of the mixed ones (§5.2), so a `NODE` export and a `GT`
//! export never share a file (conformance V6). Each file's schema records which it is.

pub mod arrow_ipc;
pub mod jsonl;
pub mod parquet;
pub mod scan;
pub mod schema;
pub mod table;

use std::path::{Path, PathBuf};

use crate::channels;
use crate::error::{RecordError, Result};
use crate::reader::RecordedRecord;

pub use schema::{ColumnKind, ColumnSpec, SchemaBuilder, TableSchema};

/// Which artefact format a file is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Apache Parquet.
    Parquet,
    /// Arrow IPC file format.
    ArrowIpc,
    /// JSON Lines.
    Jsonl,
    /// A plain JSON document — a schema sidecar or a manifest.
    Json,
}

impl ExportFormat {
    /// The file extension this format uses.
    pub const fn extension(self) -> &'static str {
        match self {
            ExportFormat::Parquet => "parquet",
            ExportFormat::ArrowIpc => "arrow",
            ExportFormat::Jsonl => "jsonl",
            ExportFormat::Json => "json",
        }
    }
}

/// How much of the data an export may contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportProfile {
    /// Everything the recording holds.
    #[default]
    Full,
    /// No ground-truth channel and no ground-truth column (§5.2).
    NodeOnly,
}

/// One file an export produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportedFile {
    /// Where it is.
    pub path: PathBuf,
    /// What it is in.
    pub format: ExportFormat,
    /// The channel it holds, or `None` for a sidecar.
    pub channel: Option<String>,
    /// Rows written.
    pub rows: usize,
    /// Bytes written.
    pub bytes: u64,
    /// The schema its values are declared against, or `None` for a sidecar.
    pub schema: Option<TableSchema>,
}

/// Exports a recording's serde records to a directory.
#[derive(Debug, Clone)]
pub struct Exporter {
    dir: PathBuf,
    profile: ExportProfile,
}

impl Exporter {
    /// An exporter writing into `dir`, which is created if it does not exist.
    ///
    /// # Errors
    /// [`RecordError::Io`] if the directory cannot be created.
    pub fn new(dir: impl AsRef<Path>, profile: ExportProfile) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| RecordError::io(&dir, e))?;
        Ok(Exporter { dir, profile })
    }

    /// The directory being written into.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The profile.
    pub fn profile(&self) -> ExportProfile {
        self.profile
    }

    /// Exports one channel in one format, plus its `schema.json` sidecar.
    ///
    /// Returns the data file and the sidecar, in that order.
    ///
    /// # Errors
    /// [`RecordError::UnknownChannel`] for a channel outside the catalogue,
    /// [`RecordError::VisibilityDenied`] if the profile forbids the channel, and whatever
    /// the format's writer returns.
    pub fn export_channel(
        &self,
        channel: &str,
        records: &[RecordedRecord],
        format: ExportFormat,
    ) -> Result<Vec<ExportedFile>> {
        let spec = channels::by_name(channel)
            .ok_or_else(|| RecordError::UnknownChannel(channel.to_string()))?;
        if self.profile == ExportProfile::NodeOnly && spec.is_ground_truth_channel() {
            return Err(RecordError::VisibilityDenied {
                channel: channel.to_string(),
                visibility: spec.visibility,
            });
        }
        let rows: Vec<(u64, Vec<u8>)> = records
            .iter()
            .filter(|r| r.channel == channel)
            .map(|r| (r.sim_time, r.json.clone()))
            .collect();
        let inferred = TableSchema::infer(channel, &rows)?;
        let schema = if self.profile == ExportProfile::NodeOnly {
            inferred.without_ground_truth()
        } else {
            inferred
        };

        let stem = channel.replace('.', "_");
        let data_path = self.dir.join(format!("{stem}.{}", format.extension()));
        let bytes = match format {
            ExportFormat::Parquet => {
                let batch = table::build_batch(&schema, &rows)?;
                parquet::write(&data_path, &batch)?
            }
            ExportFormat::ArrowIpc => {
                let batch = table::build_batch(&schema, &rows)?;
                arrow_ipc::write(&data_path, &[batch])?
            }
            ExportFormat::Jsonl => jsonl::write(&data_path, &schema, &rows)?,
            ExportFormat::Json => {
                return Err(RecordError::malformed(
                    "export",
                    "plain JSON is for sidecars, not for a record table",
                ));
            }
        };
        let schema_path = self.dir.join(format!("{stem}.schema.json"));
        let schema_bytes = schema.to_json()?;
        std::fs::write(&schema_path, &schema_bytes)
            .map_err(|e| RecordError::io(&schema_path, e))?;
        Ok(vec![
            ExportedFile {
                path: data_path,
                format,
                channel: Some(channel.to_string()),
                rows: rows.len(),
                bytes,
                schema: Some(schema),
            },
            ExportedFile {
                path: schema_path,
                format: ExportFormat::Json,
                channel: Some(channel.to_string()),
                rows: 0,
                bytes: schema_bytes.len() as u64,
                schema: None,
            },
        ])
    }

    /// Exports every channel present in `records`, each in `format`.
    ///
    /// Channels are visited in name order, so an export is reproducible.
    ///
    /// # Errors
    /// As [`Exporter::export_channel`], except that a channel the profile forbids is
    /// skipped rather than refused — a `NODE-only` export of a full recording is a normal
    /// thing to want.
    pub fn export_all(
        &self,
        records: &[RecordedRecord],
        format: ExportFormat,
    ) -> Result<Vec<ExportedFile>> {
        let mut names: Vec<&str> = records.iter().map(|r| r.channel.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        let mut out = Vec::new();
        for name in names {
            let forbidden = self.profile == ExportProfile::NodeOnly
                && channels::by_name(name).is_some_and(|s| s.is_ground_truth_channel());
            if forbidden {
                continue;
            }
            out.extend(self.export_channel(name, records, format)?);
        }
        Ok(out)
    }

    /// [`Exporter::export_all`] over a recording on disk, without holding its records.
    ///
    /// The files are the bytes `export_all` writes from `reader.records(None)`, but
    /// memory no longer scales with the run: a five-minute Manhattan run records about
    /// 6 GB of reception JSON, and the list of every record plus each channel's copy of
    /// its rows took a server on an 8 GB machine to an 18 GB footprint. A first pass over
    /// the recording infers each channel's schema record by record ([`SchemaBuilder`]);
    /// JSONL then streams every row to its channel's file in a second pass, and the
    /// columnar formats, which build a whole table, hold one channel's rows at a time.
    ///
    /// # Errors
    /// As [`Exporter::export_all`], and whatever reading the recording returns.
    pub fn export_recording<S: crate::index::Source>(
        &self,
        reader: &mut crate::Reader<S>,
        format: ExportFormat,
    ) -> Result<Vec<ExportedFile>> {
        if format == ExportFormat::Json {
            return Err(RecordError::malformed(
                "export",
                "plain JSON is for sidecars, not for a record table",
            ));
        }
        let forbidden = |name: &str| {
            self.profile == ExportProfile::NodeOnly
                && channels::by_name(name).is_some_and(|s| s.is_ground_truth_channel())
        };
        // Pass 1: the channels present, in name order, and each one's schema.
        let mut builders: std::collections::BTreeMap<String, schema::SchemaBuilder> =
            std::collections::BTreeMap::new();
        reader.for_each_message(
            |_| Ok(()),
            |r| {
                if forbidden(&r.channel) {
                    return Ok(());
                }
                match builders.get_mut(&r.channel) {
                    Some(b) => b.observe(&r.json),
                    None => {
                        let mut b = schema::SchemaBuilder::new(&r.channel)?;
                        b.observe(&r.json)?;
                        builders.insert(r.channel, b);
                        Ok(())
                    }
                }
            },
        )?;
        let mut tables: Vec<(String, TableSchema)> = builders
            .into_iter()
            .map(|(name, b)| {
                let inferred = b.finish();
                let schema = if self.profile == ExportProfile::NodeOnly {
                    inferred.without_ground_truth()
                } else {
                    inferred
                };
                (name, schema)
            })
            .collect();
        let data_path = |channel: &str| {
            let stem = channel.replace('.', "_");
            self.dir.join(format!("{stem}.{}", format.extension()))
        };
        // Pass 2: the rows.
        let mut written: Vec<(usize, u64)> = Vec::with_capacity(tables.len());
        if format == ExportFormat::Jsonl {
            let mut writers = std::collections::BTreeMap::new();
            for (name, schema) in &tables {
                writers.insert(
                    name.clone(),
                    jsonl::JsonlWriter::create(data_path(name), schema.clone())?,
                );
            }
            reader.for_each_message(
                |_| Ok(()),
                |r| match writers.get_mut(&r.channel) {
                    Some(w) => w.write_row(r.sim_time, &r.json),
                    None => Ok(()),
                },
            )?;
            for (_, w) in writers {
                let rows = w.rows();
                written.push((rows, w.finish()?));
            }
        } else {
            for (name, schema) in &tables {
                let mut rows: Vec<(u64, Vec<u8>)> = Vec::new();
                reader.for_each_message(
                    |_| Ok(()),
                    |r| {
                        if r.channel == *name {
                            rows.push((r.sim_time, r.json));
                        }
                        Ok(())
                    },
                )?;
                let batch = table::build_batch(schema, &rows)?;
                let count = rows.len();
                drop(rows);
                let path = data_path(name);
                let bytes = match format {
                    ExportFormat::Parquet => parquet::write(&path, &batch)?,
                    _ => arrow_ipc::write(&path, &[batch])?,
                };
                written.push((count, bytes));
            }
        }
        let mut out = Vec::with_capacity(2 * tables.len());
        for ((name, schema), (rows, bytes)) in tables.drain(..).zip(written) {
            let stem = name.replace('.', "_");
            let schema_path = self.dir.join(format!("{stem}.schema.json"));
            let schema_bytes = schema.to_json()?;
            std::fs::write(&schema_path, &schema_bytes)
                .map_err(|e| RecordError::io(&schema_path, e))?;
            out.push(ExportedFile {
                path: data_path(&name),
                format,
                channel: Some(name.clone()),
                rows,
                bytes,
                schema: Some(schema),
            });
            out.push(ExportedFile {
                path: schema_path,
                format: ExportFormat::Json,
                channel: Some(name),
                rows: 0,
                bytes: schema_bytes.len() as u64,
                schema: None,
            });
        }
        Ok(out)
    }
}
