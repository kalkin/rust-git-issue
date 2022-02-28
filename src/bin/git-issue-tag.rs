#![allow(missing_docs)]
use clap::Parser;

use posix_errors::PosixError;

use git_issue::{DataSource, Id};

#[derive(Parser, Default, logflag::LogFromArgs)]
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

fn add_tags(data: &DataSource, id: &Id, tags: &[String]) -> Result<String, PosixError> {
    let short_id = &id.0[..8];
    let cur_tags = data.tags(id);
    let mut applied: Vec<&str> = Vec::with_capacity(tags.len());

    for tag in tags {
        if cur_tags.contains(tag) {
            log::warn!("Skipping tag {}. {} already tagged with it.", tag, short_id);
        } else {
            log::info!("Adding tag {} to {}", tag, short_id);
            data.add_tag(id, tag)?;
            applied.push(tag);
        }
    }

    let word = if applied.len() > 1 { "tags" } else { "tag" };
    let msg = format!("gi({}): Add {}: {}", short_id, word, applied.join(", "));
    Ok(msg)
}

fn remove_tags(data: &DataSource, id: &Id, tags: &[String]) -> Result<String, PosixError> {
    let short_id = &id.0[..8];
    let cur_tags = data.tags(id);
    let mut applied: Vec<&str> = Vec::with_capacity(tags.len());

    for tag in tags {
        if cur_tags.contains(tag) {
            log::info!("Removing tag {} from {}", tag, short_id);
            data.remove_tag(id, tag)?;
            applied.push(tag);
        } else {
            log::warn!("Skipping tag {}. {} not tagged with it.", tag, short_id);
        }
    }

    let word = if applied.len() > 1 { "tags" } else { "tag" };
    let msg = format!("gi({}): Remove {}: {}", short_id, word, applied.join(", "));
    Ok(msg)
}

fn execute(args: &Args, mut data: DataSource) -> Result<(), PosixError> {
    let id = data.find_issue(&args.issue_id).map_err(PosixError::from)?;
    log::info!("Starting transaction");
    data.start_transaction().map_err(PosixError::from)?;

    let result = if args.remove {
        remove_tags(&data, &id, &args.tags)
    } else {
        add_tags(&data, &id, &args.tags)
    };
    match result {
        Ok(message) => {
            log::info!("Committing transaction");
            data.finish_transaction(&message).map_err(PosixError::from)
        }
        Err(e) => {
            log::warn!("An error happend. Rolling back transaction.");
            data.rollback_transaction()?;
            Err(e)
        }
    }
}

#[cfg(not(tarpaulin_include))]
fn main() {
    let args = Args::parse();
    set_log_level(&args);
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
    use git_issue::{DataSource, Id};
    use std::path::Path;

    fn prepare(tmp_dir: &Path, tags: &[String]) -> Id {
        git_issue::create(tmp_dir, false).unwrap();
        let issues_dir = tmp_dir.join(".issues");
        let data = DataSource::try_from(issues_dir.as_path()).unwrap();
        let result = data.create_issue("Foo Bar", tags.to_vec(), None);
        result.expect("Created new issue")
    }

    #[test]
    fn add_tag() {
        let tmp_dir = tempdir::TempDir::new("tag").unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(&tmp, &[]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let mut args = crate::Args::default();
            args.issue_id = id.0.clone();
            args.tags = vec!["foo".to_string()];
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
        let tmp_dir = tempdir::TempDir::new("tag").unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(&tmp, &["foo".to_string()]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let mut args = crate::Args::default();
            args.issue_id = id.0.clone();
            args.tags = vec!["foo".to_string()];
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
        let tmp_dir = tempdir::TempDir::new("tag").unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(&tmp, &["foo".to_string()]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let mut args = crate::Args::default();
            args.issue_id = id.0.clone();
            args.tags = vec!["foo".to_string()];
            args.remove = true;
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
        let tmp_dir = tempdir::TempDir::new("tag").unwrap();
        let tmp = tmp_dir.path();
        let id = prepare(&tmp, &[]);
        {
            let data = DataSource::try_from(tmp).unwrap();
            let mut args = crate::Args::default();
            args.issue_id = id.0.clone();
            args.tags = vec!["foo".to_string()];
            args.remove = true;
            assert!(crate::execute(&args, data).is_ok());
        }
        let data = DataSource::try_from(tmp).unwrap();
        {
            let tags = data.tags(&id);
            assert_eq!(tags, ["open".to_string()], "Only tag open");
        }
    }
}
