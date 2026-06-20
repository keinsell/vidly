variable "aws_region" {
  description = "AWS region to deploy the instance into."
  type        = string
  default     = "eu-central-1"
}

variable "aws_access_key" {
  description = "AWS access key used to authenticate Terraform."
  type        = string
  sensitive   = true
}

variable "aws_secret_key" {
  description = "AWS secret key used to authenticate Terraform."
  type        = string
  sensitive   = true
}

variable "aws_session_token" {
  description = "Optional AWS session token for temporary credentials."
  type        = string
  sensitive   = true
  default     = ""
}

variable "keypair_name" {
  description = "Existing AWS Lightsail key pair name used for SSH access."
  type        = string
}
