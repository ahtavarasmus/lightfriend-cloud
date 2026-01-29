# Lightfriend Nitro Enclave

Single-container deployment for AWS Nitro Enclaves with privacy-preserving architecture.

## What's Inside

- **Lightfriend Core** (Rust backend)
- **5× mautrix bridges** (WhatsApp, Signal, Messenger, Instagram, Telegram)
- **PostgreSQL 15** (ephemeral, in-enclave)
- **s6-overlay** (process supervisor)
- **VSOCK proxy** (communication with parent instance)

## Build Modes

### Source Build (Default)

Builds Core from Rust source. Slow (~15-20 min) but reproducible.

```bash
./build-eif.sh
```

**Use for:**
- Production builds
- When you need PCR values to match your exact code
- Security-critical deployments

### Prebuilt Mode (Fast)

Uses pre-built image from Docker Hub. Fast (~2-3 min).

```bash
# Use latest
./build-eif.sh latest prebuilt

# Use specific commit/branch
./build-eif.sh latest prebuilt master-abc123
```

**Use for:**
- Testing enclave configuration
- Iterating on s6 services or scripts
- Development when Core code hasn't changed

**Available tags:** https://hub.docker.com/r/ahtavarasmus/lightfriend-core/tags

## Manual Build

```bash
# Source build
docker build -f Dockerfile -t lightfriend-enclave:latest ../..

# Prebuilt
docker build -f Dockerfile -t lightfriend-enclave:test \
  --build-arg CORE_BINARY_SOURCE=prebuilt \
  --build-arg CORE_IMAGE=ahtavarasmus/lightfriend-core:master-abc123 \
  ../..
```

## Running on EC2

### 1. Build EIF

On an EC2 instance with Nitro Enclaves enabled:

```bash
# Transfer Docker image or rebuild
docker pull lightfriend-enclave:latest

# Convert to EIF
nitro-cli build-enclave \
  --docker-uri lightfriend-enclave:latest \
  --output-file lightfriend-enclave.eif
```

This outputs **PCR values** - save these for attestation!

### 2. Run Enclave

```bash
nitro-cli run-enclave \
  --eif-path lightfriend-enclave.eif \
  --cpu-count 2 \
  --memory 4096 \
  --enclave-cid 16
```

### 3. Check Status

```bash
# List running enclaves
nitro-cli describe-enclaves

# View console output
nitro-cli console --enclave-id <ID>
```

## VSOCK Communication

The enclave communicates with the parent instance via VSOCK:

| Port | Service | Purpose |
|------|---------|---------|
| 5000 | Synapse proxy | Matrix homeserver access |
| 5001 | Internet proxy | Outbound HTTP/HTTPS |
| 5002 | State sync | Encrypted state persistence |
| 5003 | Attestation | Attestation document serving |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  ENCLAVE (isolated from parent)                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │ s6-overlay (PID 1)                               │   │
│  │  ├── vsock-proxy (VSOCK ↔ TCP)                   │   │
│  │  ├── postgresql (ephemeral storage)              │   │
│  │  ├── config-fetcher (startup)                    │   │
│  │  ├── mautrix-{whatsapp,signal,messenger,         │   │
│  │  │              instagram,telegram}              │   │
│  │  └── lightfriend-core                            │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**Startup order:** vsock-proxy → config-fetcher → postgresql → bridges (parallel) → core

## Troubleshooting

### Build fails with "No space left on device"

Increase Docker disk space:
```bash
docker system prune -a
```

### Enclave fails to start

Check allocator configuration on EC2:
```bash
cat /etc/nitro_enclaves/allocator.yaml
sudo systemctl status nitro-enclaves-allocator
```

### Services not starting

Check enclave console:
```bash
nitro-cli console --enclave-id <ID>
```

Common issues:
- VSOCK proxy not connecting (check parent services)
- PostgreSQL init failed (check memory allocation)
- Bridge startup timeout (increase startup timeout)

## Files

```
docker/enclave/
├── Dockerfile              # Multi-stage build
├── build-eif.sh            # Build script
├── README.md               # This file
├── s6-rc.d/                # Service definitions
│   ├── user/               # Service bundle
│   ├── vsock-proxy/
│   ├── config-fetcher/
│   ├── postgresql/
│   ├── mautrix-*/          # 5 bridge services
│   └── lightfriend-core/
├── config/                 # Configuration templates
│   ├── postgresql.conf
│   ├── pg_hba.conf
│   └── *.yaml.template     # Bridge configs
└── scripts/                # Helper scripts
    ├── vsock-proxy.sh
    ├── config-fetcher.sh
    ├── init-postgresql.sh
    ├── wait-for-postgres.sh
    ├── restore-pg-state.sh
    └── snapshot-pg-state.sh
```

## See Also

- [INFRASTRUCTURE_SETUP.md](../../docs/INFRASTRUCTURE_SETUP.md) - Terraform setup and deployment guide
- [CLAUDE.md](../../CLAUDE.md) - Project architecture and development guide
