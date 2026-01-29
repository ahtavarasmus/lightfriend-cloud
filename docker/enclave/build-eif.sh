#!/bin/bash
# Build Enclave Image Format (EIF) for AWS Nitro Enclaves
#
# Prerequisites:
# - Docker
# - nitro-cli (from amazon-linux-extras)
#
# Usage: ./build-eif.sh [tag]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TAG="${1:-latest}"

IMAGE_NAME="lightfriend-enclave"
EIF_NAME="lightfriend-enclave.eif"

echo "=========================================="
echo "Building Lightfriend Enclave Image"
echo "=========================================="
echo "Tag: $TAG"
echo "Project root: $PROJECT_ROOT"
echo ""

# Step 1: Build the Docker image
echo "Step 1: Building Docker image..."
docker build \
    -t "${IMAGE_NAME}:${TAG}" \
    -f "$SCRIPT_DIR/Dockerfile" \
    "$PROJECT_ROOT"

echo ""
echo "Docker image built: ${IMAGE_NAME}:${TAG}"

# Step 2: Convert to EIF format
echo ""
echo "Step 2: Converting to EIF format..."

# Check if nitro-cli is available
if ! command -v nitro-cli &> /dev/null; then
    echo "WARNING: nitro-cli not found. Skipping EIF conversion."
    echo "To build EIF, run this on an EC2 instance with Nitro Enclaves enabled:"
    echo "  nitro-cli build-enclave --docker-uri ${IMAGE_NAME}:${TAG} --output-file ${EIF_NAME}"
    exit 0
fi

nitro-cli build-enclave \
    --docker-uri "${IMAGE_NAME}:${TAG}" \
    --output-file "$SCRIPT_DIR/${EIF_NAME}"

echo ""
echo "=========================================="
echo "Build complete!"
echo "=========================================="
echo ""
echo "EIF file: $SCRIPT_DIR/${EIF_NAME}"
echo ""
echo "To run the enclave:"
echo "  nitro-cli run-enclave \\"
echo "    --eif-path $SCRIPT_DIR/${EIF_NAME} \\"
echo "    --cpu-count 2 \\"
echo "    --memory 4096 \\"
echo "    --enclave-cid 16"
echo ""
echo "PCR values for attestation are printed above by nitro-cli."
echo "Save these values for frontend verification."
