use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ===== Transações e Ativos =====

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "UPPERCASE")]
pub enum TransactionType {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub ticker: String,
    pub transaction_type: TransactionType,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub date: DateTime<Utc>,
}

impl Transaction {
    pub fn is_buy(&self) -> bool {
        matches!(self.transaction_type, TransactionType::Buy)
    }

    pub fn transaction_type_str(&self) -> &'static str {
        match self.transaction_type {
            TransactionType::Buy => "COMPRA",
            TransactionType::Sell => "VENDA",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub ticker: String,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub current_price: Option<Decimal>,
    pub profit_loss: Option<Decimal>,
    pub profit_loss_pct: Option<Decimal>,
}

impl Asset {
    pub fn new(ticker: &str) -> Self {
        Self {
            ticker: ticker.to_string(),
            quantity: Decimal::ZERO,
            average_price: Decimal::ZERO,
            current_price: None,
            profit_loss: None,
            profit_loss_pct: None,
        }
    }
}

// ===== Usuários e Autenticação =====

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

/// Claims embutidos no JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// user_id (armazenado como String no JWT payload)
    pub sub: String,
    pub email: String,
    /// expiration timestamp (Unix)
    pub exp: usize,
}

/// Payload de registro
#[derive(Debug, Deserialize)]
pub struct RegisterPayload {
    pub email: String,
    pub password: String,
}

/// Payload de login
#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub email: String,
    pub password: String,
}

/// Resposta de login bem-sucedido
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub email: String,
}
