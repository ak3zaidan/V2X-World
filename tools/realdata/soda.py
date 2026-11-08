"""Stage 1 (fetch): pinned SODA queries against data.transportation.gov.

Every query names its dataset id, its $select (only the columns the pipeline reads), its
$where and a total $order (ending in the Socrata row id `:id`, so paging is stable). Rows
are written as CSV, page by page, and the total is checked against `count(*)` with the
same $where. Raw rows stay outside the repository (CC BY-SA and unstated licences); only
the sha256 manifest is committed.
"""
import hashlib
import json
import os
import ssl
import time
import urllib.parse
import urllib.request

from version import USER_AGENT

BASE = "https://data.transportation.gov/resource/"
PAGE = 50000


def _ctx():
    # python.org builds on macOS ship without a CA store; use the system bundle. Never
    # disable verification.
    for cafile in (os.environ.get("SSL_CERT_FILE"), "/etc/ssl/cert.pem"):
        if cafile and os.path.exists(cafile):
            return ssl.create_default_context(cafile=cafile)
    return ssl.create_default_context()


def url(dataset, params, fmt="csv"):
    return BASE + dataset + "." + fmt + "?" + urllib.parse.urlencode(params, safe="(),*:'")


def get(u, tries=4):
    req = urllib.request.Request(u, headers={"User-Agent": USER_AGENT})
    for k in range(tries):
        try:
            with urllib.request.urlopen(req, timeout=300, context=_ctx()) as r:
                return r.read()
        except Exception:  # network hiccup: back off and retry, then give up loudly
            if k == tries - 1:
                raise
            time.sleep(5 * (k + 1))


def count(dataset, where=None):
    p = {"$select": "count(*)"}
    if where:
        p["$where"] = where
    return int(json.loads(get(url(dataset, p, "json")))[0]["count"])


def query_params(q):
    p = {"$select": ",".join(q["select"]), "$order": ",".join(q["order"])}
    if q.get("where"):
        p["$where"] = q["where"]
    return p


def fetch(q, path, page=PAGE, limit_rows=None):
    """Fetch query `q` (see sources.QUERIES) to `path`. Returns (rows, count_star)."""
    star = count(q["dataset"], q.get("where"))
    want = star if limit_rows is None else min(star, limit_rows)
    page = min(page, q.get("page", page))
    tmp = path + ".part"
    rows = 0
    with open(tmp, "wb") as f:
        while rows < want:
            p = query_params(q)
            p["$limit"] = str(min(page, want - rows))
            p["$offset"] = str(rows)
            body = get(url(q["dataset"], p))
            head, _, rest = body.partition(b"\n")
            if rows == 0:
                f.write(head + b"\n")
            n = _count_csv_records(rest)
            if n == 0:
                break
            f.write(rest if rest.endswith(b"\n") else rest + b"\n")
            rows += n
    if rows != want:
        raise RuntimeError(f"{q['name']}: fetched {rows} rows, count(*) says {want}")
    os.replace(tmp, path)
    return rows, star


def _count_csv_records(b):
    import csv
    import io
    csv.field_size_limit(1 << 30)
    return sum(1 for _ in csv.reader(io.StringIO(b.decode("utf-8"))))


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()
