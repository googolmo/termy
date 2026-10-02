use anyhow::{Context, Result};
use std::{fs, io, path::Path};

/// Stage on the destination filesystem, then replace entries by rename. Keep
/// originals until every replacement succeeds, and restore them on failure.
pub(super) fn install_staged(
    destination: &Path,
    names: &[&str],
    stage: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    install_with_rename(destination, names, stage, |from, to| fs::rename(from, to))
}

fn install_with_rename(
    destination: &Path,
    names: &[&str],
    stage: impl FnOnce(&Path) -> Result<()>,
    mut rename: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<()> {
    let workspace = tempfile::Builder::new()
        .prefix(".termy-update-")
        .tempdir_in(destination)
        .context("Failed to create update staging directory")?;
    let staged = workspace.path().join("new");
    let backup = workspace.path().join("previous");
    fs::create_dir(&staged)?;
    fs::create_dir(&backup)?;
    stage(&staged)?;
    for name in names {
        fs::symlink_metadata(staged.join(name))
            .with_context(|| format!("Update is missing {name}"))?;
    }

    let mut replaced = Vec::new();
    let result: Result<()> = (|| {
        for name in names {
            let target = destination.join(name);
            let had_original = match fs::symlink_metadata(&target) {
                Ok(_) => true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                Err(error) => return Err(error.into()),
            };
            if had_original {
                rename(&target, &backup.join(name))
                    .with_context(|| format!("Failed to back up {name}"))?;
            }
            replaced.push((*name, had_original, false));
            rename(&staged.join(name), &target)
                .with_context(|| format!("Failed to install {name}"))?;
            replaced.last_mut().unwrap().2 = true;
        }
        Ok(())
    })();

    if let Err(error) = result {
        let mut rollback_errors = Vec::new();
        for (name, had_original, installed) in replaced.into_iter().rev() {
            let target = destination.join(name);
            if installed && let Err(restore_error) = rename(&target, &staged.join(name)) {
                rollback_errors.push(format!("{name}: {restore_error}"));
                continue;
            }
            if had_original && let Err(restore_error) = rename(&backup.join(name), &target) {
                rollback_errors.push(format!("{name}: {restore_error}"));
            }
        }
        if !rollback_errors.is_empty() {
            // Never let TempDir cleanup erase the only surviving originals.
            let recovery = workspace.keep();
            anyhow::bail!(
                "{error:#}; rollback failed ({}). Recovery files retained at {}",
                rollback_errors.join(", "),
                recovery.display()
            );
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_staging_preserves_installed_bundle() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("Termy.app")).unwrap();
        fs::write(root.path().join("Termy.app/version"), "old").unwrap();
        let result = install_staged(root.path(), &["Termy.app"], |stage| {
            fs::create_dir(stage.join("Termy.app"))?;
            fs::write(stage.join("Termy.app/partial"), "incomplete")?;
            anyhow::bail!("copy failed: disk full")
        });
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(root.path().join("Termy.app/version")).unwrap(),
            "old"
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_publish_rolls_back_every_replaced_entry() {
        let root = tempfile::tempdir().unwrap();
        for name in ["termy", "termy-bin"] {
            fs::write(root.path().join(name), "old").unwrap();
        }
        let result = install_with_rename(
            root.path(),
            &["termy", "new-helper", "termy-bin"],
            |stage| {
                for name in ["termy", "new-helper", "termy-bin"] {
                    fs::write(stage.join(name), "new")?;
                }
                Ok(())
            },
            |from, to| {
                if from.ends_with("new/termy-bin") {
                    return Err(io::Error::other("injected publish failure"));
                }
                fs::rename(from, to)
            },
        );
        assert!(result.is_err());
        for name in ["termy", "termy-bin"] {
            assert_eq!(fs::read_to_string(root.path().join(name)).unwrap(), "old");
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn failed_rollback_retains_recovery_files() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("termy"), "old").unwrap();
        let result = install_with_rename(
            root.path(),
            &["termy"],
            |stage| {
                fs::write(stage.join("termy"), "new")?;
                Ok(())
            },
            |from, to| {
                if from.ends_with("new/termy") || from.ends_with("previous/termy") {
                    return Err(io::Error::other("injected failure"));
                }
                fs::rename(from, to)
            },
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Recovery files retained")
        );
        let recovery = fs::read_dir(root.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read_to_string(recovery.join("previous/termy")).unwrap(),
            "old"
        );
    }

    #[test]
    fn successful_bundle_install_replaces_old_contents() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("Termy.app")).unwrap();
        fs::write(root.path().join("Termy.app/obsolete"), "old").unwrap();
        install_staged(root.path(), &["Termy.app"], |stage| {
            fs::create_dir(stage.join("Termy.app"))?;
            fs::write(stage.join("Termy.app/version"), "new")?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("Termy.app/version")).unwrap(),
            "new"
        );
        assert!(!root.path().join("Termy.app/obsolete").exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
