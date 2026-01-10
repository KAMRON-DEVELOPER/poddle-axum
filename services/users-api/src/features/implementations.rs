use bcrypt::verify;
use shared::{servicesservices::database::Database, utilities::errors::AppError};
use validator::Validate;

use crate::features::{
    models::{OAuthUser, Provider, User, UserRole, UserStatus},
    schemas::{AuthIn, GithubOAuthUser, GoogleOAuthUser},
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

impl AuthIn {
    pub async fn verify(&self, database: &Database) -> Result<Option<User>, AppError> {
        self.validate()?;

        let maybe_user = sqlx::query_as!(
            User,
            r#"
                SELECT
                    id, 
                    username,
                    email,
                    password,
                    picture,
                    role AS "role: UserRole",
                    status AS "status: UserStatus",
                    email_verified,
                    oauth_user_id,
                    created_at,
                    updated_at
                FROM users WHERE email = $1
            "#,
            self.email,
        )
        .fetch_optional(&database.pool)
        .await?;

        if let Some(user) = maybe_user {
            let verified = verify(&self.password, &user.password)?;

            if !verified {
                return Err(AppError::ValidationError(
                    "Password is incorrect".to_string(),
                ));
            }

            return Ok(Some(user));
        }

        Ok(None)
    }
}
