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
kubectl apply -f infrastructure/charts/prometheus-community/prometheus-ingress.yaml
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
kubectl apply -f infrastructure/charts/loki/loki-ingress.yaml
```

### Install Tempo

```bash
helm upgrade --install tempo grafana/tempo \
  --f infrastructure/charts/tempo/tempo-values.yaml \
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
kubectl apply -f infrastructure/charts/tempo/tempo-ingress.yaml
```

### Install Grafana

Grafana provides visualization and dashboards for your observability stack (Prometheus, Mimir, Loki, Tempo).

#### 1. Create Admin Credentials Secret

```bash
kubectl create ns grafana
kubectl create secret generic grafana-credentials \
  --from-literal=admin-user=admin \
  --from-literal=admin-password='1213' \
  -n grafana
```

> **Note**: Change the default password in production environments.

#### 2. Install Grafana with Helm

```bash
helm upgrade --install grafana grafana/grafana \
  -f infrastructure/charts/grafana/grafana-values.yaml \
  -n grafana --create-namespace
```

#### 3. Apply Datasource Configuration

The datasources are managed via ConfigMap and will be automatically loaded by Grafana's sidecar:

```bash
kubectl apply -f infrastructure/charts/grafana/grafana-datasources.yaml
```

#### 4. Access Grafana

```bash
kubectl apply -f infrastructure/charts/grafana/grafana-ingress.yaml
```

Then open: <https://grafana.poddle.uz>

**Login credentials:**

- Username: `admin`
- Password: `admin` (or the password you set in the secret)

#### 5. Verify Datasources

After logging in, navigate to **Configuration → Data Sources** to verify that Prometheus, Loki, and Tempo are connected.

#### Configuration Files

- `grafana-values.yaml` - Main Helm values with persistence, resources, and sidecar configuration
- `grafana-datasources.yaml` - ConfigMap with all datasource definitions (Prometheus, Loki, Tempo)
- `grafana-ingress.yaml` - (Optional) Ingress configuration for external access

#### Managing Datasources

Datasources are managed dynamically via ConfigMaps with the label `grafana_datasource: "1"`.

**To add a new datasource:**

1. Create a ConfigMap with the `grafana_datasource: "1"` label
2. Apply it with `kubectl apply`
3. Grafana will auto-detect and load it

**To update an existing datasource:**

```bash
kubectl edit configmap grafana-datasource -n grafana
# Or update the YAML file and reapply
kubectl apply -f infrastructure/charts/grafana/grafana-datasources.yaml
```

**To remove a datasource:**

```bash
kubectl delete configmap grafana-datasource -n grafana
```

Troubleshooting

**Check if datasources are loaded:**

```bash
kubectl logs -n grafana -l app.kubernetes.io/name=grafana -c grafana-sc-datasources
```

**Restart Grafana pod if needed:**

```bash
kubectl rollout restart deployment grafana -n grafana
```

**Verify pod is running:**

```bash
kubectl get pods -n grafana
kubectl describe pod -n grafana <pod-name>
```

### Install Alloy

#### Installing helm releases

Folder structure

```bash
infrastructure/charts/alloy
├── agent
│   ├── alloy-values.yaml
│   └── config.alloy
├── gateway
│   ├── alloy-values.yaml
│   └── config.alloy
└── values.yaml
```

Labels added by rust backend

```rust
let mut labels = BTreeMap::new();
labels.insert("app".to_string(), deployment_name.to_string());
labels.insert("project-id".to_string(), project_id.to_string());
labels.insert("deployment-id".to_string(), deployment_id.to_string());
labels.insert("managed-by".to_string(), "poddle".to_string());
```

##### 1. Install Gateway (StatefulSet)

```bash
kubectl create namespace alloy-gateway --dry-run=client -o yaml | kubectl apply -f -
kubectl create configmap alloy-config \
  --from-file=config.alloy=infrastructure/charts/alloy/gateway/config.alloy \
  -n alloy-gateway --dry-run=client -o yaml | kubectl apply -f -

helm upgrade --install alloy-gateway grafana/alloy \
  -f infrastructure/charts/alloy/gateway/alloy-values.yaml \
  --namespace alloy-gateway --create-namespace

# or when `alloy.configMap.create: true`

helm upgrade --install alloy-gateway grafana/alloy \
  --values infrastructure/charts/alloy/gateway/alloy-values.yaml \
  --set-file alloy.configMap.content=infrastructure/charts/alloy/gateway/config.alloy \
  --namespace alloy-gateway --create-namespace
```

Expose

```bash
kubectl apply -f infrastructure/charts/alloy/gateway/alloy-ingress.yaml
```

##### 2. Install Agent (DaemonSet)

```bash
kubectl create namespace alloy-agent --dry-run=client -o yaml | kubectl apply -f -
kubectl create configmap alloy-config \
  --from-file=config.alloy=infrastructure/charts/alloy/agent/config.alloy \
  -n alloy-agent --dry-run=client -o yaml | kubectl apply -f -

helm upgrade --install alloy-agent grafana/alloy \
  --values infrastructure/charts/alloy/agent/alloy-values.yaml \
  --namespace alloy-agent --create-namespace

# or when `alloy.configMap.create: true`

helm upgrade --install alloy-agent grafana/alloy \
  --values infrastructure/charts/alloy/agent/alloy-values.yaml \
  --set-file alloy.configMap.content=infrastructure/charts/alloy/agent/config.alloy \
  --namespace alloy-agent --create-namespace
```

Expose

```bash
kubectl apply -f infrastructure/charts/alloy/agent/alloy-ingress.yaml
```

#### Grafana Alloy Architecture Overview

This document explains the architecture of our observability stack using Grafana Alloy in a Kubernetes environment. We deploy Alloy in two distinct patterns: **DaemonSet** and **StatefulSet**, each serving specific purposes based on the physical constraints and logical requirements of different telemetry signals.

---

#### Understanding the Two Deployment Patterns

##### Why Two Patterns?

The key insight is that different telemetry signals have different collection requirements:

- **Local signals** (logs, host metrics) benefit from physical proximity to the data source
- **Remote signals** (traces, application metrics) require centralized processing and load balancing

Using the right pattern for each signal type optimizes both performance and resource utilization.

---

#### DaemonSet: The Physical Collector

##### Role and Purpose of DaemonSet

The DaemonSet deployment runs one Alloy pod on **every node** in the cluster. It acts as a local collector that is "glued" to the node's physical resources.

##### Why DaemonSet?

**Physical Locality Requirements:**

1. **Logs**: Container logs are written to `stdout`, and Kubernetes stores them as files on the node's filesystem at `/var/log/pods/...`. Reading these files locally is far more efficient than streaming them over the network via the Kubernetes API.

2. **Host Metrics**: To measure node-level resources (CPU, RAM, disk), you need a process running directly on that node to query the kernel.

##### What Data Does DaemonSet Collect?

The DaemonSet handles **infrastructure-level signals**:

- **Container logs** from pods running on its node
- **Host metrics** (CPU, memory, disk, network) from the node itself
- **Container metrics** (cAdvisor data) for all containers on the node

##### Signal Flow for DaemonSet

```bash
Container Logs:
Rust App → stdout → Node Disk (/var/log/pods/) → DaemonSet (file read) → Loki

Host Metrics:
Kernel → DaemonSet (local query) → Prometheus/Mimir

Container Metrics:
cAdvisor/Kubelet → DaemonSet (local scrape) → Prometheus/Mimir
```

##### Key Components Used in DaemonSet

From the Alloy component list, these are the primary components configured in the DaemonSet:

**Discovery:**

- `discovery.kubernetes` - Discovers pods running on the local node only
- `discovery.relabel` - Filters targets to ensure only local node resources are collected

**Log Collection:**

- `loki.source.file` - Tails log files directly from disk
- `loki.process` - Optional log parsing and transformation
- `loki.write` - Sends logs to Loki

**Metrics Collection:**

- `prometheus.exporter.unix` - Collects host-level metrics (Node Exporter equivalent)
- `prometheus.exporter.cadvisor` - Collects container metrics
- `discovery.kubelet` - Alternative way to discover containers on the node
- `prometheus.scrape` - Scrapes local exporters
- `prometheus.remote_write` - Sends metrics to Prometheus/Mimir

**Why Not Use DaemonSet for Everything?**

While you *could* collect application metrics from the DaemonSet, it creates problems:

- Uneven load distribution: A node with 100 small pods would overload its DaemonSet, while a node with 1 large pod would have an idle DaemonSet
- No coordination: DaemonSets work independently and can't distribute work efficiently

---

#### StatefulSet: The Central Aggregator

##### Role and Purpose of StatefulSet

The StatefulSet deployment runs a **fixed number of replicas** (typically 2-3) that form a cluster. It acts as a centralized receiver and intelligent processor for application-level telemetry.

##### Why StatefulSet?

**Stable Network Identity & Clustering Requirements:**

1. **Traces (Push)**: Applications need a stable endpoint to send traces to (e.g., `http://alloy-gateway:4317`). A StatefulSet provides this through a headless service with predictable pod names (`alloy-0`, `alloy-1`).

2. **Load Balancing**: For scraping thousands of application pods, StatefulSet replicas can coordinate and distribute the scraping work evenly across the cluster, regardless of pod placement.

3. **Stateful Processing**: Advanced features like tail sampling require seeing complete traces, which means spans need to be aggregated in one place.

##### What Data Does StatefulSet Collect?

The StatefulSet handles **application-level signals**:

- **Traces** pushed from applications via OTLP
- **Application metrics** scraped from application `/metrics` endpoints
- **Custom metrics** pushed from applications via OTLP

##### Signal Flow for StatefulSet

```bash
Traces (Push):
Rust App → (OTLP gRPC/HTTP) → StatefulSet (port 4317/4318) → Tempo

Application Metrics (Pull):
StatefulSet → (HTTP scrape) → Rust App /metrics endpoint → Prometheus/Mimir

Application Metrics (Push):
Rust App → (OTLP) → StatefulSet → Prometheus/Mimir
```

##### Key Components Used in StatefulSet

From the Alloy component list, these are the primary components configured in the StatefulSet:

**OTLP Receivers:**

- `otelcol.receiver.otlp` - Accepts traces and metrics from applications (ports 4317 gRPC, 4318 HTTP)

**OTLP Processors:**

- `otelcol.processor.batch` - Batches telemetry data to reduce network overhead
- `otelcol.processor.k8sattributes` - **Critical** - Enriches data with Kubernetes metadata (namespace, pod name, labels)
- `otelcol.processor.tail_sampling` - Intelligent sampling (keep all errors, sample successful requests)
- `otelcol.processor.attributes` - Modifies or adds attributes to spans/metrics
- `otelcol.processor.filter` - Filters out unwanted telemetry
- `otelcol.processor.transform` - Advanced data transformation

**OTLP Exporters:**

- `otelcol.exporter.otlp` - Sends traces to Tempo
- `otelcol.exporter.otlphttp` - Alternative HTTP-based export
- `otelcol.exporter.prometheus` - Converts OTLP metrics to Prometheus format

**Prometheus Components:**

- `prometheus.scrape` - Scrapes application `/metrics` endpoints (with clustering enabled)
- `prometheus.operator.servicemonitors` - Discovers targets using ServiceMonitor CRDs
- `prometheus.operator.podmonitors` - Discovers targets using PodMonitor CRDs
- `prometheus.remote_write` - Sends metrics to Prometheus/Mimir

**Discovery:**

- `discovery.kubernetes` - Discovers all pods/services across the cluster
- `discovery.relabel` - Filters and relabels discovered targets

**Why StatefulSet for Application Metrics?**

The StatefulSet cluster provides intelligent load distribution:

- With 2 replicas, each replica automatically scrapes ~50% of application endpoints
- Load is balanced across the entire cluster, not per-node
- Replicas coordinate through clustering to avoid duplicate scraping

---

#### Component Reference Guide

This section lists all available Alloy components organized by category. Use this as a reference when building your configurations.

##### OpenTelemetry Collector Components (`otelcol.*`)

**Authentication:**

- `otelcol.auth.basic` - Basic authentication
- `otelcol.auth.bearer` - Bearer token authentication
- `otelcol.auth.headers` - Custom header authentication
- `otelcol.auth.oauth2` - OAuth2 authentication
- `otelcol.auth.sigv4` - AWS Signature V4 authentication

**Connectors:**

- `otelcol.connector.host_info` - Adds host information to telemetry
- `otelcol.connector.servicegraph` - Generates service dependency graphs from traces
- `otelcol.connector.spanlogs` - Converts spans to logs
- `otelcol.connector.spanmetrics` - Generates metrics from spans (RED metrics)

**Receivers:**

- `otelcol.receiver.otlp` - Receives OTLP data (traces, metrics, logs)
- `otelcol.receiver.prometheus` - Receives Prometheus metrics
- `otelcol.receiver.jaeger` - Receives Jaeger traces
- `otelcol.receiver.zipkin` - Receives Zipkin traces
- `otelcol.receiver.kafka` - Receives data from Kafka
- `otelcol.receiver.filelog` - Reads logs from files
- Others: `awscloudwatch`, `datadog`, `influxdb`, `loki`, etc.

**Processors:**

- `otelcol.processor.batch` - Batches telemetry for efficiency
- `otelcol.processor.k8sattributes` - Adds Kubernetes metadata
- `otelcol.processor.attributes` - Modifies attributes
- `otelcol.processor.filter` - Filters telemetry
- `otelcol.processor.transform` - Transforms telemetry data
- `otelcol.processor.tail_sampling` - Intelligent trace sampling
- `otelcol.processor.memory_limiter` - Prevents OOM issues
- `otelcol.processor.resourcedetection` - Detects resource attributes
- Others: `span`, `probabilistic_sampler`, `groupbyattrs`, etc.

**Exporters:**

- `otelcol.exporter.otlp` - Exports to OTLP endpoints (Tempo, etc.)
- `otelcol.exporter.otlphttp` - OTLP over HTTP
- `otelcol.exporter.prometheus` - Exports as Prometheus metrics
- `otelcol.exporter.loki` - Exports logs to Loki
- `otelcol.exporter.kafka` - Exports to Kafka
- Others: `datadog`, `splunkhec`, `awss3`, etc.

##### Loki Components (`loki.*`)

**Log Sources:**

- `loki.source.file` - Reads logs from files (used in DaemonSet)
- `loki.source.kubernetes` - Reads logs from Kubernetes API
- `loki.source.podlogs` - Specific pod log collection
- `loki.source.docker` - Docker container logs
- `loki.source.journal` - Systemd journal logs
- `loki.source.syslog` - Syslog protocol
- Others: `kafka`, `api`, `gcplog`, `cloudflare`, etc.

**Log Processing:**

- `loki.process` - Parse and transform logs
- `loki.relabel` - Relabel log streams
- `loki.enrich` - Enrich logs with additional metadata
- `loki.secretfilter` - Filter sensitive data from logs

**Log Export:**

- `loki.write` - Sends logs to Loki

##### Prometheus Components (`prometheus.*`)

**Exporters:**

- `prometheus.exporter.unix` - Linux host metrics (Node Exporter)
- `prometheus.exporter.cadvisor` - Container metrics
- `prometheus.exporter.windows` - Windows host metrics
- `prometheus.exporter.process` - Process-level metrics
- Application-specific: `mysql`, `postgres`, `redis`, `mongodb`, `kafka`, etc.
- Cloud-specific: `cloudwatch`, `azure`, `gcp`

**Service Discovery & Scraping:**

- `prometheus.scrape` - Scrapes Prometheus metrics
- `prometheus.operator.servicemonitors` - Uses ServiceMonitor CRDs
- `prometheus.operator.podmonitors` - Uses PodMonitor CRDs
- `prometheus.relabel` - Relabels metrics

**Metrics Export:**

- `prometheus.remote_write` - Sends metrics to Prometheus/Mimir

##### Discovery Components (`discovery.*`)

**Kubernetes:**

- `discovery.kubernetes` - Discovers K8s resources (pods, services, nodes)
- `discovery.kubelet` - Discovers via Kubelet API

**Cloud Providers:**

- `discovery.ec2` - AWS EC2 instances
- `discovery.gce` - Google Compute Engine
- `discovery.azure` - Azure VMs
- `discovery.digitalocean`, `discovery.hetzner`, `discovery.linode`, etc.

**Service Discovery:**

- `discovery.consul` - Consul services
- `discovery.dns` - DNS SRV records
- `discovery.docker` - Docker containers
- `discovery.http` - HTTP-based discovery

**Utility:**

- `discovery.relabel` - Filters and transforms discovered targets

##### Local Utilities (`local.*`)

- `local.file` - Reads files from disk
- `local.file_match` - Matches files by pattern

---

#### Decision Matrix: Which Pattern to Use?

| Signal Type | Deployment Pattern | Why? |
| ------------- | ------------------- | ------ |
| **Container Logs** | DaemonSet | Files are on local disk |
| **Host Metrics** | DaemonSet | Need direct kernel access |
| **Container Metrics** | DaemonSet | cAdvisor/Kubelet are node-local |
| **Traces** | StatefulSet | Need stable receiver endpoint & sampling logic |
| **Application Metrics (scrape)** | StatefulSet | Need cluster-wide load balancing |
| **Application Metrics (push)** | StatefulSet | Need stable receiver endpoint |

---

#### Common Configuration Patterns

##### DaemonSet Configuration Example

```river
// Discover pods on this node only
discovery.kubernetes "local_pods" {
  role = "pod"
  selectors {
    role  = "pod"
    field = "spec.nodeName=" + env("NODE_NAME")
  }
}

// Read logs from disk
loki.source.file "pod_logs" {
  targets    = discovery.kubernetes.local_pods.targets
  forward_to = [loki.write.default.receiver]
}

// Collect host metrics
prometheus.exporter.unix "host" { }

prometheus.scrape "host_metrics" {
  targets    = prometheus.exporter.unix.host.targets
  forward_to = [prometheus.remote_write.default.receiver]
}
```

##### StatefulSet Configuration Example

```river
// Receive traces from applications
otelcol.receiver.otlp "default" {
  grpc {
    endpoint = "0.0.0.0:4317"
  }
  http {
    endpoint = "0.0.0.0:4318"
  }
  output {
    traces  = [otelcol.processor.k8sattributes.default.input]
  }
}

// Enrich with K8s metadata
otelcol.processor.k8sattributes "default" {
  extract {
    metadata = ["k8s.namespace.name", "k8s.pod.name", "k8s.deployment.name"]
  }
  output {
    traces = [otelcol.exporter.otlp.tempo.input]
  }
}

// Discover all application pods cluster-wide
discovery.kubernetes "app_pods" {
  role = "pod"
  namespaces {
    names = ["production", "staging"]
  }
}

// Scrape with clustering enabled
prometheus.scrape "apps" {
  targets      = discovery.kubernetes.app_pods.targets
  forward_to   = [prometheus.remote_write.default.receiver]
  clustering   {
    enabled = true
  }
}
```

---

#### Best Practices

1. **Use DaemonSet for infrastructure signals** - Logs and host metrics should always be collected locally for efficiency

2. **Use StatefulSet for application signals** - Traces and application metrics benefit from centralized processing and load balancing

3. **Enable k8sattributes processor** - Always enrich OTLP data with Kubernetes metadata in the StatefulSet

4. **Configure tail sampling wisely** - Keep 100% of errors, sample 1-5% of successful traces to control costs

5. **Use clustering for scraping** - Enable clustering in StatefulSet's `prometheus.scrape` to distribute load

6. **Set resource limits** - Use `otelcol.processor.memory_limiter` to prevent OOM issues

7. **Batch before sending** - Use `otelcol.processor.batch` to reduce network overhead

8. **Monitor Alloy itself** - Use `prometheus.exporter.self` to monitor Alloy's own metrics

---

Troubleshooting

**DaemonSet Issues:**

- Check that `NODE_NAME` environment variable is set for node-local discovery
- Verify that log paths match Kubernetes log locations (`/var/log/pods/`)
- Ensure proper volume mounts for accessing host filesystem

**StatefulSet Issues:**

- Verify network policies allow traffic to ports 4317/4318
- Check that the headless service is configured correctly
- Ensure clustering is working (check logs for cluster formation)
- Verify k8sattributes processor has RBAC permissions to query Kubernetes API

**General Issues:**

- Check Alloy logs: `kubectl logs -n monitoring <pod-name>`
- Verify component connections in the pipeline
- Use `otelcol.exporter.debug` temporarily to see what data flows through
- Check metrics endpoint: `http://pod-ip:12345/metrics`

#### The following labels are included for discovered Pods

- `__meta_kubernetes_namespace`: The namespace of the Pod object.
- _*`meta_kubernetes_pod_annotation`*: Each annotation from the Pod object.
- _*`meta_kubernetes_pod_annotationpresent`*: true for each annotation from the Pod object.
- `__meta_kubernetes_pod_container_id`: ID of the container the target address points to. The ID is in the form type://container_id.
- `__meta_kubernetes_pod_container_image`: The image the container is using.
- `__meta_kubernetes_pod_container_init`: true if the container is an InitContainer.
- `__meta_kubernetes_pod_container_name`: Name of the container the target address points to.
- `__meta_kubernetes_pod_container_port_name`: Name of the container port.
- `__meta_kubernetes_pod_container_port_number`: Number of the container port.
- `__meta_kubernetes_pod_container_port_protocol`: Protocol of the container port.
- `__meta_kubernetes_pod_controller_kind`: Object kind of the Pod controller.
- `__meta_kubernetes_pod_controller_name`: Name of the Pod controller.
- `__meta_kubernetes_pod_host_ip`: The current host IP of the Pod object.
- `__meta_kubernetes_pod_ip`: The Pod IP of the Pod object.
- _*`meta_kubernetes_pod_label`*: Each label from the Pod object.
- _*`meta_kubernetes_pod_labelpresent`*: true for each label from the Pod object.
- `__meta_kubernetes_pod_name`: The name of the Pod object.
- `__meta_kubernetes_pod_node_name`: The name of the node the Pod is scheduled onto.
- `__meta_kubernetes_pod_phase`: Set to Pending, Running, Succeeded, Failed or Unknown in the lifecycle.
- `__meta_kubernetes_pod_ready`: Set to true or false for the Pod’s ready state.
- `__meta_kubernetes_pod_uid`: The UID of the Pod object.
