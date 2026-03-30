#!/usr/bin/env python3
"""
TOML read/write helper for deployment.toml management.

Uses Python 3.11+ built-in tomllib for reading.
For writing, uses tomli_w if available, otherwise minimal TOML serializer.

CLI interface:
    python3 toml_helper.py read  <file> <dotted.key.path>
    python3 toml_helper.py write <file> <dotted.key.path> <value>
    python3 toml_helper.py add-verifier <file> <chain-key> [--name X] [--version X] \
        [--selector X] [--verifier X] [--estop X] [--unroutable true/false]
    python3 toml_helper.py update-verifier <file> <chain-key> --selector X --field F --value V
    python3 toml_helper.py get-verifier-field <file> <chain-key> --selector X --field F
    python3 toml_helper.py verifier-count <file> <chain-key>
    python3 toml_helper.py verifier-rows <file> <chain-key>
    python3 toml_helper.py init-chain <file> <chain-key> --name "Stellar Testnet"
"""

import argparse
import json
import os
import re
import sys

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ImportError:
        print("error: Python 3.11+ or 'tomli' package required", file=sys.stderr)
        sys.exit(1)


# ---------------------------------------------------------------------------
# Minimal TOML writer (the schema is simple and fixed)
# ---------------------------------------------------------------------------


def _toml_quote(value):
    """Quote a string value for TOML."""
    return '"' + str(value).replace("\\", "\\\\").replace('"', '\\"') + '"'


def _toml_format_value(value):
    """Format a Python value as a TOML value string."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, str):
        return _toml_quote(value)
    if isinstance(value, list):
        inner = ", ".join(_toml_format_value(v) for v in value)
        return f"[{inner}]"
    if isinstance(value, dict):
        inner = ", ".join(
            f"{k} = {_toml_format_value(v)}" for k, v in value.items()
        )
        return f"{{{inner}}}"
    return _toml_quote(str(value))


def _write_toml_section(lines, data, prefix=""):
    """Recursively write TOML sections."""
    # Separate inline values, sub-tables, and array-of-tables
    inline_keys = []
    table_keys = []
    array_table_keys = []
    for key, value in data.items():
        if isinstance(value, list) and value and isinstance(value[0], dict):
            array_table_keys.append(key)
        elif isinstance(value, dict) and not _is_inline_table(value):
            table_keys.append(key)
        else:
            inline_keys.append(key)

    # Write inline key-value pairs
    for key in inline_keys:
        lines.append(f"{key} = {_toml_format_value(data[key])}")

    # Write sub-tables
    for key in table_keys:
        full_key = f"{prefix}.{key}" if prefix else key
        # Only emit a section header if this table has its own inline keys
        sub_data = data[key]
        has_inline = any(
            not isinstance(v, dict) or _is_inline_table(v)
            for v in sub_data.values()
            if not (isinstance(v, list) and v and isinstance(v[0], dict))
        )
        if has_inline:
            lines.append("")
            lines.append(f"[{full_key}]")
        _write_toml_section(lines, sub_data, full_key)

    # Write array-of-tables
    for key in array_table_keys:
        full_key = f"{prefix}.{key}" if prefix else key
        for item in data[key]:
            lines.append("")
            lines.append(f"[[{full_key}]]")
            _write_toml_section(lines, item, "")


def _is_inline_table(value):
    """Check if a dict should be written as an inline table.

    Only small dicts with <= 2 simple keys are inline. Anything larger
    or with nested structures is written as a TOML section.
    """
    if not isinstance(value, dict):
        return False
    if len(value) > 2:
        return False
    return all(not isinstance(v, (dict, list)) for v in value.values())


def write_toml(data, filepath):
    """Write a dict to a TOML file."""
    try:
        import tomli_w

        with open(filepath, "wb") as f:
            tomli_w.dump(data, f)
        return
    except ImportError:
        pass

    lines = []
    # Write top-level comment if present in original file
    if os.path.exists(filepath):
        with open(filepath, "r") as f:
            for line in f:
                if line.startswith("#"):
                    lines.append(line.rstrip())
                else:
                    break
        if lines:
            lines.append("")

    _write_toml_section(lines, data)
    lines.append("")  # trailing newline

    with open(filepath, "w") as f:
        f.write("\n".join(lines))


def read_toml(filepath):
    """Read a TOML file and return a dict."""
    with open(filepath, "rb") as f:
        return tomllib.load(f)


# ---------------------------------------------------------------------------
# Key path operations
# ---------------------------------------------------------------------------


def get_by_path(data, key_path):
    """Get a value from a nested dict using dotted key path."""
    keys = key_path.split(".")
    current = data
    for key in keys:
        if not isinstance(current, dict) or key not in current:
            return None
        current = current[key]
    return current


def set_by_path(data, key_path, value):
    """Set a value in a nested dict using dotted key path, creating intermediates."""
    keys = key_path.split(".")
    current = data
    for key in keys[:-1]:
        if key not in current or not isinstance(current[key], dict):
            current[key] = {}
        current = current[key]
    current[keys[-1]] = value


def parse_value(raw):
    """Parse a string value into a Python type."""
    if raw == "true":
        return True
    if raw == "false":
        return False
    if re.fullmatch(r"[+-]?[0-9]+", raw):
        # Preserve strings like selectors with leading zeros.
        unsigned = raw.lstrip("+-")
        if len(unsigned) > 1 and unsigned.startswith("0"):
            return raw
        return int(raw)
    return raw


def parse_bool_arg(raw, flag_name):
    """Parse a strict true/false string into bool, exiting on invalid input."""
    if raw == "true":
        return True
    if raw == "false":
        return False
    print(
        f"error: invalid value for {flag_name}: '{raw}' (expected 'true' or 'false')",
        file=sys.stderr,
    )
    sys.exit(1)


def get_chain(data, chain_key):
    """Return chain table by key or exit with an error."""
    chain_path = f"chains.{chain_key}"
    chain = get_by_path(data, chain_path)
    if chain is None:
        print(
            f"error: chain '{chain_key}' not found in config",
            file=sys.stderr,
        )
        sys.exit(1)
    return chain


def find_verifier(chain, selector):
    """Return verifier dict matching selector or None."""
    for verifier in chain_verifiers(chain):
        if verifier.get("selector") == selector:
            return verifier
    return None


def chain_verifiers(chain):
    """Return verifier list for a chain."""
    return chain.get("verifiers", [])


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_read(args):
    """Read a value from the TOML file."""
    data = read_toml(args.file)
    value = get_by_path(data, args.key)
    if value is None:
        print(f"error: key '{args.key}' not found", file=sys.stderr)
        sys.exit(1)
    if isinstance(value, (dict, list)):
        print(json.dumps(value))
    else:
        print(value)


def cmd_write(args):
    """Write a value to the TOML file."""
    data = read_toml(args.file)
    set_by_path(data, args.key, parse_value(args.value))
    write_toml(data, args.file)


def cmd_add_verifier(args):
    """Add a verifier entry to a chain's verifier list."""
    data = read_toml(args.file)
    chain = get_chain(data, args.chain_key)
    verifiers = chain.setdefault("verifiers", [])

    entry = {}
    for field in ("name", "version", "selector", "verifier", "estop"):
        value = getattr(args, field)
        if value:
            entry[field] = value
    if args.unroutable is not None:
        entry["unroutable"] = parse_bool_arg(args.unroutable, "--unroutable")

    verifiers.append(entry)
    write_toml(data, args.file)


def cmd_update_verifier(args):
    """Update a field on a verifier entry matched by selector."""
    data = read_toml(args.file)
    chain = get_chain(data, args.chain_key)
    verifier = find_verifier(chain, args.selector)

    if verifier is None:
        print(
            f"error: verifier with selector '{args.selector}' not found",
            file=sys.stderr,
        )
        sys.exit(1)

    if args.field == "unroutable":
        value = parse_bool_arg(args.value, "--value")
    else:
        value = parse_value(args.value)

    verifier[args.field] = value
    write_toml(data, args.file)


def cmd_init_chain(args):
    """Initialize a new chain entry."""
    data = read_toml(args.file)
    chain_path = f"chains.{args.chain_key}"
    existing = get_by_path(data, chain_path)
    if existing is not None:
        print(
            f"error: chain '{args.chain_key}' already exists",
            file=sys.stderr,
        )
        sys.exit(1)

    chain_data = {
        "name": args.name,
        "admin": "",
        "router": "",
        "timelock-controller": "",
        "timelock-delay": 0,
    }
    set_by_path(data, chain_path, chain_data)
    write_toml(data, args.file)


def cmd_get_verifier_field(args):
    """Print a single field from verifier selected by selector."""
    data = read_toml(args.file)
    chain = get_chain(data, args.chain_key)
    verifier = find_verifier(chain, args.selector)

    if verifier is None:
        print("")
        return

    value = verifier.get(args.field, "")
    if isinstance(value, (dict, list)):
        print(json.dumps(value))
    else:
        print(value)


def cmd_verifier_count(args):
    """Print number of configured verifiers for a chain."""
    data = read_toml(args.file)
    chain = get_chain(data, args.chain_key)
    print(len(chain_verifiers(chain)))


def cmd_verifier_rows(args):
    """Print verifiers as pipe-delimited rows for shell consumption."""
    data = read_toml(args.file)
    chain = get_chain(data, args.chain_key)
    for verifier in chain_verifiers(chain):
        print(
            f"{verifier.get('name', '?')}|"
            f"{verifier.get('selector', '?')}|"
            f"{verifier.get('verifier', '?')}|"
            f"{verifier.get('estop', '?')}|"
            f"{verifier.get('unroutable', '?')}"
        )


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description="TOML helper for deployment.toml management"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # read
    p_read = subparsers.add_parser("read", help="Read a value")
    p_read.add_argument("file", help="Path to TOML file")
    p_read.add_argument("key", help="Dotted key path (e.g. chains.stellar-testnet.router)")

    # write
    p_write = subparsers.add_parser("write", help="Write a value")
    p_write.add_argument("file", help="Path to TOML file")
    p_write.add_argument("key", help="Dotted key path")
    p_write.add_argument("value", help="Value to write")

    # add-verifier
    p_av = subparsers.add_parser("add-verifier", help="Add a verifier entry")
    p_av.add_argument("file", help="Path to TOML file")
    p_av.add_argument("chain_key", metavar="chain-key", help="Chain key")
    p_av.add_argument("--name", help="Verifier name")
    p_av.add_argument("--version", help="Verifier version")
    p_av.add_argument("--selector", help="Verifier selector (hex)")
    p_av.add_argument("--verifier", help="Verifier contract ID")
    p_av.add_argument("--estop", help="Emergency stop contract ID")
    p_av.add_argument("--unroutable", help="Whether verifier is unroutable (true/false)")

    # update-verifier
    p_uv = subparsers.add_parser("update-verifier", help="Update a verifier field")
    p_uv.add_argument("file", help="Path to TOML file")
    p_uv.add_argument("chain_key", metavar="chain-key", help="Chain key")
    p_uv.add_argument("--selector", required=True, help="Verifier selector to match")
    p_uv.add_argument("--field", required=True, help="Field to update")
    p_uv.add_argument("--value", required=True, help="New value")

    # get-verifier-field
    p_gvf = subparsers.add_parser(
        "get-verifier-field",
        help="Read one verifier field by selector",
    )
    p_gvf.add_argument("file", help="Path to TOML file")
    p_gvf.add_argument("chain_key", metavar="chain-key", help="Chain key")
    p_gvf.add_argument("--selector", required=True, help="Verifier selector to match")
    p_gvf.add_argument("--field", required=True, help="Field to read")

    # verifier-count
    p_vc = subparsers.add_parser("verifier-count", help="Count verifiers for a chain")
    p_vc.add_argument("file", help="Path to TOML file")
    p_vc.add_argument("chain_key", metavar="chain-key", help="Chain key")

    # verifier-rows
    p_vr = subparsers.add_parser(
        "verifier-rows",
        help="Emit pipe-delimited verifier rows",
    )
    p_vr.add_argument("file", help="Path to TOML file")
    p_vr.add_argument("chain_key", metavar="chain-key", help="Chain key")

    # init-chain
    p_ic = subparsers.add_parser("init-chain", help="Initialize a chain entry")
    p_ic.add_argument("file", help="Path to TOML file")
    p_ic.add_argument("chain_key", metavar="chain-key", help="Chain key")
    p_ic.add_argument("--name", required=True, help="Chain display name")

    args = parser.parse_args()

    commands = {
        "read": cmd_read,
        "write": cmd_write,
        "add-verifier": cmd_add_verifier,
        "update-verifier": cmd_update_verifier,
        "get-verifier-field": cmd_get_verifier_field,
        "verifier-count": cmd_verifier_count,
        "verifier-rows": cmd_verifier_rows,
        "init-chain": cmd_init_chain,
    }

    commands[args.command](args)


if __name__ == "__main__":
    main()
