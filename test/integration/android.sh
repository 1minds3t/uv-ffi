#!/bin/sh
# Integration test for bare Android (CPython Android build on emulator)
# Note: runs under Android's /system/bin/sh (mksh), not bash
set -eux

export PYTHONHOME="$PWD/prefix"
export LD_LIBRARY_PATH="$PWD/prefix/lib"
export PATH="$PWD/prefix/bin:$PATH"
export UV_CACHE_DIR="$PWD/.uv-cache"
export UV_PYTHON_INSTALL_DIR="$PWD/.uv-python"

mkdir -p "$UV_CACHE_DIR" "$UV_PYTHON_INSTALL_DIR"

echo "== Python platform =="
./prefix/bin/python3 -c 'import sys,sysconfig; print(sys.platform); print(sysconfig.get_platform())'

echo "== uv python find =="
./uv python find ./prefix/bin/python3 -v

echo "== uv run =="
./uv run -p ./prefix/bin/python3 python -c 'import sys; print(sys.platform)'

echo "== uv pip list =="
./uv pip list --python ./prefix/bin/python3 --system
