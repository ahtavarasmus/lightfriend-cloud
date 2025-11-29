#!/bin/bash

# Script to update Lightfriend: pull code, build, migrate, and restart services in safe order.
# Run as: ./update_lightfriend.sh [proxy_sleep] [homeserver_sleep] [bridges_sleep]
# Defaults: 10s after tesla proxy, 30s after homeserver, 10s after bridges.

# Configurable paths and sleeps
LIGHTFRIEND_DIR="$HOME/lightfriend-cloud"
DB_DIR="/var/lib/lightfriend"
PROXY_SLEEP=${1:-10}
HOMESERVER_SLEEP=${2:-30}
BRIDGES_SLEEP=${3:-10}

# Step 1: Pull and build
cd "$LIGHTFRIEND_DIR" || { echo "Error: Can't cd to $LIGHTFRIEND_DIR"; exit 1; }
git pull || { echo "Git pull failed"; exit 1; }

cp "${DB_DIR}/database.db" "$HOME/backup-lightfriend.db" || { echo "Database backup failed"; exit 1; }
cd backend || { echo "Error: Can't cd to backend"; exit 1; }
diesel migration run || { echo "Diesel migration failed"; exit 1; }
# Added: Check if target dir > 10GB and clean only if needed
if [ -d target ]; then
    target_size=$(du -sb target | cut -f1)
    if [ "$target_size" -gt 10737418240 ]; then
        echo "target dir is over 10GB ($target_size bytes); running cargo clean..."
        cargo clean || { echo "Cargo clean failed"; exit 1; }
    else
        echo "target dir is under 10GB ($target_size bytes); skipping cargo clean."
    fi
else
    echo "No target dir found; skipping clean."
fi
cargo build --release || { echo "Cargo build failed"; exit 1; }

cd ../frontend || { echo "Error: Can't cd to frontend"; exit 1; }
trunk build --release || { echo "Trunk build failed"; exit 1; }

cd ..  # Back to Lightfriend root

# Step 2: Restart services in order with waits
echo "Restarting tesla-proxy.service..."
sudo systemctl restart tesla-proxy.service || { echo "Restart tesla-proxy failed"; exit 1; }
echo "Waiting $PROXY_SLEEP seconds for Tesla proxy to stabilize..."
sleep "$PROXY_SLEEP"

# Verify Tesla proxy is healthy
echo "Checking Tesla proxy health..."
proxy_status=$(sudo docker ps --filter name=tesla-http-proxy --format "{{.Status}}")
if echo "$proxy_status" | grep -q "healthy"; then
    echo "Tesla proxy is healthy: $proxy_status"
elif echo "$proxy_status" | grep -q "Up"; then
    echo "Warning: Tesla proxy is up but healthcheck not yet complete: $proxy_status"
else
    echo "Error: Tesla proxy is not running properly: $proxy_status"
    exit 1
fi

echo "Restarting matrix-homeserver.service..."
sudo systemctl restart matrix-homeserver.service || { echo "Restart homeserver failed"; exit 1; }
echo "Waiting $HOMESERVER_SLEEP seconds for homeserver to stabilize..."
sleep "$HOMESERVER_SLEEP"

echo "Restarting bridges..."
sudo systemctl restart mautrix-whatsapp.service mautrix-telegram.service mautrix-signal.service || { echo "Restart bridges failed"; exit 1; }
echo "Waiting $BRIDGES_SLEEP seconds for bridges to connect..."
sleep "$BRIDGES_SLEEP"

echo "Restarting lightfriend.service..."
sudo systemctl restart lightfriend.service || { echo "Restart lightfriend failed"; exit 1; }

echo "Update successfully completed."
