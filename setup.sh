#!/bin/bash

# ---------------------------------------
# Color definitions for pretty output
# ---------------------------------------
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=======================================${NC}"
echo -e "${BLUE}   LLM Agent Installation Assistant    ${NC}"
echo -e "${BLUE}=======================================${NC}"

# ---------------------------------------
# 1. Check for prerequisites
# ---------------------------------------

echo -e "\n${BLUE}[1/4] Checking prerequisites...${NC}"

if ! command -v cargo &>/dev/null; then
    echo -e "${RED}Error: Rust (cargo) is not installed.${NC}"
    echo "Install it from https://rustup.rs/"
    exit 1
fi
echo -e "${GREEN}✓ Rust found${NC}"

if ! command -v docker &>/dev/null; then
    echo -e "${RED}Error: Docker is not installed or not in PATH.${NC}"
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &>/dev/null; then
    echo -e "${RED}Error: Docker daemon is not running.${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Docker found and running${NC}"

# Check if Ollama exists
if ! command -v ollama &>/dev/null; then
    echo -e "${RED}Warning: Ollama is not found in PATH.${NC}"
    echo "The Agent requires Ollama at localhost:11434."
    read -p "Continue anyway? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
else
    echo -e "${GREEN}✓ Ollama found${NC}"
fi

# ---------------------------------------
# 2. Configure workspace
# ---------------------------------------

echo -e "\n${BLUE}[2/4] Configuring workspace...${NC}"
echo "Where should the Agent store its files?"
echo "This folder will be shared with the Docker sandbox."

read -e -p "Path (default: ./workspace): " USER_PATH
if [ -z "$USER_PATH" ]; then
    USER_PATH="./workspace"
fi

mkdir -p "$USER_PATH"
FULL_PATH=$(realpath "$USER_PATH")

echo -e "Workspace set to: ${GREEN}$FULL_PATH${NC}"

# ---------------------------------------
# 3. Build the project
# ---------------------------------------

echo -e "\n${BLUE}[3/4] Building Rust project (release mode)...${NC}"
echo "This may take a few minutes..."

cargo build --release
if [ $? -ne 0 ]; then
    echo -e "${RED}Build failed.${NC}"
    exit 1
fi

# ---------------------------------------
# 4. Create run wrapper
# ---------------------------------------

echo -e "\n${BLUE}[4/4] Creating startup script...${NC}"

WRAPPER_NAME="run_agent.sh"
BINARY_PATH="./target/release/copilot_rust_llama"

cat >"$WRAPPER_NAME" <<EOF
#!/bin/bash

# Auto-generated wrapper script

export LLM_AGENT_WORKSPACE="$FULL_PATH"
echo "Starting Agent with workspace: \$LLM_AGENT_WORKSPACE"
"$BINARY_PATH"
EOF

chmod +x "$WRAPPER_NAME"

# ---------------------------------------
# Done
# ---------------------------------------

echo -e "\n${GREEN}=======================================${NC}"
echo -e "${GREEN}   Installation Complete!              ${NC}"
echo -e "${GREEN}=======================================${NC}"
echo ""
echo "To start the agent, run:"
echo -e "${BLUE}./$WRAPPER_NAME${NC}"
echo ""
