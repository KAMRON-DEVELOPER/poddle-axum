use uuid::Uuid;

use crate::features::{
    models::{OAuthUser, Provider},
    schemas::{EmailAuthRequest, GithubOAuthUser, GoogleOAuthUser, UserMutationPayload},
};

impl From<EmailAuthRequest> for UserMutationPayload {
    fn from(ear: EmailAuthRequest) -> Self {
        Self {
            username: ear.username,
            email: Some(ear.email),
            ..Default::default()
        }
    }
}

impl From<&GoogleOAuthUser> for UserMutationPayload {
    fn from(g: &GoogleOAuthUser) -> Self {
        Self {
            username: g.name.clone(),
            email: g.email.clone(),
            picture: g.picture.clone(),
            ..Default::default()
        }
    }
}

impl From<&GithubOAuthUser> for UserMutationPayload {
    fn from(g: &GithubOAuthUser) -> Self {
        Self {
            username: g.name.clone(),
            email: g.email.clone(),
            picture: g.picture.clone(),
            ..Default::default()
        }
    }
}

impl From<(Uuid, GoogleOAuthUser)> for OAuthUser {
    fn from((user_id, g): (Uuid, GoogleOAuthUser)) -> Self {
        Self {
            id: g.sub,
            provider: Provider::Google,
            user_id,
            username: g.name,
            email: g.email,
            picture: g.picture,
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }
}

impl From<(Uuid, GithubOAuthUser)> for OAuthUser {
    fn from((user_id, g): (Uuid, GithubOAuthUser)) -> Self {
        Self {
            id: g.id.to_string(),
            provider: Provider::Github,
            user_id,
            username: Some(g.login),
            email: g.email,
            picture: Some(g.avatar_url),
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }
}
