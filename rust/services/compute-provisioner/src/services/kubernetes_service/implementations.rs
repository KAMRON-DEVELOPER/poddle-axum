use std::collections::{BTreeMap, HashMap};

use base64::Engine;
use chrono::Utc;
use compute_core::models::{DeploymentStatus, ResourceSpec};
use compute_core::schemas::ImagePullSecret;
use compute_core::{
    channel_names::ChannelNames,
    schemas::{CreateDeploymentMessage, DeleteDeploymentMessage, UpdateDeploymentMessage},
};
use k8s_openapi::ByteString;
use k8s_openapi::api::core::v1::{EnvFromSource, LocalObjectReference, SecretEnvSource};
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
    IngressRoute, IngressRouteRoutes, IngressRouteRoutesServices, IngressRouteSpec,
    IngressRouteTls, IngressRouteTlsDomains,
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
    pub async fn preflight(&self) -> Result<(), AppError> {
        info!("🏁 Performing pre-flight infrastructure checks...");

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

    // --------------------------------------------------------------------------------------------
    // create
    // --------------------------------------------------------------------------------------------

    /// Starting point, update status to `provisioning` in DB, whetever updated or not in DB it user notified by `SEE` thrugh `pubsub`
    /// `create_resources` called and again status updated to `starting` and user notified
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
            // TODO We might change later
            let channel = ChannelNames::project_metrics(&project_id.to_string());
            let now = Utc::now().timestamp();
            let message = json!({
                "type": "message",
                "message": "Internal server error",
                "deployment_id": &deployment_id,
                "timestamp": now,
            });
            let _ = con
                .publish(channel, message.to_string())
                .instrument(info_span!("pubsub.message"))
                .await;
            warn!(
                user_id = %user_id,
                project_id = %project_id,
                deployment_id = %deployment_id,
                "❌ Update deployment status affected zero rows"
            );
        }

        // TODO We might change later
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

        self.create_resources(msg).await?;

        let query_result =
            DeploymentRepository::update_status(&deployment_id, DeploymentStatus::Running, &pool)
                .await?;

        if query_result.rows_affected() == 0 {
            // TODO We might change later
            let channel = ChannelNames::project_metrics(&project_id.to_string());
            let now = Utc::now().timestamp();
            let message = json!({
                "type": "message",
                "message": "Internal server error",
                "deployment_id": &deployment_id,
                "timestamp": now,
            });
            let _ = con
                .publish(channel, message.to_string())
                .instrument(info_span!("pubsub.message"))
                .await;
            warn!(
                user_id = %user_id,
                project_id = %project_id,
                deployment_id = %deployment_id,
                "❌ Update deployment status affected zero rows"
            );
        }

        // TODO We might change later
        let channel = ChannelNames::project_metrics(&project_id.to_string());
        let now = Utc::now().timestamp();
        let message = json!({
            "type": "status_update",
            "status": "running",
            "deployment_id": &deployment_id,
            "timestamp": now,
        });
        let _ = con
            .publish(channel, message.to_string())
            .instrument(info_span!("pubsub.status_update"))
            .await;

        info!("✅ K8s resources created for deployment {}", deployment_id);

        Ok(())
    }

    /// Prepares `ns` and `name` for resources, using same `name` for different resources is not problem
    /// Creates VSO resources(`VaultConnection` & `VaultAuth`), `vault static secret`, `deployment`, `service`, `ingressroute`
    #[tracing::instrument(
        name = "kubernetes_service.create_resources",
        skip_all,
        fields(user_id = %msg.user_id, project_id = %msg.project_id, deployment_id = %msg.deployment_id),
        err
    )]
    async fn create_resources(&self, msg: CreateDeploymentMessage) -> Result<(), AppError> {
        let ns = self.ensure_namespace(&msg.user_id).await?;
        let name = self.format_resource_name(&msg.deployment_id);

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
        labels.insert("poddle.io/preset-id".to_string(), msg.preset_id.to_string());

        self.create_vso_resources(&ns).await?;

        let secret_ref = self
            .create_vault_static_secret(&msg.deployment_id.to_string(), &ns, &name, msg.secrets)
            .await?;

        let image_pull_secret = match msg.image_pull_secret.as_ref() {
            Some(secret) => Some(self.create_image_pull_secret(&ns, &name, secret).await?),
            None => None,
        };

        let otel_service_name = msg.name.as_ref();
        let otel_resource_attributes = format!(
            "project_id={},deployment_id={},managed_by=poddle",
            msg.project_id, msg.deployment_id
        );

        self.create_deployment(
            otel_service_name,
            &otel_resource_attributes,
            &ns,
            &name,
            &msg.image,
            image_pull_secret,
            msg.port,
            msg.desired_replicas,
            &msg.resource_spec,
            secret_ref,
            msg.environment_variables,
            &labels,
        )
        .await?;

        self.create_service(&ns, &name, msg.port, &labels).await?;

        self.create_traefik_ingressroute(
            ns,
            name,
            msg.domain,
            msg.subdomain,
            Some(IntOrString::Int(msg.port)),
            labels,
        )
        .await?;

        Ok(())
    }

    #[tracing::instrument(name = "kubernetes_service.ensure_namespace", skip_all, fields(user_id = %user_id), err)]
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

        let api: Api<Namespace> = Api::all(self.client.clone());

        match api.get(&name).await {
            Ok(_) => return Ok(name),

            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                info!(user_id = %user_id, "Creating namespace {}", name);
            }

            Err(e) => {
                error!(
                    user_id = %user_id,
                    error = %e,
                    "Failed to check namespace existence"
                );
                return Err(AppError::InternalServerError(format!(
                    "Kubernetes API unavailable while checking namespace: {}",
                    e
                )));
            }
        }

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

        api.create(&PostParams::default(), &new_ns)
            .await
            .map_err(|e| {
                error!(
                    user_id = %user_id,
                    error = %e,
                    "Failed to create namespace"
                );
                AppError::InternalServerError(format!(
                    "Failed to create namespace '{}': {}",
                    name, e
                ))
            })?;

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

    /// Creates VaultConnection & VaultAuth
    #[tracing::instrument(name = "kubernetes_service.create_vso_resources", skip_all, err)]
    async fn create_vso_resources(&self, ns: &str) -> Result<(), AppError> {
        let mut vault_connection = VaultConnection::default();
        vault_connection.metadata.namespace = Some(ns.to_owned());

        if let Some(con) = &self.vault_service.cfg.vault_connection {
            vault_connection.metadata.name = con.name.clone();
            vault_connection.spec.address = con.address.clone();
            vault_connection.spec.skip_tls_verify = con.skip_tls_verify;
        };

        let mut vault_auth = VaultAuth::default();
        vault_auth.metadata.name = self.vault_service.cfg.vault_auth.name.clone();
        vault_auth.metadata.namespace = Some(ns.to_owned());
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

    /// Create VSO resources
    #[tracing::instrument(name = "kubernetes_service.create_vault_static_secret", skip_all, fields(deployment_id = %deployment_id), err)]
    async fn create_vault_static_secret(
        &self,
        deployment_id: &str,
        ns: &str,
        name: &str,
        secrets: Option<HashMap<String, String>>,
    ) -> Result<Option<String>, AppError> {
        let secrets = match secrets {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(None),
        };

        let secret_name = format!("{}-secrets", name);

        // Write to Vault
        let path = self
            .vault_service
            .store_secrets(ns, deployment_id, secrets)
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
                path,
                destination: VaultStaticSecretDestination {
                    create: Some(true),
                    name: secret_name.clone(),
                    ..Default::default()
                },
                refresh_after: self
                    .vault_service
                    .cfg
                    .vault_static_secret
                    .refresh_after
                    .clone(),
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

        let api: Api<VaultStaticSecret> = Api::namespaced(self.client.clone(), ns);

        api.create(&PostParams::default(), &vault_static_secret)
            .instrument(info_span!("create_vault_static_secret"))
            .await
            .map_err(|e| {
                error!(deployment_id=%deployment_id, "Failed to create VSO Secret");
                AppError::InternalServerError(format!("Failed to create VSO Secret: {}", e))
            })?;

        Ok(Some(secret_name))
    }

    /// Create image pull secret
    #[tracing::instrument(name = "kubernetes_service.create_image_pull_secret", skip_all, err)]
    async fn create_image_pull_secret(
        &self,
        ns: &str,
        name: &str,
        creds: &ImagePullSecret,
    ) -> Result<String, AppError> {
        let name = format!("{}-registry", name);

        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", creds.username, creds.secret));

        let dockerconfig = serde_json::json!({
            "auths": {
                creds.server.clone(): {
                    "username": creds.username,
                    "password": creds.secret,
                    "auth": auth
                }
            }
        });

        let data = Some(BTreeMap::from([(
            ".dockerconfigjson".to_string(),
            ByteString(
                base64::engine::general_purpose::STANDARD
                    .encode(dockerconfig.to_string())
                    .into_bytes(),
            ),
        )]));

        let secret = K8sSecret {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                namespace: Some(ns.to_string()),
                ..Default::default()
            },
            type_: Some("kubernetes.io/dockerconfigjson".to_string()),
            data,
            ..Default::default()
        };

        let api: Api<K8sSecret> = Api::namespaced(self.client.clone(), ns);
        api.create(&PostParams::default(), &secret)
            .await
            .map_err(|e| {
                error!(ns = %ns, "Failed to create Image Pull Secret");
                AppError::InternalServerError(format!("Failed to create Image Pull Secret: {}", e))
            })?;

        Ok(name)
    }

    /// Create Kubernetes Deployment
    #[tracing::instrument(
        name = "kubernetes_service.create_deployment",
        skip_all,
        fields(ns = %ns, image = %image, port = %port, desired_replicas = %desired_replicas, resource_spec = %resource_spec),
        err
    )]
    async fn create_deployment(
        &self,
        otel_service_name: &str,
        otel_resource_attributes: &str,
        ns: &str,
        name: &str,
        image: &str,
        image_pull_secret: Option<String>,
        port: i32,
        desired_replicas: i32,
        resource_spec: &ResourceSpec,
        secret_ref: Option<String>,
        environment_variables: Option<HashMap<String, String>>,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), ns);

        // Build environment variables
        let mut env = vec![
            EnvVar {
                name: "OTEL_SERVICE_NAME".to_owned(),
                value: Some(otel_service_name.to_string()),
                ..Default::default()
            },
            EnvVar {
                name: "OTEL_EXPORTER_OTLP_PROTOCOL".to_owned(),
                value: Some("grpc".to_string()),
                ..Default::default()
            },
            EnvVar {
                name: "OTEL_RESOURCE_ATTRIBUTES".to_owned(),
                value: Some(otel_resource_attributes.to_string()),
                ..Default::default()
            },
            EnvVar {
                name: "OTEL_EXPORTER_OTLP_ENDPOINT".to_owned(),
                value: self.cfg.otel_exporter_otlp_endpoint.clone(),
                ..Default::default()
            },
        ];
        if let Some(environment_variables) = environment_variables {
            for (key, value) in environment_variables {
                env.push(EnvVar {
                    name: key.clone(),
                    value: Some(value.clone()),
                    ..Default::default()
                });
            }
        }

        let mut env_from: Vec<EnvFromSource> = vec![];
        if let Some(secret_ref) = secret_ref {
            env_from.push(EnvFromSource {
                secret_ref: Some(SecretEnvSource {
                    name: secret_ref,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }

        // Resource requirements
        let mut requests = BTreeMap::new();
        requests.insert(
            "cpu".to_string(),
            Quantity(format!("{}m", resource_spec.cpu_request_millicores)),
        );
        requests.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", resource_spec.memory_request_mb)),
        );

        let mut limits = BTreeMap::new();
        limits.insert(
            "cpu".to_string(),
            Quantity(format!("{}m", resource_spec.cpu_limit_millicores)),
        );
        limits.insert(
            "memory".to_string(),
            Quantity(format!("{}Mi", resource_spec.memory_limit_mb)),
        );

        let image_pull_secrets = image_pull_secret.map(|name| vec![LocalObjectReference { name }]);

        let deployment = K8sDeployment {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(ns.to_string()),
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
                        image_pull_secrets,
                        containers: vec![Container {
                            name: name.to_string(),
                            image: Some(image.to_string()),
                            image_pull_policy: Some("IfNotPresent".to_string()),
                            ports: Some(vec![ContainerPort {
                                container_port: port,
                                // Must be UDP, TCP, or SCTP. Defaults to "TCP".
                                protocol: Some("TCP".to_string()),
                                ..Default::default()
                            }]),
                            env: Some(env),
                            env_from: Some(env_from),
                            resources: Some(ResourceRequirements {
                                requests: Some(requests),
                                limits: Some(limits),
                                ..Default::default()
                            }),
                            liveness_probe: None,
                            readiness_probe: None,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        api.create(&PostParams::default(), &deployment)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create K8s deployment: {}", e))
            })?;

        info!("Deployment {} created in namespace {}", name, ns);
        Ok(())
    }

    /// Create Service
    #[tracing::instrument(name = "kubernetes_service.create_service", skip_all, err)]
    async fn create_service(
        &self,
        ns: &str,
        name: &str,
        port: i32,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let api: Api<Service> = Api::namespaced(self.client.clone(), ns);

        let service = Service {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(ns.to_string()),
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
                // Valid options are ExternalName, ClusterIP, NodePort, and LoadBalancer
                type_: Some("ClusterIP".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        api.create(&PostParams::default(), &service)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to create service: {}", e))
            })?;

        info!("Service {} created in namespace {}", name, ns);
        Ok(())
    }

    /// Create Traefik IngressRoute
    #[tracing::instrument(name = "kubernetes_service.create_traefik_ingressroute", skip_all, err)]
    async fn create_traefik_ingressroute(
        &self,
        ns: String,
        name: String,
        domain: Option<String>,
        subdomain: Option<String>,
        port: Option<IntOrString>,
        labels: BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let mut routes = Vec::new();
        let mut tls_domains = Vec::new();

        // Uses Default TLSStore (Wildcard)
        // We create wildcard secret from using cert-manager
        // In Local we create wildcard secret using Vault PKI or self signed, in Prod created by Let's Encrypt
        if let Some(sub) = subdomain {
            let full_subdomain = format!("{}.{}", sub, self.cfg.traefik.base_domain);
            routes.push(IngressRouteRoutes {
                r#match: format!("Host(`{}`)", full_subdomain),
                services: Some(vec![IngressRouteRoutesServices {
                    name: name.clone(),
                    port: port.clone(),
                    ..Default::default()
                }]),
                ..Default::default()
            });

            tls_domains.push(IngressRouteTlsDomains {
                main: Some(full_subdomain),
                sans: None,
            });
        }

        // Uses CertResolver (Traefik native, Let's Encrypt)
        if let Some(user_domain) = domain {
            routes.push(IngressRouteRoutes {
                r#match: format!("Host(`{}`)", user_domain),
                services: Some(vec![IngressRouteRoutesServices {
                    name: name.clone(),
                    port: port.clone(),
                    ..Default::default()
                }]),
                ..Default::default()
            });
            tls_domains.push(IngressRouteTlsDomains {
                main: Some(user_domain),
                sans: None,
            });
        }

        if routes.is_empty() {
            return Ok(());
        }

        let ingress_route = IngressRoute {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                namespace: Some(ns.clone()),
                labels: Some(labels),
                ..Default::default()
            },
            spec: IngressRouteSpec {
                entry_points: self.cfg.traefik.entry_points.clone(),
                routes, // Pass the dynamically built vectors
                tls: Some(IngressRouteTls {
                    // This uses "letsencrypt"
                    // Traefik will use this resolver for domains that don't match the TLSStore.
                    cert_resolver: Some(self.cfg.cert_manager.cluster_issuer.clone()),
                    domains: Some(tls_domains),
                    // We set secret_name to NONE.
                    // - Subdomains will match the Wildcard in the Default TLSStore automatically.
                    // - Custom Domains will trigger the cert_resolver.
                    ..Default::default()
                }),
                ..Default::default()
            },
        };

        let api: Api<IngressRoute> = Api::namespaced(self.client.clone(), &ns);
        // Patch allow to Create or Update in one go
        api.patch(
            &name,
            &PatchParams::apply("poddle").force(),
            &Patch::Apply(&ingress_route),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, "Failed to create IngressRoute");
            AppError::InternalServerError(format!("Failed to create IngressRoute: {}", e))
        })?;

        Ok(())
    }

    // --------------------------------------------------------------------------------------------
    // update
    // --------------------------------------------------------------------------------------------

    /// Starting point
    #[tracing::instrument(
        name = "kubernetes_service.update",
        skip_all,
        fields(user_id = %msg.user_id, project_id = %msg.project_id, deployment_id = %msg.deployment_id),
        err
    )]
    pub async fn update(
        &self,
        pool: PgPool,
        mut con: MultiplexedConnection,
        msg: UpdateDeploymentMessage,
    ) -> Result<(), AppError> {
        let user_id = msg.user_id;
        let project_id = msg.project_id;
        let deployment_id = msg.deployment_id;

        let ns = self.ensure_namespace(&user_id).await?;
        let name = self.format_resource_name(&deployment_id);

        info!(
            user_id = %user_id,
            project_id = %project_id,
            deployment_id = %deployment_id,
            "🔄 Updating K8s resources"
        );

        // Notify user
        let channel = ChannelNames::project_metrics(&project_id.to_string());
        let now = Utc::now().timestamp();
        let message = json!({
            "type": "status_update",
            "status": "updating",
            "deployment_id": &deployment_id,
            "timestamp": now,
        });
        let _ = con.publish(channel, message.to_string()).await;

        // Update Deployment (replicas, image, port, resource_spec, image_pull_secret)
        if msg.desired_replicas.is_some()
            || msg.image.is_some()
            || msg.port.is_some()
            || msg.resource_spec.is_some()
            || msg.image_pull_secret.is_some()
        {
            self.update_deployment(
                &ns,
                &name,
                msg.image.as_deref(),
                msg.image_pull_secret.as_ref(),
                msg.port,
                msg.desired_replicas,
                msg.resource_spec.as_ref(),
            )
            .await?;
        }

        // Update Service (port)
        if let Some(port) = msg.port {
            self.update_service(&ns, &name, port).await?;
        }

        // Update IngressRoute (domain, subdomain)
        if msg.domain.is_some() || msg.subdomain.is_some() {
            self.update_ingressroute(&ns, &name, msg.domain, msg.subdomain)
                .await?;
        }

        // Update status back to 'healthy'
        let query_result =
            DeploymentRepository::update_status(&deployment_id, DeploymentStatus::Running, &pool)
                .await?;

        if query_result.rows_affected() == 0 {
            let channel = ChannelNames::project_metrics(&project_id.to_string());
            let now = Utc::now().timestamp();
            let message = json!({
                "type": "message",
                "message": "Internal server error",
                "deployment_id": &deployment_id,
                "timestamp": now,
            });
            let _ = con.publish(channel, message.to_string()).await;
        }

        let channel = ChannelNames::project_metrics(&project_id.to_string());
        let now = Utc::now().timestamp();
        let message = json!({
            "type": "status_update",
            "status": "running",
            "deployment_id": &deployment_id,
            "timestamp": now,
        });
        let _ = con.publish(channel, message.to_string()).await;

        info!("✅ K8s resources updated for deployment {}", deployment_id);

        Ok(())
    }

    #[tracing::instrument(name = "kubernetes_service.update_deployment", skip_all, err)]
    async fn update_deployment(
        &self,
        ns: &str,
        name: &str,
        image: Option<&str>,
        image_pull_secret: Option<&ImagePullSecret>,
        port: Option<i32>,
        desired_replicas: Option<i32>,
        resource_spec: Option<&ResourceSpec>,
    ) -> Result<(), AppError> {
        let api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), ns);

        let mut patch = serde_json::json!({});

        // Update replicas
        if let Some(replicas) = desired_replicas {
            patch["spec"]["replicas"] = json!(replicas);
        }

        // Update image
        if let Some(image) = image {
            patch["spec"]["template"]["spec"]["containers"] = json!([{
                "name": name,
                "image": image
            }]);
        }

        // Update port
        if let Some(port) = port {
            if patch["spec"]["template"]["spec"]["containers"].is_null() {
                patch["spec"]["template"]["spec"]["containers"] = json!([{
                    "name": name,
                    "ports": [{
                        "containerPort": port,
                        "protocol": "TCP"
                    }]
                }]);
            } else {
                patch["spec"]["template"]["spec"]["containers"][0]["ports"] = json!([{
                    "containerPort": port,
                    "protocol": "TCP"
                }]);
            }
        }

        // Update resource spec
        if let Some(resource_spec) = resource_spec {
            let requests = json!({
                "cpu": format!("{}m", resource_spec.cpu_request_millicores),
                "memory": format!("{}Mi", resource_spec.memory_request_mb)
            });

            let limits = json!({
                "cpu": format!("{}m", resource_spec.cpu_limit_millicores),
                "memory": format!("{}Mi", resource_spec.memory_limit_mb)
            });

            if patch["spec"]["template"]["spec"]["containers"].is_null() {
                patch["spec"]["template"]["spec"]["containers"] = json!([{
                    "name": name,
                    "resources": {
                        "requests": requests,
                        "limits": limits
                    }
                }]);
            } else {
                patch["spec"]["template"]["spec"]["containers"][0]["resources"] = json!({
                    "requests": requests,
                    "limits": limits
                });
            }
        }

        // Update image pull secret
        if let Some(creds) = image_pull_secret {
            let secret_name = format!("{}-registry", name);

            // First, update or create the secret
            self.create_image_pull_secret(ns, name, creds).await?;

            // Then patch the deployment to use it
            patch["spec"]["template"]["spec"]["imagePullSecrets"] = json!([{
                "name": secret_name
            }]);
        }

        api.patch(
            name,
            &PatchParams::apply("poddle").force(),
            &Patch::Strategic(patch),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, name=%name, "Failed to update deployment");
            AppError::InternalServerError(format!("Failed to update deployment: {}", e))
        })?;

        info!("Deployment {} updated in namespace {}", name, ns);
        Ok(())
    }

    #[tracing::instrument(name = "kubernetes_service.update_service", skip_all, err)]
    async fn update_service(&self, ns: &str, name: &str, port: i32) -> Result<(), AppError> {
        let api: Api<Service> = Api::namespaced(self.client.clone(), ns);

        let patch = json!({
            "spec": {
                "ports": [{
                    "name": "http",
                    "port": 80,
                    "targetPort": port,
                    "protocol": "TCP"
                }]
            }
        });

        api.patch(
            name,
            &PatchParams::apply("poddle").force(),
            &Patch::Strategic(patch),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, name=%name, "Failed to update service");
            AppError::InternalServerError(format!("Failed to update service: {}", e))
        })?;

        info!("Service {} updated in namespace {}", name, ns);
        Ok(())
    }

    #[tracing::instrument(name = "kubernetes_service.update_ingressroute", skip_all, err)]
    async fn update_ingressroute(
        &self,
        ns: &str,
        name: &str,
        domain: Option<String>,
        subdomain: Option<String>,
    ) -> Result<(), AppError> {
        let api: Api<IngressRoute> = Api::namespaced(self.client.clone(), ns);

        let mut routes = vec![];
        let mut domains = vec![];

        if let Some(domain) = &domain {
            routes.push(IngressRouteRoutes {
                r#match: domain.clone(),
                services: Some(vec![IngressRouteRoutesServices {
                    name: name.to_string(),
                    ..Default::default()
                }]),
                ..Default::default()
            });
            domains.push(IngressRouteTlsDomains {
                main: Some(domain.clone()),
                sans: None,
            });
        }

        if let Some(subdomain) = &subdomain {
            routes.push(IngressRouteRoutes {
                r#match: subdomain.clone(),
                services: Some(vec![IngressRouteRoutesServices {
                    name: name.to_string(),
                    ..Default::default()
                }]),
                ..Default::default()
            });
            domains.push(IngressRouteTlsDomains {
                main: Some(subdomain.clone()),
                sans: None,
            });
        }

        let patch = json!({
            "spec": {
                "routes": routes,
                "tls": {
                    "domains": domains
                }
            }
        });

        api.patch(
            name,
            &PatchParams::apply("poddle").force(),
            &Patch::Merge(patch),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, name=%name, "Failed to update IngressRoute");
            AppError::InternalServerError(format!("Failed to update IngressRoute: {}", e))
        })?;

        info!("IngressRoute {} updated in namespace {}", name, ns);
        Ok(())
    }

    // --------------------------------------------------------------------------------------------
    // update
    // --------------------------------------------------------------------------------------------

    /// Starting point
    pub async fn delete(&self, msg: DeleteDeploymentMessage) -> Result<(), AppError> {
        let user_id = msg.user_id;
        let deployment_id = msg.deployment_id;

        let ns = self.ensure_namespace(&user_id).await?;
        let name = self.format_resource_name(&deployment_id);

        let dp = DeleteParams::default();

        let ingressroute_api: Api<IngressRoute> = Api::namespaced(self.client.clone(), &ns);
        let _ = ingressroute_api.delete(&name, &dp).await;

        let service_api: Api<Service> = Api::namespaced(self.client.clone(), &ns);
        let _ = service_api.delete(&name, &dp).await;

        let deployment_api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), &ns);
        let _ = deployment_api.delete(&name, &dp).await;

        let vault_static_secret_api: Api<VaultStaticSecret> =
            Api::namespaced(self.client.clone(), &ns);
        let _ = vault_static_secret_api.delete(&name, &dp).await;

        let secret_name = format!("{}-secrets", name);
        let secret_api: Api<K8sSecret> = Api::namespaced(self.client.clone(), &ns);
        let _ = secret_api.delete(&secret_name, &dp).await;

        let secret_name = format!("{}-registry", name);
        let secret_api: Api<K8sSecret> = Api::namespaced(self.client.clone(), &ns);
        let _ = secret_api.delete(&secret_name, &dp).await;

        info!("✅ K8s resources deleted for deployment {}", deployment_id);
        Ok(())
    }
}
