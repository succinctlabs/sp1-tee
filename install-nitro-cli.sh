# Install the Nitro Enclaves CLI.
sudo dnf install aws-nitro-enclaves-cli aws-nitro-enclaves-cli-devel openssl openssl-devel -y

# Add the ec2-user to the ne group.
sudo usermod -aG ne ec2-user

# Add the ec2-user to the docker group.
sudo usermod -aG docker ec2-user

# Check the version of the Nitro Enclaves CLI.
nitro-cli --version

# Enable and start the Nitro Enclaves Allocator service.
sudo systemctl enable --now nitro-enclaves-allocator.service

# Enable and start the docker service.
sudo systemctl enable --now docker

echo "Done installing Nitro Enclaves CLI, exit the session and login again for changes to take effect."