use clap::Parser;
use posix_errors::PosixError;

use git_issue::{DataSource, FindError, Id};

#[derive(Parser, logflag::LogFromArgs)]
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
        Ok(_) => {
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
            log::info!("Committing transaction");
            data.finish_transaction(&msg).map_err(PosixError::from)
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
    let data = match git_issue::DataSource::try_new(&args.git_dir, &args.work_tree) {
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
