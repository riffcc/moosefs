#!/bin/bash

# Update system
yum update -y

# Install Docker
amazon-linux-extras install docker -y
systemctl start docker
systemctl enable docker
usermod -a -G docker ec2-user

# Install Docker Compose
curl -L "https://github.com/docker/compose/releases/download/v2.20.0/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
chmod +x /usr/local/bin/docker-compose
ln -s /usr/local/bin/docker-compose /usr/bin/docker-compose

# Install Rust for building MooseNG
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Install git and other tools
yum install -y git htop iotop tcpdump iproute-tc

# Create directories
mkdir -p /opt/mooseng/{data,logs,config}
mkdir -p /var/log/mooseng

# Clone MooseNG repository
cd /opt
git clone https://github.com/mooseng/mooseng.git || echo "Repository clone failed, using pre-built image"

# Create configuration file
cat > /opt/mooseng/config/master.toml << EOF
[server]
bind_addr = "0.0.0.0:9421"
region = "${region}"
node_id = ${node_id}

[raft]
bind_addr = "0.0.0.0:9422"
data_dir = "/opt/mooseng/data/raft"

[metrics]
bind_addr = "0.0.0.0:9423"
enabled = true

[cluster]
cluster_id = "${cluster_id}"
# Master addresses will be updated by the application

[storage]
metadata_dir = "/opt/mooseng/data/metadata"

[logging]
level = "info"
file = "/var/log/mooseng/master.log"
EOF

# Create systemd service for MooseNG master
cat > /etc/systemd/system/mooseng-master.service << EOF
[Unit]
Description=MooseNG Master Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=ec2-user
WorkingDirectory=/opt/mooseng
ExecStart=/home/ec2-user/.cargo/bin/cargo run --release --bin mooseng-master -- --config /opt/mooseng/config/master.toml
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=mooseng-master

[Install]
WantedBy=multi-user.target
EOF

# Set up network performance monitoring
cat > /usr/local/bin/network-monitor.sh << 'EOF'
#!/bin/bash
LOG_FILE="/var/log/mooseng/network-stats.log"
while true; do
    echo "$(date '+%Y-%m-%d %H:%M:%S')" >> $LOG_FILE
    ss -tuln >> $LOG_FILE
    netstat -i >> $LOG_FILE
    echo "---" >> $LOG_FILE
    sleep 60
done
EOF
chmod +x /usr/local/bin/network-monitor.sh

# Create network monitoring service
cat > /etc/systemd/system/network-monitor.service << EOF
[Unit]
Description=Network Performance Monitor
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/network-monitor.sh
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

# Set up log rotation
cat > /etc/logrotate.d/mooseng << EOF
/var/log/mooseng/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 644 ec2-user ec2-user
}
EOF

# Configure CloudWatch agent for metrics collection
cat > /opt/aws/amazon-cloudwatch-agent/etc/amazon-cloudwatch-agent.json << EOF
{
    "metrics": {
        "namespace": "MooseNG/${region}",
        "metrics_collected": {
            "cpu": {
                "measurement": [
                    "cpu_usage_idle",
                    "cpu_usage_iowait",
                    "cpu_usage_user",
                    "cpu_usage_system"
                ],
                "metrics_collection_interval": 60
            },
            "disk": {
                "measurement": [
                    "used_percent"
                ],
                "metrics_collection_interval": 60,
                "resources": [
                    "*"
                ]
            },
            "diskio": {
                "measurement": [
                    "io_time"
                ],
                "metrics_collection_interval": 60,
                "resources": [
                    "*"
                ]
            },
            "mem": {
                "measurement": [
                    "mem_used_percent"
                ],
                "metrics_collection_interval": 60
            },
            "netstat": {
                "measurement": [
                    "tcp_established",
                    "tcp_time_wait"
                ],
                "metrics_collection_interval": 60
            }
        }
    },
    "logs": {
        "logs_collected": {
            "files": {
                "collect_list": [
                    {
                        "file_path": "/var/log/mooseng/master.log",
                        "log_group_name": "mooseng-master-${region}",
                        "log_stream_name": "{instance_id}"
                    },
                    {
                        "file_path": "/var/log/mooseng/network-stats.log",
                        "log_group_name": "mooseng-network-${region}",
                        "log_stream_name": "{instance_id}"
                    }
                ]
            }
        }
    }
}
EOF

# Enable and start services
systemctl daemon-reload
systemctl enable mooseng-master
systemctl enable network-monitor

# Change ownership
chown -R ec2-user:ec2-user /opt/mooseng /var/log/mooseng

# Signal that the instance is ready
/opt/aws/bin/cfn-signal -e $? --stack ${AWS::StackName} --resource MasterInstance --region ${AWS::Region} || true

echo "Master server setup completed for region: ${region}, node_id: ${node_id}"