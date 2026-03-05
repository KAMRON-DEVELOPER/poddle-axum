# Railpack

## Local build

Docker build

```bash
docker build -t kamronbekdev/pack-fastapi-bookshop:0.0.1 .
docker build -t kamronbekdev/pack-express-notes:0.0.1 .
docker build -t kamronbekdev/pack-react-bookshop:0.0.1 .
docker build -t kamronbekdev/pack-react-notes:0.0.1 .
```

Docker push

```bash
docker push kamronbekdev/pack-fastapi-bookshop:0.0.1
docker push kamronbekdev/pack-express-notes:0.0.1
docker push kamronbekdev/pack-react-bookshop:0.0.1
docker push kamronbekdev/pack-react-notes:0.0.1
```

## Railpack build

```bash
kubectl apply -f infrastructure/buildkit/railpack/fastapi-bookshop-job.yaml
kubectl apply -f infrastructure/buildkit/railpack/express-notes-job.yaml
kubectl apply -f infrastructure/buildkit/railpack/react-bookshop-job.yaml
kubectl apply -f infrastructure/buildkit/railpack/react-notes-job.yaml
```

## GCP

GCP login

```bash
cat certs/poddle-artifact-registery-key.json | docker --config /tmp/gcp-docker login -u _json_key --password-stdin https://me-central1-docker.pkg.dev
```

Image pull

```bash
docker --config /tmp/gcp-docker pull me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-fastapi-bookshop:latest
docker --config /tmp/gcp-docker pull me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-express-notes:latest
docker --config /tmp/gcp-docker pull me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-react-bookshop:latest
docker --config /tmp/gcp-docker pull me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-react-notes:latest
```

Run the container

```bash
docker run --rm --name railpack-fastapi-bookshop -p 8000:8000 me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-fastapi-bookshop:latest
docker run --rm --name railpack-express-notes -p 8000:8000 me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-express-notes:latest
docker run --rm --name railpack-react-bookshop -p 8000:8000 me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-react-bookshop:latest
docker run --rm --name railpack-react-notes -p 8000:8000 me-central1-docker.pkg.dev/poddle-mvp/buildkit/railpack-react-notes:latest
```

## The Issue with `railpack-plan.json`

By default, the Railpack CLI hardcodes `:latest` tags for its core components (`ghcr.io/railwayapp/railpack-builder:latest` and `railpack-runtime:latest`) into the generated `railpack-plan.json`.

In a PaaS environment, this creates two major issues:

1. **Cache Invalidation:** Because the tag is `:latest`, BuildKit is forced to ping the GitHub Container Registry (GHCR) over the network on *every single build* to resolve the manifest digest.
2. **Network Hangs:** Due to aggressive unauthenticated rate limits on GHCR and international network routing latency, these manifest resolutions can hang silently, causing tenant builds to stall for 15+ minutes.

**The Solution:** We mirror these critical base images to our own controlled registry (Docker Hub) and pin them to specific versions (e.g., `v0.17.2`). An init container dynamically mutates the `railpack-plan.json` using `sed` to inject our mirrored image URIs before BuildKit executes the plan.

---

## Maintenance Guide: Updating the Mirrored Images

When upgrading Railpack, you must mirror the new frontend, builder, and runtime images to our registry.

### Fetching Image Digests (Optional but Recommended)

If you need to verify the exact SHA256 hashes of the upstream images for strict caching, you can inspect them without pulling the full image:

```bash
# Using skopeo (sudo pacman -S skopeo)
skopeo inspect docker://ghcr.io/railwayapp/railpack-runtime:latest | jq -r .Digest
skopeo inspect docker://ghcr.io/railwayapp/railpack-builder:latest | jq -r .Digest

# Using crane (sudo pacman -S crane)
crane digest ghcr.io/railwayapp/railpack-runtime:latest
crane digest ghcr.io/railwayapp/railpack-builder:latest
```

## Login for [`skopeo`](https://github.com/containers/skopeo)

Ensure you are authenticated to push to your mirrored registry:

```bash
skopeo login docker.io -u kamronbekdev
# It will prompt for your password (use your Docker Hub Personal Access Token)
```

### Mirror the Images

Use `skopeo copy --all` to preserve multi-architecture manifests while moving the images from GHCR to Docker Hub.

```bash
# Mirror Frontend
skopeo copy \
  --all \
  docker://ghcr.io/railwayapp/railpack-frontend:v0.17.2 \
  docker://docker.io/kamronbekdev/railpack-builder:v0.17.2

# Mirror Builder
skopeo copy \
  --all \
  docker://ghcr.io/railwayapp/railpack-builder:latest \
  docker://docker.io/kamronbekdev/railpack-builder:v0.17.2

# Mirror Runtime
skopeo copy --all \
  docker://ghcr.io/railwayapp/railpack-runtime:latest \
  docker://docker.io/kamronbekdev/railpack-runtime:v0.17.2
```

*(Note: Ensure you update the Rust configuration variables in spawn_railpack_job to match the newly mirrored version tags).*
