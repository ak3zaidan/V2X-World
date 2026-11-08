"""Manhattan's hourly traffic profile, from NYC DOT Automated Traffic Volume Counts.

Reproduces the tables in crates/v2xw-mobility/src/demand/daily.rs
(NYC_DOT_MANHATTAN_WEEKDAY, NYC_DOT_MANHATTAN_WEEKEND, WEEKEND_VOLUME_RATIO).

Source: NYC Open Data dataset 7ym2-wayt, "Automated Traffic Volume Counts" (NYC DOT): counts
by automated traffic recorder in 15-minute intervals. Accessed 2026-10-07.

Fetch the per-(date, hour) aggregates for Manhattan from 2015 on (one page held all 20,102
rows when this was run; a second page is fetched in case the dataset has grown):

    curl -s "https://data.cityofnewyork.us/resource/7ym2-wayt.json?\\$select=yr,m,d,hh,sum(vol)%20as%20v,count(*)%20as%20n&\\$where=boro='Manhattan'%20AND%20yr>=2015&\\$group=yr,m,d,hh&\\$order=yr,m,d,hh&\\$limit=50000&\\$offset=0" -o p0.json
    curl -s "...same...&\\$offset=50000" -o p1.json
    python3 nyc-dot-hourly-profile.py p0.json p1.json

Each hour's factor is the mean volume per 15-minute count record in that hour (so a day with
more recorders counting does not weigh more), split into weekdays and weekends by the date,
and normalised so the 24 factors average 1. The weekend's volume relative to the weekday's
is the ratio of the two per-record means over all hours.
"""
import datetime
import json
import sys

rows = []
for f in sys.argv[1:]:
    rows += json.load(open(f))
acc = {"weekday": [[0.0, 0] for _ in range(24)], "weekend": [[0.0, 0] for _ in range(24)]}
days = {"weekday": set(), "weekend": set()}
for r in rows:
    date = datetime.date(int(r["yr"]), int(r["m"]), int(r["d"]))
    kind = "weekend" if date.weekday() >= 5 else "weekday"
    h = int(r["hh"])
    acc[kind][h][0] += float(r["v"])
    acc[kind][h][1] += int(r["n"])
    days[kind].add(date)
out = {"rows": len(rows), "years": sorted({int(r["yr"]) for r in rows})}
means = {}
for kind, hours in acc.items():
    per = [v / n for v, n in hours]
    mean = sum(per) / 24
    out[kind] = {
        "factors": [round(p / mean, 3) for p in per],
        "days": len(days[kind]),
        "records": sum(n for _, n in hours),
    }
    means[kind] = sum(v for v, _ in hours) / sum(n for _, n in hours)
out["weekend_volume_ratio"] = round(means["weekend"] / means["weekday"], 3)
print(json.dumps(out, indent=1))
