use crate::features::{
    models::{OAuthUser, Provider},
    schemas::{GithubOAuthUser, GoogleOAuthUser},
};

impl From<GoogleOAuthUser> for OAuthUser {
    fn from(g: GoogleOAuthUser) -> Self {
        Self {
            id: g.sub,
            provider: Provider::Google,
            username: g.name,
            email: g.email,
            password: None,
            picture: g.picture,
            created_at: None,
            updated_at: None,
        }
    }
}

impl From<GithubOAuthUser> for OAuthUser {
    fn from(g: GithubOAuthUser) -> Self {
        Self {
            id: g.id.to_string(),
            provider: Provider::Github,
            username: Some(g.login),
            email: g.email,
            password: None,
            picture: Some(g.avatar_url),
            created_at: None,
            updated_at: None,
        }
    }
}
