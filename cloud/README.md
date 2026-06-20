# Cloud

This directory is only for the infrastructure setup needed to create the machine and obtain the connection details. Terraform should provide AWS authentication inputs, create the instance, and output the machine address/key information needed for later manual or CI/CD deployment steps.

Before you will atempt configuration, make sure you [add an SSH key to Lightsail, download it and note its name](https://eu-central-1.lightsail.aws.amazon.com/ls/webapp/account/keys) and retrieve your [access keys from the AWS Console](https://us-east-1.console.aws.amazon.com/iam/home?region=us-east-1#/security_credentials).

Configuration starts from `terraform.tfvars` file inside this directory where you are supposed to fill variables mentioned bellow with your real-world credentials.

```tf
# terraform.tfvars
aws_access_key    = "your-access-key"
aws_secret_key    = "your-secret-key"
aws_session_token = ""
ssh_key_pair_name = "..."
```

You should use Terraform Cloud before planning or applying terraform plans and remember to avoid storing `terraform.tfstate` in public. Regual workflow in order to introduce infrastructure is to use `terraform init`, `terraform plan` and `terraform apply`.