fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:warning=Build failed? Pre-built wheels are available:");
    println!("cargo:warning=  pip install uv-ffi --extra-index-url https://1minds3t.github.io/uv-ffi/");
    println!("cargo:warning=");
    println!("cargo:warning=PyPI (ABI3, Python >= 3.8): Linux x86_64/aarch64, macOS universal2, Windows amd64/arm64");
    println!("cargo:warning=Extended index: musl Linux, armv7/i686/ppc64le/s390x/riscv64, macOS arm64+x86_64 native,");
    println!("cargo:warning=  Windows x86, CPython 3.13t, PyPy, GraalPy, and per-version wheels for older releases");
}