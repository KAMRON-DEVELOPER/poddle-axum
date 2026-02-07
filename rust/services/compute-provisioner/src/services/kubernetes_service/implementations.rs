use std::collections::{BTreeMap, HashMap};

use base64::Engine;
use compute_core::event::ComputeEvent;
use compute_core::formatters::{format_namespace, format_resource_name};
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

    // ============================================================================================
    // PUBLIC HANDLERS
    // ============================================================================================

    /// Handles "Create" messages.
    /// We wrap mandatory fields in `Some()` and pass them to the `apply_*` functions.
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
        self.notify_status(
            &msg.project_id,
            &msg.deployment_id,
            DeploymentStatus::Provisioning,
            &mut con,
        )
        .await?;

        let user_id = msg.user_id.clone();
        let project_id = msg.project_id.clone();
        let deployment_id = msg.deployment_id.clone();

        info!(
            user_id = %user_id,
            project_id = %project_id,
            deployment_id = %deployment_id,
            "🚀 Creating deployment"
        );

        let ns = self.ensure_namespace(&msg.user_id).await?;
        let name = format_resource_name(&msg.deployment_id);

        let mut labels = BTreeMap::new();
        labels.insert("poddle.io/managed-by".into(), "poddle".into());
        labels.insert("poddle.io/project-id".into(), msg.project_id.into());
        labels.insert("poddle.io/deployment-id".into(), msg.deployment_id.into());
        labels.insert("poddle.io/preset-id".into(), msg.preset_id.into());

        self.create_vso_resources(&ns).await?;

        let secret_ref = self
            .create_vault_static_secret(&msg.deployment_id.to_string(), &ns, &name, msg.secrets)
            .await?;

        let image_pull_secret = match msg.image_pull_secret.as_ref() {
            Some(secret) => Some(self.apply_image_pull_secret(&ns, &name, secret).await?),
            None => None,
        };

        let otel_service_name = msg.name;
        let otel_resource_attributes = format!(
            "project_id={},deployment_id={},managed_by=poddle",
            msg.project_id, msg.deployment_id
        );

        self.apply_deployment(
            Some(otel_service_name.as_ref()),
            Some(&otel_resource_attributes),
            &ns,
            &name,
            Some(&msg.image),
            image_pull_secret,
            Some(msg.port),
            Some(msg.desired_replicas),
            Some(&msg.resource_spec),
            secret_ref,
            msg.environment_variables,
            Some(&labels),
        )
        .await?;

        // TODO we need to pass selector
        self.apply_service(&ns, &name, msg.port).await?;

        // Pass None for domain/subdomain if they don't exist, logic handles it inside
        self.apply_ingressroute(&ns, &name, msg.domain, msg.subdomain, msg.port)
            .await?;

        // 3. Finalize
        self.finalize_status(
            &msg.project_id,
            &msg.deployment_id,
            DeploymentStatus::Running,
            &pool,
            &mut con,
        )
        .await?;
        info!("✅ Created deployment {}", msg.deployment_id);
        Ok(())
    }

    /// Handles "Update" messages.
    /// We pass the `Option` fields directly. `None` means "Don't change".
    #[tracing::instrument(name = "kubernetes_service.update", skip_all, fields(id = %msg.deployment_id), err)]
    pub async fn update(
        &self,
        pool: PgPool,
        mut con: MultiplexedConnection,
        msg: UpdateDeploymentMessage,
    ) -> Result<(), AppError> {
        self.notify_status(
            &msg.project_id,
            &msg.deployment_id,
            DeploymentStatus::Updating,
            &mut con,
        )
        .await?;

        let ns = self.ensure_namespace(&msg.user_id).await?;
        let name = format_resource_name(&msg.deployment_id);

        let image_pull_secret = match msg.image_pull_secret.as_ref() {
            Some(secret) => Some(self.apply_image_pull_secret(&ns, &name, secret).await?),
            None => None,
        };

        let secret_ref = self
            .create_vault_static_secret(&msg.deployment_id.to_string(), &ns, &name, msg.secrets)
            .await?;

        let otel_service_name = msg.name;

        // 1. Apply Deployment (Partial Update)
        // If msg.image is None, SSA will ignore it and keep the existing image.
        if msg.image.is_some()
            || msg.desired_replicas.is_some()
            || msg.port.is_some()
            || msg.resource_spec.is_some()
            || msg.image_pull_secret.is_some()
        {
            self.apply_deployment(
                otel_service_name.as_deref(),
                None,
                &ns,
                &name,
                msg.image.as_deref(),
                image_pull_secret,
                msg.port,
                msg.desired_replicas,
                msg.resource_spec.as_ref(),
                secret_ref,
                msg.environment_variables,
                None,
            )
            .await?;
        }

        // 2. Apply Service
        if let Some(port) = msg.port {
            // TODO we need to pass selector
            self.apply_service(&ns, &name, port).await?;
        }

        // 3. Apply Ingress
        if msg.domain.is_some() || msg.subdomain.is_some() {
            // Note: If port didn't change, we need to fetch the existing one or pass the updated one.
            // For safety in this simplified view, we assume Ingress update usually comes with domain changes.
            // In a perfect system, you might want to read the current port if it's None in the message,
            // OR just rely on the user sending the port if they want Ingress updated.
            let port = msg.port.unwrap_or(8080); // Fallback or strict requirement
            self.apply_ingressroute(&ns, &name, msg.domain, msg.subdomain, port)
                .await?;
        }

        self.finalize_status(
            &msg.project_id,
            &msg.deployment_id,
            DeploymentStatus::Running,
            &pool,
            &mut con,
        )
        .await?;
        info!("✅ Updated deployment {}", msg.deployment_id);
        Ok(())
    }

    // ============================================================================================
    // PRIVATE APPLY FUNCTIONS
    // ============================================================================================

    #[tracing::instrument(name = "kubernetes_service.apply_deployment", skip_all, err)]
    async fn apply_deployment(
        &self,
        otel_service_name: Option<&str>,
        otel_resource_attributes: Option<&str>,
        ns: &str,
        name: &str,
        image: Option<&str>,
        image_pull_secret: Option<String>,
        port: Option<i32>,
        desired_replicas: Option<i32>,
        resource_spec: Option<&ResourceSpec>,
        secret_ref: Option<String>,
        environment_variables: Option<HashMap<String, String>>,
        labels: Option<&BTreeMap<String, String>>,
    ) -> Result<(), AppError> {
        let api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), ns);

        let mut env = vec![EnvVar {
            name: "OTEL_EXPORTER_OTLP_ENDPOINT".to_owned(),
            value: self.cfg.otel_exporter_otlp_endpoint.clone(),
            ..Default::default()
        }];

        if let Some(otel_service_name) = otel_service_name {
            env.push(EnvVar {
                name: "OTEL_SERVICE_NAME".to_owned(),
                value: Some(otel_service_name.to_string()),
                ..Default::default()
            });
        }

        if let Some(otel_resource_attributes) = otel_resource_attributes {
            env.push(EnvVar {
                name: "OTEL_RESOURCE_ATTRIBUTES".to_owned(),
                value: Some(otel_resource_attributes.to_string()),
                ..Default::default()
            });
        }

        if let Some(e) = environment_variables {
            for (key, value) in e {
                env.push(EnvVar {
                    name: key.clone(),
                    value: Some(value.clone()),
                    ..Default::default()
                });
            }
        }

        let env_from = match secret_ref {
            Some(secret_ref) => {
                let mut env_from: Vec<EnvFromSource> = vec![];
                env_from.push(EnvFromSource {
                    secret_ref: Some(SecretEnvSource {
                        name: secret_ref,
                        ..Default::default()
                    }),
                    ..Default::default()
                });
                Some(env_from)
            }
            None => None,
        };

        let resources = match resource_spec {
            Some(resource_spec) => {
                let mut resource_requirements = ResourceRequirements::default();

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

                resource_requirements.requests = Some(requests);
                resource_requirements.limits = Some(limits);

                Some(resource_requirements)
            }
            None => None,
        };

        let image_pull_secrets = image_pull_secret.map(|name| vec![LocalObjectReference { name }]);

        let mut deployment = K8sDeployment::default();

        let metadata = ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(ns.to_string()),
            labels: labels.clone().cloned(),
            ..Default::default()
        };

        let mut deployment_spec = DeploymentSpec::default();

        deployment_spec.replicas = desired_replicas;

        deployment.metadata = metadata;
        deployment_spec.selector = LabelSelector {
            match_labels: labels.clone().cloned(),
            ..Default::default()
        };

        let mut pod_template_spec = PodTemplateSpec::default();

        pod_template_spec.metadata = Some(ObjectMeta {
            labels: labels.clone().cloned(),
            ..Default::default()
        });

        let mut pod_spec = PodSpec::default();

        pod_spec.image_pull_secrets = image_pull_secrets;

        let mut container = Container::default();
        container.name = name.to_string();

        if let Some(image) = image {
            container.image = Some(image.to_string());
            container.image_pull_policy = Some("IfNotPresent".to_string());
        }

        pod_spec.containers = vec![container];

        let mut container_port = ContainerPort::default();

        if let Some(port) = port {
            container_port.container_port = port;
            container_port.protocol = Some("TCP".to_string());
        }

        container.env = Some(env);
        container.env_from = env_from;
        container.resources = resources;

        /*
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
        */

        api.patch(
            name,
            &PatchParams::apply("poddle-provisioner").force(),
            &Patch::Apply(&deployment),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, name=%name, "❌ Deployment SSA failed");
            AppError::InternalServerError(format!("Deployment SSA failed: {}", e))
        })?;

        Ok(())
    }

    #[tracing::instrument(name = "kubernetes_service.apply_service", skip_all, err)]
    async fn apply_service(
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
                // TODO what putting as selector is best?
                selector: Some(labels.clone()),
                ports: Some(vec![ServicePort {
                    name: Some("http".to_string()),
                    port,
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

        // let patch = json!({
        //     "apiVersion": "v1",
        //     "kind": "Service",
        //     "metadata": {
        //         "name": name,
        //         "namespace": ns,
        //         "labels": { "poddle.io/deployment": name }
        //     },
        //     "spec": {
        //         "type": "ClusterIP",
        //         "selector": { "poddle.io/deployment": name },
        //         "ports": [{
        //             "name": "http",
        //             "port": port,
        //             "targetPort": port,
        //             "protocol": "TCP"
        //         }],
        //     }
        // });

        api.patch(
            name,
            &PatchParams::apply("poddle-provisioner").force(),
            &Patch::Apply(&service),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, name=%name, error=%e, "🚨 Service SSA failed");
            AppError::InternalServerError(format!("🚨 Service SSA failed: {}", e))
        })?;

        Ok(())
    }

    #[tracing::instrument(name = "kubernetes_service.update_ingressroute", skip_all, err)]
    async fn apply_ingressroute(
        &self,
        ns: &str,
        name: &str,
        domain: Option<String>,
        subdomain: Option<String>,
        port: i32,
    ) -> Result<(), AppError> {
        let api: Api<IngressRoute> = Api::namespaced(self.client.clone(), ns);

        let mut routes = vec![];
        let mut domains = vec![];

        // Helper to add route
        // let mut add_route = |host: String| {
        //     routes.push(json!({
        //         "match": format!("Host(`{}`)", host),
        //         "services": [{ "name": name, "port": port }]
        //     }));
        //     domains.push(json!({ "main": host }));
        // };

        // if let Some(sub) = subdomain {
        //     add_route(format!("{}.{}", sub, self.cfg.traefik.base_domain));
        // }
        // if let Some(dom) = domain {
        //     add_route(dom);
        // }

        // if routes.is_empty() {
        //     return Ok(());
        // }

        // let patch = json!({
        //     "apiVersion": "traefik.io/v1alpha1",
        //     "kind": "IngressRoute",
        //     "metadata": {
        //         "name": name,
        //         "namespace": ns
        //     },
        //     "spec": {
        //         "entryPoints": self.cfg.traefik.entry_points,
        //         "routes": routes,
        //         "tls": {
        //             "domains": domains,
        //             "certResolver": self.cfg.cert_manager.cluster_issuer
        //         }
        //     }
        // });

        // Uses Default TLSStore (Wildcard)
        // We create wildcard secret from using cert-manager
        // In Local we create wildcard secret using Vault PKI or self signed, in Prod created by Let's Encrypt
        if let Some(sub) = subdomain {
            let full_subdomain = format!("{}.{}", sub, self.cfg.traefik.base_domain);
            routes.push(IngressRouteRoutes {
                r#match: format!("Host(`{}`)", full_subdomain),
                services: Some(vec![IngressRouteRoutesServices {
                    name: name.clone().to_string(),
                    port: Some(IntOrString::Int(port)),
                    ..Default::default()
                }]),
                ..Default::default()
            });

            domains.push(IngressRouteTlsDomains {
                main: Some(full_subdomain),
                sans: None,
            });
        }

        // Uses CertResolver (Traefik native, Let's Encrypt)
        if let Some(user_domain) = domain {
            routes.push(IngressRouteRoutes {
                r#match: format!("Host(`{}`)", user_domain),
                services: Some(vec![IngressRouteRoutesServices {
                    name: name.clone().to_string(),
                    port: Some(IntOrString::Int(port)),
                    ..Default::default()
                }]),
                ..Default::default()
            });
            domains.push(IngressRouteTlsDomains {
                main: Some(user_domain),
                sans: None,
            });
        }

        if routes.is_empty() {
            return Ok(());
        }

        let ingress_route = IngressRoute {
            metadata: ObjectMeta {
                name: Some(name.clone().to_string()),
                namespace: Some(ns.clone().to_string()),
                ..Default::default()
            },
            spec: IngressRouteSpec {
                entry_points: self.cfg.traefik.entry_points.clone(),
                routes, // Pass the dynamically built vectors
                tls: Some(IngressRouteTls {
                    // This uses "letsencrypt"
                    // Traefik will use this resolver for domains that don't match the TLSStore.
                    cert_resolver: Some(self.cfg.cert_manager.cluster_issuer.clone()),
                    domains: Some(domains),
                    // We set secret_name to NONE.
                    // - Subdomains will match the Wildcard in the Default TLSStore automatically.
                    // - Custom Domains will trigger the cert_resolver.
                    ..Default::default()
                }),
                ..Default::default()
            },
        };

        api.patch(
            name,
            &PatchParams::apply("poddle-provisioner").force(),
            &Patch::Apply(&ingress_route),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, name=%name, error=%e, "🚨 IngressRoute SSA failed");
            AppError::InternalServerError(format!("🚨 IngressRoute SSA failed: {}", e))
        })?;

        Ok(())
    }

    /// Create image pull secret
    #[tracing::instrument(name = "kubernetes_service.apply_image_pull_secret", skip_all, err)]
    async fn apply_image_pull_secret(
        &self,
        ns: &str,
        name: &str,
        creds: &ImagePullSecret,
    ) -> Result<String, AppError> {
        let secret_name = format!("{}-registry", name);

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
                name: Some(secret_name.clone()),
                namespace: Some(ns.to_string()),
                ..Default::default()
            },
            type_: Some("kubernetes.io/dockerconfigjson".to_string()),
            data,
            ..Default::default()
        };

        // let patch = json!({
        //     "apiVersion": "v1",
        //     "kind": "Secret",
        //     "metadata": { "name": secret_name, "namespace": ns },
        //     "type": "kubernetes.io/dockerconfigjson",
        //     "data": {
        //         ".dockerconfigjson": base64::engine::general_purpose::STANDARD.encode(
        //             json!({ "auths": { creds.server.clone(): { "username": creds.username, "password": creds.secret, "auth": auth } } }).to_string()
        //         )
        //     }
        // });

        let api: Api<K8sSecret> = Api::namespaced(self.client.clone(), ns);

        api.patch(
            &secret_name,
            &PatchParams::apply("poddle-provisioner").force(),
            &Patch::Apply(&secret),
        )
        .await
        .map_err(|e| {
            error!(ns = %ns, error = %e, "🚨 Image Pull Secret SSA failed");
            AppError::InternalServerError(format!("🚨 Image Pull Secret SSA failed: {}", e))
        })?;

        Ok(secret_name)
    }

    #[tracing::instrument(name = "kubernetes_service.ensure_namespace", skip_all, fields(user_id = %user_id), err)]
    async fn ensure_namespace(&self, user_id: &Uuid) -> Result<String, AppError> {
        let name = format_namespace(&user_id);

        let api: Api<Namespace> = Api::all(self.client.clone());

        match api.get(&name).await {
            Ok(_) => return Ok(name),

            Err(kube::Error::Api(ae)) if ae.code == 404 => {
                info!(user_id = %user_id, "🏗️ Creating namespace {}", name);
            }

            Err(e) => {
                error!(
                    user_id = %user_id,
                    error = %e,
                    "⚠️ Failed to check namespace existence"
                );
                return Err(AppError::InternalServerError(format!(
                    "🚨 Kubernetes API unavailable while checking namespace: {}",
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
                    "🚨 Failed to create namespace"
                );
                AppError::InternalServerError(format!(
                    "🚨 Failed to create namespace '{}': {}",
                    name, e
                ))
            })?;

        Ok(name)
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
                error!(ns=%ns, error = %e, "🚨 Failed to create VaultAuth");
                AppError::InternalServerError(format!("🚨 Failed to create VaultAuth: {}", e))
            })?;

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
                error!(deployment_id=%deployment_id, error = %e, "🚨 Failed to create VSO Secret");
                AppError::InternalServerError(format!("🚨 Failed to create VSO Secret: {}", e))
            })?;

        Ok(Some(secret_name))
    }

    // ============================================================================================
    // HELPERS
    // ============================================================================================

    async fn notify_status(
        &self,
        pid: &Uuid,
        did: &Uuid,
        status: DeploymentStatus,
        con: &mut MultiplexedConnection,
    ) -> Result<(), AppError> {
        let channel = ChannelNames::deployments_metrics(&pid.to_string());
        let message = ComputeEvent::DeploymentStatusUpdate { id: did, status };
        con.publish(channel, message).await?;
        Ok(())
    }

    async fn finalize_status(
        &self,
        pid: &Uuid,
        did: &Uuid,
        status: DeploymentStatus,
        pool: &PgPool,
        con: &mut MultiplexedConnection,
    ) -> Result<(), AppError> {
        let res = DeploymentRepository::update_status(did, status, pool).await?;
        if res.rows_affected() == 0 {
            warn!("⚠️ Update deployment status affected zero rows for {}", did);
        }
        self.notify_status(pid, did, status, con).await?;
        Ok(())
    }

    // --------------------------------------------------------------------------------------------
    // delete
    // --------------------------------------------------------------------------------------------

    /// Starting point
    pub async fn delete(&self, msg: DeleteDeploymentMessage) -> Result<(), AppError> {
        let user_id = msg.user_id;
        let deployment_id = msg.deployment_id;

        let ns = self.ensure_namespace(&user_id).await?;
        let name = format_resource_name(&deployment_id);

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
