#![allow(missing_docs)]
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};

use posix_errors::PosixError;

use git_issue::{DataSource, WriteResult};

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "Set or remove milestone",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(long_help = "Issue id")]
    issue_id: String,
    #[clap(
        short,
        long,
        long_help = "Remove milestone from issue",
        conflicts_with = "milestone"
    )]
    remove: bool,

    /// Milestone to set
    #[clap(conflicts_with = "remove", required_unless_present = "remove")]
    milestone: Option<String>,

    #[clap(long, long_help = "Directory where the GIT_DIR is")]
    git_dir: Option<String>,
    #[clap(long, long_help = "Directory where the GIT_WORK_TREE is")]
    work_tree: Option<String>,

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,
}

fn execute(args: &Args, mut data: DataSource) -> Result<(), PosixError> {
    let id = data.find_issue(&args.issue_id).map_err(PosixError::from)?;
    log::info!("Starting transaction");
    data.start_transaction().map_err(PosixError::from)?;
    if args.remove {
        match data.remove_milestone(&id) {
            Err(e) => {
                log::error!("{}", e);
                log::info!("Rolling back transaction");
                data.rollback_transaction().map_err(PosixError::from)?;
                Err(PosixError::from(e))
            }
            Ok(WriteResult::NoChanges) => {
                log::warn!("Issue already has no milestone {}", &id.0[..8]);
                log::info!("Rolling back transaction");
                data.rollback_transaction().map_err(PosixError::from)
            }
            Ok(WriteResult::Applied) => {
                log::warn!("Removed milestone from issue {}", &id.0[..8]);
                log::info!("Committing transaction");
                data.finish_transaction_without_merge()
                    .map_err(PosixError::from)
            }
        }
    } else {
        match data.add_milestone(&id, args.milestone.as_ref().expect("Expect milestone here")) {
            Err(e) => {
                log::error!("{}", e);
                log::info!("Rolling back transaction");
                data.rollback_transaction().map_err(PosixError::from)?;
                Err(PosixError::from(e))
            }
            Ok(WriteResult::NoChanges) => {
                log::warn!(
                    "Milestone “{}” already set on issue {}",
                    &args.milestone.as_ref().expect("Milestone set"),
                    &id.0[..8]
                );
                log::info!("Rolling back transaction");
                data.rollback_transaction().map_err(PosixError::from)
            }
            Ok(WriteResult::Applied) => {
                log::warn!(
                    "Set milestone “{}” on issue {}",
                    &args.milestone.as_ref().expect("Milestone set"),
                    &id.0[..8]
                );
                log::info!("Committing transaction");
                data.finish_transaction_without_merge()
                    .map_err(PosixError::from)
            }
        }
    }

    /*.map_err(PosixError::from)
    .map_err(|e| {
        })
    .and_then(|r| match r {
            WriteResult::Applied => {
                log::warn!("Set milestone “{}” on issue {}", &args.milestone, &id.0[..8]);
                log::info!("Committing transaction");
                data.finish_transaction_without_merge().map_err(PosixError::from)
            }
            WriteResult::NoChanges => {
                log::warn!("Milestone “{}” already set on issue {}", &args.milestone, &id.0[..8]);
                log::info!("Rolling back transaction");
                data.rollback_transaction().map_err(PosixError::from)
            }
        })*/
}

#[cfg(not(tarpaulin_include))]
fn main() {
    let args = Args::parse();
    cli_log::init_with_level(args.verbose.log_level_filter());
    log::debug!("Log Level is set to {}", log::max_level());
    let data = match DataSource::try_new(&args.git_dir, &args.work_tree) {
        Err(e) => {
            let err: PosixError = e.into();
            log::error!("{}", err);
            std::process::exit(err.code());
        }
        Ok(repo) => repo,
    };

    if let Err(e) = execute(&args, data) {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
}
