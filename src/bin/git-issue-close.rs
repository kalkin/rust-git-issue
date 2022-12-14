#![allow(missing_docs)]
use clap::Parser;
use clap_git_options::GitOptions;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use posix_errors::PosixError;

use git_issue::{DataSource, FindError, Id, WriteResult};

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "Remove open tag, add closed tag",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(long_help = "Issue id", required = true)]
    issue_ids: Vec<String>,

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,

    #[clap(flatten)]
    git: GitOptions,
}

fn close_issues(data: &DataSource, ids: &[Id]) -> Result<WriteResult, PosixError> {
    let mut results: Vec<WriteResult> = Vec::with_capacity(ids.len());
    for id in ids {
        let r = data.close_issue(id)?;
        match r {
            WriteResult::Applied => log::warn!(
                "Closed issue {}: {}",
                &id.short_id(),
                data.title(id).expect("Has a description")
            ),
            WriteResult::NoChanges => {
                log::warn!("Skipping issue {}. It is already closed", &id.short_id());
            }
        }

        results.push(r);
    }
    Ok(WriteResult::from(results))
}

fn execute(args: &Args, mut data: DataSource) -> Result<(), PosixError> {
    let issue_ids: Vec<Id> = args
        .issue_ids
        .iter()
        .map(|id| data.find_issue(id))
        .collect::<Result<Vec<Id>, FindError>>()
        .map_err(PosixError::from)?;

    log::info!("Starting transaction");
    data.start_transaction().map_err(PosixError::from)?;

    match close_issues(&data, &issue_ids) {
        Ok(WriteResult::Applied) => {
            let msg = if issue_ids.len() == 1 {
                format!(
                    "DONE({}): {}",
                    issue_ids[0].short_id(),
                    data.title(&issue_ids[0])?,
                )
            } else {
                let text = issue_ids
                    .iter()
                    .map(git_issue::Id::short_id)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("gi: Closed {}", text)
            };
            log::info!("Committing transaction");
            data.finish_transaction(&msg).map_err(PosixError::from)
        }
        Ok(WriteResult::NoChanges) => {
            log::warn!("Nothing to do");
            log::info!("Rolling back transaction");
            data.rollback_transaction().map_err(PosixError::from)
        }
        Err(e) => {
            log::warn!("An error happend. Rolling back transaction.");
            data.rollback_transaction()?;
            Err(e)
        }
    }
}

#[allow(clippy::exit)]
#[cfg(not(tarpaulin_include))]
fn main() {
    let args = Args::parse();
    cli_log::init_with_level(args.verbose.log_level_filter());
    let data = match git_issue::DataSource::try_new(&args.git) {
        Err(e) => {
            log::error!(" error: {}", e);
            std::process::exit(128);
        }
        Ok(d) => d,
    };

    if let Err(e) = execute(&args, data) {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
}

#[cfg(test)]
mod cmd_close {
    use clap::Parser;
    use git_issue::{DataSource, Id};
    use std::path::Path;

    fn prepare(tmp_dir: &Path, tags: &[String]) -> Id {
        git_wrapper::setup_test_author();
        git_issue::create(tmp_dir, false).unwrap();
        let issues_dir = tmp_dir.join(".issues");
        let data = DataSource::try_from(issues_dir.as_path()).unwrap();
        let result = data.create_issue("Foo Bar", tags.to_vec(), None);
        result.expect("Created new issue")
    }

    #[test]
    fn single_issue() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(tmp, &[]);

        {
            let data = DataSource::try_from(tmp).unwrap();
            let args =
                Parser::try_parse_from(&["git-issue-close", id.id()]).expect("Parsed arguments");
            crate::execute(&args, data).unwrap();
        }

        let data = DataSource::try_from(tmp).unwrap();
        {
            let tags = data.tags(&id);
            assert_eq!(tags, ["closed".to_owned()], "Only tag closed");
        }
    }

    #[test]
    fn multiple_issue() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(tmp, &[]);
        let id2 = {
            let issues_dir = tmp.join(".issues");
            let data = DataSource::try_from(issues_dir.as_path()).unwrap();
            let result = data.create_issue("Foo Bar 2", vec![], None);
            result.expect("Created new issue")
        };

        {
            let data = DataSource::try_from(tmp).unwrap();
            let args = Parser::try_parse_from(&["git-issue-close", id.id(), id2.id()])
                .expect("Parsed arguments");
            crate::execute(&args, data).unwrap();
        }

        let data = DataSource::try_from(tmp).unwrap();
        {
            let tags = data.tags(&id);
            assert_eq!(tags, ["closed".to_owned()], "Only tag closed");
        }

        {
            let tags = data.tags(&id2);
            assert_eq!(tags, ["closed".to_owned()], "Only tag closed");
        }
    }

    #[test]
    fn non_existing_issue() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        git_wrapper::setup_test_author();
        git_issue::create(tmp, false).unwrap();

        let data = DataSource::try_from(tmp).unwrap();
        let args =
            Parser::try_parse_from(&["git-issue-close", "123eaf"]).expect("Parsed arguments");
        crate::execute(&args, data).unwrap_err();
    }
}

#[cfg(test)]
mod parse_args {
    use crate::Args;
    use clap::Parser;

    #[test]
    fn no_arguments() {
        let result: Result<Args, _> = Parser::try_parse_from(&["git-issue-close"]);
        assert!(
            result.is_err(),
            "git-issue-close expects at least one arguments"
        );
    }

    #[test]
    fn one_issue() {
        let _args: Args =
            Parser::try_parse_from(&["git-issue-close", "1234"]).expect("Parse one issue");
    }

    #[test]
    fn multiple_issues() {
        let _args: Args = Parser::try_parse_from(&["git-issue-close", "1234", "abcdf"])
            .expect("Parse multiple issues");
    }
}
