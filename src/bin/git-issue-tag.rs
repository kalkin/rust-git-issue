use clap::Parser;

use posix_errors::PosixError;

use git_issue::{DataSource, Id};

#[derive(Parser, Debug, logflag::LogFromArgs)]
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

    for tag in tags {
        if cur_tags.contains(tag) {
            log::warn!("Skipping tag {}. Already applied to {}.", tag, short_id);
        } else {
            log::info!("Adding tag {} to {}", tag, short_id);
            data.add_tag(id, tag)?;
        }
    }

    Ok("Added tags".to_owned())
}

fn remove_tags(data: &DataSource, id: &Id, tags: &[String]) -> Result<String, PosixError> {
    let short_id = &id.0[..8];
    let cur_tags = data.tags(id);

    for tag in tags {
        if cur_tags.contains(tag) {
            log::info!("Removing tag {} from {}", tag, short_id);
            data.remove_tag(id, tag)?;
        } else {
            log::warn!("Skipping tag {}. {} not tagged with it.", tag, short_id);
        }
    }
    Ok("Removed tags".to_owned())
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
