use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

use getset::Getters;

/// Issue id
#[derive(Clone, Getters, PartialEq, Eq)]
pub struct Id {
    /// The id itself
    #[getset(get = "pub")]
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

/// Comment id
#[derive(Debug, Eq, Getters, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub struct CommentId {
    /// full id
    #[getset(get = "pub")]
    id: String,
}

impl From<PathBuf> for CommentId {
    #[inline]
    #[must_use]
    fn from(path: PathBuf) -> Self {
        let file_name = &path.file_name().expect("File name").to_string_lossy();
        Self {
            id: file_name.to_string(),
        }
    }
}
impl From<String> for CommentId {
    #[inline]
    #[must_use]
    fn from(id: String) -> Self {
        Self { id }
    }
}

impl CommentId {
    /// Return the short id as string
    #[inline]
    #[must_use]
    pub fn short_id(&self) -> &str {
        &self.id[..8]
    }
}
