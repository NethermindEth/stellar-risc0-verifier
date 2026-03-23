#!/usr/bin/env bash
#
# ╔══════════════════════════════════════════════════════════════════════════════╗
# ║                  RISC Zero Stellar Deployment Management                    ║
# ╚══════════════════════════════════════════════════════════════════════════════╝
#
# Unified management script for the full RISC Zero verifier lifecycle on Stellar.
#
# Usage: ./manage.sh <subcommand> [global-flags] [subcommand-flags]
#
# Global Flags:
#   -n, --network              Network (local|futurenet|testnet|mainnet)
#   -a, --account              Stellar CLI identity alias
#   -c, --config               Path to deployment.toml (default: ./deployment.toml)
#       --rpc-url              Custom Soroban RPC endpoint URL
#       --network-passphrase   Network passphrase (if non-standard)
#   -h, --help                 Show help
#
# Deploy Commands:
#   deploy-router                Deploy timelock + router (router owned by timelock)
#   deploy-verifier              Deploy groth16 verifier + emergency stop wrapper
#   deploy-mock-verifier         Deploy mock verifier (testing only, no estop)
#
# Verifier Management (via timelock):
#   schedule-add-verifier        Schedule adding a verifier to the router
#   execute-add-verifier         Execute a scheduled add-verifier operation
#   schedule-remove-verifier     Schedule removing a verifier from the router
#   execute-remove-verifier      Execute a scheduled remove-verifier operation
#
# Self-Administration (via timelock):
#   schedule-update-delay        Schedule updating the timelock minimum delay
#   execute-update-delay         Execute a scheduled update-delay operation
#   schedule-grant-role          Schedule granting a role on the timelock
#   execute-grant-role           Execute a scheduled grant-role operation
#   schedule-revoke-role         Schedule revoking a role on the timelock
#   execute-revoke-role          Execute a scheduled revoke-role operation
#
# Utility Commands:
#   renounce-role                Renounce a role (direct, no timelock)
#   cancel-operation             Cancel a pending timelock operation
#   activate-estop               Activate the emergency stop on a verifier
#   status                       Show deployment status and on-chain state
#

set -euo pipefail

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Bootstrap                                      │
# └──────────────────────────────────────────────────────────────────────────────┘

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

# Source command files
for cmd_file in "$SCRIPT_DIR"/commands/*.sh; do source "$cmd_file"; done

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/manage.XXXXXX")"
cleanup_tmp_dir() {
    rm -rf "$TMP_DIR"
}
trap cleanup_tmp_dir EXIT

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Help                                           │
# └──────────────────────────────────────────────────────────────────────────────┘

show_help() {
    print_banner
    echo -e "${BOLD_WHITE}USAGE${RESET}"
    echo -e "    ${CYAN}./manage.sh${RESET} <subcommand> [global-flags] [subcommand-flags]"
    echo ""
    echo -e "${BOLD_WHITE}GLOBAL FLAGS${RESET}"
    echo -e "    ${GREEN}-n, --network${RESET} <NETWORK>      Network (local|futurenet|testnet|mainnet)"
    echo -e "    ${GREEN}-a, --account${RESET} <IDENTITY>     Stellar CLI identity alias"
    echo -e "    ${GREEN}-c, --config${RESET}  <PATH>         Path to deployment.toml"
    echo -e "    ${GREEN}--rpc-url${RESET}     <URL>          Custom Soroban RPC endpoint URL"
    echo -e "    ${GREEN}--network-passphrase${RESET} <PASS>  Network passphrase (if non-standard)"
    echo -e "    ${GREEN}-h, --help${RESET}                   Show this help"
    echo ""
    echo -e "${BOLD_WHITE}DEPLOY COMMANDS${RESET}"
    echo -e "    ${CYAN}deploy-router${RESET}                Deploy timelock + router"
    echo -e "    ${CYAN}deploy-verifier${RESET}              Deploy groth16 verifier + emergency stop"
    echo -e "    ${CYAN}deploy-mock-verifier${RESET}         Deploy mock verifier (testing only)"
    echo ""
    echo -e "${BOLD_WHITE}VERIFIER MANAGEMENT${RESET}"
    echo -e "    ${CYAN}schedule-add-verifier${RESET}        Schedule adding a verifier"
    echo -e "    ${CYAN}execute-add-verifier${RESET}         Execute add-verifier operation"
    echo -e "    ${CYAN}schedule-remove-verifier${RESET}     Schedule removing a verifier"
    echo -e "    ${CYAN}execute-remove-verifier${RESET}      Execute remove-verifier operation"
    echo ""
    echo -e "${BOLD_WHITE}SELF-ADMINISTRATION${RESET}"
    echo -e "    ${CYAN}schedule-update-delay${RESET}        Schedule delay update"
    echo -e "    ${CYAN}execute-update-delay${RESET}         Execute delay update"
    echo -e "    ${CYAN}schedule-grant-role${RESET}          Schedule granting a role"
    echo -e "    ${CYAN}execute-grant-role${RESET}           Execute grant-role"
    echo -e "    ${CYAN}schedule-revoke-role${RESET}         Schedule revoking a role"
    echo -e "    ${CYAN}execute-revoke-role${RESET}          Execute revoke-role"
    echo ""
    echo -e "${BOLD_WHITE}UTILITY${RESET}"
    echo -e "    ${CYAN}renounce-role${RESET}                Renounce a role (direct)"
    echo -e "    ${CYAN}cancel-operation${RESET}             Cancel a pending operation"
    echo -e "    ${CYAN}activate-estop${RESET}               Activate emergency stop"
    echo -e "    ${CYAN}status${RESET}                       Show deployment status"
    echo ""
    echo -e "${BOLD_WHITE}EXAMPLES${RESET}"
    echo -e "    ${DIM}# Deploy timelock + router on testnet${RESET}"
    echo -e "    ${CYAN}./manage.sh deploy-router -n testnet -a deployer --min-delay 0${RESET}"
    echo ""
    echo -e "    ${DIM}# Deploy a verifier${RESET}"
    echo -e "    ${CYAN}./manage.sh deploy-verifier -n testnet -a deployer${RESET}"
    echo ""
    echo -e "    ${DIM}# Schedule adding a verifier to the router${RESET}"
    echo -e "    ${CYAN}./manage.sh schedule-add-verifier -n testnet -a deployer --selector abc123de${RESET}"
    echo ""
    echo -e "    ${DIM}# Check deployment status${RESET}"
    echo -e "    ${CYAN}./manage.sh status -n testnet${RESET}"
    echo ""
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Global Arg Parsing                             │
# └──────────────────────────────────────────────────────────────────────────────┘

NETWORK="${NETWORK:-}"
ACCOUNT="${ACCOUNT_NAME:-${IDENTITY_NAME:-}}"
CONFIG_FILE="${DEPLOYMENT_CONFIG:-${PROJECT_ROOT}/deployment.toml}"
RPC_URL="${RPC_URL:-}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-}"
SUBCOMMAND=""
SUBCMD_ARGS=()
SUBCMD_FLAG_VALUE=""

require_next_cli_arg() {
    local flag="$1"
    if [[ $# -lt 2 ]]; then
        fatal "Missing value for ${flag}"
    fi
    if [[ "$2" == --* ]]; then
        fatal "Missing value for ${flag}"
    fi
    printf '%s\n' "$2"
}

take_subcmd_flag_value() {
    local flag="${SUBCMD_ARGS[0]}"
    if [[ ${#SUBCMD_ARGS[@]} -lt 2 ]]; then
        fatal "Missing value for ${flag}"
    fi

    local value="${SUBCMD_ARGS[1]}"
    if [[ "$value" == --* ]]; then
        fatal "Missing value for ${flag}"
    fi
    SUBCMD_ARGS=("${SUBCMD_ARGS[@]:2}")
    SUBCMD_FLAG_VALUE="$value"
}

# Parse subcommand flags where all accepted flags take a value.
#   parse_subcmd_flags <command-name> <flag> <var-name> [<flag> <var-name> ...]
parse_subcmd_flags() {
    local command_name="$1"
    shift
    local specs=("$@")

    while [[ ${#SUBCMD_ARGS[@]} -gt 0 ]]; do
        local current="${SUBCMD_ARGS[0]}"
        local matched=0
        local i=0

        while [[ $i -lt ${#specs[@]} ]]; do
            local flag="${specs[i]}"
            local var_name="${specs[i + 1]}"
            if [[ "$current" == "$flag" ]]; then
                take_subcmd_flag_value
                printf -v "$var_name" '%s' "$SUBCMD_FLAG_VALUE"
                matched=1
                break
            fi
            i=$((i + 2))
        done

        if [[ $matched -eq 0 ]]; then
            fatal "Unknown flag for ${command_name}: ${SUBCMD_ARGS[0]}"
        fi
    done
}

# Extract subcommand (first positional arg) and global flags
while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--network)
            NETWORK="$(require_next_cli_arg "$@")"
            shift 2
            ;;
        -a|--account)
            ACCOUNT="$(require_next_cli_arg "$@")"
            shift 2
            ;;
        -c|--config)
            CONFIG_FILE="$(require_next_cli_arg "$@")"
            shift 2
            ;;
        --rpc-url)
            RPC_URL="$(require_next_cli_arg "$@")"
            shift 2
            ;;
        --network-passphrase)
            NETWORK_PASSPHRASE="$(require_next_cli_arg "$@")"
            shift 2
            ;;
        -h|--help)
            if [[ -z "$SUBCOMMAND" ]]; then
                show_help
                exit 0
            else
                SUBCMD_ARGS+=("$1")
                shift
            fi
            ;;
        -*)
            # Pass unknown flags to subcommand
            if [[ -z "$SUBCOMMAND" ]]; then
                echo -e "${RED}Unknown global flag: $1${RESET}"
                echo "Use --help for usage information"
                exit 1
            fi
            SUBCMD_ARGS+=("$1")
            shift
            ;;
        *)
            if [[ -z "$SUBCOMMAND" ]]; then
                SUBCOMMAND="$1"
                shift
            else
                SUBCMD_ARGS+=("$1")
                shift
            fi
            ;;
    esac
done

if [[ -z "$SUBCOMMAND" ]]; then
    show_help
    exit 0
fi

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Setup Helpers                                  │
# └──────────────────────────────────────────────────────────────────────────────┘

# Derive chain key from network
resolve_chain_key() {
    CHAIN_KEY="stellar-$NETWORK"
}

# Common setup for commands that need network + account
setup_environment() {
    print_section "Environment Check"
    require_stellar_cli
    require_python3
    resolve_network
    resolve_account
    resolve_chain_key

    DEPLOYER_ADDRESS=$(stellar keys address "$ACCOUNT" 2>/dev/null)
    DEPLOYER_JSON="\"$DEPLOYER_ADDRESS\""

    success "Network: ${BOLD_MAGENTA}$NETWORK${RESET}"
    if [[ -n "$RPC_URL" ]]; then
        info "RPC URL: ${DIM}$RPC_URL${RESET}"
    fi
    success "Account: ${BOLD_GREEN}$ACCOUNT${RESET}"
    info "Address: ${DIM}$DEPLOYER_ADDRESS${RESET}"
    print_section_end
}

setup_with_config() {
    setup_environment
    load_config
}

setup_with_router() {
    setup_with_config
    require_router
}

# Load timelock and router IDs from config
load_config() {
    if [[ ! -f "$CONFIG_FILE" ]]; then
        fatal "Config file not found: ${DIM}$CONFIG_FILE${RESET}"
    fi

    TIMELOCK_ID=$(config_read "chains.${CHAIN_KEY}.timelock-controller" 2>/dev/null || echo "")
    ROUTER_ID=$(config_read "chains.${CHAIN_KEY}.router" 2>/dev/null || echo "")

    if [[ -z "$TIMELOCK_ID" ]]; then
        fatal "No timelock-controller configured for chain '${CHAIN_KEY}' in $CONFIG_FILE"
    fi
}

require_router() {
    if [[ -z "$ROUTER_ID" ]]; then
        fatal "No router configured for chain '${CHAIN_KEY}'"
    fi
}

resolve_verifier_estop_from_config() {
    local selector="$1"
    config_get_verifier_field "$CHAIN_KEY" "$selector" estop 2>/dev/null || echo ""
}

resolve_verifier_contract_from_config() {
    local selector="$1"
    config_get_verifier_field "$CHAIN_KEY" "$selector" verifier 2>/dev/null || echo ""
}

parse_operation_id_from_file() {
    strip_stellar_quotes < "$1"
}

require_flag() { [[ -n "$2" ]] || fatal "Missing required flag: $1"; }

resolve_delay() {
    local delay="$1"
    if [[ -z "$delay" ]]; then
        delay=$(config_read "chains.${CHAIN_KEY}.timelock-delay" 2>/dev/null || echo "0")
    fi
    echo "$delay"
}

require_verifier_estop() {
    local selector="$1"
    local verifier_estop="$2"
    if [[ -n "$verifier_estop" ]]; then
        echo "$verifier_estop"
        return 0
    fi

    verifier_estop=$(resolve_verifier_estop_from_config "$selector")
    if [[ -n "$verifier_estop" ]]; then
        echo "$verifier_estop"
        return 0
    fi

    verifier_estop=$(resolve_verifier_contract_from_config "$selector")
    if [[ -n "$verifier_estop" ]]; then
        echo "$verifier_estop"
        return 0
    fi

    return 1
}

schedule_operation_and_report() {
    local target="$1"
    local function_name="$2"
    local args_json="$3"
    local delay="$4"
    local salt="$5"
    local output_file="$6"
    schedule_timelock_op "$target" "$function_name" "$args_json" "$ZERO32" "$delay" "$salt" "$output_file"

    local op_id
    op_id=$(parse_operation_id_from_file "$output_file")
    success "Operation scheduled!"
    kv "Operation ID" "$op_id" "$WHITE" "$BOLD_GREEN"
}

prepare_execute_operation() {
    local target="$1"
    local function_name="$2"
    local args_json="$3"
    local predecessor="$4"
    local salt="$5"

    local op_id
    local compute_err_file="${TMP_DIR}/manage_compute_op_id_err.txt"
    if ! op_id=$(compute_operation_id "$target" "$function_name" "$args_json" "$predecessor" "$salt" 2>"$compute_err_file"); then
        error "Failed to compute operation ID!"
        print_output "$(cat "$compute_err_file")"
        if grep -q "Failed to parse argument 'salt'" "$compute_err_file"; then
            warn "Invalid --salt format for Stellar CLI parser."
            info "Use a 32-byte hex value (64 chars) that includes at least one hex letter (a-f)."
            info "Numeric-only salts like 1111... may be rejected by the parser."
            info "Example: ${CYAN}--salt abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890${RESET}"
        fi
        print_section_end
        exit 1
    fi

    info "Operation ID: ${DIM}$op_id${RESET}"
    check_operation_ready "$TIMELOCK_ID" "$op_id"
    mainnet_warning
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Command Dispatch                               │
# └──────────────────────────────────────────────────────────────────────────────┘

# Show top-level help if subcommand received -h/--help
if [[ ${#SUBCMD_ARGS[@]} -gt 0 ]]; then
    for arg in "${SUBCMD_ARGS[@]}"; do
        if [[ "$arg" == "-h" || "$arg" == "--help" ]]; then
            show_help
            exit 0
        fi
    done
fi

print_banner

case "$SUBCOMMAND" in
    # Deploy
    deploy-router)             cmd_deploy_router ;;
    deploy-verifier)           cmd_deploy_verifier ;;
    deploy-mock-verifier)      cmd_deploy_mock_verifier ;;

    # Verifier Management
    schedule-add-verifier)     cmd_schedule_add_verifier ;;
    execute-add-verifier)      cmd_execute_add_verifier ;;
    schedule-remove-verifier)  cmd_schedule_remove_verifier ;;
    execute-remove-verifier)   cmd_execute_remove_verifier ;;

    # Self-Administration
    schedule-update-delay)     cmd_schedule_update_delay ;;
    execute-update-delay)      cmd_execute_update_delay ;;
    schedule-grant-role)       cmd_schedule_grant_role ;;
    execute-grant-role)        cmd_execute_grant_role ;;
    schedule-revoke-role)      cmd_schedule_revoke_role ;;
    execute-revoke-role)       cmd_execute_revoke_role ;;

    # Utility
    renounce-role)             cmd_renounce_role ;;
    cancel-operation)          cmd_cancel_operation ;;
    activate-estop)            cmd_activate_estop ;;
    status)                    cmd_status ;;

    *)
        echo -e "${RED}Unknown subcommand: $SUBCOMMAND${RESET}"
        echo ""
        echo "Run './manage.sh --help' for usage information."
        exit 1
        ;;
esac
