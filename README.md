# 📊 Carteira Inteligente de Investimentos

Uma aplicação web full-stack de alta performance desenvolvida em **Rust** para gerenciamento e consolidação de carteiras de investimentos em ações. O sistema realiza o cálculo automático de preço médio ponderado, validação de saldos e integração com cotações de mercado em tempo real.

---

## 🚀 Tecnologias Utilizadas

- **Linguagem**: [Rust (Edition 2024)](https://www.rust-lang.org/)
- **Backend / Servidor Web**: [Axum 0.8](https://github.com/tokio-rs/axum) (assíncrono, robusto e seguro)
- **Banco de Dados**: [SQLite](https://sqlite.org/) via [SQLx](https://github.com/launchbadge/sqlx) (com migrações automáticas de esquema e consultas preparadas seguras)
- **Engine de Templates**: [Askama](https://github.com/djc/askama) (HTML compilado em tempo de build, livre de erros de renderização em runtime)
- **Segurança & Autenticação**:
  - Hashing de senhas com [Argon2id](https://en.wikipedia.org/wiki/Argon2)
  - Tokens de sessão baseados em **JWT** (JSON Web Tokens)
  - Cookies seguros `HttpOnly` com flag `SameSite::Lax`
- **Cotações**: Integração com a API [brapi.dev](https://brapi.dev/) (suporta ativos brasileiros da B3 e globais)
- **Precisão Numérica**: Uso de `rust_decimal` para evitar erros de ponto flutuante em valores financeiros.

---

## ✨ Funcionalidades

- 🔐 **Autenticação Segura**: Fluxo completo de cadastro de usuário e login com criptografia de ponta a ponta.
- 💼 **Gestão de Ativos**:
  - Cadastro de transações de Compra (`Buy`) e Venda (`Sell`).
  - Consolidação automática do portfólio.
  - Cálculo de **Preço Médio Ponderado** conforme novas compras ocorrem.
- 📉 **Validação de Saldo**: O sistema impede a venda de ativos que você não possui em carteira.
- 📊 **Dashboard Dinâmico**:
  - Indicadores globais: Total Investido, Patrimônio Atual, Retorno Total (absoluto e percentual).
  - Cards detalhados de cada ativo contendo quantidade, preço médio, cotação atual, valor de mercado e lucro/prejuízo.
  - Histórico cronológico completo de todas as operações realizadas pelo usuário.

---

## ⚙️ Configuração e Instalação

### Pré-requisitos

1. Ter o **Rust (rustup)** instalado em sua máquina.
2. Criar uma conta gratuita na [brapi.dev](https://brapi.dev/dashboard) para obter um token de cotações em tempo real.

### Passos para Rodar Localmente

1. **Clonar o Repositório**:
   ```bash
   git clone <URL_DO_REPOSITORIO>
   cd "Carteira Rust"
   ```

2. **Configurar as Variáveis de Ambiente**:
   Crie um arquivo `.env` na raiz do projeto contendo as seguintes definições (ou edite o arquivo existente):
   ```env
   JWT_SECRET=sua_chave_secreta_super_segura
   BRAPI_TOKEN=seu_token_da_brapi_aqui
   DATABASE_URL=sqlite://carteira.db
   ```

3. **Compilar e Executar a Aplicação**:
   O SQLx criará e rodará automaticamente todas as migrações necessárias no banco SQLite local (`carteira.db`) na primeira execução:
   ```bash
   cargo run
   ```

4. **Acessar no Navegador**:
   Abra a aplicação em: [http://127.0.0.1:3000](http://127.0.0.1:3000)

---

## 🧪 Testes Automatizados

O projeto conta com testes unitários para a lógica crítica de cálculos financeiros e validação de saldo no portfólio. Para rodá-los:

```bash
cargo test
```

---

## 📂 Estrutura do Projeto

```text
├── migrations/          # Migrações SQLx de banco de dados
├── src/
│   ├── main.rs          # Inicialização, roteamento HTTP e handlers Axum
│   ├── auth.rs          # Middleware de autenticação, hashing Argon2 e JWT
│   ├── models.rs        # Estruturas de dados (User, Transaction, Asset, etc.)
│   ├── portfolio.rs     # Lógica central de preço médio ponderado e saldo
│   └── quotes.rs        # Cliente HTTP para busca de cotações na API brapi.dev
├── templates/           # Templates HTML compilados pelo Askama (SSR)
├── Cargo.toml           # Dependências e configuração do projeto
└── README.md            # Documentação principal
```
