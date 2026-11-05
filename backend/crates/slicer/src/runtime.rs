//! Identity of the launcher and executable, not shared libraries or deployment attestation.
use crate::Error;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
};

const PATHS: [&str; 2] = [
    "/opt/bambustudio-bin/AppRun",
    "/opt/bambustudio-bin/bin/bambu-studio",
];
const MAX_BYTES: u64 = 512 * 1024 * 1024;
static IDENTITY: LazyLock<Result<Identity, ()>> =
    LazyLock::new(|| Identity::read(PATHS.map(Path::new)));

type Stamp = (u64, u64, u64, i64, i64, i64, i64);
fn stamp(metadata: fs::Metadata) -> Result<Stamp, ()> {
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err(());
    }
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}
fn stamps(paths: [&Path; 2]) -> Result<[Stamp; 2], ()> {
    Ok([
        stamp(fs::symlink_metadata(paths[0]).map_err(|_| ())?)?,
        stamp(fs::symlink_metadata(paths[1]).map_err(|_| ())?)?,
    ])
}
struct Identity {
    digest: String,
    stamps: [Stamp; 2],
    invalid: AtomicBool,
}
impl Identity {
    fn read(paths: [&Path; 2]) -> Result<Self, ()> {
        let before = stamps(paths)?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        for (path, expected) in paths.into_iter().zip(before) {
            let mut file = File::open(path).map_err(|_| ())?;
            if stamp(file.metadata().map_err(|_| ())?)? != expected {
                return Err(());
            }
            // Length prefixes separate the two files unambiguously.
            hash.update(expected.2.to_le_bytes());
            let mut total = 0;
            loop {
                let n = file.read(&mut buffer).map_err(|_| ())?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > expected.2 {
                    return Err(());
                }
                hash.update(&buffer[..n]);
            }
            if total != expected.2 || stamp(file.metadata().map_err(|_| ())?)? != expected {
                return Err(());
            }
        }
        if stamps(paths)? != before {
            return Err(());
        }
        Ok(Self {
            digest: format!("{:x}", hash.finalize()),
            stamps: before,
            invalid: AtomicBool::new(false),
        })
    }
    fn verify(&self, paths: [&Path; 2]) -> Result<&str, ()> {
        if self.invalid.load(Ordering::Relaxed) || stamps(paths) != Ok(self.stamps) {
            self.invalid.store(true, Ordering::Relaxed);
            return Err(());
        }
        Ok(&self.digest)
    }
}
/// Hash once per process; ordinary runtime changes invalidate this process until restart.
/// This metadata guard does not close adversarial hot-swap races.
pub fn runtime_sha256() -> Result<&'static str, Error> {
    IDENTITY
        .as_ref()
        .map_err(|_| ())
        .and_then(|id| id.verify(PATHS.map(Path::new)))
        .map_err(|_| {
            Error::Failed("Slicer runtime unavailable or changed; restart required".into())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Runtime(std::path::PathBuf);
    impl Runtime {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("runtime-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&dir).unwrap();
            fs::write(dir.join("AppRun"), b"launcher").unwrap();
            fs::write(dir.join("bambu-studio"), b"executable").unwrap();
            Self(dir)
        }
        fn paths(&self) -> [std::path::PathBuf; 2] {
            [self.0.join("AppRun"), self.0.join("bambu-studio")]
        }
    }
    impl Drop for Runtime {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn stable_content_identity_and_changes() {
        let a = Runtime::new();
        let b = Runtime::new();
        let paths = a.paths();
        let id = Identity::read(paths.each_ref().map(|p| p.as_path())).unwrap();
        assert_eq!(
            id.verify(paths.each_ref().map(|p| p.as_path())),
            Ok(id.digest.as_str())
        );
        assert_eq!(
            id.digest,
            Identity::read(b.paths().each_ref().map(|p| p.as_path()))
                .unwrap()
                .digest
        );
        fs::write(&paths[1], b"different!").unwrap();
        assert_ne!(
            id.digest,
            Identity::read(paths.each_ref().map(|p| p.as_path()))
                .unwrap()
                .digest
        );
        assert!(id.verify(paths.each_ref().map(|p| p.as_path())).is_err());
    }
    #[test]
    fn rejects_missing_nonregular_and_oversized_files() {
        let runtime = Runtime::new();
        let paths = runtime.paths();
        fs::remove_file(&paths[0]).unwrap();
        assert!(Identity::read(paths.each_ref().map(|p| p.as_path())).is_err());
        fs::create_dir(&paths[0]).unwrap();
        assert!(Identity::read(paths.each_ref().map(|p| p.as_path())).is_err());
        fs::remove_dir(&paths[0]).unwrap();
        std::os::unix::fs::symlink(&paths[1], &paths[0]).unwrap();
        assert!(Identity::read(paths.each_ref().map(|p| p.as_path())).is_err());
        fs::remove_file(&paths[0]).unwrap();
        File::create(&paths[0])
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        assert!(Identity::read(paths.each_ref().map(|p| p.as_path())).is_err());
    }
    #[test]
    fn atomic_same_content_replacement_invalidates_until_restart() {
        let runtime = Runtime::new();
        let paths = runtime.paths();
        let id = Identity::read(paths.each_ref().map(|p| p.as_path())).unwrap();
        let old = runtime.0.join("old");
        fs::rename(&paths[0], &old).unwrap();
        fs::write(&paths[0], b"launcher").unwrap();
        assert!(id.verify(paths.each_ref().map(|p| p.as_path())).is_err());
        fs::rename(&old, &paths[0]).unwrap();
        assert!(id.verify(paths.each_ref().map(|p| p.as_path())).is_err());
        assert_eq!(
            id.digest,
            Identity::read(paths.each_ref().map(|p| p.as_path()))
                .unwrap()
                .digest
        );
    }
}
