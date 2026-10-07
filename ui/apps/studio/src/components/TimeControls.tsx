/**
 * The transport bar: where the run is, what it is doing, and the five controls that move it.
 *
 * Every control is one JSON-RPC call — `run.resume`, `run.pause`, `run.step`, `run.speed`,
 * `run.seek`, `run.start` — which is what makes the copilot's tool surface identical to the UI's.
 * What changed is that the bar now tells the truth about all six.
 *
 * # What it used to say
 *
 * `00:00:00.000 / 00:01:00.000 · idle`, against a server reporting `state: "finished"` at 60.1
 * seconds. Three errors in one line, all the same error: the 2 s `run.status` poll went through the
 * socket, the socket closes when a run ends, the poll failed, and the store kept the zeroes it was
 * initialised with. The bar reported the absence of an answer as a fact about the run. The poll now
 * falls back to HTTP (`StudioEngine.request`), so those figures are the engine's.
 *
 * # What it says now
 *
 *  * **One row, five things**: play or pause, step, the speed, the clock, and the timeline with its
 *    events on it. Restart and Stop are in the header's menu (and Restart is the header's own button
 *    once a run has finished); going to the start or one step back is Home and the left arrow on the
 *    timeline. The bar used to carry thirteen controls, a step-unit menu and an events menu.
 *  * **A disabled control says why.** Which controls the engine will accept depends on the state it
 *    is in, and `state/transport.ts` holds those rules with the engine's own refusals beside them.
 *  * **How much of the span exists.** The bar draws the part of the run that has been simulated
 *    separately from the part that has not, because dragging into the second one is refused.
 *  * **The events are on the timeline and can be clicked.** Each certificate change, detection,
 *    revocation and safety warning is a tick above the track, and the scenario's own planned events a
 *    taller one; a click jumps there, and Alt with an arrow key jumps to the next or previous one. The
 *    ticks used to sit under the range input, where no pointer could reach them.
 *  * **A refused seek is reported.** `run.seek` outside the produced range fails with `-32003`,
 *    whose `data` carries the range that *would* have worked. That is shown, and the range it
 *    reports is remembered and drawn.
 *
 * The scrub bar commits on release, not on change. React maps `onChange` on an
 * `<input type="range">` onto the DOM `input` event, so a plain `onChange={seek}` fires once for
 * every step the thumb crosses: a single drag across a 300 s run at the reference 0.1 s mobility
 * step is 3,000 `run.seek` calls, each one pausing the run, each one followed by a `run.status`
 * poll. While the pointer (or an arrow key) is down the value is held in local state, which also
 * stops the 5 Hz stream from fighting the thumb; one `run.seek` goes out when the gesture ends.
 *
 * Two further cases the bar carries:
 *
 *  * **A recording takes it over.** With a local recording open the scrub drives the WebAssembly
 *    reader instead of `run.seek`, over the recording's own span, and the transport is not merely
 *    disabled but gone: a recording has no clock to start, and an inert Play button invites a press
 *    that can never do anything.
 *  * **Side B follows.** While the comparison view is synchronised, every control that moves side
 *    A's clock moves side B's afterwards, never in parallel (a seek's frames precede its reply, and
 *    two in flight would interleave).
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { JsonRpcError } from "@vwp/protocol";

import { compare } from "../state/compare.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { transport as transportCaps } from "../state/transport.js";
import { durationNs, simClock } from "../lib/format.js";
import { describeError } from "../lib/errors.js";
import { useStatus } from "./Status.js";

/**
 * Speeds the selector offers.
 *
 * `0` is in the list because the engine accepts it and means something useful by it — the producer
 * skips its wall-clock sleep entirely — and because it is how anyone running a batch actually wants
 * to run: a 60 s scenario at 1× takes a minute of your life for no reason. The selector offered
 * 0.1× to 100× and left the fastest setting the server has reachable only from a terminal.
 */
const SPEEDS = [0, 0.1, 0.25, 0.5, 1, 2, 5, 10, 25, 50, 100];

function speedLabel(speed: number): string {
  return speed === 0 ? "as fast as possible" : `${speed}×`;
}

const MARK_COLOR: Record<string, string> = {
  "sec.cert": "var(--state-reported, #e69f00)",
  "det.observation": "var(--gt-tag, #009e73)",
  "proto.revocation": "var(--state-revoked, #cc79a7)",
  "app.warning": "var(--state-attacker, #d55e00)",
};

/** What each event channel is, so a marker's tooltip is a sentence and not a wire name. */
const CHANNEL_LABEL: Record<string, string> = {
  "sec.cert": "certificate change",
  "det.observation": "misbehaviour observation",
  "proto.revocation": "revocation",
  "app.warning": "safety warning",
};

/**
 * How long a seek may take. A seek past what the live kernel has produced runs the kernel to the
 * target first (with progress shown), which on a large map in a debug build can take minutes.
 */
const SEEK_TIMEOUT_MS = 10 * 60_000;

/** The seekable range an engine reported when it refused a seek (`-32003`). */
interface SeekRefusal {
  readonly minNs: number;
  readonly maxNs: number;
}

/** Read `{min_ns, max_ns}` out of a `-32003` error, if that is what this is. */
function seekRefusal(err: unknown): SeekRefusal | null {
  if (!(err instanceof JsonRpcError)) return null;
  const data = err.data;
  if (data === null || typeof data !== "object") return null;
  const min = (data as { min_ns?: unknown }).min_ns;
  const max = (data as { max_ns?: unknown }).max_ns;
  if (typeof min !== "number" || typeof max !== "number") return null;
  return { minNs: min, maxNs: max };
}

/**
 * Where a `run.pause` or `run.step` reply (§6.6, `{state, t_ns}`) left a paused run, or `null` for
 * any other reply — a resume, whose clock is moving, included.
 */
function pausedAt(reply: unknown): number | null {
  if (reply === null || typeof reply !== "object") return null;
  const { state, t_ns: t } = reply as { state?: unknown; t_ns?: unknown };
  return state === "paused" && typeof t === "number" ? t : null;
}

export function TimeControls(): React.JSX.Element {
  const run = useStudio((s) => s.run);
  const simTimeNs = useStudio((s) => s.simTimeNs);
  const timeline = useStudio((s) => s.timeline);
  const hello = useStudio((s) => s.hello);
  const connection = useStudio((s) => s.connection);
  const replay = useStudio((s) => s.replay);
  const compareSide = useStudio((s) => s.compare);
  const compareSync = useStudio((s) => s.compareSync);
  const status = useStatus();
  const seekProgress = useStudio((s) => s.seekProgress);
  const scenarioDoc = useStudio((s) => s.scenario);
  const stagedScenario = useStudio((s) => s.scenarioExtras.staged);
  const firedEvents = useStudio((s) => s.firedEvents);
  const [busy, setBusy] = useState(false);
  /** The value under the thumb while a scrub gesture is in flight; `null` when it is not. */
  const [scrubNs, setScrubNs] = useState<number | null>(null);
  /** The last thing a control said, when it was not what the user asked for. */
  const [notice, setNotice] = useState<string | null>(null);
  /** The range the engine reported the last time it refused a seek. */
  const [refused, setRefused] = useState<SeekRefusal | null>(null);
  const scrubRef = useRef<number | null>(null);
  /**
   * Where the last seek landed, until the stream's clock gets there.
   *
   * `run.seek` answers before the page's clock moves: the keyframe it streams reaches the store on
   * the next 5 Hz flush. In that gap the bar still read the old instant, so a second Alt+← right
   * after an Alt+→ looked for an event before the *previous* position — "No event before this
   * point." at 00:00:18 with an event at 12 s — and the thumb snapped back for a moment after every
   * drag. The bar reads this instead until the stream agrees, or for a second at most.
   */
  const [landingNs, setLandingNs] = useState<number | null>(null);
  const barRef = useRef<HTMLDivElement | null>(null);
  /**
   * Which of the bar's controls had the keyboard when a call made the bar busy. Every control is
   * disabled while a call is in flight, and a disabled element drops the focus to the page: Alt+→
   * jumped to the next event and the Alt+← after it went nowhere, and Space on Play started the run
   * and a second Space did not pause it. The focus goes back once the call is over — to the same
   * control, or from Play to the Pause that replaced it and back.
   */
  const refocusRef = useRef<string | null>(null);
  const markBusy = useCallback(() => {
    const active = typeof document === "undefined" ? null : document.activeElement;
    const bar = barRef.current;
    if (active instanceof HTMLElement && bar !== null && bar.contains(active)) {
      refocusRef.current = active.dataset.testid ?? null;
    }
    setBusy(true);
  }, []);
  useEffect(() => {
    const was = refocusRef.current;
    if (busy || was === null) return;
    refocusRef.current = null;
    const bar = barRef.current;
    if (bar === null) return;
    const swap: Record<string, string> = { play: "pause", pause: "play" };
    const target =
      bar.querySelector<HTMLElement>(`[data-testid="${was}"]`) ??
      (swap[was] !== undefined ? bar.querySelector<HTMLElement>(`[data-testid="${swap[was]}"]`) : null);
    target?.focus();
  }, [busy]);

  /**
   * Which clock the bar is driving.
   *
   * A local recording wins over a connection whenever one is open, because that is also what the
   * viewport is showing: `StudioEngine.openLocalRecording` detaches the stream from the viewer, so
   * a bar that kept issuing `run.seek` would move a run nobody can see. The recording's own seek is
   * the cheaper of the two anyway — one chunk and one keyframe period of deltas.
   */
  const drivingReplay = replay !== null;
  const streamNs = drivingReplay ? replay.tNs : simTimeNs > 0 ? simTimeNs : run.tNs;

  const caps = useMemo(
    () =>
      transportCaps({
        connection,
        runState: run.state,
        tNs: run.tNs,
        tEndNs: run.tEndNs > 0 ? run.tEndNs : hello?.simDurationNs ?? 0,
        streamNs,
        recording: replay === null ? null : { startNs: replay.startNs, endNs: replay.endNs },
        busy,
      }),
    [connection, run.state, run.tNs, run.tEndNs, hello?.simDurationNs, streamNs, replay, busy],
  );

  /**
   * Whether the timeline itself takes input, which unlike the buttons does not wait for a call in
   * flight. It used to be disabled with them, so an arrow key pressed while the previous press's
   * seek was still out was dropped: two quick presses moved one step (regressions e2e, "arrow keys
   * on the scrub bar", on a loaded machine where a seek takes longer). A gesture that ends while
   * a call is out now waits for it (`serial`) and then seeks, so each press is still one seek.
   */
  const scrubCaps = useMemo(
    () =>
      transportCaps({
        connection,
        runState: run.state,
        tNs: run.tNs,
        tEndNs: run.tEndNs > 0 ? run.tEndNs : hello?.simDurationNs ?? 0,
        streamNs,
        recording: replay === null ? null : { startNs: replay.startNs, endNs: replay.endNs },
        busy: false,
      }).seek,
    [connection, run.state, run.tNs, run.tEndNs, hello?.simDurationNs, streamNs, replay],
  );
  /** The bar's calls, one after another: the tail of the chain, and how many are in it. */
  const chainRef = useRef<Promise<void>>(Promise.resolve());
  const queuedRef = useRef(0);
  const serial = useCallback(<T,>(fn: () => Promise<T>): Promise<T> => {
    queuedRef.current += 1;
    const run = chainRef.current.then(fn);
    const settle = (): void => {
      queuedRef.current -= 1;
    };
    chainRef.current = run.then(settle, settle);
    return run;
  }, []);

  const { minNs: startNs, spanNs: endNs } = caps;
  // The clock, the fill and the thumb all read the dragged value, so the readout stays live while
  // the gesture is in flight and no seek has been issued yet; after a seek, the instant it landed on
  // until the stream's clock reaches it.
  const nowNs = scrubNs ?? landingNs ?? streamNs;
  useEffect(() => {
    if (landingNs === null) return;
    const slackNs = (hello?.mobilityStepNs ?? 1e8) / 2;
    if (Math.abs(streamNs - landingNs) <= slackNs) {
      setLandingNs(null);
      return;
    }
    const id = window.setTimeout(() => setLandingNs(null), 1000);
    return () => window.clearTimeout(id);
  }, [landingNs, streamNs, hello?.mobilityStepNs]);
  const span = endNs - startNs;
  const pct = useCallback(
    (tNs: number) => (span > 0 ? Math.min(100, Math.max(0, ((tNs - startNs) / span) * 100)) : 0),
    [span, startNs],
  );
  // What the engine will actually seek to: what it has produced, narrowed to whatever a refusal
  // reported. Before any refusal this is only a hint, which is why it is drawn and not enforced.
  const seekableNs = refused === null ? caps.seekMaxNs : Math.min(caps.seekMaxNs, refused.maxNs);
  const showSeekable = caps.partial && seekableNs > startNs;

  const call = useCallback(
    (fn: () => Promise<unknown>) =>
      serial(async () => {
        markBusy();
        setNotice(null);
        try {
          // A pause or a step answers with where the run now stands; that is where the bar is until
          // the stream's own clock gets there (see `landingNs`).
          const landed = pausedAt(await fn());
          if (landed !== null && !drivingReplay) setLandingNs(landed);
        } catch (err) {
          setNotice(describeError(err));
        } finally {
          setBusy(false);
          if (!drivingReplay) await engine.refreshStatus();
          // A step, a pause or a resume moves side A's clock too, so side B follows it here rather
          // than only on a scrub: `run.status` has just been refreshed, so this reads the new time.
          if (compareSide !== null && compareSync.time) {
            const state = useStudio.getState();
            await compare.seekTo(state.simTimeNs > 0 ? state.simTimeNs : state.run.tNs);
          }
        }
      }),
    [compareSide, compareSync.time, drivingReplay, markBusy, serial],
  );

  /**
   * The scenario's own timeline (`events`): what is planned to happen and when, drawn whether or
   * not it has happened yet, and filled in once the engine reports it fired. Only while the
   * document the page holds is the running one — with settings applied for the next run, the
   * document describes that run instead, and its events would be drawn on the wrong run.
   */
  const planned = useMemo(() => {
    if (span <= 0 || stagedScenario !== null || drivingReplay) return [];
    const events = (scenarioDoc as { events?: unknown } | null)?.events;
    if (!Array.isArray(events)) return [];
    return events.flatMap((raw, index) => {
      const e = raw as { t?: unknown; until?: unknown; type?: unknown; target?: unknown; value?: unknown; path?: unknown };
      if (typeof e.t !== "number") return [];
      const tNs = e.t * 1e9;
      const untilNs = typeof e.until === "number" ? e.until * 1e9 : null;
      const fired = firedEvents.filter((f) => f.index === index);
      const what =
        e.type === "closure" ? `closure of ${String(e.target)}`
        : e.type === "demand.multiplier" ? `demand ×${String(e.value)}`
        : e.type === "weather.front" ? `weather front: ${String(e.value)}`
        : e.type === "param.change" ? `${String(e.path)} → ${JSON.stringify(e.value)}`
        : e.type === "outage" ? `outage of node ${String(e.target)}`
        : String(e.type);
      return [{
        index,
        type: String(e.type),
        tNs,
        untilNs,
        left: pct(tNs),
        width: untilNs === null ? 0 : Math.max(0, pct(untilNs) - pct(tNs)),
        fired: fired.length > 0,
        title: `${what} at ${simClock(tNs)}${untilNs === null ? "" : ` until ${simClock(untilNs)}`}` +
          (fired.length > 0 ? ` — ${fired.map((f) => f.effect).join("; ")}` : " — not reached yet"),
      }];
    });
  }, [span, stagedScenario, drivingReplay, scenarioDoc, firedEvents, pct]);

  const marks = useMemo(() => {
    if (span <= 0) return [];
    const seen = new Map<string, { left: number; channel: string; label: string; tNs: number }>();
    for (const m of timeline) {
      const left = Math.min(100, Math.max(0, ((m.tNs - startNs) / span) * 100));
      const key = `${m.channel}:${left.toFixed(2)}`;
      if (!seen.has(key)) seen.set(key, { left, channel: m.channel, label: m.label, tNs: m.tNs });
    }
    return [...seen.values()];
  }, [timeline, span, startNs]);

  /**
   * Whether a seek also moves the comparison side.
   *
   * The two runs share one simulated clock while `compareSync.time` is on. Side B is always moved
   * *after* side A, never in parallel: a `run.seek` streams its keyframe and deltas before its
   * reply, so two of them in flight on one main thread would interleave their frames.
   *
   * Every transport control routes through {@link call}, which does that follow-up once. `seekTo`
   * therefore issues side A's seek only — issuing B's here as well would seek a recording twice for
   * one gesture.
   */
  const syncB = compareSide !== null && compareSync.time;

  const seekOne = useCallback(
    async (tNs: number): Promise<void> => {
      const target = Math.round(tNs);
      if (drivingReplay) {
        await engine.seekLocalReplay(target);
        return;
      }
      try {
        // A target past what the live kernel has produced is reached by running the kernel
        // there; the engine reports `job.progress` meanwhile (shown below the bar), so the call
        // is allowed as long as a long jump on a large map takes rather than the usual 30 s.
        await engine.request("run.seek", { t_ns: target, pause_after: true }, { timeoutMs: SEEK_TIMEOUT_MS });
        setLandingNs(target);
      } catch (err) {
        // §6.6's `-32003` carries the range that would have worked. Remembering it is how the bar
        // learns a bound `run.status` never publishes — and how the two engines in this repository,
        // which disagree about how far a run may be seeked, both end up drawn correctly.
        const range = seekRefusal(err);
        if (range === null) throw err;
        setRefused(range);
        throw new Error(
          `The engine could only simulate up to ${simClock(range.maxNs)}, so it cannot move to ` +
            `${simClock(target)}: the run ended or stopped before it got there.`,
        );
      }
    },
    [drivingReplay],
  );

  const seekTo = useCallback(
    (tNs: number) => {
      void call(() => seekOne(tNs));
    },
    [call, seekOne],
  );

  /**
   * End of gesture: issue the one `run.seek` the whole drag is worth.
   *
   * The thumb keeps showing where the user put it until `run.status` confirms the new time, so it
   * does not snap back to the pre-seek position for the length of the round trip.
   */
  const commitScrub = useCallback(async () => {
    const value = scrubRef.current;
    scrubRef.current = null;
    if (value === null) {
      if (queuedRef.current === 0) setScrubNs(null);
      return;
    }
    await serial(async () => {
      markBusy();
      setNotice(null);
      try {
        await seekOne(value);
        if (compareSide !== null && compareSync.time) await compare.seekTo(Math.round(value));
      } catch (err) {
        setNotice(describeError(err));
      } finally {
        setBusy(false);
        if (!drivingReplay) await engine.refreshStatus();
        // The thumb lets go of the gesture's value only when nothing else is queued behind this
        // seek and no new gesture has started: otherwise it would snap back to this landing for
        // the length of the next seek.
        if (queuedRef.current <= 1 && scrubRef.current === null) setScrubNs(null);
      }
    });
  }, [compareSide, compareSync.time, seekOne, drivingReplay, markBusy, serial]);

  // A range thumb dragged past the edge of the input releases the pointer somewhere else, so the
  // release is caught on the window rather than on the element.
  useEffect(() => {
    if (scrubNs === null) return;
    const onUp = (): void => void commitScrub();
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [scrubNs, commitScrub]);

  // A new run clears what the old one refused: `run.start` rewinds, and the range it will accept
  // grows again from zero.
  useEffect(() => {
    if (run.state === "idle" || run.tNs === 0) setRefused(null);
  }, [run.state, run.tNs]);

  /** Every event on the timeline, oldest first: the engine's marks and the scenario's plan. */
  const eventTimes = useMemo(
    () => [...new Set([...timeline.map((m) => m.tNs), ...planned.map((p) => p.tNs)])].sort((a, b) => a - b),
    [timeline, planned],
  );
  const jumpEvent = useCallback(
    (dir: 1 | -1) => {
      // Half a mobility step of slack, so standing on an event and asking for the next one moves on.
      const slack = (hello?.mobilityStepNs ?? 1e8) / 2;
      const next = dir > 0 ? eventTimes.find((t) => t > nowNs + slack) : [...eventTimes].reverse().find((t) => t < nowNs - slack);
      if (next !== undefined) seekTo(next);
      else setNotice(dir > 0 ? "No event after this point." : "No event before this point.");
    },
    [eventTimes, nowNs, seekTo, hello?.mobilityStepNs],
  );

  return (
    <div className="timebar" data-testid="time-controls" ref={barRef}>
      {run.state === "running" ? (
        <button
          type="button"
          className="icon primary tb-play"
          title={caps.pause.why}
          disabled={!caps.pause.enabled}
          onClick={() => void call(() => engine.request("run.pause", {}))}
          data-testid="pause"
          aria-label="Pause"
        >
          ❚❚
        </button>
      ) : (
        <button
          type="button"
          // Not `primary` while it is disabled: an accented button reads as the thing to press.
          className={caps.play.enabled ? "icon primary tb-play" : "icon tb-play"}
          title={caps.play.why}
          disabled={!caps.play.enabled}
          onClick={() => void call(() => engine.request("run.resume", {}))}
          data-testid="play"
          aria-label="Play"
        >
          ▶
        </button>
      )}
      <button
        type="button"
        className="icon"
        title={caps.step.enabled ? "Advance one mobility step and stop" : caps.step.why}
        disabled={!caps.step.enabled}
        onClick={() => void call(() => engine.request("run.step", { unit: "step", count: 1 }))}
        data-testid="step"
        aria-label="Step forward"
      >
        ▶|
      </button>

      <select
        className="tb-speed"
        value={String(run.speed)}
        onChange={(e) => void call(() => engine.request("run.speed", { speed: Number(e.target.value) }))}
        aria-label="Speed"
        title={caps.speed.why}
        data-testid="speed"
        disabled={!caps.speed.enabled}
      >
        {(SPEEDS.includes(run.speed) ? SPEEDS : [run.speed, ...SPEEDS]).map((sp) => (
          <option key={sp} value={String(sp)}>
            {speedLabel(sp)}
          </option>
        ))}
      </select>

      <span className="clock" data-testid="sim-clock" title={drivingReplay ? "The recording's own span" : status.headline}>
        {simClock(nowNs)}
      </span>

      <div className="scrub" data-testid="scrub">
        <div className="track" />
        {/*
          How much of the span exists. Drawn only when part of it does not: on a finished run, or
          one whose whole span has been produced, a second region would be a distinction without a
          difference.
        */}
        {showSeekable ? (
          <div
            className="produced"
            style={{ width: `${pct(seekableNs)}%` }}
            title={`Simulated up to ${simClock(seekableNs)}. Moving beyond it runs the simulation there first.`}
          />
        ) : null}
        <div className="fill" style={{ width: `${pct(nowNs)}%` }} />
        {planned.map((p) =>
          p.width > 0 ? (
            <div key={`band-${p.index}`} className="event-band" aria-hidden="true" style={{ left: `${p.left}%`, width: `${p.width}%` }} title={p.title} />
          ) : null,
        )}
        <input
          type="range"
          min={startNs}
          max={Math.max(startNs + 1, endNs)}
          step={hello?.mobilityStepNs ?? 1e8}
          value={nowNs}
          disabled={!scrubCaps.enabled}
          aria-label="Position in simulated time"
          aria-valuetext={simClock(nowNs)}
          aria-keyshortcuts="Alt+ArrowRight Alt+ArrowLeft"
          title={scrubCaps.enabled ? "Drag to move in time; Home goes to the start, Alt+arrow to the next or previous event" : scrubCaps.why}
          data-testid="scrub-range"
          onPointerDown={() => {
            scrubRef.current = nowNs;
            setScrubNs(nowNs);
          }}
          onKeyDown={(e) => {
            // Alt+arrow jumps between events, the keyboard's way to the ticks a pointer clicks.
            if (e.altKey && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
              e.preventDefault();
              jumpEvent(e.key === "ArrowRight" ? 1 : -1);
              return;
            }
            // Arrow/Home/End move the thumb; the seek waits for the key to come back up, so
            // holding an arrow down is still one call.
            if (e.key.startsWith("Arrow") || e.key === "Home" || e.key === "End" || e.key === "PageUp" || e.key === "PageDown") {
              if (scrubRef.current === null) {
                scrubRef.current = nowNs;
                setScrubNs(nowNs);
              }
            }
          }}
          onKeyUp={() => {
            if (scrubRef.current !== null) void commitScrub();
          }}
          onChange={(e) => {
            const value = Number(e.target.value);
            if (scrubRef.current !== null) {
              scrubRef.current = value;
              setScrubNs(value);
            } else {
              // No gesture in flight (a programmatic change, or a click on the track that the
              // browser reported without a pointerdown): commit it directly.
              seekTo(value);
            }
          }}
          onBlur={() => {
            if (scrubRef.current !== null) void commitScrub();
          }}
        />
        {/*
          The events, above the range input so a pointer reaches them: a tick per event, which jumps
          there when clicked. Out of the tab order (a long run has hundreds); the keyboard's way to
          them is Alt+arrow on the timeline, announced by `aria-keyshortcuts`.
        */}
        <div className="scrub-events" data-testid="timeline-events">
          {planned.map((p) => (
            <button
              key={`scenario-${p.index}`}
              type="button"
              tabIndex={-1}
              className="scenario-mark"
              data-testid="scenario-event-mark"
              data-kind={p.type}
              data-fired={p.fired ? "true" : "false"}
              style={{ left: `${p.left}%` }}
              title={p.title}
              aria-label={p.title}
              disabled={!scrubCaps.enabled}
              onClick={() => seekTo(p.tNs)}
            />
          ))}
          {marks.map((m) => (
            <button
              key={`${m.channel}-${m.left}`}
              type="button"
              tabIndex={-1}
              className="mark"
              data-testid="timeline-mark"
              data-channel={m.channel}
              style={{ left: `${m.left}%`, background: MARK_COLOR[m.channel] ?? "var(--accent)" }}
              title={`${CHANNEL_LABEL[m.channel] ?? m.channel} at ${simClock(m.tNs)} — ${m.label}`}
              aria-label={`${CHANNEL_LABEL[m.channel] ?? m.channel} at ${simClock(m.tNs)}`}
              disabled={!scrubCaps.enabled}
              onClick={() => seekTo(m.tNs)}
            />
          ))}
        </div>
      </div>

      <span className="dim tb-end" data-testid="time-state" title={drivingReplay ? "The recording's own span" : `This run covers ${durationNs(endNs)}. ${status.headline}`}>
        {durationNs(endNs)}
      </span>
      {syncB ? (
        <span className="chip" data-testid="sync-chip" title="A scrub moves both runs; see the Compare panel">
          B synced{compareSync.offsetNs === 0 ? "" : ` ${(compareSync.offsetNs / 1e9).toFixed(1)} s`}
        </span>
      ) : null}

      {seekProgress !== null ? (
        <div className="timebar-notice" role="status" data-testid="seek-progress">
          <progress value={seekProgress.progress} max={1} aria-label="Simulating ahead to the seek target" />{" "}
          {seekProgress.message !== ""
            ? seekProgress.message.replace(/^simulating/, "Simulating")
            : `Simulating ahead: ${Math.round(seekProgress.progress * 100)} %`}
        </div>
      ) : null}
      {notice ? (
        <div className="timebar-notice" role="status" data-testid="time-notice">
          {notice}
          <button type="button" className="linklike" onClick={() => setNotice(null)} aria-label="Dismiss">
            ✕
          </button>
        </div>
      ) : null}
    </div>
  );
}
