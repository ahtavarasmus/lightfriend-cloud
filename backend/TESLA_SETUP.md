# Tesla Integration Setup Guide

This guide explains how to set up and run the Tesla integration with vehicle command support.

## Prerequisites

1. Tesla Fleet API credentials configured (client ID, client secret)
2. Docker installed for running tesla-http-proxy
3. Tesla private/public key pair generated (automatically created on first run)

## Architecture Overview

The Tesla integration uses the **Vehicle Command Protocol** which requires all write commands (lock, unlock, climate control, etc.) to be cryptographically signed. To handle this:

- **tesla-http-proxy**: Official Tesla proxy that signs commands using your private key
- **Backend**: Routes write commands through the proxy, read commands directly to Fleet API
- **Read operations** (vehicle list, charge status): Direct to Tesla Fleet API
- **Write operations** (lock, unlock, climate, wake): Through tesla-http-proxy

## Quick Start

### 1. Start the Tesla HTTP Proxy

The proxy runs as a Docker container and handles command signing:

```bash
cd backend
docker-compose -f docker-compose.tesla-proxy.yml up -d
```

This will:
- Generate TLS certificates (self-signed, for local use)
- Clone and build tesla-http-proxy from Tesla's official repository
- Start the proxy on port 4443 with your private key
- Auto-restart on failures

Check logs:
```bash
docker-compose -f docker-compose.tesla-proxy.yml logs -f
```

### 2. Configure Environment Variables

Add to your backend `.env` file (or set as environment variables):

```bash
# Tesla HTTP Proxy for signed commands (required for vehicle control)
TESLA_HTTP_PROXY_URL=https://localhost:4443

# Tesla OAuth credentials (already configured)
TESLA_CLIENT_ID=your_client_id_here
TESLA_CLIENT_SECRET=your_client_secret_here
TESLA_AUDIENCE=https://fleet-api.prd.eu.vn.cloud.tesla.com  # Or your region

# Server URL for Tesla registration (already configured)
SERVER_URL=https://your-domain.com
```

### 3. Start Your Backend

```bash
cd backend
cargo run
```

On startup, you should see:
```
Tesla EC key pair ready
Public key will be served at /.well-known/appspecific/com.tesla.3p.public-key.pem
Tesla proxy enabled at: https://localhost:4443
```

## 4. Add Virtual Key to Vehicle (REQUIRED)

**CRITICAL STEP**: Before you can send commands to your vehicle, you must add the virtual key to your Tesla using the mobile app.

### Automatic Flow (Recommended)

After connecting your Tesla account via OAuth, the **frontend will automatically display pairing instructions**:

1. **Connect Tesla** in the Connections page
2. Complete OAuth login with Tesla
3. **Pairing section appears automatically** with:
   - Clear step-by-step instructions
   - QR code to scan with Tesla mobile app
   - Direct link button for mobile devices
4. Follow the prompts in your Tesla mobile app
5. Click "I've Completed Pairing" when done

The pairing UI includes:
- ⚠️ Warning banner explaining the requirement
- 📷 QR code for easy scanning
- 📱 Mobile-friendly link button
- ✓ Dismiss option once completed
- 🔑 "Show again" option if dismissed

### Manual API Access

If you need to get the pairing link programmatically:

```bash
GET /api/auth/tesla/virtual-key
Authorization: Bearer <your-token>
```

Response:
```json
{
  "pairing_link": "https://www.tesla.com/_ak/your-domain.com",
  "domain": "your-domain.com",
  "instructions": "Open this link on your mobile device or scan the QR code...",
  "qr_code_url": "https://api.qrserver.com/v1/create-qr-code/?size=300x300&data=..."
}
```

### What Happens During Pairing

- Tesla mobile app shows your domain name (from `SERVER_URL`)
- You confirm access for vehicle commands
- Your public key is registered with the vehicle(s)
- Commands will work immediately after pairing

**Security Note:** You can revoke access anytime via:
- Vehicle's Locks screen in Tesla app
- Tesla Account Security page at tesla.com

## Testing

After adding the virtual key, test the integration by asking your AI assistant to control Tesla:

- "Turn on climate control in my Tesla"
- "Lock my car"
- "What's my battery level?"

Check logs for confirmation:
```
Sending signed command 'auto_conditioning_start' via proxy to vehicle 123456789
```

If you still get "public key has not been paired" errors, ensure you completed the virtual key pairing step above.

## Troubleshooting

### "Public key has not been paired with the vehicle"

**Symptom**: Commands fail with error: `vehicle rejected request: your public key has not been paired with the vehicle`

**Cause**: You haven't completed the virtual key pairing step

**Solution**:
1. Call `/api/auth/tesla/virtual-key` to get your pairing link
2. Open the link on your mobile device or scan the QR code
3. Complete the authorization flow in the Tesla mobile app
4. Retry the command - it should now work

This is a security requirement from Tesla - even with OAuth tokens and a running proxy, you must explicitly authorize your app via the mobile app.

### Proxy not starting

**Error**: Container exits immediately

**Solution**: Check docker logs:
```bash
docker-compose -f docker-compose.tesla-proxy.yml logs
```

Common issues:
- Port 4443 already in use: Change port in docker-compose file and `TESLA_HTTP_PROXY_URL`
- Private key not found: Ensure `tesla_private_key.pem` exists in backend directory

### Commands failing with "Protocol error"

**Symptom**: Commands fail with "Tesla Vehicle Command Protocol required"

**Cause**: Proxy not running or not configured

**Solution**:
1. Verify proxy is running: `docker ps | grep tesla`
2. Check `TESLA_HTTP_PROXY_URL` environment variable is set
3. Verify backend logs show "Tesla proxy enabled at: https://localhost:4443"

If logs show: `Proxy not available - attempting direct command (will likely fail with Protocol error)`:
- Proxy is not running or URL is incorrect
- Start proxy with docker-compose command above

### Certificate errors

**Symptom**: SSL/TLS certificate errors when connecting to proxy

**Cause**: Self-signed certificate used by local proxy

**Solution**: This is expected and handled by the code. The reqwest client is configured with `danger_accept_invalid_certs(true)` for the proxy connection. If you see errors, verify:
- Proxy is running on the correct port
- `TESLA_HTTP_PROXY_URL` uses `https://` protocol
- TLS certificates were generated in `/tls` directory inside container

### Commands timeout

**Symptom**: Commands take a long time or timeout

**Possible causes**:
1. **Vehicle is asleep**: The system automatically wakes the vehicle, but this can take 10-30 seconds
2. **Proxy is slow to start**: First command after proxy start may be slow
3. **Network issues**: Check connectivity to Tesla Fleet API

**Solution**:
- Wait for vehicle to wake fully (system handles this automatically)
- Check proxy logs for errors
- Verify internet connectivity

## File Structure

```
backend/
├── docker-compose.tesla-proxy.yml  # Proxy service configuration
├── tesla_private_key.pem           # Your EC private key (keep secret!)
├── tesla_public_key.pem            # Public key (served to Tesla)
├── tesla-proxy-tls/                # TLS certificates for proxy (auto-generated)
│   ├── key.pem
│   └── cert.pem
└── src/
    ├── api/tesla.rs                # Tesla client with proxy support
    └── tool_call_utils/tesla.rs    # AI tool handlers
```

## Production Deployment

For production deployment:

1. **Use proper TLS certificates**: Replace self-signed certs with valid certificates
2. **Secure the proxy**: Don't expose port 4443 externally
3. **Network security**: Proxy should only be accessible from backend server
4. **Key management**: Keep `tesla_private_key.pem` secure, never commit to git
5. **Consider Kubernetes**: Deploy proxy as a sidecar container

Example production docker-compose:
```yaml
services:
  tesla-http-proxy:
    # Use pre-built image instead of building from source
    image: your-registry/tesla-http-proxy:latest
    volumes:
      - /secure/path/tesla_private_key.pem:/keys/private.pem:ro
      - /etc/ssl/certs:/tls:ro  # Mount real certificates
    networks:
      - internal  # Internal network only
    restart: always
```

## Stopping the Proxy

```bash
docker-compose -f docker-compose.tesla-proxy.yml down
```

To remove volumes and certificates:
```bash
docker-compose -f docker-compose.tesla-proxy.yml down -v
rm -rf tesla-proxy-tls/
```

## Additional Resources

- Tesla Fleet API Documentation: https://developer.tesla.com/docs/fleet-api
- Vehicle Command Protocol: https://github.com/teslamotors/vehicle-command
- Tesla Developer Portal: https://developer.tesla.com
