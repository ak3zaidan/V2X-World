#!/usr/bin/env python3
"""The pycrate side of a J2735 oracle for any PDU the compiled modules hold.

Reads a vector file whose entries are::

    {"name": ..., "pdu": "PersonalSafetyMessage", "value": {...}, "rust_hex": "...",
     "frame_id": 32, "rust_frame_hex": "..."}

and, for each, (1) encodes the value with pycrate and compares the octets with Rust's,
(2) decodes Rust's octets with pycrate and compares the value, and (3) does the same for
the ``MessageFrame`` wrapper when ``frame_id`` is given. Values use the markers of
``oracle.py``: ``__hex__``, ``__bits__``, ``__open__``.

Usage: ``generic_oracle.py <vectors.json> <results.json>``.
"""

from __future__ import annotations

import json
import os
import sys
import traceback

sys.path.insert(0, os.environ.get("V2XW_J2735_ORACLE_DIR", os.path.dirname(os.path.abspath(__file__))))

import j2735_all  # noqa: E402


def load(type_name: str):
    """A compiled type, from whichever module (2016 ``DSRC`` or a 2024 rebuild) has it."""
    for name in ("DSRC", type_name):
        module = getattr(j2735_all, name, None)
        if module is not None and hasattr(module, type_name):
            return getattr(module, type_name)
    for name in dir(j2735_all):
        module = getattr(j2735_all, name)
        if isinstance(module, type) and hasattr(module, "_name_") and hasattr(module, type_name):
            return getattr(module, type_name)
    raise ImportError(f"no compiled module holds {type_name}")


def to_py(value):
    if isinstance(value, dict):
        if "__hex__" in value:
            return bytes.fromhex(value["__hex__"])
        if "__bits__" in value:
            bits, length = value["__bits__"]
            return (bits, length)
        if "__open__" in value:
            name, inner = value["__open__"]
            return (name, to_py(inner))
        return {k: to_py(v) for k, v in value.items()}
    if isinstance(value, list):
        return [to_py(v) for v in value]
    return value


def canon(value):
    if isinstance(value, (bytes, bytearray)):
        return {"__hex__": bytes(value).hex()}
    if isinstance(value, dict):
        return {k: canon(v) for k, v in value.items()}
    if isinstance(value, tuple):
        if len(value) == 2 and isinstance(value[0], int) and isinstance(value[1], int):
            return {"__bits__": [value[0], value[1]]}
        if len(value) == 2 and isinstance(value[0], str):
            return {"__open__": [value[0], canon(value[1])]}
        return [canon(v) for v in value]
    if isinstance(value, list):
        return [canon(v) for v in value]
    return value


def run_vector(vector):
    pdu = load(vector["pdu"])
    result = {"name": vector["name"], "pdu": vector["pdu"]}
    pdu.set_val(to_py(vector["value"]))
    result["py_hex"] = pdu.to_uper().hex()
    result["encode_match"] = result["py_hex"] == vector["rust_hex"]
    pdu.from_uper(bytes.fromhex(vector["rust_hex"]))
    decoded = canon(pdu.get_val())
    result["py_decoded"] = decoded
    result["decode_match"] = decoded == vector["value"]
    if "frame_id" in vector:
        frame = load("MessageFrame")
        frame.set_val({"messageId": vector["frame_id"],
                       "value": (vector["pdu"], to_py(vector["value"]))})
        result["py_frame_hex"] = frame.to_uper().hex()
        result["frame_match"] = result["py_frame_hex"] == vector["rust_frame_hex"]
    return result


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    with open(sys.argv[1], encoding="utf-8") as handle:
        vectors = json.load(handle)
    results = []
    for vector in vectors:
        try:
            results.append(run_vector(vector))
        except Exception as exc:  # noqa: BLE001 - the report is the point
            results.append({"name": vector["name"], "error": f"{type(exc).__name__}: {exc}",
                            "traceback": traceback.format_exc(limit=4)})
    with open(sys.argv[2], "w", encoding="utf-8") as handle:
        json.dump({"pycrate_results": results}, handle)
    failed = [r for r in results if r.get("error") or not all(
        r.get(k, True) for k in ("encode_match", "decode_match", "frame_match"))]
    print(f"{len(results) - len(failed)}/{len(results)} vectors matched pycrate")
    for bad in failed[:5]:
        print(f"  MISMATCH {bad['name']}: {json.dumps({k: v for k, v in bad.items() if k != 'py_decoded'})[:400]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
