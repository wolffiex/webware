use anyhow::{anyhow, Context, Result};
use fnv::FnvHasher;
use rayon::prelude::*;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;
use std::{collections::HashMap, path::PathBuf};

pub fn compute_cache_key(directory: &PathBuf) -> Result<u64> {
    let entries: Vec<_> = fs::read_dir(directory)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|e| anyhow!("Failed to read directory entries: {:?}", e))?;

    let mut fs_info: Vec<(u64, String)> = entries
        .into_par_iter()
        .map(|entry_path| {
            let metadata = fs::metadata(&entry_path).context("Failed to get metadata")?;
            let mtime = metadata
                .modified()
                .context("Failed to get mtime")?
                .duration_since(UNIX_EPOCH)
                .context("Invalid timestamp")?
                .as_secs();
            let relative_path = entry_path
                .strip_prefix(&directory)
                .context("Failed to file path")?
                .as_os_str()
                .to_string_lossy()
                .into_owned();
            Ok((mtime, relative_path))
        })
        .collect::<Result<Vec<(u64, String)>>>()?;

    fs_info.par_sort_unstable_by_key(|k| k.0);

    let mut hasher = FnvHasher::default();
    for (mtime, path) in fs_info {
        hasher.write_u64(mtime);
        hasher.write(path.as_bytes());
    }

    Ok(hasher.finish())
}

pub struct DirectoryCache<T> {
    directory: PathBuf,
    current_key: u64,
    _storage: HashMap<String, T>,
}

impl<T> DirectoryCache<T> {
    pub fn new(directory: PathBuf) -> DirectoryCache<T> {
        DirectoryCache {
            directory,
            current_key: 0,
            _storage: HashMap::new(),
        }
    }

    pub fn get_or_insert<F: FnOnce() -> T>(&mut self, key: String, inserter: F) -> &T {
        let new_key = compute_cache_key(&self.directory).unwrap();
        if self.current_key != new_key {
            self.current_key = new_key;
            self._storage = HashMap::new();
        }
        self._storage.entry(key).or_insert_with(inserter)
    }
}
