#!/bin/bash
# Build script for MooseNG Docker images
# This handles the workspace dependencies properly

set -e

echo "🔧 Building MooseNG Docker Images..."
echo "==================================="

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Must run from mooseng directory"
    exit 1
fi

# Function to build a component
build_component() {
    local component=$1
    local dockerfile=$2
    
    echo ""
    echo "🏗️  Building $component..."
    
    # Create a temporary build context with all workspace members
    TEMP_DIR=$(mktemp -d)
    echo "📁 Creating build context in $TEMP_DIR"
    
    # Copy workspace files
    cp Cargo.toml Cargo.lock $TEMP_DIR/
    
    # Copy all workspace members (only the ones that exist)
    for member in mooseng-common mooseng-protocol mooseng-master mooseng-chunkserver mooseng-client mooseng-metalogger mooseng-cli mooseng-benchmarks; do
        if [ -d "$member" ]; then
            echo "📄 Copying $member..."
            cp -r $member $TEMP_DIR/
        fi
    done
    
    # Copy src directory if it exists (for main package)
    if [ -d "src" ]; then
        cp -r src $TEMP_DIR/
    fi
    
    # Copy docker directory for configs and scripts
    cp -r docker $TEMP_DIR/
    
    # Build the Docker image
    echo "🐳 Building Docker image..."
    docker build -t mooseng/$component:latest -f $dockerfile $TEMP_DIR
    
    # Clean up
    rm -rf $TEMP_DIR
    
    echo "✅ $component built successfully!"
}

# Parse command line arguments
if [ $# -eq 0 ]; then
    # Build all components
    COMPONENTS="chunkserver metalogger cli"
else
    COMPONENTS="$@"
fi

# Build requested components
for component in $COMPONENTS; do
    case $component in
        chunkserver)
            build_component "chunkserver" "docker/Dockerfile.chunkserver"
            ;;
        metalogger)
            build_component "metalogger" "docker/Dockerfile.metalogger"
            ;;
        cli)
            build_component "cli" "docker/Dockerfile.cli"
            ;;
        master)
            echo "⚠️  Master component build is currently disabled (pending implementation)"
            ;;
        client)
            echo "⚠️  Client component build is currently disabled (pending implementation)"
            ;;
        *)
            echo "❌ Unknown component: $component"
            echo "Available components: chunkserver, metalogger, cli"
            exit 1
            ;;
    esac
done

echo ""
echo "🎉 Build complete!"
echo "================="
echo "Built images:"
docker images | grep mooseng | head -10