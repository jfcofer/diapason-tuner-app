#!/usr/bin/env bash
# The JVM tooling facts doctor.sh and ktfmt.sh must agree on, in one place. Sourced, not run, after
# tools/versions.env.
#
# find_jdk: the JDK this repo's Java tools should use.
# The order is the one Gradle effectively follows for a Flutter build: `flutter config --jdk-dir`,
# then JAVA_HOME, then PATH. A machine's default `java` is often the JDK 25 that breaks Flutter
# Android builds (flutter/flutter#187223), so PATH comes last.
#
# Prints "<java binary><TAB><where it came from>", or nothing when no JDK is found:
#   IFS=$'\t' read -r java_bin java_src < <(find_jdk)

find_jdk() {
    local flutter_jdk=''
    if [[ -f "$HOME/.config/flutter/settings" ]] && command -v python3 >/dev/null 2>&1; then
        flutter_jdk="$(python3 -c '
import json, sys
try:
    print(json.load(open(sys.argv[1])).get("jdk-dir", ""))
except Exception:
    print("")
' "$HOME/.config/flutter/settings" 2>/dev/null)"
    fi

    if [[ -n "$flutter_jdk" && -x "$flutter_jdk/bin/java" ]]; then
        printf '%s\t%s\n' "$flutter_jdk/bin/java" "flutter config --jdk-dir"
    elif [[ -n "${JAVA_HOME:-}" && -x "${JAVA_HOME}/bin/java" ]]; then
        printf '%s\t%s\n' "$JAVA_HOME/bin/java" "JAVA_HOME"
    elif command -v java >/dev/null 2>&1; then
        printf '%s\t%s\n' "$(command -v java)" "PATH"
    fi
}

# Where the pinned ktfmt jar lives: the user cache, named by version, so worktrees share it and a
# changed pin never runs a stale jar (adr/0023).
ktfmt_jar() { printf '%s\n' "${XDG_CACHE_HOME:-$HOME/.cache}/diapason/ktfmt-$KTFMT_VERSION-with-dependencies.jar"; }

# SHA-256 of a file, with GNU coreutils or the macOS tool; empty when neither exists, which never
# matches a pin.
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" 2>/dev/null | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" 2>/dev/null | cut -d' ' -f1
    fi
}
