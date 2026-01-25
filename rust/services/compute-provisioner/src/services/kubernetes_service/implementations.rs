use std::collections::{BTreeMap, HashMap};

use chrono::Utc;
use compute_core::models::{DeploymentStatus, ResourceSpec};
use compute_core::{
    channel_names::ChannelNames,
    schemas::{CreateDeploymentMessage, DeleteDeploymentMessage, UpdateDeploymentMessage},
};
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
    vaultauths::{VaultAuth, VaultAuthKubernetes, VaultAuthMethod},
    vaultconnections::VaultConnection,
    vaultstaticsecrets::{
        VaultStaticSecret, VaultStaticSecretDestination, VaultStaticSecretRolloutRestartTargets,
        VaultStaticSecretRolloutRestartTargetsKind, VaultStaticSecretSpec, VaultStaticSecretType,
    },
};
use kcr_traefik_io::v1alpha1::ingressroutes::{
    IngressRoute, IngressRouteParentRefs, IngressRouteRoutes, IngressRouteRoutesServices,
    IngressRouteSpec, IngressRouteTls, IngressRouteTlsDomains,
};

use kube::{
    Api,
    api::{DeleteParams, ObjectMeta, Patch, PatchParams, PostParams},
};

use redis::AsyncTypedCommands;
use redis::aio::MultiplexedConnection;
use serde_json::json;
use sqlx::PgPool;
use tracing::{Instrument, error, info, info_span, warn};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::kubernetes_service::KubernetesService;
use crate::services::repository::DeploymentRepository;

impl KubernetesService {
    pub async fn init(&self) -> Result<(), AppError> {
        info!("🏁Performing pre-flight infrastructure checks...");

        // Check for ClusterIssuer
        let cluster_issuer_api: Api<ClusterIssuer> = Api::all(self.client.clone());
        match cluster_issuer_api
            .get(&self.cfg.cert_manager.cluster_issuer)
            .await
        {
            Ok(_) => info!(
                "✅ ClusterIssuer '{}' found.",
                self.cfg.cert_manager.cluster_issuer
            ),
            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                error!(
                    "❌ ClusterIssuer '{}' is missing",
                    self.cfg.cert_manager.cluster_issuer
                );
                return Err(AppError::InternalServerError(format!(
                    "ClusterIssuer '{}' is missing. Please apply infrastructure configuration.",
                    self.cfg.cert_manager.cluster_issuer
                )));
            }
            Err(e) => return Err(e.into()),
        }

        // Check for Wildcard Certificate
        let certificate_api: Api<Certificate> =
            Api::namespaced(self.client.clone(), &self.cfg.traefik.namespace);
        match certificate_api
            .get(&self.cfg.cert_manager.wildcard_certificate)
            .await
        {
            Ok(_) => info!(
                "✅ Wildcard Certificate '{}' found.",
                self.cfg.cert_manager.wildcard_certificate
            ),
            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                error!(
                    "❌ Wildcard Certificate '{}' is missing in namespace '{}'.",
                    self.cfg.cert_manager.wildcard_certificate, self.cfg.traefik.namespace
                );
                return Err(AppError::InternalServerError(format!(
                    "Wildcard Certificate '{}' is missing in namespace '{}'.",
                    self.cfg.cert_manager.wildcard_certificate, self.cfg.traefik.namespace
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
    pub async fn create(
        &self,
        pool: PgPool,
        mut con: MultiplexedConnection,
        msg: CreateDeploymentMessage,
    ) -> Result<(), AppError> {
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
            &pool,
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
        let _ = con
            .publish(channel, message.to_string())
            .instrument(info_span!("pubsub.status_update"))
            .await;

        self.create_k8s_resources(msg).await?;

        let query_result =
            DeploymentRepository::update_status(&deployment_id, DeploymentStatus::Starting, &pool)
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
        let namespace = self.ensure_namespace(&msg.user_id).await?;
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
                &msg.deployment_id.to_string(),
                &namespace,
                &resource_name,
                msg.secrets,
            )
            .await?;

        self.create_k8s_deployment(
            &namespace,
            &resource_name,
            &msg.image,
            msg.port,
            msg.desired_replicas,
            &msg.resource_spec,
            secret_name,
            msg.environment_variables,
            &labels,
        )
        .await?;

        self.create_k8s_service(&namespace, &resource_name, msg.port, &labels)
            .await?;

        // ! ERROR, We use Traefik IngressRoute
        self.create_traefik_ingressroute(
            namespace,
            resource_name,
            msg.domain,
            msg.subdomain,
            labels,
        )
        .await?;

        Ok(())
    }

    /// Create VSO resources
    async fn create_vault_static_secret(
        &self,
        deployment_id: &str,
        ns: &str,
        name: &str,
        secrets: Option<HashMap<String, String>>,
    ) -> Result<Option<String>, AppError> {
        // Option::filter is essentially a way to apply an additional condition to an Option that is already known to be Some
        if secrets.clone().filter(|hm| hm.is_empty()).is_none() || secrets.is_none() {
            return Ok(None);
        }

        // Write to Vault
        let secret_path = self
            .vault_service
            .store_secrets(ns, deployment_id, secrets.unwrap())
            .await?;

        let vault_static_secret = VaultStaticSecret {
            metadata: ObjectMeta {
                name: Some(name.to_owned()),
                namespace: Some(ns.to_owned()),
                ..Default::default()
            },
            spec: VaultStaticSecretSpec {
                vault_auth_ref: self.vault_service.cfg.vault_auth.name.clone(),
                mount: self.vault_service.cfg.kv_mount.clone(),
                r#type: VaultStaticSecretType::KvV2,
                path: secret_path,
                destination: VaultStaticSecretDestination {
                    create: Some(true),
                    name: name.to_owned(),
                    ..Default::default()
                },
                // reconcilation interval
                refresh_after: self
                    .vault_service
                    .cfg
                    .vault_static_secret
                    .refresh_after
                    .clone(),
                // Restart the deployment if secrets change
                rollout_restart_targets: Some(vec![VaultStaticSecretRolloutRestartTargets {
                    kind: VaultStaticSecretRolloutRestartTargetsKind::Deployment,
                    name: name.to_string(),
                }]),
                hmac_secret_data: Some(true),
                namespace: Some(ns.to_owned()),
                sync_config: None,
                version: Some(2),
            },
            status: None,
        };

        // APPLY the VSO CRD
        let vault_static_secret_api: Api<VaultStaticSecret> =
            Api::namespaced(self.client.clone(), ns);

        vault_static_secret_api
            .create(&PostParams::default(), &vault_static_secret)
            .instrument(info_span!("create_vault_static_secret"))
            .await
            .map_err(|e| {
                error!(deployment_id=%deployment_id, "Failed to create VSO Secret");
                AppError::InternalServerError(format!("Failed to create VSO Secret: {}", e))
            })?;

        Ok(Some(name.to_owned()))
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
        port: i32,
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
                    target_port: Some(IntOrString::Int(port)),
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
        ns: String,
        name: String,
        domain: Option<String>,
        subdomain: Option<String>,
        labels: BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let mut ingrees_route = IngressRoute {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                namespace: Some(ns.clone()),
                labels: Some(labels),
                ..Default::default()
            },
            spec: IngressRouteSpec {
                entry_points: self.cfg.traefik.entry_points.clone(),
                parent_refs: Some(vec![IngressRouteParentRefs {
                    name: name.clone(),
                    ..Default::default()
                }]),
                tls: Some(IngressRouteTls {
                    cert_resolver: Some(self.cfg.traefik.cluster_issuer.clone()),
                    domains: Some(vec![
                        IngressRouteTlsDomains {
                            main: domain.clone(),
                            sans: None,
                        },
                        IngressRouteTlsDomains {
                            main: subdomain.clone(),
                            sans: None,
                        },
                    ]),
                    secret_name: Some(self.cfg.cert_manager.wildcard_certificate_secret.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        };

        if let Some(domain) = domain {
            ingrees_route.spec.routes.push(IngressRouteRoutes {
                r#match: domain,
                services: Some(vec![IngressRouteRoutesServices {
                    name: name.clone(),
                    ..Default::default()
                }]),
                ..Default::default()
            });
        }

        if let Some(subdomain) = subdomain {
            ingrees_route.spec.routes.push(IngressRouteRoutes {
                r#match: subdomain,
                services: Some(vec![IngressRouteRoutesServices {
                    name,
                    ..Default::default()
                }]),
                ..Default::default()
            });
        }

        let ingressroute_api: Api<IngressRoute> = Api::namespaced(self.client.clone(), &ns);
        ingressroute_api
            .create(&PostParams::default(), &ingrees_route)
            .await?;

        Ok(())
    }

    pub async fn update(
        &self,
        pool: PgPool,
        mut con: MultiplexedConnection,
        message: UpdateDeploymentMessage,
    ) -> Result<(), AppError> {
        let user_id = message.user_id;
        let project_id = message.project_id;
        let deployment_id = message.deployment_id;

        let deployment = DeploymentRepository::get_one_by_id(&deployment_id, &pool).await?;

        // ! We can send pubsub message to notify users via SEE

        let namespace = self.ensure_namespace(&user_id).await?;
        let resource_name = self.format_resource_name(&deployment.id);

        let deployments_api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), &namespace);

        if let Some(replicas) = message.desired_replicas {
            let patch = serde_json::json!({
                "spec": {
                    "replicas": replicas
                }
            });

            deployments_api
                .patch(
                    &resource_name,
                    &PatchParams::default(),
                    &Patch::Strategic(patch),
                )
                .await
                .map_err(|e| {
                    AppError::InternalServerError(format!("Failed to update deployment: {}", e))
                })?;

            // ! We can send pubsub message to notify users via SEE

            let query_result =
                DeploymentRepository::update_replicas(&deployment_id, replicas, &pool).await?;

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

    pub async fn delete(
        &self,
        pool: PgPool,
        message: DeleteDeploymentMessage,
    ) -> Result<(), AppError> {
        let user_id = message.user_id;
        let deployment_id = message.deployment_id;

        let deployment = DeploymentRepository::get_one_by_id(&deployment_id, &pool).await?;
        // ! We can send pubsub message to notify users via SEE

        let ns = self.ensure_namespace(&user_id).await?;
        let name = self.format_resource_name(&deployment.id);

        let delete_params = DeleteParams::default();

        let ingressroute_api: Api<IngressRoute> = Api::namespaced(self.client.clone(), &ns);
        let _ = ingressroute_api.delete(&name, &delete_params).await;

        let service_api: Api<Service> = Api::namespaced(self.client.clone(), &ns);
        let _ = service_api.delete(&name, &delete_params).await;

        let deployment_api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), &ns);
        let _ = deployment_api.delete(&name, &delete_params).await;

        let secret_name = format!("{}-secrets", name);
        let secret_api: Api<K8sSecret> = Api::namespaced(self.client.clone(), &ns);
        let _ = secret_api.delete(&secret_name, &delete_params).await;

        info!("✅ K8s resources deleted for deployment {}", deployment_id);
        Ok(())
    }

    /// creates namespace for user like `user-{user_id[:8]}`
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
        let mut vault_connection = VaultConnection::default();
        vault_connection.metadata.namespace = Some(ns.clone());

        if let Some(con) = &self.vault_service.cfg.vault_connection {
            vault_connection.metadata.name = con.name.clone();
            vault_connection.spec.address = con.address.clone();
            vault_connection.spec.skip_tls_verify = con.skip_tls_verify;
        };

        let mut vault_auth = VaultAuth::default();
        vault_auth.metadata.name = self.vault_service.cfg.vault_auth.name.clone();
        vault_auth.metadata.namespace = Some(ns.clone());
        vault_auth.spec.method = Some(VaultAuthMethod::Kubernetes);
        vault_auth.spec.mount = self.vault_service.cfg.vault_auth.mount.clone();
        vault_auth.spec.vault_connection_ref = vault_connection.clone().metadata.name;
        vault_auth.spec.kubernetes = Some(VaultAuthKubernetes {
            role: self.vault_service.cfg.vault_auth.k8s.role.clone(),
            service_account: self
                .vault_service
                .cfg
                .vault_auth
                .k8s
                .service_account
                .clone(),
            ..Default::default()
        });

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
