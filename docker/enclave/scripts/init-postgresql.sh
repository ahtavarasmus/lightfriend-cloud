#!/bin/bash
# Initialize PostgreSQL database cluster and create bridge databases
# This runs on first boot when the data directory is empty

set -e

PGDATA="/var/lib/postgresql/15/main"
PGBIN="/usr/lib/postgresql/15/bin"

# Check if already initialized
if [ -f "$PGDATA/PG_VERSION" ]; then
    echo "PostgreSQL already initialized"
    exit 0
fi

echo "Initializing PostgreSQL database cluster..."

# Initialize the database cluster
su postgres -c "$PGBIN/initdb -D $PGDATA --encoding=UTF8 --locale=C"

# Start PostgreSQL temporarily to create databases
su postgres -c "$PGBIN/pg_ctl -D $PGDATA -o '-c config_file=/etc/lightfriend/config/postgresql.conf' start -w"

# Wait for PostgreSQL to be ready
until su postgres -c "$PGBIN/pg_isready -q"; do
    sleep 1
done

echo "Creating bridge databases..."

# Create databases and users for each bridge
su postgres -c "psql" <<-EOSQL
    -- WhatsApp bridge database
    CREATE USER whatsapp_user WITH PASSWORD 'whatsapp_password';
    CREATE DATABASE whatsapp_db OWNER whatsapp_user;

    -- Signal bridge database
    CREATE USER signal_user WITH PASSWORD 'signal_password';
    CREATE DATABASE signal_db OWNER signal_user;

    -- Messenger bridge database
    CREATE USER messenger_user WITH PASSWORD 'messenger_password';
    CREATE DATABASE messenger_db OWNER messenger_user;

    -- Instagram bridge database
    CREATE USER instagram_user WITH PASSWORD 'instagram_password';
    CREATE DATABASE instagram_db OWNER instagram_user;

    -- Telegram bridge database
    CREATE USER telegram_user WITH PASSWORD 'telegram_password';
    CREATE DATABASE telegram_db OWNER telegram_user;
EOSQL

echo "Bridge databases created"

# Stop the temporary PostgreSQL instance (s6 will start it properly)
su postgres -c "$PGBIN/pg_ctl -D $PGDATA stop -w"

echo "PostgreSQL initialization complete"
