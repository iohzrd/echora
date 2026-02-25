# CLAUDE.md

## Rules

- Always thoroghly study all existing code relavant to your current task before offering changes.
- **NEVER USE EMOJIS** in code, documentation, commits, anywhere
- Always use latest dependency versions when updating
- Always run code formatters after making changes (`cargo fmt` for backend, `npm run check` and `npx prettier --plugin prettier-plugin-svelte --write "src/**/*.{ts,svelte}" 2>&1` for frontend)
- **Infra**: ALWAYS check what already exists before creating ANY resources. List/describe first, then create only if needed.
- Always omit Claude signature when writing commit messages.

## Commands

- When the user says "review and commit", this means review ALL the uncommited changes with git diff, then commit.
- When the user says "review and report", this means re-inspect ALL the code relevant to the current task and revise your list on current options.

## Development

```bash
# Backend
cd backend && cargo run          # Dev server on http://127.0.0.1:3000
cd backend && cargo fmt          # Format
cd backend && cargo clippy       # Lint
cd backend && cargo check        # Type check

# Frontend
cd frontend && npm run dev       # Dev server on http://localhost:1420
cd frontend && npm run check     # Type check
cd frontend && npm run build     # Production build

# Local database
docker compose up -d             # PostgreSQL on localhost:5432

# Deploy backend (build + push container to registry, then trigger redeployment)
docker build -t echocell .
# Tag, push to your container registry, and trigger service redeployment
# See deploy docs or infra-resources.md (local, gitignored) for specific commands

# Deploy frontend (build + sync to static hosting, then invalidate CDN cache)
cd frontend && npm run build
# Sync build/ to your static hosting and invalidate CDN cache
```
