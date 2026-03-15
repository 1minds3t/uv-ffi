# uv-ffi

Persistent in-process execution engine for [uv](https://github.com/astral-sh/uv)'s package resolver and installer.

While `uv` is designed as a world-class CLI tool, `uv-ffi` re-architects its core as a **resident engine**. By keeping a Tokio runtime, HTTP connection pools, and site-packages metadata warm in memory across calls, it achieves execution speeds limited only by filesystem I/O.

Used internally by [omnipkg](https://github.com/1minds3t/omnipkg), but directly callable from any long-lived Python process.

## Usage

```python
import sys
sys.path.insert(0, '/path/to/omnipkg/src')
from omnipkg._vendor.uv_ffi import run, invalidate_site_packages_cache

PY = '/path/to/your/python'
BASE = f'pip install --python {PY} --link-mode symlink'

# First call initializes the engine (~65-75ms, one-time cost)
rc, installed, removed = run(f'{BASE} rich==14.3.2')

# Subsequent calls use the warm engine (~5-6ms)
rc, installed, removed = run(f'{BASE} rich==14.3.3')
# -> inst=[('rich', '14.3.3')] rem=[('rich', '14.3.2')]

# If an external tool modified the environment, force a rescan
invalidate_site_packages_cache()
rc, installed, removed = run(f'{BASE} rich==14.3.3')  # ~8ms (includes 2.5ms rescan)
```

The key is keeping the import alive in a long-lived process. Each new Python subprocess pays ~70ms (interpreter startup + engine init). In a warm daemon worker, the same operation costs ~6ms.

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

### Cache invalidation

| Method | Latency | Notes |
|:--|--:|:--|
| `uv` full site-packages rescan | ~2.5ms | paid on every CLI invocation |
| `invalidate_site_packages_cache()` | ~2.5ms | forced rescan, same cost as uv |
| `patch_site_packages_cache(installed, removed)` | **~25µs** | **~100× faster** than full rescan |

The ~5–6ms floor on a real swap is the hardware limit — VFS symlink create/unlink on NVMe. uv-ffi eliminates all software overhead above that floor.

**Important:** calling uv-ffi via a new subprocess each time (~73ms avg) is slower than calling `uv` directly (~19ms). The gains only materialize when the engine stays warm across multiple calls in the same process — a daemon, notebook kernel, API server, or test runner.

## Cache coherency

uv-ffi holds site-packages state in RAM and trusts it completely. If an external tool (`uv`, `pip`, `conda`) modifies the environment without notifying uv-ffi, the next call may return `rc=0, inst=[], rem=[]` — a silent false no-op — because the cache believes the target state is already satisfied.

Verified behavior:
```
[1] uv-ffi swap:                   6.51ms  inst=[('rich', '14.3.3')] rem=[('rich', '14.3.2')]
[2] uv pip install rich==14.3.2:  18.65ms  (disk=14.3.2, cache still thinks 14.3.3)
[3] uv-ffi ask for rich==14.3.3:   0.46ms  inst=[] rem=[]  ← silent no-op, wrong answer
[4] uv-ffi ask for rich==14.3.2:  11.35ms  inst=[('rich', '14.3.2')]  ← rescan + swap
```

Step 3 returns rc=0 with no action because the cache says `14.3.3` is already installed. The mismatch is only discovered in step 4 when something different is requested.

**This is not a bug** — it is the fundamental tradeoff of a persistent cache. Callers who need coherency with external tools have two options:

**Option A — FS watcher + delta patch** (omnipkg's approach)
Watch site-packages for filesystem events and call `patch_site_packages_cache(installed, removed)` on each change. Cost: ~25µs per patch — ~100× faster than a full rescan. Full coherency with no performance penalty on normal calls.

**Option B — Force rescan**
Call `invalidate_site_packages_cache()` before any call where external modification is possible. Cost: ~2.5ms — same as uv's own site-packages scan cost. Simple, no watcher needed, but loses the sub-millisecond no-op advantage.

If your process is the **only thing modifying the environment**, neither is needed and you get full speed with no caveats.

## Coexistence with vanilla uv

uv-ffi installs are fully compatible with vanilla `uv` operations in the same environment. uv-ffi writes complete dist-info including `RECORD`, `INSTALLER`, and `REQUESTED` — so `uv pip uninstall`, `uv pip install`, and other standard toolchain operations work correctly on packages uv-ffi installed.

Verified:
- uv-ffi installs a package → `uv pip uninstall` removes it cleanly ✓
- `uv pip install` modifies a package → uv-ffi subsequent call works correctly ✓
- No warnings, no corrupted dist-info ✓

Note: coherency caveats above still apply — coexistence means no corruption, not automatic cache synchronization.

## Architecture

```
C dispatcher  →  Unix socket
Python daemon →  dedicated uv worker per interpreter
Rust FFI      →  persistent UvEngine (OnceLock)
FS watcher    →  patch_site_packages_cache()
```

**Persistent `UvEngine` singleton**
`uv` CLI pays ~10ms on every invocation for interpreter discovery, platform tagging, cache init, and TLS pool teardown. uv-ffi does this once at import time and holds the engine in a `OnceLock`. All subsequent calls skip directly to resolution.

**Zero-clap fast path**
Common `pip install` commands bypass clap argument parsing entirely — internal Rust structs are constructed directly, saving ~2ms per call.

**Shared `SITE_PACKAGES_CACHE`**
Site-packages metadata is kept in a shared cache across calls. A `FORCE_RESCAN` atomic flag lets the FS watcher trigger a targeted rescan only when an external write is detected.

**Delta cache patching**
`patch_site_packages_cache(installed, removed)` surgically updates the in-memory metadata map for a single package in ~25µs — ~100× faster than uv's own ~2.5ms site-packages rescan.

**Idempotent initialization**
Logging and Tokio initialization are guarded so the engine can be loaded safely in any process without double-init panics.

## Benchmark methodology

- In-process tests: 10-run alternating swap (`rich==14.3.2` ↔ `rich==14.3.3`) in a single warm Python session
- Subprocess tests: 8 separate `subprocess.run` calls, new Python process each time
- Interference test: uv subprocess between uv-ffi calls, 1s settle time
- uv cache pre-warmed before all runs
- Hardware: Linux, NVMe Gen4, Python 3.11.14

## Version correspondence

uv-ffi versions track the upstream uv release they are built against:

| uv-ffi | uv upstream | Notes |
|:--|:--|:--|
| 0.10.8 | 0.10.8 | Initial release |
| 0.10.8.post1 | 0.10.8 | Persistent UvEngine, delta cache patching, verified uv coexistence |

## Attribution

This crate links against uv source code from [astral-sh/uv](https://github.com/astral-sh/uv),
copyright Astral Software Inc., used under the MIT License. See NOTICE for full attribution.

Not affiliated with, endorsed by, or sponsored by Astral Software Inc.