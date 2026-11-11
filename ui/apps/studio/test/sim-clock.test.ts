/**
 * `simClock`'s millisecond field is the instant's own millisecond, not one less.
 *
 * Taken from the seconds as a float (`(t / 1e9) % 1 * 1000`), 1.001 s became 0.000999… s and read
 * `00:00:01.000`; about half of all whole-millisecond instants under 100 s read one millisecond
 * early on the scrub bar, the event markers and the message feed.
 */

import { describe, expect, it } from "vitest";

import { simClock } from "../src/lib/format.js";

describe("simClock milliseconds", () => {
  it("prints the millisecond the instant is at", () => {
    expect(simClock(1_001_000_000)).toBe("00:00:01.001");
    expect(simClock(1_003_000_000)).toBe("00:00:01.003");
    expect(simClock(61_999_000_000)).toBe("00:01:01.999");
  });

  it("agrees with integer arithmetic for every whole millisecond of the first 100 s", () => {
    for (let ms = 0; ms < 100_000; ms++) {
      const want = String(ms % 1000).padStart(3, "0");
      const got = simClock(ms * 1_000_000).slice(-3);
      if (got !== want) throw new Error(`${ms} ms read .${got}`);
    }
  });
});
