#!/bin/bash
# Nuthatch Compositor - Setup Script
# Run this once to configure your system for DRM compositor development

set -e

echo "🐦 Nuthatch Compositor - Development Setup"
echo "=========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if running with sudo
if [ "$EUID" -eq 0 ]; then 
    echo -e "${RED}❌ Don't run this script with sudo!${NC}"
    echo "Run as your regular user: ./setup-dev.sh"
    exit 1
fi

echo "Checking system requirements..."
echo ""

# 1. Check groups
echo "1️⃣  Checking user groups..."
USER_GROUPS=$(groups)

if echo "$USER_GROUPS" | grep -q "video"; then
    echo -e "   ${GREEN}✓${NC} Already in 'video' group"
else
    echo -e "   ${YELLOW}⚠${NC}  Adding to 'video' group..."
    sudo usermod -a -G video $USER
    echo -e "   ${GREEN}✓${NC} Added to 'video' group"
    NEEDS_RELOGIN=1
fi

if echo "$USER_GROUPS" | grep -q "input"; then
    echo -e "   ${GREEN}✓${NC} Already in 'input' group"
else
    echo -e "   ${YELLOW}⚠${NC}  Adding to 'input' group..."
    sudo usermod -a -G input $USER
    echo -e "   ${GREEN}✓${NC} Added to 'input' group"
    NEEDS_RELOGIN=1
fi

if echo "$USER_GROUPS" | grep -q "render"; then
    echo -e "   ${GREEN}✓${NC} Already in 'render' group"
else
    echo -e "   ${YELLOW}⚠${NC}  Adding to 'render' group..."
    sudo usermod -a -G render $USER
    echo -e "   ${GREEN}✓${NC} Added to 'render' group"
    NEEDS_RELOGIN=1
fi

echo ""

# 2. Check GPU device
echo "2️⃣  Checking GPU device..."
if [ -e "/dev/dri/card0" ]; then
    echo -e "   ${GREEN}✓${NC} GPU device found: /dev/dri/card0"
    ls -la /dev/dri/card0
else
    echo -e "   ${RED}❌${NC} No GPU device found at /dev/dri/card0"
    echo "   Your system may not support DRM/KMS"
fi

echo ""

# 3. Add bash aliases
echo "3️⃣  Setting up bash aliases..."

ALIAS_BLOCK="
# Nuthatch Compositor Development Aliases
alias nc-check='cd ~/src/nuthatch-compositor && cargo check 2>&1 | tail -20'
alias nc-build='cd ~/src/nuthatch-compositor && cargo build --release 2>&1 | tail -20'
alias nc-run='cd ~/src/nuthatch-compositor && sudo RUST_LOG=info cargo run --release'
alias nc-log='tail -f /tmp/nuthatch-compositor.log'
alias nc-clean='cd ~/src/nuthatch-compositor && cargo clean'
alias nc-kill='sudo pkill -9 -f nuthatch-compositor'
alias nc-status='ps aux | grep nuthatch-compositor'
"

if grep -q "Nuthatch Compositor Development Aliases" ~/.bashrc; then
    echo -e "   ${GREEN}✓${NC} Aliases already configured in ~/.bashrc"
else
    echo -e "   ${YELLOW}⚠${NC}  Adding aliases to ~/.bashrc..."
    echo "$ALIAS_BLOCK" >> ~/.bashrc
    echo -e "   ${GREEN}✓${NC} Aliases added to ~/.bashrc"
fi

echo ""

# 4. Build project
echo "4️⃣  Building compositor (this may take a few minutes)..."
cd ~/src/nuthatch-compositor

if cargo build --release 2>&1 | tail -5; then
    echo -e "   ${GREEN}✓${NC} Compositor built successfully"
else
    echo -e "   ${RED}❌${NC} Build failed - check errors above"
    exit 1
fi

echo ""
echo "=========================================="
echo -e "${GREEN}✓ Setup Complete!${NC}"
echo ""

if [ -n "$NEEDS_RELOGIN" ]; then
    echo -e "${YELLOW}⚠️  IMPORTANT: Log out and back in for group changes to take effect${NC}"
    echo ""
fi

echo "📝 Next Steps:"
echo ""
echo "1. Source your bashrc to load aliases:"
echo "   source ~/.bashrc"
echo ""
echo "2. Test in GUI first:"
echo "   nc-check    # Quick syntax check"
echo "   nc-build    # Build release version"
echo ""
echo "3. Test on hardware (from TTY):"
echo "   - Press Ctrl+Alt+F2"
echo "   - Login"
echo "   - Run: nc-run"
echo "   - Press Ctrl+C to stop"
echo "   - Press Ctrl+Alt+F3 to return to KDE"
echo ""
echo "4. View logs:"
echo "   nc-log"
echo ""
echo "📚 Read the workflow guide:"
echo "   cat ~/src/nuthatch-compositor/docs/HYBRID_WORKFLOW.md"
echo ""
echo "🐦 Happy compositing!"
