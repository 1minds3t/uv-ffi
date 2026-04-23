fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:warning=If this build fails, pre-built wheels are available:");
    println!("cargo:warning=  pip install uv-ffi --extra-index-url https://1minds3t.github.io/uv-ffi/");
    println!("cargo:warning=Supports: Linux musl/glibc x86_64/aarch64/armv7/i686/ppc64le/s390x/riscv64,");
    println!("cargo:warning=  macOS universal2, Windows amd64/arm64/x86, PyPy, GraalPy, CPython 3.13t");
}
