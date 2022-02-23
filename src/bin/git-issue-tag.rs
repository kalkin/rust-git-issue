use clap::Parser;

use posix_errors::PosixError;

#[derive(Parser, Debug)]
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

fn set_log_level(args: &Args) {
    let log_level = if args.quiet {
        log::Level::Error
    } else if args.verbose == 0 {
        log::Level::Warn
    } else if args.verbose == 1 {
        log::Level::Info
    } else if args.verbose == 2 {
        log::Level::Debug
    } else {
        log::Level::Trace
    };
    simple_logger::init_with_level(log_level).unwrap();
    log::debug!("Log Level is set to {}", log::max_level());
}
fn add_tags(
    data: &git_issue::DataSource,
    id: &git_issue::Id,
    tags: Vec<String>,
) -> Result<(), PosixError> {
    for tag in tags {
        data.add_tag(id, &tag)?;
    }
    Ok(())
}

fn remove_tags(
    data: &git_issue::DataSource,
    id: &git_issue::Id,
    tags: Vec<String>,
) -> Result<(), PosixError> {
    for tag in tags {
        log::info!("Removing tag {}", tag);
        data.remove_tag(id, &tag)?;
    }
    Ok(())
}

fn main() {
    let args = Args::parse();
    set_log_level(&args);
    let mut data = match git_issue::DataSource::try_new(&args.git_dir, &args.work_tree) {
        Err(e) => {
            log::error!(" error: {}", e);
            std::process::exit(128);
        }
        Ok(repo) => repo,
    };
    if let Err(e) = data.start_transaction() {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
    let id = data.find_issue(&args.issue_id).unwrap();
    let message = if args.remove {
        if let Err(e) = remove_tags(&data, &id, args.tags) {
            log::error!("{}", e);
            log::warn!("Rolling back transaction");
            data.rollback_transaction().expect("Rollback");
            std::process::exit(e.code());
        }
        "Added tags"
    } else {
        if let Err(e) = add_tags(&data, &id, args.tags) {
            log::error!("{}", e);
            log::warn!("Rolling back transaction");
            data.rollback_transaction().expect("Rollback");
            std::process::exit(e.code());
        }
        "Removed tags"
    };
    if let Err(e) = data.finish_transaction(message) {
        log::error!("{}", e);
        log::warn!("Rolling back transaction");
        data.rollback_transaction().expect("Rollback");
        std::process::exit(e.code());
    }
}
