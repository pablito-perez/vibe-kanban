# External Services Audit & Removal Documentation

## 📋 Overview

This directory contains a complete audit of all external services that Vibe Kanban communicates with, along with detailed guides for removing them to create a fully local-only tool.

## 🗂️ Documentation Files

### 1. **AUDIT_SUMMARY.md** ⭐ START HERE
**Quick overview and action plan**
- Executive summary of findings
- Quick stats and critical services
- Three recommended strategies
- 5-minute read

### 2. **QUICK_REFERENCE.md** 📌 CHEAT SHEET
**One-page reference card**
- Table of all services
- Quick commands
- Key files to edit
- Testing checklist
- 2-minute lookup

### 3. **EXTERNAL_SERVICES_AUDIT.md** 📊 COMPLETE LIST
**Detailed catalog of every service**
- 12 service categories documented
- File locations for each service
- Environment variables
- Dependencies to remove
- 15-minute deep dive

### 4. **EXTERNAL_SERVICES_PRIORITY.md** 🎯 STRATEGY GUIDE
**Priority matrix and removal strategies**
- Services ranked by risk level
- Impact analysis
- Three removal strategies (Minimal, Pragmatic, Paranoid)
- Feature compatibility matrix
- 20-minute strategic read

### 5. **LOCAL_ONLY_CONVERSION_GUIDE.md** 📖 STEP-BY-STEP
**Comprehensive removal instructions**
- 9 phases of removal
- Exact code changes needed
- Before/after examples
- Testing procedures
- 60+ minute implementation guide

### 6. **../../scripts/strip-external-services.sh** 🤖 AUTOMATION
**Automated removal script** (located in project root scripts/)
- Three modes: minimal, pragmatic, paranoid
- Creates backups automatically
- Validates changes
- ~5 minutes to run

## 🚀 Quick Start

### For the Impatient (5 minutes)

```bash
# 1. Review what will be removed
cat docs/hardening/QUICK_REFERENCE.md

# 2. Run automated script (recommended: pragmatic mode)
# (Run from project root)
./scripts/strip-external-services.sh pragmatic

# 3. Test
pnpm run dev

# 4. Done!
```

### For the Thorough (30 minutes)

```bash
# 1. Read the summary
cat docs/hardening/AUDIT_SUMMARY.md

# 2. Choose your strategy
cat docs/hardening/EXTERNAL_SERVICES_PRIORITY.md

# 3. Run the script (from project root)
./scripts/strip-external-services.sh [minimal|pragmatic|paranoid]

# 4. Test thoroughly
pnpm run dev
# Disconnect internet
# Test all features
# Monitor network

# 5. Clean up dependencies (optional)
cd frontend && pnpm remove @sentry/react @sentry/vite-plugin posthog-js
# Edit Cargo.toml files to remove sentry dependencies
```

### For the Paranoid (2+ hours)

```bash
# 1. Read everything
cat docs/hardening/AUDIT_SUMMARY.md
cat docs/hardening/EXTERNAL_SERVICES_AUDIT.md
cat docs/hardening/EXTERNAL_SERVICES_PRIORITY.md
cat docs/hardening/LOCAL_ONLY_CONVERSION_GUIDE.md

# 2. Run paranoid mode (from project root)
./scripts/strip-external-services.sh paranoid

# 3. Follow manual steps in docs/hardening/LOCAL_ONLY_CONVERSION_GUIDE.md

# 4. Remove remote crate entirely
rm -rf crates/remote crates/review remote-frontend

# 5. Test exhaustively
# See docs/hardening/LOCAL_ONLY_CONVERSION_GUIDE.md Phase 7
```

## 🎯 Recommended Path

**Most users should follow this path:**

1. ✅ Read **docs/hardening/AUDIT_SUMMARY.md** (5 min)
2. ✅ Run `./scripts/strip-external-services.sh pragmatic` from project root (5 min)
3. ✅ Test with `pnpm run dev` (5 min)
4. ✅ Verify with offline test (5 min)
5. ✅ Optional: Clean up npm/cargo dependencies (10 min)

**Total time: ~30 minutes**

## 📊 What Gets Removed

### Critical (Data Leakage) - REMOVE IMMEDIATELY
- 🔴 **Sentry** - Error tracking (sends stack traces)
- 🔴 **PostHog** - Analytics (sends user behavior)
- 🔴 **Remote API** - Cloud sync (sends project data)

### Important (Privacy) - RECOMMENDED REMOVAL
- 🟡 **Discord API** - Online count widget
- 🟡 **GitHub API** - Stars widget
- 🟡 **Loops** - Email service (remote-only)

### Optional (Branding) - LOW PRIORITY
- 🟢 **vibekanban.com** - Documentation links
- 🟢 **Git email** - Default commit author
- 🟢 **PR branding** - Marketing text

### Remote Features - ONLY IF NOT USING CLOUD
- 🔵 **ElectricSQL** - Real-time sync
- 🔵 **OAuth** - GitHub/Google login
- 🔵 **GitHub App** - PR automation
- 🔵 **R2 Storage** - File storage
- 🔵 **Stripe** - Payments

## 🧪 Testing Your Changes

### Quick Test
```bash
pnpm run dev
# Should start without errors
```

### Offline Test
```bash
sudo ifconfig en0 down  # macOS
pnpm run dev
# Should work fully offline
sudo ifconfig en0 up
```

### Network Monitor Test
```bash
sudo tcpdump -i any -n port 53 | \
  grep -E "sentry|posthog|discord|github|vibekanban"
# Should show NO matches
```

## 📈 Removal Strategies Compared

| Aspect | Minimal | Pragmatic ⭐ | Paranoid |
|--------|---------|-------------|----------|
| **Time** | 1-2h | 4-6h | 1-2d |
| **Tracking** | ❌ Removed | ❌ Removed | ❌ Removed |
| **Widgets** | ✅ Kept | ❌ Removed | ❌ Removed |
| **Branding** | ✅ Kept | ✅ Kept | ❌ Removed |
| **Remote** | ⚠️ Disabled | ⚠️ Disabled | ❌ Removed |
| **Effort** | Low | Medium | High |
| **Result** | 90% clean | 95% clean | 100% clean |

**⭐ Pragmatic is recommended for most users**

## 🔧 Automated vs Manual

### Automated Script (Recommended)
- ✅ Fast (5-10 minutes)
- ✅ Creates backups
- ✅ Validates changes
- ✅ Three modes available
- ⚠️ May need manual tweaking

### Manual Removal
- ⚠️ Slow (2+ hours)
- ✅ Full control
- ✅ Understand every change
- ⚠️ Easy to miss something

**Recommendation**: Run script first, then manual cleanup if needed.

## 🛡️ Safety & Backups

The automated script:
- ✅ Creates timestamped backup directory
- ✅ Backs up each modified file
- ✅ Validates builds before finishing
- ✅ Provides rollback instructions

To revert:
```bash
# Find backup
ls -la .external-services-backup-*

# Restore
cd .external-services-backup-TIMESTAMP
cp -r * /path/to/project/
```

## 📞 Need Help?

### Common Issues

**"Module not found" errors**
→ Remove from package.json: `pnpm remove <package>`

**Build fails after removal**
→ Check Cargo.toml dependencies

**Still seeing external requests**
→ Check .env files, ensure VK_SHARED_API_BASE is empty

**Type errors in frontend**
→ Remove PostHogProvider wrapper

### Getting Support

1. Check **docs/hardening/QUICK_REFERENCE.md** "Common Issues" section
2. Review **docs/hardening/LOCAL_ONLY_CONVERSION_GUIDE.md** for detailed steps
3. Check backups in `.external-services-backup-*` directories

## 📝 Files Created by Audit

```
.
├── docs/
│   └── hardening/
│       ├── AUDIT_SUMMARY.md                    # Executive summary ⭐
│       ├── QUICK_REFERENCE.md                  # One-page cheat sheet
│       ├── EXTERNAL_SERVICES_AUDIT.md          # Complete catalog
│       ├── EXTERNAL_SERVICES_PRIORITY.md       # Strategy guide
│       ├── LOCAL_ONLY_CONVERSION_GUIDE.md      # Step-by-step manual
│       └── EXTERNAL_SERVICES_README.md         # This file
└── scripts/
    └── strip-external-services.sh              # Automated script
```

## ✅ Success Criteria

You've successfully converted to local-only when:

1. ✅ App starts and runs offline
2. ✅ No external DNS requests (verified with tcpdump)
3. ✅ No console errors about failed connections
4. ✅ All core features work (projects, tasks, git, executors)
5. ✅ Type checks pass: `pnpm run check`
6. ✅ Builds succeed: `cargo build`

## 🎉 Final Recommendation

**For 95% of users wanting a local-only tool:**

```bash
# Just run this:
./scripts/strip-external-services.sh pragmatic

# Then test:
pnpm run dev

# That's it! 🎉
```

**Time**: 30 minutes total (including testing)
**Result**: No tracking, no widgets, fully local
**Effort**: Minimal (automated script does the work)

---

**Last Updated**: February 11, 2026
**Audit Coverage**: Complete codebase scan
**Services Documented**: 12 external integrations
**Removal Strategies**: 3 (minimal, pragmatic, paranoid)
**Automation**: Fully scripted with backups
