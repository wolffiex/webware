use rayon::prelude::*;
use std::hash::{Hash, Hasher};
use std::fs;
use std::time::UNIX_EPOCH;
use std::io::Result;
use std::path::Path;

use std::collections::hash_map::DefaultHasher;

fn compute_cache_key<P: AsRef<Path>>(path: P) -> Result<u64> {
    let entries: Vec<_> = fs::read_dir(&path)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>>>()?;

    let mut hasher = DefaultHasher::new();

    entries.par_iter().try_for_each(|entry_path| {
        let metadata = fs::metadata(&entry_path)?;
        let mtime = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
        let relative_path = entry_path.strip_prefix(&path)?.as_os_str().as_bytes();

        relative_path.hash(&mut hasher);
        mtime.hash(&mut hasher);

        Ok(())
    })?;

    Ok(hasher.finish())
}

fn main() -> Result<()> {
    let cache_key = compute_cache_key(".")?;
    println!("Cache Key: {}", cache_key);
    Ok(())
}
