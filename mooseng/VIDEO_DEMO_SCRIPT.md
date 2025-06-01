# MooseNG Docker Demo Video Script
## Duration: 2-3 minutes

### Scene 1: Introduction (15 seconds)
- Open terminal in the MooseNG directory
- Brief intro: "Welcome to the MooseNG distributed storage demo. In this 2-minute demonstration, I'll show you how to quickly spin up a complete MooseNG cluster using Docker Compose."

### Scene 2: Architecture Overview (20 seconds) 
- Show docker-compose.yml briefly
- Explain: "Our demo includes:
  - 3 Master servers with Raft consensus
  - 3 Chunkservers for distributed storage
  - 3 Client instances
  - Full monitoring stack with Prometheus and Grafana"

### Scene 3: Starting the Demo (30 seconds)
- Run: `./start-demo.sh`
- Show the output as services start
- Highlight: "The script automatically:
  - Creates necessary directories
  - Builds Docker images in parallel
  - Starts all services
  - Waits for health checks"

### Scene 4: Verifying Service Health (30 seconds)
- Run: `./test-demo.sh`
- Show service status output
- Run: `docker compose ps`
- Explain: "All 12 services are running successfully"

### Scene 5: Exploring the Monitoring Stack (45 seconds)
- Open browser to Prometheus (http://localhost:9090)
  - Show targets page briefly
  - Show some basic queries
  
- Open Grafana (http://localhost:3000)
  - Login with admin/admin
  - Navigate to Dashboards
  - Show MooseNG Performance Dashboard
  - Highlight key metrics:
    - Cluster health status
    - Storage capacity
    - Request rates
    - Latency metrics

### Scene 6: Service Endpoints (20 seconds)
- Show the list of available endpoints:
  - Masters on ports 9421, 9431, 9441
  - Chunkservers on ports 9420, 9450, 9460
  - Client metrics on ports 9427, 9437, 9447
  - Monitoring on ports 9090 (Prometheus) and 3000 (Grafana)

### Scene 7: Cleanup (15 seconds)
- Run: `./stop-demo.sh`
- Show how it cleanly stops all services
- Mention: "Use `docker compose down -v` to also remove data volumes"

### Scene 8: Conclusion (15 seconds)
- "That's it! In just 2 minutes, we've:
  - Started a complete MooseNG cluster
  - Verified all services are healthy
  - Explored the monitoring dashboards
  - And cleanly stopped everything
  
  Check out the README for more details and advanced configurations."

## Key Points to Emphasize:
1. Easy one-command startup
2. Comprehensive monitoring out-of-the-box
3. Production-like architecture with 3-node clusters
4. Clean shutdown and data management