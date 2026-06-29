mod auth;
mod models;
mod portfolio;
mod quotes;

use askama::Template;
use auth::AuthUser;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use models::{Asset, AuthResponse, LoginPayload, RegisterPayload, Transaction, TransactionType};
use portfolio::Portfolio;
use rust_decimal::Decimal;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
use std::str::FromStr;
use tracing::{error, info};
use uuid::Uuid;

// Estado compartilhado é a pool de conexões do SQLite
type AppState = SqlitePool;

#[derive(sqlx::FromRow)]
struct TransactionRow {
    id: Uuid,
    user_id: Uuid,
    ticker: String,
    transaction_type: TransactionType,
    quantity: String,
    unit_price: String,
    date: chrono::DateTime<chrono::Utc>,
}

impl From<TransactionRow> for Transaction {
    fn from(row: TransactionRow) -> Self {
        Transaction {
            id: row.id,
            user_id: row.user_id,
            ticker: row.ticker,
            transaction_type: row.transaction_type,
            quantity: std::str::FromStr::from_str(&row.quantity).unwrap_or_default(),
            unit_price: std::str::FromStr::from_str(&row.unit_price).unwrap_or_default(),
            date: row.date,
        }
    }
}

// ===== Templates Askama =====

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    assets: Vec<Asset>,
    transactions: Vec<Transaction>,
    total_invested: Decimal,
    total_current_value: Decimal,
    total_profit_loss: Decimal,
    total_profit_loss_pct: Decimal,
    user_email: String,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterTemplate;

// ===== Funções auxiliares (eliminação de duplicação) =====

/// Busca todas as transações de um usuário no banco de dados.
/// Reutilizada por render_dashboard, get_portfolio, get_transactions e add_transaction.
async fn fetch_user_transactions(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<Transaction>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TransactionRow>(
        "SELECT id, user_id, ticker, transaction_type, quantity, unit_price, date FROM transactions WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Transaction::from).collect())
}

/// Enriquece os ativos com cotações de mercado em tempo real.
/// Retorna (assets enriquecidos, total_current_value, total_profit_loss, total_profit_loss_pct).
async fn enrich_assets_with_quotes(
    mut assets: Vec<Asset>,
    total_invested: Decimal,
) -> (Vec<Asset>, Decimal, Decimal, Decimal) {
    let tickers: Vec<String> = assets.iter().map(|a| a.ticker.clone()).collect();
    let quotes = quotes::fetch_quotes(&tickers).await.unwrap_or_default();

    let mut total_current_value = Decimal::ZERO;
    for asset in &mut assets {
        if let Some(price) = quotes.get(&asset.ticker.to_uppercase()) {
            asset.current_price = Some(*price);
            let current_value = asset.quantity * price;
            total_current_value += current_value;
            let cost_basis = asset.quantity * asset.average_price;
            let profit_loss = current_value - cost_basis;
            asset.profit_loss = Some(profit_loss);
            asset.profit_loss_pct = if cost_basis > Decimal::ZERO {
                Some((profit_loss / cost_basis) * Decimal::from(100))
            } else {
                Some(Decimal::ZERO)
            };
        } else {
            // Fallback caso não encontre cotação: assume valor = custo
            total_current_value += asset.quantity * asset.average_price;
        }
    }

    let total_profit_loss = total_current_value - total_invested;
    let total_profit_loss_pct = if total_invested > Decimal::ZERO {
        (total_profit_loss / total_invested) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    (assets, total_current_value, total_profit_loss, total_profit_loss_pct)
}

// ===== Main =====

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://carteira.db".to_string());

    info!("🔌 Conectando ao banco de dados SQLite...");
    
    let options = SqliteConnectOptions::from_str(&database_url)
        .map_err(|e| anyhow::anyhow!("Erro na URL do banco: {}", e))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|e| anyhow::anyhow!("Erro ao conectar no banco SQLite: {}", e))?;

    info!("⚙️ Rodando migrações do banco de dados...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Erro ao rodar migrações: {}", e))?;

    let app = Router::new()
        // Páginas HTML
        .route("/", get(render_dashboard))
        .route("/login", get(render_login))
        .route("/register", get(render_register))
        // Auth
        .route("/auth/register", post(handle_register))
        .route("/auth/login", post(handle_login))
        .route("/auth/logout", post(handle_logout))
        // API (protegida)
        .route("/portfolio", get(get_portfolio))
        .route("/transactions", get(get_transactions).post(add_transaction))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    info!("🚀 Servidor rodando em http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}

// ===== Handlers HTML =====

async fn render_login() -> impl IntoResponse {
    render_template(LoginTemplate)
}

async fn render_register() -> impl IntoResponse {
    render_template(RegisterTemplate)
}

async fn render_dashboard(
    jar: CookieJar,
    State(pool): State<AppState>,
) -> Response {
    // Lê token do cookie "token"
    let token = match jar.get("token") {
        Some(c) => c.value().to_string(),
        None => return axum::response::Redirect::to("/login").into_response(),
    };

    let secret = std::env::var("JWT_SECRET").unwrap_or_default();
    let claims = match auth::validate_token(&token, &secret) {
        Ok(c) => c,
        Err(_) => return axum::response::Redirect::to("/login").into_response(),
    };

    let user_uuid = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return axum::response::Redirect::to("/login").into_response(),
    };

    let transactions = match fetch_user_transactions(&pool, user_uuid).await {
        Ok(t) => t,
        Err(e) => {
            error!("Erro ao buscar transações: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao carregar transações").into_response();
        }
    };

    let (assets, mut sorted_transactions, total_invested) = Portfolio::calculate(transactions);
    // Ordenar decrescentemente por data para mostrar no dashboard (mais recente primeiro)
    sorted_transactions.sort_by(|a, b| b.date.cmp(&a.date));

    let (assets, total_current_value, total_profit_loss, total_profit_loss_pct) =
        enrich_assets_with_quotes(assets, total_invested).await;

    render_template(DashboardTemplate {
        assets,
        transactions: sorted_transactions,
        total_invested,
        total_current_value,
        total_profit_loss,
        total_profit_loss_pct,
        user_email: claims.email,
    })
}

fn render_template<T: askama::Template>(t: T) -> Response {
    match t.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("Erro ao renderizar template: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao gerar página").into_response()
        }
    }
}

// ===== Auth Handlers =====

async fn handle_register(
    State(pool): State<AppState>,
    Json(payload): Json<RegisterPayload>,
) -> Response {
    // Validação de email (deve conter @)
    if !payload.email.contains('@') || payload.email.len() < 5 {
        return (StatusCode::BAD_REQUEST, "Email inválido.").into_response();
    }

    // Validação de senha (mínimo 8 caracteres)
    if payload.password.len() < 8 {
        return (StatusCode::BAD_REQUEST, "A senha deve ter ao menos 8 caracteres.").into_response();
    }

    let exists = match sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"
    )
    .bind(&payload.email)
    .fetch_one(&pool)
    .await {
        Ok(b) => b,
        Err(e) => {
            error!("Erro ao checar existência de usuário: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Erro interno no servidor").into_response();
        }
    };

    if exists {
        return (StatusCode::CONFLICT, "Email já cadastrado").into_response();
    }

    let password_hash = match auth::hash_password(&payload.password) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao processar senha").into_response(),
    };

    let user_id = Uuid::new_v4();
    if let Err(e) = sqlx::query(
        "INSERT INTO users (id, email, password_hash, created_at) VALUES ($1, $2, $3, $4)"
    )
    .bind(user_id)
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(chrono::Utc::now())
    .execute(&pool)
    .await {
        error!("Erro ao cadastrar usuário no banco: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao salvar usuário").into_response();
    }

    info!("✅ Novo usuário registrado: {}", payload.email);
    (StatusCode::CREATED, "Usuário criado com sucesso! Faça login.").into_response()
}

async fn handle_login(
    State(pool): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<LoginPayload>,
) -> Response {
    let secret = std::env::var("JWT_SECRET").unwrap_or_default();

    let user = match sqlx::query_as::<_, models::User>(
        "SELECT id, email, password_hash, created_at FROM users WHERE email = $1"
    )
    .bind(&payload.email)
    .fetch_optional(&pool)
    .await {
        Ok(Some(u)) => u,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "Email ou senha incorretos").into_response(),
        Err(e) => {
            error!("Erro ao buscar usuário no login: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Erro interno do servidor").into_response();
        }
    };

    match auth::verify_password(&payload.password, &user.password_hash) {
        Ok(true) => {}
        _ => return (StatusCode::UNAUTHORIZED, "Email ou senha incorretos").into_response(),
    }

    let token = match auth::generate_token(&user, &secret) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao gerar token").into_response(),
    };

    info!("✅ Login realizado: {}", user.email);

    // Define cookie HttpOnly com o token, com flags de segurança
    let cookie = Cookie::build(("token", token.clone()))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .secure(false) // Mudar para true em produção (HTTPS)
        .build();

    (
        jar.add(cookie),
        Json(AuthResponse {
            token,
            email: user.email,
        }),
    )
        .into_response()
}

async fn handle_logout(jar: CookieJar) -> impl IntoResponse {
    let jar = jar.remove(Cookie::from("token"));
    (jar, axum::response::Redirect::to("/login"))
}

// ===== API Handlers (protegidos) =====

async fn get_portfolio(
    AuthUser(claims): AuthUser,
    State(pool): State<AppState>,
) -> Response {
    let user_uuid = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Token inválido").into_response(),
    };

    let transactions = match fetch_user_transactions(&pool, user_uuid).await {
        Ok(t) => t,
        Err(e) => {
            error!("Erro ao buscar transações no get_portfolio: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Erro interno").into_response();
        }
    };

    let (assets, _, total_invested) = Portfolio::calculate(transactions);

    let (assets, total_current_value, total_profit_loss, total_profit_loss_pct) =
        enrich_assets_with_quotes(assets, total_invested).await;

    Json(serde_json::json!({
        "assets": assets,
        "total_invested": total_invested,
        "total_current_value": total_current_value,
        "total_profit_loss": total_profit_loss,
        "total_profit_loss_pct": total_profit_loss_pct
    })).into_response()
}

async fn get_transactions(
    AuthUser(claims): AuthUser,
    State(pool): State<AppState>,
) -> Response {
    let user_uuid = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Token inválido").into_response(),
    };

    let mut transactions = match fetch_user_transactions(&pool, user_uuid).await {
        Ok(t) => t,
        Err(e) => {
            error!("Erro ao buscar transações no get_transactions: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Erro interno").into_response();
        }
    };

    transactions.sort_by(|a, b| b.date.cmp(&a.date));
    Json(transactions).into_response()
}

#[derive(serde::Deserialize)]
struct AddTransactionPayload {
    ticker: String,
    transaction_type: TransactionType,
    quantity: rust_decimal::Decimal,
    unit_price: rust_decimal::Decimal,
}

async fn add_transaction(
    AuthUser(claims): AuthUser,
    State(pool): State<AppState>,
    Json(payload): Json<AddTransactionPayload>,
) -> Response {
    let user_uuid = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Token inválido").into_response(),
    };

    // Obter todas as transações para validar saldo
    let transactions = match fetch_user_transactions(&pool, user_uuid).await {
        Ok(t) => t,
        Err(e) => {
            error!("Erro ao buscar transações para validação: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Erro interno no servidor").into_response();
        }
    };

    if matches!(payload.transaction_type, TransactionType::Sell) {
        if !Portfolio::has_sufficient_balance(&transactions, &payload.ticker, payload.quantity) {
            return (
                StatusCode::BAD_REQUEST,
                format!("Saldo insuficiente de {} para venda.", payload.ticker.to_uppercase()),
            )
                .into_response();
        }
    }

    let transaction_id = Uuid::new_v4();
    if let Err(e) = sqlx::query(
        "INSERT INTO transactions (id, user_id, ticker, transaction_type, quantity, unit_price, date) VALUES ($1, $2, $3, $4, $5, $6, $7)"
    )
    .bind(transaction_id)
    .bind(user_uuid)
    .bind(payload.ticker.to_uppercase())
    .bind(&payload.transaction_type)
    .bind(payload.quantity.to_string())
    .bind(payload.unit_price.to_string())
    .bind(chrono::Utc::now())
    .execute(&pool)
    .await {
        error!("Erro ao inserir transação no banco: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Erro ao salvar transação").into_response();
    }

    info!("✅ Transação de {} adicionada para {}", payload.ticker, claims.email);
    (StatusCode::CREATED, "Transação criada com sucesso!").into_response()
}
