-- Índice para otimizar consultas de transações por usuário
CREATE INDEX IF NOT EXISTS idx_transactions_user_id ON transactions(user_id);
