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
