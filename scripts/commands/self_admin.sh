# Self-administration commands: schedule/execute update-delay, grant-role, revoke-role

cmd_schedule_update_delay() {
    local new_delay=""
    local delay=""
    local salt="$ZERO32"

    parse_subcmd_flags "schedule-update-delay" \
        --new-delay new_delay \
        --delay delay \
        --salt salt

    require_flag "--new-delay" "$new_delay"

    setup_with_config

    delay=$(resolve_delay "$delay")

    print_section "Schedule: Update Delay"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "New delay" "${new_delay}s" "$CYAN" "$BOLD_WHITE"
    kv "Schedule delay" "${delay}s" "$CYAN" "$BOLD_WHITE"

    validate_proposer "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    mainnet_warning

    local args_json
    args_json=$(args_update_delay "$new_delay")
    schedule_operation_and_report "$TIMELOCK_ID" "update_delay" "$args_json" "$delay" "$salt" \
        "${TMP_DIR}/manage_schedule_delay.txt"

    print_section_end
}

cmd_execute_update_delay() {
    local new_delay=""
    local predecessor="$ZERO32"
    local salt="$ZERO32"

    parse_subcmd_flags "execute-update-delay" \
        --new-delay new_delay \
        --predecessor predecessor \
        --salt salt

    require_flag "--new-delay" "$new_delay"

    setup_with_config

    print_section "Execute: Update Delay"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "New delay" "${new_delay}s" "$CYAN" "$BOLD_WHITE"

    validate_executor "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    local args_json
    args_json=$(args_update_delay "$new_delay")

    prepare_execute_operation "$TIMELOCK_ID" "update_delay" "$args_json" "$predecessor" "$salt"

    execute_self_admin_op "manage_execute_delay" \
        "Executing update-delay..." \
        "update_delay" "$args_json" "$predecessor" "$salt" \
        update_delay \
        --new_delay "$new_delay"

    success "Delay updated to ${BOLD_WHITE}${new_delay}s${RESET}!"

    config_write "chains.${CHAIN_KEY}.timelock-delay" "$new_delay"
    success "Config updated"

    print_section_end
}

cmd_schedule_grant_role() {
    local role=""
    local target_account=""
    local delay=""
    local salt="$ZERO32"

    parse_subcmd_flags "schedule-grant-role" \
        --role role \
        --target-account target_account \
        --delay delay \
        --salt salt

    require_flag "--role" "$role"
    require_flag "--target-account" "$target_account"

    setup_with_config

    delay=$(resolve_delay "$delay")

    print_section "Schedule: Grant Role"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Role" "$role" "$CYAN" "$BOLD_YELLOW"
    kv "Target account" "$target_account" "$CYAN" "$WHITE"
    kv "Delay" "${delay}s" "$CYAN" "$BOLD_WHITE"

    validate_proposer "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    mainnet_warning

    local args_json
    args_json=$(args_grant_role "$target_account" "$role" "$TIMELOCK_ID")
    schedule_operation_and_report "$TIMELOCK_ID" "grant_role" "$args_json" "$delay" "$salt" \
        "${TMP_DIR}/manage_schedule_grant.txt"

    print_section_end
}

cmd_execute_grant_role() {
    local role=""
    local target_account=""
    local predecessor="$ZERO32"
    local salt="$ZERO32"

    parse_subcmd_flags "execute-grant-role" \
        --role role \
        --target-account target_account \
        --predecessor predecessor \
        --salt salt

    require_flag "--role" "$role"
    require_flag "--target-account" "$target_account"

    setup_with_config

    print_section "Execute: Grant Role"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Role" "$role" "$CYAN" "$BOLD_YELLOW"
    kv "Target account" "$target_account" "$CYAN" "$WHITE"

    validate_executor "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    local args_json
    args_json=$(args_grant_role "$target_account" "$role" "$TIMELOCK_ID")

    prepare_execute_operation "$TIMELOCK_ID" "grant_role" "$args_json" "$predecessor" "$salt"

    execute_self_admin_op "manage_execute_grant" \
        "Executing grant-role..." \
        "grant_role" "$args_json" "$predecessor" "$salt" \
        grant_role \
        --account "$target_account" \
        --role "$role" \
        --caller "$TIMELOCK_ID"

    success "Role ${BOLD_YELLOW}$role${RESET} granted to ${BOLD_WHITE}$target_account${RESET}!"

    print_section_end
}

cmd_schedule_revoke_role() {
    local role=""
    local target_account=""
    local delay=""
    local salt="$ZERO32"

    parse_subcmd_flags "schedule-revoke-role" \
        --role role \
        --target-account target_account \
        --delay delay \
        --salt salt

    require_flag "--role" "$role"
    require_flag "--target-account" "$target_account"

    setup_with_config

    delay=$(resolve_delay "$delay")

    print_section "Schedule: Revoke Role"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Role" "$role" "$CYAN" "$BOLD_YELLOW"
    kv "Target account" "$target_account" "$CYAN" "$WHITE"
    kv "Delay" "${delay}s" "$CYAN" "$BOLD_WHITE"

    validate_proposer "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    mainnet_warning

    local args_json
    args_json=$(args_revoke_role "$target_account" "$role" "$TIMELOCK_ID")
    schedule_operation_and_report "$TIMELOCK_ID" "revoke_role" "$args_json" "$delay" "$salt" \
        "${TMP_DIR}/manage_schedule_revoke.txt"

    print_section_end
}

cmd_execute_revoke_role() {
    local role=""
    local target_account=""
    local predecessor="$ZERO32"
    local salt="$ZERO32"

    parse_subcmd_flags "execute-revoke-role" \
        --role role \
        --target-account target_account \
        --predecessor predecessor \
        --salt salt

    require_flag "--role" "$role"
    require_flag "--target-account" "$target_account"

    setup_with_config

    print_section "Execute: Revoke Role"
    info "Timelock: ${DIM}$TIMELOCK_ID${RESET}"
    kv "Role" "$role" "$CYAN" "$BOLD_YELLOW"
    kv "Target account" "$target_account" "$CYAN" "$WHITE"

    validate_executor "$TIMELOCK_ID" "$DEPLOYER_ADDRESS"

    local args_json
    args_json=$(args_revoke_role "$target_account" "$role" "$TIMELOCK_ID")

    prepare_execute_operation "$TIMELOCK_ID" "revoke_role" "$args_json" "$predecessor" "$salt"

    execute_self_admin_op "manage_execute_revoke" \
        "Executing revoke-role..." \
        "revoke_role" "$args_json" "$predecessor" "$salt" \
        revoke_role \
        --account "$target_account" \
        --role "$role" \
        --caller "$TIMELOCK_ID"

    success "Role ${BOLD_YELLOW}$role${RESET} revoked from ${BOLD_WHITE}$target_account${RESET}!"

    print_section_end
}
