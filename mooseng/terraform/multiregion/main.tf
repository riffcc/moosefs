terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

# Configure AWS providers for multiple regions
provider "aws" {
  alias  = "us_east"
  region = var.regions.us_east
}

provider "aws" {
  alias  = "eu_west"
  region = var.regions.eu_west
}

provider "aws" {
  alias  = "ap_south"
  region = var.regions.ap_south
}

# Data sources for availability zones
data "aws_availability_zones" "us_east" {
  provider = aws.us_east
  state    = "available"
}

data "aws_availability_zones" "eu_west" {
  provider = aws.eu_west
  state    = "available"
}

data "aws_availability_zones" "ap_south" {
  provider = aws.ap_south
  state    = "available"
}

# VPC for US East region
module "vpc_us_east" {
  source = "terraform-aws-modules/vpc/aws"
  providers = {
    aws = aws.us_east
  }

  name = "${var.project_name}-vpc-us-east"
  cidr = var.vpc_cidrs.us_east

  azs             = slice(data.aws_availability_zones.us_east.names, 0, 2)
  public_subnets  = [cidrsubnet(var.vpc_cidrs.us_east, 8, 1), cidrsubnet(var.vpc_cidrs.us_east, 8, 2)]
  private_subnets = [cidrsubnet(var.vpc_cidrs.us_east, 8, 10), cidrsubnet(var.vpc_cidrs.us_east, 8, 11)]

  enable_nat_gateway = true
  enable_vpn_gateway = false
  enable_dns_hostnames = true
  enable_dns_support = true

  tags = merge(var.common_tags, {
    Region = "us-east"
  })
}

# VPC for EU West region
module "vpc_eu_west" {
  source = "terraform-aws-modules/vpc/aws"
  providers = {
    aws = aws.eu_west
  }

  name = "${var.project_name}-vpc-eu-west"
  cidr = var.vpc_cidrs.eu_west

  azs             = slice(data.aws_availability_zones.eu_west.names, 0, 2)
  public_subnets  = [cidrsubnet(var.vpc_cidrs.eu_west, 8, 1), cidrsubnet(var.vpc_cidrs.eu_west, 8, 2)]
  private_subnets = [cidrsubnet(var.vpc_cidrs.eu_west, 8, 10), cidrsubnet(var.vpc_cidrs.eu_west, 8, 11)]

  enable_nat_gateway = true
  enable_vpn_gateway = false
  enable_dns_hostnames = true
  enable_dns_support = true

  tags = merge(var.common_tags, {
    Region = "eu-west"
  })
}

# VPC for AP South region
module "vpc_ap_south" {
  source = "terraform-aws-modules/vpc/aws"
  providers = {
    aws = aws.ap_south
  }

  name = "${var.project_name}-vpc-ap-south"
  cidr = var.vpc_cidrs.ap_south

  azs             = slice(data.aws_availability_zones.ap_south.names, 0, 2)
  public_subnets  = [cidrsubnet(var.vpc_cidrs.ap_south, 8, 1), cidrsubnet(var.vpc_cidrs.ap_south, 8, 2)]
  private_subnets = [cidrsubnet(var.vpc_cidrs.ap_south, 8, 10), cidrsubnet(var.vpc_cidrs.ap_south, 8, 11)]

  enable_nat_gateway = true
  enable_vpn_gateway = false
  enable_dns_hostnames = true
  enable_dns_support = true

  tags = merge(var.common_tags, {
    Region = "ap-south"
  })
}

# Security groups
resource "aws_security_group" "mooseng_master" {
  for_each = {
    us_east  = { provider = "us_east", vpc_id = module.vpc_us_east.vpc_id }
    eu_west  = { provider = "eu_west", vpc_id = module.vpc_eu_west.vpc_id }
    ap_south = { provider = "ap_south", vpc_id = module.vpc_ap_south.vpc_id }
  }

  provider = aws.${each.value.provider}
  name     = "${var.project_name}-master-${each.key}"
  vpc_id   = each.value.vpc_id

  # gRPC port
  ingress {
    from_port   = 9421
    to_port     = 9421
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Raft port
  ingress {
    from_port   = 9422
    to_port     = 9422
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Metrics port
  ingress {
    from_port   = 9423
    to_port     = 9423
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # SSH port
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = [var.ssh_cidr]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(var.common_tags, {
    Name   = "${var.project_name}-master-${each.key}"
    Region = each.key
  })
}

resource "aws_security_group" "mooseng_chunkserver" {
  for_each = {
    us_east  = { provider = "us_east", vpc_id = module.vpc_us_east.vpc_id }
    eu_west  = { provider = "eu_west", vpc_id = module.vpc_eu_west.vpc_id }
    ap_south = { provider = "ap_south", vpc_id = module.vpc_ap_south.vpc_id }
  }

  provider = aws.${each.value.provider}
  name     = "${var.project_name}-chunkserver-${each.key}"
  vpc_id   = each.value.vpc_id

  # Chunkserver port
  ingress {
    from_port   = 9441
    to_port     = 9441
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # SSH port
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = [var.ssh_cidr]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(var.common_tags, {
    Name   = "${var.project_name}-chunkserver-${each.key}"
    Region = each.key
  })
}

# Latest Amazon Linux 2 AMI
data "aws_ami" "amazon_linux" {
  for_each = {
    us_east  = aws.us_east
    eu_west  = aws.eu_west
    ap_south = aws.ap_south
  }

  provider    = each.value
  most_recent = true
  owners      = ["amazon"]

  filter {
    name   = "name"
    values = ["amzn2-ami-hvm-*-x86_64-gp2"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# Master servers
resource "aws_instance" "mooseng_master" {
  for_each = {
    us_east  = { provider = aws.us_east, subnet = module.vpc_us_east.public_subnets[0], sg = aws_security_group.mooseng_master["us_east"].id, ami = data.aws_ami.amazon_linux["us_east"].id }
    eu_west  = { provider = aws.eu_west, subnet = module.vpc_eu_west.public_subnets[0], sg = aws_security_group.mooseng_master["eu_west"].id, ami = data.aws_ami.amazon_linux["eu_west"].id }
    ap_south = { provider = aws.ap_south, subnet = module.vpc_ap_south.public_subnets[0], sg = aws_security_group.mooseng_master["ap_south"].id, ami = data.aws_ami.amazon_linux["ap_south"].id }
  }

  provider               = each.value.provider
  ami                    = each.value.ami
  instance_type          = var.instance_types.master
  key_name              = var.key_name
  subnet_id             = each.value.subnet
  vpc_security_group_ids = [each.value.sg]

  root_block_device {
    volume_type = "gp3"
    volume_size = 20
    encrypted   = true
  }

  user_data = base64encode(templatefile("${path.module}/scripts/master-userdata.sh", {
    region     = each.key
    node_id    = each.key == "us_east" ? 1 : each.key == "eu_west" ? 2 : 3
    cluster_id = var.project_name
  }))

  tags = merge(var.common_tags, {
    Name      = "${var.project_name}-master-${each.key}"
    Region    = each.key
    Component = "master"
  })
}

# Chunkservers (2 per region)
resource "aws_instance" "mooseng_chunkserver" {
  for_each = {
    for pair in setproduct(["us_east", "eu_west", "ap_south"], [1, 2]) : "${pair[0]}-${pair[1]}" => {
      region    = pair[0]
      index     = pair[1]
      provider  = pair[0] == "us_east" ? aws.us_east : pair[0] == "eu_west" ? aws.eu_west : aws.ap_south
      subnet    = pair[0] == "us_east" ? module.vpc_us_east.private_subnets[pair[1] - 1] : pair[0] == "eu_west" ? module.vpc_eu_west.private_subnets[pair[1] - 1] : module.vpc_ap_south.private_subnets[pair[1] - 1]
      sg        = pair[0] == "us_east" ? aws_security_group.mooseng_chunkserver["us_east"].id : pair[0] == "eu_west" ? aws_security_group.mooseng_chunkserver["eu_west"].id : aws_security_group.mooseng_chunkserver["ap_south"].id
      ami       = pair[0] == "us_east" ? data.aws_ami.amazon_linux["us_east"].id : pair[0] == "eu_west" ? data.aws_ami.amazon_linux["eu_west"].id : data.aws_ami.amazon_linux["ap_south"].id
    }
  }

  provider               = each.value.provider
  ami                    = each.value.ami
  instance_type          = var.instance_types.chunkserver
  key_name              = var.key_name
  subnet_id             = each.value.subnet
  vpc_security_group_ids = [each.value.sg]

  root_block_device {
    volume_type = "gp3"
    volume_size = 20
    encrypted   = true
  }

  ebs_block_device {
    device_name = "/dev/sdf"
    volume_type = "gp3"
    volume_size = var.storage_volume_size
    encrypted   = true
  }

  user_data = base64encode(templatefile("${path.module}/scripts/chunkserver-userdata.sh", {
    region     = each.value.region
    node_id    = "${each.value.region}-${each.value.index}"
    master_addr = aws_instance.mooseng_master[each.value.region].private_ip
  }))

  tags = merge(var.common_tags, {
    Name      = "${var.project_name}-chunkserver-${each.key}"
    Region    = each.value.region
    Component = "chunkserver"
  })

  depends_on = [aws_instance.mooseng_master]
}

# Benchmark runner instance (in us-east for central coordination)
resource "aws_instance" "benchmark_runner" {
  provider               = aws.us_east
  ami                    = data.aws_ami.amazon_linux["us_east"].id
  instance_type          = var.instance_types.benchmark
  key_name              = var.key_name
  subnet_id             = module.vpc_us_east.public_subnets[0]
  vpc_security_group_ids = [aws_security_group.mooseng_master["us_east"].id]

  root_block_device {
    volume_type = "gp3"
    volume_size = 50  # Larger disk for benchmark data
    encrypted   = true
  }

  user_data = base64encode(templatefile("${path.module}/scripts/benchmark-userdata.sh", {
    project_name = var.project_name
    master_endpoints = {
      us_east  = aws_instance.mooseng_master["us_east"].public_ip
      eu_west  = aws_instance.mooseng_master["eu_west"].public_ip
      ap_south = aws_instance.mooseng_master["ap_south"].public_ip
    }
  }))

  tags = merge(var.common_tags, {
    Name      = "${var.project_name}-benchmark-runner"
    Region    = "us-east"
    Component = "benchmark"
  })

  depends_on = [aws_instance.mooseng_master]
}

# Output important information
output "master_public_ips" {
  description = "Public IP addresses of master servers"
  value = {
    us_east  = aws_instance.mooseng_master["us_east"].public_ip
    eu_west  = aws_instance.mooseng_master["eu_west"].public_ip
    ap_south = aws_instance.mooseng_master["ap_south"].public_ip
  }
}

output "master_private_ips" {
  description = "Private IP addresses of master servers"
  value = {
    us_east  = aws_instance.mooseng_master["us_east"].private_ip
    eu_west  = aws_instance.mooseng_master["eu_west"].private_ip
    ap_south = aws_instance.mooseng_master["ap_south"].private_ip
  }
}

output "benchmark_runner_ip" {
  description = "Public IP address of the benchmark runner"
  value       = aws_instance.benchmark_runner.public_ip
}

output "ssh_commands" {
  description = "SSH commands to connect to instances"
  value = {
    benchmark_runner = "ssh -i ${var.key_name}.pem ec2-user@${aws_instance.benchmark_runner.public_ip}"
    master_us_east   = "ssh -i ${var.key_name}.pem ec2-user@${aws_instance.mooseng_master["us_east"].public_ip}"
    master_eu_west   = "ssh -i ${var.key_name}.pem ec2-user@${aws_instance.mooseng_master["eu_west"].public_ip}"
    master_ap_south  = "ssh -i ${var.key_name}.pem ec2-user@${aws_instance.mooseng_master["ap_south"].public_ip}"
  }
}