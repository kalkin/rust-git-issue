use clap::Parser;

#[derive(Parser, Debug)]
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

fn main() {
    let args = Args::parse();
    set_log_level(&args);
    if let Err(e) = git_issue::create(&std::env::current_dir().expect("CWD"), args.existing) {
        std::process::exit(e.code());
    }
}
