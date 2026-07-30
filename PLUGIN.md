# webspec Plugin Protocol

This document describes the JSON-stdio protocol used by `webspec` to communicate with external generator plugins.

## Overview

An external plugin is an executable named `webspec-<target>` that reads a single JSON object from stdin and writes a single JSON object to stdout. The CLI discovers such executables on `PATH` and in the `WEBSPEC_PLUGIN_DIR` directory, or accepts an explicit path via the `--plugin` flag.

## Protocol Version

The current protocol version is `1.0.0`. Both the request and response are expected to carry a matching version indicator. If a plugin receives a request with an unsupported protocol version, it should return the `unsupported_protocol_version` field in its response.

## Request Format

The CLI sends a `GenerateRequest` object:

```json
{
  "protocol_version": "1.0.0",
  "target": "rust",
  "spec": { ... },
  "output_dir": "/path/to/output",
  "options": {}
}
```

| Field | Type | Description |
|-------|------|-------------|
| `protocol_version` | string | Must be `"1.0.0"`. |
| `target` | string | Target language or generator name. |
| `spec` | object | The parsed webspec as a JSON value. |
| `output_dir` | string | Absolute path where generated files should be written. |
| `options` | object | Optional target-specific options. |

## Response Format

The plugin must write a `GenerateResponse` object to stdout:

```json
{
  "files": [
    { "path": "src/lib.rs", "content": "..." }
  ],
  "diagnostics": [
    { "severity": "error", "message": "...", "path": "pages.home" }
  ],
  "unsupported_protocol_version": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `files` | array | Generated files, each with a relative `path` and `content`. |
| `diagnostics` | array | Structured diagnostic messages. |
| `unsupported_protocol_version` | string? | Set when the plugin cannot handle the request protocol version. |

### Diagnostics

Each diagnostic has:

```json
{
  "severity": "error",
  "message": "missing field: entity",
  "path": "pages.home"
}
```

Severity values are `error`, `warning`, or `info`. If any diagnostic has severity `error`, the CLI exits with a non-zero status.

## File Paths

Returned paths must be relative to `output_dir`. Absolute paths and paths containing `..` are rejected by the CLI.

## Environment

The CLI runs external plugins with a sanitized environment that inherits only the following variables:

- `PATH`
- `HOME`
- `WEBSPEC_PLUGIN_DIR`
- `RUST_LOG`
- `TMPDIR`, `TEMP`, `USERPROFILE` (platform temporary directories)

## Discovery

The CLI searches for executables matching `webspec-<target>` on `PATH` and in `WEBSPEC_PLUGIN_DIR`. On Windows, the `.exe` extension is optional. Built-in plugins (`rust`, `typescript`, `python`) take precedence over discovered plugins unless an explicit `--plugin` path is provided.

## Example Minimal Plugin (Python)

```python
#!/usr/bin/env python3
import json, sys

request = json.load(sys.stdin)

if request.get("protocol_version") != "1.0.0":
    json.dump({
        "files": [],
        "diagnostics": [{"severity": "error", "message": "unsupported protocol version"}],
        "unsupported_protocol_version": request.get("protocol_version")
    }, sys.stdout)
    sys.exit(0)

spec = request["spec"]
name = spec.get("name", "unknown")

json.dump({
    "files": [{"path": "README.md", "content": f"# {name}\n"}],
    "diagnostics": []
}, sys.stdout)
```

Save this as `webspec-readme`, make it executable, place it on `PATH`, and run `webspec generate --target readme --spec <spec.yaml> --output <dir>`.

## Listing Plugins

Use `webspec list-plugins` to see all registered built-in and discovered plugins.
