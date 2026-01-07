# Poddle

## Cargo Chef

`cargo chef` is a tool that optimizes build times by separating the compilation of a Rust project's dependencies from the application's source code, leveraging Docker's layer caching mechanism.

It achieves this through two main commands, typically used in separate multi-stage Dockerfile steps:

* `cargo chef prepare`
  * **Builds/Produces**: A JSON file (commonly named recipe.json) that contains a minimal "skeleton" of the project, including all Cargo.toml and Cargo.lock files, and the information required to build the dependencies.
  * **Purpose**: This recipe is a lightweight representation of the project's dependency structure. It is generated in an early Docker stage. As long as your project dependencies do not change, this file (and the subsequent build stage) remains the same, allowing Docker to use its cache for the next steps.
* `cargo chef cook`
  * **Builds/Produces**: The compiled artifacts (object files, libraries, etc.) for all the project's *dependencies* only. This command runs a `cargo build` internally on the minimal project skeleton defined in the `recipe.json`.
  * **Purpose**: This is the expensive step in a typical Rust build, which is placed in a dedicated Docker stage. Docker caches the output of this stage. When you only make changes to your application's source code (like `src/main.rs`), the recipe.json doesn't change, the `cook` stage is skipped (using the cache), and only the final application build stage is executed.

In essence, `cargo chef` allows you to treat your dependencies as a separate, cacheable Docker layer, which can drastically speed up subsequent builds when only local source code has changed.

## Docker Layer Caching Deep Dive

### Understanding Docker Cache Invalidation

Docker creates a unique cache key for each layer based on:

* The hash of all files in the layer's filesystem
* The command being executed

This means that any change to the input files invalidates the cache for that layer and all subsequent layers.

### What Each Stage Really Does

#### Stage 2: Planner

```dockerfile
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/Cargo.toml
COPY services services
RUN cargo chef prepare --recipe-path recipe.json
```

**Produces**: `recipe.json` containing:

* All `Cargo.toml` files (paths + contents)
* Workspace structure
* Dependency graph
* **NO source code** (`.rs` files)
* **NO application logic**

Think of it as answering: "Given this workspace layout, what dependencies would be needed?"

**Cache invalidation**: This stage only changes when your manifest files (`Cargo.toml`, `Cargo.lock`) change.

#### Stage 3: Cacher

```dockerfile
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
```

**Produces**:

* `target/release/deps/*.rlib` (compiled dependency libraries)
* `target/release/build/` (build script outputs)
* Incremental build artifacts
* Cargo registry cache in `/usr/local/cargo/registry/`
* Git dependencies in `/usr/local/cargo/git/`
* Index metadata

**What gets compiled**:

* All external dependencies (tokio, axum, serde, etc.)
* All build scripts
* All proc macros
* All shared workspace libraries (like your `shared` crate)

**What does NOT get compiled**:

* Any service binary (`main.rs` in services)
* Final application artifacts
* Your actual application code

**Characteristics**:

* Expensive (can take several minutes)
* Highly cacheable
* Only invalidated when `recipe.json` changes (i.e., when dependencies change)

### Why We Can't Merge Planner and Cacher

You might think: "Why not do this in one stage?"

```dockerfile
# ❌ DON'T DO THIS
COPY . .
RUN cargo chef prepare && cargo chef cook
```

**The problem**: Any change in your repository (`.rs` files, README.md, `.env`, etc.) would invalidate this layer, forcing a complete dependency rebuild.

**With separation**:

* Planner layer only invalidates on `Cargo.toml` changes
* Cacher layer remains cached across all source code edits
* You only rebuild your application code, not all dependencies

This separation is the **entire reason cargo-chef exists**.

### Why We Copy Artifacts from Cacher to Builder

In Stage 4 (Builder), we do:

```dockerfile
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
```

**Why this is critical**:

When you run `cargo build --release --bin ${SERVICE_NAME}` in the builder stage, Cargo needs:

1. The compiled dependencies (to link against)
2. The dependency metadata (to know what's already built)
3. The crate registry (to avoid re-downloading)

**Without these copies**:

* Cargo would re-download all crates from crates.io
* Cargo would rebuild all dependencies from scratch
* The entire cargo-chef optimization would be lost
* Build time would be just as slow as without cargo-chef

**With these copies**:

* Cargo sees: "All dependencies already compiled ✓"
* Cargo only compiles: Your service binary
* Build completes in seconds instead of minutes

This is **cache materialization**, not duplication. We're telling Cargo: "Use these pre-built artifacts."

### Understanding the Complete Build Flow

```bash
Planner (5s)
   ↓ produces recipe.json
Cacher (5-10min) ← cached until dependencies change
   ↓ produces target/ + cargo registry
Builder (10-30s)  ← runs on every code change
   ↓ produces final binary
Runtime (1s)
   ↓ minimal distroless image
```

**On first build**: All stages run
**On code change**: Only Builder runs (using cached dependencies)
**On dependency change**: Cacher + Builder run

## Best Practices

### Add a .dockerignore File

When using `COPY . .` in the builder stage, Docker copies everything not excluded by `.dockerignore`. You should add this file to avoid:

* Bloating Docker layers with unnecessary files
* Breaking cache determinism
* Including sensitive files in images

**Recommended `.dockerignore`**:

```bash
.git
.gitignore
.env
**/.env
.env.*
target
node_modules
archive
infrastructure
*.md
!README.md
.vscode
.idea
*.log
```

**Why this matters**:

* `.env` should never be baked into images (security risk)
* `.git` unnecessarily bloats image layers
* `target` can break cache determinism and conflicts with cargo-chef
* Development files don't belong in production images

### Result

After adding `.dockerignore`, the `COPY . .` command becomes both safe and correct, copying only the necessary source files for compilation.

## Build Commands

To build a specific service:

```bash
docker build --build-arg SERVICE_NAME=users-api -t poddle-users-api .
docker build --build-arg SERVICE_NAME=compute-api -t poddle-compute-api .
docker build --build-arg SERVICE_NAME=billing-api -t poddle-billing-api .
```

## Performance Metrics

With cargo-chef optimization:

* **First build**: 5-10 minutes (all dependencies)
* **Code-only changes**: 10-30 seconds (just your service)
* **Dependency changes**: 5-10 minutes (rebuild deps + service)

Without cargo-chef:

* **Every build**: 5-10 minutes (full rebuild)
