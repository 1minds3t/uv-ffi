from __future__ import annotations

try:
    from uv_ffi.uv_ffi import (
        run,
        get_site_packages_cache,
        invalidate_site_packages_cache,
        patch_site_packages_cache,
        clear_registry_cache,
    )
except ImportError as _e:
    import platform as _platform
    _plat = _platform.system()
    _arch = _platform.machine()
    raise ImportError(
        f"\n\nuv-ffi native extension failed to load ({_plat} {_arch}).\n"
        "A pre-built wheel for your platform may be available at the extended index:\n\n"
        "  pip install uv-ffi --extra-index-url https://exotic-wheels.github.io/\n\n"
        "If no wheel exists for your platform, build from source (requires Rust):\n"
        "  pip install uv-ffi --no-binary uv-ffi\n"
    ) from _e

try:
    from importlib.metadata import version as _v
    __version__ = _v("uv-ffi")
except Exception:
    __version__ = "unknown"

__all__ = [
    "run",
    "get_site_packages_cache",
    "invalidate_site_packages_cache",
    "patch_site_packages_cache",
    "clear_registry_cache",
    "__version__",
]
