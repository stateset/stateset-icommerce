# Publishing StateSet CLI

This guide explains how to publish the StateSet CLI to npm and make it available via `curl -fsSL https://stateset.com/install.sh | bash`.

## Prerequisites

1. **npm account** with access to `@stateset` organization
2. **GitHub repository** at `stateset/stateset-icommerce`
3. **NPM_TOKEN** secret in GitHub repository settings
4. **Website deployment** method for stateset.com

## Package Structure

```
@stateset/embedded   - Native Rust bindings (must publish first)
@stateset/cli        - CLI with AI agents (depends on @stateset/embedded)
```

## Manual Publishing (First Time)

### Step 1: Publish @stateset/embedded

```bash
cd bindings/node

# Build for current platform
npm install
npm run build

# Login to npm
npm login

# Publish (first time - creates the package)
npm publish --access public
```

### Step 2: Publish @stateset/cli

```bash
cd cli

# Update dependency to use npm package (already done)
# "@stateset/embedded": "^0.1.0"

# Install and test
npm install
node bin/stateset.js --help

# Publish
npm publish --access public
```

### Step 3: Deploy install.sh

Upload `cli/install.sh` to your website at `https://stateset.com/install.sh`.

**Option A: Static hosting (S3, Cloudflare, Vercel)**
```bash
# Upload directly
aws s3 cp cli/install.sh s3://stateset-website/install.sh \
  --content-type "text/plain" \
  --cache-control "max-age=300"
```

**Option B: Add to website repo**
```bash
# Copy to your website repository
cp cli/install.sh ../stateset-website/public/install.sh
cd ../stateset-website
git add public/install.sh
git commit -m "Add CLI installer script"
git push
```

**Option C: Vercel/Next.js**
```bash
# Place in public folder
cp cli/install.sh ../stateset-website/public/install.sh
# Vercel will serve it automatically
```

## Automated Publishing (CI/CD)

### Setup GitHub Secrets

1. Go to repository Settings → Secrets → Actions
2. Add `NPM_TOKEN` - your npm automation token
3. Add `DEPLOY_TOKEN` - for website deployment (if needed)

### Create a Release

```bash
# Tag a new CLI release
git tag cli-v0.1.0
git push origin cli-v0.1.0
```

This triggers `.github/workflows/publish-cli.yml` which:
1. Builds native bindings for all platforms
2. Publishes `@stateset/embedded` to npm
3. Publishes `@stateset/cli` to npm
4. Deploys `install.sh` to stateset.com

## Version Bumping

```bash
# Bump embedded version
cd bindings/node
npm version patch  # or minor, major

# Bump CLI version (update dependency if needed)
cd ../cli
# Update @stateset/embedded version in package.json
npm version patch

# Commit and tag
git add -A
git commit -m "Release cli-v0.1.1"
git tag cli-v0.1.1
git push && git push --tags
```

## Testing the Install Script

```bash
# Test locally
bash cli/install.sh

# Test curl install (after deploying)
curl -fsSL https://stateset.com/install.sh | bash

# Verify
stateset --version
stateset --help
```

## Troubleshooting

### Native bindings fail to load
The `@stateset/embedded` package needs platform-specific binaries. Ensure:
- CI builds for all target platforms
- `npm run artifacts` moves binaries correctly
- `napi prepublish` creates platform packages

### Permission denied on global install
```bash
# Fix npm permissions (nvm users)
npm config set prefix ~/.npm-global
export PATH=~/.npm-global/bin:$PATH

# Or use sudo (system Node.js)
sudo npm install -g @stateset/cli
```

### ANTHROPIC_API_KEY not set
```bash
export ANTHROPIC_API_KEY=sk-ant-api03-...
```

## URLs After Publishing

- **npm**: https://www.npmjs.com/package/@stateset/cli
- **Install script**: https://stateset.com/install.sh
- **Docs**: https://docs.stateset.com/cli

## Quick Install Commands

```bash
# One-liner install
curl -fsSL https://stateset.com/install.sh | bash

# Or via npm directly
npm install -g @stateset/cli

# Or via npx (no install)
npx @stateset/cli "show me all customers"
```
