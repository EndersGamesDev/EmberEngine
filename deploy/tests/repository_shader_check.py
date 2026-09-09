#!/usr/bin/env python3
"""Enforce the repository-wide runtime-rendered shader boundary."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path


ALLOWLIST_NAME = "shader-allowlist.txt"
VALIDATIONS_NAME = "shader-validation-tests.txt"
POLICY_PATH = Path("docs/shaders.md")
SHADER_MANIFEST = Path("crates/ember-shader/Cargo.toml")
SHADER_RUNTIME = Path("crates/ember-shader/src/runtime.rs")
SHADER_TEMPLATES = Path("crates/ember-shader/templates")
PROBE_ENV = "EMBER_SHADER_VALIDATION_PROBE"
PROBE_IDENTIFIER = "ember_shader_validation_probe_missing"
DELEGATED_PREFIX = "crates/labs/julibrot/"
RAW_STRING_PATTERN = re.compile(r"(?:br|rb|r)(?P<hashes>#{0,255})\"")

# Forty records were present when the repository boundary opened. This exact historical ceiling
# includes test fixtures because configuration is not an exemption; migrations may only delete it.
ALLOWLIST_CEILING: frozenset[str] = frozenset(
    """\
file|ember-engine|crates/ember-engine/src/present.wgsl|-|SH-ENGINE
file|ember-engine|crates/ember-engine/src/shader.wgsl|-|SH-ENGINE
file|ember-lab-heap|crates/labs/heap/src/draw-heap.wgsl|-|SH-HEAP
file|ember-lab-heap|crates/labs/heap/src/draw-traditional.wgsl|-|SH-HEAP
file|ember-lab-heap|crates/labs/heap/src/fetch-direct.wgsl|-|SH-HEAP
file|ember-lab-heap|crates/labs/heap/src/fetch-heap.wgsl|-|SH-HEAP
file|ember-lab-heap|crates/labs/heap/src/layer-draw.wgsl|-|SH-HEAP
file|ember-lab-heap|crates/labs/heap/src/mode-a.wgsl|-|SH-HEAP
file|ember-lab-heap|crates/labs/heap/src/mode-c.wgsl|-|SH-HEAP
include|ember-engine|crates/ember-engine/src/renderer.rs|present.wgsl|SH-ENGINE
include|ember-engine|crates/ember-engine/src/renderer.rs|shader.wgsl|SH-ENGINE
include|ember-engine|crates/ember-engine/src/renderer_gpu_test.rs|shader.wgsl|SH-ENGINE
include|ember-lab-heap|crates/labs/heap/src/kernels.rs|draw-heap.wgsl|SH-HEAP
include|ember-lab-heap|crates/labs/heap/src/kernels.rs|draw-traditional.wgsl|SH-HEAP
include|ember-lab-heap|crates/labs/heap/src/kernels.rs|fetch-direct.wgsl|SH-HEAP
include|ember-lab-heap|crates/labs/heap/src/kernels.rs|fetch-heap.wgsl|SH-HEAP
include|ember-lab-heap|crates/labs/heap/src/lattice.rs|mode-a.wgsl|SH-HEAP
include|ember-lab-heap|crates/labs/heap/src/mode_c.rs|layer-draw.wgsl|SH-HEAP
include|ember-lab-heap|crates/labs/heap/src/mode_c.rs|mode-c.wgsl|SH-HEAP
inline|ember-lab-heap|crates/labs/heap/src/dialect.rs|assemble|SH-HEAP
inline|ember-lab-heap|crates/labs/heap/src/dialect.rs|forbidden_entry_point_and_raw_storage_are_typed_refusals|SH-HEAP
inline|ember-lab-heap|crates/labs/heap/src/mode_c.rs|layer_comparator_kernel|SH-HEAP
inline|ember-lab-heap|crates/labs/heap/src/spike.rs|CONSUMER_SHADER|SH-HEAP
inline|ember-lab-heap|crates/labs/heap/src/spike.rs|PRODUCER_SHADER|SH-HEAP
inline|ember-lab-layer|crates/labs/layer/src/compute.rs|assemble|SH-LAYER
inline|ember-lab-layer|crates/labs/layer/src/demo.rs|RENDER_SHADER|SH-LAYER
inline|what-is-this|crates/what-is-this/src/gpu.rs|SHADER|SH-WHAT-IS-THIS
inline|what-is-this|crates/what-is-this/src/render_bar.rs|SHADER|SH-WHAT-IS-THIS
lowering|ember-engine|crates/ember-engine/src/renderer.rs|new|SH-ENGINE
lowering|ember-engine|crates/ember-engine/src/renderer.rs|try_compile|SH-ENGINE
lowering|ember-engine|crates/ember-engine/src/renderer_gpu_test.rs|new|SH-ENGINE
lowering|ember-lab-heap|crates/labs/heap/src/executor.rs|compute_pipeline|SH-HEAP
lowering|ember-lab-heap|crates/labs/heap/src/lattice_gpu.rs|draw_pipeline|SH-HEAP
lowering|ember-lab-heap|crates/labs/heap/src/lattice_gpu.rs|layer_compute_pipeline|SH-HEAP
lowering|ember-lab-heap|crates/labs/heap/src/spike.rs|pipeline|SH-HEAP
lowering|ember-lab-heap|crates/labs/heap/src/wasm.rs|create_pipeline|SH-HEAP
lowering|ember-lab-layer|crates/labs/layer/src/compute.rs|create_kernel|SH-LAYER
lowering|ember-lab-layer|crates/labs/layer/src/demo.rs|new|SH-LAYER
lowering|what-is-this|crates/what-is-this/src/gpu.rs|new|SH-WHAT-IS-THIS
lowering|what-is-this|crates/what-is-this/src/render_bar.rs|new|SH-WHAT-IS-THIS
""".splitlines()
)


@dataclass(frozen=True, order=True)
class Finding:
    """One repository shader source or untyped wgpu lowering."""

    kind: str
    owner: str
    path: str
    symbol: str

    @property
    def key(self) -> str:
        """Return the stable allowlist key."""
        return f"{self.kind}|{self.owner}|{self.path}|{self.symbol}"


@dataclass(frozen=True)
class RustString:
    """One Rust string literal and its byte offsets in decoded source text."""

    start: int
    end: int
    value: str


@dataclass(frozen=True)
class Validation:
    """One production template's required compiled validation fact."""

    template: str
    test_path: str
    template_symbol: str
    render_function: str
    owner: str
    owner_manifest: str
    expected_test: str


@dataclass
class StaticResult:
    """Static repository inventory and validation-pair result."""

    findings: set[Finding]
    production_templates: dict[str, str]
    validations: list[Validation]
    allowlist_count: int
    errors: list[str]


def git_files(root: Path) -> list[str]:
    """Return every tracked path in deterministic order."""
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"],
        check=True,
        capture_output=True,
    )
    return sorted(path for path in result.stdout.decode().split("\0") if path)


def package_roots(root: Path, tracked: list[str]) -> dict[Path, str]:
    """Map every tracked package directory to its declared package name."""
    packages: dict[Path, str] = {}
    for path in tracked:
        if not path.endswith("Cargo.toml") or path == "Cargo.toml":
            continue
        manifest = root / path
        try:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            continue
        package = document.get("package")
        if isinstance(package, dict) and isinstance(package.get("name"), str):
            packages[Path(path).parent] = package["name"]
    return packages


def owner_for(path: str, packages: dict[Path, str]) -> tuple[Path, str] | None:
    """Resolve a tracked path to its nearest package manifest."""
    candidate = Path(path).parent
    while candidate != Path("."):
        if candidate in packages:
            return candidate, packages[candidate]
        candidate = candidate.parent
    return None


def blank(mask: list[str], source: str, start: int, end: int) -> None:
    """Blank one lexical range while retaining its newlines."""
    for index in range(start, end):
        if source[index] != "\n":
            mask[index] = " "


def rust_lex(source: str) -> tuple[str, list[RustString]]:
    """Strip Rust comments, strings and character literals without moving offsets."""
    mask = list(source)
    strings: list[RustString] = []
    index = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = length if end == -1 else end
            blank(mask, source, index, end)
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            blank(mask, source, index, end)
            index = end
            continue

        raw = RAW_STRING_PATTERN.match(source, index)
        if raw is not None:
            delimiter = '"' + raw.group("hashes")
            content_start = raw.end()
            close = source.find(delimiter, content_start)
            end = length if close == -1 else close + len(delimiter)
            value_end = length if close == -1 else close
            strings.append(RustString(index, end, source[content_start:value_end]))
            blank(mask, source, index, end)
            index = end
            continue

        quote_at = index + 1 if source.startswith('b"', index) else index
        if quote_at < length and source[quote_at] == '"':
            cursor = quote_at + 1
            value: list[str] = []
            while cursor < length:
                if source[cursor] == "\\" and cursor + 1 < length:
                    value.extend(source[cursor : cursor + 2])
                    cursor += 2
                elif source[cursor] == '"':
                    cursor += 1
                    break
                else:
                    value.append(source[cursor])
                    cursor += 1
            strings.append(RustString(index, cursor, "".join(value)))
            blank(mask, source, index, cursor)
            index = cursor
            continue

        character_at = index + 1 if source.startswith("b'", index) else index
        if character_at < length and source[character_at] == "'":
            cursor = character_at + 1
            if cursor < length and source[cursor] == "\\":
                cursor += 2
                if cursor < length and source[cursor - 1] == "u" and source[cursor] == "{":
                    closing_brace = source.find("}", cursor + 1)
                    cursor = length if closing_brace == -1 else closing_brace + 1
            else:
                cursor += 1
            if cursor < length and source[cursor] == "'":
                cursor += 1
                blank(mask, source, index, cursor)
                index = cursor
                continue
        index += 1
    return "".join(mask), strings


FUNCTION_PATTERN = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>{}]*>)?\s*\(")


def enclosing_function(code: str, offset: int) -> tuple[str, int] | None:
    """Return the nearest lexical function before an offset."""
    matches = list(FUNCTION_PATTERN.finditer(code, 0, offset))
    if not matches:
        return None
    match = matches[-1]
    return match.group(1), match.start()


def literal_owner(source: str, code: str, offset: int) -> str:
    """Name a WGSL-bearing literal by its assignment or enclosing function."""
    window = source[max(0, offset - 800) : offset]
    assignment = re.search(
        r"(?:const|static|let)\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)[^=;]*=\s*$",
        window,
        re.DOTALL,
    )
    if assignment is not None:
        return assignment.group(1)
    function = enclosing_function(code, offset)
    return "module" if function is None else function[0]


def balanced_call_argument(code: str, opening: int) -> str | None:
    """Return one call argument from stripped Rust, respecting nested parentheses."""
    depth = 1
    cursor = opening + 1
    while cursor < len(code):
        character = code[cursor]
        if character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
            if depth == 0:
                return code[opening + 1 : cursor]
        cursor += 1
    return None


def is_rendered_lowering(code: str, offset: int, opening: int) -> bool:
    """Recognize an exact lowering rooted in a RenderedShader parameter."""
    function = enclosing_function(code, offset)
    if function is None:
        return False
    _, start = function
    body = code.find("{", start, offset)
    if body == -1:
        return False
    signature = code[start:body]
    parameter = re.search(
        r"\b([A-Za-z_][A-Za-z0-9_]*)\s*:\s*&\s*(?:::)?ember_shader::RenderedShader\b",
        signature,
    )
    if parameter is None:
        return False
    shader_name = parameter.group(1)
    if re.search(rf"\b{re.escape(shader_name)}\b", code[body + 1 : offset]) is not None:
        return False
    argument = balanced_call_argument(code, opening)
    if argument is None:
        return False
    shader = re.escape(shader_name)
    source_call = rf"{shader}\s*\.\s*source\s*\(\s*\)"
    accepted = (
        rf"\s*{source_call}\s*\.\s*into\s*\(\s*\)\s*",
        rf"\s*(?:(?:::)?std\s*::\s*borrow\s*::\s*)?Cow\s*::\s*Borrowed\s*"
        rf"\(\s*{source_call}\s*\)\s*",
    )
    return any(re.fullmatch(pattern, argument, re.DOTALL) is not None for pattern in accepted)


def scan_rust(path: str, owner: str, source: str) -> set[Finding]:
    """Find includes, WGSL literals and untyped ShaderSource lowerings in Rust."""
    code, strings = rust_lex(source)
    findings: set[Finding] = set()
    stage = re.compile(r"@(vertex|fragment|compute)\b")
    for literal in strings:
        prefix = code[max(0, literal.start - 120) : literal.start]
        if literal.value.endswith(".wgsl") and re.search(r"include_str!\s*\(\s*$", prefix):
            findings.add(Finding("include", owner, path, literal.value))
        if stage.search(literal.value) is not None and "{" in literal.value:
            findings.add(Finding("inline", owner, path, literal_owner(source, code, literal.start)))

    for match in re.finditer(r"\b(?:wgpu::)?ShaderSource::Wgsl\s*\(", code):
        if is_rendered_lowering(code, match.start(), match.end() - 1):
            continue
        function = enclosing_function(code, match.start())
        symbol = "module" if function is None else function[0]
        findings.add(Finding("lowering", owner, path, symbol))
    return findings


def scan_repository(root: Path, tracked: list[str]) -> tuple[set[Finding], list[str]]:
    """Inventory every non-Julibrot crate, including frozen packages under games."""
    packages = package_roots(root, tracked)
    findings: set[Finding] = set()
    errors: list[str] = []
    for path in tracked:
        if path.startswith(DELEGATED_PREFIX):
            continue
        resolved_owner = owner_for(path, packages)
        if path.endswith(".wgsl"):
            if resolved_owner is None:
                errors.append(f"repository WGSL has no owning crate: {path}")
            else:
                findings.add(Finding("file", resolved_owner[1], path, "-"))
        if not path.endswith(".rs") or resolved_owner is None:
            continue
        try:
            source = (root / path).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"could not read Rust shader candidate {path}: {error}")
            continue
        if not any(
            needle in source
            for needle in (
                "include_str!",
                "ShaderSource::Wgsl",
                "@vertex",
                "@fragment",
                "@compute",
            )
        ):
            continue
        findings.update(scan_rust(path, resolved_owner[1], source))
    return findings, errors


def parse_registry(root: Path, tracked: list[str]) -> tuple[dict[str, str], list[str]]:
    """Match shared production template files to the embedded runtime registry."""
    errors: list[str] = []
    runtime_path = root / SHADER_RUNTIME
    try:
        runtime = runtime_path.read_text(encoding="utf-8")
    except OSError as error:
        return {}, [f"missing shared shader runtime registry: {error}"]
    registry = re.search(
        r"const\s+EMBEDDED_TEMPLATES\s*:\s*&\[[^=]*=\s*&\[(?P<body>.*?)\];",
        runtime,
        re.DOTALL,
    )
    if registry is None:
        return {}, ["shared shader runtime has no EMBEDDED_TEMPLATES registry"]
    constants = dict(
        re.findall(
            r"pub\s+const\s+([A-Z][A-Z0-9_]*_TEMPLATE)\s*:\s*&str\s*=\s*\"([^\"]+\.wgsl\.jinja)\"\s*;",
            runtime,
        )
    )
    registered: dict[str, str] = {}
    for symbol, source_symbol in re.findall(
        r"\(\s*([A-Z][A-Z0-9_]*_TEMPLATE)\s*,\s*([A-Z][A-Z0-9_]*_SOURCE)\s*\)",
        registry.group("body"),
    ):
        name = constants.get(symbol)
        if name is None:
            errors.append(f"production template registry symbol has no public name: {symbol}")
            continue
        template = (SHADER_TEMPLATES / name).as_posix()
        if template in registered:
            errors.append(f"duplicate shared production template: {template}")
            continue
        include = re.compile(
            rf"const\s+{re.escape(source_symbol)}\s*:\s*&str\s*=\s*include_str!\(\"\.\./templates/{re.escape(name)}\"\)\s*;"
        )
        if include.search(runtime) is None:
            errors.append(f"production template source constant is not embedded: {source_symbol}")
        registered[template] = symbol

    template_files = {
        path
        for path in tracked
        if path.startswith(SHADER_TEMPLATES.as_posix() + "/") and path.endswith(".wgsl.jinja")
    }
    production_files = {path for path in template_files if not path.endswith("-test.wgsl.jinja")}
    for template in sorted(production_files - registered.keys()):
        errors.append(f"production template file is absent from shared runtime registry: {template}")
    for template in sorted(registered.keys() - production_files):
        errors.append(f"shared runtime registry has a missing or test-only template: {template}")
    return registered, errors


def validation_function(render_function: str) -> str:
    """Derive the macro-generated Rust test identifier."""
    return f"production_template_{render_function}_renders_and_validates"


def compiled_test_name(test_path: str, crate_root: Path, render_function: str) -> str:
    """Derive the test harness's fully qualified test name."""
    relative = Path(test_path).relative_to(crate_root / "src").with_suffix("")
    parts = list(relative.parts)
    if parts == ["lib"] or parts == ["main"]:
        parts = []
    elif parts and parts[-1] == "mod":
        parts.pop()
    parts.extend(["production_validation", validation_function(render_function)])
    return "::".join(parts)


def has_direct_cfg(code: str, invocation_start: int) -> bool:
    """Return whether cfg or cfg_attr directly decorates a macro invocation."""
    prefix = code[:invocation_start]
    boundary = max(prefix.rfind(";"), prefix.rfind("{"), prefix.rfind("}"))
    attributes = prefix[boundary + 1 :]
    return re.search(r"#\s*\[\s*cfg(?:_attr)?\s*\(", attributes) is not None


def parse_validations(
    root: Path,
    tracked: list[str],
    production_templates: dict[str, str],
    validations_path: Path,
) -> tuple[list[Validation], list[str]]:
    """Authenticate every lexical production validation pairing."""
    errors: list[str] = []
    validations: list[Validation] = []
    packages = package_roots(root, tracked)
    try:
        records = validations_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return [], [f"missing repository shader validation pairs: {error}"]
    seen: set[str] = set()
    for line_number, record in enumerate(records, 1):
        if not record or record.startswith("#"):
            continue
        fields = record.split("|")
        if len(fields) != 4:
            errors.append(f"malformed repository shader validation line {line_number}: {record}")
            continue
        template, test_path, template_symbol, render_function = fields
        if (
            not template.startswith(SHADER_TEMPLATES.as_posix() + "/")
            or template.endswith("-test.wgsl.jinja")
            or not test_path.endswith(".rs")
            or re.fullmatch(r"[A-Z][A-Z0-9_]*", template_symbol) is None
            or re.fullmatch(r"[a-z][a-z0-9_]*", render_function) is None
        ):
            errors.append(f"invalid repository shader validation line {line_number}: {record}")
            continue
        if template in seen:
            errors.append(f"duplicate repository shader validation pair: {template}")
            continue
        seen.add(template)
        if production_templates.get(template) != template_symbol:
            errors.append(f"stale or mismatched repository shader validation pair: {template}")
            continue
        resolved_owner = owner_for(test_path, packages)
        if resolved_owner is None:
            errors.append(f"shader validation test has no owning crate: {test_path}")
            continue
        crate_root, owner = resolved_owner
        crate_sources = [
            path
            for path in tracked
            if path.startswith(crate_root.as_posix() + "/") and path.endswith(".rs")
        ]
        expected_function = validation_function(render_function)
        invocation = re.compile(
            rf"(?<![A-Za-z0-9_:])::ember_shader::production_template_test!\s*\(\s*"
            rf"{re.escape(template_symbol)}\s*,\s*{re.escape(render_function)}\s*,\s*"
            rf"{re.escape(expected_function)}\s*,?\s*\)\s*;",
            re.DOTALL,
        )
        invocation_count = 0
        identifier_count = 0
        for source_path in crate_sources:
            source = (root / source_path).read_text(encoding="utf-8")
            code, _ = rust_lex(source)
            matches = list(invocation.finditer(code))
            invocation_count += len(matches)
            identifier_count += len(re.findall(rf"\b{re.escape(expected_function)}\b", code))
            for match in matches:
                if has_direct_cfg(code, match.start()):
                    line = code.count("\n", 0, match.start()) + 1
                    errors.append(f"production validation macro has a cfg attribute: {source_path}:{line}")
            for match in re.finditer(rf"\bfn\s+{re.escape(expected_function)}\s*\(", code):
                line = code.count("\n", 0, match.start()) + 1
                errors.append(
                    f"hand-written production validation test is forbidden: {source_path}:{line}:{expected_function}"
                )
            for match in re.finditer(
                r"\bextern\s+crate\s+[^;\s]+\s+as\s+(?:r#)?ember_shader\s*;", code
            ):
                line = code.count("\n", 0, match.start()) + 1
                errors.append(f"production validation extern name is rebound: {source_path}:{line}")
            for match in re.finditer(
                r"\bmacro_rules\s*!\s*(?:r#)?production_template_test\b", code
            ):
                line = code.count("\n", 0, match.start()) + 1
                errors.append(f"production validation macro is redefined: {source_path}:{line}")
        if invocation_count != 1:
            errors.append(
                "production template requires exactly one lexical validation macro: "
                f"{test_path}:{template_symbol},{render_function},{expected_function}"
            )
        if identifier_count != 1:
            errors.append(
                "derived validation identifier must occur exactly once in the owning crate: "
                f"{test_path}:{expected_function}"
            )
        validations.append(
            Validation(
                template,
                test_path,
                template_symbol,
                render_function,
                owner,
                (crate_root / "Cargo.toml").as_posix(),
                compiled_test_name(test_path, crate_root, render_function),
            )
        )
    for template in sorted(production_templates.keys() - seen):
        errors.append(f"production shared shader template lacks native validation test: {template}")
    return validations, errors


def load_allowlist(
    root: Path,
    path: Path,
    policy: Path,
    ceiling: frozenset[str],
) -> tuple[dict[str, str], list[str]]:
    """Load and authenticate the closed repository migration debt."""
    errors: list[str] = []
    allowed: dict[str, str] = {}
    try:
        records = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return {}, [f"missing repository shader allowlist: {error}"]
    try:
        policy_text = policy.read_text(encoding="utf-8")
    except OSError as error:
        return {}, [f"missing repository shader policy: {error}"]
    tracked = git_files(root)
    packages = package_roots(root, tracked)
    for line_number, record in enumerate(records, 1):
        if not record or record.startswith("#"):
            continue
        fields = record.split("|")
        if len(fields) != 5:
            errors.append(f"malformed repository shader allowlist line {line_number}: {record}")
            continue
        kind, owner, source_path, symbol, row = fields
        resolved_owner = owner_for(source_path, packages)
        if (
            kind not in {"file", "include", "inline", "lowering"}
            or source_path.startswith(DELEGATED_PREFIX)
            or resolved_owner is None
            or resolved_owner[1] != owner
            or re.fullmatch(r"SH-[A-Z0-9-]+", row) is None
            or (kind == "file" and symbol != "-")
            or (kind != "file" and not symbol)
        ):
            errors.append(f"invalid repository shader allowlist line {line_number}: {record}")
            continue
        if record not in ceiling:
            errors.append(f"repository shader allowlist may only shrink: {record}")
            continue
        if row not in policy_text:
            errors.append(f"repository shader migration row is absent from policy: {row}")
        key = "|".join(fields[:4])
        if key in allowed:
            errors.append(f"duplicate repository shader allowlist record: {key}")
        allowed[key] = row
    return allowed, errors


def check_static(
    root: Path,
    allowlist_path: Path,
    validations_path: Path,
    policy_path: Path,
    ceiling: frozenset[str] = ALLOWLIST_CEILING,
) -> StaticResult:
    """Run all repository checks that do not require Cargo."""
    tracked = git_files(root)
    findings, errors = scan_repository(root, tracked)
    production, registry_errors = parse_registry(root, tracked)
    errors.extend(registry_errors)
    validations, validation_errors = parse_validations(
        root, tracked, production, validations_path
    )
    errors.extend(validation_errors)
    allowed, allowlist_errors = load_allowlist(root, allowlist_path, policy_path, ceiling)
    errors.extend(allowlist_errors)
    detected = {finding.key for finding in findings}
    for key in sorted(detected - allowed.keys()):
        errors.append(f"unlisted repository shader source: {key}")
    for key in sorted(allowed.keys() - detected):
        errors.append(f"stale repository shader allowlist record: {key}|{allowed[key]}")
    return StaticResult(findings, production, validations, len(allowed), errors)


def canonical(path: str | Path) -> str:
    """Canonicalize a manifest identity for Cargo metadata comparison."""
    return os.path.normcase(os.path.realpath(path))


def dependency_identity_errors(
    metadata: dict[str, object],
    owner: str,
    owner_manifest: Path,
    shader_manifest: Path,
) -> list[str]:
    """Authenticate the reserved extern name against Cargo's resolved graph."""
    packages = metadata.get("packages")
    members = metadata.get("workspace_members")
    resolve = metadata.get("resolve")
    if not isinstance(packages, list) or not isinstance(members, list) or not isinstance(resolve, dict):
        return ["shader validation dependency identity mismatch: metadata omits required graph fields"]
    owners = [
        package
        for package in packages
        if isinstance(package, dict)
        and package.get("name") == owner
        and canonical(str(package.get("manifest_path", ""))) == canonical(owner_manifest)
    ]
    shaders = [
        package
        for package in packages
        if isinstance(package, dict)
        and package.get("name") == "ember-shader"
        and canonical(str(package.get("manifest_path", ""))) == canonical(shader_manifest)
    ]
    if len(owners) != 1 or owners[0].get("id") not in members:
        return [f"shader validation dependency identity mismatch: no workspace owner {owner}"]
    if len(shaders) != 1 or shaders[0].get("id") not in members:
        return ["shader validation dependency identity mismatch: workspace ember-shader path differs"]
    nodes = resolve.get("nodes")
    if not isinstance(nodes, list):
        return ["shader validation dependency identity mismatch: resolved nodes are absent"]
    owner_nodes = [node for node in nodes if isinstance(node, dict) and node.get("id") == owners[0]["id"]]
    if len(owner_nodes) != 1:
        return [f"shader validation dependency identity mismatch: owner node is absent for {owner}"]
    dependencies = owner_nodes[0].get("deps")
    if not isinstance(dependencies, list):
        return [f"shader validation dependency identity mismatch: owner deps are absent for {owner}"]
    externs = [
        dependency
        for dependency in dependencies
        if isinstance(dependency, dict) and dependency.get("name") == "ember_shader"
    ]
    if len(externs) != 1 or externs[0].get("pkg") != shaders[0]["id"]:
        return [
            f"shader validation dependency identity mismatch: {owner} extern ember_shader does not resolve to workspace ember-shader"
        ]
    return []


def require_compiled_test(template: str, expected: str, listing: str) -> list[str]:
    """Require exactly one compiled test-harness fact."""
    count = sum(line == f"{expected}: test" for line in listing.splitlines())
    if count == 1:
        return []
    return [f"production template {template} is missing compiled validation test {expected}"]


def require_probe_receipt(template: str, nonce: str, output: str) -> list[str]:
    """Require one nonce-bound Naga receipt naming the undeclared identifier."""
    marker = f"SHADER-VALIDATION-PROBE {nonce} {Path(template).name} "
    receipts = [line for line in output.splitlines() if marker in line]
    matching = [line for line in receipts if PROBE_IDENTIFIER in line.split(marker, 1)[1]]
    if len(receipts) == 1 and len(matching) == 1:
        return []
    return [
        f"production template {template} validation probe must report exactly one naga diagnostic naming {PROBE_IDENTIFIER}"
    ]


def cargo(
    root: Path,
    arguments: list[str],
    *,
    probe: str | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run Cargo with deterministic probe-environment handling."""
    environment = os.environ.copy()
    environment["CARGO_NET_OFFLINE"] = "true"
    if probe is None:
        environment.pop(PROBE_ENV, None)
    else:
        environment[PROBE_ENV] = probe
    return subprocess.run(
        ["cargo", *arguments],
        cwd=root,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )


def check_compiled(root: Path, validations: list[Validation]) -> tuple[list[str], int, int, int]:
    """List and execute every compiled normal/probe validation pair."""
    if not validations:
        return [], 0, 0, 0
    errors: list[str] = []
    metadata_run = cargo(root, ["metadata", "--locked", "--format-version", "1"])
    if metadata_run.returncode != 0:
        return ["could not read locked workspace metadata for repository shader validation"], 0, 0, 0
    try:
        metadata = json.loads(metadata_run.stdout)
    except json.JSONDecodeError as error:
        return [f"locked Cargo metadata is unreadable: {error}"], 0, 0, 0
    listings: dict[str, str] = {}
    for validation in validations:
        errors.extend(
            dependency_identity_errors(
                metadata,
                validation.owner,
                root / validation.owner_manifest,
                root / SHADER_MANIFEST,
            )
        )
        if validation.owner not in listings:
            listing = cargo(
                root,
                ["test", "--locked", "-p", validation.owner, "--lib", "--", "--list"],
            )
            if listing.returncode != 0:
                errors.append(f"could not list native shader tests for {validation.owner}")
                listings[validation.owner] = ""
            else:
                listings[validation.owner] = listing.stdout
        errors.extend(
            require_compiled_test(
                validation.template, validation.expected_test, listings[validation.owner]
            )
        )
    if errors:
        return errors, 0, 0, 0

    normal_ms = 0
    probe_ms = 0
    passed = 0
    for validation in validations:
        started = time.monotonic_ns()
        normal = cargo(
            root,
            [
                "test",
                "--locked",
                "-p",
                validation.owner,
                "--lib",
                "--",
                "--exact",
                validation.expected_test,
                "--include-ignored",
            ],
        )
        normal_ms += (time.monotonic_ns() - started) // 1_000_000
        if normal.returncode != 0:
            errors.append(
                f"compiled shader validation test failed: {validation.owner} {validation.expected_test}"
            )
            continue
        nonce = f"{os.getpid()}-{time.time_ns()}"
        started = time.monotonic_ns()
        probe = cargo(
            root,
            [
                "test",
                "--locked",
                "-p",
                validation.owner,
                "--lib",
                "--",
                "--exact",
                validation.expected_test,
                "--include-ignored",
                "--nocapture",
            ],
            probe=nonce,
        )
        probe_ms += (time.monotonic_ns() - started) // 1_000_000
        combined = probe.stdout + probe.stderr
        if probe.returncode != 0:
            errors.append(
                f"compiled shader validation probe failed: {validation.owner} {validation.expected_test}"
            )
            continue
        receipt_errors = require_probe_receipt(validation.template, nonce, combined)
        errors.extend(receipt_errors)
        if not receipt_errors:
            passed += 1
    return errors, passed, normal_ms, probe_ms


def write(path: Path, text: str) -> None:
    """Write one self-test fixture, creating its parent directories."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def self_test() -> int:
    """Exercise every static refusal and each compiled-fact pure check."""
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="ember-repository-shader-") as directory:
        root = Path(directory)
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        write(
            root / "Cargo.toml",
            '[workspace]\nmembers = ["crates/ember-shader", "crates/example"]\nresolver = "2"\n',
        )
        write(
            root / SHADER_MANIFEST,
            '[package]\nname = "ember-shader"\nversion = "0.0.0"\nedition = "2024"\n',
        )
        write(
            root / "crates/example/Cargo.toml",
            '[package]\nname = "example"\nversion = "0.0.0"\nedition = "2024"\n'
            '[dependencies]\nember-shader = { path = "../ember-shader" }\n',
        )
        write(
            root / SHADER_RUNTIME,
            'pub const EXAMPLE_TEMPLATE: &str = "example.wgsl.jinja";\n'
            'const EXAMPLE_SOURCE: &str = include_str!("../templates/example.wgsl.jinja");\n'
            'const EMBEDDED_TEMPLATES: &[(&str, &str)] = &[(EXAMPLE_TEMPLATE, EXAMPLE_SOURCE)];\n',
        )
        write(root / SHADER_TEMPLATES / "example.wgsl.jinja", "@compute @workgroup_size(1) fn main() {}\n")
        valid_source = (
            "fn example_shader() { render_template(); }\n"
            "fn lower(shader: &ember_shader::RenderedShader) {\n"
            "    ShaderSource::Wgsl(shader.source().into());\n"
            "}\n"
            "fn lower_borrowed(shader: &ember_shader::RenderedShader) {\n"
            "    ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader.source()));\n"
            "}\n"
            "mod production_validation {\n"
            "    ::ember_shader::production_template_test!(\n"
            "        EXAMPLE_TEMPLATE,\n"
            "        example_shader,\n"
            "        production_template_example_shader_renders_and_validates,\n"
            "    );\n"
            "}\n"
        )
        write(root / "crates/example/src/shader.rs", valid_source)
        raw_path = "crates/example/src/legacy.wgsl"
        write(root / raw_path, "@compute @workgroup_size(1) fn legacy() {}\n")
        write(
            root / "crates/labs/julibrot/delegated.wgsl",
            "@compute @workgroup_size(1) fn delegated() {}\n",
        )
        write(root / POLICY_PATH, "SH-EXAMPLE\n")
        allowlist = root / "deploy/tests" / ALLOWLIST_NAME
        validations = root / "deploy/tests" / VALIDATIONS_NAME
        base_record = f"file|example|{raw_path}|-|SH-EXAMPLE"
        write(allowlist, base_record + "\n")
        write(
            validations,
            "crates/ember-shader/templates/example.wgsl.jinja|crates/example/src/shader.rs|EXAMPLE_TEMPLATE|example_shader\n",
        )
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)
        ceiling = frozenset({base_record})

        def check() -> StaticResult:
            return check_static(root, allowlist, validations, root / POLICY_PATH, ceiling)

        result = check()
        if result.errors:
            print(f"REPOSITORY SELF-TEST FAIL: valid fixture rejected: {result.errors}", file=sys.stderr)
            return 1

        def expect(label: str, needle: str) -> bool:
            failures = check().errors
            if not any(needle in failure for failure in failures):
                print(f"REPOSITORY SELF-TEST FAIL: {label}: {failures}", file=sys.stderr)
                return False
            return True

        source_path = root / "crates/example/src/shader.rs"
        cases = [
            (
                "relative macro path",
                valid_source.replace("::ember_shader::production_template_test!", "ember_shader::production_template_test!"),
                "requires exactly one lexical validation macro",
            ),
            (
                "commented macro",
                "fn example_shader() {}\n/* ::ember_shader::production_template_test!(EXAMPLE_TEMPLATE, example_shader, production_template_example_shader_renders_and_validates); */\n",
                "requires exactly one lexical validation macro",
            ),
            (
                "string macro",
                'fn example_shader() { let text = "::ember_shader::production_template_test!(EXAMPLE_TEMPLATE, example_shader, production_template_example_shader_renders_and_validates);"; }\n',
                "requires exactly one lexical validation macro",
            ),
            (
                "cfg macro",
                valid_source.replace(
                    "    ::ember_shader::production_template_test!",
                    "    #[cfg(any())]\n    ::ember_shader::production_template_test!",
                ),
                "production validation macro has a cfg attribute",
            ),
            (
                "hand-written test",
                valid_source + "fn production_template_example_shader_renders_and_validates() {}\n",
                "hand-written production validation test is forbidden",
            ),
            (
                "duplicate derived identifier",
                valid_source
                + "const production_template_example_shader_renders_and_validates: u32 = 0;\n",
                "derived validation identifier must occur exactly once",
            ),
            (
                "reserved extern alias",
                valid_source + "extern crate self as ember_shader;\n",
                "production validation extern name is rebound",
            ),
            (
                "reserved macro definition",
                valid_source + "macro_rules! production_template_test { () => {} }\n",
                "production validation macro is redefined",
            ),
            (
                "character literals do not hide refusals",
                "const OPEN: u8 = b'\"';\n" + valid_source + "extern crate self as r#ember_shader;\nconst CLOSE: u8 = b'\"';\n",
                "production validation extern name is rebound",
            ),
        ]
        for label, source, needle in cases:
            write(source_path, source)
            if not expect(label, needle):
                return 1
        write(source_path, valid_source)

        unpaired_runtime = (root / SHADER_RUNTIME).read_text(encoding="utf-8").replace(
            "&[(EXAMPLE_TEMPLATE, EXAMPLE_SOURCE)]", "&[]"
        )
        write(root / SHADER_RUNTIME, unpaired_runtime)
        if not expect("unregistered template", "absent from shared runtime registry"):
            return 1
        write(
            root / SHADER_RUNTIME,
            'pub const EXAMPLE_TEMPLATE: &str = "example.wgsl.jinja";\n'
            'const EXAMPLE_SOURCE: &str = include_str!("../templates/example.wgsl.jinja");\n'
            'const EMBEDDED_TEMPLATES: &[(&str, &str)] = &[(EXAMPLE_TEMPLATE, EXAMPLE_SOURCE)];\n',
        )

        inventory_cases = [
            ("include", 'const SOURCE: &str = include_str!("extra.wgsl");\n', "unlisted repository shader source: include"),
            ("inline", 'const SOURCE: &str = r"@vertex fn main() {}";\n', "unlisted repository shader source: inline"),
            ("lowering", "fn direct(source: String) { ShaderSource::Wgsl(source.into()); }\n", "unlisted repository shader source: lowering"),
            (
                "raw lowering followed by typed lowering",
                "fn mixed(raw: String, shader: &ember_shader::RenderedShader) {\n"
                "    ShaderSource::Wgsl(raw.into());\n"
                "    ShaderSource::Wgsl(shader.source().into());\n"
                "}\n",
                "unlisted repository shader source: lowering",
            ),
            (
                "raw lowering followed by typed text in a comment",
                "fn commented(raw: String, shader: &ember_shader::RenderedShader) {\n"
                "    ShaderSource::Wgsl(raw.into());\n"
                "    // shader.source().into() must not attest the raw argument.\n"
                "}\n",
                "unlisted repository shader source: lowering",
            ),
            (
                "rendered shader parameter shadowed by a local binding",
                "fn shadowed(shader: &ember_shader::RenderedShader, raw: String) {\n"
                "    let shader = Raw(raw);\n"
                "    ShaderSource::Wgsl(shader.source().into());\n"
                "}\n",
                "unlisted repository shader source: lowering",
            ),
            (
                "rendered shader parameter shadowed by a closure parameter",
                "fn closure_shadow(shader: &ember_shader::RenderedShader) {\n"
                "    let lower = |shader| ShaderSource::Wgsl(shader.source().into());\n"
                "}\n",
                "unlisted repository shader source: lowering",
            ),
        ]
        extra = root / "crates/example/src/extra.rs"
        for label, source, needle in inventory_cases:
            write(extra, source)
            subprocess.run(["git", "-C", str(root), "add", str(extra.relative_to(root))], check=True)
            if not expect(f"unlisted {label}", needle):
                return 1
            extra.unlink()
            subprocess.run(
                ["git", "-C", str(root), "rm", "--cached", "-q", str(extra.relative_to(root))],
                check=True,
            )

        write(allowlist, base_record + "\nfile|example|crates/example/src/missing.wgsl|-|SH-EXAMPLE\n")
        if not expect("expanded ceiling", "allowlist may only shrink"):
            return 1
        write(allowlist, base_record.replace("|example|", "|wrong-owner|") + "\n")
        if not expect("wrong owner", "invalid repository shader allowlist"):
            return 1
        write(root / POLICY_PATH, "SH-OTHER\n")
        write(allowlist, base_record + "\n")
        if not expect("missing migration row", "migration row is absent from policy"):
            return 1
        write(root / POLICY_PATH, "SH-EXAMPLE\n")
        write(allowlist, "")
        if not expect("unlisted file", "unlisted repository shader source: file"):
            return 1
        write(allowlist, base_record + "\n")
        subprocess.run(["git", "-C", str(root), "rm", "-q", "-f", raw_path], check=True)
        if not expect("stale record", "stale repository shader allowlist record"):
            return 1

        template = "crates/ember-shader/templates/example.wgsl.jinja"
        expected = "shader::production_validation::production_template_example_shader_renders_and_validates"
        if require_compiled_test(template, expected, expected + ": test"):
            print("REPOSITORY SELF-TEST FAIL: compiled test fact rejected", file=sys.stderr)
            return 1
        if not require_compiled_test(template, expected, "unrelated: test"):
            print("REPOSITORY SELF-TEST FAIL: missing compiled test fact accepted", file=sys.stderr)
            return 1
        receipt = f"SHADER-VALIDATION-PROBE nonce example.wgsl.jinja error: {PROBE_IDENTIFIER}"
        if require_probe_receipt(template, "nonce", receipt):
            print("REPOSITORY SELF-TEST FAIL: valid probe receipt rejected", file=sys.stderr)
            return 1
        if not require_probe_receipt(template, "nonce", receipt.replace(PROBE_IDENTIFIER, "forged")):
            print("REPOSITORY SELF-TEST FAIL: forged probe receipt accepted", file=sys.stderr)
            return 1

        workspace = Path("/workspace")
        metadata = {
            "packages": [
                {
                    "id": "example",
                    "name": "example",
                    "manifest_path": str(workspace / "crates/example/Cargo.toml"),
                },
                {
                    "id": "shader",
                    "name": "ember-shader",
                    "manifest_path": str(workspace / SHADER_MANIFEST),
                },
                {
                    "id": "forged",
                    "name": "forged",
                    "manifest_path": str(workspace / "crates/forged/Cargo.toml"),
                },
            ],
            "workspace_members": ["example", "shader"],
            "resolve": {"nodes": [{"id": "example", "deps": [{"name": "ember_shader", "pkg": "shader"}]}]},
        }
        identity = dependency_identity_errors(
            metadata,
            "example",
            workspace / "crates/example/Cargo.toml",
            workspace / SHADER_MANIFEST,
        )
        if identity:
            print(f"REPOSITORY SELF-TEST FAIL: genuine dependency rejected: {identity}", file=sys.stderr)
            return 1
        metadata["resolve"] = {
            "nodes": [{"id": "example", "deps": [{"name": "ember_shader", "pkg": "forged"}]}]
        }
        if not dependency_identity_errors(
            metadata,
            "example",
            workspace / "crates/example/Cargo.toml",
            workspace / SHADER_MANIFEST,
        ):
            print("REPOSITORY SELF-TEST FAIL: forged dependency accepted", file=sys.stderr)
            return 1

        validation = result.validations[0]
        compiled_metadata = {
            "packages": [
                {
                    "id": "example",
                    "name": "example",
                    "manifest_path": str(root / "crates/example/Cargo.toml"),
                },
                {
                    "id": "shader",
                    "name": "ember-shader",
                    "manifest_path": str(root / SHADER_MANIFEST),
                },
            ],
            "workspace_members": ["example", "shader"],
            "resolve": {
                "nodes": [
                    {
                        "id": "example",
                        "deps": [{"name": "ember_shader", "pkg": "shader"}],
                    }
                ]
            },
        }
        original_cargo = globals()["cargo"]

        def fake_cargo(
            _root: Path,
            arguments: list[str],
            *,
            probe: str | None = None,
        ) -> subprocess.CompletedProcess[str]:
            if "--locked" not in arguments:
                return subprocess.CompletedProcess(arguments, 1, "", "missing --locked")
            if arguments[0] == "metadata":
                return subprocess.CompletedProcess(arguments, 0, json.dumps(compiled_metadata), "")
            if "--list" in arguments:
                return subprocess.CompletedProcess(
                    arguments, 0, validation.expected_test + ": test\n", ""
                )
            if probe is not None:
                receipt = (
                    f"SHADER-VALIDATION-PROBE {probe} example.wgsl.jinja "
                    f"error: {PROBE_IDENTIFIER}\n"
                )
                return subprocess.CompletedProcess(arguments, 0, receipt, "")
            return subprocess.CompletedProcess(arguments, 0, "test passed\n", "")

        try:
            globals()["cargo"] = fake_cargo
            compiled_errors, compiled_passed, _, _ = check_compiled(root, [validation])
            if compiled_errors or compiled_passed != 1:
                print(
                    f"REPOSITORY SELF-TEST FAIL: valid compiled pair rejected: {compiled_errors}",
                    file=sys.stderr,
                )
                return 1

            def failing_normal(
                fake_root: Path,
                arguments: list[str],
                *,
                probe: str | None = None,
            ) -> subprocess.CompletedProcess[str]:
                result = fake_cargo(fake_root, arguments, probe=probe)
                if arguments[0] == "test" and "--list" not in arguments and probe is None:
                    return subprocess.CompletedProcess(arguments, 1, "", "normal failed")
                return result

            globals()["cargo"] = failing_normal
            compiled_errors, _, _, _ = check_compiled(root, [validation])
            if not any("compiled shader validation test failed" in error for error in compiled_errors):
                print("REPOSITORY SELF-TEST FAIL: normal validation failure accepted", file=sys.stderr)
                return 1

            def forged_probe(
                fake_root: Path,
                arguments: list[str],
                *,
                probe: str | None = None,
            ) -> subprocess.CompletedProcess[str]:
                result = fake_cargo(fake_root, arguments, probe=probe)
                if probe is not None:
                    return subprocess.CompletedProcess(
                        arguments,
                        0,
                        f"SHADER-VALIDATION-PROBE {probe} example.wgsl.jinja forged\n",
                        "",
                    )
                return result

            globals()["cargo"] = forged_probe
            compiled_errors, _, _, _ = check_compiled(root, [validation])
            if not any("naga diagnostic naming" in error for error in compiled_errors):
                print("REPOSITORY SELF-TEST FAIL: forged compiled probe accepted", file=sys.stderr)
                return 1
        finally:
            globals()["cargo"] = original_cargo

    elapsed = int(time.monotonic() - started)
    print(
        "REPOSITORY SHADER SELF-TEST PASS: crate ownership, Julibrot delegation, raw files, "
        "WGSL includes, literals, lowerings and binding shadowing, exact macro pairing, cfg, "
        "comments, strings, "
        "character literals, reserved extern and macro names, Cargo identity, compiled listing, "
        f"normal and probe runs, probe receipts, stale debt and shrink-only ceiling, {elapsed}s"
    )
    return 0


def main() -> int:
    """Run inventory, self-test, or the full repository policy check."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path)
    parser.add_argument("--inventory", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    if arguments.root is None:
        parser.error("--root is required outside self-test mode")
    root = arguments.root.resolve()
    if arguments.inventory:
        tracked = git_files(root)
        findings, errors = scan_repository(root, tracked)
        for error in errors:
            print(error, file=sys.stderr)
        for finding in sorted(findings):
            print(finding.key)
        return int(bool(errors))
    tests = root / "deploy/tests"
    started = time.monotonic_ns()
    result = check_static(
        root,
        tests / ALLOWLIST_NAME,
        tests / VALIDATIONS_NAME,
        root / POLICY_PATH,
    )
    for error in result.errors:
        print(error, file=sys.stderr)
    if result.errors:
        elapsed_ms = (time.monotonic_ns() - started) // 1_000_000
        print(
            f"REPOSITORY SHADER CHECK FAIL: static policy violation, total {elapsed_ms}ms",
            file=sys.stderr,
        )
        return 1
    errors, passed, normal_ms, probe_ms = check_compiled(root, result.validations)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        elapsed_ms = (time.monotonic_ns() - started) // 1_000_000
        print(
            f"REPOSITORY SHADER CHECK FAIL: compiled validation violation, total {elapsed_ms}ms",
            file=sys.stderr,
        )
        return 1
    elapsed_ms = (time.monotonic_ns() - started) // 1_000_000
    print(
        "REPOSITORY SHADER CHECK PASS: "
        f"{passed}/{len(result.production_templates)} templates compiled and probed, "
        f"normal {normal_ms}ms, probe {probe_ms}ms, {result.allowlist_count} migration records, "
        f"total {elapsed_ms}ms"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
