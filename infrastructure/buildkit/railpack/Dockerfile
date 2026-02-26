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

FROM gcr.io/distroless/static:nonroot
COPY --from=fetch /railpack /usr/local/bin/railpack
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/railpack"]