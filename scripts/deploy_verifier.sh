#!/usr/bin/env bash
#
# ╔══════════════════════════════════════════════════════════════════════════════╗
# ║                    RISC Zero Groth16 Verifier Deployment                     ║
# ║                          Stellar Smart Contract                              ║
# ╚══════════════════════════════════════════════════════════════════════════════╝
#
# Deploy the Groth16 verifier contract to Stellar networks.
#
# Usage: ./deploy_verifier.sh [OPTIONS]
#
# Options:
#   -n, --network     Network to deploy (local|futurenet|testnet|mainnet)
#   -a, --account     Account identity alias configured in Stellar CLI
#   -h, --help        Show this help message
#
# Environment Variables:
#   NETWORK              Same as --network
#   ACCOUNT_NAME         Same as --account
#

set -euo pipefail

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Color Definitions                               │
# └──────────────────────────────────────────────────────────────────────────────┘

readonly RESET='\033[0m'
readonly BOLD='\033[1m'
readonly DIM='\033[2m'

# Colors
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[0;33m'
readonly BLUE='\033[0;34m'
readonly MAGENTA='\033[0;35m'
readonly CYAN='\033[0;36m'
readonly WHITE='\033[0;37m'

# Bold colors
readonly BOLD_RED='\033[1;31m'
readonly BOLD_GREEN='\033[1;32m'
readonly BOLD_YELLOW='\033[1;33m'
readonly BOLD_BLUE='\033[1;34m'
readonly BOLD_MAGENTA='\033[1;35m'
readonly BOLD_CYAN='\033[1;36m'
readonly BOLD_WHITE='\033[1;37m'

# Background colors
readonly BG_BLUE='\033[44m'
readonly BG_MAGENTA='\033[45m'

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Helper Functions                                │
# └──────────────────────────────────────────────────────────────────────────────┘

print_banner() {
    echo -e "${BOLD_CYAN}"
    cat << 'EOF'
    ╭─────────────────────────────────────────────────────────────────────╮
    │                                                                     │
    │   ██████╗ ██╗███████╗ ██████╗    ███████╗███████╗██████╗  ██████╗   │
    │   ██╔══██╗██║██╔════╝██╔════╝    ╚══███╔╝██╔════╝██╔══██╗██╔═══██╗  │
    │   ██████╔╝██║███████╗██║           ███╔╝ █████╗  ██████╔╝██║   ██║  │
    │   ██╔══██╗██║╚════██║██║          ███╔╝  ██╔══╝  ██╔══██╗██║   ██║  │
    │   ██║  ██║██║███████║╚██████╗    ███████╗███████╗██║  ██║╚██████╔╝  │
    │   ╚═╝  ╚═╝╚═╝╚══════╝ ╚═════╝    ╚══════╝╚══════╝╚═╝  ╚═╝ ╚═════╝   │
    │                                                                     │
    │              Groth16 Verifier • Stellar Deployment                  │
    │                                                                     │
    ╰─────────────────────────────────────────────────────────────────────╯
EOF
    echo -e "${RESET}"
}

print_section() {
    local title="$1"
    local width=70
    local padding=$(( (width - ${#title} - 2) / 2 ))
    local pad_left=$(printf '%*s' "$padding" '' | tr ' ' '─')
    local pad_right=$(printf '%*s' "$((width - ${#title} - 2 - padding))" '' | tr ' ' '─')

    echo ""
    echo -e "${BOLD_BLUE}┌${pad_left} ${BOLD_WHITE}${title} ${BOLD_BLUE}${pad_right}┐${RESET}"
}

print_section_end() {
    echo -e "${BOLD_BLUE}└──────────────────────────────────────────────────────────────────────┘${RESET}"
}

info() {
    echo -e "${BOLD_BLUE}│${RESET} ${CYAN}ℹ${RESET}  $1"
}

success() {
    echo -e "${BOLD_BLUE}│${RESET} ${GREEN}✓${RESET}  $1"
}

warn() {
    echo -e "${BOLD_BLUE}│${RESET} ${YELLOW}⚠${RESET}  $1"
}

error() {
    echo -e "${BOLD_BLUE}│${RESET} ${RED}✗${RESET}  $1"
}

kv() {
    local key="$1"
    local value="$2"
    local key_color="${3:-$DIM}"
    local value_color="${4:-$WHITE}"
    printf "${BOLD_BLUE}│${RESET}    ${key_color}%-22s${RESET} ${value_color}%s${RESET}\n" "$key:" "$value"
}

print_divider() {
    echo -e "${BOLD_BLUE}│${RESET}    ${DIM}────────────────────────────────────────────────────────────${RESET}"
}

# Spinner for long-running operations
spinner() {
    local pid=$1
    local message=$2
    local spin='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    local i=0

    while kill -0 "$pid" 2>/dev/null; do
        local char="${spin:i++%${#spin}:1}"
        printf "\r\033[K${BOLD_BLUE}│${RESET} ${MAGENTA}%s${RESET}  %s" "$char" "$message"
        sleep 0.1
    done
    # Clear the entire line before returning
    printf "\r\033[K"
}

# Print command output with box formatting
print_output() {
    local output="$1"
    while IFS= read -r line; do
        echo -e "${BOLD_BLUE}│${RESET}    ${DIM}${line}${RESET}"
    done <<< "$output"
}

fatal() {
    error "$1"
    print_section_end
    exit 1
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Network Validation                              │
# └──────────────────────────────────────────────────────────────────────────────┘

# Valid networks supported by stellar CLI
readonly VALID_NETWORKS="local futurenet testnet mainnet"

is_valid_network() {
    local network="$1"
    [[ " $VALID_NETWORKS " == *" $network "* ]]
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Argument Parsing                                │
# └──────────────────────────────────────────────────────────────────────────────┘

show_help() {
    print_banner
    echo -e "${BOLD_WHITE}USAGE${RESET}"
    echo -e "    ${CYAN}./deploy_verifier.sh${RESET} [OPTIONS]"
    echo ""
    echo -e "${BOLD_WHITE}OPTIONS${RESET}"
    echo -e "    ${GREEN}-n, --network${RESET} <NETWORK>    Network to deploy to"
    echo -e "                              ${DIM}(local, futurenet, testnet, mainnet)${RESET}"
    echo ""
    echo -e "    ${GREEN}-a, --account${RESET} <IDENTITY>   Account identity alias from Stellar CLI"
    echo -e "                              ${DIM}(configured via 'stellar keys generate')${RESET}"
    echo ""
    echo -e "    ${GREEN}-h, --help${RESET}                 Show this help message"
    echo ""
    echo -e "${BOLD_WHITE}ENVIRONMENT VARIABLES${RESET}"
    echo -e "    ${YELLOW}NETWORK${RESET}              Override network selection"
    echo -e "    ${YELLOW}ACCOUNT_NAME${RESET}         Override account identity"
    echo ""
    echo -e "${BOLD_WHITE}EXAMPLES${RESET}"
    echo -e "    ${DIM}# Deploy to testnet with identity 'deployer'${RESET}"
    echo -e "    ${CYAN}./deploy_verifier.sh -n testnet -a deployer${RESET}"
    echo ""
    echo -e "    ${DIM}# Deploy to local network${RESET}"
    echo -e "    ${CYAN}./deploy_verifier.sh --network local --account alice${RESET}"
    echo ""
    echo -e "    ${DIM}# Using environment variables${RESET}"
    echo -e "    ${CYAN}NETWORK=futurenet ACCOUNT_NAME=mykey ./deploy_verifier.sh${RESET}"
    echo ""
}

# Default values
NETWORK="${NETWORK:-}"
ACCOUNT="${ACCOUNT_NAME:-${IDENTITY_NAME:-}}"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--network)
            NETWORK="$2"
            shift 2
            ;;
        -a|--account)
            ACCOUNT="$2"
            shift 2
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${RESET}"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done


# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Main Script                                     │
# └──────────────────────────────────────────────────────────────────────────────┘

main() {
    print_banner

    # ── Check CLI ────────────────────────────────────────────────────────────
    print_section "Environment Check"

    if ! command -v stellar &>/dev/null; then
        fatal "Stellar CLI not found. Install with: ${CYAN}cargo install stellar-cli --locked${RESET}"
    fi
    success "Stellar CLI installed"

    # ── Validate Network ─────────────────────────────────────────────────────
    if [[ -z "$NETWORK" ]]; then
        print_section_end
        echo ""
        echo -e "${BOLD_WHITE}Select a network to deploy:${RESET}"
        echo ""
        echo -e "    ${CYAN}1)${RESET} local      ${DIM}─ Local standalone network${RESET}"
        echo -e "    ${CYAN}2)${RESET} futurenet  ${DIM}─ Stellar Futurenet (experimental)${RESET}"
        echo -e "    ${CYAN}3)${RESET} testnet    ${DIM}─ Stellar Testnet${RESET}"
        echo -e "    ${CYAN}4)${RESET} mainnet    ${DIM}─ Stellar Mainnet (production)${RESET}"
        echo ""
        read -rp "$(echo -e "${BOLD_WHITE}Enter choice [1-4]: ${RESET}")" choice

        case "$choice" in
            1|local) NETWORK="local" ;;
            2|futurenet) NETWORK="futurenet" ;;
            3|testnet) NETWORK="testnet" ;;
            4|mainnet) NETWORK="mainnet" ;;
            *) echo -e "${RED}Invalid choice${RESET}"; exit 1 ;;
        esac

        print_section "Environment Check (continued)"
    fi

    if ! is_valid_network "$NETWORK"; then
        fatal "Invalid network: ${BOLD_WHITE}$NETWORK${RESET}. Use: local, futurenet, testnet, or mainnet"
    fi

    success "Network: ${BOLD_MAGENTA}$NETWORK${RESET}"

    # ── Validate Account ─────────────────────────────────────────────────────
    if [[ -z "$ACCOUNT" ]]; then
        print_section_end
        echo ""
        echo -e "${BOLD_WHITE}Available identities:${RESET}"
        echo ""

        # List available identities
        IDENTITIES=$(stellar keys ls 2>/dev/null || echo "")
        if [[ -n "$IDENTITIES" ]]; then
            echo -e "${DIM}$IDENTITIES${RESET}" | sed 's/^/    /'
        else
            echo -e "    ${DIM}No identities found. Create one with:${RESET}"
            echo -e "    ${CYAN}stellar keys generate <name> --network $NETWORK${RESET}"
        fi
        echo ""
        read -rp "$(echo -e "${BOLD_WHITE}Enter account identity alias: ${RESET}")" ACCOUNT

        if [[ -z "$ACCOUNT" ]]; then
            echo -e "${RED}Account identity is required${RESET}"
            exit 1
        fi

        print_section "Environment Check (continued)"
    fi

    # Verify identity exists
    if ! stellar keys address "$ACCOUNT" &>/dev/null; then
        fatal "Identity '${BOLD_WHITE}$ACCOUNT${RESET}' not found. Create it with: ${CYAN}stellar keys generate $ACCOUNT --network $NETWORK${RESET}"
    fi

    DEPLOYER_ADDRESS=$(stellar keys address "$ACCOUNT" 2>/dev/null)
    success "Account: ${BOLD_GREEN}$ACCOUNT${RESET}"
    info "Address: ${DIM}$DEPLOYER_ADDRESS${RESET}"

    success "Contract: ${BOLD_YELLOW}groth16-verifier${RESET}"

    print_section_end

    # ── Change to project root ───────────────────────────────────────────────
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
    cd "$PROJECT_ROOT"

    # ── Build Contract ───────────────────────────────────────────────────────
    print_section "Building Contract"

    # Build and optimize contract with spinner
    stellar contract build --optimize > /tmp/build_output.txt 2>&1 &
    local build_pid=$!
    spinner $build_pid "Building and optimizing groth16-verifier..."
    wait $build_pid
    local build_status=$?
    BUILD_OUTPUT=$(cat /tmp/build_output.txt)

    if [[ $build_status -ne 0 ]]; then
        error "Build failed!"
        print_output "$BUILD_OUTPUT"
        print_section_end
        exit 1
    fi
    success "Build completed!"
    # Print build output
    if [[ -n "$BUILD_OUTPUT" ]]; then
        print_output "$BUILD_OUTPUT"
    fi

    print_section_end

    # ── Display Verifier Parameters ──────────────────────────────────────────
    print_section "Verifier Parameters"

    # Extract parameters from build output
    SELECTOR=$(echo "$BUILD_OUTPUT" | grep -o 'SELECTOR:[[:space:]]*[a-f0-9]*' | awk '{print $2}' || echo "N/A")
    CONTROL_ROOT=$(echo "$BUILD_OUTPUT" | grep -o 'CONTROL_ROOT:[[:space:]]*[a-f0-9]*' | head -1 | awk '{print $2}' || echo "N/A")
    CONTROL_ROOT_0=$(echo "$BUILD_OUTPUT" | grep -o 'CONTROL_ROOT_0:[[:space:]]*[a-f0-9]*' | awk '{print $2}' || echo "N/A")
    CONTROL_ROOT_1=$(echo "$BUILD_OUTPUT" | grep -o 'CONTROL_ROOT_1:[[:space:]]*[a-f0-9]*' | awk '{print $2}' || echo "N/A")
    BN254_CONTROL_ID=$(echo "$BUILD_OUTPUT" | grep -o 'BN254_CONTROL_ID:[[:space:]]*[a-f0-9]*' | awk '{print $2}' || echo "N/A")
    VK_DIGEST=$(echo "$BUILD_OUTPUT" | grep -o 'VERIFIER_KEY_DIGEST:[[:space:]]*[a-f0-9]*' | awk '{print $2}' || echo "N/A")
    VERSION=$(echo "$BUILD_OUTPUT" | grep -o 'VERSION:[[:space:]]*[0-9.]*' | awk '{print $2}' || echo "N/A")

    # If extraction failed, try reading from parameters.json directly
    if [[ "$SELECTOR" == "N/A" || -z "$SELECTOR" ]]; then
        warn "Could not extract parameters from build output"
        info "Reading from parameters.json..."

        if [[ -f "contracts/groth16-verifier/parameters.json" ]]; then
            VERSION=$(jq -r '.version // "N/A"' "contracts/groth16-verifier/parameters.json")
            CONTROL_ROOT=$(jq -r '.control_root // "N/A"' "contracts/groth16-verifier/parameters.json")
            BN254_CONTROL_ID=$(jq -r '.bn254_control_id // "N/A"' "contracts/groth16-verifier/parameters.json")
            SELECTOR="${DIM}(computed at build time)${RESET}"
            VK_DIGEST="${DIM}(computed at build time)${RESET}"
        fi
    fi

    echo -e "${BOLD_BLUE}│${RESET}"
    kv "VERSION" "$VERSION" "$CYAN" "$BOLD_WHITE"
    kv "SELECTOR" "$SELECTOR" "$CYAN" "$BOLD_YELLOW"
    print_divider
    kv "CONTROL_ROOT" "" "$CYAN" "$WHITE"
    echo -e "${BOLD_BLUE}│${RESET}    ${DIM}Full:${RESET}  ${WHITE}$CONTROL_ROOT${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}    ${DIM}Part 0:${RESET} ${WHITE}$CONTROL_ROOT_0${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}    ${DIM}Part 1:${RESET} ${WHITE}$CONTROL_ROOT_1${RESET}"
    print_divider
    kv "BN254_CONTROL_ID" "" "$CYAN" "$WHITE"
    echo -e "${BOLD_BLUE}│${RESET}    ${WHITE}$BN254_CONTROL_ID${RESET}"
    print_divider
    kv "VERIFIER_KEY_DIGEST" "" "$CYAN" "$WHITE"
    echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_GREEN}$VK_DIGEST${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}"

    print_section_end

    # ── Locate WASM File ─────────────────────────────────────────────────────
    print_section "Deployment"

    # With --optimize, the CLI outputs to .optimized.wasm
    WASM_PATH="target/wasm32v1-none/release/groth16_verifier.optimized.wasm"

    # Fallback to non-optimized if optimized doesn't exist
    if [[ ! -f "$WASM_PATH" ]]; then
        WASM_PATH="target/wasm32v1-none/release/groth16_verifier.wasm"
    fi

    if [[ ! -f "$WASM_PATH" ]]; then
        fatal "WASM file not found at: ${DIM}$WASM_PATH${RESET}"
    fi

    WASM_SIZE=$(du -h "$WASM_PATH" | cut -f1)
    info "WASM file: ${DIM}$WASM_PATH${RESET}"
    info "WASM size: ${DIM}$WASM_SIZE${RESET}"

    # ── Mainnet Warning ──────────────────────────────────────────────────────
    if [[ "$NETWORK" == "mainnet" ]]; then
        echo -e "${BOLD_BLUE}│${RESET}"
        echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_RED}⚠️  MAINNET DEPLOYMENT WARNING ⚠️${RESET}"
        echo -e "${BOLD_BLUE}│${RESET}    ${YELLOW}You are about to deploy to MAINNET.${RESET}"
        echo -e "${BOLD_BLUE}│${RESET}    ${YELLOW}This will use real XLM for transaction fees.${RESET}"
        echo -e "${BOLD_BLUE}│${RESET}"
        read -rp "$(echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_WHITE}Type 'DEPLOY' to confirm: ${RESET}")" confirm
        if [[ "$confirm" != "DEPLOY" ]]; then
            warn "Deployment cancelled"
            print_section_end
            exit 0
        fi
    fi

    # ── Deploy Contract ──────────────────────────────────────────────────────
    stellar contract deploy \
        --wasm "$WASM_PATH" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        --alias groth16-verifier \
        > /tmp/deploy_output.txt 2>&1 &
    local deploy_pid=$!
    spinner $deploy_pid "Deploying to $NETWORK..."
    wait $deploy_pid
    local deploy_status=$?
    DEPLOY_OUTPUT=$(cat /tmp/deploy_output.txt)

    if [[ $deploy_status -ne 0 ]]; then
        error "Deployment failed!"
        print_output "$DEPLOY_OUTPUT"
        print_section_end
        exit 1
    fi

    CONTRACT_ID=$(echo "$DEPLOY_OUTPUT" | tail -1)

    success "Deployed successfully!"
    print_output "$DEPLOY_OUTPUT"
    print_section_end

    # ── Summary ──────────────────────────────────────────────────────────────
    print_section "Deployment Summary"

    echo -e "${BOLD_BLUE}│${RESET}"
    kv "Contract" "groth16-verifier" "$WHITE" "$BOLD_CYAN"
    kv "Network" "$NETWORK" "$WHITE" "$BOLD_MAGENTA"
    kv "Deployer" "$ACCOUNT" "$WHITE" "$BOLD_GREEN"
    print_divider
    echo -e "${BOLD_BLUE}│${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_WHITE}CONTRACT ID:${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_GREEN}$CONTRACT_ID${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}"

    # Network-specific explorer link
    case "$NETWORK" in
        testnet)
            EXPLORER_URL="https://stellar.expert/explorer/testnet/contract/$CONTRACT_ID"
            info "Explorer: ${CYAN}$EXPLORER_URL${RESET}"
            ;;
        mainnet)
            EXPLORER_URL="https://stellar.expert/explorer/public/contract/$CONTRACT_ID"
            info "Explorer: ${CYAN}$EXPLORER_URL${RESET}"
            ;;
        futurenet)
            EXPLORER_URL="https://stellar.expert/explorer/futurenet/contract/$CONTRACT_ID"
            info "Explorer: ${CYAN}$EXPLORER_URL${RESET}"
            ;;
    esac

    echo -e "${BOLD_BLUE}│${RESET}"
    print_section_end

    # ── Final Banner ─────────────────────────────────────────────────────────
    echo ""
    echo -e "${BOLD_GREEN}    ✨ Deployment Complete! ✨${RESET}"
    echo ""
}

# Run main function
main "$@"

