
# Poddle Observability

In any real-world application, logging is crucial for diagnosing problems and understanding application behavior. Rust offers several powerful crates for logging (log) and structured tracing (tracing). In this post, we’ll build a simple Axum-based REST API and show how to use tracing to automatically include context—like HTTP method and path—in our logs.

We’ll also delve into the variety of log levels, how to set different log levels per crate, and why structured logs are so beneficial for observability.

## What Are Logging and Tracing?

    * **Logging** is the traditional practice of printing messages (like “User 123 not found” or “Connection error”) at various places in your code. In Rust, log is the standard facade for logging, and you typically emit messages via macros like log::info! or log::error!.
    * **Tracing** goes one step further by allowing you to create “spans” of time that carry contextual data. For instance, if you have an HTTP request, you can capture details such as the method (POST), path (/messages), and a unique request ID in a span. Any logging done within that span automatically attaches these details, making it easy to see which request triggered which log messages.
