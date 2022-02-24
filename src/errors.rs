use std::path::Path;

use git_wrapper::{CommitError, Repository, StagingError, StashingError};
use posix_errors::PosixError;

use crate::{DataSource, Id, E_ISSUES_DIR_EXIST, E_REPO_BARE, E_REPO_EXIST, E_STASH_ERROR};

#[derive(thiserror::Error, Debug)]
pub enum FindError {
    #[error("Not found issue with prefix {0}")]
    NotFound(String),
    #[error("Issue prefix {0} matched multiple issues: {1:?} ")]
    MultipleFound(String, Vec<Id>),
}

impl From<FindError> for PosixError {
    #[inline]
    fn from(e: FindError) -> Self {
        Self::new(git_wrapper::ENOENT, format!("{}", e))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InitError {
    #[error("Git repository not found")]
    GitRepoNotFound,
    #[error("Not an issues repository (or any of the parent directories)")]
    IssuesRepoNotFound,
}

#[derive(thiserror::Error, Debug)]
pub enum WritePropertyError {
    #[error("{0}")]
    StagingError(#[from] StagingError),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum WriteError {
    #[error("{0}")]
    PropertyError(#[from] WritePropertyError),
    #[error("{0}")]
    CommitError(#[from] CommitError),
}

#[derive(thiserror::Error, Debug)]
pub enum RollbackError {
    #[error("{0}\nFailed to unstash changes.\nUse git stash pop to do it manually.")]
    Unstash(String),
    #[error("Failed to reset back to commit {0}.\nUse git reset --hard {0}.")]
    Reset(String),
    #[error("Failed to reset back to commit {0}.\nTo restore your data use:\n git reset --hard {0} && git stash pop")]
    ResetUnstash(String),
}

#[derive(thiserror::Error, Debug)]
pub enum FinishError {
    #[error("{0}\nFailed to unstash changes.\nUse git stash pop to do it manually.")]
    Unstash(String),
    #[error("Failed to reset back to commit {0}.\nTo restore your repo to previous state, use:\n git reset --hard {0}")]
    Reset(String),
    #[error("Failed to reset back to commit {0}.\nTo restore your repo state, use:\n git reset --hard {0} && git stash pop")]
    ResetUnstash(String),
    #[error("Failed to merge issue changes with commit {0}.\nTo restore your repo to previous state, use:\n git reset --hard {0}")]
    MergeUnstash(String),
    #[error("Failed to reset back to commit {0}.\nTo restore your data repo to previous state:\n git reset --hard {0} && git stash pop")]
    Merge(String),
}

#[derive(thiserror::Error, Debug)]
pub enum TransactionError {
    #[error("Can not use bare git repository")]
    BareRepository,
    #[error("{0}")]
    FinishError(#[from] FinishError),
    #[error("Bug! Transaction not started!")]
    NotStarted,
    #[error("{0}")]
    RollBackFailed(#[from] RollbackError),
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

impl TryFrom<&Path> for DataSource {
    type Error = InitError;
    #[inline]
    fn try_from(p: &Path) -> Result<Self, Self::Error> {
        let issues_dir = Self::find_issues_dir(p).ok_or(InitError::IssuesRepoNotFound)?;
        let repo = Repository::from_args(Some(issues_dir.to_str().unwrap()), None, None)
            .map_err(|_err| InitError::GitRepoNotFound)?;
        Ok(Self::new(issues_dir, repo))
    }
}
