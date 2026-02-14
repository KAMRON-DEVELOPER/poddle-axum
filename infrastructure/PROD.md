# K3S setup

Create `IP addresses` from `VPC Network > IP addresses`
Create  `Instance groups` from `Compute Engine > Instance groups` and add three MVs
Create  `Instance groups` from `Compute Engine > Instance groups`

Use EXTERNAL IP (and your domain) for --tls-san. This allows your local laptop's kubectl to securely talk to the cluster's API server.

```bash
# Replace 10.x.x.x with your GCP VM Internal IP
# Replace 34.x.x.x with your GCP Static External IP
curl -sfL <https://get.k3s.io> | sh -s - server \
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
  K3S_TOKEN="${NODE_TOKEN}" sh -s - --node-ip=10.x.x.x --node-external-ip=34.x.x.x
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

Open 6643

```bash
gcloud compute firewall-rules create allow-k3s-api \
    --allow tcp:6443 \
    --source-ranges 0.0.0.0/0 \
    --description "Allow kubectl to connect to k3s API"
# Creating firewall...⠹Created [https://www.googleapis.com/compute/v1/projects/poddle-mvp/global/firewalls/allow-k3s-api].
# Creating firewall...done.
# NAME           NETWORK  DIRECTION  PRIORITY  ALLOW     DENY  DISABLED
# allow-k3s-api  default  INGRESS    1000      tcp:6443        False
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
kubectl apply -f infrastructure/charts/cert-manager/wildcard-certificate-prod.yaml
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
touch .zsh_secrets_prod
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

## Create `config.json` for services

```bash
kubectl -n poddle-system create secret generic service-config \
  --from-file=config.json=infrastructure/deploy/config.json
```
