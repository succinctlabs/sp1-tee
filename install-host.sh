# Install the Nitro Enclaves CLI.
sudo dnf install aws-nitro-enclaves-cli aws-nitro-enclaves-cli-devel openssl-devel gcc cmake3 gcc-c++ -y

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

# Install the rust toolchain.
curl https://sh.rustup.rs -sSf | sh -s -- -y

# Source the cargo env.
source $HOME/.cargo/env

# Initialize the submodules.
git submodule update --init --recursive

# Copy the allocator template to the Nitro Enclaves config directory.
sudo cp allocator.template.yaml /etc/nitro_enclaves/allocator.yaml

# Restart the Nitro Enclaves Allocator service to pick up the new allocator config.
sudo systemctl restart nitro-enclaves-allocator.service

# Install the tee server binary.
cargo install --path host --bin sp1-tee-server --features production

# Copy the tee-service template to the systemd directory.
sudo cp tee-service.template.service /etc/systemd/system/tee-service.service

# Enable and start the tee-service if the --production flag is passed.
if [ "$1" -eq "--production" ]; then
    sudo systemctl enable --now tee-service.service
else 
    echo "In order to start the tee-service automatically, you must pass the --production flag."
    echo "To start the debug mode server: `cargo run --bin sp1-tee-server -- --debug`"
fi

echo "Done installing Nitro Enclaves CLI, exit the session and login again for changes to take effect."