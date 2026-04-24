# uv-ffi

Persistent in-process execution engine for [uv](https://github.com/astral-sh/uv)'s package resolver and installer.

While `uv` is designed as a world-class CLI tool, `uv-ffi` re-architects its core as a **resident engine**. By keeping a Tokio runtime, HTTP connection pools, site-packages metadata, and interpreter state warm in memory across calls, it achieves execution speeds limited only by filesystem I/O.

Used internally by [omnipkg](https://github.com/1minds3t/omnipkg), but directly callable from any long-lived Python process.

---

## Installation

### Standard platforms
```bash
pip install uv-ffi
```
Covers Linux x86_64/aarch64, macOS arm64/x86_64/universal2, Windows amd64/arm64 — all via ABI3 wheels (Python ≥ 3.8).

### Exotic platforms (musl Alpine, armv7, riscv64, s390x, ppc64le, free-threaded CPython 3.13t, PyPy, GraalPy, Windows x86)
```bash
pip install uv-ffi --extra-index-url https://1minds3t.github.io/uv-ffi/
```
400+ wheels hosted on GitHub Releases, indexed at the URL above. These were removed from PyPI to stay under the 10 GB storage limit.

### Build from source (requires Rust toolchain)
```bash
pip install uv-ffi --no-binary uv-ffi
```
If the Rust build fails, pre-built wheels for your platform are almost certainly available via the extra-index-url above.

---

## Usage

```python
from uv_ffi import run, invalidate_site_packages_cache, patch_site_packages_cache, get_site_packages_cache, clear_registry_cache

PY = '/path/to/your/python'
BASE = f'pip install --python {PY}'  # LinkMode::Symlink);  // ← by default

# First call initializes the engine (~65-75ms, one-time cost)
rc, installed, removed, warnings = run(f'{BASE} rich==14.3.2')

# Subsequent calls use the warm engine (~5-6ms)
rc, installed, removed, warnings = run(f'{BASE} rich==14.3.3')
# -> installed=[('rich', '14.3.3')] removed=[('rich', '14.3.2')]

# Inspect engine's current in-memory view of the environment
state = get_site_packages_cache()
# -> [('rich', '14.3.3'), ('requests', '2.31.0'), ...]
```

The key is keeping the import alive in a long-lived process. Each new Python subprocess pays ~70ms (interpreter startup + engine init). In a warm daemon worker, the same operation costs ~6ms.

---

## Performance

Measured wall-clock time on Linux (NVMe SSD, Python 3.11, pre-warmed uv cache).

### No-op (package already satisfied)

| Method | Wall time | user | sys |
|:--|--:|--:|--:|
| `uv pip install` (subprocess) | ~11–12ms | 0.007s | 0.006s |
| `uv-ffi` in-process (warm engine) | **~0.4–2ms** | 0.000s | 0.000s |
| **Speedup** | **~6–8×** | | |

### Real swap (uninstall + reinstall different version)

| Method | Wall time | user | sys |
|:--|--:|--:|--:|
| `uv pip install` (subprocess) | ~17–20ms | 0.010s | 0.013s |
| `uv-ffi` in-process (warm engine) | **~5.4–6.5ms** | 0.000s | 0.002s |
| **Speedup** | **~2.5–3×** | | |

### Cache operations

| Method | Latency | Notes |
|:--|--:|:--|
| `uv` full site-packages rescan | ~2.5ms | paid on every CLI invocation |
| `invalidate_site_packages_cache()` | ~2.5ms | forced rescan, same cost as uv |
| `patch_site_packages_cache(installed, removed)` | **~25µs** | **~100× faster** than full rescan |
| Post-install cache update (internal) | **0.0ms** | zero-disk: built from resolver changelog |

The ~5–6ms floor on a real swap is the hardware limit — VFS symlink create/unlink on NVMe. uv-ffi eliminates all software overhead above that floor.

**Important:** calling uv-ffi via a new subprocess each time (~73ms avg) is slower than calling `uv` directly (~19ms). The gains only materialize when the engine stays warm across multiple calls in the same process.

---

## API

### `run(cmd: str) -> (int, list[tuple[str,str]], list[tuple[str,str]])`

Execute a uv command in-process. Returns `(exit_code, installed, removed)` where installed/removed are lists of `(name, version)` tuples.

```python
rc, installed, removed, err = run('pip install --python /usr/bin/python3 rich==14.3.3')
```

Supports all flags omnipkg uses on the fast path: `--python`, `--link-mode`, `--target`, `--index-url`, `--extra-index-url`, `--reinstall`, `-q`. Any unrecognized flag falls back to the full clap parse path automatically.

### `get_site_packages_cache() -> list[tuple[str, str]]`

Returns the engine's current in-memory view of the environment as `[(name, version), ...]`. Returns an empty list if the cache has not been populated yet (before first install call). Zero disk I/O.

```python
state = get_site_packages_cache()
# [('rich', '14.3.3'), ('requests', '2.31.0'), ...]
```

### `invalidate_site_packages_cache()`

Forces a full disk rescan on the next install call. Use when an external tool has modified the environment and you don't have the changelog. Cost: ~2.5ms on next call.

### `patch_site_packages_cache(installed, removed)`

Surgically update the in-memory cache with a known delta. ~100× faster than a full rescan. Returns `True` if the cache was live and patched, `False` if no cache was active.

```python
patch_site_packages_cache(
    installed=[['rich', '14.3.3']],
    removed=[['rich', '14.3.2']],
)
```


### `clear_registry_cache()`

Drops the persistent `RegistryClient` memory cache. Use this if you suspect the internal PyPI Simple API cache is stale. Note: The engine already auto-heals on install failures, so manual invocation is rarely needed.

### C ABI: `omnipkg_uv_run_c`

```c
int omnipkg_uv_run_c(const char *cmd, char *out_json, int max_out);
```

Runs a uv command and writes a JSON changelog to `out_json`:

```json
{"installed":[["rich","14.3.3"]],"removed":[["rich","14.3.2"]]}
```

Returns the exit code. Safe to call from Go, C++, Rust, or any language with C FFI.

---

## Isolated "Bubble" Installs (`--target`)

`uv-ffi` provides cache-safe isolated directory installs via `--target`. Standard `uv` evaluates `--target` against the host interpreter's installed packages, which can produce incorrect results and poisons in-memory state. `uv-ffi` routes `--target` installs through a pre-warmed `BUBBLE_ENVIRONMENT`:

- The resolver sees a clean slate — no existing packages, no cross-contamination
- `SITE_PACKAGES_CACHE` (which reflects main env state) is never read or written during a bubble install
- The `BUBBLE_INSTALL` atomic flag ensures the main env cache is fully protected for the duration
- After the install, the flag is reset and the next main-env call proceeds normally

```python
# Install into isolated dir — main env cache untouched
rc, installed, _ = run(f'pip install --python {PY} --target /tmp/app_env flask==3.0.0')
```

This is how omnipkg generates multiversion isolated environments entirely in-memory.

---

## Cache Coherency

uv-ffi holds site-packages state in RAM and trusts it completely. If an external tool (`uv`, `pip`, `conda`) modifies the environment without notifying uv-ffi, the next call may return `rc=0, inst=[], rem=[]` — a silent false no-op.

Verified behavior:
```
[1] uv-ffi swap:                   6.51ms  installed=[('rich','14.3.3')] removed=[('rich','14.3.2')]
[2] uv pip install rich==14.3.2:  18.65ms  (disk=14.3.2, cache still thinks 14.3.3)
[3] uv-ffi ask for rich==14.3.3:   0.46ms  inst=[] rem=[]  ← silent no-op, wrong answer
[4] uv-ffi ask for rich==14.3.2:  11.35ms  inst=[('rich','14.3.2')]  ← rescan + swap
```

**If uv-ffi is the only thing modifying the environment, no action is needed.** After every install, the cache is updated directly from the resolver's in-memory changelog at zero disk I/O cost — it's always coherent with no overhead.

For environments shared with external tools, two options:

**Option A — FS watcher + delta patch** (omnipkg's approach)
Watch site-packages for filesystem events. On each change, call `patch_site_packages_cache(installed, removed)`. Cost: ~25µs per patch. Full coherency at near-zero overhead.

**Option B — Force rescan**
Call `invalidate_site_packages_cache()` before any call where external modification is possible. Cost: ~2.5ms on next call. Simple, no watcher needed.

---

## Architecture

```
Python API  →  run() / patch_site_packages_cache() / get_site_packages_cache()
C ABI       →  omnipkg_uv_run_c() → JSON changelog
Fast path   →  try_parse_ffi_install() → run_pip_install_direct() [bypasses clap entirely]
Slow path   →  clap parse → uv::run() [pip freeze, uninstall, etc.]
Globals     →  ENGINE / BUBBLE_ENVIRONMENT / SITE_PACKAGES_CACHE / REGISTRY_CLIENT / PYTHON_ENVIRONMENT
```

**Persistent `UvEngine` singleton**
Interpreter discovery, platform tagging, cache init, and TLS pool setup happen once at import time and are held in a `OnceLock`. All subsequent calls skip directly to resolution. Includes a pre-warmed `BUBBLE_ENVIRONMENT` for `--target` installs.

**Zero-clap fast path**
`pip install` commands are parsed directly via `try_parse_ffi_install()` — a hand-written token parser covering all flags omnipkg uses. Internal Rust structs are constructed directly, skipping clap entirely (~2ms saved per call). Unrecognized flags fall back to clap automatically.

**Persistent `RegistryClient` and `PythonEnvironment`**
The HTTP client (TLS pools, connection pools) and Python environment (interpreter metadata, marker environment) are stored as global singletons after first use. Subsequent calls reuse them directly — no socket teardown, no filesystem search.


**PyPI Registry Auto-Healing**
The FFI engine keeps PyPI API responses in RAM for maximum speed. If a newly published package version is requested and not found in the RAM cache, the engine detects the internal failure, automatically drops its registry cache, and retries the network fetch transparently. You never need to restart the process to see newly published packages.

**Zero-disk post-install cache update**
After a successful install, `SITE_PACKAGES_CACHE` is updated directly from the resolver's changelog using in-memory `InstalledRegistryDist` construction. No dist-info directory scan, no `try_from_path` I/O. Post-install cache update cost: **0.0ms**.

**`SitePackages::add_dist()`**
A new method added to `uv-installer`'s `SitePackages` that surgically inserts a distribution into the in-memory index without touching disk. Used by both the post-install zero-disk update and `patch_site_packages_cache()`.

**Idempotent initialization**
Logging setup (`setup_logging`) and `miette::set_hook` are now called with `let _ =` — safe to call repeatedly in a long-running process without double-init panics.

**Persistent Tokio runtime**
The `main()` path previously created a new Tokio runtime on every call and called `shutdown_background()` on exit (leaving pending HTTP requests). The runtime is now stored in a `OnceLock` and reused across calls — no teardown overhead, no leaked requests.

---

## Profiling

Set `UV_FFI_PROFILE=1` to enable millisecond-precision phase tracing:

```
[UV-PROFILE] cache-reused: 0.12ms
[UV-PROFILE] post-site-packages-scan: 0.08ms (cached)
[UV-PROFILE] post-settings-resolve: 0.31ms
[UV-PROFILE] post-execute-plan: 4.82ms
[UV-PROFILE] post-changelog-from-local: 4.83ms
[UV-PROFILE] post-changelog-write: 4.91ms
[UV-SYNC] Zero-disk cache update: done
[UV-PROFILE] post-explicit-drop: 0.02ms
[UV-PROFILE] post-await: 5.14ms
```

---

## Coexistence with vanilla uv

uv-ffi installs are fully compatible with vanilla `uv` operations in the same environment. uv-ffi writes complete dist-info including `RECORD`, `INSTALLER`, and `REQUESTED` — so `uv pip uninstall`, `uv pip install`, and other standard toolchain operations work correctly on packages uv-ffi installed.

Coexistence means no corruption, not automatic cache synchronization — see cache coherency section above.

---

## Platform Support

| Platform | Architectures | Python | Index |
|:--|:--|:--|:--|
| Linux glibc ≥ 2.17 | x86_64, aarch64 | 3.8–3.14 | PyPI + Extra |
| Linux glibc ≥ 2.17 | i686, armv7, ppc64le, s390x | 3.8–3.14 | Extra only |
| Linux glibc ≥ 2.31 | riscv64 | 3.8–3.14 | Extra only |
| Linux musl ≥ 1.2 | x86_64, aarch64 | 3.8–3.14 | PyPI + Extra |
| Linux musl ≥ 1.2 | armv7, i686, ppc64le | 3.8–3.14 | Extra only |
| macOS (universal2) | x86_64 + arm64 | 3.8–3.14 | PyPI + Extra |
| Windows x64 / ARM64 | amd64, aarch64 | 3.8–3.14 | PyPI + Extra |
| Windows x86 | i686 | 3.8–3.14 | Extra only |
| CPython 3.13t (free-threaded) | x86_64, aarch64 | 3.13t | Extra only |
| PyPy 3.9/3.10 | x86_64 | 3.9–3.10 | Extra only |
| GraalPy | x86_64 | — | Extra only |

---

## Version Correspondence

uv-ffi versions track the upstream uv release they are built against.

| uv-ffi | uv upstream | Notes |
|:--|:--|:--|
| 0.10.8 | 0.10.8 | Initial release |
| 0.10.8.post1 | 0.10.8 | Persistent `UvEngine`, delta cache patching, zero-clap fast path, verified uv coexistence |
| 0.10.8.post2 | 0.10.8 | Windows cache path fix (`%LOCALAPPDATA%`), platform-safe temp dir fallback |
| 0.10.8.post3 | 0.10.8 | Windows ARM64 wheels, Tokio `PathError` fix on Windows runners |
| 0.10.8.post4 | 0.10.8 | Bubble environments (`--target` isolation), persistent `RegistryClient` + `PythonEnvironment`, zero-disk post-install cache update, JSON C ABI |
| 0.10.8.post5 | 0.10.8 | CI hardening, per-platform PyPI checks, sdist publishing, Windows PowerShell fixes |
| 0.10.8.post6 | 0.10.8 | Auto-healing PyPI registry cache, detailed FFI error messages (4-tuple return), `clear_registry_cache()` |
| 0.10.8.post7 | 0.10.8 | ABI3 wheels (one wheel per arch, Python ≥ 3.8), split PyPI/GitHub Releases distribution, GH Pages index |

---

## Benchmark Methodology

- In-process tests: 10-run alternating swap (`rich==14.3.2` ↔ `rich==14.3.3`) in a single warm Python session
- Subprocess tests: 8 separate `subprocess.run` calls, new Python process each time
- Interference test: uv subprocess between uv-ffi calls, 1s settle time
- uv cache pre-warmed before all runs
- Hardware: Linux, NVMe Gen4, Python 3.11.14

---

## Attribution

This crate links against uv source code from [astral-sh/uv](https://github.com/astral-sh/uv),
copyright Astral Software Inc., used under the MIT License. See NOTICE for full attribution.

Not affiliated with, endorsed by, or sponsored by Astral Software Inc.
