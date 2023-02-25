use anyhow::{Context, Result};
use regex::Regex;
use std::io::{self};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use walkdir::DirEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub rel_path: PathBuf,
    pub sha256_digest: [u8; 32],
    pub mtime: SystemTime,
    pub len: u64,
    pub last_seen: SystemTime,
    pub fully_read: SystemTime,
}

impl FileInfo {
    pub fn parse(line: &str) -> Result<FileInfo> {
        lazy_static! {
            static ref RE: Regex =
                Regex::new("([a-f0-9]{64}) /?([^/].*) # mtime (\\d+)\\.(\\d+) size (\\d+) fully_read (\\d+)(?:\\.\\d+)? last_seen (\\d+)(?:\\.\\d+)?")
                    .unwrap();
        }
        match RE.captures(line) {
            Some(m) => {
                let mut sha256_digest = [0_u8; 32];
                if let Err(err) =
                    hex::decode_to_slice(m.get(1).unwrap().as_str(), &mut sha256_digest)
                {
                    return Err(err).context(format!(
                        "invalid line (couldn't parse sha256_digest): '{}'",
                        line
                    ));
                }
                let mtime_s: Result<u64, _> = m.get(3).unwrap().as_str().parse();
                if let Err(err) = mtime_s {
                    return Err(err)
                        .context(format!("invalid line (couldn't parse mtime): '{}'", line));
                }
                let mut mtime_ns: Result<u64, _> = m.get(4).unwrap().as_str().parse();
                if let Err(err) = mtime_ns {
                    return Err(err)
                        .context(format!("invalid line (couldn't parse mtime): '{}'", line));
                } else if let Ok(v) = mtime_ns {
                    mtime_ns = Ok(v * (10_u64.pow(9 - m.get(4).unwrap().as_str().len() as u32)));
                }

                let size = m.get(5).unwrap().as_str().parse();
                if let Err(err) = size {
                    return Err(err)
                        .context(format!("invalid line (couldn't parse size): '{}'", line));
                }
                let fully_read = m.get(6).unwrap().as_str().parse();
                if let Err(err) = fully_read {
                    return Err(err).context(format!(
                        "invalid line (couldn't parse fully_read): '{}'",
                        line
                    ));
                }
                let last_seen = m.get(7).unwrap().as_str().parse();
                if let Err(err) = last_seen {
                    return Err(err).context(format!(
                        "invalid line (couldn't parse last_seen): '{}'",
                        line
                    ));
                }

                Ok(FileInfo {
                    rel_path: PathBuf::from(m.get(2).unwrap().as_str()),
                    sha256_digest,
                    mtime: SystemTime::UNIX_EPOCH
                        + Duration::from_nanos(
                            mtime_s.unwrap() * 1_000_000_000 + mtime_ns.unwrap(),
                        ),
                    fully_read: SystemTime::UNIX_EPOCH + Duration::from_secs(fully_read.unwrap()),
                    len: size.unwrap(),
                    last_seen: SystemTime::UNIX_EPOCH + Duration::from_secs(last_seen.unwrap()),
                })
            }
            _ => Err(io::Error::from(io::ErrorKind::InvalidData))
                .context(format!("invalid line: '{}'", line)),
        }
    }

    pub fn write(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        let mut sha256_hexdigest = [0_u8; 64];
        hex::encode_to_slice(self.sha256_digest, &mut sha256_hexdigest).unwrap();
        writeln!(
            to,
            "{} {} # mtime {}.{:>09} size {} fully_read {} last_seen {}",
            std::str::from_utf8(&sha256_hexdigest).unwrap(),
            self.rel_path.to_str().unwrap(),
            self.mtime.duration_since(UNIX_EPOCH).unwrap().as_secs(),
            self.mtime.duration_since(UNIX_EPOCH).unwrap().as_nanos()
                - self.mtime.duration_since(UNIX_EPOCH).unwrap().as_secs() as u128 * 1_000_000_000,
            self.len,
            self.fully_read
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            self.last_seen.duration_since(UNIX_EPOCH).unwrap().as_secs()
        )
    }

    pub fn needs_reading(&self, dir_entry: &DirEntry) -> Result<bool> {
        let m = dir_entry
            .metadata()
            .with_context(|| format!("Unable to get metadata for '{:?}'", dir_entry.path()))?;
        let mtime = m
            .modified()
            .with_context(|| format!("Unable to read mtime for '{:?}'", dir_entry.path()))?;
        let len = m.len();
        // println!(
        //     "{:?} mtime: {:?} ?= {:?}, len: {} ?= {}",
        //     dir_entry.path(),
        //     self.mtime,
        //     mtime,
        //     self.len,
        //     len
        // );
        Ok(self.mtime != mtime || self.len != len)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    use super::*;

    #[test]
    fn round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let fi = FileInfo {
            rel_path: PathBuf::from("test/äöüß/#!,.\"§$%&()=?{[]}/something"),
            sha256_digest: [5; 32],
            mtime: SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_nanos(1653660805133248800))
                .unwrap(),
            len: 123456,
            last_seen: SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(1653660810))
                .unwrap(),
            fully_read: SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(1653660817))
                .unwrap(),
        };
        let mut line = [0_u8; 500];
        fi.write(&mut line.as_mut_slice())?;

        let fi_parsed = FileInfo::parse(std::str::from_utf8(&line)?)?;

        assert_eq!(fi, fi_parsed);

        let mut line2 = [0_u8; 500];
        fi_parsed.write(&mut line2.as_mut_slice())?;

        assert_eq!(line, line2);

        Ok(())
    }
}
