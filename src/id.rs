use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

/// Issue id
#[derive(Clone, PartialEq)]
pub struct Id {
    pub(crate) id: String,
}

#[cfg(not(tarpaulin_include))]
impl std::fmt::Debug for Id {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id().fmt(f)
    }
}

impl Id {
    /// Create new instance
    #[inline]
    #[must_use]
    pub const fn new(id: String) -> Self {
        Self { id }
    }

    /// Return path to issue directory
    #[inline]
    #[must_use]
    pub fn path(&self, path: &Path) -> PathBuf {
        path.join("issues")
            .join(&self.id()[..2])
            .join(&self.id()[2..])
    }

    /// Returns full id
    #[inline]
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns id shortened to 8 chars
    #[inline]
    #[must_use]
    pub fn short_id(&self) -> &str {
        &self.id()[0..8]
    }
}

impl From<&PathBuf> for Id {
    #[inline]
    fn from(path: &PathBuf) -> Self {
        let parent = path.parent().expect("parent dir");
        let prefix = parent.file_name().expect("File name").to_str().expect("");
        let file_name = path.file_name().expect("File name").to_str().expect("");

        Self {
            id: format!("{}{}", prefix, file_name),
        }
    }
}

impl From<DirEntry> for Id {
    #[inline]
    fn from(entry: DirEntry) -> Self {
        let path_buf = entry.path();
        Self::from(&path_buf)
    }
}
