#![allow(missing_docs)]
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};

use posix_errors::PosixError;

use git_issue::DataSource;

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "Show issue",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    /// Issue id
    issue_id: String,

    /// Show comments
    #[clap(short, long)]
    comments: bool,

    /// Directory where the GIT_DIR is
    #[clap(long, help_heading = "GIT OPTIONS")]
    git_dir: Option<String>,
    /// Directory where the GIT_WORK_TREE is
    #[clap(long, help_heading = "GIT OPTIONS")]
    work_tree: Option<String>,

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,
}

#[allow(clippy::print_stdout)]
fn execute(args: &Args, data: &DataSource) -> Result<(), PosixError> {
    let mut issue = data.find(&args.issue_id)?;

    println!("issue      {}", issue.id().id());

    issue.cache_cdate().expect("Cached CDate");
    println!("Date       {}", issue.cdate());

    issue.cache_milestone().expect("Cached Milestone");
    if let Some(milestone) = issue.milestone() {
        println!("Milestone  {}", milestone);
    }

    issue.cache_ddate().expect("Cached DDate");
    if let Some(ddate) = issue.ddate() {
        println!("Due Date   {}", ddate);
    }

    issue.cache_tags().expect("Cached Tags");
    println!("Tags       {}", issue.tags().join(", "));

    println!();

    issue.cache_desc().expect("Cached Description");
    for line in issue.desc().lines() {
        println!("    {}", line);
    }

    println!();

    println!("Edit History:");
    let dir_path = &issue.id().path(&data.issues_dir);
    let files = &["description", "tags", "duedate", "milestone"];
    let paths = files.map(|d| dir_path.join(d));
    let _result = data
        .repo
        .git()
        .args(&[
            "log",
            "-M",
            "-C",
            "-C",
            "-C",
            "--reverse",
            "--format=* %>(16)%ah by %aN — %s",
            "--",
        ])
        .args(paths)
        .status();

    if args.comments {
        println!();
        issue.cache_comments();
        for comment in issue.comments() {
            println!("comment {}", comment.id());
            println!("Author: {}", comment.author());
            println!("Date    {}", comment.cdate());
            for line in comment.body().lines() {
                println!();
                println!("    {}", line);
                println!();
            }
        }
    }

    Ok(())
}

#[allow(clippy::exit)]
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

    if let Err(e) = execute(&args, &data) {
        log::error!("{}", e);
        std::process::exit(e.code());
    }
}
