use crate::file_info::FileInfo;
use anyhow::{Context, Result};

use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::SystemTime;
use walkdir::DirEntry;

/// A file that needs to be checked
#[derive(Debug)]
pub enum FileToCheck {
    /// not seen before
    New(DirEntry),

    /// previously seen with different metadata
    NeedsChecking(FileInfo),
}

/// Information about a modified file
pub struct FileCheckResultModified {
    /// FileInfo of the previous state
    pub previous: FileInfo,
    /// Current information
    pub current: FileInfo,
}

/// Result of checking a file
pub enum FileCheckResult {
    /// The file was not seen before
    New(FileInfo),
    /// The file was previously seen with different metadata but the same contents
    Unmodifed(FileInfo),
    /// The file was previously seen with different metadata and different contents
    Modified(FileCheckResultModified),
    /// The file was previously seen but is now missing
    Missing(FileInfo),
}

impl FileToCheck {
    /// Determine the current FileInfo for a file and if it's been modified
    ///
    /// This function will always read the file completely and hash
    /// it's contents.
    pub fn check(self, base_path: &Path) -> Result<FileCheckResult> {
        match self {
            FileToCheck::New(new_file) => Ok(FileCheckResult::New(
                hash_file(base_path, new_file.path())
                    .with_context(|| format!("Failed to read new file {:?}", new_file.path()))?,
            )),
            FileToCheck::NeedsChecking(file_needs_checking) => {
                let full_path = base_path.join(file_needs_checking.rel_path.as_path());
                let file_info = hash_file(base_path, full_path.as_path()).with_context(|| {
                    format!("Failed to read potentially modified file {:?}", full_path)
                })?;
                if file_info.sha256_digest == file_needs_checking.sha256_digest {
                    Ok(FileCheckResult::Unmodifed(file_info))
                } else {
                    Ok(FileCheckResult::Modified(FileCheckResultModified {
                        previous: file_needs_checking,
                        current: file_info,
                    }))
                }
            }
        }
    }
}

/// Reads a file, hashes it's contents and returns the current FileInfo
fn hash_file(base_path: &Path, file: &Path) -> Result<FileInfo, io::Error> {
    thread_local!(static BUF: RefCell<Vec<u8>>  = RefCell::new(vec![0_u8; 4 * 1024 * 1024]));

    BUF.with(|buf| {
        let mut f = File::open(file)?;
        let mut hasher = Sha256::new();
        let mut total_bytes_read = 0;
        loop {
            let bytes_read = f.read(buf.borrow_mut().as_mut_slice())?;
            if bytes_read > 0 {
                total_bytes_read += bytes_read;
                hasher.update(&buf.borrow()[0..bytes_read]);
            } else {
                break;
            }
        }
        let file_digest = hasher.finalize();
        Ok(FileInfo {
            rel_path: file.strip_prefix(base_path).unwrap().to_path_buf(),
            sha256_digest: file_digest.try_into().expect("wrong length"),
            mtime: file.metadata()?.modified()?,
            len: total_bytes_read as u64,
            fully_read: SystemTime::now(),
            last_seen: SystemTime::now(),
        })
    })
}
