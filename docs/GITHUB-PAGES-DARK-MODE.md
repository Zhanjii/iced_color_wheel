# GitHub Pages Dark Mode Setup

This guide covers setting up GitHub Pages documentation with the Just the Docs theme and dark mode.

## Overview

GitHub Pages can serve documentation directly from your repository. Using the **Just the Docs** theme provides:

- Dark color scheme
- Search functionality
- Navigation sidebar
- Heading anchor links
- Back to top links
- Mobile responsive design

## Quick Setup

### 1. Create Documentation Repository

For separate documentation, create a `{project-name}-docs` repository:

```bash
# Create docs repo
gh repo create my-project-docs --public --description "Documentation for My Project"
cd my-project-docs
```

Or use a `/docs` folder in your main repository.

### 2. Add Jekyll Configuration

Create `_config.yml` in your docs root:

```yaml
# Just the Docs theme configuration
remote_theme: just-the-docs/just-the-docs

# Site settings
title: My Project
description: Project description here
url: https://username.github.io
baseurl: /my-project-docs

# Color scheme - "light" or "dark"
# Note: "auto" is NOT supported by GitHub Pages Jekyll version
color_scheme: dark

# Navigation
nav_enabled: true
nav_sort: case_insensitive

# Search
search_enabled: true
search.button: true

# Features
back_to_top: true
back_to_top_text: "Back to top"
heading_anchors: true

# GitHub link in header
aux_links:
  "GitHub":
    - "https://github.com/username/my-project"
aux_links_new_tab: true

# Footer
footer_content: "My Project - Description"
```

### 3. Add Front Matter to Markdown Files

Each markdown file needs front matter for navigation:

**index.md (Home page):**
```yaml
---
layout: home
title: Home
nav_order: 1
description: "Project description"
permalink: /
---

# My Project

Welcome to the documentation.
```

**Other pages:**
```yaml
---
layout: default
title: User Guide
nav_order: 2
description: "How to use this project"
---

# User Guide

Content here...
```

### 4. Enable GitHub Pages

**Via CLI:**
```bash
# For docs in separate repo (root)
echo '{"source":{"branch":"main","path":"/"}}' | gh api repos/username/my-project-docs/pages -X POST --input -

# For docs in /docs folder of main repo
echo '{"source":{"branch":"main","path":"/docs"}}' | gh api repos/username/my-project/pages -X POST --input -
```

**Via GitHub UI:**
1. Go to repository Settings → Pages
2. Under "Source", select branch and folder
3. Click Save

### 5. Verify Build

```bash
# Check build status
gh run list --limit 1

# Check Pages status
gh api repos/username/my-project-docs/pages --jq '.html_url, .status'
```

## File Structure

Recommended documentation structure:

```
docs/
├── _config.yml           # Jekyll configuration
├── index.md              # Home page (nav_order: 1)
├── user-guide.md         # User guide (nav_order: 2)
├── installation.md       # Installation (nav_order: 3)
├── configuration.md      # Configuration (nav_order: 4)
├── developer-guide.md    # Developer guide (nav_order: 5)
└── CHANGELOG.md          # Changelog (nav_order: 6)
```

## Front Matter Reference

| Field | Description | Example |
|-------|-------------|---------|
| `layout` | Page layout | `home`, `default` |
| `title` | Navigation title | `User Guide` |
| `nav_order` | Position in sidebar | `1`, `2`, `3`... |
| `description` | SEO description | `How to use this` |
| `permalink` | Custom URL path | `/`, `/guide/` |
| `parent` | Parent page for nesting | `User Guide` |
| `has_children` | Has child pages | `true` |

## Nested Navigation

For hierarchical documentation:

**Parent page:**
```yaml
---
layout: default
title: API Reference
nav_order: 4
has_children: true
---
```

**Child page:**
```yaml
---
layout: default
title: Authentication
parent: API Reference
nav_order: 1
---
```

## Color Schemes

Just the Docs supports:
- `light` - Light background
- `dark` - Dark background (recommended)

**Important:** The `auto` setting (system preference) is NOT supported by the GitHub Pages Jekyll version. Use `dark` or `light` explicitly.

## Excluding Files

Exclude files from Jekyll processing in `_config.yml`:

```yaml
exclude:
  - templates/
  - "*.py"
  - "*.json"
  - node_modules/
  - vendor/
```

## Troubleshooting

### Build Failures

Check build logs:
```bash
gh run view $(gh run list --limit 1 --json databaseId -q '.[0].databaseId') --log-failed
```

Common issues:
- **"File to import not found: ./color_schemes/auto"** - Change `color_scheme: auto` to `color_scheme: dark`
- **Liquid syntax errors** - Check for `{{ }}` in code blocks (escape with `{% raw %}{% endraw %}`)
- **Front matter errors** - Ensure YAML is valid (proper indentation, quotes around strings with colons)

### Cache Issues

After deployment, if changes don't appear:
- Hard refresh: `Ctrl + Shift + R`
- Clear browser cache
- Open in incognito/private window
- Wait 2-3 minutes for CDN propagation

### Pages Not Enabled

Enable Pages via API:
```bash
echo '{"source":{"branch":"main","path":"/"}}' | gh api repos/username/repo/pages -X POST --input -
```

## Complete Example

**_config.yml:**
```yaml
remote_theme: just-the-docs/just-the-docs

title: Ralph WigGUIm
description: Automated Development Loop GUI for Claude Code
url: https://zhanjii.github.io
baseurl: /ralph-wigguim-docs

color_scheme: dark

nav_enabled: true
nav_sort: case_insensitive
search_enabled: true
search.button: true
back_to_top: true
heading_anchors: true

aux_links:
  "GitHub":
    - "https://github.com/Zhanjii/ralph-wigguim"
aux_links_new_tab: true

footer_content: "Ralph WigGUIm - Automated Development Loop GUI"
```

**index.md:**
```yaml
---
layout: home
title: Home
nav_order: 1
description: "Automated Development Loop GUI for Claude Code"
permalink: /
---

# Ralph WigGUIm Documentation

Automated Development Loop GUI for Claude Code.

## Quick Start

1. Download the latest release
2. Run the application
3. Add a project
4. Start automation!
```

## Batch Setup Script

For applying dark mode to multiple docs repos:

```bash
#!/bin/bash
# Apply Just the Docs dark theme to multiple repos

REPOS=("project1-docs" "project2-docs" "project3-docs")

for repo in "${REPOS[@]}"; do
    echo "Processing $repo..."

    # Clone
    git clone "https://github.com/username/$repo.git"
    cd "$repo"

    # Create config
    cat > _config.yml << 'EOF'
remote_theme: just-the-docs/just-the-docs
title: PROJECT_NAME
description: PROJECT_DESCRIPTION
color_scheme: dark
nav_enabled: true
search_enabled: true
back_to_top: true
heading_anchors: true
EOF

    # Commit and push
    git add _config.yml
    git commit -m "feat: Add Just the Docs dark theme"
    git push

    cd ..
done
```

## Resources

- [Just the Docs Documentation](https://just-the-docs.com/)
- [GitHub Pages Documentation](https://docs.github.com/en/pages)
- [Jekyll Documentation](https://jekyllrb.com/docs/)
