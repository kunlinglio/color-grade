use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum MediaKind {
    JPEG,
    PNG,
}

impl MediaKind {
    fn from_ext(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "jpg" => Some(Self::JPEG),
            "png" => Some(Self::PNG),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Media {
    pub path: PathBuf,
    pub kind: MediaKind,
    pub hash: MediaHash,
}

impl Media {
    pub fn from_path(path: PathBuf) -> Option<Self> {
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            return None;
        };
        let Some(kind) = MediaKind::from_ext(ext) else {
            return None;
        };
        let Ok(hash) = MediaHash::from_file(&path) else {
            return None;
        };
        Some(Media { path, kind, hash })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
