#!/usr/bin/env bash
# Re-issue historical release tags with series-prefixed three-grade names.
# Dry-run by default; run from a clean main checkout with --apply and every
# authoritative remote named explicitly to perform the migration.
set -euo pipefail

started=$SECONDS
apply=""
remotes=()

usage() {
    echo "usage: bash deploy/retag.sh [--apply] [remote ...]"
}

die() {
    echo "retag: $*" >&2
    exit 1
}

timed() {
    local label="$1" began=$SECONDS status=0
    shift
    "$@" || status=$?
    printf 'retag: %s: %ss\n' "$label" "$((SECONDS - began))" >&2
    return "$status"
}

verify_tag() {
    git verify-tag "$1" >/dev/null 2>&1
}

for arg in "$@"; do
    case "$arg" in
        --apply) apply=1 ;;
        --help|-h) usage; exit 0 ;;
        --*) die "unknown option: $arg" ;;
        *) remotes+=("$arg") ;;
    esac
done

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"
git rev-parse --git-dir >/dev/null 2>&1 || die "$repo is not a git checkout"

if [ -n "$apply" ]; then
    [ "$(git symbolic-ref --quiet --short HEAD)" = "main" ] || die "--apply must run from main"
    [ -z "$(git status --porcelain)" ] || die "--apply requires a clean checkout"
    [ "${#remotes[@]}" -gt 0 ] || die "--apply requires at least one remote"
fi

for remote in "${remotes[@]}"; do
    timed "inspect remote $remote" git remote get-url "$remote" >/dev/null || die "unknown remote: $remote"
done

new_for() {
    local old="$1"
    if [[ "$old" =~ ^v([0-9]+)$ ]]; then
        echo "arena-${BASH_REMATCH[1]}.0.0"
    elif [[ "$old" =~ ^([a-z0-9-]+)-v([0-9]+)$ ]]; then
        echo "${BASH_REMATCH[1]}-${BASH_REMATCH[2]}.0.0"
    elif [[ "$old" =~ ^[a-z0-9-]+-[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        return 1
    else
        die "tag '$old' does not match a historical or three-grade release name"
    fi
}

olds=()
news=()
targets=()
states=()
while IFS= read -r old; do
    [ -n "$old" ] || continue
    if ! new="$(new_for "$old")"; then
        continue
    fi
    target="$(git rev-parse "refs/tags/$old^{commit}")"
    state="create"
    if git show-ref --verify --quiet "refs/tags/$new"; then
        existing="$(git rev-parse "refs/tags/$new^{commit}")"
        [ "$existing" = "$target" ] || die "$new already points at $existing, expected $target from $old"
        [ "$(git cat-file -t "refs/tags/$new")" = "tag" ] || die "$new exists at the right commit but is not annotated"
        timed "verify existing $new" verify_tag "$new" || die "$new exists at the right commit but is not signed by a trusted key"
        state="skip-existing"
    fi
    olds+=("$old")
    news+=("$new")
    targets+=("$target")
    states+=("$state")
done < <(git tag --list --sort=version:refname)

for index in "${!olds[@]}"; do
    printf '%s -> %s @ %s [%s]\n' "${olds[$index]}" "${news[$index]}" "${targets[$index]}" "${states[$index]}"
done

if [ -z "$apply" ]; then
    echo "retag: dry-run; ${#olds[@]} historical tag(s), ${#remotes[@]} remote(s), $((SECONDS - started))s"
    exit 0
fi

work="$(mktemp -d -t ember-retag-XXXXXX)" || die "unable to create temporary directory"
trap 'rm -r -- "$work"' EXIT

for index in "${!olds[@]}"; do
    [ "${states[$index]}" = "create" ] || continue
    old="${olds[$index]}"
    new="${news[$index]}"
    target="${targets[$index]}"
    message="$work/$index.message"
    if [ "$(git cat-file -t "refs/tags/$old")" = "tag" ]; then
        git for-each-ref --format='%(contents:subject)%0a%0a%(contents:body)' "refs/tags/$old" > "$message"
    else
        git log -1 --format=%B "$target" > "$message"
    fi
    printf '\nReplaces historical tag: %s\n' "$old" >> "$message"
    timed "create $new" git tag -s -F "$message" "$new" "$target"
done

objects=()
for index in "${!news[@]}"; do
    new="${news[$index]}"
    target="${targets[$index]}"
    [ "$(git cat-file -t "refs/tags/$new")" = "tag" ] || die "$new is not an annotated tag after creation"
    [ "$(git rev-parse "refs/tags/$new^{commit}")" = "$target" ] || die "$new moved from its expected target after creation"
    timed "verify $new" verify_tag "$new" || die "new tag $new did not verify after signing"
    objects[$index]="$(git rev-parse "refs/tags/$new")"
done

for remote in "${remotes[@]}"; do
    for index in "${!news[@]}"; do
        old="${olds[$index]}"
        new="${news[$index]}"
        target="${targets[$index]}"
        object="${objects[$index]}"

        remote_refs="$(timed "inspect $remote replacement $new" git ls-remote --tags "$remote" "refs/tags/$new" "refs/tags/$new^{}")"
        direct="$(printf '%s\n' "$remote_refs" | awk '$2 !~ /\^\{\}$/ { print $1 }')"
        if [ -n "$direct" ]; then
            peeled="$(printf '%s\n' "$remote_refs" | awk '$2 ~ /\^\{\}$/ { print $1 }')"
            [ -n "$peeled" ] || die "$remote already has non-annotated or unreadable tag $new"
            [ "$direct" = "$object" ] || die "$remote already has $new object $direct, expected $object"
            [ "$peeled" = "$target" ] || die "$remote already has $new at $peeled, expected $target"
        fi

        remote_refs="$(timed "inspect $remote historical $old" git ls-remote --tags "$remote" "refs/tags/$old" "refs/tags/$old^{}")"
        direct="$(printf '%s\n' "$remote_refs" | awk '$2 !~ /\^\{\}$/ { print $1 }')"
        [ -n "$direct" ] || continue
        peeled="$(printf '%s\n' "$remote_refs" | awk '$2 ~ /\^\{\}$/ { print $1 }')"
        remote_target="${peeled:-$direct}"
        [ "$remote_target" = "$target" ] || die "$remote has historical tag $old at $remote_target, expected $target"
    done
done

for remote in "${remotes[@]}"; do
    for index in "${!news[@]}"; do
        new="${news[$index]}"
        object="${objects[$index]}"
        remote_refs="$(timed "inspect $remote before pushing $new" git ls-remote --tags "$remote" "refs/tags/$new" "refs/tags/$new^{}")"
        direct="$(printf '%s\n' "$remote_refs" | awk '$2 !~ /\^\{\}$/ { print $1 }')"
        if [ -n "$direct" ]; then
            [ "$direct" = "$object" ] || die "$remote acquired conflicting $new object $direct, expected $object"
            echo "retag: $remote already has the exact $new tag object; skipping"
            continue
        fi
        timed "push $new to $remote" git push "$remote" "refs/tags/$new:refs/tags/$new"
    done
done

delete_remotes=()
delete_olds=()
for remote in "${remotes[@]}"; do
    for index in "${!olds[@]}"; do
        old="${olds[$index]}"
        target="${targets[$index]}"
        remote_refs="$(timed "reinspect $remote historical $old" git ls-remote --tags "$remote" "refs/tags/$old" "refs/tags/$old^{}")"
        direct="$(printf '%s\n' "$remote_refs" | awk '$2 !~ /\^\{\}$/ { print $1 }')"
        [ -n "$direct" ] || continue
        peeled="$(printf '%s\n' "$remote_refs" | awk '$2 ~ /\^\{\}$/ { print $1 }')"
        remote_target="${peeled:-$direct}"
        [ "$remote_target" = "$target" ] || die "$remote moved historical tag $old to $remote_target, expected $target"
        delete_remotes+=("$remote")
        delete_olds+=("$old")
    done
done

for remote in "${remotes[@]}"; do
    for index in "${!news[@]}"; do
        new="${news[$index]}"
        target="${targets[$index]}"
        object="${objects[$index]}"
        remote_refs="$(timed "verify $remote replacement $new before deletion" git ls-remote --tags "$remote" "refs/tags/$new" "refs/tags/$new^{}")"
        direct="$(printf '%s\n' "$remote_refs" | awk '$2 !~ /\^\{\}$/ { print $1 }')"
        peeled="$(printf '%s\n' "$remote_refs" | awk '$2 ~ /\^\{\}$/ { print $1 }')"
        [ "$direct" = "$object" ] || die "$remote has $new object ${direct:-missing} before deletion, expected $object"
        [ "$peeled" = "$target" ] || die "$remote has $new at ${peeled:-unreadable} before deletion, expected $target"
    done
done

for index in "${!delete_olds[@]}"; do
    remote="${delete_remotes[$index]}"
    old="${delete_olds[$index]}"
    timed "delete $old from $remote" git push "$remote" ":refs/tags/$old"
done

for old in "${olds[@]}"; do
    timed "delete local $old" git tag -d "$old"
done

echo "retag: applied ${#olds[@]} historical tag(s) to ${#remotes[@]} remote(s) in $((SECONDS - started))s"
