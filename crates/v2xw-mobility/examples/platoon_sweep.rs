//! String stability, gap error and lane throughput of the cooperative car-following models
//! against packet loss and latency (`v2xw_mobility::carfollowing::platoon`).
//!
//! ```text
//! cargo run -p v2xw-mobility --example platoon_sweep            # markdown table
//! cargo run -p v2xw-mobility --example platoon_sweep -- --csv   # CSV
//! cargo run -p v2xw-mobility --example platoon_sweep -- --burst 4
//! cargo run -p v2xw-mobility --example platoon_sweep -- --abrupt   # also without graceful degradation
//! ```
//!
//! Every row is one eight-vehicle platoon at 25 m/s whose leader brakes at 2 m/s² for 3 s
//! and recovers at 1 m/s², with every member broadcasting at 10 Hz through a channel of the
//! row's loss and latency. `acc` is the radar-only baseline every CACC degrades to.

use v2xw_mobility::carfollowing::library::CfChoice;
use v2xw_mobility::carfollowing::platoon::{Channel, PlatoonSetup, sweep};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let csv = args.iter().any(|a| a == "--csv");
    let burst = args
        .iter()
        .position(|a| a == "--burst")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);
    let base = PlatoonSetup {
        channel: Channel {
            burst_len: burst,
            ..Channel::default()
        },
        ..PlatoonSetup::default()
    };
    // `--abrupt` also runs every CACC with the abrupt switch to ACC (no graceful ramp).
    let abrupt = args.iter().any(|a| a == "--abrupt");
    let mut models: Vec<CfChoice> = ["acc", "cacc-milanes", "cacc-ploeg", "cacc-path"]
        .iter()
        .map(|m| CfChoice::of(m))
        .collect();
    if abrupt {
        for m in ["cacc-milanes", "cacc-ploeg", "cacc-path"] {
            models.push(CfChoice {
                params: serde_json::json!({"degrade_ramp_s": 0.0}),
                ..CfChoice::of(m)
            });
        }
    }
    let losses = [0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0];
    let latencies = [0.01, 0.1, 0.2, 0.3];
    let rows = sweep(&base, &models, &losses, &latencies).expect("every model builds");
    if csv {
        println!(
            "model,loss,latency_s,accel_amplification,gap_error_amplification,string_stable,\
             rms_gap_error_m,max_gap_error_m,cruise_gap_m,throughput_veh_h,cooperative_share,\
             min_gap_m,max_decel_mps2"
        );
        for r in &rows {
            let x = &r.result;
            println!(
                "{},{},{},{:.4},{:.4},{},{:.3},{:.3},{:.2},{:.0},{:.3},{:.2},{:.2}",
                r.model,
                r.loss,
                r.latency_s,
                x.accel_amplification,
                x.gap_error_amplification,
                x.string_stable(),
                x.rms_gap_error_m,
                x.max_gap_error_m,
                x.cruise_gap_m,
                x.throughput_veh_h,
                x.cooperative_share,
                x.min_gap_m,
                x.max_decel_mps2
            );
        }
        return;
    }
    println!("8 vehicles, 25 m/s, leader -2 m/s² for 3 s; 10 Hz beacons; burst length {burst}\n");
    println!(
        "| model | loss | latency s | accel gain | stable | RMS gap err m | cruise gap m | \
         veh/h/lane | coop share | min gap m |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for r in &rows {
        let x = &r.result;
        println!(
            "| {} | {:.1} | {:.2} | {:.3} | {} | {:.2} | {:.1} | {:.0} | {:.2} | {:.1} |",
            r.model,
            r.loss,
            r.latency_s,
            x.accel_amplification,
            if x.string_stable() { "yes" } else { "no" },
            x.rms_gap_error_m,
            x.cruise_gap_m,
            x.throughput_veh_h,
            x.cooperative_share,
            x.min_gap_m
        );
    }
}
