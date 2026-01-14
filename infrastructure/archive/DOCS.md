# 🏗️ Poddle PaaS - Architecture & Design Document

## 📋 Table of Contents

1. [System Overview](#system-overview)
2. [Functional Requirements](#functional-requirements)
3. [Non-Functional Requirements](#non-functional-requirements)
4. [Architecture Design](#architecture-design)
5. [Service Breakdown](#service-breakdown)
6. [Critical Design Decisions](#critical-design-decisions)
7. [Edge Cases & Solutions](#edge-cases-solutions)
8. [Security Considerations](#security-considerations)
9. [Scaling Strategy](#scaling-strategy)
10. [Cost Management](#cost-management)

---

## 1. System Overview

**Poddle** is a Platform-as-a-Service (PaaS) targeting the Uzbekistan and Central Asia markets, allowing users to deploy containerized applications with zero infrastructure management.

**Target Market**: Small-to-medium businesses, startups, developers in Uzbekistan/Central Asia
**Initial Infrastructure**: 3-10 colocation servers in Tashkent
**Core Value Proposition**: Simple deployment, local data residency, competitive pricing

---

## 2. Functional Requirements

### Core Features

- ✅ **User Management**: Registration, authentication (email/OAuth), profile management
- ✅ **Project Organization**: Users can create projects containing multiple deployments
- ✅ **Container Deployment**: Deploy any Docker image with custom configuration
- ✅ **Environment Variables & Secrets**: Secure configuration management
- ✅ **Scaling**: Manual horizontal scaling (replicas)
- ✅ **Custom Domains**: User-provided domain mapping with SSL
- ✅ **Real-time Monitoring**: CPU, memory, network metrics via SSE/WebSocket
- ✅ **Logs Streaming**: Live application logs
- ✅ **Billing**: Pay-per-use model with balance management

### Phase 2 Features (Future)

- 🔄 **Auto-scaling**: Based on CPU/memory thresholds
- 🔄 **CI/CD Integration**: GitHub/GitLab webhooks for auto-deploy
- 🔄 **Managed Databases**: PostgreSQL, Redis, MongoDB as add-ons
- 🔄 **Cron Jobs**: Scheduled task execution
- 🔄 **Team Collaboration**: Share projects with team members

---

## 3. Non-Functional Requirements

| Requirement          | Target                   | Priority |
| -------------------- | ------------------------ | -------- |
| **Availability**     | 99.5% uptime             | High     |
| **Response Time**    | API < 200ms (p95)        | High     |
| **Deployment Time**  | < 2 minutes              | Medium   |
| **Concurrent Users** | 500-1000                 | Medium   |
| **Data Residency**   | All data in Uzbekistan   | Critical |
| **Security**         | SOC 2 Type II (future)   | High     |
| **Scalability**      | 10,000 deployments       | Medium   |
| **Cost Efficiency**  | 60% resource utilization | High     |

---

## 4. Architecture Design

### 4.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         CLIENT LAYER                         │
│  (Web App, CLI, Mobile App)                                 │
└─────────────────┬───────────────────────────────────────────┘
                  │ HTTPS
┌─────────────────▼───────────────────────────────────────────┐
│                      API GATEWAY / LB                        │
│              (Traefik / Nginx with rate limiting)           │
└─────┬──────────┬──────────┬─────────────┬──────────────────┘
      │          │          │             │
┌─────▼──────┐ ┌▼──────┐ ┌─▼────────┐ ┌──▼────────┐
│   Users    │ │Billing│ │ Compute  │ │Monitoring │
│  Service   │ │Service│ │ Service  │ │ Service   │
└─────┬──────┘ └┬──────┘ └─┬────────┘ └──┬────────┘
      │         │          │             │
      └─────────┴──────────┴─────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│                    MESSAGE BROKER LAYER                      │
│        RabbitMQ (deployment events, billing, logs)          │
└─────┬──────────┬──────────┬─────────────┬──────────────────┘
      │          │          │             │
┌─────▼──────┐ ┌▼──────┐ ┌─▼────────┐ ┌──▼────────┐
│ Compute-   │ │Compute│ │ Compute- │ │  Billing  │
│ Deployer   │ │Watcher│ │ Informer │ │  Worker   │
└─────┬──────┘ └┬──────┘ └─┬────────┘ └──┬────────┘
      │         │          │             │
      └─────────┴──────────┴─────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│                   KUBERNETES CLUSTER(S)                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Namespace: user-abc123  (per-user isolation)        │  │
│  │  - Deployments (user containers)                     │  │
│  │  - Services (ClusterIP)                              │  │
│  │  - Ingress (Traefik with cert-manager)              │  │
│  │  - Secrets (env vars)                                │  │
│  │  - Network Policies (traffic isolation)             │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│                    DATA LAYER                                │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │PostgreSQL│  │  Redis   │  │  S3/Minio│  │Prometheus│   │
│  │  (Main)  │  │ (Cache)  │  │  (Logs)  │  │ (Metrics)│   │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 Event-Driven Flow for Deployments

```
User creates deployment
    │
    ▼
[Compute API]
    │
    ├─► Validate request
    │
    ├─► Create DB record (status: pending)
    │
    ├─► Publish to RabbitMQ: deployment.create
    │
    └─► Return 202 Accepted to user
         │
         ▼
    [User polls/streams status via SSE/WebSocket]

────────────────────────────────────────────────

[Compute-Deployer] (background worker)
    │
    ├─► Subscribe to: deployment.create
    │
    ├─► Create Kubernetes resources:
    │       - Namespace (if not exists)
    │       - Secret (env vars)
    │       - Deployment
    │       - Service
    │       - Ingress
    │
    ├─► Update DB: status → running
    │
    ├─► Publish to RabbitMQ: deployment.created
    │
    └─► If error:
            - Update DB: status → failed
            - Publish: deployment.failed
            - Send notification

────────────────────────────────────────────────

[Compute-Watcher] (background worker)
    │
    ├─► Watch Kubernetes events (pod crashes, OOM, etc.)
    │
    ├─► On pod failure:
    │       - Log event to DB
    │       - Update deployment status
    │       - Publish: deployment.unhealthy
    │       - Send alert to user
    │
    └─► On pod restart:
            - Log event
            - Track restart count

────────────────────────────────────────────────

[Compute-Informer] (background worker)
    │
    ├─► Subscribe to Kubernetes Informer API
    │
    ├─► On resource changes:
    │       - Cache state in Redis
    │       - Broadcast to WebSocket clients
    │
    └─► Maintain real-time deployment state
```

---

## 5. Service Breakdown

### 5.1 **users** (API Service)

**Responsibility**: Authentication, user management, sessions

**Endpoints**:

- `POST /auth/register` - User registration
- `POST /auth/login` - Email/password login
- `POST /auth/oauth/{provider}` - OAuth login (Google/GitHub)
- `POST /auth/refresh` - Refresh access token
- `POST /auth/logout` - Invalidate session
- `GET /users/me` - Get current user
- `PATCH /users/me` - Update profile
- `GET /users/me/sessions` - List active sessions
- `DELETE /users/me/sessions/{id}` - Revoke session

**Database Tables**: `users`, `oauth_users`, `sessions`

**Critical Considerations**:

- ⚠️ **Rate Limiting**: 5 login attempts per 15 min per IP
- ⚠️ **Session Management**: Redis for session storage (fast lookup)
- ⚠️ **Email Verification**: Required before deployment access
- ⚠️ **MFA**: Add TOTP support for high-value accounts

---

### 5.2 **billing** (API Service)

**Responsibility**: Balance management, transactions, invoicing

**Endpoints**:

- `GET /billing/balance` - Get current balance
- `POST /billing/fund` - Add funds (payment integration)
- `GET /billing/transactions` - Transaction history
- `GET /billing/invoices` - Monthly invoices
- `GET /billing/usage` - Current month usage breakdown

**Database Tables**: `balances`, `transactions`, `billings`

**Critical Considerations**:

- ⚠️ **Atomic Transactions**: MUST use DB transactions to prevent race conditions
- ⚠️ **Negative Balance Prevention**: Trigger checks before allowing new deployments
- ⚠️ **Billing Accuracy**: Record usage hourly (not real-time) to reduce DB load
- ⚠️ **Payment Gateway**: Integrate with local payment providers (Payme, Click, Uzum)

**Billing Logic**:

```sql
-- Example: Hourly billing job
INSERT INTO billings (user_id, deployment_id, resources_snapshot,
                      cpu_millicores, memory_mb, cost_per_hour, hours_used)
SELECT
    d.user_id,
    d.id,
    d.resources,
    (d.resources->>'cpu_limit_millicores')::int,
    (d.resources->>'memory_limit_mb')::int,
    calculate_cost(d.resources),
    1.0
FROM deployments d
WHERE d.status = 'running';

-- This automatically triggers transaction via DB trigger
```

---

### 5.3 **compute** (API Service)

**Responsibility**: Entry point for compute operations, validation

**Endpoints**:

- `GET /projects` - List user projects
- `POST /projects` - Create project
- `GET /projects/{id}/deployments` - List deployments
- `POST /projects/{id}/deployments` - Create deployment (async)
- `GET /deployments/{id}` - Get deployment details
- `PATCH /deployments/{id}/scale` - Scale replicas
- `GET /deployments/{id}/logs` - Stream logs (SSE)
- `GET /deployments/{id}/metrics` - Real-time metrics (SSE)
- `POST /deployments/{id}/restart` - Restart deployment
- `DELETE /deployments/{id}` - Delete deployment

**Critical Considerations**:

- ⚠️ **Async Operations**: NEVER create K8s resources in HTTP handlers (use RabbitMQ)
- ⚠️ **Idempotency**: Use `deployment.cluster_deployment_name` as idempotency key
- ⚠️ **Validation**: Strict validation on image names, resource limits, subdomain format
- ⚠️ **User Quotas**: Check user limits before accepting deployment requests

**Current Code Issues**:

```rust
// ❌ PROBLEM: Creating K8s resources synchronously
pub async fn create_deployment(...) {
    let deployment = create_db_record(); // OK
    create_k8s_resources().await; // ❌ BLOCKS HTTP REQUEST
    return deployment;
}

// ✅ SOLUTION: Publish to message queue
pub async fn create_deployment(...) {
    let deployment = create_db_record(status: Pending); // OK
    publish_to_rabbitmq("deployment.create", deployment.id); // Async
    return deployment; // Return immediately
}
```

---

### 5.4 **compute-deployer** (Background Worker)

**Responsibility**: Create/update Kubernetes resources

**RabbitMQ Subscriptions**:

- `deployment.create` → Create all K8s resources
- `deployment.update` → Update existing resources
- `deployment.scale` → Patch replica count
- `deployment.delete` → Clean up K8s resources

**Logic**:

```rust
async fn handle_deployment_create(deployment_id: Uuid) {
    // 1. Load deployment from DB
    let deployment = load_deployment(deployment_id)?;

    // 2. Check user balance (prevent deploys with $0 balance)
    let balance = check_balance(deployment.user_id)?;
    if balance < MIN_BALANCE {
        update_status(deployment_id, Failed);
        send_notification("Insufficient balance");
        return;
    }

    // 3. Create K8s namespace (if needed)
    create_namespace(deployment.user_id)?;

    // 4. Create resources (with retries)
    with_retries(3, || {
        create_secret()?;
        create_deployment()?;
        create_service()?;
        create_ingress()?;
    })?;

    // 5. Wait for deployment to be ready (timeout: 5 min)
    wait_for_ready(deployment_id, timeout: 300)?;

    // 6. Update DB status
    update_status(deployment_id, Running);

    // 7. Publish success event
    publish("deployment.created", deployment_id);

    // 8. Start billing
    publish("billing.start", deployment_id);
}
```

**Critical Considerations**:

- ⚠️ **Idempotency**: Must handle duplicate messages (check if K8s resource exists)
- ⚠️ **Rollback**: If any step fails, clean up partial resources
- ⚠️ **Dead Letter Queue**: Move failed messages to DLQ after 3 retries
- ⚠️ **Resource Validation**: Verify user hasn't exceeded quotas

---

### 5.5 **compute-watcher** (Background Worker)

**Responsibility**: Monitor deployment health, handle failures

**Kubernetes Watch Targets**:

- Pod events (OOMKilled, CrashLoopBackOff, ImagePullBackOff)
- Deployment events (rollout status)
- Ingress events (certificate issues)

**Logic**:

```rust
async fn watch_pod_events() {
    let pods: Api<Pod> = Api::all(client);
    let mut stream = pods.watch(&ListParams::default()).await?;

    while let Some(event) = stream.try_next().await? {
        match event {
            Event::Modified(pod) => {
                // Check for OOMKilled
                if pod.status.container_statuses.any(|c| c.state.terminated.reason == "OOMKilled") {
                    handle_oom_killed(pod);
                }

                // Check for excessive restarts
                if pod.status.container_statuses.any(|c| c.restart_count > 10) {
                    handle_excessive_restarts(pod);
                }
            },
            Event::Deleted(pod) => {
                // Unexpected deletion
                handle_unexpected_deletion(pod);
            },
            _ => {}
        }
    }
}

async fn handle_oom_killed(pod: Pod) {
    let deployment_id = pod.metadata.labels.get("deployment-id");

    // 1. Log event
    log_event(deployment_id, "oom_killed", "Container ran out of memory");

    // 2. Update status
    update_status(deployment_id, Unhealthy);

    // 3. Send notification
    send_notification(deployment_id, "Your app is crashing due to OOM. Consider increasing memory limits.");

    // 4. Auto-remediation (optional)
    // suggest_scale_up(deployment_id);
}
```

**Critical Considerations**:

- ⚠️ **Event Deduplication**: K8s sends duplicate events, dedupe in Redis
- ⚠️ **Alert Throttling**: Don't spam users with alerts (max 1 per 5 min per deployment)
- ⚠️ **Auto-remediation**: Consider auto-restarting stuck deployments (with limits)

---

### 5.6 **compute-informer** (Background Worker)

**Responsibility**: Cache K8s state, broadcast real-time updates

**Purpose**: Reduce K8s API load by maintaining cached state

**Logic**:

```rust
async fn start_deployment_informer() {
    let deployments: Api<K8sDeployment> = Api::all(client);
    let informer = Informer::new(deployments);

    loop {
        for event in informer.poll().await? {
            match event {
                Event::Applied(deployment) => {
                    // Update Redis cache
                    cache_deployment_state(deployment);

                    // Broadcast to WebSocket clients
                    broadcast_to_subscribers(deployment.id, deployment.status);
                },
                Event::Deleted(deployment) => {
                    remove_from_cache(deployment.id);
                    broadcast_deletion(deployment.id);
                },
                _ => {}
            }
        }
    }
}

// WebSocket handler (in compute service)
async fn websocket_handler(ws: WebSocket, deployment_id: Uuid) {
    // Subscribe to Redis pub/sub
    let mut subscriber = redis.subscribe(&format!("deployment:{}", deployment_id));

    while let Some(msg) = subscriber.next().await {
        ws.send(msg).await;
    }
}
```

**Critical Considerations**:

- ⚠️ **Connection Management**: Limit WebSocket connections per user (max 50)
- ⚠️ **Backpressure**: Use Redis Streams instead of pub/sub for better reliability
- ⚠️ **Reconnection**: Clients should reconnect with exponential backoff

---

### 5.7 **monitoring** (Service)

**Responsibility**: Metrics, logs, alerts

**Components**:

- **Prometheus**: Scrape K8s metrics, custom application metrics
- **Loki**: Log aggregation (from all deployments)
- **Grafana**: Dashboards for ops team
- **Alertmanager**: Send alerts via Telegram/Email

**User-facing Endpoints** (in compute service):

- `GET /deployments/{id}/metrics/cpu` - CPU usage over time
- `GET /deployments/{id}/metrics/memory` - Memory usage over time
- `GET /deployments/{id}/logs?tail=100` - Recent logs
- `GET /deployments/{id}/logs/stream` - Live log streaming (SSE)

**Critical Considerations**:

- ⚠️ **Log Retention**: 7 days for free tier, 30 days for paid
- ⚠️ **Log Size Limits**: Max 10MB/day per deployment (prevent abuse)
- ⚠️ **Metrics Storage**: Use Prometheus remote write to S3 for long-term storage

---

## 6. Critical Design Decisions

### 6.1 **Multi-Tenancy Strategy**

**Decision**: Namespace-per-user isolation

**Rationale**:

- ✅ Simple RBAC (users can't access other namespaces)
- ✅ Resource quotas per user
- ✅ Network policies for isolation
- ✅ Easy cost attribution (label metrics by namespace)
- ❌ Limited to ~1000 namespaces per cluster (K8s limit)

**Alternative (not chosen)**: Label-based isolation

- ❌ More complex RBAC
- ❌ Risk of misconfiguration leaking data
- ✅ Better scalability (unlimited users per cluster)

**Implementation**:

```yaml
# Namespace with quotas
apiVersion: v1
kind: Namespace
metadata:
  name: user-abc123
  labels:
    user-id: abc123
---
apiVersion: v1
kind: ResourceQuota
metadata:
  name: user-quota
  namespace: user-abc123
spec:
  hard:
    requests.cpu: "4"
    requests.memory: 8Gi
    limits.cpu: "8"
    limits.memory: 16Gi
    persistentvolumeclaims: "10"
---
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: deny-cross-namespace
  namespace: user-abc123
spec:
  podSelector: {}
  policyTypes:
    - Ingress
    - Egress
  ingress:
    - from:
        - podSelector: {} # Only from same namespace
  egress:
    - to:
        - namespaceSelector: {} # Allow egress to all (for external APIs)
```

---

### 6.2 **Secrets Management**

**Current Issue**: Your code has `deployment_secrets` table but stores secrets in K8s

**Decision**: **K8s Secrets ONLY** (remove DB table)

**Rationale**:

- ✅ K8s Secrets are already encrypted at rest (if using etcd encryption)
- ✅ Avoid secret duplication
- ✅ Simpler lifecycle management
- ❌ Secrets lost if K8s cluster is destroyed

**Alternative (for future)**: External Secrets Operator + HashiCorp Vault

- ✅ Centralized secret management
- ✅ Secret rotation
- ✅ Audit logs
- ❌ Additional complexity

**Implementation**:

```rust
// ❌ DELETE this table from migrations
// CREATE TABLE deployment_secrets ...

// ✅ Store ONLY in K8s
async fn create_secret(namespace: &str, name: &str, data: HashMap<String, String>) {
    let secret = Secret {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        string_data: Some(data),  // K8s handles base64 encoding
        ..Default::default()
    };

    secrets_api.create(&PostParams::default(), &secret).await?;
}

// When user wants to view secrets, fetch from K8s
async fn get_secret_keys(deployment_id: Uuid) -> Vec<String> {
    let deployment = get_deployment(deployment_id)?;
    let secret_name = format!("{}-secrets", deployment.name);

    let secret = secrets_api.get(&secret_name).await?;
    Ok(secret.data.unwrap_or_default().keys().cloned().collect())
}
```

---

### 6.3 **Billing Model**

**Pricing Structure** (example):
| Resource | Price (UZS/hour) |
|----------|------------------|
| 250m CPU | 100 |
| 100MB Memory | 50 |
| 1 Replica | Base fee 200 |

**Calculation**:

```rust
fn calculate_hourly_cost(resources: &ResourceSpec, replicas: i32) -> Decimal {
    let cpu_cost = (resources.cpu_limit_millicores / 250) * 100;
    let mem_cost = (resources.memory_limit_mb / 100) * 50;
    let base_fee = replicas * 200;

    Decimal::from(cpu_cost + mem_cost + base_fee)
}
```

**Billing Worker** (runs every hour):

```rust
async fn hourly_billing_job(pool: &PgPool) {
    let deployments = sqlx::query!(
        "SELECT * FROM deployments WHERE status = 'running'"
    ).fetch_all(pool).await?;

    for deployment in deployments {
        let cost = calculate_hourly_cost(&deployment.resources, deployment.replicas);

        // Create billing record (triggers balance deduction via DB trigger)
        sqlx::query!(
            "INSERT INTO billings (user_id, deployment_id, resources_snapshot,
                                   cpu_millicores, memory_mb, cost_per_hour, hours_used)
             VALUES ($1, $2, $3, $4, $5, $6, 1.0)",
            deployment.user_id,
            deployment.id,
            deployment.resources,
            extract_cpu(&deployment.resources),
            extract_memory(&deployment.resources),
            cost
        ).execute(pool).await?;
    }
}
```

**Automatic Suspension**:

```rust
async fn check_negative_balances(pool: &PgPool) {
    let users = sqlx::query!(
        "SELECT user_id FROM balances WHERE amount < 0"
    ).fetch_all(pool).await?;

    for user in users {
        // Stop all deployments
        sqlx::query!(
            "UPDATE deployments SET status = 'terminated'
             WHERE user_id = $1 AND status = 'running'",
            user.user_id
        ).execute(pool).await?;

        // Publish events to delete K8s resources
        publish("deployment.terminate_all", user.user_id);

        // Send notification
        send_email(user.user_id, "Your deployments were stopped due to insufficient balance");
    }
}
```

---

### 6.4 **Image Registry Strategy**

**Options**:

1. **Allow any public registry** (Docker Hub, ghcr.io, etc.)
   - ✅ User flexibility
   - ❌ Security risk (malicious images)
   - ❌ Rate limits from registries

2. **Force use of private registry** (Harbor, GitLab Registry)
   - ✅ Image scanning for vulnerabilities
   - ✅ Caching (faster pulls)
   - ❌ Users must push images to your registry first

**Recommendation**: Hybrid approach

- Phase 1: Allow any public registry with **image scanning**
- Phase 2: Offer managed registry as premium feature

**Image Scanning Integration**:

```rust
async fn validate_image(image: &str) -> Result<(), AppError> {
    // 1. Check if image exists
    let client = DockerRegistryClient::new();
    client.get_manifest(image).await?;

    // 2. Scan with Trivy (open-source scanner)
    let scan_result = trivy::scan_image(image).await?;

    // 3. Check for critical vulnerabilities
    if scan_result.critical_count > 0 {
        return Err(AppError::BadRequest(
            format!("Image has {} critical vulnerabilities. Please fix before deploying.",
                    scan_result.critical_count)
        ));
    }

    Ok(())
}
```

---

## 7. Edge Cases & Solutions

### 7.1 **Deployment Stuck in Pending**

**Scenario**: User creates deployment, status stays "pending" for >10 minutes

**Causes**:

1. compute-deployer worker is down
2. Image pull failed (invalid image name, auth issues)
3. Insufficient cluster resources
4. Network policy blocking

**Solution**:

```rust
// Timeout job (runs every 5 minutes)
async fn check_stuck_deployments(pool: &PgPool) {
    let stuck = sqlx::query!(
        "SELECT * FROM deployments
         WHERE status = 'pending'
         AND created_at < NOW() - INTERVAL '10 minutes'"
    ).fetch_all(pool).await?;

    for deployment in stuck {
        // Check K8s events
        let events = get_k8s_events(deployment.id).await?;

        if events.contains("ImagePullBackOff") {
            update_status(deployment.id, Failed);
            log_event(deployment.id, "image_pull_failed",
                     "Failed to pull image. Check image name and credentials.");
        } else if events.contains("Insufficient") {
            update_status(deployment.id, Failed);
            log_event(deployment.id, "insufficient_resources",
                     "Cluster has insufficient resources. Please try again later.");
        } else {
            // Retry deployment
            publish("deployment.create", deployment.id);
        }
    }
}
```

---

### 7.2 **User Exhausts Resources (DoS)**

**Scenario**: Malicious user creates 100 deployments with max resources

**Prevention**:

1. **User Quotas** (enforced at API level)

```rust
struct UserLimits {
    max_projects: usize,
    max_deployments_per_project: usize,
    max_total_cpu: i32,
    max_total_memory: i32,
    max_replicas_per_deployment: i32,
}

impl UserLimits {
    fn for_tier(tier: &str) -> Self {
        match tier {
            "free" => Self {
                max_projects: 3,
                max_deployments_per_project: 5,
                max_total_cpu: 2000,  // 2 CPUs total
                max_total_memory: 2048,  // 2GB total
                max_replicas_per_deployment: 3,
            },
            "pro" => Self {
                max_projects: 10,
                max_deployments_per_project: 20,
                max_total_cpu: 16000,  // 16 CPUs
                max_total_memory: 32768,  // 32GB
                max_replicas_per_deployment: 10,
            },
            _ => Self::default(),
        }
    }
}

async fn check_user_limits(user_id: Uuid, req: &CreateDeploymentRequest) -> Result<(), AppError> {
    let limits = UserLimits::for_tier(&get_user_tier(user_id));

    // Check total resource usage
    let current_usage = sqlx::query!(
        "SELECT
            SUM((resources->>'cpu_limit_millicores')::int) as total_cpu,
            SUM((resources->>'memory_limit_mb')::int) as total_memory
         FROM deployments
         WHERE user_id = $1 AND status = 'running'",
        user_id
    ).fetch_one(pool).await?;

    let new_cpu = current_usage.total_cpu + req.resources.cpu_limit_millicores;
    let new_memory = current_usage.total_memory + req.resources.memory_limit_mb;

    if new_cpu > limits.max_total_cpu {
        return Err(AppError::QuotaExceeded("CPU limit exceeded"));
    }

    if new_memory > limits.max_total_memory {
        return Err(AppError::QuotaExceeded("Memory limit exceeded"));
    }

    Ok(())
}
```

2. **Rate Limiting** (API gateway level)

```yaml
# Traefik rate limit middleware
apiVersion: traefik.containo.us/v1alpha1
kind: Middleware
metadata:
  name: rate-limit
spec:
  rateLimit:
    average: 10 # 10 requests per second
    burst: 20
    period: 1s
```

---

### 7.3 **Certificate Provisioning Fails**

**Scenario**: cert-manager fails to provision Let's Encrypt certificate

**Causes**:

1. DNS not propagated yet
2. Rate limit hit (Let's Encrypt: 50 certs/week)
3. Firewall blocking ACME challenge

**Solution**:

```rust
async fn check_certificate_status(deployment_id: Uuid) {
    let ingress = get_ingress(deployment_id).await?;
    let cert_name = format!("{}-tls", ingress.name);

    let cert: Api<Certificate> = Api::namespaced(client, &namespace);
    let certificate = cert.get(&cert_name).await?;

    if let Some(status) = certificate.status {
        for condition in status.conditions {
            if condition.type_ == "Ready" && condition.status == "False" {
                match condition.reason.as_str() {
                    "Pending" => {
                        // Wait for DNS propagation
                        log_event(deployment_id, "cert_pending",
                                 "Waiting for DNS propagation...");
                    },
                    "Failed" => {
                        // Try HTTP-01 challenge instead of DNS-01
                        recreate_certificate_with_http_challenge(cert_name).await?;
                    },
                    _ => {}
                }
            }
        }
    }
}
```

---

### 7.4 **Database Connection Pool Exhaustion**

**Scenario**: High traffic causes "too many connections" error

**Solution**:

```rust
// shared/src/services/database.rs
pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)  // ⚠️ Tune based on load
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .idle_timeout(Some(Duration::from_secs(600)))
        .max_lifetime(Some(Duration::from_secs(1800)))
        // ⚠️ CRITICAL: Use pgbouncer for connection pooling
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                conn.execute("SET application_name = 'poddle'").await?;
                Ok(())
            })
        })
        .connect(&config.url).await
}
```

**PgBouncer Configuration**:

```ini
[databases]
poddle = host=postgres port=5432 dbname=poddle

[pgbouncer]
listen_port = 6432
listen_addr = *
auth_type = md5
pool_mode = transaction  # Better for API workloads
max_client_conn = 1000
default_pool_size = 25
reserve_pool_size = 5
reserve_pool_timeout = 3
```

---

### 7.5 **Kubernetes API Overload**

**Scenario**: Frequent K8s API calls cause rate limiting (429 errors)

**Solution**: Implement caching via `compute-informer`

```rust
// Cache deployment state in Redis
#[derive(Serialize, Deserialize)]
struct CachedDeploymentState {
    replicas: i32,
    ready_replicas: i32,
    status: String,
    last_updated: DateTime<Utc>,
}

async fn get_deployment_state(deployment_id: Uuid) -> Result<CachedDeploymentState, AppError> {
    // Try cache first
    let cache_key = format!("deployment:{}:state", deployment_id);

    if let Some(cached) = redis.get::<String>(&cache_key).await? {
        return Ok(serde_json::from_str(&cached)?);
    }

    // Cache miss - fetch from K8s
    let state = fetch_from_k8s(deployment_id).await?;

    // Cache for 30 seconds
    redis.setex(&cache_key, 30, &serde_json::to_string(&state)?).await?;

    Ok(state)
}
```

---

## 8. Security Considerations

### 8.1 **Container Security**

**Requirements**:

1. **No privileged containers**
2. **Read-only root filesystem** (where possible)
3. **Drop all capabilities**
4. **Run as non-root user**

**Enforcement** (via PodSecurityPolicy or OPA):

```yaml
apiVersion: policy/v1beta1
kind: PodSecurityPolicy
metadata:
  name: restricted
spec:
  privileged: false
  allowPrivilegeEscalation: false
  requiredDropCapabilities:
    - ALL
  volumes:
    - "configMap"
    - "emptyDir"
    - "projected"
    - "secret"
    - "downwardAPI"
    - "persistentVolumeClaim"
  runAsUser:
    rule: "MustRunAsNonRoot"
  seLinux:
    rule: "RunAsAny"
  fsGroup:
    rule: "RunAsAny"
```

---

### 8.2 **Network Security**

**Requirements**:

1. Deployments can't access each other across namespaces
2. Deployments can access external APIs (egress allowed)
3. Only ingress controller can reach deployments

**Implementation**: See NetworkPolicy in section 6.1

---

### 8.3 **Secrets Security**

**Requirements**:

1. Secrets encrypted at rest in etcd
2. Secrets transmitted over TLS only
3. Secrets never logged or exposed in API responses

**Kubernetes etcd Encryption**:

```yaml
# /etc/kubernetes/enc/encryption-config.yaml
apiVersion: apiserver.config.k8s.io/v1
kind: EncryptionConfiguration
resources:
  - resources:
      - secrets
    providers:
      - aescbc:
          keys:
            - name: key1
              secret: <base64-encoded-32-byte-key>
      - identity: {}
```

**API Response Filtering**:

```rust
// ❌ NEVER return secret values
pub struct DeploymentDetailResponse {
    pub secret_keys: Vec<String>,  // ✅ Only return keys
    // ❌ pub secrets: HashMap<String, String>,
}
```

---

### 8.4 **RBAC (Kubernetes)**

**Service Account Permissions**:

```yaml
# compute-deployer service account
apiVersion: v1
kind: ServiceAccount
metadata:
  name: compute-deployer
  namespace: poddle-system
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: compute-deployer
rules:
  - apiGroups: [""]
    resources: ["namespaces"]
    verbs: ["get", "list", "create"]
  - apiGroups: ["apps"]
    resources: ["deployments"]
    verbs: ["get", "list", "create", "update", "patch", "delete"]
  - apiGroups: [""]
    resources: ["services", "secrets"]
    verbs: ["get", "list", "create", "update", "delete"]
  - apiGroups: ["networking.k8s.io"]
    resources: ["ingresses"]
    verbs: ["get", "list", "create", "update", "delete"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: compute-deployer
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: compute-deployer
subjects:
  - kind: ServiceAccount
    name: compute-deployer
    namespace: poddle-system
```

---

## 9. Scaling Strategy

### 9.1 **Phase 1: Single Cluster (Months 1-6)**

- 3-10 servers (1 master, 2-9 workers)
- Target: 500-1000 deployments
- Focus: Feature development, user acquisition

### 9.2 **Phase 2: Multi-Cluster (Months 6-12)**

- Add second K8s cluster for redundancy
- Implement cluster selection logic (least loaded)
- Database replication (master-slave)

```rust
enum ClusterRegion {
    TashkentPrimary,
    TashkentSecondary,
}

async fn select_cluster(user_id: Uuid) -> ClusterRegion {
    let primary_load = get_cluster_load(ClusterRegion::TashkentPrimary).await;
    let secondary_load = get_cluster_load(ClusterRegion::TashkentSecondary).await;

    if primary_load.cpu_usage < 0.7 && primary_load.memory_usage < 0.8 {
        ClusterRegion::TashkentPrimary
    } else {
        ClusterRegion::TashkentSecondary
    }
}
```

### 9.3 **Phase 3: Regional Expansion (Year 2)**

- Deploy in Almaty (Kazakhstan), Bishkek (Kyrgyzstan)
- Geo-based routing (lowest latency)
- Cross-region database replication

---

## 10. Cost Management

### 10.1 **Infrastructure Cost Estimation**

**Initial Setup** (Tashkent colocation):
| Item | Cost (USD/month) |
|------|------------------|
| 3x Bare Metal Servers (32 CPU, 128GB RAM) | $600 |
| Colocation (power, cooling, bandwidth) | $300 |
| Backup Storage (1TB) | $50 |
| Monitoring Tools (Grafana Cloud) | $50 |
| SSL Certificates (Let's Encrypt) | $0 |
| **Total** | **$1000/month** |

**Revenue Target** (to break even):

- 100 users paying $10/month = $1000
- OR: 20 users paying $50/month = $1000

**Pricing Tiers**:
| Tier | Price (UZS/month) | Included Resources |
|------|-------------------|-------------------|
| **Free** | 0 | 500m CPU, 512MB RAM, 1 project |
| **Starter** | 50,000 (~$5) | 2 CPU, 4GB RAM, 5 projects |
| **Pro** | 200,000 (~$20) | 8 CPU, 16GB RAM, unlimited projects |

---

## 11. Implementation Roadmap

### Month 1: Foundation

- [x] Basic auth (email/password)
- [x] Project & deployment CRUD
- [x] Kubernetes integration
- [ ] Fix async deployment creation (RabbitMQ)
- [ ] Implement compute-deployer worker
- [ ] Basic billing (hourly job)

### Month 2: Reliability

- [ ] compute-watcher (health monitoring)
- [ ] compute-informer (real-time state)
- [ ] Automatic deployment recovery
- [ ] User quotas & rate limiting
- [ ] Log streaming (Loki)

### Month 3: User Experience

- [ ] Web dashboard (Next.js)
- [ ] CLI tool (Rust)
- [ ] Email notifications
- [ ] Custom domains
- [ ] SSL certificate automation

### Month 4: Payment Integration

- [ ] Payme/Click integration
- [ ] Invoice generation
- [ ] Automatic suspension for non-payment
- [ ] Usage analytics dashboard

### Month 5: Advanced Features

- [ ] Environment variable encryption at rest
- [ ] Image vulnerability scanning
- [ ] One-click deploys from GitHub
- [ ] Managed databases (PostgreSQL addon)

### Month 6: Launch

- [ ] Beta testing with 50 users
- [ ] Performance testing (load tests)
- [ ] Security audit
- [ ] Public launch marketing

---

## 12. Recommended Service Architecture Changes

### Current Issues:

1. ❌ `create_deployment` creates K8s resources synchronously
2. ❌ Database transaction not used correctly in handlers
3. ❌ No retry mechanism for failed operations
4. ❌ Secrets stored in both DB and K8s (inconsistent)

### Recommended Changes:

#### **Rename Services for Clarity**:

```
compute           → compute-api
compute-deployer  → compute-provisioner  (clearer purpose)
compute-watcher   → compute-health-monitor
compute-informer  → compute-state-sync
```

#### **Add New Services**:

```
compute-scaler       → Auto-scaling based on metrics
billing-worker       → Hourly billing job
notification-service → Email/Telegram alerts
log-aggregator       → Forward logs to Loki
```

#### **Event Flow** (RabbitMQ queues):

```
# Deployment lifecycle
deployment.create.requested   → compute-provisioner
deployment.create.succeeded   → compute-state-sync, billing-worker
deployment.create.failed      → notification-service

# Health monitoring
deployment.health.degraded    → compute-health-monitor → notification-service
deployment.health.recovered   → compute-health-monitor → notification-service

# Scaling
deployment.scale.requested    → compute-provisioner
deployment.autoscale.trigger  → compute-scaler → compute-provisioner

# Billing
billing.hourly.trigger        → billing-worker
billing.balance.low           → notification-service
billing.balance.negative      → compute-health-monitor (suspend deployments)
```

---

## Final Recommendations

### **Top 5 Priorities Before Launch**:

1. ✅ **Fix async deployment creation** - Move K8s operations to background worker
2. ✅ **Implement user quotas** - Prevent resource exhaustion attacks
3. ✅ **Add payment integration** - Start with one local provider (Payme or Click)
4. ✅ **Build monitoring dashboard** - For ops team to detect issues early
5. ✅ **Security hardening** - PodSecurityPolicy, network policies, secrets encryption

### **Cost Optimization Tips**:

- Use **bin packing** (schedule pods efficiently to reduce wasted resources)
- Implement **auto-scaling** (scale down deployments during low traffic)
- Use **spot instances** (when moving to cloud providers)
- Cache Docker images locally (reduce registry bandwidth costs)

### **Marketing Strategy**:

- Target: Uzbek startups currently using Heroku/Render (expensive for them)
- Value prop: "Same features, 50% cheaper, data stays in Uzbekistan"
- Launch partners: Offer 3 months free for first 20 companies

---

**Good luck with Poddle! 🚀 Feel free to ask for clarification on any section.**
