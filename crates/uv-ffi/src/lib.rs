use std::os::raw::c_int;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("uv-ffi: failed to build tokio runtime")
});
static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

use uv::commands;
use uv::commands::ExitStatus;
use uv::printer::Printer;
use uv::settings::PipInstallSettings;
use uv_cache::Cache;
use uv_client::{BaseClientBuilder, Connectivity};
use uv_configuration::{Concurrency, DryRun};
use uv_preview::Preview;
use uv_python::{PythonDownloads, PythonPreference};
use uv_requirements::RequirementsSource;
use uv_requirements::specification::GroupsSpecification;
use uv_settings::EnvironmentOptions;
use uv_cli::{
    Maybe,
    PipInstallArgs,
    RefreshArgs,
    ResolverInstallerArgs,
    IndexArgs,
    compat::PipInstallCompatArgs,
};

struct ParsedCmd {
    packages:   Vec<String>,
    python:     Option<String>,
    target:     Option<String>,
    is_upgrade: bool,
    is_dry_run: bool,
    no_deps:    bool,
}

fn parse_cmd(cmd: &str) -> ParsedCmd {
    let mut packages   = Vec::new();
    let mut python     = None;
    let mut target     = None;
    let mut is_upgrade = false;
    let mut is_dry_run = false;
    let mut no_deps    = false;
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() && matches!(tokens[i], "pip" | "install") { i += 1; }
    while i < tokens.len() {
        match tokens[i] {
            "--python" if i + 1 < tokens.len() => { python = Some(tokens[i+1].to_string()); i += 2; }
            "--target" if i + 1 < tokens.len() => { target = Some(tokens[i+1].to_string()); i += 2; }
            "--cache-dir" | "--index-url" | "--extra-index-url" | "--link-mode"
                if i + 1 < tokens.len() => { i += 2; }
            "--upgrade" | "-U"  => { is_upgrade = true; i += 1; }
            "--dry-run"          => { is_dry_run = true; i += 1; }
            "--no-deps"          => { no_deps    = true; i += 1; }
            f if f.starts_with('-') => { i += 1; }
            pkg => { packages.push(pkg.to_string()); i += 1; }
        }
    }
    ParsedCmd { packages, python, target, is_upgrade, is_dry_run, no_deps }
}

fn run_install(cmd: &str) -> i32 {
    let p = parse_cmd(cmd);
    if p.packages.is_empty() { return 1; }

    let args = PipInstallArgs {
        package:                  p.packages.clone(),
        requirements:             vec![],
        editable:                 vec![],
        constraints:              vec![],
        overrides:                vec![],
        excludes:                 vec![],
        build_constraints:        vec![],
        extra:                    None,
        all_extras:               false,
        no_all_extras:            false,
        group:                    vec![],
        installer: ResolverInstallerArgs {
            index_args:           IndexArgs {
                                      index:         None,
                                      default_index: None,
                                      index_url:     None,
                                      extra_index_url: None,
                                      no_index:      false,
                                      find_links:    None,
                                  },
            upgrade:              p.is_upgrade,
            no_upgrade:           false,
            upgrade_package:      vec![],
            reinstall:            false,
            no_reinstall:         false,
            reinstall_package:    vec![],
            index_strategy:       None,
            keyring_provider:     None,
            resolution:           None,
            prerelease:           None,
            fork_strategy:        None,
            pre:                  false,
            config_setting:       None,
            config_settings_package: None,
            no_build_isolation:   false,
            no_build_isolation_package: vec![],
            build_isolation:      false,
            exclude_newer:        None,
            exclude_newer_package: None,
            link_mode:            None,
            compile_bytecode:     false,
            no_compile_bytecode:  false,
            no_sources:           false,
            no_sources_package:   vec![],
        },
        refresh: RefreshArgs {
            refresh:              false,
            no_refresh:           false,
            refresh_package:      vec![],
        },
        no_deps:                  p.no_deps,
        deps:                     false,
        require_hashes:           false,
        no_require_hashes:        false,
        verify_hashes:            false,
        no_verify_hashes:         false,
        // Option<Maybe<String>> — wrap the inner String
        python:                   p.python.map(Maybe::Some),
        system:                   false,
        no_system:                false,
        break_system_packages:    false,
        no_break_system_packages: false,
        // Option<PathBuf>
        target:                   p.target.map(PathBuf::from),
        prefix:                   None,
        no_build:                 false,
        build:                    false,
        // Option<Vec<PackageNameSpecifier>>
        no_binary:                None,
        only_binary:              None,
        python_version:           None,
        python_platform:          None,
        inexact:                  false,
        exact:                    false,
        strict:                   false,
        no_strict:                false,
        dry_run:                  p.is_dry_run,
        torch_backend:            None,
        // PipInstallCompatArgs fields are private — use mem::zeroed (all bool = false)
        compat_args:              unsafe { std::mem::zeroed::<PipInstallCompatArgs>() },
    };

    let env = match EnvironmentOptions::new() {
        Ok(e)  => e,
        Err(_) => return 1,
    };

    let mut resolved = PipInstallSettings::resolve(args, None, env);

    if p.is_upgrade {
        resolved.settings.upgrade = uv_configuration::Upgrade::all();
    }

    let requirements: Vec<RequirementsSource> = resolved.package.iter()
        .filter_map(|pkg| RequirementsSource::from_package_argument(pkg).ok())
        .collect();
    if requirements.is_empty() { return 1; }

    let empty: Vec<RequirementsSource> = vec![];
    let cache = Cache::from_path(std::path::PathBuf::from("/home/minds3t/.cache/uv"));

    let client_builder = BaseClientBuilder::new(
        Connectivity::Online,
        false,                          // native_tls
        vec![],                         // allow_insecure_host: Vec<TrustedHost>
        Preview::default(),
        Duration::from_secs(30),        // read_timeout
        Duration::from_secs(10),        // connect_timeout
        3u32,                           // retries
    );

    let dry_run = if p.is_dry_run { DryRun::Enabled } else { DryRun::Disabled };
    let groups  = GroupsSpecification::default();

    let result = RUNTIME.block_on(async {
        commands::pip_install(
            &requirements,
            &empty,
            &empty,
            &empty,
            &empty,
            resolved.constraints_from_workspace,
            resolved.overrides_from_workspace,
            resolved.excludes_from_workspace,
            resolved.build_constraints_from_workspace,
            &resolved.settings.extras,
            &groups,
            resolved.settings.resolution,
            resolved.settings.prerelease,
            resolved.settings.dependency_mode,
            resolved.settings.upgrade,
            resolved.settings.index_locations,
            resolved.settings.index_strategy,
            resolved.settings.torch_backend,
            resolved.settings.dependency_metadata,
            resolved.settings.keyring_provider,
            &client_builder.subcommand(vec!["pip".to_owned(), "install".to_owned()]),
            resolved.settings.reinstall,
            resolved.settings.link_mode,
            resolved.settings.compile_bytecode,
            resolved.settings.hash_checking,
            true,
            &resolved.settings.config_setting,
            &resolved.settings.config_settings_package,
            resolved.settings.build_isolation.clone(),
            &resolved.settings.extra_build_dependencies,
            &resolved.settings.extra_build_variables,
            resolved.settings.build_options,
            resolved.modifications,
            resolved.settings.python_version,
            resolved.settings.python_platform,
            PythonDownloads::default(),
            resolved.settings.install_mirrors,
            resolved.settings.strict,
            resolved.settings.exclude_newer,
            resolved.settings.sources,
            resolved.settings.python,
            resolved.settings.system,
            resolved.settings.break_system_packages,
            resolved.settings.target,
            resolved.settings.prefix,
            PythonPreference::default(),
            Concurrency::default(),
            cache,
            dry_run,
            Printer::Default,
            Preview::default(),
        ).await
    });

    match result {
        Ok(ExitStatus::Success) => 0,
        _ => 1,
    }
}

use pyo3::prelude::*;

/// run("pip install rich==14.3.2") -> (rc: int, stdout: str, stderr: str)
/// Calls pip_install directly — no fork, no CLI re-init, unlimited calls.
#[pyfunction]
fn run(cmd: &str) -> PyResult<(i32, String, String)> {
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    // Capture stderr so callers can parse uv's +/- diff output
    let captured = Arc::new(StdMutex::new(Vec::<u8>::new()));
    let captured2 = captured.clone();

    // Redirect stderr to a pipe for the duration of the call
    let (pipe_r, pipe_w) = {
        let mut fds = [0i32; 2];
        unsafe { libc::pipe(fds.as_mut_ptr()) };
        (fds[0], fds[1])
    };
    let saved_stderr = unsafe { libc::dup(2) };
    unsafe { libc::dup2(pipe_w, 2); libc::close(pipe_w); }

    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let rc = run_install(cmd);

    // Restore stderr
    unsafe { libc::dup2(saved_stderr, 2); libc::close(saved_stderr); }

    // Drain the pipe
    unsafe { libc::fcntl(pipe_r, libc::F_SETFL, libc::O_NONBLOCK); }
    let mut stderr_out = Vec::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = unsafe { libc::read(pipe_r, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n <= 0 { break; }
        stderr_out.extend_from_slice(&buf[..n as usize]);
    }
    unsafe { libc::close(pipe_r); }

    let stderr_str = String::from_utf8_lossy(&stderr_out).into_owned();
    Ok((rc, String::new(), stderr_str))
}

#[pymodule]
fn uv_ffi(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)?;
    Ok(())
}
