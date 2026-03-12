#!/bin/bash
#
# ClovaLink Installer & Updater
# Install:  curl -fsSL https://raw.githubusercontent.com/ClovaLink/ClovaLink/main/install.sh | bash
# Update:   curl -fsSL https://raw.githubusercontent.com/ClovaLink/ClovaLink/main/install.sh | bash -s -- --update
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
        read -r input
    else
        read -r input < /dev/tty
    fi

    if [ -n "$input" ]; then
        eval "$var_name=\"$input\""
    elif [ -n "$default" ]; then
        eval "$var_name=\"$default\""
    fi
}

# Detect compose command
detect_compose() {
    if check_command docker; then
        if docker compose version &> /dev/null; then
            COMPOSE_CMD="docker compose"
        elif check_command docker-compose; then
            COMPOSE_CMD="docker-compose"
        fi
    elif check_command podman; then
        if podman compose version &> /dev/null 2>&1; then
            COMPOSE_CMD="podman compose"
        elif check_command podman-compose; then
            COMPOSE_CMD="podman-compose"
        fi
    fi
}

# ==================== UPDATE MODE ====================
if [ "$1" = "--update" ] || [ "$1" = "update" ]; then
    echo ""
    echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}                                                               ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}   ${BOLD}🍀 ClovaLink Updater${NC}                                        ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}   Update your existing installation                           ${CYAN}║${NC}"
    echo -e "${CYAN}║${NC}                                                               ${CYAN}║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    # Detect compose command
    detect_compose
    if [ -z "$COMPOSE_CMD" ]; then
        echo -e "  ${RED}✗${NC} Docker/Podman Compose not found. Cannot update."
        exit 1
    fi
    echo -e "  ${GREEN}✓${NC} Using: ${COMPOSE_CMD}"

    # Find installation directory
    INSTALL_DIR="${CLOVALINK_DIR:-$HOME/clovalink}"
    echo ""
    echo -e "  ${CYAN}Where is ClovaLink installed?${NC}"
    echo -e "  ${CYAN}Press Enter for default: ${INSTALL_DIR}${NC}"
    read_input "  Directory: " USER_DIR ""
    if [ -n "$USER_DIR" ]; then
        INSTALL_DIR="$USER_DIR"
    fi

    # Verify it's a ClovaLink installation
    if [ ! -f "$INSTALL_DIR/compose.yml" ] || [ ! -f "$INSTALL_DIR/.env" ]; then
        echo -e "  ${RED}✗${NC} Not a ClovaLink installation (missing compose.yml or .env)"
        echo -e "  ${YELLOW}  Expected files in: ${INSTALL_DIR}${NC}"
        exit 1
    fi

    cd "$INSTALL_DIR"
    echo -e "  ${GREEN}✓${NC} Found installation: $INSTALL_DIR"

    # Back up current files
    echo ""
    echo -e "${BLUE}[1/4]${NC} Backing up current configuration..."
    BACKUP_TS=$(date +%Y%m%d%H%M%S)
    cp .env ".env.backup.${BACKUP_TS}"
    echo -e "  ${GREEN}✓${NC} Backed up .env → .env.backup.${BACKUP_TS}"
    cp compose.yml "compose.yml.backup.${BACKUP_TS}"
    echo -e "  ${GREEN}✓${NC} Backed up compose.yml → compose.yml.backup.${BACKUP_TS}"

    # Download latest compose.yml
    echo ""
    echo -e "${BLUE}[2/4]${NC} Downloading latest configuration..."
    REPO_URL="https://raw.githubusercontent.com/ClovaLink/ClovaLink/main/infra"
    if curl -fsSL "$REPO_URL/compose.yml" -o compose.yml.new 2>/dev/null; then
        mv compose.yml.new compose.yml
        echo -e "  ${GREEN}✓${NC} Updated compose.yml"
    else
        echo -e "  ${YELLOW}⚠${NC}  Could not download latest compose.yml, keeping current version"
        rm -f compose.yml.new
    fi

    # Check for new env vars introduced in newer versions
    if ! grep -q "BACKUP_MASTER_KEY" .env; then
        echo ""
        echo -e "  ${BOLD}New in v0.1.6: Backup Master Key${NC}"
        echo -e "  Required for scheduled auto-backups (encrypts passphrase at rest)."
        echo -e "  ${YELLOW}Manual export/import works without it.${NC}"
        echo ""
        echo -e "  ${CYAN}Generate backup master key? (Y/n):${NC}"
        read_input "  Generate: " ENABLE_BACKUP_KEY "y"
        if [[ "$ENABLE_BACKUP_KEY" =~ ^[Yy]$ ]]; then
            if command -v openssl &> /dev/null; then
                NEW_KEY=$(openssl rand -base64 48)
                echo "" >> .env
                echo "# Backup Encryption (added by updater)" >> .env
                echo "BACKUP_MASTER_KEY=${NEW_KEY}" >> .env
                echo -e "  ${GREEN}✓${NC} Added BACKUP_MASTER_KEY to .env"
            else
                echo -e "  ${YELLOW}⚠${NC} OpenSSL not found. Add BACKUP_MASTER_KEY to .env manually."
            fi
        else
            echo -e "  ${YELLOW}⚠${NC} Skipped — auto-backups will be disabled until configured"
        fi
    fi

    # Pull latest images
    echo ""
    echo -e "${BLUE}[3/4]${NC} Pulling latest images..."
    echo ""
    $COMPOSE_CMD pull

    # Restart services
    echo ""
    echo -e "${BLUE}[4/4]${NC} Restarting services..."
    echo ""
    echo -e "  ${YELLOW}Migrations will run automatically on startup...${NC}"
    echo ""
    $COMPOSE_CMD up -d

    # Wait for health check
    echo ""
    echo -e "  Waiting for services to start..."
    sleep 5

    if $COMPOSE_CMD ps | grep -q "Up\|running"; then
        echo ""
        echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║${NC}                                                               ${GREEN}║${NC}"
        echo -e "${GREEN}║${NC}   ${BOLD}🎉 ClovaLink has been updated!${NC}                               ${GREEN}║${NC}"
        echo -e "${GREEN}║${NC}                                                               ${GREEN}║${NC}"
        echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
        echo ""
        echo -e "  ${BOLD}Backup files:${NC}"
        echo -e "    ${CYAN}.env.backup.${BACKUP_TS}${NC}"
        echo -e "    ${CYAN}compose.yml.backup.${BACKUP_TS}${NC}"
        echo ""
        echo -e "  ${BOLD}If something went wrong:${NC}"
        echo -e "    ${CYAN}cd $INSTALL_DIR${NC}"
        echo -e "    ${CYAN}cp .env.backup.${BACKUP_TS} .env${NC}"
        echo -e "    ${CYAN}cp compose.yml.backup.${BACKUP_TS} compose.yml${NC}"
        echo -e "    ${CYAN}$COMPOSE_CMD up -d${NC}"
        echo ""
        echo -e "  ${BOLD}View logs:${NC}  ${CYAN}cd $INSTALL_DIR && $COMPOSE_CMD logs -f${NC}"
        echo ""
    else
        echo ""
        echo -e "${RED}Update may have failed. Check the logs:${NC}"
        echo -e "  cd $INSTALL_DIR && $COMPOSE_CMD logs"
        echo ""
        echo -e "${YELLOW}To rollback:${NC}"
        echo -e "  cp .env.backup.${BACKUP_TS} .env"
        echo -e "  cp compose.yml.backup.${BACKUP_TS} compose.yml"
        echo -e "  $COMPOSE_CMD up -d"
        exit 1
    fi

    exit 0
fi

# ==================== INSTALL MODE ====================

# Print banner
echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${NC}                                                               ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}   ${BOLD}🍀 ClovaLink Installer${NC}                                      ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}   Enterprise File Management Made Simple                      ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}                                                               ${CYAN}║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Function to generate random string (install only)
generate_secret() {
    if command -v openssl &> /dev/null; then
        openssl rand -base64 32 | tr -d '/+=' | head -c 32
    elif [ -f /dev/urandom ]; then
        cat /dev/urandom | tr -dc 'a-zA-Z0-9' | head -c 32
    else
        date +%s%N | sha256sum | head -c 32
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
PUBLIC_URL=""
SETUP_NGINX="n"

if [[ "$IS_VPS" =~ ^[Yy]$ ]]; then
    # Try to detect public IP
    DETECTED_IP=$(curl -s --max-time 5 ifconfig.me 2>/dev/null || curl -s --max-time 5 icanhazip.com 2>/dev/null || echo "")

    if [ -n "$DETECTED_IP" ]; then
        echo -e "  ${GREEN}✓${NC} Detected public IP: ${DETECTED_IP}"
    fi

    # Ask for public URL (for share links)
    echo ""
    echo -e "  ${BOLD}Public URL Configuration${NC}"
    echo -e "  This URL will be used for share links, email notifications, etc."
    echo -e "  Examples: ${CYAN}files.yourcompany.com${NC} or ${CYAN}${DETECTED_IP:-your-server-ip}${NC}"
    echo ""
    echo -e "  ${CYAN}Enter your domain or public IP:${NC}"
    if [ -n "$DETECTED_IP" ]; then
        read_input "  URL: " PUBLIC_URL "$DETECTED_IP"
    else
        read_input "  URL: " PUBLIC_URL ""
        if [ -z "$PUBLIC_URL" ]; then
            echo -e "  ${RED}✗${NC} Public URL is required for VPS installation"
            exit 1
        fi
    fi
    HOST_ADDRESS="$PUBLIC_URL"

    # Ask about HTTPS
    echo ""
    echo -e "  ${CYAN}Will you be using HTTPS? (recommended for production) (y/N):${NC}"
    read_input "  HTTPS: " USE_HTTPS "n"

    if [[ "$USE_HTTPS" =~ ^[Yy]$ ]]; then
        PROTOCOL="https"
    else
        PROTOCOL="http"
    fi

    # Ask about nginx setup
    echo ""
    echo -e "  ${CYAN}Would you like to set up nginx as a reverse proxy? (Y/n):${NC}"
    read_input "  Setup nginx: " SETUP_NGINX "y"
else
    PROTOCOL="http"
fi

# Ask about local storage encryption
ENCRYPTION_KEY=""
echo ""
echo -e "  ${BOLD}Local Storage Encryption${NC}"
echo -e "  ClovaLink can encrypt files at rest using ChaCha20-Poly1305."
echo -e "  ${YELLOW}Note: Only applies to local storage (S3 uses provider encryption).${NC}"
echo ""
echo -e "  ${CYAN}Enable local file encryption? (recommended for production) (Y/n):${NC}"
read_input "  Encrypt: " ENABLE_ENCRYPTION "y"

if [[ "$ENABLE_ENCRYPTION" =~ ^[Yy]$ ]]; then
    # Generate a secure 32-byte encryption key
    if command -v openssl &> /dev/null; then
        ENCRYPTION_KEY=$(openssl rand -base64 32)
        echo -e "  ${GREEN}✓${NC} Generated secure encryption key"
        echo ""
        echo -e "  ${RED}${BOLD}⚠️  IMPORTANT: Save this key securely!${NC}"
        echo -e "  ${RED}   Losing this key means losing access to all encrypted files.${NC}"
        echo -e "  ${YELLOW}   Key: ${ENCRYPTION_KEY}${NC}"
        echo ""
    else
        echo -e "  ${YELLOW}⚠${NC} OpenSSL not found, skipping encryption key generation"
        echo -e "  You can manually set ENCRYPTION_KEY in .env later."
    fi
fi

# Ask about backup encryption key
BACKUP_MASTER_KEY=""
echo ""
echo -e "  ${BOLD}Backup Encryption${NC}"
echo -e "  Required if you plan to use scheduled auto-backups."
echo -e "  Encrypts the system-generated backup passphrase at rest in the database."
echo -e "  ${YELLOW}Note: Manual export/import works without this.${NC}"
echo ""
echo -e "  ${CYAN}Generate backup master key? (recommended) (Y/n):${NC}"
read_input "  Generate: " ENABLE_BACKUP_KEY "y"

if [[ "$ENABLE_BACKUP_KEY" =~ ^[Yy]$ ]]; then
    if command -v openssl &> /dev/null; then
        BACKUP_MASTER_KEY=$(openssl rand -base64 48)
        echo -e "  ${GREEN}✓${NC} Generated backup master key"
    else
        echo -e "  ${YELLOW}⚠${NC} OpenSSL not found, skipping key generation"
        echo -e "  You can manually set BACKUP_MASTER_KEY in .env later."
    fi
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
    echo "# Local Storage Encryption (ChaCha20-Poly1305)"
    echo "# Only applies to local storage - S3 uses provider-side encryption"
    if [ -n "$ENCRYPTION_KEY" ]; then
        echo "ENCRYPTION_KEY=${ENCRYPTION_KEY}"
    else
        echo "# ENCRYPTION_KEY=your-base64-encoded-32-byte-key"
    fi
    echo ""
    echo "# Backup Encryption (encrypts auto-backup passphrases at rest)"
    echo "# Required for scheduled auto-backups, not needed for manual export/import"
    echo "# Generate with: openssl rand -base64 48"
    if [ -n "$BACKUP_MASTER_KEY" ]; then
        echo "BACKUP_MASTER_KEY=${BACKUP_MASTER_KEY}"
    else
        echo "# BACKUP_MASTER_KEY=your-key-here-minimum-32-chars"
    fi
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
    echo ""
    echo "# Base URL for share links and notifications"
    if [[ "$SETUP_NGINX" =~ ^[Yy]$ ]]; then
        echo "BASE_URL=${PROTOCOL}://${HOST_ADDRESS}"
    elif [ "$HOST_ADDRESS" != "localhost" ]; then
        echo "BASE_URL=${PROTOCOL}://${HOST_ADDRESS}:${WEB_PORT}"
    else
        echo "BASE_URL=http://localhost:${WEB_PORT}"
    fi
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
        DISPLAY_URL="${PROTOCOL}://${HOST_ADDRESS}"
        echo -e "  ${BOLD}Web Interface:${NC}  ${CYAN}${DISPLAY_URL}${NC}"
        echo -e "  ${BOLD}API Endpoint:${NC}   ${CYAN}${DISPLAY_URL}/api${NC}"
    elif [ "$HOST_ADDRESS" != "localhost" ]; then
        DISPLAY_URL="${PROTOCOL}://${HOST_ADDRESS}:${WEB_PORT}"
        echo -e "  ${BOLD}Web Interface:${NC}  ${CYAN}${DISPLAY_URL}${NC}"
        echo -e "  ${BOLD}API Endpoint:${NC}   ${CYAN}${PROTOCOL}://${HOST_ADDRESS}:${API_PORT}${NC}"
    else
        DISPLAY_URL="http://localhost:${WEB_PORT}"
        echo -e "  ${BOLD}Web Interface:${NC}  ${CYAN}${DISPLAY_URL}${NC}"
        echo -e "  ${BOLD}API Endpoint:${NC}   ${CYAN}http://localhost:${API_PORT}${NC}"
    fi
    echo ""
    echo -e "  ${BOLD}Share Links:${NC}    ${CYAN}${DISPLAY_URL}/share/...${NC}"
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
    echo -e "    Update:       ${CYAN}curl -fsSL https://raw.githubusercontent.com/ClovaLink/ClovaLink/main/install.sh | bash -s -- --update${NC}"
    echo ""
else
    echo ""
    echo -e "${RED}Something went wrong. Check the logs:${NC}"
    echo -e "  cd $INSTALL_DIR && $COMPOSE_CMD logs"
    exit 1
fi
