#!/usr/bin/env python3
"""validate_release.py <release-root>: independent checks of a V2X-SCMS-MBD release.

The contract is docs/dataset/schema.md. This tool shares no code with the Rust writer
(crates/v2xw-record/src/dataset/) or with make_fixture.py: it reads the files with pyarrow
and checks them against schema/<table>.json when the release ships it, falling back to the
column names in schema.md otherwise.

Each check is a function check_<name>(release) -> Result and prints one verdict line:

  schema     columns, types and nullability per table
  joins      evidence digests, rx ids and report ids resolve across views
  leakage    no truth column, and no truth-only id value, in local/ or global/
  labels     message, device and report labels agree; attack_mix fractions hold
  time       0 <= t <= duration; per-receiver order; causal order across views
  units      speeds, headings, radio values, lat/lon inside the run's area
  splits     one split per run, partitions agree with runs.parquet, held-out attacks
  manifest   SHA-256 and size of every file, and no unlisted file
  shortcuts  a metadata-only classifier must not separate attacks that are not rate or
             size attacks (ROC-AUC above 0.7 fails)

Exit code: 0 when every selected check passes (or is skipped), 1 when any fails,
2 on a usage error. "--only joins,time" runs a subset, so one check has one exit code.
Reads no clock and draws no random number (the classifier has a fixed seed).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
import zipfile
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pandas as pd
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

VIEWS = ("local", "global", "truth", "ml")
OBSERVER_VIEWS = ("local", "global")
SPLITS = ("train", "validation", "test")

REQUIRED_TABLES = (
    "local/ego", "local/rx", "local/checks", "local/reports", "local/pki",
    "global/reports", "global/decisions", "global/crl",
    "truth/devices", "truth/pseudonyms", "truth/transmissions", "truth/receptions",
    "truth/reports", "truth/timeline", "truth/app_effects",
)

# schema.md column names, used only when a table has no schema/<table>.json.
_F = {
    "runs": "run_id split heldout_attacks city world_source t0_utc duration_s "
            "origin_lat_deg origin_lon_deg vehicles equipped_fraction demand_veh_per_h rat "
            "bsm_rate_hz scms_mode calendar local_suite ma_policy attacker_fraction "
            "attack_mix instrumented_receivers engine_version git_sha scenario_hash seed",
    "local/ego": "run_id receiver_ref t_ns x_m y_m lat_deg lon_deg speed_mps heading_deg "
                 "accel_mps2 pos_accuracy_m own_cert_id",
    "local/rx": "run_id receiver_ref rx_id t_rx_ns spdu_digest rssi_dbm cbr size_bytes psid "
                "cert_id signer_kind issuer_id verify_result verify_latency_us "
                "generation_time_us msg_type msg_cnt temp_id sec_mark_ms lat_deg lon_deg x_m "
                "y_m elevation_m pos_accuracy speed_mps heading_deg accel brakes transmission "
                "length_m width_m event_flags ego_x_m ego_y_m ego_speed_mps ego_heading_deg "
                "range_m",
    "local/checks": "run_id receiver_ref t_ns rx_id cert_id suite check fired score excess",
    "local/reports": "run_id receiver_ref report_id t_created_ns subject_cert_id mbr_type "
                     "subtypes evidence_spdu_digests size_bytes fate t_uploaded_ns path "
                     "weight_at_upload",
    "local/pki": "run_id receiver_ref t_ns event cert_id crl_series crl_number crl_entries "
                 "bytes latency_ms",
    "global/reports": "run_id ma_id report_id t_received_ns generation_time_us "
                      "generation_location psid reporter_cert_id subject_cert_id "
                      "subject_issuer mbr_type subtypes evidence "
                      "evidence_start_us evidence_end_us set_aside la_link la_link_t_ns",
    "global/decisions": "run_id ma_id t_ns subject_kind subject_cert_id la_link decision rule "
                        "report_ids n_reports n_distinct_reporters",
    "global/crl": "run_id t_issued_ns crl_series crl_number entry_kind la_link i_period "
                  "t_effective_ns",
    "truth/devices": "run_id device_id label attack_id la_link receiver_ref",
    "truth/pseudonyms": "run_id cert_id device_id",
    "truth/transmissions": "run_id tx_id device_id spdu_digest label attack_id family "
                           "veremi_ext_code fields_falsified magnitude x_m y_m speed_mps "
                           "heading_deg",
    "truth/receptions": "run_id rx_id tx_id device_id label",
    "truth/reports": "run_id report_id subject_label verdict",
    "truth/timeline": "run_id device_id",
    "truth/app_effects": "run_id app tx_id severity",
}
FALLBACK_COLUMNS = {k: v.split() for k, v in _F.items()}

# Names that are ground truth wherever they appear: the FORBIDDEN_FEATURE_KEYS of
# leakage.rs and its prefix/suffix rules, plus the truth-only columns of schema.md 5 and 8.
FORBIDDEN_KEYS = {
    "attack_family", "attack_id", "attack_type", "attacker_role", "colluding_group_id",
    "falsified", "is_attacker", "is_fake", "is_faulty", "is_vru", "real_id", "realid",
    "report_correctness", "reported_real_id", "reportedrealid", "reporter_true_id",
    "sender_real_id", "senderrealid", "should_have_been_revoked", "subject_true_id",
    "true_accel", "true_heading", "true_linkage_seed", "true_revocation_time", "true_speed",
    "true_vehicle_id", "true_x", "true_y",
    # schema.md truth-only columns
    "device_id", "tx_id", "label", "family", "verdict", "subject_label", "fields_falsified",
    "magnitude", "gnss_error_m", "fix_to_stamp_ms", "veremi_ext_code", "veremi1_code",
    "nextgen_name", "dare_name", "attack_params", "sender_device_id", "reporter_device_id",
    "subject_device_id", "receiver_device_id", "true_distance_m", "driver_reaction",
    "attackertype",
    # The engine's own names for a reception's true transmitter and true distance
    # (phy.rx, node.rx); the release carries them only as truth/receptions'
    # tx_device_id and true_distance_m.
    "tx", "dist_m",
}
TRUTH_VISIBILITY = {"truth", "oracle", "gt", "ground_truth", "ground-truth"}

# Attacks whose definition IS message rate, size or pseudonym statistics: a metadata
# classifier is expected to see them, so the shortcut audit reports but does not flag them.
RATE_OR_SIZE_ATTACKS = {
    "dos", "dosrandom", "disruptive", "oversizedflood", "dosdisruptive", "sybil",
    "ghostvehicles", "datareplay", "relayreplay", "selectivedrop",
    # F2MD's floods under Sybil pseudonyms: a multiplied beacon rate is the attack.
    "dosrandomsybil", "dosdisruptivesybil",
}

SPEED_MAX_TRUE_MPS = 90.0       # own and true speeds: a road vehicle
SPEED_MAX_CLAIMED_MPS = 163.82  # J2735 Speed: 0..8190 x 0.02 m/s (8191 = unavailable)
DEFAULT_AREA_RADIUS_M = 25_000.0
M_PER_DEG = 111_320.0


def is_forbidden_key(name: str) -> bool:
    """is_forbidden_feature_key of leakage.rs, re-implemented, plus the schema.md names."""
    lk = name.strip().lstrip("_").lower()
    norm = lk.replace("_", "")
    if lk in FORBIDDEN_KEYS or norm in FORBIDDEN_KEYS:
        return True
    return (lk.startswith(("true_", "label_", "attack_", "gt_"))
            or lk.endswith(("_true_id", "real_id")) or norm.endswith(("realid", "trueid"))
            or lk == "vehicleid_true")


def attack_key(a) -> str:
    """threat/attacker/legacy/ConstPos, const-pos and ConstPos are one attack."""
    if a is None or (isinstance(a, float) and math.isnan(a)):
        return ""
    return re.sub(r"[^a-z0-9]", "", str(a).rsplit("/", 1)[-1].lower())


# ---------------------------------------------------------------------------------------
# Reading the release


@dataclass
class Part:
    split: str | None
    run: str | None
    path: Path


@dataclass
class Result:
    name: str
    ok: bool | None          # None = skipped
    verdict: str
    problems: list[str] = field(default_factory=list)

    def line(self) -> str:
        tag = "SKIP" if self.ok is None else ("PASS" if self.ok else "FAIL")
        return f"{tag} {self.name}: {self.verdict}"


def _fail(name: str, problems: list[str], what: str) -> Result:
    more = f" (+{len(problems) - 3} more)" if len(problems) > 3 else ""
    return Result(name, False, f"{len(problems)} {what}: " + "; ".join(problems[:3]) + more,
                  problems)


class Release:
    def __init__(self, root: Path):
        self.root = Path(root)
        self.tables: dict[str, list[Part]] = {}
        for view in VIEWS:
            vdir = self.root / view
            if not vdir.is_dir():
                continue
            for tdir in sorted(p for p in vdir.iterdir() if p.is_dir()):
                parts = []
                for f in sorted(tdir.rglob("*.parquet")):
                    kv = dict(seg.split("=", 1) for seg in f.relative_to(tdir).parts[:-1]
                              if "=" in seg)
                    parts.append(Part(kv.get("split"), kv.get("run"), f))
                self.tables[f"{view}/{tdir.name}"] = parts
        rp = self.root / "runs.parquet"
        self.runs = pq.read_table(rp).to_pandas() if rp.is_file() else pd.DataFrame()
        self._schemas: dict[str, dict | None] = {}

    def run_ids(self) -> list[str]:
        return sorted(self.runs["run_id"].astype(str)) if "run_id" in self.runs else []

    def run_row(self, run: str) -> dict:
        r = self.runs[self.runs["run_id"].astype(str) == run]
        return r.iloc[0].to_dict() if len(r) else {}

    def duration_ns(self, run: str) -> float | None:
        d = self.run_row(run).get("duration_s")
        return None if d is None or pd.isna(d) else float(d) * 1e9

    def has(self, table: str) -> bool:
        return bool(self.tables.get(table))

    def files(self, table: str, run: str | None = None) -> list[Path]:
        return [p.path for p in self.tables.get(table, []) if run is None or p.run == run]

    def arrow(self, table: str, run: str | None = None, columns=None) -> pa.Table | None:
        files = self.files(table, run)
        if not files:
            return None
        out = []
        for f in files:
            pf = pq.ParquetFile(f)
            names = pf.schema_arrow.names
            cols = None if columns is None else [c for c in columns if c in names]
            out.append(pf.read(columns=cols))
        return pa.concat_tables(out, promote_options="permissive") if len(out) > 1 else out[0]

    def df(self, table: str, run: str | None = None, columns=None) -> pd.DataFrame:
        t = self.arrow(table, run, columns)
        if t is None:
            return pd.DataFrame(columns=list(columns or []))
        return t.to_pandas()

    def schema(self, table: str) -> dict | None:
        if table not in self._schemas:
            found = None
            for cand in (self.root / "schema" / f"{table.replace('/', '_')}.json",
                         self.root / "schema" / f"{table}.json"):
                if cand.is_file():
                    found = json.loads(cand.read_text())
                    break
            self._schemas[table] = found
        return self._schemas[table]


def values(tbl: pa.Table | None, path: str) -> pa.Array:
    """The leaf values at a dotted path through lists and structs (evidence.spdu_digest)."""
    if tbl is None:
        return pa.array([], pa.string())
    head, *rest = path.split(".")
    if head not in tbl.column_names:
        return pa.array([], pa.string())
    arr = tbl.column(head).combine_chunks()
    for name in rest + [None]:
        while pa.types.is_list(arr.type) or pa.types.is_large_list(arr.type):
            arr = pc.list_flatten(arr)
        if name is None:
            break
        if not pa.types.is_struct(arr.type):
            return pa.array([], pa.string())
        arr = arr.field(name)
    return arr


def leaves(typ: pa.DataType, prefix: str):
    """Every (dotted name, leaf type) under a column type."""
    while pa.types.is_list(typ) or pa.types.is_large_list(typ):
        typ = typ.value_type
    if pa.types.is_struct(typ):
        for f in typ:
            yield from leaves(f.type, f"{prefix}.{f.name}")
    else:
        yield prefix, typ


def strset(arr: pa.Array) -> set[str]:
    return {v for v in pc.unique(pc.cast(arr, pa.string())).to_pylist() if v is not None}


def nums(arr) -> np.ndarray:
    v = np.asarray(arr.to_numpy(zero_copy_only=False), dtype=float)
    return v[~np.isnan(v)]


# ---------------------------------------------------------------------------------------
# Types

_ALIASES = {"double": "float64", "float": "float32", "halffloat": "float16",
            "boolean": "bool", "utf8": "string", "large_string": "string",
            "large_utf8": "string", "str": "string", "long": "int64", "integer": "int64",
            "int": "int64", "large_list": "list", "largeutf8": "string"}


def arrow_type_str(t: pa.DataType) -> str:
    if pa.types.is_list(t) or pa.types.is_large_list(t):
        return f"list<{arrow_type_str(t.value_type)}>"
    if pa.types.is_struct(t):
        return "struct<" + ",".join(f"{f.name}:{arrow_type_str(f.type)}" for f in t) + ">"
    if pa.types.is_dictionary(t):
        return arrow_type_str(t.value_type)
    s = str(t)
    return _ALIASES.get(s, s)


def _rs_type(s: str) -> str:
    """The Rust writer's arrow-rs DataType text (`List(Field { name: "item", data_type:
    Utf8, .. })`, `Struct(a Float64, b Int64)`, `Struct([Field { .. }])`) in the canonical
    form arrow_type_str prints."""
    pos = 0

    def ws():
        nonlocal pos
        while pos < len(s) and s[pos] in " \t\n,":
            pos += 1

    def eat(c):
        nonlocal pos
        ws()
        if not s.startswith(c, pos):
            raise ValueError(f"expected {c!r} at {pos} in {s!r}")
        pos += len(c)

    def ident():
        nonlocal pos
        ws()
        m = re.compile(r'"?([A-Za-z0-9_]+)"?:?').match(s, pos)
        if not m:
            raise ValueError(f"expected a name at {pos} in {s!r}")
        pos = m.end()
        return m.group(1)

    def skip_value():
        nonlocal pos
        ws()
        depth, quoted = 0, False
        while pos < len(s):
            c = s[pos]
            if quoted:
                quoted = c != '"' or s[pos - 1] == "\\"
            elif c == '"':
                quoted = True
            elif c in "{([":
                depth += 1
            elif c in "})]":
                if depth == 0:
                    return
                depth -= 1
            elif c == "," and depth == 0:
                return
            pos += 1

    def field():
        nonlocal pos
        eat("Field")
        eat("{")
        name = typ = None
        while True:
            ws()
            if s.startswith("}", pos):
                pos += 1
                return name, typ
            key = ident()
            if key == "name":
                ws()
                m = re.compile(r'"([^"]*)"').match(s, pos)
                name, pos = m.group(1), m.end()
            elif key == "data_type":
                typ = parse()
            else:
                skip_value()

    def parse():
        nonlocal pos
        name = ident()
        ws()
        if not s.startswith("(", pos):
            low = name.lower()
            return _ALIASES.get(low, low)
        pos += 1
        if name in ("List", "LargeList", "FixedSizeList"):
            _, inner = field()
            skip_value()
            eat(")")
            return f"list<{inner}>"
        if name == "Struct":
            fields = []
            ws()
            if s.startswith("[", pos):
                pos += 1
                while (ws() or True) and not s.startswith("]", pos):
                    fields.append(field())
                pos += 1
            else:
                while (ws() or True) and not s.startswith(")", pos):
                    fname = ident()
                    fields.append((fname, parse()))
            eat(")")
            return "struct<" + ",".join(f"{n}:{t}" for n, t in fields) + ">"
        if name == "Dictionary":
            parse()
            inner = parse()
            eat(")")
            return inner
        skip_value()
        eat(")")
        return name.lower()

    out = parse()
    ws()
    if pos != len(s):
        raise ValueError(f"trailing text at {pos} in {s!r}")
    return out


def norm_type_str(s: str) -> str:
    s = str(s)
    if re.match(r"\s*[A-Z]", s):  # arrow-rs text from the Rust writer
        try:
            s = _rs_type(s)
        except (ValueError, AttributeError):
            pass
    s = re.sub(r"\s+", "", s.lower())
    s = re.sub(r"(list|large_list)<(item|element):", "list<", s)
    # a word followed by a colon is a struct field name; any other word is a type name
    return re.sub(r"[a-z_][a-z0-9_]*(?![a-z0-9_:])",
                  lambda m: _ALIASES.get(m.group(0), m.group(0)), s)


def schema_columns(spec: dict) -> list[dict]:
    out = []
    for c in spec.get("columns", spec.get("fields", [])):
        out.append({"name": c.get("name"),
                    "type": c.get("type", c.get("dtype", c.get("arrow_type"))),
                    "nullable": c.get("nullable", True),
                    "visibility": c.get("visibility")})
    return out


# ---------------------------------------------------------------------------------------
# Checks


def check_schema(rel: Release) -> Result:
    """Every required table present; columns, types and nullability per schema/<table>.json."""
    name, problems = "schema", []
    if rel.runs.empty:
        problems.append("runs.parquet missing or empty")
    for t in REQUIRED_TABLES:
        if not rel.has(t):
            problems.append(f"{t}: no partitions")
    tables = ["runs"] + sorted(rel.tables)
    n_cols, fallback = 0, []
    for t in tables:
        files = [rel.root / "runs.parquet"] if t == "runs" else rel.files(t)
        files = [f for f in files if f.is_file()]
        if not files:
            continue
        spec = rel.schema(t)
        if spec is None:
            if t in FALLBACK_COLUMNS:
                fallback.append(t)
            elif not t.startswith("ml/"):
                problems.append(f"{t}: no schema/{t.replace('/', '_')}.json")
        cols = schema_columns(spec) if spec else [{"name": c, "type": None, "nullable": True}
                                                  for c in FALLBACK_COLUMNS.get(t, [])]
        want = {c["name"]: c for c in cols}
        for f in files:
            pf = pq.ParquetFile(f)
            have = {fld.name: fld.type for fld in pf.schema_arrow}
            rel_f = f.relative_to(rel.root)
            missing = [c for c in want if c not in have]
            if missing:
                problems.append(f"{rel_f}: missing {missing[:4]}")
            if spec:
                extra = [c for c in have if c not in want]
                if extra:
                    problems.append(f"{rel_f}: undeclared {extra[:4]}")
            tbl = None
            for c, meta in want.items():
                if c not in have:
                    continue
                if meta.get("type"):
                    got, exp = arrow_type_str(have[c]), norm_type_str(meta["type"])
                    if got != exp:
                        problems.append(f"{rel_f}: {c} is {got}, schema says {exp}")
                if meta.get("nullable") is False:
                    tbl = tbl if tbl is not None else pf.read()
                    nulls = tbl.column(c).null_count
                    if nulls:
                        problems.append(f"{rel_f}: {c} has {nulls} nulls, declared non-null")
            n_cols += len(have)
    if problems:
        return _fail(name, problems, "schema problem(s)")
    fb = f"; schema.md fallback for {len(fallback)} table(s)" if fallback else ""
    return Result(name, True, f"{len(tables)} tables, {n_cols} file columns conform{fb}")


def check_joins(rel: Release) -> Result:
    """Evidence digests, rx ids and report ids resolve between local, global and truth."""
    name, problems, n = "joins", [], 0

    def need(sub: set, sup: set, what: str, run: str):
        nonlocal n
        n += len(sub)
        miss = sorted(sub - sup)
        if miss:
            problems.append(f"{run}: {len(miss)} {what} (e.g. {miss[0]})")

    for run in rel.run_ids():
        def a(t, cols):
            return rel.arrow(t, run, cols)
        tx = a("truth/transmissions", ["tx_id", "spdu_digest", "device_id"])
        trx = a("truth/receptions", ["rx_id", "tx_id"])
        dev = strset(values(a("truth/devices", ["device_id"]), "device_id"))
        digests = strset(values(tx, "spdu_digest"))
        g = a("global/reports", ["report_id", "evidence"])
        lrep = a("local/reports", ["report_id", "fate", "evidence_spdu_digests"])
        trep = strset(values(a("truth/reports", ["report_id"]), "report_id"))
        need(strset(values(g, "evidence.spdu_digest")), digests,
             "global/reports evidence spdu_digest not in truth/transmissions", run)
        need(strset(values(lrep, "evidence_spdu_digests")), digests,
             "local/reports evidence digest not in truth/transmissions", run)
        rx_truth = strset(values(trx, "rx_id"))
        need(strset(values(a("local/rx", ["rx_id"]), "rx_id")), rx_truth,
             "local/rx rx_id not in truth/receptions", run)
        need(strset(values(a("local/checks", ["rx_id"]), "rx_id")), rx_truth,
             "local/checks rx_id not in truth/receptions", run)
        need(strset(values(trx, "tx_id")), strset(values(tx, "tx_id")),
             "truth/receptions tx_id not in truth/transmissions", run)
        need(strset(values(tx, "device_id")), dev,
             "truth/transmissions device_id not in truth/devices", run)
        need(strset(values(a("truth/pseudonyms", ["device_id"]), "device_id")), dev,
             "truth/pseudonyms device_id not in truth/devices", run)
        gids = strset(values(g, "report_id"))
        lids = strset(values(lrep, "report_id"))
        uploaded = set()
        if lrep is not None and "fate" in lrep.column_names:
            uploaded = strset(values(lrep.filter(pc.equal(lrep.column("fate"), "uploaded")),
                                     "report_id"))
        need(gids, uploaded, "global report_id not an uploaded local/reports row", run)
        # An uploaded report left its device as bytes: its row names them (the first v1.1
        # build left 1,083 uploads with no report_id and no evidence).
        if lrep is not None and "fate" in lrep.column_names:
            up = lrep.filter(pc.equal(lrep.column("fate"), "uploaded"))
            n += up.num_rows
            no_id = up.num_rows - pc.count(up.column("report_id")).as_py()
            ev_len = pc.fill_null(pc.list_value_length(up.column("evidence_spdu_digests")), 0)
            no_ev = int(pc.sum(pc.equal(ev_len, 0)).as_py() or 0)
            if no_id or no_ev:
                problems.append(f"{run}: uploaded local/reports rows without their bytes: "
                                f"{no_id} with no report_id, {no_ev} with no evidence")
        need(lids, trep, "local/reports report_id not in truth/reports", run)
        need(trep, lids, "truth/reports report_id not in local/reports", run)
        need(strset(values(a("global/decisions", ["report_ids"]), "report_ids")), gids,
             "global/decisions report_id not in global/reports", run)
    if problems:
        return _fail(name, problems, "broken join(s)")
    return Result(name, True, f"{n} keys resolve across local, global and truth")


def _truth_only_values(rel: Release) -> set[str]:
    vals: set[str] = set()
    for t, col in (("truth/devices", "device_id"), ("truth/transmissions", "tx_id"),
                   ("truth/reports", "reporter_device_id"),
                   ("truth/reports", "subject_device_id")):
        vals |= strset(values(rel.arrow(t, None, [col]), col))
    return {v for v in vals if len(v) >= 3}


def _json_items(obj):
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield k, v
            yield from _json_items(v)
    elif isinstance(obj, list):
        for v in obj:
            yield from _json_items(v)


def check_leakage(rel: Release) -> Result:
    """No truth column name, truth visibility tag, or truth-only id value in local/global
    (and none in the DARE compat report files)."""
    name, problems = "leakage", []
    secret = _truth_only_values(rel)
    n_cols = 0
    for t in sorted(rel.tables):
        if t.split("/")[0] not in OBSERVER_VIEWS:
            continue
        spec = rel.schema(t)
        for c in schema_columns(spec) if spec else []:
            if str(c.get("visibility") or "").lower() in TRUTH_VISIBILITY:
                problems.append(f"{t}: schema tags {c['name']} visibility {c['visibility']}")
        named = set()
        for f in rel.files(t):
            pf = pq.ParquetFile(f)
            tbl = None
            for fld in pf.schema_arrow:
                for leaf, ltyp in leaves(fld.type, fld.name):
                    n_cols += 1
                    last = leaf.rsplit(".", 1)[-1]
                    if leaf not in named and (is_forbidden_key(last) or is_forbidden_key(fld.name)):
                        problems.append(f"{t}: truth column {leaf}")
                    named.add(leaf)
                    is_str = pa.types.is_string(ltyp) or pa.types.is_large_string(ltyp)
                    is_idint = pa.types.is_integer(ltyp) and re.search(r"(^|_)(id|ref)$", last)
                    if secret and (is_str or is_idint):
                        tbl = tbl if tbl is not None else pf.read()
                        hit = strset(values(tbl, leaf)) & secret
                        if hit:
                            problems.append(f"{f.relative_to(rel.root)}: {leaf} carries "
                                            f"truth-only id value {sorted(hit)[0]}")
    for z in sorted((rel.root / "compat" / "dare").glob("*.zip")):
        with zipfile.ZipFile(z) as zf:
            for member in zf.namelist():
                if not member.startswith("reports/") or not member.endswith(".json"):
                    continue
                for k, v in _json_items(json.loads(zf.read(member))):
                    if is_forbidden_key(k):
                        problems.append(f"{z.name}:{member}: truth key {k}")
                    if isinstance(v, str) and v in secret:
                        problems.append(f"{z.name}:{member}: truth-only id value {v}")
    if problems:
        return _fail(name, problems, "leak(s)")
    return Result(name, True, f"{n_cols} observer columns clean; {len(secret)} truth-only "
                              f"id values absent from local/ and global/")


def _mix(row: dict) -> list[dict]:
    m = row.get("attack_mix")
    if m is None or isinstance(m, float):
        return []
    if isinstance(m, str):
        m = json.loads(m)
    return [dict(x) for x in list(m)]


def check_labels(rel: Release) -> Result:
    """Malicious messages carry an attack, benign ones none; devices, receptions and reports
    agree with them; attacker counts per run match runs.attack_mix within rounding."""
    name, problems, n = "labels", [], 0
    for run in rel.run_ids():
        dev = rel.df("truth/devices", run)
        tx = rel.df("truth/transmissions", run)
        if "t_tx_ns" not in tx and "t_ns" in tx:  # the Rust writer's name for the air time
            tx = tx.rename(columns={"t_ns": "t_tx_ns"})
        tx = tx[[c for c in ("tx_id", "device_id", "label", "attack_id", "t_tx_ns") if c in tx]]
        n += len(tx)
        for t, d in (("truth/transmissions", tx), ("truth/devices", dev)):
            if "label" not in d or "attack_id" not in d:
                problems.append(f"{run}: {t} lacks label/attack_id")
                continue
            bad = set(d["label"].dropna()) - {"benign", "faulty", "malicious"}
            if bad:
                problems.append(f"{run}: {t} label vocabulary {sorted(bad)}")
            mal = d["label"] == "malicious"
            has = d["attack_id"].notna()
            if (mal & ~has).any():
                problems.append(f"{run}: {int((mal & ~has).sum())} malicious {t} rows "
                                f"without attack_id")
            if (~mal & has).any():
                problems.append(f"{run}: {int((~mal & has).sum())} non-malicious {t} rows "
                                f"with attack_id")
        if {"device_id", "label", "attack_id"} <= set(dev) and len(tx):
            m = tx[tx["label"] == "malicious"]
            if len(m):
                dl = dev.set_index("device_id").reindex(m["device_id"])
                wrong = ((dl["label"].values != "malicious")
                         | (dl["attack_id"].map(attack_key).values
                            != m["attack_id"].map(attack_key).values))
                if wrong.any():
                    problems.append(f"{run}: {int(wrong.sum())} malicious messages from a "
                                    f"device without that attack")
                if {"attack_start_ns", "attack_end_ns"} <= set(dev) and "t_tx_ns" in m:
                    s = dl["attack_start_ns"].astype(float).values
                    e = dl["attack_end_ns"].astype(float).fillna(np.inf).values
                    t = m["t_tx_ns"].astype(float).values
                    out = (t < s) | (t > e)
                    if out.any():
                        problems.append(f"{run}: {int(out.sum())} malicious messages outside "
                                        f"their device attack window")
        rcp = rel.df("truth/receptions", run, ["rx_id", "tx_id", "label", "device_id"])
        if len(rcp) and len(tx) and "label" in rcp:
            j = rcp.merge(tx[["tx_id", "label", "device_id"]], on="tx_id", how="left",
                          suffixes=("", "_tx"))
            bad = int((j["label"] != j["label_tx"]).sum())
            if "device_id" in j and "device_id_tx" in j:
                bad += int((j["device_id"] != j["device_id_tx"]).sum())
            if bad:
                problems.append(f"{run}: {bad} truth/receptions disagree with the label or "
                                f"sender of their transmission")
        trep = rel.df("truth/reports", run)
        if len(trep) and {"subject_label", "verdict"} <= set(trep):
            bad = (((trep["subject_label"] == "malicious") & (trep["verdict"] != "true_accusation"))
                   | ((trep["subject_label"] == "benign") & (trep["verdict"] != "false_accusation")))
            if bad.any():
                problems.append(f"{run}: {int(bad.sum())} report verdicts contradict "
                                f"subject_label")
            if "subject_device_id" in trep and "device_id" in dev:
                sl = trep["subject_device_id"].map(dev.set_index("device_id")["label"])
                if (sl != trep["subject_label"]).any():
                    problems.append(f"{run}: truth/reports subject_label differs from "
                                    f"truth/devices label")
        # A poisoned report is a forgery an attacker filed: its reporter is malicious.
        if len(trep) and "poisoned" in trep and {"reporter_device_id"} <= set(trep) \
                and {"device_id", "label"} <= set(dev):
            p = trep[trep["poisoned"].fillna(False).astype(bool)]
            rl = p["reporter_device_id"].map(dev.set_index("device_id")["label"])
            bad = int((rl != "malicious").sum())
            if bad:
                problems.append(f"{run}: {bad} poisoned reports filed by a device that is "
                                f"not malicious")
        # Per attack: a fraction of the vehicles, or a count when the scenario names them.
        # One attack can be several populations (one per start time): their vehicles add up.
        mix: dict[str, list] = {}
        for x in _mix(rel.run_row(run)):
            mix.setdefault(attack_key(x.get("id")), []).append(
                (x.get("count"), float(x.get("fraction") or 0)))
        if len(dev) and "label" in dev:
            pool = dev[dev["class"] == "vehicle"] if "class" in dev else dev
            nveh = len(pool)
            counts = pool[pool["label"] == "malicious"]["attack_id"].map(attack_key).value_counts()
            for a, c in counts.items():
                if a not in mix:
                    problems.append(f"{run}: attack {a} on {c} devices, absent from attack_mix")
            for a, pops in mix.items():
                lo = hi = 0
                said = []
                for k, f in pops:
                    named = k is not None and k == k
                    exp = float(k) if named else f * nveh
                    lo += math.floor(exp + 1e-9)
                    hi += math.ceil(exp - 1e-9)
                    said.append(f"count {int(k)}" if named else f"{f:g}")
                c = int(counts.get(a, 0))
                if not lo <= c <= hi:
                    problems.append(f"{run}: {a} on {c} of {nveh} vehicles, attack_mix says "
                                    f"{' + '.join(said)} ({lo}-{hi})")
    if problems:
        return _fail(name, problems, "label problem(s)")
    return Result(name, True, f"{n} messages, devices, receptions and reports consistent; "
                              f"attack_mix fractions hold")


_TIME_COL = re.compile(r"^(t|t_.*|.*_t)_ns$")
_MONOTONIC = {  # table: (time column, per-observer key, strictly increasing)
    "local/ego": ("t_ns", "receiver_ref", True),
    "local/rx": ("t_rx_ns", "receiver_ref", False),
    "local/checks": ("t_ns", "receiver_ref", False),
    "local/reports": ("t_created_ns", "receiver_ref", False),
    "local/pki": ("t_ns", "receiver_ref", False),
    "global/reports": ("t_received_ns", "ma_id", False),
    "global/decisions": ("t_ns", "ma_id", False),
    "global/crl": ("t_issued_ns", None, False),
}


def check_time(rel: Release) -> Result:
    """Every event time inside the run; per-receiver order in file order; causes before
    effects (tx before rx, rx before its check, upload before arrival, reports before the
    decision that cites them)."""
    name, problems, n = "time", [], 0
    for run in rel.run_ids():
        dur = rel.duration_ns(run)
        if dur is None:
            problems.append(f"{run}: runs.duration_s missing")
            continue
        for t in sorted(rel.tables):
            if t.startswith("ml/"):
                continue
            tbl = rel.arrow(t, run)
            if tbl is None:
                continue
            for c in tbl.column_names:
                if not _TIME_COL.match(c):
                    continue
                v = nums(tbl.column(c))
                n += len(v)
                bad = (v < 0) | (v > dur)
                if bad.any():
                    problems.append(f"{run}: {t}.{c} {int(bad.sum())} values outside "
                                    f"[0, {dur / 1e9:g} s] (e.g. {v[bad][0]:.0f})")
            if t in _MONOTONIC:
                col, key, strict = _MONOTONIC[t]
                if col not in tbl.column_names:
                    continue
                d = tbl.select([c for c in (col, key) if c and c in tbl.column_names]).to_pandas()
                groups = d.groupby(key, sort=False)[col] if key in d else [(None, d[col])]
                for k, s in groups:
                    diff = np.diff(s.astype(float).values)
                    if (diff < 0).any() or (strict and (diff == 0).any()):
                        problems.append(f"{run}: {t}.{col} not {'strictly ' if strict else ''}"
                                        f"increasing for {key}={k}")
                        break
        tx = rel.df("truth/transmissions", run, ["tx_id", "t_tx_ns"])
        rcp = rel.df("truth/receptions", run, ["rx_id", "tx_id"])
        rx = rel.df("local/rx", run, ["rx_id", "t_rx_ns"])
        if len(rx) and "t_tx_ns" in tx and len(rcp):
            j = rx.merge(rcp, on="rx_id").merge(tx, on="tx_id")
            bad = int((j["t_rx_ns"] < j["t_tx_ns"]).sum())
            if bad:
                problems.append(f"{run}: {bad} receptions before their transmission")
        ch = rel.df("local/checks", run, ["rx_id", "t_ns"])
        if len(ch) and len(rx):
            j = ch.merge(rx, on="rx_id")
            bad = int((j["t_ns"] < j["t_rx_ns"]).sum())
            if bad:
                problems.append(f"{run}: {bad} checks before their reception")
        lr = rel.df("local/reports", run, ["report_id", "t_created_ns", "t_uploaded_ns"])
        gr = rel.df("global/reports", run, ["report_id", "t_received_ns"])
        if len(lr) and "t_uploaded_ns" in lr:
            bad = int((lr["t_uploaded_ns"] < lr["t_created_ns"]).sum())
            if bad:
                problems.append(f"{run}: {bad} reports uploaded before created")
            j = gr.merge(lr, on="report_id")
            bad = int((j["t_received_ns"] < j["t_uploaded_ns"]).sum())
            if bad:
                problems.append(f"{run}: {bad} reports at the MA before upload")
        dec = rel.df("global/decisions", run, ["t_ns", "report_ids"])
        if len(dec) and len(gr) and "report_ids" in dec:
            tr = dict(zip(gr["report_id"], gr["t_received_ns"]))
            for t_ns, ids in zip(dec["t_ns"], dec["report_ids"]):
                ts = [tr[r] for r in (list(ids) if ids is not None else []) if r in tr]
                if ts and t_ns < max(ts):
                    problems.append(f"{run}: decision at {t_ns} ns before a report it cites")
                    break
    if problems:
        return _fail(name, problems, "time problem(s)")
    return Result(name, True, f"{n} timestamps inside their run; order and causality hold")


def check_units(rel: Release) -> Result:
    """Speeds 0..90 m/s for own and true state (0..163.82, the J2735 range, for claimed
    values), headings 0..360, radio values in range, lat/lon inside the area of the run."""
    name, problems, n = "units", [], 0

    def rng(run, where, arr, lo, hi, unit):
        nonlocal n
        v = nums(arr)
        n += len(v)
        bad = (v < lo) | (v > hi)
        if bad.any():
            problems.append(f"{run}: {where} {int(bad.sum())} values outside "
                            f"[{lo:.6g}, {hi:.6g}] {unit} (e.g. {v[bad][0]:.6g})")

    for run in rel.run_ids():
        row = rel.run_row(run)
        lat0, lon0 = row.get("origin_lat_deg"), row.get("origin_lon_deg")
        if lat0 is None or lon0 is None or pd.isna(lat0) or pd.isna(lon0):
            problems.append(f"{run}: runs origin_lat_deg/origin_lon_deg missing")
            continue
        lat0, lon0 = float(lat0), float(lon0)
        radius = row.get("area_radius_m")
        radius = DEFAULT_AREA_RADIUS_M if radius is None or pd.isna(radius) else float(radius)
        dlat = radius / M_PER_DEG
        dlon = radius / (M_PER_DEG * max(math.cos(math.radians(lat0)), 1e-6))
        for t in sorted(rel.tables):
            if t.startswith("ml/"):
                continue
            tbl = rel.arrow(t, run)
            if tbl is None:
                continue
            claimed = t in ("local/rx", "global/reports")
            for fld in tbl.schema:
                for leaf, ltyp in leaves(fld.type, fld.name):
                    if not (pa.types.is_integer(ltyp) or pa.types.is_floating(ltyp)):
                        continue
                    last, w = leaf.rsplit(".", 1)[-1], f"{t}.{leaf}"
                    arr = values(tbl, leaf)
                    if last.endswith("speed_mps"):
                        own = last.startswith("ego_") or not claimed
                        rng(run, w, arr, 0, SPEED_MAX_TRUE_MPS if own else SPEED_MAX_CLAIMED_MPS,
                            "m/s")
                    elif last.endswith("heading_deg") or last == "orientation_deg":
                        rng(run, w, arr, 0, 360, "deg")
                    elif last.endswith("lat_deg"):
                        rng(run, w, arr, lat0 - dlat, lat0 + dlat, "deg")
                    elif last.endswith("lon_deg"):
                        rng(run, w, arr, lon0 - dlon, lon0 + dlon, "deg")
                    elif last in ("x_m", "y_m", "ego_x_m", "ego_y_m"):
                        rng(run, w, arr, -radius, radius, "m")
                    elif last == "rssi_dbm":
                        rng(run, w, arr, -130, 0, "dBm")
                    elif last == "cbr":
                        rng(run, w, arr, 0, 1, "")
                    elif last == "size_bytes" and t == "local/rx":
                        rng(run, w, arr, 1, 2304, "bytes")
                    elif last.endswith("accuracy_m") or last in ("semi_major_m", "semi_minor_m"):
                        rng(run, w, arr, 0, 1000, "m")
    if problems:
        return _fail(name, problems, "unit problem(s)")
    return Result(name, True, f"{n} physical values in range and inside the area of each run")


def check_splits(rel: Release) -> Result:
    """Each run in one split; partition paths agree with runs.parquet and with the run_id
    inside; held-out attacks absent from train; no scenario+seed in two splits."""
    name, problems = "splits", []
    if rel.runs.empty:
        return _fail(name, ["runs.parquet missing"], "split problem(s)")
    runs = rel.runs
    dup = runs["run_id"][runs["run_id"].duplicated()].tolist()
    if dup:
        problems.append(f"run_id repeated in runs.parquet: {dup[:3]}")
    split_of = dict(zip(runs["run_id"].astype(str), runs["split"].astype(str)))
    bad = sorted(set(split_of.values()) - set(SPLITS))
    if bad:
        problems.append(f"unknown split names {bad}")
    seen: dict[str, set[str]] = {}
    for t, parts in sorted(rel.tables.items()):
        for p in parts:
            if p.split is None or p.run is None:
                problems.append(f"{p.path.relative_to(rel.root)}: not under split=/run=")
                continue
            seen.setdefault(p.run, set()).add(p.split)
            if p.run not in split_of:
                problems.append(f"{t}: run={p.run} not in runs.parquet")
            elif split_of[p.run] != p.split:
                problems.append(f"{t}: run={p.run} under split={p.split}, runs says "
                                f"{split_of[p.run]}")
            pf = pq.ParquetFile(p.path)
            ids = strset(values(pf.read(columns=["run_id"]), "run_id")) \
                if "run_id" in pf.schema_arrow.names else set()
            if ids - {p.run}:
                problems.append(f"{p.path.relative_to(rel.root)}: holds run_id "
                                f"{sorted(ids - {p.run})[:2]}")
    for r, s in sorted(seen.items()):
        if len(s) > 1:
            problems.append(f"run {r} appears in splits {sorted(s)}")
    if {"scenario_hash", "seed"} <= set(runs):
        key = runs["scenario_hash"].astype(str) + "/" + runs["seed"].astype(str)
        for k, grp in runs.groupby(key):
            if grp["split"].nunique() > 1:
                problems.append(f"same scenario+seed {k[:24]} in splits {sorted(grp['split'])}")
    train_attacks = set()
    for _, r in runs[runs["split"] == "train"].iterrows():
        train_attacks |= {attack_key(x.get("id")) for x in _mix(r.to_dict())}
    named_heldout = 0
    for _, r in runs.iterrows():
        held = r.get("heldout_attacks")
        for a in (list(held) if held is not None and not isinstance(held, float) else []):
            named_heldout += 1
            if attack_key(a) in train_attacks:
                problems.append(f"{r['run_id']}: held-out attack {a} appears in a train run")
    if problems:
        return _fail(name, problems, "split problem(s)")
    counts = runs["split"].value_counts().to_dict()
    # A one-run or one-split release has nothing to compare across splits: say so rather
    # than claim a property that was never tested.
    held_txt = ("held-out attacks absent from train" if named_heldout else
                "held-out: not applicable (no run names a held-out attack)")
    seed_txt = ("no scenario+seed in two splits" if len(counts) > 1 else
                "scenario+seed across splits: not applicable (one split)")
    return Result(name, True, "runs per split " + ", ".join(f"{k}={v}" for k, v in
                                                          sorted(counts.items()))
                  + f"; partitions agree; {held_txt}; {seed_txt}")


MANIFEST_NAME = "manifest.json"


def check_manifest(rel: Release) -> Result:
    """manifest.json lists every file with its SHA-256 and size, and nothing else."""
    name, problems = "manifest", []
    mp = rel.root / MANIFEST_NAME
    if not mp.is_file():
        return _fail(name, [f"{MANIFEST_NAME} missing"], "manifest problem(s)")
    man = json.loads(mp.read_text())
    entries = man.get("files", man.get("outputs"))
    if not isinstance(entries, list):
        return _fail(name, ["manifest has no files list"], "manifest problem(s)")
    listed = {e["path"]: e for e in entries}
    on_disk = {p.relative_to(rel.root).as_posix() for p in rel.root.rglob("*")
               if p.is_file() and p.relative_to(rel.root).as_posix() != MANIFEST_NAME
               and p.name != ".DS_Store" and ".cache" not in p.parts}
    for missing in sorted(set(listed) - on_disk):
        problems.append(f"listed but absent: {missing}")
    for extra in sorted(on_disk - set(listed)):
        problems.append(f"not in manifest: {extra}")
    total = 0
    for pth in sorted(set(listed) & on_disk):
        data = (rel.root / pth).read_bytes()
        total += len(data)
        if hashlib.sha256(data).hexdigest() != str(listed[pth].get("sha256", "")).lower():
            problems.append(f"sha256 mismatch: {pth}")
        if "bytes" in listed[pth] and int(listed[pth]["bytes"]) != len(data):
            problems.append(f"size mismatch: {pth}")
    if problems:
        return _fail(name, problems, "manifest problem(s)")
    return Result(name, True, f"{len(listed)} files, {total / 1e6:.2f} MB, every SHA-256 matches")


SHORTCUT_FEATURES = ["size_bytes", "msg_cnt", "signer_is_cert", "gap_ms", "cert_rows",
                     "cert_span_s", "cert_rate_hz"]


def shortcut_frame(rel: Release) -> pd.DataFrame:
    """local/rx metadata features joined with truth labels, one row per reception."""
    frames = []
    for run in rel.run_ids():
        rx = rel.df("local/rx", run, ["rx_id", "receiver_ref", "cert_id", "t_rx_ns",
                                      "size_bytes", "msg_cnt", "signer_kind"])
        if rx.empty:
            continue
        rcp = rel.df("truth/receptions", run, ["rx_id", "tx_id"])
        tx = rel.df("truth/transmissions", run, ["tx_id", "label", "attack_id"])
        d = rx.merge(rcp, on="rx_id", how="inner").merge(tx, on="tx_id", how="inner")
        d = d.sort_values(["receiver_ref", "cert_id", "t_rx_ns"], kind="stable")
        g = d.groupby(["receiver_ref", "cert_id"], sort=False)["t_rx_ns"]
        d["gap_ms"] = (g.diff() / 1e6).fillna(-1.0)
        d["cert_rows"] = g.transform("size").astype(float)
        span = (g.transform("max") - g.transform("min")) / 1e9
        d["cert_span_s"] = span
        d["cert_rate_hz"] = (d["cert_rows"] / span.where(span > 0)).fillna(0.0)
        kind = d["signer_kind"] if "signer_kind" in d else pd.Series("", index=d.index)
        d["signer_is_cert"] = kind.astype(str).str.lower().str.startswith("cert").astype(float)
        d["group"] = run + ":" + d["cert_id"].astype(str)
        frames.append(d)
    return pd.concat(frames, ignore_index=True) if frames else pd.DataFrame()


def check_shortcuts(rel: Release, max_rows: int = 200_000) -> Result:
    """A classifier on metadata only (size, msg_cnt, signer kind, per-pseudonym rate and
    counts) must stay at or below ROC-AUC 0.7 for attacks that are not rate or size attacks.
    Folds are grouped by pseudonym, so a model cannot memorise one attacker certificate."""
    name = "shortcuts"
    try:
        from sklearn.ensemble import RandomForestClassifier
        from sklearn.metrics import roc_auc_score
        from sklearn.model_selection import StratifiedGroupKFold
    except ImportError:
        return Result(name, None, "scikit-learn not installed")
    d = shortcut_frame(rel)
    if d.empty:
        return Result(name, None, "no local/rx rows joined to truth")
    for c in SHORTCUT_FEATURES:
        d[c] = pd.to_numeric(d[c], errors="coerce").fillna(-1.0)
    benign = d["label"].isin(["benign", "faulty"])
    scores, flagged, skipped = {}, [], []
    for attack in sorted(d.loc[d["attack_id"].notna(), "attack_id"].astype(str).unique()):
        sub = d[benign | (d["attack_id"].astype(str) == attack)]
        if len(sub) > max_rows:
            sub = sub.iloc[:: int(math.ceil(len(sub) / max_rows))]
        y = (sub["attack_id"].astype(str) == attack).astype(int).values
        groups = sub["group"].values
        npos = len(set(groups[y == 1]))
        k = min(5, npos, len(set(groups[y == 0])))
        if k < 2:
            skipped.append(f"{attack} ({npos} malicious pseudonym)")
            continue
        X = sub[SHORTCUT_FEATURES].values
        oof = np.full(len(sub), np.nan)
        for tr, te in StratifiedGroupKFold(n_splits=k).split(X, y, groups):
            if len(set(y[tr])) < 2:
                continue
            clf = RandomForestClassifier(n_estimators=100, max_depth=8, min_samples_leaf=5,
                                         random_state=0, n_jobs=1)
            clf.fit(X[tr], y[tr])
            oof[te] = clf.predict_proba(X[te])[:, 1]
        ok = ~np.isnan(oof)
        if len(set(y[ok])) < 2:
            skipped.append(f"{attack} (folds lack a class)")
            continue
        auc = float(roc_auc_score(y[ok], oof[ok]))
        scores[attack] = auc
        if auc > 0.7 and attack_key(attack) not in RATE_OR_SIZE_ATTACKS:
            flagged.append(f"{attack} ROC-AUC {auc:.2f} from metadata alone")
    shown = ", ".join(f"{a} {s:.2f}" for a, s in scores.items())
    if flagged:
        return Result(name, False, f"{len(flagged)} metadata shortcut(s): "
                      + "; ".join(flagged), flagged)
    if not scores:
        return Result(name, None, "no attack had two malicious pseudonyms: " + "; ".join(skipped))
    extra = f"; skipped {', '.join(skipped)}" if skipped else ""
    return Result(name, True, f"metadata-only ROC-AUC {shown} (limit 0.7){extra}")


CHECKS = {
    "schema": check_schema, "joins": check_joins, "leakage": check_leakage,
    "labels": check_labels, "time": check_time, "units": check_units,
    "splits": check_splits, "manifest": check_manifest, "shortcuts": check_shortcuts,
}


def run_checks(root, only=None) -> list[Result]:
    rel = Release(Path(root))
    out = []
    for nm, fn in CHECKS.items():
        if only and nm not in only:
            continue
        try:
            out.append(fn(rel))
        except Exception as e:  # a check that crashes has failed, never passed
            out.append(Result(nm, False, f"crashed: {type(e).__name__}: {e}"))
    return out


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("root", help="release root (the folder holding runs.parquet)")
    ap.add_argument("--only", help="comma-separated checks: " + ",".join(CHECKS))
    ap.add_argument("--json", help="write every result, with all problems, to this file")
    a = ap.parse_args(argv)
    only = set(a.only.split(",")) if a.only else None
    if only and only - set(CHECKS):
        print(f"unknown check(s): {sorted(only - set(CHECKS))}", file=sys.stderr)
        return 2
    if not Path(a.root).is_dir():
        print(f"not a directory: {a.root}", file=sys.stderr)
        return 2
    results = run_checks(a.root, only)
    for r in results:
        print(r.line())
    if a.json:
        Path(a.json).write_text(json.dumps([r.__dict__ for r in results], indent=1))
    failed = [r.name for r in results if r.ok is False]
    print(f"{'FAIL' if failed else 'PASS'}: {len(results) - len(failed)}/{len(results)} checks"
          + (f" (failed: {', '.join(failed)})" if failed else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
