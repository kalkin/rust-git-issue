use git_wrapper::{PosixError, EINVAL};

pub type Cache<T> = Option<T>;

/// Error during caching
#[allow(missing_docs)]
#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error(transparent)]
    ParseError(#[from] time::error::Parse),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<CacheError> for PosixError {
    #[inline]
    fn from(e: CacheError) -> Self {
        match e {
            CacheError::Io(err) => Self::from(err),
            CacheError::ParseError(err) => Self::new(EINVAL, format!("{}", err)),
        }
    }
}
