"""Summarise one Paper 1 validation run into <run>/realism.json (matrix.py extract + x.*).

  python3 -I summ.py <run_dir> [--e4] [--stamp] [--veremi] [--ma] [--clean]

x.fate.*   pki.report_fate (the device store's log with the truth join), per report: its
           last fate; lost = evicted_quota | decayed_out | expired.
x.ma.*     ma.received rows; with --ma, ma-replay.csv (run `v2xw ma-replay` first).
x.e4.*     RandomPosition residuals from node.tx through tools/realdata, KS against Wyoming.
x.st.*     the locked stamp model's signature (secMark offset chain, along/v by offset step,
           > 5 cm share on 0 -> -1 pairs), as tools/realdata/e4stampmc.py reads real data.
x.vr.*     VeReMi round trip against phy.rx / node.tx, an independent leakage check.
x.bf.*     TS 103 759 BeaconFrequency replayed on the VeReMi receptions (75 ms, 2 in a row).
Usage of --e4 needs V2XW_P1_WYOMING (the raw wyoming.csv of tools/realdata).
--clean deletes the jsonl directory, the recording and the VeReMi directory afterwards.
"""
import csv
import glob
import hashlib
import importlib.util
import json
import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
sys.path.insert(0, HERE)
LOST = {"evicted_quota", "decayed_out", "expired"}
# ts103759.rs BeaconProfile::BSM_J2945_1 and Ts103759Params::min_consecutive
BF_MIN_S, BF_STREAK = 0.1 - 0.025, 2


def load_matrix():
    spec = importlib.util.spec_from_file_location("matrix", os.path.join(REPO, "tools/realworld/matrix.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def jl(path):
    if not os.path.exists(path):
        return
    with open(path) as f:
        for line in f:
            yield json.loads(line)


def obj(v):
    return json.loads(v) if isinstance(v, str) else v


def fates(d, run):
    # P1-E6 common random numbers: every vehicle's exposure row (node, la_link, joined,
    # left, attacker, attack start) is the vehicle's own draws; an RSU-count change must
    # leave it byte-identical. RSUs have no row.
    ex = sorted(json.dumps({k: v for k, v in r.items() if k != "t"}, sort_keys=True)
                for r in jl(os.path.join(run, "jsonl", "gt_device_exposure.jsonl")))
    d["x.exposure.rows"] = len(ex)
    d["x.exposure.sig"] = hashlib.sha256("\n".join(ex).encode()).hexdigest()
    last, attack, rows = {}, set(), []
    for r in jl(os.path.join(run, "jsonl", "pki_report_fate.jsonl")):
        t = obj(r.get("truth")) or {}
        rid = r["report_id"]
        rows.append((r["t"], rid, r["fate"], r.get("weight")))
        if t.get("subject_attacker"):
            attack.add(rid)
        if rid not in last or r["t"] >= last[rid][0]:
            last[rid] = (r["t"], r["fate"])
    term = {k: f for k, (_, f) in last.items()}
    allf = [f for _, _, f, _ in rows]
    lost = [k for k, f in term.items() if f in LOST]
    delivered = {rid for _, rid, f, _ in rows if f == "delivered"}
    d["x.fate.records"] = len(rows)
    d["x.fate.reports"] = len(term)
    d["x.fate.attack_reports"] = len(attack)
    d["x.fate.honest_reports"] = len(term) - len(attack)
    d["x.fate.attack_lost"] = sum(1 for k in lost if k in attack)
    d["x.fate.attack_lost_share"] = (d["x.fate.attack_lost"] / len(attack)) if attack else None
    d["x.fate.lost_share"] = (len(lost) / len(term)) if term else None
    d["x.fate.delivered"] = len(delivered)
    d["x.fate.attack_delivered"] = len(delivered & attack)
    for f in ("evicted_quota", "decayed_out", "expired", "uploaded", "held_at_end", "stored"):
        d["x.fate." + f] = allf.count(f)
    d["x.fate.sig"] = hashlib.sha256(json.dumps(sorted(rows, key=lambda x: (x[0], x[1], x[2]))).encode()).hexdigest()
    rx = list(jl(os.path.join(run, "jsonl", "ma_received.jsonl")))
    d["x.ma.received"] = len(rx)
    # P1-E8: the MA view's digest (order-free), and how the reports about attackers read.
    keyed = sorted(json.dumps(r, sort_keys=True) for r in rx)
    d["x.ma.sig"] = hashlib.sha256("\n".join(keyed).encode()).hexdigest()

    def locked_share(rs):
        return (sum(1 for r in rs if r["secmark_phase_ms"] <= 1 or r["secmark_phase_ms"] >= 49) / len(rs)) if rs else None
    att = [r for r in rx if r["report_id"] in attack]
    d["x.ma.attack_reports"] = len(att)
    d["x.ma.attack_locked_share"] = locked_share(att)
    d["x.ma.honest_locked_share"] = locked_share([r for r in rx if r["report_id"] not in attack])


def ma(d, run):
    p = os.path.join(run, "ma-replay.csv")
    rows = list(csv.DictReader(open(p)))
    sound = [r for r in rows if r["sound"] == "true"]
    d["x.ma.rules"] = len(rows)
    d["x.ma.sound_rules"] = len(sound)
    d["x.ma.sound_honest_max"] = max(int(r["honest_blocked"]) for r in sound)
    d["x.ma.sound_attackers_max"] = max(int(r["attackers_revoked"]) for r in sound)
    d["x.ma.attackers"] = max(int(r["attackers"]) for r in rows)
    d["x.ma.unsound_honest_max"] = max([int(r["honest_blocked"]) for r in rows if r["sound"] != "true"] or [0])
    d["x.ma.sound_honest_nonzero"] = sorted(r["rule"] + ":" + r["params"] for r in sound if int(r["honest_blocked"]) > 0)
    # P1-E8: the attack budget (s, summed over attackers and the family's sound parameter
    # rows) and the attackers revoked, per rule family.
    for fam in sorted({r["rule"] for r in sound}):
        fr = [r for r in sound if r["rule"] == fam]
        if "attack_budget_s" in fr[0]:
            d["x.ma.budget." + fam] = round(sum(float(r["attack_budget_s"]) for r in fr), 1)
        d["x.ma.revoked." + fam] = sum(int(r["attackers_revoked"]) for r in fr)


def e4(d, run):
    import ks
    import resid
    rows, nid = resid.residuals(resid.engine_ids(os.path.join(run, "jsonl", "node_tx.jsonl")), 10.0)
    rs = [r for r, _, _ in rows]
    # held-out Wyoming (raw rows stay outside the repository): V2XW_P1_WYOMING=<raw dir>/wyoming.csv
    wy_rows, _ = resid.residuals(resid.normalise.stream_ids(os.environ["V2XW_P1_WYOMING"], "wyoming"), 10.0)
    wy = [r for r, _, _ in wy_rows]
    trig = sum(r > resid.mc.GPS_DRIFT_M for r in rs)
    idmin = sum(dt for _, dt, _ in rows) / 60.0
    vs = sorted(v for _, _, v in rows)
    d["x.e4.ids"] = nid
    d["x.e4.pairs"] = len(rs)
    d["x.e4.ks"] = ks.ks(rs, wy) if rs else None
    d["x.e4.triggers"] = trig
    d["x.e4.rate_per_idmin"] = trig / idmin if idmin else None
    d["x.e4.v_p50"] = vs[len(vs) // 2] if vs else None
    # the speed covariate's spread (E4-SPEED: Wyoming p10 / p50 / p90 28.6 / 34.4 / 35.3 m/s)
    d["x.e4.v_p10"] = vs[len(vs) // 10] if vs else None
    d["x.e4.v_p90"] = vs[(9 * len(vs)) // 10] if vs else None
    d["x.e4.r_p50"] = resid.q(rs, 0.5)
    d["x.e4.r_p90"] = resid.q(rs, 0.9)
    d["x.e4.r_p99"] = resid.q(rs, 0.99)
    for f in (0.1, 0.25, 0.75):
        d["x.e4.r_p%d" % round(f * 100)] = resid.q(rs, f)
    # where the two CDFs are furthest apart (the KS statistic's location), m
    if rs:
        import bisect
        a, b = sorted(rs), sorted(wy)
        d["x.e4.ks_at_m"] = max(a + b, key=lambda x: abs(bisect.bisect_right(a, x) / len(a) - bisect.bisect_right(b, x) / len(b)))
    # TM-1: secMark steps of consecutive BSMs of one ID (msgCnt + 1): how often secMark
    # repeats (0 ms) and the period excess over 100 ms (CV-pilot free devices: never 0, p50 +6 ms)
    steps = []
    for _, bsms in resid.engine_ids(os.path.join(run, "jsonl", "node_tx.jsonl")):
        bs = sorted(bsms, key=lambda b: b.order)
        for a, b in zip(bs, bs[1:]):
            if a.secmark is None or b.secmark is None or a.msgcnt is None or b.msgcnt is None:
                continue
            if (b.msgcnt - a.msgcnt) % 128 == 1:
                steps.append((b.secmark - a.secmark) % 60000)
    single = sorted(s - 100 for s in steps if 50 < s < 150)
    d["x.e4.sm_steps"] = len(steps)
    d["x.e4.sm_repeat_share"] = sum(1 for s in steps if s == 0) / len(steps) if steps else None
    d["x.e4.sm_excess_p50"] = single[len(single) // 2] if single else None
    # :00/:50 grid hops: consecutive-msgCnt secMark steps of 50 or 150 ms (± 5 ms), the
    # pipeline's grid-state tolerance (tools/realdata/pipeline.py HOP_TOL_MS)
    hops = sum(1 for s in steps if abs(s - 50) <= 5 or abs(s - 150) <= 5)
    d["x.e4.hop_share"] = hops / len(steps) if steps else None


def stamp(d, run):
    """x.st.*: the locked stamp model's signature on the transmitted BSMs, measured the way
    tools/realdata/e4stampmc.py measures Wyoming and Tampa (p1-realdata-baseline.md §7.2):
    o = secMark offset from the nearest 50 ms epoch, per consecutive-msgCnt pair at v >= 10."""
    import e4stampmc
    import resid
    ps = e4stampmc.pairs_from(resid.engine_ids(os.path.join(run, "jsonl", "node_tx.jsonl")), False)
    d["x.st.pairs"] = len(ps)
    if not ps:
        return
    two = [x for x in ps if x["o1"] in (-1, 0) and x["o2"] in (-1, 0)]
    d["x.st.offset_other_share"] = 1 - len(two) / len(ps)
    d["x.st.early_share"] = sum(1 for x in ps if x["o2"] == -1) / len(ps)
    s_m, s_0 = e4stampmc.chain(ps)
    d["x.st.stay_minus1"] = s_m
    d["x.st.stay_zero"] = s_0
    d["x.st.transition_share"] = sum(1 for x in two if x["o1"] != x["o2"]) / len(ps)

    def grp(a, b):
        return [x for x in ps if (x["o1"], x["o2"]) == (a, b)]

    for name, (a, b) in (("e_plus1", (-1, 0)), ("e_minus1", (0, -1))):
        g = [1000.0 * x["al"] / x["v"] for x in grp(a, b)]
        if g:
            m = sum(g) / len(g)
            d["x.st.along_%s_ms" % name] = m
            d["x.st.along_%s_sd" % name] = (sum((v - m) ** 2 for v in g) / len(g)) ** 0.5
    tr = grp(0, -1)
    other = [x for x in ps if (x["o1"], x["o2"]) != (0, -1)]
    d["x.st.over5_on_0m1"] = sum(1 for x in tr if x["r"] > 0.05) / len(tr) if tr else None
    d["x.st.over5_other"] = sum(1 for x in other if x["r"] > 0.05) / len(other) if other else None
    # P(r > 5 cm) by transition (p1-realdata-baseline.md §7.5; Wyoming held out:
    # -1 -> -1 0.000, -1 -> 0 0.105, 0 -> 0 0.049, 0 -> -1 0.483)
    for name, (a, b) in (("m1m1", (-1, -1)), ("m10", (-1, 0)), ("00", (0, 0))):
        g = grp(a, b)
        d["x.st.over5_on_" + name] = sum(1 for x in g if x["r"] > 0.05) / len(g) if g else None


def veremi_lines(run):
    out = []
    for p in sorted(glob.glob(os.path.join(run, "veremi", "traceJSON-*.json"))):
        rx = int(os.path.basename(p).split("-")[1])
        for r in jl(p):
            out.append((rx, r))
    return out


def py_leaks(lines, truth_ids):
    leaks, by_pseudo = 0, {}
    for _, r in lines:
        if r.get("type") != 3:
            continue
        if r.get("sender") in truth_ids:
            leaks += 1
        s = by_pseudo.setdefault(r.get("senderPseudo"), r.get("sender"))
        if s != r.get("sender"):
            leaks += 1
        for k in ("senderRealId", "attackerType", "realId"):
            if k in r:
                leaks += 1
    return leaks


def bf_events(lines, ok):
    gaps = {}
    for rx, r in lines:
        if r.get("type") == 3 and ok(r):
            gaps.setdefault((rx, r["senderPseudo"]), []).append(r["rcvTime"])
    ev, mn = 0, 1e9
    for ts in gaps.values():
        ts.sort()
        streak = 0
        for a, b in zip(ts, ts[1:]):
            dt = b - a
            if dt <= 0:
                continue
            mn = min(mn, dt)
            streak = streak + 1 if BF_MIN_S / dt >= 1.0 else 0
            ev += streak >= BF_STREAK
    return ev, mn


def veremi(d, run):
    files = glob.glob(os.path.join(run, "veremi", "*"))
    d["x.vr.export_ok"] = bool(glob.glob(os.path.join(run, "veremi", "traceJSON-*.json")))
    d["x.vr.files"] = len(files)
    tx = {}
    for r in jl(os.path.join(run, "jsonl", "node_tx.jsonl")):
        if r.get("msg") is not None:
            tx[r["msg"]] = r
    rx_ok = set()
    for r in jl(os.path.join(run, "jsonl", "phy_rx.jsonl")):
        if r.get("outcome") == "ok" and r.get("msg") is not None:
            rx_ok.add((r["rx"], r["msg"]))
    lines = veremi_lines(run)
    t3 = [(rx, r) for rx, r in lines if r.get("type") == 3]
    truth = []
    for p in glob.glob(os.path.join(run, "veremi", "traceGroundTruthJSON-*.json")):
        truth.extend(jl(p))
    mism = 0
    seen = set()
    for rx, r in t3:
        k = (rx, r["messageID"])
        seen.add(k)
        t = tx.get(r["messageID"])
        if k not in rx_ok or t is None:
            mism += 1
            continue
        c = obj(t.get("content")) or {}
        gen = (t.get("t_generated") or t["t"]) / 1e9
        if abs(r["sendTime"] - round(gen, 3)) > 1.5e-3 or r["senderPseudo"] != t.get("pseudonym"):
            mism += 1
        elif "claimed_x_m" in c and (abs(r["pos"][0] - c["claimed_x_m"]) > 0.01 or abs(r["pos"][1] - c["claimed_y_m"]) > 0.01):
            mism += 1
    d["x.vr.type3"] = len(t3)
    d["x.vr.rx_ok"] = len(rx_ok)
    d["x.vr.rx_ok_unexported"] = len(rx_ok - seen)
    d["x.vr.roundtrip_mismatch"] = mism + len(rx_ok - seen)
    attackers = {r["actor"] for r in jl(os.path.join(run, "jsonl", "gt_attack_action.jsonl"))}
    coded = {t["sender"] for t in truth if t.get("attackerType")}
    codes = {t.get("attackerType") for t in truth if t.get("attackerType")}
    d["x.vr.truth"] = len(truth)
    d["x.vr.truth_zero_pos"] = sum(1 for t in truth if t["pos"][:2] == [0.0, 0.0])
    d["x.vr.own_position_lines"] = sum(1 for _, r in lines if r.get("type") == 2)
    d["x.vr.gt_attack_senders"] = len(coded)
    d["x.vr.gt_attackers_ok"] = bool(coded) and coded <= attackers and codes == {1}
    truth_ids = {t["sender"] for t in truth}
    d["x.vr.py_leaks"] = py_leaks(lines, truth_ids)
    # the fault: one honest line's `sender` replaced by its true id
    bad = [list(x) for x in lines]
    for x in bad:
        r = x[1]
        if r.get("type") == 3:
            tr = next(t for t in truth if t["messageID"] == r["messageID"])
            x[1] = dict(r, sender=tr["sender"])
            break
    d["x.vr.py_leaks_injected"] = py_leaks(bad, truth_ids)
    honest_pseudo = {t["senderPseudo"] for t in truth if not t.get("attackerType")}
    attack_pseudo = {t["senderPseudo"] for t in truth if t.get("attackerType")}
    bsm = {m for m, t in tx.items() if t.get("msg_type") == "bsm"}

    def ok(r):
        return r["messageID"] in bsm and r["senderPseudo"] in honest_pseudo and r["senderPseudo"] not in attack_pseudo

    ev, mn = bf_events(lines, ok)
    d["x.bf.events_honest"] = ev
    d["x.bf.min_gap_ms"] = round(mn * 1000.0, 3)
    # the fault: one honest sender heard at 20 Hz (every reception duplicated at +50 ms)
    victim = next(r["senderPseudo"] for _, r in t3 if ok(r))
    extra = [(rx, dict(r, rcvTime=round(r["rcvTime"] + 0.05, 3), messageID=r["messageID"]))
             for rx, r in t3 if r["senderPseudo"] == victim]
    ev2, _ = bf_events(lines + extra, ok)
    d["x.bf.events_injected20hz"] = ev2


def main():
    run = os.path.abspath(sys.argv[1])
    m = load_matrix()
    d = m.extract(run)
    for k in ("positionJump", "constantPositionFrozen"):
        d.setdefault("p2.verdicts." + k, 0)  # a detector that never fired has no key
    fates(d, run)
    if "--ma" in sys.argv:
        ma(d, run)
    if "--e4" in sys.argv:
        e4(d, run)
    if "--stamp" in sys.argv:
        stamp(d, run)
    if "--veremi" in sys.argv:
        veremi(d, run)
    json.dump(d, open(os.path.join(run, "realism.json"), "w"), indent=0, sort_keys=True)
    keys = [k for k in d if k.startswith("x.")] + ["p2.verdicts.positionJump", "p2.verdicts.constantPositionFrozen",
                                                   "p2.reports_sent", "digest.record", "cost.wall_s", "cost.peak_mb"]
    print(os.path.basename(run), json.dumps({k: d.get(k) for k in keys if k != "x.fate.sig"})[:1500])
    if "--clean" in sys.argv:
        for p in ("jsonl", "veremi"):
            shutil.rmtree(os.path.join(run, p), ignore_errors=True)
        for p in ("recording.mcap",):
            if os.path.exists(os.path.join(run, p)):
                os.remove(os.path.join(run, p))


if __name__ == "__main__":
    main()
