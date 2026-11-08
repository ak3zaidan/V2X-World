/**
 * A read in flight when the socket drops is asked again over HTTP, not reported as an error.
 *
 * The engine end-to-end suite cuts the connection at random moments while a run plays, and the page
 * logged `runs.list: socket closed (1006)`: the run history's read was on the socket that died. The
 * link resumed with nothing missed, so the error was the page's own. A method that only reads is now
 * asked again over `POST /rpc`; one that changes something (a run start, a scenario edit) is never
 * repeated, because the engine may have acted on it before the socket went.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { SocketClosedError } from "@vwp/protocol";

import { StudioEngine } from "../src/state/engine.js";
import { useStudio } from "../src/state/store.js";

function engineWithDroppingSocket(): { engine: StudioEngine; http: string[] } {
  const engine = new StudioEngine();
  const http: string[] = [];
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (engine as any).client = {
    state: "streaming",
    request: async () => {
      throw new SocketClosedError("socket closed (1006)");
    },
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (engine as any).requestHttp = async (method: string): Promise<unknown> => {
    http.push(method);
    return { items: [] };
  };
  return { engine, http };
}

const errors = (): string[] => useStudio.getState().logs.filter((l) => l.level === "error").map((l) => l.message);

describe("a request on a socket that drops", () => {
  beforeEach(() => {
    useStudio.setState({ logs: [] });
  });

  it("a read is answered over HTTP, with no error", async () => {
    const { engine, http } = engineWithDroppingSocket();
    await expect(engine.request("runs.list", {})).resolves.toEqual({ items: [] });
    expect(http).toEqual(["runs.list"]);
    expect(errors()).toEqual([]);
  });

  it("a change is not repeated, and the drop is reported", async () => {
    const { engine, http } = engineWithDroppingSocket();
    await expect(engine.request("run.start", {})).rejects.toThrow("socket closed (1006)");
    expect(http).toEqual([]);
    expect(errors()).toEqual(["run.start: socket closed (1006)"]);
  });

  it("any other failure of a read is the engine's answer, not asked again", async () => {
    const { engine, http } = engineWithDroppingSocket();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (engine as any).client.request = async () => {
      throw new Error("runs.list timed out after 30000 ms");
    };
    await expect(engine.request("runs.list", {})).rejects.toThrow("timed out");
    expect(http).toEqual([]);
  });
});
