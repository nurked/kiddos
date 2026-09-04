//! `SAVE "game"` writes `game.bas` into a folder on the virtual drive.

use async_trait::async_trait;
use endbasic_std::storage::{DiskSpace, Drive, DriveFactory, DriveFiles, Metadata};
use kiddos_kernel::Proc;
use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

pub struct KidDrive {
    proc: Arc<Proc>,
    dir: String,
}

fn ioerr(e: kiddos_kernel::VfsError) -> io::Error {
    let kind = match e {
        kiddos_kernel::VfsError::NotFound(_) => io::ErrorKind::NotFound,
        kiddos_kernel::VfsError::Permission(_) | kiddos_kernel::VfsError::NotPermitted(_) => {
            io::ErrorKind::PermissionDenied
        }
        kiddos_kernel::VfsError::Exists(_) => io::ErrorKind::AlreadyExists,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, e.to_string())
}

fn check_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name"));
    }
    Ok(())
}

#[async_trait(?Send)]
impl Drive for KidDrive {
    async fn delete(&mut self, name: &str) -> io::Result<()> {
        check_name(name)?;
        self.proc.fs().unlink(&format!("{}/{name}", self.dir)).map_err(ioerr)
    }

    async fn enumerate(&self) -> io::Result<DriveFiles> {
        let entries = self.proc.fs().readdir(&self.dir).map_err(ioerr)?;
        let mut dirents = BTreeMap::new();
        for e in entries.into_iter().filter(|e| e.is_file()) {
            let date =
                time::OffsetDateTime::from_unix_timestamp(e.mtime as i64).unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
            dirents.insert(e.name, Metadata { date, length: e.size });
        }
        let free = 64 * 1024 * 1024u64 - self.proc.fs().used_bytes().min(64 * 1024 * 1024);
        Ok(DriveFiles::new(
            dirents,
            Some(DiskSpace::new(64 * 1024 * 1024, 100_000)),
            Some(DiskSpace::new(free, 100_000)),
        ))
    }

    async fn get(&self, name: &str) -> io::Result<Vec<u8>> {
        check_name(name)?;
        self.proc.fs().read(&format!("{}/{name}", self.dir)).map_err(ioerr)
    }

    async fn put(&mut self, name: &str, content: &[u8]) -> io::Result<()> {
        check_name(name)?;
        self.proc
            .fs()
            .write(&format!("{}/{name}", self.dir), content)
            .map_err(ioerr)
    }
}

pub struct KidDriveFactory {
    pub proc: Arc<Proc>,
}

impl DriveFactory for KidDriveFactory {
    fn create(&self, target: &str) -> io::Result<Box<dyn Drive>> {
        let dir = if target.is_empty() {
            self.proc.cwd()
        } else {
            self.proc.fs().path(target)
        };
        if !self.proc.fs().is_dir(&dir) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{dir} is not a folder"),
            ));
        }
        Ok(Box::new(KidDrive {
            proc: self.proc.clone(),
            dir,
        }))
    }
}
