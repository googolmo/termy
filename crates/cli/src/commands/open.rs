use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run(path: Option<PathBuf>, new_window: bool, new_tab: bool) {
    match launch_termy(path, new_window, new_tab) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn launch_termy(path: Option<PathBuf>, new_window: bool, new_tab: bool) -> Result<(), String> {
    let working_dir = path.as_deref().map(resolve_working_dir).transpose()?;
    let app_binary = find_termy_app_binary()?;

    let mut command = Command::new(&app_binary);
    if new_window {
        command.arg("--new-window");
    } else if new_tab {
        command.arg("--new-tab");
    }
    if let Some(working_dir) = working_dir {
        command.arg("--working-directory").arg(working_dir);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Failed to launch {}: {error}", app_binary.display()))?;

    Ok(())
}

fn resolve_working_dir(path: &Path) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("Failed to resolve current directory: {error}"))?
            .join(path)
    };

    let path = path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve {}: {error}", path.display()))?;

    if !path.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }

    Ok(path)
}

fn find_termy_app_binary() -> Result<PathBuf, String> {
    let reported_exe_path =
        std::env::current_exe().map_err(|error| format!("Failed to resolve CLI path: {error}"))?;
    find_app_for_cli(reported_exe_path, sibling_app_binary_names())
}

fn find_app_for_cli(reported_exe_path: PathBuf, sibling_names: &[&str]) -> Result<PathBuf, String> {
    let exe_path = resolve_executable_path(reported_exe_path);
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| format!("CLI path {} has no parent directory", exe_path.display()))?;

    // Debian/RPM put the CLI in lib/termy and the launcher in bin. Prefer
    // that launcher so its X11/Wayland environment setup is preserved.
    if exe_dir.ends_with("lib/termy") {
        let launcher = resolve_executable_path(exe_dir.join("../../bin/termy"));
        if is_executable_file(&launcher) && launcher != exe_path {
            return Ok(launcher);
        }
    }

    for sibling_name in sibling_names {
        let sibling = exe_dir.join(sibling_name);
        if is_executable_file(&sibling) && sibling != exe_path {
            return Ok(sibling);
        }
    }

    let app_binary_name = format!("termy{}", std::env::consts::EXE_SUFFIX);
    for candidate in fallback_termy_app_binary_paths(&app_binary_name) {
        if is_executable_file(&candidate) && candidate != exe_path {
            return Ok(candidate);
        }
    }

    Err("Termy app binary not found. Build it with: cargo build -p termy".to_string())
}

fn resolve_executable_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn sibling_app_binary_names() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &["Termy", "termy"]
    }

    #[cfg(target_os = "windows")]
    {
        &["termy.exe"]
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        LINUX_APP_BINARY_NAMES
    }
}

#[cfg(any(not(any(target_os = "macos", target_os = "windows")), all(test, unix)))]
const LINUX_APP_BINARY_NAMES: &[&str] = &["termy", "termy-bin"];

fn fallback_termy_app_binary_paths(app_binary_name: &str) -> [PathBuf; 2] {
    [
        PathBuf::from("target/debug").join(app_binary_name),
        PathBuf::from("target/release").join(app_binary_name),
    ]
}

fn is_executable_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
mod tests {
    use super::{resolve_executable_path, resolve_working_dir, sibling_app_binary_names};

    #[cfg(unix)]
    #[test]
    fn packaged_linux_cli_finds_gui_after_resolving_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("usr/lib/termy");
        let bin = temp.path().join("usr/bin");
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bundle.join("termy-cli"), b"cli").unwrap();
        std::fs::write(bundle.join("termy-bin"), b"gui").unwrap();
        std::os::unix::fs::symlink("../lib/termy/termy-cli", bin.join("termy-cli")).unwrap();
        assert_eq!(
            super::find_app_for_cli(bin.join("termy-cli"), super::LINUX_APP_BINARY_NAMES).unwrap(),
            bundle.join("termy-bin").canonicalize().unwrap()
        );
        std::fs::write(bin.join("termy"), b"system launcher").unwrap();
        assert_eq!(
            super::find_app_for_cli(bin.join("termy-cli"), super::LINUX_APP_BINARY_NAMES).unwrap(),
            bin.join("termy").canonicalize().unwrap()
        );
        std::fs::remove_file(bin.join("termy")).unwrap();
        // Tarball installations should keep using the launcher when available.
        std::fs::write(bundle.join("termy"), b"launcher").unwrap();
        assert_eq!(
            super::find_app_for_cli(bin.join("termy-cli"), super::LINUX_APP_BINARY_NAMES).unwrap(),
            bundle.join("termy").canonicalize().unwrap()
        );
    }

    #[test]
    fn open_resolves_existing_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let resolved = resolve_working_dir(temp.path()).expect("directory should resolve");
        assert_eq!(resolved, temp.path().canonicalize().unwrap());
    }

    #[test]
    fn open_rejects_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "content").expect("write file");
        let error = resolve_working_dir(&file).expect_err("file should be rejected");
        assert!(error.contains("is not a directory"));
    }

    #[test]
    fn bundled_native_app_binary_is_a_sibling_candidate() {
        #[cfg(target_os = "macos")]
        assert_eq!(sibling_app_binary_names().first(), Some(&"Termy"));
    }

    #[cfg(unix)]
    #[test]
    fn installed_cli_symlink_resolves_back_to_bundle() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let bundled = temp.path().join("Termy.app/Contents/MacOS/termy-cli");
        std::fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        std::fs::write(&bundled, b"cli").unwrap();
        let installed = temp.path().join("home/.local/bin/termy");
        std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
        symlink(&bundled, &installed).unwrap();

        assert_eq!(
            resolve_executable_path(installed),
            bundled.canonicalize().unwrap()
        );
    }
}
