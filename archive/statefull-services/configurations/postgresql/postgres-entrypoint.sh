#!/bin/sh

set -e

echo "Setting up PostgreSQL SSL certificates..."

# Copy and set permissions for private key
cp /run/secrets/pg-server-key.pem /tmp/pg-server-key.pem
chown postgres:postgres /tmp/pg-server-key.pem
chmod 600 /tmp/pg-server-key.pem

# Verify certificate files exist and have correct permissions
echo "Verifying SSL certificate files..."
if [ ! -f /run/secrets/pg-server-cert.pem ]; then
    echo "ERROR: Server certificate not found!"
    exit 1
fi

if [ ! -f /run/secrets/ca.pem ]; then
    echo "ERROR: CA certificate not found!"
    exit 1
fi

# Set proper ownership for certificate files
chown postgres:postgres /tmp/pg-server-key.pem

echo "Starting PostgreSQL with SSL-only configuration..."

# Pass control to the official entrypoint with SSL-enforced configuration
exec docker-entrypoint.sh postgres \
    -c config_file=/etc/postgresql/postgresql.conf \
    -c hba_file=/etc/postgresql/pg_hba.conf \
    -c ssl_key_file='/tmp/pg-server-key.pem' \
    -c log_connections=off \
    -c log_disconnections=off \
    -c log_statement=all