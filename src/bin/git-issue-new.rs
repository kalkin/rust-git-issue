use clap::Parser;

use posix_errors::PosixError;

#[derive(Parser, Debug)]
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
