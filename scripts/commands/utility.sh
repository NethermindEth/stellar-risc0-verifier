# Utility commands: renounce-role, cancel-operation, activate-estop, status

cmd_renounce_role() {
    local role=""

    parse_subcmd_flags "renounce-role" --role role

    require_flag "--role" "$role"

    setup_with_config

    print_section "Renounce Role"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Role" "$role" "$CYAN" "$BOLD_YELLOW"
    kv "Caller" "$DEPLOYER_ADDRESS" "$CYAN" "$WHITE"

    mainnet_warning

    run_stellar_op "${TMP_DIR}/manage_renounce.txt" \
        "Renouncing role..." \
        "Renounce failed!" \
        stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        renounce_role \
        --role "$role" \
        --caller "$DEPLOYER_JSON"

    success "Role ${BOLD_YELLOW}$role${RESET} renounced by ${BOLD_WHITE}$DEPLOYER_ADDRESS${RESET}!"

    print_section_end
}

cmd_cancel_operation() {
    local operation_id=""

    parse_subcmd_flags "cancel-operation" --operation-id operation_id

    require_flag "--operation-id" "$operation_id"

    setup_with_config

    print_section "Cancel Operation"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Operation ID" "$operation_id" "$CYAN" "$WHITE"

    validate_canceller "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    mainnet_warning

    run_stellar_op "${TMP_DIR}/manage_cancel.txt" \
        "Cancelling operation..." \
        "Cancel failed!" \
        stellar contract invoke \
        --id "$TIMELOCK_ID" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        cancel_op \
        --operation_id "$operation_id" \
        --canceller "$DEPLOYER_JSON"

    success "Operation cancelled!"

    print_section_end
}

cmd_activate_estop() {
    local selector=""
    local estop=""

    parse_subcmd_flags "activate-estop" \
        --selector selector \
        --estop estop

    if [[ -z "$estop" && -z "$selector" ]]; then
        fatal "Provide either --estop <address> or --selector <hex>"
    fi
    if [[ -n "$selector" ]]; then
        validate_selector "$selector"
    fi

    setup_environment

    # Resolve estop from config by selector
    if [[ -z "$estop" ]]; then
        if [[ -f "$CONFIG_FILE" ]]; then
            estop=$(resolve_verifier_estop_from_config "$selector")
        fi
        if [[ -z "$estop" ]]; then
            fatal "No estop address found for selector '$selector' in config"
        fi
    fi

    print_section "Activate Emergency Stop"
    kv "Emergency Stop" "$estop" "$CYAN" "$BOLD_RED"

    mainnet_warning

    warn "This will ${BOLD_RED}permanently${RESET} pause the verifier. This cannot be undone."
    read -rp "$(echo -e "${BOLD_BLUE}│${RESET}    ${BOLD_WHITE}Type 'ESTOP' to confirm: ${RESET}")" confirm
    if [[ "$confirm" != "ESTOP" ]]; then
        warn "Cancelled"
        print_section_end
        return
    fi

    run_stellar_op "${TMP_DIR}/manage_estop.txt" \
        "Activating emergency stop..." \
        "Emergency stop failed!" \
        stellar contract invoke \
        --id "$estop" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        -- \
        estop

    success "Emergency stop ${BOLD_RED}activated${RESET}!"
    warn "The verifier is now permanently paused."

    print_section_end
}

cmd_status() {
    setup_environment

    if [[ ! -f "$CONFIG_FILE" ]]; then
        fatal "Config file not found: ${DIM}$CONFIG_FILE${RESET}"
    fi

    print_section "Deployment Status: ${CHAIN_KEY}"

    local timelock_id router_id admin timelock_delay
    timelock_id=$(config_read "chains.${CHAIN_KEY}.timelock-controller" 2>/dev/null || echo "")
    router_id=$(config_read "chains.${CHAIN_KEY}.router" 2>/dev/null || echo "")
    admin=$(config_read "chains.${CHAIN_KEY}.admin" 2>/dev/null || echo "")
    timelock_delay=$(config_read "chains.${CHAIN_KEY}.timelock-delay" 2>/dev/null || echo "")

    echo -e "${BOLD_BLUE}│${RESET}"
    kv "Chain" "$CHAIN_KEY" "$WHITE" "$BOLD_MAGENTA"
    kv "Network" "$NETWORK" "$WHITE" "$BOLD_MAGENTA"
    kv "Admin" "${admin:-<not set>}" "$WHITE" "$DIM"
    print_divider

    # Timelock
    if [[ -n "$timelock_id" ]]; then
        kv "Timelock" "$timelock_id" "$WHITE" "$BOLD_CYAN"

        # Query on-chain min delay
        local on_chain_delay
        on_chain_delay=$(query_min_delay "$timelock_id" 2>/dev/null || echo "?")
        kv "Min Delay (config)" "${timelock_delay}s" "$DIM" "$WHITE"
        kv "Min Delay (on-chain)" "${on_chain_delay}s" "$DIM" "$WHITE"
    else
        kv "Timelock" "<not deployed>" "$WHITE" "$YELLOW"
    fi

    # Router
    if [[ -n "$router_id" ]]; then
        kv "Router" "$router_id" "$WHITE" "$BOLD_CYAN"
    else
        kv "Router" "<not deployed>" "$WHITE" "$YELLOW"
    fi

    print_divider

    # Verifiers
    local verifier_count
    verifier_count=$(config_verifier_count "$CHAIN_KEY" 2>/dev/null || echo "0")

    if [[ "$verifier_count" == "0" ]]; then
        info "No verifiers configured"
    else
        info "${BOLD_WHITE}${verifier_count}${RESET} verifier(s) configured:"
        echo -e "${BOLD_BLUE}│${RESET}"

        while IFS='|' read -r vname vselector vcontract vestop vunroutable; do
            kv "  Name" "$vname" "$DIM" "$WHITE"
            kv "  Selector" "$vselector" "$DIM" "$BOLD_YELLOW"
            kv "  Verifier" "$vcontract" "$DIM" "$DIM"
            kv "  E-Stop" "$vestop" "$DIM" "$DIM"

            if [[ "$vunroutable" == "True" || "$vunroutable" == "true" ]]; then
                kv "  Status" "unroutable (not in router)" "$DIM" "$YELLOW"
            else
                # Query on-chain state
                if [[ -n "$router_id" ]]; then
                    local state
                    state=$(query_verifiers "$router_id" "$vselector" 2>/dev/null || echo "?")
                    if echo "$state" | grep -q '"Active"'; then
                        kv "  Router Status" "Active" "$DIM" "$GREEN"
                    elif echo "$state" | grep -q '"Tombstone"'; then
                        kv "  Router Status" "Tombstone" "$DIM" "$RED"
                    else
                        kv "  Router Status" "$state" "$DIM" "$YELLOW"
                    fi
                fi

                # Check if paused
                if [[ -n "$vestop" && "$vestop" != "?" ]]; then
                    local paused_status=0
                    query_paused "$vestop" >/dev/null 2>&1 || paused_status=$?

                    if [[ $paused_status -eq 0 ]]; then
                        kv "  E-Stop" "PAUSED" "$DIM" "$BOLD_RED"
                    elif [[ $paused_status -eq 1 ]]; then
                        kv "  E-Stop" "active (not paused)" "$DIM" "$GREEN"
                    else
                        kv "  E-Stop" "unknown (query failed)" "$DIM" "$YELLOW"
                    fi
                fi
            fi
            print_divider
        done < <(config_verifier_rows "$CHAIN_KEY" 2>/dev/null || true)
    fi

    echo -e "${BOLD_BLUE}│${RESET}"
    print_section_end
}
