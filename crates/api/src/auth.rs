use axum::{
    body::Body,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseClaims {
    pub sub: Uuid,
    pub role: String,
    pub email: Option<String>,
    pub exp: usize,
    pub iss: String,
    pub aud: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub email: Option<String>,
}

#[derive(Clone)]
pub struct AuthVerifier {
    jwks: Arc<JwkSet>,
    issuer: String,
}

impl AuthVerifier {
    pub async fn from_supabase_url(project_url: &str) -> anyhow::Result<Self> {
        let base = project_url.trim_end_matches('/');
        let jwks_url = std::env::var("SUPABASE_JWKS_URL")
            .unwrap_or_else(|_| format!("{base}/auth/v1/.well-known/jwks.json"));
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?
            .get(jwks_url)
            .send()
            .await?
            .error_for_status()?;
        let jwks = response.json::<JwkSet>().await?;
        anyhow::ensure!(!jwks.keys.is_empty(), "Supabase JWKS contains no signing keys");
        Ok(Self {
            jwks: Arc::new(jwks),
            issuer: format!("{base}/auth/v1"),
        })
    }

    pub fn verify(&self, token: &str) -> anyhow::Result<SupabaseClaims> {
        let header = decode_header(token)?;
        let kid = header.kid.as_deref().ok_or_else(|| anyhow::anyhow!("JWT has no key id"))?;
        match header.alg {
            Algorithm::RS256 | Algorithm::ES256 | Algorithm::EdDSA => {}
            _ => anyhow::bail!("JWT signing algorithm is not allowed"),
        }
        let jwk = self.jwks.find(kid).ok_or_else(|| anyhow::anyhow!("JWT signing key is unknown"))?;
        let key = DecodingKey::from_jwk(jwk)?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&["authenticated"]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        let token = decode::<SupabaseClaims>(token, &key, &validation)?;
        anyhow::ensure!(token.claims.role == "authenticated", "JWT role is not a user role");
        Ok(token.claims)
    }
}

pub async fn require_auth(
    axum::extract::State(verifier): axum::extract::State<AuthVerifier>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let Some(token) = token else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":"missing_bearer_token"}))).into_response();
    };

    match verifier.verify(token) {
        Ok(claims) => {
            request.extensions_mut().insert(AuthenticatedUser {
                id: claims.sub,
                email: claims.email,
            });
            next.run(request).await
        }
        Err(error) => {
            tracing::warn!(error = %error, "Supabase access token rejected");
            (StatusCode::UNAUTHORIZED, Json(json!({"error":"invalid_access_token"}))).into_response()
        }
    }
}
