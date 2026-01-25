# Contributing

## Branch Protection

As branches `main` e `stage` estão protegidas - push direto não é permitido. Todas as mudanças devem passar por pull requests.

## Workflow

1. Crie uma feature branch a partir de `stage`
2. Abra uma PR para `stage`
3. Após merge, as mudanças são deployadas automaticamente para staging
4. Periodicamente, `stage` é promovida para `main` via PR
5. Merge para `main` dispara deploy para produção

## CI Pipeline

Toda PR para `main` ou `stage` executa automaticamente os seguintes checks:

### Backend (Rust)

```bash
cargo fmt --check      # Formatação
cargo clippy -- -D warnings  # Linting
cargo test             # Testes unitários
```

### Frontend (SvelteKit)

```bash
pnpm run check         # TypeScript/Svelte validation
pnpm run build         # Build de produção
```

A PR só pode ser mergeada após todos os checks passarem.

## Deploy

O deploy é gerenciado pela integração nativa do Railway com GitHub:

- **Staging**: Commits na branch `stage` são automaticamente deployados
- **Production**: Commits na branch `main` são automaticamente deployados

### Deploy Manual (Railway CLI)

```bash
railway up --service amigo-oculto-staging  # Staging
railway up --service amigo-oculto          # Production
```

## Dependabot

Atualizações de dependências são verificadas semanalmente e direcionadas para a branch `stage`:

- **Cargo** - Dependências Rust
- **npm** - Dependências frontend
- **GitHub Actions** - Versões das actions

Updates de minor/patch são agrupados para reduzir o número de PRs.

## Desenvolvimento Local

Antes de abrir uma PR, verifique localmente:

```bash
# Backend
cd backend
cargo fmt
cargo clippy -- -D warnings
cargo test

# Frontend
cd frontend
pnpm run check
pnpm run build
```
