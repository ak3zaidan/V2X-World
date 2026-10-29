"""Reference ML baseline on a VeReMi Extension export (WP-24 P24.3, f2md.md §5 P11).

  python3 -I tools/realworld/veremi_baseline.py features <run>/veremi <features.csv>
  python3 -I tools/realworld/veremi_baseline.py train <out.json> --train a.csv b.csv --test c.csv [--seed 0]
  python3 -I tools/realworld/veremi_baseline.py repro <out.json> --train a.csv b.csv --test c.csv [--seed 0]

The recipe is in docs/guides/veremi-ext-baseline.md.

`features` reads only what a receiver can see: its own type-2 positions and the type-3
beacons it received (claimed position, speed, heading, pos_noise, sendTime, rcvTime,
senderPseudo). The ground truth (traceGroundTruthJSON-*, type 4) is read only for the label,
joined on messageID. Every reception gets ten check scores, each about 1 at its firing
threshold, with the engine's TS 103 759 defaults (crates/v2xw-threat/src/ts103759.rs
`Ts103759Params::default`). The feature vector is the scores of this reception and of the
two before it from the same pseudonym at the same receiver (F2MD's "last 3 messages",
f2md.md §5 P11), 30 numbers, plus how many of the two earlier messages exist.

`train` fits XGBoost (gradient-boosted trees) and an MLP with fixed seeds on the training
runs and scores per reception on the held-out run: MCC, informedness (BM), markedness (MK),
Cohen's kappa, F1, and the recall per VeReMi Extension attack code. Splitting by run keeps
every vehicle and pseudonym of the test run out of training. Each model is hashed (the
XGBoost JSON dump; the MLP's weights), and `repro` trains twice and fails unless both
hashes and every metric agree. No F2MD Python is used (it has no licence).

Substitutions, recorded in the output: torch is not installed, so the MLP is
scikit-learn's MLPClassifier; if xgboost cannot be imported the trees are scikit-learn's
HistGradientBoostingClassifier.
"""
import csv
import glob
import hashlib
import json
import math
import os
import sys

# Ts103759Params::default (crates/v2xw-threat/src/ts103759.rs) and BeaconProfile::F2MD_VEREMI_1HZ.
MAX_RANGE_M = 420.0           # max_plausible_range_m (F2MD ART)
MAX_SPEED_MPS = 40.0          # max_plausible_speed_mps
MAX_ACCEL_MPS2 = 6.0          # max_plausible_accel_mps2
MAX_DECEL_MPS2 = 10.0         # max_plausible_decel_mps2
MGT_RNG_M = 4.0               # mgt_rng_m
MGT_UP_MPS = 2.1              # mgt_rng_up_mps
MGT_DOWN_MPS = 6.2            # mgt_rng_down_mps
MAX_HEADING_DEG = 90.0        # max_heading_change_deg
POS_HEADING_TIME_S = 1.1      # pos_heading_time_s
SUDDEN_APPEARANCE_M = 30.0    # sudden_appearance_m
PROXIMITY_M = 2.0             # proximity_distance_m
MAX_DELTA_INTER_S = 2.0       # max_delta_inter_s
KALMAN_POS_FLOOR_M = 4.0      # kalman_pos_floor_m
KALMAN_SPEED_FLOOR_MPS = 1.0  # kalman_speed_floor_mps
MIN_BEACON_S = 0.9            # F2MD_VEREMI_1HZ: 1.0 s less its 0.1 s tolerance
MAX_AGE_S = 0.1               # one 100 ms generation interval: a fresh beacon is heard within it
CLIP = 10.0
CHECKS = ("range", "speed", "pos_cons", "speed_cons", "pos_speed", "pos_heading", "beacon_freq",
          "sudden_app", "intersection", "cv_predict", "age")
HISTORY = 3


def _q(x):
    return round(min(CLIP, max(0.0, x)), 3)


def _norm(v):
    return math.hypot(v[0], v[1])


def check_scores(m, prev, own, others):
    """The check scores of reception m (dict: pos, spd, hed, sendTime, rcvTime), given the
    previous reception from the same pseudonym at this receiver (or None), the receiver's
    own position, and the other pseudonyms' latest claims [(pos, sendTime)]."""
    p, v = m["pos"], _norm(m["spd"])
    s = dict.fromkeys(CHECKS, 0.0)
    if own is not None:
        s["range"] = math.dist(p[:2], own[:2]) / MAX_RANGE_M
    s["speed"] = v / MAX_SPEED_MPS
    s["age"] = max(0.0, m["rcvTime"] - m["sendTime"]) / MAX_AGE_S
    near = [math.dist(p[:2], own[:2])] if own is not None else []
    near += [math.dist(p[:2], q[:2]) for q, t in others if abs(m["sendTime"] - t) <= MAX_DELTA_INTER_S]
    if near:
        s["intersection"] = PROXIMITY_M / max(min(near), 0.1)
    if prev is None:
        if own is not None:
            s["sudden_app"] = SUDDEN_APPEARANCE_M / max(math.dist(p[:2], own[:2]), 1.0)
        return {k: _q(x) for k, x in s.items()}
    dt = m["sendTime"] - prev["sendTime"]
    if dt <= 0:
        s["beacon_freq"] = CLIP
        return {k: _q(x) for k, x in s.items()}
    pp, pv = prev["pos"], _norm(prev["spd"])
    disp = math.dist(p[:2], pp[:2])
    s["beacon_freq"] = MIN_BEACON_S / dt
    s["pos_cons"] = disp / (MAX_SPEED_MPS * dt + MGT_RNG_M)
    dv = v - pv
    s["speed_cons"] = (dv / (MAX_ACCEL_MPS2 * dt + MGT_UP_MPS) if dv >= 0
                       else -dv / (MAX_DECEL_MPS2 * dt + MGT_DOWN_MPS))
    s["pos_speed"] = abs(disp - 0.5 * (v + pv) * dt) / (MGT_RNG_M + 0.5 * MAX_ACCEL_MPS2 * dt * dt)
    if disp >= 1.0 and v >= 1.0 and dt <= POS_HEADING_TIME_S:
        h = m["hed"]
        c = ((p[0] - pp[0]) * h[0] + (p[1] - pp[1]) * h[1]) / (disp * max(_norm(h), 1e-9))
        s["pos_heading"] = math.degrees(math.acos(max(-1.0, min(1.0, c)))) / MAX_HEADING_DEG
    pred = (pp[0] + prev["spd"][0] * dt, pp[1] + prev["spd"][1] * dt)
    s["cv_predict"] = math.dist(p[:2], pred) / (KALMAN_POS_FLOOR_M + KALMAN_SPEED_FLOOR_MPS * dt)
    return {k: _q(x) for k, x in s.items()}


def _lines(path):
    with open(path) as f:
        for ln in f:
            ln = ln.strip()
            if ln:
                yield json.loads(ln)


def labels(export):
    lab = {}
    for path in sorted(glob.glob(os.path.join(export, "traceGroundTruthJSON-*.json"))):
        for r in _lines(path):
            if r.get("type") == 4:
                lab[r["messageID"]] = int(r.get("attackerType", 0))
    return lab


def receiver_rows(path):
    """Feature rows of one receiver's trace, in file order."""
    own, last, hist, out = None, {}, {}, []
    for r in _lines(path):
        if r["type"] == 2:
            own = r["pos"]
            continue
        if r["type"] != 3:
            continue
        ps = r["senderPseudo"]
        h = hist.setdefault(ps, [])
        prev = h[-1][0] if h else None
        others = [v for k, v in last.items() if k != ps]
        sc = check_scores(r, prev, own, others)
        h.append((r, sc))
        del h[:-HISTORY]
        last[ps] = (r["pos"], r["sendTime"])
        out.append((r["messageID"], [x[1] for x in reversed(h)]))
    return out


def feature_names():
    return ["%s_%d" % (c, k) for k in range(HISTORY) for c in CHECKS] + ["history"]


def cmd_features(export, out):
    lab = labels(export)
    names = feature_names()
    n = 0
    with open(out, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["receiver", "messageID", "label"] + names)
        for path in sorted(glob.glob(os.path.join(export, "traceJSON-*.json")),
                           key=lambda p: int(os.path.basename(p).split("-")[1])):
            rx = int(os.path.basename(path).split("-")[1])
            for mid, scs in receiver_rows(path):
                vec = []
                for k in range(HISTORY):
                    vec += [scs[k][c] for c in CHECKS] if k < len(scs) else [0.0] * len(CHECKS)
                w.writerow([rx, mid, lab.get(mid, -1)] + vec + [len(scs) - 1])
                n += 1
    return n


# ---------------------------------------------------------------------------- training

def load(paths):
    import numpy as np
    xs, ys = [], []
    for p in paths:
        with open(p) as f:
            rd = csv.reader(f)
            next(rd)
            for row in rd:
                if int(row[2]) < 0:
                    continue  # no ground-truth line: not a labelled message
                ys.append(int(row[2]))
                xs.append([float(v) for v in row[3:]])
    return np.array(xs, dtype=np.float64), np.array(ys, dtype=np.int64)


def scores(y, yhat):
    tp = sum(1 for a, b in zip(y, yhat) if a and b)
    tn = sum(1 for a, b in zip(y, yhat) if not a and not b)
    fp = sum(1 for a, b in zip(y, yhat) if not a and b)
    fn = sum(1 for a, b in zip(y, yhat) if a and not b)
    n = tp + tn + fp + fn
    tpr = tp / (tp + fn) if tp + fn else 0.0
    tnr = tn / (tn + fp) if tn + fp else 0.0
    ppv = tp / (tp + fp) if tp + fp else 0.0
    npv = tn / (tn + fn) if tn + fn else 0.0
    den = math.sqrt((tp + fp) * (tp + fn) * (tn + fp) * (tn + fn))
    po = (tp + tn) / n
    pe = ((tp + fp) * (tp + fn) + (tn + fn) * (tn + fp)) / (n * n)
    return {"n": n, "tp": tp, "tn": tn, "fp": fp, "fn": fn,
            "mcc": round((tp * tn - fp * fn) / den, 6) if den else 0.0,
            "bm": round(tpr + tnr - 1, 6), "mk": round(ppv + npv - 1, 6),
            "kappa": round((po - pe) / (1 - pe), 6) if pe < 1 else 0.0,
            "f1": round(2 * tp / (2 * tp + fp + fn), 6) if tp else 0.0,
            "precision": round(ppv, 6), "recall": round(tpr, 6), "fpr": round(1 - tnr, 6)}


def per_attack(codes, yhat):
    out = {}
    for c in sorted(set(codes)):
        idx = [i for i, k in enumerate(codes) if k == c]
        out[str(c)] = {"messages": len(idx), "flagged": round(sum(1 for i in idx if yhat[i]) / len(idx), 6)}
    return out


def make_trees(seed):
    try:
        import xgboost
        return "xgboost %s" % xgboost.__version__, xgboost.XGBClassifier(
            n_estimators=200, max_depth=6, learning_rate=0.1, subsample=0.8, colsample_bytree=0.8,
            tree_method="hist", n_jobs=1, random_state=seed, eval_metric="logloss")
    except Exception:  # noqa: BLE001 — the recorded substitution
        from sklearn.ensemble import HistGradientBoostingClassifier
        return "sklearn HistGradientBoostingClassifier (xgboost not importable)", \
            HistGradientBoostingClassifier(max_iter=200, max_depth=6, learning_rate=0.1, random_state=seed)


def model_hash(name, model):
    h = hashlib.sha256()
    if name.startswith("xgboost"):
        h.update(bytes(model.get_booster().save_raw("json")))
    elif hasattr(model, "coefs_"):
        for a in list(model.coefs_) + list(model.intercepts_):
            h.update(a.tobytes())
    else:
        import pickle
        h.update(pickle.dumps(model, protocol=4))
    return h.hexdigest()


def train(train_paths, test_paths, seed=0):
    import numpy as np
    import sklearn
    from sklearn.neural_network import MLPClassifier
    from sklearn.preprocessing import StandardScaler
    xtr, ctr = load(train_paths)
    xte, cte = load(test_paths)
    ytr, yte = (ctr > 0).astype(np.int64), (cte > 0).astype(np.int64)
    doc = {"seed": seed, "features": feature_names(), "train": [os.path.basename(p) for p in train_paths],
           "test": [os.path.basename(p) for p in test_paths], "train_messages": int(len(ytr)),
           "test_messages": int(len(yte)), "train_attack_share": round(float(ytr.mean()), 6),
           "test_attack_share": round(float(yte.mean()), 6), "sklearn": sklearn.__version__,
           "numpy": np.__version__, "models": {}}
    tname, trees = make_trees(seed)
    trees.fit(xtr, ytr)
    yhat = trees.predict(xte).astype(int).tolist()
    doc["models"]["trees"] = {"impl": tname, "hash": model_hash(tname, trees),
                              "scores": scores(yte.tolist(), yhat), "per_attack": per_attack(cte.tolist(), yhat)}
    sc = StandardScaler().fit(xtr)
    mlp = MLPClassifier(hidden_layer_sizes=(64, 32), activation="relu", solver="adam", alpha=1e-4,
                        batch_size=256, learning_rate_init=1e-3, max_iter=200, early_stopping=False,
                        shuffle=True, random_state=seed, tol=1e-5)
    import warnings
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        mlp.fit(sc.transform(xtr), ytr)
    yhat = mlp.predict(sc.transform(xte)).astype(int).tolist()
    mname = "sklearn MLPClassifier (64, 32) relu adam (torch not installed)"
    doc["models"]["mlp"] = {"impl": mname, "hash": model_hash(mname, mlp), "iterations": int(mlp.n_iter_),
                            "scores": scores(yte.tolist(), yhat), "per_attack": per_attack(cte.tolist(), yhat)}
    return doc


def main(argv):
    import argparse
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    f = sub.add_parser("features")
    f.add_argument("export")
    f.add_argument("out")
    for name in ("train", "repro"):
        t = sub.add_parser(name)
        t.add_argument("out")
        t.add_argument("--train", nargs="+", required=True)
        t.add_argument("--test", nargs="+", required=True)
        t.add_argument("--seed", type=int, default=0)
    a = ap.parse_args(argv)
    if a.cmd == "features":
        print("rows", cmd_features(a.export, a.out))
        return 0
    doc = train(a.train, a.test, a.seed)
    if a.cmd == "repro":
        again = train(a.train, a.test, a.seed)
        doc["repro"] = {k: doc["models"][k]["hash"] == again["models"][k]["hash"]
                        and doc["models"][k]["scores"] == again["models"][k]["scores"] for k in doc["models"]}
    with open(a.out, "w") as fh:
        json.dump(doc, fh, indent=1, sort_keys=True)
    for k, m in doc["models"].items():
        s = m["scores"]
        print(k, m["hash"][:16], "mcc %.3f bm %.3f mk %.3f kappa %.3f f1 %.3f" % (
            s["mcc"], s["bm"], s["mk"], s["kappa"], s["f1"]))
    if a.cmd == "repro":
        print("repro", doc["repro"])
        return 0 if all(doc["repro"].values()) else 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
