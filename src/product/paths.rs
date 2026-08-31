use std::{
    fs, io,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
};

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
