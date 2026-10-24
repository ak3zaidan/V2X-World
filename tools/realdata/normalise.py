"""Stage 2 (normalise): one record schema for every source, streamed per temporary ID.

Units and conventions follow SAE J2735 (DE_Latitude/Longitude 1e-7 deg, DE_Speed 0.02 m/s,
DE_Heading 0.0125 deg clockwise from north, DE_Acceleration 0.01 m/s^2, DE_SemiMajorAxisAccuracy
0.05 m, DE_SecondMark ms within the minute, DE_MsgCount 0..127, BrakeAppliedStatus bit string
unavailable|leftFront|leftRear|rightFront|rightRear). Each source's unavailable sentinels map
to None. Records are plain tuples (memory: Orem has 4.1 M rows, never held as dicts).

Time bases per record:
  secmark  - DE_SecondMark (the BSM's own timestamp), all sources;
  header   - the 1609.2 header generation time as the dataset documents it (Wyoming
             `metadata_generatedat`, "taken from the communication header"; P1-E3 must still
             confirm it is not ODE log time);
  rx       - the receive/log time of an RSU (Tampa `metadata_recordgeneratedat`, Orem
             `tmstp_epoch`).
`order` is the time used to sequence one ID's BSMs (rx, header, or NYC's relative T_s).
"""
import csv
import datetime
import json
from collections import namedtuple

csv.field_size_limit(1 << 30)

Bsm = namedtuple("Bsm", "id secmark msgcnt order t_hdr t_rx lat lon speed heading accel smaj smin brakes")


def fnum(s):
    if s is None or s == "":
        return None
    try:
        return float(s)
    except ValueError:
        return None


def iso_s(t):
    """ISO-8601 local timestamp (no zone) to seconds; only differences are used."""
    if not t:
        return None
    t = t.rstrip("Z")
    for fmt in ("%Y-%m-%dT%H:%M:%S.%f", "%Y-%m-%dT%H:%M:%S"):
        try:
            d = datetime.datetime.strptime(t[:26], fmt)
            return (d - datetime.datetime(1970, 1, 1)).total_seconds()
        except ValueError:
            pass
    return None


def j_lat(v):
    return None if v is None or abs(v) >= 900000001 else v * 1e-7


def j_lon(v):
    return None if v is None or abs(v) >= 1800000001 else v * 1e-7


def j_speed(v):
    return None if v is None or v >= 8191 or v < 0 else v * 0.02


def j_heading(v):
    return None if v is None or v >= 28800 or v < 0 else v * 0.0125


def j_accel(v):
    return None if v is None or v >= 2001 or v <= -2001 else v * 0.01


def j_axis(v):
    return None if v is None or v >= 255 or v < 0 else v * 0.05


def j_secmark(v):
    return None if v is None or v >= 60000 or v < 0 else int(round(v))


def j_msgcnt(v):
    return None if v is None or v < 0 or v > 127 else int(round(v))


def j_brakes(bits):
    """BrakeAppliedStatus as a bit string; leading zeros may be stripped (Orem writes
    '1111' for 01111). True = a wheel brake applied, False = none, None = unavailable."""
    if bits is None or bits == "":
        return None
    b = bits.strip().zfill(5)
    if len(b) != 5 or set(b) - {"0", "1"}:
        return None
    if b[0] == "1":
        return None
    return "1" in b[1:]


def tampa_row(r):
    return Bsm(r["coredata_id"], j_secmark(fnum(r["coredata_secmark"])), j_msgcnt(fnum(r["coredata_msgcnt"])),
               iso_s(r["metadata_recordgeneratedat"]), None, iso_s(r["metadata_recordgeneratedat"]),
               j_lat(fnum(r["coredata_lat"])), j_lon(fnum(r["coredata_long"])),
               j_speed(fnum(r["coredata_speed"])), j_heading(fnum(r["coredata_heading"])),
               j_accel(fnum(r["coredata_accelset_long"])), j_axis(fnum(r["coredata_accuracy_semimajor"])),
               j_axis(fnum(r["coredata_accuracy_semiminor"])), j_brakes(r["coredata_wheelbrakes"]))


def orem_row(r):
    t = fnum(r["tmstp_epoch"])
    t = None if t is None else t / 1000.0
    return Bsm(r["temp_id"], j_secmark(fnum(r["sec_mark"])), j_msgcnt(fnum(r["msg_count"])), t, None, t,
               j_lat(fnum(r["latitude"])), j_lon(fnum(r["longitude"])), j_speed(fnum(r["speed"])),
               j_heading(fnum(r["heading"])), j_accel(fnum(r["longitudinal_acceleration"])),
               j_axis(fnum(r["semi_major_axis_accuracy"])), j_axis(fnum(r["semi_minor_axis_accuracy"])),
               j_brakes(r["wheel_brakes_flags"]))


def wyoming_row(r):
    """Wyoming is already in SI units (degrees, m/s, degrees, metres). Its brake columns are
    constant across all 6,597 rows (leftRear applied at 35 m/s everywhere), so they are read
    as unavailable; it has no longitudinal acceleration column."""
    t = iso_s(r["metadata_generatedat"])
    sp = fnum(r["coredata_speed"])
    hd = fnum(r["coredata_heading"])
    sm = j_secmark(fnum(r["coredata_secmark"]))
    return Bsm(r["coredata_id"], sm, j_msgcnt(fnum(r["coredata_msgcnt"])),
               unwrap_secmark(t, sm), t, None, fnum(r["coredata_position_lat"]), fnum(r["coredata_position_long"]),
               None if sp is None or sp < 0 or sp > 163.8 else sp,
               None if hd is None or hd < 0 or hd >= 360 else hd, None,
               fnum(r["coredata_accuracy_semimajor"]), fnum(r["coredata_accuracy_semiminor"]), None)


def unwrap_secmark(t, secmark):
    """The absolute time nearest t whose millisecond-of-minute is secmark: sequences an OBU
    log by its own timestamps (Wyoming's header time jitters by +-50 ms, which could swap
    neighbours). Falls back to t when secMark is unavailable."""
    if t is None or secmark is None:
        return t
    d = ((round(t * 1000) - secmark + 30000) % 60000) - 30000
    return (round(t * 1000) - d) / 1000.0


# NYC positions are X_m/Y_m translated to the warning BSM (DataProcessing.txt); they are
# carried in lat/lon slots as metres with this flag so pairing uses them directly.
NYC_XY = "xy"


def nyc_event_bsms(event_key, bsmlist):
    bl = json.loads(bsmlist) if isinstance(bsmlist, str) else bsmlist
    out = []
    for b in bl:
        cd = b["bsmRecord"]["bsmMsg"]["coreData"]
        acc = cd.get("accuracy", {}) or {}
        a = (cd.get("accelSet") or {}).get("long_mpss")
        wb = (cd.get("brakes") or {}).get("wheelBrakes")
        t = cd.get("T_s")
        out.append(Bsm("%s:%s" % (event_key, cd.get("id")), None, cd.get("msgCnt"), t, None, None,
                       cd.get("Y_m"), cd.get("X_m"), _nyc_speed(cd.get("speed_mps")), _nyc_heading(cd.get("heading_deg")),
                       a if a is not None and abs(a) < 20.01 else None, j_axis(acc.get("semiMajor")),
                       j_axis(acc.get("semiMinor")), None if wb in (None, "") else j_brakes(_hex_bits(wb))))
    return out


def _nyc_speed(v):
    # NYC converted J2735 speed to m/s; 8191 (unavailable) becomes 163.82
    return None if v is None or v < 0 or v >= 163.8 else v


def _nyc_heading(v):
    # 28800 (unavailable) becomes 360.0
    return None if v is None or v < 0 or v >= 360.0 else v


def _hex_bits(wb):
    # NYC writes wheelBrakes as a 2-hex-digit octet ('00'); the top 5 bits are the status.
    try:
        return format(int(wb, 16), "08b")[:5]
    except ValueError:
        return None


ROW = {"tampa": tampa_row, "orem": orem_row, "wyoming": wyoming_row}
ID_COL = {"tampa": "coredata_id", "orem": "temp_id", "wyoming": "coredata_id"}


def stream_ids(path, kind):
    """Yield (id, [Bsm...]) per temporary ID from a CSV ordered by ID (the pinned $order).
    Only one ID's rows are in memory at a time."""
    if kind == "nyc":
        with open(path, newline="") as f:
            for r in csv.DictReader(f):
                by = {}
                for b in nyc_event_bsms(r[":id"], r["bsmlist"]):
                    by.setdefault(b.id, []).append(b)
                for k in sorted(by):
                    yield k, by[k]
        return
    conv, idc = ROW[kind], ID_COL[kind]
    cur, buf = None, []
    seen_ids = set()
    with open(path, newline="") as f:
        for r in csv.DictReader(f):
            k = r[idc]
            if k != cur:
                if buf:
                    yield cur, buf
                if k in seen_ids:
                    raise ValueError(f"{path}: rows of ID {k} are not contiguous (bad $order)")
                seen_ids.add(k)
                cur, buf = k, []
            buf.append(conv(r))
    if buf:
        yield cur, buf
