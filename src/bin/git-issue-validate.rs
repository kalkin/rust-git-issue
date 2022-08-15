#![allow(missing_docs)]
use std::path::Path;
use std::{fs, process::Command};

use clap::Parser;
use clap_git_options::GitOptions;
use clap_verbosity_flag::{Verbosity, WarnLevel};

use posix_errors::PosixError;

use git_issue::{DataSource, Id};

#[derive(Parser)]
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

    #[clap(flatten, next_help_heading = "Output")]
    verbose: Verbosity<WarnLevel>,

    #[clap(flatten)]
    git: GitOptions,
}

fn validate_issue(id: &Id, path: &Path, fix: bool) -> Result<bool, PosixError> {
    log::info!("Validating issue: {}", id.short_id());
    let mut result = true;

    for entry in fs::read_dir(path)? {
        let dir_entry = entry?;
        if !dir_entry.file_type()?.is_dir() {
            let cur_path = dir_entry.path();
            let text = fs::read_to_string(&cur_path)?;
            if !text.ends_with('\n') {
                let url = cur_path.to_string_lossy();
                let name = format!(
                    "{}/{}",
                    id.short_id(),
                    dir_entry.file_name().to_string_lossy()
                );
                let link = terminal_link::Link::new(&name, &url);
                if fix {
                    log::warn!("{}:Fixing NL at EOF", link);
                    fs::write(cur_path, format!("{}\n", text))?;
                } else {
                    log::warn!("{}:Missing NL at EOF", link);
                    result = false;
                }
            }
        }
    }

    let out = Command::new("git")
        .args(&["rev-list", "--quiet", "-1", id.id(), "--"])
        .output()?;
    if !out.status.success() {
        let expected = {
            let out_child = Command::new("git")
                .args(&["rev-list", "--reverse", "-1", "HEAD", "--"])
                .arg(path.join("description"))
                .output()?;
            let child_commit = String::from_utf8_lossy(&out_child.stdout);
            let out_parent = Command::new("git")
                .args(&[
                    "rev-list",
                    "-1",
                    &format!("{}^1", child_commit.trim()),
                    "--",
                ])
                .output()?;
            let output = String::from_utf8_lossy(&out_parent.stdout);
            Id::new(output.trim().to_owned())
        };

        if fix {
            log::warn!(
                "Fixing issue id!\n\tGot: {}\n\tExpected {}",
                id.short_id(),
                expected.short_id(),
            );
            let parent_dir = path
                .parent()
                .expect("prefix dir")
                .parent()
                .expect("issues dir")
                .parent()
                .expect("issues root dir");
            let src_dir = id.path(parent_dir);
            let dst_dir = expected.path(parent_dir);
            fs::create_dir_all(dst_dir.parent().expect("prefix dir"))?;
            log::warn!("Moving to {:?}", dst_dir);
            let _status = Command::new("git")
                .arg("mv")
                .args(&[src_dir, dst_dir])
                .status()?;
        } else {
            log::warn!(
                "Invalid issue id!\n\tGot: {}\n\tExpected {}",
                id.short_id(),
                expected.short_id(),
            );
            result = false;
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
                    let path = dir_entry.path();
                    let id = Id::from(dir_entry);
                    if !validate_issue(&id, &path, fix)? {
                        result = false;
                    }
                }
            }
        }
    }
    Ok(result)
}

#[allow(clippy::exit)]
fn main() {
    let args = Args::parse();
    cli_log::init_with_level(args.verbose.log_level_filter());
    let data = match git_issue::DataSource::try_new(&args.git) {
        Err(e) => {
            log::error!("{}", e);
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
