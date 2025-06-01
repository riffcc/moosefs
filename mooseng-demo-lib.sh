#!/bin/bash

# MooseNG Demo Scripts Shared Library
# Common functions for demo startup scripts
#
# Usage: Source this file in your demo scripts:
#   source ./mooseng-demo-lib.sh
#
# Environment Variables:
#   DEBUG=1  Enable debug output
#   NO_COLOR=1  Disable colored output

# Exit on error, undefined variable, and pipe failures
set -euo pipefail

# Color codes for output
if [[ "${NO_COLOR:-0}" == "1" ]]; then
    readonly RED=''
    readonly GREEN=''
    readonly YELLOW=''
    readonly BLUE=''
    readonly CYAN=''
    readonly MAGENTA=''
    readonly NC=''
else
    readonly RED='\033[0;31m'
    readonly GREEN='\033[0;32m'
    readonly YELLOW='\033[1;33m'
    readonly BLUE='\033[0;34m'
    readonly CYAN='\033[0;36m'
    readonly MAGENTA='\033[0;35m'
    readonly NC='\033[0m' # No Color
fi

# Print functions with validation
# Usage: print_info "message"
# Output: Blue colored informational message
print_info() {
    [[ $# -eq 0 ]] && return 1
    echo -e "${BLUE}$*${NC}"
}

# Usage: print_success "message"
# Output: Green colored success message
print_success() {
    [[ $# -eq 0 ]] && return 1
    echo -e "${GREEN}$*${NC}"
}

# Usage: print_warning "message"
# Output: Yellow colored warning message
print_warning() {
    [[ $# -eq 0 ]] && return 1
    echo -e "${YELLOW}$*${NC}" >&2
}

# Usage: print_error "message"
# Output: Red colored error message to stderr
print_error() {
    [[ $# -eq 0 ]] && return 1
    echo -e "${RED}$*${NC}" >&2
}

# Usage: print_debug "message"
# Output: Cyan colored debug message (only if DEBUG=1)
print_debug() {
    [[ $# -eq 0 ]] && return 1
    if [[ "${DEBUG:-0}" == "1" ]]; then
        echo -e "${CYAN}[DEBUG] $*${NC}" >&2
    fi
}

# Usage: print_header "Title" ["separator_char"]
# Output: Title with underline using separator character (default: =)
# Example: print_header "Setup" "-"
print_header() {
    [[ $# -eq 0 ]] && { print_error "print_header: title required"; return 1; }
    local title="$1"
    local separator="${2:-=}"
    
    # Validate separator is single character
    [[ ${#separator} -ne 1 ]] && { print_error "print_header: separator must be single character"; return 1; }
    
    echo ""
    echo "$title"
    echo "${title//?/$separator}"
}

# Usage: show_spinner PID ["message"]
# Display animated spinner while process is running
# Example: long_command & show_spinner $! "Processing..."
show_spinner() {
    [[ $# -eq 0 ]] && { print_error "show_spinner: PID required"; return 1; }
    local pid=$1
    local message="${2:-Working...}"
    local spinner='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    local delay=0.1
    
    # Validate PID
    if ! kill -0 "$pid" 2>/dev/null; then
        print_debug "show_spinner: PID $pid not found or already finished"
        return 0
    fi
    
    # Disable spinner if output is not a terminal
    if [[ ! -t 1 ]]; then
        echo "$message"
        wait "$pid" 2>/dev/null || true
        return 0
    fi
    
    while kill -0 "$pid" 2>/dev/null; do
        for i in $(seq 0 $((${#spinner} - 1))); do
            printf "\r  ${spinner:$i:1} %s" "$message"
            sleep $delay
        done
    done
    printf "\r  ✓ %-${#message}s\n" "Done!"
}

# Usage: check_docker_prerequisites
# Check if Docker and Docker Compose are installed and running
# Returns: 0 on success, 1 on failure
check_docker_prerequisites() {
    print_info "🔍 Checking Docker prerequisites..."
    
    # Check if Docker command exists
    if ! command -v docker &> /dev/null; then
        print_error "❌ Error: Docker command not found. Please install Docker."
        return 1
    fi
    
    # Check if Docker daemon is running
    if ! docker info >/dev/null 2>&1; then
        print_error "❌ Error: Docker is not running. Please start Docker first."
        print_debug "Docker info output: $(docker info 2>&1 || true)"
        return 1
    fi
    
    # Check Docker Compose availability (both standalone and plugin)
    local compose_available=0
    if command -v docker-compose &> /dev/null; then
        compose_available=1
        print_debug "Found standalone docker-compose"
    fi
    
    if docker compose version &> /dev/null 2>&1; then
        compose_available=1
        print_debug "Found docker compose plugin"
    fi
    
    if [[ $compose_available -eq 0 ]]; then
        print_error "❌ Error: Docker Compose is not available."
        print_error "Install with: docker plugin install compose"
        return 1
    fi
    
    print_success "✅ Docker prerequisites check passed"
    return 0
}

# Usage: create_mount_directories dir1 [dir2 ...]
# Create directories with proper permissions for Docker mounts
# Returns: 0 on success, 1 on any failure
create_mount_directories() {
    [[ $# -eq 0 ]] && { print_error "create_mount_directories: at least one directory required"; return 1; }
    local dirs=("$@")
    local failed=0
    
    print_info "📂 Creating mount directories..."
    
    for dir in "${dirs[@]}"; do
        # Skip empty directory names
        [[ -z "$dir" ]] && continue
        
        # Create directory with parent directories
        if mkdir -p "$dir" 2>/dev/null; then
            # Ensure directory is writable by current user
            if [[ -w "$dir" ]]; then
                print_success "  ✓ Created $dir"
            else
                print_warning "  ⚠️  Created $dir but it's not writable"
                chmod u+w "$dir" 2>/dev/null || true
            fi
        else
            print_error "  ✗ Failed to create $dir"
            print_debug "mkdir error: $(mkdir -p "$dir" 2>&1 || true)"
            ((failed++))
        fi
    done
    
    [[ $failed -eq 0 ]] && return 0 || return 1
}

# Usage: cleanup_containers compose_file ["--volumes"]
# Stop and remove containers defined in compose file
# Options: --volumes to also remove volumes
# Returns: 0 on success, 1 on failure
cleanup_containers() {
    [[ $# -eq 0 ]] && { print_error "cleanup_containers: compose file required"; return 1; }
    local compose_file="$1"
    local extra_opts="${2:-}"
    
    # Validate compose file exists
    if [[ ! -f "$compose_file" ]]; then
        print_error "cleanup_containers: compose file not found: $compose_file"
        return 1
    fi
    
    print_info "🧹 Cleaning up existing containers..."
    
    # Check if any containers exist
    local existing_containers
    existing_containers=$(docker compose -f "$compose_file" ps --quiet 2>/dev/null || true)
    
    if [[ -n "$existing_containers" ]]; then
        print_debug "Found containers: $(echo "$existing_containers" | wc -l) running"
        
        # Attempt graceful shutdown first
        if docker compose -f "$compose_file" down $extra_opts 2>/dev/null; then
            print_success "  ✓ Cleaned up existing containers"
        else
            # Force removal if graceful shutdown fails
            print_warning "  ⚠️  Graceful shutdown failed, forcing removal..."
            if docker compose -f "$compose_file" down --remove-orphans --timeout 0 $extra_opts 2>/dev/null; then
                print_success "  ✓ Forcefully removed containers"
            else
                print_error "  ✗ Failed to clean up containers"
                return 1
            fi
        fi
    else
        print_info "  ℹ️  No existing containers to clean up"
    fi
    
    return 0
}

# Usage: start_docker_services compose_file [extra_args]
# Start services defined in docker-compose file
# Example: start_docker_services docker-compose.yml "--scale worker=3"
# Returns: 0 on success, 1 on failure
start_docker_services() {
    [[ $# -eq 0 ]] && { print_error "start_docker_services: compose file required"; return 1; }
    local compose_file="$1"
    local extra_args="${2:-}"
    
    # Validate compose file
    if ! validate_compose_file "$compose_file"; then
        return 1
    fi
    
    print_info "🔧 Starting services..."
    
    # Pull images first if connected
    if docker compose -f "$compose_file" pull 2>/dev/null; then
        print_debug "Successfully pulled latest images"
    else
        print_debug "Could not pull images (offline or registry issue)"
    fi
    
    # Start services
    if docker compose -f "$compose_file" up -d $extra_args 2>&1 | while read -r line; do
        print_debug "$line"
    done; then
        print_success "✅ Services started successfully"
        
        # Show service count
        local service_count
        service_count=$(docker compose -f "$compose_file" ps --quiet | wc -l)
        print_info "  ℹ️  Started $service_count service(s)"
        
        return 0
    else
        print_error "❌ Error: Failed to start services"
        # Show recent logs for debugging
        print_error "Recent logs:"
        docker compose -f "$compose_file" logs --tail=20 2>&1 | while read -r line; do
            print_error "  $line"
        done
        return 1
    fi
}

# Usage: wait_for_services_animated [wait_time] ["message"]
# Display animated progress while waiting
# Default: 10 seconds with standard message
# Returns: Always returns 0
wait_for_services_animated() {
    local wait_time="${1:-10}"
    local message="${2:-Waiting for services to be ready...}"
    
    # Validate wait_time is numeric
    if ! [[ "$wait_time" =~ ^[0-9]+$ ]]; then
        print_error "wait_for_services_animated: wait_time must be numeric"
        return 1
    fi
    
    print_info "⏳ $message"
    
    # Skip animation if not in terminal
    if [[ ! -t 1 ]]; then
        sleep "$wait_time"
        print_info "  ✓ Wait complete"
        return 0
    fi
    
    local elapsed=0
    local spinner='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    
    while [[ $elapsed -lt $wait_time ]]; do
        local spin_index=$((elapsed % ${#spinner}))
        printf "\r  ${spinner:$spin_index:1} Waiting... %ds/%ds" "$elapsed" "$wait_time"
        sleep 1
        ((elapsed++))
    done
    
    printf "\r  ✓ %-${#message}s\n" "Services should be ready now!"
    return 0
}

# Usage: check_service_health service_name compose_file [max_attempts]
# Check if a service reports healthy status
# Default: 30 attempts (30 seconds)
# Returns: 0 if healthy, 1 if timeout
check_service_health() {
    [[ $# -lt 2 ]] && { print_error "check_service_health: service_name and compose_file required"; return 1; }
    local service_name="$1"
    local compose_file="$2"
    local max_attempts="${3:-30}"
    
    # Validate inputs
    if [[ ! -f "$compose_file" ]]; then
        print_error "check_service_health: compose file not found: $compose_file"
        return 1
    fi
    
    if ! [[ "$max_attempts" =~ ^[0-9]+$ ]] || [[ $max_attempts -le 0 ]]; then
        print_error "check_service_health: max_attempts must be positive integer"
        return 1
    fi
    
    print_info "🏥 Checking health of $service_name..."
    local attempts=0
    
    while [[ $attempts -lt $max_attempts ]]; do
        # Get service status
        local status
        status=$(docker compose -f "$compose_file" ps "$service_name" 2>/dev/null || true)
        
        if [[ -z "$status" ]]; then
            print_debug "Service $service_name not found"
            return 1
        fi
        
        # Check for healthy status
        if echo "$status" | grep -q "healthy"; then
            print_success "  ✓ $service_name is healthy"
            return 0
        fi
        
        # Check if service exited
        if echo "$status" | grep -q "Exit"; then
            print_error "  ✗ $service_name has exited"
            return 1
        fi
        
        print_debug "Attempt $((attempts + 1))/$max_attempts: $service_name not healthy yet"
        sleep 1
        ((attempts++))
    done
    
    print_error "  ✗ $service_name health check timeout after $max_attempts seconds"
    return 1
}

# Usage: show_service_status compose_file
# Display current status of all services
# Returns: 0 on success, 1 on failure
show_service_status() {
    [[ $# -eq 0 ]] && { print_error "show_service_status: compose file required"; return 1; }
    local compose_file="$1"
    
    if [[ ! -f "$compose_file" ]]; then
        print_error "show_service_status: compose file not found: $compose_file"
        return 1
    fi
    
    print_header "📊 Service Status" "="
    
    # Get detailed status
    if ! docker compose -f "$compose_file" ps; then
        print_error "Failed to get service status"
        return 1
    fi
    
    # Summary information
    local running_count
    running_count=$(docker compose -f "$compose_file" ps --quiet --status running 2>/dev/null | wc -l || echo 0)
    local total_count
    total_count=$(docker compose -f "$compose_file" ps --quiet 2>/dev/null | wc -l || echo 0)
    
    echo ""
    print_info "Summary: $running_count/$total_count services running"
    return 0
}

# Usage: display_service_endpoints endpoint_array service_type icon
# Display formatted endpoint information
# Example: 
#   declare -A endpoints=([master1]="http://localhost:8080" [master2]="http://localhost:8081")
#   display_service_endpoints endpoints "Masters" "🌐"
# Note: Uses nameref, requires bash 4.3+
display_service_endpoints() {
    [[ $# -lt 3 ]] && { print_error "display_service_endpoints: requires 3 arguments"; return 1; }
    
    # Check bash version for nameref support
    if [[ ${BASH_VERSION%%.*} -lt 4 ]] || { [[ ${BASH_VERSION%%.*} -eq 4 ]] && [[ ${BASH_VERSION#*.} -lt 3 ]]; }; then
        print_error "display_service_endpoints: requires bash 4.3+ for nameref"
        return 1
    fi
    
    local -n endpoints=$1
    local service_type="$2"
    local icon="$3"
    
    # Check if array is empty
    if [[ ${#endpoints[@]} -eq 0 ]]; then
        echo "$icon $service_type: (none configured)"
        return 0
    fi
    
    echo "$icon $service_type:"
    # Sort and display endpoints
    for key in $(printf '%s\n' "${!endpoints[@]}" | sort); do
        printf "   %-20s %s\n" "${service_type%s} $key:" "${endpoints[$key]}"
    done
}

# Function to display command help
display_command_help() {
    local compose_file="$1"
    local additional_commands=("${@:2}")
    
    print_header "🔍 Useful Commands" "-"
    echo "View logs:       docker compose -f $compose_file logs -f [service]"
    echo "Stop services:   docker compose -f $compose_file down"
    echo "Clean up all:    docker compose -f $compose_file down -v"
    echo "Service status:  docker compose -f $compose_file ps"
    
    if [ ${#additional_commands[@]} -gt 0 ]; then
        echo ""
        for cmd in "${additional_commands[@]}"; do
            echo "$cmd"
        done
    fi
}

# Function to validate compose file
validate_compose_file() {
    local compose_file="$1"
    
    if [ ! -f "$compose_file" ]; then
        print_error "❌ Error: Compose file '$compose_file' not found"
        return 1
    fi
    
    if ! docker compose -f "$compose_file" config --quiet 2>/dev/null; then
        print_error "❌ Error: Invalid compose file '$compose_file'"
        return 1
    fi
    
    return 0
}

# Function to get container logs safely
get_container_logs() {
    local compose_file="$1"
    local service="$2"
    local lines="${3:-50}"
    
    docker compose -f "$compose_file" logs --tail="$lines" "$service" 2>/dev/null || true
}

# Function to execute command in container
exec_in_container() {
    local compose_file="$1"
    local service="$2"
    local command="${3:-/bin/sh}"
    
    docker compose -f "$compose_file" exec "$service" $command
}

# Function to check if all services are running
all_services_running() {
    local compose_file="$1"
    
    local total_services=$(docker compose -f "$compose_file" ps --quiet | wc -l)
    local running_services=$(docker compose -f "$compose_file" ps --quiet --status running | wc -l)
    
    [ "$total_services" -eq "$running_services" ] && [ "$total_services" -gt 0 ]
}

# Export all functions for use in scripts that source this library
export -f print_info print_success print_warning print_error print_debug
export -f print_header show_spinner
export -f check_docker_prerequisites create_mount_directories
export -f cleanup_containers start_docker_services wait_for_services_animated
export -f check_service_health show_service_status display_service_endpoints
export -f display_command_help validate_compose_file
export -f get_container_logs exec_in_container all_services_running