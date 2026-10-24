"""Stage 4 (checks): the SCMS Manager CV-Pilot PSID-0x20 checks, both readings.

Sources: [SM-MBR] SCMS Manager, Misbehavior Report and Application Specification for CV
Pilot Deployment v1.0 (2019-02-14) §2.3; [SM-MBR-ASN] ASN.1 for Misbehavior Reporting v1.1
(2019-04-23) p.8. Defaults are the published ones (gpsDrift 1 m, MaxSpeed 90 m/s,
MaxAcceleration 10 m/s^2).

RandomPosition, two readings (Paper 1 must report both, check-paper1-preemption.md §2.1):
  v1.0   |p1 + v1*Δt*u(h1) - p2| > gpsDrift                       (velocity only)
  v1.1   |p1 + (v1*Δt + a1*Δt^2/2)*u(h1) - p2| > gpsDrift         (velocity and acceleration;
         the ASN.1 defines gpsDrift for BSMs "sent 0.1 seconds apart", so its canonical cell
         is the 0.1 s gap limit). a1 is the BSM's accelSet.long; where a source has none
         (Wyoming) the term is 0 and v1.1 equals v1.0 (flagged in the tables).
The gap limit L is not in either text (the engine's 0.5 s is an assumption); it is swept.
A pair is judged under limit L when 0 < Δt < L + GAP_TOL_S, i.e. Δt rounded to the nominal
100 ms BSM interval is at most L (free-class secMarks jitter around 100 ms).

ConstantPosition: both speeds >= 1 LSB (0.02 m/s) and the position moved < 1 cm.
MaxSpeed: v2 > 90 m/s (judged on the second BSM of each judged pair).
MaxAcceleration, two readings: implied |v2 - v1|/Δt > 10 (the engine's reading), and the
  transmitted |accelSet.long| of the second BSM > 10.
BrakeAcceleration: brakes applied on the second BSM and its accelSet.long >= 0 while moving
  (v2 >= 0.02 m/s). ASSUMPTION: "brakes on, acceleration not negative"; stationary braked
  vehicles are excluded because a held stop reports a = 0.
"""
import math

GPS_DRIFT_M = 1.0
MAX_SPEED_MPS = 90.0
MAX_ACCEL_MPS2 = 10.0
POS_RES_M = 0.01
SPEED_RES_MPS = 0.02
GAP_LIMITS_S = (0.1, 0.2, 0.5)
GAP_TOL_S = 0.05
FIX_INTERVAL_S = 0.1
SPEED_BANDS = ((0.0, 0.5), (0.5, 5.0), (5.0, 10.0), (10.0, 20.0), (20.0, 1e9))
BAND_NAMES = ("0-0.5", "0.5-5", "5-10", "10-20", ">=20")
# indices into a per-pair result vector
CHECKS = ("rp10", "rp11", "cpos", "maxspeed", "maxacc_implied", "maxacc_tx", "brakeacc")


def band(v):
    for i, (lo, hi) in enumerate(SPEED_BANDS):
        if lo <= v < hi:
            return i
    return len(SPEED_BANDS) - 1


def judged(dt, limit):
    return dt is not None and 0.0 < dt < limit + GAP_TOL_S


def residual(p, dt, accel=False):
    """Prediction error vector (east, north) of the RandomPosition check."""
    h = math.radians(p.h1)
    s = p.v1 * dt
    if accel and p.a1 is not None:
        s += 0.5 * p.a1 * dt * dt
    return p.dx - s * math.sin(h), p.dy - s * math.cos(h)


def along_cross(p, ex, ey):
    h = math.radians(p.h1)
    ux, uy = math.sin(h), math.cos(h)
    return ex * ux + ey * uy, -ex * uy + ey * ux


def slip_signature(p, dt):
    """'zero' (|Δp| ≈ 0: stale fix), 'double' (|Δp| ≈ 2 v Δt: fix from one epoch later), else
    'other'. Defined only while moving (v1 >= 5 m/s)."""
    if p.v1 is None or p.v1 < 5.0:
        return None
    r = math.hypot(p.dx, p.dy) / (p.v1 * dt)
    if r < 0.4:
        return "zero"
    if 1.6 <= r <= 2.4:
        return "double"
    return "other"


def run(p, dt, gps_drift=GPS_DRIFT_M):
    """Results of every check on pair p judged with Δt = dt. Returns (fired tuple aligned with
    CHECKS, v1.0 residual, v1.0 error vector). Caller has checked v1 and h1 are present."""
    ex, ey = residual(p, dt)
    r10 = math.hypot(ex, ey)
    ex1, ey1 = residual(p, dt, accel=True)
    r11 = math.hypot(ex1, ey1)
    move = math.hypot(p.dx, p.dy)
    v2 = p.v2
    fired = (
        r10 > gps_drift,
        r11 > gps_drift,
        v2 is not None and p.v1 >= SPEED_RES_MPS and v2 >= SPEED_RES_MPS and move < POS_RES_M,
        v2 is not None and v2 > MAX_SPEED_MPS,
        v2 is not None and abs(v2 - p.v1) / dt > MAX_ACCEL_MPS2,
        p.a2 is not None and abs(p.a2) > MAX_ACCEL_MPS2,
        p.brakes2 is True and p.a2 is not None and p.a2 >= 0.0 and v2 is not None and v2 >= SPEED_RES_MPS,
    )
    return fired, r10, (ex, ey)
