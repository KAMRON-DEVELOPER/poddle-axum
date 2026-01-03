# Poddle PaaS Observability Stack Deployment Guide

```bash
helm show values grafana/loki > infrastructure/charts/loki/values.yaml
helm show values grafana/tempo > infrastructure/charts/tempo/values.yaml
helm show values grafana/mimir-distributed > infrastructure/charts/mimir/values.yaml
```
