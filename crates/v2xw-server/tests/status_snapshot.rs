//! `run.status` never pairs one run's generation with another run's descriptor.
//!
//! A run on a large map takes seconds to build, and `run.start` builds it with the engine
//! locked. `run.status` read the descriptor first, then waited on the engine lock for the
//! clock and the counts, then read the generation: a status asked for during the build
//! answered the new run's generation with the old run's length. The Studio's owner-flow
//! test, waiting for "the run after this generation", read `t_end_ns` 300 s on a run it had
//! just set to 20 s.
//!
//! Here an engine whose start takes 400 ms and changes the run's length stands in for the
//! large map, and a status is asked for in the middle of that start.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use v2xw_core::time::SimTime;
use v2xw_server::engine::{Control, ControlOutcome, Query};
use v2xw_server::rpc::{self, Context};
use v2xw_server::{
    Engine, Result, Run, RunDescriptor, RunState, StepOutput, StubEngine, StubOptions,
};
use v2xw_world::WorldPayload;

/// The stub engine, with a slow start that gives the next run another length.
#[derive(Debug)]
struct SlowStart {
    inner: StubEngine,
    descriptor: RunDescriptor,
}

impl Engine for SlowStart {
    fn descriptor(&self) -> &RunDescriptor {
        &self.descriptor
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
        if matches!(command, Control::Start { .. }) {
            std::thread::sleep(Duration::from_millis(400));
            self.descriptor.duration = 20_000_000_000;
        }
        self.inner.control(command)
    }
    fn step(&mut self) -> Result<Option<StepOutput>> {
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
        .unwrap_or_else(|e| panic!("{method} failed: {e}"))
        .result
}

#[test]
fn a_status_during_a_start_is_one_run_or_the_other() {
    let inner = StubEngine::new(StubOptions {
        paused: true,
        duration_s: 300,
        ..StubOptions::default()
    })
    .expect("stub");
    let world_json = inner.world_json().unwrap_or_else(|| "{}".to_string());
    let descriptor = inner.descriptor().clone();
    let run = Run::new(Box::new(SlowStart { inner, descriptor }), world_json).expect("run");
    let before = call(&run, "run.status", json!({}));
    assert_eq!(before["t_end_ns"], json!(300_000_000_000_u64));
    let g0 = before["generation"].as_u64().expect("generation");

    std::thread::scope(|s| {
        let starter = s.spawn(|| call(&run, "run.start", json!({"paused": true})));
        // Inside the start's 400 ms: the status waits on the engine lock for the clock.
        std::thread::sleep(Duration::from_millis(100));
        let during = call(&run, "run.status", json!({}));
        starter.join().expect("start");
        let g = during["generation"].as_u64().expect("generation");
        let t_end = during["t_end_ns"].as_u64().expect("t_end_ns");
        let expected = if g == g0 {
            300_000_000_000
        } else {
            20_000_000_000
        };
        assert_eq!(
            t_end, expected,
            "generation {g} (started from {g0}) answered with t_end_ns {t_end}"
        );
    });
    let after = call(&run, "run.status", json!({}));
    assert_eq!(after["generation"].as_u64(), Some(g0 + 1));
    assert_eq!(after["t_end_ns"], json!(20_000_000_000_u64));
}
