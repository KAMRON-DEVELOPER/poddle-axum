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

helm install cilium cilium/cilium \
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
helm repo add metallb https://metallb.github.io/metallb
helm repo update

helm install metallb metallb/metallb \
  --namespace metallb-system --create-namespace

# or

helm install metallb metallb/metallb \                                                      
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

helm install traefik traefik/traefik \                                                      
  --values infrastructure/charts/traefik/traefik-values.yaml
  --namespace traefik --create-namespace \
```

Verify Traefik got an external IP:

```bash
kubectl get svc -n traefik
# Should show EXTERNAL-IP: 192.168.31.10
```

---

## 4. Install vault

```bash
helm install vault hashicorp/vault \
  --namespace traefik --create-namespace

# or

helm install vault hashicorp/vault \ 
  --values infrastructure/charts/vault/vault-values.yaml
  --namespace vault --create-namespace

```

### Initialization (important)

After pods are running:

```bash
kubectl exec -it vault-0 -- vault operator init
```

Save:

* Unseal keys
* Root token

Then unseal each pod because each pod has vault

```bash
vault operator unseal &UNSEAL_KEY1
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

## 5. Install vault-secrets-operator

### Prerequisites

* `kubectl` configured to access your cluster
* Vault server running and accessible
* `vault` CLI installed and configured

```bash
helm install vault-secrets-operator hashicorp/vault-secrets-operator \
  -n vault-secrets-operator --create-namespace

# or

helm install vault-secrets-operator hashicorp/vault-secrets-operator \
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

## 6. Install cert-manager

```bash
helm repo add jetstack https://charts.jetstack.io
helm repo update

helm install cert-manager jetstack/cert-manager \
  --set crds.enabled=true \
  --namespace cert-manager --create-namespace

# or

helm repo add jetstack https://charts.jetstack.io --force-update
helm upgrade --install \
cert-manager jetstack/cert-manager \
--set crds.enabled=true \
--set "extraArgs={--enable-gateway-api}" \
--namespace cert-manager --create-namespace
```

Verify:

```bash
kubectl get pods -n cert-manager
# All pods should be Running
```
