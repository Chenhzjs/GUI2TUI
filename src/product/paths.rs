use std::{
    fs, io,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompanionLocation {
    InstalledLibexec,
    BuildSibling,
    SourceScript,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Companion {
    pub path: PathBuf,
    pub location: CompanionLocation,
}

fn executable_file(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(io::Error::other("component is not an executable file"));
    }
    Ok(())
}

/// Resolve a private component relative to the running executable. Installed
/// layouts never depend on the current directory or a source checkout.
pub fn companion(name: &str) -> io::Result<Companion> {
    let current = std::env::current_exe()?;
    let bin = current
        .parent()
        .ok_or_else(|| io::Error::other("Current executable has no parent"))?;
    let installed = bin.join("../libexec/gui2tui").join(name);
    if bin.file_name().is_some_and(|directory| directory == "bin") {
        executable_file(&installed)?;
        return Ok(Companion {
            path: installed,
            location: CompanionLocation::InstalledLibexec,
        });
    }
    let sibling = bin.join(name);
    if executable_file(&sibling).is_ok() {
        return Ok(Companion {
            path: sibling,
            location: CompanionLocation::BuildSibling,
        });
    }
    // Developer builds place binaries in target/{debug,release} and retain
    // the bounded shell helper in scripts. A packaged user-prefix install is
    // resolved above and never reaches this fallback.
    if let Some(target_dir) = bin.parent()
        && let Some(project_dir) = target_dir.parent()
    {
        let path = project_dir.join("scripts").join(name);
        if executable_file(&path).is_ok() {
            return Ok(Companion {
                path,
                location: CompanionLocation::SourceScript,
            });
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Required internal component '{name}' is missing or not executable"),
    ))
}

pub fn current_executable_is_usable() -> io::Result<()> {
    executable_file(&std::env::current_exe()?)
}

/// A user-prefix install created by the bundled installer carries this private
/// marker for exact-file uninstall. Developer and manually extracted layouts
/// intentionally return `None`.
pub fn user_install_manifest() -> io::Result<Option<PathBuf>> {
    let current = std::env::current_exe()?;
    let Some(bin) = current.parent() else {
        return Ok(None);
    };
    let path = bin.join("../libexec/gui2tui/install-manifest-v1");
    match fs::symlink_metadata(&path) {
        Ok(metadata)
            if metadata.is_file()
                && metadata.uid() == rustix::process::geteuid().as_raw()
                && metadata.nlink() == 1
                && metadata.mode() & 0o777 == 0o600 =>
        {
            Ok(Some(path))
        }
        Ok(_) => Err(io::Error::other(
            "User installation manifest is not a private current-user regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn config_path_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> io::Result<PathBuf> {
    let base = match xdg.filter(|p| p.is_absolute()) {
        Some(base) => base,
        None => home
            .filter(|p| p.is_absolute())
            .ok_or_else(|| io::Error::other("Set HOME or an absolute XDG_CONFIG_HOME"))?
            .join(".config"),
    };
    Ok(base.join("gui2tui/config.toml"))
}

pub fn config_path() -> io::Result<PathBuf> {
    config_path_from(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

pub fn verify_private_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::other(
            "Runtime directory must be owned by the current user, not a symlink, and mode 0700",
        ));
    }
    Ok(())
}

/// XDG is preferred; a missing XDG runtime is normal in SSH sessions.
/// An explicitly unsafe XDG path is rejected, not silently bypassed.
pub fn runtime_dir() -> io::Result<PathBuf> {
    let root = if let Some(base) = std::env::var_os("XDG_RUNTIME_DIR") {
        let base = PathBuf::from(base);
        if !base.is_absolute() {
            return Err(io::Error::other("XDG_RUNTIME_DIR must be absolute"));
        }
        verify_private_directory(&base)?;
        base.join("gui2tui")
    } else {
        std::env::temp_dir().join(format!(
            "gui2tui-runtime-{}",
            rustix::process::geteuid().as_raw()
        ))
    };
    match fs::DirBuilder::new().mode(0o700).create(&root) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    verify_private_directory(&root)?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xdg_paths_override_home_and_relative_xdg_is_ignored() {
        assert_eq!(
            config_path_from(Some("/xdg".into()), Some("/home/test".into())).unwrap(),
            PathBuf::from("/xdg/gui2tui/config.toml")
        );
        assert_eq!(
            config_path_from(Some("relative".into()), Some("/home/test".into())).unwrap(),
            PathBuf::from("/home/test/.config/gui2tui/config.toml")
        );
        assert!(config_path_from(None, None).is_err());
    }
    #[test]
    fn runtime_rejects_symlink_and_public_directory() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        let link = temp.path().join("link");
        symlink(temp.path(), &link).unwrap();
        assert!(verify_private_directory(&link).is_err());
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(verify_private_directory(temp.path()).is_err());
    }
}
