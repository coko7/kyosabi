use std::sync::Arc;

use axum::extract::FromRef;
use axum_csrf::CsrfConfig;
use axum_extra::extract::cookie::Key;

use crate::config::AppConfig;
use crate::db::Db;
use crate::oidc::OidcContext;
use crate::rustypaste::RustypasteClient;
use crate::token_map::TokenMap;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: Db,
    pub http: reqwest::Client,
    pub token_map: Arc<TokenMap>,
    pub rustypaste: RustypasteClient,
    pub oidc: Arc<OidcContext>,
    pub cookie_key: Key,
    pub csrf_config: CsrfConfig,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

impl FromRef<AppState> for CsrfConfig {
    fn from_ref(state: &AppState) -> Self {
        state.csrf_config.clone()
    }
}
