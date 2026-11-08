# `python/v2xw/`

The `v2xw` Python package. Read `../README.md` for what it does and how to build it; this
directory holds:

| File | What is in it |
|---|---|
| `__init__.py` | the package surface, and the one wall-clock read in the whole system |
| `plugins.py` | `CarFollowing`, `Detector` and `MaPolicy` to subclass, and `card()`/`parameter()`/`equation()`/`source()` to build a model card that validates |
| `conformance.py` | the Python half of the plug-in conformance suite: the purity guard and the source scan |
| `_v2xw.pyi` | typed stubs for the native module |
| `py.typed` | the marker that tells a type checker the stubs are authoritative |

The native module is built from `../../crates/v2xw-py` and installed here as `_v2xw` by
maturin. It is not in version control.
