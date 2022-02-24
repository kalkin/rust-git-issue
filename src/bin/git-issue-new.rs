use clap::Parser;

use posix_errors::PosixError;

#[derive(Parser, Debug, Default, logflag::LogFromArgs)]
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
    summary: String,

    #[clap(short, long, long_help = "Edit the issue")]
    edit: bool,

    #[clap(long, long_help = "Directory where the GIT_DIR is")]
    git_dir: Option<String>,
    #[clap(long, long_help = "Directory where the GIT_WORK_TREE is")]
    work_tree: Option<String>,

    #[clap(
        short,
        long,
        parse(from_occurrences),
        long_help = "Log level up to -vvv"
    )]
    verbose: usize,
    #[clap(
        short,
        long,
        parse(from_flag),
        long_help = "Only print errors (Overrides -v)"
    )]
    quiet: bool,
}

fn execute(args: &Args, mut data: git_issue::DataSource) -> Result<git_issue::Id, PosixError> {
    let empty: Vec<String> = vec![];
    let tags = args.tags.as_ref().unwrap_or(&empty).clone();
    let milestone = args.milestone.clone();
    let description = if args.edit {
        let template = format!(
            "{}\n\n{}",
            args.summary,
            &git_issue::read_template(&data.repo, "description").unwrap_or_default()
        );
        git_issue::edit(&data.repo, &template)?
    } else {
        args.summary.clone()
    };

    data.start_transaction()?;
    match data.create_issue(&description, tags, milestone) {
        Ok(id) => {
            let message = format!("gi({}): {}", &id.0[..8], &args.summary);
            #[cfg(not(feature = "strict-compatibility"))]
            log::info!("Merging issue creation as not fast forward branch");
            data.finish_transaction(&message)?;
            Ok(id)
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
    set_log_level(&args);
    let data = match git_issue::DataSource::try_new(&args.git_dir, &args.work_tree) {
        Err(e) => {
            log::error!(" error: {}", e);
            std::process::exit(128);
        }
        Ok(repo) => repo,
    };
    match execute(&args, data) {
        Ok(id) => println!("Added issue {}: {}", &id.0[..8], args.summary),
        Err(e) => std::process::exit(e.code()),
    }
}

#[cfg(test)]
mod cmd_new {

    use git_issue::{DataSource, Id};
    use std::path::Path;

    const SUMMARY: &str = "New Issue";

    fn execute_new(args: &crate::Args, tmp: &Path) -> Id {
        let data = DataSource::try_from(tmp).unwrap();
        let result = crate::execute(&args, data);
        result.expect("Execution successful")
    }

    #[test]
    fn only_message() {
        let tmp_dir = tempdir::TempDir::new("new").unwrap();
        let tmp = tmp_dir.path();
        git_issue::create(tmp, false).unwrap();

        let id = {
            let mut args = crate::Args::default();
            args.summary = SUMMARY.to_owned();
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
        let tmp_dir = tempdir::TempDir::new("new").unwrap();
        let tmp = tmp_dir.path();
        git_issue::create(tmp, false).unwrap();
        let tags = vec!["foo".to_string(), "bar".to_string()];

        let id = {
            let mut args = crate::Args::default();
            args.summary = SUMMARY.to_owned();
            args.tags = Some(tags.clone());

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
        let tmp_dir = tempdir::TempDir::new("new").unwrap();
        let tmp = tmp_dir.path();
        git_issue::create(tmp, false).unwrap();
        let milestone = "World Domination!";

        let id = {
            let mut args = crate::Args::default();
            args.summary = SUMMARY.to_owned();
            args.milestone = Some(milestone.to_owned());

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
