use crate::models::{Asset, Transaction, TransactionType};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Portfolio {
    pub assets: HashMap<String, Asset>,
    pub transactions: Vec<Transaction>,
}

impl Portfolio {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstrói a carteira (ativos e preço médio) a partir de uma lista de transações de um usuário.
    /// Retorna (Ativos com saldo > 0, Transações ordenadas por data, Valor total investido).
    pub fn calculate(transactions: Vec<Transaction>) -> (Vec<Asset>, Vec<Transaction>, Decimal) {
        let mut user_assets: HashMap<String, Asset> = HashMap::new();
        
        // O cálculo do preço médio deve ser feito em ordem cronológica (mais antiga para mais recente).
        let mut sorted_transactions = transactions;
        sorted_transactions.sort_by(|a, b| a.date.cmp(&b.date));

        for t in &sorted_transactions {
            let asset = user_assets
                .entry(t.ticker.clone())
                .or_insert_with(|| Asset::new(&t.ticker));
            match t.transaction_type {
                TransactionType::Buy => {
                    let total = asset.quantity * asset.average_price + t.quantity * t.unit_price;
                    asset.quantity += t.quantity;
                    asset.average_price = if asset.quantity > Decimal::ZERO {
                        total / asset.quantity
                    } else {
                        Decimal::ZERO
                    };
                }
                TransactionType::Sell => {
                    // Previne saldo negativo em memória, mas no fluxo real o banco valida isso.
                    if asset.quantity >= t.quantity {
                        asset.quantity -= t.quantity;
                    } else {
                        asset.quantity = Decimal::ZERO;
                    }
                }
            }
        }

        let mut assets: Vec<Asset> = user_assets
            .into_values()
            .filter(|a| a.quantity > Decimal::ZERO)
            .collect();
        assets.sort_by(|a, b| a.ticker.cmp(&b.ticker));

        let total_invested: Decimal = assets.iter().map(|a| a.quantity * a.average_price).sum();

        (assets, sorted_transactions, total_invested)
    }

    /// Verifica se o usuário possui saldo suficiente de um ativo para realizar uma venda.
    pub fn has_sufficient_balance(transactions: &[Transaction], ticker: &str, quantity_to_sell: Decimal) -> bool {
        let mut balance = Decimal::ZERO;
        for t in transactions {
            if t.ticker.eq_ignore_ascii_case(ticker) {
                match t.transaction_type {
                    TransactionType::Buy => balance += t.quantity,
                    TransactionType::Sell => balance -= t.quantity,
                }
            }
        }
        balance >= quantity_to_sell
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[test]
    fn test_calculate_portfolio() {
        let user_id = Uuid::new_v4();
        let t1 = Transaction {
            id: Uuid::new_v4(),
            user_id,
            ticker: "AAPL".to_string(),
            transaction_type: TransactionType::Buy,
            quantity: dec!(10),
            unit_price: dec!(150.0),
            date: Utc::now() - chrono::Duration::days(2),
        };
        let t2 = Transaction {
            id: Uuid::new_v4(),
            user_id,
            ticker: "AAPL".to_string(),
            transaction_type: TransactionType::Buy,
            quantity: dec!(5),
            unit_price: dec!(160.0),
            date: Utc::now() - chrono::Duration::days(1),
        };
        
        let (assets, _, total_invested) = Portfolio::calculate(vec![t1, t2]);
        
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].ticker, "AAPL");
        assert_eq!(assets[0].quantity, dec!(15));
        // Preço médio ponderado: ((10 * 150) + (5 * 160)) / 15 = (1500 + 800) / 15 = 2300 / 15 = 153.3333333333
        assert_eq!(assets[0].average_price, dec!(2300) / dec!(15));
        assert_eq!(total_invested, dec!(2300));
    }

    #[test]
    fn test_has_sufficient_balance() {
        let user_id = Uuid::new_v4();
        let transactions = vec![
            Transaction {
                id: Uuid::new_v4(),
                user_id,
                ticker: "AAPL".to_string(),
                transaction_type: TransactionType::Buy,
                quantity: dec!(10),
                unit_price: dec!(150.0),
                date: Utc::now(),
            }
        ];
        
        assert!(Portfolio::has_sufficient_balance(&transactions, "AAPL", dec!(5)));
        assert!(Portfolio::has_sufficient_balance(&transactions, "AAPL", dec!(10)));
        assert!(!Portfolio::has_sufficient_balance(&transactions, "AAPL", dec!(15)));
    }
}
