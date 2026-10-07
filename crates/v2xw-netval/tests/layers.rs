//! Every validation check, as a test: the honest run passes and the faulted run fails.
//!
//! One test per layer, so `cargo test -p v2xw-netval --test layers physical` runs one
//! layer. The whole-run checks of layer 5 are `#[ignore]`d by default — they run the engine
//! on every shipped scenario — and run with `-- --ignored` or through the report binary.

use v2xw_netval::{Cost, Layer, Status, of_layer, run};

fn layer(l: Layer, include_slow: bool) {
    let mut problems = Vec::new();
    for c in of_layer(l) {
        if c.cost == Cost::Slow && !include_slow {
            continue;
        }
        let v = run(&c);
        println!("{} {} — {} | faulted: {:?} {}", c.id, v.label(), v.honest.measured, v.faulted.status, v.faulted.measured);
        if v.honest.status == Status::Skip {
            println!("  skipped: {}", v.honest.measured);
            continue;
        }
        if !v.valid() {
            problems.push(format!("{} {}: {} (faulted: {})", c.id, v.label(), v.honest.measured, v.faulted.measured));
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn physical() {
    layer(Layer::Physical, false);
}

#[test]
fn access() {
    layer(Layer::Access, false);
}

#[test]
fn network() {
    layer(Layer::Network, false);
}

#[test]
fn security() {
    layer(Layer::Security, false);
}

#[test]
fn end_to_end_fast() {
    layer(Layer::EndToEnd, false);
}

#[test]
#[ignore = "runs the engine on every shipped scenario; the report binary runs it too"]
fn end_to_end_whole_runs() {
    layer(Layer::EndToEnd, true);
}
