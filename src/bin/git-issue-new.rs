#![allow(missing_docs)]
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};

use posix_errors::PosixError;

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "Create new issue",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(short, name = "TAG", long = "tag", long_help = "Tags to assign")]
    tags: Option<Vec<String>>,
    #[clap(
        short,
        name = "MILESTONE",
        long = "milestone",
        long_help = "Milestone to assign to"
    )]
    milestone: Option<String>,
    #[clap(short, long_help = "Issue summary")]
    summary: Option<String>,

    #[clap(short, long, long_help = "Edit the issue")]
    edit: bool,

    #[clap(long, long_help = "Directory where the GIT_DIR is")]
    git_dir: Option<String>,
    #[clap(long, long_help = "Directory where the GIT_WORK_TREE is")]
    work_tree: Option<String>,

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,
}

fn execute(
    args: &Args,
    mut data: git_issue::DataSource,
) -> Result<(git_issue::Id, String), PosixError> {
    let empty: Vec<String> = vec![];
    let tags = args.tags.as_ref().unwrap_or(&empty).clone();
    let milestone = args.milestone.clone();
    let description = if args.edit || args.summary.is_none() {
        let template = format!(
            "{}\n\n{}",
            args.summary.as_deref().unwrap_or_default(),
            &git_issue::read_template(&data.repo, "description").unwrap_or_default()
        );
        git_issue::edit(&data.repo, &template)?
    } else {
        args.summary.as_ref().expect("Summary is provided").clone()
    };

    data.start_transaction()?;
    match data.create_issue(&description, tags, milestone) {
        Ok(id) => {
            let title = description
                .lines()
                .next()
                .expect("Expected at least one line");
            let message = format!("gi({}): {}", &id.short_id(), &title);
            #[cfg(not(feature = "strict-compatibility"))]
            log::info!("Merging issue creation as not fast forward branch");
            data.finish_transaction(&message)?;
            Ok((id, title.to_owned()))
        }
        Err(e) => {
            log::error!("{}", e);
            log::warn!("Rolling back transaction");
            data.rollback_transaction()?;
            Err(e.into())
        }
    }
}

#[cfg(not(tarpaulin_include))]
fn main() {
    let args = Args::parse();
    cli_log::init_with_level(args.verbose.log_level_filter());
    let data = match git_issue::DataSource::try_new(&args.git_dir, &args.work_tree) {
        Err(e) => {
            log::error!(" error: {}", e);
            std::process::exit(128);
        }
        Ok(repo) => repo,
    };
    match execute(&args, data) {
        Ok((id, title)) => log::warn!("Added issue {}: {}", &id.short_id(), title),
        Err(e) => std::process::exit(e.code()),
    }
}

#[cfg(test)]
mod cmd_new {
    use clap::Parser;
    use git_issue::{DataSource, Id};
    use std::path::Path;

    pub const SUMMARY: &str = "New Issue";

    fn execute_new(args: &crate::Args, tmp: &Path) -> Id {
        let data = DataSource::try_from(tmp).unwrap();
        let result = crate::execute(args, data);
        result.expect("Execution successful").0
    }

    #[test]
    fn only_message() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        git_issue::create(tmp, false).unwrap();

        let id = {
            let args =
                Parser::try_parse_from(&["git-issue-new", "-s", SUMMARY]).expect("Parsed args");
            execute_new(&args, tmp)
        };

        let data = DataSource::try_from(tmp).unwrap();
        {
            let actual = data.title(&id).unwrap();
            let expected = SUMMARY.to_owned();
            assert_eq!(actual, expected);
        }
        {
            let actual = data.tags(&id);
            let expected = vec!["open".to_string()];
            assert_eq!(actual, expected);
        }
        {
            let actual = data.milestone(&id);
            assert!(actual.is_none(), "Expected no milestone");
        }
    }

    #[test]
    fn with_tags() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        git_issue::create(tmp, false).unwrap();

        let id = {
            let args = Parser::try_parse_from(&[
                "git-issue-new",
                "-s",
                SUMMARY,
                "-t",
                "foo",
                "--tag",
                "bar",
            ])
            .expect("Parsed args");
            execute_new(&args, tmp)
        };

        let data = DataSource::try_from(tmp).unwrap();
        {
            let actual = data.title(&id).unwrap();
            let expected = SUMMARY.to_owned();
            assert_eq!(actual, expected);
        }
        {
            let actual = data.tags(&id);
            let expected = vec!["bar".to_string(), "foo".to_string(), "open".to_string()];
            assert_eq!(actual, expected);
        }
        {
            let actual = data.milestone(&id);
            assert!(actual.is_none(), "Expected no milestone");
        }
    }

    #[test]
    fn with_milestone() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let tmp = tmp_dir.path();
        git_issue::create(tmp, false).unwrap();
        let milestone = "World Domination!";

        let id = {
            let args = Parser::try_parse_from(&["git-issue-new", "-s", SUMMARY, "-m", milestone])
                .expect("Parsed args");
            execute_new(&args, tmp)
        };
        let data = DataSource::try_from(tmp).unwrap();
        {
            let actual = data.title(&id).unwrap();
            let expected = SUMMARY.to_owned();
            assert_eq!(actual, expected);
        }
        {
            let actual = data.tags(&id);
            let expected = vec!["open".to_string()];
            assert_eq!(actual, expected);
        }
        {
            let actual = data.milestone(&id).unwrap();
            let expected = milestone.to_string();
            assert_eq!(actual, expected);
        }
    }
}

#[cfg(test)]
mod parse_args {
    use crate::Args;
    use clap::Parser;

    #[test]
    fn no_arguments() {
        let _args: Args = Parser::try_parse_from(&["git-issue-new"])
            .expect("If no summary provided, the editor is opened");
    }

    #[test]
    fn with_summary() {
        let _args: Args = Parser::try_parse_from(&["git-issue-new", "-s", crate::cmd_new::SUMMARY])
            .expect("Only summary");
        let _args: Args =
            Parser::try_parse_from(&["git-issue-new", "-s", crate::cmd_new::SUMMARY, "-t", "foo"])
                .expect("Summary + tag");
        let _args: Args =
            Parser::try_parse_from(&["git-issue-new", "-s", crate::cmd_new::SUMMARY, "-m", "foo"])
                .expect("Summary + milestone");
        let _args: Args = Parser::try_parse_from(&[
            "git-issue-new",
            "-s",
            crate::cmd_new::SUMMARY,
            "-t",
            "bar",
            "-m",
            "foo",
        ])
        .expect("Summary + milestone");
    }

    #[test]
    fn with_tags() {
        let result: Result<Args, _> = Parser::try_parse_from(&["git-issue-new", "-t"]);
        assert!(result.is_err(), "-t expects an argument");
        let _args: Args =
            Parser::try_parse_from(&["git-issue-new", "-t", "asd"]).expect("With one tag");

        let _args: Args = Parser::try_parse_from(&[
            "git-issue-new",
            "-s",
            crate::cmd_new::SUMMARY,
            "-t",
            "asd",
            "--tag=foo",
        ])
        .expect("With multiple tags");
    }

    #[test]
    fn with_milestone() {
        let result: Result<Args, _> = Parser::try_parse_from(&["git-issue-new", "-m"]);
        assert!(result.is_err(), "-m expects an argument");
        let _args: Args =
            Parser::try_parse_from(&["git-issue-new", "-m", "asd"]).expect("With milestone");
    }
}
