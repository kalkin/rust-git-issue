#![allow(missing_docs)]
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};

#[derive(Parser)]
#[clap(
    author,
    version,
    about = "Create new issues repository in $CWD",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(short, long, long_help = "Use existing git repository")]
    existing: bool,

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,
}

fn main() {
    let args = Args::parse();
    cli_log::init_with_level(args.verbose.log_level_filter());
    if let Err(e) = git_issue::create(&std::env::current_dir().expect("CWD"), args.existing) {
        std::process::exit(e.code());
    }
}

#[cfg(test)]
mod parse_args {
    use crate::Args;
    use clap::Parser;

    #[test]
    fn no_arguments() {
        let _args: Args = Parser::try_parse_from(&["git-issue-new"]).expect("No arguments");
    }

    #[test]
    fn with_existing() {
        let _args: Args = Parser::try_parse_from(&["git-issue-new", "-e"]).expect("With -e");
        let _args: Args =
            Parser::try_parse_from(&["git-issue-new", "--existing"]).expect("With --existing");
    }
}
