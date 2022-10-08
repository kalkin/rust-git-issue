# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.10] - 2022-10-08

### Changed

- Fix all clippy::arithmetics warnings
- Use clap-git-options

## [0.0.9] - 2022-04-21

### Added

- Implement `show` command
- Support for reading comments (Cleanup unwrap)

### Changed

- `milestone list` only show milestones with open issues

## [0.0.8] - 2022-03-29

### Added

- Command `validate` check issue id for validity

### Fixed

- fix: Use proper git tag remove annotation on commit
- fix: Handle parsing DateTime parsing errors
- fix: Handle caching errors during issue formatting

## [0.0.7] - 2022-03-29

### Added

- List all milestones with `gi milestone`
- Implement `gi list` command
- Add support for due date
- Add support for creation date

### Changed

- Issue::cache_* functions return &mut Self
- `gi milestone` Use subcommands instead of switches
- Make internal `Id` structure crate public

## [0.0.6] - 2022-03-06

### Added

- Implement `milestone` command

### Changed

- Do not use commit id when using release version
- Better commit messages when !strict-compatibility is set
- Commands `tag` & `close` handle when there is nothing to do
- Show `-q` & `-v` flags in own section in usage
- Use `claps::derive` `Args` for command line parsing

## [0.0.5] - 2022-02-28

### Changed

- `tag` command merge message

### Fixed

- Do not return empty string as tag

## [0.0.4] - 2022-02-24

### Added

- Implement `close` command (bef6c4a)

### Changed

- Unify error codes
- Disable commit hooks during issue operations

### Fixed

- Handle multiple matching ids
- Add new line at EOF in description file

## [0.0.3] - 2022-02-23

### Added

- Implement `tag` command
- Implement `init` command
- Add flag for strict git-issue compatibility

## [0.0.2] - 2022-02-21

### Added

- `validate` command for checkin that each issue file ends in a newline
- Add support for opening `$EDITOR`
- Add `-m` milestone switch to `new` command
- Add development milestones to the README.md

### Changed

- Make `start_transaction` module private
- Make each action own commit

### Fixed

- Add new line at EOF in tags file
- Implement original .issues directory discovery algorithm

## [0.0.1] - 2022-02-08

### Added

- Add README.md
- Implement `git issue new` command
