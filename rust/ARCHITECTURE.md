# Architecture

In Alloy/Loki pipeline stages, each log entry has:

- **The log line** (the actual text payload that will be stored)
- **A timestamp** (used for ordering/query range)
- **Labels** (the index/stream identity in Loki; high cardinality here hurts)
- **Extracted map** (temporary key/value map used between stages; not stored unless you promote values)
- **Structured metadata** (stored with the entry, not indexed like labels; searchable in newer Loki, but not used to split streams)

buildkitd = the daemon (server). This does the real building.

buildctl = the client that sends “build instructions” to the daemon.

buildctl (client) and talks to buildkitd (daemon).

BuildKit is a Server. It speaks via GRPC over TCP
buildctl (the client) talks to buildkitd (the server) over gRPC, so we use TCP protocol "tcp://"

Job vs. Deployment
