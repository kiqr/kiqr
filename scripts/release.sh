#!/bin/sh
# Cut a KIQR release: bump the version, write the changelog, commit, tag and
# push. The tag push starts .github/workflows/release.yml, which builds every
# platform and publishes the GitHub Release.
#
#   scripts/release.sh patch      0.2.1 -> 0.2.2   fixes only
#   scripts/release.sh minor      0.2.1 -> 0.3.0   new features (or, before 1.0, a breaking change)
#   scripts/release.sh major      0.2.1 -> 1.0.0
#   scripts/release.sh rc         0.2.1 -> 0.2.2-rc.1, 0.2.2-rc.1 -> 0.2.2-rc.2   a pre-release
#   scripts/release.sh 0.4.0-rc.1 an exact version
#
# patch/minor/major on an rc finalizes it when that is the same release
# (0.3.0-rc.2 + minor -> 0.3.0). Add --dry-run to see the result and change nothing.
set -eu

die() { printf 'release: %s\n' "$*" >&2; exit 1; }

cd "$(dirname "$0")/.."
bump=""
dry=0
for a in "$@"; do
  case "$a" in
    --dry-run|-n) dry=1 ;;
    -h|--help) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) [ -z "$bump" ] || die "one version argument, please"; bump="$a" ;;
  esac
done
[ -n "$bump" ] || die "usage: scripts/release.sh <patch|minor|major|rc|X.Y.Z[-rc.N]> [--dry-run]"

current=$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version *= *"\(.*\)"/\1/p;}' Cargo.toml)
[ -n "$current" ] || die "no version in Cargo.toml [workspace.package]"

core=${current%%-*}
pre=""
[ "$core" = "$current" ] || pre=${current#*-}
major=${core%%.*}; rest=${core#*.}; minor=${rest%%.*}; patch=${rest#*.}

case "$bump" in
  patch) if [ -n "$pre" ]; then next="$core"; else next="$major.$minor.$((patch + 1))"; fi ;;
  minor) if [ -n "$pre" ] && [ "$patch" = 0 ]; then next="$core"; else next="$major.$((minor + 1)).0"; fi ;;
  major) if [ -n "$pre" ] && [ "$minor.$patch" = "0.0" ]; then next="$core"; else next="$((major + 1)).0.0"; fi ;;
  rc)
    case "$pre" in
      rc.*) next="$core-rc.$((${pre#rc.} + 1))" ;;
      "") next="$major.$minor.$((patch + 1))-rc.1" ;;
      *) die "don't know how to bump the pre-release $pre" ;;
    esac ;;
  [0-9]*.[0-9]*.[0-9]*) next="${bump#v}" ;;
  *) die "unknown bump: $bump" ;;
esac
echo "$next" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$' || die "not a version: $next"
tag="v$next"

# The changelog entry: what is under "## Unreleased" in CHANGELOG.md when
# someone wrote it by hand, else the commit subjects since the last release.
last=$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)
unreleased=""
[ -f CHANGELOG.md ] && unreleased=$(awk '/^## /{ if (on) exit; if ($0 ~ /^## Unreleased/) { on = 1; next } } on' CHANGELOG.md | sed '/./,$!d')
if [ -n "$unreleased" ]; then
  notes=$unreleased
elif [ -n "$last" ]; then
  notes=$(git log --no-merges --format='- %s' "$last..HEAD" | grep -v -e '^- \[auto\]' -e '^- Release v' -e '^- Rebuild the embedded web UI' || true)
fi
[ -n "${notes:-}" ] || notes="- Maintenance release."

section=$(printf '## %s — %s\n\n%s\n' "$tag" "$(date +%Y-%m-%d)" "$notes")

printf 'KIQR %s -> %s (%s)\n\n%s\n' "$current" "$next" "${last:-first release}" "$section"
if [ "$dry" = 1 ]; then
  echo
  echo "(dry run: nothing changed)"
  exit 0
fi

[ -z "$(git status --porcelain)" ] || die "the working tree has changes; commit or stash them first"
branch=$(git rev-parse --abbrev-ref HEAD)
[ "$branch" = main ] || die "releases are cut from main (you are on $branch)"
git fetch --quiet origin main --tags
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "main is not in step with origin/main; pull or push first"
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "$tag already exists"

# The version, everywhere Cargo keeps it.
sed -i.bak '/^\[workspace.package\]/,/^\[/s/^version *= *".*"/version = "'"$next"'"/' Cargo.toml && rm Cargo.toml.bak
cargo update --workspace --quiet

# The embedded UI, so `cargo install --git` builds the same thing the release does.
(cd web && npm ci --silent && npm run build --silent)

# The changelog, newest first: header and intro, an empty Unreleased for next
# time, this release, then the ones before it.
if [ -f CHANGELOG.md ]; then
  intro=$(awk '/^## /{ exit } { print }' CHANGELOG.md)
  older=$(awk '/^## Unreleased/ { skip = 1; next } /^## / { skip = 0; on = 1 } on && !skip' CHANGELOG.md)
else
  intro='# Changelog'
  older=""
fi
printf '%s\n\n## Unreleased\n\n%s\n\n%s\n' "$intro" "$section" "$older" | cat -s > CHANGELOG.md

git add Cargo.toml Cargo.lock CHANGELOG.md web/dist
git commit --quiet -m "Release $tag"
git tag -a "$tag" -m "KIQR $tag"
git push --quiet origin main "$tag"

echo
echo "Pushed $tag. The release workflow is building it:"
echo "  https://github.com/kiqr/kiqr/actions/workflows/release.yml"
