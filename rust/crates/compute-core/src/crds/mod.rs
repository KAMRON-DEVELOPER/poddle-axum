use kube::CustomResource;
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Image Resource (Used to trigger the build process)
// -----------------------------------------------------------------------------
#[derive(CustomResource, Deserialize, Serialize, Clone, Default, Debug)]
#[kube(
    group = "kpack.io",
    version = "v1alpha2",
    kind = "Image",
    plural = "images",
    namespaced,
    schema = "disabled", // <-- This bypasses the JsonSchema trait error
    status = "ImageStatus" // <-- Allows you to read the Image status
)]
#[serde(rename_all = "camelCase")]
pub struct ImageSpec {
    /// Destination registry tag (the "base" tag).
    /// Example: me-central1-docker.pkg.dev/poddle-mvp/kpack/<deployment-id>
    pub tag: String,
    pub service_account_name: String,
    /// Logical Builder reference (Builder or ClusterBuilder).
    pub builder: ImageBuilderRef,
    pub source: SourceConfig,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageBuilderRef {
    /// Usually "Builder" (namespaced) or "ClusterBuilder" (cluster-scoped).
    pub kind: Option<String>,
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageStatus {
    pub latest_image: Option<String>,
    pub latest_build_ref: Option<String>,
    pub latest_build_reason: Option<String>,
    pub latest_stack: Option<String>,
    pub observed_generation: Option<i64>,
    pub build_counter: Option<i64>,
    pub build_cache_name: Option<String>,
    pub latest_build_image_generation: Option<i64>,
    pub conditions: Option<Vec<Condition>>,
}

// -----------------------------------------------------------------------------
// Build Resource (Used to watch the actual build execution)
// -----------------------------------------------------------------------------
#[derive(CustomResource, Deserialize, Serialize, Clone, Default, Debug)]
#[kube(
    group = "kpack.io",
    version = "v1alpha2",
    kind = "Build",
    plural = "builds",
    namespaced,
    schema = "disabled", // <-- Bypass JsonSchema here too
    status = "BuildStatus" // <-- Critical for watching the build succeed/fail
)]
#[serde(rename_all = "camelCase")]
pub struct BuildSpec {
    pub tags: Option<Vec<String>>,
    pub builder: BuildBuilderRef,
    pub source: SourceConfig,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BuildBuilderRef {
    pub image: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BuildStatus {
    pub latest_image: Option<String>,
    pub pod_name: Option<String>,
    pub lifecycle_version: Option<String>,
    pub observed_generation: Option<i64>,
    pub conditions: Option<Vec<Condition>>,
}

// -----------------------------------------------------------------------------
// Shared sub-types
// -----------------------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceConfig {
    pub git: Option<GitSource>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GitSource {
    pub url: String,
    pub revision: String,
}

/// Standard Kubernetes condition (used in both Image and Build status).
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    pub r#type: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}
