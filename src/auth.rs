use crate::models::{Claims, User};
use anyhow::Result;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use std::future::Future;

const TOKEN_EXPIRY_HOURS: i64 = 24;

/// Faz o hash seguro de uma senha usando Argon2id
pub fn hash_password(password: &str) -> Result<String> {
    // Gerar sal via getrandom diretamente (já presente no dependency tree)
    let mut salt_bytes = [0u8; 16];
    getrandom::fill(&mut salt_bytes)
        .map_err(|e| anyhow::anyhow!("Erro ao gerar bytes aleatórios: {}", e))?;
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|e| anyhow::anyhow!("Erro ao codificar salt: {}", e))?;

    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Falha ao gerar hash da senha: {}", e))?
        .to_string();
    Ok(hash)
}

/// Verifica se uma senha bate com um hash Argon2
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("Hash inválido: {}", e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Gera um JWT assinado com as informações do usuário
pub fn generate_token(user: &User, secret: &str) -> Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(chrono::Duration::hours(TOKEN_EXPIRY_HOURS))
        .expect("Invalid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!("Falha ao gerar JWT: {}", e))?;

    Ok(token)
}

/// Valida um JWT e retorna os Claims se válido
pub fn validate_token(token: &str, secret: &str) -> Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| anyhow::anyhow!("Token inválido: {}", e))?;
    Ok(token_data.claims)
}

// ===== Middleware de Autenticação para Axum 0.8 =====

/// Extrai o usuário autenticado de uma requisição.
/// Aceita autenticação via:
///   1. Cookie `token` (usado pelo frontend SSR/formulários HTML)
///   2. Header `Authorization: Bearer <token>` (usado pela API REST)
/// Falha com 401 se nenhum token válido for encontrado.
pub struct AuthUser(pub Claims);

#[derive(Debug)]
pub struct AuthError;

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            "Token inválido ou ausente. Faça login para continuar.",
        )
            .into_response()
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // 1. Tenta extrair token do header Authorization: Bearer <token>
        let bearer_token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|t| t.to_string());

        // 2. Tenta extrair token do cookie "token" (fallback para formulários HTML)
        let cookie_token = parts
            .headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies.split(';')
                    .map(|s| s.trim())
                    .find(|s| s.starts_with("token="))
                    .map(|s| s.strip_prefix("token=").unwrap_or("").to_string())
            });

        async move {
            let secret = std::env::var("JWT_SECRET").unwrap_or_default();
            // Prioridade: Bearer token > Cookie token
            let token = bearer_token.or(cookie_token).ok_or(AuthError)?;
            let claims = validate_token(&token, &secret).map_err(|_| AuthError)?;
            Ok(AuthUser(claims))
        }
    }
}
