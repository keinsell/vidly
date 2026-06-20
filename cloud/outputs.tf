output "instance_id" {
  value = aws_lightsail_instance.app.id
}

output "ipv4_address" {
  description = "Public IPv4 address for the instance."
  value       = aws_lightsail_instance.app.public_ip_address
}

output "ipv6_address" {
  description = "Primary IPv6 address for the instance."
  value       = aws_lightsail_instance.app.ipv6_addresses[0]
}

output "ipv6_addresses" {
  description = "All IPv6 addresses for the instance."
  value       = aws_lightsail_instance.app.ipv6_addresses
}

output "availability_zone" {
  value = aws_lightsail_instance.app.availability_zone
}
