#!/usr/bin/env bash
# The steps of .github/workflows/release.yml, runnable locally the same way:
#
#   scripts/release.sh version                       print the workspace version
#   scripts/release.sh check-tag v1.0.0              the tag must name that version
#   scripts/release.sh notes v1.0.0 [changelog]      print the draft release's text
#   scripts/release.sh package <target> <directory>  build, smoke-test and archive
#   scripts/release.sh archive <target> <binary> <directory>
#                                                    archive a binary that is built
#   scripts/release.sh install-check <target> <directory>
#                                                    install the archive as a user would
#   scripts/release.sh checksums v1.0.0 <directory>  write SHA256SUMS and VERSION
#
# Targets bash 3.2 (macOS's /bin/bash) and tools both runner families have.
set -euo pipefail
cd "$(dirname "$0")/.."

# The archives install.sh and `aether update` ask for; keep the three in step.
TARGETS="x86_64-unknown-linux-musl aarch64-unknown-linux-musl aarch64-apple-darwin x86_64-apple-darwin"
VERSION_PATTERN='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'
INSTALL_COMMAND='curl -fsSL https://raw.githubusercontent.com/mengsig/aether/main/install.sh | sh'

fail() {
  printf 'release.sh: error: %s\n' "$*" >&2
  exit 1
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$@"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$@"
  else
    fail "neither sha256sum nor shasum is installed"
  fi
}

# The version every crate inherits from [workspace.package] in Cargo.toml.
workspace_version() {
  local id version
  id=$(cargo pkgid --locked -p aether)
  version=${id##*[#@]}
  [[ "$version" =~ $VERSION_PATTERN ]] \
    || fail "the workspace version '$version' is not MAJOR.MINOR.PATCH; install.sh and \`aether update\` accept nothing else"
  printf '%s\n' "$version"
}

check_tag_form() {
  [[ "$1" == v* && "${1#v}" =~ $VERSION_PATTERN ]] || fail "tag '$1' is not of the form v1.2.3"
}

check_tag() {
  [ "$#" -eq 1 ] || fail "usage: release.sh check-tag <tag>"
  local tag=$1 version
  check_tag_form "$tag"
  version=$(workspace_version)
  [ "$tag" = "v$version" ] \
    || fail "tag $tag is not the workspace version: Cargo.toml says $version, so the tag must be v$version"
  printf 'tag %s is the workspace version\n' "$tag"
}

# The text of the draft release: the tag's section of the changelog, without
# its heading, then how to install. Fails when the section is missing or empty.
notes() {
  if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    fail "usage: release.sh notes <tag> [changelog]"
  fi
  local tag=$1 changelog=${2:-CHANGELOG.md} version section
  check_tag_form "$tag"
  version=${tag#v}
  [ -f "$changelog" ] || fail "$changelog does not exist; the release notes are its '## $version' section"
  section=$(awk -v heading="## $version" '
    found && /^## / { exit }
    found { print }
    $0 == heading { found = 1 }
  ' "$changelog")
  # Blank lines around the section are not part of it.
  section=$(printf '%s\n' "$section" | sed '/[^[:space:]]/,$!d')
  [ -n "$section" ] \
    || fail "$changelog has no section headed exactly '## $version' with text under it; write it before tagging $tag"
  cat <<EOF
$section

## Install

\`\`\`sh
$INSTALL_COMMAND
\`\`\`

Put \`AETHER_VERSION=$tag\` before \`sh\` to install exactly this release. An installed copy updates itself with \`aether update\`.
EOF
}

is_release_target() {
  local target
  for target in $TARGETS; do
    [ "$target" = "$1" ] && return 0
  done
  return 1
}

# Runs on a machine of the target's own kind, so the binary can be executed.
package() {
  [ "$#" -eq 2 ] || fail "usage: release.sh package <target> <directory>"
  local target=$1 out=$2 version binary reported
  is_release_target "$target" || fail "'$target' is not a release target; they are: $TARGETS"
  version=$(workspace_version)
  case "$target" in
    *-linux-musl)
      command -v musl-gcc >/dev/null 2>&1 \
        || fail "musl-gcc is not installed (Ubuntu: apt-get install musl-tools)"
      # SQLite and the TLS library are C: they must be compiled against musl too.
      export "CC_${target//-/_}=musl-gcc"
      ;;
  esac
  cargo build --release --locked -p aether --target "$target"
  binary="${CARGO_TARGET_DIR:-target}/$target/release/aether"
  if [ ! -f "$binary" ] || [ ! -x "$binary" ]; then
    fail "the build left no executable at $binary"
  fi
  reported=$("$binary" --version)
  [ "$reported" = "aether $version" ] \
    || fail "$binary reports '$reported', not 'aether $version'"
  case "$target" in
    *-linux-musl) check_static "$binary" ;;
  esac
  archive "$target" "$binary" "$out"
}

# Packs a binary named `aether` with the licence and the third-party notices.
archive() {
  [ "$#" -eq 3 ] || fail "usage: release.sh archive <target> <binary> <directory>"
  local target=$1 binary=$2 out=$3 name member
  is_release_target "$target" || fail "'$target' is not a release target; they are: $TARGETS"
  if [ ! -f "$binary" ] || [ ! -x "$binary" ]; then
    fail "$binary is not an executable file"
  fi
  [ "$(basename "$binary")" = aether ] || fail "$binary is not named aether, as install.sh asks for it"
  for member in LICENSE THIRD-PARTY-NOTICES; do
    [ -s "$member" ] || fail "$member is missing or empty; every archive carries it"
  done
  name="aether-$target.tar.gz"
  mkdir -p "$out"
  # Members are named bare, as install.sh asks for them. COPYFILE_DISABLE
  # keeps macOS tar from adding ._* members.
  COPYFILE_DISABLE=1 tar -czf "$out/$name" -C "$(dirname "$binary")" aether \
    -C "$PWD" LICENSE THIRD-PARTY-NOTICES
  (cd "$out" && sha256 "$name" >"$name.sha256")
  printf 'packaged %s (%s bytes)\n' "$out/$name" "$(wc -c <"$out/$name" | tr -d ' ')"
}

# Runs on a machine of the target's own kind: install.sh picks its archive by the host.
install_check() {
  [ "$#" -eq 2 ] || fail "usage: release.sh install-check <target> <directory>"
  is_release_target "$1" || fail "'$1' is not a release target; they are: $TARGETS"
  command -v python3 >/dev/null 2>&1 || fail "python3 is needed to serve the archive to install.sh"
  python3 scripts/release_install_check.py "$1" "$2" "$(workspace_version)"
}

# A musl binary that still needs a loader would not run on a glibc-only host.
check_static() {
  local description
  description=$(file -b "$1")
  case "$description" in
    *"statically linked"* | *"static-pie linked"*) ;;
    *) fail "$1 is not statically linked: $description" ;;
  esac
}

checksums() {
  [ "$#" -eq 2 ] || fail "usage: release.sh checksums <tag> <directory>"
  local tag=$1 dir=$2 target archive
  check_tag_form "$tag"
  [ -d "$dir" ] || fail "$dir is not a directory"
  cd "$dir"
  : >SHA256SUMS
  for target in $TARGETS; do
    archive="aether-$target.tar.gz"
    if [ ! -s "$archive" ] || [ ! -s "$archive.sha256" ]; then
      fail "$dir holds no $archive with its .sha256"
    fi
    sha256 -c "$archive.sha256" >/dev/null || fail "$archive does not match $archive.sha256"
    cat "$archive.sha256" >>SHA256SUMS
  done
  # `aether update` reads the newest tag from this asset of the latest release.
  printf '%s\n' "$tag" >VERSION
  cat SHA256SUMS
}

[ "$#" -ge 1 ] || fail "usage: release.sh version | check-tag <tag> | notes <tag> [changelog] | package <target> <directory> | archive <target> <binary> <directory> | install-check <target> <directory> | checksums <tag> <directory>"
command=$1
shift
case "$command" in
  version) workspace_version ;;
  check-tag) check_tag "$@" ;;
  notes) notes "$@" ;;
  package) package "$@" ;;
  archive) archive "$@" ;;
  install-check) install_check "$@" ;;
  checksums) checksums "$@" ;;
  *) fail "unknown command '$command'" ;;
esac
