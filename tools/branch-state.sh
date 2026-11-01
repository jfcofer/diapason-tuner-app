#!/usr/bin/env bash
# What session-start.sh and session-end.sh both need to know about a branch (T-022). Sourced, not run.
# Reads local refs only: no network call, so a verdict is as fresh as the last `git fetch`.

# The commits on $1 that origin/main lacks. main only accepts rebase-merges, which rewrite every SHA,
# so ancestry calls a merged branch unmerged forever; `git cherry` compares patches instead. `grep -c`
# reads all its input: under pipefail, `grep -q` could stop early and kill `git cherry` with SIGPIPE.
commits_not_in_main() {
    git cherry origin/main "$1" 2>/dev/null | grep -c '^+'
}

# Whether local branch $1 holds work the remote has never seen. Three traps:
# - `%(upstream:track)` is empty both for a branch in sync with its upstream and for one with none.
# - `git checkout -b x origin/main` sets origin/main as x's upstream, so an upstream alone proves
#   nothing: only one other than origin/main means the branch went somewhere of its own.
# - A branch pushed without `-u` has no upstream and is still on the remote, so check that ref too.
never_pushed() {
    local upstream
    upstream="$(git for-each-ref --format='%(upstream)' "refs/heads/$1")"
    [[ -z "$upstream" || "$upstream" == refs/remotes/origin/main ]] || return 1
    git rev-parse --verify --quiet "refs/remotes/origin/$1" >/dev/null && return 1
    [[ "$(commits_not_in_main "$1")" != 0 ]]
}
