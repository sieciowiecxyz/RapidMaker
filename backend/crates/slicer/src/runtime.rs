//! Identity of the fixed bundled tree, not system libraries or deployment attestation.
use crate::Error;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
};

const ROOT: &str = "/opt/bambustudio-bin";
struct Limits {
    entries: usize,
    file_bytes: u64,
    total_bytes: u64,
    depth: usize,
    path_bytes: usize,
}
const LIMITS: Limits = Limits {
    entries: 16_384,
    file_bytes: 512 * 1024 * 1024,
    total_bytes: 1024 * 1024 * 1024,
    depth: 16,
    path_bytes: 4096,
};
static IDENTITY: LazyLock<Result<Identity, ()>> = LazyLock::new(|| Identity::read(Path::new(ROOT)));
static CHECKS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

type Stamp = (u64, u64, u64, i64, i64, i64, i64);
fn stamp(metadata: &fs::Metadata) -> Stamp {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    path: PathBuf,
    directory: bool,
    mode: u32,
    stamp: Stamp,
}
fn inventory(root: &Path, limits: &Limits) -> Result<Vec<Entry>, ()> {
    fn visit(
        root: &Path,
        path: PathBuf,
        depth: usize,
        limits: &Limits,
        entries: &mut Vec<Entry>,
        total: &mut u64,
    ) -> Result<(), ()> {
        if entries.len() >= limits.entries
            || depth > limits.depth
            || path.as_os_str().as_bytes().len() > limits.path_bytes
        {
            return Err(());
        }
        let full = root.join(&path);
        let metadata = fs::symlink_metadata(&full).map_err(|_| ())?;
        let directory = metadata.is_dir();
        if !directory {
            if !metadata.is_file() || metadata.len() > limits.file_bytes {
                return Err(());
            }
            *total = total.checked_add(metadata.len()).ok_or(())?;
            if *total > limits.total_bytes {
                return Err(());
            }
        }
        entries.push(Entry {
            path: path.clone(),
            directory,
            mode: metadata.mode(),
            stamp: stamp(&metadata),
        });
        if directory {
            // Iterate directly: never collect an unbounded directory before applying limits.
            for child in fs::read_dir(full).map_err(|_| ())? {
                let child = child.map_err(|_| ())?;
                visit(
                    root,
                    path.join(child.file_name()),
                    depth + 1,
                    limits,
                    entries,
                    total,
                )?;
            }
        }
        Ok(())
    }
    let mut entries = Vec::new();
    visit(root, PathBuf::new(), 0, limits, &mut entries, &mut 0)?;
    if !entries[0].directory
        || ["AppRun", "bin/bambu-studio"].iter().any(|required| {
            !entries.iter().any(|entry| {
                !entry.directory && entry.mode & 0o111 != 0 && entry.path == Path::new(required)
            })
        })
    {
        return Err(());
    }
    entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}
struct Identity {
    digest: String,
    entries: Vec<Entry>,
    invalid: AtomicBool,
}
impl Identity {
    fn read(root: &Path) -> Result<Self, ()> {
        let before = inventory(root, &LIMITS)?;
        let mut hash = Sha256::new();
        hash.update(b"rapidmaker-bundled-runtime-v1\0");
        let mut buffer = [0; 64 * 1024];
        for entry in &before {
            let path = entry.path.as_os_str().as_bytes();
            hash.update([u8::from(entry.directory)]);
            hash.update(entry.mode.to_le_bytes());
            hash.update((path.len() as u64).to_le_bytes());
            hash.update(path);
            if entry.directory {
                continue;
            }
            let mut file = File::open(root.join(&entry.path)).map_err(|_| ())?;
            let metadata = file.metadata().map_err(|_| ())?;
            if !metadata.is_file() || stamp(&metadata) != entry.stamp {
                return Err(());
            }
            hash.update(entry.stamp.2.to_le_bytes());
            let mut total = 0;
            loop {
                let n = file.read(&mut buffer).map_err(|_| ())?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > entry.stamp.2 {
                    return Err(());
                }
                hash.update(&buffer[..n]);
            }
            if total != entry.stamp.2 || stamp(&file.metadata().map_err(|_| ())?) != entry.stamp {
                return Err(());
            }
        }
        if inventory(root, &LIMITS)? != before {
            return Err(());
        }
        Ok(Self {
            digest: format!("{:x}", hash.finalize()),
            entries: before,
            invalid: AtomicBool::new(false),
        })
    }
    fn verify(&self, root: &Path) -> Result<&str, ()> {
        if self.invalid.load(Ordering::Relaxed)
            || inventory(root, &LIMITS).as_ref() != Ok(&self.entries)
        {
            self.invalid.store(true, Ordering::Relaxed);
            return Err(());
        }
        Ok(&self.digest)
    }
}
/// Hash once per process; ordinary runtime changes invalidate this process until restart.
/// Every use scans bounded tree metadata. This guard does not close adversarial hot-swap races.
pub async fn runtime_sha256() -> Result<&'static str, Error> {
    let permit = CHECKS.acquire().await.map_err(|_| Error::Busy)?;
    tokio::task::spawn_blocking(move || {
        // Cancelling the caller does not stop this task; retain its slot until completion.
        let _permit = permit;
        IDENTITY
            .as_ref()
            .map_err(|_| ())
            .and_then(|id| id.verify(Path::new(ROOT)))
            .map_err(|_| {
                Error::Failed("Slicer runtime unavailable or changed; restart required".into())
            })
    })
    .await
    .map_err(|_| Error::Failed("Slicer runtime check failed".into()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixListener,
    };

    struct Runtime(PathBuf);
    impl Runtime {
        fn new(reverse: bool) -> Self {
            let dir = std::env::temp_dir().join(format!("runtime-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&dir).unwrap();
            let mut files = [
                ("AppRun", "launcher"),
                ("bin/bambu-studio", "executable"),
                ("resources/profiles/a.json", "profile"),
                ("resources/shaders/b.glsl", "shader"),
                ("usr/lib/plugin.so", "bundled library"),
            ];
            if reverse {
                files.reverse();
            }
            for (path, content) in files {
                let path = dir.join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, content).unwrap();
                if matches!(
                    path.file_name().and_then(|p| p.to_str()),
                    Some("AppRun" | "bambu-studio")
                ) {
                    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
                }
            }
            Self(dir)
        }
        fn read(&self) -> Identity {
            Identity::read(&self.0).unwrap()
        }
    }
    impl Drop for Runtime {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn stable_identity_ignores_root_and_creation_order_but_covers_resources() {
        let a = Runtime::new(false);
        let b = Runtime::new(true);
        let id = a.read();
        assert_eq!(id.verify(&a.0), Ok(id.digest.as_str()));
        assert_eq!(id.digest, b.read().digest);
        fs::write(a.0.join("resources/profiles/a.json"), b"changed").unwrap();
        assert!(id.verify(&a.0).is_err());
        assert_ne!(id.digest, a.read().digest);
        assert_eq!(fs::read(a.0.join("AppRun")).unwrap(), b"launcher");
        assert_eq!(
            fs::read(a.0.join("bin/bambu-studio")).unwrap(),
            b"executable"
        );
    }
    #[test]
    fn inventory_changes_and_restoration_invalidate_until_restart() {
        for change in ["add", "remove", "rename", "empty_directory", "replace"] {
            let runtime = Runtime::new(false);
            let id = runtime.read();
            let resource = runtime.0.join("resources/profiles/a.json");
            let other = runtime.0.join("resources/profiles/other");
            match change {
                "add" => fs::write(&other, b"extra").unwrap(),
                "remove" => fs::remove_file(&resource).unwrap(),
                "rename" => fs::rename(&resource, &other).unwrap(),
                "empty_directory" => fs::create_dir(&other).unwrap(),
                "replace" => {
                    fs::rename(&resource, &other).unwrap();
                    fs::write(&resource, b"profile").unwrap();
                    fs::remove_file(&other).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(id.verify(&runtime.0).is_err(), "{change}");
            match change {
                "add" => fs::remove_file(other).unwrap(),
                "remove" => fs::write(resource, b"profile").unwrap(),
                "rename" => fs::rename(other, resource).unwrap(),
                "empty_directory" => fs::remove_dir(other).unwrap(),
                _ => (),
            }
            assert!(id.verify(&runtime.0).is_err(), "restored {change}");
            assert_eq!(id.digest, runtime.read().digest);
        }
    }
    #[test]
    fn rejects_missing_required_files_symlinks_and_special_entries() {
        for required in ["AppRun", "bin/bambu-studio"] {
            let runtime = Runtime::new(false);
            let path = runtime.0.join(required);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(Identity::read(&runtime.0).is_err());
            fs::remove_file(&path).unwrap();
            assert!(Identity::read(&runtime.0).is_err());
            fs::create_dir(&path).unwrap();
            assert!(Identity::read(&runtime.0).is_err());
        }
        let runtime = Runtime::new(false);
        let extra = runtime.0.join("extra");
        // Reject links both inside and outside the bundle, including directory links/root.
        for target in [
            runtime.0.join("AppRun"),
            runtime.0.clone(),
            PathBuf::from("/usr"),
        ] {
            symlink(target, &extra).unwrap();
            assert!(Identity::read(&runtime.0).is_err());
            fs::remove_file(&extra).unwrap();
        }
        symlink(&runtime.0, &extra).unwrap();
        assert!(Identity::read(&extra).is_err());
        fs::remove_file(&extra).unwrap();
        let _socket = UnixListener::bind(&extra).unwrap();
        assert!(Identity::read(&runtime.0).is_err());
    }
    #[test]
    fn permission_changes_change_digest_even_after_restart() {
        let runtime = Runtime::new(false);
        let path = runtime.0.join("AppRun");
        let original = fs::metadata(&path).unwrap().permissions().mode();
        let id = runtime.read();
        fs::set_permissions(&path, fs::Permissions::from_mode(original ^ 0o100)).unwrap();
        assert!(id.verify(&runtime.0).is_err());
        assert_ne!(id.digest, runtime.read().digest);
        fs::set_permissions(path, fs::Permissions::from_mode(original)).unwrap();
        assert!(id.verify(&runtime.0).is_err());
        assert_eq!(id.digest, runtime.read().digest);
    }
    #[tokio::test]
    #[ignore = "requires installed Bambu bundle; reads all runtime bytes"]
    async fn installed_bundle_identity_is_stable() {
        let started = std::time::Instant::now();
        let identity = runtime_sha256().await.unwrap().to_owned();
        let cold = started.elapsed();
        let started = std::time::Instant::now();
        for _ in 0..10 {
            assert_eq!(runtime_sha256().await.unwrap(), identity);
        }
        eprintln!(
            "runtime identity: cold={cold:?} warm10={:?}",
            started.elapsed()
        );
    }
    #[test]
    fn bounds_are_enforced_without_large_allocations() {
        let runtime = Runtime::new(false);
        let entries = inventory(&runtime.0, &LIMITS).unwrap();
        let total: u64 = entries
            .iter()
            .filter(|e| !e.directory)
            .map(|e| e.stamp.2)
            .sum();
        for limits in [
            Limits {
                entries: entries.len() - 1,
                ..LIMITS
            },
            Limits {
                file_bytes: 3,
                ..LIMITS
            },
            Limits {
                total_bytes: total - 1,
                ..LIMITS
            },
            Limits { depth: 2, ..LIMITS },
            Limits {
                path_bytes: 10,
                ..LIMITS
            },
        ] {
            assert!(inventory(&runtime.0, &limits).is_err());
        }
        File::create(runtime.0.join("too-large"))
            .unwrap()
            .set_len(LIMITS.file_bytes + 1)
            .unwrap();
        assert!(Identity::read(&runtime.0).is_err());
    }
}
