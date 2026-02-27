use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn abi_conformance_c_fixture() -> TestResult {
    let Some(compiler) = first_available_tool(&["cc", "clang", "gcc"]) else {
        eprintln!("skipping C ABI fixture test: no C compiler found");
        return Ok(());
    };

    let fixture_root = fixture_root();
    let include_dir = fixture_root.join("include");
    let source = fixture_root.join("c/abi_contract.c");
    let build_dir = tempfile::tempdir()?;
    let output = build_dir.path().join("ffi_abi_c_fixture");

    let (_, library_dirs) = ffi_library()?;

    let mut cmd = Command::new(compiler);
    cmd.arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(&source)
        .arg("-I")
        .arg(&include_dir);
    add_library_dirs(&mut cmd, &library_dirs);
    add_rpath_flags(&mut cmd, &library_dirs);
    cmd.arg("-lstateset_ffi");
    if cfg!(target_os = "linux") {
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
    }
    cmd.arg("-o").arg(&output);
    run_command(&mut cmd)?;

    run_executable(&output, &library_dirs)
}

#[test]
fn abi_conformance_cpp_fixture() -> TestResult {
    let Some(compiler) = first_available_tool(&["c++", "clang++", "g++"]) else {
        eprintln!("skipping C++ ABI fixture test: no C++ compiler found");
        return Ok(());
    };

    let fixture_root = fixture_root();
    let include_dir = fixture_root.join("include");
    let source = fixture_root.join("cpp/abi_contract.cpp");
    let build_dir = tempfile::tempdir()?;
    let output = build_dir.path().join("ffi_abi_cpp_fixture");

    let (_, library_dirs) = ffi_library()?;

    let mut cmd = Command::new(compiler);
    cmd.arg("-std=c++17")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg(&source)
        .arg("-I")
        .arg(&include_dir);
    add_library_dirs(&mut cmd, &library_dirs);
    add_rpath_flags(&mut cmd, &library_dirs);
    cmd.arg("-lstateset_ffi");
    if cfg!(target_os = "linux") {
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
    }
    cmd.arg("-o").arg(&output);
    run_command(&mut cmd)?;

    run_executable(&output, &library_dirs)
}

#[test]
fn python_ctypes_smoke_fixture() -> TestResult {
    let Some(python) = first_available_tool(&["python3", "python"]) else {
        eprintln!("skipping Python ctypes fixture test: no Python interpreter found");
        return Ok(());
    };

    let fixture = fixture_root().join("python/ctypes_smoke.py");
    let (library, library_dirs) = ffi_library()?;

    let mut cmd = Command::new(python);
    cmd.arg(&fixture).arg(&library);
    set_runtime_library_env(&mut cmd, &library_dirs)?;
    run_command(&mut cmd)
}

#[test]
fn swift_ffi_smoke_fixture() -> TestResult {
    let Some(swiftc) = first_available_tool(&["swiftc"]) else {
        eprintln!("skipping Swift FFI fixture test: swiftc not found");
        return Ok(());
    };

    let fixture = fixture_root().join("swift/ffi_smoke.swift");
    let build_dir = tempfile::tempdir()?;
    let output = build_dir.path().join("ffi_swift_smoke");
    let (_, library_dirs) = ffi_library()?;

    let mut compile = Command::new(swiftc);
    compile.arg(&fixture);
    add_library_dirs(&mut compile, &library_dirs);
    for dir in &library_dirs {
        compile.arg("-Xlinker").arg("-rpath").arg("-Xlinker").arg(dir);
    }
    compile.arg("-lstateset_ffi").arg("-o").arg(&output);
    run_command(&mut compile)?;

    run_executable(&output, &library_dirs)
}

fn run_executable(path: &Path, library_dirs: &[PathBuf]) -> TestResult {
    let mut cmd = Command::new(path);
    set_runtime_library_env(&mut cmd, library_dirs)?;
    run_command(&mut cmd)
}

fn ffi_library() -> TestResult<(PathBuf, Vec<PathBuf>)> {
    let dirs = library_search_dirs();

    if let Some(path) = dylib_candidates(&dirs).into_iter().find(|path| path.exists()) {
        return Ok((path, dirs));
    }

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut build = Command::new(cargo);
    build.current_dir(workspace_root()).arg("build").arg("-p").arg("stateset-ffi");
    run_command(&mut build)?;

    if let Some(path) = dylib_candidates(&dirs).into_iter().find(|path| path.exists()) {
        return Ok((path, dirs));
    }

    let tried = dylib_candidates(&dirs);
    Err(format!(
        "unable to locate stateset-ffi shared library after build; tried: {}",
        tried.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
    )
    .into())
}

fn dylib_candidates(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter().map(|dir| dir.join(dylib_name())).collect()
}

fn dylib_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "libstateset_ffi.dylib"
    } else if cfg!(target_os = "windows") {
        "stateset_ffi.dll"
    } else {
        "libstateset_ffi.so"
    }
}

fn library_search_dirs() -> Vec<PathBuf> {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"));
    let profile_dir = target_dir.join(profile);
    vec![profile_dir.clone(), profile_dir.join("deps")]
}

fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("stateset-ffi should live at workspace/crates/stateset-ffi")
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("abi-fixtures")
}

fn add_library_dirs(cmd: &mut Command, dirs: &[PathBuf]) {
    for dir in dirs {
        cmd.arg("-L").arg(dir);
    }
}

fn add_rpath_flags(cmd: &mut Command, dirs: &[PathBuf]) {
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        for dir in dirs {
            cmd.arg(format!("-Wl,-rpath,{}", dir.display()));
        }
    }
}

fn set_runtime_library_env(cmd: &mut Command, dirs: &[PathBuf]) -> TestResult {
    let key = runtime_library_env_key();
    let mut paths = dirs.to_vec();
    if let Some(existing) = env::var_os(key) {
        paths.extend(env::split_paths(&existing));
    }

    let joined = env::join_paths(paths)?;
    cmd.env(key, joined);
    Ok(())
}

fn runtime_library_env_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "DYLD_LIBRARY_PATH"
    } else if cfg!(target_os = "windows") {
        "PATH"
    } else {
        "LD_LIBRARY_PATH"
    }
}

fn first_available_tool(candidates: &[&str]) -> Option<String> {
    for candidate in candidates {
        if Command::new(candidate).arg("--version").output().is_ok() {
            return Some((*candidate).to_owned());
        }
    }
    None
}

fn run_command(cmd: &mut Command) -> TestResult {
    let display = format!("{cmd:?}");
    let output = cmd.output()?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "command failed: {display}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status, stdout, stderr
    )
    .into())
}
