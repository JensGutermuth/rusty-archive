use crate::file_check::FileCheckResult;
use crate::file_info::FileInfo;

use anyhow::{Context, Result};
use chrono::Local;
use std::collections::HashMap;
use std::fs::{remove_file, File};
use std::io::{self, BufRead, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn read_state(state_dir: &Path) -> Result<HashMap<PathBuf, FileInfo>> {
    let state_path = WalkDir::new(state_dir)
        .max_depth(1)
        .sort_by_file_name()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("Unable to list files in state directory {:?}", state_dir))?
        .into_iter()
        .filter(|f| f.file_name().to_string_lossy().ends_with(".state"))
        .last();
    if let Some(state_path) = state_path {
        BufReader::new(File::open(state_path.path())?)
            .lines()
            .map(|l| FileInfo::parse(l.unwrap().as_str()))
            .map(|f| match f {
                Ok(f) => Ok((f.rel_path.clone(), f)),
                Err(err) => {
                    Err(err).context(format!("Failed to read state from {:?}", state_path.path()))
                }
            })
            .collect::<Result<HashMap<_, _>, _>>()
    } else {
        println!("no previous state found in {:?}", state_dir);
        Ok(HashMap::new())
    }
}

pub fn write_state<'a>(
    state_dir: &Path,
    checked_files: impl Iterator<Item = &'a FileCheckResult>,
) -> Result<(), io::Error> {
    let now = Local::now(); // e.g. `2014-11-28T21:45:59.324310806+09:00`
    let mut state_f = BufWriter::with_capacity(
        1024 * 1024,
        File::options()
            .write(true)
            .create_new(true)
            .open(state_dir.join(format!("{}.state", now.format("%Y%m%d %H%M%S"))))?,
    );
    let mut modified_f = BufWriter::with_capacity(
        1024 * 1024,
        File::options()
            .write(true)
            .create_new(true)
            .open(state_dir.join(format!("{}.modified", now.format("%Y%m%d %H%M%S"))))?,
    );
    let mut missing_f = BufWriter::with_capacity(
        1024 * 1024,
        File::options()
            .write(true)
            .create_new(true)
            .open(state_dir.join(format!("{}.missing", now.format("%Y%m%d %H%M%S"))))?,
    );

    let mut modified_files = 0;
    let mut missing_files = 0;
    for file in checked_files {
        match file {
            FileCheckResult::New(fi) | FileCheckResult::Unmodifed(fi) => {
                fi.write(&mut state_f)?;
            }
            FileCheckResult::Modified(fi) => {
                modified_files += 1;
                fi.previous.write(&mut modified_f)?;
                fi.current.write(&mut state_f)?;
            }
            FileCheckResult::Missing(fi) => {
                missing_files += 1;
                fi.write(&mut missing_f)?;
            }
        }
    }
    drop(state_f);
    drop(modified_f);
    drop(missing_f);
    if modified_files == 0 {
        remove_file(state_dir.join(format!("{}.modified", now.format("%Y%m%d %H%M%S"))))?;
    }
    if missing_files == 0 {
        remove_file(state_dir.join(format!("{}.missing", now.format("%Y%m%d %H%M%S"))))?;
    }

    Ok(())
}
