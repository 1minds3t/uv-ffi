# uv-ffi

> This is the source repository. For the full README, API docs, and platform support table, see [`crates/uv-ffi/README.md`](crates/uv-ffi/README.md) or the [PyPI page](https://pypi.org/project/uv-ffi/).

## Quick install

```bash
# Standard platforms (PyPI)
pip install uv-ffi

# Exotic platforms — musl Alpine, armv7, riscv64, s390x, free-threaded, PyPy, GraalPy, cp37
pip install uv-ffi --extra-index-url https://exotic-wheels.github.io
```
**[Browse all wheels](https://exotic-wheels.github.io/)** — PyPI wheels + 600+ additional wheels hosted on GitHub Releases

The extra index combines PyPI wheels + 400+ additional wheels hosted on GitHub Releases, covering every target the PyPI 10 GB limit forced out. If `pip` can't find a wheel for your platform on PyPI, it'll find it there.

## Build from source

```bash
pip install uv-ffi --no-binary uv-ffi
```

Requires a Rust toolchain. If the build fails, check the extra-index-url above — there's almost certainly a pre-built wheel for your platform.
