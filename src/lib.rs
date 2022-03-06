//! Library for manipulating a data in a git-issue(1) tracker

use std::path::{Path, PathBuf};

use git_wrapper::x;
use git_wrapper::{CommitError, Repository};
use posix_errors::PosixError;

mod errors;
pub use crate::errors::*;

/// `$EDITOR` was quit with error
pub const E_EDITOR_KILLED: i32 = posix_errors::EINTR; // 4

// Repository errors
/// Repository already exists.
pub const E_REPO_EXIST: i32 = 128 + posix_errors::EEXIST; // 135
/// Bare Repository
pub const E_REPO_BARE: i32 = 128 + posix_errors::EPROTOTYPE; // 169

/// .issues directory missing
pub const E_ISSUES_DIR_EXIST: i32 = 128 + 16 + posix_errors::EEXIST; // 151

/// Stashing operation failed
pub const E_STASH_ERROR: i32 = 128 + 16 + 16 + posix_errors::EIO; // 165

/// Transaction struct
#[derive(Debug)]
pub struct Transaction {
    start_sha: String,
    stash_before: bool,
}

/// Issue id
#[derive(Clone, PartialEq)]
pub struct Id(pub String);

#[cfg(not(tarpaulin_include))]
impl std::fmt::Debug for Id {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Id {
    #[must_use]
    fn path(&self, path: &Path) -> PathBuf {
        path.join("issues").join(&self.0[..2]).join(&self.0[2..])
    }
}

impl From<&PathBuf> for Id {
    #[inline]
    fn from(path: &PathBuf) -> Self {
        let parent = path.parent().expect("parent dir");
        let prefix = parent.file_name().expect("File name").to_str().expect("");
        let file_name = path.file_name().expect("File name").to_str().expect("");

        Self(format!("{}{}", prefix, file_name))
    }
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
enum Property {
    Description,
    Tags,
    Milestone,
}

impl Property {
    #[must_use]
    #[inline]
    pub fn filename(&self) -> String {
        match self {
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
pub struct DataSource {
    /// Git repository instance
    pub repo: Repository,
    /// Path to `.issues` directory
    pub issues_dir: PathBuf,
    transaction: Option<Transaction>,
}

impl DataSource {
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

    /// # Errors
    ///
    /// Throws an error if fails to create new issue
    #[inline]
    pub fn create_issue(
        &self,
        description: &str,
        tags: Vec<String>,
        milestone: Option<String>,
    ) -> Result<Id, WriteError> {
        let mark_text = "gi new mark";
        let message = format!("gi: Add issue\n\n{}", mark_text);
        self.repo.commit_extended(&message, true, true)?;
        let git_head = self.repo.head().expect("At this point HEAD should exist");
        let id: Id = Id(git_head);
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
                        return Ok(Id(needle.to_owned()));
                    }
                }
                let path = self.issues_dir.join("issues").join(&needle[..2]);
                let ids: Vec<Id> = list_dirs(&path)
                    .iter()
                    .map(Id::from)
                    .filter(|id| id.0.starts_with(needle))
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
        let start_sha = self.repo.head().ok_or(TransactionError::BareRepository)?;

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
    fn read(&self, id: &Id, prop: &Property) -> Result<String, std::io::Error> {
        let path = id.path(&self.issues_dir).join(prop.filename());
        Ok(std::fs::read_to_string(path)?.trim_end().to_owned())
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
            id: id.0.clone(),
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
            id: id.0.clone(),
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
    pub fn remove_milestone(&self, id: &Id, milestone: &str) -> Result<(), WriteError> {
        let property = CommitProperty::Milestone {
            action: Action::Add,
            milestone: milestone.to_owned(),
        };
        self.write(id, &property).map_err(Into::into)
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
    pub fn tags(&self, id: &Id) -> Vec<String> {
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
                    value.as_ref().unwrap().lines().collect::<Vec<&str>>()
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
            } => format!("gi: Add tag\n\ngi tag add {}", tag),
            CommitProperty::Tag {
                action: Action::Remove,
                tag,
                ..
            } => format!("gi: Remove tag\n\ngi tag remove {}", tag),
            CommitProperty::Milestone {
                action: Action::Add,
                milestone,
                ..
            } => format!("gi: Add milestone\n\ngi milestone add {}", milestone),
            CommitProperty::Milestone {
                action: Action::Remove,
                milestone,
                ..
            } => format!("gi: Remove milestone\n\ngi milestone remove {}", milestone),
        };

        self.repo
            .commit_extended(&message, false, true)
            .map_err(Into::into)
    }

    /// # Errors
    ///
    /// Will throw error on failure to commit
    #[inline]
    pub fn finish_transaction(&mut self, message: &str) -> Result<(), TransactionError> {
        let transaction = &self.transaction.as_ref().expect("A started transaction");
        #[cfg(not(feature = "strict-compatibility"))]
        {
            log::info!("Merging issue changes as not fast forward branch");
            let sha = self.repo.head().ok_or(TransactionError::BareRepository)?;
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
        let repo = Repository::from_args(Some(issues_dir.to_str().unwrap()), None, None)
            .map_err(|_err| InitError::GitRepoNotFound)?;
        Ok(Self::new(issues_dir, repo))
    }
}

const DESCRIPTION: &str = "

# Start with a one-line summary of the issue.  Leave a blank line and
# continue with the issue's detailed description.
#
# Remember:
# - Be precise
# - Be clear: explain how to reproduce the problem, step by step,
#   so others can reproduce the issue
# - Include only one problem per issue report
#
# Lines starting with '#' will be ignored, and an empty message aborts
# the issue addition.
";

const COMMENT: &str = "

# Please write here a comment regarding the issue.
# Keep the conversation constructive and polite.
# Lines starting with '#' will be ignored, and an empty message aborts
# the issue addition.
";

const README: &str = "This is an distributed issue tracking repository based on Git.
Visit [git-issue](https://github.com/dspinellis/git-issue) for more information.
";

/// Read a template file from `.issues/.templates`
#[must_use]
#[inline]
pub fn read_template(repo: &Repository, template: &str) -> Option<String> {
    let mut path_buf = repo.work_tree().expect("Non bare repository");
    path_buf = path_buf.join(".issues");
    path_buf = path_buf.join("templates");
    path_buf = path_buf.join(template);
    std::fs::read_to_string(path_buf).ok()
}

/// # Errors
///
/// Throws an error when it fails to create repository or to make a commit
#[inline]
pub fn create(path: &Path, existing: bool) -> Result<(), PosixError> {
    let issues_dir = path.join(".issues");
    if issues_dir.exists() {
        return Err(PosixError::new(
            posix_errors::EEXIST,
            "An .issues directory is already present".to_owned(),
        ));
    }
    std::fs::create_dir_all(&issues_dir)?;

    let repo = if existing {
        match Repository::default() {
            Err(e) => Err(e.into()),
            Ok(r) => Ok(r),
        }
    } else {
        match Repository::create(&issues_dir) {
            Ok(r) => Ok(r),
            Err(e) => Err(PosixError::new(1, e)),
        }
    }?;

    let config = issues_dir.join("config");
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&config)?;
    repo.stage(&config)?;

    let templates = issues_dir.join("templates");
    std::fs::create_dir_all(&templates)?;

    let description = templates.join("description");
    std::fs::write(&description, DESCRIPTION)?;
    repo.stage(&description)?;

    let comment = templates.join("comment");
    std::fs::write(&comment, COMMENT)?;
    repo.stage(&comment)?;

    let readme = issues_dir.join("README.md");
    std::fs::write(&readme, README)?;
    repo.stage(&readme)?;

    let message = "gi: Initialize issues repository\n\ngi init";
    match repo.commit_extended(message, false, false) {
        Ok(_) => Ok(()),
        Err(CommitError::Failure(msg, code)) => Err(PosixError::new(code, msg)),
        Err(CommitError::BareRepository) => {
            Err(PosixError::new(E_REPO_BARE, "Bare repository".to_owned()))
        }
    }
}

/// # Errors
///
/// Throws an error when any read/write operation fails or the editor exits with error
#[inline]
pub fn edit(repo: &Repository, text: &str) -> Result<String, PosixError> {
    let mut tmpfile = repo.work_tree().expect("Non bare repository");
    tmpfile = tmpfile.join(".issues");
    tmpfile = tmpfile.join("TMP");
    std::fs::write(&tmpfile, text)?;
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .expect("VISUAL or EDITOR is set");
    let mut cmd = std::process::Command::new(editor);
    cmd.arg(&tmpfile);
    let result = match cmd
        .spawn()
        .expect("Failed to execute EDITOR")
        .wait()?
        .code()
    {
        None => Err(PosixError::new(
            E_EDITOR_KILLED,
            "Process terminated by signal".to_owned(),
        )),
        Some(0) => {
            let input = std::fs::read_to_string(&tmpfile)?;
            let lines = input.lines();
            Ok(lines
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<&str>>()
                .join("\n"))
        }
        Some(1) => Err(PosixError::new(1, "Editor aborted".to_owned())),
        Some(code) => Err(PosixError::new(code, "Editor exited with error".to_owned())),
    };
    #[allow(unused_must_use)]
    {
        // We do not care if we succseed in removing TMP file
        std::fs::remove_file(tmpfile);
    }
    result
}

fn list_dirs(path: &Path) -> Vec<PathBuf> {
    if !path.exists() {
        return vec![];
    }
    let paths: Vec<PathBuf> = path
        .read_dir()
        .expect("Directory")
        .filter(|x| {
            if let Ok(dir_entry) = x {
                if let Ok(meta) = dir_entry.metadata() {
                    return meta.is_dir();
                }
            }
            false
        })
        .map(|d| d.expect("IO Successful").path())
        .collect();
    paths
}

#[cfg(test)]
#[cfg(not(tarpaulin_include))]
mod test_find_issue {
    use crate::DataSource;
    #[test]
    fn by_full_id() {
        let data = DataSource::try_new(&None, &None).unwrap();
        let issue = data
            .find_issue("2d9deaf1b8b146d7e3c4c92133532b314da3e350")
            .expect("Found issue");
        assert_eq!(issue.0, "2d9deaf1b8b146d7e3c4c92133532b314da3e350");
    }

    #[test]
    fn by_one_char_multiple() {
        let data = DataSource::try_new(&None, &None).unwrap();
        let issue = data.find_issue("2");
        assert!(issue.is_err());
    }

    #[test]
    fn by_one_char() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let issue_id = data.create_issue(&"Foo Bar", vec![], None).unwrap();
        let needle = issue_id.0.chars().next().unwrap().to_string();
        let issue = data.find_issue(&needle).expect("Found issue");
        assert_eq!(issue_id, issue);
    }

    #[test]
    fn by_two_chars() {
        let data = DataSource::try_new(&None, &None).unwrap();
        let issue = data.find_issue("2d").expect("Found issue");
        assert_eq!(issue.0, "2d9deaf1b8b146d7e3c4c92133532b314da3e350");
    }

    #[test]
    fn multiple_results() {
        let data = DataSource::try_new(&None, &None).unwrap();
        let issue = data.find_issue("1f");
        assert!(issue.is_err());
    }

    #[test]
    fn not_found() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        assert!(data.find_issue(&"1").is_err());
        assert!(data.find_issue(&"12").is_err());
        assert!(data.find_issue(&"123423eaf").is_err());
    }

    #[test]
    fn short_id() {
        let data = DataSource::try_new(&None, &None).unwrap();
        let issue = data.find_issue("2d9deaf").expect("Found issue");
        assert_eq!(issue.0, "2d9deaf1b8b146d7e3c4c92133532b314da3e350");
    }
}

#[cfg(test)]
mod create_repo {
    #[test]
    fn dir_exists() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        assert!(
            std::fs::create_dir(tmp.join(".issues")).is_ok(),
            "Created dir"
        );
        let result = crate::create(tmp, false);
        let msg = format!("{:?}", result);
        assert!(!result.is_ok(), "{}", msg);
    }

    #[test]
    fn create() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        let result = crate::create(tmp, false);
        let msg = format!("{:?}", result);
        assert!(result.is_ok(), "{}", msg);
    }
}
#[cfg(test)]
fn test_source(tmp: &Path) -> DataSource {
    assert!(create(tmp, false).is_ok(), "Create issue repository");
    DataSource::try_from(tmp).unwrap()
}

#[cfg(test)]
mod create_issue {

    #[test]
    fn only_message() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let desc = "Foo Bar";
        let result = data.create_issue(&desc, vec![], None);
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(&issue_id.0).unwrap();
        let actual_desc = data.read(&issue_id, &crate::Property::Description).unwrap();
        assert_eq!(actual_desc, desc);

        let actual_tags = data.tags(&issue_id);
        assert_eq!(actual_tags, vec!["open".to_string()]);

        let actual_milestone = data.milestone(&issue_id);
        assert_eq!(actual_milestone, None);
    }

    #[test]
    fn with_milestone() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let desc = "Foo Bar";
        let result = data.create_issue(&desc, vec![], Some("High Goal".to_string()));
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(&issue_id.0).unwrap();

        let actual_desc = data.read(&issue_id, &crate::Property::Description).unwrap();
        assert_eq!(actual_desc, desc);

        let actual_tags = data.tags(&issue_id);
        assert_eq!(actual_tags, vec!["open".to_string()]);

        let actual_milestone = data.milestone(&issue_id);
        assert_eq!(actual_milestone, Some("High Goal".to_string()));
    }

    #[test]
    fn with_tags() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let desc = "Foo Bar";
        let result = data.create_issue(&desc, vec!["foo".to_string()], None);
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(&issue_id.0).unwrap();

        let actual_desc = data.read(&issue_id, &crate::Property::Description).unwrap();
        assert_eq!(actual_desc, desc);

        let actual_tags = data.tags(&issue_id);
        assert_eq!(actual_tags, vec!["foo".to_string(), "open".to_string()]);

        let actual_milestone = data.milestone(&issue_id);
        assert_eq!(actual_milestone, None);
    }

    #[test]
    fn nl_at_eof() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let desc = "Foo Bar";
        let result = data.create_issue(
            &desc,
            vec!["foo".to_string()],
            Some("World domination!".to_owned()),
        );
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(&issue_id.0).unwrap();
        let issue_dir = issue_id.path(&data.issues_dir);
        {
            let actual = std::fs::read_to_string(issue_dir.join("description")).unwrap();
            let expected = "Foo Bar\n";
            assert_eq!(actual, expected, "Description ends with NL");
        }

        {
            let actual = std::fs::read_to_string(issue_dir.join("tags")).unwrap();
            let expected = "foo\nopen\n";
            assert_eq!(actual, expected, "Tags ends with NL");
        }

        {
            let actual = std::fs::read_to_string(issue_dir.join("milestone")).unwrap();
            let expected = "World domination!\n";
            assert_eq!(actual, expected, "Milestone ends with NL");
        }
    }
}

#[cfg(test)]
mod tags {
    use crate::WriteResult;
    #[test]
    fn add_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let desc = "Foo Bar";
        let issue_id = data.create_issue(&desc, vec![], None).unwrap();
        {
            let actual = data.add_tag(&issue_id, "foo").expect("Added tag foo");
            assert_eq!(actual, WriteResult::Applied, "Changed data");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["foo".to_string(), "open".to_string()];
        assert_eq!(actual_tags, expected_tags);
    }

    #[test]
    fn add_duplicate_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let desc = "Foo Bar";
        let issue_id = data.create_issue(&desc, vec![], None).unwrap();
        {
            let actual = data
                .add_tag(&issue_id, "open")
                .expect("Add tag is succesful");
            assert_eq!(actual, WriteResult::NoChanges, "No changes were applied");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["open".to_string()];
        assert_eq!(actual_tags, expected_tags);
    }

    #[test]
    fn remove_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let desc = "Foo Bar";
        let issue_id = data
            .create_issue(&desc, vec!["foo".to_string()], None)
            .unwrap();
        {
            let actual = data.remove_tag(&issue_id, "foo").expect("Removed tag foo");
            assert_eq!(actual, WriteResult::Applied, "Changed data");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["open".to_string()];
        assert_eq!(actual_tags, expected_tags);
    }

    #[test]
    fn remove_non_existing_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let desc = "Foo Bar";
        let issue_id = data.create_issue(&desc, vec![], None).unwrap();
        {
            let actual = data
                .remove_tag(&issue_id, "foo")
                .expect("Successful remove tag");
            assert_eq!(actual, WriteResult::NoChanges, "No changes were applied");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["open".to_string()];
        assert_eq!(actual_tags, expected_tags);
    }
}
