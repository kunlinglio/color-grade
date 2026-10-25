use std::io;
use std::path::Path;

use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaHash(u128);

impl MediaHash {
    pub fn from_file(path: &Path) -> io::Result<Self> {
        Ok(MediaHash(hash_file(path)?))
    }
}

fn hash_file(path: &Path) -> io::Result<u128> {
    let mut file = File::open(path)?;
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.digest128())
}
