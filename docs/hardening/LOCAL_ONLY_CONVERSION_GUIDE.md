# Local-Only Conversion Guide

This guide provides step-by-step instructions to strip all external service dependencies from Vibe Kanban, converting it into a fully local-only tool.

## Phase 1: Remove Analytics & Telemetry

### 1.1 Remove PostHog (Frontend)

**Edit `frontend/src/main.tsx`:**
```typescript
// Remove these imports:
import posthog from 'posthog-js';
import { PostHogProvider } from 'posthog-js/react';

// Remove PostHog initialization (lines ~46-56)
// Remove PostHogProvider wrapper from render
```

**Edit `frontend/src/App.tsx`:**
```typescript
// Remove:
import { usePostHog } from 'posthog-js/react';

// Remove in AppContent():
const posthog = usePostHog();

// Remove the entire useEffect for analytics_enabled (lines ~62-73)
```

**Edit `frontend/package.json`:**
```json
// Remove dependency:
"posthog-js": "^1.276.0",
```

### 1.2 Remove PostHog (Backend)

**Edit `crates/services/src/services/analytics.rs`:**
- Delete the entire file or comment out all code

**Remove from code that uses analytics:**
- `crates/server/build.rs` - Remove POSTHOG env var handling
- Search for `AnalyticsService` and remove all instantiations

### 1.3 Remove Sentry Error Tracking

**Edit `frontend/src/main.tsx`:**
```typescript
// Remove import:
import * as Sentry from '@sentry/react';

// Remove Sentry.init() call (lines ~30-43)
// Remove Sentry.setTag()
// Remove Sentry.ErrorBoundary wrapper from render
```

**Edit `frontend/vite.config.ts`:**
```typescript
// Remove import:
import { sentryVitePlugin } from "@sentry/vite-plugin";

// Remove from plugins array:
sentryVitePlugin({ org: 'bloop-ai', project: 'vibe-kanban' }),
```

**Edit `frontend/package.json`:**
```json
// Remove dependencies:
"@sentry/react": "^9.34.0",
"@sentry/vite-plugin": "^3.5.0",
```

**Backend Sentry Removal:**

**Edit `crates/utils/src/lib.rs`:**
```rust
// Comment out or remove:
pub mod sentry;
```

**Edit `crates/server/src/main.rs`:**
```rust
// Remove sentry initialization:
utils::sentry::init_once(utils::sentry::SentrySource::Backend);
// Remove sentry layer from tracing
```

**Edit `crates/server/src/bin/mcp_task_server.rs`:**
```rust
// Remove sentry initialization
```

**Edit `crates/remote/src/main.rs`** (if keeping remote):
```rust
// Remove sentry initialization
```

**Edit `crates/server/Cargo.toml` and `crates/utils/Cargo.toml`:**
```toml
# Remove dependencies:
sentry = { ... }
sentry-tracing = { ... }
```

## Phase 2: Remove Social/Community Widgets

### 2.1 Remove Discord Widget

**Edit `frontend/src/hooks/useDiscordOnlineCount.ts`:**
- Delete the entire file

**Edit `frontend/src/components/layout/Navbar.tsx`:**
```typescript
// Remove import:
import { useDiscordOnlineCount } from '@/hooks/useDiscordOnlineCount';

// Remove usage of useDiscordOnlineCount()
// Remove Discord-related UI elements
```

**Edit `frontend/src/components/ui-new/primitives/AppBar.tsx`:**
```typescript
// Remove Discord widget code
```

### 2.2 Remove GitHub Stars Widget

**Edit `frontend/src/hooks/useGitHubStars.ts`:**
- Delete the entire file

**Search and remove all usages of `useGitHubStars()`**

## Phase 3: Remove/Localize Documentation Links

### 3.1 Update Release Notes

**Edit `frontend/src/components/dialogs/global/ReleaseNotesDialog.tsx`:**
```typescript
// Change:
const RELEASE_NOTES_BASE_URL = 'https://vibekanban.com/release-notes';
// To local path or remove feature entirely
```

### 3.2 Update Other vibekanban.com Links

**Files to update:**
- `frontend/src/components/dialogs/global/DisclaimerDialog.tsx`
- `frontend/src/components/layout/Navbar.tsx`
- `frontend/src/components/ui-new/views/PreviewBrowser.tsx`
- `remote-frontend/src/pages/HomePage.tsx`
- `README.md`

Replace all `https://vibekanban.com` links with:
- Local documentation paths
- Or remove the links entirely
- Or replace with your own documentation

### 3.3 Update PR Description Branding

**Edit `shared/types.ts`:**
```typescript
// In DEFAULT_PR_DESCRIPTION_PROMPT, remove or change:
"This PR was written using [Vibe Kanban](https://vibekanban.com)"
```

**Edit `crates/services/src/services/config/mod.rs`:**
```rust
// Update PR_DESCRIPTION_PROMPT similarly
```

### 3.4 Update Git Default Email

**Edit `crates/git/src/lib.rs`:**
```rust
// Change:
cfg.set_str("user.email", "noreply@vibekanban.com")?;
// To:
cfg.set_str("user.email", "noreply@localhost")?;
// Or make it configurable
```

## Phase 4: Remove Remote/Cloud Features

### 4.1 Remove Remote Crate (Optional if you don't need multi-user)

**Edit `Cargo.toml`:**
```toml
[workspace]
members = [
    # ... keep these ...
    # "crates/remote",  # REMOVE THIS
    # "crates/review",  # REMOVE THIS
    # "crates/deployment",  # REMOVE THIS (if not needed)
]
```

**Delete directories:**
```bash
rm -rf crates/remote
rm -rf crates/review
rm -rf remote-frontend
```

### 4.2 Remove Remote API Integration

**Edit `frontend/src/lib/remoteApi.ts`:**
```typescript
// Change:
export const REMOTE_API_URL = import.meta.env.VITE_VK_SHARED_API_BASE || '';
// To:
export const REMOTE_API_URL = '';
// Or add a check to disable remote features when empty
```

**Edit `crates/local-deployment/src/lib.rs`:**
```rust
// Remove VK_SHARED_API_BASE handling
// Or ensure it defaults to empty/disabled
```

**Edit `local-build.sh`:**
```bash
# Comment out:
# export VK_SHARED_API_BASE="https://api.vibekanban.com"
# export VITE_VK_SHARED_API_BASE="https://api.vibekanban.com"
```

### 4.3 Remove Electric SQL (if not using remote sync)

**Search for `ELECTRIC_URL` and remove related code**

**Frontend:**
- Remove `frontend/src/lib/electric/`
- Remove Electric-related contexts in `frontend/src/contexts/remote/`

**Backend:**
- Remove `crates/remote/src/routes/electric_proxy.rs`

### 4.4 Remove OAuth Providers

**If you removed `crates/remote`, this is already done.**

**Otherwise, in `crates/remote/src/config.rs`:**
```rust
// Comment out or remove OAuth config loading
```

## Phase 5: Environment Variable Cleanup

### 5.1 Create Local-Only .env Template

**Create `.env.local.template`:**
```bash
# Backend
BACKEND_PORT=3001
HOST=127.0.0.1

# Frontend
FRONTEND_PORT=3000
VITE_OPEN=false

# Database (keep for local SQLite)
DATABASE_URL=sqlite://./vibe-kanban.db

# Disable all external services
# (No Sentry, PostHog, Discord, GitHub, Remote API)
```

### 5.2 Update Documentation

**Edit `README.md`:**
- Remove references to remote features
- Update installation to reflect local-only setup
- Remove links to vibekanban.com docs

## Phase 6: Disable User Tracking Config

### 6.1 Remove Analytics Opt-in UI

**Edit config service files:**
- `crates/services/src/services/config/versions/*.rs`
- Remove `analytics_enabled` field or set to always false

**Edit frontend:**
- Remove analytics toggle from settings
- Remove analytics-related preferences

## Phase 7: Testing & Verification

### 7.1 Network Monitoring Test

```bash
# Run the app and monitor network calls
pnpm run dev

# In another terminal, monitor DNS requests:
sudo tcpdump -i any -n port 53 | grep -E "sentry|posthog|discord|github|vibekanban|loops|stripe"

# Should see NO requests to external services
```

### 7.2 Build Test

```bash
# Clean build to ensure no external dependencies
pnpm run clean
pnpm install
pnpm run build

# Check that it still builds without errors
```

### 7.3 Offline Test

```bash
# Disconnect from internet
# Start the app
pnpm run dev

# App should work fully offline (except for npm installs)
```

## Phase 8: Optional Cleanup

### 8.1 Remove Unused Dependencies

After removing code, run:
```bash
# Frontend
cd frontend
pnpm run check
npm-check-updates  # Check for unused dependencies

# Backend
cd ..
cargo machete  # Finds unused Cargo dependencies
cargo update
cargo build --release
```

### 8.2 Update Package Metadata

**Edit `frontend/package.json`:**
```json
{
  "name": "vibe-kanban-local",
  "description": "Local-only task management tool",
  // Update other metadata
}
```

**Edit `Cargo.toml`:**
```toml
[package]
name = "vibe-kanban-local"
description = "Local-only task management tool"
# Update metadata
```

## Phase 9: Security Hardening

### 9.1 Firewall Rules (Optional)

Create a script to ensure the app can't call external services:

```bash
#!/bin/bash
# block-external.sh
# Block specific domains (macOS example)

echo "127.0.0.1 sentry.io" | sudo tee -a /etc/hosts
echo "127.0.0.1 ingest.de.sentry.io" | sudo tee -a /etc/hosts
echo "127.0.0.1 discord.com" | sudo tee -a /etc/hosts
echo "127.0.0.1 api.github.com" | sudo tee -a /etc/hosts
echo "127.0.0.1 vibekanban.com" | sudo tee -a /etc/hosts
echo "127.0.0.1 api.vibekanban.com" | sudo tee -a /etc/hosts
echo "127.0.0.1 app.loops.so" | sudo tee -a /etc/hosts
echo "127.0.0.1 api.stripe.com" | sudo tee -a /etc/hosts

echo "External services blocked. Restart to take effect."
```

## Summary Checklist

- [ ] PostHog removed (frontend & backend)
- [ ] Sentry removed (frontend & backend)
- [ ] Discord widget removed
- [ ] GitHub stars widget removed
- [ ] vibekanban.com links updated/removed
- [ ] PR branding removed/updated
- [ ] Git default email updated
- [ ] Remote crate removed (if not needed)
- [ ] Remote API integration disabled
- [ ] ElectricSQL removed (if not needed)
- [ ] OAuth providers removed
- [ ] Environment variables cleaned up
- [ ] Documentation updated
- [ ] Analytics UI removed
- [ ] Network monitoring test passed
- [ ] Offline test passed
- [ ] Build test passed
- [ ] Unused dependencies removed

## Minimal Required Changes

If you want to do the **absolute minimum** to make it local-only:

1. **Comment out Sentry in `frontend/src/main.tsx`** (lines ~30-43)
2. **Comment out PostHog in `frontend/src/main.tsx`** (lines ~46-56)
3. **Remove `<PostHogProvider>` wrapper** in same file
4. **Comment out backend sentry init** in `crates/server/src/main.rs`
5. **Set `VK_SHARED_API_BASE=""` in environment** to disable remote features
6. **Skip `crates/remote` in workspace** by adding it to `exclude` in root `Cargo.toml`

This will disable most external calls while keeping the codebase relatively intact.
