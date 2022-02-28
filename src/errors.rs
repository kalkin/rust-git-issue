use git_wrapper::{CommitError, StagingError, StashingError};
use posix_errors::PosixError;

use crate::{Id, E_ISSUES_DIR_EXIST, E_REPO_BARE, E_REPO_EXIST, E_STASH_ERROR};

/// Failure to find an issue
#[derive(thiserror::Error, Debug)]
pub enum FindError {
    /// When a string matches no issue ids
    #[error("Not found issue with prefix {0}")]
    NotFound(String),
    /// When a string matches multiple issues ids
    #[error("Issue prefix {0} matched multiple issues: {1:?} ")]
    MultipleFound(String, Vec<Id>),
}

impl From<FindError> for PosixError {
    #[inline]
    fn from(e: FindError) -> Self {
        Self::new(git_wrapper::ENOENT, format!("{}", e))
    }
}

/// Error during `DataSource` initialization
#[derive(thiserror::Error, Debug)]
pub enum InitError {
    /// No git repository found
    #[error("Git repository not found")]
    GitRepoNotFound,
    /// No `.issues/` directory found
    #[error("Not an issues repository (or any of the parent directories)")]
    IssuesRepoNotFound,
}

/// Writing an issue property failed
#[derive(thiserror::Error, Debug)]
pub enum WritePropertyError {
    /// Failed to execute `git stage`
    #[error("{0}")]
    StagingError(#[from] StagingError),
    /// IO Failure
    #[error("{0}")]
    IoError(#[from] std::io::Error),
}

/// Failure to write issue changes
#[derive(thiserror::Error, Debug)]
pub enum WriteError {
    /// Failed to write changes to an issue property
    #[error("{0}")]
    PropertyError(#[from] WritePropertyError),
    /// Failed to commit
    #[error("{0}")]
    CommitError(#[from] CommitError),
}

/// Failed to roll back a transaction.
#[derive(thiserror::Error, Debug)]
pub enum RollbackError {
    /// Failed to unstash
    #[error("{0}\nFailed to unstash changes.\nUse git stash pop to do it manually.")]
    Unstash(String),
    #[error("Failed to reset back to commit {0}.\nUse git reset --hard {0}.")]
    /// Failed to reset HEAD
    Reset(String),
    #[error("Failed to reset back to commit {0}.\nTo restore your data use:\n git reset --hard {0} && git stash pop")]
    /// Failed to reset HEAD and unstash.
    ResetUnstash(String),
}

/// Failed to commit transaction.
#[derive(thiserror::Error, Debug)]
pub enum FinishError {
    /// Failed to unstash
    #[error("{0}\nFailed to unstash changes.\nUse git stash pop to do it manually.")]
    Unstash(String),
    /// Failed to reset HEAD
    #[error("Failed to reset back to commit {0}.\nTo restore your repo to previous state, use:\n git reset --hard {0}")]
    Reset(String),
    #[error("Failed to reset back to commit {0}.\nTo restore your repo state, use:\n git reset --hard {0} && git stash pop")]
    /// Failed to reset HEAD and unstash.
    ResetUnstash(String),
    #[error("Failed to merge issue changes with commit {0}.\nTo restore your repo to previous state, use:\n git reset --hard {0}")]
    /// Failed to merge not fast forward branch and unstash.
    MergeUnstash(String),
    /// Failed to merge not fast forward branch
    #[error("Failed to reset back to commit {0}.\nTo restore your data repo to previous state:\n git reset --hard {0} && git stash pop")]
    Merge(String),
}

/// Error during starting a transaction
#[derive(thiserror::Error, Debug)]
pub enum TransactionError {
    /// Bare git repository
    #[error("Can not use bare git repository")]
    BareRepository,
    /// Failed to commit transaction
    #[error("{0}")]
    FinishError(#[from] FinishError),
    /// This should never happen developer fuckup!
    #[error("Bug! Transaction not started!")]
    NotStarted,
    /// Failed to rollback a transaction
    #[error("{0}")]
    RollBackFailed(#[from] RollbackError),
    /// Stashing operation failed
    #[error("{0}")]
    StashingError(#[from] StashingError),
}

impl From<WritePropertyError> for PosixError {
    #[inline]
    fn from(e: WritePropertyError) -> Self {
        match e {
            WritePropertyError::IoError(err) => err.into(),
            WritePropertyError::StagingError(err) => match err {
                StagingError::BareRepository => {
                    Self::new(E_REPO_BARE, "Can not use bare git repository".to_owned())
                }
                StagingError::FileDoesNotExist(p) => {
                    Self::new(posix_errors::EDOOFUS, format!("Unstaged file {:?} Bug?", p))
                }
                StagingError::Failure(msg, code) => Self::new(code, msg),
            },
        }
    }
}
impl From<WriteError> for PosixError {
    #[inline]
    fn from(e: WriteError) -> Self {
        match e {
            WriteError::PropertyError(err) => err.into(),
            WriteError::CommitError(err) => match err {
                CommitError::Failure(msg, code) => Self::new(code, msg),
                CommitError::BareRepository => Self::new(E_REPO_BARE, "Bare repository".to_owned()),
            },
        }
    }
}
impl From<TransactionError> for PosixError {
    #[inline]
    fn from(e: TransactionError) -> Self {
        match e {
            TransactionError::BareRepository => Self::new(E_REPO_BARE, format!("{}", e)),
            TransactionError::NotStarted => Self::new(posix_errors::EDOOFUS, format!("{}", e)),
            TransactionError::RollBackFailed(err) => {
                Self::new(posix_errors::ENOTEXEC, format!("{}", err))
            }
            TransactionError::FinishError(err) => {
                Self::new(posix_errors::ENOTEXEC, format!("{}", err))
            }
            TransactionError::StashingError(err) => Self::new(E_STASH_ERROR, format!("{}", err)),
        }
    }
}
impl From<InitError> for PosixError {
    #[inline]
    fn from(e: InitError) -> Self {
        match e {
            InitError::GitRepoNotFound => Self::new(E_REPO_EXIST, format!("{}", e)),
            InitError::IssuesRepoNotFound => Self::new(E_ISSUES_DIR_EXIST, format!("{}", e)),
        }
    }
}
