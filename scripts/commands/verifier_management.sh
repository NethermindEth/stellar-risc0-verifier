# Verifier management commands: schedule/execute add/remove verifier

cmd_schedule_add_verifier() {
    local selector=""
    local verifier_target=""
    local delay=""
    local salt="$ZERO32"

    parse_subcmd_flags "schedule-add-verifier" \
        --selector selector \
        --verifier-estop verifier_target \
        --delay delay \
        --salt salt

    require_flag "--selector" "$selector"
    validate_selector "$selector"

    setup_with_router

    if ! verifier_target=$(require_verifier_estop "$selector" "$verifier_target"); then
        fatal "No estop/verifier address found for selector '$selector'. Provide --verifier-estop."
    fi
    delay=$(resolve_delay "$delay")

    print_section "Schedule: Add Verifier"
    info "Router: ${DIM}$ROUTER_ID${RESET}"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Selector" "$selector" "$CYAN" "$BOLD_YELLOW"
    kv "Verifier target" "$verifier_target" "$CYAN" "$WHITE"
    kv "Delay" "${delay}s" "$CYAN" "$BOLD_WHITE"

    # Precondition checks
    validate_proposer "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"
    check_selector_available "$ROUTER_ID" "$selector"

    mainnet_warning

    local args_json
    args_json=$(args_add_verifier "$selector" "$verifier_target")

    schedule_operation_and_report "$ROUTER_ID" "add_verifier" "$args_json" "$delay" "$salt" \
        "${TMP_DIR}/manage_schedule_add.txt"

    if [[ "$delay" == "0" ]]; then
        info "Delay is 0 — operation is immediately ready for execution"
        info "Run: ${CYAN}./manage.sh execute-add-verifier -n $NETWORK -a $ACCOUNT --selector $selector${RESET}"
    else
        info "Operation will be ready after ${BOLD_WHITE}${delay}s${RESET} delay"
    fi

    print_section_end
}

cmd_execute_add_verifier() {
    local selector=""
    local verifier_target=""
    local predecessor="$ZERO32"
    local salt="$ZERO32"

    parse_subcmd_flags "execute-add-verifier" \
        --selector selector \
        --verifier-estop verifier_target \
        --predecessor predecessor \
        --salt salt

    require_flag "--selector" "$selector"
    validate_selector "$selector"

    setup_with_router

    if ! verifier_target=$(require_verifier_estop "$selector" "$verifier_target"); then
        fatal "No estop/verifier address found for selector '$selector'. Provide --verifier-estop."
    fi

    print_section "Execute: Add Verifier"
    info "Router: ${DIM}$ROUTER_ID${RESET}"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Selector" "$selector" "$CYAN" "$BOLD_YELLOW"
    kv "Verifier target" "$verifier_target" "$CYAN" "$WHITE"

    # Precondition checks
    validate_executor "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    local args_json
    args_json=$(args_add_verifier "$selector" "$verifier_target")

    prepare_execute_operation "$ROUTER_ID" "add_verifier" "$args_json" "$predecessor" "$salt"

    execute_timelock_op "${TMP_DIR}/manage_execute_add.txt" \
        "Executing add-verifier operation..." \
        "$ROUTER_ID" "add_verifier" "$args_json" "$predecessor" "$salt"

    success "Verifier added to router!"

    # Update config
    config_update_verifier "$CHAIN_KEY" \
        --selector "$selector" \
        --field unroutable \
        --value false
    success "Config updated (unroutable=false)"

    print_section_end
}

cmd_schedule_remove_verifier() {
    local selector=""
    local delay=""
    local salt="$ZERO32"

    parse_subcmd_flags "schedule-remove-verifier" \
        --selector selector \
        --delay delay \
        --salt salt

    require_flag "--selector" "$selector"
    validate_selector "$selector"

    setup_with_router

    delay=$(resolve_delay "$delay")

    print_section "Schedule: Remove Verifier"
    info "Router: ${DIM}$ROUTER_ID${RESET}"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Selector" "$selector" "$CYAN" "$BOLD_YELLOW"
    kv "Delay" "${delay}s" "$CYAN" "$BOLD_WHITE"

    validate_proposer "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"
    check_selector_exists "$ROUTER_ID" "$selector"

    mainnet_warning

    local args_json
    args_json=$(args_remove_verifier "$selector")
    schedule_operation_and_report "$ROUTER_ID" "remove_verifier" "$args_json" "$delay" "$salt" \
        "${TMP_DIR}/manage_schedule_remove.txt"

    print_section_end
}

cmd_execute_remove_verifier() {
    local selector=""
    local predecessor="$ZERO32"
    local salt="$ZERO32"

    parse_subcmd_flags "execute-remove-verifier" \
        --selector selector \
        --predecessor predecessor \
        --salt salt

    require_flag "--selector" "$selector"
    validate_selector "$selector"

    setup_with_router

    print_section "Execute: Remove Verifier"
    info "Router: ${DIM}$ROUTER_ID${RESET}"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Selector" "$selector" "$CYAN" "$BOLD_YELLOW"

    validate_executor "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    local args_json
    args_json=$(args_remove_verifier "$selector")

    prepare_execute_operation "$ROUTER_ID" "remove_verifier" "$args_json" "$predecessor" "$salt"

    execute_timelock_op "${TMP_DIR}/manage_execute_remove.txt" \
        "Executing remove-verifier operation..." \
        "$ROUTER_ID" "remove_verifier" "$args_json" "$predecessor" "$salt"

    success "Verifier removed from router!"

    # Update config
    config_update_verifier "$CHAIN_KEY" \
        --selector "$selector" \
        --field unroutable \
        --value true
    success "Config updated (marked as removed)"

    print_section_end
}
