# K3S setup

Create `IP addresses` from `VPC Network > IP addresses`
Create  `Instance groups` from `Compute Engine > Instance groups` and add three MVs
Create  `Instance groups` from `Compute Engine > Instance groups`

Use EXTERNAL IP (and your domain) for --tls-san. This allows your local laptop's kubectl to securely talk to the cluster's API server.

```bash
# Replace 10.x.x.x with your GCP VM Internal IP
# Replace 34.x.x.x with your GCP Static External IP
curl -sfL https://get.k3s.io | sh -s - server \
  --write-kubeconfig-mode=644 \
  --disable traefik \
  --disable servicelb \
  --flannel-backend=none \
  --disable-network-policy \
  --disable-kube-proxy \
  --cluster-cidr=10.42.0.0/16 \
  --service-cidr=10.43.0.0/16 \
  --bind-address=10.x.x.x \
  --advertise-address=10.x.x.x \
  --node-ip=10.x.x.x \
  --tls-san=34.x.x.x \
  --tls-san=poddle.uz \
  --node-external-ip=34.x.x.x
```

```bash
curl -sfL https://get.k3s.io | K3S_URL="https://${MASTER_IP}:6443" \
  K3S_TOKEN="${NODE_TOKEN}" sh -s - \
  --node-ip=10.x.x.x \
  --node-external-ip=34.x.x.x
```

if you can ssh into VM you can copy kube config or by cat.

```bash
#        / internal /  exteral   /
sed -i 's/10.212.0.6/34.18.96.229/g' ~/.kube/config
```

GCP blocks port 6443 by default

```bash
gcloud auth login
```

Set project id if not selected by default

```bash
gcloud projects list
```

```bash
# cloud config set project PROJECT_ID
gcloud config set project poddle-mvp
```

We use Cloudflare beacuse we need DNS-01 challange for wildcard domains.

```bash
kubectl create secret generic cloudflare-api-token-secret \
  --namespace cert-manager \
  --from-literal=api-token=YOUR_CLOUDFLARE_TOKEN_HERE
```

Loki need access to GCS

```bash
kubectl create namespace loki
kubectl create namespace tempo
kubectl -n loki create secret generic poddle-gcs-sa-key \
  --from-file=service-account.json=certs/poddle-gcs-sa-key.json
kubectl -n tempo create secret generic poddle-gcs-sa-key \
  --from-file=service-account.json=certs/poddle-gcs-sa-key.json
```

## Installation Order

- Cilium
- Cert-Manager
- Traefik
- Vault

### Cert-Canager

```bash
kubectl apply -f infrastructure/charts/cert-manager/clusterissuer-prod.yaml
```

Check

```bash
kubectl get clusterissuers
# NAME                              READY   AGE
# letsencrypt-production-dns01-ci   True    107s
# vault-root-ca-ci                  True    20m
```

```bash
kubectl create namespace traefik
kubectl apply -f infrastructure/charts/cert-manager/wildcard-certificate-prod.yaml
```

Check (You need to wait, this one take quiet a bit long)

```bash
kubectl get certificates -n traefik
# NAME                   READY   SECRET                AGE
# wildcard-certificate   True    wildcard-tls-secret   3m27s
```

### Traefik

```bash
kubectl apply -f
```

### Vault (HA)

```bash
kubectl apply -f infrastructure/charts/cert-manager/vault/ca/selfsigned-issuer.yaml
kubectl apply -f infrastructure/charts/cert-manager/vault/ca/vault-root-ca-certificate.yaml
kubectl apply -f infrastructure/charts/cert-manager/vault/ca/vault-root-ca-ci.yaml
kubectl apply -f infrastructure/charts/cert-manager/vault/ca/vault-server-tls-certificate.yaml
```

```bash
kubectl exec -n vault vault-0 -- vault operator init -key-shares=5 -key-threshold=3 \
  -format=json > ~/certs/vault-keys-prod.json
```

```bash
cat > ~/.zsh_secrets_prod <<EOF
# Vault unseal keys
export UNSEAL_KEY1_PROD="$(jq -r '.unseal_keys_b64[0]' ~/certs/vault-keys-prod.json)"
export UNSEAL_KEY2_PROD="$(jq -r '.unseal_keys_b64[1]' ~/certs/vault-keys-prod.json)"
export UNSEAL_KEY3_PROD="$(jq -r '.unseal_keys_b64[2]' ~/certs/vault-keys-prod.json)"
export UNSEAL_KEY4_PROD="$(jq -r '.unseal_keys_b64[3]' ~/certs/vault-keys-prod.json)"
export UNSEAL_KEY5_PROD="$(jq -r '.unseal_keys_b64[4]' ~/certs/vault-keys-prod.json)"

# Vault root token
export VAULT_TOKEN_PROD="$(jq -r '.root_token' ~/certs/vault-keys-prod.json)"
EOF
```

```bash
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY1_PROD
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY2_PROD
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY3_PROD

kubectl exec -n vault vault-1 -- vault operator unseal $UNSEAL_KEY1_PROD
kubectl exec -n vault vault-1 -- vault operator unseal $UNSEAL_KEY2_PROD
kubectl exec -n vault vault-1 -- vault operator unseal $UNSEAL_KEY3_PROD

kubectl exec -n vault vault-2 -- vault operator unseal $UNSEAL_KEY1_PROD
kubectl exec -n vault vault-2 -- vault operator unseal $UNSEAL_KEY2_PROD
kubectl exec -n vault vault-2 -- vault operator unseal $UNSEAL_KEY3_PROD
```

```bash
kubectl exec -n vault vault-0 -- vault login $VAULT_TOKEN_PROD
```

## Application setup

Create `config.json` for services

```bash
kubectl create namespace poddle-system
kubectl create configmap service-config \
  -n poddle-system \
  --from-file=config.json=infrastructure/deploy/config.json

# or

kubectl create namespace poddle-system --dry-run=client -o yaml | kubectl apply -f -
kubectl create configmap service-config \
  -n poddle-system \
  --from-file=config.json=infrastructure/deploy/config.json \
  --dry-run=client -o yaml | kubectl apply -f -
```

Compute provisioner need access to cert manager for preflight check, we create RBACK

```bash
kubectl apply -f infrastructure/deploy/compute-provisioner-rbac.yaml
```

Deployments

```bash
kubectl apply -f infrastructure/deploy/deployments
```

Services

```bash
kubectl apply -f infrastructure/deploy/poddle-services.yaml
```

IngressRoutes

```bash
kubectl apply -f infrastructure/deploy/poddle-ingressroutes.yaml
```

## GCP Setup

Phase 1: Configure the Instance Group (Named Ports)

A TCP Proxy Load Balancer needs to know which "Named Port" to look for on your Instance Group.

  Go to Compute Engine -> Instance Groups.

  Click on your Unmanaged Instance Group.

  Click Edit.

  Look for the Port mapping or Named ports section.

  Add the following two items:

  Name: traefik-http | Port: 30000

  Name: traefik-https | Port: 30443

  Save the changes.

Phase 2: Create Firewall Rules

The Load Balancer and Health Checkers need permission to talk to your VMs.

  Go to VPC network -> Firewall.

  Click Create Firewall Rule.

  Name: allow-lb-health-check

  Targets: All instances in the network (or use Service Account/Tags if you configured them for your VMs).

  Source IPv4 ranges: Add these two specific ranges (these are Google's Load Balancer ranges):

  130.211.0.0/22

  35.191.0.0/16

  Protocols and ports:

  Check TCP and enter: 30000, 30443

  Click Create.
