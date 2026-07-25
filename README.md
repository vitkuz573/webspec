# webspec

Universal spec-to-code generator for web scraping SDKs.

Parses [webspec protocol](https://github.com/vitkuz573/webspec-proto) YAML specifications and generates typed client SDKs in multiple languages.

## Supported Languages

- **Rust** — Full async client with error handling
- **TypeScript** — Typed interfaces and fetch-based client
- **Python** — Dataclasses with httpx client

## Installation

```bash
cargo install webspec
```

Or build from source:

```bash
git clone https://github.com/vitkuz573/webspec
cd webspec
cargo build --release
```

## Usage

```bash
# Validate a spec
webspec validate --spec my-service.yaml

# Generate Rust SDK
webspec generate --spec my-service.yaml --target rust --output ./sdk/

# Generate TypeScript SDK
webspec generate --spec my-service.yaml --target typescript --output ./sdk/

# Generate Python SDK
webspec generate --spec my-service.yaml --target python --output ./sdk/

# List available targets
webspec list-targets

# Dry-run (print to stdout without writing)
webspec generate --spec my-service.yaml --target rust --output ./sdk/ --dry-run

# Watch mode (re-generate on file changes)
webspec watch --spec my-service.yaml --target rust --output ./sdk/
```

## CLI Options

| Option | Description |
|--------|-------------|
| `-v, --verbose` | Enable verbose output |
| `-q, --quiet` | Suppress non-error output |
| `--dry-run` | Print generated code to stdout without writing files |

## Subcommands

| Command | Description |
|---------|-------------|
| `generate` | Generate SDK from a spec |
| `validate` | Validate a spec file |
| `list-targets` | List available language targets |
| `watch` | Re-generate on spec file changes |

## How It Works

1. Parse YAML spec (entities, pages, types, enums)
2. Resolve type references and newtypes
3. Generate language-specific code using templates
4. Write output to the specified directory

## Project Structure

```
webspec/
├── Cargo.toml
└── src/
    ├── main.rs          # CLI entry point
    ├── lib.rs           # Core library
    ├── spec.rs          # YAML spec parser
    ├── validation.rs    # Spec validation
    ├── emitter.rs       # Code emitter
    ├── traits.rs        # Generator trait
    └── generators/
        ├── mod.rs
        ├── rust.rs      # Rust code generator
        ├── typescript.rs # TypeScript code generator
        └── python.rs    # Python code generator
```

## Protocol Specification

See [webspec-proto](https://github.com/vitkuz573/webspec-proto) for the full protocol specification, JSON Schema, and examples.

## License

MIT
