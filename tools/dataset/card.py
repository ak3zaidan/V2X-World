"""card.py: the Hugging Face dataset card (README.md) of a V2X-SCMS-MBD release.

`render(...)` is called by assemble_release.py. Everything in the card is computed from the
release itself (runs.parquet, schema/, the Parquet tables, scenarios/) and, when given, from
validate_release.py's --json output, so the card never states a number the files do not
hold. No clock, no randomness: the same release gives the same card.
"""

from __future__ import annotations

import json
from pathlib import Path

import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

SPLITS = ("train", "validation", "test")
VIEW_ORDER = ("local", "global", "truth", "ml", "wire")

# The table the viewer opens first: what one OBU received, with no label beside it.
DEFAULT_CONFIG = "local_rx"

# The annotated example rows: per table, one bold line saying what a row is, and short
# `//` comments on the fields a reader would not guess. A comment is a string, a pair
# (comment when the field has a value, comment when it is null), or a function of the value. A null field that is null
# in every row of the release also says so. Keys of nested fields are "parent.child".
EXAMPLES = (
    ("local/rx", "one safety message (BSM) as a receiving vehicle's OBU got it and checked "
     "it. Only what the vehicle could see.", {
         "receiver_ref": "receiving vehicle, anonymised",
         "rx_id": "this reception, anonymised",
         "t_rx_ns": "receive time since run start",
         "spdu_digest": "fingerprint of the signed message; joins to MA evidence and truth",
         "rssi_dbm": "received signal strength",
         "cbr": "channel busy ratio the receiver measured",
         "psid": "application id: 32 = BSM",
         "cert_id": "sender's pseudonym certificate (HashedId8)",
         "signer_kind": "full certificate attached, or digest only",
         "issuer_id": "CA that issued that certificate",
         "verify_result": "signature check outcome",
         "generation_time_us": "signing time, IEEE 1609.2 clock (µs since 2004)",
         "msg_cnt": "BSM rolling counter 0-127",
         "temp_id": "BSM temporary id, rotates with the pseudonym",
         "sec_mark_ms": "millisecond within the minute",
         "lat_deg": "claimed position (synthetic grid map near 0,0)",
         "x_m": "same position in local metres",
         "pos_accuracy": "claimed GPS error ellipse",
         "speed_mps": "claimed motion",
         "heading_deg": "clockwise from north",
         "brakes": "brake status bits",
         "event_flags": ("hazard event bits", "hazard events, none here"),
         "ego_x_m": "receiver's own state at reception, and its range to the claimed position",
     }),
    ("local/reports", "a misbehaviour report (MBR) a vehicle filed after its onboard checks "
     "flagged a sender, seen from the vehicle's side.", {
         "receiver_ref": "reporting vehicle, anonymised",
         "report_id": "fingerprint of the report; joins to global/reports",
         "subject_cert_id": "accused pseudonym",
         "mbr_type": lambda v: "US SM-MBR category" + MBR_TYPES.get(v, ""),
         "subtypes": "onboard checks that fired",
         "evidence_spdu_digests": "attached messages; join to local/rx and truth",
         "fate": "what happened to the report",
         "path": "cellular or via a roadside unit",
         "weight_at_upload": "share of the reporter's report budget",
     }),
    ("global/reports", "the same report as the Misbehavior Authority (MA) received it: only "
     "what the real US report format carries.", {
         "ma_id": "which MA (one per SCMS provider)",
         "report_id": "same report as above",
         "t_received_ns": "arrival at the MA (uploads are batched)",
         "generation_time_us": "when the reporter signed it",
         "generation_location": "where the reporter signed it",
         "psid": "application the report is about (BSM)",
         "reporter_cert_id": "reporter's certificate, not its identity",
         "subject_cert_id": "accused pseudonym",
         "subject_issuer": "CA of the accused certificate",
         "subtypes": "official SM-MBR codes with check parameters",
         "evidence": "accused vehicle's BSMs attached as proof",
         "evidence.spdu_digest": "joins to local/rx and truth/transmissions",
         "evidence_start_ns": "time span the evidence covers",
         "set_aside": "set when the MA discards the report (e.g. duplicate)",
         "la_link": "linkage value tying pseudonyms to one device, once the MA asks for it",
         "la_link_t_ns": "the decision that asked for it",
     }),
    ("global/decisions", "an MA decision taken on the reports it collected.", {
         "subject_kind": "whole vehicle revoked, not just one pseudonym",
         "subject_cert_id": "certificate that triggered it",
         "la_link": "linkage value put on the CRL",
         "rule": "MA policy that decided",
         "report_ids": "evidence behind the decision",
     }),
    ("truth/transmissions", "the answer key for one transmitted message: the hidden truth, "
     "never part of the observer views.", {
         "tx_id": "real message id (truth only)",
         "device_id": "real vehicle id (truth only)",
         "spdu_digest": "joins to local/rx and MA evidence",
         "cert_id": "pseudonym it was signed with",
         "label": "this message",
         "device_label": "its sender",
         "veremi_ext_code": "same attack in other datasets' labels (null: no such code)",
         "fields_falsified": "what the attacker changed, and by how much",
         "x_m": "true state; compare with what the message claimed",
         "gnss_error_m": "real GPS error (honest noise)",
         "fix_to_stamp_ms": "GPS fix to timestamp delay",
     }),
)

MBR_TYPES = {
    "1VInconsistent": ": one vehicle's messages contradict each other",
}

def md(x) -> str:
    if x is None:
        return ""
    if isinstance(x, float):
        x = round(x, 3)
    return str(x).replace("|", "\\|").replace("\n", " ")


def table_key(t: str) -> str:
    return t.replace("/", "_")


def read_table(root: Path, t: str, columns=None) -> pa.Table | None:
    files = sorted((root / t).rglob("*.parquet")) if (root / t).is_dir() else []
    if not files:
        return None
    parts = []
    for f in files:
        pf = pq.ParquetFile(f)
        cols = [c for c in columns if c in pf.schema_arrow.names] if columns else None
        parts.append(pf.read(columns=cols))
    return pa.concat_tables(parts, promote_options="permissive")


def shorten(v, depth=0):
    """An example value: lists of records cut to one item, other lists to two, floats to
    six places."""
    if isinstance(v, list):
        n = 1 if v and isinstance(v[0], dict) else 2
        return [shorten(x, depth + 1) for x in v[:n]] + (["..."] if len(v) > n else [])
    if isinstance(v, dict):
        return {k: shorten(x, depth + 1) for k, x in v.items()}
    if isinstance(v, float):
        return round(v, 6)
    return v


def filled(v) -> int:
    """How many leaf values of v are not null (an empty list counts as none)."""
    if isinstance(v, dict):
        return sum(filled(x) for x in v.values())
    if isinstance(v, list):
        return sum(filled(x) for x in v)
    return 0 if v is None else 1


def fullest_row(tb: pa.Table, scan: int = 5000) -> dict | None:
    """The first row, among the first `scan`, with the most non-null values."""
    if tb is None or tb.num_rows == 0:
        return None
    rows = tb.slice(0, scan).to_pylist()
    best = max(range(len(rows)), key=lambda i: (filled(rows[i]), -i))
    return {k: v for k, v in rows[best].items() if k not in ("split", "run")}


def jsonc(row: dict, comments: dict, all_null: set[str]) -> list[str]:
    """`row` as indented JSON with `// comment` on the commented fields, the comments
    lined up as on the published card."""
    lines: list[str] = []

    def note(path: str, v) -> str:
        c = comments.get(path)
        if isinstance(c, tuple):
            c = c[0] if v is not None else c[1]
        elif callable(c):
            c = c(v)
        if v is None and path in all_null:
            c = (c + " " if c else "") + "(not recorded in this sample)"
        return c or ""

    def put(text: str, c: str):
        lines.append(text.ljust(40) + f" // {c}" if c else text)

    def emit(v, ind: int, prefix: str, path: str, last: bool, item: bool = False):
        """`item`: an element of a list, which takes no comment of its own (its fields do,
        under the list's path)."""
        pad = " " * ind
        comma = "" if last else ","
        c = "" if item else note(path, v)
        if isinstance(v, dict) and v:
            put(f"{pad}{prefix}{{", c)
            items = list(v.items())
            for i, (k, x) in enumerate(items):
                emit(x, ind + 1, f"{json.dumps(k)}: ", f"{path}.{k}", i == len(items) - 1)
            lines.append(f"{pad}}}{comma}")
        elif isinstance(v, list) and v:
            put(f"{pad}{prefix}[", c)
            for i, x in enumerate(v):
                emit(x, ind + 1, "", path, i == len(v) - 1, item=True)
            lines.append(f"{pad}]{comma}")
        else:
            put(f"{pad}{prefix}{json.dumps(v, default=str, ensure_ascii=False)}{comma}", c)

    lines.append("{")
    items = list(row.items())
    for i, (k, v) in enumerate(items):
        emit(v, 1, f"{json.dumps(k)}: ", k, i == len(items) - 1)
    lines.append("}")
    return lines


def example_rows(out: Path) -> list[str]:
    """The `## Example rows` body: one annotated row per table of EXAMPLES. The MA's report
    is the fullest one; the vehicle-side report shown is the same report when the OBU's
    copy is there, so the two can be read side by side."""
    picked: dict[str, dict] = {}
    tbl: dict[str, pa.Table] = {}
    for t, _, _ in EXAMPLES:
        tb = read_table(out, t)
        if tb is not None and tb.num_rows:
            tbl[t] = tb
    if "global/reports" in tbl:
        tb = tbl["global/reports"]  # a report the MA kept, when there is one
        kept = tb.filter(pc.is_null(tb["set_aside"])) if "set_aside" in tb.column_names else tb
        picked["global/reports"] = fullest_row(kept if kept.num_rows else tb)
    if "local/reports" in tbl:
        tb = tbl["local/reports"]
        g = picked.get("global/reports")
        if g is not None:
            same = tb.filter(pc.equal(tb["report_id"], g["report_id"]))
            tb = same if same.num_rows else tb
        picked["local/reports"] = fullest_row(tb)
    if "global/decisions" in tbl:
        tb = tbl["global/decisions"]
        rev = tb.filter(pc.equal(tb["decision"], "revoke")) if "decision" in tb.column_names \
            else tb
        picked["global/decisions"] = fullest_row(rev if rev.num_rows else tb)
    if "truth/transmissions" in tbl:
        tb = tbl["truth/transmissions"]
        mal = tb.filter(pc.equal(tb["label"], "malicious"))
        picked["truth/transmissions"] = fullest_row(mal if mal.num_rows else tb)
    if "local/rx" in tbl:
        picked["local/rx"] = fullest_row(tbl["local/rx"])
    L = ["Real rows from this release (lists shortened). Comments are added here for "
         "reading; they are not in the data.\n"]
    for t, what, comments in EXAMPLES:
        row = picked.get(t)
        if not row:
            continue
        nulls = {c for c in tbl[t].column_names if tbl[t][c].null_count == tbl[t].num_rows}
        L += [f"**`{t}`**: {what}\n", "```jsonc",
              *jsonc({k: shorten(v) for k, v in row.items()}, comments, nulls), "```\n"]
    return L


def scenario_attackers(root: Path, run_id: str) -> list[dict]:
    """[{attack, vehicles, from_s, to_s, params}] of a run's scenario (vehicle attackers),
    plus the infrastructure and passive threats."""
    p = root / "scenarios" / f"{run_id}.yaml"
    if not p.is_file():
        return []
    try:
        import yaml
        s = yaml.safe_load(p.read_text()) or {}
    except Exception:  # no PyYAML, or a scenario that is not plain YAML
        return []
    if not isinstance(s, dict):
        return []
    th = s.get("threats") or {}
    out = []
    for a in th.get("attackers") or []:
        sch = a.get("schedule") or {}
        ids = a.get("actor_ids") or []
        out.append(dict(attack=str(a.get("id", "")).rsplit("/", 1)[-1], kind="vehicle",
                        vehicles=len(ids) if ids else a.get("count"),
                        fraction=a.get("fraction"), from_s=sch.get("from_s"),
                        to_s=sch.get("to_s"), params=a.get("params")))
    for j in th.get("jammers") or []:
        pr = j.get("params") or {}
        name = {"constant": "ConstantJamming", "reactive": "ReactiveJamming",
                "pulsed": "RandomDutyJamming"}.get(str(j.get("id", "")).rsplit("/", 1)[-1],
                                                    str(j.get("id", "")).rsplit("/", 1)[-1])
        out.append(dict(attack=name, kind="radio jammer", vehicles=None,
                        from_s=pr.get("from_s"), to_s=pr.get("to_s"),
                        params={k: v for k, v in pr.items() if k not in ("from_s", "to_s")}))
    rsu = th.get("compromised_rsu_attack")
    if rsu and th.get("compromised_rsus"):
        pr = (rsu.get("params") or {})
        out.append(dict(attack=pr.get("kind", "CompromisedRsu"), kind="compromised RSU",
                        vehicles=None, from_s=pr.get("from_s"), to_s=pr.get("to_s"),
                        params={k: v for k, v in pr.items()
                                if k not in ("from_s", "to_s", "kind")}))
    if th.get("eavesdropper"):
        out.append(dict(attack="PrivacyObserver", kind="passive observer", vehicles=None,
                        from_s=None, to_s=None, params=None))
    return out


def null_columns(root: Path, t: str) -> list[str]:
    files = sorted((root / t).rglob("*.parquet")) if (root / t).is_dir() else []
    if not files:
        return []
    total, nulls = 0, {}
    for f in files:
        md_ = pq.ParquetFile(f).metadata
        total += md_.num_rows
        tb = pq.ParquetFile(f).read()
        for c in tb.column_names:
            col = tb[c]
            n = col.null_count
            if pa.types.is_list(col.type) or pa.types.is_large_list(col.type):
                n = int(pc.sum(pc.or_kleene(pc.is_null(col),
                                            pc.equal(pc.list_value_length(col), 0))
                               .cast(pa.int64())).as_py() or 0)
            nulls[c] = nulls.get(c, 0) + n
    return [c for c, n in nulls.items() if total and n == total]


# What one row of each table is, as the card's table list says it.
ROW_IS = {
    "local/rx": "a message an instrumented OBU received (features only, no label)",
    "local/checks": "a check that fired, inside a report an OBU filed (not every check run)",
    "local/ego": "the receiving OBU's own state, at its BSM rate",
    "local/pki": "an event in an OBU's credential life (enrolment, pseudonym change, CRL)",
    "local/reports": "a misbehavior report an OBU created, and what became of it",
    "global/reports": "a report the MA decrypted",
    "global/decisions": "an MA decision: revoke, dismiss or monitor",
    "global/crl": "a CRL entry the MA caused",
    "truth/transmissions": "a transmitted message: sender, label, attack, true state",
    "truth/receptions": "the label of one `local/rx` row",
    "truth/devices": "a device: class, label, attack and its window",
    "truth/pseudonyms": "a certificate and the device that used it",
    "truth/reports": "a report: who accused whom, and whether it was true",
    "truth/timeline": "a bad or falsely accused device: the time of each step to revocation",
    "truth/app_effects": "a warning an application raised because of a bad sender",
    "ml/local": "`local/rx` joined with its label, for training",
    "ml/global": "`global/reports` joined with its verdict, for training",
}

VIEW_TEXT = {
    "local": "one vehicle's OBU: its own state, the messages it decoded, the checks that "
             "fired, the reports it filed, its credentials",
    "global": "the Misbehavior Authority: the reports it decrypted, its decisions, the CRL "
              "entries they caused",
    "truth": "nobody in the system: who really sent what, which attack, what happened next",
    "ml": "copies of `local/rx` and `global/reports` joined with their labels, for training",
}

# How the summary names a threat kind, and the radio.
KIND_WORD = {"radio jammer": "jamming", "compromised RSU": "compromised-RSU",
             "passive observer": "privacy"}
RAT_TEXT = {"dsrc-80211p": "DSRC (IEEE 802.11p)"}

# Where a threat that sends no labelled message of its own shows in the data.
SILENT_WHERE = {"radio jammer": "lost receptions, RSSI and CBR in `local/rx`",
                "compromised RSU": "the report path and the CRL",
                "passive observer": "tracking success, in `truth/` only"}


def num(x) -> str:
    """A number as the card prints it: 15.0 -> 15."""
    return str(int(x)) if isinstance(x, float) and x.is_integer() else md(x)


def plural(n: int, word: str) -> str:
    return f"{n} {word}{'' if n == 1 else 's'}"


def render(name: str, repo: str, runs: pa.Table, tables: dict, schemas: dict, out: Path,
           validation: list[dict] | None = None, size_category=None,
           repairs: list[str] | None = None, notes: dict | None = None) -> str:
    """notes: the release's own text, from assemble_release.py --notes: `version`,
    `version_note` (one line under the summary: what changed since the previous version),
    `changes` (an optional short changelog, printed at the very end) and `limitations`
    (bullets added to the Limitations section)."""
    notes = notes or {}
    rows = runs.to_pylist()
    total_rows = {t: sum(v.values()) for t, v in tables.items()}
    order = sorted(tables, key=lambda t: (VIEW_ORDER.index(t.split("/")[0])
                                          if t.split("/")[0] in VIEW_ORDER else 9, t))
    configs = []
    for t in order:
        if t.startswith("wire/"):
            continue
        splits = [s for s in SPLITS if tables[t].get(s)]
        if splits:
            configs.append({"config_name": table_key(t),
                            "data_files": [{"split": s, "path": f"{t}/split={s}/**/*.parquet"}
                                           for s in splits]})
    # One table of every run: a single `train` split (`all` is reserved by Hugging Face
    # `datasets`). Each row names its own split in the `split` column.
    configs.append({"config_name": "runs", "data_files": [{"split": "train",
                                                           "path": "runs.parquet"}]})
    names = [c["config_name"] for c in configs]
    default = DEFAULT_CONFIG if DEFAULT_CONFIG in names else names[0]
    configs.sort(key=lambda c: c["config_name"] != default)  # the default listed first
    for c in configs:
        if c["config_name"] == default:
            c["default"] = True
    header = {
        "license": "cc-by-4.0",
        "pretty_name": f"V2X-SCMS-MBD ({name})",
        "language": ["en"],
        "tags": ["v2x", "misbehavior-detection", "vehicular-networks", "scms", "ieee-1609.2",
                 "j2735", "connected-vehicles", "intrusion-detection", "security", "synthetic",
                 "tabular", "parquet"],
        "task_categories": ["tabular-classification"],
        "size_categories": [size_category(max(total_rows.values() or [0]))]
        if size_category else [],
        "configs": configs,
    }
    try:
        import yaml
        yhead = yaml.safe_dump(header, sort_keys=False, default_flow_style=False, width=100)
    except ImportError:  # JSON is valid YAML
        yhead = json.dumps(header, indent=1) + "\n"

    split_runs = {s: [r["run_id"] for r in rows if r.get("split") == s] for s in SPLITS}
    used_splits = [s for s in SPLITS if split_runs[s]]
    att = {r["run_id"]: scenario_attackers(out, r["run_id"]) for r in rows}
    heldout = sorted({a for r in rows for a in (r.get("heldout_attacks") or [])})
    train_attacks = {a["attack"] for r in rows if r.get("split") in ("train", "validation")
                     for a in att[r["run_id"]]}

    # per-attack facts from truth/transmissions
    attacks: dict[str, dict] = {}
    tx = read_table(out, "truth/transmissions",
                    ["run_id", "label", "attack_id", "family", "veremi_ext_code",
                     "veremi1_code", "nextgen_name", "dare_name"])
    label_counts = {}
    if tx is not None:
        for r in tx.group_by(["label"]).aggregate([("label", "count")]).to_pylist():
            label_counts[r["label"]] = r["label_count"]
        if "attack_id" in tx.column_names:
            m = tx.filter(pc.is_valid(tx["attack_id"]))
            keys = [c for c in ("attack_id", "family", "veremi_ext_code", "veremi1_code",
                                "nextgen_name", "dare_name") if c in m.column_names]
            for r in m.group_by(keys).aggregate([]).to_pylist():
                attacks.setdefault(r["attack_id"], dict(r))
            for r in m.group_by(["attack_id"]).aggregate([("attack_id", "count")]).to_pylist():
                attacks[r["attack_id"]]["messages"] = r["attack_id_count"]
            for r in m.group_by(["attack_id", "run_id"]).aggregate([]).to_pylist():
                attacks[r["attack_id"]].setdefault("runs", set()).add(r["run_id"])

    # The scenario's attackers: vehicles and start times per attack, and the threats that
    # send no labelled message of their own.
    per_attack: dict[str, int] = {}
    starts: dict[str, set] = {}
    silent: dict[tuple, list] = {}
    for rid, a in att.items():
        for x in a:
            if x["kind"] == "vehicle":
                per_attack[x["attack"]] = per_attack.get(x["attack"], 0) + (x["vehicles"] or 0)
                if x["from_s"] is not None:
                    starts.setdefault(x["attack"], set()).add(x["from_s"])
            else:
                silent.setdefault((x["attack"], x["kind"], x["from_s"], x["to_s"]),
                                  []).append(rid)
    kinds = sorted({k for _, k, _, _ in silent})
    n_silent = sum(len(v) for v in silent.values())
    other_threats = ""
    if kinds:
        other_threats = (" plus " + ("a " if n_silent == 1 else "")
                         + " and ".join(KIND_WORD.get(k, k) for k in kinds)
                         + (" threat" if n_silent == 1 else " threats"))
    n_att_veh = sum(x["vehicles"] or 0 for a in att.values() for x in a
                    if x["kind"] == "vehicle")
    n_veh = sum(r.get("vehicles") or 0 for r in rows)
    version = notes.get("version")
    r0 = rows[0] if rows else {}

    # ---- title and summary
    if len(rows) == 1:
        sample = (f"{'Version ' + version if version else 'This version'} is a **one-run "
                  f"sample**: 1 run of {md(r0.get('duration_s'))} s with "
                  f"{md(r0.get('vehicles'))} vehicles")
    else:
        sample = (f"This is the **mini sample**: {len(rows)} runs of "
                  f"{md(r0.get('duration_s'))} s with {md(r0.get('vehicles'))} vehicles each")
    sample += (f", {len(attacks)} message attacks{other_threats}, "
               f"{total_rows.get('local/rx', 0):,} received messages and "
               f"{total_rows.get('global/reports', 0):,} misbehavior reports at the MA.")
    if len(rows) == 1:
        sample += (" It shows the format, the attacks and the whole chain from message to "
                   "revocation; the full multi-run release follows as a later version.")
    L = [f"---\n{yhead}---\n",
         f"# V2X-SCMS-MBD: {name}\n",
         "**Misbehavior-detection data for connected vehicles, seen from both places a real "
         "US V2X deployment observes it**: each vehicle's on-board unit (OBU, the `local/` "
         "view) and the Misbehavior Authority of the Security Credential Management System "
         "(MA, the `global/` view). The answers, who really sent what and which attack, are "
         "kept apart in `truth/`. Observer columns hold only what that device could see under "
         "IEEE 1609.2, SAE J2735 and the SCMS misbehavior-report specification SM-MBR v1.0, "
         "and one message can be followed from the air to a neighbour's check, its "
         "misbehavior report (MBR), the MA's decision and the certificate revocation list "
         "(CRL) entry.\n",
         sample + "\n"]
    if notes.get("version_note"):
        L[-1] = sample + " " + notes["version_note"] + "\n"
    L += ["> Synthetic data from the V2X World Simulator. No real vehicle, person or location "
          "trace is included.\n"]

    # ---- the three views
    L += ["## The three views\n", "| View | Who sees it | Tables |", "|---|---|---|"]
    for v in ("local", "global", "truth", "ml"):
        ts = [t.split("/", 1)[1] for t in order if t.startswith(v + "/")]
        if ts:
            L.append(f"| `{v}/` | {VIEW_TEXT[v]} | {', '.join(f'`{x}`' for x in ts)} |")
    L += ["",
          "**No label leaks.** Unlike VeReMi, VeReMi Extension, VeReMi NextGen and DARE, no "
          "sender identity or attack label sits next to the features: `local/` and `global/` "
          "know a sender only by its certificate, and splits are by run, never by message.\n",
          "**Join keys.** `spdu_digest` (a hash of the signed message) and `cert_id` (its "
          "certificate's HashedId8) join a message across `local/`, the MA's evidence and "
          "`truth/transmissions`; `report_id` joins a report across `local/`, `global/` and "
          "`truth/reports`; `rx_id` joins a reception to its label in `truth/receptions`. "
          "Every table carries `run_id`.\n"]

    # ---- how it was generated
    L += ["## How it was generated\n",
          "- **Simulator**: the V2X World Simulator, a deterministic Rust discrete-event "
          "engine; the same scenario and seed give the same bytes. `scenarios/<run>.yaml` is "
          "each run's scenario, `runs.parquet` (config `runs`) its parameters and seed."]
    if rows:
        L += [f"- **World and fleet**: a {md(r0.get('world_source'))} street grid (offsets "
              f"from (0, 0), not a real place); {md(r0.get('vehicles'))} vehicles, "
              f"{int(round((r0.get('equipped_fraction') or 0) * 100))} % V2X-equipped, "
              f"{RAT_TEXT.get(r0.get('rat'), md(r0.get('rat')))} radio, BSMs at about "
              f"{md(round(r0.get('bsm_rate_hz') or 0, 1))} Hz; "
              f"{md(r0.get('instrumented_receivers'))} instrumented OBUs per run record "
              "everything they receive.",
              f"- **SCMS in the loop**: enrolment, pseudonym certificates, misbehavior reports "
              "encrypted to the MA, revocation through the Linkage Authorities and the CRL; "
              f"certificate calendar {md(r0.get('calendar'))} to fit the run.",
              f"- **Detection**: every OBU runs `{md(r0.get('local_suite'))}` (12 plausibility "
              f"and consistency checks in the F2MD tradition); the MA runs "
              f"`{md(r0.get('ma_policy'))}` (revoke once the reports against a device cover "
              "enough distinct seconds)."]
    if n_att_veh:
        share = f" ({100 * n_att_veh / n_veh:.1f} %)" if n_veh else ""
        L.append(f"- **Attackers**: {n_att_veh} of {n_veh} vehicles{share}. Each attack runs "
                 "on several vehicles that start at different times and misbehave to the end "
                 "of the run, so how long a sender has been heard does not give it away.")
    if len(used_splits) == 1:
        L.append(f"- **Splits**: per run. This version is **one split** (`{used_splits[0]}`, "
                 f"{plural(len(split_runs[used_splits[0]]), 'run')}): no test run and no "
                 "held-out attack, so group evaluation folds by `cert_id` to keep each "
                 "certificate on one side.")
    else:
        sp = "; ".join(f"{s} {', '.join(split_runs[s])}" for s in used_splits)
        held = ""
        if heldout:
            held = (f" {', '.join(f'`{h}`' for h in heldout)} "
                    f"{'appears' if len(heldout) == 1 else 'appear'} only in test runs, for "
                    "a zero-day test" + (" (absent from every train and validation run)"
                                         if not (set(heldout) & train_attacks) else "") + ".")
        L.append(f"- **Splits**: per run, so one car's messages never straddle train and "
                 f"test: {sp}.{held}")
    attacked = {rid for a in attacks.values() for rid in a.get("runs", ())}
    benign_runs = [r["run_id"] for r in rows
                   if not att[r["run_id"]] and r["run_id"] not in attacked]
    if benign_runs:
        L.append(f"- **Baseline**: {'run' if len(benign_runs) == 1 else 'runs'} "
                 f"{', '.join(benign_runs)} {'has' if len(benign_runs) == 1 else 'have'} "
                 "no attacker.")
    L.append("")

    # ---- tables
    L += ["## Tables\n", "| config | one row is | rows |", "|---|---|---|"]
    for t in order:
        if t.startswith("wire/"):
            continue
        what = ROW_IS.get(t) or md((schemas.get(t) or {}).get("description") or "")
        L.append(f"| `{table_key(t)}` | {what} | {total_rows[t]:,} |")
    L += [f"| `runs` | a run: scenario, seed, parameters, engine version | {len(rows)} |", "",
          "Every column's type, unit, who could see it and what it means is in "
          "`schema/<config>.json` (for example `schema/local_rx.json`) and in "
          "`croissant.json`. Times `t_*_ns` are nanoseconds of simulated time from the run's "
          "start; `lat_deg` / `lon_deg` are degrees, `x_m` / `y_m` metres on the map.\n"]

    # ---- example rows (the owner's approved style: unchanged)
    L += ["## Example rows\n", *example_rows(out)]

    # ---- attacks
    L += ["## Attacks and the crosswalk\n"]
    if attacks:
        many = len(rows) > 1
        L += ["| attack | family | vehicles (start s) | malicious messages | VeReMi Ext | "
              "VeReMi 1.0 | NextGen | DARE |" + (" runs |" if many else ""),
              "|---|---|---|---|---|---|---|---|" + ("---|" if many else "")]
        for a in sorted(attacks):
            r = attacks[a]
            veh = str(per_attack.get(a, "")) + (
                " (" + ", ".join(num(s) for s in sorted(starts[a])) + ")" if a in starts else "")
            L.append(f"| {a} | {md(r.get('family'))} | {veh} | {r.get('messages', 0):,} | "
                     + " | ".join(md(r.get(k)) for k in ("veremi_ext_code", "veremi1_code",
                                                          "nextgen_name", "dare_name"))
                     + (f" | {', '.join(sorted(r.get('runs', [])))} |" if many else " |"))
        L += ["", "The crosswalk gives the closest code or name in VeReMi Extension (F2MD), "
              "VeReMi 1.0, VeReMi NextGen and DARE; empty where that codebook has no such "
              "attack."]
    if silent:
        L.append("")
    for (n, k, f, to), rs in sorted(silent.items(), key=lambda x: x[0][:2]):
        win = f", {num(f)}-{num(to)} s" if f is not None else ""
        L.append(f"**{n}** ({k}{win}{', runs ' + ', '.join(rs) if len(rows) > 1 else ''}) "
                 f"sends no labelled message of its own; its effect would show in "
                 f"{SILENT_WHERE.get(k, 'its effects downstream')}.")
    L.append("")

    # ---- labels
    L += ["## Labels\n",
          "All labels are in `truth/` (and the joined `ml/` copies).\n",
          "- **message**: `truth/transmissions.label` and `attack_id`; per reception "
          "`truth/receptions` (join `local/rx` on `run_id`, `rx_id`). `malicious` = under an "
          "attack, in its window; `faulty` = an honest device whose GNSS or timing is out of "
          "specification; `benign` = everything else.",
          "- **device**: `truth/devices.label`, `attack_id` and attack window; "
          "`truth/pseudonyms` maps each `cert_id` to its device.",
          "- **report**: `truth/reports.verdict`, `true_accusation` or `false_accusation` "
          "(join on `report_id`).",
          "- **outcome**: `truth/timeline`, per bad or falsely accused device, the time of each "
          "step from attack start to the CRL reaching its neighbours.\n"]
    if label_counts:
        L += ["Transmissions by label: " + ", ".join(
            f"{k} {v:,}" for k, v in sorted(label_counts.items())) + ".\n"]

    # ---- loading
    sp = used_splits[0] if used_splits else "train"
    L += ["## How to load it\n",
          "```python\nfrom datasets import load_dataset\n\n"
          f"rx = load_dataset(\"{repo}\", \"local_rx\", split=\"{sp}\")      # what OBUs received\n"
          f"ml = load_dataset(\"{repo}\", \"ml_local\", split=\"{sp}\")      # the same rows with labels\n"
          f"mbr = load_dataset(\"{repo}\", \"global_reports\", split=\"{sp}\")  # the MA's reports\n\n"
          f"# or with pandas, after: huggingface-cli download {repo} --repo-type dataset "
          "--local-dir v2x\n"
          "import pandas as pd\n"
          f"rx = pd.read_parquet(\"v2x/local/rx/split={sp}\")\n"
          f"lab = pd.read_parquet(\"v2x/truth/receptions/split={sp}\")\n"
          "df = rx.merge(lab, on=[\"run_id\", \"rx_id\"], how=\"left\")  # features + label\n"
          "```\n",
          "Other files: `compat/veremi/<run>.zip` (VeReMi Extension traces, so VeReMi tooling "
          "runs unchanged), `compat/dare/<run>.zip` (DARE-style reports, real ids in a "
          "separate `real_ids.json`), `croissant.json` (Croissant 1.0 metadata) and "
          "`manifest.json` (SHA-256 of every file).\n"]

    # ---- validation: one line
    L += ["## Validation\n"]
    if validation:
        npass = sum(1 for v in validation if v.get("ok") is True)
        failed = [v.get("name") for v in validation if v.get("ok") is False]
        names_ = ", ".join(str(v.get("name")) for v in validation)
        line = (f"**All {len(validation)} validation checks pass**" if npass == len(validation)
                else f"**{npass} of {len(validation)} validation checks pass**"
                + (f" (failing: {', '.join(failed)})" if failed else ""))
        line += (f" ({names_}), run by `tools/dataset/validate_release.py` in the simulator "
                 "repository.")
        sc = next((v for v in validation if v.get("name") == "shortcuts"), None)
        if sc and sc.get("verdict"):
            line += (" Shortcut audit, a random forest on metadata only with folds grouped by "
                     f"pseudonym: {md(sc.get('verdict'))}.")
        L.append(line + "\n")
    else:
        L.append("Check a copy with `tools/dataset/validate_release.py` in the simulator "
                 "repository.\n")

    # ---- limitations: the few that matter
    nulls = [f"`{t}.{c}`" for t in order if not t.startswith(("ml/", "wire/"))
             for c in null_columns(out, t)]
    L += ["## Limitations\n",
          f"- **Small sample.** {plural(len(rows), 'short run')} on one procedural grid with "
          f"{md(r0.get('vehicles'))} vehicles: enough to show the format and the chain, not "
          "to rank detectors."
          + (f" Each attack has {min(per_attack.values())}"
             + (f" to {max(per_attack.values())}" if max(per_attack.values())
                != min(per_attack.values()) else "")
             + " attacker vehicles, so per-attack results rest on few pseudonyms."
             if per_attack else ""),
          "- **Simulated, not field data.** Radio, mobility, GNSS error and the SCMS are "
          "models that follow IEEE 1609.2, SAE J2735 and SM-MBR v1.0; no real device produced "
          "these bytes.",
          *[f"- {x}" for x in notes.get("limitations", [])]]
    if nulls:
        L.append("- **Empty in every row** (kept so the schema is the same in every version): "
                 + ", ".join(nulls) + ".")
    L.append("")
    if repairs:
        L += ["**Truth repaired at assembly.** Three known defects of the v1 dataset writer in "
              "`truth/` were repaired by `assemble_release.py` (the VeReMi files are the "
              "exporter's own and do not carry these repairs):\n"]
        L += [f"- {md(n)}" for n in repairs]
        L.append("")

    # ---- licence, citation, changelog
    L += ["## Licence\n",
          "[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). You may share and "
          "adapt the data for any purpose, with attribution.\n",
          "## Citation\n",
          "```bibtex\n@misc{akzaidan_v2x_scms_mbd_2026,\n"
          "  title        = {V2X-SCMS-MBD: local and global views of misbehavior in a "
          "simulated US SCMS deployment},\n"
          "  author       = {akzaidan},\n"
          "  year         = {2026},\n"
          "  publisher    = {Hugging Face},\n"
          f"  howpublished = {{\\url{{https://huggingface.co/datasets/{repo}}}}},\n"
          f"  note         = {{Dataset {name}}}\n}}\n```\n"]
    if notes.get("changes"):
        L += [f"## Changes in v{version}\n" if version else "## Changes\n",
              *[f"- {c}" for c in notes["changes"]], ""]
    return "\n".join(L)
