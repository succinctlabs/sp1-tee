# Ensure we are in the directory of the script.
pushd $(dirname $0)

# Install nginx.
sudo dnf install -y nginx

# Ensure the service is enabled and started.
sudo systemctl enable --now nginx

# Copy the nginx config.
sudo cp ../proxy.nginx.conf /usr/local/nginx/nginx.conf

# Reload the nginx config.
sudo nginx -s reload

# Start the server
sudo nginx 

popd
