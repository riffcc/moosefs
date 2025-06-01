#!/bin/bash
# Enhanced MooseNG Demo Script for Professional Video Recording
# This script provides a smooth, visually appealing demo with optimal pacing

set -euo pipefail

# Script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Enhanced color scheme
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
DIM='\033[2m'
NORMAL='\033[22m'
NC='\033[0m' # No Color

# Configurable timing (can be overridden via environment)
PAUSE_SHORT=${PAUSE_SHORT:-2}
PAUSE_MEDIUM=${PAUSE_MEDIUM:-4}
PAUSE_LONG=${PAUSE_LONG:-6}
PAUSE_EXTRA_LONG=${PAUSE_EXTRA_LONG:-10}
INTERACTIVE_MODE=${INTERACTIVE_MODE:-false}
DRY_RUN=${DRY_RUN:-false}
TYPING_SPEED=${TYPING_SPEED:-0.05}

# Section headers with visual separation
section_header() {
    local title="$1"
    local width=70
    local title_len=${#title}
    local padding=$(( (width - title_len - 2) / 2 ))
    local right_padding=$(( width - title_len - 2 - padding ))
    
    echo
    echo -e "${CYAN}╔$(printf '═%.0s' $(seq 1 $width))╗${NC}"
    echo -e "${CYAN}║$(printf ' %.0s' $(seq 1 $padding))${BOLD}${GREEN}$title${NORMAL}${CYAN}$(printf ' %.0s' $(seq 1 $right_padding))║${NC}"
    echo -e "${CYAN}╚$(printf '═%.0s' $(seq 1 $width))╝${NC}"
    echo
    sleep $PAUSE_SHORT
}

# Enhanced demo echo with configurable pause
demo_echo() {
    local message="$1"
    local pause_time="${2:-$PAUSE_MEDIUM}"
    echo -e "${GREEN}$message${NC}"
    interactive_pause $pause_time
}

# Typing effect for commands
type_command() {
    local cmd="$1"
    echo -ne "${BLUE}$ ${BOLD}"
    if [[ "$DRY_RUN" == "true" ]]; then
        echo -e "$cmd${NORMAL}${NC}"
    else
        for (( i=0; i<${#cmd}; i++ )); do
            echo -ne "${cmd:$i:1}"
            sleep $TYPING_SPEED
        done
        echo -e "${NORMAL}${NC}"
    fi
    sleep 1
}

# Enhanced command runner with output handling
demo_run() {
    local cmd="$1"
    local pause_after="${2:-$PAUSE_MEDIUM}"
    
    type_command "$cmd"
    
    if [[ "$DRY_RUN" != "true" ]]; then
        # Execute command with error handling
        if ! eval "$cmd"; then
            echo -e "${RED}Command failed! Press ENTER to continue anyway...${NC}"
            read -r
        fi
    fi
    
    interactive_pause $pause_after
}

# Progress indicator for long operations
show_progress() {
    local duration=$1
    local message=$2
    echo -ne "${YELLOW}$message "
    
    if [[ "$DRY_RUN" == "true" ]]; then
        echo -e "${GREEN}✓${NC} ${DIM}(dry-run: skipped)${NC}"
        return
    fi
    
    for i in $(seq 1 $duration); do
        echo -ne "."
        sleep 1
    done
    echo -e " ${GREEN}✓${NC}"
}

# Interactive pause with option to continue
interactive_pause() {
    local duration="${1:-$PAUSE_MEDIUM}"
    
    if [[ "$INTERACTIVE_MODE" == "true" ]]; then
        echo -e "${DIM}Press ENTER to continue...${NC}"
        read -r
    else
        sleep "$duration"
    fi
}

# Countdown before starting
countdown() {
    echo -e "${YELLOW}${BOLD}Starting demo in...${NORMAL}${NC}"
    for i in 3 2 1; do
        echo -e "${BOLD}${CYAN}    $i${NORMAL}"
        sleep 1
    done
    echo -e "${GREEN}${BOLD}    GO!${NORMAL}${NC}"
    sleep 0.5
    clear
}

# Check prerequisites
check_prerequisites() {
    local errors=0
    
    echo -e "${YELLOW}Checking prerequisites...${NC}"
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}✗ Docker is not installed${NC}"
        ((errors++))
    else
        echo -e "${GREEN}✓ Docker found${NC}"
    fi
    
    # Check Docker Compose
    if ! docker compose version &> /dev/null 2>&1; then
        echo -e "${RED}✗ Docker Compose is not available${NC}"
        ((errors++))
    else
        echo -e "${GREEN}✓ Docker Compose found${NC}"
    fi
    
    # Check required scripts
    for script in start-demo.sh stop-demo.sh test-demo.sh; do
        if [[ ! -f "./$script" ]]; then
            echo -e "${RED}✗ Missing required script: $script${NC}"
            ((errors++))
        else
            echo -e "${GREEN}✓ Found $script${NC}"
        fi
    done
    
    # Check ports availability
    echo -e "${YELLOW}Checking port availability...${NC}"
    local ports=(9090 3000 9420 9421 9430 9431 9440 9441 9450 9460)
    for port in "${ports[@]}"; do
        if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
            echo -e "${YELLOW}⚠ Port $port is already in use${NC}"
        fi
    done
    
    if [[ $errors -gt 0 ]]; then
        echo -e "${RED}Prerequisites check failed with $errors errors${NC}"
        echo -e "${YELLOW}Continue anyway? (y/N)${NC}"
        read -r response
        if [[ ! "$response" =~ ^[Yy]$ ]]; then
            exit 1
        fi
    else
        echo -e "${GREEN}All prerequisites satisfied!${NC}"
    fi
    
    sleep $PAUSE_SHORT
}

# Display service endpoints with nice formatting
display_endpoints() {
    echo
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│                    ${BOLD}Service Endpoints${NORMAL}${CYAN}                            │${NC}"
    echo -e "${CYAN}├─────────────────────────────────────────────────────────────────┤${NC}"
    echo -e "${CYAN}│ ${BOLD}${GREEN}Masters:${NORMAL}${CYAN}                                                        │${NC}"
    echo -e "${CYAN}│   • Master 1: ${BLUE}http://localhost:9421${CYAN}                            │${NC}"
    echo -e "${CYAN}│   • Master 2: ${BLUE}http://localhost:9431${CYAN}                            │${NC}"
    echo -e "${CYAN}│   • Master 3: ${BLUE}http://localhost:9441${CYAN}                            │${NC}"
    echo -e "${CYAN}├─────────────────────────────────────────────────────────────────┤${NC}"
    echo -e "${CYAN}│ ${BOLD}${GREEN}Chunkservers:${NORMAL}${CYAN}                                                   │${NC}"
    echo -e "${CYAN}│   • Chunkserver 1: ${BLUE}http://localhost:9420${CYAN}                       │${NC}"
    echo -e "${CYAN}│   • Chunkserver 2: ${BLUE}http://localhost:9450${CYAN}                       │${NC}"
    echo -e "${CYAN}│   • Chunkserver 3: ${BLUE}http://localhost:9460${CYAN}                       │${NC}"
    echo -e "${CYAN}├─────────────────────────────────────────────────────────────────┤${NC}"
    echo -e "${CYAN}│ ${BOLD}${GREEN}Monitoring:${NORMAL}${CYAN}                                                     │${NC}"
    echo -e "${CYAN}│   • Prometheus: ${BLUE}http://localhost:9090${CYAN}                          │${NC}"
    echo -e "${CYAN}│   • Grafana: ${BLUE}http://localhost:3000${CYAN} ${DIM}(admin/admin)${NC}${CYAN}              │${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────┘${NC}"
    echo
}

# Browser instruction box
browser_instruction() {
    local url="$1"
    local description="$2"
    local extra_info="${3:-}"
    
    echo
    echo -e "${MAGENTA}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${MAGENTA}║                    ${BOLD}Open in Browser${NORMAL}${MAGENTA}                            ║${NC}"
    echo -e "${MAGENTA}╟───────────────────────────────────────────────────────────────╢${NC}"
    echo -e "${MAGENTA}║ ${YELLOW}${BOLD}$description${NORMAL}${MAGENTA}$(printf ' %.0s' $(seq 1 $((45-${#description}))))║${NC}"
    echo -e "${MAGENTA}║ ${BLUE}$url${MAGENTA}$(printf ' %.0s' $(seq 1 $((59-${#url}))))║${NC}"
    if [[ -n "$extra_info" ]]; then
        echo -e "${MAGENTA}║ ${DIM}$extra_info${NC}${MAGENTA}$(printf ' %.0s' $(seq 1 $((59-${#extra_info}))))║${NC}"
    fi
    echo -e "${MAGENTA}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo
}

# Main demo flow
main() {
    # Check for help
    if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
        cat << EOF
Enhanced MooseNG Demo Script

Usage: $0 [OPTIONS]

Options:
  --interactive, -i     Enable interactive mode (pause for ENTER)
  --dry-run, -d        Dry run mode (show commands without executing)
  --fast               Use shorter pauses
  --slow               Use longer pauses
  --help, -h           Show this help

Environment Variables:
  PAUSE_SHORT          Short pause duration (default: 2)
  PAUSE_MEDIUM         Medium pause duration (default: 4)
  PAUSE_LONG           Long pause duration (default: 6)
  PAUSE_EXTRA_LONG     Extra long pause duration (default: 10)
  TYPING_SPEED         Typing effect speed (default: 0.05)

EOF
        exit 0
    fi
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --interactive|-i)
                INTERACTIVE_MODE=true
                shift
                ;;
            --dry-run|-d)
                DRY_RUN=true
                shift
                ;;
            --fast)
                PAUSE_SHORT=1
                PAUSE_MEDIUM=2
                PAUSE_LONG=3
                PAUSE_EXTRA_LONG=5
                TYPING_SPEED=0.02
                shift
                ;;
            --slow)
                PAUSE_SHORT=3
                PAUSE_MEDIUM=6
                PAUSE_LONG=9
                PAUSE_EXTRA_LONG=15
                TYPING_SPEED=0.08
                shift
                ;;
            *)
                echo "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    # Start demo
    clear
    
    # Pre-flight checks
    check_prerequisites
    
    # Countdown
    countdown
    
    # Introduction
    section_header "MooseNG Docker Demo"
    demo_echo "🎬 Welcome to the MooseNG distributed storage demo!" $PAUSE_LONG
    demo_echo "📦 This demo will show you how to quickly spin up a complete MooseNG cluster"
    demo_echo "⏱️  Total duration: approximately 2-3 minutes"
    
    # Directory structure
    section_header "Demo Directory Structure"
    demo_echo "📁 First, let's look at our demo directory structure..."
    demo_run "ls -la | head -15" $PAUSE_LONG
    
    # Docker Compose configuration
    section_header "Docker Compose Configuration"
    demo_echo "📋 Let's examine our Docker Compose configuration..."
    demo_echo "🔍 This defines our entire cluster architecture:"
    demo_run "head -25 docker-compose.yml" $PAUSE_LONG
    
    # Architecture explanation
    demo_echo "🏗️  Our demo architecture includes:"
    echo -e "    ${CYAN}•${NC} ${BOLD}3 Master servers${NORMAL} with Raft consensus"
    echo -e "    ${CYAN}•${NC} ${BOLD}3 Chunkservers${NORMAL} for distributed storage"
    echo -e "    ${CYAN}•${NC} ${BOLD}3 Client instances${NORMAL} for access"
    echo -e "    ${CYAN}•${NC} ${BOLD}Full monitoring stack${NORMAL} with Prometheus & Grafana"
    interactive_pause $PAUSE_LONG
    
    # Start the cluster
    section_header "Starting the MooseNG Cluster"
    demo_echo "🚀 Now, let's start the entire MooseNG cluster..."
    demo_echo "⚡ This single command will handle everything:"
    demo_run "./start-demo.sh" $PAUSE_EXTRA_LONG
    
    # Wait for initialization
    demo_echo "✅ Great! All services are starting up."
    show_progress 15 "Waiting for services to initialize"
    
    # Verify health
    section_header "Service Health Verification"
    demo_echo "🧪 Now let's verify that all services are healthy..."
    demo_run "./test-demo.sh" $PAUSE_LONG
    
    # Check containers
    demo_echo "📊 Let's check the Docker containers status..."
    demo_run "docker compose ps" $PAUSE_LONG
    
    # Display endpoints
    section_header "Service Endpoints"
    demo_echo "🌐 Here are all the available service endpoints:"
    display_endpoints
    interactive_pause $PAUSE_LONG
    
    # Prometheus section
    section_header "Monitoring with Prometheus"
    demo_echo "🔍 Let's explore the Prometheus monitoring interface..."
    browser_instruction "http://localhost:9090/targets" "Prometheus Targets" "View all monitored endpoints"
    interactive_pause $PAUSE_EXTRA_LONG
    
    # Grafana section
    section_header "Dashboards with Grafana"
    demo_echo "📊 Now let's explore the Grafana dashboards..."
    browser_instruction "http://localhost:3000" "Grafana Login" "Use credentials: admin/admin"
    interactive_pause $PAUSE_EXTRA_LONG
    
    demo_echo "📈 In Grafana, you can view comprehensive metrics:"
    echo -e "    ${CYAN}•${NC} ${BOLD}Cluster health status${NORMAL} - Overall system health"
    echo -e "    ${CYAN}•${NC} ${BOLD}Storage capacity and usage${NORMAL} - Real-time storage metrics"
    echo -e "    ${CYAN}•${NC} ${BOLD}Request rates and latency${NORMAL} - Performance indicators"
    echo -e "    ${CYAN}•${NC} ${BOLD}Node performance metrics${NORMAL} - Individual node health"
    interactive_pause $PAUSE_LONG
    
    # Cleanup
    section_header "Demo Cleanup"
    demo_echo "🛑 Finally, let's stop the demo and clean up..."
    demo_run "./stop-demo.sh" $PAUSE_LONG
    
    # Conclusion
    section_header "Demo Complete!"
    demo_echo "✅ ${BOLD}That's it!${NORMAL} In just a few minutes, we've:"
    echo
    echo -e "    ${GREEN}✓${NC} Started a complete MooseNG cluster"
    echo -e "    ${GREEN}✓${NC} Verified all services are healthy"  
    echo -e "    ${GREEN}✓${NC} Explored the monitoring dashboards"
    echo -e "    ${GREEN}✓${NC} Cleanly stopped everything"
    echo
    demo_echo "📚 For more information, check out our README and documentation."
    demo_echo "🙏 Thank you for watching the MooseNG Docker demo!"
    echo
    
    # Final statistics
    if [[ "$DRY_RUN" != "true" ]]; then
        echo -e "${DIM}Demo completed at: $(date)${NC}"
    fi
}

# Run main function
main "$@"