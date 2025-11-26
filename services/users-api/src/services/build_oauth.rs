use oauth2::{
    AuthUrl, ClientId, ClientSecret, RedirectUrl, RevocationUrl, TokenUrl, basic::BasicClient,
};
use shared::utilities::{config::Config, errors::AppError};

pub type GoogleOAuthClient = oauth2::Client<
    oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
    oauth2::StandardTokenIntrospectionResponse<
        oauth2::EmptyExtraTokenFields,
        oauth2::basic::BasicTokenType,
    >,
    oauth2::StandardRevocableToken,
    oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>,
    oauth2::EndpointSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointSet,
    oauth2::EndpointSet,
>;

pub type GithubOAuthClient = oauth2::Client<
    oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
    oauth2::StandardTokenIntrospectionResponse<
        oauth2::EmptyExtraTokenFields,
        oauth2::basic::BasicTokenType,
    >,
    oauth2::StandardRevocableToken,
    oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>,
    oauth2::EndpointSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointSet,
>;

pub fn build_google_oauth_client(config: &Config) -> Result<GoogleOAuthClient, AppError> {
    let google_client_id = ClientId::new(config.google_oauth_client_id.clone());
    let google_client_secret = ClientSecret::new(config.google_oauth_client_secret.clone());

    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?;
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())?;
    let redirect_uri = RedirectUrl::new(config.google_oauth_redirect_url.clone())?;
    let revocation_url = RevocationUrl::new("https://oauth2.googleapis.com/revoke".to_string())?;

    // Create an OAuth2 client by specifying the client ID, client secret, authorization URL and
    // token URL.
    Ok(BasicClient::new(google_client_id)
        .set_client_secret(google_client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_uri)
        .set_revocation_url(revocation_url))
}

pub fn build_github_oauth_client(config: &Config) -> Result<GithubOAuthClient, AppError> {
    let github_client_id = ClientId::new(config.github_oauth_client_id.clone());
    let github_client_secret = ClientSecret::new(config.github_oauth_client_secret.clone());

    let auth_url = AuthUrl::new("https://github.com/login/oauth/authorize".to_string())?;
    let token_url = TokenUrl::new("https://github.com/login/oauth/access_token".to_string())?;
    let redirect_uri = RedirectUrl::new(config.github_oauth_redirect_url.clone())?;

    // Create an OAuth2 client by specifying the client ID, client secret, authorization URL and
    // token URL.
    Ok(BasicClient::new(github_client_id)
        .set_client_secret(github_client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_uri))
}
