"""Stage 3 (de-duplicate and pair): consecutive BSMs of one temporary ID.

De-duplication key is (ID, secMark, msgCnt) (NYC, which has no secMark: (ID, T_s, msgCnt)).
BSMs are sequenced by the source's `order` time. Each pair carries Δt under every time
base the source has; a base whose Δt is missing or inconsistent is None for that pair.
"""
import math
from collections import namedtuple

R_EARTH = 6371008.8
BASES = ("secmark", "header", "rx")
# secMark Δt (mod 60 s) must agree with the sequencing clock to within this, else the pair
# straddles a wrap we cannot resolve or the receiver buffered it; the base is then None.
SECMARK_AGREE_S = 1.0

Pair = namedtuple("Pair", "dt v1 v2 h1 h2 a1 a2 dx dy brakes2 dcnt sm1 sm2")


def dedupe_sort(bsms):
    seen, out = set(), []
    for b in bsms:
        if b.lat is None or b.lon is None or b.order is None:
            continue
        k = (b.secmark if b.secmark is not None else b.order, b.msgcnt)
        if k in seen:
            continue
        seen.add(k)
        out.append(b)
    out.sort(key=lambda b: b.order)
    return out


def secmark_dt(s1, s2, tgap):
    if s1 is None or s2 is None:
        return None
    d = ((s2 - s1) % 60000) / 1000.0
    if tgap is None or abs(d - tgap) > SECMARK_AGREE_S:
        return None
    return d


def displacement(a, b, xy=False):
    if xy:
        return b.lon - a.lon, b.lat - a.lat
    lat_m = math.radians(0.5 * (a.lat + b.lat))
    return (math.radians(b.lon - a.lon) * R_EARTH * math.cos(lat_m),
            math.radians(b.lat - a.lat) * R_EARTH)


def pairs(bsms, xy=False, nyc=False):
    """Consecutive pairs of an already de-duplicated, sorted BSM list."""
    out = []
    for a, b in zip(bsms, bsms[1:]):
        tgap = b.order - a.order
        if nyc:
            dsm = tgap  # NYC's T_s is the (translated) secMark time
        else:
            dsm = secmark_dt(a.secmark, b.secmark, tgap)
        dh = None if a.t_hdr is None or b.t_hdr is None else b.t_hdr - a.t_hdr
        dr = None if a.t_rx is None or b.t_rx is None else b.t_rx - a.t_rx
        dx, dy = displacement(a, b, xy)
        dcnt = None if a.msgcnt is None or b.msgcnt is None else (b.msgcnt - a.msgcnt) % 128
        out.append(Pair((dsm, dh, dr), a.speed, b.speed, a.heading, b.heading, a.accel, b.accel, dx, dy,
                        b.brakes, dcnt, a.secmark, b.secmark))
    return out
