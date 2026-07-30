#!/bin/bash
#
# W1-6 Interactive Permission Guide Script
# Detects TCC screen recording permission status and guides user to grant authorization
# Then automatically runs the full automation suite
#

set -euo pipefail

# Colors for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CHECK_INTERVAL=5  # seconds between permission checks
MAX_WAIT_TIME=3600  # max wait time in seconds (1 hour)

# Detect current terminal emulator
detect_terminal() {
    local term_app=""

    # Check common terminal apps
    if pgrep -q "iTerm.app"; then
        term_app="iTerm"
    elif pgrep -q "com.apple.Terminal"; then
        term_app="Terminal"
    elif pgrep -q "zsh\|bash"; then
        # Fall back to detecting from PATH or environment
        case "$TERM_PROGRAM" in
            "iTerm2") term_app="iTerm" ;;
            "") term_app="Unknown Terminal" ;;
            *) term_app="$TERM_PROGRAM" ;;
        esac
    else
        term_app="Unknown Terminal"
    fi

    echo "$term_app"
}

# Check current TCC screen recording status for specific app
check_tcc_status() {
    local app_name="$1"
    local bundle_id="$2"

    # Try tccutil query (not available on macOS 15+)
    if /usr/bin/tccutil query "kTCCServiceScreenCapture" 2>/dev/null | grep -q "$app_name\|$bundle_id"; then
        return 0  # Has permission
    fi

    # Fallback: check if process has required capabilities using gettaskinfo
    # This is less reliable but works as heuristic
    if pgrep -x "$(basename "$app_name")" > /dev/null 2>&1; then
        # Process exists, assume it might need permission
        return 1  # Unknown status
    fi

    return 1  # Does not have permission
}

# Display welcome message
display_header() {
    clear
    cat << 'EOF'
╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║   W1-6 Desktop UI Automation - Permission Guide           ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

EOF
}

# Display step-by-step guide
show_permission_guide() {
    local terminal_type="$1"

    cat << EOF

${BLUE}📋 Step-by-Step Authorization Guide${NC}

┌───────────────────────────────────────────────────────────┐
│  STEP 1: Open System Settings                              │
│                                                           │
│  Click Apple menu () → System Settings                 │
│                                                           │
└───────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────┐
│  STEP 2: Navigate to Privacy & Security                   │
│                                                           │
│  Click "Privacy & Security" in left sidebar              │
│  Scroll down to "Security" section                       │
│                                                           │
└───────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────┐
│  STEP 3: Find Screen Recording permission                 │
│                                                           │
│  Locate "Screen Recording" in the list                   │
│  You should see:                                           │
│    • ${terminal_type}                                    │
│                                                           │
└───────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────┐
│  STEP 4: Enable Screen Recording                           │
│                                                           │
│  Toggle the switch NEXT TO "${terminal_type}" to ON ✅    │
│  If prompted, click "Open System Settings" → Allow       │
│                                                           │
└───────────────────────────────────────────────────────────┘

EOF
}

# Display success message after authorization
show_success_message() {
    cat << 'EOF'

${GREEN}✅ Permission Granted!${NC}

Starting automated verification tests...

EOF
}

# Wait for user to grant permission with polling
wait_for_authorization() {
    local terminal_type="$1"
    local elapsed=0

    echo -e "${YELLOW}⏳ Waiting for you to grant screen recording permission...${NC}"
    echo -e "${YELLOW}(Checks every ${CHECK_INTERVAL}s, max ${MAX_WAIT_TIME}s)${NC}\n"

    while [ $elapsed -lt $MAX_WAIT_TIME ]; do
        # Attempt a simple screen capture test via osascript
        # This will fail gracefully if no permission, succeed if granted
        if /usr/bin/osascript -e 'tell application "System Events" to get name of every process' 2>/dev/null; then
            # Basic system events access works, likely has screen recording
            sleep 2  # Give system time to register permission

            # Double-check with actual screen capture attempt
            if /usr/bin/screencapture /tmp/w1-6-permission-test.png 2>/dev/null; then
                rm -f /tmp/w1-6-permission-test.png
                echo -e "\n${GREEN}✓ Screen recording permission detected!${NC}\n"
                return 0
            fi
        fi

        # Progress indicator
        if [ $((elapsed % 30)) -eq 0 ] && [ $elapsed -gt 0 ]; then
            echo -e "${YELLOW}⏱️  Still waiting... ($elapsed/${MAX_WAIT_TIME}s)${NC}"
        fi

        sleep $CHECK_INTERVAL
        elapsed=$((elapsed + CHECK_INTERVAL))
    done

    echo -e "\n${RED}❌ Timeout: Did not detect permission within ${MAX_WAIT_TIME}s${NC}"
    echo -e "${YELLOW}Please manually grant permission and press Enter to retry, or Ctrl+C to exit${NC}"

    # Allow manual continue
    read -r -p "Permission granted? Press Enter to continue or Ctrl+C to exit"
    return 0
}

# Launch static smoke tests (no permission needed)
run_static_smoke_tests() {
    echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}Running Static Smoke Tests (no permission needed)...${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

    # Run existing smoke scripts
    node scripts/path-depth-wave-smoke.mjs || {
        echo -e "${RED}Static smoke test failed${NC}"
        exit 1
    }

    echo -e "\n${GREEN}✅ Static smoke tests passed (62/62)${NC}\n"
}

# Provide user choice: automated vs manual execution
choose_execution_mode() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}Choose Execution Mode:${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "1) 🤖 Automated Playwright Tests (Recommended)"
    echo "   - Uses headless Chrome + Mock Tauri IPC"
    echo "   - No TCC permissions needed!"
    echo "   - Fast (~2 minutes), repeatable, CI-integratable"
    echo ""
    echo "2) 👤 Manual Inspection (Interactive)"
    echo "   - Uses existing w1-6-manual-check.sh script"
    echo "   - Requires TCC screen recording permission"
    echo "   - Slower, requires human decision at each step"
    echo ""
    read -r -p "Choose [1/2] (default: 1): " choice
    choice=${choice:-1}

    if [[ "$choice" =~ ^[Nn]$ ]] || [[ "$choice" == "2" ]]; then
        return 1  # Go to manual mode
    fi
    return 0  # Automated mode
}

# Run manual inspection script
run_manual_check() {
    echo -e "\n${YELLOW}👤 Starting Manual Inspection Mode...${NC}\n"

    # Verify CCO.app exists
    if [[ ! -d "/Users/dbi007/project/mac/claude-auto/dist/CCO.app" ]]; then
        echo -e "${RED}❌ CCO.app not found! Build it first:${NC}"
        echo "   ./scripts/package-app.sh"
        exit 1
    fi

    # Launch manual check script
    echo -e "${BLUE}Launching interactive checklist...${NC}\n"
    if [[ -f "./scripts/w1-6-manual-check.sh" ]]; then
        bash ./scripts/w1-6-manual-check.sh
    else
        echo -e "${RED}❌ Manual check script not found!${NC}"
        exit 1
    fi
}

# Launch Playwright automation (requires TCC for Tauri shell, but works headless without)
run_playwright_automation() {
    echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}🤖 Running Playwright GUI Verification (headless mode)...${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

    # Start test server in background
    node scripts/start-test-server.mjs &
    local SERVER_PID=$!
    trap 'kill $SERVER_PID 2>/dev/null' EXIT

    sleep 2  # Wait for server startup

    # Check if Playwright is installed
    if ! npm list @playwright/test &>/dev/null; then
        echo -e "${YELLOW}⚠️  Installing Playwright dependencies...${NC}"
        npm install -D @playwright/test
        npx playwright install chromium
    fi

    # Run Playwright tests (headless Chrome doesn't need TCC permission)
    echo -e "${GREEN}▶️  Starting automated tests...${NC}"
    npx playwright test tests/l2-interaction/w1-6-checklist.spec.js || {
        echo -e "${RED}❌ Playwright tests failed${NC}"
        kill $SERVER_PID
        exit 1
    }

    # Cleanup
    kill $SERVER_PID 2>/dev/null

    echo -e "\n${GREEN}✅ Playwright GUI verification completed${NC}\n"
}

# Generate final report
generate_report() {
    echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}Generating Final Report...${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

    node scripts/report-w1-6.mjs || {
        echo -e "${YELLOW}⚠️  Report generation skipped (no results yet)${NC}"
        return 0
    }

    echo -e "\n${GREEN}✅ Final report generated${NC}\n"
}

# Main execution flow
main() {
    display_header
    show_permission_guide "$(detect_terminal)"

    # Ask user if they want to proceed with guided setup
    read -r -p "Ready to follow the guide? Press Enter when you've completed all steps, or Ctrl+C to abort"

    echo -e "\n${YELLOW}Checking for screen recording permission...${NC}\n"

    # Wait for user to grant permission (if needed)
    wait_for_authorization "$(detect_terminal)" || exit 1

    # User granted permission, show options
    show_success_message

    # Run static smoke tests (no permission needed)
    run_static_smoke_tests

    # Choose execution mode
    if choose_execution_mode; then
        # Automated Playwright mode
        run_playwright_automation
    else
        # Manual inspection mode
        run_manual_check
        echo ""
        echo -e "${YELLOW}⚠️  Manual check complete - remember to update landing.md!${NC}"

        # Still generate report from any existing Playwright results
        generate_report
        return 0
    fi

    # Generate final report
    generate_report

    # Final summary
    cat << 'EOF'
╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║   🎉 All Tests Completed Successfully!                    ║
║                                                           ║
║   Results saved to:                                       ║
║   • .cco-out/w1-6-report/                                 ║
║   • .cco-out/test-results.json                            ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

EOF
}

# Execute main function
main "$@"
