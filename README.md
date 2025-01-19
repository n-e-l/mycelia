# Rel
Relational note taking

## Building
1. Checkout this repository
2. `cargo build --release`
3. Create an environment file `.env` next to the executable in `target/release/`

Example `.env` file:
```bash
NEO_URI=neo4j://example.org
NEO_USERNAME=neo4j
NEO_PASSWORD=password
```

## Running
```./target/release/rel```