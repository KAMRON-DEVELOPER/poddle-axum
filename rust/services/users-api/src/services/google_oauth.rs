use oauth2::{
    AuthUrl, ClientId, ClientSecret, RedirectUrl, RevocationUrl, TokenUrl, basic::BasicClient,
};

use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct GoogleOAuthServiceConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

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

pub fn build_google_oauth_client(cfg: &GoogleOAuthServiceConfig) -> GoogleOAuthClient {
    let client_id = ClientId::new(cfg.client_id.clone());
    let client_secret = ClientSecret::new(cfg.client_secret.clone());

    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .unwrap_or_else(|e| panic!("Couldn't create AuthUrl: {}", e));
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
        .unwrap_or_else(|e| panic!("Couldn't create TokenUrl: {}", e));
    let redirect_uri = RedirectUrl::new(cfg.redirect_url.clone())
        .unwrap_or_else(|e| panic!("Couldn't create RedirectUrl: {}", e));
    let revocation_url = RevocationUrl::new("https://oauth2.googleapis.com/revoke".to_string())
        .unwrap_or_else(|e| panic!("Couldn't create RevocationUrl: {}", e));

    // Create an OAuth2 client by specifying the client ID, client secret, authorization URL and
    // token URL.
    BasicClient::new(client_id)
        .set_client_secret(client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_uri)
        .set_revocation_url(revocation_url)
}
