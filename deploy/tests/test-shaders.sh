#!/usr/bin/env bash
# Enforce Julibrot's runtime-template boundary and its closed migration debt.
#
#   bash deploy/tests/test-shaders.sh
#   bash deploy/tests/test-shaders.sh --self-test
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
ALLOWLIST="$HERE/julibrot-shader-allowlist.txt"
VALIDATIONS="$HERE/julibrot-shader-validation-tests.txt"
POLICY="$ROOT/docs/julibrot/shaders.md"
PYTHON_BIN="${PYTHON:-python3}"

declare -A ALLOWED=()
declare -A DETECTED=()
declare -A PRODUCTION_TEMPLATES=()
declare -A TEMPLATE_FILES=()
declare -A VALIDATED_TEMPLATES=()
declare -A VALIDATION_CRATES=()
declare -A VALIDATION_MANIFESTS=()
declare -A VALIDATION_TESTS=()
allowlist_count=0
template_count=0
production_template_count=0
validation_count=0
compiled_validation_count=0
normal_validation_ms=0
probe_validation_ms=0
rendered_shader_lowering_count=0
failures=0

ceiling_contains() {
    local record="$1"
    grep -Fqx -- "$record" <<'CEILING'
# R3-04 landed after this lane's base: one production source and one test-only source/lowering pair.
inline|crates/labs/julibrot/present/src/shader.rs|HEAP_SCENE_PREFIX|JB-PRESENT-SCENE
inline|crates/labs/julibrot/present/src/shader.rs|SCENE_BODY|JB-PRESENT-SCENE
inline|crates/labs/julibrot/present/src/shader.rs|GLITCH_COUNT_BODY|JB-PRESENT-SCENE
inline|crates/labs/julibrot/present/src/warp_shader.rs|WARP_SHADER|JB-PRESENT-WARP
inline|crates/labs/julibrot/present/src/gpu/device/tests.rs|SOURCE|JB-PRESENT-NATIVE-TEST
inline|crates/labs/julibrot/app/src/frame/loop/tests.rs|TEMPLATE|JB-APP-PAIRED-TEST
inline|crates/labs/julibrot/kernels/src/gpu.rs|RECONSTRUCTION_BODY|JB-KERNEL-RECONSTRUCTION
lowering|crates/labs/julibrot/present/src/gpu/device/scene.rs|create_scene_pipeline|JB-PRESENT-SCENE
lowering|crates/labs/julibrot/present/src/gpu/device/tests.rs|native_split_value_pipeline|JB-PRESENT-NATIVE-TEST
lowering|crates/labs/julibrot/present/src/gpu/device/warp.rs|create_warp_pipeline|JB-PRESENT-WARP
lowering|crates/labs/julibrot/app/src/frame/loop/tests.rs|paired_gpu_readback_pipelines|JB-APP-PAIRED-TEST
file|crates/labs/julibrot/kernels/src/shallow.wgsl|-|JB-KERNEL-SHALLOW
file|crates/labs/julibrot/kernels/src/perturb.wgsl|-|JB-KERNEL-PERTURB
CEILING
}

report_failure() {
    printf '%s\n' "$1" >&2
    failures=1
}

load_allowlist() {
    local allowlist="$1"
    local policy="$2"
    local line_number=0 record kind path symbol row key
    local -a fields=()

    if [ ! -f "$allowlist" ]; then
        report_failure "missing Julibrot shader allowlist: $allowlist"
        return
    fi
    if [ ! -f "$policy" ]; then
        report_failure "missing Julibrot shader policy: $policy"
        return
    fi

    while IFS= read -r record || [ -n "$record" ]; do
        line_number=$((line_number + 1))
        case "$record" in
            ''|'#'*) continue ;;
        esac
        fields=()
        IFS='|' read -r -a fields <<< "$record"
        if [ "${#fields[@]}" -ne 4 ]; then
            report_failure "malformed shader allowlist line $line_number: $record"
            continue
        fi
        kind="${fields[0]}"
        path="${fields[1]}"
        symbol="${fields[2]}"
        row="${fields[3]}"
        if [[ "$kind" != "file" && "$kind" != "inline" && "$kind" != "lowering" ]] \
            || [[ "$path" != crates/labs/julibrot/* ]] \
            || [[ ! "$symbol" =~ ^(-|[A-Za-z_][A-Za-z0-9_]*)$ ]] \
            || [[ ! "$row" =~ ^JB-[A-Z0-9-]+$ ]] \
            || { [ "$kind" = "file" ] && [ "$symbol" != "-" ]; } \
            || { [ "$kind" != "file" ] && [ "$symbol" = "-" ]; }
        then
            report_failure "invalid shader allowlist line $line_number: $record"
            continue
        fi
        if ! ceiling_contains "$record"; then
            report_failure "shader allowlist may only shrink; remove unapproved record: $record"
            continue
        fi
        if ! grep -Fq -- "$row" "$policy"; then
            report_failure "shader allowlist row is absent from policy: $row"
            continue
        fi
        key="$kind|$path|$symbol"
        if [ -n "${ALLOWED[$key]+present}" ]; then
            report_failure "duplicate shader allowlist record: $key"
            continue
        fi
        ALLOWED["$key"]="$row"
        allowlist_count=$((allowlist_count + 1))
    done < "$allowlist"
}

strip_rust_non_code() {
    awk '
        function spaces(text, blank) {
            blank = text
            gsub(/./, " ", blank)
            return blank
        }

        function char_literal_length(text, quote_at, content_at, value, escape, closing, candidate, brace, digits) {
            if (substr(text, 1, 2) == ("b" apostrophe)) {
                quote_at = 2
            } else if (substr(text, 1, 1) == apostrophe) {
                quote_at = 1
            } else {
                return 0
            }

            content_at = quote_at + 1
            value = substr(text, content_at, 1)
            if (value == "" || value == apostrophe || value == "\\") {
                if (value != "\\") {
                    return 0
                }
                escape = substr(text, content_at + 1, 1)
                if (escape == "n" || escape == "r" || escape == "t" || escape == "0" \
                    || escape == "\\" || escape == apostrophe || escape == "\"")
                {
                    closing = content_at + 2
                } else if (escape == "x" \
                    && substr(text, content_at + 2, 1) ~ /^[[:xdigit:]]$/ \
                    && substr(text, content_at + 3, 1) ~ /^[[:xdigit:]]$/)
                {
                    closing = content_at + 4
                } else if (escape == "u" && substr(text, content_at + 2, 1) == "{") {
                    candidate = substr(text, content_at + 2)
                    brace = index(candidate, "}")
                    if (brace < 3) {
                        return 0
                    }
                    digits = substr(candidate, 2, brace - 2)
                    if (digits !~ /^[[:xdigit:]_]+$/) {
                        return 0
                    }
                    closing = content_at + brace + 2
                } else {
                    return 0
                }
            } else {
                closing = content_at + 1
            }

            if (substr(text, closing, 1) != apostrophe) {
                return 0
            }
            return closing
        }

        BEGIN {
            apostrophe = sprintf("%c", 39)
            block_depth = 0
            in_string = 0
            raw_end = ""
        }

        {
            line = $0
            output = ""
            cursor = 1
            while (cursor <= length(line)) {
                character = substr(line, cursor, 1)
                pair = substr(line, cursor, 2)
                rest = substr(line, cursor)

                if (block_depth > 0) {
                    if (pair == "/*") {
                        block_depth++
                        output = output "  "
                        cursor += 2
                    } else if (pair == "*/") {
                        block_depth--
                        output = output "  "
                        cursor += 2
                    } else {
                        output = output " "
                        cursor++
                    }
                } else if (raw_end != "") {
                    if (substr(line, cursor, length(raw_end)) == raw_end) {
                        output = output spaces(raw_end)
                        cursor += length(raw_end)
                        raw_end = ""
                    } else {
                        output = output " "
                        cursor++
                    }
                } else if (in_string) {
                    if (character == "\\") {
                        escaped = substr(line, cursor, 2)
                        output = output spaces(escaped)
                        cursor += length(escaped)
                    } else {
                        output = output " "
                        cursor++
                        if (character == "\"") {
                            in_string = 0
                        }
                    }
                } else if (pair == "//") {
                    output = output spaces(rest)
                    cursor = length(line) + 1
                } else if (pair == "/*") {
                    block_depth = 1
                    output = output "  "
                    cursor += 2
                } else if ((literal_length = char_literal_length(rest)) > 0) {
                    token = substr(rest, 1, literal_length)
                    output = output spaces(token)
                    cursor += literal_length
                } else if (match(rest, /^r#*\"/)) {
                    hashes = substr(rest, 2, RLENGTH - 2)
                    raw_end = "\"" hashes
                    token = substr(rest, 1, RLENGTH)
                    output = output spaces(token)
                    cursor += RLENGTH
                } else if (character == "\"") {
                    in_string = 1
                    output = output " "
                    cursor++
                } else {
                    output = output character
                    cursor++
                }
            }
            print output
        }
    ' "$1"
}

count_validation_invocations() {
    local source="$1"
    local invocation_pattern="$2"
    local flattened

    flattened="$(strip_rust_non_code "$source" | tr '\n' ' ')"
    pairing_live_count="$({ grep -o -E -- "$invocation_pattern" <<< "$flattened" || :; } | awk 'END { print NR }')"
}

validation_identifier_lines() {
    local source="$1"
    local identifier="$2"

    strip_rust_non_code "$source" | awk -v identifier="$identifier" '
        {
            rest = $0
            pattern = "(^|[^[:alnum:]_])" identifier "([^[:alnum:]_]|$)"
            while (match(rest, pattern)) {
                print NR
                rest = substr(rest, RSTART + RLENGTH)
            }
        }
    '
}

validation_function_lines() {
    local source="$1"
    local identifier="$2"

    strip_rust_non_code "$source" | awk -v identifier="$identifier" '
        {
            pattern = "(^|[^[:alnum:]_])fn[[:space:]]+" identifier "[[:space:]]*\\("
            if ($0 ~ pattern) {
                print NR
            }
        }
    '
}

cfg_validation_invocation_lines() {
    local source="$1"

    strip_rust_non_code "$source" | awk '
        function trim(text) {
            sub(/^[[:space:]]+/, "", text)
            sub(/[[:space:]]+$/, "", text)
            return text
        }

        {
            line = trim($0)
            if (attribute_open) {
                attributes = attributes " " line
                attribute_end = index(line, "]")
                if (attribute_end == 0) {
                    next
                }
                attribute_open = 0
                line = trim(substr(line, attribute_end + 1))
            }
            while (line ~ /^#\[/) {
                attribute_end = index(line, "]")
                if (attribute_end == 0) {
                    attributes = attributes " " line
                    attribute_open = 1
                    next
                }
                attributes = attributes " " substr(line, 1, attribute_end)
                line = trim(substr(line, attribute_end + 1))
            }
            if (line == "") {
                next
            }
            if (index(line, "::ember_julibrot_shader::production_template_test!") != 0 \
                && attributes ~ /(^|[^[:alnum:]_])cfg(_attr)?[[:space:]]*\(/)
            {
                print NR
            }
            attributes = ""
        }
    '
}

validation_extern_shadow_records() {
    local source="$1"

    strip_rust_non_code "$source" | "$PYTHON_BIN" -c '
import re
import sys

source = sys.stdin.read()
checks = (
    (
        "extern",
        re.compile(
            r"(?<![A-Za-z0-9_])extern\s+crate\s+[^\s;]+\s+as\s+"
            r"(?:r#)?ember_julibrot_shader\s*;"
        ),
    ),
    (
        "macro",
        re.compile(
            r"(?<![A-Za-z0-9_])macro_rules\s*!\s*"
            r"(?:r#)?production_template_test\b"
        ),
    ),
)
for kind, pattern in checks:
    for match in pattern.finditer(source):
        line = source.count("\n", 0, match.start()) + 1
        print(f"{kind}|{line}")
'
}

validation_test_function() {
    printf 'production_template_%s_renders_and_validates' "$1"
}

validation_test_name() {
    local test_path="$1"
    local render_function="$2"
    local crate_root relative module function

    crate_root="${test_path%%/src/*}"
    relative="${test_path#"$crate_root/src/"}"
    module="${relative%.rs}"
    case "$module" in
        lib|main) module="" ;;
        */mod) module="${module%/mod}" ;;
    esac
    module="${module//\//::}"
    function="$(validation_test_function "$render_function")"
    if [ -n "$module" ]; then
        printf '%s::production_validation::%s' "$module" "$function"
    else
        printf 'production_validation::%s' "$function"
    fi
}

require_shader_dependency_identity() {
    local metadata="$1"
    local owning_crate="$2"
    local owning_manifest="$3"
    local shader_manifest="$4"

    "$PYTHON_BIN" -c '
import json
import os
import sys

owning_crate, owning_manifest, shader_manifest = sys.argv[1:]


def canonical(path):
    return os.path.normcase(os.path.realpath(path))


def reject(reason):
    print(f"shader validation dependency identity mismatch: {reason}", file=sys.stderr)
    raise SystemExit(1)


try:
    document = json.load(sys.stdin)
except (json.JSONDecodeError, OSError) as error:
    reject(f"cargo metadata is unreadable: {error}")

packages = document.get("packages")
workspace_members = document.get("workspace_members")
resolve = document.get("resolve")
if not isinstance(packages, list) or not isinstance(workspace_members, list) or not isinstance(resolve, dict):
    reject("cargo metadata omits packages, workspace members or the resolved graph")

owners = [
    package
    for package in packages
    if package.get("name") == owning_crate
    and canonical(package.get("manifest_path", "")) == canonical(owning_manifest)
]
if len(owners) != 1 or owners[0].get("id") not in workspace_members:
    reject(f"could not identify owning workspace package {owning_crate}")

shaders = [
    package
    for package in packages
    if package.get("name") == "ember-julibrot-shader"
    and canonical(package.get("manifest_path", "")) == canonical(shader_manifest)
]
if len(shaders) != 1 or shaders[0].get("id") not in workspace_members:
    reject(
        "workspace package ember-julibrot-shader is not "
        "crates/labs/julibrot/shader/Cargo.toml"
    )

owner_nodes = [node for node in resolve.get("nodes", []) if node.get("id") == owners[0]["id"]]
if len(owner_nodes) != 1:
    reject(f"resolved graph omits owning package {owning_crate}")

extern_dependencies = [
    dependency
    for dependency in owner_nodes[0].get("deps", [])
    if dependency.get("name") == "ember_julibrot_shader"
]
if len(extern_dependencies) != 1 or extern_dependencies[0].get("pkg") != shaders[0]["id"]:
    reject(
        f"{owning_crate} extern ember_julibrot_shader does not resolve to workspace "
        "package ember-julibrot-shader at crates/labs/julibrot/shader/Cargo.toml"
    )
' "$owning_crate" "$owning_manifest" "$shader_manifest" <<< "$metadata"
}

load_validation_pairs() {
    local repo="$1"
    local validations="$2"
    local line_number=0 record template test_path template_symbol render_function
    local key runtime template_name invocation_pattern invocation_count global_invocation_count
    local expected_function expected_test crate_root manifest owning_crate source_path
    local identifier_count identifier_locations occurrence_line function_line cfg_line crate_source
    local shadow_kind shadow_line
    local -a fields=()

    if [ ! -f "$validations" ]; then
        report_failure "missing Julibrot shader validation pairs: $validations"
        return
    fi
    runtime="$repo/crates/labs/julibrot/shader/src/runtime.rs"
    if [ ! -f "$runtime" ]; then
        report_failure "missing Julibrot shader runtime registry: $runtime"
        return
    fi

    while IFS= read -r record || [ -n "$record" ]; do
        line_number=$((line_number + 1))
        case "$record" in
            ''|'#'*) continue ;;
        esac
        fields=()
        IFS='|' read -r -a fields <<< "$record"
        if [ "${#fields[@]}" -ne 4 ]; then
            report_failure "malformed shader validation line $line_number: $record"
            continue
        fi
        template="${fields[0]}"
        test_path="${fields[1]}"
        template_symbol="${fields[2]}"
        render_function="${fields[3]}"
        if [[ "$template" != crates/labs/julibrot/shader/templates/*.wgsl.jinja ]] \
            || [[ "$template" = *-test.wgsl.jinja ]] \
            || [[ "$test_path" != crates/labs/julibrot/*.rs ]] \
            || [[ ! "$template_symbol" =~ ^[A-Z][A-Z0-9_]*$ ]] \
            || [[ ! "$render_function" =~ ^[a-z][a-z0-9_]*$ ]]
        then
            report_failure "invalid shader validation line $line_number: $record"
            continue
        fi
        key="$template"
        if [ -n "${VALIDATED_TEMPLATES[$key]+present}" ]; then
            report_failure "duplicate shader validation pair: $template"
            continue
        fi
        if [ -z "${PRODUCTION_TEMPLATES[$template]+present}" ]; then
            report_failure "stale shader validation pair for non-production template: $template"
            continue
        fi
        template_name="${template##*/}"
        if ! grep -Fq -- "pub const $template_symbol: &str = \"$template_name\";" "$runtime" \
            || ! grep -Fq -- "($template_symbol," "$runtime"
        then
            report_failure "production template is absent from the runtime registry: $template"
        fi
        if [ ! -f "$repo/$test_path" ]; then
            report_failure "shader validation test source is absent: $test_path"
        else
            expected_function="$(validation_test_function "$render_function")"
            expected_test="$(validation_test_name "$test_path" "$render_function")"
            crate_root="${test_path%%/src/*}"
            invocation_pattern="(^|[^[:alnum:]_:])::ember_julibrot_shader::production_template_test![[:space:]]*\([[:space:]]*$template_symbol[[:space:]]*,[[:space:]]*$render_function[[:space:]]*,[[:space:]]*$expected_function[[:space:]]*,?[[:space:]]*\)[[:space:]]*;"
            count_validation_invocations "$repo/$test_path" "$invocation_pattern"
            invocation_count="$pairing_live_count"
            global_invocation_count=0
            while IFS= read -r source_path; do
                count_validation_invocations "$repo/$source_path" "$invocation_pattern"
                global_invocation_count=$((global_invocation_count + pairing_live_count))
                while IFS= read -r cfg_line; do
                    [ -n "$cfg_line" ] || continue
                    report_failure "production validation macro has a cfg attribute: $source_path:$cfg_line"
                done < <(cfg_validation_invocation_lines "$repo/$source_path")
            done < <(
                git -C "$repo" grep -l -F 'production_template_test!' \
                    -- 'crates/labs/julibrot/**/*.rs' 2>/dev/null || :
            )
            if [ "$invocation_count" -ne 1 ] || [ "$global_invocation_count" -ne 1 ]; then
                report_failure "production template requires exactly one lexical validation macro: $test_path:$template_symbol,$render_function,$expected_function"
            fi
            identifier_count=0
            identifier_locations=""
            while IFS= read -r crate_source; do
                [ -n "$crate_source" ] || continue
                while IFS= read -r occurrence_line; do
                    [ -n "$occurrence_line" ] || continue
                    identifier_count=$((identifier_count + 1))
                    identifier_locations+="$crate_source:$occurrence_line"$'\n'
                done < <(validation_identifier_lines "$repo/$crate_source" "$expected_function")
                while IFS= read -r function_line; do
                    [ -n "$function_line" ] || continue
                    report_failure "hand-written production validation test is forbidden: $crate_source:$function_line:$expected_function"
                done < <(validation_function_lines "$repo/$crate_source" "$expected_function")
                while IFS='|' read -r shadow_kind shadow_line; do
                    [ -n "$shadow_kind" ] || continue
                    case "$shadow_kind" in
                        extern)
                            report_failure "production validation extern name is rebound: $crate_source:$shadow_line"
                            ;;
                        macro)
                            report_failure "production validation macro is redefined: $crate_source:$shadow_line"
                            ;;
                    esac
                done < <(validation_extern_shadow_records "$repo/$crate_source")
            done < <(git -C "$repo" ls-files "$crate_root/**/*.rs")
            if [ "$identifier_count" -ne 1 ]; then
                report_failure "derived validation identifier must occur exactly once in the owning crate: $test_path:$expected_function"
                while IFS= read -r source_path; do
                    [ -n "$source_path" ] || continue
                    report_failure "derived validation identifier occurrence: $source_path:$expected_function"
                done <<< "$identifier_locations"
            fi
            manifest="$repo/$crate_root/Cargo.toml"
            if [ ! -f "$manifest" ]; then
                report_failure "shader validation test crate manifest is absent: ${manifest#"$repo/"}"
            else
                owning_crate="$(sed -n -E 's/^name[[:space:]]*=[[:space:]]*"([a-z0-9-]+)".*/\1/p' "$manifest" | head -n 1)"
                if [ -z "$owning_crate" ]; then
                    report_failure "shader validation test crate has no package name: ${manifest#"$repo/"}"
                else
                    VALIDATION_CRATES["$key"]="$owning_crate"
                    VALIDATION_MANIFESTS["$key"]="$crate_root/Cargo.toml"
                    VALIDATION_TESTS["$key"]="$expected_test"
                fi
            fi
        fi
        VALIDATED_TEMPLATES["$key"]="$test_path:$template_symbol,$render_function"
        validation_count=$((validation_count + 1))
    done < "$validations"
}

require_compiled_validation_test() {
    local template="$1"
    local expected_test="$2"
    local listed_tests="$3"
    local listed_count

    listed_count="$(awk -v expected="$expected_test: test" '$0 == expected { count++ } END { print count + 0 }' <<< "$listed_tests")"
    if [ "$listed_count" -ne 1 ]; then
        printf 'production template %s is missing compiled validation test %s\n' "$template" "$expected_test" >&2
        return 1
    fi
}

require_validation_probe_receipt() {
    local template="$1"
    local nonce="$2"
    local output="$3"
    local marker="SHADER-VALIDATION-PROBE $nonce ${template##*/} "
    local identifier="ember_shader_validation_probe_missing"
    local receipt_count diagnostic_count

    read -r receipt_count diagnostic_count < <(
        awk -v marker="$marker" -v identifier="$identifier" '
            {
                marker_at = index($0, marker)
                if (marker_at != 0) {
                    receipts++
                    suffix = substr($0, marker_at + length(marker))
                    if (index(suffix, identifier) != 0) {
                        diagnostics++
                    }
                }
            }
            END { print receipts + 0, diagnostics + 0 }
        ' <<< "$output"
    )
    if [ "$receipt_count" -ne 1 ] || [ "$diagnostic_count" -ne 1 ]; then
        printf 'production template %s validation probe must report exactly one naga diagnostic naming %s\n' \
            "$template" "$identifier" >&2
        return 1
    fi
}

check_compiled_validation_tests() {
    local repo="$1"
    local metadata template owning_crate owning_manifest expected_test listed_tests identity_error
    local run_started run_finished probe_nonce probe_output probe_error
    local -A listings=()
    local -A listing_failures=()

    if ! metadata="$(cd "$repo" && cargo metadata --locked --format-version 1)"; then
        report_failure "could not read locked workspace metadata for shader validation"
        return 1
    fi

    while IFS= read -r template; do
        [ -n "$template" ] || continue
        owning_crate="${VALIDATION_CRATES[$template]:-}"
        owning_manifest="${VALIDATION_MANIFESTS[$template]:-}"
        expected_test="${VALIDATION_TESTS[$template]:-}"
        if [ -z "$owning_crate" ] || [ -z "$owning_manifest" ] || [ -z "$expected_test" ]; then
            report_failure "production template has no compiled validation identity: $template"
            continue
        fi
        if ! identity_error="$(require_shader_dependency_identity \
            "$metadata" "$owning_crate" "$repo/$owning_manifest" \
            "$repo/crates/labs/julibrot/shader/Cargo.toml" 2>&1)"
        then
            report_failure "$identity_error"
            continue
        fi
        if [ -z "${listings[$owning_crate]+present}" ] \
            && [ -z "${listing_failures[$owning_crate]+present}" ]
        then
            if listed_tests="$(cd "$repo" && cargo test -p "$owning_crate" --lib -- --list)"; then
                listings["$owning_crate"]="$listed_tests"
            else
                report_failure "could not list native tests for shader validation crate: $owning_crate"
                listing_failures["$owning_crate"]=1
            fi
        fi
        if [ -n "${listing_failures[$owning_crate]+present}" ]; then
            continue
        fi
        if ! require_compiled_validation_test \
            "$template" "$expected_test" "${listings[$owning_crate]}"
        then
            failures=1
        fi
    done < <(printf '%s\n' "${!PRODUCTION_TEMPLATES[@]}" | sort)

    [ "$failures" -eq 0 ] || return 1
    while IFS= read -r template; do
        [ -n "$template" ] || continue
        owning_crate="${VALIDATION_CRATES[$template]}"
        expected_test="${VALIDATION_TESTS[$template]}"
        run_started="$(date +%s%N)"
        if ! (cd "$repo" && env -u EMBER_SHADER_VALIDATION_PROBE \
            cargo test -p "$owning_crate" --lib -- \
            --exact "$expected_test" --include-ignored)
        then
            report_failure "compiled shader validation test failed: $owning_crate $expected_test"
            continue
        fi
        run_finished="$(date +%s%N)"
        normal_validation_ms=$((normal_validation_ms + (run_finished - run_started) / 1000000))

        probe_nonce="$$-$(date +%s%N)-$RANDOM"
        run_started="$(date +%s%N)"
        if probe_output="$(cd "$repo" && env EMBER_SHADER_VALIDATION_PROBE="$probe_nonce" \
            cargo test -p "$owning_crate" --lib -- \
            --exact "$expected_test" --include-ignored --nocapture 2>&1)"
        then
            run_finished="$(date +%s%N)"
            printf '%s\n' "$probe_output"
        else
            run_finished="$(date +%s%N)"
            printf '%s\n' "$probe_output" >&2
            report_failure "compiled shader validation probe failed: $owning_crate $expected_test"
            continue
        fi
        probe_validation_ms=$((probe_validation_ms + (run_finished - run_started) / 1000000))
        if ! probe_error="$(require_validation_probe_receipt \
            "$template" "$probe_nonce" "$probe_output" 2>&1)"
        then
            report_failure "$probe_error"
            continue
        fi
        compiled_validation_count=$((compiled_validation_count + 1))
    done < <(printf '%s\n' "${!PRODUCTION_TEMPLATES[@]}" | sort)
    [ "$failures" -eq 0 ]
}

record_detected() {
    local kind="$1"
    local path="$2"
    local symbol="$3"
    local key="$kind|$path|$symbol"

    if [ -n "${DETECTED[$key]+present}" ]; then
        report_failure "ambiguous duplicate shader source: $key"
        return
    fi
    DETECTED["$key"]=1
}

scan_shader_files() {
    local repo="$1"
    local path

    while IFS= read -r path; do
        case "$path" in
            crates/labs/julibrot/shader/templates/*.wgsl.jinja)
                TEMPLATE_FILES["$path"]=1
                template_count=$((template_count + 1))
                ;;
            crates/labs/julibrot/*.wgsl|crates/labs/julibrot/*.wgsl.jinja)
                record_detected "file" "$path" "-"
                ;;
        esac
    done < <(git -C "$repo" ls-files)
}

scan_production_registry() {
    local repo="$1"
    local runtime match entry symbol source_symbol template_name template

    runtime="$repo/crates/labs/julibrot/shader/src/runtime.rs"
    if [ ! -f "$runtime" ]; then
        report_failure "missing Julibrot shader runtime registry: $runtime"
        return
    fi
    while IFS= read -r match; do
        entry="${match#(}"
        entry="${entry%)}"
        symbol="${entry%%,*}"
        symbol="${symbol//[[:space:]]/}"
        source_symbol="${entry#*,}"
        source_symbol="${source_symbol//[[:space:]]/}"
        if [[ ! "$symbol" =~ ^[A-Z][A-Z0-9_]*_TEMPLATE$ ]] \
            || [[ ! "$source_symbol" =~ ^[A-Z][A-Z0-9_]*_SOURCE$ ]]
        then
            report_failure "invalid production template registry entry: $match"
            continue
        fi
        template_name="$(sed -n "s/^pub const $symbol: &str = \"\([^\"]*\.wgsl\.jinja\)\";$/\1/p" "$runtime")"
        if [ -z "$template_name" ]; then
            report_failure "production template registry symbol has no public template name: $symbol"
            continue
        fi
        template="crates/labs/julibrot/shader/templates/$template_name"
        if [ -n "${PRODUCTION_TEMPLATES[$template]+present}" ]; then
            report_failure "duplicate production template registry entry: $template"
            continue
        fi
        if [ -z "${TEMPLATE_FILES[$template]+present}" ]; then
            report_failure "production template registry source is absent: $template"
        fi
        if [[ "$template" = *-test.wgsl.jinja ]]; then
            report_failure "test-only template entered the production registry: $template"
        fi
        PRODUCTION_TEMPLATES["$template"]="$symbol"
        production_template_count=$((production_template_count + 1))
    done < <(
        awk '/^const EMBEDDED_TEMPLATES:/ { registry = 1 } registry { print } registry && /\];/ { exit }' "$runtime" \
            | tr '\n' ' ' \
            | grep -o -E '\([[:space:]]*[A-Z][A-Z0-9_]*[[:space:]]*,[[:space:]]*[A-Z][A-Z0-9_]*[[:space:]]*\)'
    )
    if [ "$production_template_count" -eq 0 ]; then
        report_failure "Julibrot shader production registry is empty"
    fi
}

scan_inline_raw_strings() {
    local repo="$1"
    local pattern path line source symbol

    pattern='(^|[[:space:]])(const|static|let)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[^=]*=[[:space:]]*r[#]*"'
    while IFS=: read -r path line source; do
        if [[ "$source" =~ (const|static|let)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
            symbol="${BASH_REMATCH[2]}"
            record_detected "inline" "$path" "$symbol"
        else
            report_failure "could not identify inline shader symbol at $path:$line"
        fi
    done < <(git -C "$repo" grep -n -E "$pattern" -- 'crates/labs/julibrot/**/*.rs' 2>/dev/null || :)
}

scan_inline_quoted_wgsl() {
    local repo="$1"
    local pattern path line source symbol

    pattern='(^|[[:space:]])(const|static|let)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[^=]*=[[:space:]]*"[^"]*(@vertex|@fragment|@compute|@group)'
    while IFS=: read -r path line source; do
        if [[ "$source" =~ (const|static|let)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
            symbol="${BASH_REMATCH[2]}"
            record_detected "inline" "$path" "$symbol"
        else
            report_failure "could not identify quoted shader symbol at $path:$line"
        fi
    done < <(git -C "$repo" grep -n -E "$pattern" -- 'crates/labs/julibrot/**/*.rs' 2>/dev/null || :)

    pattern='ShaderSource::Wgsl[[:space:]]*\('
    while IFS=: read -r path line source; do
        symbol="$(
            sed -n "1,${line}p" "$repo/$path" \
                | sed -n -E 's/^(pub(\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*/\4/p' \
                | tail -n 1
        )"
        if [ -z "$symbol" ]; then
            report_failure "could not identify ShaderSource::Wgsl owner at $path:$line"
            continue
        fi
        # The sole allowed lowering accepts RenderedShader itself, so Rust's type checker preserves
        # provenance up to this constructor. Every untyped or additional constructor is migration debt.
        if [ "$path" = "crates/labs/julibrot/present/src/gpu/device/shade.rs" ] \
            && [ "$symbol" = "create_shade_pipeline" ] \
            && [[ "$source" = *'ShaderSource::Wgsl(shader.source().into())'* ]] \
            && grep -Fq -- 'shader: &ember_julibrot_shader::RenderedShader,' "$repo/$path"
        then
            rendered_shader_lowering_count=$((rendered_shader_lowering_count + 1))
            continue
        fi
        record_detected "lowering" "$path" "$symbol"
    done < <(git -C "$repo" grep -n -E "$pattern" -- 'crates/labs/julibrot/**/*.rs' 2>/dev/null || :)
}

compare_inventory() {
    local key

    while IFS= read -r key; do
        [ -n "$key" ] || continue
        if [ -z "${ALLOWED[$key]+present}" ]; then
            report_failure "unlisted Julibrot shader source: $key"
        fi
    done < <(printf '%s\n' "${!DETECTED[@]}" | sort)

    while IFS= read -r key; do
        [ -n "$key" ] || continue
        if [ -z "${DETECTED[$key]+present}" ]; then
            report_failure "stale Julibrot shader allowlist record: $key|${ALLOWED[$key]}"
        fi
    done < <(printf '%s\n' "${!ALLOWED[@]}" | sort)
}

compare_validation_inventory() {
    local template

    while IFS= read -r template; do
        [ -n "$template" ] || continue
        if [ -z "${VALIDATED_TEMPLATES[$template]+present}" ]; then
            report_failure "production shader template lacks native validation test: $template"
        fi
    done < <(printf '%s\n' "${!PRODUCTION_TEMPLATES[@]}" | sort)

    while IFS= read -r template; do
        [ -n "$template" ] || continue
        if [[ "$template" != *-test.wgsl.jinja ]] \
            && [ -z "${PRODUCTION_TEMPLATES[$template]+present}" ]
        then
            report_failure "production template file is absent from the runtime registry: $template"
        fi
    done < <(printf '%s\n' "${!TEMPLATE_FILES[@]}" | sort)
}

check_repo() {
    local repo="$1"
    local allowlist="$2"
    local policy="$3"
    local validations="$4"

    ALLOWED=()
    DETECTED=()
    PRODUCTION_TEMPLATES=()
    TEMPLATE_FILES=()
    VALIDATED_TEMPLATES=()
    VALIDATION_CRATES=()
    VALIDATION_MANIFESTS=()
    VALIDATION_TESTS=()
    allowlist_count=0
    template_count=0
    production_template_count=0
    validation_count=0
    compiled_validation_count=0
    normal_validation_ms=0
    probe_validation_ms=0
    rendered_shader_lowering_count=0
    failures=0
    load_allowlist "$allowlist" "$policy"
    scan_shader_files "$repo"
    scan_production_registry "$repo"
    load_validation_pairs "$repo" "$validations"
    scan_inline_raw_strings "$repo"
    scan_inline_quoted_wgsl "$repo"
    if [ "$rendered_shader_lowering_count" -ne 1 ]; then
        report_failure "expected one typed RenderedShader lowering; found $rendered_shader_lowering_count"
    fi
    compare_inventory
    compare_validation_inventory
    [ "$failures" -eq 0 ]
}

expect_rejection() {
    local label="$1"
    local needle="$2"
    local repo="$3"
    local allowlist="$4"
    local policy="$5"
    local validations="$6"
    local output

    if output="$(check_repo "$repo" "$allowlist" "$policy" "$validations" 2>&1)"; then
        printf 'SELF-TEST FAIL: %s was accepted\n' "$label" >&2
        return 1
    fi
    if [[ "$output" != *"$needle"* ]]; then
        printf 'SELF-TEST FAIL: %s reported the wrong failure: %s\n' "$label" "$output" >&2
        return 1
    fi
}

expect_double_rejection() {
    local label="$1"
    local first_needle="$2"
    local second_needle="$3"
    local repo="$4"
    local allowlist="$5"
    local policy="$6"
    local validations="$7"
    local output

    if output="$(check_repo "$repo" "$allowlist" "$policy" "$validations" 2>&1)"; then
        printf 'SELF-TEST FAIL: %s was accepted\n' "$label" >&2
        return 1
    fi
    if [[ "$output" != *"$first_needle"* || "$output" != *"$second_needle"* ]]; then
        printf 'SELF-TEST FAIL: %s was not caught twice: %s\n' "$label" "$output" >&2
        return 1
    fi
    if [[ "$output" == *'production template requires exactly one lexical validation macro'* ]]; then
        printf 'SELF-TEST FAIL: %s hid the exact validation invocation: %s\n' "$label" "$output" >&2
        return 1
    fi
}

expect_static_acceptance() {
    local label="$1"
    local repo="$2"
    local allowlist="$3"
    local policy="$4"
    local validations="$5"
    local output

    if ! output="$(check_repo "$repo" "$allowlist" "$policy" "$validations" 2>&1)"; then
        printf 'SELF-TEST FAIL: static pre-check rejected %s: %s\n' "$label" "$output" >&2
        return 1
    fi
}

expect_compiled_listing_rejection() {
    local label="$1"
    local template="$2"
    local expected_test="$3"
    local output needle

    if output="$(require_compiled_validation_test \
        "$template" "$expected_test" 'unrelated::native_test: test' 2>&1)"
    then
        printf 'SELF-TEST FAIL: synthetic test listing accepted %s\n' "$label" >&2
        return 1
    fi
    needle="production template $template is missing compiled validation test $expected_test"
    if [[ "$output" != *"$needle"* ]]; then
        printf 'SELF-TEST FAIL: %s reported the wrong compiled-test failure: %s\n' \
            "$label" "$output" >&2
        return 1
    fi
}

write_migrated_shade_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        'mod production_validation {' \
        '    ::ember_julibrot_shader::production_template_test!(' \
        '        PRESENT_SHADE_TEMPLATE,' \
        '        shade_shader,' \
        '        production_template_shade_shader_renders_and_validates' \
        '    );' \
        '}' > "$path"
}

write_spoofed_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '#[test]' \
        'fn shade_source_parses_and_validates() {}' \
        'fn unrelated_text() {' \
        '    let shader = shade_shader().expect("production context renders");' \
        '    let module = naga::front::wgsl::parse_str(shader.source()).expect("source parses");' \
        '    validator.validate(&module).expect("source validates");' \
        '}' > "$path"
}

write_commented_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '/*' \
        '::ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader, production_template_shade_shader_renders_and_validates);' \
        '*/' > "$path"
}

write_cfg_disabled_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '#[cfg(any())]' \
        '::ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader, production_template_shade_shader_renders_and_validates);' \
        > "$path"
}

write_cfg_attr_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '#[cfg_attr(all(), cfg(any()))]' \
        '::ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader, production_template_shade_shader_renders_and_validates);' \
        > "$path"
}

write_enclosing_cfg_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '#[cfg(any())]' \
        'mod disabled {' \
        '    ::ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader, production_template_shade_shader_renders_and_validates);' \
        '}' > "$path"
}

write_unexpanded_macro_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        'macro_rules! disabled {' \
        '    () => {' \
        '        ::ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader, production_template_shade_shader_renders_and_validates);' \
        '    };' \
        '}' > "$path"
}

write_handwritten_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '#[cfg(test)]' \
        'mod production_validation {' \
        '    #[test]' \
        '    fn production_template_shade_shader_renders_and_validates() {}' \
        '}' > "$path"
}

write_composite_validation_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '#[cfg(test)]' \
        'mod production_validation {' \
        '    #[cfg(any())]' \
        '    ::ember_julibrot_shader::production_template_test!(' \
        '        PRESENT_SHADE_TEMPLATE,' \
        '        shade_shader,' \
        '        production_template_shade_shader_renders_and_validates,' \
        '    );' \
        '' \
        '    #[test]' \
        '    fn production_template_shade_shader_renders_and_validates() {}' \
        '}' > "$path"
}

write_local_macro_shadow_fixture() {
    local path="$1"

    printf '%s\n' \
        'mod ember_julibrot_shader {' \
        '    macro_rules! production_template_test {' \
        '        ($template:ident, $render:path, $test_name:ident $(,)?) => {' \
        '            #[test]' \
        '            fn $test_name() {' \
        '                if let Ok(nonce) = std::env::var("EMBER_SHADER_VALIDATION_PROBE") {' \
        '                    println!(' \
        '                        "SHADER-VALIDATION-PROBE {nonce} {} forged",' \
        '                        $template' \
        '                    );' \
        '                }' \
        '            }' \
        '        };' \
        '    }' \
        '    pub(crate) use production_template_test;' \
        '}' \
        '' \
        'ember_julibrot_shader::production_template_test!(' \
        '    PRESENT_SHADE_TEMPLATE,' \
        '    shade_shader,' \
        '    production_template_shade_shader_renders_and_validates,' \
        ');' > "$path"
}

write_self_alias_validation_fixture() {
    local path="$1"
    local reserved_alias="$2"

    printf '%s\n' \
        'extern crate ember_julibrot_shader as real_shader;' \
        '' \
        '#[macro_export]' \
        'macro_rules! production_template_test {' \
        '    ($template:ident, $render:path, $test_name:ident $(,)?) => {' \
        '        #[test]' \
        '        fn $test_name() {' \
        '            if let Ok(nonce) = std::env::var("EMBER_SHADER_VALIDATION_PROBE") {' \
        '                println!(' \
        '                    "SHADER-VALIDATION-PROBE {nonce} {} ember_shader_validation_probe_missing",' \
        '                    $template' \
        '                );' \
        '            }' \
        '        }' \
        '    };' \
        '}' \
        '' \
        "extern crate self as $reserved_alias;" \
        '' \
        '::ember_julibrot_shader::production_template_test!(' \
        '    PRESENT_SHADE_TEMPLATE,' \
        '    shade_shader,' \
        '    production_template_shade_shader_renders_and_validates,' \
        ');' > "$path"
}

write_root_self_alias_fixture() {
    local path="$1"
    local reserved_alias="$2"

    printf '%s\n' \
        'extern crate ember_julibrot_shader as real_shader;' \
        '' \
        '#[macro_export]' \
        'macro_rules! production_template_test {' \
        '    ($template:ident, $render:path, $test_name:ident $(,)?) => {' \
        '        #[test]' \
        '        fn $test_name() {' \
        '            if let Ok(nonce) = std::env::var("EMBER_SHADER_VALIDATION_PROBE") {' \
        '                println!(' \
        '                    "SHADER-VALIDATION-PROBE {nonce} {} ember_shader_validation_probe_missing",' \
        '                    $template' \
        '                );' \
        '            }' \
        '        }' \
        '    };' \
        '}' \
        '' \
        "extern crate self as $reserved_alias;" > "$path"
}

write_character_delimiter_fixture() {
    local path="$1"

    printf '%s\n' \
        'const _OPEN: char = '\''"'\'';' \
        'extern crate self as ember_julibrot_shader;' \
        'const _CLOSE: char = '\''"'\'';' \
        '' \
        'const _ESCAPED_OPEN: char = '\''\"'\'';' \
        '#[macro_export]' \
        'macro_rules! production_template_test { () => {} }' \
        'const _ESCAPED_CLOSE: char = '\''\"'\'';' \
        '' \
        'const _BYTE_OPEN: u8 = b'\''"'\'';' \
        'extern crate self as r#ember_julibrot_shader;' \
        'const _BYTE_CLOSE: u8 = b'\''"'\'';' > "$path"
}

write_reserved_macro_fixture() {
    local path="$1"
    local attribute="$2"

    if [ -n "$attribute" ]; then
        printf '%s\n' "$attribute" > "$path"
        printf '%s\n' 'macro_rules! production_template_test { () => {} }' >> "$path"
    else
        printf '%s\n' 'macro_rules! production_template_test { () => {} }' > "$path"
    fi
}

write_lifetime_and_label_fixture() {
    local path="$1"

    printf '%s\n' \
        "fn borrow<'a>(value: &'a u8) -> &'a u8 {" \
        "    'label: loop {" \
        "        break 'label value;" \
        '    }' \
        '}' > "$path"
}

synthetic_shader_metadata() {
    local resolved_dependency="$1"

    printf '%s\n' \
        '{' \
        '  "packages": [' \
        '    {"id": "present", "name": "ember-julibrot-present", "manifest_path": "/workspace/crates/labs/julibrot/present/Cargo.toml"},' \
        '    {"id": "shader", "name": "ember-julibrot-shader", "manifest_path": "/workspace/crates/labs/julibrot/shader/Cargo.toml"},' \
        '    {"id": "forged", "name": "forged-shader", "manifest_path": "/workspace/crates/forged-shader/Cargo.toml"}' \
        '  ],' \
        '  "workspace_members": ["present", "shader"],' \
        '  "resolve": {' \
        '    "nodes": [' \
        "      {\"id\": \"present\", \"deps\": [{\"name\": \"ember_julibrot_shader\", \"pkg\": \"$resolved_dependency\"}]}" \
        '    ]' \
        '  }' \
        '}'
}

write_test_runtime_registry() {
    local path="$1"

    if [ "${2:-}" = "with-new" ]; then
        printf '%s\n' \
            'pub const PRESENT_SHADE_TEMPLATE: &str = "present-shade.wgsl.jinja";' \
            'const PRESENT_SHADE_SOURCE: &str = include_str!("../templates/present-shade.wgsl.jinja");' \
            'pub const NEW_SHADE_TEMPLATE: &str = "new-shade.wgsl.jinja";' \
            'const NEW_SHADE_SOURCE: &str = include_str!("../templates/new-shade.wgsl.jinja");' \
            'const EMBEDDED_TEMPLATES: &[(&str, &str)] = &[' \
            '    (PRESENT_SHADE_TEMPLATE, PRESENT_SHADE_SOURCE),' \
            '    (NEW_SHADE_TEMPLATE, NEW_SHADE_SOURCE),' \
            '];' > "$path"
    else
        printf '%s\n' \
            'pub const PRESENT_SHADE_TEMPLATE: &str = "present-shade.wgsl.jinja";' \
            'const PRESENT_SHADE_SOURCE: &str = include_str!("../templates/present-shade.wgsl.jinja");' \
            'const EMBEDDED_TEMPLATES: &[(&str, &str)] = &[(PRESENT_SHADE_TEMPLATE, PRESENT_SHADE_SOURCE)];' \
            > "$path"
    fi
}

self_test() {
    local started temporary repo allowlist policy validations validation_template expected_test
    local valid_metadata forged_metadata dependency_output probe_output
    started="$(date +%s)"
    temporary="$(mktemp -d -p "${TMPDIR:-/tmp}" ember-shadertest-XXXXXX)"
    SHADER_TEST_TMP="$temporary"
    trap '[ -z "${SHADER_TEST_TMP:-}" ] || rm -rf -- "$SHADER_TEST_TMP"' EXIT
    repo="$temporary/repo"
    allowlist="$repo/deploy/tests/julibrot-shader-allowlist.txt"
    validations="$repo/deploy/tests/julibrot-shader-validation-tests.txt"
    policy="$repo/docs/julibrot/shaders.md"
    validation_template="crates/labs/julibrot/shader/templates/present-shade.wgsl.jinja"
    expected_test="$(validation_test_name \
        'crates/labs/julibrot/present/src/shade_shader.rs' shade_shader)"
    if [ "$expected_test" != "shade_shader::production_validation::production_template_shade_shader_renders_and_validates" ]; then
        printf 'SELF-TEST FAIL: derived the wrong compiled validation test: %s\n' "$expected_test" >&2
        return 1
    fi

    git -C "$temporary" init -q repo
    mkdir -p "$repo/crates/labs/julibrot/kernels/src"
    mkdir -p "$repo/crates/labs/julibrot/present/src/gpu/device"
    mkdir -p "$repo/crates/labs/julibrot/shader/src" "$repo/crates/labs/julibrot/shader/templates"
    mkdir -p "$repo/deploy/tests" "$repo/docs/julibrot"
    printf '%s\n' '{{ "PaletteUniform"|wgsl_type }}' > "$repo/crates/labs/julibrot/shader/templates/present-shade.wgsl.jinja"
    write_test_runtime_registry "$repo/crates/labs/julibrot/shader/src/runtime.rs"
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    printf '%s\n' \
        '[package]' \
        'name = "ember-julibrot-present"' \
        'version = "0.0.0"' \
        'edition = "2024"' > "$repo/crates/labs/julibrot/present/Cargo.toml"
    printf '%s\n' \
        'fn create_shade_pipeline(' \
        '    shader: &ember_julibrot_shader::RenderedShader,' \
        ') { ShaderSource::Wgsl(shader.source().into()); }' \
        > "$repo/crates/labs/julibrot/present/src/gpu/device/shade.rs"
    printf '%s\n' 'kernel body' > "$repo/crates/labs/julibrot/kernels/src/shallow.wgsl"
    printf '%s\n' 'const WARP_SHADER: &str = r"' '@vertex fn warp() {}' '";' > "$repo/crates/labs/julibrot/present/src/warp_shader.rs"
    printf '%s\n' 'JB-PRESENT-SHADE' 'JB-PRESENT-WARP' 'JB-PRESENT-SCENE' 'JB-KERNEL-SHALLOW' 'JB-KERNEL-PERTURB' > "$policy"
    printf '%s\n' \
        'inline|crates/labs/julibrot/present/src/warp_shader.rs|WARP_SHADER|JB-PRESENT-WARP' \
        'file|crates/labs/julibrot/kernels/src/shallow.wgsl|-|JB-KERNEL-SHALLOW' > "$allowlist"
    printf '%s\n' \
        'crates/labs/julibrot/shader/templates/present-shade.wgsl.jinja|crates/labs/julibrot/present/src/shade_shader.rs|PRESENT_SHADE_TEMPLATE|shade_shader' \
        > "$validations"
    git -C "$repo" add .

    if ! check_repo "$repo" "$allowlist" "$policy" "$validations"; then
        printf 'SELF-TEST FAIL: reviewed debt and migrated template were rejected\n' >&2
        return 1
    fi
    if ! require_compiled_validation_test \
        "$validation_template" "$expected_test" "$expected_test: test"
    then
        printf 'SELF-TEST FAIL: the synthetic compiled validation test was rejected\n' >&2
        return 1
    fi
    valid_metadata="$(synthetic_shader_metadata shader)"
    if ! require_shader_dependency_identity \
        "$valid_metadata" ember-julibrot-present \
        /workspace/crates/labs/julibrot/present/Cargo.toml \
        /workspace/crates/labs/julibrot/shader/Cargo.toml
    then
        printf 'SELF-TEST FAIL: the genuine workspace shader dependency was rejected\n' >&2
        return 1
    fi
    forged_metadata="$(synthetic_shader_metadata forged)"
    if dependency_output="$(require_shader_dependency_identity \
        "$forged_metadata" ember-julibrot-present \
        /workspace/crates/labs/julibrot/present/Cargo.toml \
        /workspace/crates/labs/julibrot/shader/Cargo.toml 2>&1)"
    then
        printf 'SELF-TEST FAIL: a dependency aliased as ember_julibrot_shader was accepted\n' >&2
        return 1
    fi
    if [[ "$dependency_output" != *'extern ember_julibrot_shader does not resolve to workspace package ember-julibrot-shader'* ]]; then
        printf 'SELF-TEST FAIL: dependency alias reported the wrong failure: %s\n' \
            "$dependency_output" >&2
        return 1
    fi
    if ! require_validation_probe_receipt \
        "$validation_template" self-test \
        'SHADER-VALIDATION-PROBE self-test present-shade.wgsl.jinja error: ember_shader_validation_probe_missing'
    then
        printf 'SELF-TEST FAIL: a genuine validation probe receipt was rejected\n' >&2
        return 1
    fi
    if probe_output="$(require_validation_probe_receipt \
        "$validation_template" self-test \
        'SHADER-VALIDATION-PROBE self-test present-shade.wgsl.jinja forged' 2>&1)"
    then
        printf 'SELF-TEST FAIL: a validation probe receipt without the missing identifier was accepted\n' >&2
        return 1
    fi
    if [[ "$probe_output" != *'naga diagnostic naming ember_shader_validation_probe_missing'* ]]; then
        printf 'SELF-TEST FAIL: forged validation receipt reported the wrong failure: %s\n' \
            "$probe_output" >&2
        return 1
    fi

    printf '%s\n' '{{ "NewUniform"|wgsl_type }}' > "$repo/crates/labs/julibrot/shader/templates/new-shade.wgsl.jinja"
    write_test_runtime_registry "$repo/crates/labs/julibrot/shader/src/runtime.rs" with-new
    git -C "$repo" add crates/labs/julibrot/shader/templates/new-shade.wgsl.jinja
    expect_rejection "an unpaired production template" "lacks native validation test" "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/shader/templates/new-shade.wgsl.jinja
    write_test_runtime_registry "$repo/crates/labs/julibrot/shader/src/runtime.rs"

    write_spoofed_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a validation spoof with unrelated strings" "requires exactly one lexical validation macro" "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_commented_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a block-commented validation macro" "requires exactly one lexical validation macro" "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_cfg_disabled_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a directly cfg-disabled validation macro" \
        "production validation macro has a cfg attribute" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_cfg_attr_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a cfg-attr-disabled validation macro" \
        "production validation macro has a cfg attribute" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_enclosing_cfg_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_static_acceptance "a validation macro in a cfg-disabled module" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    expect_compiled_listing_rejection "a validation macro in a cfg-disabled module" \
        "$validation_template" "$expected_test" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_unexpanded_macro_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_static_acceptance "a validation invocation in an unexpanded macro body" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    expect_compiled_listing_rejection "a validation invocation in an unexpanded macro body" \
        "$validation_template" "$expected_test" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_handwritten_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a hand-written validation test" \
        "hand-written production validation test is forbidden" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_composite_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a disabled macro paired with a hand-written empty test" \
        "hand-written production validation test is forbidden" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_local_macro_shadow_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a local module that re-exports a forged validation macro" \
        "production template requires exactly one lexical validation macro" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_self_alias_validation_fixture \
        "$repo/crates/labs/julibrot/present/src/shade_shader.rs" ember_julibrot_shader
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_double_rejection "an extern-prelude self-alias with a forged exported macro" \
        "production validation extern name is rebound: crates/labs/julibrot/present/src/shade_shader.rs:18" \
        "production validation macro is redefined: crates/labs/julibrot/present/src/shade_shader.rs:4" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_self_alias_validation_fixture \
        "$repo/crates/labs/julibrot/present/src/shade_shader.rs" r#ember_julibrot_shader
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_double_rejection "a raw-identifier extern-prelude self-alias" \
        "production validation extern name is rebound: crates/labs/julibrot/present/src/shade_shader.rs:18" \
        "production validation macro is redefined: crates/labs/julibrot/present/src/shade_shader.rs:4" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    write_root_self_alias_fixture \
        "$repo/crates/labs/julibrot/present/src/lib.rs" ember_julibrot_shader
    git -C "$repo" add crates/labs/julibrot/present/src/lib.rs
    expect_double_rejection "a crate-root extern-prelude self-alias" \
        "production validation extern name is rebound: crates/labs/julibrot/present/src/lib.rs:18" \
        "production validation macro is redefined: crates/labs/julibrot/present/src/lib.rs:4" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/lib.rs

    write_character_delimiter_fixture "$repo/crates/labs/julibrot/present/src/lib.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/lib.rs
    expect_double_rejection "character literals around an ordinary reserved declaration" \
        "production validation extern name is rebound: crates/labs/julibrot/present/src/lib.rs:2" \
        "production validation macro is redefined: crates/labs/julibrot/present/src/lib.rs:7" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    expect_double_rejection "byte-character literals around a raw reserved declaration" \
        "production validation extern name is rebound: crates/labs/julibrot/present/src/lib.rs:11" \
        "production validation macro is redefined: crates/labs/julibrot/present/src/lib.rs:7" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/lib.rs

    write_reserved_macro_fixture \
        "$repo/crates/labs/julibrot/present/src/lib.rs" '#[cfg_attr(all(), macro_export)]'
    git -C "$repo" add crates/labs/julibrot/present/src/lib.rs
    expect_rejection "a cfg-attr-exported reserved validation macro" \
        "production validation macro is redefined: crates/labs/julibrot/present/src/lib.rs:2" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_reserved_macro_fixture "$repo/crates/labs/julibrot/present/src/lib.rs" ''
    git -C "$repo" add crates/labs/julibrot/present/src/lib.rs
    expect_rejection "an unattributed reserved validation macro" \
        "production validation macro is redefined: crates/labs/julibrot/present/src/lib.rs:1" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/lib.rs

    write_lifetime_and_label_fixture "$repo/crates/labs/julibrot/present/src/lib.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/lib.rs
    expect_static_acceptance "Rust lifetimes and labels" \
        "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/lib.rs

    printf '%s\n' \
        '::ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader, production_template_shade_shader_renders_and_validates);' \
        > "$repo/crates/labs/julibrot/present/src/duplicate_validation.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/duplicate_validation.rs
    expect_rejection "a duplicate lexical validation macro" "requires exactly one lexical validation macro" "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/duplicate_validation.rs

    printf '%s\n' 'unlisted body' > "$repo/crates/labs/julibrot/present/src/new.wgsl"
    git -C "$repo" add crates/labs/julibrot/present/src/new.wgsl
    expect_rejection "an unlisted .wgsl file" "unlisted Julibrot shader source" "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/new.wgsl

    printf '%s\n' 'const NEW_SHADER: &str = r"' '@fragment fn fragment() {}' '";' > "$repo/crates/labs/julibrot/present/src/new_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/new_shader.rs
    expect_rejection "an unlisted inline shader" "unlisted Julibrot shader source" "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/new_shader.rs

    printf '%s\n' 'fn direct_shader() { ShaderSource::Wgsl(r"@vertex fn vertex() {}"); }' > "$repo/crates/labs/julibrot/present/src/direct_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/direct_shader.rs
    expect_rejection "a direct inline ShaderSource literal" "unlisted Julibrot shader source" "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/direct_shader.rs

    printf '%s\n' 'fn direct_shader(source: String) { ShaderSource::Wgsl(source.into()); }' > "$repo/crates/labs/julibrot/present/src/direct_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/direct_shader.rs
    expect_rejection "a direct computed ShaderSource" "unlisted Julibrot shader source" "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/present/src/direct_shader.rs

    printf '%s\n' 'const SHADE_SHADER: &str = r"' '@fragment fn old_shade() {}' '";' > "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "the migrated shade inline source" "unlisted Julibrot shader source" "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        '#[cfg(test)]' \
        'const TEST_SHADE_SOURCE: &str = r"' \
        '@fragment fn test_shade() {}' \
        '";' > "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a cfg(test) inline shader fixture" "unlisted Julibrot shader source" "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    printf '%s\n' 'file|crates/labs/julibrot/kernels/src/perturb.wgsl|-|JB-KERNEL-PERTURB' >> "$allowlist"
    expect_rejection "a stale allowlist record" "stale Julibrot shader allowlist record" "$repo" "$allowlist" "$policy" "$validations" || return 1
    sed -i '$d' "$allowlist"

    printf '%s\n' 'const NEW_SHADER: &str = r"' '@compute @workgroup_size(1) fn new_shader() {}' '";' > "$repo/crates/labs/julibrot/present/src/new_shader.rs"
    printf '%s\n' 'inline|crates/labs/julibrot/present/src/new_shader.rs|NEW_SHADER|JB-PRESENT-SCENE' >> "$allowlist"
    git -C "$repo" add crates/labs/julibrot/present/src/new_shader.rs
    expect_rejection "an expanded allowlist" "shader allowlist may only shrink" "$repo" "$allowlist" "$policy" "$validations" || return 1

    printf 'SELF-TEST PASS: Rust character stripping, reserved extern-bound validation pairings, root self-alias, attributed and unattributed macro definitions, dependency alias and composite spoofs, naga probe receipts, templates, closed debt, files, test fixtures, inline and direct sources, stale records and shrink-only ceiling, %ss\n' "$(( $(date +%s) - started ))"
}

if [ "${1:-}" = "--self-test" ]; then
    [ "$#" -eq 1 ] || { printf 'usage: bash deploy/tests/test-shaders.sh [--self-test]\n' >&2; exit 2; }
    if self_test && "$PYTHON_BIN" "$HERE/repository_shader_check.py" --self-test; then
        exit 0
    fi
    exit 1
fi
[ "$#" -eq 0 ] || { printf 'usage: bash deploy/tests/test-shaders.sh [--self-test]\n' >&2; exit 2; }

started="$(date +%s)"
if check_repo "$ROOT" "$ALLOWLIST" "$POLICY" "$VALIDATIONS" \
    && check_compiled_validation_tests "$ROOT"
then
    printf 'SHADER CHECK PASS: %s/%s production templates have compiled and passing native validation tests, normal run %sms, probe run %sms, %s production/test templates, %s reviewed migration exceptions, %ss\n' "$compiled_validation_count" "$production_template_count" "$normal_validation_ms" "$probe_validation_ms" "$template_count" "$allowlist_count" "$(( $(date +%s) - started ))"
else
    status=$?
    printf 'SHADER CHECK FAIL: Julibrot shader policy violation, %ss\n' "$(( $(date +%s) - started ))" >&2
    exit "$status"
fi

"$PYTHON_BIN" "$HERE/repository_shader_check.py" --root "$ROOT"
