//! `run.step` and `Run::tick_wait` count steps, not wall time.
//!
//! The live engine waits at most 60 ms for the kernel's next step (a transport bound, so the
//! run lock is not held while the kernel computes) and answers `Ok(None)` when the step is not
//! ready yet. `Run::advance`, the body of `run.step`, read that `None` as the end of the run:
//! `run.step {count: 5}` on a dense scenario on a loaded machine answered `stepped: 2`, and
//! the lifecycle test (`lifecycle.rs`, which asserted 20 ticks and a 5-step `run.step`) failed
//! about half its runs. The engine here is the stub with every step made "not ready" for its
//! first few attempts, which is exactly what a slow kernel looks like from the run, with no
//! wall clock in it: the result cannot depend on the machine's load.
//!
//! Fault: `Run::advance` calls `tick` instead of `tick_wait` -> `run_step_steps_its_count_
//! whatever_the_kernel_takes` red (`stepped: 0`).

use std::sync::Arc;

use serde_json::{Value, json};
use v2xw_core::time::SimTime;
use v2xw_server::engine::{Control, ControlOutcome, Query};
use v2xw_server::rpc::{self, Context};
use v2xw_server::{Engine, Result, Run, RunDescriptor, RunState, StepOutput};
use v2xw_server::{StubEngine, StubOptions};
use v2xw_world::WorldPayload;

/// The stub, but each step is "not ready" for its first `pending` attempts.
#[derive(Debug)]
struct SlowKernel {
    inner: StubEngine,
    pending: u32,
    misses: u32,
}

impl Engine for SlowKernel {
    fn descriptor(&self) -> &RunDescriptor {
        self.inner.descriptor()
    }
    fn world(&self) -> &Arc<WorldPayload> {
        self.inner.world()
    }
    fn state(&self) -> RunState {
        self.inner.state()
    }
    fn sim_time(&self) -> SimTime {
        self.inner.sim_time()
    }
    fn speed(&self) -> (f64, bool) {
        self.inner.speed()
    }
    fn counts(&self) -> (u32, u32) {
        self.inner.counts()
    }
    fn control(&mut self, command: Control) -> Result<ControlOutcome> {
        self.inner.control(command)
    }
    fn step(&mut self) -> Result<Option<StepOutput>> {
        if self.misses < self.pending {
            self.misses += 1;
            return Ok(None);
        }
        self.misses = 0;
        self.inner.step()
    }
    fn seek(&mut self, t: SimTime) -> Result<Vec<StepOutput>> {
        self.inner.seek(t)
    }
    fn seek_range(&self) -> (u64, u64) {
        self.inner.seek_range()
    }
    fn query(&mut self, query: &Query) -> Result<Value> {
        self.inner.query(query)
    }
}

fn slow_run(duration_s: u64, pending: u32) -> Arc<Run> {
    let inner = StubEngine::new(StubOptions {
        actors: 4,
        grid: 3,
        duration_s,
        paused: true,
        ..StubOptions::default()
    })
    .expect("stub");
    Run::new(
        Box::new(SlowKernel {
            inner,
            pending,
            misses: 0,
        }),
        "{}".to_string(),
    )
    .expect("run")
}

fn call(run: &Run, method: &str, params: Value) -> Value {
    let text = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let request = rpc::parse(&text).expect("parse");
    let mut ctx = Context {
        run,
        session: None,
        pending: None,
        received_at: None,
    };
    rpc::dispatch(&mut ctx, &request)
        .unwrap_or_else(|e| panic!("{method}: {e}"))
        .result
}

#[test]
fn run_step_steps_its_count_whatever_the_kernel_takes() {
    let run = slow_run(15, 3);
    let step_ns = run.descriptor().cadence.mobility_step.as_nanos();
    assert_eq!(run.state(), RunState::Paused);
    let before = call(&run, "run.status", json!({}))["t_ns"]
        .as_u64()
        .expect("t_ns");
    let stepped = call(&run, "run.step", json!({"count": 5}));
    assert_eq!(stepped["stepped"], 5, "{stepped}");
    assert_eq!(stepped["t_ns"].as_u64(), Some(before + 5 * step_ns));
    assert_eq!(
        run.state(),
        RunState::Paused,
        "a step leaves the run paused"
    );
}

#[test]
fn tick_wait_is_false_only_at_the_end_of_the_run() {
    let count = |run: &Run| {
        let mut produced = 0u64;
        while run.tick_wait().expect("tick") {
            produced += 1;
            assert!(produced < 1_000, "tick_wait never reported the end");
        }
        produced
    };
    // The same stream however slow the kernel is: as many steps as an engine whose every
    // step is ready at once, and not one fewer.
    let ready = count(&slow_run(2, 0));
    let run = slow_run(2, 2);
    assert!(
        ready >= 20,
        "a 2 s run at 100 ms has 20 steps or more: {ready}"
    );
    assert_eq!(count(&run), ready);
    assert!(run.ended());
    assert_eq!(run.state(), RunState::Finished);
    // A not-ready step is not the end; `tick` alone says false for it.
    let fresh = slow_run(2, 1);
    assert!(
        !fresh.tick().expect("tick"),
        "the first attempt is not ready"
    );
    assert!(
        !fresh.ended(),
        "a step not ready yet is not the end of the run"
    );
    assert!(fresh.tick().expect("tick"), "the second attempt is");
}
