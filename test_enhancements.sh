#!/usr/bin/env bash
# Test script for nh enhancements
# This script tests various features of the enhanced nh tool

set -e  # Exit on error
YELLOW='\033[1;33m'
GREEN='\033[1;32m'
RED='\033[1;31m'
BLUE='\033[1;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== NH Enhancement Test Script ===${NC}"
echo

# Function to run a test and report result
run_test() {
    local test_name="$1"
    local command="$2"
    
    echo -e "${YELLOW}Testing: ${test_name}${NC}"
    echo -e "Command: ${command}"
    
    if eval "$command"; then
        echo -e "${GREEN}✓ Test passed${NC}"
    else
        echo -e "${RED}❌ Test failed${NC}"
        return 1
    fi
    echo
}

# Test 1: Basic help command
run_test "Basic help command" "cargo run --bin nh -- --help"

# Test 2: Verbosity levels
run_test "Verbosity level 1" "cargo run --bin nh -- -v --help | grep -q 'verbose'"
run_test "Verbosity level 2" "cargo run --bin nh -- -vv --help | grep -q 'verbose'"

# Test 3: UI styling with test program
run_test "UI styling test" "cargo run --bin test_enhanced_ui"

# Test 4: Git check (if in a git repo)
if [ -d ".git" ]; then
    # Create a temporary untracked .nix file
    echo "{ }" > untracked_test.nix
    run_test "Git check warning" "cargo run --bin nh -- -v os --help | grep -q 'Git Warning'"
    # Clean up
    rm -f untracked_test.nix
else
    echo -e "${YELLOW}Skipping Git check test (not a git repository)${NC}"
    echo
fi

# Test 5: Parse check with syntax error
echo "{ foo = " > syntax_error_test.nix
run_test "Parse check with syntax error" "cargo run --bin nh -- -v os --dry 2>&1 | grep -q 'syntax error'"
# Clean up
rm -f syntax_error_test.nix

# Test 6: Lint check (if formatters are available)
if command -v nixpkgs-fmt >/dev/null || command -v alejandra >/dev/null; then
    # Create a poorly formatted .nix file
    echo "{ foo=1;bar = 2;   baz=3; }" > format_test.nix
    run_test "Lint check" "cargo run --bin nh -- -v os --dry 2>&1 | grep -q 'Lint'"
    # Clean up
    rm -f format_test.nix
else
    echo -e "${YELLOW}Skipping lint check test (no formatters available)${NC}"
    echo
fi

# Test 7: Progress indicators
run_test "Progress indicators" "cargo run --bin nh -- -v os --dry 2>&1 | grep -q '✓'"

# Test 8: Error reporting
echo "{ this = is_invalid syntax }" > error_test.nix
run_test "Error reporting" "cargo run --bin nh -- -v os --dry 2>&1 | grep -q 'COMMAND ABORTED' || true"
# Clean up
rm -f error_test.nix

echo -e "${BLUE}=== All tests completed ===${NC}"