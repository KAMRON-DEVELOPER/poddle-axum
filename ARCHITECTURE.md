# Architecture

In Alloy/Loki pipeline stages, each log entry has:

- **The log line** (the actual text payload that will be stored)
- **A timestamp** (used for ordering/query range)
- **Labels** (the index/stream identity in Loki; high cardinality here hurts)
- **Extracted map** (temporary key/value map used between stages; not stored unless you promote values)
- **Structured metadata** (stored with the entry, not indexed like labels; searchable in newer Loki, but not used to split streams)
