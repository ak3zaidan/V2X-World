//! Progress for long runs: one line per slice of simulated time.
//!
//! A run of days is driven in slices ([`v2xw_engine::Engine::advance`]), and at every
//! slice boundary this writes one JSON line to `progress.jsonl` in the output directory and
//! a short human line to stderr: the simulated instant, the wall clock the slice took, the
//! process's physical memory footprint and the sizes of the collections the engine keeps
//! ([`v2xw_engine::Engine::census`]). That is what "is the cost per simulated hour flat and
//! is memory bounded?" is answered from, while the run is still going.
//!
//! Nothing here is an input to the run: the stopwatch and the footprint are measurements
//! of the tool, written to a file no digest covers.

use std::io::Write;
use std::path::Path;

use crate::wall::Stopwatch;

/// The process's physical memory footprint, bytes — what Activity Monitor's "Memory"
/// column shows and what the machine's memory cap reads. `None` where it cannot be read.
#[must_use]
#[allow(unsafe_code)]
pub fn phys_footprint_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: `proc_pid_rusage` fills a caller-owned `rusage_info_v2` for this
        // process's own pid; the buffer is the size the flavour requires.
        unsafe {
            let mut info: libc::rusage_info_v2 = std::mem::zeroed();
            let rc = libc::proc_pid_rusage(
                libc::getpid(),
                libc::RUSAGE_INFO_V2,
                (&raw mut info).cast::<libc::rusage_info_t>(),
            );
            if rc == 0 {
                return Some(info.ri_phys_footprint);
            }
        }
        None
    }
    #[cfg(target_os = "linux")]
    {
        // VmRSS is the closest Linux counterpart.
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// Writes the progress lines of one run.
pub struct Progress {
    file: Option<std::io::BufWriter<std::fs::File>>,
    started: Stopwatch,
    last_wall_s: f64,
    last_t_s: f64,
    wall_offset_s: f64,
    bytes: u64,
    quiet: bool,
}

impl Progress {
    /// Opens `progress.jsonl` in `dir`. A file that cannot be created only loses the file:
    /// the stderr lines still appear and the run is not refused for it.
    pub fn open(dir: &Path, quiet: bool) -> Progress {
        let file = std::fs::File::create(dir.join("progress.jsonl"))
            .ok()
            .map(std::io::BufWriter::new);
        Progress {
            file,
            started: Stopwatch::start(),
            last_wall_s: 0.0,
            last_t_s: 0.0,
            wall_offset_s: 0.0,
            bytes: 0,
            quiet,
        }
    }

    /// Carries on `progress.jsonl` from a checkpoint: cut back to the `bytes` that were
    /// complete at simulated second `t_s`, after `wall_s` of earlier wall clock.
    pub fn resume(dir: &Path, quiet: bool, bytes: u64, t_s: f64, wall_s: f64) -> Progress {
        let path = dir.join("progress.jsonl");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .ok()
            .and_then(|f| f.set_len(bytes).ok().map(|()| f))
            .and_then(|f| {
                drop(f);
                std::fs::OpenOptions::new().append(true).open(&path).ok()
            })
            .map(std::io::BufWriter::new);
        Progress {
            file,
            started: Stopwatch::start(),
            last_wall_s: 0.0,
            last_t_s: t_s,
            wall_offset_s: wall_s,
            bytes,
            quiet,
        }
    }

    /// Bytes of `progress.jsonl` written so far, for a checkpoint.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Wall-clock seconds the run has spent driving the engine, across resumes.
    #[must_use]
    pub fn wall_s(&self) -> f64 {
        self.wall_offset_s + self.started.elapsed_s()
    }

    /// Restarts the slice clock, after a resume's replay, so the first slice after it is
    /// not charged with the replay.
    pub fn restart_slice(&mut self) {
        self.last_wall_s = self.started.elapsed_s();
    }

    /// One line on stderr only, for a resume's replay.
    pub fn note(&self, text: &str) {
        if !self.quiet {
            eprintln!("v2xw: {text}");
        }
    }

    /// Records one slice boundary at simulated second `t_s` with whatever extra fields the
    /// caller has (recording bytes so far, metric samples kept, …).
    pub fn line(
        &mut self,
        t_s: f64,
        duration_s: f64,
        census: &[(&'static str, u64)],
        extra: serde_json::Map<String, serde_json::Value>,
    ) {
        let elapsed = self.started.elapsed_s();
        let slice_wall = elapsed - self.last_wall_s;
        let wall_s = self.wall_offset_s + elapsed;
        let slice_sim = (t_s - self.last_t_s).max(f64::MIN_POSITIVE);
        let footprint = phys_footprint_bytes();
        let mut obj = serde_json::Map::new();
        obj.insert("t_s".into(), t_s.into());
        obj.insert("wall_s".into(), wall_s.into());
        obj.insert("slice_wall_s".into(), slice_wall.into());
        obj.insert(
            "wall_s_per_sim_h".into(),
            (slice_wall / slice_sim * 3600.0).into(),
        );
        if let Some(b) = footprint {
            obj.insert("footprint_mb".into(), (b as f64 / 1_048_576.0).into());
        }
        obj.extend(extra);
        let sizes: serde_json::Map<String, serde_json::Value> = census
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).into()))
            .collect();
        obj.insert("census".into(), sizes.into());
        if let Some(f) = self.file.as_mut() {
            let mut line = serde_json::to_vec(&serde_json::Value::Object(obj)).unwrap_or_default();
            line.push(b'\n');
            if f.write_all(&line).and_then(|()| f.flush()).is_ok() {
                self.bytes += line.len() as u64;
            }
        }
        if !self.quiet {
            let actors = census
                .iter()
                .find(|(k, _)| *k == "actors")
                .map_or(0, |(_, v)| *v);
            let eta = if t_s > 0.0 {
                wall_s / t_s * (duration_s - t_s)
            } else {
                f64::NAN
            };
            eprintln!(
                "v2xw: {} of {}  wall {}  slice {:.1} s ({:.0} s per simulated hour)  memory {}  actors {}  eta {}",
                fmt_sim(t_s),
                fmt_sim(duration_s),
                fmt_sim(wall_s),
                slice_wall,
                slice_wall / slice_sim * 3600.0,
                footprint.map_or_else(
                    || "?".to_string(),
                    |b| format!("{:.0} MB", b as f64 / 1_048_576.0)
                ),
                actors,
                fmt_sim(eta),
            );
        }
        self.last_wall_s = elapsed;
        self.last_t_s = t_s;
    }
}

/// A duration in seconds as `1d 02:03:04` / `02:03:04`.
#[must_use]
pub fn fmt_sim(s: f64) -> String {
    if !s.is_finite() || s < 0.0 {
        return "?".to_string();
    }
    let total = s.round() as u64;
    let (d, rem) = (total / 86_400, total % 86_400);
    let (h, m, sec) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    if d > 0 {
        format!("{d}d {h:02}:{m:02}:{sec:02}")
    } else {
        format!("{h:02}:{m:02}:{sec:02}")
    }
}

/// Parses a simulated duration: `90`, `90s`, `15m`, `6h`, `2d` (and combinations such as
/// `1d12h`). Plain numbers are seconds.
///
/// # Errors
/// A message naming what could not be read.
pub fn parse_duration_s(text: &str) -> Result<f64, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("an empty duration".to_string());
    }
    if let Ok(v) = t.parse::<f64>() {
        return if v.is_finite() && v >= 0.0 {
            Ok(v)
        } else {
            Err(format!("'{text}' is not a non-negative number of seconds"))
        };
    }
    let mut total = 0.0;
    let mut num = String::new();
    for c in t.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            continue;
        }
        let unit = match c {
            's' => 1.0,
            'm' => 60.0,
            'h' => 3600.0,
            'd' => 86_400.0,
            ' ' => continue,
            _ => return Err(format!("'{text}': unknown unit '{c}' (use s, m, h or d)")),
        };
        let v: f64 = num
            .parse()
            .map_err(|_| format!("'{text}': a unit without a number before it"))?;
        total += v * unit;
        num.clear();
    }
    if !num.is_empty() {
        return Err(format!("'{text}': '{num}' has no unit (use s, m, h or d)"));
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_parse_in_every_unit() {
        assert_eq!(parse_duration_s("90").unwrap(), 90.0);
        assert_eq!(parse_duration_s("15m").unwrap(), 900.0);
        assert_eq!(parse_duration_s("6h").unwrap(), 21_600.0);
        assert_eq!(parse_duration_s("2d").unwrap(), 172_800.0);
        assert_eq!(parse_duration_s("1d12h").unwrap(), 129_600.0);
        assert!(parse_duration_s("6x").is_err());
        assert!(parse_duration_s("12h5").is_err());
        assert_eq!(fmt_sim(90_061.0), "1d 01:01:01");
    }
}
