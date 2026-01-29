#!/bin/bash
# VSOCK proxy service
# Proxies TCP connections from enclave to parent instance via VSOCK
#
# VSOCK Ports:
# 5000 - Synapse (Matrix homeserver)
# 5001 - Internet proxy (for external API calls)
# 5002 - State sync (for encrypted state persistence)
# 5003 - Attestation service

set -e

echo "Starting VSOCK proxy services..."

# Parent instance CID (always 3 for parent from enclave perspective)
PARENT_CID=3

# Start proxy for Synapse (Matrix homeserver)
# Listens on localhost:5000, forwards to parent's VSOCK port 5000
socat TCP-LISTEN:${VSOCK_SYNAPSE_PORT:-5000},bind=127.0.0.1,fork,reuseaddr \
    VSOCK-CONNECT:${PARENT_CID}:${VSOCK_SYNAPSE_PORT:-5000} &

# Start proxy for internet access (HTTP/HTTPS)
# Listens on localhost:5001, forwards to parent's VSOCK port 5001
socat TCP-LISTEN:${VSOCK_INTERNET_PORT:-5001},bind=127.0.0.1,fork,reuseaddr \
    VSOCK-CONNECT:${PARENT_CID}:${VSOCK_INTERNET_PORT:-5001} &

# Start proxy for state sync
# Listens on localhost:5002, forwards to parent's VSOCK port 5002
socat TCP-LISTEN:${VSOCK_STATE_PORT:-5002},bind=127.0.0.1,fork,reuseaddr \
    VSOCK-CONNECT:${PARENT_CID}:${VSOCK_STATE_PORT:-5002} &

# Start proxy for attestation service
# Listens on localhost:5003, forwards to parent's VSOCK port 5003
socat TCP-LISTEN:${VSOCK_ATTESTATION_PORT:-5003},bind=127.0.0.1,fork,reuseaddr \
    VSOCK-CONNECT:${PARENT_CID}:${VSOCK_ATTESTATION_PORT:-5003} &

echo "VSOCK proxies started:"
echo "  - Synapse: localhost:${VSOCK_SYNAPSE_PORT:-5000} -> parent:${VSOCK_SYNAPSE_PORT:-5000}"
echo "  - Internet: localhost:${VSOCK_INTERNET_PORT:-5001} -> parent:${VSOCK_INTERNET_PORT:-5001}"
echo "  - State sync: localhost:${VSOCK_STATE_PORT:-5002} -> parent:${VSOCK_STATE_PORT:-5002}"
echo "  - Attestation: localhost:${VSOCK_ATTESTATION_PORT:-5003} -> parent:${VSOCK_ATTESTATION_PORT:-5003}"

# Wait for any child process to exit
wait -n

# If we get here, a proxy died - exit with error
echo "ERROR: A VSOCK proxy process died unexpectedly"
exit 1
