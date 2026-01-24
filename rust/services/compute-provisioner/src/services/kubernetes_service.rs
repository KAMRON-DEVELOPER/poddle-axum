use std::collections::{BTreeMap, HashMap};

use chrono::Utc;
use compute_core::models::{DeploymentStatus, ResourceSpec};
use compute_core::{
    channel_names::ChannelNames,
    schemas::{CreateDeploymentMessage, DeleteDeploymentMessage, UpdateDeploymentMessage},
};
use factory::factories::redis::Redis;
use k8s_openapi::{
    api::{
        apps::v1::{Deployment as K8sDeployment, DeploymentSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, Namespace, PodSpec, PodTemplateSpec,
            ResourceRequirements, Secret as K8sSecret, Service, ServicePort, ServiceSpec,
        },
    },
    apimachinery::pkg::{
        api::resource::Quantity, apis::meta::v1::LabelSelector, util::intstr::IntOrString,
    },
};
use kcr_cert_manager_io::v1::{certificates::Certificate, clusterissuers::ClusterIssuer};
use kcr_secrets_hashicorp_com::v1beta1::{
    vaultauths::{VaultAuth, VaultAuthKubernetes, VaultAuthMethod, VaultAuthSpec},
    vaultconnections::{VaultConnection, VaultConnectionSpec},
    vaultstaticsecrets::{
        VaultStaticSecret, VaultStaticSecretDestination, VaultStaticSecretRolloutRestartTargets,
        VaultStaticSecretRolloutRestartTargetsKind, VaultStaticSecretSpec, VaultStaticSecretType,
    },
};
use kcr_traefik_io::v1alpha1::ingressroutes::{
    IngressRoute, IngressRouteParentRefs, IngressRouteRoutes, IngressRouteRoutesServices,
    IngressRouteSpec,
};

use kube::{
    api::{DeleteParams, ObjectMeta, Patch, PatchParams, PostParams},
    {Api, Client},
};

use redis::AsyncTypedCommands;
use serde_json::json;
use sqlx::PgPool;
use tracing::{Instrument, error, info, info_span, warn};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::repository::DeploymentRepository;
use crate::services::vault_service::VaultService;

#[derive(Clone)]
pub struct KubernetesService {
    pub client: Client,
    pub pool: PgPool,
    pub redis: Redis,
    pub vault_service: VaultService,
    pub domain: String,
    pub traefik_namespace: String,
    pub cluster_issuer_name: String,
    pub ingress_class_name: Option<String>,
    pub ingressroute_entry_points: Option<Vec<String>>,
    pub wildcard_certificate_name: String,
    pub wildcard_certificate_secret_name: String,
}

impl KubernetesService {
    pub async fn init(&self) -> Result<(), AppError> {
        info!("🏁Performing pre-flight infrastructure checks...");

        // Check for ClusterIssuer
        let cluster_issuer_api: Api<ClusterIssuer> = Api::all(self.client.clone());
        match cluster_issuer_api.get(&self.cluster_issuer_name).await {
            Ok(_) => info!("✅ ClusterIssuer '{}' found.", self.cluster_issuer_name),
            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                error!("❌ ClusterIssuer '{}' is missing", self.cluster_issuer_name);
                return Err(AppError::InternalServerError(format!(
                    "ClusterIssuer '{}' is missing. Please apply infrastructure configuration.",
                    self.cluster_issuer_name
                )));
            }
            Err(e) => return Err(e.into()),
        }

        // Check for Wildcard Certificate
        let certificate_api: Api<Certificate> =
            Api::namespaced(self.client.clone(), &self.traefik_namespace);
        match certificate_api.get(&self.wildcard_certificate_name).await {
            Ok(_) => info!(
                "✅ Wildcard Certificate '{}' found.",
                self.wildcard_certificate_name
            ),
            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                error!(
                    "❌ Wildcard Certificate '{}' is missing in namespace '{}'.",
                    self.wildcard_certificate_name, self.traefik_namespace
                );
                return Err(AppError::InternalServerError(format!(
                    "Wildcard Certificate '{}' is missing in namespace '{}'.",
                    self.wildcard_certificate_name, self.traefik_namespace
                )));
            }
            Err(e) => return Err(e.into()),
        }

        info!("🚀 Infrastructure checks passed. Provisioner ready.");
        Ok(())
    }

    #[tracing::instrument(
        name = "kubernetes_service.create",
        skip_all,
        fields(user_id = %msg.user_id, project_id = %msg.project_id, deployment_id = %msg.deployment_id),
        err
    )]
    pub async fn create(&self, msg: CreateDeploymentMessage) -> Result<(), AppError> {
        let user_id = msg.user_id.clone();
        let project_id = msg.project_id.clone();
        let deployment_id = msg.deployment_id.clone();

        info!(
            user_id = %user_id,
            project_id = %project_id,
            deployment_id = %deployment_id,
            "🚀 Creating K8s resources"
        );

        // Update status to 'provisioning'
        let query_result = DeploymentRepository::update_status(
            &deployment_id,
            DeploymentStatus::Provisioning,
            &self.pool,
        )
        .await?;

        if query_result.rows_affected() == 0 {
            warn!(
                user_id = %user_id,
                project_id = %project_id,
                deployment_id = %deployment_id,
                "❌ Update deployment status affected zero rows"
            );
        }

        let channel = ChannelNames::project_metrics(&project_id.to_string());
        let now = Utc::now().timestamp();
        let message = json!({
            "type": "status_update",
            "status": "provisioning",
            "deployment_id": &deployment_id,
            "timestamp": now,
        });
        let _ = self
            .redis
            .clone()
            .connection
            .publish(channel, message.to_string())
            .instrument(info_span!("pubsub.status_update"))
            .await;

        self.create_k8s_resources(msg).await?;

        let query_result = DeploymentRepository::update_status(
            &deployment_id,
            DeploymentStatus::Starting,
            &self.pool,
        )
        .await?;

        if query_result.rows_affected() == 0 {
            warn!(
                user_id = %user_id,
                project_id = %project_id,
                deployment_id = %deployment_id,
                "❌ Update deployment status affected zero rows"
            );
        }

        info!("✅ K8s resources created for deployment {}", deployment_id);

        Ok(())
    }

    async fn create_k8s_resources(&self, msg: CreateDeploymentMessage) -> Result<(), AppError> {
        let user_namespace = self.ensure_namespace(&msg.user_id).await?;
        let resource_name = self.format_resource_name(&msg.deployment_id);

        let mut labels = BTreeMap::new();
        labels.insert(
            "app.kubernetes.io/managed-by".to_string(),
            "poddle".to_string(),
        );
        labels.insert(
            "poddle.io/project-id".to_string(),
            msg.project_id.to_string(),
        );
        labels.insert(
            "poddle.io/deployment-id".to_string(),
            msg.deployment_id.to_string(),
        );

        let secret_name = self
            .create_vault_static_secret(
                &deployment_namespace,
                &deployment_name,
                &msg.deployment_id.to_string(),
                msg.secrets,
            )
            .await?;

        self.create_k8s_deployment(
            &deployment_namespace,
            &deployment_name,
            &msg.image,
            msg.port,
            msg.desired_replicas,
            &msg.resource_spec,
            secret_name,
            msg.environment_variables,
            &labels,
        )
        .await?;

        self.create_k8s_service(&deployment_namespace, &service_name, msg.port, &labels)
            .await?;

        // ! ERROR, We use Traefik IngressRoute
        self.create_traefik_ingressroute(
            deployment_namespace,
            service,
            &deployment_name,
            msg.domain.as_deref(),
            msg.domain.as_deref(),
            msg.subdomain.as_deref() & labels,
        )
        .await?;

        Ok(())
    }

    /// Create VSO resources
    async fn create_vault_static_secret(
        &self,
        deployment_namespace: &str,
        deployment_name: &str,
        deployment_id: &str,
        secrets: Option<HashMap<String, String>>,
    ) -> Result<Option<String>, AppError> {
        // Option::filter is essentially a way to apply an additional condition to an Option that is already known to be Some
        // We are checking to be empty as additional condition
        if secrets.filter(|hm| hm.is_empty()).is_none() || secrets.is_none() {
            return Ok(None);
        }

        // Write to Vault
        let secret_path = self
            .vault_service
            .store_secrets(deployment_namespace, deployment_id, secrets)
            .await?;

        // Define the VSO Resource
        let vault_static_secret_name = format!("{}-vso", deployment_name);
        let secret_name = format!("{}-secrets", deployment_name);

        let vault_static_secret = VaultStaticSecret {
            metadata: ObjectMeta {
                name: Some(vault_static_secret_name),
                namespace: Some(deployment_namespace.to_owned()),
                ..Default::default()
            },
            spec: VaultStaticSecretSpec {
                vault_auth_ref: self.vault_service.vault_auth,
                mount: self.vault_service.kv_mount,
                r#type: VaultStaticSecretType::KvV2,
                path: secret_path,
                destination: VaultStaticSecretDestination {
                    create: Some(true),
                    name: secret_name,
                    ..Default::default()
                },
                // reconcilation interval
                refresh_after: self.vault_service.refresh_after,
                // Restart the deployment if secrets change
                rollout_restart_targets: Some(vec![VaultStaticSecretRolloutRestartTargets {
                    kind: VaultStaticSecretRolloutRestartTargetsKind::Deployment,
                    name: deployment_name.to_string(),
                }]),
                hmac_secret_data: Some(true),
                namespace: Some(deployment_namespace.to_owned()),
                sync_config: None,
                version: Some(2),
            },
            status: None,
        };

        // APPLY the VSO CRD
        let vault_static_secret_api: Api<VaultStaticSecret> =
            Api::namespaced(self.client, deployment_namespace);

        vault_static_secret_api
            .create(&PostParams::default(), &vault_static_secret)
            .instrument(info_span!("create_vault_static_secret"))
            .await
            .map_err(|e| {
                error!(deployment_id=%deployment_id, "Failed to create VSO Secret");
                AppError::InternalServerError(format!("Failed to create VSO Secret: {}", e))
            })?;

        Ok(Some(secret_name))
    }

    async fn _create_k8s_secret(
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
            string_data: Some(string_data),
            ..Default::default()
        };

        secrets_api
            .create(&PostParams::default(), &secret)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create secret: {}", e))
            })?;

        info!("Secret {} created in namespace {}", secret_name, namespace);
        Ok(())
    }

    /// Create Kubernetes Deployment
    async fn create_k8s_deployment(
        &self,
        namespace: &str,
        name: &str,
        image: &str,
        port: i32,
        desired_replicas: i32,
        resource_spec: &ResourceSpec,
        secret_name: Option<String>,
        environment_variables: Option<HashMap<String, String>>,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let deployments_api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), namespace);

        // Build environment variables
        let mut container_env = vec![];

        if let Some(environment_variables) = environment_variables {
            for (key, value) in environment_variables {
                container_env.push(EnvVar {
                    name: key.clone(),
                    value: Some(value.clone()),
                    ..Default::default()
                });
            }
        }

        if let Some(secret_name) = secret_name {
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
            Quantity(format!("{}m", resource_spec.cpu_request_millicores)),
        );
        resource_requests.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", resource_spec.memory_request_mb)),
        );

        let mut resource_limits = BTreeMap::new();
        resource_limits.insert(
            "cpu".to_string(),
            Quantity(format!("{}m", resource_spec.cpu_limit_millicores)),
        );
        resource_limits.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", resource_spec.memory_limit_mb)),
        );

        let deployment = K8sDeployment {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(desired_replicas),
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
                                container_port: port,
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
                AppError::InternalServerError(format!("Failed to create K8s deployment: {}", e))
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
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create service: {}", e))
            })?;

        info!("Service {} created in namespace {}", name, namespace);
        Ok(())
    }

    /// Create Traefik IngressRoute
    async fn create_traefik_ingressroute(
        &self,
        namespace: String,
        service: String,
        name: String,
        domain: Option<String>,
        subdomain: String,
        labels: Option<BTreeMap<String, String>>,
    ) -> Result<(), AppError> {
        let mut ingrees_route = IngressRoute {
            metadata: ObjectMeta {
                name: Some(name),
                namespace: Some(namespace),
                labels,
                ..Default::default()
            },
            spec: IngressRouteSpec {
                entry_points: self.ingressroute_entry_points,
                parent_refs: Some(vec![IngressRouteParentRefs {
                    name: service.clone(),
                    ..Default::default()
                }]),
                routes: vec![IngressRouteRoutes {
                    r#match: subdomain,
                    services: Some(vec![IngressRouteRoutesServices {
                        name: service.clone(),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }],
                tls: todo!(),
            },
        };

        if let Some(domain) = domain {
            ingrees_route.spec.routes.push(IngressRouteRoutes {
                r#match: domain,
                services: Some(vec![IngressRouteRoutesServices {
                    name: service,
                    ..Default::default()
                }]),
                ..Default::default()
            });
        }

        let ingressroute_api: Api<IngressRoute> = Api::namespaced(self.client.clone(), &namespace);
        ingressroute_api
            .create(&PostParams::default(), &ingrees_route)
            .await?;
    }

    pub async fn update(&self, message: UpdateDeploymentMessage) -> Result<(), AppError> {
        let user_id = message.user_id;
        let project_id = message.project_id;
        let deployment_id = message.deployment_id;

        let deployment = DeploymentRepository::get_one_by_id(&deployment_id, &self.pool).await?;

        // ! We can send pubsub message to notify users via SEE

        let namespace = self.ensure_namespace(user_id).await?;
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
                    AppError::InternalServerError(format!("Failed to update deployment: {}", e))
                })?;

            // ! We can send pubsub message to notify users via SEE

            let query_result =
                DeploymentRepository::update_replicas(&deployment_id, replicas, &self.pool).await?;

            if query_result.rows_affected() == 0 {
                warn!(
                    user_id = %user_id,
                    project_id = %project_id,
                    deployment_id = %deployment_id,
                    "❌ Update deployment replicas affected zero rows"
                );
            }

            // ! We can send pubsub message to notify users via SEE

            info!("✅ Deployment updated");
        }

        Ok(())
    }

    pub async fn delete(&self, message: DeleteDeploymentMessage) -> Result<(), AppError> {
        let user_id = message.user_id;
        let project_id = message.project_id;
        let deployment_id = message.deployment_id;

        let deployment = DeploymentRepository::get_one_by_id(&deployment_id, &self.pool).await?;
        // ! We can send pubsub message to notify users via SEE

        let namespace = self.ensure_namespace(user_id).await?;
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

    /// creates namespace for user like `user-{user_id[:16]}`
    /// additionally VaultAuth and VaultConnection, default VaultConnection can be used instead namespaced
    async fn ensure_namespace(&self, user_id: &Uuid) -> Result<String, AppError> {
        let name = format!(
            "user-{}",
            user_id
                .as_simple()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        );
        let ns_api: Api<Namespace> = Api::all(self.client.clone());

        if ns_api.get(&name).await.is_ok() {
            return Ok(name);
        }

        info!(user_id = %user_id, "Creating namespace {}", name);
        let mut labels = BTreeMap::new();
        labels.insert("user-id".to_string(), user_id.to_string());

        let new_ns = Namespace {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                labels: Some(labels),
                ..Default::default()
            },
            ..Default::default()
        };

        ns_api.create(&PostParams::default(), &new_ns).await?;
        info!(user_id = %user_id, "Namespace {} created successfully", name);

        Ok(name)
    }

    /// generate resource name from `deployment_id` like `app-{deployment_id[:8]}`
    fn format_resource_name(&self, deployment_id: &Uuid) -> String {
        format!(
            "app-{}",
            deployment_id
                .as_simple()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        )
    }

    async fn create_vso_resources(&self, ns: String) -> Result<(), AppError> {
        let vault_connection = VaultConnection {
            metadata: ObjectMeta {
                name: self.vault_service.vault_connection.clone().name,
                namespace: Some(ns.clone()),
                ..Default::default()
            },
            spec: VaultConnectionSpec {
                address: self.vault_service.vault_connection.address.clone(),
                skip_tls_verify: self.vault_service.vault_connection.skip_tls_verify,
                ..Default::default()
            },
            ..Default::default()
        };

        let vault_auth = VaultAuth {
            metadata: ObjectMeta {
                name: self
                    .vault_service
                    .vault_auth
                    .clone()
                    .unwrap_or_default()
                    .name,
                namespace: Some(ns.clone()),
                ..Default::default()
            },
            spec: VaultAuthSpec {
                method: Some(VaultAuthMethod::Kubernetes),
                mount: self
                    .vault_service
                    .vault_auth
                    .clone()
                    .unwrap_or_default()
                    .mount,
                kubernetes: Some(VaultAuthKubernetes {
                    role: self
                        .vault_service
                        .vault_auth
                        .clone()
                        .unwrap_or_default()
                        .kubernetes
                        .unwrap_or_default()
                        .role,
                    service_account: self
                        .vault_service
                        .vault_auth
                        .clone()
                        .unwrap_or_default()
                        .kubernetes
                        .unwrap_or_default()
                        .service_account,
                    ..Default::default()
                }),
                vault_connection_ref: vault_connection.clone().metadata.name,
                ..Default::default()
            },
            ..Default::default()
        };

        let vault_connection_api: Api<VaultConnection> = Api::namespaced(self.client.clone(), &ns);
        let vault_auth_api: Api<VaultAuth> = Api::namespaced(self.client.clone(), &ns);

        vault_connection_api
            .create(&PostParams::default(), &vault_connection)
            .instrument(info_span!("create_vault_connection"))
            .await
            .map_err(|e| {
                error!(ns=%ns, "Failed to create VaultConnection");
                AppError::InternalServerError(format!("Failed to create VaultConnection: {}", e))
            })?;
        vault_auth_api
            .create(&PostParams::default(), &vault_auth)
            .instrument(info_span!("create_vault_api"))
            .await
            .map_err(|e| {
                error!(ns=%ns, "Failed to create VaultAuth");
                AppError::InternalServerError(format!("Failed to create VaultAuth: {}", e))
            })?;

        info!(ns=%ns, "VaultAuth and VaultConnection created in {}", ns);

        Ok(())
    }
}
