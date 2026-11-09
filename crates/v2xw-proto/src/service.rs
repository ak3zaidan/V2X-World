//! Backend service models: the reason provisioning costs something.
//!
//! Plainly: nothing in a backend entity is instantaneous. An
//! entity here is a `c`-server queue: a request occupies one server for a service time
//! composed from the cryptographic operations it performs plus a fixed per-request
//! overhead, and a request that arrives while every server is busy waits. That is the
//! `medium` tier's structure with a deterministic service time.
//!
//! The `high` tier's variable service time is [`ServiceTimeModel::LogNormal`]:
//! each request's `work + fixed_overhead` is multiplied by a log-normal draw with median 1
//! and coefficient of variation `cv`, so the median service time is the deterministic one.
//! Off by default ([`ServiceTimeModel::Deterministic`]), and its `cv` is a labelled
//! ASSUMPTION for the sensitivity sweeps.
//!
//! The `high` tier's availability model is [`AvailabilityModel`]: a two-state
//! alternating renewal process per entity, off by default. Its
//! parameters are `todo-calibrate` in the design, so every value is a labelled
//! ASSUMPTION (`docs/reference/scms-constants.md`) for the sensitivity sweeps.
//! Its down intervals are applied as `offline` faults ([`crate::fault`]), exactly as a
//! scenario's `actors.backend.failures[]` window is.
//!
//! Determinism: no RNG stream, and no float in the deterministic queue. Service times are
//! sums of the integer nanosecond durations of `v2xw_sec::primitive`'s cost tables, and
//! the server a request lands on is the lowest-indexed one that is free earliest, chosen
//! by an explicit total order. The availability and service-time draws are keyed hashes
//! ([`keyed_uniform`]): each interval or multiplier is a pure function of `(seed, entity,
//! k, kind)`, takes no position in any stream, and uses only `v2xw_core::math`'s
//! transcendentals, which give the same bits on every platform.

use v2xw_core::time::{Duration, SimTime};

/// The upper bounds, in milliseconds, of the fixed buckets of [`AckDelays`]. A delay
/// above the last bound goes in the overflow bucket. Integer bounds, so the histogram is
/// the same on every platform.
pub const ACK_BUCKETS_MS: [u64; 16] = [
    1, 2, 5, 10, 20, 50, 100, 200, 500, 1_000, 2_000, 5_000, 10_000, 30_000, 60_000, 300_000,
];

/// The \[CAMP122\] §5.2.8.3 SCMS-521 bound: a request is acknowledged within 1 s.
pub const ACK_SLA_NS: u64 = 1_000_000_000;

/// How long an entity took to answer the requests it received from outside its provider:
/// `done - arrival` per request, where `arrival` is when the request reached
/// the entity and `done` when its answer left, so the wait for a free server counts.
///
/// \[CAMP122\] §5.2.8.3 SCMS-521 asks the RA to acknowledge a request within 1 s. No
/// deployed RA figure exists, so nothing is refused for missing it: the delays are
/// measured, `over_1s` counts the misses and [`AckDelays::p99_ms`] reads the tail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct AckDelays {
    /// Requests per bucket of [`ACK_BUCKETS_MS`]; the last entry is the overflow bucket.
    pub buckets: [u64; ACK_BUCKETS_MS.len() + 1],
    /// Requests measured.
    pub count: u64,
    /// Requests answered later than [`ACK_SLA_NS`].
    pub over_1s: u64,
    /// The longest delay measured, in nanoseconds.
    pub max_ns: u64,
}

impl AckDelays {
    /// Records one request answered `ns` after it arrived.
    pub fn record(&mut self, ns: u64) {
        let ms_ceil = ns.div_ceil(1_000_000);
        let b = ACK_BUCKETS_MS.partition_point(|&ub| ub < ms_ceil);
        self.buckets[b] += 1;
        self.count += 1;
        if ns > ACK_SLA_NS {
            self.over_1s += 1;
        }
        self.max_ns = self.max_ns.max(ns);
    }

    /// The 99th-percentile delay in milliseconds, as the upper bound of the bucket the
    /// 99th percentile falls in (the longest delay, rounded up to a millisecond, when that
    /// is the overflow bucket or below the bound). Zero when nothing was measured.
    pub fn p99_ms(&self) -> u64 {
        if self.count == 0 {
            return 0;
        }
        // The rank of the 99th percentile, 1-based: ceil(0.99 * count), in integers.
        let rank = (self.count * 99).div_ceil(100);
        let max_ms = self.max_ns.div_ceil(1_000_000);
        let mut seen = 0;
        for (i, &n) in self.buckets.iter().enumerate() {
            seen += n;
            if seen >= rank {
                return ACK_BUCKETS_MS.get(i).map_or(max_ms, |&ub| ub.min(max_ms));
            }
        }
        max_ms
    }
}

/// The batching windows: release when either bound is reached.
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
    /// shuffle.]
    pub const CAMP_SHUFFLE: BatchPolicy = BatchPolicy {
        max_items: 10_000,
        max_delay: Duration::from_secs(86_400),
    };

    /// The CAMP 1.2.2 certificate-request shuffle, the engine's default: release at
    /// `shuffle_threshold` = 1,000 requests ("minimum number for full privacy mode") or
    /// when the oldest request reaches `max_cert_request_age` = 2 days, whichever comes
    /// first (\[CAMP122\] §5.2.15.5 Tables 67-69, SCMS-631, §11).
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
/// function of its key, so no other draw moves it and it moves no other.
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

/// The two-state availability model of an entity: it
/// alternates up and down, up times exponential with mean `mtbf`, down times exponential
/// with mean `mttr`. The long-run down share is `mttr / (mtbf + mttr)`.
///
/// ASSUMPTION (labelled, `scms-constants.md`): every entity starts up at time 0 (the run
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
        let x = -(mean.as_nanos() as f64) * v2xw_core::math::ln(1.0 - u);
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

/// How an entity's service times vary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceTimeModel {
    /// The cost table's time, exactly: `work + fixed_overhead` (the default).
    #[default]
    Deterministic,
    /// `work + fixed_overhead` times a log-normal multiplier with median 1 and coefficient
    /// of variation `cv_milli / 1000`: `μ = 0`, `σ = sqrt(ln(1 + cv²))`. ASSUMPTION
    /// (labelled, `scms-constants.md`): the shape and `cv` are not measured.
    LogNormal {
        /// The coefficient of variation, in thousandths (250 is 0.25).
        cv_milli: u32,
    },
}

/// Draw kinds of [`ServiceTimeModel::multiplier`]: the two uniforms of a Box-Muller
/// normal.
const SERVICE_RADIUS_DRAW: u64 = 0x0073_7663_2d72; // "svc-r"
const SERVICE_ANGLE_DRAW: u64 = 0x0073_7663_2d61; // "svc-a"

impl ServiceTimeModel {
    /// The log-normal's `σ = sqrt(ln(1 + cv²))`; 0 when deterministic.
    #[must_use]
    pub fn sigma(&self) -> f64 {
        match *self {
            ServiceTimeModel::Deterministic => 0.0,
            ServiceTimeModel::LogNormal { cv_milli } => {
                let cv = f64::from(cv_milli) / 1000.0;
                v2xw_core::math::sqrt(v2xw_core::math::ln(1.0 + cv * cv))
            }
        }
    }

    /// The multiplier of the `n`-th request (0-based) entity `entity` serves under `seed`:
    /// `exp(σ·z)`, with `z` a standard normal from two keyed uniforms (Box-Muller,
    /// `z = sqrt(−2·ln(1 − u₁))·cos(2π·u₂)`). Exactly 1 when deterministic.
    #[must_use]
    pub fn multiplier(&self, seed: u64, entity: u64, n: u64) -> f64 {
        let sigma = self.sigma();
        if sigma == 0.0 {
            return 1.0;
        }
        let u1 = keyed_uniform(seed, entity, n, SERVICE_RADIUS_DRAW);
        let u2 = keyed_uniform(seed, entity, n, SERVICE_ANGLE_DRAW);
        let r = v2xw_core::math::sqrt(-2.0 * v2xw_core::math::ln(1.0 - u1));
        let z = r * v2xw_core::math::cos(2.0 * core::f64::consts::PI * u2);
        v2xw_core::math::exp(sigma * z)
    }

    /// `base` scaled by [`ServiceTimeModel::multiplier`], rounded to whole nanoseconds.
    /// The deterministic model returns `base` untouched, with no float on its path.
    #[must_use]
    pub fn apply(&self, base: Duration, seed: u64, entity: u64, n: u64) -> Duration {
        if matches!(self, ServiceTimeModel::Deterministic) {
            return base;
        }
        #[allow(clippy::cast_precision_loss)]
        let x = base.as_nanos() as f64 * self.multiplier(seed, entity, n);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let ns = if x >= u64::MAX as f64 {
            u64::MAX
        } else {
            x.round() as u64
        };
        Duration::from_nanos(ns)
    }
}

/// How an entity serves requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceModelSpec {
    /// Servers, the `c` of the M/M/c backend queue.
    pub servers: u32,
    /// Per-request overhead on top of the cryptographic cost.
    pub fixed_overhead: Duration,
    /// The batching window, where the entity has one.
    pub batching: Option<BatchPolicy>,
    /// Availability: `None` (always up, the default) or the two-state model whose down
    /// intervals [`crate::Kernel::availability_windows`] draws.
    pub availability: Option<AvailabilityModel>,
    /// Requests that may wait beyond the busy servers; one more arriving is turned away
    /// as busy (`capacity.<role>.queue_limit`). `None`:
    /// unlimited, the default.
    pub queue_limit: Option<u32>,
    /// How its service times vary: [`ServiceTimeModel::Deterministic`] (the default) or
    /// the log-normal multiplier.
    pub service_time: ServiceTimeModel,
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
            service_time: ServiceTimeModel::Deterministic,
        }
    }

    /// The same, with service times varied under `model`.
    #[must_use]
    pub const fn varied(self, model: ServiceTimeModel) -> ServiceModelSpec {
        ServiceModelSpec {
            service_time: model,
            ..self
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

    /// The same, with random outages under `model`.
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
    /// How its service times vary.
    service_time: ServiceTimeModel,
    /// The run seed its service-time draws are keyed by ([`ServiceQueue::keyed`]).
    seed: u64,
    /// The entity its service-time draws are keyed by: its node index once hosted.
    entity: u64,
    /// How long it took to answer the requests that came from outside its provider
    /// ([`ServiceQueue::admit_request`]).
    acks: AckDelays,
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
            service_time: spec.service_time,
            seed: 0,
            entity: 0,
            acks: AckDelays::default(),
        }
    }

    /// The same queue with its service-time draws keyed by `(seed, entity)`: the `n`-th
    /// request it serves gets [`ServiceTimeModel::multiplier`]`(seed, entity, n)`, whatever
    /// any other entity serves. [`crate::Kernel::host`] keys each queue by its node index
    /// and [`crate::Kernel::seed_service_times`] sets the run seed.
    #[must_use]
    pub const fn keyed(mut self, seed: u64, entity: u64) -> ServiceQueue {
        self.seed = seed;
        self.entity = entity;
        self
    }

    /// Sets the run seed its service-time draws are keyed by.
    pub const fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
    }

    /// How its service times vary.
    pub const fn service_time(&self) -> ServiceTimeModel {
        self.service_time
    }

    /// The model it was built from (without the batching, which its entity applies).
    pub const fn spec(&self) -> ServiceModelSpec {
        ServiceModelSpec::new(self.servers, self.fixed_overhead)
            .limited(self.limit)
            .available(self.availability)
            .varied(self.service_time)
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
    /// Its service time is `work + fixed_overhead`, scaled under a log-normal
    /// [`ServiceTimeModel`] by the multiplier keyed by `(seed, entity, served)`.
    ///
    /// Returns the instant the entity finishes it — which is when its outgoing messages
    /// leave and when its stages are stamped.
    pub fn admit(&mut self, arrival: SimTime, work: Duration) -> SimTime {
        self.admit_at(arrival, work).1
    }

    /// [`ServiceQueue::admit`] for a request from outside the entity's provider, which is
    /// owed an acknowledgement (\[CAMP122\] SCMS-521): also records `done - arrival`, the
    /// wait for a server included, in [`ServiceQueue::acks`].
    pub fn admit_request(&mut self, arrival: SimTime, work: Duration) -> SimTime {
        let (_start, done) = self.admit_at(arrival, work);
        self.acks.record(done - arrival);
        done
    }

    /// How long it took to answer the requests [`ServiceQueue::admit_request`] admitted.
    pub const fn acks(&self) -> &AckDelays {
        &self.acks
    }

    /// Admits a request: returns when a server took it up and when it finished.
    fn admit_at(&mut self, arrival: SimTime, work: Duration) -> (SimTime, SimTime) {
        let (index, free) = self
            .free_at
            .iter()
            .copied()
            .enumerate()
            .min_by_key(|&(i, t)| (t, i))
            .unwrap_or((0, arrival));
        let start = free.max(arrival);
        let service = self.service_time.apply(
            Duration::from_nanos(
                work.as_nanos()
                    .saturating_add(self.fixed_overhead.as_nanos()),
            ),
            self.seed,
            self.entity,
            self.served,
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
        (start, done)
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
