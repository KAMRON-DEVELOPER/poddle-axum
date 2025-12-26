# Poddle PaaS Observability Stack Deployment Guide

## Architecture Overview

### Development Setup

- **Host (192.168.31.53)**: Mimir, Loki, Tempo, Grafana (Docker Compose)
- **K3s Cluster (192.168.31.10)**: Alloy agents collecting telemetry
- **DNS**: `*.poddle.uz` resolves to K3s, observability endpoints to host

### Production Setup (initially 3 Servers)

- All services run inside Kubernetes with HA
- Alloy deployed as DaemonSet on all nodes
- Mimir, Loki, Tempo deployed with replication

---

## Development Deployment

### Setup DNS Records

Add to your `/etc/dnsmasq.d/local-domains.conf`:

```conf
# Existing
address=/.poddle.uz/192.168.31.10
address=/vault.poddle.uz/192.168.31.53

# Add observability endpoints
address=/grafana.poddle.uz/192.168.31.53
address=/mimir.poddle.uz/192.168.31.53
address=/loki.poddle.uz/192.168.31.53
address=/tempo.poddle.uz/192.168.31.53
```

Restart dnsmasq:

```bash
sudo systemctl restart dnsmasq
```

### Deploy Observability Stack on Host

> Later

### Deploy Alloy to K3s

Create ConfigMap from your config:

```bash
cd ~/Documents/Coding/rust/backend/poddle-axum
kubectl create namespace observability
kubectl create configmap alloy-config --from-file=config.alloy=infrastructure/observability/alloy/config.alloy -n observability --dry-run=client -o yaml | kubectl apply -f -
```

Install Alloy with Helm:

```bash
helm upgrade --install alloy grafana/alloy \
  --namespace observability \
  -f infrastructure/observability/alloy/alloy-values.yaml \
  --create-namespace
```

Verify deployment:

```bash
kubectl get pods -n observability
kubectl logs -n observability -l app.kubernetes.io/name=alloy -f
```

## Production Deployment (3 Servers)

For production, you'll deploy everything inside Kubernetes:

### Architecture

```bash
Server 1, 2, 3: K3s cluster with embedded etcd
├── Mimir (3 replicas)
├── Loki (3 replicas)
├── Tempo (3 replicas)
├── Grafana (2 replicas)
└── Alloy (DaemonSet on all nodes)
```

### Install Mimir (HA)

```bash
helm repo add grafana https://grafana.github.io/helm-charts

helm upgrade --install mimir grafana/mimir-distributed \
  --namespace observability \
  --set mimir.structuredConfig.common.storage.backend=s3 \
  --set mimir.structuredConfig.common.storage.s3.endpoint=minio.storage.svc:9000 \
  --set ingester.replicas=3 \
  --set distributor.replicas=3 \
  --set querier.replicas=3 \
  --set query-frontend.replicas=2 \
  --set store-gateway.replicas=3
```

### Install Loki (HA)

```bash
helm upgrade --install loki grafana/loki \
  --namespace observability \
  --set loki.auth_enabled=false \
  --set loki.commonConfig.replication_factor=3 \
  --set read.replicas=3 \
  --set write.replicas=3 \
  --set backend.replicas=3
```

### Install Tempo (HA)

```bash
helm upgrade --install tempo grafana/tempo-distributed \
  --namespace observability \
  --set traces.otlp.grpc.enabled=true \
  --set traces.otlp.http.enabled=true \
  --set ingester.replicas=3 \
  --set distributor.replicas=3 \
  --set querier.replicas=3 \
  --set compactor.replicas=1
```

### Install Grafana

```bash
helm upgrade --install grafana grafana/grafana \
  --namespace observability \
  --set replicas=2 \
  --set persistence.enabled=true \
  --set persistence.size=10Gi
```

---

---

---

> [!NOTE]
> Head to <https://github.com/grafana/helm-charts/blob/main/README.md>

---

## Usage

[Helm](https://helm.sh) must be installed to use the charts.
Please refer to Helm's [documentation](https://helm.sh/docs/) to get started.

> [!NOTE]
> First add the Grafana Helm chart repository, Helm repo is at <https://artifacthub.io/packages/helm/grafana/grafana>

```bash
helm repo add grafana https://grafana.github.io/helm-charts
helm repo update
```

You can then run `helm search repo grafana` to see the charts.

---

## Installing Alloy

> [!NOTE]
> You can head to these and  for more info.
> <https://grafana.com/docs/alloy/latest/set-up/install/kubernetes/>
> <https://grafana.com/docs/alloy/latest/set-up/deploy/>
> <https://grafana.com/docs/alloy/latest/configure/kubernetes/>
> Helm repo is at <https://artifacthub.io/packages/helm/grafana/alloy>

```bash
helm install alloy grafana/alloy \
  --version 1.5.1 \
  -n observability \
  --create-namespace \

# or

helm install alloy grafana/alloy \
  -n observability \
  --create-namespace \
  --values infrastructure/observability/alloy/alloy-values.yaml
```

To delete alloy run this command

```bash
helm delete alloy -n observability
```

> [!NOTE]
> Also you probably want to use `alloy fmt -w infrastructure/observability/alloy/config.alloy`.
> So download and install binary or install `grafana-alloy` aur package using `yay` by `yay -S grafana-alloy`.
Download the latest Linux binary

```bash
curl -LO https://github.com/grafana/alloy/releases/latest/download/alloy-linux-amd64.zip
```

Extract and install

```bash
unzip alloy-linux-amd64.zip
sudo install alloy-linux-amd64 /usr/local/bin/alloy
```

### Alloy configurations

> You need to prepared configuration before.

```bash
kubectl create configmap --namespace observability alloy-config "--from-file=config.alloy=./infrastructure/observability/alloy/config.alloy"
```

```bash
helm upgrade --namespace observability alloy grafana/alloy -f infrastructure/observability/alloy/alloy-values.yaml
```

---

## Installing Loki

> [!NOTE]
> You can head to <https://grafana.com/docs/alloy/latest/set-up/install/kubernetes/> for more info.
> Helm repo is at <https://artifacthub.io/packages/helm/grafana/alloy>

```bash
helm install loki grafana/loki \
  --version 1.5.1 \
  -n observability \
  --create-namespace \

# or

helm install loki grafana/loki \
  -n observability \
  --create-namespace \
  --values infrastructure/observability/loki/loki-values.yaml
```

To delete loki run this command

```bash
helm delete loki -n observability
```

---

## Installing Tempo

> [!NOTE]
> You can head to <https://grafana.com/docs/alloy/latest/set-up/install/kubernetes/> for more info.
> Helm repo is at <https://artifacthub.io/packages/helm/grafana/alloy>

```bash
helm install tempo-distributed grafana/tempo-distributed \
  --version 1.5.1 \
  -n observability \
  --create-namespace \

# or

helm install tempo-distributed grafana/tempo-distributed \
  -n observability \
  --create-namespace \
  --values infrastructure/observability/tempo-distributed/tempo-distributed-values.yaml
```

To delete tempo-distributed run this command

```bash
helm delete tempo-distributed -n observability
```

---

## Installing Mimir

> [!NOTE]
> You can head to <https://grafana.com/docs/alloy/latest/set-up/install/kubernetes/> for more info.
> Helm repo is at <https://artifacthub.io/packages/helm/grafana/alloy>

```bash
helm install tempo-distributed grafana/tempo-distributed \
  --version 1.5.1 \
  -n observability \
  --create-namespace \

# or

helm install tempo-distributed grafana/tempo-distributed \
  -n observability \
  --create-namespace \
  --values infrastructure/observability/tempo-distributed/tempo-distributed-values.yaml
```

To delete tempo-distributed run this command

```bash
helm delete tempo-distributed -n observability
```
