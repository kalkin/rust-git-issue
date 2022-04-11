#![allow(missing_docs)]
use clap::ArgEnum;
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use git_issue::CacheError;
use git_issue::FormatString;
use git_issue::Issue;
use posix_errors::PosixError;

use git_issue::DataSource;

#[derive(clap::Args)]
#[clap(next_help_heading = "FILTERS")]
struct FilterArgs {
    /// List open & closed issues
    #[clap(short, long)]
    all: bool,

    /// Include issues having specified tag
    #[clap(short = 't', long)]
    with_tags: Vec<String>,

    /// Include issues *not* having specified tag
    #[clap(short = 'T', long)]
    without_tags: Vec<String>,
    /// Include issues with specified milestone
    #[clap(
        name = "milestone",
        short = 'm',
        long,
        conflicts_with = "without-milestone"
    )]
    with_milestone: Option<String>,

    /// Include issues *without* any milestone
    #[clap(short = 'M', long)]
    without_milestone: bool,
}

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "List issues",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(
        long,
        long_help = "Directory where the GIT_DIR is",
        help_heading = "Git Options"
    )]
    git_dir: Option<String>,
    #[clap(
        long,
        long_help = "Directory where the GIT_WORK_TREE is",
        help_heading = "Git Options"
    )]
    work_tree: Option<String>,

    #[clap(flatten)]
    filter: FilterArgs,

    #[clap(flatten, next_help_heading = "OUTPUT")]
    verbose: Verbosity<WarnLevel>,

    /// Format string
    #[clap(
        short = 'l',
        help_heading = "OUTPUT",
        default_value = "simple",
        parse(try_from_str=TryFrom::try_from)
    )]
    format_string: FormatString,

    /// Print results in reverse order
    #[clap(short, long, help_heading = "ORDER OPTIONS")]
    reverse: bool,

    /// Order issues by specified fields
    #[clap(arg_enum, short, long, help_heading = "ORDER OPTIONS")]
    order: Option<SortKey>,
}

#[derive(Copy, Clone, ArgEnum)]
enum SortKey {
    #[clap(name = "%c")]
    CreationDate,
    #[clap(name = "%d")]
    DueDate,
    #[clap(name = "%D")]
    Description,
    #[clap(name = "%M")]
    Milestone,
}

#[derive(Debug)]
enum MilestoneFilter<'args> {
    Without,
    Any,
    Value(&'args String),
}

impl PartialEq for MilestoneFilter<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MilestoneFilter::Value(a), MilestoneFilter::Value(b)) => *a == *b,
            (MilestoneFilter::Any, MilestoneFilter::Any)
            | (MilestoneFilter::Without, MilestoneFilter::Without) => true,
            (_, _) => false,
        }
    }
}

struct Filter<'args> {
    with_tags: Vec<&'args String>,
    without_tags: Vec<&'args String>,
    milestone: MilestoneFilter<'args>,
}

impl<'args> From<&'args mut FilterArgs> for Filter<'args> {
    fn from(args: &'args mut FilterArgs) -> Self {
        let milestone = if args.without_milestone {
            MilestoneFilter::Without
        } else if let Some(m) = &args.with_milestone {
            MilestoneFilter::Value(m)
        } else {
            MilestoneFilter::Any
        };
        if !args.all {
            args.with_tags.push("open".to_owned());
        }
        let with_tags = args.with_tags.iter().collect();

        Self {
            milestone,
            with_tags,
            without_tags: args.without_tags.iter().collect(),
        }
    }
}

impl<'args> Filter<'args> {
    fn cache<'src>(&'args self, input: Vec<Issue<'src>>) -> (Vec<Issue<'src>>, Vec<CacheError>) {
        let mut errors = vec![];
        let mut result = vec![];
        for mut issue in input {
            if self.milestone != MilestoneFilter::Any {
                if let Err(e) = issue.cache_milestone() {
                    errors.push(e);
                    continue;
                }
            }
            if !self.without_tags.is_empty() || !self.with_tags.is_empty() {
                if let Err(e) = issue.cache_tags() {
                    errors.push(e);
                    continue;
                }
            }
            result.push(issue);
        }
        (result, errors)
    }

    fn apply<'src>(&'args self, input: Vec<Issue<'src>>) -> (Vec<Issue<'src>>, Vec<CacheError>) {
        let (cached, errors): (Vec<Issue<'src>>, Vec<CacheError>) = self.cache(input);
        let issues: Vec<_> = cached
            .into_iter()
            .filter(|issue| {
                if !(match self.milestone {
                    MilestoneFilter::Any => true,
                    MilestoneFilter::Without => issue.milestone().is_none(),
                    MilestoneFilter::Value(expected) => match issue.milestone() {
                        None => return false,
                        Some(actual) => *actual == *expected,
                    },
                }) {
                    return false;
                }

                log::info!("Matching milestone");

                if !self.without_tags.is_empty() {
                    for tag in &self.without_tags {
                        if issue.tags().contains(tag) {
                            return false;
                        }
                    }
                }

                if !self.with_tags.is_empty() {
                    for tag in &self.with_tags {
                        if !issue.tags().contains(tag) {
                            return false;
                        }
                    }
                }

                true
            })
            .collect();
        (issues, errors)
    }
}

struct Query<'args> {
    selection: Filter<'args>,
    projection: &'args FormatString,
    order: Option<SortKey>,
    reverse: bool,
}

impl<'args> From<&'args mut Args> for Query<'args> {
    fn from(args: &'args mut Args) -> Self {
        let projection = &args.format_string;
        let selection = Filter::from(&mut args.filter);
        Self {
            selection,
            projection,
            reverse: args.reverse,
            order: args.order,
        }
    }
}

#[allow(clippy::todo, clippy::panic_in_result_fn)]
pub(crate) fn execute<'src>(args: &mut Args, data: &'src DataSource) {
    let select = Query::from(args);
    let filtered_issues = {
        let (f, errors): (Vec<_>, Vec<_>) = {
            let (issue_results, _issue_errors): (Vec<_>, Vec<_>) =
                data.all().into_iter().partition(Result::is_ok);
            let issues: Vec<Issue<'src>> = issue_results.into_iter().map(Result::unwrap).collect();
            select.selection.apply(issues)
        };
        for e in errors {
            log::error!("{}", e);
        }
        f
    };

    let mut sorted_issues = if let Some(sort) = select.order {
        let (cached, errors): (Vec<_>, Vec<_>) = filtered_issues
            .into_iter()
            .map(|mut i| -> Result<Issue<'_>, CacheError> {
                match sort {
                    SortKey::CreationDate => {
                        i.cache_cdate()?;
                    }
                    SortKey::Description => {
                        i.cache_desc()?;
                    }
                    SortKey::Milestone => {
                        i.cache_milestone()?;
                    }
                    SortKey::DueDate => {
                        i.cache_ddate()?;
                    }
                }
                Ok(i)
            })
            .partition(Result::is_ok);

        for e in errors.into_iter().map(Result::unwrap_err) {
            log::error!("{}", e);
        }
        let mut issues: Vec<Issue<'src>> = cached.into_iter().map(Result::unwrap).collect();
        issues.sort_unstable_by(|a, b| match sort {
            SortKey::CreationDate => a.cdate().cmp(b.cdate()),
            SortKey::Description => a.desc().cmp(b.desc()),
            SortKey::Milestone => a.milestone().cmp(b.milestone()),
            SortKey::DueDate => a.ddate().cmp(b.ddate()),
        });
        issues
    } else {
        filtered_issues
    };

    if select.reverse {
        sorted_issues.reverse();
    }

    for mut i in sorted_issues {
        log::warn!("{}", select.projection.format(&mut i));
    }
}

#[cfg(not(tarpaulin_include))]
#[allow(clippy::exit)]
fn main() {
    let mut args = Args::parse();
    cli_log::init_with_level(args.verbose.log_level_filter());
    log::debug!("Log Level is set to {}", log::max_level());
    let data = match DataSource::try_new(&args.git_dir, &args.work_tree) {
        Err(e) => {
            let err: PosixError = e.into();
            log::error!(" error: {}", err);
            std::process::exit(err.code());
        }
        Ok(repo) => repo,
    };

    execute(&mut args, &data);
}
