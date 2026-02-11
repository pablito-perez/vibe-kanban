# External Services Removal Priority Matrix

This document categorizes external services by priority and impact, helping you decide what to remove first for a local-only tool.

## Priority 1: CRITICAL - Remove Immediately (Data Leakage)

These services actively send your data to external servers:

### 🔴 Sentry (Error Tracking)
- **Risk**: HIGH - Sends error messages, stack traces, user IDs, environment info
- **Impact**: Every error is transmitted to sentry.io
- **Action**: Remove immediately
- **Effort**: Medium (3-4 files + dependencies)
- **Files**: `frontend/src/main.tsx`, `crates/utils/src/sentry.rs`, `crates/server/src/main.rs`

### 🔴 PostHog (Analytics)
- **Risk**: HIGH - Sends user behavior, events, telemetry, device info
- **Impact**: Tracks every user action when analytics_enabled
- **Action**: Remove immediately
- **Effort**: Medium (4-5 files + dependencies)
- **Files**: `frontend/src/main.tsx`, `frontend/src/App.tsx`, `crates/services/src/services/analytics.rs`

### 🔴 Remote API (vibekanban.com)
- **Risk**: MEDIUM-HIGH - Sends project/issue data if remote features used
- **Impact**: Only if user explicitly enables remote features
- **Action**: Disable by default, remove if not needed
- **Effort**: High (entire crate)
- **Files**: `crates/remote/`, `frontend/src/lib/remoteApi.ts`

## Priority 2: IMPORTANT - Remove for Privacy

These services leak non-sensitive metadata:

### 🟡 Discord API (Widget)
- **Risk**: LOW - Only fetches public guild member count
- **Impact**: Calls discord.com every 10 minutes
- **Action**: Remove (cosmetic feature)
- **Effort**: Low (1-2 files)
- **Files**: `frontend/src/hooks/useDiscordOnlineCount.ts`

### 🟡 GitHub API (Stars)
- **Risk**: LOW - Only fetches public repo star count
- **Impact**: Calls api.github.com every 10 minutes
- **Action**: Remove (cosmetic feature)
- **Effort**: Low (1 file)
- **Files**: `frontend/src/hooks/useGitHubStars.ts`

### 🟡 Loops Email Service
- **Risk**: MEDIUM - Sends email addresses and names
- **Impact**: Only used in remote server (crates/remote)
- **Action**: Remove if removing remote features
- **Effort**: Low (1 file)
- **Files**: `crates/remote/src/mail.rs`

## Priority 3: OPTIONAL - Branding & Links

These don't leak data but reference external services:

### 🟢 vibekanban.com Links
- **Risk**: NONE - Just hyperlinks to documentation
- **Impact**: User clicks = external navigation
- **Action**: Update to local docs or remove
- **Effort**: Low (search & replace)
- **Files**: Multiple (see audit doc)

### 🟢 Release Notes URL
- **Risk**: NONE - Fetches markdown from website
- **Impact**: Only when user clicks "What's New"
- **Action**: Point to local file or remove
- **Effort**: Low (1 file)
- **Files**: `frontend/src/components/dialogs/global/ReleaseNotesDialog.tsx`

### 🟢 Git Default Email
- **Risk**: NONE - Just default commit author
- **Impact**: Shows up in git commits
- **Action**: Change to generic email
- **Effort**: Trivial (1 line)
- **Files**: `crates/git/src/lib.rs`

### 🟢 PR Description Branding
- **Risk**: NONE - Just marketing text
- **Impact**: Shows up in PR descriptions
- **Action**: Remove or change branding
- **Effort**: Trivial (2 files)
- **Files**: `shared/types.ts`, `crates/services/src/services/config/mod.rs`

## Priority 4: OPTIONAL - Remote Features

Only needed if using multi-user cloud features:

### 🔵 ElectricSQL
- **Risk**: NONE (unless credentials exposed)
- **Impact**: Real-time sync for remote features
- **Action**: Remove if not using remote
- **Effort**: High (multiple files)
- **Files**: `crates/remote/src/routes/electric_proxy.rs`

### 🔵 OAuth (GitHub/Google)
- **Risk**: NONE (standard OAuth flow)
- **Impact**: User authentication for remote
- **Action**: Remove if not using remote
- **Effort**: Medium (remove from config)
- **Files**: `crates/remote/src/auth/`, `crates/remote/src/config.rs`

### 🔵 GitHub App Integration
- **Risk**: NONE (requires explicit setup)
- **Impact**: PR review automation
- **Action**: Remove if not using
- **Effort**: Medium (multiple files)
- **Files**: `crates/remote/src/github_app/`

### 🔵 Cloudflare R2 Storage
- **Risk**: NONE (requires explicit setup)
- **Impact**: File storage for reviews
- **Action**: Remove if not using remote
- **Effort**: Low (config only)
- **Files**: `crates/remote/src/config.rs`

### 🔵 Stripe Billing
- **Risk**: NONE (feature-flagged, optional)
- **Impact**: Payment processing
- **Action**: Don't enable feature flag
- **Effort**: None (already optional)
- **Feature**: `vk-billing` in `crates/remote`

## Recommended Removal Strategies

### Strategy A: Paranoid (Maximum Privacy)
**Goal**: Zero external calls, completely air-gapped

**Remove:**
- ✅ All Priority 1 (Sentry, PostHog, Remote API)
- ✅ All Priority 2 (Discord, GitHub, Loops)
- ✅ All Priority 3 (Update all links)
- ✅ All Priority 4 (Remove remote crate entirely)

**Effort**: High (1-2 days)
**Result**: 100% local, no network calls

### Strategy B: Pragmatic (Disable Tracking)
**Goal**: Stop data collection, keep useful links

**Remove:**
- ✅ Priority 1: Sentry, PostHog, Remote API
- ✅ Priority 2: Discord, GitHub widgets
- ⚠️  Priority 3: Keep links (just documentation)
- ⚠️  Priority 4: Disable but keep code

**Effort**: Medium (4-6 hours)
**Result**: No tracking, doc links still work

### Strategy C: Minimal (Quick Disable)
**Goal**: Stop immediate data leaks with minimal code changes

**Remove:**
- ✅ Priority 1: Sentry & PostHog only
- ⚠️  Everything else: Keep as-is

**Effort**: Low (1-2 hours)
**Result**: No error/analytics tracking

**Quick changes:**
```bash
# Comment out in frontend/src/main.tsx:
# - Lines 30-43 (Sentry.init)
# - Lines 46-56 (posthog.init)
# - Remove <PostHogProvider> wrapper

# Comment out in crates/server/src/main.rs:
# - utils::sentry::init_once(...)

# Set environment variable:
export VK_SHARED_API_BASE=""
```

## Impact Summary by Service

| Service | Data Sent | Frequency | User Control | Priority |
|---------|-----------|-----------|--------------|----------|
| Sentry | Errors, stack traces, user IDs | On error | None | 🔴 Critical |
| PostHog | Events, behavior, device info | Continuous | Opt-in setting | 🔴 Critical |
| Remote API | Projects, issues, workspaces | On sync | Explicit enable | 🔴 Critical |
| Discord API | None (public data fetch) | Every 10min | None | 🟡 Important |
| GitHub API | None (public data fetch) | Every 10min | None | 🟡 Important |
| Loops Email | Email addresses, names | On invite | Implicit (remote) | 🟡 Important |
| Doc Links | None (just navigation) | On click | User initiated | 🟢 Optional |
| ElectricSQL | DB sync data | Continuous | Remote feature | 🔵 Optional |
| OAuth | Auth tokens | On login | User initiated | 🔵 Optional |
| GitHub App | PR data | On webhook | Explicit setup | 🔵 Optional |
| R2 Storage | Files | On upload | Explicit setup | 🔵 Optional |
| Stripe | Payment info | On checkout | Explicit action | 🔵 Optional |

## Testing Your Removal

After removing services, test with:

```bash
# 1. Disconnect from internet
sudo ifconfig en0 down  # macOS WiFi
# or
sudo nmcli networking off  # Linux

# 2. Start app
pnpm run dev

# 3. Test features
# - Should start successfully
# - Should create projects
# - Should create tasks
# - Should run executors
# - Should commit to git

# 4. Check console for errors
# - No "failed to connect" errors
# - No "network error" logs

# 5. Reconnect internet
sudo ifconfig en0 up

# 6. Monitor network (optional)
# Use Little Snitch, Wireshark, or tcpdump
# Should see NO requests to:
# - sentry.io
# - discord.com
# - api.github.com
# - vibekanban.com
# - loops.so
# - stripe.com
```

## Feature Compatibility Matrix

After removal, what still works?

| Feature | No Changes | Minimal | Pragmatic | Paranoid |
|---------|------------|---------|-----------|----------|
| Local projects | ✅ | ✅ | ✅ | ✅ |
| Task management | ✅ | ✅ | ✅ | ✅ |
| Git integration | ✅ | ✅ | ✅ | ✅ |
| Code executors | ✅ | ✅ | ✅ | ✅ |
| Error reporting | ✅ | ❌ | ❌ | ❌ |
| Analytics | ✅ | ❌ | ❌ | ❌ |
| Remote sync | ✅ | ⚠️ | ❌ | ❌ |
| Online widgets | ✅ | ✅ | ❌ | ❌ |
| Doc links | ✅ | ✅ | ✅ | ⚠️ |
| Multi-user | ✅ | ⚠️ | ❌ | ❌ |
| PR reviews | ✅ | ⚠️ | ❌ | ❌ |

## Conclusion

**For a tight local-only tool**, follow **Strategy B (Pragmatic)** or **Strategy A (Paranoid)**.

**Start with Priority 1**, then move to Priority 2 if needed. Priority 3 and 4 are cosmetic/optional.

The most important removals are **Sentry and PostHog** as they actively send data without explicit user action.
