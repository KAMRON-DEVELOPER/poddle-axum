use std::collections::BTreeMap;
use std::collections::HashMap;

use k8s_openapi::api::apps::v1::{Deployment as K8sDeployment, DeploymentSpec};
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, ResourceRequirements,
    Secret as K8sSecret, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::api::networking::v1::{
    HTTPIngressPath, HTTPIngressRuleValue, Ingress, IngressBackend, IngressRule,
    IngressServiceBackend, IngressSpec, IngressTLS, ServiceBackendPort,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{DeleteParams, ObjectMeta, Patch, PatchParams, PostParams};
use kube::{Api, Client};
use shared::utilities::errors::AppError;
use sqlx::PgPool;
use tracing::error;
use tracing::info;
use uuid::Uuid;

pub struct DeploymentService;

impl DeploymentService {
    async fn get_user_namespace(k8s_client: &Client, user_id: Uuid) -> Result<String, AppError> {
        let ns_string = format!("user-{}", &user_id.to_string().replace("-", "")[..16]);
        let ns_api: Api<Namespace> = Api::all(k8s_client.clone());

        match ns_api.get(&ns_string).await {
            Ok(_) => {
                info!("Namespace {} already exists", ns_string);
                Ok(ns_string)
            }
            Err(_) => {
                info!("Creating namespace {}", ns_string);
                let mut labels = BTreeMap::new();
                labels.insert("user-id".to_string(), user_id.to_string());

                let ns = Namespace {
                    metadata: ObjectMeta {
                        name: Some(ns_string.clone()),
                        labels: Some(labels),
                        ..Default::default()
                    },
                    ..Default::default()
                };

                ns_api
                    .create(&PostParams::default(), &ns)
                    .await
                    .map_err(|e| {
                        AppError::InternalError(format!("Failed to create namespace: {}", e))
                    })?;

                info!("Namespace {} created successfully", ns_string);
                Ok(ns_string)
            }
        }
    }

    pub async fn create(
        pool: &PgPool,
        k8s_client: &Client,
        user_id: Uuid,
        project_id: Uuid,
        base_domain: &str,
        req: CreateDeploymentRequest,
    ) -> Result<DeploymentResponse, AppError> {
        let namespace = Self::get_user_namespace(k8s_client, user_id).await?;

        // Generate unique deployment name
        let deployment_name = format!(
            "{}-{}",
            req.name.to_lowercase().replace("_", "-"),
            &Uuid::new_v4().to_string()[..8]
        );

        // Determine subdomain
        let subdomain = req.subdomain.unwrap_or_else(|| {
            format!("{}-{}", req.name, &user_id.to_string()[..8])
                .to_lowercase()
                .replace("_", "-")
        });

        let external_url = format!("{}.{}", subdomain, base_domain);

        // Prepare env vars (non-sensitive)
        let env_vars = req.env_vars.unwrap_or_default();
        let env_vars_json = serde_json::to_value(&env_vars)?;

        // Prepare resources
        let resources = req.resources.unwrap_or_default();
        let resources_json = serde_json::to_value(&resources)?;

        // Prepare labels
        let labels_json = req.labels.map(|l| serde_json::to_value(l).unwrap());

        // Start database transaction
        let mut tx = pool.begin().await?;

        // Create deployment record
        let deployment = DeploymentRepository::create(
            &mut tx,
            user_id,
            project_id,
            &req.name,
            &req.image,
            env_vars_json.clone(),
            req.replicas,
            resources_json.clone(),
            labels_json,
            &namespace,
            &deployment_name,
        )
        .await?;

        // Commit transaction
        tx.commit().await?;

        // Create Kubernetes resources (secrets are NOT stored in DB)
        let secrets = req.secrets.unwrap_or_default();

        match Self::create_k8s_resources(
            k8s_client,
            &namespace,
            &deployment_name,
            &deployment.id,
            &req.image,
            req.port,
            req.replicas,
            &resources,
            &external_url,
            env_vars,
            secrets,
        )
        .await
        {
            Ok(_) => {
                // Update status to running
                DeploymentRepository::update_status(pool, deployment.id, DeploymentStatus::Running)
                    .await?;

                // Log success event
                DeploymentEventRepository::create(
                    pool,
                    deployment.id,
                    "deployment_created",
                    Some(&format!("Deployment created at {}", external_url)),
                )
                .await?;

                info!("Deployment {} created successfully", deployment.id);

                Ok(DeploymentResponse {
                    id: deployment.id,
                    project_id: deployment.project_id,
                    name: deployment.name,
                    image: deployment.image,
                    status: DeploymentStatus::Running,
                    replicas: deployment.replicas,
                    resources,
                    external_url: Some(external_url),
                    created_at: deployment.created_at,
                    updated_at: deployment.updated_at,
                })
            }
            Err(e) => {
                error!("Failed to create K8s resources: {}", e);

                // Update status to failed
                DeploymentRepository::update_status(pool, deployment.id, DeploymentStatus::Failed)
                    .await?;

                // Log failure event
                DeploymentEventRepository::create(
                    pool,
                    deployment.id,
                    "deployment_failed",
                    Some(&format!("Failed to create K8s resources: {}", e)),
                )
                .await?;

                Err(e)
            }
        }
    }

    /// Create all Kubernetes resources (Secret, Deployment, Service, Ingress)
    async fn create_k8s_resources(
        client: &Client,
        namespace: &str,
        name: &str,
        deployment_id: &Uuid,
        image: &str,
        container_port: i32,
        replicas: i32,
        resources: &ResourceSpec,
        external_url: &str,
        env_vars: HashMap<String, String>,
        secrets: HashMap<String, String>,
    ) -> Result<(), AppError> {
        // Common labels
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), name.to_string());
        labels.insert("deployment-id".to_string(), deployment_id.to_string());
        labels.insert("managed-by".to_string(), "poddle".to_string());

        // 1. Create Secret (if any secrets provided)
        let secret_name = format!("{}-secrets", name);
        if !secrets.is_empty() {
            Self::create_k8s_secret(client, namespace, &secret_name, &labels, secrets.clone())
                .await?;
        }

        // 2. Create Deployment
        Self::create_k8s_deployment(
            client,
            namespace,
            name,
            image,
            container_port,
            replicas,
            resources,
            &labels,
            &env_vars,
            if secrets.is_empty() {
                None
            } else {
                Some(&secret_name)
            },
        )
        .await?;

        // 3. Create Service
        Self::create_k8s_service(client, namespace, name, container_port, &labels).await?;

        // 4. Create Ingress
        Self::create_k8s_ingress(client, namespace, name, external_url, &labels).await?;

        Ok(())
    }

    /// Create Kubernetes Secret (secrets stored only in K8s, not in DB)
    async fn create_k8s_secret(
        client: &Client,
        namespace: &str,
        secret_name: &str,
        labels: &BTreeMap<String, String>,
        secrets: HashMap<String, String>,
    ) -> Result<(), AppError> {
        let secrets_api: Api<K8sSecret> = Api::namespaced(client.clone(), namespace);

        let mut string_data = BTreeMap::new();
        for (key, value) in secrets {
            string_data.insert(key, value);
        }

        let secret = K8sSecret {
            metadata: ObjectMeta {
                name: Some(secret_name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            string_data: Some(string_data), // K8s handles base64 encoding
            ..Default::default()
        };

        secrets_api
            .create(&PostParams::default(), &secret)
            .await
            .map_err(|e| AppError::InternalError(format!("Failed to create secret: {}", e)))?;

        info!("Secret {} created in namespace {}", secret_name, namespace);
        Ok(())
    }

    /// Create Kubernetes Deployment
    async fn create_k8s_deployment(
        client: &Client,
        namespace: &str,
        name: &str,
        image: &str,
        container_port: i32,
        replicas: i32,
        resources: &ResourceSpec,
        labels: &BTreeMap<String, String>,
        env_vars: &HashMap<String, String>,
        secret_name: Option<&str>,
    ) -> Result<(), AppError> {
        let deployments_api: Api<K8sDeployment> = Api::namespaced(client.clone(), namespace);

        // Build environment variables
        let mut container_env = vec![];

        // Regular env vars
        for (key, value) in env_vars {
            container_env.push(EnvVar {
                name: key.clone(),
                value: Some(value.clone()),
                ..Default::default()
            });
        }

        // Secret env vars
        if let Some(secret_name) = secret_name {
            // Note: You'll need to know which keys are in the secret
            // For now, we'll reference the entire secret as env vars
            // In production, you might want to track secret keys separately
            container_env.push(EnvVar {
                name: "SECRET_REFERENCE".to_string(),
                value: Some(secret_name.to_string()),
                ..Default::default()
            });
        }

        // Resource requirements
        let mut resource_requests = BTreeMap::new();
        resource_requests.insert(
            "cpu".to_string(),
            Quantity(format!("{}m", resources.cpu_request_millicores)),
        );
        resource_requests.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", resources.memory_request_mb)),
        );

        let mut resource_limits = BTreeMap::new();
        resource_limits.insert(
            "cpu".to_string(),
            Quantity(format!("{}m", resources.cpu_limit_millicores)),
        );
        resource_limits.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", resources.memory_limit_mb)),
        );

        let deployment = K8sDeployment {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(replicas),
                selector: LabelSelector {
                    match_labels: Some(labels.clone()),
                    ..Default::default()
                },
                template: PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(labels.clone()),
                        ..Default::default()
                    }),
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: "app".to_string(),
                            image: Some(image.to_string()),
                            image_pull_policy: Some("Always".to_string()),
                            ports: Some(vec![ContainerPort {
                                container_port,
                                protocol: Some("TCP".to_string()),
                                ..Default::default()
                            }]),
                            env: if container_env.is_empty() {
                                None
                            } else {
                                Some(container_env)
                            },
                            resources: Some(ResourceRequirements {
                                requests: Some(resource_requests),
                                limits: Some(resource_limits),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        deployments_api
            .create(&PostParams::default(), &deployment)
            .await
            .map_err(|e| {
                AppError::InternalError(format!("Failed to create K8s deployment: {}", e))
            })?;

        info!("Deployment {} created in namespace {}", name, namespace);
        Ok(())
    }

    /// Create Kubernetes Service
    async fn create_k8s_service(
        client: &Client,
        namespace: &str,
        name: &str,
        container_port: i32,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let services_api: Api<Service> = Api::namespaced(client.clone(), namespace);

        let service = Service {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: Some(labels.clone()),
                ports: Some(vec![ServicePort {
                    name: Some("http".to_string()),
                    port: 80,
                    target_port: Some(IntOrString::Int(container_port)),
                    protocol: Some("TCP".to_string()),
                    ..Default::default()
                }]),
                type_: Some("ClusterIP".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        services_api
            .create(&PostParams::default(), &service)
            .await
            .map_err(|e| AppError::InternalError(format!("Failed to create service: {}", e)))?;

        info!("Service {} created in namespace {}", name, namespace);
        Ok(())
    }

    /// Create Kubernetes Ingress with Traefik
    async fn create_k8s_ingress(
        client: &Client,
        namespace: &str,
        name: &str,
        external_url: &str,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let ingress_api: Api<Ingress> = Api::namespaced(client.clone(), namespace);

        let mut annotations = BTreeMap::new();
        annotations.insert(
            "kubernetes.io/ingress.class".to_string(),
            "traefik".to_string(),
        );
        annotations.insert(
            "traefik.ingress.kubernetes.io/router.entrypoints".to_string(),
            "websecure".to_string(),
        );
        annotations.insert(
            "cert-manager.io/cluster-issuer".to_string(),
            "letsencrypt-prod".to_string(),
        );

        let ingress = Ingress {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                annotations: Some(annotations),
                ..Default::default()
            },
            spec: Some(IngressSpec {
                rules: Some(vec![IngressRule {
                    host: Some(external_url.to_string()),
                    http: Some(HTTPIngressRuleValue {
                        paths: vec![HTTPIngressPath {
                            path: Some("/".to_string()),
                            path_type: "Prefix".to_string(),
                            backend: IngressBackend {
                                service: Some(IngressServiceBackend {
                                    name: name.to_string(),
                                    port: Some(ServiceBackendPort {
                                        number: Some(80),
                                        ..Default::default()
                                    }),
                                }),
                                ..Default::default()
                            },
                        }],
                    }),
                }]),
                tls: Some(vec![IngressTLS {
                    hosts: Some(vec![external_url.to_string()]),
                    secret_name: Some(format!("{}-tls", name)),
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        ingress_api
            .create(&PostParams::default(), &ingress)
            .await
            .map_err(|e| AppError::InternalError(format!("Failed to create ingress: {}", e)))?;

        info!("Ingress {} created in namespace {}", name, namespace);
        Ok(())
    }

    /// Scale deployment replicas
    pub async fn scale(
        pool: &PgPool,
        k8s_client: &Client,
        deployment_id: Uuid,
        user_id: Uuid,
        new_replicas: i32,
    ) -> Result<DeploymentResponse, AppError> {
        let deployment =
            DeploymentRepository::update_replicas(pool, deployment_id, user_id, new_replicas)
                .await?;

        let deployments_api: Api<K8sDeployment> =
            Api::namespaced(k8s_client.clone(), &deployment.cluster_namespace);

        let patch = serde_json::json!({
            "spec": {
                "replicas": new_replicas
            }
        });

        deployments_api
            .patch(
                &deployment.cluster_deployment_name,
                &PatchParams::default(),
                &Patch::Strategic(patch),
            )
            .await
            .map_err(|e| AppError::InternalError(format!("Failed to scale deployment: {}", e)))?;

        DeploymentEventRepository::create(
            pool,
            deployment.id,
            "deployment_scaled",
            Some(&format!("Scaled to {} replicas", new_replicas)),
        )
        .await?;

        let resources: ResourceSpec = serde_json::from_value(deployment.resources.clone())?;

        Ok(DeploymentResponse {
            id: deployment.id,
            project_id: deployment.project_id,
            name: deployment.name,
            image: deployment.image,
            status: deployment.status,
            replicas: deployment.replicas,
            resources,
            external_url: None,
            created_at: deployment.created_at,
            updated_at: deployment.updated_at,
        })
    }

    /// Delete deployment and all K8s resources
    pub async fn delete(
        pool: &PgPool,
        k8s_client: &Client,
        deployment_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        let deployment = DeploymentRepository::get_by_id(pool, deployment_id, user_id).await?;

        let namespace = &deployment.cluster_namespace;
        let name = &deployment.cluster_deployment_name;
        let delete_params = DeleteParams::default();

        // Delete in reverse order (Ingress -> Service -> Deployment -> Secret)
        let ingress_api: Api<Ingress> = Api::namespaced(k8s_client.clone(), namespace);
        let _ = ingress_api.delete(name, &delete_params).await;

        let service_api: Api<Service> = Api::namespaced(k8s_client.clone(), namespace);
        let _ = service_api.delete(name, &delete_params).await;

        let deployment_api: Api<K8sDeployment> = Api::namespaced(k8s_client.clone(), namespace);
        let _ = deployment_api.delete(name, &delete_params).await;

        let secret_name = format!("{}-secrets", name);
        let secret_api: Api<K8sSecret> = Api::namespaced(k8s_client.clone(), namespace);
        let _ = secret_api.delete(&secret_name, &delete_params).await;

        // Delete from database
        DeploymentRepository::delete(pool, deployment_id, user_id).await?;

        info!("Deployment {} deleted successfully", deployment_id);
        Ok(())
    }

    /// Get deployment details with live K8s status
    pub async fn get_detail(
        pool: &PgPool,
        k8s_client: &Client,
        deployment_id: Uuid,
        user_id: Uuid,
    ) -> Result<DeploymentDetailResponse, AppError> {
        let deployment = DeploymentRepository::get_by_id(pool, deployment_id, user_id).await?;

        // Get live K8s deployment status
        let deployments_api: Api<K8sDeployment> =
            Api::namespaced(k8s_client.clone(), &deployment.cluster_namespace);

        let ready_replicas = match deployments_api
            .get(&deployment.cluster_deployment_name)
            .await
        {
            Ok(k8s_dep) => k8s_dep
                .status
                .and_then(|s| s.ready_replicas)
                .map(|r| r as i32),
            Err(e) => {
                error!("Failed to get K8s deployment status: {}", e);
                None
            }
        };

        let env_vars: HashMap<String, String> =
            serde_json::from_value(deployment.env_vars.clone())?;
        let resources: ResourceSpec = serde_json::from_value(deployment.resources.clone())?;
        let labels: Option<HashMap<String, String>> = deployment
            .labels
            .as_ref()
            .map(|l| serde_json::from_value(l.clone()).unwrap());

        // Get Ingress URL
        let ingress_api: Api<Ingress> =
            Api::namespaced(k8s_client.clone(), &deployment.cluster_namespace);
        let external_url = match ingress_api.get(&deployment.cluster_deployment_name).await {
            Ok(ingress) => ingress
                .spec
                .and_then(|spec| spec.rules)
                .and_then(|rules| rules.first().cloned())
                .and_then(|rule| rule.host),
            Err(_) => None,
        };

        Ok(DeploymentDetailResponse {
            id: deployment.id,
            project_id: deployment.project_id,
            name: deployment.name,
            image: deployment.image,
            status: deployment.status,
            replicas: deployment.replicas,
            ready_replicas,
            resources,
            env_vars,
            secret_keys: vec![], // Don't expose secret keys
            labels,
            external_url,
            cluster_namespace: deployment.cluster_namespace,
            created_at: deployment.created_at,
            updated_at: deployment.updated_at,
        })
    }
}
