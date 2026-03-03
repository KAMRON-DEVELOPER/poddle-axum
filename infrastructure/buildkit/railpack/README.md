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
