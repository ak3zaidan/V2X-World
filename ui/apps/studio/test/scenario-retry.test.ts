/**
 * `scenario.get` is retried, a bounded number of times, when it fails.
 *
 * The Studio asked for the scenario once, at connect, and never again: an engine that answered the
 * socket before it could answer `scenario.get` (still loading its world, or a proxy that came up
 * first) left the page with no scenario document, no settings form and no quick settings for the
 * whole connection. The request is now retried on a backoff, gives up after the last delay with one
 * log line, is abandoned when a newer request or a disconnect supersedes it, and is asked again on a
 * resumed reconnect if it never succeeded.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { HelloFlags, type HelloMessage } from "@vwp/protocol";

import { StudioEngine } from "../src/state/engine.js";
import { useStudio } from "../src/state/store.js";

const ANSWER = {
  scenario: { name: "grid" },
  hash: "abc",
  schema: null,
  fields: [],
  statuses: [],
  groups: [],
};

interface Harness {
  engine: StudioEngine;
  calls: string[];
  waits: number[];
}

/** An engine whose `scenario.get` fails `failures` times, then answers; waits resolve at once. */
function harness(failures: number): Harness {
  const engine = new StudioEngine();
  const calls: string[] = [];
  const waits: number[] = [];
  let left = failures;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (engine as any).request = async (method: string): Promise<unknown> => {
    calls.push(method);
    if (method === "scenario.get") {
      if (left > 0) {
        left--;
        throw new Error("engine not ready");
      }
      return ANSWER;
    }
    if (method === "scenario.list") return { items: [] };
    throw new Error(`unexpected ${method}`);
  };
  engine.sleep = async (ms: number) => {
    waits.push(ms);
  };
  return { engine, calls, waits };
}

const gets = (h: Harness): number => h.calls.filter((c) => c === "scenario.get").length;

describe("scenario.get retries until the engine answers", () => {
  beforeEach(() => {
    useStudio.setState({ scenario: null, logs: [] });
  });

  it("gets the scenario when the first answers are failures", async () => {
    const h = harness(3);
    await h.engine.refreshScenario();
    expect(gets(h)).toBe(4);
    expect(h.waits).toEqual([500, 1000, 2000]);
    expect(useStudio.getState().scenario).toEqual({ name: "grid" });
    // A retry that succeeded is not the user's problem.
    expect(useStudio.getState().logs.filter((l) => l.target === "scenario")).toEqual([]);
  });

  it("asks once when the first answer is good", async () => {
    const h = harness(0);
    await h.engine.refreshScenario();
    expect(gets(h)).toBe(1);
    expect(h.waits).toEqual([]);
  });

  it("gives up after the last delay, with one log line", async () => {
    const h = harness(Number.POSITIVE_INFINITY);
    await h.engine.refreshScenario();
    expect(gets(h)).toBe(1 + h.engine.scenarioRetryDelaysMs.length);
    expect(h.waits).toEqual([...h.engine.scenarioRetryDelaysMs]);
    expect(useStudio.getState().scenario).toBeNull();
    const lines = useStudio.getState().logs.filter((l) => l.target === "scenario");
    expect(lines.length).toBe(1);
    expect(lines[0].message).toContain("6 times");
    // The scenario list is still asked for: it does not depend on the document.
    expect(h.calls.at(-1)).toBe("scenario.list");
  });

  it("stops retrying once a disconnect supersedes it", async () => {
    const h = harness(Number.POSITIVE_INFINITY);
    h.engine.sleep = async (ms: number) => {
      h.waits.push(ms);
      h.engine.disconnect();
    };
    await h.engine.refreshScenario();
    expect(gets(h)).toBe(1);
    expect(h.calls).not.toContain("scenario.list");
  });

  it("asks again on a resumed reconnect when it never got an answer", async () => {
    const h = harness(Number.POSITIVE_INFINITY);
    h.engine.scenarioRetryDelaysMs = [];
    await h.engine.refreshScenario();
    expect(gets(h)).toBe(1);
    // The engine is ready now; the socket drops and the session resumes.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (h.engine as any).request = async (method: string): Promise<unknown> => {
      h.calls.push(method);
      return method === "scenario.get" ? ANSWER : { items: [] };
    };
    await h.engine.afterLaterHello({ helloFlags: HelloFlags.RESUMED, resumeSeq: 7n } as unknown as HelloMessage);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(gets(h)).toBe(2);
    expect(useStudio.getState().scenario).toEqual({ name: "grid" });
  });
});
