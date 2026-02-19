# One-Click VPS Deployment

Research notes for creating a self-hosted one-click deploy experience for Echora, similar to Vultr Marketplace apps or DigitalOcean 1-Click Apps.

## Current State

- Backend deployed as a Docker container behind a load balancer
- Frontend deployed as static files via CDN
- PostgreSQL via Docker Compose locally, managed DB in production
- Single-instance architecture -- no horizontal scaling needed
- In-memory state (DashMap for voice sessions, broadcast channels, presence) means only one backend instance can run

## Goal

Let anyone deploy their own Echora instance on a fresh VPS with minimal effort. Two approaches: a universal installer script and platform-specific marketplace images.

## Approach A: Universal Installer Script (Recommended starting point)

A single shell script that works on any fresh Ubuntu/Debian VPS. No marketplace approval process needed.

```bash
curl -sSL https://install.echora.example/install.sh | bash -s -- --domain chat.example.com --email admin@example.com
```

### What the script does

1. **System setup** -- install Docker, Docker Compose
2. **Generate secrets** -- JWT secret, Postgres password, admin credentials
3. **Write config** -- generate `docker-compose.yml` and `.env` from templates
4. **TLS** -- install and configure Caddy (automatic HTTPS via Let's Encrypt)
5. **Pull images** -- pull pre-built Echora backend and frontend images from a public container registry (GHCR or Docker Hub)
6. **Database** -- start Postgres, run migrations
7. **Start services** -- bring up the full stack
8. **Systemd** -- install a systemd service for auto-start on boot
9. **Print summary** -- show the URL, admin credentials, and next steps

### Generated docker-compose.yml

```yaml
services:
  caddy:
    image: caddy:2
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - caddy_data:/data
      - caddy_config:/config
      - ./frontend:/srv/frontend

  backend:
    image: ghcr.io/echora/backend:latest
    restart: unless-stopped
    environment:
      - DATABASE_URL=postgres://echora:${POSTGRES_PASSWORD}@postgres:5432/echora
      - JWT_SECRET=${JWT_SECRET}
      - CORS_ORIGINS=https://${DOMAIN}
      - RUST_LOG=info
    depends_on:
      postgres:
        condition: service_healthy

  postgres:
    image: postgres:17
    restart: unless-stopped
    environment:
      - POSTGRES_USER=echora
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
      - POSTGRES_DB=echora
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U echora"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  pgdata:
  caddy_data:
  caddy_config:
```

### Generated Caddyfile

```
{$DOMAIN} {
    # Frontend -- serve static SvelteKit build
    handle {
        root * /srv/frontend
        try_files {path} /index.html
        file_server
    }

    # Backend API and WebSocket
    handle /api/* {
        reverse_proxy backend:3000
    }

    handle /ws {
        reverse_proxy backend:3000
    }
}
```

### Script requirements

- Idempotent -- safe to re-run (updates images, preserves data)
- Supports `--update` flag to pull latest images and restart
- Supports `--uninstall` to tear down cleanly
- Validates DNS points to the server's IP before requesting TLS certs
- Minimum server requirements: 1 vCPU, 1GB RAM, 10GB disk

### CI/CD for images

Need a GitHub Actions workflow to build and push Docker images on every release:

```yaml
# .github/workflows/release.yml
on:
  push:
    tags: ['v*']

jobs:
  build-backend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ghcr.io/${{ github.repository }}/backend:latest,ghcr.io/${{ github.repository }}/backend:${{ github.ref_name }}

  build-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cd frontend && npm ci && npm run build
      - uses: docker/build-push-action@v6
        with:
          context: frontend
          file: frontend/Dockerfile.static
          push: true
          tags: ghcr.io/${{ github.repository }}/frontend:latest,ghcr.io/${{ github.repository }}/frontend:${{ github.ref_name }}
```

The frontend image could just be a simple nginx/caddy image serving the static build, or the installer could download and extract a tarball instead of using a container.

## Approach B: Marketplace Images

Pre-baked VM snapshots with everything installed. Faster deploy (~60 seconds) but requires maintaining images per platform.

### How marketplace apps work

1. Spin up a base VM (Ubuntu 22.04 LTS)
2. Install all dependencies (Docker, Caddy, etc.)
3. Pre-pull Echora Docker images
4. Install a first-boot script at `/var/lib/cloud/scripts/per-instance/setup.sh`
5. Snapshot the VM
6. Submit to marketplace for review

### First-boot script

Runs once on first deploy. Handles instance-specific setup:

```bash
#!/bin/bash
# /var/lib/cloud/scripts/per-instance/setup.sh

# Get instance metadata
DOMAIN=$(curl -s http://169.254.169.254/v1/load/domain 2>/dev/null || echo "")
PUBLIC_IP=$(curl -s http://169.254.169.254/v1/load/ip || hostname -I | awk '{print $1}')

# Generate secrets
JWT_SECRET=$(openssl rand -hex 32)
POSTGRES_PASSWORD=$(openssl rand -hex 16)
ADMIN_PASSWORD=$(openssl rand -base64 12)

# Write environment file
cat > /opt/echora/.env <<EOF
DOMAIN=${DOMAIN:-$PUBLIC_IP}
JWT_SECRET=${JWT_SECRET}
POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
EOF

# Start services
cd /opt/echora && docker compose up -d

# Run migrations
docker compose exec backend echora-migrate

# Print credentials to console (visible in cloud provider's console output)
echo "=============================="
echo "Echora is ready!"
echo "URL: https://${DOMAIN:-$PUBLIC_IP}"
echo "Admin password: ${ADMIN_PASSWORD}"
echo "=============================="
```

### Supported platforms

| Platform | Mechanism | Image format | Metadata endpoint |
|---|---|---|---|
| Vultr | Marketplace Apps | Snapshot | `http://169.254.169.254/v1/` |
| DigitalOcean | 1-Click Apps | Snapshot | `http://169.254.169.254/metadata/v1/` |
| Linode | StackScripts | Bash script | User-defined fields (UDF) |
| Hetzner | Apps | cloud-init | `http://169.254.169.254/hetzner/v1/` |

### Image build automation with Packer

```hcl
# echora.pkr.hcl
packer {
  required_plugins {
    vultr  = { version = ">= 2.6.0", source = "github.com/vultr/vultr" }
    digitalocean = { version = ">= 1.4.0", source = "github.com/digitalocean/digitalocean" }
  }
}

source "vultr" "echora" {
  api_key      = var.vultr_api_key
  os_id        = 2284  # Ubuntu 24.04
  plan_id      = "vc2-1c-1gb"
  region_id    = "ewr"  # New Jersey
  snapshot_description = "Echora Chat ${var.version}"
}

build {
  sources = ["source.vultr.echora"]

  provisioner "shell" {
    scripts = [
      "packer/scripts/01-base.sh",       # apt update, install Docker
      "packer/scripts/02-echora.sh",     # pull images, write compose file
      "packer/scripts/03-caddy.sh",       # install Caddy
      "packer/scripts/04-firstboot.sh",   # install first-boot script
      "packer/scripts/99-cleanup.sh",     # clean apt cache, zero free space
    ]
  }
}
```

## Approach C: cloud-init

Works on most cloud providers without marketplace approval. User pastes a cloud-init config when creating a VM.

```yaml
#cloud-config
packages:
  - docker.io
  - docker-compose-v2

write_files:
  - path: /opt/echora/docker-compose.yml
    content: |
      # ... same as Approach A ...

runcmd:
  - systemctl enable --now docker
  - cd /opt/echora && docker compose up -d
```

Less polished than marketplace but works everywhere and requires no infrastructure to maintain.

## Comparison

| Aspect | Installer Script | Marketplace Image | cloud-init |
|---|---|---|---|
| Deploy time | 2-5 min | ~60 sec | 2-5 min |
| Platform lock-in | None | Per-provider | Minimal |
| Maintenance | Low (one script) | High (per-provider images) | Low |
| User experience | Good | Best | Okay |
| Approval process | None | Per-provider review | None |
| Works on bare metal | Yes | No | No |

## Update Mechanism

Regardless of approach, need a simple update path:

```bash
# Pull latest images and restart
cd /opt/echora
docker compose pull
docker compose up -d

# Or via the installer script
curl -sSL https://install.echora.example/install.sh | bash -s -- --update
```

Could also add a webhook endpoint to the backend that triggers an update when a GitHub release is published (self-updating, opt-in).

## Backup and Restore

The installer should include backup/restore utilities:

```bash
# Backup -- dump Postgres + config
echora-backup              # creates /opt/echora/backups/echora-YYYY-MM-DD.tar.gz

# Restore
echora-restore /path/to/echora-YYYY-MM-DD.tar.gz
```

Backup contents:
- Postgres dump (`pg_dump`)
- `.env` file (secrets)
- Uploaded files / media (if applicable)

## Security Considerations

- Generate cryptographically random secrets (JWT, Postgres password) on first boot
- Caddy handles TLS automatically (ACME / Let's Encrypt)
- Firewall: only expose 80, 443, and optionally 22 (SSH)
- Postgres not exposed externally (Docker internal network only)
- The install script should never be piped to bash with elevated privileges blindly -- provide a checksum or GPG signature
- Consider adding `ufw` or `nftables` rules in the installer

## Recommended VPS Providers for Self-Hosting

Based on API access, pricing, and suitability for Echora's single-instance architecture:

| Provider | Minimum plan | Approx. cost/mo | Marketplace support | API quality | Notes |
|---|---|---|---|---|---|
| Hetzner | CX22 (2 vCPU / 4GB) | ~$4 | Custom images | Excellent | Best price/performance ratio |
| Vultr | vc2-1c-1gb | ~$6 | Marketplace Apps | Good | One-click apps, Wireguard integration |
| DigitalOcean | Basic 1GB | ~$6 | 1-Click Apps | Excellent | Best docs and developer experience |
| Linode (Akamai) | Nanode 1GB | ~$5 | StackScripts | Good | Managed DB available |
| Oracle Cloud | ARM A1.Flex | Free | None | Decent | Always-free 4 OCPU / 24GB ARM instance |

Any of these are well-suited for Echora's single-instance architecture at low cost.

## Implementation Plan

1. **Phase 1 -- Docker image pipeline**
   - Create a proper slim Dockerfile (fix the known issue: use `debian:bookworm-slim` instead of `rust:latest` for runtime)
   - Add GitHub Actions workflow to build and push to GHCR on tagged releases
   - Create a minimal frontend static-serve image or tarball

2. **Phase 2 -- Installer script**
   - Write `install.sh` with `--domain`, `--email`, `--update`, `--uninstall` flags
   - Include Caddy reverse proxy config generation
   - Include systemd service installation
   - Test on Ubuntu 22.04 and 24.04

3. **Phase 3 -- Documentation**
   - Self-hosting guide with provider-specific notes
   - DNS setup instructions
   - Backup/restore documentation

4. **Phase 4 -- Marketplace images (optional)**
   - Packer configs for Vultr and DigitalOcean
   - First-boot scripts per provider
   - Submit to marketplaces

## References

- [Vultr Marketplace docs](https://www.vultr.com/marketplace/)
- [DigitalOcean 1-Click App guidelines](https://docs.digitalocean.com/products/marketplace/)
- [Linode StackScripts](https://www.linode.com/docs/products/tools/stackscripts/)
- [Hetzner Cloud API](https://docs.hetzner.cloud/)
- [Packer -- build automated machine images](https://www.packer.io/)
- [cloud-init documentation](https://cloudinit.readthedocs.io/)
- [Caddy automatic HTTPS](https://caddyserver.com/docs/automatic-https)
