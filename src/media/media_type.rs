use std::path::PathBuf;

use crate::media::hash::MediaHash;

#[derive(Debug, Copy, Clone)]
pub enum MediaKind {
    JPEG,
    PNG,
}

impl MediaKind {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::JPEG => "jpg",
            Self::PNG => "png",
        }
    }

    fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            "jpg" => Some(Self::JPEG),
            "png" => Some(Self::PNG),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
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
