//! A GTFS Schedule reader: the tables a timetable needs, streamed so a metropolitan feed's
//! `stop_times.txt` (millions of rows) is filtered while it is read rather than after.
//!
//! The format is the GTFS Schedule Reference [gtfs.org/schedule/reference, revised
//! 2024]: `stops.txt`, `routes.txt`, `trips.txt`, `stop_times.txt`, `calendar.txt`,
//! `calendar_dates.txt` and `frequencies.txt`. Times are `H:MM:SS` from "noon minus 12 h"
//! of the service day and may exceed `24:00:00` for trips running past midnight.
//! [`clip`] writes a smaller feed holding only a world's stops and one service day.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::error::{DemandError, Result};

/// A stop.
#[derive(Debug, Clone, PartialEq)]
pub struct Stop {
    /// `stop_id`.
    pub id: String,
    /// `stop_name`.
    pub name: String,
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
}

/// A route (a line).
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// `route_id`.
    pub id: String,
    /// `route_short_name`, else `route_long_name`.
    pub name: String,
    /// `route_type` (0 tram/streetcar/light rail, 3 bus, 11 trolleybus, …; extended
    /// types 700–799 bus, 900–999 tram).
    pub route_type: u16,
}

/// A trip.
#[derive(Debug, Clone, PartialEq)]
pub struct Trip {
    /// `trip_id`.
    pub id: String,
    /// `route_id`.
    pub route_id: String,
    /// `service_id`.
    pub service_id: String,
    /// `trip_headsign`.
    pub headsign: String,
}

/// One scheduled call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopTime {
    /// `stop_sequence`.
    pub seq: u32,
    /// Index into [`Feed::stop_ids`].
    pub stop: u32,
    /// Arrival, seconds from the service day's start; `None` when not a timepoint.
    pub arrival_s: Option<u32>,
    /// Departure, seconds.
    pub departure_s: Option<u32>,
}

/// A `frequencies.txt` row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frequency {
    /// First departure of the window, s.
    pub start_s: u32,
    /// End of the window, s.
    pub end_s: u32,
    /// Headway, s.
    pub headway_s: u32,
}

/// The tables of a feed this crate uses.
#[derive(Debug, Clone, Default)]
pub struct Feed {
    /// Stops by id.
    pub stops: BTreeMap<String, Stop>,
    /// Stop ids in index order, for [`StopTime::stop`].
    pub stop_ids: Vec<String>,
    /// Routes by id.
    pub routes: BTreeMap<String, Route>,
    /// Trips by id.
    pub trips: BTreeMap<String, Trip>,
    /// Each trip's calls, by `stop_sequence`.
    pub stop_times: BTreeMap<String, Vec<StopTime>>,
    /// `calendar.txt`: service → (weekday flags Mon..Sun, start YYYYMMDD, end YYYYMMDD).
    pub calendar: BTreeMap<String, ([bool; 7], u32, u32)>,
    /// `calendar_dates.txt`: (service, date) → 1 added, 2 removed.
    pub calendar_dates: BTreeMap<(String, u32), u8>,
    /// `frequencies.txt` rows by trip.
    pub frequencies: BTreeMap<String, Vec<Frequency>>,
}

/// Splits one CSV record, honouring quotes (RFC 4180).
pub fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            '\r' | '\n' if !quoted => {}
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// A table's rows as `(header → value)` lookups, streamed to `each`.
fn read_table(path: &Path, mut each: impl FnMut(&dyn Fn(&str) -> String) -> Result<()>) -> Result<bool> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(DemandError::io(path, e)),
    };
    let mut lines = BufReader::new(file).lines();
    let Some(head) = lines.next() else {
        return Ok(true);
    };
    let head = head.map_err(|e| DemandError::io(path, e))?;
    let cols: Vec<String> = split_csv(head.trim_start_matches('\u{feff}'))
        .into_iter()
        .map(|c| c.trim().to_string())
        .collect();
    let index: BTreeMap<String, usize> = cols.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
    for line in lines {
        let line = line.map_err(|e| DemandError::io(path, e))?;
        if line.trim().is_empty() {
            continue;
        }
        let f = split_csv(&line);
        let get = |name: &str| -> String {
            index
                .get(name)
                .and_then(|i| f.get(*i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        each(&get)?;
    }
    Ok(true)
}

/// Parses a GTFS time `H:MM:SS` into seconds; `None` for an empty field.
pub fn parse_time(s: &str) -> Option<u32> {
    let mut it = s.trim().split(':');
    let h: u32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let sec: u32 = it.next().unwrap_or("0").parse().ok()?;
    Some(h * 3600 + m * 60 + sec)
}

/// The weekday of a `YYYYMMDD` date, 0 = Monday (Sakamoto's method).
pub fn weekday(date: u32) -> usize {
    let (mut y, m, d) = (date / 10000, (date / 100) % 100, date % 100);
    const T: [u32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 {
        y -= 1;
    }
    let sunday0 = (y + y / 4 - y / 100 + y / 400 + T[(m as usize).saturating_sub(1) % 12] + d) % 7;
    ((sunday0 + 6) % 7) as usize
}

/// Which trips to read.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Only stops inside this box `[south, west, north, east]` are kept, and only the calls
    /// at them.
    pub bbox: Option<[f64; 4]>,
    /// Only trips running on this `YYYYMMDD` service date.
    pub date: Option<u32>,
    /// Only these route types (empty: all).
    pub route_types: Vec<u16>,
}

impl Feed {
    /// The services running on `date`.
    pub fn services_on(&self, date: u32) -> BTreeSet<String> {
        let wd = weekday(date);
        let mut out: BTreeSet<String> = self
            .calendar
            .iter()
            .filter(|(_, (days, a, b))| days[wd] && date >= *a && date <= *b)
            .map(|(s, _)| s.clone())
            .collect();
        for ((s, d), kind) in &self.calendar_dates {
            if *d == date {
                if *kind == 1 {
                    out.insert(s.clone());
                } else if *kind == 2 {
                    out.remove(s);
                }
            }
        }
        out
    }

    /// Reads the feed in directory `dir`, keeping what `filter` selects.
    ///
    /// # Errors
    /// A missing required table, I/O errors, unparsable numbers.
    pub fn load(dir: &Path, filter: &Filter) -> Result<Feed> {
        let mut feed = Feed::default();
        let bad = |table: &str, what: String| DemandError::parse(&dir.join(table), what);
        let found = read_table(&dir.join("stops.txt"), |get| {
            let (Ok(lat), Ok(lon)) = (get("stop_lat").parse::<f64>(), get("stop_lon").parse::<f64>()) else {
                return Ok(()); // a station entrance or node without a position
            };
            if let Some([s, w, n, e]) = filter.bbox
                && !(lat >= s && lat <= n && lon >= w && lon <= e)
            {
                return Ok(());
            }
            let id = get("stop_id");
            feed.stops.insert(
                id.clone(),
                Stop {
                    id,
                    name: get("stop_name"),
                    lat,
                    lon,
                },
            );
            Ok(())
        })?;
        if !found {
            return Err(bad("stops.txt", "missing".to_string()));
        }
        feed.stop_ids = feed.stops.keys().cloned().collect();
        let stop_index: BTreeMap<&str, u32> =
            feed.stop_ids.iter().enumerate().map(|(i, s)| (s.as_str(), i as u32)).collect();
        read_table(&dir.join("routes.txt"), |get| {
            let route_type: u16 = get("route_type").parse().unwrap_or(3);
            if !filter.route_types.is_empty() && !filter.route_types.contains(&route_type) {
                return Ok(());
            }
            let short = get("route_short_name");
            let id = get("route_id");
            feed.routes.insert(
                id.clone(),
                Route {
                    id,
                    name: if short.is_empty() { get("route_long_name") } else { short },
                    route_type,
                },
            );
            Ok(())
        })?;
        read_table(&dir.join("calendar.txt"), |get| {
            let day = |n: &str| get(n) == "1";
            let days = [
                day("monday"),
                day("tuesday"),
                day("wednesday"),
                day("thursday"),
                day("friday"),
                day("saturday"),
                day("sunday"),
            ];
            let a: u32 = get("start_date").parse().unwrap_or(0);
            let b: u32 = get("end_date").parse().unwrap_or(u32::MAX);
            feed.calendar.insert(get("service_id"), (days, a, b));
            Ok(())
        })?;
        read_table(&dir.join("calendar_dates.txt"), |get| {
            if let (Ok(d), Ok(k)) = (get("date").parse::<u32>(), get("exception_type").parse::<u8>()) {
                feed.calendar_dates.insert((get("service_id"), d), k);
            }
            Ok(())
        })?;
        let services = filter.date.map(|d| feed.services_on(d));
        read_table(&dir.join("trips.txt"), |get| {
            let route_id = get("route_id");
            if !feed.routes.contains_key(&route_id) {
                return Ok(());
            }
            let service_id = get("service_id");
            if let Some(s) = &services
                && !s.contains(&service_id)
            {
                return Ok(());
            }
            let id = get("trip_id");
            feed.trips.insert(
                id.clone(),
                Trip {
                    id,
                    route_id,
                    service_id,
                    headsign: get("trip_headsign"),
                },
            );
            Ok(())
        })?;
        let found = read_table(&dir.join("stop_times.txt"), |get| {
            let trip = get("trip_id");
            if !feed.trips.contains_key(&trip) {
                return Ok(());
            }
            let Some(stop) = stop_index.get(get("stop_id").as_str()).copied() else {
                return Ok(());
            };
            let seq: u32 = get("stop_sequence")
                .parse()
                .map_err(|_| bad("stop_times.txt", format!("trip {trip}: bad stop_sequence")))?;
            feed.stop_times.entry(trip).or_default().push(StopTime {
                seq,
                stop,
                arrival_s: parse_time(&get("arrival_time")),
                departure_s: parse_time(&get("departure_time")),
            });
            Ok(())
        })?;
        if !found {
            return Err(bad("stop_times.txt", "missing".to_string()));
        }
        read_table(&dir.join("frequencies.txt"), |get| {
            let trip = get("trip_id");
            if !feed.trips.contains_key(&trip) {
                return Ok(());
            }
            if let (Some(a), Some(b), Ok(h)) = (
                parse_time(&get("start_time")),
                parse_time(&get("end_time")),
                get("headway_secs").parse::<u32>(),
            ) && h > 0
            {
                feed.frequencies.entry(trip).or_default().push(Frequency {
                    start_s: a,
                    end_s: b,
                    headway_s: h,
                });
            }
            Ok(())
        })?;
        for calls in feed.stop_times.values_mut() {
            calls.sort_by_key(|c| c.seq);
        }
        // Trips with no call left inside the box are of no use.
        let keep: BTreeSet<String> = feed.stop_times.keys().cloned().collect();
        feed.trips.retain(|id, _| keep.contains(id));
        Ok(feed)
    }
}

/// Writes the part of the feed in `src` that `filter` selects to `dst`: the world's stops
/// and the trips running on one day, with only their calls inside the box. A metropolitan
/// feed of hundreds of megabytes becomes a city centre's day of a few.
///
/// # Errors
/// I/O and parse errors.
pub fn clip(src: &Path, dst: &Path, filter: &Filter) -> Result<(usize, usize, usize)> {
    let feed = Feed::load(src, filter)?;
    std::fs::create_dir_all(dst).map_err(|e| DemandError::io(dst, e))?;
    let q = |s: &str| {
        if s.contains(',') || s.contains('"') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let write = |name: &str, body: String| -> Result<()> {
        let p = dst.join(name);
        let mut f = std::fs::File::create(&p).map_err(|e| DemandError::io(&p, e))?;
        f.write_all(body.as_bytes()).map_err(|e| DemandError::io(&p, e))
    };
    let used_stops: BTreeSet<u32> = feed.stop_times.values().flatten().map(|c| c.stop).collect();
    let mut s = String::from("stop_id,stop_name,stop_lat,stop_lon\n");
    for i in &used_stops {
        let st = &feed.stops[&feed.stop_ids[*i as usize]];
        s += &format!("{},{},{},{}\n", q(&st.id), q(&st.name), st.lat, st.lon);
    }
    write("stops.txt", s)?;
    let used_routes: BTreeSet<&str> = feed.trips.values().map(|t| t.route_id.as_str()).collect();
    let mut s = String::from("route_id,route_short_name,route_long_name,route_type\n");
    for r in feed.routes.values().filter(|r| used_routes.contains(r.id.as_str())) {
        s += &format!("{},{},,{}\n", q(&r.id), q(&r.name), r.route_type);
    }
    write("routes.txt", s)?;
    let mut s = String::from("route_id,service_id,trip_id,trip_headsign\n");
    for t in feed.trips.values() {
        s += &format!("{},{},{},{}\n", q(&t.route_id), q(&t.service_id), q(&t.id), q(&t.headsign));
    }
    write("trips.txt", s)?;
    let fmt = |t: Option<u32>| {
        t.map(|t| format!("{:02}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60))
            .unwrap_or_default()
    };
    let mut s = String::from("trip_id,arrival_time,departure_time,stop_id,stop_sequence\n");
    let mut rows = 0usize;
    for (trip, calls) in &feed.stop_times {
        for c in calls {
            rows += 1;
            s += &format!(
                "{},{},{},{},{}\n",
                q(trip),
                fmt(c.arrival_s),
                fmt(c.departure_s),
                q(&feed.stop_ids[c.stop as usize]),
                c.seq
            );
        }
    }
    write("stop_times.txt", s)?;
    let services: BTreeSet<&str> = feed.trips.values().map(|t| t.service_id.as_str()).collect();
    let mut s = String::from(
        "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n",
    );
    for (sid, (days, a, b)) in feed.calendar.iter().filter(|(k, _)| services.contains(k.as_str())) {
        let d: Vec<&str> = days.iter().map(|x| if *x { "1" } else { "0" }).collect();
        s += &format!("{},{},{},{}\n", q(sid), d.join(","), a, b);
    }
    write("calendar.txt", s)?;
    let mut s = String::from("service_id,date,exception_type\n");
    for ((sid, d), k) in feed.calendar_dates.iter().filter(|((k, _), _)| services.contains(k.as_str())) {
        s += &format!("{},{},{}\n", q(sid), d, k);
    }
    write("calendar_dates.txt", s)?;
    if !feed.frequencies.is_empty() {
        let mut s = String::from("trip_id,start_time,end_time,headway_secs\n");
        for (trip, fs) in &feed.frequencies {
            for f in fs {
                s += &format!("{},{},{},{}\n", q(trip), fmt(Some(f.start_s)), fmt(Some(f.end_s)), f.headway_s);
            }
        }
        write("frequencies.txt", s)?;
    }
    Ok((used_stops.len(), feed.trips.len(), rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_and_times() {
        assert_eq!(split_csv("a,\"b,c\",\"d\"\"e\""), vec!["a", "b,c", "d\"e"]);
        assert_eq!(parse_time("25:01:02"), Some(25 * 3600 + 62));
        assert_eq!(parse_time(""), None);
    }

    #[test]
    fn weekdays() {
        assert_eq!(weekday(20261007), 2); // a Wednesday
        assert_eq!(weekday(20000101), 5); // a Saturday
    }
}
