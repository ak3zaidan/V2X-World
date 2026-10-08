# Human drivers: heterogeneity, reaction time and perception errors

The scenario block `actors.vehicles.drivers` decides how human the drivers are. The model
lives in `crates/v2xw-mobility/src/drivers.rs`, with every constant's source. The engine
reads the block in `crates/v2xw-engine/src/wiring.rs` and `driving.rs`.

```yaml
actors:
  vehicles:
    drivers:
      realism: human          # calibrated (default) | identical | heterogeneous | human
      parameter_spread: 1.0   # optional: multiplier on the per-class spread of T, a, b, s0 (0..2)
      reaction_time_s: 0.8    # optional: car-following reaction time (0..2 s)
      perception_error: 1.0   # optional: multiplier on the HDM estimation errors (0..3)
      truck_lane_restriction: true   # optional: trucks out of a motorway's leftmost lane (3+ lanes)
      motorcycle_filtering: true     # optional: override the jurisdiction's filtering rule
```

| realism | what each driver gets |
|---|---|
| `identical` | the preset's driver exactly: the posted limit, no start-up reaction, one following style. Use it to test a model on its own. |
| `calibrated` | the default. Each driver has their own free speed (SUMO `speedFactor`), a start-up reaction at a green (Taoka 1989), an amber decision (Zegeer & Deen 1978) and one of three following styles. Car following itself reacts at once. |
| `heterogeneous` | `calibrated`, plus each driver's own time gap T, acceleration a, braking b and standstill gap s0. Each is a mean-one lognormal factor with a per-class cv of 0.15–0.30, on the order of the inter-driver spread in per-driver IDM fits to trajectory data. a and b are correlated at 0.5. |
| `human` | `heterogeneous`, plus a 0.8 s car-following reaction time (Green 2000) and the Human Driver Model's estimation errors (Treiber, Kesting & Helbing 2006): the gap is misjudged by 5 % and the inverse time-to-collision by 0.01 s⁻¹, as Ornstein-Uhlenbeck processes with τ̃ = 20 s. A driver who sees a need to brake harder than their comfortable deceleration reacts at once. |

Each population keeps the fleet's mean parameters, so the HCM saturation flow the city
drivers were calibrated to does not shift. Only the spread and the dynamics change. The
`human` population is the one that forms stop-and-go waves with no bottleneck, as
Sugiyama's drivers did. The `identical` and `calibrated` drivers are too stable to do that.

- `truck_lane_restriction`: trucks and trailers make no discretionary change into the
  leftmost lane of a motorway or trunk edge with three or more driving lanes. This
  follows StVO §18(11) and the US states' left-lane truck restrictions.
- `motorcycle_filtering: true` lets motorcycles and mopeds ride between stopped cars. It
  turns on the SL2015 sub-lane model if the scenario has none, because filtering needs
  sub-lane positions.

All the noise is keyed by driver and step (SplitMix64 over a per-trip key from the
`DesiredSpeed` stream). A run therefore stays bit-reproducible, and one driver's draws
never depend on another's.

Validation: `cargo run -p v2xw-trafficval --bin traffic_calibration` writes
`docs/validation/traffic-calibration.md`. It covers fundamental diagrams on a city ring and
a freeway corridor ring with Greenshields and triangular fits, capacity against the HCM,
the stop-and-go wave speed, headways near capacity, the stop-and-go emergence check, the
IDM rest gap, string stability and Sugiyama's ring, all beside SUMO's default driver.
