# Changelog & GitHub Pages Setup

This guide covers automatic changelog generation from git commits and publishing documentation to GitHub Pages.

## Overview

The system provides:
- **Automatic changelog generation** from git commit history
- **Conventional commit parsing** for structured changelog sections
- **GitHub Actions workflow** for automation on releases
- **GitHub Pages publishing** via a separate public docs repository

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Your Private Repository                      │
│                                                                  │
│  src/                    scripts/                                │
│  ├── main.rs             └── generate_changelog.sh               │
│  ├── lib.rs                  (or generate_changelog.rs)          │
│  └── ...                                                         │
│                                                                  │
│  .github/workflows/                                              │
│  └── sync-docs.yml       <- Triggers on release/tag               │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               │ On Release:
                               │ 1. Generate CHANGELOG.md
                               │ 2. Copy documentation
                               │ 3. Push to public repo
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Public Docs Repository                       │
│                     (username/app-name-docs)                     │
│                                                                  │
│  CHANGELOG.md                                                    │
│  index.md              <- Landing page for GitHub Pages           │
│  user-guide.md                                                   │
│  installation.md                                                 │
│                                                                  │
│  GitHub Pages URL: https://username.github.io/app-name-docs/     │
└─────────────────────────────────────────────────────────────────┘
```

## Commit Message Convention

Use conventional commits to enable automatic changelog categorization:

| Prefix | Changelog Section | Example |
|--------|------------------|---------|
| `feat:` | **Added** | `feat: Add dark mode toggle` |
| `fix:` | **Fixed** | `fix: Correct file path on Windows` |
| `perf:` | **Improved** | `perf: Speed up image loading by 50%` |
| `refactor:` | **Changed** | `refactor: Simplify config loading` |
| `security:` | **Security** | `security: Fix XSS vulnerability` |

### Commits Excluded from Changelog

These prefixes are automatically filtered out (internal/maintenance):

- `docs:` - Documentation only
- `test:` - Test changes
- `style:` - Code formatting
- `ci:` - CI/CD changes
- `build:` - Build system changes
- `chore:` - Routine maintenance
- Merge commits
- Commits containing "WIP"

### Action Words

If you don't use prefixes, commits are categorized by their first word:

| First Word | Category |
|------------|----------|
| Add, Added | Added |
| Fix, Fixed | Fixed |
| Update, Updated | Changed |
| Improve, Improved | Improved |
| Remove, Removed | Removed |

## Setup Instructions

### Step 1: Create GitHub Personal Access Token

1. Go to **GitHub Settings** -> **Developer settings** -> **Personal access tokens** -> **Tokens (classic)**

   URL: https://github.com/settings/tokens

2. Click **Generate new token (classic)**

3. Configure the token:
   - **Note:** `docs-deploy-token` (or descriptive name)
   - **Expiration:** Choose based on your needs (90 days to no expiration)
   - **Scopes:** Check `repo` (Full control of private repositories)

4. Click **Generate token**

5. **Copy the token immediately** - you won't see it again!

### Step 2: Create Public Docs Repository

1. Go to https://github.com/new

2. Create a new repository:
   - **Name:** `your-app-docs` (e.g., `my-app-docs`)
   - **Visibility:** Public (required for GitHub Pages)
   - **Initialize:** Check "Add a README file"

3. Enable GitHub Pages:
   - Go to repository **Settings** -> **Pages**
   - **Source:** Deploy from a branch
   - **Branch:** `main` / `(root)`
   - Click **Save**

4. Note your Pages URL: `https://username.github.io/your-app-docs/`

### Step 3: Add Token to Source Repository

1. Go to your **source repository** (the private one with your app)

2. Navigate to **Settings** -> **Secrets and variables** -> **Actions**

3. Click **New repository secret**

4. Add the secret:
   - **Name:** `DOCS_DEPLOY_TOKEN`
   - **Secret:** Paste the token from Step 1

5. Click **Add secret**

### Step 4: Add Changelog Script

You can write the changelog generator as either a shell script or a Rust binary. A shell script is simplest since the logic is purely git-based.

Copy `templates/generate_changelog.sh` to `scripts/generate_changelog.sh` in your project.

Update the configuration at the top of the file:

```bash
#!/usr/bin/env bash
# Update these to match your project
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHANGELOG_OUTPUT="$REPO_ROOT/CHANGELOG.md"

# Read version from Cargo.toml
get_current_version() {
    grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/'
}
```

Alternatively, if you prefer a Rust-based generator, add it as a binary target in `Cargo.toml`:

```toml
[[bin]]
name = "generate-changelog"
path = "scripts/generate_changelog.rs"
```

### Step 5: Add GitHub Actions Workflow

Create `.github/workflows/sync-docs.yml`:

```yaml
# Copy content from templates/sync-docs.yml
# Update these values:
# - destination-github-username: 'your-username'
# - destination-repository-name: 'your-app-docs'
# - user-email: your-email@example.com
# - user-name: your-username
```

### Step 6: Add Documentation Files

Create documentation files that will be synced:

```
docs/
├── public/                # Files that go to GitHub Pages
│   └── README.md          # Can serve as landing page
├── user-guide.md          # User documentation
├── installation.md        # Installation guide
└── configuration.md       # Configuration reference
```

## Usage

### Manual Changelog Generation

```bash
# Preview without writing file
./scripts/generate_changelog.sh --dry-run

# Generate CHANGELOG.md
./scripts/generate_changelog.sh

# Or if using the Rust binary
cargo run --bin generate-changelog -- --dry-run
cargo run --bin generate-changelog
```

### Automatic Generation on Release

1. Create a git tag:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

2. **OR** create a GitHub Release:
   - Go to repository -> Releases -> Draft a new release
   - Choose or create a tag (e.g., `v1.0.0`)
   - Add release notes
   - Click **Publish release**

3. The workflow automatically:
   - Generates fresh CHANGELOG.md from git history
   - Copies documentation to the public repo
   - Updates GitHub Pages

### Manual Workflow Trigger

You can also trigger the workflow manually:

1. Go to **Actions** -> **Sync Documentation**
2. Click **Run workflow**
3. Optionally enter a reason
4. Click **Run workflow**

## Workflow in Practice

### Daily Development

```bash
# Make changes and commit with conventional format
git add .
git commit -m "feat: Add export to CSV functionality"

# Or for bug fixes
git commit -m "fix: Handle unicode filenames correctly"

# Internal changes (won't appear in changelog)
git commit -m "test: Add unit tests for export"
git commit -m "chore: Update dependencies"
```

### Creating a Release

```bash
# 1. Update version in Cargo.toml
# 2. Commit the version bump
git add Cargo.toml Cargo.lock
git commit -m "chore(release): Bump version to 1.2.0"

# 3. Create and push tag
git tag v1.2.0
git push origin main
git push origin v1.2.0

# Workflow runs automatically and:
# - Generates CHANGELOG.md
# - Syncs docs to public repo
# - Updates GitHub Pages
```

### Version Numbering

Use semantic versioning (SemVer):

| Version | When to Use |
|---------|-------------|
| `1.0.0` -> `1.0.1` | Bug fixes only |
| `1.0.0` -> `1.1.0` | New features (backward compatible) |
| `1.0.0` -> `2.0.0` | Breaking changes |
| `1.0.0-alpha.1` | Alpha pre-release |
| `1.0.0-rc.1` | Release candidate |

## Generated Changelog Format

The script generates a changelog following [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
# Changelog

All notable changes to YourApp will be documented in this file.

## [1.2.0] - 2024-01-15

### Added
- Export to CSV functionality
- Dark mode toggle in settings

### Fixed
- Unicode filename handling on Windows
- Memory leak in image processing

### Changed
- Improved startup performance

## [1.1.0] - 2024-01-01

### Added
- ...

---

## Version History Summary

| Version | Date | Highlights |
|---------|------|------------|
| 1.2.0 | 2024-01-15 | Export to CSV, Dark mode toggle |
| 1.1.0 | 2024-01-01 | ... |
```

## Troubleshooting

### Workflow Fails with "Permission Denied"

- Ensure `DOCS_DEPLOY_TOKEN` secret is set correctly
- Token needs `repo` scope
- Token must not be expired

### Changelog is Empty

- Ensure you have tags in your repository (`git tag -l`)
- Check that commits follow conventional format
- Run `./scripts/generate_changelog.sh --dry-run` locally to debug

### GitHub Pages Not Updating

- Check repository Settings -> Pages is enabled
- Verify the branch is `main` and folder is `/(root)`
- Wait a few minutes - Pages can take time to update
- Check the Actions tab for workflow errors

### Token Expired

1. Generate a new token (Step 1)
2. Update the repository secret (Step 3)
3. Re-run the workflow

## Security Notes

- **Never commit tokens** to your repository
- Use repository secrets for sensitive values
- The public docs repo should only contain documentation
- Source code stays in your private repository

## File Reference

| File | Purpose |
|------|---------|
| `scripts/generate_changelog.sh` | Generates CHANGELOG.md from git |
| `.github/workflows/sync-docs.yml` | Automates doc sync on release |
| `CHANGELOG.md` | Generated changelog |
| `docs/public/` | Files synced to public repo |

## See Also

- [BUILD-DISTRIBUTION.md](BUILD-DISTRIBUTION.md) - Build and release process
- [VERSIONING.md](VERSIONING.md) - Version management
- [templates/generate_changelog.sh](templates/generate_changelog.sh) - Script template
- [templates/sync-docs.yml](templates/sync-docs.yml) - Workflow template
