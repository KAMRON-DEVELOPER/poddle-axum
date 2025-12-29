# Infrastructure Setup

---

## Chart setup

```bash
helm repo add cilium https://helm.cilium.io/
helm repo add metallb https://metallb.github.io/metallb
helm repo add traefik https://traefik.github.io/charts
helm repo add hashicorp https://helm.releases.hashicorp.com
helm repo add jetstack https://charts.jetstack.io
helm repo add grafana https://grafana.github.io/helm-charts
helm repo add minio https://helm.min.io/
```

Update local charts respository

```bash
helm repo update
```

### We dump default values into `infrastructure/charts`

```bash
mkdir -p infrastructure/charts/cilium
mkdir -p infrastructure/charts/metallb
mkdir -p infrastructure/charts/traefik
mkdir -p infrastructure/charts/vault
mkdir -p infrastructure/charts/vault-secrets-operator
mkdir -p infrastructure/charts/cert-manager
mkdir -p infrastructure/charts/grafana
mkdir -p infrastructure/charts/alloy
mkdir -p infrastructure/charts/tempo
mkdir -p infrastructure/charts/loki
mkdir -p infrastructure/charts/mimir
mkdir -p infrastructure/charts/minio

helm show values cilium/cilium > infrastructure/charts/cilium/values.yaml
helm show values metallb/metallb > infrastructure/charts/metallb/values.yaml
helm show values traefik/traefik > infrastructure/charts/traefik/values.yaml
helm show values hashicorp/vault > infrastructure/charts/vault/values.yaml
helm show values hashicorp/vault-secrets-operator > infrastructure/charts/vault-secrets-operator/values.yaml
helm show values jetstack/cert-manager > infrastructure/charts/cert-manager/values.yaml
helm show values grafana/grafana > infrastructure/charts/grafana/values.yaml
helm show values grafana/alloy > infrastructure/charts/alloy/values.yaml
helm show values grafana/tempo-distributed > infrastructure/charts/tempo/values.yaml
helm show values grafana/loki-distributed > infrastructure/charts/loki/values.yaml
helm show values grafana/mimir-distributed > infrastructure/charts/mimir/values.yaml
helm show values minio/minio > infrastructure/charts/minio/values.yaml
```

### Chart pull commands

```bash
helm pull cilium/cilium --untar --untardir infrastructure/charts/cilium
helm pull metallb/metallb --untar --untardir infrastructure/charts/metallb
helm pull traefik/traefik --untar --untardir infrastructure/charts/traefik
helm pull hashicorp/vault --untar --untardir infrastructure/charts/vault
helm pull hashicorp/vault-secrets-operator --untar --untardir infrastructure/charts/vault-secrets-operator
helm pull jetstack/cert-manager --untar --untardir infrastructure/charts/cert-manager
helm pull grafana/grafana --untar --untardir infrastructure/charts/grafana
helm pull grafana/alloy --untar --untardir infrastructure/charts/cert-manager
helm pull grafana/tempo-distributed --untar --untardir infrastructure/charts/tempo
helm pull grafana/loki-distributed --untar --untardir infrastructure/charts/loki
helm pull grafana/mimir-distributed --untar --untardir infrastructure/charts/mimir
helm pull minio/minio --untar --untardir infrastructure/charts/minio

helm pull prometheus-community/kube-prometheus-stack --untar --untardir infrastructure/charts/prometheus-community-manifests
helm pull open-telemetry/opentelemetry-collector --untar --untardir infrastructure/charts/open-telemetry-manifests
```

---

## Installing charts

### 1. Install CNI (Cilium)

```bash
helm install cilium cilium/cilium \
  --namespace kube-system \
  --set k8sServiceHost=192.168.31.4 \
  --set k8sServicePort=6443 \
  --set ipam.mode=kubernetes \
  --set kubeProxyReplacement=true

# or

helm upgrade --install cilium cilium/cilium \
  --namespace kube-system \
  --set k8sServiceHost=192.168.31.4 \
  --set k8sServicePort=6443 \
  --set ipam.mode=kubernetes \
  --set kubeProxyReplacement=true \
  --values infrastructure/charts/cilium/cilium-values.yaml
```

Wait for Cilium to be ready:

> Don't forget to install cilium-cli. On arch ```sudo pacman -S cilium-cli```

```bash
kubectl -n kube-system rollout status deployment/cilium-operator
cilium-cli status --wait
```

Verify nodes are now Ready:

```bash
kubectl get nodes
# All nodes should show Ready status
```

---

### 2. Install MetalLB

```bash
helm install metallb metallb/metallb \
  --namespace metallb-system --create-namespace

# or

helm upgrade --install metallb metallb/metallb \
  --values infrastructure/charts/metallb/metallb-values.yaml \
  --namespace metallb-system --create-namespace
```

Wait for MetalLB pods:

```bash
kubectl -n metallb-system rollout status deployment/metallb-controller
```

Apply IP pool configuration:

```bash
kubectl apply -f infrastructure/charts/metallb/metallb-config.yaml
```

infrastructure/charts/metallb/metallb-config.yaml

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: metallb-system
---
apiVersion: metallb.io/v1beta1
kind: IPAddressPool
metadata:
  name: ip-address-pool
  namespace: metallb-system
spec:
  addresses:
    - 192.168.31.10-192.168.31.19
---
apiVersion: metallb.io/v1beta1
kind: L2Advertisement
metadata:
  name: l2-advertisement
  namespace: metallb-system
spec:
  ipAddressPools:
    - ip-address-pool
```

---

## 3. Install Traefik

```bash
helm install traefik traefik/traefik \
  --namespace traefik --create-namespace

# or `https://doc.traefik.io/traefik/getting-started/quick-start-with-kubernetes/`

helm install traefik traefik/traefik --wait \
  --set ingressRoute.dashboard.enabled=true \
  --set ingressRoute.dashboard.matchRule='Host(`traefik.poddle.uz`)' \
  --set ingressRoute.dashboard.entryPoints={web} \
  --set providers.kubernetesGateway.enabled=true \
  --set gateway.listeners.web.namespacePolicy.from=All \
  --namespace traefik --create-namespace

# or

helm upgrade --install traefik traefik/traefik \
  --values infrastructure/charts/traefik/traefik-values.yaml \
  --namespace traefik --create-namespace
```

Verify Traefik got an external IP:

```bash
kubectl get svc -n traefik
# Should show EXTERNAL-IP: 192.168.31.10
```

---

## 4. Install cert-manager

```bash
helm install cert-manager jetstack/cert-manager \
  --set crds.enabled=true \
  --namespace cert-manager --create-namespace

# or
 
helm upgrade --install cert-manager jetstack/cert-manager \
--set crds.enabled=true \
--set "extraArgs={--enable-gateway-api}" \
--namespace cert-manager --create-namespace
```

Verify:

```bash
kubectl get pods -n cert-manager
# Expected output:
# NAME                                       READY   STATUS    RESTARTS   AGE
# cert-manager-7ff7f97d55-m2hmn              1/1     Running   0          6m29s
# cert-manager-cainjector-59bb669f8d-76btl   1/1     Running   0          6m29s
# cert-manager-webhook-59bbd786df-bvqxj      1/1     Running   0          6m29s
```

---

## 5. Install Vault (HA) with internal TLS via cert-manager

[!NOTE]
If you plan to use the Vault CLI locally, install it and enable shell completion:

```bash
sudo pacman -S vault
vault -autocomplete-install
```

### 5.1 Create Vault namespace

```bash
kubectl create namespace vault
```

### 5.2 Bootstrap Vault internal PKI (cert-manager)

This directory implements a self-contained internal PKI used only to bootstrap Vault TLS.

```bash
infrastructure/charts/cert-manager/vault/
├── ca/
│   ├── selfsigned-issuer.yaml
│   ├── vault-root-ca-certificate.yaml
│   └── vault-root-ca-issuer.yaml
└── certs/
    └── vault-server-tls-certificate.yaml
```

Apply manifests in order:

```bash
kubectl apply -f infrastructure/charts/cert-manager/vault/ca/selfsigned-issuer.yaml
kubectl apply -f infrastructure/charts/cert-manager/vault/ca/vault-root-ca-certificate.yaml
kubectl apply -f infrastructure/charts/cert-manager/vault/ca/vault-root-ca-issuer.yaml
kubectl apply -f infrastructure/charts/cert-manager/vault/certs/vault-server-tls-certificate.yaml
```

### 5.3 Verify PKI resources

```bash
kubectl get issuers -n vault
kubectl get certificates -n vault
kubectl get secrets -n vault
```

Expected important secrets:

* `vault-root-ca-secret`
* `vault-server-tls-secret`

The server TLS secret should contain:

* `tls.crt`
* `tls.key`
* `ca.crt`

### 5.4 What this PKI setup does (important)

This setup solves the chicken-and-egg problem:
> Vault needs TLS to start, but you want Vault to be your long-term PKI.

So cert-manager provides only the bootstrap PKI, after which Vault can take over.

1. Flow overview
    * Bootstrap Issuer
    * File: selfsigned-issuer.yaml
    * Creates a one-time self-signed Issuer
    * Used only to mint the root CA
2. Root CA Certificate
    * File: vault-root-ca-certificate.yaml
    * Generates a self-signed root CA
    * Stored in Secret: vault-root-ca-secret
3. CA-backed Issuer
    * File: vault-root-ca-issuer.yaml
    * Uses vault-root-ca-secret
    * Becomes the real signing authority
4. Vault Server TLS Certificate
    * File: vault-server-tls-certificate.yaml
    * Issues TLS certs for:
      * vault.vault.svc
      * vault-active
      * vault-standby
      * StatefulSet pod DNS
      * Internal wildcards
    * Stored in Secret: vault-server-tls-secret

### 5.6 Export Vault CA for clients and ClusterIssuers

* This CA is required by:
  * Axum microservices
  * Vault CLI
  * cert-manager ClusterIssuer (vault-k8s-ci)

Export CA certificate

```bash
mkdir -p ~/certs
kubectl get secret vault-root-ca-secret -n vault \
  -o jsonpath='{.data.ca\.crt}' | base64 -d > ~/certs/vault-root-ca.crt
```

For cert-manager ClusterIssuer

```bash
kubectl get secret vault-root-ca-secret -n vault \
  -o jsonpath='{.data.ca\.crt}' | base64 -d
```

Use this value as caBundle in your vault-k8s-ci ClusterIssuer.

### 5.7 Configure Vault CLI trust

Add to your shell secrets:

```bash
cat >> ~/.zsh_secrets <<EOF
export VAULT_CACERT=~/certs/vault-root-ca.crt
EOF
```

Reload:

```bash
source ~/.zsh_secrets
```

### Setup GCP KMS for auto unseal (if enabled)

1. Enable `Cloud Key Management Service (KMS) API` from <https://console.cloud.google.com/marketplace/product/google/cloudkms.googleapis.com>
2. Create a Service Account with the role `Cloud KMS CryptoKey Encrypter/Decrypter` and `Cloud KMS Viewer` from <https://console.cloud.google.com/apis/credentials>
3. Create Key Ring and CryptoKey(HSM Protection level, Symmetric encrypt/decrypt) from <https://console.cloud.google.com/security/kms>
4. ownload the JSON Key for this service account(Create key for the service account)

```bash
kubectl create secret generic poddle-kms-service-account-secret \
  --from-file=poddle-kms-service-account.json=$HOME/certs/poddle-kms-service-account.json \
  -n vault
```

### How Vault HA Works with Raft on Kubernetes

**Key Concept**: In Raft-based HA mode, only the **first pod (vault-0)** is initialized. The other pods (vault-1, vault-2) are **standby replicas** that join the Raft cluster automatically but are NOT independently initialized.

#### Architecture Overview

```yaml
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   vault-0   │────▶│   vault-1   │────▶│   vault-2   │
│  (Leader)   │◀────│ (Follower)  │◀────│ (Follower)  │
│ INITIALIZED │     │ JOINS RAFT  │     │ JOINS RAFT  │
└─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │
       └───────────────────┴───────────────────┘
              Raft Consensus Protocol
```

#### How It Works

1. **Raft Consensus**: Vault uses the Raft algorithm for distributed consensus. One node is the leader (active), others are followers (standby).
2. **Storage**: Each pod has its own persistent volume (`/vault/data`), but they replicate data through Raft protocol.
3. **Leader Election**: If the leader fails, followers automatically elect a new leader (typically within seconds).
4. **Data Replication**: All write operations go through the leader, which replicates them to followers.
5. **High Availability**:
   * With 3 replicas, the cluster can tolerate 1 node failure
   * Minimum 2 nodes needed for quorum (majority)
   * Formula: quorum = (n/2) + 1, where n = total nodes

```bash
helm install vault hashicorp/vault \
  --namespace traefik --create-namespace

# or

helm upgrade --install vault hashicorp/vault \
  --values infrastructure/charts/vault/vault-values.yaml \
  --namespace vault --create-namespace
```

```bash
kubectl get pods -n vault -w
# Expected output:
# NAME                       READY   STATUS    RESTARTS   AGE
# vault-0                    0/1     Running   0          30s
# vault-1                    0/1     Running   0          30s
# vault-2                    0/1     Running   0          30s
```

All pods will be 0/1 (Not Ready) because Vault is sealed.

#### Initialize Vault (vault-0 only)

```bash
# Initialize vault-0
kubectl exec -n vault vault-0 -- vault operator init \
  -key-shares=5 \
  -key-threshold=3 \
  -format=json > ~/certs/vault-keys.json

# Extract keys (save these securely!)
cat ~/certs/vault-keys.json | jq -r '.unseal_keys_b64[]'
cat ~/certs/vault-keys.json | jq -r '.root_token'
```

Export temporarly

```bash
export UNSEAL_KEY1=$(cat ~/certs/vault-keys.json | jq -r '.unseal_keys_b64[0]')
export UNSEAL_KEY2=$(cat ~/certs/vault-keys.json | jq -r '.unseal_keys_b64[1]')
export UNSEAL_KEY3=$(cat ~/certs/vault-keys.json | jq -r '.unseal_keys_b64[2]')
export UNSEAL_KEY4=$(cat ~/certs/vault-keys.json | jq -r '.unseal_keys_b64[3]')
export UNSEAL_KEY5=$(cat ~/certs/vault-keys.json | jq -r '.unseal_keys_b64[4]')
export VAULT_ROOT_TOKEN=$(cat ~/certs/vault-keys.json | jq -r '.root_token')
```

Keep persistent in ~/.zsh_secrets, Generate the secrets file from `vault-keys.json`

```bash
cat > ~/.zsh_secrets <<EOF
# Vault unseal keys
export UNSEAL_KEY1="$(jq -r '.unseal_keys_b64[0]' ~/certs/vault-keys.json)"
export UNSEAL_KEY2="$(jq -r '.unseal_keys_b64[1]' ~/certs/vault-keys.json)"
export UNSEAL_KEY3="$(jq -r '.unseal_keys_b64[2]' ~/certs/vault-keys.json)"
export UNSEAL_KEY4="$(jq -r '.unseal_keys_b64[3]' ~/certs/vault-keys.json)"
export UNSEAL_KEY5="$(jq -r '.unseal_keys_b64[4]' ~/certs/vault-keys.json)"

# Vault root token
export VAULT_TOKEN="$(jq -r '.root_token' ~/certs/vault-keys.json)"
EOF
```

Reload

```bash
[ -f ~/.zsh_secrets ] && source ~/.zsh_secrets
```

#### Unseal vault-0

```bash
# Unseal vault-0 (need 3 keys)
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY1
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY2
kubectl exec -n vault vault-0 -- vault operator unseal $UNSEAL_KEY3

# Verify vault-0 is unsealed and ready
kubectl exec -n vault vault-0 -- vault status
```

#### Join and Unseal vault-1 and vault-2

With retry_join configured, vault-1 and vault-2 should automatically join. You just need to unseal them.

For vault-1:

```bash
# Check if it auto-joined (wait 30 seconds after vault-0 is unsealed)
kubectl logs -n vault vault-1 | grep -i "join\|raft"

# If auto-joined, just unseal it
kubectl exec -n vault vault-1 -- vault operator unseal $UNSEAL_KEY1
kubectl exec -n vault vault-1 -- vault operator unseal $UNSEAL_KEY2
kubectl exec -n vault vault-1 -- vault operator unseal $UNSEAL_KEY3

# Verify
kubectl exec -n vault vault-1 -- vault status
```

For vault-2:

```bash
kubectl exec -n vault vault-2 -- vault operator unseal $UNSEAL_KEY1
kubectl exec -n vault vault-2 -- vault operator unseal $UNSEAL_KEY2
kubectl exec -n vault vault-2 -- vault operator unseal $UNSEAL_KEY3

# Verify
kubectl exec -n vault vault-2 -- vault status
```

Verify Cluster

```bash
# Check all pods are ready
kubectl get pods -n vault

# Expected:
# NAME      READY   STATUS    RESTARTS   AGE
# vault-0   1/1     Running   0          5m
# vault-1   1/1     Running   0          5m
# vault-2   1/1     Running   0          5m

# Check Raft cluster status
kubectl exec -n vault vault-0 -- vault login $VAULT_ROOT_TOKEN
kubectl exec -n vault vault-0 -- vault operator raft list-peers
# Node       Address                        State       Voter
# ----       -------                        -----       -----
# vault-0    vault-0.vault-internal:8201    leader      true
# vault-1    vault-1.vault-internal:8201    follower    true
# vault-2    vault-2.vault-internal:8201    follower    true
```

#### TLS Verification

```bash
kubectl apply -f infrastructure/charts/vault/ingress.yaml
```

```bash
# Get CA certificate
kubectl get secret -n vault vault-server-tls-secret \
  -o jsonpath='{.data.ca\.crt}' | base64 -d > ~/certs/vault-root-ca.crt

# Port forward
kubectl port-forward -n vault vault-0 8200:8200

# In another terminal, test
curl --cacert ~/certs/vault-root-ca.crt https://vault.poddle.uz/v1/sys/health
```

-=-=-=-=-=-=-=-=-=-=--=-=============-=-=-=-=-=-=-=-=-=-=--=-=============-=-=-=-=-=-=-=-=-=-=--=-=============

### Initialization (important)

After pods are running:

```bash
kubectl exec -it -n vault vault-0 -- vault operator init
# Unseal Key 1: ...
# Unseal Key 2: ...
# Unseal Key 3: ...
# Unseal Key 4: ...
# Unseal Key 5: ...
#
# Initial Root Token: ...
#
# Vault initialized with 5 key shares and a key threshold of 3. Please securely
# distribute the key shares printed above. When the Vault is re-sealed,
# restarted, or stopped, you must supply at least 3 of these keys to unseal it
# before it can start servicing requests.
#
# Vault does not store the generated root key. Without at least 3 keys to
# reconstruct the root key, Vault will remain permanently sealed!
#
# It is possible to generate new unseal keys, provided you have a quorum of
# existing unseal keys shares. See "vault operator rekey" for more information.
```

```bash
cat > ~/.zsh_secrets <<EOF
UNSEAL_KEY1=''
UNSEAL_KEY2=''
UNSEAL_KEY3=''
UNSEAL_KEY4=''
UNSEAL_KEY5=''

VAULT_TOKEN=''
EOF && source ~/.zshrc
```

Then unseal each pod because each pod has vault

vault-0

```bash
kubectl exec -it -n vault vault-0 -- vault operator unseal $UNSEAL_KEY1
kubectl exec -it -n vault vault-0 -- vault operator unseal $UNSEAL_KEY2
kubectl exec -it -n vault vault-0 -- vault operator unseal $UNSEAL_KEY3
```

vault-1

```bash
kubectl exec -it -n vault vault-1 -- vault operator unseal $UNSEAL_KEY1
kubectl exec -it -n vault vault-1 -- vault operator unseal $UNSEAL_KEY2
kubectl exec -it -n vault vault-1 -- vault operator unseal $UNSEAL_KEY3
```

vault-2

```bash
kubectl exec -it -n vault vault-2 -- vault operator unseal $UNSEAL_KEY1
kubectl exec -it -n vault vault-2 -- vault operator unseal $UNSEAL_KEY2
kubectl exec -it -n vault vault-2 -- vault operator unseal $UNSEAL_KEY3
```

Repeat until quorum is reached (usually 2/3)

#### Vault KV setup with Kubernetes auth method

## Setup vault policies and roles

### Tenant setup

#### This is the "Dynamic" policy. It uses the {{identity...}} template to lock the user into their own namespace

> First run this command and get Accessor, because vault generate dynamically!

```bash
~ ❯ vault auth list
Path           Type          Accessor                    Description                Version
----           ----          --------                    -----------                -------
kubernetes/    kubernetes    auth_kubernetes_4df6263c    n/a                        n/a
token/         token         auth_token_3bb5335c         token based credentials    n/a
~ ❯
```

> Then replace tenant-policy.hcl to take account correct Accessor!

```bash
vault policy write tenant-policy infrastructure/vault/tenant-policy.hcl
```

#### Write role for tenant

> Vault secret policies to roles because it enforces least privilege, ensuring applications and users only access the > specific secrets and paths they need, rather than having broad access. Roles act as logical groupings for identities > (like apps or users), and policies define what actions (read, write, list) they can perform on specific secret paths > (e.g., kv/data/myapp/*), creating fine-grained authorization for secure, efficient secrets management.

```bash
vault write auth/kubernetes/role/tenant-role \
  bound_service_account_names=default \
  bound_service_account_namespaces="user-*" \
  policies=tenant-policy \
  ttl=24h
```

### Admin setup

```bash
vault policy write admin-policy infrastructure/vault/admin-policy.hcl
```

#### Write role for admin

```bash
vault write auth/kubernetes/role/compute-provisioner \
  bound_service_account_names=compute-provisioner \
  bound_service_account_namespaces=poddle-system \
  policies=admin-policy \
  ttl=24h
```

---

## 6. Install vault-secrets-operator

### Prerequisites

* `kubectl` configured to access your cluster
* Vault server running and accessible
* `vault` CLI installed and configured

```bash
helm install vault-secrets-operator hashicorp/vault-secrets-operator \
  -n vault-secrets-operator --create-namespace

# or

helm upgrade --install vault-secrets-operator hashicorp/vault-secrets-operator \
  --values infrastructure/charts/vault-secrets-operator/vault-secrets-operator-values.yaml \
  -n vault-secrets-operator --create-namespace
```

Verify

```bash
~/Documents/linux-setup master ❯ kubectl get pods -n vault-secrets-operator
NAME                                                         READY   STATUS    RESTARTS   AGE
vault-secrets-operator-controller-manager-645c4f6b6d-jpkrz   3/3     Running   0          85s
~/Documents/linux-setup master ❯
```

---
