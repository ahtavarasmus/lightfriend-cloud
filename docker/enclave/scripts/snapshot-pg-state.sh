#!/bin/bash
# Snapshot PostgreSQL state and send to parent for persistence
# Called periodically (every 5 min) and on graceful shutdown

set -e

echo "Creating PostgreSQL state snapshot..."

SNAPSHOT_DIR="/tmp/pg_snapshot"
STATE_URL="http://127.0.0.1:${VSOCK_STATE_PORT:-5002}/state/postgres"

mkdir -p "$SNAPSHOT_DIR"
cd "$SNAPSHOT_DIR"

PGBIN="/usr/lib/postgresql/15/bin"

# Dump each bridge database
for db in whatsapp signal messenger instagram telegram; do
    echo "Dumping ${db}_db..."
    $PGBIN/pg_dump -h localhost -U ${db}_user ${db}_db > "${db}_db.sql" 2>/dev/null || true
done

# Create compressed archive
tar -czf pg_backup.tar.gz *.sql

# Upload to parent via VSOCK state sync port
if curl -sf -X PUT --data-binary @pg_backup.tar.gz "$STATE_URL"; then
    echo "State snapshot uploaded to parent"
else
    echo "WARNING: Failed to upload state snapshot to parent"
fi

# Cleanup
rm -rf "$SNAPSHOT_DIR"

echo "Snapshot complete"
