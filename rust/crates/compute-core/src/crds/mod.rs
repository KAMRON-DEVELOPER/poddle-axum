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
    pub tag: String,
    pub service_account_name: String,
    pub builder: BuilderReference,
    pub source: SourceConfig,

    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageStatus {
    pub latest_image: Option<String>,
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
    pub tags: Vec<String>,
    pub builder: BuilderReference,
    pub source: SourceConfig,

    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BuildStatus {
    pub latest_image: Option<String>,
    pub conditions: Option<Vec<Condition>>,
}

// -----------------------------------------------------------------------------
// Shared Sub-Types
// -----------------------------------------------------------------------------
#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BuilderReference {
    // For your cluster-wide builds, you will set this to Some("ClusterBuilder".to_string())
    pub kind: Option<String>,
    pub name: String,
}

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

// Standard Kubernetes condition format for checking if a build is "Ready" or "Succeeded"
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    pub r#type: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}
