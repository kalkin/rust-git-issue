use clap::Parser;
use posix_errors::PosixError;

use git_issue::{DataSource, FindError, Id};

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

fn close_issues(data: &DataSource, ids: &[Id]) -> Result<(), PosixError> {
    for id in ids {
        data.remove_tag(id, "open")?;
        data.add_tag(id, "closed")?;
        println!("Closed issue {}: {}", &id.0[..8], data.title(id).unwrap());
    }
    Ok(())
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

fn main() {
    let args = Args::parse();
    set_log_level(&args);
    let mut data = match git_issue::DataSource::try_new(&args.git_dir, &args.work_tree) {
        Err(e) => {
            log::error!(" error: {}", e);
            std::process::exit(128);
        }
        Ok(d) => d,
    };

    let issue_ids: Vec<Id> = match args
        .issue_ids
        .iter()
        .map(|id| data.find_issue(id))
        .collect::<Result<Vec<Id>, FindError>>()
        .map_err(PosixError::from)
    {
        Err(e) => {
            log::error!("{}", e);
            std::process::exit(e.code());
        }
        Ok(ids) => ids,
    };

    log::info!("Starting transaction");
    if let Err(e) = data.start_transaction().map_err(PosixError::from) {
        log::error!("{}", e);
        std::process::exit(e.code());
    }

    if let Err(e) = close_issues(&data, &issue_ids) {
        log::error!("{}", e);
        log::warn!("Rolling back transaction");
        if let Err(err) = data.rollback_transaction().map_err(PosixError::from) {
            log::error!("{}", err);
            std::process::exit(err.code());
        }
        std::process::exit(e.code());
    }

    let msg = if issue_ids.len() == 1 {
        format!(
            "DONE({}): {}",
            &issue_ids[0].0[..8],
            data.title(&issue_ids[0]).unwrap()
        )
    } else {
        let text = issue_ids
            .iter()
            .map(|id| &id.0[..8])
            .collect::<Vec<_>>()
            .join(", ");
        format!("gi: Closed {}", text)
    };

    log::info!("Commiting transaction");
    if let Err(e) = data.finish_transaction(&msg).map_err(PosixError::from) {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
}
