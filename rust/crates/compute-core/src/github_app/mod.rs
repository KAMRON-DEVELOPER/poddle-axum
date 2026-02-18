use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod error;
pub mod implementations;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GithubAppConfig {
    pub client_id: String,
    pub client_secret: String,
    pub public_link: String,
    pub private_key_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GithubAppClaims {
    // Issued at time
    pub iat: i64,
    // JWT expiration time
    pub exp: i64,
    // GitHub App's client ID
    pub iss: String,
}

#[derive(Clone, Debug)]
pub struct GithubApp {
    pub cfg: GithubAppConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Installation {
    pub id: i64,
    pub account: InstallationAccount,
    pub access_tokens_url: String,
    pub repositories_url: String,
    pub html_url: String,
    pub app_id: i64,
    pub target_id: i64,
    pub target_type: String,
    pub permissions: InstallationPermissions,
    pub events: Vec<String>,
    pub single_file_name: String,
    pub has_multiple_single_files: bool,
    pub single_file_paths: Vec<String>,
    pub repository_selection: String,
    pub created_at: String,
    pub updated_at: String,
    pub app_slug: String,
    pub suspended_at: Option<String>,
    pub suspended_by: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallationPermissions {
    pub checks: String,
    pub metadata: String,
    pub contents: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallationAccount {
    pub login: String,
    pub id: i64,
    pub node_id: String,
    pub avatar_url: String,
    pub gravatar_id: String,
    pub url: String,
    pub html_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: String,
    pub subscriptions_url: String,
    pub organizations_url: String,
    pub repos_url: String,
    pub events_url: String,
    pub received_events_url: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub site_admin: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallationAccessToken {
    pub token: String,
    pub expires_at: String,
    pub permissions: InstallationAccessTokenPermissions,
    pub repository_selection: String,
    pub repositories: Vec<Repository>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Repository {
    pub id: i64,
    pub node_id: String,
    pub name: String,
    pub full_name: String,
    pub owner: RepositoryOwner,
    pub private: bool,
    pub html_url: String,
    pub description: String,
    pub fork: bool,
    pub url: String,
    pub archive_url: String,
    pub assignees_url: String,
    pub blobs_url: String,
    pub branches_url: String,
    pub collaborators_url: String,
    pub comments_url: String,
    pub commits_url: String,
    pub compare_url: String,
    pub contents_url: String,
    pub contributors_url: String,
    pub deployments_url: String,
    pub downloads_url: String,
    pub events_url: String,
    pub forks_url: String,
    pub git_commits_url: String,
    pub git_refs_url: String,
    pub git_tags_url: String,
    pub git_url: String,
    pub issue_comment_url: String,
    pub issue_events_url: String,
    pub issues_url: String,
    pub keys_url: String,
    pub labels_url: String,
    pub languages_url: String,
    pub merges_url: String,
    pub milestones_url: String,
    pub notifications_url: String,
    pub pulls_url: String,
    pub releases_url: String,
    pub ssh_url: String,
    pub stargazers_url: String,
    pub statuses_url: String,
    pub subscribers_url: String,
    pub subscription_url: String,
    pub tags_url: String,
    pub teams_url: String,
    pub trees_url: String,
    pub clone_url: String,
    pub mirror_url: String,
    pub hooks_url: String,
    pub svn_url: String,
    pub homepage: String,
    pub language: Option<String>,
    pub forks_count: i64,
    pub stargazers_count: i64,
    pub watchers_count: i64,
    pub size: i64,
    pub default_branch: String,
    pub open_issues_count: i64,
    pub is_template: bool,
    pub topics: Vec<String>,
    pub has_issues: bool,
    pub has_projects: bool,
    pub has_wiki: bool,
    pub has_pages: bool,
    pub has_downloads: bool,
    pub archived: bool,
    pub disabled: bool,
    pub visibility: String,
    pub pushed_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub permissions: InstallationAccessTokenPermissions,
    pub allow_rebase_merge: bool,
    pub template_repository: Option<String>,
    pub temp_clone_token: String,
    pub allow_squash_merge: bool,
    pub allow_auto_merge: bool,
    pub delete_branch_on_merge: bool,
    pub allow_merge_commit: bool,
    pub subscribers_count: i64,
    pub network_count: i64,
    pub license: RepositoryLicense,
    pub forks: i64,
    pub open_issues: i64,
    pub watchers: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryLicense {
    pub key: String,
    pub name: String,
    pub url: String,
    pub spdx_id: String,
    pub node_id: String,
    pub html_url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryOwner {
    pub login: String,
    pub id: i64,
    pub node_id: String,
    pub avatar_url: String,
    pub gravatar_id: String,
    pub url: String,
    pub html_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: String,
    pub subscriptions_url: String,
    pub organizations_url: String,
    pub repos_url: String,
    pub events_url: String,
    pub received_events_url: String,
    #[serde(rename = "type")]
    pub repository_owner_type: String,
    pub site_admin: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallationAccessTokenPermissions {
    pub issues: String,
    pub contents: String,
}
