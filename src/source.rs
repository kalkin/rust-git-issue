use std::path::{Path, PathBuf};

use git_wrapper::x;
use git_wrapper::Repository;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::errors::{
    FindError, FinishError, InitError, RollbackError, TransactionError, WriteError,
    WritePropertyError,
};
use crate::id::Id;
use crate::Issue;

/// Transaction struct
#[derive(Debug)]
pub struct Transaction {
    start_sha: String,
    stash_before: bool,
}

/// Returned by functions when data change is requested
#[derive(Debug, PartialEq)]
pub enum WriteResult {
    /// The requested data change was applied
    Applied,
    /// The requested data change was redundant and was not applied
    NoChanges,
}

impl From<Vec<Self>> for WriteResult {
    #[inline]
    fn from(list: Vec<Self>) -> Self {
        if list.into_iter().any(|r| r == Self::Applied) {
            return Self::Applied;
        }
        Self::NoChanges
    }
}

#[derive(Debug)]
pub enum Property {
    Description,
    DueDate,
    Tags,
    Milestone,
}

impl Property {
    #[must_use]
    #[inline]
    pub fn filename(&self) -> String {
        match self {
            Self::DueDate => "duedate",
            Self::Description => "description",
            Self::Tags => "tags",
            Self::Milestone => "milestone",
        }
        .to_owned()
    }
}
enum ChangeAction {
    New,
    Edit,
}

#[derive(Debug)]
enum Action {
    Add,
    Remove,
}

enum CommitProperty {
    Description {
        action: ChangeAction,
        id: String,
        description: String,
    },
    Tag {
        action: Action,
        tag: String,
    },
    Milestone {
        action: Action,
        milestone: String,
    },
}
impl CommitProperty {
    #[must_use]
    pub fn filename(&self) -> String {
        match self {
            Self::Description { .. } => "description",
            Self::Tag { .. } => "tags",
            Self::Milestone { .. } => "milestone",
        }
        .to_owned()
    }
}

/// Use this to manipulate your issues
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct DataSource {
    /// Git repository instance
    pub repo: Repository,
    /// Path to `.issues` directory
    pub issues_dir: PathBuf,
    transaction: Option<Transaction>,
}

/// Vector of Strings containing tags
pub type Tags = Vec<String>;

impl<'src> DataSource {
    /// Create new `DataSource` instance
    #[must_use]
    #[inline]
    pub const fn new(issues_dir: PathBuf, repo: Repository) -> Self {
        Self {
            repo,
            issues_dir,
            transaction: None,
        }
    }

    /// Return an iterator over all issues
    #[inline]
    pub fn all(&'src self) -> impl Iterator<Item = std::io::Result<Issue<'src>>> {
        self.all_ids().into_iter().map(|v| match v {
            Ok(id) => {
                let i: Issue<'src> = Issue::new(self, id);
                Ok(i)
            }
            Err(e) => Err(e),
        })
    }

    /// Return an iterator over all issue ids
    #[inline]
    fn all_ids(&self) -> impl Iterator<Item = std::io::Result<Id>> {
        let path = self.issues_dir.join("issues");

        let prefix_dirs = path.read_dir().expect("Directory").filter(dir_filter);
        prefix_dirs.flat_map(Self::list_issue_dirs)
    }

    fn list_issue_dirs(
        dir_entry_result: std::io::Result<std::fs::DirEntry>,
    ) -> Box<dyn Iterator<Item = std::io::Result<Id>>> {
        match dir_entry_result {
            Ok(dir_entry) => match dir_entry.path().read_dir() {
                Ok(ls) => Box::new(ls.filter(dir_filter).map(
                    |result_dir_entry| -> std::io::Result<Id> {
                        result_dir_entry.map(|p| p.path()).map(|p| Id::from(&p))
                    },
                )),
                Err(e) => Box::new(vec![Err(e)].into_iter()),
            },
            Err(e) => Box::new(vec![Err(e)].into_iter()),
        }
    }

    /// # Errors
    ///
    /// Throws an error if fails to create new issue
    #[inline]
    pub fn create_issue(
        &self,
        description: &str,
        tags: Tags,
        milestone: Option<String>,
    ) -> Result<Id, WriteError> {
        let mark_text = "gi new mark";
        let message = format!("gi: Add issue\n\n{}", mark_text);
        self.repo.commit_extended(&message, true, true)?;
        let git_head = self.repo.head();
        let id: Id = Id { id: git_head };
        log::debug!("{} {:?}", mark_text, id);

        self.new_description(&id, description)?;
        log::debug!("gi new description {:?}", id);
        for t in tags {
            self.add_tag(&id, &t)?;
            log::debug!("gi tag add {}", t);
        }
        if let Some(m) = milestone {
            self.add_milestone(&id, &m)?;
            log::debug!("gi milestone add {}", m);
        }
        Ok(id)
    }

    /// Return the creation date
    #[inline]
    #[must_use]
    pub fn creation_date(&self, id: &Id) -> OffsetDateTime {
        let mut cmd = self.repo.git();
        cmd.args(&["show", "--no-patch", "--format=%aI", id.id()]);
        let out = cmd.output().expect("Failed to execute git-stash(1)");

        let output = String::from_utf8_lossy(&out.stdout);
        let date_text = output.trim();
        OffsetDateTime::parse(date_text, &Rfc3339).expect("Valid RFC-3339 date")
    }

    /// # Errors
    ///
    /// Will throw an error when:
    /// - Fails to find a non-bare git repository
    /// - Fails to resolve HEAD ref
    #[inline]
    pub fn try_new(
        git_dir: &Option<String>,
        work_tree: &Option<String>,
    ) -> Result<Self, InitError> {
        let path = std::env::current_dir().expect("Failed to get CWD");
        let issues_dir = Self::find_issues_dir(&path).ok_or(InitError::IssuesRepoNotFound)?;
        let repo = match Repository::from_args(
            Some(&issues_dir.to_string_lossy()),
            git_dir.as_deref(),
            work_tree.as_deref(),
        ) {
            Ok(repo) => Ok(repo),
            Err(_) => Err(InitError::GitRepoNotFound),
        }?;
        Ok(Self {
            repo,
            issues_dir,
            transaction: None,
        })
    }

    /// # Errors
    ///
    /// Returns an error if no issue matching id found or more than one issue are found.
    #[inline]
    pub fn find_issue(&self, needle: &str) -> Result<Id, FindError> {
        match needle.len() {
            1 => {
                let path = self.issues_dir.join("issues");
                let dirs: Vec<PathBuf> = list_dirs(&path);
                if dirs.len() == 1 {
                    let prefix = &dirs[0].file_name().expect("File name").to_str().expect("");
                    self.find_issue(prefix)
                } else {
                    let ids: Vec<Id> = dirs
                        .into_iter()
                        .flat_map(|p: PathBuf| {
                            list_dirs(&p).iter().map(Id::from).collect::<Vec<Id>>()
                        })
                        .collect();
                    Err(FindError::MultipleFound(needle.to_owned(), ids))
                }
            }
            2 => {
                let path = self.issues_dir.join("issues").join(&needle);
                if path.exists() {
                    let dirs: Vec<PathBuf> = list_dirs(&path);
                    match dirs.len() {
                        0 => Err(FindError::NotFound(needle.to_owned())),
                        1 => Ok(Id::from(&dirs[0])),
                        _ => {
                            let ids = dirs.iter().map(Id::from).collect();
                            Err(FindError::MultipleFound(needle.to_owned(), ids))
                        }
                    }
                } else {
                    Err(FindError::NotFound(needle.to_owned()))
                }
            }
            _ => {
                {
                    let path = self
                        .issues_dir
                        .join("issues")
                        .join(&needle[..2])
                        .join(&needle[2..]);
                    if path.exists() {
                        return Ok(Id {
                            id: needle.to_owned(),
                        });
                    }
                }
                let path = self.issues_dir.join("issues").join(&needle[..2]);
                let ids: Vec<Id> = list_dirs(&path)
                    .iter()
                    .map(Id::from)
                    .filter(|id| id.id().starts_with(needle))
                    .collect();
                match ids.len() {
                    0 => Err(FindError::NotFound(needle.to_owned())),
                    1 => Ok(ids[0].clone()),
                    _ => Err(FindError::MultipleFound(needle.to_owned(), ids)),
                }
            }
        }
    }

    /// Close an issue
    ///
    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn close_issue(&self, id: &Id) -> Result<WriteResult, WriteError> {
        let remove_result = self.remove_tag(id, "open")?;
        let add_result = self.add_tag(id, "closed")?;
        Ok(WriteResult::from(vec![remove_result, add_result]))
    }

    fn find_issues_dir(p: &Path) -> Option<PathBuf> {
        let mut cur = p.to_path_buf();
        loop {
            let needle = cur.join(".issues");
            if needle.exists() {
                return Some(needle);
            }
            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                return None;
            }
        }
    }

    /// Start transaction
    ///
    /// # Errors
    ///
    /// Will fail when `HEAD` can not be resolved
    #[inline]
    pub fn start_transaction(&mut self) -> Result<(), TransactionError> {
        let start_sha = self.repo.head();

        let stash_before = !self.repo.is_clean();
        let transaction = Transaction {
            start_sha,
            stash_before,
        };
        if stash_before {
            log::debug!("Stashing repository changes");
            self.repo.stash_almost_all("git-issue: Start Transaction")?;
        }
        self.transaction = Some(transaction);
        Ok(())
    }

    /// Rolls back any commits made during the transaction and restores stashed changes if any.
    ///
    /// # Errors
    ///
    /// Throws an error when `git reset --hard` or poping stashed changes fails.
    #[inline]
    pub fn rollback_transaction(mut self) -> Result<(), TransactionError> {
        let transaction = self.transaction.ok_or(TransactionError::NotStarted)?;
        x::reset_hard(&self.repo, &transaction.start_sha).map_err(|e| {
            if transaction.stash_before {
                RollbackError::ResetUnstash(e.message())
            } else {
                RollbackError::Reset(e.message())
            }
        })?;
        if transaction.stash_before {
            log::debug!("Unstashing repository changes");
            self.repo
                .stash_pop()
                .map_err(|e| RollbackError::Unstash(format!("{}", e)))?;
        }
        self.transaction = None;
        Ok(())
    }

    /// # Errors
    ///
    /// Will throw error on failure to read from file
    #[inline]
    pub(crate) fn read(&self, id: &Id, prop: &Property) -> Result<String, std::io::Error> {
        let path = id.path(&self.issues_dir).join(prop.filename());
        Ok(std::fs::read_to_string(path)?.trim_end().to_owned())
    }

    /// Returns duedate of an issue
    ///
    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn duedate(&self, id: &Id) -> Result<Option<OffsetDateTime>, std::io::Error> {
        match self.read(id, &Property::DueDate) {
            Ok(date_text) => Ok(Some(
                OffsetDateTime::parse(&date_text, &Rfc3339).expect("Valid RFC-3339 date"),
            )),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(e),
            },
        }
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn new_description(&self, id: &Id, text: &str) -> Result<(), WriteError> {
        let tag = CommitProperty::Tag {
            action: Action::Add,
            tag: "open".to_owned(),
        };
        let description = CommitProperty::Description {
            action: ChangeAction::New,
            id: id.id().to_owned(),
            description: text.to_owned(),
        };
        #[cfg(feature = "strict-compatibility")]
        {
            self.write_to_file(id, &tag)?;
            self.write(id, &description).map_err(Into::into)
        }
        #[cfg(not(feature = "strict-compatibility"))]
        {
            self.write(id, &description)?;
            self.write(id, &tag).map_err(Into::into)
        }
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn edit_description(&self, id: &Id, text: &str) -> Result<(), WriteError> {
        let property = CommitProperty::Description {
            action: ChangeAction::Edit,
            id: id.id().to_owned(),
            description: text.to_owned(),
        };
        self.write(id, &property).map_err(Into::into)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn add_tag(&self, id: &Id, tag: &str) -> Result<WriteResult, WriteError> {
        if self.tags(id).contains(&tag.to_owned()) {
            Ok(WriteResult::NoChanges)
        } else {
            let property = CommitProperty::Tag {
                action: Action::Add,
                tag: tag.to_owned(),
            };
            self.write(id, &property).map_err(WriteError::from)?;
            Ok(WriteResult::Applied)
        }
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn remove_tag(&self, id: &Id, tag: &str) -> Result<WriteResult, WriteError> {
        if self.tags(id).contains(&tag.to_owned()) {
            let property = CommitProperty::Tag {
                action: Action::Remove,
                tag: tag.to_owned(),
            };
            self.write(id, &property).map_err(WriteError::from)?;
            Ok(WriteResult::Applied)
        } else {
            Ok(WriteResult::NoChanges)
        }
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn add_milestone(&self, id: &Id, milestone: &str) -> Result<WriteResult, WriteError> {
        if let Some(cur_milestone) = self.milestone(id) {
            if cur_milestone == milestone {
                return Ok(WriteResult::NoChanges);
            }
        }
        let property = CommitProperty::Milestone {
            action: Action::Add,
            milestone: milestone.to_owned(),
        };
        self.write(id, &property).map_err(WriteError::from)?;
        Ok(WriteResult::Applied)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn remove_milestone(&self, id: &Id) -> Result<WriteResult, WriteError> {
        if let Some(milestone) = self.milestone(id) {
            let property = CommitProperty::Milestone {
                action: Action::Remove,
                milestone,
            };
            self.write(id, &property).map_err(WriteError::from)?;
            Ok(WriteResult::Applied)
        } else {
            Ok(WriteResult::NoChanges)
        }
    }

    /// Returns milestone of an issue if set.
    #[must_use]
    #[inline]
    pub fn milestone(&self, id: &Id) -> Option<String> {
        self.read(id, &Property::Milestone).ok()
    }

    /// # Errors
    ///
    /// Will throw error on failure to read from description file
    #[inline]
    pub fn title(&self, id: &Id) -> Result<String, std::io::Error> {
        let description = self.read(id, &Property::Description)?;
        Ok(description.lines().next().unwrap_or("").to_owned())
    }

    /// Returns tags for an issue
    #[must_use]
    #[inline]
    pub fn tags(&self, id: &Id) -> Tags {
        self.read(id, &Property::Tags)
            .map(|v| {
                v.trim()
                    .lines()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn write_to_file(&self, id: &Id, property: &CommitProperty) -> Result<(), WritePropertyError> {
        let dir_path = id.path(&self.issues_dir);
        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path)?;
        }
        let path = &dir_path.join(property.filename());

        // Execute write
        log::debug!("Writing {:?}", path);
        match property {
            CommitProperty::Description { description, .. } => {
                std::fs::write(path, format!("{}\n", description.trim_end()))?;
            }
            CommitProperty::Tag { tag, action, .. } => {
                let value = std::fs::read_to_string(path);
                let mut tags = if value.is_ok() && path.exists() {
                    value
                        .as_ref()
                        .expect("value is set at this point")
                        .lines()
                        .collect::<Vec<&str>>()
                } else {
                    vec![]
                };
                match action {
                    Action::Add => {
                        tags.push(tag);
                    }
                    Action::Remove => {
                        tags.retain(|t| *t != tag);
                    }
                }
                tags.sort_unstable();
                tags.dedup();
                std::fs::write(path, format!("{}\n", tags.join("\n")))?;
            }
            CommitProperty::Milestone {
                milestone, action, ..
            } => match action {
                Action::Add => {
                    std::fs::write(path, format!("{}\n", milestone))?;
                }
                Action::Remove => {
                    std::fs::remove_file(path)?;
                }
            },
        };

        log::debug!("Staging {:?}", &path);
        self.repo.stage(path).map_err(Into::into)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO or commiting
    fn write(&self, target_id: &Id, property: &CommitProperty) -> Result<(), WriteError> {
        self.write_to_file(target_id, property)?;

        let message = match property {
            CommitProperty::Description {
                action: ChangeAction::New,
                id,
                ..
            } => format!("gi: Add issue description\n\ngi new description {}", id),
            CommitProperty::Description {
                action: ChangeAction::Edit,
                id,
                ..
            } => format!("gi: Edit issue description\n\ngit edit description {}", id),
            CommitProperty::Tag {
                action: Action::Add,
                tag,
                ..
            } => {
                #[cfg(feature = "strict-compatibility")]
                {
                    format!("gi: Add tag\n\ngi tag add {}", tag)
                }
                #[cfg(not(feature = "strict-compatibility"))]
                {
                    format!(
                        "gi({}): Add tag {}\n\ngi tag add {}",
                        &target_id.short_id(),
                        tag,
                        tag
                    )
                }
            }
            CommitProperty::Tag {
                action: Action::Remove,
                tag,
                ..
            } => {
                #[cfg(feature = "strict-compatibility")]
                {
                    format!("gi: Remove tag\n\ngi tag remove {}", tag)
                }
                #[cfg(not(feature = "strict-compatibility"))]
                {
                    format!(
                        "gi({}): Remove tag {}\n\ngi tag remove {}",
                        &target_id.short_id(),
                        tag,
                        tag
                    )
                }
            }
            CommitProperty::Milestone {
                action: Action::Add,
                milestone,
                ..
            } => {
                #[cfg(feature = "strict-compatibility")]
                {
                    format!("gi: Add milestone\n\ngi milestone add {}", milestone)
                }
                #[cfg(not(feature = "strict-compatibility"))]
                {
                    format!(
                        "gi({}): Add milestone {}\n\ngi milestone add {}",
                        &target_id.short_id(),
                        milestone,
                        milestone
                    )
                }
            }
            CommitProperty::Milestone {
                action: Action::Remove,
                milestone,
                ..
            } => {
                #[cfg(feature = "strict-compatibility")]
                {
                    format!("gi: Remove milestone\n\ngi milestone remove {}", milestone)
                }
                #[cfg(not(feature = "strict-compatibility"))]
                {
                    format!(
                        "gi({}): Remove milestone {}\n\ngi milestone remove {}",
                        &target_id.short_id(),
                        milestone,
                        milestone
                    )
                }
            }
        };

        self.repo
            .commit_extended(&message, false, true)
            .map_err(Into::into)
    }

    /// # Errors
    ///
    /// Will throw error on failure to commit
    #[inline]
    pub fn finish_transaction_without_merge(&mut self) -> Result<(), TransactionError> {
        let transaction = &self.transaction.as_ref().expect("A started transaction");
        if transaction.stash_before {
            log::debug!("Unstashing repository changes");
            self.repo
                .stash_pop()
                .map_err(|e| FinishError::Unstash(format!("{}", e)))?;
        }
        self.transaction = None;
        Ok(())
    }

    /// # Errors
    ///
    /// Will throw error on failure to commit
    #[allow(unused_variables)]
    #[inline]
    pub fn finish_transaction(&mut self, message: &str) -> Result<(), TransactionError> {
        let transaction = &self.transaction.as_ref().expect("A started transaction");
        #[cfg(not(feature = "strict-compatibility"))]
        {
            log::info!("Merging issue changes as not fast forward branch");
            let sha = self.repo.head();
            x::reset_hard(&self.repo, &transaction.start_sha).map_err(|e| {
                if transaction.stash_before {
                    TransactionError::FinishError(FinishError::ResetUnstash(e.message()))
                } else {
                    TransactionError::FinishError(FinishError::Reset(e.message()))
                }
            })?;

            let mut cmd = self.repo.git();
            let out = cmd
                .args(&["merge", "--no-ff", "-m", message, &sha])
                .output()
                .expect("Failed to execute git-stash(1)");

            if !out.status.success() {
                let output = String::from_utf8_lossy(&out.stderr).to_string();
                if transaction.stash_before {
                    return Err(TransactionError::FinishError(FinishError::MergeUnstash(
                        output,
                    )));
                }
                return Err(TransactionError::FinishError(FinishError::Merge(output)));
            }
        }
        if transaction.stash_before {
            log::debug!("Unstashing repository changes");
            self.repo
                .stash_pop()
                .map_err(|e| FinishError::Unstash(format!("{}", e)))?;
        }
        self.transaction = None;
        Ok(())
    }
}

impl TryFrom<&Path> for DataSource {
    type Error = InitError;
    #[inline]
    fn try_from(p: &Path) -> Result<Self, Self::Error> {
        let issues_dir = Self::find_issues_dir(p).ok_or(InitError::IssuesRepoNotFound)?;
        let repo = Repository::from_args(
            Some(issues_dir.to_str().expect("Convert to string")),
            None,
            None,
        )
        .map_err(|_err| InitError::GitRepoNotFound)?;
        Ok(Self::new(issues_dir, repo))
    }
}

fn dir_filter(read_dir_result: &Result<std::fs::DirEntry, std::io::Error>) -> bool {
    read_dir_result
        .as_ref()
        .map(|dir_entry| dir_entry.metadata().map(|d| d.is_dir()).unwrap_or(false))
        .unwrap_or(false)
}

fn list_dirs(path: &Path) -> Vec<PathBuf> {
    if !path.exists() {
        return vec![];
    }
    let paths: Vec<PathBuf> = path
        .read_dir()
        .expect("Directory")
        .filter(dir_filter)
        .map(|d| d.expect("IO Successful").path())
        .collect();
    paths
}
