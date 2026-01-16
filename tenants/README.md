# Setup GitHub Container Registry (GHCR)

> You can follow the official instruction at [working-with-a-github-packages-registry](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)

## Prerequisites

- A GitHub account.
- Docker installed locally.
- A container image you want to push or a Dockerfile to build one.

### Step 1: Create a Personal Access Token (PAT)

> GHCR requires a personal access token (classic) with specific scopes for authentication.

1. Go to your GitHub `Settings`, then `Developer settings` > [`Personal access tokens`](https://github.com/settings/tokens).
2. Click Generate new token (classic).
3. Give the token a descriptive name and set an expiration.
4. Select the necessary scopes:
    1. `read:packages` to download images.
    2. `write:packages` to upload and download images.
    3. `delete:packages` to delete images (optional).
    4. `repo` if the repository associated with the package is private.
5. Click `Generate token` and `copy the token immediately`. You will not be able to see it again.

### Step 2: Authenticate to the Container Registry

Use your PAT to log in to the GHCR via the Docker CLI. It is recommended to store your token as an environment variable to avoid exposing it in your shell history.

#### 1. Set the PAT as an environment variable in your terminal (example for Linux/macOS)

```bash
export CR_PAT=YOUR_TOKEN
```

For Windows, you can add it via the control panel or command line.

#### 2. Log in to the GHCR using your GitHub username and the PAT

```bash
echo $CR_PAT | docker login ghcr.io -u YOUR_GITHUB_USERNAME --password-stdin
```

You should see a `Login Succeeded` message.

### Step 3: Build and Tag your Docker Image

Navigate to your project's directory containing the Dockerfile.

1. Build your Docker image:

```bash
docker build -t IMAGE_NAME .
```

1. Tag the image with the GHCR destination, using the format `ghcr.io/OWNER/REPOSITORY_NAME:TAG`:

```bash
docker tag IMAGE_NAME ghcr.io/YOUR_GITHUB_USERNAME/YOUR_REPO_NAME:latest
```

(Replace `IMAGE_NAME`, `YOUR_GITHUB_USERNAME`, and `YOUR_REPO_NAME` with your details).

### Step 4: Push the Image to GHCR

Push the tagged image to the GitHub Container Registry:
bash

```bash
docker push ghcr.io/YOUR_GITHUB_USERNAME/YOUR_REPO_NAME:latest
```

Once pushed, you can view the image in the `Packages` section of your GitHub profile or repository.

Like:
<https://github.com/settings/packages>
<https://github.com/KAMRON-DEVELOPER?tab=packages>
