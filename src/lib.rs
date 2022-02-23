use std::path::{Path, PathBuf};

use git_wrapper::x;
use git_wrapper::{CommitError, Repository, StagingError};
use posix_errors::PosixError;

pub struct Transaction {
    start_sha: String,
    stash_before: bool,
}

#[derive(Debug, Clone)]
pub struct Id(pub String);

impl Id {
    #[must_use]
    fn path(&self, path: &Path) -> PathBuf {
        path.join("issues").join(&self.0[..2]).join(&self.0[2..])
    }
}

pub enum Property {
    Description(String),
    Tags(Vec<String>),
    Milestone(String),
}

impl Property {
    #[must_use]
    #[inline]
    pub fn filename(&self) -> String {
        match self {
            Self::Description(_) => "description",
            Self::Tags(_) => "tags",
            Self::Milestone(_) => "milestone",
        }
        .to_owned()
    }
}
enum ChangeAction {
    New,
    Edit,
}

pub enum Action {
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

pub struct DataSource {
    pub repo: Repository,
    pub issues_dir: PathBuf,
    pub transaction: Option<Transaction>,
}

impl DataSource {
    /// # Errors
    ///
    /// Will throw an error when:
    /// - Fails to find a non-bare git repository
    /// - Fails to resolve HEAD ref
    #[inline]
    pub fn try_new(
        git_dir: &Option<String>,
        work_tree: &Option<String>,
    ) -> Result<Self, PosixError> {
        let issues_dir = Self::find_issues_dir();
        let repo = match Repository::from_args(
            Some(&issues_dir.to_string_lossy()),
            git_dir.as_deref(),
            work_tree.as_deref(),
        ) {
            Ok(repo) => Ok(repo),
            Err(e) => Err(PosixError::new(4, format!("{}", e))),
        }?;
        Ok(Self {
            repo,
            issues_dir,
            transaction: None,
        })
    }

    fn find_issues_dir() -> PathBuf {
        let mut cur = std::env::current_dir().expect("Failed to get CWD");
        loop {
            let needle = cur.join(".issues");
            if needle.exists() {
                return needle;
            }
            cur = cur
                .parent()
                .expect("Failed to find any .issue dirs")
                .to_path_buf();
        }
    }

    /// Start transaction
    ///
    /// # Errors
    ///
    /// Will fail when `HEAD` can not be resolved
    #[inline]
    pub fn start_transaction(&mut self) -> Result<(), PosixError> {
        self.transaction = Some(start_transaction(&self.repo)?);
        Ok(())
    }

    /// Rolls back any commits made during the transaction and restores stashed changes if any.
    ///
    /// # Errors
    ///
    /// Throws an error when `git reset --hard` or poping stashed changes fails.
    #[inline]
    pub fn rollback_transaction(mut self) -> Result<(), PosixError> {
        rollback_transaction(&self.transaction.expect("Foo"), &self.repo)?;
        self.transaction = None;
        Ok(())
    }

    /// # Errors
    ///
    /// Will throw error on failure to read from file
    #[inline]
    pub fn read(&self, id: &Id, prop: &Property) -> Result<String, PosixError> {
        let path = id.path(&self.issues_dir).join(prop.filename());
        Ok(std::fs::read_to_string(path)?)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn new_description(&self, id: &Id, text: &str) -> Result<(), PosixError> {
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
            self.write(id, &description)
        }
        #[cfg(not(feature = "strict-compatibility"))]
        {
            self.write(id, &description)?;
            self.write(id, &tag)
        }
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn edit_description(&self, id: &Id, text: &str) -> Result<(), PosixError> {
        let property = CommitProperty::Description {
            action: ChangeAction::Edit,
            id: id.0.clone(),
            description: text.to_owned(),
        };
        self.write(id, &property)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn add_tag(&self, id: &Id, tag: &str) -> Result<(), PosixError> {
        let property = CommitProperty::Tag {
            action: Action::Add,
            tag: tag.to_owned(),
        };
        self.write(id, &property)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn remove_tag(&self, id: &Id, tag: &str) -> Result<(), PosixError> {
        let property = CommitProperty::Tag {
            action: Action::Remove,
            tag: tag.to_owned(),
        };
        self.write(id, &property)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn add_milestone(&self, id: &Id, milestone: &str) -> Result<(), PosixError> {
        let property = CommitProperty::Milestone {
            action: Action::Add,
            milestone: milestone.to_owned(),
        };
        self.write(id, &property)
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO
    #[inline]
    pub fn remove_milestone(&self, id: &Id, milestone: &str) -> Result<(), PosixError> {
        let property = CommitProperty::Milestone {
            action: Action::Add,
            milestone: milestone.to_owned(),
        };
        self.write(id, &property)
    }

    fn write_to_file(&self, id: &Id, property: &CommitProperty) -> Result<(), PosixError> {
        let dir_path = id.path(&self.issues_dir);
        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path)?;
        }
        let path = &dir_path.join(property.filename());

        // Execute write
        log::debug!("Writing {:?}", path);
        match property {
            CommitProperty::Description { description, .. } => {
                std::fs::write(path, description)?;
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
        match self.repo.stage(path) {
            Ok(_) => Ok(()),
            Err(StagingError::BareRepository) => {
                Err(PosixError::new(128, "Bare repository".to_owned()))
            }
            Err(StagingError::FileDoesNotExist(p)) => Err(PosixError::new(
                128,
                format!("File does not exists: {:?}", &p),
            )),
            Err(StagingError::Failure(msg, code)) => Err(PosixError::new(code, msg)),
        }
    }

    /// # Errors
    ///
    /// Will throw error on failure to do IO or commiting
    fn write(&self, target_id: &Id, property: &CommitProperty) -> Result<(), PosixError> {
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

        match self.repo.commit(&message) {
            Ok(_) => Ok(()),
            Err(CommitError::Failure(msg, code)) => Err(PosixError::new(code, msg)),
            Err(CommitError::BareRepository) => {
                Err(PosixError::new(128, "Bare repository".to_owned()))
            }
        }
    }

    /// # Errors
    ///
    /// Will throw error on failure to commit
    #[inline]
    pub fn finish_transaction(&mut self, message: &str) -> Result<(), PosixError> {
        let transaction = &self.transaction.as_ref().expect("A started transaction");
        log::info!("Merging issue changes as not fast forward branch");
        #[cfg(not(feature = "strict-compatibility"))]
        {
            let sha = self
                .repo
                .head()
                .ok_or_else(|| PosixError::new(2, "Failed to resolve HEAD".to_owned()))?;

            let start_sha = &transaction.start_sha;
            x::reset_hard(&self.repo, start_sha)?;
            let mut cmd = self.repo.git();
            let out = cmd
                .args(&["merge", "--no-ff", "-m", message, &sha])
                .output()
                .expect("Failed to execute git-stash(1)");

            if !out.status.success() {
                let output = String::from_utf8_lossy(&out.stderr).to_string();
                let code = out.status.code().unwrap_or(1);
                return Err(PosixError::new(code, output));
            }
        }
        if transaction.stash_before {
            stash_pop(&self.repo)?;
        }
        self.transaction = None;
        Ok(())
    }
}

/// # Errors
///
/// Will fail when `HEAD` can not be resolved
fn start_transaction(repo: &Repository) -> Result<Transaction, PosixError> {
    let start_sha = repo
        .head()
        .ok_or_else(|| PosixError::new(2, "Failed to resolve HEAD".to_owned()))?;

    let stash_before = !repo.is_clean();
    let result = Transaction {
        start_sha,
        stash_before,
    };
    if stash_before {
        log::debug!("Stashing repository changes");
        let mut cmd = repo.git();
        cmd.arg("stash");
        if log::max_level() != log::Level::Trace {
            cmd.arg("--quiet");
        }
        cmd.args(&["--include-untracked", "-m", "git-issue: Start Transaction"]);
        let out = cmd.output().expect("Failed to execute git-stash(1)");
        print!("{}", String::from_utf8_lossy(&out.stdout));
        if !out.status.success() {
            let message = String::from_utf8_lossy(&out.stderr).to_string();
            let code = out.status.code().unwrap_or(1);
            return Err(PosixError::new(code, message));
        }
    }
    Ok(result)
}

fn stash_pop(repo: &Repository) -> Result<(), PosixError> {
    let mut cmd = repo.git();
    log::debug!("Popping stashed repository changes");
    let out = cmd
        .args(&["stash", "pop", "--quiet"])
        .output()
        .expect("Failed to execute git-stash(1)");

    if !out.status.success() {
        let message = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(1);
        return Err(PosixError::new(code, message));
    }
    Ok(())
}

/// # Errors
///
/// Throws an error when any of the git commands fail
fn rollback_transaction(transaction: &Transaction, repo: &Repository) -> Result<(), PosixError> {
    x::reset_hard(repo, &transaction.start_sha)?;
    if transaction.stash_before {
        stash_pop(repo)?;
    }

    Ok(())
}

/// # Errors
///
/// Throws an error when it fails to commit
#[inline]
pub fn commit(repo: &Repository, subject: &str, body: &str) -> Result<(), PosixError> {
    let message = format!("{}\n\n{}", subject, body);
    let mut cmd = repo.git();
    let out = cmd
        .args(&[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-q",
            "-m",
            &message,
        ])
        .output()
        .expect("Failed to execute git-commit(1)");
    if !out.status.success() {
        let output = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(1);
        return Err(PosixError::new(code, output));
    }
    Ok(())
}

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
            129,
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

#[cfg(test)]
#[cfg(not(tarpaulin_include))]
macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);

        // Find and cut the rest of the path
        match &name[..name.len() - 3].rfind(':') {
            Some(pos) => &name[pos + 1..name.len() - 3],
            None => &name[..name.len() - 3],
        }
    }};
}

#[cfg(test)]
fn test_source(tmp: &Path) -> DataSource {
    assert!(create(tmp, false).is_ok(), "Create issue repository");
    let issues_dir = tmp.join(".issues");
    let repo = Repository::from_args(Some(issues_dir.to_str().unwrap()), None, None).unwrap();
    DataSource::new(issues_dir.to_path_buf(), repo)
}
