# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.8.post13] — 2026-05-09

abi3 feature flag + cp37 support + piwheels memory fix

This release brings official support for Python 3.7 (End-Of-Life) without impacting modern Python environments, introduces a new dedicated domain for extended/exotic wheels, and ships critical memory optimizations to prevent Out-Of-Memory (OOM) errors on constrained 32-bit build systems like PiWheels.

* **Elegant Python 3.7 Backwards Compatibility:**
  * Converted PyO3 `abi3-py38` from a hard requirement to a default-enabled Cargo feature.
  * Python >= 3.8 continues to use the highly compatible `abi3` single-wheel target.
  * Python 3.7 drops the `abi3` feature during source builds (`--no-default-features`), gracefully falling back to a version-specific (`cp37-cp37m`) extension.
  * Lowered `requires-python` in `pyproject.toml` to `>=3.7` and added the appropriate PyPI classifiers.
* **Proactive Source Build Fallback Guides:** Updated the `build.rs` script to output a highly visible ASCII-art warning when a source build fails, immediately directing users to the pre-built `exotic-wheels` index.
* **New Dedicated Exotic Wheel Index:** Migrated extended wheel hosting from the personal GitHub Pages URL to a new dedicated domain: `https://exotic-wheels.github.io/`.

* **Resolved 32-bit ARM OOM Crashes:** Fixed a critical issue where compiling on Raspberry Pi build nodes would crash LLVM due to memory exhaustion.
* **Workspace Profile Inheritance:** Moved the `[profile.release]` block out of `crates/uv-ffi/Cargo.toml` and into the root workspace `Cargo.toml` so Maturin correctly applies it.
* **Aggressive Size/Memory Tuning:** Changed optimization levels from `s` to `z` (most aggressive size/memory optimization) and fine-tuned `codegen-units` to heavily restrict peak RAM usage per thread.

* **Automated Wheel Indexing (`build_index.py`):** Added a new Python script that dynamically fetches release assets from PyPI and GitHub Releases, seamlessly generating a static HTML index for `pip` to parse via `--extra-index-url` or `-f`.
* **Extended `cp37` CI Matrices:** Added dedicated `manylinux_2_17` jobs to automatically build CPython 3.7 wheels across `x86_64`, `aarch64`, `i686`, `armv7`, and `ppc64le`.
* **Toolchain Pinning:** Pinned the Rust toolchain to `1.77.0` in the extended build workflows to ensure maximum compatibility with older legacy targets.
* **Workflow Tag Matching:** Improved GitHub Actions tag matching logic to gracefully handle blank-tag manual workflow dispatches.

---

**📝 Code Changes:**
- UPDATE: crates/uv-ffi/build.rs (15 lines changed)

**⚙️ Configuration:**
- Cargo.toml (4 lines)
- crates/uv-ffi/Cargo.toml (13 lines)
- crates/uv-ffi/pyproject.toml (5 lines)

**Updates:**
- Update tag matching logic in publish workflow

_13 files changed, 171 insertions(+), 28 deletions(-)_

## [0.10.8.post12] — 2026-04-26

The "PiWheels Survival & Global Index" Release

This release brings massive improvements to memory-constrained builds (solving the dreaded 32-bit LLVM Out-Of-Memory crashes) and fully launches our PEP 503-compliant Extended Wheel Index on GitHub Pages.

*   **Fixed LLVM OOMs on 32-bit architectures:** Applied a strict memory-safe `[profile.release]` to the workspace root (where Cargo actually respects it).
*   **Disabled LTO & Chunked Codegen:** Set `lto = false` and `codegen-units = 16` to prevent the compiler from exhausting the 3GB 32-bit address space during the final linking phase.
*   **Binary Shrinkage:** Added `opt-level = "s"`, `panic = "abort"`, and `strip = true`. This drops peak RSS RAM usage during compilation by gigabytes and shrinks the final `.so` binary size from ~55MB down to <15MB.

*   **Fully Functional PEP 503 Index:** The CI now generates a static, pip-compatible HTML index and deploys it directly to GitHub Pages (`https://1minds3t.github.io/uv-ffi/`).
*   **Fixed API Pagination (The Ghost Asset Bug):** Refactored the `backfill_wheels.yml` script to properly paginate through the GitHub Releases API. All 600+ exotic wheels are now correctly discovered and indexed (bypassing the 100-item hard limit).
*   **macOS Optimization:** Filtered out redundant macOS `x86_64` and `arm64` wheels from the index in favor of the `universal2` binaries to reduce index bloat.

*   **Conda-Forge Recipe Added:** Shipped the initial `meta.yaml` to prepare `uv-ffi` for official distribution on the `conda-forge` channel.
*   **Smarter Build Fallbacks:** Updated the `build.rs` compile-time warnings and `README.md` to explicitly map out which architectures are served from PyPI (Mainstream ABI3) vs. our Extended Index (musllinux, armv7, riscv64, s390x, ppc64le, PyPy, 3.13t).

---

**📝 Code Changes:**
- UPDATE: crates/uv-ffi/build.rs (10 lines changed)

**⚙️ Configuration:**
- Cargo.toml (7 lines)
- crates/uv-ffi/Cargo.toml (8 lines)
- crates/uv-ffi/pyproject.toml (2 lines)

**Additional Changes:**
- chore: Update configuration
- chore: apply strict memory-safe release profile to workspace root for PiWheels
- chore: optimize release profiles to fix ARM32 OOMs and reduce binary size

**Updates:**
- Update README with link to browse all wheels
- Update backfill_wheels.yml to exclude specific wheels

_8 files changed, 147 insertions(+), 67 deletions(-)_

## [0.10.8.post11] — 2026-04-24

PiWheels and ARM Optimization

v0.10.8.post11
This release focuses on CI/CD infrastructure improvements and build reliability fixes, particularly for resource-constrained build environments like piwheels.
Build Fixes
	•	Added lower memory build settings to Cargo.toml to prevent OOM crashes on piwheels for Python 3.13 aarch64 builds
CI/CD Overhaul
	•	Completely refactored the publish workflow — wheels now go to both GitHub Releases and PyPI
	•	Added new publish-collect.yml workflow that waits for all three platform builds to succeed before publishing, preventing partial releases
	•	Improved backfill workflow to target both GitHub Releases and PyPI with better artifact handling
	•	Switched from PYPI_API_TOKEN to OMNIPKG_DEPLOY_KEY for PyPI authentication
	•	Enhanced error logging for upload failures to make debugging easier
	•	Added workflow_dispatch trigger and tag input to publish-collect for manual runs
Infra
	•	Refactored find_run to handle multiple RUN_IDs
	•	Added functions to locate latest successful workflow runs for backfill targeting

---

**⚙️ Configuration:**
- crates/uv-ffi/Cargo.toml (5 lines)

_4 files changed, 667 insertions(+), 337 deletions(-)_

## [0.10.8.post10] — 2026-04-23

The Local Filesystem Auto-Heal Release

**The Local Filesystem Auto-Heal Release**

This release introduces a surgical, high-performance auto-healing mechanism for local filesystem desyncs, making the `uv-ffi` engine practically immune to external tampering (like a rogue `pip uninstall` run outside of the daemon).

* **Local Filesystem Auto-Heal:** If an external tool modifies the environment behind `uv-ffi`'s back, the in-memory cache becomes stale. The engine now explicitly catches the resulting `os error 2 (No such file or directory)` when trying to read missing `dist-info/METADATA` files. It automatically drops the RAM cache, forces a true disk rescan, and transparently retries the install.
* **Proactive Patching Proven:** Benchmarking this new heal path (~7.6ms) against a proactive delta-patch (~4.0ms) mathematically proved that calling `patch_site_packages_cache` saves **~3.6ms per invocation**. It bypasses both the disk rescan penalty and allows the hot dependency resolver to skip cold evaluation logic entirely.

* **README Accuracy:** Updated the Python API examples to correctly reflect the new 4-tuple return signature (`rc, installed, removed, err`) introduced in `post6`.
* **Cleaned Examples:** Simplified the `pip install` examples in the documentation, removing redundant default flags (like `--link-mode`).

* **Wheel Backfill Polish:** Improved the `.github/workflows/backfill_wheels.yml` script to handle artifact extraction and temporary directories more robustly, preventing disk bloat on the GitHub runner.
* **HTML Generation Fixes:** Fixed indentation and formatting bugs in the automated PEP-503 GitHub Pages index generator.

---

**📝 Code Changes:**
- UPDATE: crates/uv-ffi/src/lib.rs (15 lines changed)

**⚙️ Configuration:**
- .github/workflows/backfill_wheels.yml (179 lines)

**Additional Changes:**
- perf: auto-heal stale site-packages cache on dist-info ENOENT
- Update configuration

_4 files changed, 124 insertions(+), 132 deletions(-)_

## [0.10.8.post9] — 2026-04-23

Fix sdist NOTICE packaging & self-healing registry cache

**⚙️ Configuration:**
- crates/uv-ffi/pyproject.toml (2 lines)

_1 file changed, 1 insertion(+), 1 deletion(-)_

## [0.10.8.post8] — 2026-04-23

The UX Safety Net & Wheel Backfill Release

**The UX Safety Net & Wheel Backfill Release**

This release completely overhauls the developer experience (DX) for users on "exotic" platforms (musl, PyPy, riscv64, s390x, etc.) and introduces a massive CI backfill system to ensure every wheel ever built is permanently indexed and available.

* **Smart Source Build Hints (`build.rs`):** If a user runs `pip install uv-ffi` on an exotic platform and `pip` attempts to build from source, a custom `build.rs` script now intercepts the build process. Before throwing a compiler error, it explicitly prints a clean, highlighted warning instructing the user to use the `--extra-index-url` to grab the pre-built wheel.
* **Graceful Import Failures (`__init__.py`):** Added a pure-Python wrapper around the native extension. If the `.so`/`.pyd` file fails to load due to ABI/architecture mismatches, the resulting `ImportError` is intercepted. The user is presented with a highly descriptive error detailing their OS/Architecture and the exact `pip` command to resolve the issue using the extended GitHub index.

* **Historical Wheel Recovery:** Deployed a powerful new `backfill_wheels.yml` workflow. This script scans the GitHub API across all past CI runs, deduplicates, downloads, and intelligently routes hundreds of non-expired exotic wheels into their correct GitHub Releases.
* **Unified PEP-503 Index:** The index generator now intelligently merges PyPI's JSON API responses with the GitHub Release assets, creating a single, comprehensive HTML index of every `uv-ffi` wheel in existence.

* **README Overhaul:** Completely rewrote the root and crate-level `README.md` files to clearly separate "Standard Platforms" (PyPI) from "Exotic Platforms" (GitHub Pages), complete with a detailed compatibility matrix.
* **Surgical Publishing:** Added a `source_only` manual dispatch option to `publish.yml` allowing for quick, wheel-less updates to the source distribution.
* **Strict Platform Filtering:** Enhanced the publish workflow to strictly filter which CPython targets are permitted onto PyPI, guaranteeing the 10GB limit is never breached.

---

**📚 Documentation:**
- crates/uv-ffi/CHANGELOG.md (33 lines)

**⚙️ Configuration:**
- crates/uv-ffi/pyproject.toml (9 lines)

**Updates:**
- Update HTML writing logic in backfill_wheels.yml

_8 files changed, 600 insertions(+), 332 deletions(-)_

## [0.10.8.post7] — 2026-04-22

ABI3 Wheels & Split Distribution Pipeline

This release introduces a major overhaul of the `uv-ffi` build and distribution system, significantly reducing PyPI storage usage while improving install reliability across platforms.

---

`uv-ffi` now builds using `abi3-py38`, producing **one wheel per architecture** compatible with all Python versions ≥3.8.

- Eliminates per-version wheel duplication (cp38–cp313)
- Reduces PyPI storage footprint dramatically
- Speeds up CI build times
- Simplifies downstream compatibility

---

The release pipeline now separates artifacts into two tiers:

- **PyPI (Primary Distribution)**
  - macOS `universal2`
  - Linux `manylinux` (x86_64, aarch64)
  - Windows (`amd64`, `arm64`)
  - (optional musllinux targets)

- **GitHub Releases (Extended/Exotic)**
  - s390x, ppc64le, riscv64
  - legacy / niche architectures
  - experimental targets

This keeps PyPI lean while still supporting advanced use cases.

---

A lightweight index is now generated from GitHub Release assets, allowing access to non-PyPI wheels.

This lays the groundwork for:
- `--extra-index-url` usage
- external wheel hosting without PyPI storage pressure

---

- **Single Orchestrator Workflow**
  - `publish.yml` now owns the full release lifecycle
  - build workflows no longer auto-trigger on release
  - eliminates duplicate CI runs and race conditions

- **Artifact Routing**
  - Wheels are automatically classified and routed to:
    - PyPI (core)
    - GitHub Releases (exotic)

- **Improved Reliability**
  - Increased workflow timeouts for large builds
  - Added retry logic for checkout and build steps

---

- Massive reduction in PyPI storage usage
- Cleaner, maintainable build matrix
- Faster installs for the majority of users
- Continued support for niche platforms without bloating distribution

---

This release marks the transition from a monolithic wheel distribution strategy to a scalable, multi-tier delivery system.

---

**New Features:**
- feat: enable abi3-py38 for universal wheels (reduce matrix bloat)

**Updates:**
- Update build-wheels workflow for better checks

**Other Changes:**
- Refactor build-wheels-exotic workflow and artifact names
- Refactor build-wheels-extended.yml for clarity
- Enhance publish workflow for wheels and index updates
- Fix wheel filename variable in build-wheels.yml
- Add retry attempts for checkout in build-wheels.yml
- ...and 1 more changes

_5 files changed, 296 insertions(+), 190 deletions(-)_

## [0.10.8.post6] — 2026-04-22

The Auto-Healing & Transparent Errors Release

This release makes the persistent `uv-ffi` engine entirely self-contained, auto-healing, and transparent to the Python caller.

- **Aggressive Registry Auto-Healing:** The persistent PyPI Simple API cache now self-corrects. If the engine fails to find a newly published package (e.g., `torch==2.11.0` dropped 5 minutes ago) due to a stale RAM cache, it instantly detects the failure, drops its registry, and transparently retries the network fetch. Zero false-negatives, zero process restarts required.
- **Detailed FFI Error Surfacing:** `uv_ffi.run()` now returns a 4-tuple: `(rc, installed, removed, err_msg)`. Real error strings from `uv`'s internal resolution (e.g., "No matching distribution found") are captured and passed directly to Python. No more guessing why `rc=1` happened.
- **Manual Cache Control:** Exposed `clear_registry_cache()` to Python as an escape hatch / debugging tool.
- **Strict ExitStatus Mapping:** Fixed a bug where slow-path resolutions falsely returned `rc=0` on failure. The engine now strictly adheres to `uv::commands::ExitStatus`.
- **Restored CDYLIB Build:** Fixed `Cargo.toml` to ensure PyO3 builds the extension module cleanly out-of-the-box.

---

**Bug Fixes:**
- fix: use thin LTO for manylinux i686 to avoid 32-bit OOM
- fix: remove s390x from musl matrix (Tier 3 target, not buildable)
- fix: install all cross-compilers on host runner before maturin build
- fix: install s390x cross-compiler inside maturin container via before-script
- fix: remove duplicate env block
- fix: add aarch64 CFLAGS for ring crate assembly, install cross-compilers
- fix: install s390x cross-compiler for glibc build
- fix: pass $TAG shell var to dispatch instead of step output
- fix: use try/catch for powershell PyPI check to handle 404
- fix: handle 404 in powershell PyPI check gracefully

**Updates:**
- Update build-wheels-extended.yml

**Other Changes:**
- docs: document PyPI registry auto-healing and clear_registry_cache API
- fix(uv-ffi): implement aggressive auto-heal and strict ExitStatus mapping
- feat(uv-ffi): expose detailed error messages to Python caller
- feat(uv-ffi): implement self-healing registry cache and restore cdylib build config
- Add GitHub Actions workflow for static content deployment
- ...and 10 more changes

_10 files changed, 763 insertions(+), 444 deletions(-)_

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
