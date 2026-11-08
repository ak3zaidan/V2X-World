#!/usr/bin/env python3
"""make_fixture.py --out <dir> [--layout release|runs]: a tiny synthetic two-run release.

Follows docs/dataset/schema.md table for table and column for column, so the release
tools can be built and tested before the Rust dataset exporter exists. It is NOT engine
output: ten vehicles on a straight road for six seconds, a closed-form radio, the six
SM-MBR v1.0 checks, a three-reporter MA rule and one CRL. What it does reproduce is the
shape of the real chain: a falsified BSM is received, a check fires, the OBU files an MBR
whose evidence digests are the SPDU digests, the MA decrypts it, links the pseudonyms,
revokes, publishes a CRL, and receivers start rejecting the attacker (on_crl).

  run r0001 (train): ConstPos attacker from 1.5 s
  run r0002 (test):  RandomPos attacker from 1.5 s (held out of train), one faulty GNSS

Layouts:
  release  one release root: runs.parquet, schema/, local/ global/ truth/ partitioned
           split=<s>/run=<id>/part-00000.parquet, compat/veremi/<run>.zip, scenarios/,
           manifest.json (what assemble_release.py produces, minus card and compat/dare)
  runs     one <out>/<run_id>/dataset/ folder per run, partitions under run=<id>/, a
           compat/veremi/ folder of JSON and a per-run manifest.json with "outputs":
           the shape assemble_release.py takes as input

Deterministic: every pseudo-random value is a SHA-256 of its name; no clock, no RNG.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import sys
import zipfile
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq

DURATION_S = 6.0
N_VEH = 10
BSM_HZ = 10
INSTRUMENTED = (0, 4, 8)          # vehicle indices whose rx/ego are written in full
CERT_CHANGE_S = 3.0
ATTACK_FROM_S = 1.5
FAULT_FROM_S = 4.0
ORIGIN_LAT, ORIGIN_LON = 42.2808, -83.7430
AREA_RADIUS_M = 2000.0
T0_UTC = "2026-06-01T12:00:00Z"
LEAP_S_SINCE_2004 = 5             # 2005, 2008, 2012, 2015, 2016: TAI-UTC grew by 5 s
PSID_BSM = 0x20
M_PER_DEG = 111_320.0
MS = 1_000_000
UPLOAD_DELAY_NS = 400 * MS
MA_DELAY_NS = 50 * MS
MAX_SPEED_MPS = 90.0              # SM-MBR Implausible/MaxSpeed default threshold
MAX_ACCEL_MPS2 = 10.0

RUNS = [
    dict(run_id="r0001", split="train", seed=11, attack="ConstPos", attacker=3, faulty=None,
         heldout=[]),
    dict(run_id="r0002", split="test", seed=12, attack="RandomPos", attacker=6, faulty=5,
         heldout=["RandomPos"]),
]
# attack: family, veremi_ext_code, veremi1_code, nextgen_name, dare_name
CROSSWALK = dict(ConstPos=("Position", 1, 1, None, "ConstPos"),
                 RandomPos=("Position", 4, 8, "randomPositionOffset", "RandomPos"))
CHECKS = [("ConstantPosition", "1VInconsistent", 1), ("RandomPosition", "1VInconsistent", 2),
          ("MaxSpeed", "Implausible", 1), ("MaxAcceleration", "Implausible", 2),
          ("BrakeAcceleration", "Implausible", 3), ("Distance", "ObsImplausible", 1)]


def h(*parts) -> bytes:
    return hashlib.sha256("|".join(map(str, parts)).encode()).digest()


def hx(nbytes: int, *parts) -> str:
    return h(*parts)[:nbytes].hex()


def u(*parts) -> float:
    return int.from_bytes(h(*parts)[:8], "big") / 2.0 ** 64


def latlon(x: float, y: float) -> tuple[float, float]:
    return (ORIGIN_LAT + y / M_PER_DEG,
            ORIGIN_LON + x / (M_PER_DEG * math.cos(math.radians(ORIGIN_LAT))))


def tai_us(t_ns: int) -> int:
    """IEEE 1609.2 Time64: TAI microseconds since 2004-01-01 00:00:00 UTC."""
    t0 = dt.datetime.fromisoformat(T0_UTC.replace("Z", "+00:00"))
    epoch = dt.datetime(2004, 1, 1, tzinfo=dt.timezone.utc)
    return int((t0 - epoch).total_seconds() + LEAP_S_SINCE_2004) * 1_000_000 + t_ns // 1000


def sec_mark(t_ns: int) -> int:
    """J2735 DSecond: milliseconds within the UTC minute."""
    t0 = dt.datetime.fromisoformat(T0_UTC.replace("Z", "+00:00"))
    return (t0.second * 1000 + t_ns // MS) % 60000


# ---------------------------------------------------------------------------------------
# Table schemas: (column, arrow type, nullable, description). Written to schema/<t>.json.

S, I32, I64, F, B = pa.string(), pa.int32(), pa.int64(), pa.float64(), pa.bool_()
PARAMS = pa.list_(pa.struct([("name", S), ("value", F)]))
SUBTYPES = pa.list_(pa.struct([("code", I32), ("name", S), ("params", PARAMS)]))
POSACC = pa.struct([("semi_major_m", F), ("semi_minor_m", F), ("orientation_deg", F)])
ACCEL = pa.struct([("long", F), ("lat", F), ("vert", F), ("yaw_rate", F)])
EVIDENCE = pa.list_(pa.struct([
    ("spdu_digest", S), ("generation_time_us", I64), ("sec_mark_ms", I32), ("msg_cnt", I32),
    ("lat_deg", F), ("lon_deg", F), ("speed_mps", F), ("heading_deg", F),
    ("accel_long_mps2", F), ("brakes", I32), ("pos_accuracy_m", F)]))
MIX = pa.list_(pa.struct([("id", S), ("fraction", F), ("from_s", F), ("to_s", F),
                          ("params", PARAMS)]))
K = False  # not nullable
N = True   # nullable

RUN_KEY = ("run_id", S, K, "run identifier, the run= partition key")
RX_REF = ("receiver_ref", S, K, "opaque id of the OBU that wrote the row")
SCHEMAS = {}
SCHEMAS["runs"] = [
    RUN_KEY,
    ("split", S, K, "train / validation / test, assigned per run"),
    ("heldout_attacks", pa.list_(S), K, "attacks absent from train, for zero-day tests"),
    ("city", S, K, "world name"), ("world_source", S, K, "osm or procedural"),
    ("t0_utc", S, K, "UTC instant of t = 0 (ISO 8601)"),
    ("duration_s", F, K, "simulated duration"),
    ("origin_lat_deg", F, K, "ENU origin latitude"),
    ("origin_lon_deg", F, K, "ENU origin longitude"),
    ("area_radius_m", F, K, "radius around the origin that holds every position"),
    ("vehicles", I64, K, "equipped vehicles in the run"),
    ("equipped_fraction", F, K, "fraction of vehicles with an OBU"),
    ("demand_veh_per_h", F, N, "traffic demand, null for a fixed fleet"),
    ("rat", S, K, "radio access technology"), ("bsm_rate_hz", F, K, "BSM rate"),
    ("scms_mode", S, K, "in-loop / abstracted / off"),
    ("calendar", S, K, "real or compressed"),
    ("calendar_factors", pa.struct([("shuffle_window_s", F), ("crl_cadence_s", F)]), N,
     "compressed-calendar settings"),
    ("local_suite", S, K, "OBU check suite"), ("ma_policy", S, K, "MA decision rule"),
    ("attacker_fraction", F, K, "fraction of vehicles that attack"),
    ("attack_mix", MIX, K, "attacks with fraction, schedule and parameters"),
    ("instrumented_receivers", I64, K, "OBUs whose local/rx and local/ego are complete"),
    ("instrumented_receiver_refs", pa.list_(S), K, "their receiver_ref values"),
    ("engine_version", S, K, "engine version"), ("git_sha", S, K, "engine commit"),
    ("scenario_hash", S, K, "SHA-256 of the resolved scenario"),
    ("seed", I64, K, "run seed"),
]
SCHEMAS["local/ego"] = [
    RUN_KEY, RX_REF, ("t_ns", I64, K, "ns since run start"),
    ("x_m", F, K, "own GNSS east, ENU"), ("y_m", F, K, "own GNSS north, ENU"),
    ("lat_deg", F, K, "own GNSS latitude"), ("lon_deg", F, K, "own GNSS longitude"),
    ("speed_mps", F, K, "own speed"), ("heading_deg", F, K, "own heading, clockwise from north"),
    ("accel_mps2", F, K, "own longitudinal acceleration"),
    ("pos_accuracy_m", F, K, "own GNSS accuracy (semi-major)"),
    ("own_cert_id", S, K, "HashedId8 of the pseudonym the OBU signs with"),
]
SCHEMAS["local/rx"] = [
    RUN_KEY, RX_REF, ("rx_id", S, K, "unique id of this reception row"),
    ("t_rx_ns", I64, K, "reception time, ns since run start"),
    ("spdu_digest", S, K, "first 8 bytes of SHA-256 over the signed SPDU"),
    ("rssi_dbm", F, K, "received power"), ("cbr", F, K, "receiver channel busy ratio"),
    ("size_bytes", I32, K, "SPDU size on air"),
    ("psid", I32, K, "IEEE 1609.2 PSID (0x20 = BSM)"),
    ("cert_id", S, K, "HashedId8 of the signing certificate"),
    ("signer_kind", S, K, "digest or certificate"),
    ("issuer_id", S, K, "HashedId8 of the issuing pseudonym CA"),
    ("verify_result", S, K, "ok, bad_signature, expired, not_yet_valid, on_crl, "
                            "unknown_issuer, not_checked"),
    ("verify_latency_us", I32, N, "signature verification latency"),
    ("generation_time_us", I64, K, "1609.2 generation time, TAI us since 2004"),
    ("msg_type", S, K, "J2735 message"), ("msg_cnt", I32, K, "J2735 msgCnt"),
    ("temp_id", S, K, "J2735 TemporaryID (hex)"), ("sec_mark_ms", I32, K, "J2735 secMark"),
    ("lat_deg", F, K, "claimed latitude"), ("lon_deg", F, K, "claimed longitude"),
    ("x_m", F, K, "claimed east, ENU"), ("y_m", F, K, "claimed north, ENU"),
    ("elevation_m", F, N, "claimed elevation"),
    ("pos_accuracy", POSACC, K, "claimed PositionalAccuracy ellipse"),
    ("speed_mps", F, K, "claimed speed"), ("heading_deg", F, K, "claimed heading"),
    ("accel", ACCEL, K, "claimed AccelerationSet4Way (m/s2, deg/s)"),
    ("brakes", I32, K, "claimed BrakeSystemStatus wheelBrakes bits"),
    ("transmission", S, N, "claimed TransmissionState"),
    ("length_m", F, N, "claimed vehicle length"), ("width_m", F, N, "claimed vehicle width"),
    ("event_flags", I32, N, "claimed VehicleEventFlags bits"),
    ("ego_x_m", F, K, "receiver own east at reception"),
    ("ego_y_m", F, K, "receiver own north at reception"),
    ("ego_speed_mps", F, K, "receiver own speed"),
    ("ego_heading_deg", F, K, "receiver own heading"),
    ("range_m", F, K, "distance from the receiver to the claimed position"),
]
SCHEMAS["local/checks"] = [
    RUN_KEY, RX_REF, ("t_ns", I64, K, "check time"),
    ("rx_id", S, K, "the reception checked"), ("cert_id", S, K, "sender pseudonym"),
    ("suite", S, K, "sm-mbr-us, research or ts103759"), ("check", S, K, "check name"),
    ("fired", B, K, "true when the check flagged the message"),
    ("score", F, K, "check score in [0, 1]"),
    ("excess", F, N, "amount over the threshold, null when not fired"),
]
SCHEMAS["local/reports"] = [
    RUN_KEY, RX_REF, ("report_id", S, K, "first 8 bytes of SHA-256 over the signed MBR"),
    ("t_created_ns", I64, K, "creation time"),
    ("subject_cert_id", S, K, "accused pseudonym"),
    ("mbr_type", S, K, "1VInconsistent, Implausible or ObsImplausible"),
    ("subtypes", SUBTYPES, K, "SM-MBR subtypes with parameters"),
    ("evidence_spdu_digests", pa.list_(S), K, "digests of the BSMs carried as evidence"),
    ("size_bytes", I32, K, "encoded MBR size"),
    ("fate", S, K, "uploaded, evicted, expired, pending_at_end"),
    ("t_uploaded_ns", I64, N, "upload time"), ("path", S, N, "cellular or rsu_relay"),
    ("weight_at_upload", F, N, "store weight when uploaded"),
]
SCHEMAS["local/pki"] = [
    RUN_KEY, RX_REF, ("t_ns", I64, K, "event time"),
    ("event", S, K, "enrolled, batch_downloaded, pseudonym_change, crl_downloaded, "
                    "download_refused"),
    ("cert_id", S, N, "pseudonym concerned"), ("crl_series", I32, N, "CRL series"),
    ("crl_number", I32, N, "CRL number"), ("crl_entries", I32, N, "entries in the CRL"),
    ("bytes", I64, N, "bytes downloaded"), ("latency_ms", F, N, "download latency"),
]
SCHEMAS["global/reports"] = [
    RUN_KEY, ("ma_id", S, K, "Misbehavior Authority"),
    ("report_id", S, K, "first 8 bytes of SHA-256 over the signed MBR"),
    ("t_received_ns", I64, K, "time the MA decrypted it"),
    ("generation_time_us", I64, K, "report generation time, TAI us since 2004"),
    ("generation_location", pa.struct([("lat_deg", F), ("lon_deg", F), ("elevation_m", F)]),
     K, "reporter position at generation"),
    ("psid", I32, K, "PSID of the reported messages (0x20)"),
    ("reporter_cert_id", S, K, "pseudonym that signed the report"),
    ("subject_cert_id", S, K, "accused pseudonym"),
    ("subject_issuer", S, K, "HashedId8 of the accused certificate issuer"),
    ("mbr_type", S, K, "1VInconsistent, Implausible or ObsImplausible"),
    ("subtypes", SUBTYPES, K, "SM-MBR subtypes with parameters"),
    ("evidence", EVIDENCE, K, "SignedBsmsWithCertificate, decoded"),
    ("evidence_start_us", I64, K, "earliest evidence generation time"),
    ("evidence_end_us", I64, K, "latest evidence generation time"),
    ("set_aside", S, N, "null, unverifiable or time_implausible"),
    ("la_link", S, N, "linkage token, null until the policy asks the LAs"),
    ("la_link_t_ns", I64, N, "time the LAs answered"),
]
SCHEMAS["global/decisions"] = [
    RUN_KEY, ("ma_id", S, K, "Misbehavior Authority"), ("t_ns", I64, K, "decision time"),
    ("subject_kind", S, K, "pseudonym or device"),
    ("subject_cert_id", S, K, "pseudonym the case opened on"),
    ("la_link", S, N, "linkage token when the subject is a device"),
    ("decision", S, K, "revoke, dismiss or monitor"), ("rule", S, K, "rule that decided"),
    ("report_ids", pa.list_(S), K, "reports the decision rests on"),
    ("n_reports", I32, K, "number of reports"),
    ("n_distinct_reporters", I32, K, "distinct reporter pseudonyms"),
]
SCHEMAS["global/crl"] = [
    RUN_KEY, ("t_issued_ns", I64, K, "CRL issue time"), ("crl_series", I32, K, "CRL series"),
    ("crl_number", I32, K, "CRL number"), ("entry_kind", S, K, "linkage_seed or hash_id"),
    ("la_link", S, N, "linkage token of the revoked device"),
    ("i_period", I32, N, "linkage i-period of the entry"),
    ("t_effective_ns", I64, K, "time the entry takes effect"),
]
SCHEMAS["truth/devices"] = [
    RUN_KEY, ("device_id", S, K, "engine device id (oracle only)"),
    ("class", S, K, "vehicle, rsu or vru"), ("provider", S, K, "OBU provider"),
    ("gnss_class", S, K, "GNSS receiver class"),
    ("label", S, K, "benign, faulty or malicious"), ("attack_id", S, N, "attack, if malicious"),
    ("attack_params", PARAMS, N, "attack parameters"),
    ("attack_start_ns", I64, N, "attack start"), ("attack_end_ns", I64, N, "attack end"),
    ("t_joined_ns", I64, K, "time the device joined"), ("t_left_ns", I64, N, "time it left"),
    ("la_link", S, K, "linkage token of the device"),
    ("receiver_ref", S, N, "its receiver_ref in local/"),
]
SCHEMAS["truth/pseudonyms"] = [
    RUN_KEY, ("cert_id", S, K, "HashedId8"), ("device_id", S, K, "owner"),
    ("valid_start_us", I64, K, "validity start, TAI us since 2004"),
    ("valid_end_us", I64, K, "validity end, TAI us since 2004"),
    ("t_first_use_ns", I64, N, "first signature"), ("t_last_use_ns", I64, N, "last signature"),
]
SCHEMAS["truth/transmissions"] = [
    RUN_KEY, ("tx_id", S, K, "engine transmission id (oracle only)"),
    ("t_tx_ns", I64, K, "transmission time"), ("device_id", S, K, "sender"),
    ("cert_id", S, K, "signing pseudonym"), ("spdu_digest", S, K, "join key to local/global"),
    ("label", S, K, "benign, faulty or malicious"), ("attack_id", S, N, "attack"),
    ("family", S, N, "attack family"), ("veremi_ext_code", I32, N, "F2MD / VeReMi Ext code"),
    ("veremi1_code", I32, N, "VeReMi 1.0 code"), ("nextgen_name", S, N, "VeReMi NextGen name"),
    ("dare_name", S, N, "DARE name"), ("fields_falsified", pa.list_(S), K, "falsified fields"),
    ("magnitude", F, N, "size of the falsification (m for position)"),
    ("x_m", F, K, "true east"), ("y_m", F, K, "true north"), ("speed_mps", F, K, "true speed"),
    ("heading_deg", F, K, "true heading"), ("accel_mps2", F, K, "true acceleration"),
    ("gnss_error_m", F, K, "sender honest GNSS error"),
    ("fix_to_stamp_ms", F, K, "GNSS fix to timestamp lag"),
]
SCHEMAS["truth/receptions"] = [
    RUN_KEY, ("rx_id", S, K, "reception"), ("tx_id", S, K, "transmission received"),
    ("device_id", S, K, "sender device"), ("receiver_device_id", S, K, "receiver device"),
    ("label", S, K, "label of the transmission"), ("true_distance_m", F, K, "true distance"),
]
SCHEMAS["truth/reports"] = [
    RUN_KEY, ("report_id", S, K, "report"), ("reporter_device_id", S, K, "reporter"),
    ("subject_device_id", S, K, "accused device"),
    ("subject_label", S, K, "label of the accused device"),
    ("verdict", S, K, "true_accusation or false_accusation"),
    ("fate", S, K, "store fate"), ("t_at_ma_ns", I64, N, "arrival at the MA"),
]
SCHEMAS["truth/timeline"] = [
    RUN_KEY, ("device_id", S, K, "misbehaving or accused device"),
    ("label", S, K, "its label"), ("attack_id", S, N, "its attack"),
    ("t_attack_start_ns", I64, N, "attack or fault start"),
    ("t_first_falsified_ns", I64, N, "first falsified or faulty message"),
    ("t_first_detection_ns", I64, N, "first check that fired on it"),
    ("t_first_report_created_ns", I64, N, "first report created"),
    ("t_first_report_uploaded_ns", I64, N, "first report uploaded"),
    ("t_first_report_at_ma_ns", I64, N, "first report at the MA"),
    ("t_decision_ns", I64, N, "MA decision"), ("t_crl_published_ns", I64, N, "CRL published"),
    ("t_crl_first_neighbour_ns", I64, N, "CRL at the first neighbour"),
    ("t_crl_median_neighbour_ns", I64, N, "CRL at the median neighbour"),
    ("t_crl_p95_neighbour_ns", I64, N, "CRL at the 95th-percentile neighbour"),
    ("t_last_malicious_accepted_ns", I64, N, "last falsified message a receiver accepted"),
]
SCHEMAS["truth/app_effects"] = [
    RUN_KEY, ("t_ns", I64, K, "warning time"), ("app", S, K, "FCW, EEBL, IMA, RLVW"),
    ("receiver_device_id", S, K, "device that raised it"),
    ("tx_id", S, K, "message that caused it"), ("severity", S, K, "advisory or warning"),
    ("driver_reaction", S, K, "none, brake or swerve"),
]


def arrow_schema(table: str) -> pa.Schema:
    return pa.schema([pa.field(n, t, nullable=nl) for n, t, nl, _ in SCHEMAS[table]])


# ---------------------------------------------------------------------------------------
# The synthetic world


class World:
    """Ten vehicles eastbound on a two-lane road; the attacker and faulty device of a run."""

    def __init__(self, spec: dict):
        self.spec, self.run = spec, spec["run_id"]
        r = self.run
        self.dur_ns = int(DURATION_S * 1e9)
        self.dev = [f"veh-{r}-{i:02d}" for i in range(N_VEH)]
        self.ref = ["obu-" + hx(6, r, "obu", i) for i in range(N_VEH)]
        self.la = [hx(8, r, "la", i) for i in range(N_VEH)]
        self.cert = [[hx(8, r, "cert", i, k) for k in (0, 1)] for i in range(N_VEH)]
        self.temp = [[hx(4, r, "tmp", i, k) for k in (0, 1)] for i in range(N_VEH)]
        self.issuer = hx(8, r, "pca")
        self.attacker, self.faulty, self.attack = spec["attacker"], spec["faulty"], spec["attack"]

    @staticmethod
    def true_state(i: int, t_ns: int):
        t = t_ns / 1e9
        v = 9.0 + 0.6 * i
        return -120.0 + 25.0 * i + v * t, (-1.75 if i % 2 == 0 else 1.75), v, 90.0

    def own_gnss(self, i: int, t_ns: int):
        """What device i believes about itself: truth plus GNSS error (and the fault)."""
        x, y, v, hd = self.true_state(i, t_ns)
        ex = (u(self.run, "gx", i, t_ns) - 0.5) * 0.8
        ey = (u(self.run, "gy", i, t_ns) - 0.5) * 0.8
        if i == self.faulty and t_ns >= FAULT_FROM_S * 1e9:
            ey += 30.0
        return x + ex, y + ey, v, hd, math.hypot(ex, ey)

    @staticmethod
    def cert_index(t_ns: int) -> int:
        return 0 if t_ns < CERT_CHANGE_S * 1e9 else 1

    def label(self, i: int, t_ns: int) -> str:
        if i == self.attacker and t_ns >= ATTACK_FROM_S * 1e9:
            return "malicious"
        if i == self.faulty and t_ns >= FAULT_FROM_S * 1e9:
            return "faulty"
        return "benign"

    def claimed(self, i: int, k: int, t_ns: int):
        """BSM core data as sent: honest GNSS, or the attack falsification."""
        x, y, v, hd, err = self.own_gnss(i, t_ns)
        if self.label(i, t_ns) == "malicious":
            if self.attack == "ConstPos":
                x, y, _, _ = self.true_state(i, int(ATTACK_FROM_S * 1e9))
            else:  # RandomPos: uniform over the playground
                x = (u(self.run, "rpx", i, k) - 0.5) * 600.0
                y = (u(self.run, "rpy", i, k) - 0.5) * 600.0
        return x, y, v, hd, err

    def transmissions(self) -> list[dict]:
        txs = []
        for i in range(N_VEH):
            for k in range(int(DURATION_S * BSM_HZ)):
                t_ns = k * 100 * MS + (i * 7 + 1) * MS
                if t_ns > self.dur_ns:
                    continue
                cx, cy, v, hd, err = self.claimed(i, k, t_ns)
                tx, ty, tv, thd = self.true_state(i, t_ns)
                ci = self.cert_index(t_ns)
                lat, lon = latlon(cx, cy)
                gen = tai_us(t_ns)
                with_cert = k % 10 == 0
                lab = self.label(i, t_ns)
                cert = self.cert[i][ci]
                digest = hx(8, "spdu", cert, gen, k % 128, f"{lat:.7f}", f"{lon:.7f}",
                            f"{v:.2f}", f"{hd:.4f}")
                txs.append(dict(i=i, k=k, t_ns=t_ns, cx=cx, cy=cy, lat=lat, lon=lon, v=v,
                                hd=hd, err=err, true=(tx, ty, tv, thd), cert=cert,
                                temp=self.temp[i][ci], gen=gen, msg_cnt=k % 128,
                                sec_mark=sec_mark(t_ns), with_cert=with_cert,
                                size=325 if with_cert else 196, label=lab, digest=digest))
        txs.sort(key=lambda d: (d["t_ns"], d["i"]))
        for n, d in enumerate(txs):
            d["tx_id"] = f"tx-{self.run}-{n:06d}"
        return txs

    def receptions(self, txs: list[dict]) -> list[dict]:
        out = []
        for d in txs:
            tx, ty = d["true"][0], d["true"][1]
            for j in range(N_VEH):
                if j == d["i"]:
                    continue
                rx, ry, _, _ = self.true_state(j, d["t_ns"])
                dist = math.hypot(tx - rx, ty - ry)
                if dist > 500.0 or u(self.run, "per", d["tx_id"], j) < 0.08:
                    continue
                out.append(dict(tx=d, j=j, dist=dist,
                                t_rx=d["t_ns"] + 100_000 + int(dist / 0.299792458),
                                rx_id=hx(8, self.run, "rx", d["tx_id"], j)))
        out.sort(key=lambda r: (r["j"], r["t_rx"]))
        return out


def crl_download_ns(crl: dict, j: int) -> int:
    return crl["t_issued"] + 300 * MS + j * 23 * MS


def revoked_for(world: World, crls: list[dict], j: int, i: int, t_ns: int) -> bool:
    return any(i in c["devices"] and crl_download_ns(c, j) <= min(t_ns, world.dur_ns)
               for c in crls)


def run_checks(world: World, rx: dict, prev: dict | None):
    """The six SM-MBR v1.0 checks on one verified BSM: [(name, fired, score, excess)]."""
    d = rx["tx"]
    ex, ey, _, _, _ = world.own_gnss(rx["j"], rx["t_rx"])
    rng = math.hypot(d["cx"] - ex, d["cy"] - ey)
    out = []
    cp = rp = None
    if prev is not None:
        step = math.hypot(d["cx"] - prev["cx"], d["cy"] - prev["cy"])
        dt_s = max((d["gen"] - prev["gen"]) / 1e6, 1e-3)
        cp = step < 0.01 and d["v"] > 1.0
        allowed = d["v"] * dt_s + 3.0
        rp = (step > allowed, step - allowed)
    out.append(("ConstantPosition", bool(cp), 1.0 if cp else 0.0, d["v"] if cp else None))
    fired = bool(rp and rp[0])
    out.append(("RandomPosition", fired, min(1.0, rp[1] / 10.0) if fired else 0.0,
                rp[1] if fired else None))
    out.append(("MaxSpeed", d["v"] > MAX_SPEED_MPS, 0.0, None))
    out.append(("MaxAcceleration", False, 0.0, None))
    out.append(("BrakeAcceleration", False, 0.0, None))
    far = rng > 500.0
    out.append(("Distance", far, 1.0 if far else 0.0, rng - 500.0 if far else None))
    return out, rng


def process(world: World, txs: list[dict], rxs: list[dict], crls: list[dict]) -> dict:
    """Every receiver verifies, checks and reports, given the CRLs it has downloaded."""
    checks, reports, seen = [], [], []
    last = {}
    reported = set()
    own_tx = {}
    for d in txs:
        own_tx.setdefault(d["i"], []).append(d)
    for rx in rxs:
        d, j = rx["tx"], rx["j"]
        rx["verify"] = "on_crl" if revoked_for(world, crls, j, d["i"], rx["t_rx"]) else "ok"
        rx["lat_us"] = 400 + int(u(world.run, "vl", rx["rx_id"]) * 200)
        seen.append(rx)
        if rx["verify"] != "ok":
            continue
        key = (j, d["cert"])
        prev = last.get(key)
        last[key] = d
        results, rng = run_checks(world, rx, prev)
        rx["range"] = rng
        t_chk = rx["t_rx"] + rx["lat_us"] * 1000 + 100_000
        for name, fired, score, excess in results:
            checks.append(dict(j=j, t=t_chk, rx=rx, name=name, fired=fired, score=score,
                               excess=excess))
        hit = [(n, e) for n, f, _, e in results if f]
        if not hit or key in reported:
            continue
        reported.add(key)
        name = hit[0][0]
        mbr_type, code = next((m, c) for n, m, c in CHECKS if n == name)
        if mbr_type == "1VInconsistent":
            ev = [prev, d]
        elif mbr_type == "Implausible":
            ev = [d]
        else:
            mine = [x for x in own_tx[j] if x["t_ns"] <= rx["t_rx"]]
            ev = [d] + mine[-1:]
        params = {"ConstantPosition": [], "RandomPosition": [("gpsDrift_mm", 1000.0)],
                  "MaxSpeed": [("threshold_mps", MAX_SPEED_MPS)],
                  "Distance": [("threshold_m", 500.0), ("gpsDrift_m", 1.0)]}.get(name, [])
        t_c = t_chk + 1 * MS
        rep_cert = world.cert[j][world.cert_index(t_c)]
        rid = hx(8, "mbr", rep_cert, d["cert"], t_c, *[e["digest"] for e in ev])
        t_up = t_c + UPLOAD_DELAY_NS + int(u(world.run, "up", rid) * 20) * MS
        up = t_up <= world.dur_ns
        reports.append(dict(j=j, report_id=rid, t_created=t_c, subject=d, cert=rep_cert,
                            mbr_type=mbr_type, code=code, name=name, params=params,
                            evidence=ev, fate="uploaded" if up else "pending_at_end",
                            t_up=t_up if up else None))
    return dict(rx=seen, checks=checks, reports=reports)


def ma(world: World, reports: list[dict], cert_owner: dict) -> dict:
    """legacy-window(k=3): revoke the device once three distinct reporters accuse one
    pseudonym; ask the LAs for its linkage value first; publish a CRL 200 ms later."""
    arrived = []
    for r in reports:
        if r["t_up"] is not None and r["t_up"] + MA_DELAY_NS <= world.dur_ns:
            arrived.append(dict(r, t_recv=r["t_up"] + MA_DELAY_NS))
    arrived.sort(key=lambda r: (r["t_recv"], r["report_id"]))
    by_subj, decisions, crls, revoked = {}, [], [], {}
    for r in arrived:
        subj = r["subject"]["cert"]
        dev = cert_owner[subj]
        case = by_subj.setdefault(subj, [])
        case.append(r)
        if dev in revoked:
            r["la"], r["la_t"] = world.la[dev], max(revoked[dev], r["t_recv"])
            continue
        reporters = {x["cert"] for x in case}
        if len(reporters) >= 3:
            la_t = r["t_recv"] + 20 * MS
            t_dec = la_t + 10 * MS
            revoked[dev] = la_t
            for x in case:
                x["la"], x["la_t"] = world.la[dev], la_t
            decisions.append(dict(t=t_dec, subject=subj, kind="device", la=world.la[dev],
                                  decision="revoke", ids=[x["report_id"] for x in case],
                                  n=len(case), nrep=len(reporters)))
            t_crl = t_dec + 200 * MS
            if t_crl <= world.dur_ns:
                crls.append(dict(t_issued=t_crl, number=len(crls) + 1, devices={dev},
                                 la=world.la[dev]))
    for subj, case in sorted(by_subj.items()):
        if cert_owner[subj] not in revoked:
            decisions.append(dict(t=min(case[-1]["t_recv"] + 10 * MS, world.dur_ns),
                                  subject=subj, kind="pseudonym", la=None, decision="monitor",
                                  ids=[x["report_id"] for x in case], n=len(case),
                                  nrep=len({x["cert"] for x in case})))
    decisions.sort(key=lambda x: x["t"])
    return dict(arrived=arrived, decisions=decisions, crls=crls)


def scenario_text(spec: dict) -> str:
    return "\n".join([
        "schema: v2xw/scenario/1",
        f"meta: {{ name: fixture {spec['run_id']}, note: synthetic fixture, not engine output }}",
        f"seed: {spec['seed']}",
        f"time: {{ duration_s: {DURATION_S} }}",
        f"actors: {{ vehicles: {{ fleet_size: {N_VEH} }} }}",
        "threats:",
        "  attackers:",
        f"    - {{ id: threat/attacker/legacy/{spec['attack']}, fraction: 0.1, "
        f"schedule: {{ from_s: {ATTACK_FROM_S}, to_s: {DURATION_S} }} }}",
        "detection: {{ ma: {{ id: threat/ma/legacy-window }} }}",
        "export: {{ dataset: {{ profile: release-v1 }} }}", ""])


def simulate(spec: dict) -> dict:
    w = World(spec)
    txs = w.transmissions()
    rxs = w.receptions(txs)
    cert_owner = {c: i for i in range(N_VEH) for c in w.cert[i]}
    crls: list[dict] = []
    for _ in range(4):  # CRLs change what receivers accept; iterate to a fixed point
        out = process(w, txs, rxs, crls)
        mres = ma(w, out["reports"], cert_owner)
        if [c["t_issued"] for c in mres["crls"]] == [c["t_issued"] for c in crls]:
            break
        crls = mres["crls"]
    return dict(world=w, txs=txs, **out, **mres, cert_owner=cert_owner)


def _ego_row(w: World, j: int, t_ns: int) -> dict:
    x, y, v, hd, _ = w.own_gnss(j, t_ns)
    lat, lon = latlon(x, y)
    return dict(run_id=w.run, receiver_ref=w.ref[j], t_ns=t_ns, x_m=x, y_m=y, lat_deg=lat,
                lon_deg=lon, speed_mps=v, heading_deg=hd, accel_mps2=0.0, pos_accuracy_m=1.5,
                own_cert_id=w.cert[j][w.cert_index(t_ns)])


def _evidence(e: dict) -> dict:
    return dict(spdu_digest=e["digest"], generation_time_us=e["gen"], sec_mark_ms=e["sec_mark"],
                msg_cnt=e["msg_cnt"], lat_deg=e["lat"], lon_deg=e["lon"], speed_mps=e["v"],
                heading_deg=e["hd"], accel_long_mps2=0.0, brakes=0, pos_accuracy_m=1.5)


def _subtypes(r: dict) -> list[dict]:
    return [dict(code=r["code"], name=r["name"],
                 params=[dict(name=n, value=v) for n, v in r["params"]])]


def local_rows(sim: dict) -> dict:
    w, r = sim["world"], sim["world"].run
    rows = {t: [] for t in ("local/ego", "local/rx", "local/checks", "local/reports",
                            "local/pki")}
    for j in INSTRUMENTED:
        for k in range(int(DURATION_S * BSM_HZ)):
            rows["local/ego"].append(_ego_row(w, j, k * 100 * MS + 50 * MS))
    for rx in sim["rx"]:
        j, d = rx["j"], rx["tx"]
        if j not in INSTRUMENTED:
            continue
        ex, ey, ev, ehd, _ = w.own_gnss(j, rx["t_rx"])
        rows["local/rx"].append(dict(
            run_id=r, receiver_ref=w.ref[j], rx_id=rx["rx_id"], t_rx_ns=rx["t_rx"],
            spdu_digest=d["digest"], rssi_dbm=-50.0 - 20.0 * math.log10(max(rx["dist"], 1.0) / 10.0),
            cbr=round(0.18 + 0.04 * u(r, "cbr", j, rx["t_rx"] // (100 * MS)), 4),
            size_bytes=d["size"], psid=PSID_BSM, cert_id=d["cert"],
            signer_kind="certificate" if d["with_cert"] else "digest", issuer_id=w.issuer,
            verify_result=rx["verify"], verify_latency_us=rx["lat_us"],
            generation_time_us=d["gen"], msg_type="bsm", msg_cnt=d["msg_cnt"],
            temp_id=d["temp"], sec_mark_ms=d["sec_mark"], lat_deg=d["lat"], lon_deg=d["lon"],
            x_m=d["cx"], y_m=d["cy"], elevation_m=250.0,
            pos_accuracy=dict(semi_major_m=1.5, semi_minor_m=1.0, orientation_deg=0.0),
            speed_mps=d["v"], heading_deg=d["hd"],
            accel=dict(long=0.0, lat=0.0, vert=0.0, yaw_rate=0.0), brakes=0,
            transmission="forwardGears", length_m=4.8, width_m=1.9, event_flags=0,
            ego_x_m=ex, ego_y_m=ey, ego_speed_mps=ev, ego_heading_deg=ehd,
            range_m=math.hypot(d["cx"] - ex, d["cy"] - ey)))
    for c in sim["checks"]:
        rows["local/checks"].append(dict(
            run_id=r, receiver_ref=w.ref[c["j"]], t_ns=c["t"], rx_id=c["rx"]["rx_id"],
            cert_id=c["rx"]["tx"]["cert"], suite="sm-mbr-us", check=c["name"],
            fired=c["fired"], score=c["score"], excess=c["excess"]))
    for p in sim["reports"]:
        up = p["t_up"] is not None
        rows["local/reports"].append(dict(
            run_id=r, receiver_ref=w.ref[p["j"]], report_id=p["report_id"],
            t_created_ns=p["t_created"], subject_cert_id=p["subject"]["cert"],
            mbr_type=p["mbr_type"], subtypes=_subtypes(p),
            evidence_spdu_digests=[e["digest"] for e in p["evidence"]],
            size_bytes=120 + 330 * len(p["evidence"]), fate=p["fate"], t_uploaded_ns=p["t_up"],
            path="cellular" if up else None, weight_at_upload=1.0 if up else None))
    for j in range(N_VEH):
        ev = [dict(t_ns=0, event="batch_downloaded", cert_id=None, bytes=20 * 152,
                   latency_ms=120.0),
              dict(t_ns=int(CERT_CHANGE_S * 1e9), event="pseudonym_change",
                   cert_id=w.cert[j][1])]
        for c in sim["crls"]:
            t_dl = crl_download_ns(c, j)
            if t_dl <= w.dur_ns:
                ev.append(dict(t_ns=t_dl, event="crl_downloaded", crl_series=1,
                               crl_number=c["number"], crl_entries=c["number"],
                               bytes=96 + 48 * c["number"], latency_ms=300.0 + j * 23.0))
        for e in sorted(ev, key=lambda e: e["t_ns"]):
            rows["local/pki"].append(dict(dict(run_id=r, receiver_ref=w.ref[j], cert_id=None,
                                               crl_series=None, crl_number=None,
                                               crl_entries=None, bytes=None, latency_ms=None),
                                          **e))
    for t, key in (("local/rx", "t_rx_ns"), ("local/checks", "t_ns"),
                   ("local/reports", "t_created_ns")):
        rows[t].sort(key=lambda x: (x["receiver_ref"], x[key]))
    rows["local/ego"].sort(key=lambda x: (x["receiver_ref"], x["t_ns"]))
    rows["local/pki"].sort(key=lambda x: (x["receiver_ref"], x["t_ns"]))
    return rows


def global_rows(sim: dict) -> dict:
    w, r = sim["world"], sim["world"].run
    rows = {t: [] for t in ("global/reports", "global/decisions", "global/crl")}
    for p in sim["arrived"]:
        x, y, _, _, _ = w.own_gnss(p["j"], p["t_created"])
        lat, lon = latlon(x, y)
        gens = [e["gen"] for e in p["evidence"]]
        rows["global/reports"].append(dict(
            run_id=r, ma_id="ma-1", report_id=p["report_id"], t_received_ns=p["t_recv"],
            generation_time_us=tai_us(p["t_created"]),
            generation_location=dict(lat_deg=lat, lon_deg=lon, elevation_m=250.0),
            psid=PSID_BSM, reporter_cert_id=p["cert"], subject_cert_id=p["subject"]["cert"],
            subject_issuer=w.issuer, mbr_type=p["mbr_type"], subtypes=_subtypes(p),
            evidence=[_evidence(e) for e in p["evidence"]],
            evidence_start_us=min(gens), evidence_end_us=max(gens), set_aside=None,
            la_link=p.get("la"), la_link_t_ns=p.get("la_t")))
    for d in sim["decisions"]:
        rows["global/decisions"].append(dict(
            run_id=r, ma_id="ma-1", t_ns=d["t"], subject_kind=d["kind"],
            subject_cert_id=d["subject"], la_link=d["la"], decision=d["decision"],
            rule="legacy-window(k=3,w=10s)", report_ids=d["ids"], n_reports=d["n"],
            n_distinct_reporters=d["nrep"]))
    for c in sim["crls"]:
        rows["global/crl"].append(dict(
            run_id=r, t_issued_ns=c["t_issued"], crl_series=1, crl_number=c["number"],
            entry_kind="linkage_seed", la_link=c["la"], i_period=1,
            t_effective_ns=c["t_issued"]))
    return rows


def truth_rows(sim: dict) -> dict:
    w, r, spec = sim["world"], sim["world"].run, sim["world"].spec
    rows = {t: [] for t in ("truth/devices", "truth/pseudonyms", "truth/transmissions",
                            "truth/receptions", "truth/reports", "truth/timeline",
                            "truth/app_effects")}
    for i in range(N_VEH):
        mal, fau = i == w.attacker, i == w.faulty
        rows["truth/devices"].append(dict(
            run_id=r, device_id=w.dev[i], **{"class": "vehicle"}, provider="provider-1",
            gnss_class="automotive-l1", label="malicious" if mal else "faulty" if fau else "benign",
            attack_id=w.attack if mal else None,
            attack_params=[dict(name="from_s", value=ATTACK_FROM_S)] if mal else None,
            attack_start_ns=int(ATTACK_FROM_S * 1e9) if mal else None,
            attack_end_ns=w.dur_ns if mal else None, t_joined_ns=0, t_left_ns=None,
            la_link=w.la[i], receiver_ref=w.ref[i]))
    uses = {}
    for d in sim["txs"]:
        uses.setdefault(d["cert"], []).append(d["t_ns"])
    for i in range(N_VEH):
        for k, c in enumerate(w.cert[i]):
            t = uses.get(c, [])
            rows["truth/pseudonyms"].append(dict(
                run_id=r, cert_id=c, device_id=w.dev[i],
                valid_start_us=tai_us(0) - 86_400_000_000 + k * 3_000_000,
                valid_end_us=tai_us(0) + 7 * 86_400_000_000,
                t_first_use_ns=min(t) if t else None, t_last_use_ns=max(t) if t else None))
    for d in sim["txs"]:
        mal = d["label"] == "malicious"
        fam, ve, v1, ng, dn = CROSSWALK[w.attack] if mal else (None,) * 5
        tx, ty, tv, thd = d["true"]
        rows["truth/transmissions"].append(dict(
            run_id=r, tx_id=d["tx_id"], t_tx_ns=d["t_ns"], device_id=w.dev[d["i"]],
            cert_id=d["cert"], spdu_digest=d["digest"], label=d["label"],
            attack_id=w.attack if mal else None, family=fam, veremi_ext_code=ve,
            veremi1_code=v1, nextgen_name=ng, dare_name=dn,
            fields_falsified=["position"] if mal else [],
            magnitude=math.hypot(d["cx"] - tx, d["cy"] - ty) if mal else None,
            x_m=tx, y_m=ty, speed_mps=tv, heading_deg=thd, accel_mps2=0.0,
            gnss_error_m=d["err"], fix_to_stamp_ms=0.0))
    for rx in sim["rx"]:
        d = rx["tx"]
        rows["truth/receptions"].append(dict(
            run_id=r, rx_id=rx["rx_id"], tx_id=d["tx_id"], device_id=w.dev[d["i"]],
            receiver_device_id=w.dev[rx["j"]], label=d["label"], true_distance_m=rx["dist"]))
    at_ma = {p["report_id"]: p["t_recv"] for p in sim["arrived"]}
    for p in sim["reports"]:
        subj = sim["cert_owner"][p["subject"]["cert"]]
        lab = rows["truth/devices"][subj]["label"]
        true_acc = any(e["label"] != "benign" for e in p["evidence"] if e["i"] == subj)
        rows["truth/reports"].append(dict(
            run_id=r, report_id=p["report_id"], reporter_device_id=w.dev[p["j"]],
            subject_device_id=w.dev[subj], subject_label=lab,
            verdict="true_accusation" if true_acc else "false_accusation", fate=p["fate"],
            t_at_ma_ns=at_ma.get(p["report_id"])))
    rows["truth/timeline"] = timeline_rows(sim, rows)
    if w.attack == "ConstPos":
        first = next(x for x in sim["rx"] if x["j"] == INSTRUMENTED[0]
                     and x["tx"]["label"] == "malicious")
        rows["truth/app_effects"].append(dict(
            run_id=r, t_ns=first["t_rx"] + 300 * MS, app="FCW",
            receiver_device_id=w.dev[INSTRUMENTED[0]], tx_id=first["tx"]["tx_id"],
            severity="warning", driver_reaction="none"))
    return rows


def timeline_rows(sim: dict, truth: dict) -> list[dict]:
    w = sim["world"]
    out = []
    owner = sim["cert_owner"]
    for i in sorted({owner[p["subject"]["cert"]] for p in sim["reports"]}
                    | {x for x in (w.attacker, w.faulty) if x is not None}):
        dev = truth["truth/devices"][i]

        def first(vals):
            vals = [v for v in vals if v is not None]
            return min(vals) if vals else None

        bad = [d["t_ns"] for d in sim["txs"] if d["i"] == i and d["label"] != "benign"]
        reps = [p for p in sim["reports"] if owner[p["subject"]["cert"]] == i]
        dec = [d["t"] for d in sim["decisions"]
               if d["decision"] == "revoke" and owner[d["subject"]] == i]
        crl = [c for c in sim["crls"] if i in c["devices"]]
        nb = sorted(crl_download_ns(crl[0], j) for j in range(N_VEH) if j != i) if crl else []
        nb = [t for t in nb if t <= w.dur_ns]
        acc = [x["t_rx"] for x in sim["rx"] if x["tx"]["i"] == i and x["verify"] == "ok"
               and x["tx"]["label"] != "benign"]
        start = (int(ATTACK_FROM_S * 1e9) if i == w.attacker else
                 int(FAULT_FROM_S * 1e9) if i == w.faulty else None)
        out.append(dict(
            run_id=w.run, device_id=w.dev[i], label=dev["label"], attack_id=dev["attack_id"],
            t_attack_start_ns=start, t_first_falsified_ns=first(bad),
            t_first_detection_ns=first(c["t"] for c in sim["checks"]
                                       if c["fired"] and c["rx"]["tx"]["i"] == i),
            t_first_report_created_ns=first(p["t_created"] for p in reps),
            t_first_report_uploaded_ns=first(p["t_up"] for p in reps),
            t_first_report_at_ma_ns=first(p["t_recv"] for p in sim["arrived"]
                                          if owner[p["subject"]["cert"]] == i),
            t_decision_ns=first(dec), t_crl_published_ns=crl[0]["t_issued"] if crl else None,
            t_crl_first_neighbour_ns=nb[0] if nb else None,
            t_crl_median_neighbour_ns=nb[len(nb) // 2] if nb else None,
            t_crl_p95_neighbour_ns=nb[min(len(nb) - 1, math.ceil(0.95 * len(nb)) - 1)] if nb else None,
            t_last_malicious_accepted_ns=max(acc) if acc else None))
    return out


def build_run(spec: dict) -> dict:
    """Every table of one run as a pyarrow Table, plus its compat and scenario files."""
    sim = simulate(spec)
    rows = {**local_rows(sim), **global_rows(sim), **truth_rows(sim)}
    w = sim["world"]
    rows["runs"] = [dict(
        run_id=w.run, split=spec["split"], heldout_attacks=spec["heldout"],
        city="fixture-straight-road", world_source="procedural", t0_utc=T0_UTC,
        duration_s=DURATION_S, origin_lat_deg=ORIGIN_LAT, origin_lon_deg=ORIGIN_LON,
        area_radius_m=AREA_RADIUS_M, vehicles=N_VEH, equipped_fraction=1.0,
        demand_veh_per_h=None, rat="dsrc-80211p", bsm_rate_hz=float(BSM_HZ),
        scms_mode="in-loop", calendar="compressed",
        calendar_factors=dict(shuffle_window_s=1.0, crl_cadence_s=0.2),
        local_suite="sm-mbr-us", ma_policy="legacy-window(k=3,w=10s)",
        attacker_fraction=0.1,
        attack_mix=[dict(id=spec["attack"], fraction=0.1, from_s=ATTACK_FROM_S,
                         to_s=DURATION_S, params=[])],
        instrumented_receivers=len(INSTRUMENTED),
        instrumented_receiver_refs=[w.ref[j] for j in INSTRUMENTED],
        engine_version="fixture-1", **{"git_sha": hx(20, "fixture", "commit")},
        scenario_hash=hashlib.sha256(scenario_text(spec).encode()).hexdigest(),
        seed=spec["seed"])]
    tables = {t: pa.Table.from_pylist(v, schema=arrow_schema(t)) for t, v in rows.items()}
    return dict(tables=tables, veremi=veremi_files(sim), scenario=scenario_text(spec))


def veremi_files(sim: dict) -> dict:
    """compat/veremi: VeReMi-style JSON lines per instrumented receiver plus ground truth.
    sender is derived from the pseudonym (the deliberate deviation in veremi.rs)."""
    w = sim["world"]
    files = {}

    def num(hexs):
        return int(hexs[:8], 16)

    for j in INSTRUMENTED:
        lines = []
        for k in range(int(DURATION_S * BSM_HZ)):
            e = _ego_row(w, j, k * 100 * MS + 50 * MS)
            lines.append(dict(type=2, rcvTime=e["t_ns"] / 1e9, pos=[e["x_m"], e["y_m"], 0.0],
                              spd=[e["speed_mps"], 0.0, 0.0]))
        for rx in sim["rx"]:
            if rx["j"] == j:
                d = rx["tx"]
                lines.append(dict(type=3, rcvTime=rx["t_rx"] / 1e9, sendTime=d["t_ns"] / 1e9,
                                  sender=num(d["cert"]), senderPseudo=num(d["cert"]),
                                  messageID=num(d["digest"]), pos=[d["cx"], d["cy"], 0.0],
                                  spd=[d["v"], 0.0, 0.0], RSSI=-60.0))
        lines.sort(key=lambda x: x["rcvTime"])
        files[f"traceJSON-{w.ref[j]}.json"] = "".join(json.dumps(x, sort_keys=True) + "\n"
                                                      for x in lines)
    gt = [dict(type=4, sendTime=d["t_ns"] / 1e9, sender=d["i"], messageID=num(d["digest"]),
               attackerType=CROSSWALK[w.attack][1] if d["label"] == "malicious" else 0,
               pos=list(d["true"][:2]) + [0.0]) for d in sim["txs"]]
    files["traceGroundTruthJSON.json"] = "".join(json.dumps(x, sort_keys=True) + "\n" for x in gt)
    return files


# ---------------------------------------------------------------------------------------
# Writing

UNITS = [("_mps2", "m/s2"), ("_mps", "m/s"), ("_dbm", "dBm"), ("_deg", "deg"), ("_ns", "ns"),
         ("_us", "us"), ("_ms", "ms"), ("_bytes", "bytes"), ("_hz", "Hz"), ("_m", "m"),
         ("_s", "s")]


def type_name(t: pa.DataType) -> str:
    if pa.types.is_list(t):
        return "list<" + type_name(t.value_type) + ">"
    if pa.types.is_struct(t):
        return "struct<" + ",".join(f.name + ":" + type_name(f.type) for f in t) + ">"
    return {"double": "float64", "bool": "bool", "string": "string"}.get(str(t), str(t))


def schema_doc(table: str) -> dict:
    view = table.split("/")[0] if "/" in table else "public"
    cols = []
    for name, typ, nullable, desc in SCHEMAS[table]:
        unit = next((un for suf, un in UNITS if name.endswith(suf)), None)
        cols.append(dict(name=name, type=type_name(typ), nullable=nullable, unit=unit,
                         visibility=view, description=desc))
    return dict(table=table, view=view, schema_version=1, columns=cols)


def write_parquet(path: Path, table: pa.Table) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(table, path, compression="zstd")


def write_zip(path: Path, files: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as zf:
        for name in sorted(files):
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            zf.writestr(info, files[name])


def file_list(root: Path, skip: set) -> list[dict]:
    out = []
    for p in sorted(root.rglob("*")):
        rel = p.relative_to(root).as_posix()
        if p.is_file() and rel not in skip:
            data = p.read_bytes()
            out.append(dict(path=rel, sha256=hashlib.sha256(data).hexdigest(), bytes=len(data)))
    return out


def write_release(out: Path, runs: list[dict]) -> None:
    built = [(spec, build_run(spec)) for spec in runs]
    for t in SCHEMAS:
        (out / "schema").mkdir(parents=True, exist_ok=True)
        (out / "schema" / f"{t.replace('/', '_')}.json").write_text(
            json.dumps(schema_doc(t), indent=1) + "\n")
    write_parquet(out / "runs.parquet",
                  pa.concat_tables([b["tables"]["runs"] for _, b in built]))
    for spec, b in built:
        for t, tbl in b["tables"].items():
            if t == "runs":
                continue
            write_parquet(out / t / f"split={spec['split']}" / f"run={spec['run_id']}"
                          / "part-00000.parquet", tbl)
        write_zip(out / "compat" / "veremi" / f"{spec['run_id']}.zip", b["veremi"])
        (out / "scenarios").mkdir(parents=True, exist_ok=True)
        (out / "scenarios" / f"{spec['run_id']}.yaml").write_text(b["scenario"])
    man = dict(dataset="v2x-scms-mbd-fixture", schema_version="1.0.0-draft",
               generator="tools/dataset/make_fixture.py", files=file_list(out, {"manifest.json"}))
    (out / "manifest.json").write_text(json.dumps(man, indent=1, sort_keys=True) + "\n")


def write_runs(out: Path, runs: list[dict]) -> None:
    """One <out>/<run>/dataset/ per run: the per-run shape assemble_release.py takes."""
    for spec in runs:
        b = build_run(spec)
        root = out / spec["run_id"] / "dataset"
        for t in SCHEMAS:
            (root / "schema").mkdir(parents=True, exist_ok=True)
            (root / "schema" / f"{t.replace('/', '_')}.json").write_text(
                json.dumps(schema_doc(t), indent=1) + "\n")
        write_parquet(root / "runs.parquet", b["tables"]["runs"])
        for t, tbl in b["tables"].items():
            if t != "runs":
                write_parquet(root / t / f"run={spec['run_id']}" / "part-00000.parquet", tbl)
        for name, text in b["veremi"].items():
            (root / "compat" / "veremi").mkdir(parents=True, exist_ok=True)
            (root / "compat" / "veremi" / name).write_text(text)
        (root / "scenario.yaml").write_text(b["scenario"])
        man = dict(run_id=spec["run_id"], dataset_version="v2x-scms-mbd/1.0.0-draft",
                   seed=spec["seed"], profile="release-v1",
                   scenario_hash=hashlib.sha256(b["scenario"].encode()).hexdigest(),
                   outputs=[dict(path=f["path"], sha256=f["sha256"])
                            for f in file_list(root, {"manifest.json"})])
        (root / "manifest.json").write_text(json.dumps(man, indent=1, sort_keys=True) + "\n")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", required=True, help="output folder (must be empty or absent)")
    ap.add_argument("--layout", choices=("release", "runs"), default="release")
    a = ap.parse_args(argv)
    out = Path(a.out)
    if out.exists() and any(out.iterdir()):
        print(f"refusing to write into a non-empty folder: {out}", file=sys.stderr)
        return 2
    out.mkdir(parents=True, exist_ok=True)
    (write_release if a.layout == "release" else write_runs)(out, RUNS)
    n = sum(1 for p in out.rglob("*") if p.is_file())
    print(f"fixture ({a.layout}): {n} files under {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
