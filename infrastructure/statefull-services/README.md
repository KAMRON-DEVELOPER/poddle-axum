# Deployment Guide

This guide covers deploying PostgreSQL, Redis, RabbitMQ, and Kafka services to your K3s cluster.

## Prerequisites

- K3s cluster running (master + agent nodes)
- `kubectl` configured to connect to your cluster
- Local storage provisioner enabled (k3s includes `local-path` by default)

## Setup Steps

### 1. Create Configuration ConfigMaps

These ConfigMaps contain service configuration files that will be mounted into the pods. The configuration files are created from local `.conf` files using `kubectl create configmap --from-file`.

**Create PostgreSQL ConfigMap:**

```bash
kubectl create namespace postgres-ns
kubectl create configmap postgres-conf -n postgres-ns \
  --from-file=postgresql.conf=configurations/postgresql/postgresql.local.conf \
  --from-file=pg_hba.conf=configurations/postgresql/pg_hba.local.conf
```

**Create Redis ConfigMap:**

```bash
kubectl create namespace redis-ns
kubectl create configmap redis-conf -n redis-ns \
  --from-file=redis.conf=configurations/redis-stack/redis-stack.local.conf
```

**Create RabbitMQ ConfigMap:**

```bash
kubectl create namespace rabbitmq-ns
kubectl create configmap rabbitmq-conf -n rabbitmq-ns \
  --from-file=rabbitmq.conf=configurations/rabbitmq/rabbitmq.local.conf
```

**Kafka ConfigMap Note:**

Kafka uses a ConfigMap defined in `kafka.yaml` for the cluster ID. No separate ConfigMap creation needed.

### 2. Deploy Services

Apply all deployment manifests:

```bash
kubectl apply -f postgres.yaml
kubectl apply -f redis.yaml
kubectl apply -f rabbitmq.yaml
kubectl apply -f kafka.yaml
```

Or apply all at once:

```bash
kubectl apply -f .
```

### 3. Verify Deployments

Check that all pods are running:

```bash
kubectl get pods -n postgres-ns
kubectl get pods -n redis-ns
kubectl get pods -n rabbitmq-ns
kubectl get pods -n kafka-ns
```

Check StatefulSets:

```bash
kubectl get statefulsets --all-namespaces
```

Check Services:

```bash
kubectl get svc --all-namespaces | grep -E "postgres|redis|rabbitmq|kafka"
```

## Accessing Services Locally

To access services from your local machine, use port forwarding.

**PostgreSQL Access:**

```bash
kubectl port-forward -n postgres-ns svc/postgres-service 5432:5432
```

Connection string: `postgresql://postgres:password@localhost:5432/cloud_service_db`

**Redis Access:**

```bash
kubectl port-forward -n redis-ns svc/redis-service 6379:6379
```

Connection string: `redis://default:password@localhost:6379`

**RabbitMQ Access:**

```bash
kubectl port-forward -n rabbitmq-ns svc/rabbitmq-service 5672:5672 15672:15672
```

- AMQP: `amqp://guest:password@localhost:5672`
- Management UI: `http://localhost:15672` (username: `guest`, password: `password`)

**Kafka Access:**

```bash
# Port-forward to a specific broker pod
kubectl port-forward -n kafka-ns kafka-ss-0 9092:9092
```

Bootstrap server: `localhost:9092`

**Note:** For external access to all Kafka brokers, you would need to port-forward to each broker separately or configure proper external listeners.

## Service Details

### PostgreSQL Service

- **Namespace:** `postgres-ns`
- **Service:** `postgres-service` (ClusterIP on port 5432)
- **Headless Service:** `postgres-hs`
- **StatefulSet:** `postgres-ss` (1 replica)
- **Image:** `postgres:bookworm`
- **Configuration:** Custom `postgresql.conf` and `pg_hba.conf` from ConfigMap `postgres-conf`
- **Credentials:**
  - User: `postgres` (from ConfigMap)
  - Password: `password` (from Secret)
  - Database: `cloud_service_db` (from ConfigMap)
- **Storage:** 1Gi PVC per pod (`local-path` storage class)
- **Resources:**
  - Requests: 500m CPU, 512Mi memory
  - Limits: 1 CPU, 1Gi memory

**Configuration Features:**

- Listens on all interfaces (`listen_addresses = '*'`)
- SCRAM-SHA-256 password encryption
- Max 100 connections
- 128MB shared buffers
- Trust authentication for local connections
- SCRAM-SHA-256 authentication for TCP connections

### Redis Service

- **Namespace:** `redis-ns`
- **Service:** `redis-service` (ClusterIP on port 6379)
- **Headless Service:** `redis-hs`
- **StatefulSet:** `redis-ss` (1 replica)
- **Image:** `redis/redis-stack-server:latest`
- **Configuration:** Custom `redis.conf` from ConfigMap `redis-conf`
- **Credentials:**
  - User: `default` (from ConfigMap)
  - Password: `password` (from Secret, applied via environment variable)
- **Storage:** 1Gi PVC per pod (`local-path` storage class)
- **Resources:**
  - Requests: 500m CPU, 512Mi memory
  - Limits: 1 CPU, 1Gi memory

**Configuration Features:**

- AOF (Append Only File) persistence enabled
- 4GB max memory with allkeys-lru eviction policy
- RediSearch and ReJSON modules loaded
- Protected mode enabled (requires password authentication)
- Automatic RDB snapshots:
  - After 3600s if at least 1 change
  - After 300s if at least 100 changes
  - After 60s if at least 10000 changes
- RDB compression enabled

**Health Checks:**

- Readiness probe: Redis CLI ping with authentication (every 10s)
- Liveness probe: Redis CLI ping with authentication (every 20s)

### RabbitMQ Service

- **Namespace:** `rabbitmq-ns`
- **Service:** `rabbitmq-service` (ClusterIP)
  - AMQP: port 5672
  - Management UI: port 15672
- **Headless Service:** `rabbitmq-hs`
- **StatefulSet:** `rabbitmq-ss` (1 replica, parallel pod management)
- **Image:** `rabbitmq:management-alpine`
- **Configuration:** Custom `rabbitmq.conf` from ConfigMap `rabbitmq-conf`
- **Credentials:**
  - User: `guest` (from ConfigMap)
  - Password: `password` (from Secret)
- **Storage:** 1Gi PVC per pod (`local-path` storage class)
- **Resources:**
  - Requests: 500m CPU, 512Mi memory
  - Limits: 1 CPU, 1Gi memory

**Configuration Features:**

- TCP listener on port 5672
- SHA-512 password hashing for enhanced security
- Management plugin enabled for web UI access

**Health Checks:**

- Readiness probe: RabbitMQ diagnostics ping (every 10s)

### Kafka Service

- **Namespace:** `kafka-ns`
- **Headless Service:** `kafka-hs` (with `publishNotReadyAddresses: true`)
  - Kafka: port 9092
  - Controller: port 9093
- **StatefulSet:** `kafka-ss` (3 replicas, parallel pod management)
- **Image:** `apache/kafka:latest`
- **Mode:** KRaft (no ZooKeeper required)
- **Cluster ID:** `MkU3OEVBNTcwNTJENDM2Qk` (from ConfigMap `kafka-cm`)
- **Storage:** 1Gi PVC per pod (`local-path` storage class)
- **Resources:**
  - Requests: 500m CPU, 512Mi memory
  - Limits: 1 CPU, 1Gi memory

**Cluster Configuration:**

- 3 brokers in KRaft mode (each pod is both broker and controller)
- Offsets topic replication factor: 3
- Transaction state log replication factor: 3
- Transaction state log min ISR: 2
- Quorum voters: All 3 brokers participate in controller quorum

**Pod Configuration:**

- Each pod gets a unique node ID from its hostname (0, 1, or 2)
- Advertised listeners use pod's DNS name within the cluster
- Storage formatted automatically on first start only
- Listeners: PLAINTEXT on 9092, CONTROLLER on 9093

**Broker DNS Names:**

- `kafka-ss-0.kafka-hs.kafka-ns.svc.cluster.local:9092`
- `kafka-ss-1.kafka-hs.kafka-ns.svc.cluster.local:9092`
- `kafka-ss-2.kafka-hs.kafka-ns.svc.cluster.local:9092`

## Updating Configurations

If you need to update configuration files after deployment:

**Update PostgreSQL Configuration:**

```bash
kubectl delete configmap postgres-conf -n postgres-ns
kubectl create configmap postgres-conf -n postgres-ns \
  --from-file=postgresql.conf=configurations/postgresql/postgresql.local.conf \
  --from-file=pg_hba.conf=configurations/postgresql/pg_hba.local.conf
kubectl rollout restart statefulset/postgres-ss -n postgres-ns
```

**Update Redis Configuration:**

```bash
kubectl delete configmap redis-conf -n redis-ns
kubectl create configmap redis-conf -n redis-ns \
  --from-file=redis.conf=configurations/redis-stack/redis-stack.local.conf
kubectl rollout restart statefulset/redis-ss -n redis-ns
```

**Update RabbitMQ Configuration:**

```bash
kubectl delete configmap rabbitmq-conf -n rabbitmq-ns
kubectl create configmap rabbitmq-conf -n rabbitmq-ns \
  --from-file=rabbitmq.conf=configurations/rabbitmq/rabbitmq.local.conf
kubectl rollout restart statefulset/rabbitmq-ss -n rabbitmq-ns
```

**Update Kafka Configuration:**

Kafka's ConfigMap is defined in `kafka.yaml`. To update environment variables, edit `kafka.yaml` and reapply:

```bash
kubectl apply -f kafka.yaml
kubectl rollout restart statefulset/kafka-ss -n kafka-ns
```

## Cleanup

**Remove all services:**

```bash
kubectl delete -f postgres.yaml
kubectl delete -f redis.yaml
kubectl delete -f rabbitmq.yaml
kubectl delete -f kafka.yaml
```

**Delete ConfigMaps:**

```bash
kubectl delete configmap postgres-conf -n postgres-ns
kubectl delete configmap redis-conf -n redis-ns
kubectl delete configmap rabbitmq-conf -n rabbitmq-ns
```

**Delete namespaces (removes all resources):**

```bash
kubectl delete namespace postgres-ns
kubectl delete namespace redis-ns
kubectl delete namespace rabbitmq-ns
kubectl delete namespace kafka-ns
```

## Troubleshooting

### Viewing Logs

```bash
kubectl logs -n <namespace> <pod-name>
# Follow logs in real-time
kubectl logs -n <namespace> <pod-name> -f
# View previous container logs (after restart)
kubectl logs -n <namespace> <pod-name> --previous
```

### Describing Resources

```bash
kubectl describe pod -n <namespace> <pod-name>
kubectl describe statefulset -n <namespace> <statefulset-name>
kubectl describe svc -n <namespace> <service-name>
```

### Executing Commands in Pods

**PostgreSQL:**

```bash
kubectl exec -it -n postgres-ns postgres-ss-0 -- psql -U postgres -d cloud_service_db
```

**Redis:**

```bash
# Connect with authentication
kubectl exec -it -n redis-ns redis-ss-0 -- redis-cli -u redis://default:password@127.0.0.1:6379
# Or using -a flag
kubectl exec -it -n redis-ns redis-ss-0 -- redis-cli -a password
```

**RabbitMQ:**

```bash
kubectl exec -it -n rabbitmq-ns rabbitmq-ss-0 -- rabbitmqctl status
```

**Kafka:**

```bash
# List topics
kubectl exec -it -n kafka-ns kafka-ss-0 -- \
  /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 --list
```

### Checking Persistent Volumes

```bash
kubectl get pv
kubectl get pvc --all-namespaces
```

### Kafka-Specific Commands

**Check cluster metadata:**

```bash
kubectl exec -it -n kafka-ns kafka-ss-0 -- \
  /opt/kafka/bin/kafka-metadata.sh --snapshot /var/lib/kafka/data/__cluster_metadata-0/00000000000000000000.log --print-records
```

**List topics:**

```bash
kubectl exec -it -n kafka-ns kafka-ss-0 -- \
  /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 --list
```

**Create a test topic:**

```bash
kubectl exec -it -n kafka-ns kafka-ss-0 -- \
  /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 \
  --create --topic test-topic --partitions 3 --replication-factor 3
```

**Describe a topic:**

```bash
kubectl exec -it -n kafka-ns kafka-ss-0 -- \
  /opt/kafka/bin/kafka-topics.sh --bootstrap-server localhost:9092 \
  --describe --topic test-topic
```

### Common Issues and Solutions

**Issue: Pods not starting**

- Check events: `kubectl describe pod -n <namespace> <pod-name>`
- Review logs: `kubectl logs -n <namespace> <pod-name>`

**Issue: Storage problems**

- Verify PVCs are bound: `kubectl get pvc -n <namespace>`
- Check PV status: `kubectl get pv`

**Issue: ConfigMap not found**

- Ensure ConfigMaps are created before applying deployment manifests
- Verify ConfigMap exists: `kubectl get configmap -n <namespace>`

**Issue: Kafka brokers not forming quorum**

- Check all 3 pods are running: `kubectl get pods -n kafka-ns`
- Verify network connectivity via headless service
- Check logs for quorum-related errors

**Issue: Redis authentication failures**

- Ensure `REDIS_PASSWORD` environment variable is set correctly
- Verify `protected-mode` is set to `yes` in redis.conf
- Check that probes use authentication (`-u` or `-a` flag)

**Issue: RabbitMQ not starting**

- Check if ConfigMap is mounted correctly
- Verify configuration syntax in `rabbitmq.conf`
- Review RabbitMQ logs for configuration errors

## Production Considerations

For production deployments, consider the following:

1. **Security:**
   - Change all default passwords in Secrets
   - Enable TLS/SSL for all service connections
   - Use proper authentication mechanisms
   - Implement network policies to restrict pod-to-pod communication

2. **Storage:**
   - Use production-grade storage classes (e.g., Ceph, Longhorn, or cloud provider storage)
   - Increase storage capacity based on expected data volume
   - Implement backup and disaster recovery strategies

3. **Resource Allocation:**
   - Adjust CPU and memory limits based on actual workload
   - Monitor resource usage and scale accordingly
   - Set appropriate resource requests for guaranteed QoS

4. **High Availability:**
   - Increase replicas for PostgreSQL (using streaming replication)
   - Deploy Redis in cluster or sentinel mode for HA
   - Scale RabbitMQ to multiple nodes with mirrored queues
   - Kafka already has 3 replicas, ensure proper replication factor for topics

5. **Monitoring and Observability:**
   - Deploy Prometheus exporters for all services
   - Set up Grafana dashboards for visualization
   - Configure alerting for critical metrics
   - Implement distributed tracing

6. **Networking:**
   - Implement NetworkPolicies to control traffic
   - Consider using service mesh (e.g., Istio, Linkerd) for advanced networking features
   - Configure proper DNS and service discovery

7. **Backup Strategy:**
   - Automate PostgreSQL backups using tools like pgBackRest or WAL-G
   - Implement Redis RDB/AOF backup automation
   - Back up RabbitMQ definitions and message persistence
   - Use Kafka MirrorMaker or Kafka Connect for data replication

8. **Configuration Management:**
   - Store sensitive configuration in external secret management (e.g., HashiCorp Vault, AWS Secrets Manager)
   - Use GitOps practices for deployment automation
   - Version control all configuration files

9. **Logging:**
   - Centralize logs using ELK stack or Loki
   - Set up log rotation and retention policies
   - Enable audit logging for security compliance

10. **Testing:**
    - Perform load testing to validate performance
    - Test failover scenarios
    - Conduct disaster recovery drills
