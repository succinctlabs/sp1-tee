# Install nginx.
sudo dnf install -y nginx

# Create the nginx directory if it doesn't exist.
sudo mkdir -p /usr/local/nginx

# Copy the nginx config.
sudo cp ../proxy.nginx.conf /etc/nginx/nginx.conf

# Ensure the service is enabled and started.
sudo systemctl enable --now nginx

# Restart the service for idempotency.
sudo systemctl restart nginx
