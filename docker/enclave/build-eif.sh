#!/bin/bash
# Build Enclave Image Format (EIF) for AWS Nitro Enclaves
#
# Prerequisites:
# - Docker
# - nitro-cli (from amazon-linux-extras, only needed for EIF conversion)
#
# Usage:
#   ./build-eif.sh [tag]                           # Build from source (default)
#   ./build-eif.sh [tag] prebuilt                  # Use pre-built image (fast)
#   ./build-eif.sh [tag] prebuilt master-abc123    # Use specific pre-built tag

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TAG="${1:-latest}"
BUILD_MODE="${2:-source}"
CORE_IMAGE_TAG="${3:-latest}"

IMAGE_NAME="lightfriend-enclave"
EIF_NAME="lightfriend-enclave.eif"
CORE_IMAGE="ahtavarasmus/lightfriend-core:${CORE_IMAGE_TAG}"

echo "=========================================="
echo "Building Lightfriend Enclave Image"
echo "=========================================="
echo "Tag: $TAG"
echo "Build mode: $BUILD_MODE"
if [ "$BUILD_MODE" = "prebuilt" ]; then
    echo "Core image: $CORE_IMAGE"
fi
echo "Project root: $PROJECT_ROOT"
echo ""

# Step 1: Build the Docker image
echo "Step 1: Building Docker image..."
if [ "$BUILD_MODE" = "prebuilt" ]; then
    echo "Using pre-built core image (fast build)..."
    docker build \
        -t "${IMAGE_NAME}:${TAG}" \
        -f "$SCRIPT_DIR/Dockerfile" \
        --build-arg CORE_BINARY_SOURCE=prebuilt \
        --build-arg CORE_IMAGE="$CORE_IMAGE" \
        "$PROJECT_ROOT"
else
    echo "Building from source (slow, ~15-20 min)..."
    docker build \
        -t "${IMAGE_NAME}:${TAG}" \
        -f "$SCRIPT_DIR/Dockerfile" \
        "$PROJECT_ROOT"
fi

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
