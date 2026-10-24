"""ID-clustered bootstrap for rates per ID-minute.

Units are temporary IDs (pseudonym periods), not pairs: pairs of one ID are correlated
(one device class, one trip), so the IDs are resampled with replacement and the rate of
each replicate is Σ triggers / Σ minutes. Seeds are keyed by the cell name, so the same
cell gets the same interval on every run (determinism).
"""
import hashlib
import random

B = 1000
SEED = "tools/realdata/p1-e1"


def rng(key):
    return random.Random(int(hashlib.sha256((SEED + "|" + key).encode()).hexdigest()[:16], 16))


def rate(units):
    t = sum(u[0] for u in units)
    m = sum(u[1] for u in units)
    return t / m if m > 0 else float("nan")


def ci(units, key, b=B, alpha=0.05):
    """units: list of (triggers, minutes) per ID. Returns (rate, lo, hi)."""
    units = [u for u in units if u[1] > 0]
    if not units:
        return float("nan"), float("nan"), float("nan")
    r = rng(key)
    n = len(units)
    trig = [u[0] for u in units]
    mins = [u[1] for u in units]
    reps = []
    ids = range(n)
    for _ in range(b):
        idx = r.choices(ids, k=n)
        t = sum(map(trig.__getitem__, idx))
        m = sum(map(mins.__getitem__, idx))
        reps.append(t / m if m > 0 else 0.0)
    reps.sort()
    lo = reps[int(alpha / 2 * b)]
    hi = reps[min(b - 1, int((1 - alpha / 2) * b))]
    return rate(units), lo, hi


def quantile(xs, q):
    xs = sorted(xs)
    if not xs:
        return float("nan")
    k = (len(xs) - 1) * q
    f = int(k)
    c = min(f + 1, len(xs) - 1)
    return xs[f] + (xs[c] - xs[f]) * (k - f)
