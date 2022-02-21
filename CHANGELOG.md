# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
