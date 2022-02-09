use clap::{AppSettings, Parser};

use git_wrapper::Repository;

#[derive(Parser, Debug)]
#[clap(author, version, about = "Create new issue")]
#[clap(global_setting(AppSettings::DontCollapseArgsInUsage))]
#[clap(global_setting(AppSettings::HelpExpected))]
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
    log::info!("Log Level is set to {}", log::max_level());
}

fn new_issue(args: &Args, repo: &Repository) -> Result<git_issue::Id, git_issue::Error> {
    let transaction = git_issue::start_transaction(repo)?;
    if let Err(e) = git_issue::commit(repo, "gi: Add issue", "gi new mark") {
        log::warn!("Rolling back transaction");
        log::error!("{}", e.message());
        git_issue::rollback_transaction(&transaction, repo)?;
        return Err(e);
    }
    let tags = {
        let empty: Vec<String> = vec![];
        let mut tags = args.tags.as_ref().unwrap_or(&empty).clone();
        tags.extend(vec!["open".to_string()]);
        tags.sort();
        tags.dedup();
        tags
    };

    let id: git_issue::Id = git_issue::Id(repo.head().expect("HEAD ref exists"));
    let path = id.path(repo);

    log::info!("{:?} + {:?}: {:?}", id, path, tags);
    let description = args.summary.clone();
    let milestone = args.milestone.clone();
    let issue = git_issue::Issue {
        id: id.clone(),
        description,
        milestone,
        tags,
    };
    log::info!("Creating issue: {:?}", issue.id.short());
    if let Err(e) = git_issue::create_issue(&issue, repo) {
        log::warn!("Rolling back transaction");
        log::error!("{}", e.message());
        git_issue::rollback_transaction(&transaction, repo)?;
        return Err(e);
    }
    if let Err(e) = git_issue::commit(
        repo,
        "gi: Add issue description",
        &format!("gi new description {}", issue.id.0),
    ) {
        log::warn!("Rolling back transaction");
        log::error!("{}", e.message());
        git_issue::rollback_transaction(&transaction, repo)?;
        return Err(e);
    }

    let message = format!("gi({}): {}", &id.0[..8], &args.summary);
    git_issue::commit_transaction(&transaction, repo, &message)?;

    Ok(id)
}

fn main() {
    let args = Args::parse();
    set_log_level(&args);
    let repo = match Repository::from_args(None, args.git_dir.as_deref(), args.work_tree.as_deref())
    {
        Err(e) => {
            log::error!(" error: {}", e);
            std::process::exit(128);
        }
        Ok(repo) => repo,
    };
    match new_issue(&args, &repo) {
        Ok(id) => println!("Added issue {}: {}", &id.0[..8], args.summary),
        Err(e) => std::process::exit(*e.code()),
    }
}
