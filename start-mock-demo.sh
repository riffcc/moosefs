#!/bin/bash

# MooseNG Mock Demo Startup Script
# Demonstrates the architecture with mock components

set -e

# Get the directory of this script
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Source the shared library
source "${SCRIPT_DIR}/mooseng-demo-lib.sh"

# Configuration
readonly COMPOSE_FILE="docker-compose.mock-demo.yml"
readonly MOOSENG_DIR="mooseng"
readonly CLIENT_MOUNT_DIRS=("mnt/client-1" "mnt/client-2" "mnt/client-3")
readonly STARTUP_WAIT_TIME=10

# Service endpoints configuration
declare -A MASTER_ENDPOINTS=(
    ["1"]="http://localhost:9421"
    ["2"]="http://localhost:9431"
    ["3"]="http://localhost:9441"
)

declare -A CHUNKSERVER_ENDPOINTS=(
    ["1"]="http://localhost:9420"
    ["2"]="http://localhost:9450"
    ["3"]="http://localhost:9460"
)

# Function to display startup banner
display_startup_banner() {
    echo "🚀 Starting MooseNG Mock Demo..."
    echo "================================"
    print_warning "⚠️  Note: This uses mock components to demonstrate the architecture"
    echo "   Real MooseNG components will replace these once implemented"
    echo ""
}

# Function to setup working directory
setup_working_directory() {
    print_info "📁 Setting up working directory..."
    
    cd "$MOOSENG_DIR" || {
        print_error "❌ Error: Failed to navigate to $MOOSENG_DIR directory"
        exit 1
    }
    
    print_info "📁 Working directory: $(pwd)"
}

# Function to display all endpoints
display_all_endpoints() {
    print_header "📋 Service Endpoints" "-"
    
    display_service_endpoints MASTER_ENDPOINTS "Mock Master Servers" "🔧"
    echo ""
    display_service_endpoints CHUNKSERVER_ENDPOINTS "Mock ChunkServers" "💾"
    
    print_header "📊 Monitoring" "-"
    echo "📈 Prometheus:      http://localhost:9090"
    echo "📊 Grafana:         http://localhost:3000 (admin/admin)"
    
    print_header "🛠️  Management" "-"
    echo "To access CLI: docker compose -f $COMPOSE_FILE exec cli /bin/sh"
}

# Function to display completion message
display_completion_message() {
    echo ""
    print_success "🎉 MooseNG Mock Demo Started!"
    echo "============================="
}

# Main execution function
main() {
    display_startup_banner
    
    # Use shared library functions
    check_docker_prerequisites || exit 1
    
    setup_working_directory
    
    # Validate compose file exists
    validate_compose_file "$COMPOSE_FILE" || exit 1
    
    # Create directories using shared function
    create_mount_directories "${CLIENT_MOUNT_DIRS[@]}" || exit 1
    
    # Clean up and start services
    cleanup_containers "$COMPOSE_FILE"
    start_docker_services "$COMPOSE_FILE" || exit 1
    
    # Wait for services
    wait_for_services_animated "$STARTUP_WAIT_TIME" "Waiting for mock services to initialize..."
    
    # Show status and endpoints
    show_service_status "$COMPOSE_FILE"
    display_completion_message
    
    display_all_endpoints
    
    # Display helpful commands
    local additional_commands=(
        "Test client:    docker compose -f $COMPOSE_FILE exec client-1 /bin/sh"
        "Monitor logs:   docker compose -f $COMPOSE_FILE logs -f --tail=50"
    )
    display_command_help "$COMPOSE_FILE" "${additional_commands[@]}"
    
    echo ""
    print_success "✨ Mock demo is ready for architecture demonstration!"
}

# Execute main function
main "$@"