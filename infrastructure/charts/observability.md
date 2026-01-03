# Poddle PaaS Observability Stack Deployment Guide

---

## Prerequisites: External MinIO Service Configuration

> [!IMPORTANT]
> Before deploying the observability stack, you must configure access to the external MinIO instance running on the host machine.
>
> Since MinIO runs outside the Kubernetes cluster, we need to create a Service that points to the host IP address.

### Understanding the Setup

When you create a `Service` without selectors in Kubernetes, you must manually create an `EndpointSlice` object to define the backend IP addresses. The Service and `EndpointSlice` are implicitly bound by sharing the same name and namespace.

This allows Loki and Tempo to reference MinIO using the cluster-internal DNS name:

```bash
minio-external.observability.svc.cluster.local:9000
```

> [!NOTE]
> **Why not use `ExternalName` service type?**
>
> The `ExternalName` service type only works with DNS hostnames, not IP addresses. Since we're connecting to `192.168.31.2`, we need to use a regular Service with manual `EndpointSlice`.

### Deploy the External Service

```bash
# Create the observability namespace
kubectl create ns observability

# Apply the MinIO external service configuration
kubectl apply -f infrastructure/manifests/minio-external.yaml
```

## Install Promethues, Loki, Tempo, Alloy

### Create Minio secrets for `loki` and `tempo`, so they can connect

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

### Install Prometheus

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

#### We may need to create IngressRoute(Traefik) for Prometheus, so axum microservices can access

```bash
kubectl apply -f infrastructure/charts/prometheus-community/ingress.yaml
```

### Install Loki

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

#### We may need to create IngressRoute(Traefik) for Loki, so axum microservices can access

```bash
kubectl apply -f infrastructure/charts/loki/ingress.yaml
```

### Install Tempo

```bash
helm upgrade --install tempo grafana/tempo \
  --values infrastructure/charts/tempo/tempo-values.yaml \
  --namespace tempo --create-namespace
```

Verify

```bash
kubectl get all -n tempo
# NAME          READY   STATUS    RESTARTS   AGE
# pod/tempo-0   1/1     Running   0          2m41s

# NAME            TYPE        CLUSTER-IP    EXTERNAL-IP   PORT(S)                                                                                                   AGE
# service/tempo   ClusterIP   10.43.57.85   <none>        6831/UDP,6832/UDP,3200/TCP,14268/TCP,14250/TCP,9411/TCP,55680/TCP,55681/TCP,4317/TCP,4318/TCP,55678/TCP   2m41s

# NAME                     READY   AGE
# statefulset.apps/tempo   1/1     2m41s
```

#### We may need to create IngressRoute(Traefik) for Tempo, so axum microservices can access

```bash
kubectl apply -f infrastructure/charts/tempo/ingress.yaml
```
