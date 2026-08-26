#!/usr/bin/env bash
# Rename the app and its bundle ID across every layer of the tree.
#
#   tools/rename.sh "New Name" com.example.newname
#
# Touches: Android applicationId/namespace and package dirs, iOS bundle identifier, the launcher
# labels in every flavour's string resources and InfoPlist.strings, the Dart app package name and
# every import of it, flavour JSON titles, and the docs that quote the bundle ID.
#
# Rust crate names are NOT renamed: they are `diapason_*` internal identifiers referenced only by
# path, never published, and renaming them buys nothing (docs/REPO_LAYOUT.md).
#
# WARNING: changing the bundle ID of an app that has already shipped does not rename it, it creates
# a new app. Existing users do not upgrade. See docs/adr/0013.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

if [[ $# -ne 2 ]]; then
    echo "usage: tools/rename.sh <display-name> <bundle-id>" >&2
    echo "   e.g. tools/rename.sh \"Diapason\" dev.jfcofer.diapason" >&2
    exit 1
fi

NEW_NAME="$1"
NEW_ID="$2"

if ! [[ "$NEW_ID" =~ ^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$ ]]; then
    echo "✗ '$NEW_ID' is not a valid bundle identifier." >&2
    echo "  Lowercase reverse-DNS, at least two segments, each starting with a letter." >&2
    echo "  Android rejects segments starting with a digit, and every identifier must be ASCII." >&2
    exit 1
fi

# The ASCII slug used for package/directory names, derived from the bundle ID's last segment.
NEW_SLUG="${NEW_ID##*.}"

OLD_ID="$(grep -oP 'applicationId = "\K[^"]+' apps/diapason/android/app/build.gradle.kts | head -1)"
OLD_SLUG="$(grep -oP '^name: \K.+' apps/diapason/pubspec.yaml)"

if [[ -z "$OLD_ID" || -z "$OLD_SLUG" ]]; then
    echo "✗ could not determine the current name or bundle ID. Is the scaffold intact?" >&2
    exit 1
fi

echo "  $OLD_SLUG -> $NEW_SLUG"
echo "  $OLD_ID -> $NEW_ID"
echo "  display name -> $NEW_NAME"

files() {
    find . -type f \
        \( -name '*.dart' -o -name '*.yaml' -o -name '*.kts' -o -name '*.gradle' -o -name '*.xml' \
           -o -name '*.plist' -o -name '*.strings' -o -name '*.pbxproj' -o -name '*.json' \
           -o -name '*.md' -o -name '*.podspec' -o -name 'justfile' \) \
        -not -path './.git/*' -not -path '*/build/*' -not -path '*/.dart_tool/*' \
        -not -path './target/*' -not -path '*/cargokit/*' -not -path '*/Pods/*'
}

# 1. Bundle ID (longest first, so the flavour suffixes are handled by the base replacement).
files | xargs -r sed -i "s|${OLD_ID}|${NEW_ID}|g"

# 2. Dart package name and every import of it.
files | xargs -r sed -i "s|package:${OLD_SLUG}/|package:${NEW_SLUG}/|g"
sed -i "s|^name: ${OLD_SLUG}$|name: ${NEW_SLUG}|" apps/diapason/pubspec.yaml

# 3. Launcher labels. The base name only - flavour suffixes are preserved.
OLD_NAME="$(grep -oP '<string name="app_name">\K[^<]+' \
    apps/diapason/android/app/src/prod/res/values/strings.xml 2>/dev/null || echo '')"
if [[ -n "$OLD_NAME" ]]; then
    find apps/diapason/android/app/src -name strings.xml -print0 |
        xargs -0 -r sed -i "s|>${OLD_NAME}|>${NEW_NAME}|g"
    find apps/diapason/ios -name 'InfoPlist.strings' -print0 2>/dev/null |
        xargs -0 -r sed -i "s|\"${OLD_NAME}|\"${NEW_NAME}|g"
    # Flavour titles carried through --dart-define-from-file.
    find apps/diapason/flavors -name '*.json' -print0 |
        xargs -0 -r sed -i "s|\"${OLD_NAME}|\"${NEW_NAME}|g"
fi

# 4. Android source-set package directories, if the namespace changed shape.
old_path="${OLD_ID//./\/}"
new_path="${NEW_ID//./\/}"
for root in apps/diapason/android/app/src/*/kotlin apps/diapason/android/app/src/*/java; do
    [[ -d "$root/$old_path" ]] || continue
    mkdir -p "$(dirname "$root/$new_path")"
    git mv "$root/$old_path" "$root/$new_path" 2>/dev/null || mv "$root/$old_path" "$root/$new_path"
done

# 5. The app directory itself.
if [[ "$OLD_SLUG" != "$NEW_SLUG" && -d "apps/$OLD_SLUG" ]]; then
    git mv "apps/$OLD_SLUG" "apps/$NEW_SLUG" 2>/dev/null || mv "apps/$OLD_SLUG" "apps/$NEW_SLUG"
    sed -i "s|apps/${OLD_SLUG}|apps/${NEW_SLUG}|g" pubspec.yaml justfile
fi

echo
echo "✓ renamed. Now run, in this order:"
echo "    flutter pub get && just gen && just verify"
echo "  and check that nothing missed it:"
echo "    git grep -n '${OLD_ID}\\|${OLD_SLUG}'"
