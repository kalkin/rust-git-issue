#![allow(missing_docs)]

use std::collections::HashMap;

use clap::{Parser, Subcommand};
use clap_verbosity_flag::{Verbosity, WarnLevel};

use posix_errors::PosixError;

use git_issue::{DataSource, Issue, WriteResult};

#[derive(Subcommand)]
enum Command {
    /// List milestones
    List {
        /// List milestones without open issues
        #[clap(short, long)]
        all: bool,
    },
    /// Remove milestone from issue
    Remove {
        /// Issue id
        issue_id: String,
    },
    /// Set issue milestone
    Set {
        /// Issue id
        issue_id: String,

        /// Milestone name
        milestone: String,
    },
}

impl Default for Command {
    fn default() -> Self {
        Self::List { all: false }
    }
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
    command: Option<Command>,

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

enum Milestone {
    No { closed: bool },
    Named { name: String, closed: bool },
}

#[allow(clippy::print_stdout)]
fn list_cmd(data: &DataSource, all: bool) -> Result<(), PosixError> {
    let mut error = false;

    let mut issues: Vec<Issue<'_>> = {
        let (success, errors): (Vec<_>, Vec<_>) = data.all().into_iter().partition(Result::is_ok);
        if !errors.is_empty() {
            error = true;
            for e in errors.into_iter().map(Result::unwrap_err) {
                log::warn!("{}", e);
            }
        }
        success.into_iter().map(Result::unwrap).collect()
    };

    let (mut no_milestone_open, mut no_milestone_closed) = (0, 0);
    let mut results: Vec<_> = {
        let milestones = {
            let cached_issues = {
                let cached_milestone_issues = {
                    let (success, errors): (Vec<_>, Vec<_>) = issues
                        .iter_mut()
                        .map(Issue::cache_milestone)
                        .partition(Result::is_ok);
                    for e in errors.into_iter().map(Result::unwrap_err) {
                        error = true;
                        log::warn!("{}", e);
                    }
                    success.into_iter().map(Result::unwrap)
                };

                let (success, errors): (Vec<_>, Vec<_>) = cached_milestone_issues
                    .map(Issue::cache_tags)
                    .partition(Result::is_ok);
                for e in errors.into_iter().map(Result::unwrap_err) {
                    error = true;
                    log::warn!("{}", e);
                }
                success.into_iter().map(Result::unwrap)
            };

            cached_issues.map(|issue| -> Milestone {
                match (issue.milestone(), issue.is_closed()) {
                    (None, closed) => Milestone::No { closed },
                    (Some(m), closed) => Milestone::Named {
                        name: m.clone(),
                        closed,
                    },
                }
            })
        };

        let mut all_milestones: HashMap<String, (usize, usize)> = HashMap::new();
        for milestone in milestones {
            match milestone {
                Milestone::No { closed: true } => no_milestone_closed += 1,
                Milestone::No { closed: false } => no_milestone_open += 1,
                Milestone::Named { name, closed } => {
                    match (all_milestones.get_mut(&name), closed) {
                        (None, true) => {
                            all_milestones.insert(name, (0, 1));
                        }
                        (None, false) => {
                            all_milestones.insert(name, (1, 0));
                        }
                        (Some((_, c)), true) => *c += 1,
                        (Some((o, _)), false) => *o += 1,
                    }
                }
            }
        }
        all_milestones.into_iter().collect()
    };

    results.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    if !all {
        results.retain(|(_, (open, _))| *open != 0);
    }

    for (name, (open, closed)) in results {
        println!("{}\t{}/{}", name, open, open + closed);
    }
    println!(
        "No Milestone\t{}/{}",
        no_milestone_open,
        no_milestone_open + no_milestone_closed
    );

    if error {
        Err(PosixError::new(1, "Errors happened".to_owned()))
    } else {
        Ok(())
    }
}

#[cfg(not(tarpaulin_include))]
#[allow(clippy::exit)]
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
        None => list_cmd(&data, false),
        Some(Command::List { all }) => list_cmd(&data, all),
        Some(Command::Remove { issue_id }) => remove_cmd(data, &issue_id),
        Some(Command::Set {
            issue_id,
            milestone,
        }) => set_cmd(data, &issue_id, &milestone),
    } {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
}
