//! Library for manipulating a data in a git-issue(1) tracker

use std::path::Path;

use git_wrapper::{CommitError, Repository};
use posix_errors::PosixError;

mod caching;
mod errors;
mod id;
mod issues;
mod source;
pub use crate::caching::CacheError;
pub use crate::errors::*;
pub use crate::id::Id;
pub use crate::issues::{FormatString, Issue};
pub use crate::source::{DataSource, WriteResult};

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

/// Vector of Strings containing tags
pub type Tags = Vec<String>;
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
        assert_eq!(issue.id(), "2d9deaf1b8b146d7e3c4c92133532b314da3e350");
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
        let issue_id = data.create_issue("Foo Bar", vec![], None).unwrap();
        let needle = issue_id.id().chars().next().unwrap().to_string();
        let issue = data.find_issue(&needle).expect("Found issue");
        assert_eq!(issue_id, issue);
    }

    #[test]
    fn by_two_chars() {
        let data = DataSource::try_new(&None, &None).unwrap();
        let issue = data.find_issue("2d").expect("Found issue");
        assert_eq!(issue.id(), "2d9deaf1b8b146d7e3c4c92133532b314da3e350");
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
        assert!(data.find_issue("1").is_err());
        assert!(data.find_issue("12").is_err());
        assert!(data.find_issue("123423eaf").is_err());
    }

    #[test]
    fn short_id() {
        let data = DataSource::try_new(&None, &None).unwrap();
        let issue = data.find_issue("2d9deaf").expect("Found issue");
        assert_eq!(issue.id(), "2d9deaf1b8b146d7e3c4c92133532b314da3e350");
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
        assert!(result.is_err(), "{}", msg);
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
        let result = data.create_issue(desc, vec![], None);
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(issue_id.id()).unwrap();
        let actual_desc = data
            .read(&issue_id, &crate::source::Property::Description)
            .unwrap();
        assert_eq!(actual_desc, desc);

        let actual_tags = data.tags(&issue_id);
        assert_eq!(actual_tags, vec!["open".to_owned()]);

        let actual_milestone = data.milestone(&issue_id);
        assert_eq!(actual_milestone, None);
    }

    #[test]
    fn with_milestone() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let desc = "Foo Bar";
        let result = data.create_issue(desc, vec![], Some("High Goal".to_owned()));
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(issue_id.id()).unwrap();

        let actual_desc = data
            .read(&issue_id, &crate::source::Property::Description)
            .unwrap();
        assert_eq!(actual_desc, desc);

        let actual_tags = data.tags(&issue_id);
        assert_eq!(actual_tags, vec!["open".to_owned()]);

        let actual_milestone = data.milestone(&issue_id);
        assert_eq!(actual_milestone, Some("High Goal".to_owned()));
    }

    #[test]
    fn with_tags() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let desc = "Foo Bar";
        let result = data.create_issue(desc, vec!["foo".to_owned()], None);
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(issue_id.id()).unwrap();

        let actual_desc = data
            .read(&issue_id, &crate::source::Property::Description)
            .unwrap();
        assert_eq!(actual_desc, desc);

        let actual_tags = data.tags(&issue_id);
        assert_eq!(actual_tags, vec!["foo".to_owned(), "open".to_owned()]);

        let actual_milestone = data.milestone(&issue_id);
        assert_eq!(actual_milestone, None);
    }

    #[test]
    fn nl_at_eof() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());
        let desc = "Foo Bar";
        let result = data.create_issue(
            desc,
            vec!["foo".to_owned()],
            Some("World domination!".to_owned()),
        );
        assert!(result.is_ok());
        let issue_id = result.unwrap();
        data.find_issue(issue_id.id()).unwrap();
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
        let issue_id = data.create_issue(desc, vec![], None).unwrap();
        {
            let actual = data.add_tag(&issue_id, "foo").expect("Added tag foo");
            assert_eq!(actual, WriteResult::Applied, "Changed data");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["foo".to_owned(), "open".to_owned()];
        assert_eq!(actual_tags, expected_tags);
    }

    #[test]
    fn add_duplicate_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let desc = "Foo Bar";
        let issue_id = data.create_issue(desc, vec![], None).unwrap();
        {
            let actual = data
                .add_tag(&issue_id, "open")
                .expect("Add tag is succesful");
            assert_eq!(actual, WriteResult::NoChanges, "No changes were applied");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["open".to_owned()];
        assert_eq!(actual_tags, expected_tags);
    }

    #[test]
    fn remove_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let desc = "Foo Bar";
        let issue_id = data
            .create_issue(desc, vec!["foo".to_owned()], None)
            .unwrap();
        {
            let actual = data.remove_tag(&issue_id, "foo").expect("Removed tag foo");
            assert_eq!(actual, WriteResult::Applied, "Changed data");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["open".to_owned()];
        assert_eq!(actual_tags, expected_tags);
    }

    #[test]
    fn remove_non_existing_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let desc = "Foo Bar";
        let issue_id = data.create_issue(desc, vec![], None).unwrap();
        {
            let actual = data
                .remove_tag(&issue_id, "foo")
                .expect("Successful remove tag");
            assert_eq!(actual, WriteResult::NoChanges, "No changes were applied");
        }

        let actual_tags = data.tags(&issue_id);
        let expected_tags = vec!["open".to_owned()];
        assert_eq!(actual_tags, expected_tags);
    }
}

#[cfg(test)]
mod milestone {
    use crate::WriteResult;
    #[test]
    fn no_milestone() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let expected = None;
        let issue_id = data
            .create_issue("Foo Bar", vec![], expected.clone())
            .unwrap();

        let actual = data.milestone(&issue_id);
        assert_eq!(actual, expected);
    }

    #[test]
    fn add_milestone_on_creation() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let expected = Some("World Domination".to_owned());
        let issue_id = data
            .create_issue("Foo Bar", vec![], expected.clone())
            .expect("Created an issue");

        {
            let actual = data.milestone(&issue_id);
            assert_eq!(actual, expected);
        }

        {
            let actual = data
                .add_milestone(&issue_id, "World Domination")
                .expect("Add milestone");
            assert_eq!(actual, WriteResult::NoChanges, "No changes were applied");
        }
    }

    #[test]
    fn add_milestone() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let expected = "World Domination";
        let issue_id = data
            .create_issue("Foo Bar", vec![], None)
            .expect("Created an issue");

        assert_eq!(data.milestone(&issue_id), None, "Has no milestone");

        {
            let actual = data
                .add_milestone(&issue_id, expected)
                .expect("Add milestone");
            assert_eq!(actual, WriteResult::Applied, "Changed data");
        }
        {
            let actual = data.milestone(&issue_id);
            assert_eq!(
                actual,
                Some(expected.to_owned()),
                "Milestone “{}” is set",
                expected
            );
        }
    }

    #[test]
    fn remove_milestone() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let expected = Some("World Domination".to_owned());
        let issue_id = data
            .create_issue("Foo Bar", vec![], expected.clone())
            .unwrap();

        {
            let actual = data.milestone(&issue_id);
            assert_eq!(actual, expected, "Has a milestone");
        }

        {
            let actual = data
                .remove_milestone(&issue_id)
                .expect("Successful removal of milestone");
            assert_eq!(actual, WriteResult::Applied, "Changed data");
        }
        let actual = data.milestone(&issue_id);
        assert_eq!(actual, None, "Has no milestone");
    }

    #[test]
    fn remove_no_milestone() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let data = crate::test_source(tmp_dir.path());

        let expected = None;
        let issue_id = data
            .create_issue("Foo Bar", vec![], expected.clone())
            .unwrap();
        {
            let actual = data.milestone(&issue_id);
            assert_eq!(actual, expected, "No milestone");
        }

        {
            let actual = data
                .remove_milestone(&issue_id)
                .expect("Successful removal of milestone");
            assert_eq!(actual, WriteResult::NoChanges, "No changes were applied");
        }

        {
            let actual = data.milestone(&issue_id);
            assert_eq!(actual, None, "Has still no milestone");
        }
    }
}
