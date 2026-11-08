//! `v2xw.plugins.MaPolicy` — a Misbehaviour Authority decision rule written in Python.
//!
//! [`PyMaPolicy`] wears the engine's [`MaPolicyHook`]: each step in which a deciding MA
//! receives new evidence, the Python object's `decide(ctx, batch)` is called once, with
//! the step's evidence as one [`MaBatch`] (the `ma.received` columns, in ingest order),
//! and returns `(subject, revoke)` or `(subject, revoke, row)` tuples. A step with no new
//! evidence makes no call. It is reachable only from a run Python starts
//! (`v2xw.run(..., ma_policy=obj)`): a scenario file cannot name Python code.
//!
//! # The batch is Arrow
//!
//! [`MaBatch::to_pyarrow`] hands the batch to `pyarrow` through the C data interface, as a
//! detector's window is handed over. The same batch also answers `column(name)` and
//! `rows()` in plain Python values, so a rule runs where `pyarrow` is not installed; both
//! read the same rows.
//!
//! # The plug-in rules
//!
//! The rules of [`crate::plugins`] apply (no own randomness, wall clock or platform libm,
//! no hash-ordered iteration reaching a result), with one difference that is the point of
//! the family: an MA holds evidence between calls. Its state must be a function of the
//! calls it received, which the engine makes the same on every run of a seed; the
//! two-run digest comparison is what checks it.
//!
//! # Pinning
//!
//! The manifest records the rule as `ma_policy:python:<module>.<class>` with its card's
//! version and the SHA-256 of its module's source (`inspect.getsource`). A rule whose
//! source cannot be read (typed into an interactive prompt) is refused: a run must say
//! which code decided its revocations.
//!
//! # A rule that raises
//!
//! The MA decides nothing for that call, the run continues, and `v2xw.run` raises the
//! first traceback once the run ends, so a broken rule cannot pass for one that decided
//! nothing.

use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanArray, Float64Array, ListBuilder, RecordBatch, StringArray, StringBuilder,
    UInt32Array, UInt64Array,
};
use arrow::datatypes::{Field, Schema};
use arrow::pyarrow::IntoPyArrow;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use v2xw_core::card::{Family, ModelCard};
use v2xw_core::manifest::PluginPin;
use v2xw_engine::ma_hook::{MaHookCall, MaHookRow, MaPolicyHook, MaVerdict};

use crate::err;

/// The batch's columns, in order.
pub const COLUMNS: [&str; 16] = [
    "t_ns",
    "observed_ns",
    "report_id",
    "ma",
    "reporter_cert",
    "subject_cert",
    "subtypes",
    "generation_time_ns",
    "evidence_start_ns",
    "evidence_end_ns",
    "excess",
    "pos_accuracy_m",
    "secmark_phase_ms",
    "issuer",
    "la_link",
    "reporter_infrastructure",
];

/// Where a rule is called: the step's instant and the deciding MA.
#[pyclass(name = "MaCtx", module = "v2xw.plugins", frozen, get_all)]
#[derive(Debug, Clone)]
pub struct MaCtx {
    /// The step's instant, ns.
    pub t_ns: u64,
    /// The deciding MA's provider index, in deployment order.
    pub ma: u32,
    /// Its provider's name.
    pub provider: String,
}

#[pymethods]
impl MaCtx {
    fn __repr__(&self) -> String {
        format!(
            "MaCtx(t_ns={}, ma={}, provider={:?})",
            self.t_ns, self.ma, self.provider
        )
    }
}

/// One step's evidence at one MA.
#[pyclass(name = "MaBatch", module = "v2xw.plugins", frozen)]
#[derive(Debug, Clone)]
pub struct MaBatch {
    rows: Vec<MaHookRow>,
}

impl MaBatch {
    /// The batch over `rows`.
    #[must_use]
    pub fn new(rows: Vec<MaHookRow>) -> Self {
        Self { rows }
    }

    /// The rows as one Arrow record batch with [`COLUMNS`].
    ///
    /// # Panics
    /// Never: every column has one entry per row by construction.
    #[must_use]
    pub fn arrow(&self) -> RecordBatch {
        let r = &self.rows;
        let u64s = |f: fn(&MaHookRow) -> u64| -> ArrayRef {
            Arc::new(UInt64Array::from(r.iter().map(f).collect::<Vec<_>>()))
        };
        let strs = |f: fn(&MaHookRow) -> &str| -> ArrayRef {
            Arc::new(StringArray::from(r.iter().map(f).collect::<Vec<_>>()))
        };
        let mut subtypes = ListBuilder::new(StringBuilder::new());
        for row in r {
            for s in &row.view.subtypes {
                subtypes.values().append_value(s);
            }
            subtypes.append(true);
        }
        let columns: Vec<ArrayRef> = vec![
            u64s(|x| x.view.t),
            u64s(|x| x.observed),
            strs(|x| &x.view.report_id),
            Arc::new(UInt32Array::from(
                r.iter().map(|x| x.view.ma).collect::<Vec<_>>(),
            )),
            strs(|x| &x.view.reporter_cert),
            strs(|x| &x.view.subject_cert),
            Arc::new(subtypes.finish()),
            u64s(|x| x.view.generation_time),
            u64s(|x| x.view.evidence_start),
            u64s(|x| x.view.evidence_end),
            Arc::new(Float64Array::from(
                r.iter().map(|x| x.view.excess).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                r.iter().map(|x| x.view.pos_accuracy_m).collect::<Vec<_>>(),
            )),
            Arc::new(UInt32Array::from(
                r.iter()
                    .map(|x| x.view.secmark_phase_ms)
                    .collect::<Vec<_>>(),
            )),
            strs(|x| &x.view.issuer),
            strs(|x| &x.view.la_link),
            Arc::new(BooleanArray::from(
                r.iter()
                    .map(|x| x.reporter_infrastructure)
                    .collect::<Vec<_>>(),
            )),
        ];
        let fields: Vec<Field> = COLUMNS
            .iter()
            .zip(&columns)
            .map(|(name, c)| Field::new(*name, c.data_type().clone(), false))
            .collect();
        RecordBatch::try_new(Arc::new(Schema::new(fields)), columns)
            .expect("one entry per row in every column")
    }

    fn value<'py>(py: Python<'py>, row: &MaHookRow, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let v = &row.view;
        Ok(match name {
            "t_ns" => v.t.into_pyobject(py)?.into_any(),
            "observed_ns" => row.observed.into_pyobject(py)?.into_any(),
            "report_id" => v.report_id.as_str().into_pyobject(py)?.into_any(),
            "ma" => v.ma.into_pyobject(py)?.into_any(),
            "reporter_cert" => v.reporter_cert.as_str().into_pyobject(py)?.into_any(),
            "subject_cert" => v.subject_cert.as_str().into_pyobject(py)?.into_any(),
            "subtypes" => PyList::new(py, &v.subtypes)?.into_any(),
            "generation_time_ns" => v.generation_time.into_pyobject(py)?.into_any(),
            "evidence_start_ns" => v.evidence_start.into_pyobject(py)?.into_any(),
            "evidence_end_ns" => v.evidence_end.into_pyobject(py)?.into_any(),
            "excess" => v.excess.into_pyobject(py)?.into_any(),
            "pos_accuracy_m" => v.pos_accuracy_m.into_pyobject(py)?.into_any(),
            "secmark_phase_ms" => v.secmark_phase_ms.into_pyobject(py)?.into_any(),
            "issuer" => v.issuer.as_str().into_pyobject(py)?.into_any(),
            "la_link" => v.la_link.as_str().into_pyobject(py)?.into_any(),
            "reporter_infrastructure" => pyo3::types::PyBool::new(py, row.reporter_infrastructure)
                .to_owned()
                .into_any(),
            other => {
                return Err(pyo3::exceptions::PyKeyError::new_err(format!(
                    "no column {other:?}; the columns are {COLUMNS:?}"
                )));
            }
        })
    }
}

#[pymethods]
impl MaBatch {
    /// How many reports the batch holds.
    #[getter]
    fn num_rows(&self) -> usize {
        self.rows.len()
    }

    fn __len__(&self) -> usize {
        self.rows.len()
    }

    /// The column names, in order.
    #[getter]
    fn columns(&self) -> Vec<&'static str> {
        COLUMNS.to_vec()
    }

    /// One column as a Python list.
    fn column<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyList>> {
        let mut out = Vec::with_capacity(self.rows.len());
        for r in &self.rows {
            out.push(Self::value(py, r, name)?);
        }
        PyList::new(py, out)
    }

    /// Every row as a `dict` of its columns, in ingest order.
    fn rows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let mut out = Vec::with_capacity(self.rows.len());
        for r in &self.rows {
            let d = PyDict::new(py);
            for name in COLUMNS {
                d.set_item(name, Self::value(py, r, name)?)?;
            }
            out.push(d);
        }
        PyList::new(py, out)
    }

    /// The batch as a `pyarrow.RecordBatch`, sharing the engine's buffers.
    fn to_pyarrow(&self, py: Python<'_>) -> PyResult<PyObject> {
        self.arrow().into_pyarrow(py)
    }

    fn __repr__(&self) -> String {
        format!("<v2xw.plugins.MaBatch rows={}>", self.rows.len())
    }
}

/// An MA decision rule written in Python, wearing the engine's [`MaPolicyHook`].
pub struct PyMaPolicy {
    object: Py<PyAny>,
    card: ModelCard,
    id: String,
    source_sha256: String,
}

impl std::fmt::Debug for PyMaPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyMaPolicy")
            .field("id", &self.id)
            .field("source_sha256", &self.source_sha256)
            .finish_non_exhaustive()
    }
}

impl PyMaPolicy {
    /// Adapts a Python object: its `card` validates as an `ma-pipeline` card, it has a
    /// callable `decide(ctx, batch)`, and its module's source can be read and hashed.
    ///
    /// # Errors
    /// `V2xwError` naming what is missing.
    pub fn attach(object: Bound<'_, PyAny>) -> PyResult<Self> {
        let py = object.py();
        let card_obj = object.getattr("card").map_err(|_| {
            err::V2xwError::new_err(
                "an MA policy must carry a model card: set a `card` attribute \
                  with family \"ma-pipeline\"",
            )
        })?;
        let card = crate::plugins::card_from_py(&card_obj)?;
        card.validate().map_err(|e| {
            err::V2xwError::new_err(format!("model card {}: {}", card.id, err::chain(&e)))
        })?;
        if card.family != Family::MaPipeline {
            return Err(err::V2xwError::new_err(format!(
                "model card {} declares family {}, but it is being attached as an MA policy \
                 (family ma-pipeline)",
                card.id, card.family
            )));
        }
        if !object.getattr("decide").is_ok_and(|a| a.is_callable()) {
            return Err(err::V2xwError::new_err(format!(
                "MA policy {} has no callable `decide(ctx, batch)`",
                card.id
            )));
        }
        let cls = object.get_type();
        let module: String = cls.getattr("__module__")?.extract()?;
        let qualname: String = cls.getattr("__qualname__")?.extract()?;
        let inspect = py.import("inspect")?;
        let source: String = inspect
            .call_method1("getmodule", (&cls,))
            .and_then(|m| inspect.call_method1("getsource", (m,)))
            .or_else(|_| inspect.call_method1("getsource", (&cls,)))
            .and_then(|s| s.extract())
            .map_err(|e| {
                err::V2xwError::new_err(format!(
                    "the source of MA policy {module}.{qualname} cannot be read ({e}); define \
                     it in a module file, so the manifest can pin the code that decided the \
                     run's revocations"
                ))
            })?;
        Ok(Self {
            object: object.unbind(),
            card,
            id: format!("python:{module}.{qualname}"),
            source_sha256: v2xw_core::hash::sha256_hex(source.as_bytes()),
        })
    }

    /// The id the manifest pins, after `ma_policy:`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The SHA-256 of its module's source, hex.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    fn verdicts(result: &Bound<'_, PyAny>) -> PyResult<Vec<MaVerdict>> {
        let mut out = Vec::new();
        for item in result.try_iter()? {
            let item = item?;
            let t = item.downcast::<PyTuple>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(format!(
                    "decide must return (subject, revoke) or (subject, revoke, row) tuples, \
                     got {}",
                    item.repr()
                        .map_or_else(|_| "?".to_string(), |r| r.to_string())
                ))
            })?;
            let (subject, revoke, row) = match t.len() {
                2 => (t.get_item(0)?.extract()?, t.get_item(1)?.extract()?, None),
                3 => (
                    t.get_item(0)?.extract()?,
                    t.get_item(1)?.extract()?,
                    Some(t.get_item(2)?.extract::<usize>()?),
                ),
                n => {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "a decision is (subject, revoke) or (subject, revoke, row), not a \
                         {n}-tuple"
                    )));
                }
            };
            out.push(MaVerdict {
                subject,
                revoke,
                row,
            });
        }
        Ok(out)
    }
}

impl MaPolicyHook for PyMaPolicy {
    fn pin(&self) -> PluginPin {
        PluginPin::new(
            self.id.clone(),
            self.card.version.clone(),
            self.source_sha256.clone(),
        )
    }

    fn decide(&mut self, call: &MaHookCall<'_>) -> std::result::Result<Vec<MaVerdict>, String> {
        Python::with_gil(|py| {
            let ctx = MaCtx {
                t_ns: call.t,
                ma: call.ma,
                provider: call.provider.to_string(),
            };
            let batch = MaBatch::new(call.rows.to_vec());
            self.object
                .bind(py)
                .call_method1("decide", (ctx, batch))
                .and_then(|r| Self::verdicts(&r))
                .map_err(|e| {
                    let tb = e
                        .traceback(py)
                        .and_then(|t| t.format().ok())
                        .unwrap_or_default();
                    format!("MA policy {} raised: {e}\n{tb}", self.id)
                })
        })
    }
}

/// Registers the family's classes on `v2xw.plugins`.
///
/// # Errors
/// Whatever `PyModule::add_class` returns.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<MaCtx>()?;
    m.add_class::<MaBatch>()?;
    m.add("MA_BATCH_COLUMNS", COLUMNS.to_vec())?;
    Ok(())
}
