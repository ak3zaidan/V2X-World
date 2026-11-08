# The Studio (browser UI)

`ui/` is a pnpm workspace (Node 22+, pnpm 10). The Studio adds nothing to the wire: every
engine interaction is a VWP v1 frame or JSON-RPC method of
[protocol/vwp-v1.md](protocol/vwp-v1.md). A walk-through of the page for users is the
[Studio tour](getting-started/studio-tour.md); this page is the developer's view.

| Package | What it is |
|---|---|
| `packages/protocol` (`@vwp/protocol`) | VWP v1 client: framing, binary decoders, pose dequantisation, slot model, `vwp-world/1` parser, typed JSON-RPC client, `VwpClient`, Web Worker transport; no runtime dependencies |
| `packages/viewer` (`@vwp/viewer`) | framework-free Three.js viewer: one scene for 2D and 3D, instanced actors, per-tile world geometry, signals, overlays, cameras, picking |
| `packages/mock-server` (`@vwp/mock-server`) | a Node server speaking the server side of VWP v1 over a synthetic world, for UI development and tests |
| `apps/studio` (`@vwp/studio`) | the React shell (React, Zustand, uPlot) |

## Running

```sh
pnpm --dir ui install
# engine (any one of these) on :8787
./run-sim.sh
v2xw-server --scenario scenarios/midtown-normal.yaml --port 8787
v2xw-server --actors 200 --port 8787                     # synthetic fixture, no map
# page
pnpm --dir ui --filter @vwp/studio dev                   # http://127.0.0.1:5173
```

`VWP_ENGINE` (default `http://127.0.0.1:8787`) sets the engine the dev server proxies
`/vwp/v1`, `/rpc`, `/world` and `/agent` to; `VWP_STUDIO_PORT` sets the page's port. The
endpoints are relative, so the page is same-origin with the engine.

## Structure (`ui/apps/studio/src/`)

| Area | Where | What |
|---|---|---|
| shell | `shell/Header.tsx`, `shell/panels.tsx`, `shell/route.ts` | run state, scenario and city picker, clock, panels mirrored in the URL hash |
| viewport | `components/Viewport.tsx`, `components/TimeControls.tsx` | the canvas, overlays and cameras, transport and scrub bar |
| inspector | `components/Inspector.tsx`, `components/ObuHud.tsx`, `components/pki/` | a selected node's state, messages, provenance ("why"), log; the OBU HUD; credential-system views |
| settings | `settings/SettingsWindow.tsx`, `settings/QuickSettings.tsx` | every scenario setting, generated from the engine's published surface (`scenario.get`), validated with `scenario.validate`, applied with `scenario.set` |
| metrics | `metrics/` (`Dashboard.tsx`, `Chart.tsx`) | live `MetricSample` series and breakdowns |
| runs | `components/RunBrowser.tsx`, `components/ComparisonView.tsx`, `components/RunDetails.tsx` | past runs and same-seed comparison |
| assistant | `components/AgentPanel.tsx`, `components/CopilotPanel.tsx` | the optional assistant (`/agent/*`) |
| state | `state/engine.ts`, `state/store.ts` | the hot path and the React projection |

`state/engine.ts` owns the `VwpClient`, the `Viewer`, the decoded world, the provenance
dictionary, telemetry rings and metric history. React sees a projection refreshed at 5 Hz
(`state/store.ts`); poses, interpolation and instance writes never pass through React.

Protocol behaviours the UI relies on:

- `view.follow` is the telemetry subscription: selecting an actor follows it and
  `Telemetry` frames for that node start arriving.
- Channels start unsubscribed; the app subscribes to the event channels its overlays and
  markers need at connect.
- Values carrying a `prov_id` resolve their provenance with no round trip; others offer
  the `explain` method.
- Sentinel values (`0xFFFF`, `u64::MAX`, NaN) mean "not modelled at this tier" and render
  as n/a.

**Replay without an engine.** `src/lib/replay.ts` loads the `v2xw-wasm` reader and feeds
recorded `Keyframe`/`Delta` frames, byte for byte as the engine produced them, through the
same decoders and pose buffer as the live socket path.

## Checking changes

```sh
pnpm --dir ui typecheck
pnpm --dir ui lint
pnpm --dir ui test                                   # vitest in every package; Playwright for the Studio
pnpm --filter @vwp/studio --dir ui test:unit         # Node-only store and projection tests
pnpm --filter @vwp/studio --dir ui test:e2e          # browser tests against the mock server
pnpm --filter @vwp/studio --dir ui test:engine       # browser tests against a real engine
pnpm --dir ui build
```

Playwright needs a browser once: `pnpm --dir ui --filter @vwp/studio exec playwright
install chromium`.
