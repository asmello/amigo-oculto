# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Amigo Oculto is a Secret Santa (Sorteio de Amigo Oculto) web application with a Rust/Axum backend and TypeScript/SvelteKit frontend. The interface is in Brazilian Portuguese.

## Build & Development Commands

### Backend (Rust)
```bash
cd backend
cargo run                    # Development server
cargo check                  # Type-check without building
cargo test                   # Run matching algorithm tests
cargo build --release        # Production build
```

### Frontend (SvelteKit)
```bash
cd frontend
npm install                  # Install dependencies (uses pnpm)
npm run dev                  # Dev server on localhost:5173
npm run build                # Static build to ./build
npm run check                # Type-check and Svelte validation
```

### Docker
```bash
# Build frontend first, then run Docker
cd frontend && npm install && npm run build && cd ..
docker-compose up -d         # Start backend with Docker Compose
docker build -t amigo-oculto:latest .  # Full production build
```

## Architecture

**Backend** (`/backend/src/`):
- `main.rs` - Server entry point, routes setup, background tasks
- `routes.rs` - All API endpoint handlers
- `db.rs` - SQLite initialization and queries (sqlx)
- `models.rs` - Data structures (Game, Participant, EmailVerification)
- `matching.rs` - Secret Santa matching algorithm (Fisher-Yates shuffle)
- `email.rs` - SMTP email service (Lettre)
- `token.rs` - Typed newtypes for IDs, tokens, and domain values (EmailAddress, VerificationCode)
- `email_templates/` - HTML/plain text email templates using Maud

**Frontend** (`/frontend/src/`):
- `/routes/+page.svelte` - Home page, game creation with verification flow
- `/routes/jogo/[game_id]/` - Game management (add participants, perform draw)
- `/routes/admin/[admin_token]/` - Organizer dashboard
- `/routes/revelar/[view_token]/` - Participant match reveal page

**Data Flow**:
1. Organizer requests email verification → Creates game with admin_token
2. Organizer adds participants via `/jogo/[game_id]`
3. POST `/api/games/{game_id}/draw` executes matching, sends emails
4. Participants receive unique `view_token` links to reveal their match

**Security Model**: Token-based access (no authentication). Admin tokens for organizers, view tokens for participants. Organizers cannot see matched pairs.

## Database

SQLite database at `./data/amigo_oculto.db` (auto-created). Tables:
- `games` - Event info, organizer_email, admin_token, drawn status
- `participants` - Name, email, matched_with_id, view_token, has_viewed
- `email_verifications` - Verification codes with expiry and attempt tracking
- `email_resends` - Rate limiting for resend operations

## Environment Variables

Required in `/backend/.env`:
```
DATABASE_URL=sqlite:./data/amigo_oculto.db
PORT=3000
BASE_URL=http://localhost:3000
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=your@gmail.com
SMTP_PASSWORD=app-password
SMTP_FROM=your@gmail.com
STATIC_DIR=../frontend/build
```

## API Routes (prefix: `/api`)

- `POST /verifications/request` - Request email verification code
- `POST /verifications/verify` - Verify code and create game
- `POST /games` - Direct game creation
- `GET /games/{game_id}?admin_token=xxx` - Get game details
- `POST /games/{game_id}/participants` - Add participant
- `PATCH /games/{game_id}/participants/{id}` - Edit participant
- `POST /games/{game_id}/draw` - Execute Secret Santa matching
- `GET /reveal/{view_token}` - Get participant's match

## Style guidance

The Rust code should favour type safety. Whenever a value represents an identifier, token, or domain-specific value, it should use the newtype pattern instead of primitive types like `String` or `&str`. Examples in this codebase:

- **IDs**: `GameId`, `ParticipantId`, `VerificationId` (wrap ULID)
- **Tokens**: `AdminToken`, `ViewToken`, `AdminSessionToken` (wrap String, 32-char alphanumeric)
- **Domain values**: `EmailAddress` (wraps `lettre::address::Address`), `VerificationCode` (6-digit numeric, uses `[u8; 6]`)

Newtypes should implement validation in `FromStr` and serde's `Deserialize`, so invalid values are rejected at API boundaries during JSON deserialization (resulting in HTTP 422). This is preferable to accepting invalid data and failing later during processing. For values that are `Copy` (like `VerificationCode`), pass by value rather than by reference.

Similarly, dates and other values that are typically represented as strings when serialized should internally be held in data structures as unambiguous types such as `chrono::DateTime`.

Use unsigned integer types where semantically appropriate. Counts, pagination parameters (limit, offset), and other naturally non-negative values should use unsigned types (`u32`, `u64`, `usize`) rather than signed types. This makes the API more self-documenting and prevents invalid negative values. Note that SQLite returns signed integers (`i64`), so conversion at the database boundary may be necessary.

Prefer `TryFrom`/`TryInto` over the `as` keyword for type conversions. The `as` keyword performs potentially lossy conversions silently, while `TryFrom` makes failure explicit and allows proper error handling. For infallible conversions (like `usize` to `u64` on 64-bit platforms), use `From`/`Into`. Only use `as` for primitive numeric widening that cannot fail (e.g., `u32 as u64`).

Error handling should prioritise context. We use `anyhow`, which provides the `Context` trait for annotating fallible operations with surrounding context of their execution. Generally, this should describe what the code was doing when it failed - for example, "fetching the user from the database" or "parsing the value as an admin token". This context should be surfaced in server logs, but generally shouldn't be exposed directly through the API in the form of HTTP responses. That said, the error codes and responses should be useful for clients to determine what they did wrong when they are the cause of the error.

### Panicking

We adhere to the convention that panics should only happen when there's a bug in our application. At the same time, when an invariant we rely upon is violated, we should always panic rather than continue the program. A caveat is that a panic that happens when handling a request will typically be intercepted by axum, and won't cause the whole server to crash. Because of this, if any bugs would cause state corruption that persists across requests, they should trigger a graceful shutdown of the server (or immediately abort, depending on severity).

## CI/CD & GitHub Workflow

### Branch Protection

The `main` branch is protected with the following rules:
- **No direct pushes** - All changes must go through pull requests
- **Signed commits required** - All commits must be GPG/SSH signed
- **CI checks must pass** - PRs cannot be merged until all checks succeed

### Making Changes

Always work on a feature branch and create a PR:

```bash
git checkout -b my-feature
# make changes
cargo fmt --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml -- -D warnings
git add -A && git commit -m "feat: description"
git push -u origin my-feature
gh pr create --title "feat: description" --body "Summary of changes"
```

After CI passes, merge with squash: `gh pr merge --squash`

### CI Checks (on PRs)

- `fmt` - Rust formatting check
- `clippy / stable` - Linting on stable Rust
- `clippy / beta` - Linting on beta Rust (catches upcoming issues)
- `test` - Unit tests
- `frontend` - TypeScript check and build

### Automatic Deployment

Commits to `main` automatically deploy to Fly.io staging (`amigo-oculto-staging`).

### Scheduled Builds

Daily builds run at 7:00 UTC to catch breakages from:
- Rust nightly changes
- Dependency updates

### Dependabot

Weekly updates for Cargo, npm, and GitHub Actions. Major version bumps may require manual migration (check CI logs for breaking changes).
