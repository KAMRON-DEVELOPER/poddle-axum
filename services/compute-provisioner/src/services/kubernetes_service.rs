use std::collections::BTreeMap;
use std::collections::HashMap;

use chrono::Utc;
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
use kcr_cert_manager_io::v1::certificates::Certificate;
use kcr_traefik_io::v1alpha1::ingressroutes::{IngressRoute, IngressRouteSpec};
use kube::api::{DeleteParams, ObjectMeta, Patch, PatchParams, PostParams};
use kube::{Api, Client};
use serde_json::json;
use shared::models::ResourceSpec;
use shared::schemas::CreateDeploymentMessage;
use shared::schemas::DeleteDeploymentMessage;
use shared::schemas::UpdateDeploymentMessage;
use shared::services::redis::Redis;
use shared::utilities::channel_names::ChannelNames;
use shared::utilities::errors::AppError;
use sqlx::PgPool;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::services::vault_service::VaultService;

#[derive(Clone)]
pub struct KubernetesService {
    pub client: Client,
    pub pool: PgPool,
    pub redis: Redis,
    pub vault_service: VaultService,
    pub base_domain: String,
    pub enable_tls: bool,
    pub cluster_issuer: String,
    pub wildcard_certificate_name: String,
}

impl KubernetesService {
    pub async fn create(&self, message: CreateDeploymentMessage) -> Result<(), AppError> {
        let user_id = message.user_id;
        let project_id = message.project_id;
        let deployment_id = message.deployment_id;
        info!("🚀 Creating K8s resources for deployment {}", deployment_id);

        // Update status to 'provisioning'
        sqlx::query!(
            r#"UPDATE deployments SET status = 'provisioning' WHERE id = $1"#,
            deployment_id
        )
        .execute(&self.pool)
        .await?;

        // ! We can send pubsub message to notify users via SEE

        // TODO we must create in db side before uesr request it | Get or create namespace
        let deployment_namespace = Self::get_namespace_or_create(&self.client, user_id).await?;

        // TODO we must create in db side before uesr request it | Generate unique deployment name
        let deployment_name = format!(
            "{}-{}",
            message
                .name
                .to_lowercase()
                .replace("_", "-")
                .replace(" ", "-"),
            &deployment_id.to_string()[..8]
        );

        self.create_k8s_resources(
            &project_id,
            &deployment_id,
            &deployment_namespace,
            &deployment_name,
            message.image,
            message.port,
            message.replicas,
            message.resources,
            message.secrets,
            message.environment_variables,
            message.subdomain.as_deref(),
            message.custom_domain.as_deref(),
        )
        .await?;

        sqlx::query!(
            r#"
                UPDATE deployments
                SET status = 'starting',
                    cluster_namespace = $2,
                    cluster_deployment_name = $3
                WHERE id = $1
            "#,
            deployment_id,
            deployment_namespace,
            deployment_name
        )
        .execute(&self.pool)
        .await?;

        info!("✅ K8s resources created for deployment {}", deployment_id);

        Ok(())
    }

    async fn example_just_for_referance(&mut self) -> Result<(), AppError> {
        let now = Utc::now().timestamp();
        let mut pipe = redis::pipe();
        let message = json!({
            "type": "metrics_update",
            "timestamp": now,
        });
        let channel = ChannelNames::project_metrics(&"project_id");
        pipe.publish(channel, message.to_string()).ignore();
        let _: () = pipe
            .query_async(&mut self.redis.connection)
            .await
            .map_err(|e| AppError::InternalError(format!("Redis pipeline failed: {}", e)))?;

        Ok(())
    }

    async fn create_k8s_resources(
        &self,
        project_id: &Uuid,
        deployment_id: &Uuid,
        deployment_namespace: &str,
        deployment_name: &str,
        image: String,
        port: i32,
        replicas: i32,
        resources: ResourceSpec,
        secrets: Option<HashMap<String, String>>,
        environment_variables: Option<HashMap<String, String>>,
        subdomain: Option<&str>,
        custom_domain: Option<&str>,
    ) -> Result<(), AppError> {
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), deployment_name.to_string());
        labels.insert("project-id".to_string(), project_id.to_string());
        labels.insert("deployment-id".to_string(), deployment_id.to_string());
        labels.insert("managed-by".to_string(), "poddle".to_string());

        let secret_name = format!("{}-secrets", deployment_name);
        if !secrets.is_empty() {
            self.create_k8s_secret(deployment_namespace, &secret_name, secrets.clone(), &labels)
                .await?;
        }

        self.create_k8s_deployment(
            deployment_namespace,
            deployment_name,
            image,
            port,
            replicas,
            resources,
            &labels,
            &environment_variables,
            if secrets.is_empty() {
                None
            } else {
                Some(&secret_name)
            },
        )
        .await?;

        self.create_k8s_service(deployment_namespace, deployment_name, port, &labels)
            .await?;

        self.create_k8s_ingress(
            deployment_namespace,
            deployment_name,
            subdomain,
            custom_domain,
            &labels,
        )
        .await?;

        Ok(())
    }

    /// Create Kubernetes Secret
    async fn create_k8s_secret(
        &self,
        namespace: &str,
        secret_name: &str,
        secrets: HashMap<String, String>,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let secrets_api: Api<K8sSecret> = Api::namespaced(self.client.clone(), namespace);

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
        &self,
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
        let deployments_api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), namespace);

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
        &self,
        namespace: &str,
        name: &str,
        container_port: i32,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let services_api: Api<Service> = Api::namespaced(self.client.clone(), namespace);

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
    async fn create_k8s_ingress_route(
        &self,
        namespace: &str,
        name: &str,
        subdomain: &str,
        custom_domain: &str,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let ingrees_route = IngressRoute {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                annotations: Some(annotations),
                ..Default::default()
            },
            spec: IngressRouteSpec {
                entry_points: todo!(),
                parent_refs: todo!(),
                routes: todo!(),
                tls: todo!(),
            },
        };
    }

    /// Create Kubernetes Ingress with Traefik
    async fn create_k8s_ingress(
        &self,
        deployment_namespace: &str,
        deployment_name: &str,
        subdomain: Option<&str>,
        custom_domain: Option<&str>,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let ingress_api: Api<Ingress> = Api::namespaced(self.client.clone(), deployment_namespace);
        let certificate_api: Api<Certificate> =
            Api::namespaced(self.client.clone(), deployment_namespace);

        let mut annotations = BTreeMap::new();

        // ******************************************************************
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

        // For development with self-signed certs
        if !self.enable_tls {
            annotations.insert(
                "traefik.ingress.kubernetes.io/router.entrypoints".to_string(),
                "web,websecure".to_string(),
            );
            annotations.insert(
                "traefik.ingress.kubernetes.io/router.tls".to_string(),
                "true".to_string(),
            );
        } else {
            // Production with Let's Encrypt
            annotations.insert(
                "traefik.ingress.kubernetes.io/router.entrypoints".to_string(),
                "websecure".to_string(),
            );
            annotations.insert(
                "cert-manager.io/cluster-issuer".to_string(),
                self.cluster_issuer.clone(),
            );
        }

        // Redirect HTTP to HTTPS
        annotations.insert(
            "traefik.ingress.kubernetes.io/redirect-entry-point".to_string(),
            "https".to_string(),
        );
        annotations.insert(
            "traefik.ingress.kubernetes.io/redirect-permanent".to_string(),
            "true".to_string(),
        );
        // ******************************************************************

        let mut ingress = Ingress {
            metadata: ObjectMeta {
                name: Some(deployment_name.to_string()),
                namespace: Some(deployment_namespace.to_string()),
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
                                    name: deployment_name.to_string(),
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
                tls: Some(vec![
                    // ! we use wildcard for subdomains, cluster issuer not needed
                    IngressTLS {
                        hosts: Some(vec![external_url.to_string()]),
                        secret_name: Some(format!("{}-tls", deployment_name)),
                    },
                    // ! we use auto created secret via cluster issuer
                    IngressTLS {
                        hosts: Some(vec![external_url.to_string()]),
                        secret_name: Some(format!("{}-tls", deployment_name)),
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        if let Some(subdomain) = subdomain {
            // ! first we need to check wildcard issuer is exist or not
            let subdomain = format!("{}.{}", subdomain, self.base_domain);

            match certificate_api.get(&self.wildcard_certificate_name).await {
                Ok(certificate) => {
                    let secret_name = certificate.spec.secret_name;
                }
                Err(_) => {
                    // ! we try to create it
                }
            }
        }

        if let (custom_domain) = custom_domain {}

        ingress_api
            .create(&PostParams::default(), &ingress)
            .await
            .map_err(|e| AppError::InternalError(format!("Failed to create ingress: {}", e)))?;

        info!(
            "Ingress {} created in namespace {}",
            deployment_name, deployment_namespace
        );
        Ok(())
    }

    pub async fn update(&self, message: UpdateDeploymentMessage) -> Result<(), AppError> {
        let deployment_id = message.deployment_id;
        let user_id = message.user_id;

        let deployment = sqlx::query!(
            r#"
                SELECT cluster_namespace, cluster_deployment_name
                FROM deployments
                WHERE id = $1 AND user_id = $2
            "#,
            deployment_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFoundError("Deployment not found".to_string()))?;

        // ! We can send pubsub message to notify users via SEE

        let namespace = deployment.cluster_namespace;
        let name = deployment.cluster_deployment_name;

        let deployments_api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), &namespace);

        if let Some(replicas) = message.replicas {
            let patch = serde_json::json!({
                "spec": {
                    "replicas": replicas
                }
            });

            deployments_api
                .patch(&name, &PatchParams::default(), &Patch::Strategic(patch))
                .await
                .map_err(|e| {
                    AppError::InternalError(format!("Failed to update deployment: {}", e))
                })?;

            // ! We can send pubsub message to notify users via SEE

            sqlx::query!(
                r#"UPDATE deployments SET replicas = $1 WHERE id = $2"#,
                replicas,
                deployment_id
            )
            .execute(&self.pool)
            .await?;

            // ! We can send pubsub message to notify users via SEE

            info!("✅ Deployment updated");
        }

        Ok(())
    }

    pub async fn delete(&self, message: DeleteDeploymentMessage) -> Result<(), AppError> {
        let user_id = message.user_id;
        let deployment_id = message.deployment_id;

        let deployment = sqlx::query!(
            r#"
                SELECT cluster_namespace, cluster_deployment_name
                FROM deployments
                WHERE id = $1 AND user_id = $2
            "#,
            deployment_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFoundError("Deployment not found".to_string()))?;
        // ! We can send pubsub message to notify users via SEE

        let namespace = deployment.cluster_namespace;
        let name = deployment.cluster_deployment_name;

        let delete_params = DeleteParams::default();

        let ingress_api: Api<Ingress> = Api::namespaced(self.client.clone(), &namespace);
        let _ = ingress_api.delete(&name, &delete_params).await;

        let service_api: Api<Service> = Api::namespaced(self.client.clone(), &namespace);
        let _ = service_api.delete(&name, &delete_params).await;

        let deployment_api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), &namespace);
        let _ = deployment_api.delete(&name, &delete_params).await;

        let secret_name = format!("{}-secrets", name);
        let secret_api: Api<K8sSecret> = Api::namespaced(self.client.clone(), &namespace);
        let _ = secret_api.delete(&secret_name, &delete_params).await;

        info!("✅ K8s resources deleted for deployment {}", deployment_id);
        Ok(())
    }

    async fn get_namespace_or_create(client: &Client, user_id: Uuid) -> Result<String, AppError> {
        let ns_string = format!("user-{}", &user_id.to_string().replace("-", "")[..16]);
        let ns_api: Api<Namespace> = Api::all(client.clone());

        match ns_api.get(&ns_string).await {
            Ok(ns) => {
                info!("Namespace {:?} already exists", ns);
                Ok(ns_string)
            }
            Err(e) => {
                warn!("Namespace not exist: {}", e);
                info!("Creating namespace {}", ns_string);
                let mut labels = BTreeMap::new();
                labels.insert("user-id".to_string(), user_id.to_string());

                let new_ns = Namespace {
                    metadata: ObjectMeta {
                        name: Some(ns_string.clone()),
                        labels: Some(labels),
                        ..Default::default()
                    },
                    // spec: Some(NamespaceSpec {
                    //     finalizers: todo!(),
                    // }),
                    // status: Some(NamespaceStatus {
                    //     conditions: vec![NamespaceCondition {
                    //         last_transition_time: todo!(),
                    //         message: todo!(),
                    //         reason: todo!(),
                    //         status: todo!(),
                    //         type_: todo!(),
                    //     }],
                    //     phase: todo!(),
                    // }),
                    ..Default::default()
                };

                ns_api
                    .create(&PostParams::default(), &new_ns)
                    .await
                    .map_err(|e| {
                        AppError::InternalError(format!("Failed to create namespace: {}", e))
                    })?;

                info!("Namespace {} created successfully", ns_string);
                Ok(ns_string)
            }
        }
    }
}
