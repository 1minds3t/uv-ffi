# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.8.post1] — 2026-03-15

Persistent Engine

This release transforms uv-ffi into a high-performance persistent execution engine for uv's package resolver and installer. Designed for long-running processes, daemons, notebooks, and interactive tools.

## Performance (warm cache, Linux NVMe, Python 3.11)

| Operation | uv CLI | uv-ffi | Speedup |
|:--|--:|--:|--:|
| No-op / satisfied check | ~11–12ms | ~0.4–2ms | ~6–8× |
| Real swap (install different version) | ~17–20ms | ~5.4–6.5ms | ~2.5–3× |
| Site-packages delta patch | ~2.5ms (full rescan) | ~25µs | ~100× |

The ~5–6ms floor is the hardware limit for VFS symlink create/unlink on NVMe. All software overhead above that floor is eliminated.

## What's new

**Persistent `UvEngine` singleton**
Previously uv-ffi paid the full uv startup cost on every call — interpreter discovery, platform tagging, HTTP pool init, TLS setup, logging init. Now this happens once at import time and is held in a `OnceLock`. Every subsequent call skips directly to resolution.

**Zero-clap fast path**
Common `pip install` commands now bypass clap argument parsing entirely. Internal Rust structs are constructed directly, saving ~2ms per call.

**Persistent `RegistryClient`**
The HTTP client lives for the lifetime of the engine — eliminating ~1–2ms socket/TLS teardown per call.

**Delta cache patching**
`patch_site_packages_cache(installed, removed)` updates the in-memory site-packages map surgically in ~25µs — ~100× faster than uv's own ~2.5ms full rescan. Use this with an FS watcher to maintain coherency after external changes at near-zero cost.

**Daemon-safe internals**
Tokio runtime, logging, and uv globals are now idempotent. No re-init panics or resource leaks in long-running host processes.

**Full dist-info compatibility**
uv-ffi installs now write complete dist-info including `RECORD`, `INSTALLER`, and `REQUESTED` — verified compatible with `uv pip uninstall` and standard Python toolchain operations.

## Cache coherency — important

uv-ffi trusts its in-memory cache completely. If an external tool modifies the environment without notifying uv-ffi, the next call may silently return rc=0 with no action (false no-op).

Verified behavior with external interference:
```
[1] uv-ffi swap:                   6.5ms   ✓ inst=[('rich','14.3.3')]
[2] external uv pip install:      18.7ms   disk changed behind our back
[3] uv-ffi (cache stale):          0.5ms   ✗ silent no-op — wrong answer
[4] uv-ffi (different target):    11.4ms   ✓ rescan detected mismatch, corrected
```

**Solutions:**
- **~25µs** — `patch_site_packages_cache(installed, removed)` via FS watcher (omnipkg's approach)
- **~2.5ms** — `invalidate_site_packages_cache()` forced rescan before each call
- **Nothing needed** — if your process is the only writer

## Scope

This release optimizes `pip install` on the fast path. Commands like `uv pip uninstall`, `uv run`, `uv sync` fall back to the standard clap path (~15–20ms). Bringing these into the zero-overhead path is the next focus.

## Who this is for

- Daemon and background service authors who need in-process package management
- Notebook and REPL users who want instant dependency swaps
- Tool builders who want uv's resolver speed without CLI overhead

This engine powers omnipkg's daemon, where sub-10ms environment swaps and real-time FS coherence are table stakes.

---

`pip install uv-ffi==0.10.8.post1`

Thank you to the uv team for the foundation this is built on.

---

**📝 Code Changes:**
- UPDATE: crates/uv-cache/src/cli.rs (2 lines changed)
- UPDATE: crates/uv-cli/src/compat.rs (1 lines changed)
- UPDATE: crates/uv-cli/src/lib.rs (138 lines changed)
- UPDATE: crates/uv-ffi/src/lib.rs (612 lines changed)
- UPDATE: crates/uv-installer/src/site_packages.rs (8 lines changed)
- UPDATE: crates/uv/src/commands/mod.rs (2 lines changed)
- UPDATE: crates/uv/src/commands/pip/install.rs (130 lines changed)
- UPDATE: crates/uv/src/commands/pip/mod.rs (2 lines changed)
- UPDATE: crates/uv/src/commands/pip/operations.rs (19 lines changed)
- UPDATE: crates/uv/src/lib.rs (102 lines changed)
- UPDATE: crates/uv/src/logging.rs (19 lines changed)

**📚 Documentation:**
- CHANGELOG.md (64 lines)
- crates/uv-ffi/CHANGELOG.md (114 lines)
- crates/uv-ffi/README.md (152 lines)

**⚙️ Configuration:**
- crates/uv-ffi/Cargo.toml (16 lines)
- crates/uv-ffi/pyproject.toml (24 lines)
- pyproject.toml

**Additional Changes:**
- docs: Prepare changelog for release.
- docs: Restore UV's changelog.
- docs: clarify cache coherency model and FS-scan speedup; fix Cargo version
- feat: uv-ffi v0.10.8.post1 — persistent in-process engine & cache coherency
- Bump toml to prepare for release.
- perf(uv-ffi): persistent UvEngine runtime for daemon execution

**Updates:**
- Update publish.yml

_18 files changed, 1394 insertions(+), 282 deletions(-)_
