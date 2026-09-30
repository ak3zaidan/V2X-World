# 14 — The Studio shell: a simpler main screen and a settings window

Status: written 2026-09-29 before the change, from an inventory of the page as it stood at
`f4af62c`. It is the layout contract the shell track built to, and the plug-in contract the
metrics-dashboard and viewport/inspector tracks build inside.

## 1. The complaint

The owner: "The UI is too cluttered and too complicated to use; it has to be simpler. The settings
could be in their own settings window that opens up, like the VS Code settings. Get rid of unneeded
stuff on the screen." And: "the metrics should be a panel that opens up and covers the whole
screen."

Measured on the page before the change (mock engine, 200 vehicles, Chromium):

| window | visible controls and labelled elements | viewport (the simulator) |
|---|---|---|
| 1440 x 900 | 109–113 | 800 x 615 px — 38 % of the window |
| 1280 x 800 | 106 | 740 x 515 px — 37 % of the window; the settings panel's **Run** button and the **Commands** tab are cut off at the sidebar's right edge |

Five regions competed for the eye at once: a 280 px settings sidebar with four tabs, a 360 px
inspector, a 150 px plots strip, the header and the viewport's own toolbar. Two of the five were
empty most of the time (the inspector says "select something"; the plots say "nothing yet"), and the
one thing the page exists for — the moving picture — got about a third of the screen.

## 2. Inventory and verdict

Each element on screen, and where it goes. **Main** means always visible; **one click** means
reachable from a control that is always visible; **dev** means behind the developer toggle;
**remove** means not shown at all.

| element | today | verdict | reasoning |
|---|---|---|---|
| Product name "V2X Simulator" | header | main, smaller | Identifies the tool; costs one word. |
| Run state chip (Running, Paused, Finished…) | header | main | The first question anyone has. |
| Scenario name | header | main, and it opens the scenario switcher | The second question. Loading a different scenario is the obvious thing to do from its name. |
| Simulated clock | header | main | The third question. |
| Primary action (Run / Play / Pause / Run again / Connect) | header | main | The one button worth pressing next, always in the same place. |
| Details disclosure (engine, run id, digests, frame counters) | header | one click, in the menu | Identity is cited, not watched. |
| Theme toggle | header | one click, in the menu | Set once. |
| Left tabs: Scenario / Runs / Compare / Commands | sidebar | removed as a sidebar | See the next four rows. |
| Scenario settings form (100+ fields) | sidebar | one click: **Settings window** (gear, Ctrl/Cmd+,) | A form of a hundred fields in a 280 px column is the clutter. It gets the whole page when wanted and none of it otherwise. |
| Ready-made scenarios (load) | sidebar | one click: scenario switcher in the header, and in the Settings window | Choosing a scenario is a menu, not a permanent panel. |
| Check / Apply / Discard / Run | sidebar foot | Settings window footer | They act on the form, so they live with it. |
| Staged-settings note, Revert | sidebar | Settings window footer; the gear carries a dot while edits or staged settings wait | The page must still say "your change waits for the next run" when the window is closed. |
| Runs tab (the run on screen, recordings, world file) | sidebar | one click: menu → **Runs and recordings** (side sheet) | Occasional. |
| Compare tab | sidebar | one click: menu → **Compare** (side sheet); the split viewport stays in the main view while B is open | Occasional; the result of it (two viewports) is main-view. |
| Commands tab (RPC method list, calls made) | sidebar | one click: menu → **Commands** (side sheet) | A reference for developers and agents. |
| Inspector (state / messages / why / log) | right column, always | right column **when something is selected**, or when opened with its toggle; closes with × | Empty most of the time. Opens itself when you select a radio, click a value to explain it, or dock the HUD. |
| Plots strip + breakdowns | bottom strip, always | one click: **Metrics** button → full-screen metrics panel | The owner asked for exactly this. The strip moves in unchanged; the dashboard track replaces it. |
| Status banner (lost contact, finished, …) | above viewport | main, when there is something to say | Often the only sentence worth reading. |
| Time controls (transport, speed, scrub, events) | under viewport | main | You drive the run with them. |
| Viewport toolbar: overlays, camera, follow | viewport | main | Viewing controls, used constantly. |
| World chip "920 lanes · 524 buildings · 12 RSUs" | viewport | dev | A build report, not something a researcher acts on. |
| fps · draw calls · p95 chip | viewport | dev | A renderer readout. The owner named it. |
| HUD dock button | viewport | main | Moves a panel the user is looking at. |
| OBU HUD placeholder "No radio selected" | viewport, always | remove while nothing is selected | The inspector's empty state already says it; floating over the picture it is decoration. |
| State legend | viewport | main | Needed to read the colours. |

## 3. The layout

```
┌ header (40 px) ─────────────────────────────────────────────────────────────────────────┐
│ V2X  ● Running  [manhattan-midtown ▾]  00:00:31.9 sim      [Pause]  Metrics  ◧  ⚙•  ⋯    │
├──────────────────────────────────────────────────────────────────────┬──────────────────┤
│ status banner (only when there is something to say)                  │ inspector        │
│                                                                      │ (only when open) │
│                        viewport                                      │                  │
│                                                                      │                  │
├──────────────────────────────────────────────────────────────────────┤                  │
│ time controls                                                        │                  │
└──────────────────────────────────────────────────────────────────────┴──────────────────┘
```

* The viewport takes every pixel that is not the header, the time bar and — when open — the
  inspector. At 1440 x 900 with nothing selected that is the whole width.
* **Full-screen panels** (Settings, Metrics) cover everything under the header. The header stays,
  so the run state, the clock and the primary action are never hidden: you can Pause from inside
  the settings or the metrics.
* **Side sheets** (Runs, Compare, Commands, Details) slide over the left edge of the viewport and
  close with × or Escape. They do not resize the viewport.
* Escape closes whatever is on top. Focus moves into a panel when it opens and back to the control
  that opened it when it closes.
* The open panel is in the URL hash (`#settings`, `#metrics`, `#runs`…), so a reload keeps it,
  a link opens it and the browser's Back button closes it.

## 4. The settings window

Modelled on VS Code's settings editor.

* Opens from the gear or **Ctrl/Cmd + ,**; covers the page under the header; **Open in new window**
  puts it in its own browser window (`?view=settings`), which talks to the engine over HTTP only —
  no stream, no WebGL — and tells the main window when it applied or ran something.
* A search box, focused on open and again on **Ctrl/Cmd + F**, that filters every setting by title,
  description, dotted path, pointer, group and the engine's status note. Words are ANDed; a word
  with an underscore matches whole or in its parts (`duration_s`, `duration`).
* A category tree on the left from the engine's published groups (`x-group`), each with its second
  level (the scenario section under it) and a count. Clicking one scrolls to it; the tree follows
  the scroll; the arrow keys, Home and End walk it.
* Each setting: title, unit, the engine's status (applied / partly applied / not applied, with the
  engine's sentence), description, the control, the default, the dotted path. A setting whose value
  is not the default carries a **modified** bar and a *Reset to default*; an unapplied edit carries
  an **edited** bar and an *Undo edit* that restores the value the engine holds.
* Filters: **Modified** (edited or not default) and **Unsupported**. Settings the engine does not
  act on (`not-implemented`, `unknown`) are hidden unless Unsupported is on — the filter says how
  many there are, so nothing is hidden silently.
* **JSON** view of the whole scenario, which round-trips with the form: an edit in either shows in
  the other. (YAML is not offered: the page has no YAML parser, and one written for the purpose
  would be a second loader that can disagree with the engine's. The engine accepts the JSON form of
  the same document.)
* Footer: the edit count, the staged note with Revert, **Check**, **Discard**, **Apply**, **Run**.
  Run applies unapplied edits, starts the run and closes the window so you see it.
* Validation errors appear at the field they name, and as a list at the top whose entries jump to
  the field.

## 5. Plug-in points for the next tracks

`src/shell/panels.tsx` is the registry. A panel is `{ id, title, kind: "fullscreen" | "sheet",
keepMounted?, render }`; adding one is an id in `PANEL_IDS` (`state/store.ts`) and an entry in
`PANELS`, whose `Record<PanelId, …>` type makes the two agree. Opening one is `openPanel(id)` from
`shell/route.ts` (the hash follows, Escape and Back close it, focus returns to the opener);
`useStudio().panel` is the one open. The frames are `FullPanel` and `Sheet` in
`shell/PanelFrame.tsx`.

* **Metrics dashboard track** — replace the `metrics` entry's `render` (today `components/MetricsPanel.tsx`, hosting
  the old `PlotsStrip`). It is `keepMounted`, so a chart the reader expanded is still expanded on a
  second look. A chart that expands to more detail can be its own panel or a state inside the body.
* **Viewport and inspector track** — owns `components/Viewport.tsx` and `components/Inspector.tsx`,
  which `App.tsx` places in the viewport and inspector slots; `useStudio().inspectorOpen` is the
  inspector's visibility (selecting a radio, asking *why* and docking the HUD open it). Developer
  readouts over the viewport are shown when `useStudio().devDetails` is on.

## 6. What is deliberately not changed

The time controls, the viewport's own toolbar (beyond the two dev readouts), the inspector's
content and the plots' content are other tracks' work. Every test id the suites use is kept on the
control that now does the job, so the suites move with the layout rather than being rewritten.
