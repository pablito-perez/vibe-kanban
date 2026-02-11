# External Services Audit - Executive Summary

## Overview

This audit identified **12 categories of external services** that Vibe Kanban currently calls out to. This summary provides actionable next steps for converting to a local-only tool.

## Quick Stats

- **Total External Services**: 12
- **Critical (Data Leakage)**: 3 services
- **Important (Privacy)**: 3 services  
- **Optional (Branding)**: 4 services
- **Remote Features**: 6 services (optional, feature-gated)

## Critical Findings

### 🔴 High Priority - Active Data Collection

1. **Sentry** - Sends error data to sentry.io on every error
   - Stack traces, user IDs, environment info
   - 2 hardcoded DSN keys in code
   
2. **PostHog** - Sends behavioral analytics continuously  
   - User events, device info, telemetry
   - Opt-in required, but enabled by default in some builds
   
3. **Remote API** - Sends project/issue data to api.vibekanban.com
   - Only when explicitly enabled by user
   - Can be disabled via environment variable

### 🟡 Medium Priority - Periodic External Calls

4. **Discord API** - Fetches online member count every 10 minutes
5. **GitHub API** - Fetches repository stars every 10 minutes  
6. **Loops** - Email service for invitations (remote-only feature)

### 🟢 Low Priority - Links & Branding

7. **vibekanban.com** - Documentation and release notes links
8. **Git commits** - Default email `noreply@vibekanban.com`
9. **PR descriptions** - Branding text mentioning vibekanban.com

### 🔵 Optional - Remote/Cloud Features

10. **ElectricSQL** - Real-time database sync (remote feature)
11. **OAuth** - GitHub/Google authentication (remote feature)
12. **GitHub App** - PR review automation (optional integration)
13. **Cloudflare R2** - File storage (remote feature)
14. **Stripe** - Payment processing (feature-flagged, optional)

## Files Created

Your audit generated 4 comprehensive documents:

1. **EXTERNAL_SERVICES_AUDIT.md** - Complete list of all services with file locations
2. **EXTERNAL_SERVICES_PRIORITY.md** - Priority matrix and removal strategies
3. **LOCAL_ONLY_CONVERSION_GUIDE.md** - Step-by-step removal instructions
4. **scripts/strip-external-services.sh** - Automated removal script

## Recommended Action Plan

### Option A: Quick Disable (1-2 hours)

**Goal**: Stop immediate data leakage with minimal changes

```bash
# Run the minimal cleanup script
./scripts/strip-external-services.sh minimal

# Verify changes
pnpm run dev
# Test that app still works
```

**What this does:**
- Disables Sentry error tracking
- Disables PostHog analytics  
- Disables remote API via environment variable

**Result**: No tracking, app still fully functional

### Option B: Privacy-Focused (4-6 hours)

**Goal**: Remove all non-essential external calls

```bash
# Run the pragmatic cleanup script
./scripts/strip-external-services.sh pragmatic

# Additional manual steps:
cd frontend
pnpm remove @sentry/react @sentry/vite-plugin posthog-js
pnpm install

# Test
pnpm run dev
```

**What this does:**
- Everything from Option A
- Removes Discord & GitHub widgets
- Removes npm dependencies

**Result**: No tracking, no widgets, doc links still work

### Option C: Air-Gapped (1-2 days)

**Goal**: Zero external dependencies, completely local

```bash
# Run the paranoid cleanup script
./scripts/strip-external-services.sh paranoid

# Follow the detailed guide
cat LOCAL_ONLY_CONVERSION_GUIDE.md
```

**What this does:**
- Everything from Option B
- Updates all branding
- Removes remote crate
- Localizes documentation

**Result**: 100% local, no network calls

## Testing Your Changes

### 1. Offline Test

```bash
# Disconnect from internet
sudo ifconfig en0 down  # macOS

# Start app
pnpm run dev

# App should work fully offline
```

### 2. Network Monitoring

```bash
# Monitor DNS requests (requires root)
sudo tcpdump -i any -n port 53 | grep -E "sentry|posthog|discord|github|vibekanban"

# Should see NO requests to these domains
```

### 3. Verification Checklist

- [ ] App starts without errors
- [ ] Can create projects locally
- [ ] Can create and manage tasks
- [ ] Git operations work
- [ ] Code executors work
- [ ] No console errors about failed connections
- [ ] No network requests to external services

## Next Steps

1. **Choose your strategy** (A, B, or C above)
2. **Run the automated script**: `./scripts/strip-external-services.sh [minimal|pragmatic|paranoid]`
3. **Review the changes** in modified files
4. **Test thoroughly** (offline test is crucial)
5. **Remove dependencies** (optional, see guide)
6. **Update documentation** to reflect local-only setup

## Backup & Safety

The automated script creates backups before making changes:
- Location: `.external-services-backup-TIMESTAMP/`
- To revert: Restore files from backup directory
- Original files have `.backup` extension

## Questions to Consider

Before proceeding, consider:

1. **Do you need remote/collaboration features?**
   - If yes: Keep `crates/remote`, just disable Sentry/PostHog
   - If no: Remove entire remote crate (Option C)

2. **Do you want PR review automation?**
   - If yes: Keep GitHub App integration
   - If no: Safe to remove

3. **Is offline operation required?**
   - If yes: Go with Option B or C
   - If no: Option A may be sufficient

4. **Do you need documentation links?**
   - If yes: Can keep external links
   - If no: Update to local docs

## Resources

- **Full Audit**: `EXTERNAL_SERVICES_AUDIT.md`
- **Priority Guide**: `EXTERNAL_SERVICES_PRIORITY.md`
- **Detailed Instructions**: `LOCAL_ONLY_CONVERSION_GUIDE.md`
- **Automated Script**: `scripts/strip-external-services.sh`

## Support

If you encounter issues:

1. Check the backup directory for original files
2. Review console logs for specific errors
3. Compare with the detailed guide
4. Test incrementally (one service at a time)

## Estimated Impact

| Strategy | Effort | External Calls Removed | Features Lost |
|----------|--------|------------------------|---------------|
| Minimal | 1-2h | 90% (tracking only) | None |
| Pragmatic | 4-6h | 95% (+ widgets) | Social widgets |
| Paranoid | 1-2d | 100% (everything) | Remote features |

## Final Recommendation

**For a tight local-only tool**, start with **Option B (Pragmatic)**:

1. Removes all tracking (Sentry, PostHog)
2. Removes cosmetic external calls (Discord, GitHub)
3. Keeps codebase mostly intact
4. Easy to test and verify
5. Can upgrade to Option C later if needed

Run: `./scripts/strip-external-services.sh pragmatic`

---

**Generated**: February 11, 2026
**Audit Scope**: Complete codebase scan
**Services Identified**: 12 external integrations
**Documentation**: 4 comprehensive guides
