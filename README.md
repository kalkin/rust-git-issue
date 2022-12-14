# git-issue

## About

A reimplementation of [git-issue(1)](https://github.com/dspinellis/git-issue) in
Rust. I will keep the differences in output and command line arguments to a minimum.
Current goal is to make it a drop in replacement.

## Differences

- `new` accepts multiple tags with `-t, --tags` and a milestone with `-m`,
  `--milestone`
- [ ] `new` creates the issue in branch later merged with `--no-ff` this should
  be a configurable option.

## Status

- Start an issue repository
  - [ ] `clone` Clone the specified remote repository.
  - [x] `init` Create a new issues repository in the current directory.

- Work with an issue
  - [x] `new` Create a new open issue
  - [ ] `show` Show specified issue (and its comments with `-c`).
  - [ ] `comment` Add an issue comment.
  - [ ] `edit` Edit the specified issue's (or comment's with `-c`) description
  - [x] `tag` Add (or remove with `-r`) a tag.
  - [x] `milestone` Edit issue's milestone.
  - [ ] `weight` Edit issue's weight.
  - [ ] `duedate` Edit issue's due date.
  - [ ] `timeestimate` Edit time estimate for this issue.
  - [ ] `timespent` Edit time spent working on an issue so far.
  - [ ] `assign` Assign (or remove `-r`) an issue to a person.
  - [ ] `attach` Attach (or remove with `-r`) a file to an issue.
  - [ ] `watcher` Add (or remove with `-r`) an issue watcher.
  - [x] `close` Remove the open tag, add the closed tag

- Show multiple issues
  - [ ] `list` List open issues (or all with -a).
  - [ ]  `list -l FORMATSTRING` This will list issues in the specified format,
    given as an argument to `-l`.

- Work with multiple issues
  - [ ] `filter-apply COMMAND` Run command in every issue directory. The
    following environment variables will be set:

- Synchronize with remote repositories
  - [ ] `push` Update remote Git repository with local changes.
  - [ ] `pull` Update local Git repository with remote changes.
  - [ ] `import` Import/update GitHub/GitLab issues from the specified project.
  - [ ] `create` Create the issue in the provided GitHub repository.
  - [ ] `export` Export modified issues for the specified project.
  - [ ] `exportall` Export all open issues in the database (`-a` to include closed
    ones) to GitHub/GitLab. Useful for cloning whole repositories.

## Milestones

### 1 Core

1. [ ] `cancel`
1. [ ] `close`
1. [ ] `comment`
1. [ ] `init`
1. [ ] `new`
   1. [x] Command line usage
   1. [ ] Opening editor.
1. [ ] `list` - only show open or all (`-a`) issues
1. [ ] `show`
1. [ ] `tag`
1. [ ] `validate`
   1. [ ] Find issues with invalid ids

### 2 Extended

1. [ ] `milestone`
1. [ ] `weight`
1. [ ] `duedate`
1. [ ] `timeestimate`
1. [ ] `timespent`
1. [ ] `assign`
1. [ ] `attach`
1. [ ] `watcher`
1. [ ] `filter-apply`

### 3 Remote

Synchronisation with remote repositories.
