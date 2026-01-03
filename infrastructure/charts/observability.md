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
