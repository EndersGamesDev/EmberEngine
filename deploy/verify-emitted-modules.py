#!/usr/bin/env python3
"""Verify or materialize assembled modules from compiler-emitted bytes."""

import argparse
import pathlib
import re


LOADER_TOKEN = re.compile(rb"(loader\.js)\?v=1(?![0-9])")


def expected_bytes(path: pathlib.Path, stamp: bytes, stamped: bool) -> bytes:
    emitted = path.read_bytes()
    if not stamped:
        return emitted
    expected, count = LOADER_TOKEN.subn(rb"\1?v=" + stamp, emitted)
    if count != 1:
        raise SystemExit(
            f"FAILED: stamped emitted module must contain exactly one loader cache token: {path}"
        )
    return expected


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--emitted-root", required=True, type=pathlib.Path)
    parser.add_argument("--assembled-root", required=True, type=pathlib.Path)
    parser.add_argument("--stamp", required=True)
    parser.add_argument("--module", action="append", required=True)
    parser.add_argument("--stamped", action="append", default=[])
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()

    modules = set(args.module)
    stamped = set(args.stamped)
    if not stamped <= modules:
        unknown = ", ".join(sorted(stamped - modules))
        raise SystemExit(f"FAILED: stamped module is absent from the emitted copy list: {unknown}")

    for relative in args.module:
        expected = expected_bytes(
            args.emitted_root / relative,
            args.stamp.encode("utf-8"),
            relative in stamped,
        )
        assembled = args.assembled_root / relative
        if args.write:
            assembled.parent.mkdir(parents=True, exist_ok=True)
            assembled.write_bytes(expected)
        elif not assembled.is_file() or assembled.read_bytes() != expected:
            raise SystemExit(
                f"FAILED: assembled module does not match its emitted expectation: {relative}"
            )
    action = "materialized" if args.write else "verified"
    print(f"   {action} {len(args.module)} emitted module(s), {len(stamped)} stamped")


if __name__ == "__main__":
    main()
