# Railpack

```bash
~/Documents/Coding/rust/backend/poddle-axum master ❯ cat certs/poddle-artifact-registery-key.json | docker --config /tmp/gcp-docker login -u _json_key --password-stdin https://me-central1-docker.pkg.dev && docker --config /tmp/gcp-docker pull me-central1-docker.pkg.dev/poddle-mvp/buildkit/debug-railpack:latest
Login Succeeded
latest: Pulling from poddle-mvp/buildkit/debug-railpack
1adabd6b0d6b: Pull complete
9b7bf8e626ed: Pull complete
e9efee35afce: Pull complete
b6ab26ac98c1: Pull complete
f497b7d97281: Pull complete
de24d7c29da9: Pull complete
ac0b5e4f74f1: Pull complete
312a967b27d4: Pull complete
056aa6e18654: Pull complete
Digest: sha256:7a4ed435f41b7fb0ce0bf6cb61d11cc968b90283607ee9bde0803c97988bd707
Status: Downloaded newer image for me-central1-docker.pkg.dev/poddle-mvp/buildkit/debug-railpack:latest
me-central1-docker.pkg.dev/poddle-mvp/buildkit/debug-railpack:latest
~/Documents/Coding/rust/backend/poddle-axum master ❯ docker run --rm --name debug-railpack -p 8000:8000 me-central1-docker.pkg.dev/poddle-mvp/buildkit/debug-railpack:latest
INFO:     Started server process [1]
INFO:     Waiting for application startup.
INFO:     Application startup complete.
INFO:     Uvicorn running on http://0.0.0.0:8000 (Press CTRL+C to quit)
^CINFO:     Shutting down
INFO:     Waiting for application shutdown.
INFO:     Application shutdown complete.
INFO:     Finished server process [1]
~/Documents/Coding/rust/backend/poddle-axum master ❯
```
