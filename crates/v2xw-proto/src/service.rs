//! Backend service models: the reason provisioning costs something.
//!
//! 06-node-models §4 says it plainly — "nothing in a backend entity is instantaneous". An
//! entity here is a `c`-server queue: a request occupies one server for a service time
//! composed from the cryptographic operations it performs plus a fixed per-request
//! overhead, and a request that arrives while every server is busy waits. That is the
//! `medium` tier's structure with a deterministic service time.
//!
//! The `high` tier's availability model is [`AvailabilityModel`] (SC-P12.1): a two-state
//! alternating renewal process per entity (06-node-models §4), off by default. Its
//! parameters are `todo-calibrate` in the design, so every value is a labelled
//! ASSUMPTION (`docs/design/scms-real/constants.md`) for the sensitivity sweeps (09 §6).
//! Its down intervals are applied as `offline` faults ([`crate::fault`]), exactly as a
//! scenario's `actors.backend.failures[]` window is.
//!
//! Determinism: no RNG stream, no float in the queue. Service times are sums of the
//! integer nanosecond durations of `v2xw_sec::primitive`'s cost tables, and the server a
//! request lands on is the lowest-indexed one that is free earliest, chosen by an
//! explicit total order. The availability draws are keyed hashes ([`keyed_uniform`]):
//! each interval is a pure function of `(seed, entity, k, kind)`, and takes no position
//! in any stream.

use v2xw_core::time::{Duration, SimTime};

/// The batching windows of 06-node-models §4: release when either bound is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchPolicy {
    /// Release when this many items are held.
    pub max_items: u32,
    /// Release when the oldest held item is this old.
    pub max_delay: Duration,
}

impl BatchPolicy {
    /// The CAMP PoC shuffle: 10,000 requests or one day, whichever comes first.
    ///
    /// [CAMP-EE §2.2.7 for the certificate-request shuffle; SCMS-765 for the report
    /// shuffle; restated in 05-protocols §3.2 and 06-node-models §4.]
    pub const CAMP_SHUFFLE: BatchPolicy = BatchPolicy {
        max_items: 10_000,
        max_delay: Duration::from_secs(86_400),
    };

    /// The CAMP 1.2.2 certificate-request shuffle, the engine's default: release at
    /// `shuffle_threshold` = 1,000 requests ("minimum number for full privacy mode") or
    /// when the oldest request reaches `max_cert_request_age` = 2 days, whichever comes
    /// first ([CAMP122] §5.2.15.5 Tables 67-69, SCMS-631; 03 §6.3, §11).
    pub const CAMP_122_CERT_SHUFFLE: BatchPolicy = BatchPolicy {
        max_items: 1_000,
        max_delay: Duration::from_secs(2 * 86_400),
    };
}

/// SplitMix64's finaliser: a bijective 64-bit mix.
const fn mix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A uniform draw in `[0, 1)` keyed by `(seed, entity, k, kind)` (53 bits): a pure
/// function of its key, so no other draw moves it and it moves no other (§0 rule 5 of
/// `wp16-24-build-plans.md`).
#[must_use]
pub fn keyed_uniform(seed: u64, entity: u64, k: u64, kind: u64) -> f64 {
    let mut h = mix64(seed ^ 0x6176_6169_6c61_6269); // "availabi"
    h = mix64(h ^ entity);
    h = mix64(h ^ k);
    h = mix64(h ^ kind);
    #[allow(clippy::cast_precision_loss)]
    let u = (h >> 11) as f64 / (1u64 << 53) as f64;
    u
}

/// The two-state availability model of an entity (06-node-models §4; SC-P12.1): it
/// alternates up and down, up times exponential with mean `mtbf`, down times exponential
/// with mean `mttr`. The long-run down share is `mttr / (mtbf + mttr)`.
///
/// ASSUMPTION (labelled, `constants.md`): every entity starts up at time 0 (the run
/// starts after the backend came up); the parameters are the scenario's, not measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvailabilityModel {
    /// Mean time between failures: the mean up time.
    pub mtbf: Duration,
    /// Mean time to repair: the mean down time.
    pub mttr: Duration,
}

/// Draw kinds of [`AvailabilityModel::outages`].
const UP_DRAW: u64 = 0x7570; // "up"
const DOWN_DRAW: u64 = 0x646f_776e; // "down"

impl AvailabilityModel {
    /// One exponential interval with mean `mean`: `-mean·ln(1 − u)`, rounded to whole
    /// nanoseconds and at least 1 ns.
    fn interval(mean: Duration, seed: u64, entity: u64, k: u64, kind: u64) -> u64 {
        let u = keyed_uniform(seed, entity, k, kind);
        #[allow(clippy::cast_precision_loss)]
        let x = -(mean.as_nanos() as f64) * (1.0 - u).ln();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ns = x.round() as u64;
        ns.max(1)
    }

    /// The long-run share of time down, `mttr / (mtbf + mttr)`.
    #[must_use]
    pub fn down_share(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let (b, r) = (self.mtbf.as_nanos() as f64, self.mttr.as_nanos() as f64);
        if b + r == 0.0 { 0.0 } else { r / (b + r) }
    }

    /// Entity `entity`'s down intervals `[from, until)` that start before `horizon`,
    /// drawn under `seed`: the k-th up time is `-mtbf·ln(1 − keyed_uniform(seed, entity,
    /// k, up))` and the k-th down time the same with `mttr` and `down`. The last one may
    /// end after `horizon`. None when `mttr` is zero; `mtbf` zero is treated as 1 ns.
    #[must_use]
    pub fn outages(&self, seed: u64, entity: u64, horizon: SimTime) -> Vec<(SimTime, SimTime)> {
        let mut out = Vec::new();
        if self.mttr.as_nanos() == 0 {
            return out;
        }
        let mut t: SimTime = 0;
        let mut k = 0u64;
        loop {
            t = t.saturating_add(Self::interval(self.mtbf, seed, entity, k, UP_DRAW));
            if t >= horizon {
                return out;
            }
            let end = t.saturating_add(Self::interval(self.mttr, seed, entity, k, DOWN_DRAW));
            out.push((t, end));
            t = end;
            k += 1;
        }
    }
}

/// How an entity serves requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceModelSpec {
    /// Servers, the `c` of the M/M/c of 06-node-models §4.
    pub servers: u32,
    /// Per-request overhead on top of the cryptographic cost.
    pub fixed_overhead: Duration,
    /// The batching window, where the entity has one.
    pub batching: Option<BatchPolicy>,
    /// Availability: `None` (always up, the default) or the two-state model whose down
    /// intervals [`crate::Kernel::availability_faults`] draws (SC-P12.1).
    pub availability: Option<AvailabilityModel>,
    /// Requests that may wait beyond the busy servers; one more arriving is turned away
    /// as busy (`capacity.<role>.queue_limit`, 08-settings-ux-and-gaps §6). `None`:
    /// unlimited, the default.
    pub queue_limit: Option<u32>,
}

impl ServiceModelSpec {
    /// A `c`-server entity with the given per-request overhead and no batching.
    pub const fn new(servers: u32, fixed_overhead: Duration) -> ServiceModelSpec {
        ServiceModelSpec {
            servers,
            fixed_overhead,
            batching: None,
            availability: None,
            queue_limit: None,
        }
    }

    /// The same, with at most `limit` requests waiting beyond the busy servers.
    #[must_use]
    pub const fn limited(self, limit: Option<u32>) -> ServiceModelSpec {
        ServiceModelSpec {
            queue_limit: limit,
            ..self
        }
    }

    /// The same, with random outages under `model` (SC-P12.1).
    #[must_use]
    pub const fn available(self, model: Option<AvailabilityModel>) -> ServiceModelSpec {
        ServiceModelSpec {
            availability: model,
            ..self
        }
    }

    /// The same, with a batching window.
    #[must_use]
    pub const fn batched(self, policy: BatchPolicy) -> ServiceModelSpec {
        ServiceModelSpec {
            batching: Some(policy),
            ..self
        }
    }
}

/// A live `c`-server queue.
///
/// Holds one "free at" instant per server. A request arriving at `t` takes the server
/// that becomes free earliest; ties go to the lowest index, which is an explicit total
/// order rather than whatever order a map happened to iterate in.
#[derive(Debug, Clone)]
pub struct ServiceQueue {
    free_at: Vec<SimTime>,
    fixed_overhead: Duration,
    busy_ns: u64,
    served: u64,
    waited_ns: u64,
    /// When each admitted request that may still be in the system finishes, oldest first.
    /// Arrivals reach one entity in time order, so everything finished by the latest
    /// arrival can be forgotten; what is left is the queue.
    finishing: std::collections::VecDeque<SimTime>,
    servers: u32,
    /// Requests that may wait beyond the busy servers (`None`: unlimited).
    limit: Option<u32>,
    /// Requests turned away because the queue was at its limit.
    refused: u64,
    /// Its availability model, if it fails at random.
    availability: Option<AvailabilityModel>,
}

impl ServiceQueue {
    /// A queue for `spec`.
    pub fn new(spec: &ServiceModelSpec) -> ServiceQueue {
        ServiceQueue {
            free_at: vec![0; spec.servers.max(1) as usize],
            fixed_overhead: spec.fixed_overhead,
            busy_ns: 0,
            served: 0,
            waited_ns: 0,
            finishing: std::collections::VecDeque::new(),
            servers: spec.servers.max(1),
            limit: spec.queue_limit,
            refused: 0,
            availability: spec.availability,
        }
    }

    /// The model it was built from (without the batching, which its entity applies).
    pub const fn spec(&self) -> ServiceModelSpec {
        ServiceModelSpec::new(self.servers, self.fixed_overhead)
            .limited(self.limit)
            .available(self.availability)
    }

    /// Its availability model, if it fails at random.
    pub const fn availability(&self) -> Option<AvailabilityModel> {
        self.availability
    }

    /// Whether a request arriving at `t` finds the queue full: every server busy and
    /// `queue_limit` requests already waiting. Always false without a limit.
    pub fn is_full_at(&self, t: SimTime) -> bool {
        self.limit
            .is_some_and(|l| self.depth_at(t) >= self.servers as usize + l as usize)
    }

    /// Counts a request turned away as busy.
    pub const fn refuse(&mut self) {
        self.refused += 1;
    }

    /// How many requests it turned away as busy.
    pub const fn refused(&self) -> u64 {
        self.refused
    }

    /// How many requests are in service or waiting at `t`: admitted and not yet finished.
    pub fn depth_at(&self, t: SimTime) -> usize {
        self.finishing.iter().filter(|&&done| done > t).count()
    }

    /// The `c` of the M/M/c.
    pub const fn servers(&self) -> u32 {
        self.servers
    }

    /// Admits a request that arrived at `arrival` and needs `work` of cryptographic time.
    ///
    /// Returns the instant the entity finishes it — which is when its outgoing messages
    /// leave and when its stages are stamped.
    pub fn admit(&mut self, arrival: SimTime, work: Duration) -> SimTime {
        let (index, free) = self
            .free_at
            .iter()
            .copied()
            .enumerate()
            .min_by_key(|&(i, t)| (t, i))
            .unwrap_or((0, arrival));
        let start = free.max(arrival);
        let service = Duration::from_nanos(
            work.as_nanos()
                .saturating_add(self.fixed_overhead.as_nanos()),
        );
        let done = service.after(start);
        self.free_at[index] = done;
        self.busy_ns = self.busy_ns.saturating_add(service.as_nanos());
        self.waited_ns = self.waited_ns.saturating_add(start - arrival);
        self.served += 1;
        while self.finishing.front().is_some_and(|&t| t <= arrival) {
            self.finishing.pop_front();
        }
        // Kept sorted: with several servers a later request can finish earlier.
        let at = self.finishing.partition_point(|&t| t <= done);
        self.finishing.insert(at, done);
        done
    }

    /// The latest instant any of this entity's servers becomes free.
    ///
    /// What "this entity has finished everything it was given" means for a `c`-server
    /// queue: the last server to finish, not the first.
    pub fn busy_until(&self) -> SimTime {
        self.free_at.iter().copied().max().unwrap_or(0)
    }

    /// How many requests this entity has served.
    pub const fn served(&self) -> u64 {
        self.served
    }

    /// Total time its servers were busy.
    pub const fn busy(&self) -> Duration {
        Duration::from_nanos(self.busy_ns)
    }

    /// Total time requests spent waiting for a free server.
    pub const fn waiting(&self) -> Duration {
        Duration::from_nanos(self.waited_ns)
    }
}
