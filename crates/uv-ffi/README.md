# uv-ffi

> ⚠️ This package is not intended for direct use. It exists solely as a runtime dependency of [omnipkg](https://github.com/1minds3t/omnipkg).

In-process Python FFI bindings to uv's package resolver core. Rather than spawning a uv subprocess, omnipkg calls uv's install/resolve logic directly via FFI inside a pre-warmed daemon worker — eliminating binary startup cost entirely.

## Performance

| Method | wall time | user | sys |
|--------|-----------|------|-----|
| `uv pip install` (subprocess) | ~17-20ms | 0.006s | 0.013s |
| `8pkg` via uv-ffi + daemon | ~12-16ms | 0.000s | 0.002s |
| `8pkg` (preflight cache hit) | ~4-5ms | 0.000s | 0.000s |

The gain comes entirely from eliminating binary spawn overhead (`user+sys`). uv's resolver itself is unchanged.

## How it works

- uv's private crates are linked directly (not via subprocess)
- uv's install function is called in-process, bypassing the CLI entrypoint
- omnipkg's daemon pre-loads the FFI module once at startup
- Environment and site-packages state is cached across calls
- Falls back to pip if the FFI call fails

## Attribution

This crate uses uv source code from [astral-sh/uv](https://github.com/astral-sh/uv),
copyright Astral Software Inc., used under the **MIT License**. See `NOTICE` for full attribution.

## Not affiliated with Astral

This project is not affiliated with, endorsed by, or sponsored by Astral Software Inc.
