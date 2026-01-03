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
# kubectl get all -n prometheus
# NAME                                                READY   STATUS    RESTARTS   AGE
# pod/promethues-prometheus-server-7656d857d9-wfjjg   2/2     Running   0          53s

# NAME                                   TYPE        CLUSTER-IP      EXTERNAL-IP   PORT(S)   AGE
# service/promethues-prometheus-server   ClusterIP   10.43.203.233   <none>        80/TCP    53s

# NAME                                           READY   UP-TO-DATE   AVAILABLE   AGE
# deployment.apps/promethues-prometheus-server   1/1     1            1           53s

# NAME                                                      DESIRED   CURRENT   READY   AGE
# replicaset.apps/promethues-prometheus-server-7656d857d9   1         1         1       53s
# kubectl get pod promethues-prometheus-server-7656d857d9-wfjjg -n prometheus -o jsonpath='{.spec.containers[*].name}'
# prometheus-server-configmap-reload prometheus-server
```

### Create ingress, so applications can access to prometheus

```bash
kubectl apply -f infrastructure/charts/prometheus-community/ingress.yaml
```
