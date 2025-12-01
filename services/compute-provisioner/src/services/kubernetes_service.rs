use std::collections::BTreeMap;
use std::collections::HashMap;

use k8s_openapi::api::apps::v1::{Deployment as K8sDeployment, DeploymentSpec};
use k8s_openapi::api::core::v1::Namespace;
// use k8s_openapi::api::core::v1::NamespaceCondition;
// use k8s_openapi::api::core::v1::NamespaceSpec;
// use k8s_openapi::api::core::v1::NamespaceStatus;
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
use shared::models::ResourceSpec;
use shared::schemas::CreateDeploymentMessage;
use shared::schemas::DeleteDeploymentMessage;
use shared::schemas::UpdateDeploymentMessage;
use shared::utilities::errors::AppError;
use sqlx::PgPool;
use tracing::error;
use tracing::info;
use uuid::Uuid;

pub struct KubernetesService {
    pub client: Client,
    pub pool: PgPool,
    pub base_domain: String,
    pub enable_tls: bool,
    pub cluster_issuer: String,
}

impl KubernetesService {
    async fn get_namespace_or_create(client: &Client, user_id: Uuid) -> Result<String, AppError> {
        let ns_string = format!("user-{}", &user_id.to_string().replace("-", "")[..16]);
        let ns_api: Api<Namespace> = Api::all(client.clone());

        match ns_api.get(&ns_string).await {
            Ok(ns) => {
                info!("Namespace {:?} already exists", ns);
                Ok(ns_string)
            }
            Err(e) => {
                error!("Namespace not exist: {}", e);
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

    pub async fn create(&self, message: CreateDeploymentMessage) -> Result<(), AppError> {
        let user_id = message.user_id;
        let deployment_id = message.deployment_id;
        info!("🚀 Creating K8s resources for deployment {}", deployment_id);

        // Update status to 'provisioning'
        sqlx::query!(
            r#"
                UPDATE deployments
                SET status = 'provisioning'
                WHERE id = $1
            "#,
            deployment_id
        )
        .execute(&self.pool)
        .await?;

        // Get or create namespace
        let namespace = Self::get_namespace_or_create(&self.client, user_id).await?;

        // Generate unique deployment name
        let deployment_name = format!(
            "{}-{}",
            message
                .name
                .to_lowercase()
                .replace("_", "-")
                .replace(" ", "-"),
            &deployment_id.to_string()[..8]
        );

        // Determine subdomain
        let subdomain = message.subdomain.unwrap_or_else(|| {
            format!("{}-{}", message.name, &user_id.to_string()[..8])
                .to_lowercase()
                .replace("_", "-")
        });

        let external_url = format!("{}.{}", subdomain, self.base_domain);

        info!("📍 External URL: {}", external_url);

        let env_vars = message.environment_variables.unwrap_or_default();
        let secrets = message.secrets.unwrap_or_default();

        self.create_k8s_resources(
            &namespace,
            &deployment_name,
            &deployment_id,
            &message.image,
            message.port,
            message.replicas,
            &message.resources,
            &external_url,
            env_vars,
            secrets,
        )
        .await?;

        sqlx::query!(
            r#"
            UPDATE deployments
            SET status = 'starting',
                external_url = $2,
                cluster_namespace = $3,
                cluster_deployment_name = $4
            WHERE id = $1
            "#,
            deployment_id,
            external_url,
            namespace,
            deployment_name
        )
        .execute(&self.pool)
        .await?;

        info!("✅ K8s resources created for deployment {}", deployment_id);

        Ok(())
    }

    async fn create_k8s_resources(
        &self,
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
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), name.to_string());
        labels.insert("deployment-id".to_string(), deployment_id.to_string());
        labels.insert("managed-by".to_string(), "poddle".to_string());

        let secret_name = format!("{}-secrets", name);
        if !secrets.is_empty() {
            self.create_k8s_secret(namespace, &secret_name, &labels, secrets.clone())
                .await?;
        }

        self.create_k8s_deployment(
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

        self.create_k8s_service(namespace, name, container_port, &labels)
            .await?;

        self.create_k8s_ingress(namespace, name, external_url, &labels)
            .await?;

        Ok(())
    }

    /// Create Kubernetes Secret (secrets stored only in K8s, not in DB)
    async fn create_k8s_secret(
        &self,
        namespace: &str,
        secret_name: &str,
        labels: &BTreeMap<String, String>,
        secrets: HashMap<String, String>,
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
    async fn create_k8s_ingress(
        &self,
        namespace: &str,
        name: &str,
        external_url: &str,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let ingress_api: Api<Ingress> = Api::namespaced(self.client.clone(), namespace);

        let mut annotations = BTreeMap::new();

        // Traefik configuration
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

        let namespace = deployment.cluster_namespace;
        let name = deployment.cluster_deployment_name;

        let deployments_api: Api<K8sDeployment> =
            Api::namespaced(self.client.clone(), &namespace);

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

            sqlx::query!(
                r#"UPDATE deployments SET replicas = $2 WHERE id = $1"#,
                deployment_id,
                replicas
            )
            .execute(&self.pool)
            .await?;

            info!("✅ Deployment {} scaled to {} replicas", deployment_id, replicas);
        }

        Ok(())
    }

    pub async fn delete(&self, message: DeleteDeploymentMessage) -> Result<(), AppError> {
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

        sqlx::query!(
            r#"DELETE FROM deployments WHERE id = $1 AND user_id = $2"#,
            deployment_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        info!("✅ Deployment {} deleted successfully", deployment_id);
        Ok(())
    }
}
