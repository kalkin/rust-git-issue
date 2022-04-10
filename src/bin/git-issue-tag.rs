#![allow(missing_docs)]
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};

use posix_errors::PosixError;

use git_issue::{DataSource, Id, WriteResult};

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "Add or remove a tag",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(long_help = "Issue id")]
    issue_id: String,
    #[clap(short, long, long_help = "Remove tags from issue")]
    remove: bool,
    #[clap(long_help = "One or multiple tags", required = true)]
    tags: Vec<String>,

    #[clap(long, long_help = "Directory where the GIT_DIR is")]
    git_dir: Option<String>,
    #[clap(long, long_help = "Directory where the GIT_WORK_TREE is")]
    work_tree: Option<String>,

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,
}

fn add_tags<'a>(
    data: &DataSource,
    id: &Id,
    tags: &'a [String],
) -> Result<Vec<&'a str>, PosixError> {
    let short_id = &id.short_id();
    let mut applied: Vec<&'a str> = Vec::with_capacity(tags.len());

    for tag in tags {
        match data.add_tag(id, tag)? {
            WriteResult::Applied => {
                log::info!("Adding tag {} to {}", tag, short_id);
                applied.push(tag);
            }
            WriteResult::NoChanges => {
                log::warn!("Skipping tag {}. {} already tagged with it.", tag, short_id);
            }
        }
    }

    Ok(applied)
}

fn remove_tags<'a>(
    data: &DataSource,
    id: &Id,
    tags: &'a [String],
) -> Result<Vec<&'a str>, PosixError> {
    let short_id = &id.short_id();
    let mut applied: Vec<&'a str> = Vec::with_capacity(tags.len());

    for tag in tags {
        match data.remove_tag(id, tag)? {
            WriteResult::Applied => {
                log::info!("Removing tag {} from {}", tag, short_id);
                applied.push(tag);
            }
            WriteResult::NoChanges => {
                log::warn!("Skipping tag {}. {} not tagged with it.", tag, short_id);
            }
        }
    }

    Ok(applied)
}

fn execute(args: &Args, mut data: DataSource) -> Result<(), PosixError> {
    let id = data.find_issue(&args.issue_id).map_err(PosixError::from)?;
    log::info!("Starting transaction");
    data.start_transaction().map_err(PosixError::from)?;

    let applied_tags_result = if args.remove {
        remove_tags(&data, &id, &args.tags)
    } else {
        add_tags(&data, &id, &args.tags)
    };
    applied_tags_result
        .map_err(PosixError::from)
        .and_then(|applied| {
            if applied.is_empty() {
                log::warn!("Nothing to do");
                log::info!("Rolling back transaction");
                data.rollback_transaction().map_err(PosixError::from)
            } else {
                let word = if applied.len() > 1 { "tags" } else { "tag" };
                let message = if args.remove {
                    format!(
                        "gi({}): Remove {}: {}",
                        &id.short_id(),
                        word,
                        applied.join(", ")
                    )
                } else {
                    format!(
                        "gi({}): Add {}: {}",
                        &id.short_id(),
                        word,
                        applied.join(", ")
                    )
                };

                log::info!("Committing transaction");
                data.finish_transaction(&message).map_err(PosixError::from)
            }
        })
}

#[cfg(not(tarpaulin_include))]
fn main() {
    let args = Args::parse();
    cli_log::init_with_level(args.verbose.log_level_filter());
    log::debug!("Log Level is set to {}", log::max_level());
    let data = match DataSource::try_new(&args.git_dir, &args.work_tree) {
        Err(e) => {
            let err: PosixError = e.into();
            log::error!(" error: {}", err);
            std::process::exit(err.code());
        }
        Ok(repo) => repo,
    };

    if let Err(e) = execute(&args, data) {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
}

#[cfg(test)]
mod cmd_tag {
    use clap::Parser;

    use std::path::Path;

    use git_issue::{DataSource, Id};

    fn prepare(tmp_dir: &Path, tags: &[String]) -> Id {
        git_issue::create(tmp_dir, false).unwrap();
        let issues_dir = tmp_dir.join(".issues");
        let data = DataSource::try_from(issues_dir.as_path()).unwrap();
        let result = data.create_issue("Foo Bar", tags.to_vec(), None);
        result.expect("Created new issue")
    }

    #[test]
    fn add_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(tmp, &[]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let args = Parser::try_parse_from(&["git-issue-tag", id.id(), "foo"])
                .expect("Parsed arguments");
            assert!(crate::execute(&args, data).is_ok());
        }
        let data = DataSource::try_from(tmp).unwrap();
        {
            let tags = data.tags(&id);
            assert_eq!(
                tags,
                ["foo".to_string(), "open".to_string()],
                "Tags foo and open"
            );
        }
    }

    #[test]
    fn add_duplicate_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(tmp, &["foo".to_string()]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let args = Parser::try_parse_from(&["git-issue-tag", id.id(), "foo"])
                .expect("Parsed arguments");
            assert!(crate::execute(&args, data).is_ok());
        }
        let data = DataSource::try_from(tmp).unwrap();
        {
            let tags = data.tags(&id);
            assert_eq!(
                tags,
                ["foo".to_string(), "open".to_string()],
                "Still only tags foo and open"
            );
        }
    }

    #[test]
    fn remove_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(tmp, &["foo".to_string()]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let args = Parser::try_parse_from(&["git-issue-tag", id.id(), "-r", "foo"])
                .expect("Parsed arguments");
            assert!(crate::execute(&args, data).is_ok());
        }
        let data = DataSource::try_from(tmp).unwrap();
        {
            let tags = data.tags(&id);
            assert_eq!(tags, ["open".to_string()], "Only tag open");
        }
    }

    #[test]
    fn remove_non_existing_tag() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(tmp, &[]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let args = Parser::try_parse_from(&["git-issue-tag", id.id(), "-r", "foo"])
                .expect("Parsed arguments");
            assert!(crate::execute(&args, data).is_ok());
        }
        let data = DataSource::try_from(tmp).unwrap();
        {
            let tags = data.tags(&id);
            assert_eq!(tags, ["open".to_string()], "Only tag open");
        }
    }
}

#[cfg(test)]
mod parse_args {
    use crate::Args;
    use clap::Parser;

    #[test]
    fn no_arguments() {
        let result: Result<Args, _> = Parser::try_parse_from(&["git-issue-tag"]);
        assert!(result.is_err(), "git-issue-tag expects two arguments");
    }

    #[test]
    fn no_tag_argument() {
        let result: Result<Args, _> = Parser::try_parse_from(&["git-issue-tag", "1234"]);
        assert!(result.is_err(), "git-issue-tag expects two arguments");
    }

    #[test]
    fn one_tag() {
        let _args: Args =
            Parser::try_parse_from(&["git-issue-tag", "1234", "foo"]).expect("Parse one tag");
    }

    #[test]
    fn multiple_tags() {
        let _args: Args = Parser::try_parse_from(&["git-issue-tag", "1234", "foo", "bar"])
            .expect("Parse multiple tags");
    }
}
