/**
 * The long soak: one run of simulated hours with everything on, watched through the page.
 *
 * Vehicles, pedestrians, the full SCMS lifecycle (pseudonym batches, top-ups, certificate changes,
 * misbehaviour reports, revocation and CRLs), attackers, scenario events (a road closure, a demand
 * surge, a weather front) and the chase view's message feed on a followed vehicle, re-followed
 * whenever the one it was on leaves the map. The page plays it as fast as the engine goes.
 *
 * Asserted: the run reaches its end; the engine never enters `error`; the page logs no error and
 * the console shows none; the engine's resident memory and the tab's heap are flat from a warm
 * sample (a quarter of the way in) to the end.
 *
 * It runs only on purpose, because it takes a long time on a debug build:
 *
 *   VWP_SOAK_SIM_S=3600 VWP_ENGINE_BIN=… pnpm exec playwright test -c playwright.engine.config.ts soak
 */

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, REPO, open, status } from "./support.js";

const SIM_S = Number(process.env.VWP_SOAK_SIM_S ?? "0");

/** `credential-lifecycle.yaml` stretched to `seconds`, with pedestrians, events and attackers. */
function soakScenario(seconds: number): string {
  let text = readFileSync(join(REPO, "scenarios/credential-lifecycle.yaml"), "utf8");
  const swap = (from: string, to: string): void => {
    if (!text.includes(from)) throw new Error(`credential-lifecycle.yaml no longer has "${from}"`);
    text = text.replace(from, to);
  };
  swap("name: credential-lifecycle", "name: soak-everything");
  swap("duration_s: 300.0", `duration_s: ${seconds}.0`);
  swap("rate_veh_per_h: 1500.0", "rate_veh_per_h: 900.0");
  swap("        to_s: 300.0", `        to_s: ${seconds}.0`);
  swap(
    "  backend:\n    protocol: protocol/scms/camp",
    "  backend:\n    protocol: protocol/scms/camp\n  vru:\n    pedestrians: 30\n    device_fraction: 0.5",
  );
  const at = (f: number): number => Math.round(seconds * f);
  text += [
    "",
    "events:",
    `  - {t: ${at(0.2)}, until: ${at(0.35)}, type: closure, target: "edge:3"}`,
    `  - {t: ${at(0.45)}, until: ${at(0.6)}, type: demand.multiplier, value: 1.5}`,
    `  - {t: ${at(0.7)}, type: weather.front, value: rain, intensity: 0.6}`,
    "",
  ].join("\n");
  const dir = join(tmpdir(), `vwp-soak-${process.pid}`);
  mkdirSync(dir, { recursive: true });
  const path = join(dir, "soak-everything.yaml");
  writeFileSync(path, text);
  return path;
}

test.skip(SIM_S <= 0, "the long soak runs on purpose: set VWP_SOAK_SIM_S");

const engine = new EngineProcess(SIM_S > 0 ? soakScenario(SIM_S) : "", undefined, "0");

test.beforeAll(async () => {
  if (SIM_S > 0) await engine.start();
});
test.afterAll(async () => {
  await engine.stop();
});

async function heapBytes(page: Page): Promise<number> {
  return page.evaluate(() => {
    (window as unknown as { gc?: () => void }).gc?.();
    return (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize ?? 0;
  });
}

/** Follow an equipped vehicle in the chase view, with its message feed; the id, or null. */
async function followSomeone(page: Page): Promise<number | null> {
  return page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as {
      client: { poses: { count: number; occupied: Uint8Array; actorId: Uint32Array } } | null;
      nodeByActor: Map<number, number>;
      selectActor(id: number | null, mode?: string): Promise<void>;
    };
    const poses = e.client?.poses;
    if (!poses) return null;
    for (let slot = 0; slot < poses.count; slot++) {
      if (poses.occupied[slot] !== 1) continue;
      const id = poses.actorId[slot];
      if (e.nodeByActor.get(id) !== undefined) {
        void e.selectActor(id, "chase");
        return id;
      }
    }
    return null;
  });
}

/** Whether actor `id` is still live in the page's pose buffer. */
async function isLive(page: Page, id: number): Promise<boolean> {
  return page.evaluate((actor) => {
    const poses = (
      window.__vwpStudio?.engine as unknown as {
        client: { poses: { count: number; occupied: Uint8Array; actorId: Uint32Array } } | null;
      }
    ).client?.poses;
    if (!poses) return false;
    for (let slot = 0; slot < poses.count; slot++) if (poses.occupied[slot] === 1 && poses.actorId[slot] === actor) return true;
    return false;
  }, id);
}

test("an hour of everything, through the page: no crash, no leak, no console error", async ({ page }) => {
  test.setTimeout(6 * 60 * 60_000);
  const consoleErrors: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error") consoleErrors.push(m.text());
  });
  page.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));

  await open(page);
  await page.getByTestId("primary-action").click();
  await expect.poll(async () => (await status(page)).state, { timeout: 120_000 }).toMatch(/running|finished/);

  const samples: { simS: number; footprintMb: number; heapMb: number; wallS: number }[] = [];
  const started = Date.now();
  let followed: number | null = null;
  let follows = 0;
  let lastSample = -Infinity;
  for (;;) {
    const s = await status(page);
    expect(s.state, `the engine went to error at ${s.t_ns / 1e9} s`).not.toBe("error");
    if (followed === null || !(await isLive(page, followed))) {
      followed = await followSomeone(page);
      if (followed !== null) follows++;
    }
    const simS = s.t_ns / 1e9;
    if (simS - lastSample >= SIM_S / 12 || s.state === "finished") {
      // The physical footprint, not RSS, which macOS compression moves on its own (see
      // `EngineProcess.footprintKb`). Printed as it is taken, so a soak stopped early still
      // leaves its trend behind.
      const sample = { simS, footprintMb: engine.footprintKb() / 1024, heapMb: (await heapBytes(page)) / 2 ** 20, wallS: (Date.now() - started) / 1000 };
      samples.push(sample);
      console.warn(`soak sample: t=${sample.simS.toFixed(0)} s (wall ${sample.wallS.toFixed(0)} s): engine footprint ${sample.footprintMb.toFixed(1)} MB, tab heap ${sample.heapMb.toFixed(1)} MB`);
      lastSample = simS;
    }
    if (s.state === "finished") {
      // `finished` can be reported one step before the stream position reaches the horizon
      // (measured: 239.9 s of 240 s); the run's output digest is published once it has.
      await expect
        .poll(async () => (await status(page)).engine.output_digest, { timeout: 60_000 })
        .not.toBeNull();
      const end = await status(page);
      expect(end.t_ns, "the run reached its end").toBe(end.t_end_ns);
      break;
    }
    await page.waitForTimeout(5_000);
  }
  await expect(page.getByTestId("connection-state")).toHaveAttribute("data-state", "streaming");
  const pageErrors = await page.evaluate(() => {
    const hook = (window as unknown as { __vwpStudio?: { logs(): { level: string; target: string; message: string }[] } }).__vwpStudio;
    return (hook?.logs() ?? []).filter((l) => l.level === "error").map((l) => `${l.target}: ${l.message}`);
  });

  console.warn(
    `soak of ${SIM_S} simulated s, ${follows} vehicles followed in turn:\n` +
      samples.map((x) => `  t=${x.simS.toFixed(0)} s (wall ${x.wallS.toFixed(0)} s): engine footprint ${x.footprintMb.toFixed(1)} MB, tab heap ${x.heapMb.toFixed(1)} MB`).join("\n"),
  );
  expect(pageErrors, "the page logged no error").toEqual([]);
  expect(consoleErrors, "the console shows no error").toEqual([]);
  expect(follows, "a vehicle was followed with its feed").toBeGreaterThan(0);
  const warm = samples.find((x) => x.simS >= SIM_S / 4) ?? samples[0];
  const last = samples[samples.length - 1];
  expect(last.footprintMb, "engine memory is flat from a quarter of the way in to the end").toBeLessThan(warm.footprintMb * 1.3 + 50);
  expect(last.heapMb, "tab memory is flat from a quarter of the way in to the end").toBeLessThan(warm.heapMb * 1.3 + 20);
});
