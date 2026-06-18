# embedb

An embedded, persistent vector store for Rust — no server, no config, no credentials.

> **Status: early development.** The storage foundation is in place; vector insert/search APIs are not yet implemented. Not production-ready.

---

## Why

Vector stores haven't had their SQLite moment yet. The current landscape forces a choice between managed cloud services, self-hosted server processes, or low-level libraries with hostile APIs. There's no option that's just a library you drop into a project.

`embedb` aims to be that library: in-process, single-file persistence, metadata filtering, and a clean API — fast enough for applications up to ~1–5M vectors.

## Planned API

### Rust

```rust
use embedb::EmbedBClient;

let db = EmbedBClient::new("./db")?;

// Insert a vector with arbitrary metadata
db.insert("doc-123", &embedding, json!({"source": "invoice", "date": "2024-03-01"}))?;

// ANN search with optional metadata filter
let results = db.search(&query_embedding, 5, Some(json!({"source": "invoice"})))?;

for result in results {
    println!("{} {:.4} {:?}", result.id, result.score, result.metadata);
}

// Namespaces — multiple independent indexes in one file
let invoices = db.namespace("invoices");
let emails   = db.namespace("emails");

// Delete and upsert
db.delete("doc-123")?;
db.upsert("doc-123", &new_embedding, json!({"source": "invoice"}))?;
```

### Python

Python bindings are planned for v0.2 via PyO3.

```python
import embedb

# Open or create a store (single file)
db = embedb.open("./db")

# Insert a vector with arbitrary metadata
db.insert("doc-123", embedding, {"source": "invoice", "date": "2024-03-01", "amount": 412.50})

# ANN search with optional metadata filter
results = db.search(
    query_embedding,
    k=5,
    filter={"source": "invoice", "date": {"$gte": "2024-01-01"}}
)

for r in results:
    print(r.id, r.score, r.metadata)

# Namespaces — multiple independent indexes in one file
invoices = db.namespace("invoices")
emails   = db.namespace("emails")

# Delete and upsert
db.delete("doc-123")
db.upsert("doc-123", new_embedding, {"source": "invoice"})
```

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
embedb = "0.1"
```

Requires Rust 1.85 or later (Rust 2024 edition).

## Architecture

`embedb` uses a hybrid storage layout:

- **Raw vectors** — stored in a memory-mapped flat file (`store.embedb`) that grows automatically: doubles up to 1 GB, then in fixed 1 MB increments.
- **Metadata** — stored in a SQLite database alongside the vector file, giving crash-safe durability and transactional consistency via WAL mode.

The index algorithm follows a progressive strategy: brute-force (exact) search up to a configurable threshold, then HNSW above it — transparent to the caller.

## Roadmap

**v0.1 — MVP**
- [ ] Flat (brute-force) index with cosine, dot product, and L2 distance
- [ ] SQLite-backed metadata store
- [ ] Equality-based metadata filtering (post-filter)
- [ ] `insert` / `search` / `delete` / `upsert` API

**v0.2**
- [ ] HNSW index
- [ ] Range filters on metadata
- [ ] Python bindings via PyO3

**v0.3+**
- [ ] Node.js bindings via napi-rs
- [ ] int8 / binary quantization
- [ ] Namespace support

## Contributing

Contributions are welcome. Please open an issue before starting significant work so we can align on direction.

```sh
git clone https://github.com/Hacker-007/embedb
cd embedb
cargo build
cargo test
```

## License

Licensed under the [MIT License](LICENSE).
