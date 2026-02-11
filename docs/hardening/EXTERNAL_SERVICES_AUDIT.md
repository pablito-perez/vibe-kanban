# External Services Audit Report

This document lists all external services that Vibe Kanban calls out to, organized by category.

## 1. Analytics & Telemetry

### PostHog
- **Frontend**: `frontend/src/main.tsx` initializes PostHog with API key and endpoint
- **Backend**: `crates/services/src/services/analytics.rs` - sends events to PostHog
- **Environment Variables**:
  - `POSTHOG_API_KEY`
  - `POSTHOG_API_ENDPOINT`
  - `VITE_POSTHOG_API_KEY` (frontend)
  - `VITE_POSTHOG_API_ENDPOINT` (frontend)
- **Usage**: User behavior analytics, event tracking, telemetry
- **Files**:
  - `frontend/src/main.tsx`
  - `frontend/src/App.tsx`
  - `crates/services/src/services/analytics.rs`
  - `crates/server/build.rs`

## 2. Error Tracking

### Sentry
- **DSNs**:
  - Default: `https://1065a1d276a581316999a07d5dffee26@o4509603705192449.ingest.de.sentry.io/4509605576441937`
  - Remote: `https://d6e4c45af2b081fadb10fb0ba726ccaf@o4509603705192449.ingest.de.sentry.io/4510305669283920`
- **Frontend**: `frontend/src/main.tsx` - React error boundary
- **Backend**: `crates/utils/src/sentry.rs` - error logging
- **Files**:
  - `frontend/src/main.tsx`
  - `frontend/src/App.tsx`
  - `frontend/vite.config.ts` (Sentry Vite plugin)
  - `crates/utils/src/sentry.rs`
  - `crates/server/src/main.rs`
  - `crates/server/src/bin/mcp_task_server.rs`
  - `crates/remote/src/main.rs`
- **Dependencies**:
  - `@sentry/react` (frontend)
  - `@sentry/vite-plugin` (frontend)
  - `sentry` crate (backend)
  - `sentry-tracing` crate (backend)

## 3. Community/Social

### Discord
- **API**: `https://discord.com/api/guilds/{GUILD_ID}/widget.json`
- **Guild ID**: `1423630976524877857`
- **Usage**: Fetches online member count for display in UI
- **Files**:
  - `frontend/src/hooks/useDiscordOnlineCount.ts`
  - `frontend/src/components/layout/Navbar.tsx`
  - `frontend/src/components/ui-new/primitives/AppBar.tsx`

### GitHub
- **API**: `https://api.github.com/repos/BloopAI/vibe-kanban`
- **Usage**: Fetches star count for display in UI
- **Files**:
  - `frontend/src/hooks/useGitHubStars.ts`

## 4. Documentation & Website Links

### vibekanban.com
- **Release Notes**: `https://vibekanban.com/release-notes`
- **Documentation**: `https://vibekanban.com/docs`
- **Various Docs Pages**:
  - Getting started: `https://www.vibekanban.com/docs/getting-started`
  - Safety notice: `https://www.vibekanban.com/docs/getting-started#safety-notice`
  - Testing: `https://www.vibekanban.com/docs/core-features/testing-your-application`
  - Configuration: `https://vibekanban.com/docs/configuration-customisation/global-settings#remote-ssh-configuration`
- **Files**:
  - `frontend/src/components/dialogs/global/ReleaseNotesDialog.tsx`
  - `frontend/src/components/dialogs/global/DisclaimerDialog.tsx`
  - `frontend/src/components/layout/Navbar.tsx`
  - `frontend/src/components/ui-new/views/PreviewBrowser.tsx`
  - `remote-frontend/src/pages/HomePage.tsx`
  - `docs/agents/*.mdx` (logo URLs)
  - `README.md`

## 5. Remote/Cloud Services (crates/remote)

### Remote API Backend
- **Environment Variable**: `VK_SHARED_API_BASE` / `VITE_VK_SHARED_API_BASE`
- **Default**: `https://api.vibekanban.com`
- **Usage**: Remote project collaboration, workspaces, issues
- **Files**:
  - `frontend/src/lib/remoteApi.ts`
  - `crates/local-deployment/src/lib.rs`
  - `crates/server/build.rs`
  - `crates/review/src/main.rs`
  - `local-build.sh`

### ElectricSQL
- **Environment Variable**: `ELECTRIC_URL`
- **Usage**: Real-time database synchronization
- **Files**:
  - `crates/remote/src/config.rs`
  - `crates/remote/src/routes/electric_proxy.rs`
  - `frontend/src/lib/electric/collections.ts`

### OAuth Providers
- **GitHub OAuth**:
  - `GITHUB_OAUTH_CLIENT_ID`
  - `GITHUB_OAUTH_CLIENT_SECRET`
- **Google OAuth**:
  - `GOOGLE_OAUTH_CLIENT_ID`
  - `GOOGLE_OAUTH_CLIENT_SECRET`
- **Files**:
  - `crates/remote/src/config.rs`
  - `crates/remote/src/auth/provider.rs`
  - `crates/remote/src/routes/oauth.rs`

### GitHub App Integration
- **API**: `https://api.github.com`
- **Environment Variables**:
  - `GITHUB_APP_ID`
  - `GITHUB_APP_PRIVATE_KEY`
  - `GITHUB_APP_WEBHOOK_SECRET`
  - `GITHUB_APP_SLUG`
- **Usage**: GitHub PR reviews, GitHub App webhooks
- **Files**:
  - `crates/remote/src/config.rs`
  - `crates/remote/src/github_app/service.rs`
  - `crates/remote/src/github_app/pr_review.rs`
  - `crates/remote/src/routes/github_app.rs`
  - `crates/review/src/github.rs`

### Email Service (Loops)
- **API**: `https://app.loops.so/api/v1/transactional`
- **Environment Variable**: `LOOPS_EMAIL_API_KEY`
- **Template IDs**:
  - Invitation: `cmhvy2wgs3s13z70i1pxakij9`
  - Review Ready: `cmj47k5ge16990iylued9by17`
  - Review Failed: `cmj49ougk1c8s0iznavijdqpo`
- **Usage**: Transactional emails for org invitations, PR review notifications
- **Files**:
  - `crates/remote/src/mail.rs`
  - `crates/remote/src/app.rs`

### Cloudflare R2 Storage
- **Environment Variables**:
  - `R2_ACCESS_KEY_ID`
  - `R2_SECRET_ACCESS_KEY`
  - `R2_REVIEW_ENDPOINT`
  - `R2_REVIEW_BUCKET`
  - `R2_PRESIGN_EXPIRY_SECS`
- **Usage**: File storage for reviews
- **Files**:
  - `crates/remote/src/config.rs`

### Stripe Payment Processing (Feature Flag: vk-billing)
- **Environment Variables** (when `vk-billing` feature enabled):
  - Stripe secret key
  - Stripe price ID
  - Stripe webhook secret
- **API**: Stripe payment webhooks
- **Files**:
  - `crates/remote/src/billing.rs`
  - `crates/remote/src/routes/billing.rs`
  - `crates/remote/src/main.rs`

## 6. Git Configuration

### Default Git Identity
- **Name**: "Vibe Kanban"
- **Email**: `noreply@vibekanban.com`
- **Usage**: Default git commit author when user hasn't configured git
- **Files**:
  - `crates/git/src/lib.rs`
  - `crates/git/tests/git_workflow.rs`

## 7. Default Prompts & Branding

### PR Description Template
- Contains: "This PR was written using [Vibe Kanban](https://vibekanban.com)"
- **Files**:
  - `shared/types.ts`
  - `crates/services/src/services/config/mod.rs`

---

## Summary of Services to Remove for Local-Only Tool

### Required Removals:
1. **Sentry** - Remove error tracking
2. **PostHog** - Remove analytics
3. **Discord API** - Remove online count widget
4. **GitHub stars API** - Remove star count widget
5. **vibekanban.com links** - Replace with local docs or remove
6. **Remote API** (`VK_SHARED_API_BASE`) - Remove remote features
7. **ElectricSQL** - Remove if not using remote sync
8. **OAuth providers** - Remove GitHub/Google OAuth
9. **GitHub App** - Remove if not needed for PR reviews
10. **Loops email** - Remove transactional emails
11. **Cloudflare R2** - Remove cloud storage
12. **Stripe billing** - Remove payment processing

### Optional Removals:
- Change default git email from `noreply@vibekanban.com` to generic or configurable
- Remove PR description branding mentioning vibekanban.com

### Crates to Exclude/Remove:
- `crates/remote` - Entire remote server functionality
- `crates/review` - PR review service
- `crates/deployment` - Deployment scripts
- `remote-frontend/` - Remote deployment frontend

### Dependencies to Remove:
**Frontend:**
- `@sentry/react`
- `@sentry/vite-plugin`
- `posthog-js`

**Backend:**
- `sentry` crate
- `sentry-tracing` crate
- Remove `reqwest` if only used for external calls
