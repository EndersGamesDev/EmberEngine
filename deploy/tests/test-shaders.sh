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

declare -A ALLOWED=()
declare -A DETECTED=()
declare -A PRODUCTION_TEMPLATES=()
declare -A TEMPLATE_FILES=()
declare -A VALIDATED_TEMPLATES=()
allowlist_count=0
template_count=0
production_template_count=0
validation_count=0
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

load_validation_pairs() {
    local repo="$1"
    local validations="$2"
    local line_number=0 record template test_path template_symbol render_function
    local key runtime template_name invocation_pattern invocation_count global_invocation_count
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
            invocation_pattern="^[[:space:]]*ember_julibrot_shader::production_template_test![[:space:]]*\([[:space:]]*$template_symbol[[:space:]]*,[[:space:]]*$render_function[[:space:]]*\)[[:space:]]*;[[:space:]]*$"
            invocation_count="$(grep -Ec -- "$invocation_pattern" "$repo/$test_path" || :)"
            global_invocation_count="$({
                git -C "$repo" grep -E "$invocation_pattern" -- 'crates/labs/julibrot/**/*.rs' \
                    2>/dev/null || :
            } | wc -l)"
            if [ "$invocation_count" -ne 1 ] || [ "$global_invocation_count" -ne 1 ]; then
                report_failure "production template requires exactly one structural validation macro: $test_path:$template_symbol,$render_function"
            fi
        fi
        VALIDATED_TEMPLATES["$key"]="$test_path:$template_symbol,$render_function"
        validation_count=$((validation_count + 1))
    done < "$validations"
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
    allowlist_count=0
    template_count=0
    production_template_count=0
    validation_count=0
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

write_migrated_shade_fixture() {
    local path="$1"

    printf '%s\n' \
        'fn shade_shader() { render_template(); }' \
        'mod production_validation {' \
        '    ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader);' \
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
    local started temporary repo allowlist policy validations
    started="$(date +%s)"
    temporary="$(mktemp -d -p "${TMPDIR:-/tmp}" ember-shadertest-XXXXXX)"
    SHADER_TEST_TMP="$temporary"
    trap '[ -z "${SHADER_TEST_TMP:-}" ] || rm -rf -- "$SHADER_TEST_TMP"' EXIT
    repo="$temporary/repo"
    allowlist="$repo/deploy/tests/julibrot-shader-allowlist.txt"
    validations="$repo/deploy/tests/julibrot-shader-validation-tests.txt"
    policy="$repo/docs/julibrot/shaders.md"

    git -C "$temporary" init -q repo
    mkdir -p "$repo/crates/labs/julibrot/kernels/src"
    mkdir -p "$repo/crates/labs/julibrot/present/src/gpu/device"
    mkdir -p "$repo/crates/labs/julibrot/shader/src" "$repo/crates/labs/julibrot/shader/templates"
    mkdir -p "$repo/deploy/tests" "$repo/docs/julibrot"
    printf '%s\n' '{{ "PaletteUniform"|wgsl_type }}' > "$repo/crates/labs/julibrot/shader/templates/present-shade.wgsl.jinja"
    write_test_runtime_registry "$repo/crates/labs/julibrot/shader/src/runtime.rs"
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
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

    printf '%s\n' '{{ "NewUniform"|wgsl_type }}' > "$repo/crates/labs/julibrot/shader/templates/new-shade.wgsl.jinja"
    write_test_runtime_registry "$repo/crates/labs/julibrot/shader/src/runtime.rs" with-new
    git -C "$repo" add crates/labs/julibrot/shader/templates/new-shade.wgsl.jinja
    expect_rejection "an unpaired production template" "lacks native validation test" "$repo" "$allowlist" "$policy" "$validations" || return 1
    git -C "$repo" rm -q -f crates/labs/julibrot/shader/templates/new-shade.wgsl.jinja
    write_test_runtime_registry "$repo/crates/labs/julibrot/shader/src/runtime.rs"

    write_spoofed_validation_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs
    expect_rejection "a validation spoof with unrelated strings" "requires exactly one structural validation macro" "$repo" "$allowlist" "$policy" "$validations" || return 1
    write_migrated_shade_fixture "$repo/crates/labs/julibrot/present/src/shade_shader.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/shade_shader.rs

    printf '%s\n' \
        'ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader);' \
        > "$repo/crates/labs/julibrot/present/src/duplicate_validation.rs"
    git -C "$repo" add crates/labs/julibrot/present/src/duplicate_validation.rs
    expect_rejection "a duplicate structural validation macro" "requires exactly one structural validation macro" "$repo" "$allowlist" "$policy" "$validations" || return 1
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

    printf 'SELF-TEST PASS: production validation pairs, templates, closed debt, files, test fixtures, inline and direct sources, stale records and shrink-only ceiling, %ss\n' "$(( $(date +%s) - started ))"
}

if [ "${1:-}" = "--self-test" ]; then
    [ "$#" -eq 1 ] || { printf 'usage: bash deploy/tests/test-shaders.sh [--self-test]\n' >&2; exit 2; }
    self_test
    exit $?
fi
[ "$#" -eq 0 ] || { printf 'usage: bash deploy/tests/test-shaders.sh [--self-test]\n' >&2; exit 2; }

started="$(date +%s)"
if check_repo "$ROOT" "$ALLOWLIST" "$POLICY" "$VALIDATIONS"; then
    printf 'SHADER CHECK PASS: %s/%s production templates have native validation pairs, %s production/test templates, %s reviewed migration exceptions, %ss\n' "$validation_count" "$production_template_count" "$template_count" "$allowlist_count" "$(( $(date +%s) - started ))"
else
    status=$?
    printf 'SHADER CHECK FAIL: unrendered Julibrot shader source, %ss\n' "$(( $(date +%s) - started ))" >&2
    exit "$status"
fi
