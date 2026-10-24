"""Stage 5 (device class): secMark phase classifier.

An ID is `locked` when more than LOCKED_GT of its secMarks lie within TOL_MS of a multiple
of EPOCH_MS, `free` when fewer than FREE_LT do, otherwise `mixed`; IDs with fewer than
MIN_BSMS secMarks are `unclassified`. The thresholds are the pre-registered ones of
paper-plan-2026-10.md §1.3 (P1-E2). They are frozen with the fit statistics in
tools/realdata/frozen/classifier-v1.json and committed before Wyoming is scored.
"""
EPOCH_MS = 50
TOL_MS = 1
LOCKED_GT = 0.8
FREE_LT = 0.3
MIN_BSMS = 10
CLASSES = ("locked", "mixed", "free")


def near_epoch(secmark, epoch_ms=EPOCH_MS, tol_ms=TOL_MS):
    m = secmark % epoch_ms
    return min(m, epoch_ms - m) <= tol_ms


def locked_fraction(secmarks, epoch_ms=EPOCH_MS, tol_ms=TOL_MS):
    sm = [s for s in secmarks if s is not None]
    if not sm:
        return None, 0
    return sum(1 for s in sm if near_epoch(s, epoch_ms, tol_ms)) / len(sm), len(sm)


def classify(frac, n, params=None):
    p = params or {}
    if frac is None or n < p.get("min_bsms", MIN_BSMS):
        return "unclassified"
    if frac > p.get("locked_gt", LOCKED_GT):
        return "locked"
    if frac < p.get("free_lt", FREE_LT):
        return "free"
    return "mixed"


def params():
    return {"epoch_ms": EPOCH_MS, "tol_ms": TOL_MS, "locked_gt": LOCKED_GT, "free_lt": FREE_LT,
            "min_bsms": MIN_BSMS}


def phase100(secmark, fix_ms=100):
    """Timestamp minus the 100 ms fix epoch, in [0, fix_ms)."""
    return secmark % fix_ms


def phase_wrapped(secmark, epoch_ms=EPOCH_MS):
    """Signed distance to the nearest EPOCH_MS epoch, in [-epoch/2, epoch/2)."""
    return ((secmark + epoch_ms // 2) % epoch_ms) - epoch_ms // 2
