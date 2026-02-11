# External Services - Quick Reference Card

## One-Page Cheat Sheet

### External Services Found

| # | Service | Type | Risk | Status | Action |
|---|---------|------|------|--------|--------|
| 1 | **Sentry** | Error tracking | 🔴 HIGH | Active | Remove |
| 2 | **PostHog** | Analytics | 🔴 HIGH | Active | Remove |
| 3 | **Remote API** | Cloud sync | 🔴 HIGH | Optional | Disable |
| 4 | **Discord** | Widget | 🟡 MED | Active | Remove |
| 5 | **GitHub** | Widget | 🟡 MED | Active | Remove |
| 6 | **Loops** | Email | 🟡 MED | Remote-only | Remove |
| 7 | **vibekanban.com** | Links | 🟢 LOW | Passive | Update |
| 8 | **ElectricSQL** | DB sync | 🔵 OPT | Remote-only | Keep/Remove |
| 9 | **OAuth** | Auth | 🔵 OPT | Remote-only | Keep/Remove |
| 10 | **GitHub App** | PR reviews | 🔵 OPT | Optional | Keep/Remove |
| 11 | **R2 Storage** | Files | 🔵 OPT | Remote-only | Keep/Remove |
| 12 | **Stripe** | Payments | 🔵 OPT | Feature-flag | Keep disabled |

### Quick Commands

```bash
# Automated removal (choose one):
./scripts/strip-external-services.sh minimal    # Just tracking
./scripts/strip-external-services.sh pragmatic  # Tracking + widgets
./scripts/strip-external-services.sh paranoid   # Everything

# Manual quick fix:
export VK_SHARED_API_BASE=""
export VITE_VK_SHARED_API_BASE=""

# Test offline:
sudo ifconfig en0 down  # Disconnect
pnpm run dev            # Should still work
sudo ifconfig en0 up    # Reconnect

# Monitor network:
sudo tcpdump -i any -n port 53 | grep -E "sentry|posthog|discord"
```

### Key Files to Edit

**Frontend (TypeScript):**
```
frontend/src/main.tsx                    # Sentry & PostHog init
frontend/src/App.tsx                     # PostHog usage
frontend/src/hooks/useDiscordOnlineCount.ts  # Discord widget
frontend/src/hooks/useGitHubStars.ts     # GitHub widget
frontend/src/lib/remoteApi.ts            # Remote API
frontend/vite.config.ts                  # Sentry plugin
```

**Backend (Rust):**
```
crates/utils/src/sentry.rs               # Sentry module
crates/services/src/services/analytics.rs # Analytics
crates/server/src/main.rs                # Sentry init
crates/server/src/bin/mcp_task_server.rs # Sentry init
crates/remote/                           # Entire remote server
```

**Config:**
```
.env                                     # Environment variables
frontend/package.json                    # npm dependencies
crates/*/Cargo.toml                      # Rust dependencies
```

### Environment Variables to Disable

```bash
# Analytics (set to empty)
POSTHOG_API_KEY=
POSTHOG_API_ENDPOINT=
VITE_POSTHOG_API_KEY=
VITE_POSTHOG_API_ENDPOINT=

# Remote features (set to empty)
VK_SHARED_API_BASE=
VITE_VK_SHARED_API_BASE=

# Remote server (don't set these)
# ELECTRIC_URL=
# GITHUB_OAUTH_CLIENT_ID=
# GOOGLE_OAUTH_CLIENT_ID=
# LOOPS_EMAIL_API_KEY=
```

### npm Packages to Remove

```bash
cd frontend
pnpm remove @sentry/react @sentry/vite-plugin posthog-js
pnpm install
```

### Cargo Dependencies to Remove

**Edit `crates/server/Cargo.toml` and `crates/utils/Cargo.toml`:**
```toml
# Remove these lines:
sentry = { ... }
sentry-tracing = { ... }
```

### Hardcoded Endpoints

Replace these URLs in code:

```
Sentry DSNs:
  https://1065a1d276a581316999a07d5dffee26@o4509603705192449.ingest.de.sentry.io/4509605576441937
  https://d6e4c45af2b081fadb10fb0ba726ccaf@o4509603705192449.ingest.de.sentry.io/4510305669283920

Discord:
  https://discord.com/api/guilds/1423630976524877857/widget.json

GitHub:
  https://api.github.com/repos/BloopAI/vibe-kanban

Loops:
  https://app.loops.so/api/v1/transactional

Remote:
  https://api.vibekanban.com

Docs:
  https://vibekanban.com/*
```

### Testing Checklist

- [ ] App starts successfully
- [ ] No console errors
- [ ] Create local project works
- [ ] Create task works
- [ ] Git operations work
- [ ] Executor runs work
- [ ] Works offline (disconnect internet)
- [ ] No external DNS requests
- [ ] Type checks pass: `pnpm run check`
- [ ] Build succeeds: `cargo build`

### Verification Commands

```bash
# Type check
pnpm run check

# Build check
cargo check --workspace

# Run tests
cargo test --workspace

# Start dev server
pnpm run dev

# Check for external URLs in code
rg "https://(sentry|posthog|discord|github\.com|vibekanban)" \
   --type rust --type ts | grep -v node_modules
```

### Rollback

If something breaks:

```bash
# Restore from backup
cd .external-services-backup-TIMESTAMP
cp -r * /path/to/project/

# Or restore git
git restore .
git clean -fd
```

### Strategy Comparison

| What | Minimal | Pragmatic | Paranoid |
|------|---------|-----------|----------|
| Time | 1-2h | 4-6h | 1-2d |
| Sentry | ❌ | ❌ | ❌ |
| PostHog | ❌ | ❌ | ❌ |
| Remote API | ❌ | ❌ | ❌ |
| Widgets | ✅ | ❌ | ❌ |
| Links | ✅ | ✅ | ⚠️ |
| Remote crate | ✅ | ✅ | ❌ |
| Effort | Low | Med | High |
| Result | 90% clean | 95% clean | 100% clean |

### Decision Tree

```
Do you need multi-user cloud features?
├─ YES → Keep remote crate, just remove Sentry/PostHog (Minimal)
└─ NO
   └─ Do you want completely offline?
      ├─ YES → Remove everything (Paranoid)
      └─ NO → Remove tracking + widgets (Pragmatic) ← RECOMMENDED
```

### Common Issues

**Issue**: Build fails after removing Sentry
**Fix**: Also remove sentry imports and layer setup

**Issue**: Type errors after removing PostHog
**Fix**: Remove PostHogProvider wrapper in main.tsx

**Issue**: "Module not found" errors
**Fix**: Remove from package.json and run `pnpm install`

**Issue**: App still calls external services
**Fix**: Check environment variables are unset

### Success Criteria

You're done when:
1. ✅ Offline test passes
2. ✅ No external DNS requests
3. ✅ No console warnings about failed connections
4. ✅ All features work locally
5. ✅ Build succeeds without warnings

### Need Help?

1. Check `EXTERNAL_SERVICES_AUDIT.md` for complete list
2. Check `LOCAL_ONLY_CONVERSION_GUIDE.md` for detailed steps
3. Check `EXTERNAL_SERVICES_PRIORITY.md` for strategy guidance
4. Check `AUDIT_SUMMARY.md` for overview

### Final Command

For most users, this is all you need:

```bash
./scripts/strip-external-services.sh pragmatic
pnpm run dev
# Test everything works
# Done! 🎉
```

---

**Quick Start**: Run the script, test offline, verify no external calls.
**Time**: 30 minutes to 6 hours depending on strategy.
**Risk**: Low (backups created automatically).
