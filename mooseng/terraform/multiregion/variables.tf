variable "project_name" {
  description = "Name of the project"
  type        = string
  default     = "mooseng-multiregion"
}

variable "regions" {
  description = "AWS regions for deployment"
  type = object({
    us_east  = string
    eu_west  = string
    ap_south = string
  })
  default = {
    us_east  = "us-east-1"
    eu_west  = "eu-west-1"
    ap_south = "ap-south-1"
  }
}

variable "vpc_cidrs" {
  description = "CIDR blocks for VPCs in each region"
  type = object({
    us_east  = string
    eu_west  = string
    ap_south = string
  })
  default = {
    us_east  = "10.1.0.0/16"
    eu_west  = "10.2.0.0/16"
    ap_south = "10.3.0.0/16"
  }
}

variable "instance_types" {
  description = "EC2 instance types for different components"
  type = object({
    master      = string
    chunkserver = string
    benchmark   = string
  })
  default = {
    master      = "t3.medium"
    chunkserver = "t3.large"
    benchmark   = "t3.xlarge"
  }
}

variable "key_name" {
  description = "AWS EC2 Key Pair name for SSH access"
  type        = string
}

variable "ssh_cidr" {
  description = "CIDR block allowed for SSH access"
  type        = string
  default     = "0.0.0.0/0"
}

variable "storage_volume_size" {
  description = "Size of EBS volume for chunkserver storage (GB)"
  type        = number
  default     = 100
}

variable "common_tags" {
  description = "Common tags to apply to all resources"
  type        = map(string)
  default = {
    Project     = "MooseNG"
    Environment = "benchmark"
    ManagedBy   = "terraform"
  }
}

variable "enable_detailed_monitoring" {
  description = "Enable detailed CloudWatch monitoring"
  type        = bool
  default     = true
}

variable "enable_flow_logs" {
  description = "Enable VPC Flow Logs for network analysis"
  type        = bool
  default     = true
}

variable "benchmark_schedule" {
  description = "Cron schedule for automated benchmark runs"
  type        = string
  default     = "0 2 * * *"  # Daily at 2 AM UTC
}

variable "retention_days" {
  description = "Number of days to retain benchmark results"
  type        = number
  default     = 30
}