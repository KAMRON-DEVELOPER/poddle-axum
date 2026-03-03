FROM alpine:latest AS fetch

ARG RAILPACK_VERSION=v0.17.2

RUN apk add --no-cache curl ca-certificates tar;

RUN set -eux; \
    apk add --no-cache curl ca-certificates tar; \
    curl -fL -o /tmp/railpack.tar.gz \
    "https://github.com/railwayapp/railpack/releases/download/${RAILPACK_VERSION}/railpack-${RAILPACK_VERSION}-x86_64-unknown-linux-musl.tar.gz"; \
    tar -xzf /tmp/railpack.tar.gz -C /tmp; \
    chmod +x "$(find /tmp -maxdepth 3 -type f -name railpack | head -n 1)"; \
    cp "$(find /tmp -maxdepth 3 -type f -name railpack | head -n 1)" /railpack

# Railpack tries to initialize mise in /tmp
# If the mise binary downloaded by Railpack is dynamically linked (which most are), the
# Linux kernel will throw a "no such file or directory" error because it cannot find the
# dynamic linker/loader required to run the binary.
FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=fetch /railpack /usr/local/bin/railpack
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/railpack"]