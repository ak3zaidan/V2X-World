/**
 * What the engine said when it refused a call, in words a user can act on.
 *
 * The page used to show a refusal as the JSON-RPC message alone — "run.start failed: scenario
 * invalid (-32004)" — which names neither the setting nor the fix, although the engine sends both
 * (`data.errors`, one `{path, message, hint}` row per problem). These helpers read the rows out so
 * the page can say what to change and the settings window can mark the field.
 */
import { JsonRpcError, RpcErrorCode, type ValidationError } from "@vwp/protocol";

/** The rows a refusal carries: `-32004`'s `data.errors`, or `-32602`'s `data` array. */
export function refusedRows(err: unknown): ValidationError[] | null {
  if (!(err instanceof JsonRpcError)) return null;
  if (err.code !== RpcErrorCode.SCENARIO_INVALID && err.code !== RpcErrorCode.INVALID_PARAMS) return null;
  const data = err.data;
  const list: unknown = Array.isArray(data)
    ? data
    : data !== null && typeof data === "object"
      ? (data as { errors?: unknown }).errors
      : null;
  if (!Array.isArray(list)) return null;
  const rows = list.filter(
    (r): r is ValidationError =>
      r !== null && typeof r === "object" && typeof (r as ValidationError).path === "string" && typeof (r as ValidationError).message === "string",
  );
  return rows.length > 0 ? rows : null;
}

/** One row as a sentence: the setting, what is wrong, and the hint when it adds one. */
function rowText(r: ValidationError): string {
  const dotted = r.path.replace(/^\//, "").split("/").join(".");
  // The engine's message usually starts with the setting's own dotted name already.
  const message = dotted !== "" && r.message.startsWith(dotted) ? r.message : `${dotted === "" ? "the scenario" : dotted}: ${r.message}`;
  return r.hint && !/see the field's help text/.test(r.hint) ? `${message} (${r.hint})` : message;
}

/**
 * A caught error as the sentence the page shows.
 *
 * A refusal names each setting; an internal error says it is the engine's fault and not the
 * user's; anything else keeps the engine's own words without the method-and-code wrapping.
 */
export function describeError(err: unknown): string {
  const rows = refusedRows(err);
  if (rows !== null) {
    const n = rows.length;
    const head = `The engine refused ${n === 1 ? "a setting" : `${n} settings`}`;
    return `${head}: ${rows.map(rowText).join("; ")}.`;
  }
  if (err instanceof JsonRpcError) {
    // The server's message without the "<method> failed: … (<code>)" frame JsonRpcError adds.
    const inner = err.message.replace(/^\S+ failed: /, "").replace(/ \(-?\d+\)$/, "");
    if (err.code === RpcErrorCode.INTERNAL_ERROR) {
      return `The engine failed while doing ${err.method} — this is a fault in the engine, not in your settings: ${inner.replace(/^internal error: /, "")}`;
    }
    return inner;
  }
  return err instanceof Error ? err.message : String(err);
}
