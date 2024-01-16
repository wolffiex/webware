use std::fs;
use std::io::{Result, Write};
use std::time::UNIX_EPOCH;
use std::path::Path;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::io::BufWriter;

fn compute_cache_key<P: AsRef<Path>>(path: P) -> Result<u64> {
    let mut entries: Vec<_> = fs::read_dir(path)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>>>()?;

    // sort entries by name
    entries.sort();

    let mut writer = BufWriter::new(Vec::new());
    for entry_path in entries {
        let metadata = fs::metadata(&entry_path)?;
        let mtime = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
        writer.write_all(&entry_path.as_os_str().as_bytes())?;
        writer.write_all(&mtime.to_ne_bytes())?;
    }
    let data = writer.buffer();

    let mut hasher = DefaultHasher::new();
    hasher.write(data);
    Ok(hasher.finish())
}

fn main() -> Result<()> {
    let cache_key = compute_cache_key(".")?;
    println!("Cache Key: {}", cache_key);
    Ok(())
}
