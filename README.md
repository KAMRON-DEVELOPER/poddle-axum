
# Poddle

## Cargo Chef

`cargo chef` is a tool that optimizes build times by separating the compilation of a Rust project's dependencies from the application's source code, leveraging Docker's layer caching mechanism

It achieves this through two main commands, typically used in separate multi-stage Dockerfile steps:

    * `cargo chef prepare`
        * **Builds/Produces**: A JSON file (commonly named recipe.json) that contains a minimal "skeleton" of the project, including all Cargo.toml and Cargo.lock files, and the information required to build the dependencies.
        * **Purpose**: This recipe is a lightweight representation of the project's dependency structure. It is generated in an early Docker stage. As long as your project dependencies do not change, this file (and the subsequent build stage) remains the same, allowing Docker to use its cache for the next steps
    * `cargo chef cook`
        * **Builds/Produces**: The compiled artifacts (object files, libraries, etc.) for all the project's *dependencies* only. This command runs a `cargo build` internally on the minimal project skeleton defined in the `recipe.json`.
    * **Purpose**: This is the expensive step in a typical Rust build, which is placed in a dedicated Docker stage. Docker caches the output of this stage. When you only make changes to your application's source code (like `src/main.rs`), the recipe.json doesn't change, the `cook` stage is skipped (using the cache), and only the final application build stage is executed.

In essence, `cargo chef` allows you to treat your dependencies as a separate, cacheable Docker layer, which can drastically speed up subsequent builds when only local source code has changed.
