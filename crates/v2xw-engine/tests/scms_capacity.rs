//! Server capacity as settings (docs/design/scms-real/08-settings-ux-and-gaps.md §6, pieces
//! C2a-C2b): a queue limit turns a burst's surplus requests away as busy, each turned-away
//! device retries under SCMS-522/523, the workers serve as an M/M/c queue does, one
//! provider's own capacity changes only its servers, and the settings that contradict
//! each other are refused.

use serde_json::{Value, json};
use v2xw_core::ids::NodeId;
use v2xw_engine::Scenario;
use v2xw_engine::phase2::{CAMP_SCMS, deployment_of};
use v2xw_engine::scms_topology::apply_capacity;
use v2xw_proto::ScmsParams;
use v2xw_proto::scms::params::{EE_RETRY_MAX_PER_HOUR, EE_RETRY_TIMEOUT_S, EE_RETRY_WINDOW_S};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::StageId;

fn scenario(deployment: Value) -> Scenario {
    let mut s = Scenario::minimal();
    s.security.protocol = Some(serde_json::from_value(json!({"id": CAMP_SCMS})).expect("a choice"));
    s.security.scms =
        Some(serde_json::from_value(json!({"deployment": deployment})).expect("the block parses"));
    s
}

fn errors(s: &Scenario) -> Vec<String> {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// Four times the RA's room (one worker and ten places): the burst's surplus has to be
/// turned away. Kept small, with the PDU layer off, so the run stays well under the
/// machine's memory cap.
const DEVICES: u32 = 44;
const S: u64 = 1_000_000_000;

/// A keyed uniform draw in [0, 1) (splitmix64 of the device index): each device's own
/// offset, so no draw takes a position in a shared stream.
fn keyed_uniform(k: u32) -> f64 {
    let mut z = u64::from(k).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// How long the burst lasts: 44 requests in 20 ms is over twenty times what one worker at
/// about 1 ms a request can take.
const BURST_NS: f64 = 20e6;

/// The home run of the scenario's deployment, its capacity applied, after `DEVICES`
/// devices enrolled and then all asked for pseudonyms within `BURST_NS`. Each device asks
/// at its own keyed offset: a burst at one instant with identical links has every device
/// retry in lockstep (SCMS-522 has no jitter) and reach the RA in the same order every
/// time, so the same devices are turned away for ever, which no real access network does.
fn burst(deployment: Value) -> (ScmsRun, Vec<(NodeId, v2xw_proto::stage::FlowRun)>) {
    let s = scenario(deployment);
    let d = deployment_of(&s, true).expect("builds");
    let mut params = ScmsParams::default().quick();
    params.pdu.enabled = false;
    let run = ScmsRun::new_deployed(params, 0, d).expect("builds");
    let mut runs = vec![run];
    apply_capacity(&s, &mut runs).expect("applies");
    let mut run = runs.pop().expect("the home run");
    let devices: Vec<NodeId> = (0..DEVICES).map(|k| NodeId::new(5_000_500 + k)).collect();
    for &dev in &devices {
        run.add_device(dev);
        run.enrol(dev);
    }
    run.run().expect("enrols");
    let t = run.kernel.now();
    let mut order: Vec<(u64, u32)> = (0..DEVICES)
        .map(|k| ((keyed_uniform(k) * BURST_NS) as u64, k))
        .collect();
    order.sort_unstable();
    let mut flows = Vec::new();
    for (offset, k) in order {
        let dev = devices[k as usize];
        flows.push((dev, run.provision_at(dev, t + offset, 0, 1, 2)));
    }
    run.run_until(t + 6 * 3_600 * S).expect("provisions");
    (run, flows)
}

/// Accept test 2: one RA worker with `queue_limit: 10` under a burst of 44 provisioning
/// requests in 20 ms (the plan's 500 is past this machine's memory cap) turns some away
/// (`refused_busy > 0`; none without the limit). Each device asks again no sooner than
/// `ee_retry_timeout_s` (10 s) after its last request, never more than
/// `ee_retry_max_per_hour` times in an hour, and every device ends provisioned. Fault:
/// the limit is never consulted, so nothing is refused.
#[test]
fn a_full_queue_turns_requests_away_and_devices_retry() {
    let limited = json!({"capacity": {"ra": {"workers": 1, "queue_limit": 10}}});
    let (run, flows) = burst(limited);
    let ra = run.state.nodes.ra;
    let q = run.kernel.queue(ra).expect("the RA is hosted");
    assert_eq!((q.spec().servers, q.spec().queue_limit), (1, Some(10)));
    let refused = q.refused();
    let retries: Vec<u32> = flows
        .iter()
        .map(|(dev, _)| run.state.devices[dev].ee_retries)
        .collect();
    let diag = format!(
        "refused {refused}, retries {retries:?}, RA served {} waited {} ms",
        q.served(),
        q.waiting().as_nanos() / 1_000_000
    );
    assert!(refused > 0, "the burst refused none: {diag}");
    let view = v2xw_proto::view::queue_of(&run.kernel, ra, run.kernel.now()).expect("a view");
    assert_eq!(view.refused_busy, refused);
    // A turned-away request is one HTTP request of a session; the device repeats the
    // whole session after its timeout (SCMS-522).
    assert!(retries.iter().any(|&r| r > 0), "{diag}");
    let window = EE_RETRY_WINDOW_S * S;
    for (dev, flow) in &flows {
        let mut asked: Vec<u64> = run
            .kernel
            .stages
            .run(*flow)
            .iter()
            .filter(|st| st.stage == StageId::Requested)
            .map(|st| st.t)
            .collect();
        asked.sort_unstable();
        for w in asked.windows(2) {
            assert!(
                w[1] - w[0] >= EE_RETRY_TIMEOUT_S * S,
                "{dev:?} asked again after {} ns: {asked:?}",
                w[1] - w[0]
            );
        }
        for (k, &t) in asked.iter().enumerate() {
            let in_hour = asked[k..].iter().take_while(|&&u| u < t + window).count();
            assert!(
                in_hour <= 1 + EE_RETRY_MAX_PER_HOUR as usize,
                "{dev:?} asked {in_hour} times within an hour: {asked:?}"
            );
        }
        assert!(
            !run.state.devices[dev].credentials.is_empty(),
            "{dev:?} never got its pseudonyms: {diag}"
        );
    }

    let (base, base_flows) = burst(json!({"capacity": {"ra": {"workers": 1}}}));
    assert_eq!(
        base.kernel.queue(base.state.nodes.ra).map(|q| q.refused()),
        Some(0)
    );
    assert!(
        base_flows
            .iter()
            .all(|(dev, _)| base.state.devices[dev].ee_retries == 0)
    );
}

/// Accept test 4: `actors.backend.entities.ra.service_model` together with
/// `capacity.ra.service` is refused naming both; so are `fifo-1` with more than one
/// worker and any capacity under the abstract tier.
#[test]
fn contradicting_capacity_settings_are_refused() {
    let mut s = scenario(json!({"capacity": {"ra": {"service": "fifo-1"}}}));
    s.actors.backend.entities.insert(
        "ra".to_string(),
        serde_json::from_value(json!({"service_model": "service/mm1"})).expect("an entity"),
    );
    let got = errors(&s);
    assert!(
        got.iter().any(
            |m| m.starts_with("security.scms.deployment.capacity.ra.service")
                && m.contains("actors.backend.entities.ra.service_model")
        ),
        "{got:#?}"
    );
    let got = errors(&scenario(
        json!({"capacity": {"ra": {"service": "fifo-1", "workers": 4}}}),
    ));
    assert!(
        got.iter()
            .any(|m| m.starts_with("security.scms.deployment.capacity.ra.workers")),
        "{got:#?}"
    );
    let mut s = scenario(json!({"capacity": {"ra": {"workers": 8}}}));
    s.nodes.backend_tier = v2xw_core::card::Tier::Abstract;
    let got = errors(&s);
    assert!(
        got.iter()
            .any(|m| m.starts_with("security.scms.deployment.capacity") && m.contains("abstract")),
        "{got:#?}"
    );
    // Each alone loads.
    assert!(
        errors(&scenario(
            json!({"capacity": {"ra": {"service": "fifo-1"}}})
        ))
        .is_empty()
    );
    assert!(
        errors(&scenario(
            json!({"capacity": {"ra": {"workers": 8, "queue_limit": 100, "overhead_ms": 5.0}}})
        ))
        .is_empty()
    );
}

/// Provider-a's run and provider-b's, both under the scenario's capacity.
fn provider_runs(deployment: Value) -> Vec<ScmsRun> {
    let s = scenario(deployment);
    let d = deployment_of(&s, true).expect("builds");
    let mut params = ScmsParams::default().quick();
    params.pdu.enabled = false;
    let home = ScmsRun::new_deployed(params, 0, d).expect("builds");
    let peer = ScmsRun::new_peer(&home, 1).expect("provider-b's run builds");
    let mut runs = vec![home, peer];
    apply_capacity(&s, &mut runs).expect("applies");
    runs
}

/// The Erlang-C mean sojourn of an M/M/c queue: `c` servers, arrivals at `lambda` and
/// service at `mu` per second (Kleinrock, *Queueing Systems* I, §3.5).
fn erlang_c_sojourn(c: u32, lambda: f64, mu: f64) -> f64 {
    let a = lambda / mu;
    let mut term = 1.0;
    let mut sum = 0.0;
    for k in 0..c {
        if k > 0 {
            term *= a / f64::from(k);
        }
        sum += term;
    }
    let last = term * a / f64::from(c) * f64::from(c) / (f64::from(c) - a);
    let wait_prob = last / (sum + last);
    wait_prob / (f64::from(c) * mu - lambda) + 1.0 / mu
}

/// Accept test 1: provider-b's RA with `workers: 1` and with `workers: 4` (overhead 0, so
/// the service time is the work alone), fed one Poisson stream of 400,000 requests with
/// exponential work (mean 10 ms, 50 requests a second): each mean sojourn is within 5 %
/// of the Erlang-C figure for the measured arrival and service rates. Fault: `workers` is
/// parsed and ignored.
#[test]
fn the_workers_serve_as_an_m_m_c_queue() {
    const N: u32 = 400_000;
    let (lambda, mean_work_s) = (50.0, 0.010);
    let mut arrivals = Vec::with_capacity(N as usize);
    let mut works = Vec::with_capacity(N as usize);
    let mut t = 0.0;
    for k in 0..N {
        t += -(1.0 - keyed_uniform(2 * k)).ln() / lambda;
        arrivals.push((t * 1e9) as u64);
        works.push((-(1.0 - keyed_uniform(2 * k + 1)).ln() * mean_work_s * 1e9) as u64);
    }
    let span_s = (arrivals[N as usize - 1] - arrivals[0]) as f64 / 1e9;
    let lambda_hat = f64::from(N - 1) / span_s;
    let mu_hat = 1e9 * f64::from(N) / works.iter().map(|&w| w as f64).sum::<f64>();
    for c in [1u32, 4] {
        let runs = provider_runs(json!({"providers": [
            {"name": "provider-b", "capacity": {"ra": {"workers": c, "overhead_ms": 0.0}}}
        ]}));
        let ra = runs[1].state.nodes.ra;
        let spec = runs[1].kernel.queue(ra).expect("hosted").spec();
        let mut q = v2xw_proto::ServiceQueue::new(&spec);
        for (&at, &w) in arrivals.iter().zip(&works) {
            q.admit(at, v2xw_core::time::Duration::from_nanos(w));
        }
        let sojourn = (q.waiting().as_nanos() + q.busy().as_nanos()) as f64 / 1e9 / f64::from(N);
        let expected = erlang_c_sojourn(c, lambda_hat, mu_hat);
        assert!(
            (sojourn / expected - 1.0).abs() < 0.05,
            "c = {c}: mean sojourn {sojourn:.5} s against Erlang-C {expected:.5} s \
             (servers {}, lambda {lambda_hat:.3}/s, mu {mu_hat:.3}/s)",
            spec.servers
        );
    }
}

/// Accept test 3: provider-b's `capacity.crlg.overhead_ms: 5000` slows only provider-b's
/// CRL Generator, over the deployment's `workers: 2` for every provider: a CRL job at
/// provider-b's takes 5 s longer, provider-a's keeps today's 1 ms. Fault: the override is
/// applied to the home provider.
#[test]
fn one_providers_capacity_changes_only_its_servers() {
    let runs = provider_runs(json!({
        "capacity": {"crlg": {"workers": 2}},
        "providers": [{"name": "provider-b", "capacity": {"crlg": {"overhead_ms": 5000.0}}}]
    }));
    let spec = |k: usize| {
        let crlg = runs[k].state.nodes.crlg;
        runs[k].kernel.queue(crlg).expect("hosted").spec()
    };
    let (a, b) = (spec(0), spec(1));
    assert_eq!(
        (a.servers, b.servers),
        (2, 2),
        "the deployment's block reaches both"
    );
    let ms = |d: v2xw_core::time::Duration| d.as_nanos() / 1_000_000;
    assert_eq!(
        (ms(a.fixed_overhead), ms(b.fixed_overhead)),
        (1, 5_000),
        "only provider-b's CRL Generator is slowed"
    );
    // One CRL job of no cryptographic work, at each.
    let job = |k: usize| {
        let crlg = runs[k].state.nodes.crlg;
        let mut q = runs[k].kernel.queue(crlg).expect("hosted").clone();
        q.admit(S, v2xw_core::time::Duration::ZERO) - S
    };
    assert_eq!((job(0), job(1)), (1_000_000, 5_000_000_000));
    // And a provider's own role it does not run is refused at build.
    let s = scenario(json!({"providers": [{"name": "provider-b",
                                           "capacity": {"lop": {"workers": 2}}}]}));
    let d = deployment_of(&s, true).expect("builds");
    let home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    let peer = ScmsRun::new_peer(&home, 1).expect("builds");
    let got = apply_capacity(&s, &mut [home, peer]);
    assert!(
        got.as_ref()
            .is_err_and(|e| e.to_string().contains("does not run")),
        "{got:?}"
    );
}
