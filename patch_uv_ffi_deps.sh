#!/usr/bin/env bash
# Run from: /home/minds3t/omnipkg/src/omnipkg/_vendor/uv
set -e

UV_LIB="crates/uv/src/lib.rs"
FFI_CARGO="crates/uv-ffi/Cargo.toml"
ROOT_CARGO="Cargo.toml"

# ── 1. Make commands, settings, printer pub in uv/src/lib.rs ─────────────────
sed -i \
  's/^pub(crate) mod commands;/pub mod commands;/' \
  "$UV_LIB"
sed -i \
  's/^pub(crate) mod settings;/pub mod settings;/' \
  "$UV_LIB"
sed -i \
  's/^pub(crate) mod printer;/pub mod printer;/' \
  "$UV_LIB"

echo "✅ uv/src/lib.rs visibility patched"

# ── 2. Add all needed deps to uv-ffi/Cargo.toml ──────────────────────────────
# First grab the exact versions already used in the workspace
get_ver() {
    grep "^$1 = \|\"$1\"" "$ROOT_CARGO" | head -1 | grep -oP '"\d+[^"]*"' | head -1 | tr -d '"'
}

cat >> "$FFI_CARGO" << 'EOF'

tokio            = { version = "1.40.0", features = ["rt", "rt-multi-thread", "macros"] }
uv-cache         = { path = "../uv-cache" }
uv-cli           = { path = "../uv-cli" }
uv-client        = { path = "../uv-client" }
uv-configuration = { path = "../uv-configuration" }
uv-pep508        = { path = "../uv-pep508" }
uv-requirements  = { path = "../uv-requirements" }
uv-resolver      = { path = "../uv-resolver" }
uv-workspace     = { path = "../uv-workspace" }
uv-python        = { path = "../uv-python" }
uv-normalize     = { path = "../uv-normalize" }
EOF

echo "✅ uv-ffi/Cargo.toml deps added"

# ── 3. Verify the crate paths exist ──────────────────────────────────────────
for crate in uv-cache uv-cli uv-client uv-configuration uv-pep508 \
             uv-requirements uv-resolver uv-workspace uv-python uv-normalize; do
    if [ -d "crates/$crate" ]; then
        echo "  ✓ crates/$crate"
    else
        echo "  ✗ MISSING: crates/$crate"
    fi
done
