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

        // Define Labels & Selector
        let mut labels = BTreeMap::new();
        labels.insert("poddle.io/managed-by".into(), "poddle".into());
        labels.insert("poddle.io/project-id".into(), msg.project_id.into());
        labels.insert("poddle.io/deployment-id".into(), msg.deployment_id.into());
        labels.insert("poddle.io/preset-id".into(), msg.preset_id.into());

        // Selector is invariant
        let mut selector = BTreeMap::new();
        selector.insert(
            "poddle.io/deployment-id".to_string(),
            deployment_id.to_string(),
        );

        self.apply_vso_resources(&ns).await?;

        // This creates the VSO Resource AND writes the initial data to Vault
        let secret_ref = self
            .apply_vault_static_secret(&msg.deployment_id, &ns, &name, msg.secrets, &pool)
            .await?;

        let image_pull_secret_data = if let Some(secret) = msg.image_pull_secret.as_ref() {
            Some(self.apply_image_pull_secret(&ns, &name, secret).await?)
        } else {
            None
        };

        let otel_resource_attributes = format!(
            "project_id={},deployment_id={},managed_by=poddle",
            msg.project_id, msg.deployment_id
        );

        self.apply_deployment(
            Some(&msg.name),
            Some(&otel_resource_attributes),
            &ns,
            &name,
            Some(&msg.image),
            image_pull_secret_data,
            Some(msg.port),
            Some(msg.desired_replicas),
            Some(&msg.resource_spec),
            secret_ref,
            msg.environment_variables,
            Some(&labels),
            &selector,
        )
        .await?;

        // deployment_id will be selector
        self.apply_service(&ns, &name, msg.port, Some(&labels), &selector)
            .await?;

        self.apply_ingressroute(&ns, &name, msg.domain, msg.subdomain, msg.port)
            .await?;

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

        let user_id = msg.user_id.clone();
        let project_id = msg.project_id.clone();
        let deployment_id = msg.deployment_id.clone();

        info!(
            user_id = %user_id,
            project_id = %project_id,
            deployment_id = %deployment_id,
            "🚀 Updating deployment"
        );

        let ns = self.ensure_namespace(&msg.user_id).await?;
        let name = format_resource_name(&msg.deployment_id);

        // Define Labels & Selector
        let mut labels = BTreeMap::new();
        labels.insert("poddle.io/managed-by".into(), "poddle".into());
        labels.insert("poddle.io/project-id".into(), msg.project_id.into());
        labels.insert("poddle.io/deployment-id".into(), msg.deployment_id.into());

        let deployment = DeploymentRepository::get_preset_id(&deployment_id, &pool)
            .await
            .map_err(|e| {
                error!(ns=%ns, name=%name, error = %e, "🚨 Failed to get deployment from database");
                AppError::InternalServerError(format!(
                    "🚨 Failed to get deployment from database: {}",
                    e
                ))
            })?;

        let preset_id = msg.preset_id.unwrap_or(deployment.preset_id);
        labels.insert("poddle.io/preset-id".into(), preset_id.to_string());

        // Selector is invariant
        let mut selector = BTreeMap::new();
        selector.insert(
            "poddle.io/deployment-id".to_string(),
            deployment_id.to_string(),
        );

        // Update Secrets (if provided)
        // We call the same function. It updates Vault data. VSO detects change -> Restarts Pods.
        let secret_ref = self
            .apply_vault_static_secret(&msg.deployment_id, &ns, &name, msg.secrets, &pool)
            .await?;

        let image_pull_secret_data = match msg.image_pull_secret.as_ref() {
            Some(secret) => Some(self.apply_image_pull_secret(&ns, &name, secret).await?),
            None => None,
        };

        // image_pull_secret is not optional because we add automatic restart when image pull secret change
        if msg.image.is_some()
            || msg.image_pull_secret.is_some()
            || msg.port.is_some()
            || msg.resource_spec.is_some()
            || msg.desired_replicas.is_some()
            || msg.environment_variables.is_some()
        {
            self.apply_deployment(
                msg.name.as_deref(),
                None,
                &ns,
                &name,
                msg.image.as_deref(),
                image_pull_secret_data,
                msg.port,
                msg.desired_replicas,
                msg.resource_spec.as_ref(),
                secret_ref,
                msg.environment_variables,
                Some(&labels),
                &selector,
            )
            .await?;
        }

        // Service (Only if port changes)
        if let Some(port) = msg.port {
            self.apply_service(&ns, &name, port, None, &selector)
                .await?;
        }

        // Ingress (If Port OR Domain changes)
        if msg.port.is_some() || msg.domain.is_some() || msg.subdomain.is_some() {
            // Determine the port to use for Ingress.
            // If the user sent a new port, use it.
            // If not, we MUST fetch the existing port from the DB, because Ingress requires it.
            let port = if let Some(p) = msg.port {
                p
            } else {
                deployment.port
            };

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

    #[tracing::instrument(name = "kubernetes_service.delete", skip_all, fields(project_id = %msg.project_id, deployment_id = %msg.deployment_id), err)]
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

        info!("✅ Deleted deployment {}", msg.deployment_id);

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
        image_pull_secret_data: Option<(String, String)>,
        port: Option<i32>,
        desired_replicas: Option<i32>,
        resource_spec: Option<&ResourceSpec>,
        secret_ref: Option<String>,
        environment_variables: Option<HashMap<String, String>>,
        labels: Option<&BTreeMap<String, String>>,
        selector: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let api: Api<K8sDeployment> = Api::namespaced(self.client.clone(), ns);

        // We use default() to initialize, then only set fields that are Some.
        // k8s-openapi structs are #[serde(skip_serializing_if = "Option::is_none")]
        // so this creates the perfect "Partial JSON" automatically.

        let container = self.create_container(
            otel_service_name,
            otel_resource_attributes,
            name,
            image,
            port,
            resource_spec,
            secret_ref,
            environment_variables,
        );

        let (pull_secret, pull_secret_checksum) = match image_pull_secret_data {
            Some((n, c)) => (Some(n), Some(c)),
            None => (None, None),
        };

        let image_pull_secrets = pull_secret.map(|name| vec![LocalObjectReference { name }]);

        // PodSpec:
        //      image_pull_secrets: Option<Vec<LocalObjectReference>>
        //      containers: Vec<Container>
        let pod_spec = PodSpec {
            image_pull_secrets,
            containers: vec![container],
            ..Default::default()
        };

        let mut annotations: Option<BTreeMap<String, String>> = None;
        if let Some(sum) = pull_secret_checksum {
            // MAGIC IS HERE: Changing this value forces a restart
            let a = annotations.get_or_insert_with(BTreeMap::new);
            a.insert("poddle.io/registry-checksum".to_string(), sum);
        }

        // PodTemplateSpec:
        //      metadata: Option<ObjectMeta>
        //      spec: Option<PodSpec>
        let pod_template_spec = PodTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: labels.clone().cloned(),
                annotations,
                ..Default::default()
            }),
            spec: Some(pod_spec),
        };

        // DeploymentSpec:
        //      replicas: Option<i32>
        //      selector: LabelSelector
        //      template: PodTemplateSpec
        let deployment_spec = DeploymentSpec {
            replicas: desired_replicas,
            selector: LabelSelector {
                match_labels: Some(selector.clone()),
                ..Default::default()
            },
            template: pod_template_spec,
            ..Default::default()
        };

        // K8sDeployment:
        //      metadata: ObjectMeta
        //      spec: Option<DeploymentSpec>
        let deployment = K8sDeployment {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(ns.to_string()),
                labels: labels.cloned(),
                ..Default::default()
            },
            spec: Some(deployment_spec),
            ..Default::default()
        };

        api.patch(
            name,
            &PatchParams::apply("poddle-provisioner").force(),
            &Patch::Apply(&deployment),
        )
        .await
        .map_err(|e| {
            error!(ns=%ns, name=%name, error = %e, "🚨 Deployment SSA failed");
            AppError::InternalServerError(format!("🚨 Deployment SSA failed: {}", e))
        })?;

        Ok(())
    }

    fn create_container(
        &self,
        otel_service_name: Option<&str>,
        otel_resource_attributes: Option<&str>,
        name: &str,
        image: Option<&str>,
        port: Option<i32>,
        resource_spec: Option<&ResourceSpec>,
        secret_ref: Option<String>,
        environment_variables: Option<HashMap<String, String>>,
    ) -> Container {
        // Container:
        //      name: String
        //      image: Option<String>
        //      image_pull_policy: Option<String>
        //      ports: Option<Vec<ContainerPort>>
        //      env: ss
        //      env_from: ss
        //      resources: ss
        let mut container = Container::default();
        container.name = name.to_string();
        container.image = image.map(str::to_string);
        // Image pull policy. One of Always, Never, IfNotPresent. Defaults to Always if :latest tag is specified, or IfNotPresent otherwise. Cannot be updated.
        container.image_pull_policy = Some("IfNotPresent".to_string());
        let ports = port.map(|container_port| {
            vec![ContainerPort {
                container_port,
                protocol: Some("TCP".into()),
                ..Default::default()
            }]
        });
        container.ports = ports;

        // Environment Variables
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
        container.env = Some(env);

        if let Some(name) = secret_ref {
            container.env_from = Some(vec![EnvFromSource {
                secret_ref: Some(SecretEnvSource {
                    name: name,
                    ..Default::default()
                }),
                ..Default::default()
            }]);
        };

        if let Some(resource_spec) = resource_spec {
            let mut req = BTreeMap::new();
            req.insert(
                "cpu".to_string(),
                Quantity(format!("{}m", resource_spec.cpu_request_millicores)),
            );
            req.insert(
                "memory".to_string(),
                Quantity(format!("{}Mi", resource_spec.memory_request_mb)),
            );

            let mut lim = BTreeMap::new();
            lim.insert(
                "cpu".to_string(),
                Quantity(format!("{}m", resource_spec.cpu_limit_millicores)),
            );
            lim.insert(
                "memory".to_string(),
                Quantity(format!("{}Mi", resource_spec.memory_limit_mb)),
            );

            container.resources = Some(ResourceRequirements {
                requests: Some(req),
                limits: Some(lim),
                ..Default::default()
            });
        };

        container
    }

    #[tracing::instrument(name = "kubernetes_service.apply_service", skip_all, err)]
    async fn apply_service(
        &self,
        ns: &str,
        name: &str,
        port: i32,
        labels: Option<&BTreeMap<String, String>>,
        selector: &BTreeMap<String, String>,
    ) -> Result<(), AppError> {
        let api: Api<Service> = Api::namespaced(self.client.clone(), ns);

        let service = Service {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(ns.to_string()),
                labels: labels.cloned(),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: Some(selector.clone()),
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

    #[tracing::instrument(name = "kubernetes_service.apply_ingressroute", skip_all, err)]
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
                    name: name.to_string(),
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
                    name: name.to_string(),
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
                name: Some(name.to_string()),
                namespace: Some(ns.to_string()),
                ..Default::default()
            },
            spec: IngressRouteSpec {
                entry_points: self.cfg.traefik.entry_points.clone(),
                routes,
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
        secret: &ImagePullSecret,
    ) -> Result<(String, String), AppError> {
        let secret_name = format!("{}-registry", name);

        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", secret.username, secret.secret));

        let dockerconfig = serde_json::json!({
            "auths": {
                secret.server.clone(): {
                    "username": secret.username,
                    "password": secret.secret,
                    "auth": auth
                }
            }
        })
        .to_string();

        // Calculate SHA256 Checksum of the config
        // When secrets change pods doesn't restart until timeout beacuse name and image not changed
        // if we set checksum as label to pod they does restart
        let checksum = sha256::digest(&dockerconfig);

        // ByteString type from k8s-openapi automatically Base64 encodes its contents
        let data = Some(BTreeMap::from([(
            ".dockerconfigjson".to_string(),
            ByteString(dockerconfig.into_bytes()),
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

        Ok((secret_name, checksum))
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
    async fn apply_vso_resources(&self, ns: &str) -> Result<(), AppError> {
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

        let vc_api_patch_name = &vault_connection.metadata.name.clone().unwrap();
        vault_connection_api
            .patch(
                vc_api_patch_name,
                &PatchParams::apply("poddle-provisioner").force(),
                &Patch::Apply(vault_connection),
            )
            .instrument(info_span!("apply_vault_connection"))
            .await
            .map_err(|e| {
                error!(ns=%ns, error = %e, "🚨 VaultConnection SSA failed");
                AppError::InternalServerError(format!("🚨 VaultConnection SSA failed: {}", e))
            })?;

        let va_api_patch_name = &vault_auth.metadata.name.clone().unwrap();
        vault_auth_api
            .patch(
                va_api_patch_name,
                &PatchParams::apply("poddle-provisioner").force(),
                &Patch::Apply(vault_auth),
            )
            .instrument(info_span!("apply_vault_api"))
            .await
            .map_err(|e| {
                error!(ns=%ns, error = %e, "🚨 VaultAuth SSA failed");
                AppError::InternalServerError(format!("🚨 VaultAuth SSA failed: {}", e))
            })?;

        Ok(())
    }

    /// Create VSO resources
    #[tracing::instrument(name = "kubernetes_service.create_vault_static_secret", skip_all, fields(deployment_id = %deployment_id), err)]
    async fn apply_vault_static_secret(
        &self,
        deployment_id: &Uuid,
        ns: &str,
        name: &str,
        secrets: Option<HashMap<String, String>>,
        pool: &PgPool,
    ) -> Result<Option<String>, AppError> {
        let secrets = match secrets {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(None),
        };

        let secret_name = format!("{}-secrets", name);

        let keys = secrets.keys().cloned().collect();

        // Write to Vault
        let path = self
            .vault_service
            .store_secrets(ns, &deployment_id.to_string(), secrets)
            .await?;

        DeploymentRepository::set_vault_secret_path(deployment_id, &path, pool).await?;
        DeploymentRepository::set_secret_keys(deployment_id, keys, pool).await?;

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

        api.patch(
            name,
            &PatchParams::apply("poddle-provisioner").force(),
            &Patch::Apply(vault_static_secret),
        )
        .instrument(info_span!("apply_vault_static_secret"))
        .await
        .map_err(|e| {
            error!(deployment_id=%deployment_id, error = %e, "🚨 VaultStaticSecret SSA failed");
            AppError::InternalServerError(format!("🚨 VaultStaticSecret SSA failed: {}", e))
        })?;

        Ok(Some(secret_name))
    }

    // ============================================================================================
    // HELPERS
    // ============================================================================================

    #[tracing::instrument(name = "kubernetes_service.notify_status", skip_all, fields(project_id = %project_id, deployment_id = %deployment_id), err)]
    async fn notify_status(
        &self,
        project_id: &Uuid,
        deployment_id: &Uuid,
        status: DeploymentStatus,
        con: &mut MultiplexedConnection,
    ) -> Result<(), AppError> {
        let channel = ChannelNames::deployments_metrics(&project_id.to_string());
        let message = ComputeEvent::DeploymentStatusUpdate {
            id: deployment_id,
            status,
        };
        con.publish(channel, message).await?;
        Ok(())
    }

    #[tracing::instrument(name = "kubernetes_service.finalize_status", skip_all, fields(project_id = %project_id, deployment_id = %deployment_id), err)]
    async fn finalize_status(
        &self,
        project_id: &Uuid,
        deployment_id: &Uuid,
        status: DeploymentStatus,
        pool: &PgPool,
        con: &mut MultiplexedConnection,
    ) -> Result<(), AppError> {
        let res = DeploymentRepository::update_status(deployment_id, status, pool).await?;
        if res.rows_affected() == 0 {
            warn!(
                "⚠️ Update deployment status affected zero rows for {}",
                deployment_id
            );
        }
        self.notify_status(project_id, deployment_id, status, con)
            .await?;
        Ok(())
    }
}
