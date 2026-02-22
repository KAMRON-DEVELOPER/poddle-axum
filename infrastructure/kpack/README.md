# Kpack

<https://github.com/buildpacks-community/kpack/blob/main/docs/tutorial.md>

```bash
kubectl create namespace kpack-build
kubectl create namespace buildkit
```

1. Create a secret with push credentials

```bash
kubectl create secret docker-registry registry-secret \
  --docker-username=my-dockerhub-username \
  --docker-password=my-dockerhub-password \
  --docker-server=https://index.docker.io/v1/ \
  --namespace kpack

# jq -c makes the JSON a single line
kubectl create secret docker-registry registry-secret \
  --namespace=kpack \
  --docker-server="https://me-central1-docker.pkg.dev" \
  --docker-username="_json_key" \
  --docker-password="$(jq -c . < certs/poddle-artifact-registery-key.json)"
```

1. Create a service account

```bash
kubectl apply -f infrastructure/kpack/sa.yaml
```

1. Create a cluster store configuration

A store resource is a repository of buildpacks packaged in [buildpackages](https://buildpacks.io/docs/buildpack-author-guide/package-a-buildpack/) that can be used by kpack to build OCI images. Later you will reference this store in a Builder configuration.

```bash
kubectl apply -f infrastructure/kpack/cluster-store.yaml
```

1. Create a cluster stack configuration

A stack resource is the specification for a [cloud native buildpacks stack](https://buildpacks.io/docs/concepts/components/stack/) used during build and in the resulting app image.

```bash
kubectl apply -f infrastructure/kpack/cluster-store.yaml
```

1. Apply a lifecycle resource

A lifecycle orchestrates buildpacks, then assembles the resulting artifacts into an OCI image.

```bash
kubectl apply -f infrastructure/kpack/cluster-lifecycle.yaml
```

1. Create a Builder configuration

A Builder is the kpack configuration for a [builder image](https://buildpacks.io/docs/concepts/components/builder/) that includes the stack and buildpacks needed to build an OCI image from your app source code.

The Builder configuration will write to the registry with the secret configured in step one and will reference the stack and store created in step three and four. The builder order will determine the order in which buildpacks are used in the builder.

```bash
kubectl apply -f infrastructure/kpack/builder.yaml
```

Replace `DOCKER-IMAGE-TAG` with a valid image tag that exists in the registry you configured with the `--docker-server` flag when creating a Secret in step #1. The tag should be something like: `your-name/builder` or `gcr.io/your-project/builder`

1. Apply a kpack image resource

An image resource is the specification for an OCI image that kpack should build and manage.

```bash
kubectl apply -f infrastructure/kpack/image.yaml
```
