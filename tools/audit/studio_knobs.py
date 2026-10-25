#!/usr/bin/env python3
"""Does the settings panel change the engine? One knob per settings section, through the
server API the Studio's settings window uses (`scenario.set` with an RFC 6902 patch, then
`run.start`), each run to its end and compared with the base run on the same seed.

    studio_knobs.py --server BIN --scenario FILE --port N [--fault]

Starts `BIN --scenario FILE --port N --paused --speed 0`, runs the base and every knob
(written below with the direction theory gives, before any run), prints one line per knob
and a verdict line, and stops the server by PID. Exit 1 when a knob does not move its
observable the way its row says. `--fault` sends every knob's patch to a path the engine
ignores in the form (it applies the base document instead), which must turn every
`up`/`down` row red: the proof that the check can fail.

The observable is read back from the engine, never from the page: `run.status`
(`t_end_ns`, `actors`), or a metric pooled over the whole run with `metrics.query`.
"""
import argparse
import copy
import json
import os
import signal
import subprocess
import sys
import time
import urllib.request

# (section, JSON pointer, value, observable, expect, source). Observables: 'status.<key>'
# or 'metric.<name>' (pooled over the run), or 'catalogue' (how many metrics exist).
KNOBS = [
    ("Run", "/time/duration_s", 3.0, "status.t_end_ns", "down",
     "the run's horizon: half the simulated time"),
    ("World", "/world/buildings/enabled", False, "metric.prr", "up",
     "no building footprints, no obstacle loss (Sommer et al. 2011): more links clear the sensitivity"),
    ("Traffic", "/actors/vehicles/demand/rate_veh_per_h", 9000.0, "status.actors", "up",
     "three times the Poisson arrival rate: more vehicles in the same 6 s"),
    ("Environment", "/weather", {"initial": "rain", "intensity": 1.0}, "metric.traffic.speed_mean_mps", "down",
     "drivers slow in heavy rain (HCM 7th ed. Exhibit 11-20 speed adjustment; FHWA weather-responsive traffic)"),
    ("Radio", "/radio/devices/obu/tx_power_dbm", 5.0, "metric.prr", "down",
     "about 15 dB less EIRP: the far links fall under the sensitivity (Friis; IEEE 802.11-2020 Table 17-18)"),
    ("Network", "/net/channel_access", "alternating", "metric.e2e_ms_mean", "up",
     "IEEE 1609.4 alternating access: a frame generated in the service-channel interval waits up to 50 ms"),
    ("Messages", "/messages/rate_hz", 5.0, "metric.tx_rate_hz", "down",
     "5 Hz instead of J2945/1's 10 Hz"),
    ("Security", "/security/signer_id_policy/digest_otherwise", False, "metric.psdu_bytes", "up",
     "a certificate in every message instead of an 8-octet digest (IEEE 1609.2-2016 6.4.8)"),
    ("Nodes", "/nodes/peer_cert_cache", 1, "metric.aoi_ms", "up",
     "a one-entry peer-certificate cache: digest-signed messages from evicted signers wait for the next certificate"),
    ("Measurement", "/metrics", ["prr"], "catalogue", "down",
     "only the PRR provider is configured, so the catalogue shrinks"),
    ("Timeline", "/events", [{"t": 0.0, "until": 6.0, "type": "demand.multiplier", "value": 4.0}],
     "status.actors", "up", "a demand multiplier of 4 for the whole run: more arrivals"),
]


class Rpc:
    def __init__(self, port):
        self.url = f"http://127.0.0.1:{port}/rpc"
        self.n = 0

    def __call__(self, method, **params):
        self.n += 1
        body = json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}).encode()
        req = urllib.request.Request(self.url, data=body, headers={"content-type": "application/json"})
        with urllib.request.urlopen(req, timeout=600) as r:
            out = json.loads(r.read())
        if "error" in out:
            raise RuntimeError(f"{method}: {json.dumps(out['error'])[:400]}")
        return out["result"]


def wait_up(rpc, deadline=120):
    t0 = time.time()
    while time.time() - t0 < deadline:
        try:
            return rpc("run.status")
        except Exception:
            time.sleep(0.5)
    raise RuntimeError("server did not answer")


def run_to_end(rpc, timeout=900):
    rpc("run.start", speed=0)
    t0 = time.time()
    st = rpc("run.status")
    while st["state"] not in ("finished", "error"):
        if time.time() - t0 > timeout:
            raise RuntimeError(f"run did not finish: {st['state']} at {st['t_ns']}")
        time.sleep(0.5)
        st = rpc("run.status")
    if st["state"] == "error":
        raise RuntimeError(f"run errored: {json.dumps(st.get('engine'))[:300]}")
    return st


def observe(rpc, st, what):
    if what.startswith("status."):
        return float(st[what.split(".", 1)[1]])
    if what == "catalogue":
        return float(len(rpc("metrics.query")["catalogue"]))
    name = what.split(".", 1)[1]
    res = rpc("metrics.query", metrics=[name], bin_ns=10**15)
    cols = [c["name"] for c in res["columns"]]
    vals = [row[cols.index(name)] for row in res["rows"] if row[cols.index(name)] is not None]
    return float(sum(vals) / len(vals)) if vals else None


def apply(rpc, base_doc, pointer, value):
    doc = copy.deepcopy(base_doc)
    parts = [p for p in pointer.split("/") if p]
    cur = doc
    for p in parts[:-1]:
        cur = cur.setdefault(p, {})
    cur[parts[-1]] = value
    rpc("scenario.set", scenario=doc)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--server", required=True)
    ap.add_argument("--scenario", required=True)
    ap.add_argument("--port", type=int, default=8754)
    ap.add_argument("--duration", type=float, default=6.0)
    ap.add_argument("--fault", action="store_true")
    a = ap.parse_args()
    log = open(os.devnull, "w")
    srv = subprocess.Popen([a.server, "--scenario", a.scenario, "--port", str(a.port), "--paused",
                            "--speed", "0", "--quiet", "--no-agent", "--build-utc", "2026-01-01T00:00:00Z"],
                           stdout=log, stderr=log)
    fails = 0
    try:
        rpc = Rpc(a.port)
        wait_up(rpc)
        base_doc = rpc("scenario.get")["scenario"]
        base_doc.setdefault("time", {})["duration_s"] = a.duration
        rpc("scenario.set", scenario=base_doc)
        st = run_to_end(rpc)
        base = {k[3]: observe(rpc, st, k[3]) for k in KNOBS}
        for section, ptr, value, what, expect, source in KNOBS:
            try:
                if a.fault:
                    rpc("scenario.set", scenario=base_doc)
                else:
                    apply(rpc, base_doc, ptr, value)
                st = run_to_end(rpc)
                v = observe(rpc, st, what)
                b = base[what]
                ok = (v is not None and b is not None and
                      ((expect == "up" and v > b * 1.001) or (expect == "down" and v < b * 0.999)))
                line = f"{'PASS' if ok else 'FAIL'} {section:12s} {ptr} = {json.dumps(value)[:40]}: {what} {b} -> {v} ({expect})"
            except Exception as e:  # a refusal is a failure of the row, not of the script
                ok = False
                line = f"FAIL {section:12s} {ptr}: {str(e)[:200]}"
            fails += not ok
            print(line, flush=True)
            rpc("scenario.set", scenario=base_doc)
    finally:
        srv.send_signal(signal.SIGTERM)
        try:
            srv.wait(10)
        except subprocess.TimeoutExpired:
            srv.kill()
    print(f"{len(KNOBS) - fails}/{len(KNOBS)} settings sections moved the engine as expected")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
