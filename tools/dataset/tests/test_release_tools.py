"""assemble_release.py and hf_upload.py on the make_fixture.py per-run outputs.

Run: python -I -m pytest tools/dataset/tests -q
(with mlcroissant installed, test_croissant_passes_mlcroissant also validates the JSON-LD;
it is skipped otherwise and the structural test still runs.)
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import time
import types
import zipfile
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq
import pytest
import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import assemble_release as ar  # noqa: E402
import hf_upload  # noqa: E402
import make_fixture  # noqa: E402
import validate_release as vr  # noqa: E402

REPO = "example-org/v2x-scms-mbd-mini"
FAKE_TOKEN = "hf_FAKEtokenFORtests0123456789abcdef"


@pytest.fixture(scope="session")
def run_dirs(tmp_path_factory) -> list[Path]:
    out = tmp_path_factory.mktemp("runs") / "runs"
    assert make_fixture.main(["--out", str(out), "--layout", "runs"]) == 0
    return sorted(p for p in out.iterdir() if p.is_dir())


@pytest.fixture(scope="session")
def release(run_dirs, tmp_path_factory) -> Path:
    out = tmp_path_factory.mktemp("release") / "rel"
    ar.assemble(run_dirs, out, "v2x-scms-mbd-mini", repo=REPO)
    return out


def read(p: Path) -> pa.Table:
    return pq.ParquetFile(p).read()


def table(root: Path, t: str) -> pa.Table:
    return pa.concat_tables([read(p) for p in sorted((root / t).rglob("*.parquet"))],
                            promote_options="permissive")


# ---- assemble_release.py ------------------------------------------------------------------

def test_assembled_release_passes_every_check(release):
    bad = [r.line() for r in vr.run_checks(release) if r.ok is not True]
    assert not bad, bad


def test_partitions_land_under_the_runs_split(release):
    runs = {r["run_id"]: r["split"] for r in read(release / "runs.parquet").to_pylist()}
    assert runs == {"r0001": "train", "r0002": "test"}
    for p in release.glob("*/*/split=*/run=*/*.parquet"):
        kv = dict(s.split("=") for s in p.parts if "=" in s)
        assert runs[kv["run"]] == kv["split"], p


def test_split_override(run_dirs, tmp_path):
    out = tmp_path / "rel"
    ar.assemble(run_dirs, out, "x", split_override={"r0002": "validation"})
    assert (out / "local/rx/split=validation/run=r0002").is_dir()
    assert not (out / "local/rx/split=test").exists()
    with pytest.raises(ar.AssembleError, match="unknown run"):
        ar.assemble(run_dirs, tmp_path / "rel2", "x", split_override={"r9999": "test"})


def test_a_one_run_release_is_one_split_and_says_what_does_not_apply(run_dirs, tmp_path):
    # The v1.1 sample is one run in one split: it must pass, and the splits check must name
    # what it could not test rather than claim it.
    out = tmp_path / "one"
    ar.assemble(run_dirs[:1], out, "v2x-scms-mbd-mini", repo=REPO)
    res = {r.name: r for r in vr.run_checks(out)}
    assert not [r.line() for r in res.values() if r.ok is False]
    s = res["splits"]
    assert s.ok is True, s.line()
    assert "held-out: not applicable" in s.verdict, s.verdict
    assert "across splits: not applicable (one split)" in s.verdict, s.verdict
    card = (out / "README.md").read_text()
    assert "1 run of" in card and "**one split** (`train`, 1 run)" in card
    # What is checkable on one split is still checked: the run filed under a split that
    # runs.parquet does not give it fails (every table moved, so only that check sees it),
    # and so does a run that names a held-out attack it runs in train.
    moved = [(p, p.parent.parent / "split=test" / p.name)
             for p in out.glob("*/*/split=train/run=*")]
    assert moved
    for a, b in moved:
        b.parent.mkdir(exist_ok=True)
        shutil.move(str(a), str(b))
    assert vr.check_splits(vr.Release(out)).ok is False
    for a, b in moved:
        shutil.move(str(b), str(a))
    assert vr.check_splits(vr.Release(out)).ok is True
    runs = read(out / "runs.parquet")
    i = runs.schema.get_field_index("heldout_attacks")
    pq.write_table(runs.set_column(i, runs.schema.field(i),
                                   pa.array([["ConstPos"]], type=runs.schema.field(i).type)),
                   out / "runs.parquet")
    assert vr.check_splits(vr.Release(out)).ok is False


def test_assembly_is_deterministic(run_dirs, release, tmp_path):
    # A fresh process (so import-time values are new too), past the 2 s resolution of zip
    # timestamps, so a clock read anywhere shows up; this also runs the command line.
    time.sleep(2.1)
    again = tmp_path / "again"
    r = subprocess.run([sys.executable, "-I", str(HERE.parent / "assemble_release.py"),
                        *map(str, run_dirs), "--out", str(again), "--repo", REPO],
                       capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    assert "release v2x-scms-mbd-mini: 2 runs" in r.stdout
    assert (again / "manifest.json").read_bytes() == (release / "manifest.json").read_bytes()


def test_a_tampered_input_is_refused(run_dirs, tmp_path):
    runs = [tmp_path / d.name for d in run_dirs]
    for d, c in zip(run_dirs, runs):
        shutil.copytree(d, c)
    victim = sorted((runs[0] / "dataset/global/reports").rglob("*.parquet"))[0]
    data = bytearray(victim.read_bytes())
    data[len(data) // 2] ^= 0x01
    victim.write_bytes(bytes(data))
    with pytest.raises(ar.AssembleError, match="sha256 mismatch"):
        ar.assemble(runs, tmp_path / "rel", "x")


def test_schema_drift_between_runs_is_refused(run_dirs, tmp_path):
    runs = [tmp_path / d.name for d in run_dirs]
    for d, c in zip(run_dirs, runs):
        shutil.copytree(d, c)
    doc = runs[1] / "dataset/schema/local_rx.json"
    spec = json.loads(doc.read_text())
    spec["columns"][0]["description"] = "changed"
    doc.write_text(json.dumps(spec))
    with pytest.raises(ar.AssembleError, match="differs between runs"):
        ar.assemble(runs, tmp_path / "rel", "x", verify=False)


def test_refuses_a_non_empty_output(run_dirs, tmp_path):
    (tmp_path / "rel").mkdir()
    (tmp_path / "rel" / "keep.txt").write_text("x")
    with pytest.raises(ar.AssembleError, match="non-empty"):
        ar.assemble(run_dirs, tmp_path / "rel", "x")


def test_ml_tables_carry_the_truth_labels(release):
    rx, ml = table(release, "local/rx"), table(release, "ml/local")
    assert ml.num_rows == rx.num_rows
    rcp = {r["rx_id"]: r["label"] for r in table(release, "truth/receptions").to_pylist()}
    tx = {r["tx_id"]: r["attack_id"] for r in table(release, "truth/transmissions").to_pylist()}
    tx_of = {r["rx_id"]: r["tx_id"] for r in table(release, "truth/receptions").to_pylist()}
    for r in ml.select(["rx_id", "label", "attack_id"]).to_pylist():
        assert r["label"] == rcp[r["rx_id"]]
        assert r["attack_id"] == tx[tx_of[r["rx_id"]]]
    # the writer's ml/local declares veremi_ext_code (release/ml.rs): the join carries it
    code = {r["tx_id"]: r["veremi_ext_code"]
            for r in table(release, "truth/transmissions").to_pylist()}
    for r in ml.select(["rx_id", "veremi_ext_code"]).to_pylist():
        assert r["veremi_ext_code"] == code[tx_of[r["rx_id"]]]
    assert "malicious" in set(ml["label"].to_pylist())
    verdict = {r["report_id"]: r["verdict"] for r in table(release, "truth/reports").to_pylist()}
    for r in table(release, "ml/global").select(["report_id", "verdict"]).to_pylist():
        assert r["verdict"] == verdict[r["report_id"]]


def test_veremi_zip_holds_the_runs_traces(run_dirs, release):
    for d in run_dirs:
        src = sorted(p.name for p in (d / "dataset/compat/veremi").iterdir())
        with zipfile.ZipFile(release / "compat/veremi" / f"{d.name}.zip") as zf:
            assert sorted(zf.namelist()) == src


def test_dare_reports_hold_no_truth_and_real_ids_are_apart(release):
    truth_values = set()
    for t, c in (("truth/devices", "device_id"), ("truth/transmissions", "tx_id")):
        truth_values |= set(table(release, t)[c].to_pylist())
    truth_reports = {r["report_id"]: r for r in table(release, "truth/reports").to_pylist()}
    global_reports = {r["report_id"]: r for r in table(release, "global/reports").to_pylist()}
    seen = set()
    for z in sorted((release / "compat/dare").glob("*.zip")):
        with zipfile.ZipFile(z) as zf:
            names = zf.namelist()
            real = json.loads(zf.read("real_ids.json"))
            for n in (n for n in names if n.startswith("reports/")):
                text = zf.read(n).decode()
                rep = json.loads(text)["Report"]
                md = rep["Metadata"]
                rid = md["reportId"]
                seen.add(rid)
                for key in ("senderRealId", "reportedRealId", "attackType"):
                    assert key not in text, f"{n}: {key} inside an observer report"
                leaked = [v for v in truth_values if v in text]
                assert not leaked, f"{n}: truth-only ids {leaked[:3]}"
                g = global_reports[rid]
                assert md["reportedId"] == g["subject_cert_id"]
                assert md["senderId"] == g["reporter_cert_id"]
                assert [m["messageId"] for m in rep["Messages"]] == \
                    [e["spdu_digest"] for e in g["evidence"]]
                assert set(rep["Checks"][0]) == {s["name"] for s in g["subtypes"]} | \
                    set(g.get("nonstandard_triggers") or [])
                t = truth_reports[rid]
                assert real[rid]["reportedRealId"] == t["subject_device_id"]
                assert real[rid]["senderRealId"] == t["reporter_device_id"]
                assert real[rid]["verdict"] == t["verdict"]
    assert seen == set(global_reports)


def test_card_header_is_a_valid_hugging_face_config(release):
    text = (release / "README.md").read_text()
    assert text.startswith("---\n")
    head = yaml.safe_load(text.split("---\n", 2)[1])
    assert head["license"] == "cc-by-4.0"
    assert head["pretty_name"] and head["tags"] and head["task_categories"]
    names = {c["config_name"]: c for c in head["configs"]}
    for t in sorted({f"{p.parts[-5]}/{p.parts[-4]}"
                     for p in release.glob("*/*/split=*/run=*/*.parquet")}):
        assert ar.table_key(t) in names, f"no config for {t}"
    for c in head["configs"]:
        for d in c["data_files"]:
            # Hugging Face `datasets` refuses `all` as a split name (it means the union of
            # the splits), so the viewer could not load such a config.
            assert d["split"] in ("train", "validation", "test")
            hits = list(release.glob(d["path"]))
            assert hits, f"{c['config_name']} {d['split']}: {d['path']} matches nothing"
            if c["config_name"] != "runs":
                assert all(f"split={d['split']}" in h.as_posix() for h in hits)
    assert sum(1 for c in head["configs"] if c.get("default")) == 1
    body = text.split("---\n", 2)[2]
    for section in ("## The three views", "## How it was generated", "## Tables",
                    "## Example rows", "## Attacks and the crosswalk", "## Labels",
                    "## Citation"):
        assert section in body
    assert "| ConstPos |" in body and "| RandomPos |" in body


def test_card_is_finished(release):
    """The published card names no placeholder, opens on the OBU's view, and cites the
    account handle only."""
    text = (release / "README.md").read_text()
    head = yaml.safe_load(text.split("---\n", 2)[1])
    assert [c["config_name"] for c in head["configs"] if c.get("default")] == ["local_rx"]
    assert head["configs"][0]["config_name"] == "local_rx"
    for tag in ("v2x", "misbehavior-detection", "vehicular-networks", "scms", "ieee-1609.2",
                "j2735"):
        assert tag in head["tags"]
    body = text.split("---\n", 2)[2]
    for bad in ("TBD", "TODO", "<org>", "Zenodo"):
        assert bad not in body, bad
    assert "author       = {akzaidan}" in body
    for section in ("## How to load it", "## Validation", "## Limitations", "## Licence"):
        assert section in body
    croissant = json.loads((release / "croissant.json").read_text())
    assert "TBD" not in croissant["citeAs"]


def test_card_is_short(release):
    """The card is read by people new to the data: the essentials only. Columns are in
    schema/<table>.json, not listed in the card; one line per table, at most six
    limitations, and nothing of the long sections the owner cut."""
    text = (release / "README.md").read_text()
    body = text.split("---\n", 2)[2]
    # the YAML header grows with the splits; what a reader reads is the body (v1.1: 870)
    assert len(body.splitlines()) < 350, len(body.splitlines())
    for cut in ("## Contents", "## Datasheet", "## Why this dataset", "## Runs and splits",
                "## Compatibility files", "## Validation results", "## Changes", "<details>"):
        assert cut not in body, cut
    assert "`schema/<config>.json`" in body
    tables = body.split("## Tables\n", 1)[1].split("\n## ", 1)[0]
    listed = [x for x in tables.splitlines() if x.startswith("| `")]
    head = yaml.safe_load(text.split("---\n", 2)[1])
    names = [x.split("`")[1] for x in listed]
    assert sorted(names) == sorted(c["config_name"] for c in head["configs"])  # one line each
    lim = body.split("## Limitations\n", 1)[1].split("\n## ", 1)[0]
    assert 2 <= sum(1 for x in lim.splitlines() if x.startswith("- ")) <= 6


def jsonc_blocks(card_text: str) -> list[tuple[str, dict, list[str]]]:
    """(the bold line before it, the parsed row, the comment lines) of every jsonc block."""
    import re
    out = []
    parts = card_text.split("```jsonc\n")
    for before, block in zip(parts, parts[1:]):
        body = block.split("```", 1)[0]
        lead = [x for x in before.strip().splitlines() if x.strip()][-1]
        lines = body.splitlines()
        plain = [re.sub(r" // [^\"\n]*$", "", x) for x in lines]
        out.append((lead, json.loads("\n".join(plain)), [x for x in lines if " // " in x]))
    return out


def test_card_example_rows_are_annotated_rows_of_the_release(release):
    """Each example is a real row of its table under a one-line bold description, with
    `//` comments on the non-obvious fields, and says "not recorded" only for a column that
    is null in every row."""
    card_text = (release / "README.md").read_text()
    blocks = jsonc_blocks(card_text)
    assert [b[0].split("`")[1] for b in blocks] == [t for t, _, _ in card_mod().EXAMPLES]
    for lead, row, comments in blocks:
        t = lead.split("`")[1]
        assert lead.startswith(f"**`{t}`**: ") and len(lead) > len(t) + 20, lead
        assert len(comments) >= 3, (t, comments)
        tb = table(release, t)
        assert set(row) == set(tb.column_names) - {"split", "run"}, t
        key = {"local/rx": "rx_id", "local/reports": "report_id", "global/reports": "report_id",
               "global/decisions": "t_ns", "truth/transmissions": "tx_id"}[t]
        assert row[key] in tb[key].to_pylist(), (t, row[key])
        for c in comments:
            name = c.strip().split('"')[1]
            if "(not recorded in this sample)" in c:
                assert tb[name].null_count == tb.num_rows, (t, name)
    mal = blocks[-1][1]
    assert mal["label"] == "malicious" and mal["attack_id"]
    by_t = {b[0].split("`")[1]: b[1] for b in blocks}
    assert by_t["local/reports"]["report_id"] == by_t["global/reports"]["report_id"]
    # "not recorded" is said of a column null in every row, not of one null value.
    lines = card_mod().jsonc({"some": None, "never": None, "pair": None, "deep": {"x": 1}},
                             {"some": "a note", "pair": ("filled", "empty"), "deep.x": "inner"},
                             {"never"})
    assert lines[1].endswith("// a note") and "not recorded" not in lines[1], lines
    assert lines[2].endswith("// (not recorded in this sample)"), lines
    assert lines[3].endswith("// empty"), lines
    assert lines[5].endswith("// inner"), lines
    assert all(len(x.split(" //")[0]) >= 40 for x in lines if " //" in x), lines


def test_card_carries_the_release_notes(run_dirs, tmp_path):
    """The one-run statement and the version note open the card; a changelog, when given,
    comes last; the validation result is one line with the real outcome."""
    out = tmp_path / "noted"
    notes = {"version_note": "v1.1 replaces v1.0, a version note.",
             "changes": ["**One run instead of seven.** a change line"],
             "limitations": ["**A limit.** a limitation line"]}
    ok = [{"name": "schema", "ok": True, "verdict": "18 tables conform"},
          {"name": "shortcuts", "ok": True,
           "verdict": "metadata-only ROC-AUC ConstPos 0.54 (limit 0.7)"}]
    ar.assemble(run_dirs[:1], out, "v2x-scms-mbd-mini", repo=REPO, version="1.1",
                card_notes=notes, validation=ok)
    text = (out / "README.md").read_text()
    assert len(text.splitlines()) < 450, len(text.splitlines())  # v1.1 was 967
    summary = text.split("---\n", 2)[2].split("\n## ", 1)[0]
    assert "Version 1.1 is a **one-run sample**: 1 run of" in summary
    assert "full multi-run release follows" in summary
    assert "v1.1 replaces v1.0, a version note." in summary
    head, rest = text.split("## Changes in v1.1\n", 1)
    assert "## Citation" in head and "\n## " not in rest  # the changelog is last
    assert rest.strip() == "- **One run instead of seven.** a change line"
    lim = text.split("## Limitations\n", 1)[1].split("\n## ", 1)[0]
    assert "- **A limit.** a limitation line" in lim
    val = text.split("## Validation\n", 1)[1].split("\n## ", 1)[0].strip()
    assert len(val.splitlines()) == 1, val
    assert val.startswith("**All 2 validation checks pass** (schema, shortcuts)"), val
    assert "metadata-only ROC-AUC ConstPos 0.54 (limit 0.7)" in val
    assert 'split="train"' in text
    # the attacks: one row per attack in truth/transmissions, with its crosswalk columns
    tx = table(out, "truth/transmissions")
    att = sorted({a for a in tx["attack_id"].to_pylist() if a})
    sec = text.split("## Attacks and the crosswalk\n", 1)[1].split("\n## ", 1)[0]
    assert "| attack | family | vehicles (start s) | malicious messages | VeReMi Ext |" in sec
    assert [x.split(" | ")[0][2:] for x in sec.splitlines()
            if x.startswith("| ") and not x.startswith(("| attack ", "|---"))] == att
    plain = tmp_path / "plain"
    bad = [ok[0], {"name": "shortcuts", "ok": False, "verdict": "ConstPos 0.9 (limit 0.7)"}]
    ar.assemble(run_dirs, plain, "v2x-scms-mbd-mini", repo=REPO, validation=bad)
    t2 = (plain / "README.md").read_text()
    assert "## Changes" not in t2 and "one-run sample" not in t2
    assert "**1 of 2 validation checks pass** (failing: shortcuts)" in t2


def card_mod():
    import card
    return card


def test_attacker_fraction_counts_named_vehicles():
    runs = pa.table({"vehicles": [40], "attacker_fraction": [-0.0]})
    sc = yaml.safe_dump({"threats": {"attackers": [
        {"id": "a/ConstPos", "actor_ids": [3, 9]}, {"id": "a/Sybil", "actor_ids": [9]},
        {"id": "a/RandomPos", "fraction": 0.05}]}}).encode()
    assert ar.fix_attacker_fraction(runs, sc)["attacker_fraction"][0].as_py() == 0.1
    none = yaml.safe_dump({"threats": {"attackers": []}}).encode()
    assert ar.fix_attacker_fraction(runs, none)["attacker_fraction"][0].as_py() == -0.0


def test_croissant_structure(release):
    doc = json.loads((release / "croissant.json").read_text())
    assert doc["conformsTo"] == "http://mlcommons.org/croissant/1.0"
    assert doc["@context"]["cr"] == "http://mlcommons.org/croissant/"
    assert doc["@context"]["rai"] == "http://mlcommons.org/croissant/RAI/"
    assert doc["license"].startswith("https://creativecommons.org/licenses/by/4.0")
    assert sum(1 for k in doc if k.startswith("rai:")) >= 8
    sets = {d["@id"]: d for d in doc["distribution"] if d["@type"] == "cr:FileSet"}
    rsets = {r["@id"]: r for r in doc["recordSet"]}
    tables = sorted({f"{p.parts[-5]}/{p.parts[-4]}"
                     for p in release.glob("*/*/split=*/run=*/*.parquet")})
    assert sorted(rsets) == sorted(ar.table_key(t) for t in tables)
    for t in tables:
        key = ar.table_key(t)
        assert list(release.glob(sets[f"{key}-files"]["includes"])), key
        cols = pq.ParquetFile(sorted((release / t).rglob("*.parquet"))[0]).schema_arrow.names
        fields = rsets[key]["field"]
        assert [f["name"] for f in fields] == cols, key
        for f in fields:
            assert f["source"]["fileSet"]["@id"] == f"{key}-files"
            assert f["source"]["extract"]["column"] == f["name"]


def test_croissant_passes_mlcroissant(release):
    mlc = pytest.importorskip("mlcroissant")
    ds = mlc.Dataset(jsonld=str(release / "croissant.json"))  # raises on any error
    assert len(ds.metadata.record_sets) == len(json.loads(
        (release / "croissant.json").read_text())["recordSet"])


# ---- hf_upload.py -------------------------------------------------------------------------

@pytest.fixture()
def fake_hub(monkeypatch):
    calls = []

    class HfApi:
        def __init__(self, token=None):
            calls.append(("init", token))

        def create_repo(self, **kw):
            calls.append(("create_repo", kw))

        def upload_folder(self, **kw):
            calls.append(("upload_folder", kw))

        def upload_large_folder(self, **kw):
            calls.append(("upload_large_folder", kw))
    mod = types.ModuleType("huggingface_hub")
    mod.HfApi = HfApi
    monkeypatch.setitem(sys.modules, "huggingface_hub", mod)
    monkeypatch.delenv("HF_TOKEN", raising=False)
    return calls


@pytest.fixture()
def env_file(tmp_path) -> Path:
    p = tmp_path / ".env"
    p.write_text(f"# test\nOTHER=1\nHF_TOKEN={FAKE_TOKEN}\n")
    return p


def test_dry_run_lists_and_uploads_nothing(release, fake_hub, env_file, capsys):
    rc = hf_upload.main(["--root", str(release), "--repo", REPO, "--env", str(env_file),
                         "--no-validate"])
    out = capsys.readouterr().out
    assert rc == 0
    assert fake_hub == []
    n = len(json.loads((release / "manifest.json").read_text())["files"]) + 1
    assert f"DRY RUN: {n} files" in out
    assert "nothing uploaded" in out
    assert FAKE_TOKEN not in out and "(not shown)" in out


def test_yes_uploads_the_folder_as_a_dataset(release, fake_hub, env_file, capsys):
    rc = hf_upload.main(["--root", str(release), "--repo", REPO, "--env", str(env_file),
                         "--no-validate", "--private", "--yes"])
    out = capsys.readouterr()
    assert rc == 0
    kinds = [c[0] for c in fake_hub]
    assert kinds == ["init", "create_repo", "upload_folder"]
    assert fake_hub[0][1] == FAKE_TOKEN
    assert fake_hub[1][1]["repo_type"] == "dataset" and fake_hub[1][1]["private"] is True
    assert fake_hub[2][1]["folder_path"] == str(release)
    assert fake_hub[2][1]["repo_id"] == REPO
    assert FAKE_TOKEN not in out.out + out.err


def test_sync_deletes_hub_files_the_release_lacks_in_the_same_commit(release, fake_hub,
                                                                      env_file, capsys):
    rc = hf_upload.main(["--root", str(release), "--repo", REPO, "--env", str(env_file),
                         "--no-validate", "--sync", "--yes"])
    assert rc == 0
    up = [c[1] for c in fake_hub if c[0] == "upload_folder"]
    assert len(up) == 1 and up[0]["delete_patterns"] == ["*"]
    assert "Hub files not in the release are deleted" in capsys.readouterr().out
    fake_hub.clear()
    assert hf_upload.main(["--root", str(release), "--repo", REPO, "--env", str(env_file),
                           "--no-validate", "--yes"]) == 0
    assert [c[1] for c in fake_hub if c[0] == "upload_folder"][0]["delete_patterns"] is None
    fake_hub.clear()
    assert hf_upload.main(["--root", str(release), "--repo", REPO, "--env", str(env_file),
                           "--no-validate", "--sync", "--large", "--yes"]) == 2
    assert fake_hub == []


def test_refuses_a_release_that_contains_the_token(release, fake_hub, env_file, tmp_path,
                                                   capsys):
    rel = tmp_path / "rel"
    shutil.copytree(release, rel)
    with open(rel / "scenarios" / "r0001.yaml", "a") as f:
        f.write(f"# {FAKE_TOKEN}\n")
    rc = hf_upload.main(["--root", str(rel), "--repo", REPO, "--env", str(env_file),
                         "--no-validate", "--yes"])
    out = capsys.readouterr()
    assert rc == 1
    assert [c[0] for c in fake_hub] == []
    assert "contains the HF token" in out.out
    assert FAKE_TOKEN not in out.out + out.err


def test_refuses_a_release_that_fails_validation(release, fake_hub, env_file, tmp_path,
                                                 capsys):
    rel = tmp_path / "rel"
    shutil.copytree(release, rel)
    with open(rel / "scenarios" / "r0001.yaml", "a") as f:
        f.write("# edited after assembly\n")
    rc = hf_upload.main(["--root", str(rel), "--repo", REPO, "--env", str(env_file), "--yes"])
    assert rc == 1
    assert fake_hub == []
    assert "validate_release: manifest failed" in capsys.readouterr().out


def test_rejects_a_bad_repo_id(release, capsys):
    assert hf_upload.main(["--root", str(release), "--repo", "no-slash"]) == 2


def test_repair_truth_merges_twins_relabels_and_widens(tmp_path):
    """The v1 writer's three truth defects (assemble_release.repair_truth)."""
    part = lambda t: tmp_path / t / "split=train" / "run=r1" / "part-00000.parquet"  # noqa: E731
    for t, rows in {
        "truth/transmissions": [
            {"tx_id": 1, "device_id": 7, "t_ns": 100, "label": "malicious", "attack_id": "A"},
            {"tx_id": 2, "device_id": 7, "t_ns": 200, "label": "benign", "attack_id": None},
            {"tx_id": 2, "device_id": 7, "t_ns": None, "label": "malicious", "attack_id": "A"},
            {"tx_id": 3, "device_id": 7, "t_ns": 900, "label": "malicious", "attack_id": "A"},
        ],
        "truth/receptions": [
            {"rx_id": "x", "tx_id": 2, "label": "benign", "attack_id": None},
            {"rx_id": "y", "tx_id": 1, "label": "malicious", "attack_id": "A"},
        ],
        "truth/devices": [
            {"device_id": 7, "label": "malicious", "attack_id": "A",
             "attack_start_ns": 150, "attack_end_ns": 890},
        ],
    }.items():
        part(t).parent.mkdir(parents=True)
        ar.write_parquet(part(t), pa.Table.from_pylist(rows))
    notes = ar.repair_truth(tmp_path, "train", "r1")
    assert len(notes) == 3, notes
    tx = read(part("truth/transmissions")).to_pylist()
    assert [r["tx_id"] for r in tx] == [1, 2, 3]
    assert tx[1]["label"] == "malicious" and tx[1]["t_ns"] == 200
    assert read(part("truth/receptions")).to_pylist()[0]["label"] == "malicious"
    d = read(part("truth/devices")).to_pylist()[0]
    assert (d["attack_start_ns"], d["attack_end_ns"]) == (100, 900)
    assert ar.repair_truth(tmp_path, "train", "r1") == []  # idempotent


def test_repair_truth_only_checks_unless_asked(tmp_path):
    """By default the assembler checks an export for the v1.0 defects (none in an export
    of the fixed writer) and changes nothing: `fix=False` finds the twin and leaves the
    file as it was."""
    part = tmp_path / "truth/transmissions" / "split=train" / "run=r1" / "part-00000.parquet"
    part.parent.mkdir(parents=True)
    rows = [{"tx_id": 2, "device_id": 7, "t_ns": 200, "label": "benign", "attack_id": None},
            {"tx_id": 2, "device_id": 7, "t_ns": None, "label": "malicious", "attack_id": "A"}]
    ar.write_parquet(part, pa.Table.from_pylist(rows))
    before = part.read_bytes()
    notes = ar.repair_truth(tmp_path, "train", "r1", fix=False)
    assert len(notes) == 1 and "duplicate tx_id" in notes[0], notes
    assert part.read_bytes() == before


def test_an_input_with_v1_truth_defects_is_refused(run_dirs, tmp_path):
    """assemble refuses an export with the v1.0 writer's defects unless --repair-v1-truth."""
    src = Path(run_dirs[0])
    bad = tmp_path / "bad"
    shutil.copytree(src, bad)
    f = next((bad / "dataset" / "truth" / "transmissions").rglob("*.parquet"), None) \
        or next((bad / "truth" / "transmissions").rglob("*.parquet"))
    t = read(f)
    t = t.cast(pa.schema([x.with_nullable(True) for x in t.schema]))
    twin = t.slice(0, 1).to_pylist()
    for c in ("t_ns", "t_tx_ns"):
        if c in twin[0]:
            twin[0][c] = None
    twin[0]["label"] = "malicious"
    ar.write_parquet(f, pa.concat_tables([t, pa.Table.from_pylist(twin, schema=t.schema)]))
    with pytest.raises(ar.AssembleError, match="v1.0 writer"):
        ar.assemble([bad] + list(run_dirs[1:]), tmp_path / "out", "x", verify=False)
    res = ar.assemble([bad] + list(run_dirs[1:]), tmp_path / "out2", "x", verify=False,
                      repair_v1=True)
    assert any("duplicate tx_id" in n for n in res["notes"])


def test_a_run_without_reports_gets_no_empty_ml_file(run_dirs, tmp_path):
    """Hugging Face `datasets` cannot read a Parquet file with no row group, so a run with
    no MA reports (a benign baseline) must not get an empty ml/global file."""
    dirs = []
    for d in run_dirs:
        shutil.copytree(d, tmp_path / d.name)
        dirs.append(tmp_path / d.name)
    gone = sorted(dirs[1].rglob("global/reports/**/*.parquet"))
    assert gone
    for f in gone:
        ar.write_parquet(f, read(f).slice(0, 0))
    out = tmp_path / "rel"
    ar.assemble(dirs, out, "x", repo=REPO, verify=False)
    rid = read(out / "runs.parquet")["run_id"][1].as_py()
    assert not list(out.glob(f"ml/global/split=*/run={rid}/*.parquet"))
    for f in out.glob("ml/*/split=*/run=*/*.parquet"):
        assert pq.ParquetFile(f).num_row_groups > 0, f
