# Poddle PaaS Observability Stack Deployment Guide

---

## Chart setup

```bash
helm show values grafana/loki > infrastructure/charts/loki/values.yaml
helm show values grafana/tempo > infrastructure/charts/tempo/values.yaml
helm show values grafana/mimir-distributed > infrastructure/charts/mimir/values.yaml
```

---

## Constraints

> [!INFO]
> In dev mode we don't deploy mimir, it is too heavy, we deploy prometheus-community/promethues.

```bash
helm upgrade --install prometheus prometheus-community/prometheus \
  --values infrastructure/charts/prometheus-community/prometheus-values.yaml \
  --namespace prometheus --create-namespace
```

Verify

```bash
kubectl get all -n prometheus
# NAME                                     READY   STATUS    RESTARTS   AGE
# pod/prometheus-server-674f658949-spg7p   2/2     Running   0          49s
# 
# NAME                        TYPE        CLUSTER-IP    EXTERNAL-IP   PORT(S)   AGE
# service/prometheus-server   ClusterIP   10.43.59.10   <none>        80/TCP    49s
# 
# NAME                                READY   UP-TO-DATE   AVAILABLE   AGE
# deployment.apps/prometheus-server   1/1     1            1           49s
# 
# NAME                                           DESIRED   CURRENT   READY   AGE
# replicaset.apps/prometheus-server-674f658949   1         1         1       49s
```

```bash
kubectl get pod prometheus-server-674f658949-spg7p -n prometheus -o jsonpath='{.spec.containers[*].name}'
# prometheus-server-configmap-reload prometheus-server
```

### Create ingress, so applications can access to prometheus

```bash
kubectl apply -f infrastructure/charts/prometheus-community/ingress.yaml
```

### When creating Loki and Tempo we use Traefik's externalaa to referance host Minio instance

Then `Loki` and `Tempo` s3 endpoint will be like `minio-external.observability.svc.cluster.local:9000`

```bash
kubectl create ns observability
kubectl apply -f infrastructure/manifests/minio-external.yaml
```

#### Create Minio secrets for `loki` and `tempo`, so they can connect

```bash
kubectl create ns loki
kubectl create secret generic minio-credentials \
  --from-literal=S3_ACCESS_KEY=... \
  --from-literal=S3_SECRET_KEY=... \
  -n loki
kubectl create ns tempo
kubectl create secret generic minio-credentials \
  --from-literal=S3_ACCESS_KEY=... \
  --from-literal=S3_SECRET_KEY=... \
  -n tempo
```

### Loki setup

```bash
helm upgrade --install loki grafana/loki \
  --values infrastructure/charts/loki/loki-values.yaml \
  --namespace loki --create-namespace
```

Verify

```bash
kubectl get all -n loki
# NAME         READY   STATUS    RESTARTS   AGE
# pod/loki-0   2/2     Running   0          2m24s

# NAME                      TYPE        CLUSTER-IP     EXTERNAL-IP   PORT(S)             AGE
# service/loki              ClusterIP   10.43.102.19   <none>        3100/TCP,9095/TCP   2m24s
# service/loki-headless     ClusterIP   None           <none>        3100/TCP            2m24s
# service/loki-memberlist   ClusterIP   None           <none>        7946/TCP            2m24s

# NAME                    READY   AGE
# statefulset.apps/loki   1/1     2m24s
```

```bash
kubectl get pod loki-0 -n loki -o jsonpath='{.spec.containers[*].name}'
# loki loki-sc-rules
```

#### We may need to create IngressRoute(Traefik), so axum microservices can access

```bash
kubectl apply -f infrastructure/charts/loki/ingress.yaml
```

### Tempo setup

```bash
helm upgrade --install tempo grafana/tempo \
  --values infrastructure/charts/tempo/tempo-values.yaml \
  --namespace tempo --create-namespace
```

Connecting Grafana to Loki

If Grafana operates within the cluster, you'll set up a new Loki datasource by utilizing the following URL:

<http://loki.loki.svc.cluster.local:3100/>
