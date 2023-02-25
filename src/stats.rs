use std::cmp::max;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::file_info::FileInfo;

#[derive(Default, Clone)]
pub struct Stats {
    pub bytes_read: u64,
    pub files_checked: u64,
    pub files_read: u64,
    pub files_new: u64,
    pub files_modified: u64,
    pub files_not_found: u64,
    pub files_duplicate_removed: u64,
    pub files_unchanged: u64,
    pub files_unchanged_size: u64,
}

#[derive(Clone)]
pub struct StatsCollector {
    stats: Arc<Mutex<Stats>>,
}

impl StatsCollector {
    pub fn new() -> Self {
        StatsCollector {
            stats: Arc::new(Mutex::new(Stats::default())),
        }
    }
    pub fn files_checked(&self, amount: u64) {
        let mut s = self.stats.lock().unwrap();
        s.files_checked += amount;
    }
    pub fn files_not_found(&self, amount: u64) {
        let mut s = self.stats.lock().unwrap();
        s.files_not_found += amount;
    }
    pub fn file_not_found(&self) {
        self.files_not_found(1)
    }
    pub fn duplicates_removed(&self, amount: u64) {
        let mut s = self.stats.lock().unwrap();
        s.files_duplicate_removed += amount;
    }
    pub fn file_unchanged(&self, file_info: &FileInfo) {
        let mut s = self.stats.lock().unwrap();
        s.files_unchanged += 1;
        s.files_unchanged_size += file_info.len;
    }
    pub fn file_read_unmodifed(&self, file_info: &FileInfo) {
        println!("  {:}", file_info.rel_path.to_string_lossy());
        let mut s = self.stats.lock().unwrap();
        s.files_read += 1;
        s.bytes_read += file_info.len;
        s.files_unchanged += 1;
        s.files_unchanged_size += file_info.len;
    }
    pub fn file_read_modified(&self, file_info: &FileInfo) {
        println!("M {:}", file_info.rel_path.to_string_lossy());
        let mut s = self.stats.lock().unwrap();
        s.files_read += 1;
        s.bytes_read += file_info.len;
        s.files_modified += 1;
    }
    pub fn file_read_new(&self, file_info: &FileInfo) {
        println!("+ {:}", file_info.rel_path.to_string_lossy());
        let mut s = self.stats.lock().unwrap();
        s.files_read += 1;
        s.bytes_read += file_info.len;
        s.files_new += 1;
    }
    pub fn get_results(&self) -> Stats {
        let s = self.stats.lock().unwrap();
        s.clone()
    }
    pub fn print_results_for_update(&self, duration: Duration, newly_missing: u64) {
        let r = self.get_results();
        let prec_1 = (max(r.files_checked, r.files_unchanged) as f64)
            .log10()
            .ceil() as usize;
        println!(
            "{:prec$} files checked in {:.1?}:",
            r.files_checked,
            duration,
            prec = prec_1
        );

        let prec_2 = (max(r.files_read, r.files_not_found) as f64).log10().ceil() as usize;
        println!(
            "    {:prec$} files read ({:.1} GiB, {:.0} MiB/s):",
            r.files_read,
            (r.bytes_read as f64) / 1024.0 / 1024.0 / 1024.0,
            (r.bytes_read as f64) / 1024.0 / 1024.0 / duration.as_secs_f64(),
            prec = prec_2
        );

        let prec_3 = (max(
            max(r.files_new, r.files_modified),
            max(r.files_duplicate_removed, newly_missing),
        ) as f64)
            .log10()
            .ceil() as usize;
        println!("        {:prec$} new files", r.files_new, prec = prec_3);
        println!(
            "        {:prec$} files modified",
            r.files_modified,
            prec = prec_3
        );
        println!(
            "    {:prec$} files not found:",
            r.files_not_found,
            prec = prec_2
        );
        println!(
            "        {:prec$} files found elsewhere (moved or duplicates removed)",
            r.files_duplicate_removed,
            prec = prec_3
        );
        println!(
            "        {:prec$} files newly missing",
            newly_missing,
            prec = prec_3
        );
        println!(
            "{:prec$} files unchanged ({:.1} GiB)",
            r.files_unchanged,
            r.files_unchanged_size as f64 / 1024.0 / 1024.0 / 1024.0,
            prec = prec_1
        );
    }

    pub(crate) fn print_results_for_verify(&self, duration: Duration) {
        let r = self.get_results();
        let prec_1 = (max(r.files_checked, r.files_unchanged) as f64)
            .log10()
            .ceil() as usize;
        println!(
            "{:prec$} files checked in {:.1?}:",
            r.files_checked,
            duration,
            prec = prec_1
        );

        let prec_2 = (max(r.files_read, r.files_not_found) as f64).log10().ceil() as usize;
        println!(
            "    {:prec$} files read ({:.1} GiB, {:.0} MiB/s):",
            r.files_read,
            (r.bytes_read as f64) / 1024.0 / 1024.0 / 1024.0,
            (r.bytes_read as f64) / 1024.0 / 1024.0 / duration.as_secs_f64(),
            prec = prec_2
        );
    }
}
