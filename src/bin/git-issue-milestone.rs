#![allow(missing_docs)]
use clap::{Parser, Subcommand};
use clap_verbosity_flag::{Verbosity, WarnLevel};

use posix_errors::PosixError;

use git_issue::{DataSource, WriteResult};

#[derive(Subcommand)]
enum Command {
    Remove {
        /// Issue id
        issue_id: String,
    },
    Set {
        /// Issue id
        issue_id: String,

        /// Milestone name
        milestone: String,
    },
}

/// Manage milestones
#[derive(Parser)]
#[clap(
    author,
    version,
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(subcommand)]
    command: Command,

    #[clap(long, long_help = "Directory where the GIT_DIR is")]
    git_dir: Option<String>,
    #[clap(long, long_help = "Directory where the GIT_WORK_TREE is")]
    work_tree: Option<String>,

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,
}

fn set_cmd(mut data: DataSource, issue_id: &str, milestone: &str) -> Result<(), PosixError> {
    let id = data.find_issue(issue_id).map_err(PosixError::from)?;
    log::info!("Starting transaction");
    data.start_transaction().map_err(PosixError::from)?;

    match data.add_milestone(&id, milestone) {
        Err(e) => {
            log::error!("{}", e);
            log::info!("Rolling back transaction");
            data.rollback_transaction().map_err(PosixError::from)?;
            Err(PosixError::from(e))
        }
        Ok(WriteResult::NoChanges) => {
            log::warn!(
                "Milestone “{}” already set on issue {}",
                milestone,
                &id.short_id()
            );
            log::info!("Rolling back transaction");
            data.rollback_transaction().map_err(PosixError::from)
        }
        Ok(WriteResult::Applied) => {
            log::warn!("Set milestone “{}” on issue {}", milestone, &id.short_id());
            log::info!("Committing transaction");
            data.finish_transaction_without_merge()
                .map_err(PosixError::from)
        }
    }
}

fn remove_cmd(mut data: DataSource, issue_id: &str) -> Result<(), PosixError> {
    let id = data.find_issue(issue_id).map_err(PosixError::from)?;
    log::info!("Starting transaction");
    match data.remove_milestone(&id) {
        Err(e) => {
            log::error!("{}", e);
            log::info!("Rolling back transaction");
            data.rollback_transaction().map_err(PosixError::from)?;
            Err(PosixError::from(e))
        }
        Ok(WriteResult::NoChanges) => {
            log::warn!("Issue already has no milestone {}", &id.short_id());
            log::info!("Rolling back transaction");
            data.rollback_transaction().map_err(PosixError::from)
        }
        Ok(WriteResult::Applied) => {
            log::warn!("Removed milestone from issue {}", &id.short_id());
            log::info!("Committing transaction");
            data.finish_transaction_without_merge()
                .map_err(PosixError::from)
        }
    }
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

    if let Err(e) = match args.command {
        Command::Remove { issue_id } => remove_cmd(data, &issue_id),
        Command::Set {
            issue_id,
            milestone,
        } => set_cmd(data, &issue_id, &milestone),
    } {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
}
