# scriptor

Archivum
Codex
Folio
Fragmentum

## Testing

Run all tests:

```bash
cargo test --all-targets
```

Run tests with coverage (requires [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)):

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage
```

Open `coverage/index.html` to view the coverage report. The test suite targets 90%+ coverage on testable modules.