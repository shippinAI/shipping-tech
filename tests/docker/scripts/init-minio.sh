#!/bin/bash
set -e

# Wait for MinIO to be ready
echo "Waiting for MinIO..."
until mc alias set local http://minio:9000 minioadmin minioadmin 2>/dev/null; do
  sleep 1
done

# Create the shippin.tech bucket
mc mb local/shippin.tech --ignore-existing
echo "MinIO bucket 'shippin.tech' created."
