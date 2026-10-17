"""1609.2 Time32 / Time64: TAI seconds / microseconds since 2004-01-01 00:00:00 UTC (02 section 4).

Leap-second table: IERS Bulletin C, Leap_Second.dat "Updated through IERS Bulletin 72 issued in July
2026", "File expires on 28 June 2027" (https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat,
sha256 6cb6f5d4b819f2e568e25db4b0b26d89dedf031fdffb18bc94d40f4e94e268d7, fetched 2026-10-16).
TAI-UTC was 32 s on 2004-01-01 and is 37 s since 2017-01-01: five leap seconds after the epoch.
"""

import calendar
import datetime as dt

from . import faults

EPOCH_POSIX = 1072915200  # 2004-01-01T00:00:00Z
TAI_MINUS_UTC_AT_EPOCH = 32
# (first UTC instant at which the new offset applies, TAI-UTC from then on)
LEAPS = [
    (dt.datetime(2006, 1, 1, tzinfo=dt.timezone.utc), 33),
    (dt.datetime(2009, 1, 1, tzinfo=dt.timezone.utc), 34),
    (dt.datetime(2012, 7, 1, tzinfo=dt.timezone.utc), 35),
    (dt.datetime(2015, 7, 1, tzinfo=dt.timezone.utc), 36),
    (dt.datetime(2017, 1, 1, tzinfo=dt.timezone.utc), 37),
]
TABLE_VALID_UNTIL = dt.datetime(2027, 6, 28, tzinfo=dt.timezone.utc)


def tai_minus_utc(t):
    off = TAI_MINUS_UTC_AT_EPOCH
    for when, value in LEAPS:
        if t >= when:
            off = value
    return off


def leaps_since_epoch(t):
    return 0 if faults.on('time32-no-leap') else tai_minus_utc(t) - TAI_MINUS_UTC_AT_EPOCH


def _check(t):
    if t.tzinfo is None:
        raise ValueError('aware UTC datetime required')
    if t > TABLE_VALID_UNTIL:
        raise ValueError('instant beyond the leap-second table validity (update from IERS Bulletin C)')


def time32(t):
    """Time32 for an aware UTC datetime (whole seconds)."""
    _check(t)
    posix = calendar.timegm(t.utctimetuple())
    return posix - EPOCH_POSIX + leaps_since_epoch(t)


def time64(t):
    """Time64 (microseconds) for an aware UTC datetime."""
    _check(t)
    posix = calendar.timegm(t.utctimetuple())
    return (posix - EPOCH_POSIX + leaps_since_epoch(t)) * 1_000_000 + t.microsecond


def from_time32(v):
    """UTC datetime for a Time32, or None for a value that falls inside an inserted leap second."""
    for off in range(0, len(LEAPS) + 1):
        t = dt.datetime.fromtimestamp(v + EPOCH_POSIX - off, dt.timezone.utc)
        if time32(t) == v:
            return t
    return None


# Known instants, derived by hand: (posix - 1072915200) + leap seconds inserted since 2004 (00 gap table row 4)
KNOWN = [
    (dt.datetime(2004, 1, 1, 0, 0, 0, tzinfo=dt.timezone.utc), 0),
    (dt.datetime(2005, 12, 31, 23, 59, 59, tzinfo=dt.timezone.utc), 63158399),
    (dt.datetime(2006, 1, 1, 0, 0, 0, tzinfo=dt.timezone.utc), 63158401),
    (dt.datetime(2015, 1, 6, 9, 0, 0, tzinfo=dt.timezone.utc), 347619603),
    (dt.datetime(2017, 1, 1, 0, 0, 0, tzinfo=dt.timezone.utc), 410313605),
    # SCMS Manager test CTL effectiveDate 679737605 (01 section 3.1) = 2025-07-16 08:00:00 UTC
    (dt.datetime(2025, 7, 16, 8, 0, 0, tzinfo=dt.timezone.utc), 679737605),
]
