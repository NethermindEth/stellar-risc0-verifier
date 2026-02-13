#!/usr/bin/env bash
#
# Shared library for RISC Zero Stellar deployment management scripts.
#
# Source this file: source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
#

# Guard against double-sourcing
[[ -n "${_LIB_SH_LOADED:-}" ]] && return 0
readonly _LIB_SH_LOADED=1

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Constants                                      │
# └──────────────────────────────────────────────────────────────────────────────┘

readonly ZERO32="0000000000000000000000000000000000000000000000000000000000000000"
readonly WASM_DIR="target/wasm32v1-none/release"

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Color Definitions                              │
# └──────────────────────────────────────────────────────────────────────────────┘

readonly RESET='\033[0m'
readonly DIM='\033[2m'

readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[0;33m'
readonly MAGENTA='\033[0;35m'
readonly CYAN='\033[0;36m'
readonly WHITE='\033[0;37m'

readonly BOLD_RED='\033[1;31m'
readonly BOLD_GREEN='\033[1;32m'
readonly BOLD_YELLOW='\033[1;33m'
readonly BOLD_BLUE='\033[1;34m'
readonly BOLD_MAGENTA='\033[1;35m'
readonly BOLD_CYAN='\033[1;36m'
readonly BOLD_WHITE='\033[1;37m'


# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Display Functions                              │
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
    │              Stellar Deployment Management                          │
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
    printf "\r\033[K"

    wait "$pid"
}

# Run a stellar command in the background with a spinner, exiting on failure.
#   run_stellar_op <output_file> <spinner_msg> <error_msg> <cmd...>
run_stellar_op() {
    local output_file="$1"
    local spinner_msg="$2"
    local error_msg="$3"
    shift 3

    "$@" > "$output_file" 2>&1 &
    local pid=$!
    local status=0
    spinner "$pid" "$spinner_msg" || status=$?

    if [[ $status -ne 0 ]]; then
        error "$error_msg"
        print_output "$(cat "$output_file")"
        print_section_end
        exit 1
    fi
}

# Execute a self-admin timelock operation with fallback to execute_op.
#   execute_self_admin_op <output_base> <spinner_msg> <function_name> \
#       <args_json> <predecessor> <salt> <direct_call_args...>
#
# Tries calling the function directly on the timelock first. If that fails,
# falls back to execute_op. Uses globals: TIMELOCK_ID, ACCOUNT, NETWORK,
# DEPLOYER_ADDRESS, TMP_DIR.
execute_self_admin_op() {
    local output_base="$1"
    local spinner_msg="$2"
    local function_name="$3"
    local args_json="$4"
    local predecessor="$5"
    local salt="$6"
    shift 6

    stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        "$@" \
        > "${TMP_DIR}/${output_base}.txt" 2>&1 &
    local pid=$!
    local status=0
    spinner "$pid" "$spinner_msg" || status=$?

    if [[ $status -eq 0 ]]; then
        return 0
    fi

    error "Direct execution failed!"
    print_output "$(cat "${TMP_DIR}/${output_base}.txt")"
    warn "Falling back to execute_op..."

    stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        execute_op \
        --target "$TIMELOCK_ID" \
        --function "$function_name" \
        --args "$args_json" \
        --predecessor "$predecessor" \
        --salt "$salt" \
        --executor "$DEPLOYER_JSON" \
        > "${TMP_DIR}/${output_base}_fallback.txt" 2>&1 &
    pid=$!
    status=0
    spinner "$pid" "Executing via execute_op fallback..." || status=$?

    if [[ $status -ne 0 ]]; then
        error "Fallback execution also failed!"
        print_output "$(cat "${TMP_DIR}/${output_base}_fallback.txt")"
        print_section_end
        exit 1
    fi
}

# Execute a self-admin timelock operation using an explicit auth-entry flow.
#   execute_self_admin_op_auth <output_base> <spinner_msg> <predecessor> <salt> <direct_call_args...>
#
# Builds a transaction envelope for a direct timelock self-call, simulates it to
# attach Soroban auth/resource data, injects OperationMeta custom-account
# signature data, signs it with the source account, and sends it to the network.
execute_self_admin_op_auth() {
    local output_base="$1"
    local spinner_msg="$2"
    local predecessor="$3"
    local salt="$4"
    shift 4

    local build_file="${TMP_DIR}/${output_base}_build.txt"
    local simulate_file="${TMP_DIR}/${output_base}_simulate.txt"
    local resimulate_file="${TMP_DIR}/${output_base}_resimulate.txt"
    local decode_file="${TMP_DIR}/${output_base}_decode.json"
    local patched_json_file="${TMP_DIR}/${output_base}_patched.json"
    local patch_log_file="${TMP_DIR}/${output_base}_patch.log"
    local encode_file="${TMP_DIR}/${output_base}_encode.txt"
    local sign_file="${TMP_DIR}/${output_base}_sign.txt"
    local send_file="${TMP_DIR}/${output_base}_send.txt"

    run_stellar_op "$build_file" \
        "Building self-admin transaction..." \
        "Failed to build self-admin transaction!" \
        stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        --build-only \
        -- \
        "$@"

    local build_xdr
    build_xdr=$(tail -n 1 "$build_file")
    if [[ -z "$build_xdr" ]]; then
        fatal "Failed to capture built transaction envelope"
    fi

    local latest_ledger
    latest_ledger=$(
        stellar network health --network "$NETWORK" 2>&1 \
            | sed -nE 's/.*Latest ledger: ([0-9]+).*/\1/p' \
            | tail -n 1
    )
    if [[ -z "$latest_ledger" ]]; then
        fatal "Failed to determine latest ledger for self-admin signature expiration"
    fi

    local signature_ledger_buffer="${SELF_ADMIN_SIGNATURE_LEDGER_BUFFER:-1000}"
    if [[ ! "$signature_ledger_buffer" =~ ^[0-9]+$ ]]; then
        fatal "Invalid SELF_ADMIN_SIGNATURE_LEDGER_BUFFER '${signature_ledger_buffer}' (must be a non-negative integer)"
    fi

    local signature_expiration_ledger=$((latest_ledger + signature_ledger_buffer))

    # Retry with larger instruction leeway if the network reports ResourceLimitExceeded.
    local -a leeway_attempts=(
        "${SELF_ADMIN_INSTRUCTION_LEEWAY:-5000000}"
        "${SELF_ADMIN_MAX_INSTRUCTION_LEEWAY:-50000000}"
    )

    local attempt=0
    local send_status=0
    while [[ $attempt -lt ${#leeway_attempts[@]} ]]; do
        local leeway="${leeway_attempts[$attempt]}"
        local simulate_msg="Simulating self-admin transaction..."
        local -a simulate_cmd=(
            stellar tx simulate
            --network "$NETWORK"
            --source-account "$ACCOUNT"
        )
        if [[ "$leeway" != "0" ]]; then
            if [[ $attempt -eq 0 ]]; then
                simulate_msg="Simulating self-admin transaction (instruction leeway: ${leeway})..."
            else
                simulate_msg="Re-simulating self-admin transaction (instruction leeway: ${leeway})..."
            fi
            simulate_cmd+=(--instruction-leeway "$leeway")
        fi

        run_stellar_op "$simulate_file" \
            "$simulate_msg" \
            "Self-admin simulation failed!" \
            "${simulate_cmd[@]}" \
            "$build_xdr"

        local simulated_xdr
        simulated_xdr=$(tail -n 1 "$simulate_file")
        if [[ -z "$simulated_xdr" ]]; then
            fatal "Failed to capture simulated transaction envelope"
        fi

        run_stellar_op "$decode_file" \
            "Decoding self-admin transaction..." \
            "Failed to decode self-admin transaction!" \
            stellar tx decode \
            --output json \
            "$simulated_xdr"

        if ! python3 - "$decode_file" "$TIMELOCK_ID" "$DEPLOYER_ADDRESS" "$predecessor" "$salt" "$signature_expiration_ledger" \
            > "$patched_json_file" 2> "$patch_log_file" <<'PY'
import json
import sys

decode_path, timelock_id, executor_addr, predecessor, salt, sig_exp_ledger = sys.argv[1:]
sig_exp_ledger = int(sig_exp_ledger)

with open(decode_path, "r", encoding="utf-8") as infile:
    tx = json.load(infile)

op = tx["tx"]["tx"]["operations"][0]["body"]["invoke_host_function"]
invoke_contract = op["host_function"]["invoke_contract"]
fn_name = invoke_contract["function_name"]
fn_args = invoke_contract["args"]
auth = op.get("auth", [])
if not auth:
    raise ValueError("missing Soroban auth entries")

address_credentials = auth[0].get("credentials", {}).get("address")
if address_credentials is None:
    raise ValueError("first auth entry is not address credentials")

address_credentials["signature_expiration_ledger"] = sig_exp_ledger
address_credentials["signature"] = {
    "vec": [
        {
            "map": [
                {"key": {"symbol": "executor"}, "val": {"address": executor_addr}},
                {"key": {"symbol": "predecessor"}, "val": {"bytes": predecessor}},
                {"key": {"symbol": "salt"}, "val": {"bytes": salt}},
            ]
        }
    ]
}

# Ensure executor auth for require_auth_for_args() in __check_auth.
auth = [
    auth_entry
    for auth_entry in auth
    if not (
        auth_entry.get("credentials") == "source_account"
        and auth_entry.get("root_invocation", {})
        .get("function", {})
        .get("contract_fn", {})
        .get("function_name")
        == "__check_auth"
    )
]
auth.append(
    {
        "credentials": "source_account",
        "root_invocation": {
            "function": {
                "contract_fn": {
                    "contract_address": timelock_id,
                    "function_name": "__check_auth",
                    "args": [
                        {"symbol": "execute_op"},
                        {"address": timelock_id},
                        {"symbol": fn_name},
                        {"vec": fn_args},
                        {"bytes": predecessor},
                        {"bytes": salt},
                    ],
                }
            },
            "sub_invocations": [],
        },
    }
)
op["auth"] = auth

json.dump(tx, sys.stdout, separators=(",", ":"))
PY
        then
            error "Failed to inject self-admin auth metadata!"
            print_output "$(cat "$patch_log_file")"
            print_section_end
            exit 1
        fi

        run_stellar_op "$encode_file" \
            "Encoding self-admin transaction..." \
            "Failed to encode self-admin transaction!" \
            stellar tx encode \
            "$patched_json_file"

        local patched_xdr
        patched_xdr=$(tail -n 1 "$encode_file")
        if [[ -z "$patched_xdr" ]]; then
            fatal "Failed to capture patched transaction envelope"
        fi

        run_stellar_op "$resimulate_file" \
            "Re-simulating self-admin transaction with injected auth..." \
            "Self-admin auth re-simulation failed!" \
            "${simulate_cmd[@]}" \
            "$patched_xdr"

        local rebudgeted_xdr
        rebudgeted_xdr=$(tail -n 1 "$resimulate_file")
        if [[ -z "$rebudgeted_xdr" ]]; then
            fatal "Failed to capture re-simulated transaction envelope"
        fi

        run_stellar_op "$sign_file" \
            "Signing self-admin transaction..." \
            "Signing self-admin transaction failed!" \
            stellar tx sign \
            --network "$NETWORK" \
            --sign-with-key "$ACCOUNT" \
            "$rebudgeted_xdr"

        local signed_xdr
        signed_xdr=$(tail -n 1 "$sign_file")
        if [[ -z "$signed_xdr" ]]; then
            fatal "Failed to capture signed transaction envelope"
        fi

        stellar tx send \
            --network "$NETWORK" \
            "$signed_xdr" \
            > "$send_file" 2>&1 &
        local pid=$!
        send_status=0
        spinner "$pid" "$spinner_msg" || send_status=$?
        if [[ $send_status -eq 0 ]]; then
            return 0
        fi

        if ! grep -q "ResourceLimitExceeded" "$send_file"; then
            break
        fi
        if [[ $attempt -ge $((${#leeway_attempts[@]} - 1)) ]]; then
            break
        fi

        local next_leeway="${leeway_attempts[$((attempt + 1))]}"
        warn "Resource limit exceeded; retrying with instruction leeway ${next_leeway}"
        attempt=$((attempt + 1))
    done

    error "Self-admin transaction failed!"
    print_output "$(cat "$send_file")"
    print_section_end
    exit 1
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Validation Functions                           │
# └──────────────────────────────────────────────────────────────────────────────┘

readonly VALID_NETWORKS="local futurenet testnet mainnet"

validate_selector() {
    local selector="$1"
    if [[ ! "$selector" =~ ^[0-9a-fA-F]{8}$ ]]; then
        fatal "Invalid selector '${BOLD_WHITE}$selector${RESET}': must be exactly 8 hex characters"
    fi
}

require_stellar_cli() {
    if ! command -v stellar &>/dev/null; then
        fatal "Stellar CLI not found. Install with: ${CYAN}cargo install stellar-cli --locked${RESET}"
    fi
    success "Stellar CLI installed"
}

require_python3() {
    if ! command -v python3 &>/dev/null; then
        fatal "Python 3 not found. Required for config management."
    fi
}

is_valid_network() {
    local network="$1"
    [[ " $VALID_NETWORKS " == *" $network "* ]]
}

resolve_network() {
    if [[ -n "${NETWORK:-}" ]]; then
        if ! is_valid_network "$NETWORK"; then
            fatal "Invalid network: ${BOLD_WHITE}$NETWORK${RESET}. Use: local, futurenet, testnet, or mainnet"
        fi
        return
    fi

    print_section_end
    echo ""
    echo -e "${BOLD_WHITE}Select a network:${RESET}"
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
}

resolve_account() {
    if [[ -n "${ACCOUNT:-}" ]]; then
        if ! stellar keys address "$ACCOUNT" &>/dev/null; then
            fatal "Identity '${BOLD_WHITE}$ACCOUNT${RESET}' not found. Create with: ${CYAN}stellar keys generate $ACCOUNT --network $NETWORK${RESET}"
        fi
        return
    fi

    print_section_end
    echo ""
    echo -e "${BOLD_WHITE}Available identities:${RESET}"
    echo ""

    local identities
    identities=$(stellar keys ls 2>/dev/null || echo "")
    if [[ -n "$identities" ]]; then
        echo -e "${DIM}$identities${RESET}" | sed 's/^/    /'
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

    if ! stellar keys address "$ACCOUNT" &>/dev/null; then
        fatal "Identity '${BOLD_WHITE}$ACCOUNT${RESET}' not found."
    fi

    print_section "Environment Check (continued)"
}

mainnet_warning() {
    if [[ "$NETWORK" != "mainnet" ]]; then
        return
    fi

    echo -e "${BOLD_BLUE}│${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_RED}⚠️  MAINNET WARNING ⚠️${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}    ${YELLOW}You are about to execute on MAINNET.${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}    ${YELLOW}This will use real XLM for transaction fees.${RESET}"
    echo -e "${BOLD_BLUE}│${RESET}"
    read -rp "$(echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_WHITE}Type 'CONFIRM' to proceed: ${RESET}")" confirm
    if [[ "$confirm" != "CONFIRM" ]]; then
        warn "Cancelled"
        print_section_end
        exit 0
    fi
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              WASM Functions                                 │
# └──────────────────────────────────────────────────────────────────────────────┘

build_contracts() {
    print_section "Building Contracts"

    cd "$PROJECT_ROOT"

    local build_output_file="${TMP_DIR:-/tmp}/manage_build_output.txt"

    run_stellar_op "$build_output_file" \
        "Building and optimizing contracts..." \
        "Build failed!" \
        stellar contract build --optimize

    success "Build completed!"
    local build_output
    build_output=$(cat "$build_output_file")
    if [[ -n "$build_output" ]]; then
        print_output "$build_output"
    fi
    print_section_end
}

find_wasm() {
    local contract_name="$1"
    local optimized="${WASM_DIR}/${contract_name}.optimized.wasm"
    local fallback="${WASM_DIR}/${contract_name}.wasm"

    if [[ -f "$optimized" ]]; then
        echo "$optimized"
    elif [[ -f "$fallback" ]]; then
        echo "$fallback"
    else
        fatal "WASM file not found for contract '${contract_name}' in ${WASM_DIR}/"
    fi
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Config Functions                               │
# └──────────────────────────────────────────────────────────────────────────────┘

config_read() {
    local key_path="$1"
    python3 "$SCRIPT_DIR/toml_helper.py" read "$CONFIG_FILE" "$key_path"
}

config_write() {
    local key_path="$1"
    local value="$2"
    python3 "$SCRIPT_DIR/toml_helper.py" write "$CONFIG_FILE" "$key_path" "$value"
}

config_add_verifier() {
    local chain_key="$1"
    shift
    python3 "$SCRIPT_DIR/toml_helper.py" add-verifier "$CONFIG_FILE" "$chain_key" "$@"
}

config_update_verifier() {
    local chain_key="$1"
    shift
    python3 "$SCRIPT_DIR/toml_helper.py" update-verifier "$CONFIG_FILE" "$chain_key" "$@"
}

config_get_verifier_field() {
    local chain_key="$1"
    local selector="$2"
    local field="$3"
    python3 "$SCRIPT_DIR/toml_helper.py" get-verifier-field \
        "$CONFIG_FILE" \
        "$chain_key" \
        --selector "$selector" \
        --field "$field"
}

config_verifier_count() {
    local chain_key="$1"
    python3 "$SCRIPT_DIR/toml_helper.py" verifier-count "$CONFIG_FILE" "$chain_key"
}

config_verifier_rows() {
    local chain_key="$1"
    python3 "$SCRIPT_DIR/toml_helper.py" verifier-rows "$CONFIG_FILE" "$chain_key"
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Contract Query Functions                       │
# └──────────────────────────────────────────────────────────────────────────────┘

# Strip surrounding quotes from Stellar CLI output and take the last line.
strip_stellar_quotes() {
    tail -n 1 | sed -e 's/^"//' -e 's/"$//'
}

# Base helper for invoking a read-only query on a Stellar contract.
stellar_query() {
    local contract_id="$1"; shift
    stellar contract invoke \
        --id "$contract_id" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- "$@" 2>/dev/null
}

query_selector() { stellar_query "$1" selector | strip_stellar_quotes; }

query_version() { stellar_query "$1" version | strip_stellar_quotes; }

query_has_role() {
    local result
    result=$(stellar_query "$1" has_role --account "$2" --role "$3" | tail -n 1)
    # has_role returns Some(index) if role is present, null/None otherwise
    [[ "$result" != "null" && "$result" != "None" && -n "$result" ]]
}

query_role_member_count() { stellar_query "$1" get_role_member_count --role "$2" | tail -n 1; }

query_operation_state() { stellar_query "$1" get_operation_state --operation_id "$2" | strip_stellar_quotes; }

query_is_operation_ready() {
    local result
    result=$(stellar_query "$1" is_operation_ready --operation_id "$2" | tail -n 1)
    [[ "$result" == "true" ]]
}

query_verifiers() { stellar_query "$1" verifiers --selector "$2" | tail -n 1; }

query_min_delay() { stellar_query "$1" get_min_delay | tail -n 1; }

query_paused() {
    local estop_id="$1"
    local result
    if ! result=$(stellar_query "$estop_id" paused | tail -n 1); then
        return 2
    fi
    [[ "$result" == "true" ]]
}

capitalize_first() {
    local input="$1"
    if [[ -z "$input" ]]; then
        printf ''
        return
    fi
    printf '%s%s' "$(printf '%s' "${input:0:1}" | tr '[:lower:]' '[:upper:]')" "${input:1}"
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Role Pre-validation                            │
# └──────────────────────────────────────────────────────────────────────────────┘

validate_role() {
    local timelock_id="$1"
    local account_addr="$2"
    local role="$3"
    if ! query_has_role "$timelock_id" "$account_addr" "$role"; then
        fatal "Account ${BOLD_WHITE}$account_addr${RESET} does not have the ${BOLD_YELLOW}$role${RESET} role on timelock ${DIM}$timelock_id${RESET}"
    fi
    success "$(capitalize_first "$role") role verified"
}

validate_proposer() { validate_role "$1" "$2" "proposer"; }
validate_bootstrap() { validate_role "$1" "$2" "bootstrap"; }

validate_executor() {
    local timelock_id="$1"
    local account_addr="$2"
    local executor_count
    executor_count=$(query_role_member_count "$timelock_id" "executor")
    if [[ "$executor_count" == "0" ]]; then
        info "No executors configured — anyone can execute"
        return
    fi
    validate_role "$timelock_id" "$account_addr" "executor"
}

validate_canceller() { validate_role "$1" "$2" "canceller"; }

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Precondition Checks                            │
# └──────────────────────────────────────────────────────────────────────────────┘

check_selector_available() {
    local router_id="$1"
    local selector="$2"
    local result
    if ! result=$(query_verifiers "$router_id" "$selector"); then
        fatal "Failed to query selector ${BOLD_WHITE}$selector${RESET} on router ${DIM}$router_id${RESET}"
    fi

    # If the query returns an Active or Tombstone entry, selector is not available
    if echo "$result" | grep -q '"Active"'; then
        fatal "Selector ${BOLD_WHITE}$selector${RESET} is already active in the router"
    fi
    if echo "$result" | grep -q '"Tombstone"'; then
        fatal "Selector ${BOLD_WHITE}$selector${RESET} has been tombstoned — cannot be re-added"
    fi
    success "Selector ${BOLD_WHITE}$selector${RESET} is available"
}

check_selector_exists() {
    local router_id="$1"
    local selector="$2"
    local result
    if ! result=$(query_verifiers "$router_id" "$selector"); then
        fatal "Failed to query selector ${BOLD_WHITE}$selector${RESET} on router ${DIM}$router_id${RESET}"
    fi

    if echo "$result" | grep -q '"Active"'; then
        success "Selector ${BOLD_WHITE}$selector${RESET} is active"
        return
    fi
    fatal "Selector ${BOLD_WHITE}$selector${RESET} is not active in the router"
}

check_operation_ready() {
    local timelock_id="$1"
    local op_id="$2"
    if ! query_is_operation_ready "$timelock_id" "$op_id"; then
        local state
        state=$(query_operation_state "$timelock_id" "$op_id")
        fatal "Operation ${DIM}$op_id${RESET} is not ready (current state: ${BOLD_WHITE}$state${RESET})"
    fi
    success "Operation is ready for execution"
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Timelock Operation Helpers                     │
# └──────────────────────────────────────────────────────────────────────────────┘

# Compute a timelock operation ID by calling hash_operation on-chain.
#   compute_operation_id <target> <function_name> <args_json> <predecessor> <salt>
compute_operation_id() {
    local target="$1"
    local function_name="$2"
    local args_json="$3"
    local predecessor="$4"
    local salt="$5"
    stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        hash_operation \
        --target "$target" \
        --function "$function_name" \
        --args "$args_json" \
        --predecessor "$predecessor" \
        --salt "$salt" \
        | strip_stellar_quotes
}

# Schedule a timelock operation via schedule_op.
#   schedule_timelock_op <target> <function_name> <args_json> <predecessor> <delay> <salt> <output_file>
schedule_timelock_op() {
    local target="$1"
    local function_name="$2"
    local args_json="$3"
    local predecessor="$4"
    local delay="$5"
    local salt="$6"
    local output_file="$7"
    stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        schedule_op \
        --target "$target" \
        --function "$function_name" \
        --args "$args_json" \
        --predecessor "$predecessor" \
        --salt "$salt" \
        --delay "$delay" \
        --proposer "$DEPLOYER_JSON" \
        > "$output_file" 2>&1 &
    local pid=$!
    local status=0
    spinner "$pid" "Scheduling ${function_name//_/-} operation..." || status=$?

    if [[ $status -ne 0 ]]; then
        error "Schedule failed!"
        print_output "$(cat "$output_file")"

        # TimelockError::OperationAlreadyScheduled = #4000.
        if grep -q "Error(Contract, #4000)" "$output_file"; then
            warn "This operation hash is already scheduled (same target/function/args/predecessor/salt)."
            local existing_op_id=""
            local existing_state=""
            if existing_op_id=$(compute_operation_id "$target" "$function_name" "$args_json" "$predecessor" "$salt" 2>/dev/null); then
                info "Existing Operation ID: ${DIM}${existing_op_id}${RESET}"
                if existing_state=$(query_operation_state "$TIMELOCK_ID" "$existing_op_id" 2>/dev/null); then
                    info "Existing operation state: ${BOLD_WHITE}${existing_state}${RESET}"
                fi
            fi
            info "Use a unique 32-byte hex salt when scheduling, then pass the same --salt on execute."
            info "Use a value containing hex letters (a-f); numeric-only salts may be rejected by the parser."
            info "Example: ${CYAN}--salt abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890${RESET}"
        fi

        print_section_end
        exit 1
    fi
}

# Execute a timelock operation targeting a contract (e.g. the router).
#   execute_timelock_op <output_file> <spinner_msg> <target> <function_name> \
#       <args_json> <predecessor> <salt>
execute_timelock_op() {
    local output_file="$1"
    local spinner_msg="$2"
    local target="$3"
    local function_name="$4"
    local args_json="$5"
    local predecessor="$6"
    local salt="$7"
    run_stellar_op "$output_file" \
        "$spinner_msg" \
        "Execution failed!" \
        stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        execute_op \
        --target "$target" \
        --function "$function_name" \
        --args "$args_json" \
        --predecessor "$predecessor" \
        --salt "$salt" \
        --executor "$DEPLOYER_JSON"
}

# ┌──────────────────────────────────────────────────────────────────────────────┐
# │                              Args JSON Builders                             │
# └──────────────────────────────────────────────────────────────────────────────┘

args_add_verifier() {
    local selector="$1"
    local estop_addr="$2"
    echo "[{\"bytes\":\"$selector\"},{\"address\":\"$estop_addr\"}]"
}

args_remove_verifier() {
    local selector="$1"
    echo "[{\"bytes\":\"$selector\"}]"
}

args_update_delay() {
    local new_delay="$1"
    echo "[{\"u32\":$new_delay}]"
}

args_role_with_caller() {
    local account="$1"
    local role="$2"
    local caller="$3"
    echo "[{\"address\":\"$account\"},{\"symbol\":\"$role\"},{\"address\":\"$caller\"}]"
}

args_grant_role() { args_role_with_caller "$@"; }

args_revoke_role() { args_role_with_caller "$@"; }
