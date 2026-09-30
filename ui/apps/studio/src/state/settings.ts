/**
 * What the settings window does to the engine: check, apply, discard, run, load a ready-made
 * scenario, withdraw an applied one.
 *
 * # Applying an edit
 *
 * An edit reaches the next run. **Apply** sends the edited document to `scenario.set`, which runs it
 * through the loader (the same code a scenario file goes through) and holds it for the next
 * `run.start`; **Run** applies any unapplied edit first, then starts a run, which builds a fresh
 * kernel on the held scenario. The run's `Hello` then carries the new scenario digest, the new run id
 * and the new world, so what the page shows is provably the edited run and not the old one.
 *
 * # Two windows
 *
 * The window can be opened in a browser window of its own. Both copies talk to one engine, so each
 * tells the other when it changed what the engine holds (`scenario-changed`), and the detached one
 * asks the main page to start a run (`run`) — the main page holds the stream a run is watched on.
 * With no main page listening, the detached window starts the run itself over HTTP.
 */

import { engine } from "./engine.js";
import { useStudio, type ScenarioListItem } from "./store.js";
import { changedPointers, getPointer, setPointer } from "../lib/schema.js";
import { isUnsupported, settingsFields, type Field } from "../settings/model.js";

/** The message shape on the settings channel. */
type ChannelMessage =
  | { readonly kind: "scenario-changed" }
  | { readonly kind: "run"; readonly id: string }
  | { readonly kind: "run-ack"; readonly id: string; readonly state?: string; readonly error?: string };

const CHANNEL = "vwp-studio-settings";

let channel: BroadcastChannel | null = null;
function bus(): BroadcastChannel | null {
  if (channel === null && typeof BroadcastChannel !== "undefined") channel = new BroadcastChannel(CHANNEL);
  return channel;
}

function post(message: ChannelMessage): void {
  try {
    bus()?.postMessage(message);
  } catch {
    /* a closed channel is a window that went away */
  }
}

/**
 * Listen for the other window. The main page passes `onRun`, which starts a run on its stream; the
 * detached window passes none and only follows scenario changes.
 */
export function listenForOtherWindow(onRun?: () => Promise<string>): () => void {
  const b = bus();
  if (b === null) return () => undefined;
  const handler = (ev: MessageEvent<ChannelMessage>): void => {
    const m = ev.data;
    if (m.kind === "scenario-changed") {
      void engine.refreshScenario();
    } else if (m.kind === "run" && onRun) {
      onRun().then(
        (state) => post({ kind: "run-ack", id: m.id, state }),
        (err: unknown) => post({ kind: "run-ack", id: m.id, error: err instanceof Error ? err.message : String(err) }),
      );
    }
  };
  b.addEventListener("message", handler);
  return () => b.removeEventListener("message", handler);
}

/** True in the settings window opened in a browser window of its own. */
export function isDetachedSettings(): boolean {
  return typeof window !== "undefined" && new URLSearchParams(window.location.search).get("view") === "settings";
}

/** The fields the window is built from, as the store holds them now. */
export function currentFields(): readonly Field[] {
  const s = useStudio.getState();
  return settingsFields(s.scenarioExtras.fields, s.scenarioSchema, s.scenario).fields;
}

/** The unapplied edits, as pointers. */
export function currentEdits(): string[] {
  const s = useStudio.getState();
  return s.draft === null ? [] : changedPointers(s.scenario, s.draft);
}

/** Put one value into the draft. */
export function editSetting(pointer: string, value: unknown): void {
  useStudio.getState().setDraft((prev) => setPointer<Record<string, unknown>>(prev ?? {}, pointer, value));
}

/** Put one setting back to what the engine holds for the next run. */
export function undoEdit(pointer: string): void {
  editSetting(pointer, getPointer(useStudio.getState().scenario, pointer));
}

/** Put the whole draft back to what the engine holds. */
export function discardEdits(): void {
  const s = useStudio.getState();
  s.setDraft((s.scenario ?? null) as Record<string, unknown> | null);
  s.setSettingsMessage({ text: "Edits discarded. The form shows the engine's settings again.", tone: "info" });
}

/**
 * Run one of the window's actions: mark it busy, clear the last message, and put what it said —
 * or why it failed, in the engine's own words — where the window shows it.
 */
export async function runSettingsAction(name: string, fn: () => Promise<string>): Promise<void> {
  const s = useStudio.getState();
  if (s.settingsBusy !== null) return;
  s.setSettingsBusy(name);
  s.setSettingsMessage(null);
  try {
    useStudio.getState().setSettingsMessage({ text: await fn(), tone: "info" });
  } catch (err) {
    useStudio
      .getState()
      .setSettingsMessage({ text: `${name} did not work: ${err instanceof Error ? err.message : String(err)}`, tone: "err" });
  } finally {
    useStudio.getState().setSettingsBusy(null);
  }
}

/** Edited fields the engine will not act on, by the status it published for them. */
export function inertEdits(fields: readonly Field[], edits: readonly string[]): Field[] {
  return fields.filter((f) => isUnsupported(f) && edits.some((p) => p === f.pointer || p.startsWith(`${f.pointer}/`)));
}

/** Ask the engine whether it accepts the draft, changing nothing. */
export async function checkDraft(): Promise<string> {
  const draft = useStudio.getState().draft;
  const res = await engine.request("scenario.validate", { ...(draft ? { scenario: draft } : {}), strict: false });
  useStudio.getState().setValidation({ valid: res.valid, errors: res.errors, warnings: res.warnings });
  return res.valid
    ? "The engine accepts these settings."
    : `${res.errors.length} setting${res.errors.length === 1 ? "" : "s"} need fixing — each is marked where it is.`;
}

/** Send the draft to the engine for the next run. Returns a sentence, or throws with the reasons. */
export async function applyDraft(): Promise<string> {
  const draft = useStudio.getState().draft;
  if (draft === null) return "There is nothing to apply.";
  const inert = inertEdits(currentFields(), currentEdits());
  const res = await engine.request("scenario.set", { scenario: draft, validate: false });
  useStudio.getState().setValidation({ valid: res.valid, errors: res.errors ?? [], warnings: [] });
  if (!res.valid) {
    const n = (res.errors ?? []).length;
    throw new Error(`the engine refused ${n} setting${n === 1 ? "" : "s"} — each is marked where it is. Nothing was changed.`);
  }
  await engine.refreshScenario();
  post({ kind: "scenario-changed" });
  const changed = res.requires_restart ?? [];
  const inertNote =
    inert.length > 0
      ? ` ${inert.length} of your edits (${inert.map((f) => f.label).join(", ")}) change nothing in this build — see the marks on those fields.`
      : "";
  return changed.length === 0
    ? `These are the settings of the run already on screen; there is nothing to change.${inertNote}`
    : `Applied ${changed.length} change${changed.length === 1 ? "" : "s"}. The next run uses them — press Run.${inertNote}`;
}

/**
 * Start a run, on the stream this page holds or — in the detached window — by asking the main page
 * to, falling back to HTTP when no main page answers within two seconds.
 */
async function startRunFromHere(): Promise<string> {
  if (!isDetachedSettings()) return engine.startRun();
  const b = bus();
  if (b !== null) {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const answer = await new Promise<ChannelMessage | null>((resolve) => {
      const timer = setTimeout(() => {
        b.removeEventListener("message", onMessage);
        resolve(null);
      }, 2000);
      function onMessage(ev: MessageEvent<ChannelMessage>): void {
        if (ev.data.kind === "run-ack" && ev.data.id === id) {
          clearTimeout(timer);
          b?.removeEventListener("message", onMessage);
          resolve(ev.data);
        }
      }
      b.addEventListener("message", onMessage);
      post({ kind: "run", id });
    });
    if (answer !== null && answer.kind === "run-ack") {
      if (answer.error) throw new Error(answer.error);
      await engine.refreshStatus();
      return answer.state ?? "running";
    }
  }
  const started = await engine.requestHttp("run.start", {});
  await engine.refreshStatus();
  return (started as { state?: string }).state ?? "running";
}

/** Apply any unapplied edit, then start a run with it. */
export async function runWithDraft(): Promise<string> {
  let applied = "";
  if (currentEdits().length > 0) applied = `${await applyDraft()} `;
  const state = await startRunFromHere();
  const now = useStudio.getState().run;
  return `${applied.replace(" — press Run.", ".")}${state === "running" ? "Running" : `Started; the engine reports it is ${state}`} — ${Math.round(now.tEndNs / 1e9)} s of simulated time.`;
}

/** Load a ready-made scenario into the form; the next Run runs it. */
export async function loadPreset(item: ScenarioListItem): Promise<string> {
  const res = await engine.request("scenario.load", { path: item.id, validate: true });
  const s = useStudio.getState();
  // Loading a scenario replaces the form. Unapplied edits are not rebased onto it (as they are
  // when the engine's copy is merely re-fetched): an edit left over from before — a refused one
  // included — would otherwise ride into the loaded scenario unseen and be what the next Run applies.
  s.setDraft(null);
  await engine.refreshScenario();
  post({ kind: "scenario-changed" });
  useStudio.getState().setDraft((d) => d ?? ((useStudio.getState().scenario ?? null) as Record<string, unknown> | null));
  if (!res.valid) {
    useStudio.getState().setValidation({ valid: false, errors: res.errors ?? [], warnings: [] });
    return `${item.name ?? item.id} does not load: each problem is marked where it is.`;
  }
  // A previous edit's refusal was about the form this load replaced.
  useStudio.getState().setValidation(null);
  return res.hash === useStudio.getState().scenarioExtras.runningHash
    ? "That is the scenario the engine is running now."
    : `Loaded ${item.name ?? item.id}. Press Run to start it.`;
}

/** Withdraw what Apply staged; the next run is the scenario on screen. */
export async function revertStaged(): Promise<string> {
  await engine.request("scenario.load", { path: useStudio.getState().scenarioExtras.runningHash, validate: true });
  await engine.refreshScenario();
  post({ kind: "scenario-changed" });
  return "The applied settings were withdrawn. The next run is the scenario on screen.";
}

/** The ready-made scenarios the engine lists. */
export function presetsOf(list: readonly ScenarioListItem[]): ScenarioListItem[] {
  return list.filter((i) => i.kind === "preset");
}
