"""Engine run -> the real-data SM-MBR checker (stages 3-5), and the K2 agreement check.

  python3 -I tools/realdata/enginebridge.py <run_dir> [--gap 0.1] [--clock secmark]
      [--side tx|rx] [--inject MSG|auto] [--json out.json] [--no-k2]

Reads <run_dir>/jsonl/node_tx.jsonl (node.tx: the J2735 fields each OBU sent), turns every
BSM into the realdata Bsm tuple keyed by temp_id, and runs it through pairing.py and every
mbrchecks check exactly as real data is judged. Rates are triggers per temporary-ID-minute
with exposure = judged pairs x 0.1 s (FIX_INTERVAL_S), by secMark class (devclass.py, per
temp_id over its transmitted secMarks) and speed band (v1).

Sides: `tx` (default) pairs every transmitted BSM of an ID (a perfect receiver); `rx` pairs,
per receiver, the BSMs node.rx says were delivered with a valid signature (what the engine's
detector judged).

Clocks (the pair's dt): secmark (secMark, unwrapped as pairing.py does), generation (the
1609.2 generationTime, node.tx t_generated), receive (tx side: the transmit instant; rx side:
the delivery instant), nominal (0.1 s x msgCnt difference).

K2 (needs node_rx.jsonl, i.e. --record-level receptions or full, and det_observation.jsonl,
i.e. `metrics: [..., detection-scores]`; with the dataset exporter, which needs --record-level
full, node.tx also carries accelSet and brakes):
on the receiver-side pairs the engine's own detector judged (same receiver, same signer,
consecutively received, dt <= its max_gap_s on its time_base), the engine's det.observation
verdicts are joined to the bridge's on the second BSM of each pair, and agreement is
reported per check (both / engine-only / bridge-only). The engine's settings are read from
scenario.resolved.yaml; the bridge is run on the same clock and gap for the join. K2 pairs
are sequenced by arrival (as the engine does), not by secMark time (pairing.py).
det.observation is recorded when the scenario names `metrics: [..., detection-scores]`; the
us-sm-mbr rows carry the judged BSM's msgCnt, its predecessor's (pair checks), the pair's dt_s
and the check's residual, and the join then also requires the msgCnt pair to match the
bridge's pair (a mismatch counts as a disagreement, listed under `disagreements` with both
sides' dt and the engine's residual). Without det.observation K2 falls back to (a) the run report's mbr.verdicts_by_detector totals against the bridge's
receiver-side totals and (b), when the `dataset` exporter was on (node.report_built,
pki.report_fate checks, node.tx spdu_digest), a per-report join on the report's evidence
pair at the reporter. The dataset exporter also adds accelSet and brakes to node.tx, without
which rp11 = rp10 and maxacc_tx / brakeacc cannot fire on the bridge side.

--inject moves one BSM's position 3 m north (MSG = node.tx msg id; auto = the middle BSM of
the first ID with a moving judged pair), so rp10 must rise: the fault proof.
"""
import json
import math
import os
import re
import sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import devclass  # noqa: E402
import mbrchecks as mc  # noqa: E402
import normalise  # noqa: E402
import pairing  # noqa: E402

CLOCKS = ("secmark", "generation", "receive", "nominal")
CLASSES = devclass.CLASSES + ("unclassified",)
INJECT_M = 3.0
# det.observation detector id -> mbrchecks check (positionJump and implausibleAcceleration
# depend on the engine's reading; resolved in engine_check()).
DET_MAP = {"constantPositionFrozen": "cpos", "implausibleSpeed": "maxspeed",
           "brakeAcceleration": "brakeacc"}


def _ns(x):
    return None if x is None else x / 1e9


def brakes_of(c):
    b = c.get("brakes", c.get("brakes_applied"))
    if b is None or isinstance(b, bool):
        return b
    if isinstance(b, (int, float)):
        # J2735 wheelBrakes BIT STRING (SIZE(5)) as the engine records it
        # (BrakeAppliedStatus.0): bit 0 'unavailable' is the MOST significant of the five
        # (0x10), bits 1-4 a wheel braking (leftFront 0x08 .. rightRear 0x01)
        if int(b) & 0x10:
            return None
        return bool(int(b) & 0x0F)
    if isinstance(b, str):
        return {"applied": True, "on": True, "true": True, "off": False, "false": False}.get(b.lower())
    return None


def bsm_of(c, t_gen, t_seq):
    """The realdata Bsm of one engine BSM content; t_seq is the 'receive' clock instant."""
    sm = c.get("sec_mark_ms")
    if sm is not None and sm >= 60000:
        sm = None
    hd = c.get("heading_deg")
    if hd is not None and (hd < 0 or hd >= 360):
        hd = None
    order = normalise.unwrap_secmark(t_gen, sm)
    return normalise.Bsm(c["temp_id"], sm, c.get("msg_count"), order, t_gen, t_seq,
                         c["lat_deg"], c["lon_deg"], c.get("speed_mps"), hd,
                         c.get("accel_long_mps2"), None, None, brakes_of(c))


def read_tx(path, inject=None):
    """{msg: (node, pseudonym, Bsm)} of every transmitted BSM; returns (tx, injected msg)."""
    tx = {}
    with open(path) as f:
        for line in f:
            if '"bsm"' not in line:
                continue
            r = json.loads(line)
            if r.get("msg_type") != "bsm":
                continue
            c = r.get("content")
            if isinstance(c, str):
                c = json.loads(c)
            if not c or c.get("lat_deg") is None or c.get("temp_id") is None:
                continue
            t_gen = _ns(r.get("t_generated", r["t"]))
            tx[r["msg"]] = (r.get("node"), r.get("pseudonym"), bsm_of(c, t_gen, _ns(r["t"])))
    hit = None
    if inject is not None:
        hit = pick_inject(tx) if inject == "auto" else int(inject)
        n, ps, b = tx[hit]
        dlat = math.degrees(INJECT_M / pairing.R_EARTH)
        tx[hit] = (n, ps, b._replace(lat=b.lat + dlat))
    return tx, hit


def pick_inject(tx):
    by = defaultdict(list)
    for m in sorted(tx):
        by[tx[m][2].id].append(m)
    for k in sorted(by):
        ms = sorted(by[k], key=lambda m: tx[m][2].order)
        for i in range(1, len(ms) - 1):
            a, b = tx[ms[i - 1]][2], tx[ms[i]][2]
            if a.speed is not None and a.speed >= 5.0 and b.speed is not None and a.heading is not None:
                return ms[i]
    raise SystemExit("--inject auto: no moving BSM found")


def pair_dts(p, clock):
    """dt of pair p on the chosen clock (pairing.Pair.dt is (secmark, header, rx))."""
    if clock == "secmark":
        return p.dt[0]
    if clock == "generation":
        return p.dt[1]
    if clock == "receive":
        return p.dt[2]
    return None if p.dcnt is None else 0.1 * p.dcnt


def classes_of(tx):
    sms = defaultdict(list)
    for _, _, b in tx.values():
        sms[b.id].append(b.secmark)
    return {k: devclass.classify(*devclass.locked_fraction(v)) for k, v in sms.items()}


def streams_tx(tx):
    by = defaultdict(list)
    for m in sorted(tx):
        by[tx[m][2].id].append((m, tx[m][2]))
    for k in sorted(by):
        yield (None, k), by[k]


def judge_stream(items, clock, gap, arrival=False):
    """[(msg_b, id, v1, fired tuple, dt, dcnt, msg_a)] for every judged pair of one stream.

    items: [(msg, Bsm)]; pairing.dedupe_sort/pairs run on the Bsm list unchanged, the msg of
    the second BSM is recovered by identity. arrival=True re-sequences the de-duplicated BSMs
    by receive instant (t_rx), the order the engine detector pairs them in (K2)."""
    bs = pairing.dedupe_sort([b for _, b in items])
    if arrival:
        bs.sort(key=lambda b: b.t_rx)
    msg_of = {id(b): m for m, b in items}
    out = []
    for i, p in enumerate(pairing.pairs(bs)):
        dt = pair_dts(p, clock)
        if p.v1 is None or p.h1 is None or not mc.judged(dt, gap):
            continue
        fired, _, _ = mc.run(p, dt)
        out.append((msg_of[id(bs[i + 1])], bs[i + 1].id, p.v1, fired, dt, p.dcnt, msg_of[id(bs[i])]))
    return out


def rates(judged, cls):
    """{(class, band): {'pairs', 'minutes', check: (n, rate)}} plus ('all', band) rows."""
    acc = defaultdict(lambda: [0] + [0] * len(mc.CHECKS))
    ids = defaultdict(set)
    for _, tid, v1, fired, _, _, _ in judged:
        b = mc.band(v1)
        for c in (cls.get(tid, "unclassified"), "all"):
            a = acc[(c, b)]
            a[0] += 1
            ids[(c, b)].add(tid)
            for ci, f in enumerate(fired):
                a[1 + ci] += f
    out = {}
    for (c, b), a in sorted(acc.items(), key=lambda kv: (CLASSES.index(kv[0][0]) if kv[0][0] in CLASSES else 9, kv[0][1])):
        mins = a[0] * mc.FIX_INTERVAL_S / 60.0
        row = {"class": c, "band": mc.BAND_NAMES[b], "ids": len(ids[(c, b)]), "pairs": a[0],
               "id_minutes": round(mins, 4)}
        for ci, ch in enumerate(mc.CHECKS):
            row[ch] = {"n": a[1 + ci], "rate": round(a[1 + ci] / mins, 4) if mins else None}
        out[(c, b)] = row
    return list(out.values())


def table(rows):
    hdr = ["class", "band", "ids", "pairs", "id-min"] + list(mc.CHECKS)
    lines = ["  ".join(f"{h:>8}" for h in hdr)]
    for r in rows:
        cells = [r["class"], r["band"], str(r["ids"]), str(r["pairs"]), f"{r['id_minutes']:.2f}"]
        for ch in mc.CHECKS:
            n, rate = r[ch]["n"], r[ch]["rate"]
            cells.append(f"{rate:.3g}({n})" if rate is not None else "-")
        lines.append("  ".join(f"{c:>8}" for c in cells))
    return "\n".join(lines)


# ---- receiver side ----------------------------------------------------------------------

def read_rx(path, tx):
    """{(rx node, pseudonym): [(msg, Bsm with t_rx = delivery instant)]}, delivered+valid BSMs,
    and {(rx, msg): delivery instants} for the observation join."""
    by = defaultdict(list)
    when = {}
    with open(path) as f:
        for line in f:
            if '"bsm"' not in line or "delivered" not in line:
                continue
            r = json.loads(line)
            if r.get("msg_type") != "bsm" or r.get("t_delivered") is None or r["msg"] not in tx:
                continue
            v = r.get("verification")
            if v not in (None, "valid", "verified") and not (isinstance(v, str) and v.startswith("valid")):
                continue
            node, ps, b = tx[r["msg"]]
            ts = [x for x in (r.get("t_delivered"), r.get("t_verify_done"), r.get("t_rx_done")) if x is not None]
            when[(r["rx"], r["msg"])] = ts
            by[(r["rx"], ps)].append((r["msg"], b._replace(t_rx=_ns(r["t_delivered"]))))
    return by, when


def read_obs(path):
    """{(node, subject): [(t_ns, detector, row)]} of every det.observation (row: the record,
    with the us-sm-mbr msg_cnt / prev_msg_cnt / dt_s / residual when present)."""
    by = defaultdict(list)
    with open(path) as f:
        for line in f:
            r = json.loads(line)
            by[(r["node"], r["subject"])].append((r["t"], r["detector"], r))
    return by


PAIR_CHECKS = ("rp10", "rp11", "cpos", "maxacc_implied")


def engine_params(run_dir):
    """The us-sm-mbr params from scenario.resolved.yaml (a flat scan; no yaml dependency)."""
    p = {"reading": "v1_0", "time_base": "generation_time", "max_gap_s": 0.5,
         "consecutive": "msg_count", "max_accel_reading": "accel-set"}
    path = os.path.join(run_dir, "scenario.resolved.yaml")
    if not os.path.exists(path):
        return p, False
    with open(path) as f:
        txt = f.read()
    lines = txt.splitlines()
    at = next((i for i, ln in enumerate(lines) if "detect/us-sm-mbr" in ln), None)
    if at is None:
        return p, False
    ind = len(lines[at]) - len(lines[at].lstrip())
    for ln in lines[at + 1:]:
        if ln.strip() and len(ln) - len(ln.lstrip()) <= ind:
            break  # the next detector or section
        m = re.match(r"\s*(\w+):\s*['\"]?([\w.\-]+)", ln)
        if m and m.group(1) in p:
            p[m.group(1)] = float(m.group(2)) if m.group(1) == "max_gap_s" else m.group(2)
    return p, True


def not_run_by_engine(params, b):
    """Checks the engine's configured profile does not evaluate (on BSM b, when given)."""
    out = {"rp10" if params["reading"] == "asn1_v1_1" else "rp11"}
    if params["max_accel_reading"] == "accel-set":
        if b is None or b.accel is not None:
            out.add("maxacc_implied")
        if b is not None and b.accel is None:
            out.add("maxacc_tx")
    else:
        out.add("maxacc_tx")
    return out


def engine_check(det, b, params):
    if det == "positionJump":
        return "rp11" if params["reading"] == "asn1_v1_1" else "rp10"
    if det == "implausibleAcceleration":
        if params["max_accel_reading"] == "accel-set" and b.accel is not None:
            return "maxacc_tx"
        return "maxacc_implied"
    return DET_MAP.get(det)


def k2(run_dir, tx, params):
    rxp = os.path.join(run_dir, "jsonl", "node_rx.jsonl")
    obp = os.path.join(run_dir, "jsonl", "det_observation.jsonl")
    if not os.path.exists(rxp):
        return {"joined": False, "why": "no node_rx.jsonl (record level events/metrics drops receptions)"}
    have_obs = os.path.exists(obp)
    obs = read_obs(obp) if have_obs else {}
    rx, when = read_rx(rxp, tx)
    clock = "secmark" if params["time_base"] == "secmark" else "generation"
    gap = params["max_gap_s"]
    agree = {ch: {"both": 0, "engine_only": 0, "bridge_only": 0} for ch in mc.CHECKS}
    judged = joined_obs = 0
    used = set()
    unmapped = defaultdict(int)
    bridge_n = defaultdict(int)
    bridge_at = {}
    pair_mismatch = 0
    disagreements = []
    not_run = defaultdict(int)
    for key in sorted(rx, key=lambda k: (k[0], str(k[1]))):
        items = rx[key]
        # the engine judges in arrival order; pairing.py sequences by secMark time
        items.sort(key=lambda mb: mb[1].t_rx)
        bmap = dict(items)
        tlist = obs.get(key, [])
        for msg, _, _, fired, dt, dcnt, msg_a in judge_stream(items, clock, gap, arrival=True):
            # the engine's own gap rule: dt <= max_gap_s (the bridge: dt < L + 0.05)
            if dt > gap + 1e-9:
                continue
            if params["consecutive"] == "msg_count" and dcnt not in (None, 1):
                continue  # the engine's default: msgCnt successors only
            judged += 1
            ts = set(when.get((key[0], msg), []))
            eng = set()
            rows = {}
            b2, b1 = bmap[msg], bmap[msg_a]
            for j, (t, det, row) in enumerate(tlist):
                if t not in ts:
                    continue
                # a row naming its msgCnt is about that BSM only (two BSMs delivered in one
                # instant are told apart by it)
                if row.get("msg_cnt") is not None and b2.msgcnt is not None and row["msg_cnt"] != b2.msgcnt:
                    continue
                used.add((key, j))
                ch = engine_check(det, b2, params)
                if ch is None:
                    unmapped[det] += 1
                    continue
                if (ch in PAIR_CHECKS and row.get("prev_msg_cnt") is not None
                        and b1.msgcnt is not None and row["prev_msg_cnt"] != b1.msgcnt):
                    pair_mismatch += 1  # the engine judged this BSM against another one
                    continue
                eng.add(ch)
                rows[ch] = row
            joined_obs += bool(eng)
            bridge_at[(key[0], msg)] = (msg_a, {ch for ci, ch in enumerate(mc.CHECKS) if fired[ci]}, bmap[msg])
            skip = not_run_by_engine(params, b2)
            for ci, ch in enumerate(mc.CHECKS):
                bridge_n[ch] += bool(fired[ci])
                e, br = ch in eng, bool(fired[ci])
                if ch in skip and not e:
                    # the engine's profile does not evaluate this check on this BSM
                    not_run[ch] += br
                    continue
                if e and br:
                    agree[ch]["both"] += 1
                elif e:
                    agree[ch]["engine_only"] += 1
                elif br:
                    agree[ch]["bridge_only"] += 1
                if e != br and len(disagreements) < 50:
                    r = rows.get(ch, {})
                    disagreements.append({"rx": key[0], "subject": key[1], "msg": msg, "msg_a": msg_a,
                                          "check": ch, "side": "engine" if e else "bridge",
                                          "msg_cnt": (b1.msgcnt, b2.msgcnt), "bridge_dt_s": dt,
                                          "engine_dt_s": r.get("dt_s"), "engine_residual": r.get("residual")})
    n_obs = sum(len(v) for v in obs.values())
    if not have_obs:
        out = aggregate(run_dir, params, clock, gap, len(rx), judged, bridge_n)
        rep = report_join(run_dir, tx, params, bridge_at)
        if rep is not None:
            out["reports"] = rep
        return out
    return {"joined": True, "clock": clock, "gap_s": gap, "engine_params": params,
            "rx_streams": len(rx), "judged_pairs": judged, "obs_total": n_obs,
            "obs_joined": len(used), "obs_unjoined": n_obs - len(used),
            "pair_mismatch": pair_mismatch, "disagreements": disagreements,
            "bridge_fired_not_run_by_engine": dict(not_run),
            "unmapped_detectors": dict(unmapped), "agreement": agree,
            "note": ("engine judges single-BSM checks (maxspeed, maxacc, brakeacc) on every BSM, "
                     "the bridge only on the second BSM of a judged pair; unjoined observations "
                     "are those, first BSMs of a stream, or pairs outside the bridge's gap rule")
            if n_obs else "no det.observation records in this run (no check fired at any receiver)"}


def read_digests(path):
    """{spdu_digest: msg} from node.tx content (present when the dataset exporter is on)."""
    out = {}
    with open(path) as f:
        for line in f:
            if '"spdu_digest"' not in line or '"bsm"' not in line:
                continue
            r = json.loads(line)
            c = r.get("content") or {}
            if isinstance(c, str):
                c = json.loads(c)
            if c.get("spdu_digest"):
                out[c["spdu_digest"]] = r["msg"]
    return out


def report_join(run_dir, tx, params, bridge_at):
    """Per-report join: each misbehaviour report the engine built (node.report_built: its
    evidence BSM digests; pki.report_fate: the reporter's fired checks) against the bridge's
    verdict on the same pair (reporter, first evidence BSM, second evidence BSM). Needs the
    `dataset` exporter. bridge-only here is a check the bridge fires on a reported pair that
    the report does not carry; unreported fired pairs are not counted (reporting policy)."""
    jd = os.path.join(run_dir, "jsonl")
    bp, fp = os.path.join(jd, "node_report_built.jsonl"), os.path.join(jd, "pki_report_fate.jsonl")
    if not (os.path.exists(bp) and os.path.exists(fp)):
        return None
    dig = read_digests(os.path.join(jd, "node_tx.jsonl"))
    if not dig:
        return None
    ev, checks, reporter = {}, {}, {}
    with open(bp) as f:
        for line in f:
            r = json.loads(line)
            ev.setdefault(r["report_id"], r.get("evidence_spdu_digests") or [])
            reporter[r["report_id"]] = r["reporter"]
    with open(fp) as f:
        for line in f:
            r = json.loads(line)
            if r.get("checks"):
                checks.setdefault(r["report_id"], r["checks"])
                reporter.setdefault(r["report_id"], r["reporter"])
    agree = {ch: {"both": 0, "engine_only": 0, "bridge_only": 0} for ch in mc.CHECKS}
    n = joined = 0
    why = defaultdict(int)
    diffs = []
    for rid in sorted(checks):
        n += 1
        msgs = [dig.get(d) for d in ev.get(rid, [])]
        if not msgs or None in msgs:
            why["evidence not found in node.tx"] += 1
            continue
        hit = bridge_at.get((reporter[rid], msgs[-1]))
        if hit is None:
            why["no bridge pair ending at the last evidence BSM at the reporter"] += 1
            continue
        if len(msgs) > 1 and hit[0] != msgs[0]:
            why["bridge pairs the last evidence BSM with a different first BSM"] += 1
            continue
        joined += 1
        eng = {engine_check(c, hit[2], params) for c in checks[rid]} - {None}
        for ch in mc.CHECKS:
            if ch in not_run_by_engine(params, hit[2]):
                continue
            e, b = ch in eng, ch in hit[1]
            if e != b and len(diffs) < 20:
                diffs.append({"report": rid, "check": ch, "engine": e, "bridge": b, "msgs": msgs})
            if e and b:
                agree[ch]["both"] += 1
            elif e:
                agree[ch]["engine_only"] += 1
            elif b:
                agree[ch]["bridge_only"] += 1
    return {"reports_with_checks": n, "joined": joined, "unjoined": dict(why), "agreement": agree,
            "not_run_by_engine": sorted(not_run_by_engine(params, None)), "disagreements": diffs}


def aggregate(run_dir, params, clock, gap, nrx, judged, bridge_n):
    """No det.observation channel in the run: compare the run report's per-detector verdict
    totals (mbr.verdicts_by_detector, every receiver) with the bridge's receiver-side totals."""
    path = os.path.join(run_dir, "run-report.json")
    eng = {}
    if os.path.exists(path):
        with open(path) as f:
            stack = [json.load(f)]
        while stack:
            o = stack.pop()
            if isinstance(o, dict):
                if isinstance(o.get("verdicts_by_detector"), dict):
                    eng = o["verdicts_by_detector"]
                    break
                stack.extend(o.values())
    totals = {ch: {"engine": 0, "bridge": bridge_n.get(ch, 0)} for ch in mc.CHECKS}
    unmapped = {}
    for det, n in sorted(eng.items()):
        if det == "positionJump":
            ch = "rp11" if params["reading"] == "asn1_v1_1" else "rp10"
        elif det == "implausibleAcceleration":
            ch = "maxacc_tx" if params["max_accel_reading"] == "accel-set" else "maxacc_implied"
        else:
            ch = DET_MAP.get(det)
        if ch is None:
            unmapped[det] = n
        else:
            totals[ch]["engine"] += n
    return {"joined": "aggregate", "clock": clock, "gap_s": gap, "engine_params": params,
            "rx_streams": nrx, "judged_pairs": judged, "totals": totals, "unmapped_detectors": unmapped,
            "why": ("no det.observation channel in this recording, so per-pair verdicts cannot be "
                    "joined; compared run-report mbr.verdicts_by_detector totals with the bridge's "
                    "receiver-side totals on the engine's clock, gap and pairing rule. Engine "
                    "single-BSM checks also fire on unpaired BSMs, so maxspeed/maxacc/brakeacc "
                    "engine totals may exceed the bridge's")}


def main(argv):
    if not argv or argv[0].startswith("-"):
        raise SystemExit(__doc__)
    run_dir = argv[0]

    def opt(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    gap = float(opt("--gap", "0.1"))
    clock = opt("--clock", "secmark")
    side = opt("--side", "tx")
    if clock not in CLOCKS:
        raise SystemExit(f"--clock must be one of {CLOCKS}")
    tx, hit = read_tx(os.path.join(run_dir, "jsonl", "node_tx.jsonl"), opt("--inject"))
    cls = classes_of(tx)
    if side == "tx":
        streams = streams_tx(tx)
    else:
        rx, _ = read_rx(os.path.join(run_dir, "jsonl", "node_rx.jsonl"), tx)
        streams = ((k, rx[k]) for k in sorted(rx, key=lambda k: (k[0], str(k[1]))))
    judged = []
    for _, items in streams:
        judged.extend(judge_stream(items, clock, gap))
    rows = rates(judged, cls)
    ncls = defaultdict(int)
    for v in cls.values():
        ncls[v] += 1
    out = {"run": run_dir, "side": side, "clock": clock, "gap_s": gap, "bsms": len(tx),
           "ids": len(cls), "id_classes": dict(sorted(ncls.items())), "judged_pairs": len(judged),
           "injected_msg": hit, "unit": "triggers per temporary-ID-minute (exposure = judged pairs x 0.1 s)",
           "rows": rows}
    print(f"{side} side, clock {clock}, gap {gap} s: {len(tx)} BSMs, {len(cls)} IDs {dict(ncls)}, "
          f"{len(judged)} judged pairs" + (f", injected msg {hit}" if hit is not None else ""))
    print("rate per ID-minute (count)")
    print(table(rows))
    if "--no-k2" not in argv:
        params, found = engine_params(run_dir)
        if not found:
            out["k2"] = {"joined": False, "why": "no detect/us-sm-mbr in scenario.resolved.yaml"}
        else:
            out["k2"] = k2(run_dir, tx, params)
        k = out["k2"]
        print(f"K2: {json.dumps({x: k[x] for x in k if x not in ('agreement', 'engine_params', 'totals', 'disagreements')})}")
        if k.get("joined") == "aggregate":
            print("  check           engine  bridge")
            for ch, a in k["totals"].items():
                print(f"  {ch:<14}{a['engine']:>8}{a['bridge']:>8}")
            rep = k.get("reports")
            if rep:
                print(f"K2 per-report join: {rep['joined']} of {rep['reports_with_checks']} reports joined; "
                      f"unjoined {rep['unjoined']}")
                print("  check           both  engine-only  bridge-only")
                for ch, a in rep["agreement"].items():
                    print(f"  {ch:<14}{a['both']:>6}{a['engine_only']:>13}{a['bridge_only']:>13}")
        elif k.get("joined"):
            print("  check           both  engine-only  bridge-only")
            for ch, a in k["agreement"].items():
                print(f"  {ch:<14}{a['both']:>6}{a['engine_only']:>13}{a['bridge_only']:>13}")
            for x in k.get("disagreements", [])[:10]:
                print(f"  disagree: {json.dumps(x)}")
    if "--json" in argv:
        with open(opt("--json"), "w") as f:
            json.dump(out, f, indent=1)
    return out


if __name__ == "__main__":
    main(sys.argv[1:])
