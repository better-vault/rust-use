use super::IpcError;
use interprocess::local_socket::{Listener, ListenerOptions, Name, prelude::*};
use std::{env, io};

/// A cross-platform local socket identified by application name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    name: String,
}

impl Endpoint {
    /// Select an explicit endpoint.
    ///
    /// # Errors
    /// Returns an error unless the name matches `[a-z0-9-]{1,40}`.
    pub fn new(name: &str) -> Result<Self, IpcError> {
        if !(1..=40).contains(&name.len())
            || !name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err(IpcError::InvalidName(name.to_owned()));
        }
        Ok(Self {
            name: name.to_owned(),
        })
    }

    /// Use a valid `RUST_USE_APP`, otherwise sanitize the executable stem.
    pub fn from_env() -> Self {
        if let Some(endpoint) = env::var("RUST_USE_APP")
            .ok()
            .and_then(|s| Self::new(&s).ok())
        {
            return endpoint;
        }
        let stem = env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "app".into());
        Self {
            name: stem
                .chars()
                .take(40)
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect(),
        }
    }

    /// The validated application name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Local file for clients persisting this app's revision cursor.
    pub fn revision_path(&self) -> std::path::PathBuf {
        #[cfg(unix)]
        let dir = directory();
        #[cfg(windows)]
        let dir = env::temp_dir().join("rust-use");
        dir.join(format!("{}.rev", self.name))
    }

    #[cfg(unix)]
    fn path(&self) -> std::path::PathBuf {
        directory().join(format!("{}.sock", self.name))
    }

    pub(super) fn address(&self) -> io::Result<Name<'static>> {
        #[cfg(unix)]
        {
            self.path()
                .to_fs_name::<interprocess::local_socket::GenericFilePath>()
        }
        #[cfg(windows)]
        {
            format!("rust-use-{}", self.name)
                .to_ns_name::<interprocess::local_socket::GenericNamespaced>()
        }
    }

    pub(super) fn bind(&self) -> io::Result<Listener> {
        #[cfg(unix)]
        let _lock = private_directory()?;
        let options = ListenerOptions::new().name(self.address()?);
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            let path = self.path();
            if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_socket())
                && matches!(interprocess::local_socket::Stream::connect(self.address()?), Err(e) if e.kind()==io::ErrorKind::ConnectionRefused)
            {
                std::fs::remove_file(path)?;
            }
        }
        options.create_sync()
    }
}
#[cfg(unix)]
fn directory() -> std::path::PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .filter(|p| !p.is_empty())
        .map_or_else(
            || env::temp_dir().join(format!("rust-use-{}", rustix::process::geteuid().as_raw())),
            |p| std::path::PathBuf::from(p).join("rust-use"),
        )
}

#[cfg(unix)]
fn private_directory() -> io::Result<std::fs::File> {
    use std::{
        fs,
        os::unix::fs::{DirBuilderExt, MetadataExt},
    };
    let path = directory();
    match fs::DirBuilder::new().mode(0o700).create(&path) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        result => result?,
    }
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_dir()
        || meta.uid() != rustix::process::geteuid().as_raw()
        || meta.mode() & 0o777 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "expected a private, owner-only socket directory",
        ));
    }
    // Serialize stale-socket recovery and bind across competing app processes.
    let dir = fs::File::open(path)?;
    rustix::fs::flock(&dir, rustix::fs::FlockOperation::LockExclusive)?;
    Ok(dir)
}

/// List live endpoints in name order on Unix. Windows returns an empty list.
pub fn list_endpoints() -> Vec<Endpoint> {
    #[cfg(unix)]
    {
        let mut endpoints: Vec<_> = directory()
            .read_dir()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                use std::os::unix::fs::FileTypeExt;
                if !entry.file_type().ok()?.is_socket() {
                    return None;
                }
                let name = entry.file_name();
                let endpoint = Endpoint::new(name.to_str()?.strip_suffix(".sock")?).ok()?;
                super::Client::connect(&endpoint).ok()?;
                Some(endpoint)
            })
            .collect();
        endpoints.sort_by(|a, b| a.name.cmp(&b.name));
        endpoints
    }
    #[cfg(windows)]
    {
        Vec::new()
    }
}
