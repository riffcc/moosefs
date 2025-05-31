#!/bin/bash
set -e

# MooseNG Docker Build Script
# This script builds all Docker images for MooseNG components

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Configuration
IMAGE_TAG=${IMAGE_TAG:-latest}
RUST_VERSION=${RUST_VERSION:-1.79}
BUILD_CONTEXT="."
PUSH_IMAGES=${PUSH_IMAGES:-false}
REGISTRY=${REGISTRY:-}
NO_CACHE=${NO_CACHE:-false}

# Components to build
COMPONENTS=("master" "chunkserver" "client" "cli" "metalogger")

# Function to build a single component
build_component() {
    local component=$1
    local dockerfile="docker/Dockerfile.${component}"
    local image_name="mooseng-${component}"
    
    if [ -n "$REGISTRY" ]; then
        image_name="${REGISTRY}/${image_name}"
    fi
    
    print_status "Building ${component} component..."
    
    local build_args=""
    if [ "$NO_CACHE" = "true" ]; then
        build_args="--no-cache"
    fi
    
    if docker build \
        $build_args \
        --build-arg RUST_VERSION="$RUST_VERSION" \
        -f "$dockerfile" \
        -t "${image_name}:${IMAGE_TAG}" \
        "$BUILD_CONTEXT"; then
        print_success "Successfully built ${image_name}:${IMAGE_TAG}"
        
        # Tag as latest if not already latest
        if [ "$IMAGE_TAG" != "latest" ]; then
            docker tag "${image_name}:${IMAGE_TAG}" "${image_name}:latest"
            print_status "Tagged ${image_name}:latest"
        fi
        
        return 0
    else
        print_error "Failed to build ${component} component"
        return 1
    fi
}

# Function to push images
push_component() {
    local component=$1
    local image_name="mooseng-${component}"
    
    if [ -n "$REGISTRY" ]; then
        image_name="${REGISTRY}/${image_name}"
    fi
    
    print_status "Pushing ${component} component..."
    
    if docker push "${image_name}:${IMAGE_TAG}"; then
        if [ "$IMAGE_TAG" != "latest" ]; then
            docker push "${image_name}:latest"
        fi
        print_success "Successfully pushed ${image_name}"
        return 0
    else
        print_error "Failed to push ${component} component"
        return 1
    fi
}

# Function to show usage
usage() {
    cat << EOF
Usage: $0 [OPTIONS] [COMPONENTS...]

Build Docker images for MooseNG components.

OPTIONS:
    -t, --tag TAG           Set image tag (default: latest)
    -r, --registry REGISTRY Set registry prefix for images
    -p, --push              Push images to registry after building
    -n, --no-cache          Build without using cache
    -h, --help              Show this help message

COMPONENTS:
    Available components: ${COMPONENTS[*]}
    If no components specified, all components will be built.

ENVIRONMENT VARIABLES:
    IMAGE_TAG               Image tag to use (default: latest)
    RUST_VERSION           Rust version for builds (default: 1.79)
    REGISTRY               Registry prefix for images
    PUSH_IMAGES            Push images after building (true/false)
    NO_CACHE               Build without cache (true/false)

EXAMPLES:
    # Build all components
    $0

    # Build specific components
    $0 master chunkserver

    # Build with custom tag
    $0 --tag v1.0.0

    # Build and push to registry
    $0 --registry ghcr.io/myorg --push

    # Build without cache
    $0 --no-cache
EOF
}

# Parse command line arguments
COMPONENTS_TO_BUILD=()

while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--tag)
            IMAGE_TAG="$2"
            shift 2
            ;;
        -r|--registry)
            REGISTRY="$2"
            shift 2
            ;;
        -p|--push)
            PUSH_IMAGES=true
            shift
            ;;
        -n|--no-cache)
            NO_CACHE=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            print_error "Unknown option $1"
            usage
            exit 1
            ;;
        *)
            # Check if it's a valid component
            if [[ " ${COMPONENTS[@]} " =~ " $1 " ]]; then
                COMPONENTS_TO_BUILD+=("$1")
            else
                print_error "Invalid component: $1"
                print_error "Available components: ${COMPONENTS[*]}"
                exit 1
            fi
            shift
            ;;
    esac
done

# If no specific components specified, build all
if [ ${#COMPONENTS_TO_BUILD[@]} -eq 0 ]; then
    COMPONENTS_TO_BUILD=("${COMPONENTS[@]}")
fi

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "docker" ]; then
    print_error "This script must be run from the mooseng project root directory"
    exit 1
fi

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed or not in PATH"
    exit 1
fi

# Show build configuration
print_status "MooseNG Docker Build Configuration:"
echo "  Image tag: $IMAGE_TAG"
echo "  Rust version: $RUST_VERSION"
echo "  Registry: ${REGISTRY:-<none>}"
echo "  Push images: $PUSH_IMAGES"
echo "  No cache: $NO_CACHE"
echo "  Components: ${COMPONENTS_TO_BUILD[*]}"
echo ""

# Build components
failed_builds=()
successful_builds=()

for component in "${COMPONENTS_TO_BUILD[@]}"; do
    if build_component "$component"; then
        successful_builds+=("$component")
    else
        failed_builds+=("$component")
    fi
    echo ""
done

# Push images if requested
if [ "$PUSH_IMAGES" = "true" ] && [ ${#successful_builds[@]} -gt 0 ]; then
    if [ -z "$REGISTRY" ]; then
        print_warning "No registry specified, skipping push"
    else
        print_status "Pushing built images to registry..."
        
        failed_pushes=()
        for component in "${successful_builds[@]}"; do
            if ! push_component "$component"; then
                failed_pushes+=("$component")
            fi
        done
        
        if [ ${#failed_pushes[@]} -gt 0 ]; then
            print_error "Failed to push: ${failed_pushes[*]}"
        fi
    fi
fi

# Summary
echo ""
print_status "Build Summary:"
if [ ${#successful_builds[@]} -gt 0 ]; then
    print_success "Successfully built: ${successful_builds[*]}"
fi

if [ ${#failed_builds[@]} -gt 0 ]; then
    print_error "Failed to build: ${failed_builds[*]}"
    exit 1
else
    print_success "All components built successfully!"
fi