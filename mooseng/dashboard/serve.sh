#!/bin/bash

# Simple development server for MooseNG Dashboard
# Serves the dashboard on localhost:8080

PORT=${1:-8080}
DIRECTORY="public"

echo "Starting MooseNG Dashboard server..."
echo "URL: http://localhost:$PORT"
echo "Directory: $DIRECTORY"
echo ""
echo "Press Ctrl+C to stop the server"
echo ""

if command -v python3 &> /dev/null; then
    python3 -m http.server $PORT --directory $DIRECTORY
elif command -v python &> /dev/null; then
    cd $DIRECTORY && python -m SimpleHTTPServer $PORT
elif command -v node &> /dev/null && npm list -g http-server &> /dev/null; then
    npx http-server $DIRECTORY -p $PORT
else
    echo "Error: No suitable server found."
    echo "Please install Python 3 or Node.js with http-server"
    exit 1
fi