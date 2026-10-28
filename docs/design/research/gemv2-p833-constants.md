# GEMV² and ITU-R P.833 constants (WP-18)

Status: 2026-10-28. The sources were resolved by the P18.1 sources track, and the dated
note is in `docs/design/frameworks/wp16-24-build-plans.md` §WP-18. This file records which
constants the code uses and where each comes from.

## Foliage loss: `obstacle/foliage/p833` (`crates/v2xw-radio/src/foliage.rs`, P18.3)

`A = min(d·γ, J(ν_canopy))`

| Term | Value | Source |
|---|---|---|
| γ, specific attenuation | `0.79·f_GHz^0.61` dB/m: **2.33 dB/m at 5.9 GHz** | GEMV² (Boban, Barros, Tonguz, IEEE TVT 63(9) 2014, eq. (4), pp. 6-7, after Goldhirsh & Vogel); agrees with Durgin et al. 1998 at 5.85 GHz |
| `A = d·γ`, capped at the lowest-loss path around the vegetation | eq. (7) | ITU-R P.833-10 (09/2021) §3.1, p. 9 |
| Path around the vegetation | single knife edge over the canopy top, P.526 `J(ν)` | P.526-14 eq. 31 (`obstacle::knife_edge_loss_db`) |
| Canopy height | 15 m | **ASSUMPTION**: OSM rarely tags a wood's height |
| Tree row width, single-tree crown | 8 m (`diameter_crown` read on trees) | **ASSUMPTION**: the OSM wiki gives no width (Tag:natural=tree_row) |

**Departure from P.833.** §3.1 is stated up to 1 GHz. Above 1 GHz, §3.2.1 (terrestrial,
both terminals outside the vegetation) asks for the radiative-energy-transfer method.
Its fitted parameters (Tables 4-8, pp. 15-19) are given at 1.3, 2, 2.2, 11, 37 and
61.5 GHz, with no 5.9 GHz column. Eq. (1) (one terminal inside woodland) does not apply
to a road-to-road path. So §3.1 is used above its range, with GEMV²'s γ, and the card
says so. Building §3.2.1 would need a 5.9 GHz RET parameter set, interpolated between
2.2 and 11 GHz (an ASSUMPTION of its own). That is left open.

The loss of the hand-computed 200 m link at 1.5 m, with the wood at mid-path:

- 5 m of wood costs 11.6 dB, which is `d·γ`.
- 30 m of wood would cost 69.9 dB through the trees. The canopy path is lower: a
  13.5 m edge at 100/100 m gives ν = 12.0, J = 34.4 dB. The test
  `foliage::tests::thirty_metres_of_wood_lose_the_p833_value` holds both values.

## Small-scale σ (P18.4, not built yet)

GEMV² eq. (10), p. 8:
`σ_i = σ_min + (σ_max − σ_min)/2 · (√(N_V,i/N_V,max) + √(A_S,i/A_S,max))`, inside the
link ellipse. Table V, p. 8:

| Class | σ_min | σ_max |
|---|---|---|
| LOS | 3.3 dB (Porto open space) | 5.2 dB (Porto downtown) |
| NLOSv | 3.8 dB (Porto highway) | 5.3 dB |
| NLOSb | 4.1 dB (Mangel et al.); 0 when reflections are computed | 6.8 dB |

The paper gives no numbers for N_V,max and A_S,max, so both are an ASSUMPTION still to be
pinned. Table VI gives the ellipse major axis r: LOS 500 m urban and 1,000 m outside,
NLOSv 400 m, NLOSb 300 m.
