use anyhow::{Context, Result};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

const EXECUTABLES: &[&str] = &["termy-bin", "termy-cli", "termy"];

pub(super) fn install_archive(archive: &Path, destination: &Path) -> Result<()> {
    let extracted = tempfile::Builder::new()
        .prefix("termy-update-extract-")
        .tempdir()
        .context("Failed to create extraction directory")?;
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(extracted.path())
        .output()
        .context("Failed to extract tarball")?;
    if !output.status.success() {
        anyhow::bail!(
            "tar extraction failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut candidates = vec![
        extracted.path().join("termy"),
        extracted.path().to_path_buf(),
    ];
    for entry in fs::read_dir(extracted.path())? {
        candidates.push(entry?.path());
    }
    let payload = candidates
        .into_iter()
        .find(|dir| {
            EXECUTABLES.iter().all(|name| {
                fs::symlink_metadata(dir.join(name)).is_ok_and(|metadata| metadata.is_file())
            })
        })
        .context("Update must contain termy, termy-bin, and termy-cli")?;

    fs::create_dir_all(destination).context("Failed to create install directory")?;
    super::staged_install::install_staged(destination, EXECUTABLES, |stage| {
        for name in EXECUTABLES {
            let target = stage.join(name);
            fs::copy(payload.join(name), &target)
                .with_context(|| format!("Failed to stage {name}"))?;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_archive(root: &Path, names: &[&str]) -> std::path::PathBuf {
        let payload = root.join("payload/termy");
        fs::create_dir_all(&payload).unwrap();
        for name in names {
            fs::write(payload.join(name), format!("new {name}")).unwrap();
        }
        let archive = root.join("update.tar.gz");
        assert!(
            Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(root.join("payload"))
                .arg("termy")
                .status()
                .unwrap()
                .success()
        );
        archive
    }

    #[test]
    fn updates_launcher_and_both_executables() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("bin");
        fs::create_dir(&destination).unwrap();
        for name in EXECUTABLES {
            fs::write(destination.join(name), "old").unwrap();
        }
        // Holding the previous executable open must not expose partial writes.
        let old_binary = fs::File::open(destination.join("termy-bin")).unwrap();
        let archive = make_archive(root.path(), EXECUTABLES);
        install_archive(&archive, &destination).unwrap();
        for name in EXECUTABLES {
            let path = destination.join(name);
            assert_eq!(fs::read_to_string(&path).unwrap(), format!("new {name}"));
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        use std::io::Read;
        let mut previous = String::new();
        (&old_binary).read_to_string(&mut previous).unwrap();
        assert_eq!(previous, "old");
    }

    #[test]
    fn incomplete_archive_keeps_existing_installation() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("bin");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("termy"), "old").unwrap();
        let archive = make_archive(root.path(), &["termy"]);
        assert!(install_archive(&archive, &destination).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("termy")).unwrap(),
            "old"
        );
        assert_eq!(fs::read_dir(destination).unwrap().count(), 1);
    }
}
