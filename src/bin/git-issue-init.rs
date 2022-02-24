use clap::Parser;

#[derive(Parser, Debug, logflag::LogFromArgs)]
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

fn main() {
    let args = Args::parse();
    set_log_level(&args);
    if let Err(e) = git_issue::create(&std::env::current_dir().expect("CWD"), args.existing) {
        std::process::exit(e.code());
    }
}
