#![allow(missing_docs)]
use std::fs;
use std::path::Path;

use clap::Parser;

use posix_errors::PosixError;

use git_issue::DataSource;

#[derive(Parser, Debug, logflag::LogFromArgs)]
#[clap(
    author,
    version,
    about = "Create new issue",
    help_expected = true,
    dont_collapse_args_in_usage = true
)]
struct Args {
    #[clap(short, long, long_help = "Fix validation errors")]
    fix: bool,
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

fn validate_issue(id: &str, path: &Path, fix: bool) -> Result<bool, PosixError> {
    let mut result = true;
    for entry in fs::read_dir(path)? {
        let dir_entry = entry?;
        if !dir_entry.file_type()?.is_dir() {
            let cur_path = dir_entry.path();
            let text = fs::read_to_string(&cur_path)?;
            if !text.ends_with('\n') {
                let url = cur_path.to_string_lossy();
                let name = format!("{}/{}", &id[..8], dir_entry.file_name().to_string_lossy());
                let link = terminal_link::Link::new(&name, &url);
                if fix {
                    println!("{}:Fixing NL at EOF", link);
                    fs::write(cur_path, format!("{}\n", text))?;
                } else {
                    println!("{}:Missing NL at EOF", link);
                    result = false;
                }
            }
        }
    }
    Ok(result)
}

fn validate(data: &DataSource, fix: bool) -> Result<bool, PosixError> {
    let mut result = true;
    let prefix_entries = fs::read_dir(&data.issues_dir.join("issues"))?;
    for prefix_entry in prefix_entries {
        let prefix_dir_entry = prefix_entry?;
        if prefix_dir_entry.file_type()?.is_dir() {
            for entry in fs::read_dir(prefix_dir_entry.path())? {
                let dir_entry = entry?;
                if prefix_dir_entry.file_type()?.is_dir() {
                    let id = format!(
                        "{}{}",
                        prefix_dir_entry.file_name().to_string_lossy(),
                        dir_entry.file_name().to_string_lossy()
                    );
                    if !validate_issue(&id, &dir_entry.path(), fix)? {
                        result = false;
                    }
                }
            }
        }
    }
    Ok(result)
}

fn main() {
    let args = Args::parse();
    let data = match DataSource::try_new(&None, &None) {
        Err(e) => {
            eprintln!(" error: {}", e);
            std::process::exit(128);
        }
        Ok(repo) => repo,
    };

    match validate(&data, args.fix) {
        Ok(valid) => {
            if !valid {
                std::process::exit(1);
            }
        }
        Err(e) => std::process::exit(e.code()),
    }
}
