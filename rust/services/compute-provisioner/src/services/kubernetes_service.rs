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
            TypedLocalObjectReference,
        },
        networking::v1::{
            HTTPIngressPath, HTTPIngressRuleValue, Ingress, IngressBackend, IngressRule,
            IngressServiceBackend, IngressSpec, IngressTLS, ServiceBackendPort,
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
use kcr_traefik_io::v1alpha1::ingressroutes::{IngressRoute, IngressRouteSpec};

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
        let deployment_namespace = self.get_namespace_or_create(&msg.user_id).await?;
        let deployment_name = self.get_deployment_name(&msg.deployment_id);

        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), deployment_name.to_string());
        labels.insert("project-id".to_string(), msg.project_id.to_string());
        labels.insert("deployment-id".to_string(), msg.deployment_id.to_string());
        labels.insert("managed-by".to_string(), "poddle".to_string());

        let mut secret_name = self.create_vso_resources(msg.secrets).await?;

        self.create_k8s_deployment(
            deployment_namespace,
            deployment_name,
            &image,
            port,
            replicas,
            &msg.resource_spec,
            secret_name.as_deref(),
            &msg.environment_variables,
            &labels,
        )
        .await?;

        self.create_k8s_service(&deployment_namespace, &deployment_name, msg.port, &labels)
            .await?;

        // ! ERROR, We use Traefik IngressRoute
        self.create_k8s_ingress(
            &deployment_namespace,
            &deployment_name,
            msg.domain.as_deref(),
            msg.subdomain.as_deref(),
            &labels,
        )
        .await?;

        Ok(())
    }

    /// Create VSO resources
    async fn create_vso_resources(
        &self,
        deployment_namespace: &str,
        deployment_name: &str,
        deployment_id: &str,
        secrets: Option<HashMap<String, String>>,
        refresh_after: Option<String>,
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
                // ! TODO we could define vault-auth first...
                vault_auth_ref: Some("vault-auth".to_string()),
                mount: self.vault_service.kv_mount,
                r#type: VaultStaticSecretType::KvV2,
                path: secret_path,
                destination: VaultStaticSecretDestination {
                    create: Some(true),
                    name: secret_name,
                    ..Default::default()
                },
                // reconcilation interval
                refresh_after,
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
                error!("Failed to create VSO Secret");
                AppError::InternalServerError(format!("Failed to create VSO Secret: {}", e))
            })?;

        Ok(Some("".to_string()))
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
        container_port: i32,
        replicas: i32,
        resource_spec: &ResourceSpec,
        secret_name: Option<&str>,
        environment_variables: Option<&HashMap<String, String>>,
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
    async fn create_k8s_ingress_route(
        &self,
        namespace: &str,
        name: &str,
        subdomain: &str,
        subdomain: &str,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let ingrees_route = IngressRoute {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                annotations: None,
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
        subdomain: Option<&str>,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let ingress_api: Api<Ingress> = Api::namespaced(self.client.clone(), deployment_namespace);

        let mut annotations = BTreeMap::new();

        annotations.insert(
            "traefik.ingress.kubernetes.io/router.tls".to_string(),
            "true".to_string(),
        );
        annotations.insert(
            "traefik.ingress.kubernetes.io/router.entrypoints".to_string(),
            "web,websecure".to_string(),
        );
        annotations.insert(
            "traefik.ingress.kubernetes.io/router.middlewares".to_string(),
            "default-permanent-redirect-middleware@kubernetescrd".to_string(),
        );

        let mut ingress = Ingress {
            metadata: ObjectMeta {
                name: Some(deployment_name.to_string()),
                namespace: Some(deployment_namespace.to_string()),
                labels: Some(labels.clone()),
                annotations: Some(annotations),
                ..Default::default()
            },
            spec: Some(IngressSpec {
                default_backend: Some(IngressBackend {
                    resource: Some(TypedLocalObjectReference {
                        api_group: todo!(),
                        kind: todo!(),
                        name: todo!(),
                    }),
                    service: Some(IngressServiceBackend {
                        name: todo!(),
                        port: todo!(),
                    }),
                }),
                ingress_class_name: self.ingress_class_name,
                ..Default::default()
            }),
            ..Default::default()
        };

        // * subdomain
        if let Some(subdomain) = subdomain {
            let host = format!("{}.{}", subdomain, self.domain);

            let ingress_rule = IngressRule {
                host: Some(host),
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
            };

            let tls = IngressTLS {
                hosts: Some(vec![host.to_string()]),
                secret_name: Some(self.wildcard_certificate_secret_name),
            };

            let ingress_spec = ingress.spec.get_or_insert_with(Default::default);
            let ingress_rules = ingress_spec.rules.get_or_insert_with(Vec::new);
            let ingress_tls = ingress_spec.tls.get_or_insert_with(Vec::new);
            ingress_rules.push(ingress_rule);
            ingress_tls.push(tls);
        }

        // * subdomain
        if let Some(subdomain) = subdomain {
            // ! we add cert-manager.io/cluster-issuer annotation and secretName to autogenerated
            let ingress_annotations = ingress
                .metadata
                .annotations
                .get_or_insert_with(BTreeMap::new);
            ingress_annotations.insert(
                "cert-manager.io/cluster-issuer".to_string(),
                self.cluster_issuer_name.clone(),
            );
        }

        ingress_api
            .create(&PostParams::default(), &ingress)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create ingress: {}", e))
            })?;

        info!(
            "Ingress {} created in namespace {}",
            deployment_name, deployment_namespace
        );
        Ok(())
    }

    pub async fn update(&self, message: UpdateDeploymentMessage) -> Result<(), AppError> {
        let user_id = message.user_id;
        let project_id = message.project_id;
        let deployment_id = message.deployment_id;

        let deployment = DeploymentRepository::get_one_by_id(&deployment_id, &self.pool).await?;

        // ! We can send pubsub message to notify users via SEE

        let namespace = self.get_namespace_or_create(user_id).await?;
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

        let namespace = self.get_namespace_or_create(user_id).await?;
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
    async fn get_namespace_or_create(&self, user_id: &Uuid) -> Result<String, AppError> {
        let user_id: String = user_id.as_simple().to_string().chars().take(16).collect();
        let ns_string = format!("user-{}", &user_id);
        let ns_api: Api<Namespace> = Api::all(self.client.clone());

        if ns_api.get(&ns_string).await.is_ok() {
            return Ok(ns_string);
        }

        info!("Creating namespace {}", ns_string);
        let mut labels = BTreeMap::new();
        labels.insert("user-id".to_string(), user_id.to_string());

        let new_ns = Namespace {
            metadata: ObjectMeta {
                name: Some(ns_string.clone()),

                labels: Some(labels),

                ..Default::default()
            },

            ..Default::default()
        };

        ns_api.create(&PostParams::default(), &new_ns).await?;
        info!("Namespace {} created successfully", ns_string);

        // Create VaultAuth and VaultConnection for the tenant
        let vault_connection_api: Api<VaultConnection> =
            Api::namespaced(self.client.clone(), &ns_string);
        let vault_auth_api: Api<VaultAuth> = Api::namespaced(self.client.clone(), &ns_string);

        let vault_connection = VaultConnection {
            metadata: ObjectMeta {
                name: Some(self.vault_service.vault_connection),
                namespace: Some(ns_string.clone()),
                ..Default::default()
            },
            spec: VaultConnectionSpec {
                address: self.vault_service.address.clone(),
                skip_tls_verify: self.vault_service.skip_tls_verify,
                ..Default::default()
            },
            ..Default::default()
        };

        let vault_auth = VaultAuth {
            metadata: ObjectMeta {
                name: Some("vault-auth".to_string()),
                namespace: Some(ns_string.clone()),
                ..Default::default()
            },
            spec: VaultAuthSpec {
                method: Some(VaultAuthMethod::Kubernetes),
                mount: Some("kubernetes".to_string()),
                kubernetes: Some(VaultAuthKubernetes {
                    role: Some("poddle-user-app".to_string()),
                    service_account: Some("default".to_string()),
                    ..Default::default()
                }),
                vault_connection_ref: Some("vault-connection".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        vault_auth_api
            .create(&PostParams::default(), &vault_auth)
            .instrument(info_span!("create_vault_api"))
            .await
            .map_err(|e| {
                error!(user_id=%user_id, "Failed to create VaultAuth");
                AppError::InternalServerError(format!("Failed to create VaultAuth: {}", e))
            })?;
        vault_connection_api
            .create(&PostParams::default(), &vault_connection)
            .instrument(info_span!("create_vault_connection"))
            .await
            .map_err(|e| {
                error!(user_id=%user_id, "Failed to create VaultConnection");
                AppError::InternalServerError(format!("Failed to create VaultConnection: {}", e))
            })?;

        info!(user_id=%user_id, "VaultAuth and VaultConnection created in {}", ns_string);
        Ok(ns_string)
    }

    fn get_deployment_name(&self, deployment_id: &Uuid) -> String {
        format!(
            "deployment-{}",
            deployment_id
                .as_simple()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        )
    }
}
