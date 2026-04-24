use std::ffi::OsString;
use once_cell::sync::Lazy;
use uv_distribution_types::Name;
use clap::Parser;

type Changelog    = uv::commands::pip::operations::Changelog;
type ChangedDist  = uv::commands::pip::operations::ChangedDist;

static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name("uv-tokio")
        .build()
        .expect("Failed building uv Tokio runtime")
});


// ── UvEngine: bypass uv::run() entirely ──────────────────────────────────────
use std::sync::OnceLock;
use uv_cache::Cache;
use uv_client::{BaseClientBuilder, RegistryClient, RegistryClientBuilder};
use uv_python::Interpreter;
use uv_pep508::MarkerEnvironment;
use uv_platform_tags::Platform;

pub struct UvEngine {
    pub cache:       Cache,
    pub interpreter: Interpreter,
    pub client:      RegistryClient,
    pub python_exe:  std::path::PathBuf,
}
unsafe impl Send for UvEngine {}
unsafe impl Sync for UvEngine {}

static ENGINE: OnceLock<UvEngine> = OnceLock::new();

// Pre-warmed empty PythonEnvironment for bubble/--target installs.
// Shares the interpreter (platform, markers, python version) with ENGINE
// but intentionally has no installed packages — so uv's resolver treats
// the target dir as a clean slate and never cross-contaminates main env.
static BUBBLE_ENVIRONMENT: OnceLock<uv_python::PythonEnvironment> = OnceLock::new();

fn get_engine(python_exe: &str) -> &'static UvEngine {
    ENGINE.get_or_init(|| {
        let cache_dir = std::env::var("UV_CACHE_DIR")
            .map(|s| std::path::PathBuf::from(s))
            .unwrap_or_else(|_| {
                #[cfg(target_os = "windows")]
                {
                    std::env::var("LOCALAPPDATA")
                        .map(|p| std::path::PathBuf::from(p).join("uv").join("cache"))
                        .unwrap_or_else(|_| std::env::temp_dir().join("uv_cache"))
                }
                #[cfg(not(target_os = "windows"))]
                {
                    std::env::var("HOME")
                        .map(|h| std::path::PathBuf::from(h).join(".cache").join("uv"))
                        .unwrap_or_else(|_| std::env::temp_dir().join("uv_cache"))
                }
            });
        let _ = std::fs::create_dir_all(&cache_dir);
        let cache = Cache::from_path(cache_dir);

        let interpreter = Interpreter::query(python_exe, &cache)
            .expect("UvEngine: interpreter query failed");

        let markers:  &'static MarkerEnvironment =
            Box::leak(Box::new(interpreter.markers().clone()));
        let platform: &'static Platform =
            Box::leak(Box::new(interpreter.platform().clone()));

        let base_client = BaseClientBuilder::default()
            .markers(markers)
            .platform(platform);

        let client = RegistryClientBuilder::new(base_client, cache.clone())
            .build();

        UvEngine {
            cache,
            interpreter,
            client,
            python_exe: std::path::PathBuf::from(python_exe),
        }
    })
}

async fn run_pip_install_direct(
    packages:        Vec<String>,
    reinstall:       bool,
    link_mode_str:   Option<String>,
    index_url:       Option<String>,
    extra_index_url: Option<String>,
    python_exe:      &str,
    target_dir:      Option<String>,
) -> anyhow::Result<uv::commands::pip::operations::Changelog> {
    use uv_requirements::RequirementsSource;
    use uv_configuration::{
        BuildIsolation, BuildOptions, Concurrency,
        DryRun, IndexStrategy, KeyringProviderType, NoSources,
        Reinstall as ReinstallSpec, Upgrade,
    };
    use uv_distribution_types::{
        ConfigSettings, DependencyMetadata, ExtraBuildVariables,
        IndexLocations, Index, IndexUrl, PackageConfigSettings,
    };
    use uv_resolver::{
        DependencyMode, ExcludeNewer, PrereleaseMode, ResolutionMode,
    };
    use uv_install_wheel::LinkMode;
    use uv_python::{PythonDownloads, PythonPreference, Target};
    use uv_settings::PythonInstallMirrors;
    use uv::commands::pip::operations::Modifications;
    use uv_workspace::pyproject::ExtraBuildDependencies;
    use uv::printer::Printer;
    use uv_preview::Preview;
    use uv_requirements::specification::GroupsSpecification;
    use std::str::FromStr;

    let engine = get_engine(python_exe);

    let target = target_dir.map(|p| Target::from(std::path::PathBuf::from(p)));

    // Seed PYTHON_ENVIRONMENT based on install destination:
    //   main env  → real site-packages (resolver sees what's installed, skips existing deps)
    //   --target  → empty bubble env   (resolver sees nothing, installs full self-contained set)
    // BUBBLE_ENVIRONMENT is constructed once and reused — interpreter/platform are warm,
    // but no dist-info scan ever runs against it, so it always looks empty to the resolver.
    if target.is_some() {
        let bubble_env = BUBBLE_ENVIRONMENT.get_or_init(|| {
            uv_python::PythonEnvironment::from_interpreter(engine.interpreter.clone())
        });
        if let Ok(mut g) = uv::PYTHON_ENVIRONMENT.lock() { *g = Some(bubble_env.clone()); }
        uv::BUBBLE_INSTALL.store(true, std::sync::atomic::Ordering::SeqCst);
    } else {
        let env = uv_python::PythonEnvironment::from_interpreter(engine.interpreter.clone());
        if let Ok(mut g) = uv::PYTHON_ENVIRONMENT.lock() { *g = Some(env); }
        uv::BUBBLE_INSTALL.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    let requirements: Vec<RequirementsSource> = packages.iter()
        .map(|p| RequirementsSource::from_package_argument(p).expect("invalid package spec"))
        .collect();

    let link_mode = link_mode_str.as_deref()
        .and_then(|s| match s {
            "symlink"  => Some(LinkMode::Symlink),
            "hardlink" => Some(LinkMode::Hardlink),
            "clone"    => Some(LinkMode::Clone),
            "copy"     => Some(LinkMode::Copy),
            _          => None,
        })
        .unwrap_or(LinkMode::Symlink);

    let reinstall_spec = if reinstall {
        ReinstallSpec::All
    } else {
        ReinstallSpec::default()
    };

    // Build index locations from optional index URLs
    let index_locations = if index_url.is_some() || extra_index_url.is_some() {
        let mut indexes = vec![];
        if let Some(url) = index_url {
            if let Ok(u) = IndexUrl::from_str(&url) {
                indexes.push(Index::from_index_url(u));
            }
        }
        if let Some(url) = extra_index_url {
            if let Ok(u) = IndexUrl::from_str(&url) {
                indexes.push(Index::from_extra_index_url(u));
            }
        }
        IndexLocations::new(indexes, vec![], false)
    } else {
        IndexLocations::default()
    };

    let groups = GroupsSpecification {
        root: std::env::current_dir().unwrap_or_default(),
        groups: Default::default(),
    };

    let base_client = BaseClientBuilder::default()
        .markers(engine.interpreter.markers())
        .platform(engine.interpreter.platform());

    // Call pip_install directly — no uv::run(), no settings resolution, no CLI
    uv::commands::pip_install(
        &requirements,
        &[],                          // constraints
        &[],                          // overrides
        &[],                          // excludes
        &[],                          // build_constraints
        vec![],                       // constraints_from_workspace
        vec![],                       // overrides_from_workspace
        vec![],                       // excludes_from_workspace
        vec![],                       // build_constraints_from_workspace
        &uv_configuration::ExtrasSpecification::default(),
        &groups,
        ResolutionMode::default(),
        PrereleaseMode::default(),
        DependencyMode::default(),
        Upgrade::default(),
        index_locations,
        IndexStrategy::default(),
        None,                         // torch_backend
        DependencyMetadata::default(),
        KeyringProviderType::default(),
        &base_client,
        reinstall_spec,
        link_mode,
        false,                        // compile
        None,                         // hash_checking
        true,                        // installer_metadata
        &ConfigSettings::default(),
        &PackageConfigSettings::default(),
        BuildIsolation::default(),
        &ExtraBuildDependencies::default(),
        &ExtraBuildVariables::default(),
        BuildOptions::default(),
        Modifications::Sufficient,
        None,                         // python_version
        None,                         // python_platform
        PythonDownloads::default(),
        PythonInstallMirrors::default(),
        false,                        // strict
        ExcludeNewer::default(),
        NoSources::default(),
        Some(engine.python_exe.to_string_lossy().into_owned()),
        false,                        // system
        false,                        // break_system_packages
        target,                       // target
        None,                         // prefix
        PythonPreference::default(),
        Concurrency::default(),
        engine.cache.clone(),
        DryRun::default(),
        Printer::Silent,
        Preview::default(),
    ).await.and_then(|status| match status {
        uv::commands::ExitStatus::Success => Ok(()),
        uv::commands::ExitStatus::Failure => Err(anyhow::anyhow!("pip_install failed: resolution or install error")),
        uv::commands::ExitStatus::Error => Err(anyhow::anyhow!("pip_install failed: internal uv error")),
        uv::commands::ExitStatus::External(code) => Err(anyhow::anyhow!("pip_install failed: external process exited with code {}", code)),
    })?;

    // Reset flag so the next main-env install is never affected.
    uv::BUBBLE_INSTALL.store(false, std::sync::atomic::Ordering::SeqCst);

    // Drain changelog — for bubble installs install.rs already dropped it,
    // for main installs take it normally.
    let changelog = uv::INSTALL_CHANGELOG.lock().ok()
        .and_then(|mut g| g.take())
        .unwrap_or_default();

    Ok(changelog)
}
// ── End UvEngine ──────────────────────────────────────────────────────────────

fn is_profile_enabled() -> bool {
    UV_FFI_PROFILE_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

static UV_FFI_PROFILE_ENABLED: std::sync::atomic::AtomicBool = 
    std::sync::atomic::AtomicBool::new(false);

fn init_profile() {
    let enabled = std::env::var("UV_FFI_PROFILE").map(|v| v == "1").unwrap_or(false);
    UV_FFI_PROFILE_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
    uv::FFI_PROFILE.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

macro_rules! prof {
    ($label:expr, $t:expr) => {
        if is_profile_enabled() {
            eprintln!("[UV-PROFILE] {}: {:.2}ms",
                $label, $t.elapsed().as_secs_f64() * 1000.0);
        }
    };
}

// ── Build a Cli struct for `pip install` without touching clap ────────────────
// Covers every flag omnipkg actually passes. Falls back to clap for anything
// else (pip freeze, pip uninstall, etc.).
struct FfiInstallOpts {
    packages:        Vec<String>,
    python:          Option<String>,
    reinstall:       bool,
    index_url:       Option<String>,
    extra_index_url: Option<String>,
    link_mode:       Option<uv_install_wheel::LinkMode>,
    quiet:           bool,
    target_dir:      Option<String>,
}

fn try_parse_ffi_install(cmd: &str) -> Option<FfiInstallOpts> {
    // Must be exactly "pip install ..."
    let rest = cmd.strip_prefix("pip install ")?;

    let mut opts = FfiInstallOpts {
        packages:        vec![],
        python:          None,
        reinstall:       false,
        index_url:       None,
        extra_index_url: None,
        link_mode:       None,
        quiet:           false,
        target_dir:      None,
    };

    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "-q" | "--quiet" => {
                opts.quiet = true;
                i += 1;
            }
            "--python" => {
                i += 1;
                opts.python = tokens.get(i).map(|s| s.to_string());
                i += 1;
            }
            "--cache-dir" => {
                // uv reads this from TopLevelArgs/CacheArgs; we ignore it here
                // because the daemon always passes a fixed cache dir via env.
                i += 2;
            }
            "--index-url" | "-i" => {
                i += 1;
                opts.index_url = tokens.get(i).map(|s| s.to_string());
                i += 1;
            }
            "--extra-index-url" => {
                i += 1;
                opts.extra_index_url = tokens.get(i).map(|s| s.to_string());
                i += 1;
            }
            "--reinstall" | "--force-reinstall" => {
                opts.reinstall = true;
                i += 1;
            }
            "--target" | "-t" => {
                i += 1;
                opts.target_dir = tokens.get(i).map(|s| s.to_string());
                i += 1;
            }
            "--link-mode" => {
                i += 1;
                opts.link_mode = tokens.get(i).and_then(|s| match *s {
                    "symlink"  => Some(uv_install_wheel::LinkMode::Symlink),
                    "hardlink" => Some(uv_install_wheel::LinkMode::Hardlink),
                    "clone"    => Some(uv_install_wheel::LinkMode::Clone),
                    "copy"     => Some(uv_install_wheel::LinkMode::Copy),
                    _          => None,
                });
                i += 1;
            }
            tok if !tok.starts_with('-') => {
                opts.packages.push(tok.to_string());
                i += 1;
            }
            _ => return None, // unknown flag → fall back to clap
        }
    }

    if opts.packages.is_empty() {
        return None;
    }
    Some(opts)
}


fn build_fast_cli(opts: FfiInstallOpts) -> uv_cli::Cli {
    use uv_cli::{
        TopLevelArgs, GlobalArgs,
        Commands, PipNamespace, PipCommand,
        PipInstallArgs,
    };

    let mut global = GlobalArgs::default();
    global.quiet = 2; // Printer::Silent — suppresses all progress output

    uv_cli::Cli {
        top_level: {
            let mut tl = TopLevelArgs::default();
            tl.cache_args  = Box::new(uv_cache::CacheArgs::default());
            tl.global_args = Box::new(global);
            tl.no_config   = true;
            tl
        },
        command: Box::new(Commands::Pip(PipNamespace {
            command: PipCommand::Install(
                PipInstallArgs::ffi_new(
                    opts.packages,
                    opts.python,
                    opts.reinstall,
                    opts.index_url,
                    opts.extra_index_url,
                    opts.link_mode,
                    false,
                )
            ),
        })),
    }
}

fn run_uv_internal(cmd: &str) -> (i32, Option<Changelog>, String) {
    static PROFILE_INIT: std::sync::Once = std::sync::Once::new();
    PROFILE_INIT.call_once(init_profile);

    // ── Engine fast path: bypass uv::run() entirely ───────────────────────
    if let Some(opts) = try_parse_ffi_install(cmd) {
        if let Some(ref python) = opts.python {
            let _t = std::time::Instant::now();
            let _t_blockon = std::time::Instant::now();
            let result = RUNTIME.block_on(run_pip_install_direct(
                opts.packages,
                opts.reinstall,
                opts.link_mode.map(|lm| match lm {
                    uv_install_wheel::LinkMode::Symlink  => "symlink".to_string(),
                    uv_install_wheel::LinkMode::Hardlink => "hardlink".to_string(),
                    uv_install_wheel::LinkMode::Clone    => "clone".to_string(),
                    uv_install_wheel::LinkMode::Copy     => "copy".to_string(),
                }),
                opts.index_url,
                opts.extra_index_url,
                python,
                opts.target_dir,
            ));
            prof!("post-block_on", _t_blockon);
            prof!("post-run_uv (engine)", _t);
            return match result {
                Ok(cl)  => (0, Some(cl), String::new()),
                Err(e)  => { let msg = format!("{:?}", e); eprintln!("[UV-FFI] error: {}", msg); (1, None, msg) },
            };
        }
    }
    // ── Fallback: uv::run() path ──────────────────────────────────────────

    let add_quiet = !is_profile_enabled() && !cmd.contains(" -q");
    let effective_cmd = if add_quiet {
        format!("{} -q", cmd)
    } else {
        cmd.to_string()
    };

    let _t_parse = std::time::Instant::now();

    let cli = if let Some(opts) = try_parse_ffi_install(&effective_cmd) {
        prof!("clap-parse (bypassed)", _t_parse);
        build_fast_cli(opts)
    } else {
        // Slow path — pip freeze, pip uninstall, etc.
        let args: Vec<OsString> = std::iter::once(OsString::from("uv"))
            .chain(effective_cmd.split_whitespace().map(OsString::from))
            .collect();
        match uv_cli::Cli::try_parse_from(&args) {
            Ok(c)  => { prof!("clap-parse (slow)", _t_parse); c }
            Err(e) => return (1, None, format!("clap parse error: {}", e)),
        }
    };

    let _t_run = std::time::Instant::now();
    let res = RUNTIME.block_on(Box::pin(uv::run(cli)));
    let (rc, slow_err) = match res {
        Ok(uv::commands::ExitStatus::Success) => (0, String::new()),
        Ok(uv::commands::ExitStatus::Failure) => (1, "resolution or install error".to_string()),
        Ok(uv::commands::ExitStatus::Error) => (1, "internal uv error".to_string()),
        Ok(uv::commands::ExitStatus::External(code)) => (1, format!("external process exited with code {}", code)),
        Err(e) => (1, format!("{:?}", e)),
    };
    prof!("post-await", _t_run);

    let changelog = uv::INSTALL_CHANGELOG.lock().ok().and_then(|mut g| g.take());
    (rc, changelog, slow_err)
}

fn dist_entry(d: &ChangedDist) -> (String, String) {
    let name = d.name().to_string();
    let ver  = d.version()
        .map(|v: &uv_pep440::Version| v.to_string())
        .unwrap_or_default();
    (name, ver)
}
/// Return the current in-memory site-packages state as a [(name, version)] list.
/// Returns an empty list if the cache has not been populated yet.

fn run_uv(cmd: &str) -> (i32, Option<Changelog>, String) {
    let (rc, changelog, err) = run_uv_internal(cmd);

    if rc == 0 {
        return (rc, changelog, err);
    }

    if cmd.contains("install") {
        // Stale site-packages cache: uv tried to read dist-info that no longer
        // exists on disk (external deletion, bubble swap, etc).
        // Force a full rescan so the next attempt sees disk reality.
        if err.contains("dist-info") && err.contains("No such file or directory") {
            eprintln!("[UV-FFI] Stale site-packages cache detected (dist-info missing). Forcing rescan and retrying...");
            uv::FORCE_RESCAN.store(true, std::sync::atomic::Ordering::SeqCst);
            return run_uv_internal(cmd);
        }

        // Registry cache stale: newly published package version not in RAM.
        // Clear the registry client and retry once.
        if is_profile_enabled() {
            eprintln!("[UV-FFI] Install failed (rc={}). Forcing registry reset and retrying...", rc);
        }
        if let Ok(mut g) = uv::REGISTRY_CLIENT.lock() {
            *g = None;
        }
        return run_uv_internal(cmd);
    }

    (rc, changelog, err)
}

#[pyo3::pyfunction]
fn get_site_packages_cache() -> Vec<(String, String)> {
    let Ok(guard) = uv::SITE_PACKAGES_CACHE.try_lock() else {
        return vec![];
    };
    let Some(ref sp) = *guard else {
        return vec![];
    };
    sp.iter()
        .map(|d| (d.name().to_string(), d.version().to_string()))
        .collect()
}

#[pyo3::pyfunction]
fn run(cmd: &str) -> pyo3::PyResult<(i32, Vec<(String, String)>, Vec<(String, String)>, String)> {
    let _t = std::time::Instant::now();
    let (rc, changelog, err) = run_uv(cmd);
    prof!("post-run_uv", _t);
    match changelog {
        Some(cl) => Ok((rc,
            cl.installed.iter().map(dist_entry).collect(),
            cl.uninstalled.iter().map(dist_entry).collect(),
            err,
        )),
        None => Ok((rc, vec![], vec![], err)),
    }
}

/// Full cache invalidation — forces a disk rescan on the next install call.
/// Use patch_site_packages_cache() instead whenever the changelog is known;
/// this is the nuclear option kept for edge cases.
#[pyo3::pyfunction]
fn invalidate_site_packages_cache() {
    uv::FORCE_RESCAN.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Surgically update the in-memory SITE_PACKAGES_CACHE with the delta from
/// an external install (e.g. vanilla `uv pip install` run outside omnipkg).
///
/// Args:
///   installed: list of [name, version] — packages that now exist on disk
///   removed:   list of [name, version] — packages that no longer exist on disk
///
/// Accepts both Python lists and tuples for the inner pairs.
/// Returns True if the cache was patched, False if no cache was live.
#[pyo3::pyfunction]
fn patch_site_packages_cache(
    installed: Vec<Vec<String>>,
    removed:   Vec<Vec<String>>,
) -> bool {
    use uv_distribution_types::{InstalledDist, InstalledDistKind, InstalledRegistryDist};
    use uv_normalize::PackageName;
    use uv_pep440::Version;
    use std::str::FromStr;

    let Ok(mut sp_guard) = uv::SITE_PACKAGES_CACHE.try_lock() else {
        // Cache is being written by an install in flight — skip, it'll be
        // up to date when it finishes.
        return false;
    };
    let Some(ref mut sp) = *sp_guard else {
        // Cache not populated yet — nothing to patch, first install will
        // do a fresh scan anyway.
        return false;
    };

    // Apply removals first so a swap (remove old + add new) stays coherent.
    for pair in &removed {
        if pair.len() < 1 { continue; }
        if let Ok(pkg_name) = PackageName::from_str(&pair[0]) {
            sp.remove_packages(&pkg_name);
        }
    }

    // Apply installs — same zero-I/O construction as install.rs lines 720-738.
    for pair in &installed {
        if pair.len() < 2 { continue; }
        let (Ok(pkg_name), Ok(version)) = (
            PackageName::from_str(&pair[0]),
            Version::from_str(&pair[1]),
        ) else { continue; };

        // Remove any stale entry for this name first (handles version upgrades).
        sp.remove_packages(&pkg_name);

        let dist_info_name = format!(
            "{}-{}.dist-info",
            pkg_name.as_dist_info_name(),
            version,
        );
        let dist_info_path = sp.interpreter().purelib().join(&dist_info_name);

        let dist = InstalledDist::from(InstalledDistKind::Registry(InstalledRegistryDist {
            name:       pkg_name,
            version,
            path:       dist_info_path.into_boxed_path(),
            cache_info: None,
            build_info: None,
        }));
        sp.add_dist(dist);
    }

    true
}

#[pyo3::pyfunction]
fn clear_registry_cache() {
    if let Ok(mut g) = uv::REGISTRY_CLIENT.lock() {
        *g = None;
    }
}

#[pyo3::pymodule]
fn uv_ffi(_py: pyo3::Python, m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(run, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(invalidate_site_packages_cache, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(patch_site_packages_cache, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(get_site_packages_cache, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(clear_registry_cache, m)?)?;
    Ok(())
}

#[no_mangle]
pub extern "C" fn omnipkg_uv_run_c(
    cmd_ptr: *const std::os::raw::c_char,
    out_json: *mut std::os::raw::c_char,
    max_out: std::os::raw::c_int,
) -> std::os::raw::c_int {
    let c_str = unsafe { std::ffi::CStr::from_ptr(cmd_ptr) };
    let cmd = c_str.to_str().unwrap_or("");
    let (rc, changelog, _err) = run_uv(cmd);

    let mut res = String::with_capacity(128);
    res.push_str("{\"installed\":[");
    if let Some(ref cl) = changelog {
        for (i, (n, v)) in cl.installed.iter().map(dist_entry).enumerate() {
            if i > 0 { res.push(','); }
            res.push_str(&format!("[\"{}\",\"{}\"]", n, v));
        }
    }
    res.push_str("],\"removed\":[");
    if let Some(ref cl) = changelog {
        for (i, (n, v)) in cl.uninstalled.iter().map(dist_entry).enumerate() {
            if i > 0 { res.push(','); }
            res.push_str(&format!("[\"{}\",\"{}\"]", n, v));
        }
    }
    res.push_str("]}");

    let bytes = res.as_bytes();
    let len = std::cmp::min(bytes.len(), (max_out - 1) as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_json as *mut u8, len);
        *out_json.add(len) = 0;
    }
    rc
}