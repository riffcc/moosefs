# MooseNG Docker Demo Makefile
# Provides convenient commands for managing the demo environment

.PHONY: help start stop restart test clean logs status build shell backup restore

# Default compose file
COMPOSE_FILE ?= docker-compose.yml
ENV_FILE ?= .env

# Colors for output
GREEN := \033[0;32m
YELLOW := \033[1;33m
RED := \033[0;31m
NC := \033[0m # No Color

# Default target
help:
	@echo "$(GREEN)MooseNG Docker Demo - Available Commands:$(NC)"
	@echo "  make start         - Start the demo environment"
	@echo "  make stop          - Stop the demo environment"
	@echo "  make restart       - Restart all services"
	@echo "  make test          - Run tests on the environment"
	@echo "  make clean         - Stop and remove all data"
	@echo "  make logs          - Follow logs for all services"
	@echo "  make status        - Show service status"
	@echo "  make build         - Build/rebuild all images"
	@echo "  make shell SVC=x   - Open shell in service container"
	@echo "  make backup        - Backup all volumes"
	@echo "  make restore       - Restore volumes from backup"
	@echo ""
	@echo "$(YELLOW)Options:$(NC)"
	@echo "  COMPOSE_FILE=x     - Use alternative compose file"
	@echo "  ENV_FILE=x         - Use alternative env file"
	@echo "  SVC=service-name   - Target specific service"
	@echo ""
	@echo "$(GREEN)Examples:$(NC)"
	@echo "  make start COMPOSE_FILE=docker-compose.improved.yml"
	@echo "  make logs SVC=master-1"
	@echo "  make shell SVC=chunkserver-1"

# Start the demo environment
start:
	@echo "$(GREEN)Starting MooseNG Demo...$(NC)"
	@if [ -f start-demo-improved.sh ]; then \
		./start-demo-improved.sh -f $(COMPOSE_FILE) -e $(ENV_FILE); \
	else \
		./start-demo.sh; \
	fi

# Stop the demo environment
stop:
	@echo "$(YELLOW)Stopping MooseNG Demo...$(NC)"
	@if [ -f stop-demo-improved.sh ]; then \
		./stop-demo-improved.sh; \
	else \
		./stop-demo.sh; \
	fi

# Restart all services
restart: stop start

# Run tests
test:
	@echo "$(GREEN)Running tests...$(NC)"
	@if [ -f test-demo-improved.sh ]; then \
		./test-demo-improved.sh; \
	else \
		./test-demo.sh; \
	fi

# Clean everything (including volumes)
clean:
	@echo "$(RED)Cleaning up everything...$(NC)"
	@echo "This will remove all data. Press Ctrl+C to cancel, or Enter to continue."
	@read confirm
	@docker compose -f $(COMPOSE_FILE) down -v --remove-orphans
	@rm -rf mnt/client-*
	@echo "$(GREEN)Cleanup complete!$(NC)"

# Show logs
logs:
ifdef SVC
	@docker compose -f $(COMPOSE_FILE) logs -f $(SVC)
else
	@docker compose -f $(COMPOSE_FILE) logs -f
endif

# Show status
status:
	@echo "$(GREEN)Service Status:$(NC)"
	@docker compose -f $(COMPOSE_FILE) ps
	@echo ""
	@echo "$(GREEN)Container Health:$(NC)"
	@docker ps --filter "label=com.docker.compose.project=mooseng" \
		--format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# Build images
build:
	@echo "$(GREEN)Building images...$(NC)"
	@docker compose -f $(COMPOSE_FILE) build --parallel

# Open shell in container
shell:
ifndef SVC
	@echo "$(RED)Error: Please specify service with SVC=service-name$(NC)"
	@echo "Example: make shell SVC=master-1"
	@exit 1
else
	@echo "$(GREEN)Opening shell in mooseng-$(SVC)...$(NC)"
	@docker exec -it mooseng-$(SVC) /bin/sh
endif

# Backup volumes
backup:
	@echo "$(GREEN)Backing up volumes...$(NC)"
	@mkdir -p backups
	@./scripts/backup-volumes.sh || echo "Backup script not found, using manual backup..."
	@BACKUP_DIR="backups/$$(date +%Y%m%d_%H%M%S)"; \
	mkdir -p $$BACKUP_DIR; \
	echo "Backing up to $$BACKUP_DIR..."; \
	docker compose -f $(COMPOSE_FILE) ps -q | while read container; do \
		docker cp $$container:/data $$BACKUP_DIR/ 2>/dev/null || true; \
	done; \
	echo "$(GREEN)Backup completed in $$BACKUP_DIR$(NC)"

# Restore volumes
restore:
	@echo "$(YELLOW)Available backups:$(NC)"
	@ls -la backups/ 2>/dev/null || echo "No backups found"
	@echo ""
	@echo "Enter backup directory name to restore (e.g., 20240101_120000):"
	@read BACKUP_NAME; \
	if [ -d "backups/$$BACKUP_NAME" ]; then \
		echo "$(GREEN)Restoring from backups/$$BACKUP_NAME...$(NC)"; \
		./scripts/restore-volumes.sh "backups/$$BACKUP_NAME" || \
		echo "$(RED)Restore failed or script not found$(NC)"; \
	else \
		echo "$(RED)Backup directory not found$(NC)"; \
	fi

# Quick health check
health:
	@echo "$(GREEN)Checking health endpoints...$(NC)"
	@for port in 9421 9431 9441; do \
		echo -n "Master on port $$port: "; \
		curl -s -f "http://localhost:$$port/health" > /dev/null 2>&1 && \
		echo "$(GREEN)✓ Healthy$(NC)" || echo "$(RED)✗ Unhealthy$(NC)"; \
	done
	@for port in 9420 9450 9460; do \
		echo -n "Chunkserver on port $$port: "; \
		curl -s -f "http://localhost:$$port/health" > /dev/null 2>&1 && \
		echo "$(GREEN)✓ Healthy$(NC)" || echo "$(RED)✗ Unhealthy$(NC)"; \
	done

# Show environment
env:
	@echo "$(GREEN)Current Environment:$(NC)"
	@echo "COMPOSE_FILE: $(COMPOSE_FILE)"
	@echo "ENV_FILE: $(ENV_FILE)"
	@if [ -f $(ENV_FILE) ]; then \
		echo ""; \
		echo "$(YELLOW)Environment Variables:$(NC)"; \
		cat $(ENV_FILE) | grep -v "^#" | grep -v "^$$"; \
	else \
		echo "$(YELLOW)No environment file found at $(ENV_FILE)$(NC)"; \
	fi

# Watch service status (requires watch command)
watch:
	@watch -n 2 'docker compose -f $(COMPOSE_FILE) ps && echo "" && \
		docker ps --filter "label=com.docker.compose.project=mooseng" \
		--format "table {{.Names}}\t{{.Status}}"'

# Pull latest images
pull:
	@echo "$(GREEN)Pulling latest images...$(NC)"
	@docker compose -f $(COMPOSE_FILE) pull

# Validate compose file
validate:
	@echo "$(GREEN)Validating compose file...$(NC)"
	@docker compose -f $(COMPOSE_FILE) config > /dev/null && \
		echo "$(GREEN)✓ Compose file is valid$(NC)" || \
		echo "$(RED)✗ Compose file has errors$(NC)"

# Show compose configuration
config:
	@docker compose -f $(COMPOSE_FILE) config

# Remove unused resources
prune:
	@echo "$(YELLOW)Removing unused Docker resources...$(NC)"
	@docker system prune -f
	@echo "$(GREEN)Cleanup complete!$(NC)"

# Install dependencies
install-deps:
	@echo "$(GREEN)Checking dependencies...$(NC)"
	@command -v docker >/dev/null 2>&1 || \
		{ echo "$(RED)Docker is not installed$(NC)"; exit 1; }
	@docker compose version >/dev/null 2>&1 || \
		{ echo "$(RED)Docker Compose v2 is not installed$(NC)"; exit 1; }
	@echo "$(GREEN)All dependencies are installed!$(NC)"

# Quick demo
demo: start
	@sleep 10
	@make health
	@echo ""
	@echo "$(GREEN)Demo is ready! Access points:$(NC)"
	@echo "  - Grafana: http://localhost:3000 (admin/admin)"
	@echo "  - Prometheus: http://localhost:9090"
	@echo "  - Dashboard: http://localhost:8080"