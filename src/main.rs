use crate::cli::commandline_options;
use crate::file_check::{FileCheckResult, FileToCheck};
use crate::state::{read_state, write_state};

mod cli;
mod file_check;
mod file_info;
mod state;
mod stats;

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Instant, SystemTime};
use walkdir::WalkDir;

#[macro_use]
extern crate lazy_static;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = commandline_options().run();

    let (state_dir, directory, read_all_files) = match &opts.cmd {
        cli::Cmd::Update {
            state_dir,
            directory,
            read_all_files,
        } => (state_dir.as_str(), directory.as_deref(), *read_all_files),
        cli::Cmd::Verify {
            ignore_missing: _,
            state_dir,
            directory,
            only_presence: _,
        } => (state_dir.as_str(), directory.as_deref(), true),
    };

    let num_threads = opts.threads.unwrap_or(1);
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()?;
    println!("using {num_threads} thread(s)");

    let start_load_old_state = Instant::now();
    let mut old_states_by_filename = read_state(Path::new(state_dir))?;
    println!(
        "loaded previous states of {} files in {:.1?} from {}",
        old_states_by_filename.len(),
        start_load_old_state.elapsed(),
        state_dir
    );

    let start = Instant::now();
    let stats = stats::StatsCollector::new();
    let base_path_buf = PathBuf::from(directory.unwrap_or("."));
    let base_path = base_path_buf.as_path();
    let mut files_checked = 0;

    let (check_files_sender, check_files_recv) = mpsc::channel();
    let mut checked_files = Vec::new();

    rayon::in_place_scope_fifo(|s| -> Result<()> {
        let files = WalkDir::new(base_path)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| {
                let file_name = e.file_name().to_str().unwrap();
                if e.file_type().is_dir() {
                    opts.exclude_directory
                        .iter()
                        .all(|re| !re.is_match(file_name))
                } else {
                    opts.exclude_file.iter().all(|re| !re.is_match(file_name))
                }
            })
            .filter(|e| match e {
                Ok(e) => e.file_type().is_file(),
                _ => true,
            });

        for file_result in files {
            let file = file_result.context("Listing files failed")?;
            let path_str = file.path().as_os_str().to_str().unwrap();
            if opts.exclude_path.iter().any(|re| re.is_match(path_str)) {
                continue;
            }

            files_checked += 1;

            let handle = |file: FileToCheck| {
                let sender = check_files_sender.clone();
                let stats = stats.clone();
                s.spawn_fifo(move |_| {
                    let result = file.check(base_path);
                    if let Ok(check_result) = &result {
                        match check_result {
                            FileCheckResult::New(file_info) => {
                                stats.file_read_new(file_info);
                            }
                            FileCheckResult::Unmodifed(file_info) => {
                                stats.file_read_unmodifed(file_info);
                            }
                            FileCheckResult::Modified(file_infos) => {
                                stats.file_read_modified(&file_infos.current);
                            }
                            FileCheckResult::Missing(_) => {
                                stats.file_not_found();
                            }
                        }
                    }
                    sender.send(result).unwrap();
                });
            };

            match old_states_by_filename.remove(file.path().strip_prefix(base_path).unwrap()) {
                None => {
                    handle(FileToCheck::New(file));
                }
                Some(fi) => match fi.needs_reading(&file) {
                    Ok(needs_reading) if (needs_reading || read_all_files) => {
                        handle(FileToCheck::NeedsChecking(fi));
                    }
                    Ok(_) => {
                        stats.file_unchanged(&fi);

                        let mut new_fi = fi;
                        new_fi.last_seen = SystemTime::now();
                        checked_files.push(FileCheckResult::Unmodifed(new_fi));
                    }
                    Err(err) => check_files_sender
                        .send(Err(err).context(format!(
                            "Failed to check if file needs to be read: {:?}",
                            file.path()
                        )))
                        .unwrap(),
                },
            }
        }
        Ok(())
    })?;
    drop(check_files_sender);
    stats.files_checked(files_checked + old_states_by_filename.len() as u64);
    stats.files_not_found(old_states_by_filename.len() as u64);

    checked_files.extend(
        old_states_by_filename
            .into_values()
            .into_iter()
            .map(FileCheckResult::Missing),
    );
    checked_files.extend(
        check_files_recv
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?,
    );

    checked_files.sort_by_cached_key(|f| match f {
        FileCheckResult::New(fi)
        | FileCheckResult::Unmodifed(fi)
        | FileCheckResult::Missing(fi) => fi.rel_path.clone(),
        FileCheckResult::Modified(fi_mod) => fi_mod.current.rel_path.clone(),
    });

    match opts.cmd {
        cli::Cmd::Update {
            read_all_files: _,
            state_dir,
            directory: _,
        } => {
            let present_sha256_digests = checked_files
                .iter()
                .filter_map(|f| match &f {
                    FileCheckResult::New(fi) | FileCheckResult::Unmodifed(fi) => {
                        Some(fi.sha256_digest)
                    }
                    FileCheckResult::Modified(fi_mod) => Some(fi_mod.current.sha256_digest),
                    _ => None,
                })
                .collect::<HashSet<_>>();

            let mut duplicates_removed: u64 = 0;
            let checked_files_deduped = checked_files
                .into_iter()
                .filter_map(|f| match f {
                    FileCheckResult::Missing(fi) => {
                        if present_sha256_digests.contains(&fi.sha256_digest) {
                            duplicates_removed += 1;
                            None
                        } else {
                            Some(FileCheckResult::Missing(fi))
                        }
                    }
                    FileCheckResult::Modified(mod_fi) => {
                        if present_sha256_digests.contains(&mod_fi.previous.sha256_digest) {
                            duplicates_removed += 1;
                            // Previous version was a duplicate, consider this to be new
                            Some(FileCheckResult::New(mod_fi.current))
                        } else {
                            Some(FileCheckResult::Modified(mod_fi))
                        }
                    }
                    other => Some(other),
                })
                .collect::<Vec<_>>();
            stats.duplicates_removed(duplicates_removed);

            write_state(Path::new(state_dir.as_str()), checked_files_deduped.iter())?;

            let newly_missing = checked_files_deduped
                .iter()
                .filter(|f| {
                    matches!(
                        f,
                        FileCheckResult::Missing(_) | FileCheckResult::Modified(_)
                    )
                })
                .count() as u64;

            stats.print_results_for_update(start.elapsed(), newly_missing);
        }
        cli::Cmd::Verify {
            ignore_missing,
            only_presence,
            state_dir: _,
            directory: _,
        } => {
            stats.print_results_for_verify(start.elapsed());

            let archive_sha256_digests = checked_files
                .iter()
                .filter_map(|f| match &f {
                    FileCheckResult::Unmodifed(fi) | FileCheckResult::Missing(fi) => {
                        Some(fi.sha256_digest)
                    }
                    FileCheckResult::Modified(fi_mod) => Some(fi_mod.previous.sha256_digest),
                    FileCheckResult::New(_) => None,
                })
                .collect::<HashSet<_>>();

            match (ignore_missing, only_presence) {
                (true, true) => {
                    // ensure all files found are present in the archive
                    let not_present = checked_files
                        .iter()
                        .filter(|f| match f {
                            FileCheckResult::New(fi) => {
                                !archive_sha256_digests.contains(&fi.sha256_digest)
                            }
                            FileCheckResult::Modified(fi_mod) => {
                                !archive_sha256_digests.contains(&fi_mod.current.sha256_digest)
                            }
                            FileCheckResult::Unmodifed(_) | FileCheckResult::Missing(_) => false,
                        })
                        .count();
                    if not_present > 0 {
                        return Err(anyhow::Error::msg(format!(
                            "{} files not found in archive",
                            not_present
                        ))
                        .into());
                    }
                }
                (true, false) => {
                    // ensure the files found match the ones in the archive at that path
                    let missing_or_modified = checked_files
                        .iter()
                        .filter(|f| matches!(f, FileCheckResult::Modified(_)))
                        .count();
                    let new = checked_files
                        .iter()
                        .filter(|f| matches!(f, FileCheckResult::New(_)))
                        .count();
                    if missing_or_modified > 0 {
                        return Err(anyhow::Error::msg(format!(
                            "{} files changed, {} files not found in archive",
                            missing_or_modified, new
                        ))
                        .into());
                    }
                }
                (false, true) => {
                    // ensure all files in the archive are found somewhere
                    let not_present = checked_files
                        .iter()
                        .filter(|f| match f {
                            FileCheckResult::New(fi) => {
                                !archive_sha256_digests.contains(&fi.sha256_digest)
                            }
                            FileCheckResult::Modified(fi_mod) => {
                                !archive_sha256_digests.contains(&fi_mod.current.sha256_digest)
                            }
                            FileCheckResult::Unmodifed(_) | FileCheckResult::Missing(_) => false,
                        })
                        .count();
                    let mut missing_sha256 = archive_sha256_digests;
                    for file in &checked_files {
                        match file {
                            FileCheckResult::New(fi) | FileCheckResult::Unmodifed(fi) => {
                                missing_sha256.remove(&fi.sha256_digest);
                            }
                            FileCheckResult::Modified(fi_mod) => {
                                missing_sha256.remove(&fi_mod.current.sha256_digest);
                            }
                            _ => {}
                        }
                    }
                    if not_present > 0 || !missing_sha256.is_empty() {
                        return Err(anyhow::Error::msg(format!(
                            "{} files not found in archive, {} files in archive not found",
                            not_present,
                            missing_sha256.len()
                        ))
                        .into());
                    }
                }
                (false, false) => {
                    // ensure all files in the archive are found at their path
                    let missing_or_modified = checked_files
                        .iter()
                        .filter(|f| {
                            matches!(
                                f,
                                FileCheckResult::Missing(_) | FileCheckResult::Modified(_)
                            )
                        })
                        .count();
                    let new = checked_files
                        .iter()
                        .filter(|f| matches!(f, FileCheckResult::New(_)))
                        .count();
                    if missing_or_modified > 0 || new > 0 {
                        return Err(anyhow::Error::msg(format!(
                            "{} files missing or changed, {} files not found in archive",
                            missing_or_modified, new
                        ))
                        .into());
                    }
                }
            }
        }
    }

    Ok(())
}
