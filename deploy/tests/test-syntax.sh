#!/usr/bin/env bash
# `bash -n` over every script in deploy/, including the tests and the shims.
#
#   bash deploy/tests/test-syntax.sh
#
# The cheapest test there is, and the one that would have caught the class of
# breakage that matters most here: a deploy script is only ever run when
# something is already wrong, and a syntax error in it is discovered at the
# worst possible moment.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEPLOY="$(cd "$HERE/.." && pwd)"
# shellcheck source=deploy/tests/lib.sh
. "$HERE/lib.sh"

# The shims have no extension on purpose — they stand in for `ssh`, `scp`,
# `cargo`, `wasm-bindgen`, `git`, `curl` and `sleep` on PATH — so they are
# listed rather than globbed.
FILES="$(find "$DEPLOY" -name '*.sh' -print; ls "$HERE"/shims/ssh "$HERE"/shims/scp "$HERE"/shims/cargo "$HERE"/shims/wasm-bindgen "$HERE"/shims/git "$HERE"/shims/curl "$HERE"/shims/sleep)"

while IFS= read -r f; do
    [ -n "$f" ] || continue
    if bash -n "$f" 2>"$HERE/.syntax-err"; then
        ok "${f#"$DEPLOY"/}"
    else
        bad "${f#"$DEPLOY"/}: $(cat "$HERE/.syntax-err")"
    fi
done <<< "$FILES"
rm -f "$HERE/.syntax-err"

echo "== every script says how it is run =="
# A deploy script nobody can invoke from memory is a deploy script nobody
# invokes. Each one carries its own usage line in its header comment.
for f in "$DEPLOY"/*.sh; do
    if head -20 "$f" | grep -qF "$(basename "$f")"; then
        ok "${f#"$DEPLOY"/} documents itself"
    else
        bad "${f#"$DEPLOY"/} has no usage line in its first 20 lines"
    fi
done

echo "== no CRLF anywhere =="
# .gitattributes forces LF on *.sh, but a shim has no extension and a script
# written by a Windows tool would still land here. A CR at the end of the
# shebang makes the whole file unrunnable on the host it is meant for.
while IFS= read -r f; do
    [ -n "$f" ] || continue
    if grep -qU $'\r' "$f" 2>/dev/null; then
        bad "${f#"$DEPLOY"/} contains CR"
    else
        ok "${f#"$DEPLOY"/} is LF-only"
    fi
done <<< "$FILES"

summary syntax
