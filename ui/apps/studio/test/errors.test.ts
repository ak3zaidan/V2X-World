/**
 * `lib/errors.ts`: an engine refusal reads as the setting and the fix, never as a code.
 *
 * The QA of 2026-09-24 found five settings whose refusal reached the page as
 * "run.start failed: internal error: … (-32603)" or "scenario invalid (-32004)". The engine now
 * refuses them with `-32004` and a row naming the field; these tests pin that the page turns the
 * row into a sentence and keeps the "internal" wording for what really is the engine's fault.
 */

import { describe, expect, it } from "vitest";
import { JsonRpcError } from "@vwp/protocol";

import { describeError, refusedRows } from "../src/lib/errors.js";

const refusal = new JsonRpcError("run.start", {
  code: -32004,
  message: "scenario invalid",
  data: {
    errors: [
      {
        path: "/world/source/path",
        message:
          "world.source.path: names the map worlds/x.osm.xml, which does not exist. Put the OpenStreetMap extract there, or point world.source.path at one that exists",
        hint: "see the field's help text for its allowed values",
      },
    ],
  },
});

describe("describeError", () => {
  it("names the setting and the fix for a -32004 refusal", () => {
    const text = describeError(refusal);
    expect(text).toContain("The engine refused a setting");
    expect(text).toContain("world.source.path");
    expect(text).toContain("point world.source.path at one that exists");
    expect(text).not.toMatch(/-32004|scenario invalid|internal error/);
    // The generic hint adds nothing and is left out.
    expect(text).not.toContain("help text");
  });

  it("counts several rows and prefixes a row that does not name its field", () => {
    const err = new JsonRpcError("scenario.set", {
      code: -32602,
      message: "invalid params",
      data: [
        { path: "/patch/0/path", message: "required", hint: "a JSON Pointer" },
        { path: "/seed", message: "`x` is not a seed" },
      ],
    });
    const text = describeError(err);
    expect(text).toContain("refused 2 settings");
    expect(text).toContain("patch.0.path: required (a JSON Pointer)");
    expect(text).toContain("seed: `x` is not a seed");
  });

  it("says an internal error is the engine's fault, without the wire code", () => {
    const err = new JsonRpcError("run.step", { code: -32603, message: "internal error: the kernel stopped" });
    const text = describeError(err);
    expect(text).toContain("fault in the engine, not in your settings");
    expect(text).toContain("the kernel stopped");
    expect(text).not.toContain("-32603");
  });

  it("keeps the engine's words for any other refusal", () => {
    const err = new JsonRpcError("run.pause", { code: -32002, message: "run not running: state is finished" });
    expect(describeError(err)).toBe("run not running: state is finished");
    expect(describeError(new Error("socket closed"))).toBe("socket closed");
    expect(describeError("plain")).toBe("plain");
  });
});

describe("refusedRows", () => {
  it("reads -32004's data.errors and -32602's data array, and nothing else", () => {
    expect(refusedRows(refusal)?.map((r) => r.path)).toEqual(["/world/source/path"]);
    expect(refusedRows(new JsonRpcError("x", { code: -32602, message: "m", data: [{ path: "/a", message: "b" }] }))).toHaveLength(1);
    expect(refusedRows(new JsonRpcError("x", { code: -32603, message: "m", data: { errors: [{ path: "/a", message: "b" }] } }))).toBeNull();
    expect(refusedRows(new JsonRpcError("x", { code: -32004, message: "m" }))).toBeNull();
    expect(refusedRows(new Error("x"))).toBeNull();
  });
});
