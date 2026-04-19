# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.8.post5] — 2026-04-18

Hardened CI Orchestration & Per-Platform PyPI Validation

This release adds two runtime fixes to uv-ffi: automatic creation of the uv cache directory when missing (previously caused a panic on first run in clean environments), and restored Python-side version detection so importing uv_ffi and checking __version__ now returns the correct version string instead of failing silently.

The bulk of commits in this release are CI/CD and wheel build pipeline improvements — per-platform PyPI existence checks, hardened multi-run artifact orchestration, sdist publishing, and Windows PowerShell compatibility fixes. These do not affect runtime behavior.

---

**Bug Fixes:**
- fix: wheel failures non-fatal so sdist can still publish, remove duplicate function
- fix: download sdist artifact, check completeness for both wheels and sdist
- fix: increase dispatch run detection wait time and attempts
- fix: platform-level PyPI check for jobs without python matrix
- fix: ignore missing artifacts, fail only if truly no wheels found from either run
- fix: per-wheel PyPI check for all extended platform jobs
- fix: keep dots in wheel version tag
- fix: use single quotes for powershell regex to avoid expansion conflict
- fix: powershell pyver string concatenation syntax
- fix: per-wheel PyPI check for all platforms instead of version-only check
- fix: use passed tag for PyPI version check, fall back to pyproject.toml
- fix: track dispatched run by timestamp to avoid version collision
- fix: read version from pyproject.toml instead of Cargo.toml for PyPI check
- fix: replace em-dashes with hyphens in PowerShell steps

**Updates:**
- Update publish.yml
- Update publish.yml to find successful runs by tag SHA
- Update GitHub Actions to handle multiple build runs
- Update build-wheels-extended.yml
- Update interpreter arguments for wheel builds
- Update build-wheels.yml

**Other Changes:**
- Change shell from PowerShell to Bash for PyPI check
- Fix indentation for PyPI check step in workflow
- Improve PyPI check for existing wheel in workflow
- Change shell to PowerShell for PyPI check
- Fix syntax error in if condition for version check
- ...and 21 more changes

_4 files changed, 763 insertions(+), 66 deletions(-)_

## [0.10.8.post4] — 2026-04-12

enforce target isolation for bubble installs and prevent cache poisoning

This release unlocks ultra-fast, in-process target installations by completely isolating the `uv` daemon's caching engine. Previously, `--target` installs (bubbles) would poison the main environment's in-memory state, forcing Omnipkg to fall back to slow subprocesses.

**Core Changes:**
* **FFI Target Support:** Added `--target` routing directly into the `FfiInstallOpts` and `run_pip_install_direct` fast path.
* **Dual-State Environments:** Introduced `BUBBLE_ENVIRONMENT` (a `OnceLock` pre-warmed empty state) so the resolver treats target directories as a clean slate without cross-contaminating the main interpreter.
* **Cache Protection:** Implemented the `BUBBLE_INSTALL` atomic flag. Target installs now completely bypass `SITE_PACKAGES_CACHE` reads/writes and safely drain the `INSTALL_CHANGELOG` without mutating global statics.
* **Cleanup:** Removed deprecated `.bak` files and bumped TOML parser dependencies.

**Impact:** Omnipkg can now generate isolated multiversion bubbles entirely in-memory using the Rust FFI, dropping bubble generation overhead to milliseconds while keeping the main environment completely safe.

---

**📝 Code Changes:**
- UPDATE: crates/uv-ffi/src/lib.rs (59 lines changed)
- UPDATE: crates/uv-python/fetch-download-metadata.py (2 lines changed)
- UPDATE: crates/uv/src/commands/pip/install.rs (27 lines changed)
- UPDATE: crates/uv/src/lib.rs (2 lines changed)

**⚙️ Configuration:**
- crates/uv-ffi/pyproject.toml (2 lines)

**Additional Changes:**
- chore: bump toml
- fix: enforce target isolation for bubble installs and prevent cache poisoning

_6 files changed, 74 insertions(+), 73 deletions(-)_

## [0.10.8.post3] — 2026-04-11

Windows ARM64 support & cache path fix

## What's Changed

Fixed a Tokio runtime panic on Windows caused by incorrect uv cache directory
resolution. The old code fell through to HOME\.cache\uv which doesn't exist on
Windows, causing a PathError that hung the daemon UV worker.

Now correctly resolves to %LOCALAPPDATA%\uv\cache on Windows.

Added wheel builds for Windows ARM64 (Python 3.11–3.14) via windows-11-arm runner.

- Linux: x86_64 + aarch64, Python 3.8–3.14
- macOS: Intel + Apple Silicon, Python 3.8–3.14
- Windows x64: Python 3.8–3.14
- Windows ARM64: Python 3.11–3.14 (new)

---

**⚙️ Configuration:**
- crates/uv-ffi/pyproject.toml (2 lines)

**Updates:**
- Update Python versions in build-wheels.yml

_3 files changed, 45 insertions(+), 10 deletions(-)_

## [0.10.8.post2] — 2026-03-28

Windows temp dir hotfix — replace hardcoded /tmp/uv with platform-safe fallback

## What's Fixed

Windows CI runners don't set `HOME`, causing uv-ffi to fall through to the
hardcoded `/tmp/uv` path which doesn't exist on Windows. This produced a
panic at `lib.rs:49` on every FFI call:

    UvEngine: interpreter query failed: Io(Custom { kind: NotFound,
      error: PathError { path: "D:/tmp/uv\\.tmpXXXXXX" ... } })

The cache directory resolution now checks in order:
1. `UV_CACHE_DIR` env var (explicit override)
2. `HOME` (Unix)
3. `USERPROFILE` (Windows home)
4. `LOCALAPPDATA` (Windows fallback)
5. `std::env::temp_dir()` (guaranteed valid on all platforms)

## Who is affected

Anyone running uv-ffi on Windows — including GitHub Actions runners.
All Unix platforms are unaffected.

## Upgrade

    pip install --upgrade uv-ffi

---

**📝 Code Changes:**
- UPDATE: crates/uv-ffi/src/lib.rs (25 lines changed)

_1 file changed, 23 insertions(+), 2 deletions(-)_

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
