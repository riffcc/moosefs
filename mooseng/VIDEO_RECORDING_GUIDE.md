# MooseNG Demo Video Recording Guide

## Prerequisites for Recording

### Recording Software Options:
1. **OBS Studio** (recommended - free and cross-platform)
2. **SimpleScreenRecorder** (Linux)
3. **QuickTime** (macOS)
4. **Windows Game Bar** (Windows)

### Pre-Recording Checklist:
- [ ] Stop the demo if running: `./stop-demo.sh`
- [ ] Clear terminal history for clean output
- [ ] Open browser tabs for Prometheus and Grafana
- [ ] Have the VIDEO_DEMO_SCRIPT.md open for reference
- [ ] Set terminal font size to be clearly readable (14-16pt)
- [ ] Close unnecessary applications to reduce system load

## Recording Instructions

### Terminal Setup:
```bash
# Set terminal size to 120x30 for good visibility
# Clear the screen
clear

# Navigate to the MooseNG directory
cd /home/wings/projects/moosefs/mooseng
```

### Recording Steps:

1. **Start Recording**
   - Record at 1920x1080 resolution minimum
   - Include system audio if demonstrating alerts

2. **Follow the Script**
   - Use VIDEO_DEMO_SCRIPT.md as your guide
   - Pause briefly after each command to show output
   - Speak clearly and at a moderate pace

3. **Browser Demonstrations**
   - Use incognito/private mode for clean UI
   - Zoom to 100% for consistency
   - Pre-login to Grafana to save time

4. **Common Issues to Avoid**
   - Don't rush through outputs
   - Ensure mouse cursor is visible
   - Avoid clicking too fast between screens

## Post-Recording

### Video Editing (Optional):
- Trim dead space at beginning/end
- Add title card with "MooseNG Docker Demo"
- Consider adding subtitles for commands
- Export as MP4, H.264 codec, 1080p

### File Naming:
`mooseng-docker-demo-[YYYY-MM-DD].mp4`

## Quick Commands Reference

```bash
# Start demo
./start-demo.sh

# Test health
./test-demo.sh

# Check services
docker compose ps

# View logs (if needed)
docker compose logs -f [service-name]

# Stop demo
./stop-demo.sh
```

## URLs to Open:
- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (admin/admin)
- Targets: http://localhost:9090/targets
- Dashboards: http://localhost:3000/dashboards