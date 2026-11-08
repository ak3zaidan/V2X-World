#!/usr/bin/env python3
"""assemble_release.py <run-dir>... --out <root> [--name v2x-scms-mbd-mini]: build a release.

Takes the per-run outputs of the dataset exporter (each <run-dir>/dataset/, or a folder
that itself holds runs.parquet) and writes one release root in the layout of
docs/dataset/schema.md §3:

  runs.parquet                 the runs' rows, concatenated in run_id order
  schema/<table>.json          one copy (refused if two runs disagree)
  local/ global/ truth/        partitions copied to <view>/<table>/split=<s>/run=<id>/
  ml/{local,global}/           the joined training tables of §8 (omit with --no-ml)
  compat/veremi/<run>.zip      the run's VeReMi Ext traces, zipped
  compat/dare/<run>.zip        DARE-style report JSON from global/reports, with the real
                               ids in a separate file (real_ids.json)
  scenarios/<run>.yaml         the scenario that regenerates the run
  README.md                    Hugging Face dataset card (YAML header + a short card body)
  croissant.json               MLCommons Croissant 1.0 metadata with RAI fields
  manifest.json                SHA-256 and size of every other file, plus run provenance

Every input file is checked against its run's manifest before it is copied. Nothing here
reads a clock or draws a random number: the same inputs give the same bytes.
Afterwards run validate_release.py on the result.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import math
import shutil
import sys
import zipfile
from pathlib import Path

import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

VIEWS = ("local", "global", "truth", "wire")
SPLITS = ("train", "validation", "test")
M_PER_DEG = 111_320.0
ZIP_DATE = (1980, 1, 1, 0, 0, 0)
SKIP_NAMES = {".DS_Store"}

VIEW_TEXT = {
    "local": "what one vehicle's OBU knows: its own state, the messages it decoded, the "
             "checks that fired, the reports it filed and its credential life",
    "global": "what the Misbehavior Authority knows: the reports it decrypted, its "
              "decisions and the CRL entries it caused",
    "truth": "the answers, seen by nobody in the system: who really sent what, which "
             "attack, and what happened downstream",
    "ml": "convenience copies joined with their labels, for training only",
}


class AssembleError(Exception):
    pass


# ---- small utilities ---------------------------------------------------------------------

def sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_parquet(p: Path) -> pa.Table:
    """The file's own columns, without hive partition columns inferred from the path."""
    return pq.ParquetFile(p).read()


def write_parquet(p: Path, t: pa.Table) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(t, p, compression="zstd")


def zip_entries(path: Path, entries: dict[str, bytes]) -> None:
    """A zip whose bytes depend only on its entries (fixed dates and modes)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as zf:
        for name in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=ZIP_DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            zf.writestr(info, entries[name])


def dumps(obj) -> str:
    return json.dumps(obj, indent=1, sort_keys=True, ensure_ascii=False, default=str) + "\n"


def table_key(table: str) -> str:
    return table.replace("/", "_")


def partition_kv(rel: Path) -> dict[str, str]:
    return dict(seg.split("=", 1) for seg in rel.parts[:-1] if "=" in seg)


def finite(x):
    return x if isinstance(x, (int, float)) and not (isinstance(x, float) and math.isnan(x)) else None


# ---- reading one run ---------------------------------------------------------------------

class RunInput:
    def __init__(self, path: Path, verify: bool = True):
        path = Path(path)
        self.dir = path / "dataset" if (path / "dataset" / "runs.parquet").is_file() else path
        self.outer = path
        if not (self.dir / "runs.parquet").is_file():
            raise AssembleError(f"{path}: no runs.parquet (looked in {path}/dataset and {path})")
        runs = read_parquet(self.dir / "runs.parquet")
        if runs.num_rows != 1:
            raise AssembleError(f"{self.dir}/runs.parquet has {runs.num_rows} rows, want 1")
        self.runs = runs
        self.row = runs.to_pylist()[0]
        self.run_id = str(self.row["run_id"])
        mp = self.dir / "manifest.json"
        self.manifest = json.loads(mp.read_text()) if mp.is_file() else {}
        self.manifest_sha256 = sha256_file(mp) if mp.is_file() else None
        if verify:
            self.verify()

    def verify(self) -> None:
        entries = self.manifest.get("outputs", self.manifest.get("files"))
        if not isinstance(entries, list):
            raise AssembleError(f"{self.dir}: manifest.json missing or has no outputs list "
                                "(pass --no-verify to assemble unverified inputs)")
        bad = []
        for e in entries:
            p = self.dir / e["path"]
            if not p.is_file():
                bad.append(f"missing {e['path']}")
            elif sha256_file(p) != str(e.get("sha256", "")).lower():
                bad.append(f"sha256 mismatch {e['path']}")
        if bad:
            raise AssembleError(f"{self.dir}: input does not match its manifest: "
                                + "; ".join(bad[:5]) + (f" (+{len(bad) - 5})" if len(bad) > 5 else ""))

    def partitions(self):
        """(view/table, source file, path below the table's partition folders)."""
        for view in VIEWS:
            vdir = self.dir / view
            if not vdir.is_dir():
                continue
            for tdir in sorted(p for p in vdir.iterdir() if p.is_dir()):
                for f in sorted(tdir.rglob("*")):
                    if not f.is_file() or f.name in SKIP_NAMES:
                        continue
                    rel = f.relative_to(tdir)
                    kv = partition_kv(rel)
                    if kv.get("run", self.run_id) != self.run_id:
                        raise AssembleError(f"{f}: partition run={kv['run']} in run "
                                            f"{self.run_id}'s folder")
                    tail = Path(*[s for s in rel.parts if "=" not in s])
                    yield f"{view}/{tdir.name}", f, tail

    def schema_docs(self) -> dict[str, Path]:
        sdir = self.dir / "schema"
        return {p.stem: p for p in sorted(sdir.glob("*.json"))} if sdir.is_dir() else {}

    def veremi(self) -> dict[str, bytes] | bytes | None:
        """The VeReMi traces: a folder of files (entries) or an existing zip (bytes)."""
        for z in (self.dir / "compat" / "veremi" / f"{self.run_id}.zip",
                  self.dir / "compat" / "veremi.zip"):
            if z.is_file():
                return z.read_bytes()
        for d in (self.dir / "compat" / "veremi", self.outer / "veremi"):
            if d.is_dir():
                files = {p.relative_to(d).as_posix(): p.read_bytes()
                         for p in sorted(d.rglob("*")) if p.is_file() and p.name not in SKIP_NAMES}
                if files:
                    return files
        return None

    def scenario(self) -> bytes | None:
        for p in (self.dir / "scenario.yaml", self.dir / "scenarios" / f"{self.run_id}.yaml",
                  self.outer / "scenario.yaml"):
            if p.is_file():
                return p.read_bytes()
        return None


# ---- the joined ml/ tables (schema.md §8) ------------------------------------------------

def lookup(keys: pa.ChunkedArray, table: pa.Table, key: str, col: str) -> pa.Array:
    idx = pc.index_in(keys, value_set=table[key].combine_chunks())
    return table[col].combine_chunks().take(idx)


def ml_local(rx: pa.Table, rcp: pa.Table, tx: pa.Table) -> pa.Table:
    tx_id = lookup(rx["rx_id"], rcp, "rx_id", "tx_id")
    out = rx
    out = out.append_column("label", lookup(rx["rx_id"], rcp, "rx_id", "label"))
    out = out.append_column("attack_id", lookup(pa.chunked_array([tx_id]), tx, "tx_id", "attack_id"))
    for c in ("family", "veremi_ext_code"):
        if c in tx.column_names:
            out = out.append_column(c, lookup(pa.chunked_array([tx_id]), tx, "tx_id", c))
    return out


def ml_global(rep: pa.Table, trep: pa.Table) -> pa.Table:
    out = rep
    for c in ("verdict", "subject_label"):
        if c in trep.column_names:
            out = out.append_column(c, lookup(rep["report_id"], trep, "report_id", c))
    return out


ML_COLUMNS = {
    "label": ("string", "truth", "message label from truth/receptions: benign, faulty or malicious"),
    "attack_id": ("string", "truth", "attack from truth/transmissions, null when benign"),
    "family": ("string", "truth", "attack family from truth/transmissions"),
    "veremi_ext_code": ("int64", "truth", "VeReMi Extension attack code from truth/transmissions"),
    "verdict": ("string", "truth", "truth/reports.verdict: true_accusation or false_accusation"),
    "subject_label": ("string", "truth", "truth/reports.subject_label: the accused device's label"),
}


def ml_schema_doc(table: str, base: dict | None, tbl: pa.Table) -> dict:
    cols = [dict(c) for c in (base or {}).get("columns", [])]
    have = {c["name"] for c in cols}
    for f in tbl.schema:
        if f.name in have:
            continue
        # The joined column's own type (the truth table's: int32 in one writer, int64 in
        # another), so the schema document says what the file holds.
        _, vis, desc = ML_COLUMNS.get(f.name, (None, "truth", "joined label"))
        cols.append(dict(name=f.name, type=str(f.type), nullable=True, unit=None, visibility=vis,
                         description=desc))
    return dict(table=table, view="ml", schema_version=(base or {}).get("schema_version", 1),
                description=f"{table}: an observer table joined with its truth labels; never use as "
                            "an observer table", columns=cols)


# ---- DARE compat (one report = Metadata + Messages + Checks) -----------------------------

def _enu(lat, lon, lat0, lon0):
    if None in (lat, lon, lat0, lon0):
        return [None, None, 0.0]
    x = (lon - lon0) * M_PER_DEG * math.cos(math.radians(lat0))
    y = (lat - lat0) * M_PER_DEG
    return [round(x, 3), round(y, 3), 0.0]


def _vec(mag, heading_deg):
    if mag is None or heading_deg is None:
        return [None, None, 0.0]
    h = math.radians(heading_deg)  # J2735 heading: clockwise from north
    return [round(mag * math.sin(h), 4), round(mag * math.cos(h), 4), 0.0]


def dare_entries(run: dict, reports: pa.Table | None, truth_reports: pa.Table | None,
                 devices: pa.Table | None) -> dict[str, bytes]:
    lat0, lon0 = finite(run.get("origin_lat_deg")), finite(run.get("origin_lon_deg"))
    entries: dict[str, bytes] = {}
    if reports is None:
        return entries
    truth = {r["report_id"]: r for r in truth_reports.to_pylist()} if truth_reports is not None else {}
    attack = {}
    if devices is not None:
        for d in devices.select([c for c in ("device_id", "label", "attack_id")
                                 if c in devices.column_names]).to_pylist():
            attack[d["device_id"]] = d
    real = {}
    for r in reports.to_pylist():
        rid = r["report_id"]
        msgs = []
        for e in r.get("evidence") or []:
            msgs.append({
                "CreationTime": e["generation_time_us"] / 1e6 if e.get("generation_time_us") is not None else None,
                "Pos": _enu(e.get("lat_deg"), e.get("lon_deg"), lat0, lon0),
                "PosLatLon": [e.get("lat_deg"), e.get("lon_deg")],
                "Speed": _vec(e.get("speed_mps"), e.get("heading_deg")),
                "Accel": _vec(e.get("accel_long_mps2"), e.get("heading_deg")),
                "Heading": _vec(1.0, e.get("heading_deg")),
                "PosConfidence": e.get("pos_accuracy_m"),
                "messageId": e.get("spdu_digest"),
                "msgCnt": e.get("msg_cnt"),
                "secMark": e.get("sec_mark_ms"),
                "brakes": e.get("brakes"),
            })
        fired = {s["name"]: 0.0 for s in r.get("subtypes") or []}
        fired.update({n: 0.0 for n in r.get("nonstandard_triggers") or []})
        gen = r.get("generation_time_us")
        report = {"Report": {
            "Metadata": {
                "reportId": rid,
                "senderId": r.get("reporter_cert_id"),
                "reportedId": r.get("subject_cert_id"),
                "reportedIssuer": r.get("subject_issuer"),
                "generationTime": gen / 1e6 if gen is not None else None,
                "receivedTime": r["t_received_ns"] / 1e9 if r.get("t_received_ns") is not None else None,
                "mbrType": r.get("mbr_type"),
                "subtypes": r.get("subtypes") or [],
                "nonstandardTriggers": r.get("nonstandard_triggers") or [],
                "maId": r.get("ma_id"),
                "laLink": r.get("la_link"),
                "setAside": r.get("set_aside"),
            },
            "Messages": msgs,
            "Checks": [fired] if fired else [],
        }}
        entries[f"reports/{rid}.json"] = (json.dumps(report, sort_keys=True) + "\n").encode()
        t = truth.get(rid)
        if t is not None:
            subj = attack.get(t.get("subject_device_id"), {})
            real[rid] = {
                "senderRealId": t.get("reporter_device_id"),
                "reportedRealId": t.get("subject_device_id"),
                "attackType": subj.get("attack_id") or ("Genuine" if subj.get("label") == "benign"
                                                         else subj.get("label")),
                "subjectLabel": t.get("subject_label"),
                "verdict": t.get("verdict"),
            }
    entries["real_ids.json"] = dumps(real).encode()
    entries["README.txt"] = DARE_README.encode()
    return entries


DARE_README = """DARE-style reports for one run (Kamel et al., DARE, 2020), built from global/reports.

reports/<report_id>.json  {"Report": {"Metadata", "Messages", "Checks"}}: only what the
                          Misbehavior Authority sees. senderId is the reporter's pseudonym
                          certificate (HashedId8), reportedId the accused one; times are
                          1609.2 TAI seconds since 2004-01-01 (receivedTime: run seconds).
                          Pos is local ENU metres from the run origin; Speed, Accel and
                          Heading are ENU vectors from the J2735 heading.
                          Checks: one object, the SM-MBR subtypes (and research triggers)
                          that fired, scored 0.0 in DARE's convention (1 = plausible).
                          SM-MBR v1.0 carries which checks fired, not per-message scores.
real_ids.json             the answers, kept apart: senderRealId, reportedRealId, attackType,
                          subjectLabel, verdict per report_id. DARE put these inside the
                          report; here they are a separate file so a model cannot read them.
"""


# ---- the dataset card --------------------------------------------------------------------

def md_cell(x) -> str:
    return str(x if x is not None else "").replace("|", "\\|").replace("\n", " ")


def example_row(p: Path) -> dict:
    t = pq.ParquetFile(p).read_row_group(0) if pq.ParquetFile(p).num_row_groups else None
    if t is None or t.num_rows == 0:
        return {}
    row = t.slice(0, 1).to_pylist()[0]

    def short(v):
        if isinstance(v, list):
            return [short(x) for x in v[:2]] + (["..."] if len(v) > 2 else [])
        if isinstance(v, dict):
            return {k: short(x) for k, x in v.items()}
        if isinstance(v, float):
            return round(v, 6)
        return v
    return {k: short(v) for k, v in row.items()}


def size_category(n: int) -> str:
    for lim, lab in ((1_000, "n<1K"), (10_000, "1K<n<10K"), (100_000, "10K<n<100K"),
                     (1_000_000, "100K<n<1M"), (10_000_000, "1M<n<10M"),
                     (100_000_000, "10M<n<100M"), (1_000_000_000, "100M<n<1B")):
        if n < lim:
            return lab
    return "1B<n<10B"


def card(name: str, repo: str, runs: pa.Table, tables: dict, schemas: dict, out: Path,
         validation: list | None = None, repairs: list | None = None,
         notes: dict | None = None) -> str:
    """tables: view/table -> {split: rows}; schemas: view/table -> schema doc. The card
    itself is tools/dataset/card.py."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import card as card_mod
    return card_mod.render(name, repo, runs, tables, schemas, out, validation, size_category,
                           repairs, notes)


# ---- Croissant ---------------------------------------------------------------------------

CROISSANT_CONTEXT = {
    "@language": "en", "@vocab": "https://schema.org/", "citeAs": "cr:citeAs",
    "column": "cr:column", "conformsTo": "dct:conformsTo", "cr": "http://mlcommons.org/croissant/",
    "rai": "http://mlcommons.org/croissant/RAI/", "data": {"@id": "cr:data", "@type": "@json"},
    "dataType": {"@id": "cr:dataType", "@type": "@vocab"}, "dct": "http://purl.org/dc/terms/",
    "examples": {"@id": "cr:examples", "@type": "@json"}, "extract": "cr:extract",
    "field": "cr:field", "fileProperty": "cr:fileProperty", "fileObject": "cr:fileObject",
    "fileSet": "cr:fileSet", "format": "cr:format", "includes": "cr:includes",
    "isLiveDataset": "cr:isLiveDataset", "jsonPath": "cr:jsonPath", "key": "cr:key",
    "md5": "cr:md5", "parentField": "cr:parentField", "path": "cr:path",
    "recordSet": "cr:recordSet", "references": "cr:references", "regex": "cr:regex",
    "repeated": "cr:repeated", "replace": "cr:replace", "sc": "https://schema.org/",
    "separator": "cr:separator", "source": "cr:source", "subField": "cr:subField",
    "transform": "cr:transform", "samplingRate": "cr:samplingRate",
    "equivalentProperty": "cr:equivalentProperty",
}


def cr_type(t: pa.DataType) -> tuple[str, bool]:
    """(Croissant dataType, repeated)."""
    if pa.types.is_list(t) or pa.types.is_large_list(t):
        return cr_type(t.value_type)[0], True
    if pa.types.is_dictionary(t):
        return cr_type(t.value_type)
    if pa.types.is_boolean(t):
        return "sc:Boolean", False
    if pa.types.is_integer(t):
        return "sc:Integer", False
    if pa.types.is_floating(t):
        return "sc:Float", False
    if pa.types.is_timestamp(t) or pa.types.is_date(t):
        return "sc:Date", False
    if pa.types.is_struct(t):
        return "sc:Text", False
    return "sc:Text", False


def croissant(name: str, repo: str, version: str, out: Path, tables: dict, schemas: dict,
              data_digest: str, date: str | None = None) -> dict:
    dist = [{
        "@type": "cr:FileObject", "@id": "repo", "name": "repo",
        "description": "The Hugging Face repository holding this release.",
        "contentUrl": f"https://huggingface.co/datasets/{repo}/tree/main",
        "encodingFormat": "git+https",
        "sha256": data_digest,
    }]
    record_sets = []
    for t in sorted(tables):
        if t.startswith("wire/"):
            continue
        files = sorted((out / t).rglob("*.parquet"))
        if not files:
            continue
        key = table_key(t)
        dist.append({"@type": "cr:FileSet", "@id": f"{key}-files", "name": f"{key}-files",
                     "description": f"Parquet partitions of {t}",
                     "containedIn": {"@id": "repo"}, "encodingFormat": "application/x-parquet",
                     "includes": f"{t}/**/*.parquet"})
        doc = {c["name"]: c for c in (schemas.get(t) or {}).get("columns", [])}
        fields = []
        for f in pq.ParquetFile(files[0]).schema_arrow:
            dtype, rep = cr_type(f.type)
            c = doc.get(f.name, {})
            desc = c.get("description") or f.name
            if c.get("unit"):
                desc += f" [{c['unit']}]"
            fld = {"@type": "cr:Field", "@id": f"{key}/{f.name}", "name": f.name,
                   "description": desc, "dataType": dtype,
                   "source": {"fileSet": {"@id": f"{key}-files"}, "extract": {"column": f.name}}}
            if rep:
                fld["repeated"] = True
            fields.append(fld)
        record_sets.append({"@type": "cr:RecordSet", "@id": key, "name": key,
                            "description": (schemas.get(t) or {}).get("description")
                            or f"{t}: {VIEW_TEXT.get(t.split('/')[0], '')}",
                            "field": fields})
    doc = {
        "@context": CROISSANT_CONTEXT,
        "@type": "sc:Dataset",
        "name": name,
        "description": "Local (OBU) and global (Misbehavior Authority) views of V2X traffic in a "
                       "simulated US SCMS deployment, with ground truth kept in a separate view.",
        "conformsTo": "http://mlcommons.org/croissant/1.0",
        "citeAs": "@misc{akzaidan_v2x_scms_mbd_2026, title={V2X-SCMS-MBD: local and global "
                  "views of misbehavior in a simulated US SCMS deployment}, author={akzaidan}, "
                  f"year={{2026}}, note={{Hugging Face dataset {repo}}}}}",
        "license": "https://creativecommons.org/licenses/by/4.0/",
        "url": f"https://huggingface.co/datasets/{repo}",
        "version": version,
        "keywords": ["V2X", "misbehavior detection", "SCMS", "IEEE 1609.2", "SAE J2735",
                     "connected vehicles", "synthetic"],
        "isLiveDataset": False,
        "creator": {"@type": "sc:Organization", "name": "V2X World Simulator project"},
        "distribution": dist,
        "recordSet": record_sets,
        "rai:dataCollection": "Simulated with the V2X World Simulator: vehicles on a road network "
                              "broadcast IEEE 1609.2-signed SAE J2735 BSMs, run SM-MBR v1.0 "
                              "checks and file misbehavior reports to an in-loop SCMS MA.",
        "rai:dataCollectionType": ["Simulation"],
        "rai:dataCollectionRawData": "The engine's recording (MCAP); scenarios/ regenerates each run.",
        "rai:dataCollectionMissingData": "local/rx and local/ego are written only for the "
                                         "instrumented receivers listed in runs.parquet.",
        "rai:dataPreprocessingProtocol": ["Exported from the recording after the run; floats "
                                          "quantized to their declared grid; no filtering."],
        "rai:dataAnnotationProtocol": "Labels are the simulator's ground truth (the attack a "
                                      "device was configured with), not human annotation.",
        "rai:machineAnnotationTools": ["V2X World Simulator ground-truth channels"],
        "rai:annotationsPerItem": "1 (oracle)",
        "rai:dataUseCases": ["Training and evaluating local and global V2X misbehavior "
                             "detectors", "report triage", "revocation latency studies"],
        "rai:dataLimitations": "Synthetic traffic and radio; attacker behaviour follows the "
                               "simulator's catalogue; a compressed SCMS calendar in short runs.",
        "rai:dataBiases": "Road networks, densities and attack mixes are chosen by the scenario "
                          "author; results may not transfer to other cities or deployments.",
        "rai:personalSensitiveInformation": "None: all vehicles, identities and certificates "
                                            "are synthetic.",
        "rai:dataSocialImpact": "Supports research on detecting and revoking misbehaving "
                                "vehicles; attack models are public in the literature.",
        "rai:dataReleaseMaintenancePlan": "Versioned releases; manifest.json pins every file.",
        "rai:hasSyntheticData": True,
    }
    if date:  # given on the command line: this tool reads no clock
        doc["datePublished"] = date
    return doc


# ---- assembly ----------------------------------------------------------------------------

def aggregate_digest(files: list[dict]) -> str:
    """SHA-256 over each path and each file's hex digest, in path order (manifest.rs)."""
    h = hashlib.sha256()
    for f in sorted(files, key=lambda f: f["path"]):
        h.update(f["path"].encode())
        h.update(f["sha256"].encode())
    return h.hexdigest()


def file_list(root: Path, skip: set[str]) -> list[dict]:
    out = []
    for p in sorted(root.rglob("*")):
        rel = p.relative_to(root).as_posix()
        if p.is_file() and rel not in skip and p.name not in SKIP_NAMES:
            out.append(dict(path=rel, sha256=sha256_file(p), bytes=p.stat().st_size))
    return out


TRUTH_COLS = ("label", "device_label", "attack_id", "family", "veremi_ext_code", "veremi1_code",
              "nextgen_name", "dare_name", "fields_falsified", "magnitude")
DEVICE_WINDOW_DOC = {
    "attack_start_ns": "scheduled or first falsification, or the air time of the device's "
                       "first falsified transmission when that is earlier",
    "attack_end_ns": "last falsification, or the air time of the device's last falsified "
                     "transmission when that is later",
}


def _part_files(out: Path, t: str, s: str, rid: str) -> list[Path]:
    return sorted((out / t / f"split={s}" / f"run={rid}").glob("*.parquet"))


def _load_parts(files: list[Path]) -> pa.Table | None:
    return pa.concat_tables([read_parquet(f) for f in files]) if files else None


def _store(files: list[Path], t: pa.Table) -> None:
    write_parquet(files[0], t)
    for f in files[1:]:
        f.unlink()


def repair_truth(out: Path, s: str, rid: str, fix: bool = True) -> list[str]:
    """Finds (and with `fix`, repairs) three defects of the v1.0 dataset writer, on the
    copied truth/ partitions of one run (each a no-op when the defect is absent; notes say
    what was found or changed). The writer has been fixed at the source since v1.1
    (`crates/v2xw-record/src/dataset/release`, tests `a_falsified_message_late_to_the_air_*`
    and `the_attack_window_covers_*`): `assemble` runs this as a checker that must find
    nothing, and repairs only under `--repair-v1-truth`, for exports made before the fix.

    1. A transmission can appear twice under one tx_id: the row joined to its air record
       (time, digest, certificate, state) and a row from the attack log with no air record.
       The air row keeps its fields and takes the attack log's labels when the log says the
       message was falsified; the twin is dropped. (Seen for NotYetValid: the receivers'
       verify_result is outside_validity for those messages, so they were falsified.)
    2. truth/receptions.label is the transmission's label (schema): re-derived from the
       repaired truth/transmissions where they differ.
    3. truth/devices' attack window is widened to cover the air time of every falsified
       transmission of the device: the v1 writer recorded falsification instants (a few ms
       before air time) and, for attacks whose first falsified values equal the truth, a
       start after the first falsified message.
    """
    notes = []
    tf = _part_files(out, "truth/transmissions", s, rid)
    tx = _load_parts(tf)
    if tx is None or "tx_id" not in tx.column_names:
        return notes
    t_col = "t_ns" if "t_ns" in tx.column_names else "t_tx_ns"
    rows = tx.to_pylist()
    by_id: dict = {}
    for i, r in enumerate(rows):
        by_id.setdefault(r["tx_id"], []).append(i)
    drop, merged = set(), 0
    for ids in by_id.values():
        if len(ids) < 2:
            continue
        air = [i for i in ids if rows[i].get(t_col) is not None]
        log = [i for i in ids if rows[i].get(t_col) is None]
        if len(air) != 1 or not log:
            continue
        a = rows[air[0]]
        mal = [i for i in log if rows[i].get("label") == "malicious"]
        if mal and a.get("label") != "malicious":
            for c in TRUTH_COLS:
                if c in a:
                    a[c] = rows[mal[0]][c]
            merged += 1
        drop.update(log)
    if drop:
        tx = pa.Table.from_pylist([r for i, r in enumerate(rows) if i not in drop],
                                  schema=tx.schema)
        if fix:
            _store(tf, tx)
        notes.append(f"{rid}: truth/transmissions: {len(drop)} duplicate tx_id rows without "
                     f"an air record dropped, {merged} air rows relabelled from the attack log")
    rows = tx.to_pylist() if drop else rows
    label_of = {r["tx_id"]: (r.get("label"), r.get("attack_id")) for r in rows}

    rf = _part_files(out, "truth/receptions", s, rid)
    rcp = _load_parts(rf)
    if rcp is not None and {"tx_id", "label"} <= set(rcp.column_names):
        rr, n = rcp.to_pylist(), 0
        for r in rr:
            want = label_of.get(r["tx_id"])
            # A receptions table without attack_id is compared on the label alone.
            if want and (r["label"], r.get("attack_id", want[1])) != want:
                r["label"] = want[0]
                if "attack_id" in r:
                    r["attack_id"] = want[1]
                n += 1
        if n:
            if fix:
                _store(rf, pa.Table.from_pylist(rr, schema=rcp.schema))
            notes.append(f"{rid}: truth/receptions: {n} labels re-derived from their "
                         "transmission")

    df = _part_files(out, "truth/devices", s, rid)
    dev = _load_parts(df)
    if dev is not None and {"attack_start_ns", "attack_end_ns", "device_id"} <= set(dev.column_names):
        span: dict = {}
        for r in rows:
            if r.get("label") == "malicious" and r.get(t_col) is not None:
                lo, hi = span.get(r["device_id"], (r[t_col], r[t_col]))
                span[r["device_id"]] = (min(lo, r[t_col]), max(hi, r[t_col]))
        dr, n = dev.to_pylist(), 0
        for r in dr:
            if r["device_id"] not in span:
                continue
            lo, hi = span[r["device_id"]]
            s0, e0 = r["attack_start_ns"], r["attack_end_ns"]
            if s0 is None or lo < s0:
                r["attack_start_ns"] = int(lo)
            if e0 is not None and hi > e0:
                r["attack_end_ns"] = int(hi)
            n += (r["attack_start_ns"], r["attack_end_ns"]) != (s0, e0)
        if n:
            if fix:
                _store(df, pa.Table.from_pylist(dr, schema=dev.schema))
            notes.append(f"{rid}: truth/devices: {n} attack windows widened to their "
                         "falsified transmissions' air times")
    return notes


def fix_attacker_fraction(t: pa.Table, scenario: bytes | None) -> pa.Table:
    """runs.attacker_fraction is the share of vehicles that attack. Exporters before
    150d0be4 summed the populations' `fraction`s only, so a scenario naming its attackers by
    `actor_ids` got 0 (or -0.0). Recompute it from the scenario: the populations'
    fractions plus the distinct named vehicles over the fleet."""
    if scenario is None or "attacker_fraction" not in t.column_names or not t.num_rows:
        return t
    try:
        import yaml
        sc = yaml.safe_load(scenario) or {}
    except Exception:
        return t
    attackers = (sc.get("threats") or {}).get("attackers") or []
    if not attackers:
        return t
    share, named = 0.0, set()
    for a in attackers:
        if a.get("fraction") is not None:
            share += float(a["fraction"])
        elif a.get("actor_ids"):
            named |= {int(i) for i in a["actor_ids"]}
    veh = t["vehicles"][0].as_py() if "vehicles" in t.column_names else None
    if named:
        if not veh:
            return t
        share += len(named) / veh
    i = t.schema.get_field_index("attacker_fraction")
    t = t.set_column(i, t.schema.field(i), pa.array([round(share, 6)],
                                                    type=t.schema.field(i).type))
    # Each attack_mix entry with no fraction: its named vehicles over the fleet.
    if veh and "attack_mix" in t.column_names:
        named_of: dict = {}
        for a in attackers:
            if a.get("fraction") is None and a.get("actor_ids"):
                k = str(a.get("id", "")).rsplit("/", 1)[-1]
                named_of.setdefault(k, set()).update(int(x) for x in a["actor_ids"])
        mix = t["attack_mix"][0].as_py() or []
        for m in mix:
            if m.get("fraction") is None and m.get("id") in named_of:
                m["fraction"] = round(len(named_of[m["id"]]) / veh, 6)
        j = t.schema.get_field_index("attack_mix")
        t = t.set_column(j, t.schema.field(j), pa.array([mix], type=t.schema.field(j).type))
    return t


def assemble(run_dirs: list[Path], out: Path, name: str, repo: str | None = None,
             version: str = "1.0.0", split_override: dict | None = None, ml: bool = True,
             verify: bool = True, date: str | None = None,
             validation: list | None = None, repair_v1: bool = False,
             card_notes: dict | None = None) -> dict:
    repo = repo or f"<org>/{name}"
    inputs = [RunInput(p, verify) for p in run_dirs]
    ids = [r.run_id for r in inputs]
    dup = sorted({i for i in ids if ids.count(i) > 1})
    if dup:
        raise AssembleError(f"run_id given twice: {dup}")
    inputs.sort(key=lambda r: r.run_id)
    split_override = split_override or {}
    unknown = set(split_override) - set(ids)
    if unknown:
        raise AssembleError(f"--split names unknown run(s): {sorted(unknown)}")
    if out.exists() and any(out.iterdir()):
        raise AssembleError(f"refusing to write into a non-empty folder: {out}")
    out.mkdir(parents=True, exist_ok=True)

    # runs.parquet, with the split of each run fixed here
    split_of = {}
    run_tables = []
    for r in inputs:
        s = split_override.get(r.run_id, r.row.get("split"))
        if s not in SPLITS:
            raise AssembleError(f"run {r.run_id}: split {s!r} is not one of {SPLITS} "
                                "(set it with --split RUN=SPLIT)")
        split_of[r.run_id] = s
        t = fix_attacker_fraction(r.runs, r.scenario())
        if "split" in t.column_names:
            i = t.schema.get_field_index("split")
            t = t.set_column(i, t.schema.field(i), pa.array([s], type=t.schema.field(i).type))
        else:
            t = t.append_column("split", pa.array([s]))
        run_tables.append(t)
    runs = pa.concat_tables(run_tables, promote_options="permissive")
    write_parquet(out / "runs.parquet", runs)

    # schema docs: one copy, identical across runs
    schemas_src: dict[str, tuple[bytes, str]] = {}
    for r in inputs:
        for stem, p in r.schema_docs().items():
            data = p.read_bytes()
            if stem in schemas_src and schemas_src[stem][0] != data:
                raise AssembleError(f"schema/{stem}.json differs between runs "
                                    f"{schemas_src[stem][1]} and {r.run_id}")
            schemas_src.setdefault(stem, (data, r.run_id))
    for stem, (data, _) in sorted(schemas_src.items()):
        (out / "schema").mkdir(parents=True, exist_ok=True)
        (out / "schema" / f"{stem}.json").write_bytes(data)
    schemas = {}
    for stem, (data, _) in schemas_src.items():
        doc = json.loads(data)
        schemas[doc.get("table") or stem.replace("_", "/", 1)] = doc

    # partitions
    tables: dict[str, dict[str, int]] = {}
    for r in inputs:
        s = split_of[r.run_id]
        seen = set()
        for table, src, tail in r.partitions():
            dst = out / table / f"split={s}" / f"run={r.run_id}" / tail
            if dst in seen:
                raise AssembleError(f"two input files map to {dst.relative_to(out)}")
            seen.add(dst)
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dst)
            if src.suffix == ".parquet":
                n = pq.ParquetFile(dst).metadata.num_rows
                tables.setdefault(table, {}).setdefault(s, 0)
                tables[table][s] += n

    # truth/: the v1.0 writer's known defects (schema.md §13), per run: none in an export
    # of the fixed writer; repaired only when asked, for an export made before the fix
    repairs = []
    for r in inputs:
        found = repair_truth(out, split_of[r.run_id], r.run_id, fix=repair_v1)
        if found and not repair_v1:
            raise AssembleError("the v1.0 writer's truth defects in an input (re-export it "
                                "with the fixed writer, or pass --repair-v1-truth): "
                                + "; ".join(found))
        repairs += found
    for t in ("truth/transmissions",):
        if t in tables:
            tables[t] = {s: sum(pq.ParquetFile(f).metadata.num_rows
                                for f in (out / t / f"split={s}").rglob("*.parquet"))
                         for s in tables[t]}
    if repairs and "truth/devices" in schemas:
        for c in schemas["truth/devices"].get("columns", []):
            if c.get("name") in DEVICE_WINDOW_DOC:
                c["description"] = DEVICE_WINDOW_DOC[c["name"]]
        (out / "schema" / "truth_devices.json").write_text(dumps(schemas["truth/devices"]))

    # ml/: the joined training copies (schema.md §8)
    if ml:
        for r in inputs:
            s, rid = split_of[r.run_id], r.run_id

            def load(t):
                fs = sorted((out / t / f"split={s}" / f"run={rid}").glob("*.parquet"))
                return pa.concat_tables([read_parquet(f) for f in fs],
                                        promote_options="permissive") if fs else None
            rx, rcp, tx = load("local/rx"), load("truth/receptions"), load("truth/transmissions")
            # A run with no rows gets no ml/ file: Hugging Face `datasets` refuses to read
            # a Parquet file without row groups ("BatchSize must be greater than 0").
            if rx is not None and rcp is not None and tx is not None and rx.num_rows:
                t = ml_local(rx, rcp, tx)
                write_parquet(out / "ml/local" / f"split={s}" / f"run={rid}" / "part-00000.parquet", t)
                tables.setdefault("ml/local", {}).setdefault(s, 0)
                tables["ml/local"][s] += t.num_rows
                schemas.setdefault("ml/local", ml_schema_doc("ml/local", schemas.get("local/rx"), t))
            rep, trep = load("global/reports"), load("truth/reports")
            if rep is not None and trep is not None and rep.num_rows:
                t = ml_global(rep, trep)
                write_parquet(out / "ml/global" / f"split={s}" / f"run={rid}" / "part-00000.parquet", t)
                tables.setdefault("ml/global", {}).setdefault(s, 0)
                tables["ml/global"][s] += t.num_rows
                schemas.setdefault("ml/global", ml_schema_doc("ml/global", schemas.get("global/reports"), t))
        for t in ("ml/local", "ml/global"):
            if t in schemas and t in tables:
                (out / "schema").mkdir(parents=True, exist_ok=True)
                (out / "schema" / f"{table_key(t)}.json").write_text(dumps(schemas[t]))

    # compat/ and scenarios/
    notes = []
    for r in inputs:
        s, rid = split_of[r.run_id], r.run_id
        v = r.veremi()
        if isinstance(v, bytes):
            (out / "compat/veremi").mkdir(parents=True, exist_ok=True)
            (out / "compat/veremi" / f"{rid}.zip").write_bytes(v)
        elif v:
            zip_entries(out / "compat/veremi" / f"{rid}.zip", v)
        else:
            notes.append(f"{rid}: no VeReMi traces")

        def load(t):
            fs = sorted((out / t / f"split={s}" / f"run={rid}").glob("*.parquet"))
            return pa.concat_tables([read_parquet(f) for f in fs],
                                    promote_options="permissive") if fs else None
        reps = load("global/reports")
        if reps is not None:
            zip_entries(out / "compat/dare" / f"{rid}.zip",
                        dare_entries(r.row, reps, load("truth/reports"), load("truth/devices")))
        else:
            notes.append(f"{rid}: no global/reports, no DARE file")
        sc = r.scenario()
        if sc is not None:
            (out / "scenarios").mkdir(parents=True, exist_ok=True)
            (out / "scenarios" / f"{rid}.yaml").write_bytes(sc)
        else:
            notes.append(f"{rid}: no scenario")

    # card, croissant, manifest
    data_files = file_list(out, set())
    data_digest = aggregate_digest(data_files)
    card_notes = dict(card_notes or {})
    card_notes.setdefault("version", version)
    (out / "README.md").write_text(card(name, repo, runs, tables, schemas, out, validation,
                                        repairs, card_notes))
    (out / "croissant.json").write_text(dumps(croissant(name, repo, version, out, tables,
                                                        schemas, data_digest, date)))
    files = file_list(out, {"manifest.json"})
    provenance = []
    for r in inputs:
        m = {k: v for k, v in r.manifest.items() if k not in ("outputs", "files")}
        m.update(run_id=r.run_id, split=split_of[r.run_id],
                 input_manifest_sha256=r.manifest_sha256,
                 input_files=len(r.manifest.get("outputs", r.manifest.get("files", [])) or []))
        provenance.append(m)
    schema_versions = sorted({str(d.get("schema_version")) for d in schemas.values()
                              if d.get("schema_version") is not None})
    manifest = dict(dataset=name, version=version, repo=repo,
                    generator="tools/dataset/assemble_release.py",
                    schema_versions=schema_versions, runs=provenance,
                    data_digest_sha256=data_digest,
                    release_digest_sha256=aggregate_digest(files), files=files)
    (out / "manifest.json").write_text(dumps(manifest))
    total = sum(f["bytes"] for f in files)
    return dict(runs=len(inputs), files=len(files) + 1, bytes=total, tables=len(tables),
                notes=repairs + notes, split_of=split_of)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("runs", nargs="+", type=Path, help="per-run output folders")
    ap.add_argument("--out", required=True, type=Path, help="release root (empty or absent)")
    ap.add_argument("--name", default="v2x-scms-mbd-mini")
    ap.add_argument("--repo", help="Hugging Face repo id for the card's links (org/name)")
    ap.add_argument("--version", default="1.0.0")
    ap.add_argument("--date", help="publication date for croissant.json (YYYY-MM-DD)")
    ap.add_argument("--split", action="append", default=[], metavar="RUN=SPLIT",
                    help="override a run's split (train, validation, test); repeatable")
    ap.add_argument("--no-ml", action="store_true", help="do not write the joined ml/ tables")
    ap.add_argument("--validation", type=Path,
                    help="validate_release.py --json output of this same release, for the "
                         "card's results section")
    ap.add_argument("--no-verify", action="store_true",
                    help="do not check inputs against their manifests")
    ap.add_argument("--repair-v1-truth", action="store_true",
                    help="repair the v1.0 writer's truth defects (exports made before the "
                         "fix) instead of refusing them")
    ap.add_argument("--notes", type=Path,
                    help="JSON of the card's release text: version_note (one line under "
                         "the summary: what changed since the previous version), changes "
                         "(an optional short changelog at the end) and limitations (bullets "
                         "added to the Limitations section)")
    a = ap.parse_args(argv)
    try:
        override = dict(s.split("=", 1) for s in a.split)
    except ValueError:
        print("--split takes RUN=SPLIT", file=sys.stderr)
        return 2
    try:
        res = assemble(a.runs, a.out, a.name, a.repo, a.version, override, not a.no_ml,
                       not a.no_verify, a.date,
                       json.loads(a.validation.read_text()) if a.validation else None,
                       a.repair_v1_truth,
                       json.loads(a.notes.read_text()) if a.notes else None)
    except AssembleError as e:
        print(f"assemble_release: {e}", file=sys.stderr)
        return 1
    splits = ", ".join(f"{k}={v}" for k, v in res["split_of"].items())
    print(f"release {a.name}: {res['runs']} runs ({splits}), {res['tables']} tables, "
          f"{res['files']} files, {res['bytes'] / 1e6:.2f} MB under {a.out}")
    for n in res["notes"]:
        print(f"note: {n}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
