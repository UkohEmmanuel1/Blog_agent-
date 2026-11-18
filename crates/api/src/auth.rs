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
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
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
    jwks: Arc<RwLock<JwkSet>>,
    jwks_url: String,
    client: reqwest::Client,
    issuer: String,
    legacy_hs256_secret: Option<Arc<String>>,
}

impl AuthVerifier {
    pub async fn from_supabase_url(project_url: &str) -> anyhow::Result<Self> {
        let base = project_url.trim_end_matches('/');
        let jwks_url = std::env::var("SUPABASE_JWKS_URL")
            .unwrap_or_else(|_| format!("{base}/auth/v1/.well-known/jwks.json"));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let legacy_hs256_secret = std::env::var("SUPABASE_JWT_SECRET")
            .ok()
            .filter(|secret| !secret.is_empty())
            .map(Arc::new);

        let fetched = client.get(&jwks_url).send().await;
        let jwks = match fetched {
            Ok(response) if response.status().is_success() => response.json::<JwkSet>().await?,
            Ok(response) => {
                anyhow::ensure!(legacy_hs256_secret.is_some(), "Supabase JWKS request failed: {}", response.status());
                JwkSet { keys: Vec::new() }
            }
            Err(error) => {
                anyhow::ensure!(legacy_hs256_secret.is_some(), "could not load Supabase JWKS: {error}");
                JwkSet { keys: Vec::new() }
            }
        };
        anyhow::ensure!(
            !jwks.keys.is_empty() || legacy_hs256_secret.is_some(),
            "Supabase JWKS has no keys and no legacy JWT secret was configured"
        );

        Ok(Self {
            jwks: Arc::new(RwLock::new(jwks)),
            jwks_url,
            client,
            issuer: format!("{base}/auth/v1"),
            legacy_hs256_secret,
        })
    }

    async fn refresh_jwks(&self) -> anyhow::Result<()> {
        let response = self.client.get(&self.jwks_url).send().await?.error_for_status()?;
        let jwks = response.json::<JwkSet>().await?;
        anyhow::ensure!(!jwks.keys.is_empty(), "refreshed Supabase JWKS contains no keys");
        *self.jwks.write().await = jwks;
        Ok(())
    }

    pub async fn verify(&self, token: &str) -> anyhow::Result<SupabaseClaims> {
        let header = decode_header(token)?;
        let key = match header.alg {
            Algorithm::HS256 => {
                let secret = self.legacy_hs256_secret.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("legacy HS256 verification is not configured"))?;
                DecodingKey::from_secret(secret.as_bytes())
            }
            Algorithm::RS256 | Algorithm::ES256 | Algorithm::EdDSA => {
                let kid = header.kid.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("asymmetric JWT has no key id"))?;
                let mut jwk = self.jwks.read().await.find(kid).cloned();
                if jwk.is_none() {
                    self.refresh_jwks().await?;
                    jwk = self.jwks.read().await.find(kid).cloned();
                }
                let jwk = jwk.ok_or_else(|| anyhow::anyhow!("JWT signing key is unknown"))?;
                DecodingKey::from_jwk(&jwk)?
            }
            _ => anyhow::bail!("JWT signing algorithm is not allowed"),
        };

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

    match verifier.verify(token).await {
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
