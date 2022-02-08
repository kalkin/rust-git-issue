use std::path::PathBuf;

use getset::Getters;
use git_wrapper::Repository;

#[derive(Getters)]
pub struct Transaction {
    #[getset(get = "pub")]
    start_sha: String,
    #[getset(get = "pub")]
    stash_before: bool,
}

#[derive(Getters)]
pub struct Error {
    #[getset(get = "pub")]
    message: String,
    #[getset(get = "pub")]
    code: i32,
}

#[derive(Debug, Clone)]
pub struct Id(pub String);

impl Id {
    #[must_use]
    pub fn path(&self, repo: &Repository) -> PathBuf {
        let mut path_buf = repo.work_tree().expect("Non bare repository");
        path_buf = path_buf.join(".issues");
        path_buf = path_buf.join("issues");
        path_buf = path_buf.join(&self.0[..2]);
        path_buf = path_buf.join(&self.0[2..]);
        path_buf
    }

    #[must_use]
    pub fn short(&self) -> &str {
        &self.0[..8]
    }
}

#[derive(Debug)]
pub struct Issue {
    pub id: Id,
    pub description: String,
    pub tags: Vec<String>,
}

/// # Errors
///
/// Will fail when `HEAD` can not be resolved
pub fn start_transaction(repo: &Repository) -> Result<Transaction, Error> {
    let start_sha = repo.head().ok_or_else(|| Error {
        message: "Failed to resolve HEAD".to_string(),
        code: 2,
    })?;

    let stash_before = !repo.is_clean();
    log::debug!("Stashing needed? {}", stash_before);
    let result = Transaction {
        start_sha,
        stash_before,
    };
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
        return Err(Error { message, code });
    }
    Ok(result)
}

fn reset_hard(repo: &Repository, sha: &str) -> Result<(), Error> {
    log::debug!("Resetting to {}", sha);
    let mut cmd = repo.git();
    let out = cmd
        .args(&["reset", "--hard", "--quiet", sha])
        .output()
        .expect("Failed to execute git-reset(1)");

    if !out.status.success() {
        let message = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(1);
        return Err(Error { message, code });
    }
    Ok(())
}

fn stash_pop(repo: &Repository) -> Result<(), Error> {
    let mut cmd = repo.git();
    log::debug!("Popping stash");
    let out = cmd
        .args(&["stash", "pop", "--quiet"])
        .output()
        .expect("Failed to execute git-stash(1)");

    if !out.status.success() {
        let message = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(1);
        return Err(Error { message, code });
    }
    Ok(())
}

/// # Errors
///
/// Throws an error when any of the git commands fail
pub fn rollback_transaction(transaction: &Transaction, repo: &Repository) -> Result<(), Error> {
    reset_hard(repo, &transaction.start_sha)?;
    if transaction.stash_before {
        stash_pop(repo)?;
    }

    Ok(())
}

/// # Errors
///
/// Throws an error when any of the git commands fail
pub fn commit_transaction(
    transaction: &Transaction,
    repo: &Repository,
    message: &str,
) -> Result<(), Error> {
    let sha = repo.head().ok_or_else(|| Error {
        message: "Failed to resolve HEAD".to_string(),
        code: 2,
    })?;

    reset_hard(repo, &transaction.start_sha)?;
    let mut cmd = repo.git();
    let out = cmd
        .args(&["merge", "--no-ff", "-m", message, &sha])
        .output()
        .expect("Failed to execute git-stash(1)");

    if !out.status.success() {
        let message = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(1);
        return Err(Error { message, code });
    }

    if transaction.stash_before {
        stash_pop(repo)?;
    }
    Ok(())
}

/// # Errors
///
/// Throws an error when it fails to commit
pub fn commit(repo: &Repository, subject: &str, message: &str) -> Result<(), Error> {
    let mut cmd = repo.git();
    let message = format!("{}\n\n{}", subject, message);
    let out = cmd
        .args(&[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-q",
            "-m",
            message.as_str(),
        ])
        .output()
        .expect("Failed to execute git-commit(1)");
    if !out.status.success() {
        let message = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(1);
        return Err(Error { message, code });
    }
    Ok(())
}

/// # Errors
///
/// Throws an error when it fails to create an issue
pub fn create_issue(issue: &Issue, repo: &Repository) -> Result<(), Error> {
    let dir_path = issue.id.path(repo);
    let description_path = dir_path.join("description");
    let tags_path = dir_path.join("tags");
    let tags = format!("{}\n", &issue.tags.join("\n"));

    std::fs::create_dir_all(dir_path)
        .and_then(|_| std::fs::write(description_path, &issue.description))
        .and_then(|_| std::fs::write(tags_path, tags))
        .map_err(|e| Error {
            message: format!("{}", e),
            code: 4,
        })?;
    let mut cmd = repo.git();
    let out = cmd
        .args(&["add", &issue.id.path(repo).to_string_lossy()])
        .output()
        .expect("Failed to execute git-add(1)");
    print!("{}", String::from_utf8_lossy(&out.stdout));
    if !out.status.success() {
        let message = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(1);
        return Err(Error { message, code });
    }

    Ok(())
}
