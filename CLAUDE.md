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
- `token.rs` - Secure 32-char alphanumeric token generation
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

The Rust code should favour type safety. Whenever a value represents an identifier or a token, it should use the newtype pattern, instead of simply be defined as a `String` or `&str` type, for example. Similarly, dates and other values that are typically represented as strings when serialized in API boundaries should internally be held in data structures as unambiguous types such as `chrono::DateTime`. 

Error handling should prioritise context. We use `anyhow`, which provides the `Context` trait for annotating fallible operations with surrounding context of their execution. Generally, this should describe what the code was doing when it failed - for example, "fetching the user from the database" or "parsing the value as an admin token". This context should be surfaced in server logs, but generally shouldn't be exposed directly through the API in the form of HTTP responses. That said, the error codes and responses should be useful for clients to determine what they did wrong when they are the cause of the error.

### Panicking

We adhere to the convention that panics should only happen when there's a bug in our application. At the same time, when an invariant we rely upon is violated, we should always panic rather than continue the program. A caveat is that a panic that happens when handling a request will typically be intercepted by axum, and won't cause the whole server to crash. Because of this, if any bugs would cause state corruption that persists across requests, they should trigger a graceful shutdown of the server (or immediately abort, depending on severity).
