#!/usr/bin/env bash
# Create or update GitHub releases from series-prefixed release tags.
# Dry-run by default. Bulk apply runs from clean main; --tag applies one tag
# from a clean checkout at its target, which is the release workflow's path.
# Use --replace-drafts only in bulk mode for the superseded v20/v22 drafts.
#
#   bash deploy/github-releases.sh [--tag TAG] [--replace-drafts]
#   bash deploy/github-releases.sh --apply [--tag TAG] [--replace-drafts]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO="$DEFAULT_REPO"
CHANGELOG="$REPO/CHANGELOG.md"
GAMES="$REPO/web/games.json"
GH_BIN="${GH:-gh}"
LATEST_TAG=""
GITHUB_RELEASES_WORK=""

usage() {
    echo "usage: bash deploy/github-releases.sh [--apply] [--tag TAG] [--replace-drafts]"
}

die() {
    echo "github-releases: $*" >&2
    exit 1
}

timed() {
    local label="$1" began=$SECONDS status=0
    shift
    "$@" || status=$?
    printf 'github-releases: %s: %ss\n' "$label" "$((SECONDS - began))" >&2
    return "$status"
}

valid_tag() {
    [[ "$1" =~ ^[a-z0-9-]+-[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

release_tags() {
    local tag
    while IFS= read -r tag; do
        valid_tag "$tag" && printf '%s\n' "$tag"
    done < <(git -C "$REPO" tag -l --sort=version:refname)
}

latest_arena_tag() {
    local tag newest=""
    while IFS= read -r tag; do
        valid_tag "$tag" && newest="$tag"
    done < <(git -C "$REPO" tag -l 'arena-*' --sort=version:refname)
    printf '%s\n' "$newest"
}

launcher_title_for() {
    local series="$1"
    awk -v wanted="$series" '
        function value(line) {
            sub(/^[^:]+:[[:space:]]*"/, "", line)
            sub(/",?[[:space:]]*$/, "", line)
            return line
        }
        /^[[:space:]]*"id":[[:space:]]*/ { id = value($0) }
        id == wanted && /^[[:space:]]*"title":[[:space:]]*/ {
            print value($0)
            exit
        }
    ' "$GAMES"
}

launcher_path_for_slot() {
    local series="$1" slot="$2"
    awk -v wanted="$series" -v wanted_slot="$slot" '
        function value(line) {
            sub(/^[^:]+:[[:space:]]*"/, "", line)
            sub(/",?[[:space:]]*$/, "", line)
            return line
        }
        /^[[:space:]]*"id":[[:space:]]*/ {
            id = value($0)
            selected = 0
        }
        id == wanted && /^[[:space:]]*"v":[[:space:]]*/ {
            selected = value($0) == wanted_slot
        }
        id == wanted && selected && /^[[:space:]]*"path":[[:space:]]*/ {
            print value($0)
            exit
        }
    ' "$GAMES"
}

launcher_path_for_version() {
    local series="$1" version="$2"
    awk -v wanted="$series" -v wanted_version="$version" '
        function value(line) {
            sub(/^[^:]+:[[:space:]]*"/, "", line)
            sub(/",?[[:space:]]*$/, "", line)
            return line
        }
        /^[[:space:]]*"id":[[:space:]]*/ {
            id = value($0)
            selected = 0
        }
        id == wanted && /^[[:space:]]*"version":[[:space:]]*/ {
            selected = value($0) == wanted_version
        }
        id == wanted && selected && /^[[:space:]]*"path":[[:space:]]*/ {
            print value($0)
            exit
        }
    ' "$GAMES"
}

series_title_for() {
    local series="$1" title
    title="$(launcher_title_for "$series")"
    if [ -n "$title" ]; then
        printf '%s\n' "$title"
        return
    fi
    case "$series" in
        ember) printf 'Ember\n' ;;
        *)
            [ -d "$REPO/web/labs/$series" ] || return 0
            printf '%s Lab\n' "${series^}"
            ;;
    esac
}

series_path_for() {
    local series="$1"
    case "$series" in
        ember) printf './\n' ;;
        *) [ ! -d "$REPO/web/labs/$series" ] || printf 'labs/%s/\n' "$series" ;;
    esac
}

changelog_entry_for() {
    local tag="$1"
    awk -v wanted="$tag" '
        function emit() {
            sub(/\n+$/, "", entry)
            printf "%s\n", entry
            emitted = 1
        }
        /^## / || /^### / {
            if (entry != "" && matched) {
                emit()
                exit
            }
            entry = ""
            matched = 0
            if ($0 ~ /^### /) {
                entry = $0 ORS
            }
            next
        }
        entry != "" {
            entry = entry $0 ORS
            if (index($0, "tag `" wanted "`")) {
                matched = 1
            }
        }
        END {
            if (!emitted && entry != "" && matched) {
                emit()
            }
        }
    ' "$CHANGELOG"
}

tag_annotation_for() {
    local tag="$1"
    [ "$(git -C "$REPO" cat-file -t "refs/tags/$tag")" = "tag" ] || return 1
    git -C "$REPO" for-each-ref \
        --format='%(contents:subject)%0a%0a%(contents:body)' "refs/tags/$tag"
}

derive_release() {
    local tag="$1" notes="$2"
    local series version entry slot source_commit annotation tag_commit title

    valid_tag "$tag" || die "tag '$tag' does not match the release tag grammar"
    series="${tag%-*}"
    version="${tag##*-}"

    title="$(series_title_for "$series")"
    [ -n "$title" ] || die "no release title for series '$series'"
    RELEASE_TITLE="$title $version"
    RELEASE_LATEST=false
    [ "$tag" = "$LATEST_TAG" ] && RELEASE_LATEST=true
    RELEASE_PRERELEASE=false
    [[ "$version" =~ ^0\. ]] && RELEASE_PRERELEASE=true

    entry="$(changelog_entry_for "$tag")"
    RELEASE_ENTRY="$entry"
    RELEASE_HAS_ENTRY=false
    RELEASE_LINE=""
    if [ -n "$entry" ]; then
        RELEASE_HAS_ENTRY=true
        RELEASE_LINE="$(printf '%s\n' "$entry" | awk 'NR > 1 && NF { print; exit }')"
        slot="$(printf '%s\n' "$entry" | awk 'NR == 1 { print $2 }')"
        RELEASE_PATH="$(launcher_path_for_slot "$series" "$slot")"
        [ -n "$RELEASE_PATH" ] || RELEASE_PATH="$(series_path_for "$series")"
        [ -n "$RELEASE_PATH" ] || die "$tag entry uses slot '$slot', which has no release path"
        source_commit="$(printf '%s\n' "$RELEASE_LINE" | sed -nE 's/.*source `([0-9a-f]{7,40})`.*/\1/p')"
        if [ -z "$source_commit" ]; then
            source_commit="$(git -C "$REPO" rev-parse "refs/tags/$tag^{commit}")"
        fi
        {
            printf 'Tag `%s` · source commit `%s`\n\n' "$tag" "$source_commit"
            printf '%s\n\n' "$entry"
            printf 'Launcher path: `%s`\n' "$RELEASE_PATH"
        } > "$notes"
    else
        tag_commit="$(git -C "$REPO" rev-parse "refs/tags/$tag^{commit}")"
        RELEASE_PATH="$(launcher_path_for_version "$series" "$version")"
        [ -n "$RELEASE_PATH" ] || RELEASE_PATH="$(series_path_for "$series")"
        [ -n "$RELEASE_PATH" ] || die "$tag has no changelog entry or release path"
        annotation="$(tag_annotation_for "$tag")" || die "$tag has no changelog entry and is not annotated"
        [ -n "$annotation" ] || die "$tag has no changelog entry or annotation message"
        {
            printf 'Tag `%s` · source commit `%s`\n\n' "$tag" "$tag_commit"
            printf 'No matching `CHANGELOG.md` entry exists for `%s`.\n\n' "$tag"
            printf 'Tag annotation message:\n\n%s\n\n' "$annotation"
            printf 'Launcher path: `%s`\n' "$RELEASE_PATH"
        } > "$notes"
    fi
}

print_command() {
    local label="$1"
    shift
    printf 'github-releases: %s:' "$label"
    printf ' %q' "$@"
    printf '\n'
}

release_exists() {
    "$GH_BIN" release view "$1" >/dev/null 2>&1
}

inspect_draft() {
    "$GH_BIN" release view "$1" --json name,tagName,isDraft \
        --jq '[.name, .tagName, .isDraft] | @tsv'
}

replace_stale_drafts() {
    local apply="$1" old_tag title details actual_title actual_tag is_draft
    local -a stale=(
        'v22|v22 - skies and weather'
        'v20|v20 - the realism pass'
    )
    RELEASE_DELETES=0

    for details in "${stale[@]}"; do
        old_tag="${details%%|*}"
        title="${details#*|}"
        if [ -z "$apply" ]; then
            echo "github-releases: plan delete draft '$title' only if its title and draft state match"
            print_command "inspect draft $title" "$GH_BIN" release view "$old_tag" \
                --json name,tagName,isDraft --jq '[.name, .tagName, .isDraft] | @tsv'
            print_command "delete confirmed draft $title" "$GH_BIN" release delete "$old_tag" --yes
            continue
        fi

        if ! details="$(timed "inspect draft $title" inspect_draft "$old_tag")"; then
            echo "github-releases: stale draft '$title' is absent; skipping"
            continue
        fi
        IFS=$'\t' read -r actual_title actual_tag is_draft <<< "$details"
        if [ "$actual_title" != "$title" ] || [ "$actual_tag" != "$old_tag" ] || [ "$is_draft" != "true" ]; then
            echo "github-releases: retained $old_tag: expected draft '$title', got title '$actual_title' and draft '$is_draft'" >&2
            continue
        fi
        timed "delete draft $title" "$GH_BIN" release delete "$old_tag" --yes
        RELEASE_DELETES=$((RELEASE_DELETES + 1))
    done
}

main() {
    local started=$SECONDS apply="" replace_drafts="" selected_tag="" tag notes target
    local creates=0 updates=0 release_count=0
    local work
    local -a tags create_args edit_args

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --apply) apply=1 ;;
            --replace-drafts) replace_drafts=1 ;;
            --tag)
                shift
                [ "$#" -gt 0 ] || die "--tag requires a tag name"
                [ -z "$selected_tag" ] || die "--tag may be supplied only once"
                selected_tag="$1"
                ;;
            --help|-h) usage; exit 0 ;;
            --*) die "unknown option: $1" ;;
            *) die "unexpected argument: $1" ;;
        esac
        shift
    done

    git -C "$REPO" rev-parse --git-dir >/dev/null 2>&1 || die "$REPO is not a git checkout"
    [ -f "$CHANGELOG" ] || die "no CHANGELOG.md at $CHANGELOG"
    [ -f "$GAMES" ] || die "no web/games.json at $GAMES"
    [ -z "$selected_tag" ] || [ -z "$replace_drafts" ] || die "--replace-drafts is available only in bulk mode"
    if [ -n "$apply" ]; then
        [ -z "$(git -C "$REPO" status --porcelain)" ] || die "--apply requires a clean checkout"
        if [ -n "$selected_tag" ]; then
            target="$(git -C "$REPO" rev-parse "refs/tags/$selected_tag^{commit}" 2>/dev/null)" \
                || die "tag '$selected_tag' does not resolve to a commit"
            [ "$(git -C "$REPO" rev-parse HEAD)" = "$target" ] || die "--apply --tag requires HEAD at $selected_tag's target"
        else
            [ "$(git -C "$REPO" symbolic-ref --quiet --short HEAD)" = "main" ] || die "bulk --apply must run from main"
        fi
    fi

    if [ -n "$selected_tag" ]; then
        valid_tag "$selected_tag" || die "tag '$selected_tag' does not match the release tag grammar"
        git -C "$REPO" rev-parse -q --verify "refs/tags/$selected_tag" >/dev/null \
            || die "tag '$selected_tag' does not exist"
        tags=("$selected_tag")
    else
        mapfile -t tags < <(release_tags)
    fi
    [ "${#tags[@]}" -gt 0 ] || die "no series-prefixed release tags found"
    LATEST_TAG="$(latest_arena_tag)"
    [ -n "$LATEST_TAG" ] || die "no arena release tag found to mark latest"

    work="$(mktemp -d "${TMPDIR:?}/ember-github-releases.XXXXXX")"
    GITHUB_RELEASES_WORK="$work"
    trap '[ -z "$GITHUB_RELEASES_WORK" ] || rm -r -- "$GITHUB_RELEASES_WORK"' EXIT

    if [ -n "$replace_drafts" ]; then
        replace_stale_drafts "$apply"
    else
        RELEASE_DELETES=0
    fi

    for tag in "${tags[@]}"; do
        notes="$work/$tag.md"
        timed "derive $tag" derive_release "$tag" "$notes"
        release_count=$((release_count + 1))
        create_args=(release create "$tag" --verify-tag --title "$RELEASE_TITLE" \
            --notes-file "$notes" --draft=false --prerelease="$RELEASE_PRERELEASE" \
            --latest="$RELEASE_LATEST")
        edit_args=(release edit "$tag" --title "$RELEASE_TITLE" --notes-file "$notes" \
            --draft=false --prerelease="$RELEASE_PRERELEASE" --latest="$RELEASE_LATEST")

        if [ -z "$apply" ]; then
            echo "github-releases: plan release $tag title='$RELEASE_TITLE' latest=$RELEASE_LATEST prerelease=$RELEASE_PRERELEASE"
            print_command "inspect $tag" "$GH_BIN" release view "$tag"
            print_command "create $tag if absent" "$GH_BIN" "${create_args[@]}"
            print_command "update $tag if present" "$GH_BIN" "${edit_args[@]}"
        elif timed "inspect $tag" release_exists "$tag"; then
            timed "update $tag" "$GH_BIN" "${edit_args[@]}"
            updates=$((updates + 1))
        else
            timed "create $tag" "$GH_BIN" "${create_args[@]}"
            creates=$((creates + 1))
        fi
    done

    if [ -n "$apply" ]; then
        echo "github-releases: applied $release_count release(s): $creates created, $updates updated, $RELEASE_DELETES stale draft(s) deleted in $((SECONDS - started))s"
    else
        echo "github-releases: dry-run; $release_count release(s), $RELEASE_DELETES confirmed deletion(s), $((SECONDS - started))s"
    fi
}

if [[ "${BASH_SOURCE[0]}" = "$0" ]]; then
    main "$@"
fi
