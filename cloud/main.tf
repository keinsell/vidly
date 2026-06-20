resource "aws_lightsail_instance" "app" {
  name              = "vidly"
  availability_zone = "eu-central-1a"
  blueprint_id      = "opensuse_16"
  bundle_id         = "nano_3_0"
  ip_address_type   = "dualstack"
  key_pair_name     = var.keypair_name
}

resource "aws_lightsail_instance_public_ports" "http" {
  instance_name = aws_lightsail_instance.app.name

  # WARNING: This intentionally allows all traffic from anywhere.
  # This is not recommended for production; it was explicitly acknowledged.
  port_info {
    from_port   = 0
    to_port     = 65535
    protocol    = "all"
    cidrs       = ["0.0.0.0/0"]
    ipv6_cidrs  = ["::/0"]
  }
}

resource "aws_lightsail_instance_public_ports" "ssh" {
  instance_name = aws_lightsail_instance.app.name

  # WARNING: This intentionally allows all traffic from anywhere.
  # This is not recommended for production; it was explicitly acknowledged.
  port_info {
    from_port   = 0
    to_port     = 65535
    protocol    = "all"
    cidrs       = ["0.0.0.0/0"]
    ipv6_cidrs  = ["::/0"]
  }
}
