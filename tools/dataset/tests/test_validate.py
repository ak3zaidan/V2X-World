"""Each validate_release.py check passes on the clean fixture and fails on its own fault.

Run: python -I -m pytest tools/dataset/tests -q

The fixture release is built once per session by make_fixture.py; every test copies it,
injects one fault (rewriting a Parquet file with its own Arrow types, so only the faulted
property changes) and runs only the check that owns that property.
"""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path

import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq
import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import make_fixture  # noqa: E402
import validate_release as vr  # noqa: E402


# ---- helpers -----------------------------------------------------------------------------

@pytest.fixture(scope="session")
def clean_release(tmp_path_factory) -> Path:
    out = tmp_path_factory.mktemp("fixture") / "rel"
    assert make_fixture.main(["--out", str(out), "--layout", "release"]) == 0
    return out


@pytest.fixture()
def rel(clean_release, tmp_path) -> Path:
    dst = tmp_path / "rel"
    shutil.copytree(clean_release, dst)
    return dst


def part(root: Path, table: str, run: str = "r0001") -> Path:
    found = sorted((root / table).glob(f"split=*/run={run}/*.parquet"))
    assert found, f"no partition for {table} run {run}"
    return found[0]


def read(path: Path) -> pa.Table:
    """The file's own columns (pq.read_table would add the hive split/run columns)."""
    return pq.ParquetFile(path).read()


def rewrite(path: Path, fn) -> None:
    tbl = read(path)
    pq.write_table(fn(tbl), path, compression="zstd")


def set_col(tbl: pa.Table, name: str, values: list) -> pa.Table:
    i = tbl.schema.get_field_index(name)
    return tbl.set_column(i, tbl.schema.field(i), pa.array(values, type=tbl.schema.field(i).type))


def edit_col(path: Path, name: str, fn) -> None:
    """Replace column `name` with fn(list of its values)."""
    rewrite(path, lambda t: set_col(t, name, fn(t.column(name).to_pylist())))


def check(root: Path, name: str) -> vr.Result:
    (res,) = vr.run_checks(root, {name})
    return res


def red(root: Path, name: str) -> vr.Result:
    res = check(root, name)
    assert res.ok is False, f"{name} should fail but said: {res.line()}"
    return res


def malicious_rx_ids(root: Path, run: str = "r0001") -> set[str]:
    rcp = read(part(root, "truth/receptions", run))
    return set(pc.filter(rcp, pc.equal(rcp["label"], "malicious"))["rx_id"].to_pylist())


# ---- the clean fixture passes everything --------------------------------------------------

def test_clean_fixture_passes_every_check(rel):
    results = vr.run_checks(rel)
    assert [r.name for r in results] == list(vr.CHECKS)
    bad = [r.line() for r in results if r.ok is not True]
    assert not bad, bad


def test_cli_exit_codes(rel, capsys):
    assert vr.main([str(rel), "--only", "joins,manifest"]) == 0
    assert vr.main([str(rel), "--only", "nosuchcheck"]) == 2
    (rel / "scenarios" / "r0001.yaml").write_text("tampered\n")
    assert vr.main([str(rel), "--only", "manifest"]) == 1
    assert "FAIL manifest" in capsys.readouterr().out


# ---- schema -------------------------------------------------------------------------------

def test_schema_fails_on_a_missing_column(rel):
    rewrite(part(rel, "local/rx"), lambda t: t.drop(["heading_deg"]))
    assert "heading_deg" in json.dumps(red(rel, "schema").problems)


def test_schema_fails_on_a_wrong_type(rel):
    p = part(rel, "local/ego")
    rewrite(p, lambda t: t.set_column(t.schema.get_field_index("speed_mps"), "speed_mps",
                                      pc.cast(t["speed_mps"], pa.string())))
    assert "speed_mps is string" in json.dumps(red(rel, "schema").problems)


def test_schema_fails_on_a_null_in_a_required_column(rel):
    def fault(t):
        i = t.schema.get_field_index("spdu_digest")
        v = t.column(i).to_pylist()
        return t.set_column(i, pa.field("spdu_digest", pa.string(), nullable=True),
                            pa.array([None] + v[1:], type=pa.string()))
    rewrite(part(rel, "local/rx"), fault)
    assert "spdu_digest has 1 nulls" in json.dumps(red(rel, "schema").problems)


def test_schema_follows_the_writers_schema_docs(rel):
    """A column the writer's schema/<table>.json declares (not in schema.md) is required."""
    doc = rel / "schema" / "local_rx.json"
    spec = json.loads(doc.read_text())
    spec["columns"].append(dict(name="writer_only", type="int32", nullable=False))
    doc.write_text(json.dumps(spec))
    assert "writer_only" in json.dumps(red(rel, "schema").problems)


def test_schema_falls_back_to_schema_md_names_without_docs(rel):
    shutil.rmtree(rel / "schema")
    res = check(rel, "schema")
    assert res.ok is True and "fallback" in res.verdict, res.line()
    rewrite(part(rel, "local/rx"), lambda t: t.drop(["spdu_digest"]))
    assert "spdu_digest" in json.dumps(red(rel, "schema").problems)


def _rs(t: pa.DataType, top: bool = True) -> str:
    """The arrow-rs text the Rust writer puts in schema/<table>.json for type t."""
    if pa.types.is_list(t):
        return ('List(Field { name: "item", data_type: ' + _rs(t.value_type, False)
                + ', nullable: true, dict_id: 0, dict_is_ordered: false, metadata: {} })')
    if pa.types.is_struct(t):
        if top:
            return "Struct(" + ", ".join(f"{f.name} {_rs(f.type, False)}" for f in t) + ")"
        return "Struct([" + ", ".join(
            f'Field {{ name: "{f.name}", data_type: {_rs(f.type, False)}, nullable: true, '
            'dict_id: 0, dict_is_ordered: false, metadata: {} }' for f in t) + "])"
    return {"string": "Utf8", "double": "Float64", "bool": "Boolean"}.get(str(t), str(t).title())


def test_schema_reads_the_rust_writers_arrow_type_text(rel):
    """schema/*.json from the Rust writer spells types as arrow-rs prints them."""
    for doc in sorted((rel / "schema").glob("*.json")):
        spec = json.loads(doc.read_text())
        table = spec["table"]
        files = sorted((rel / table).glob("split=*/run=*/*.parquet")) if table != "runs" else [
            rel / "runs.parquet"]
        if not files:
            continue
        have = {f.name: f.type for f in pq.ParquetFile(files[0]).schema_arrow}
        for c in spec["columns"]:
            if c["name"] in have:
                c.pop("type", None)
                c["arrow_type"] = _rs(have[c["name"]])
        doc.write_text(json.dumps(spec))
    res = check(rel, "schema")
    assert res.ok is True, res.line()
    doc = rel / "schema" / "local_rx.json"
    spec = json.loads(doc.read_text())
    (pa_col,) = [c for c in spec["columns"] if c["name"] == "pos_accuracy"]
    pa_col["arrow_type"] = pa_col["arrow_type"].replace("Float64", "Int64", 1)
    doc.write_text(json.dumps(spec))
    assert "pos_accuracy" in json.dumps(red(rel, "schema").problems)


def test_schema_fails_on_an_undeclared_column(rel):
    rewrite(part(rel, "global/crl"),
            lambda t: t.append_column("note", pa.array(["x"] * t.num_rows)))
    assert "undeclared" in json.dumps(red(rel, "schema").problems)


# ---- joins --------------------------------------------------------------------------------

def test_joins_fail_on_an_unknown_evidence_digest(rel):
    def fault(rows):
        rows[0] = [dict(e, spdu_digest="00" * 8) for e in rows[0]]
        return rows
    edit_col(part(rel, "global/reports"), "evidence", fault)
    red(rel, "joins")


def test_joins_fail_on_an_rx_id_missing_from_truth(rel):
    rewrite(part(rel, "truth/receptions"), lambda t: t.slice(1))
    red(rel, "joins")


def test_joins_fail_on_a_report_id_only_in_local(rel):
    edit_col(part(rel, "local/reports"), "report_id",
             lambda v: ["ffffffffffffffff"] + v[1:])
    red(rel, "joins")


def test_joins_fail_on_an_uploaded_report_without_its_evidence(rel):
    path = part(rel, "local/reports")
    fates = read(path).column("fate").to_pylist()
    i = fates.index("uploaded")
    edit_col(path, "evidence_spdu_digests", lambda v: v[:i] + [[]] + v[i + 1:])
    res = red(rel, "joins")
    assert "with no evidence" in " ".join(res.problems), res.problems


# ---- leakage ------------------------------------------------------------------------------

def test_leakage_fails_on_a_truth_column_in_local(rel):
    def fault(t):
        return t.append_column("device_id", pa.array(["v0001"] * t.num_rows))
    rewrite(part(rel, "local/rx"), fault)
    assert "device_id" in json.dumps(red(rel, "leakage").problems)


def test_leakage_fails_on_a_truth_only_value_in_global(rel):
    dev = read(part(rel, "truth/devices"))["device_id"][0].as_py()
    edit_col(part(rel, "global/decisions"), "rule", lambda v: [dev] + v[1:])
    assert dev in json.dumps(red(rel, "leakage").problems)


def test_leakage_reads_only_id_named_integers_as_ids(rel):
    """An integer psid equal to a truth id is not a leak; an integer *_id column is."""
    edit_col(part(rel, "truth/transmissions"), "tx_id", lambda v: ["135"] + v[1:])
    edit_col(part(rel, "local/rx"), "psid", lambda v: [135] * len(v))
    res = check(rel, "leakage")
    assert res.ok is True, res.line()
    rewrite(part(rel, "local/rx"),
            lambda t: t.append_column("neighbour_id", pa.array([135] * t.num_rows, pa.int64())))
    assert "neighbour_id" in json.dumps(red(rel, "leakage").problems)


# ---- labels -------------------------------------------------------------------------------

def test_labels_fail_on_a_malicious_message_without_attack_id(rel):
    p = part(rel, "truth/transmissions")
    lab = read(p)["label"].to_pylist()
    i = lab.index("malicious")
    edit_col(p, "attack_id", lambda v: v[:i] + [None] + v[i + 1:])
    red(rel, "labels")


def test_labels_fail_on_a_benign_message_with_attack_id(rel):
    p = part(rel, "truth/transmissions")
    lab = read(p)["label"].to_pylist()
    i = lab.index("benign")
    edit_col(p, "attack_id", lambda v: v[:i] + ["ConstPos"] + v[i + 1:])
    red(rel, "labels")


def test_labels_fail_on_a_poisoned_report_from_an_honest_reporter(rel):
    # A poisoned report is an attacker's forgery: marked on an honest reporter's report,
    # the label is wrong. Unmarked, the same release passes.
    p = part(rel, "truth/reports")
    n = read(p).num_rows
    rewrite(p, lambda t: t.append_column("poisoned", pa.array([False] * n)))
    assert check(rel, "labels").ok
    dev = read(part(rel, "truth/devices")).to_pylist()
    honest = {d["device_id"] for d in dev if d["label"] == "benign"}
    rep = read(p)["reporter_device_id"].to_pylist()
    i = next(k for k, r in enumerate(rep) if r in honest)
    edit_col(p, "poisoned", lambda v: v[:i] + [True] + v[i + 1:])
    red(rel, "labels")


def test_labels_fail_when_attackers_do_not_match_attack_mix(rel):
    def fault(rows):
        rows[0] = [dict(m, fraction=0.5) for m in rows[0]]
        return rows
    edit_col(rel / "runs.parquet", "attack_mix", fault)
    red(rel, "labels")


def test_labels_add_up_the_populations_of_one_attack(rel):
    # The v1.1 sample runs each attack as three populations, one per start time: the
    # attackers of all three are the attack's vehicles. Split run 1's attack in two halves
    # and it still holds; two full copies claim twice the attackers and fail.
    def split(rows):
        rows[0] = [dict(m, fraction=m["fraction"] / 2) for m in rows[0] for _ in (0, 1)]
        return rows
    edit_col(rel / "runs.parquet", "attack_mix", split)
    res = check(rel, "labels")
    assert res.ok is True, res.line()

    def double(rows):
        rows[0] = [dict(m, fraction=m["fraction"] * 2) for m in rows[0]]
        return rows
    edit_col(rel / "runs.parquet", "attack_mix", double)
    red(rel, "labels")


# ---- time ---------------------------------------------------------------------------------

def test_time_fails_on_a_timestamp_after_the_run(rel):
    edit_col(part(rel, "local/rx"), "t_rx_ns", lambda v: [v[0] + 10**12] + v[1:])
    red(rel, "time")


def test_time_fails_on_a_negative_timestamp(rel):
    edit_col(part(rel, "local/checks"), "t_ns", lambda v: [-1] + v[1:])
    red(rel, "time")


def test_time_fails_when_a_receiver_goes_back_in_time(rel):
    p = part(rel, "local/rx")
    refs = read(p)["receiver_ref"].to_pylist()

    def fault(v):
        # the first two rows of the first receiver, swapped
        i = refs.index(refs[0])
        j = next(k for k in range(i + 1, len(refs)) if refs[k] == refs[0] and v[k] > v[i])
        v[i], v[j] = v[j], v[i]
        return v
    edit_col(p, "t_rx_ns", fault)
    red(rel, "time")


# ---- units --------------------------------------------------------------------------------

def test_units_fail_on_an_own_speed_above_90_mps(rel):
    edit_col(part(rel, "local/ego"), "speed_mps", lambda v: [95.0] + v[1:])
    red(rel, "units")


def test_units_fail_on_a_position_outside_the_area(rel):
    edit_col(part(rel, "local/rx"), "lat_deg", lambda v: [v[0] + 1.0] + v[1:])
    red(rel, "units")


# ---- splits -------------------------------------------------------------------------------

def test_splits_fail_when_one_run_is_in_two_splits(rel):
    src = part(rel, "local/rx", "r0001").parent
    dst = rel / "local/rx/split=test/run=r0001"
    shutil.copytree(src, dst)
    red(rel, "splits")


def test_splits_fail_when_a_heldout_attack_is_in_train(rel):
    def fault(rows):
        return [["ConstPos"] if s == "train" else h for h, s in zip(rows, splits)]
    splits = read(rel / "runs.parquet")["split"].to_pylist()
    edit_col(rel / "runs.parquet", "heldout_attacks", fault)
    red(rel, "splits")


# ---- manifest -----------------------------------------------------------------------------

def test_manifest_fails_on_a_changed_byte(rel):
    p = part(rel, "global/crl")
    data = bytearray(p.read_bytes())
    data[len(data) // 2] ^= 0x01
    p.write_bytes(bytes(data))
    assert "sha256 mismatch" in json.dumps(red(rel, "manifest").problems)


def test_manifest_fails_on_an_unlisted_file(rel):
    (rel / "global" / "extra.parquet").write_bytes(b"x")
    assert "not in manifest" in json.dumps(red(rel, "manifest").problems)


# ---- shortcuts ----------------------------------------------------------------------------

def test_shortcuts_fail_when_size_gives_away_the_attacker(rel):
    for run in ("r0001", "r0002"):
        bad = malicious_rx_ids(rel, run)
        p = part(rel, "local/rx", run)
        ids = read(p)["rx_id"].to_pylist()
        edit_col(p, "size_bytes",
                 lambda v: [s + 120 if i in bad else s for s, i in zip(v, ids)])
    res = red(rel, "shortcuts")
    assert "ROC-AUC" in res.verdict


def test_f2md_floods_under_sybil_pseudonyms_are_rate_attacks():
    """DoSRandomSybil and DoSDisruptiveSybil multiply the beacon rate, so a rate feature
    finding them is the attack, not a shortcut (v1.1: ROC-AUC 0.98 and 0.91 once their
    copies were labelled); attacks that are not floods stay audited."""
    for a in ("threat/attacker/f2md/DoSRandomSybil", "DoSDisruptiveSybil", "DoSDisruptive"):
        assert vr.attack_key(a) in vr.RATE_OR_SIZE_ATTACKS, a
    for a in ("StopAndGo", "EventualStop", "DataReplaySybil", "PosHeadingInconsistent"):
        assert vr.attack_key(a) not in vr.RATE_OR_SIZE_ATTACKS, a

