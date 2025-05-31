#!/bin/bash

# Kubernetes deployment validation script for MooseNG
set -e

echo "🔍 Validating MooseNG Kubernetes deployment manifests..."

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    echo "❌ kubectl is not installed or not in PATH"
    exit 1
fi

# Function to validate YAML syntax
validate_yaml() {
    local file=$1
    echo "📋 Validating YAML syntax: $file"
    
    if kubectl apply --dry-run=client -f "$file" > /dev/null 2>&1; then
        echo "✅ $file syntax is valid"
    else
        echo "❌ $file has syntax errors"
        kubectl apply --dry-run=client -f "$file"
        return 1
    fi
}

# Function to validate Helm chart
validate_helm() {
    echo "📋 Validating Helm chart..."
    
    if command -v helm &> /dev/null; then
        cd ../helm/mooseng
        if helm lint .; then
            echo "✅ Helm chart is valid"
        else
            echo "❌ Helm chart has errors"
            return 1
        fi
        cd ../../k8s
    else
        echo "⚠️  helm is not installed, skipping Helm validation"
    fi
}

# Main validation
echo "🚀 Starting validation..."

# Validate individual K8s manifests
for file in *.yaml; do
    if [[ -f "$file" ]]; then
        validate_yaml "$file"
    fi
done

# Validate Helm chart
validate_helm

echo ""
echo "🎉 All validations passed!"
echo ""
echo "📋 Quick deployment guide:"
echo "1. Create namespace: kubectl apply -f namespace.yaml"
echo "2. Deploy masters: kubectl apply -f master-statefulset.yaml"
echo "3. Deploy chunk servers: kubectl apply -f chunkserver-daemonset.yaml"
echo ""
echo "Or use Helm:"
echo "helm install mooseng ../helm/mooseng/ --namespace mooseng --create-namespace"