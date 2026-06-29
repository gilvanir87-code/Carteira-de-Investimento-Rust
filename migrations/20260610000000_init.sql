-- Criação da tabela de usuários
CREATE TABLE IF NOT EXISTS users (
    id BLOB PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Criação da tabela de transações vinculada ao usuário
CREATE TABLE IF NOT EXISTS transactions (
    id BLOB PRIMARY KEY,
    user_id BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ticker TEXT NOT NULL,
    transaction_type TEXT NOT NULL,
    quantity TEXT NOT NULL,
    unit_price TEXT NOT NULL,
    date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
