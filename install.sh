#!/bin/bash
#
# ClovaLink Installer
# One-line install: curl -fsSL https://raw.githubusercontent.com/ClovaLink/ClovaLink/main/install.sh | bash
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Print banner
echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${NC}                                                               ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}   ${BOLD}🍀 ClovaLink Installer${NC}                                      ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}   Enterprise File Management Made Simple                      ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}                                                               ${CYAN}║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Function to generate random string
generate_secret() {
    if command -v openssl &> /dev/null; then
        openssl rand -base64 32 | tr -d '/+=' | head -c 32
    elif [ -f /dev/urandom ]; then
        cat /dev/urandom | tr -dc 'a-zA-Z0-9' | head -c 32
    else
        date +%s%N | sha256sum | head -c 32
    fi
}

# Function to check command exists
check_command() {
    if ! command -v "$1" &> /dev/null; then
        return 1
    fi
    return 0
}

# Function to read input (works when piped to bash)
read_input() {
    local prompt="$1"
    local var_name="$2"
    local default="$3"

    echo -en "$prompt"
    if [ -t 0 ]; then
        # stdin is a terminal, read normally
        read -r input
    else
        # stdin is a pipe, read from /dev/tty
        read -r input < /dev/tty
    fi

    if [ -n "$input" ]; then
        eval "$var_name=\"$input\""
    elif [ -n "$default" ]; then
        eval "$var_name=\"$default\""
    fi
}

# Step 1: Check Docker
echo -e "${BLUE}[1/5]${NC} Checking Docker installation..."

if check_command docker; then
    DOCKER_VERSION=$(docker --version 2>/dev/null | cut -d' ' -f3 | cut -d',' -f1)
    echo -e "  ${GREEN}✓${NC} Docker ${DOCKER_VERSION} is installed"
elif check_command podman; then
    PODMAN_VERSION=$(podman --version 2>/dev/null | cut -d' ' -f3)
    echo -e "  ${GREEN}✓${NC} Podman ${PODMAN_VERSION} is installed"
    DOCKER_CMD="podman"
    COMPOSE_CMD="podman-compose"
else
    echo -e "  ${RED}✗${NC} Docker is not installed"
    echo ""
    echo -e "${YELLOW}Please install Docker first:${NC}"
    echo ""
    echo "  Linux:   curl -fsSL https://get.docker.com | sh"
    echo "  Mac:     brew install --cask docker"
    echo "  Windows: Download from https://docker.com/products/docker-desktop"
    echo ""
    exit 1
fi

# Set compose command
if [ -z "$COMPOSE_CMD" ]; then
    if docker compose version &> /dev/null; then
        COMPOSE_CMD="docker compose"
    elif check_command docker-compose; then
        COMPOSE_CMD="docker-compose"
    else
        echo -e "  ${RED}✗${NC} Docker Compose is not available"
        echo "  Please install Docker Compose: https://docs.docker.com/compose/install/"
        exit 1
    fi
fi
echo -e "  ${GREEN}✓${NC} Using: ${COMPOSE_CMD}"

# Step 2: Create directory
echo ""
echo -e "${BLUE}[2/5]${NC} Setting up installation directory..."

INSTALL_DIR="${CLOVALINK_DIR:-$HOME/clovalink}"

# Ask for installation directory
echo -e "  Where would you like to install ClovaLink?"
echo -e "  ${CYAN}Press Enter for default: ${INSTALL_DIR}${NC}"
read_input "  Directory: " USER_DIR ""
if [ -n "$USER_DIR" ]; then
    INSTALL_DIR="$USER_DIR"
fi

# Create directory
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"
echo -e "  ${GREEN}✓${NC} Created: $INSTALL_DIR"

# Step 3: Download files
echo ""
echo -e "${BLUE}[3/5]${NC} Downloading configuration files..."

REPO_URL="https://raw.githubusercontent.com/ClovaLink/ClovaLink/main/infra"

curl -fsSL "$REPO_URL/compose.yml" -o compose.yml
echo -e "  ${GREEN}✓${NC} Downloaded compose.yml"

curl -fsSL "$REPO_URL/.env.example" -o .env.example 2>/dev/null || true
echo -e "  ${GREEN}✓${NC} Downloaded .env.example"

# Step 4: Configure environment
echo ""
echo -e "${BLUE}[4/5]${NC} Configuring your installation..."

# Generate secrets
JWT_SECRET=$(generate_secret)
POSTGRES_PASSWORD=$(generate_secret)

echo -e "  ${GREEN}✓${NC} Generated secure JWT secret"
echo -e "  ${GREEN}✓${NC} Generated secure database password"

# Ask for ports
echo ""
echo -e "  ${CYAN}Web interface port (default: 8080):${NC}"
read_input "  Port: " WEB_PORT "8080"

echo -e "  ${CYAN}API port (default: 3000):${NC}"
read_input "  Port: " API_PORT "3000"

# Ask about deployment type (local vs VPS)
echo ""
echo -e "  ${CYAN}Are you installing on a VPS/remote server? (y/N):${NC}"
read_input "  VPS: " IS_VPS "n"

HOST_ADDRESS="localhost"
SETUP_NGINX="n"

if [[ "$IS_VPS" =~ ^[Yy]$ ]]; then
    # Try to detect public IP
    DETECTED_IP=$(curl -s --max-time 5 ifconfig.me 2>/dev/null || curl -s --max-time 5 icanhazip.com 2>/dev/null || echo "")

    if [ -n "$DETECTED_IP" ]; then
        echo -e "  ${GREEN}✓${NC} Detected public IP: ${DETECTED_IP}"
        echo -e "  ${CYAN}Enter your domain or IP (default: ${DETECTED_IP}):${NC}"
        read_input "  Host: " HOST_ADDRESS "$DETECTED_IP"
    else
        echo -e "  ${CYAN}Enter your domain or server IP:${NC}"
        read_input "  Host: " HOST_ADDRESS ""
        if [ -z "$HOST_ADDRESS" ]; then
            echo -e "  ${RED}✗${NC} Host address is required for VPS installation"
            exit 1
        fi
    fi

    # Ask about nginx setup
    echo ""
    echo -e "  ${CYAN}Would you like to set up nginx as a reverse proxy? (Y/n):${NC}"
    read_input "  Setup nginx: " SETUP_NGINX "y"
fi

# Create .env file
{
    echo "# ClovaLink Configuration"
    echo "# Generated by installer on $(date)"
    echo ""
    echo "# Security - These were auto-generated, keep them secret!"
    echo "JWT_SECRET=${JWT_SECRET}"
    echo ""
    echo "# Database"
    echo "POSTGRES_PASSWORD=${POSTGRES_PASSWORD}"
    echo "DATABASE_URL=postgres://postgres:${POSTGRES_PASSWORD}@postgres:5432/clovalink"
    echo ""
    echo "# Redis"
    echo "REDIS_URL=redis://redis:6379"
    echo ""
    echo "# Storage (local by default, configure S3 for production)"
    echo "STORAGE_TYPE=local"
    echo ""
    echo "# Optional: S3/Backblaze B2/Wasabi configuration"
    echo "# S3_ENDPOINT=https://s3.us-west-002.backblazeb2.com"
    echo "# S3_BUCKET=your-bucket-name"
    echo "# S3_REGION=us-west-002"
    echo "# S3_ACCESS_KEY=your-access-key"
    echo "# S3_SECRET_KEY=your-secret-key"
    echo ""
    echo "# Environment"
    echo "ENVIRONMENT=production"
    echo "RUST_LOG=info"
} > .env

echo -e "  ${GREEN}✓${NC} Created .env configuration"

# Update compose.yml ports if changed
if [ "$WEB_PORT" != "8080" ]; then
    sed -i.bak "s/8080:80/${WEB_PORT}:80/g" compose.yml 2>/dev/null || \
    sed -i '' "s/8080:80/${WEB_PORT}:80/g" compose.yml
fi
if [ "$API_PORT" != "3000" ]; then
    sed -i.bak "s/3000:3000/${API_PORT}:3000/g" compose.yml 2>/dev/null || \
    sed -i '' "s/3000:3000/${API_PORT}:3000/g" compose.yml
fi
rm -f compose.yml.bak 2>/dev/null

# Setup nginx if requested
if [[ "$SETUP_NGINX" =~ ^[Yy]$ ]]; then
    echo ""
    echo -e "  ${CYAN}Setting up nginx reverse proxy...${NC}"

    # Check if nginx is installed
    if ! check_command nginx; then
        echo -e "  ${YELLOW}Installing nginx...${NC}"
        if check_command apt-get; then
            sudo apt-get update -qq && sudo apt-get install -y -qq nginx > /dev/null 2>&1
        elif check_command yum; then
            sudo yum install -y -q nginx > /dev/null 2>&1
        elif check_command dnf; then
            sudo dnf install -y -q nginx > /dev/null 2>&1
        else
            echo -e "  ${RED}✗${NC} Could not install nginx automatically. Please install it manually."
            SETUP_NGINX="n"
        fi
    fi

    if [[ "$SETUP_NGINX" =~ ^[Yy]$ ]] && check_command nginx; then
        # Create nginx config
        NGINX_CONF="/etc/nginx/sites-available/clovalink"
        NGINX_CONF_ENABLED="/etc/nginx/sites-enabled/clovalink"

        # Check if sites-available exists, otherwise use conf.d
        if [ ! -d "/etc/nginx/sites-available" ]; then
            NGINX_CONF="/etc/nginx/conf.d/clovalink.conf"
            NGINX_CONF_ENABLED=""
        fi

        sudo tee "$NGINX_CONF" > /dev/null << NGINX_EOF
server {
    listen 80;
    server_name ${HOST_ADDRESS};

    # Web interface
    location / {
        proxy_pass http://127.0.0.1:${WEB_PORT};
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_cache_bypass \$http_upgrade;
    }

    # API endpoint
    location /api {
        proxy_pass http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_cache_bypass \$http_upgrade;
        client_max_body_size 100M;
    }

    # File uploads - increase body size limit
    client_max_body_size 100M;
}
NGINX_EOF

        # Enable site if using sites-available/sites-enabled pattern
        if [ -n "$NGINX_CONF_ENABLED" ]; then
            sudo ln -sf "$NGINX_CONF" "$NGINX_CONF_ENABLED"
            # Remove default site if it exists
            sudo rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true
        fi

        # Test and reload nginx
        if sudo nginx -t > /dev/null 2>&1; then
            sudo systemctl reload nginx 2>/dev/null || sudo nginx -s reload 2>/dev/null || true
            echo -e "  ${GREEN}✓${NC} Nginx configured and reloaded"
        else
            echo -e "  ${RED}✗${NC} Nginx configuration test failed. Please check manually."
        fi
    fi
fi

# Step 5: Start services
echo ""
echo -e "${BLUE}[5/5]${NC} Starting ClovaLink..."
echo ""
echo -e "  ${YELLOW}This may take a few minutes on first run...${NC}"
echo ""

$COMPOSE_CMD up -d

# Wait for services to be ready
echo ""
echo -e "  Waiting for services to start..."
sleep 5

# Check if services are running
if $COMPOSE_CMD ps | grep -q "Up\|running"; then
    echo ""
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║${NC}                                                               ${GREEN}║${NC}"
    echo -e "${GREEN}║${NC}   ${BOLD}🎉 ClovaLink is now running!${NC}                                ${GREEN}║${NC}"
    echo -e "${GREEN}║${NC}                                                               ${GREEN}║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    # Display appropriate URLs based on setup type
    if [[ "$SETUP_NGINX" =~ ^[Yy]$ ]]; then
        echo -e "  ${BOLD}Web Interface:${NC}  ${CYAN}http://${HOST_ADDRESS}${NC}"
        echo -e "  ${BOLD}API Endpoint:${NC}   ${CYAN}http://${HOST_ADDRESS}/api${NC}"
    elif [ "$HOST_ADDRESS" != "localhost" ]; then
        echo -e "  ${BOLD}Web Interface:${NC}  ${CYAN}http://${HOST_ADDRESS}:${WEB_PORT}${NC}"
        echo -e "  ${BOLD}API Endpoint:${NC}   ${CYAN}http://${HOST_ADDRESS}:${API_PORT}${NC}"
    else
        echo -e "  ${BOLD}Web Interface:${NC}  ${CYAN}http://localhost:${WEB_PORT}${NC}"
        echo -e "  ${BOLD}API Endpoint:${NC}   ${CYAN}http://localhost:${API_PORT}${NC}"
    fi
    echo ""
    echo -e "  ${BOLD}Default Login:${NC}"
    echo -e "    Email:    ${CYAN}superadmin@clovalink.com${NC}"
    echo -e "    Password: ${CYAN}password123${NC}"
    echo ""
    echo -e "  ${YELLOW}⚠️  Change the default password immediately!${NC}"
    echo ""
    echo -e "  ${BOLD}Useful Commands:${NC}"
    echo -e "    View logs:    ${CYAN}cd $INSTALL_DIR && $COMPOSE_CMD logs -f${NC}"
    echo -e "    Stop:         ${CYAN}cd $INSTALL_DIR && $COMPOSE_CMD down${NC}"
    echo -e "    Update:       ${CYAN}cd $INSTALL_DIR && $COMPOSE_CMD pull && $COMPOSE_CMD up -d${NC}"
    echo ""
else
    echo ""
    echo -e "${RED}Something went wrong. Check the logs:${NC}"
    echo -e "  cd $INSTALL_DIR && $COMPOSE_CMD logs"
    exit 1
fi
