#!/bin/bash
# Fetch configuration from parent instance via VSOCK
# This runs once at startup to retrieve encrypted configs

set -e

echo "Fetching configuration from parent instance..."

CONFIG_DIR="/data/config"
BRIDGE_DIR="/data/bridges"

mkdir -p "$CONFIG_DIR" "$BRIDGE_DIR"/{whatsapp,signal,messenger,instagram,telegram}

# Wait for VSOCK proxy to be ready
sleep 2

# Fetch configuration bundle from parent via state sync port
# The parent serves a JSON bundle with all necessary configs
CONFIG_URL="http://127.0.0.1:${VSOCK_STATE_PORT:-5002}/config"

if curl -sf "$CONFIG_URL" -o /tmp/config_bundle.json; then
    echo "Configuration bundle received"

    # Extract environment variables
    jq -r '.env // empty' /tmp/config_bundle.json > /tmp/enclave.env 2>/dev/null || true
    if [ -s /tmp/enclave.env ]; then
        set -a
        source /tmp/enclave.env
        set +a
        echo "Environment variables loaded"
    fi

    # Generate bridge configs from templates
    for bridge in whatsapp signal messenger instagram telegram; do
        template="/etc/lightfriend/config/${bridge}.yaml.template"
        output="${BRIDGE_DIR}/${bridge}/config.yaml"

        if [ -f "$template" ]; then
            envsubst < "$template" > "$output"
            echo "Generated ${bridge} config"
        fi
    done

    rm -f /tmp/config_bundle.json /tmp/enclave.env
    echo "Configuration complete"
else
    echo "WARNING: Could not fetch config from parent, using defaults"

    # Generate configs with default/environment values
    for bridge in whatsapp signal messenger instagram telegram; do
        template="/etc/lightfriend/config/${bridge}.yaml.template"
        output="${BRIDGE_DIR}/${bridge}/config.yaml"

        if [ -f "$template" ]; then
            envsubst < "$template" > "$output"
            echo "Generated ${bridge} config from template"
        fi
    done
fi

echo "Config fetcher complete"
