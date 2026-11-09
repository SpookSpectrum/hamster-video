# Contributing

Issues and pull requests are welcome. Describe the problem, expected behavior and a reproducible example. Use publicly accessible sample media that you have permission to download.

For code changes, run `cargo test --release --locked` and `cargo fmt --check`. Test any changed download behavior with the local fixtures when possible. Avoid committing build products, downloaded personal media, account data or credentials.

Keep user URLs as separate process arguments and preserve `--ignore-config` and the `--` argument boundary. Keep upstream licence notices with redistributed tools. Clearly distinguish tested behavior from assumptions.

Current useful improvements include accessibility, more comprehensive playlist/subtitle tests, packaging automation and a separately validated Apple Silicon build.
