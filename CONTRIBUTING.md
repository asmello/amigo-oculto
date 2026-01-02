# Contributing

## Branch Protection

A branch `main` está protegida - push direto não é permitido. Todas as mudanças devem passar por pull requests.

## CI Pipeline

Toda PR para `main` executa automaticamente os seguintes checks:

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

### Staging

Commits na branch `main` disparam automaticamente:

1. Re-execução de todos os checks de CI
2. Deploy para Fly.io (app: `amigo-oculto-staging`)

O deploy usa o secret `FLY_API_TOKEN` configurado no GitHub.

### Deploy Manual

```bash
flyctl deploy --remote-only
```

## Dependabot

Atualizações de dependências são verificadas semanalmente:

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
