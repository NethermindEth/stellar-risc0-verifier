# Deploy commands: deploy-router, deploy-verifier, deploy-mock-verifier

cmd_deploy_router() {
    local wasm=""
    local min_delay="0"
    local admin=""

    parse_subcmd_flags "deploy-router" \
        --wasm wasm \
        --min-delay min_delay \
        --admin admin

    setup_environment

    build_contracts

    # --- Deploy Timelock ---
    local timelock_wasm="${wasm:-$(find_wasm "timelock")}"
    admin="${admin:-$DEPLOYER_ADDRESS}"

    mainnet_warning

    print_section "Deploying TimelockController"
    info "Min delay: ${BOLD_WHITE}$min_delay${RESET} seconds"

    run_stellar_op "${TMP_DIR}/manage_timelock_deploy.txt" \
        "Deploying timelock to $NETWORK..." \
        "Deployment failed!" \
        stellar contract deploy \
        --wasm "$timelock_wasm" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        --alias timelock-controller \
        -- \
        --min_delay "$min_delay" \
        --proposers "[$DEPLOYER_JSON]" \
        --executors "[$DEPLOYER_JSON]" \
        --admin "$DEPLOYER_JSON"

    TIMELOCK_ID=$(tail -n 1 "${TMP_DIR}/manage_timelock_deploy.txt")
    success "TimelockController deployed!"
    kv "Contract ID" "$TIMELOCK_ID" "$WHITE" "$BOLD_GREEN"
    print_section_end

    # --- Deploy Router ---
    local router_wasm
    router_wasm=$(find_wasm "risc0_router")

    print_section "Deploying Router (owner = timelock)"
    info "Owner: ${DIM}$TIMELOCK_ID${RESET}"

    run_stellar_op "${TMP_DIR}/manage_router_deploy.txt" \
        "Deploying router to $NETWORK..." \
        "Deployment failed!" \
        stellar contract deploy \
        --wasm "$router_wasm" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        --alias risc0-router \
        -- \
        --owner "$TIMELOCK_ID"

    ROUTER_ID=$(tail -n 1 "${TMP_DIR}/manage_router_deploy.txt")
    success "Router deployed!"
    kv "Contract ID" "$ROUTER_ID" "$WHITE" "$BOLD_GREEN"

    # Update config
    config_write "chains.${CHAIN_KEY}.timelock-controller" "$TIMELOCK_ID"
    config_write "chains.${CHAIN_KEY}.timelock-delay" "$min_delay"
    config_write "chains.${CHAIN_KEY}.admin" "$admin"
    config_write "chains.${CHAIN_KEY}.router" "$ROUTER_ID"
    success "Config updated"

    print_section_end

    # --- Summary ---
    print_section "Deployment Summary"
    echo -e "${BOLD_BLUE}│${RESET}"
    kv "Network" "$NETWORK" "$WHITE" "$BOLD_MAGENTA"
    kv "Deployer" "$ACCOUNT" "$WHITE" "$BOLD_GREEN"
    print_divider
    kv "Timelock" "$TIMELOCK_ID" "$WHITE" "$BOLD_CYAN"
    kv "Router" "$ROUTER_ID" "$WHITE" "$BOLD_CYAN"
    kv "Min Delay" "${min_delay}s" "$WHITE" "$BOLD_WHITE"
    echo -e "${BOLD_BLUE}│${RESET}"
    print_section_end

    echo ""
    echo -e "${BOLD_GREEN}    ✨ Router Deployment Complete! ✨${RESET}"
    echo ""
}

cmd_deploy_verifier() {
    local estop_owner=""
    local name="groth16-verifier"

    parse_subcmd_flags "deploy-verifier" \
        --estop-owner estop_owner \
        --name name

    setup_environment

    build_contracts

    estop_owner="${estop_owner:-$DEPLOYER_ADDRESS}"

    mainnet_warning

    # --- Deploy Groth16 Verifier ---
    local verifier_wasm
    verifier_wasm=$(find_wasm "groth16_verifier")

    print_section "Deploying Groth16 Verifier"
    info "WASM: ${DIM}$verifier_wasm${RESET}"

    run_stellar_op "${TMP_DIR}/manage_verifier_deploy.txt" \
        "Deploying groth16-verifier to $NETWORK..." \
        "Deployment failed!" \
        stellar contract deploy \
        --wasm "$verifier_wasm" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        --alias groth16-verifier

    local verifier_id
    verifier_id=$(tail -n 1 "${TMP_DIR}/manage_verifier_deploy.txt")
    success "Groth16 Verifier deployed!"
    kv "Contract ID" "$verifier_id" "$WHITE" "$BOLD_GREEN"
    print_section_end

    # --- Deploy Emergency Stop ---
    local estop_wasm
    estop_wasm=$(find_wasm "emergency_stop")

    print_section "Deploying Emergency Stop Wrapper"
    info "Wrapping verifier: ${DIM}$verifier_id${RESET}"
    info "Owner: ${DIM}$estop_owner${RESET}"

    run_stellar_op "${TMP_DIR}/manage_estop_deploy.txt" \
        "Deploying emergency-stop to $NETWORK..." \
        "Deployment failed!" \
        stellar contract deploy \
        --wasm "$estop_wasm" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        --alias emergency-stop \
        -- \
        --verifier "$verifier_id" \
        --owner "$estop_owner"

    local estop_id
    estop_id=$(tail -n 1 "${TMP_DIR}/manage_estop_deploy.txt")
    success "Emergency Stop deployed!"
    kv "Contract ID" "$estop_id" "$WHITE" "$BOLD_GREEN"
    print_section_end

    # --- Query verifier parameters ---
    print_section "Verifier Parameters"

    local selector version
    selector=$(query_selector "$verifier_id" || echo "")
    version=$(query_version "$verifier_id" || echo "")

    kv "Selector" "$selector" "$CYAN" "$BOLD_YELLOW"
    kv "Version" "$version" "$CYAN" "$BOLD_WHITE"
    kv "Verifier" "$verifier_id" "$CYAN" "$WHITE"
    kv "Emergency Stop" "$estop_id" "$CYAN" "$WHITE"

    print_section_end

    # --- Update config ---
    if [[ -n "$selector" ]]; then
        config_add_verifier "$CHAIN_KEY" \
            --name "$name" \
            --version "$version" \
            --selector "$selector" \
            --verifier "$verifier_id" \
            --estop "$estop_id" \
            --unroutable true
        success "Config updated (verifier added with unroutable=true)"
    else
        warn "Could not query selector — config not updated"
    fi

    # --- Summary ---
    print_section "Deployment Summary"
    echo -e "${BOLD_BLUE}│${RESET}"
    kv "Network" "$NETWORK" "$WHITE" "$BOLD_MAGENTA"
    kv "Deployer" "$ACCOUNT" "$WHITE" "$BOLD_GREEN"
    print_divider
    kv "Groth16 Verifier" "$verifier_id" "$WHITE" "$BOLD_CYAN"
    kv "Emergency Stop" "$estop_id" "$WHITE" "$BOLD_CYAN"
    kv "Selector" "$selector" "$WHITE" "$BOLD_YELLOW"
    kv "Version" "$version" "$WHITE" "$BOLD_WHITE"
    echo -e "${BOLD_BLUE}│${RESET}"
    info "The verifier is deployed but ${BOLD_YELLOW}unroutable${RESET} (not yet added to router)."
    info "Use ${CYAN}schedule-add-verifier${RESET} to add it via the timelock."
    echo -e "${BOLD_BLUE}│${RESET}"
    print_section_end

    echo ""
    echo -e "${BOLD_GREEN}    ✨ Verifier Deployment Complete! ✨${RESET}"
    echo ""
}

cmd_deploy_mock_verifier() {
    local selector="00000000"

    parse_subcmd_flags "deploy-mock-verifier" --selector selector

    validate_selector "$selector"

    setup_environment

    build_contracts

    if [[ "$NETWORK" == "mainnet" ]]; then
        fatal "Mock verifier must not be deployed to mainnet"
    fi

    local mock_wasm
    mock_wasm=$(find_wasm "mock_verifier")

    print_section "Deploying Mock Verifier"
    info "WASM: ${DIM}$mock_wasm${RESET}"
    kv "Selector" "$selector" "$CYAN" "$BOLD_YELLOW"
    warn "This is a ${BOLD_RED}mock${RESET} verifier — for testing only, no security guarantees."

    run_stellar_op "${TMP_DIR}/manage_mock_deploy.txt" \
        "Deploying mock-verifier to $NETWORK..." \
        "Deployment failed!" \
        stellar contract deploy \
        --wasm "$mock_wasm" \
        --source "$ACCOUNT" \
        --network "$NETWORK" \
        --alias mock-verifier \
        -- \
        --selector "$selector"

    local mock_id
    mock_id=$(tail -n 1 "${TMP_DIR}/manage_mock_deploy.txt")
    success "Mock Verifier deployed!"
    kv "Contract ID" "$mock_id" "$WHITE" "$BOLD_GREEN"

    # Update config — mock verifier has no estop wrapper
    config_add_verifier "$CHAIN_KEY" \
        --name "mock-verifier" \
        --selector "$selector" \
        --verifier "$mock_id" \
        --unroutable true
    success "Config updated (verifier added with unroutable=true)"

    print_section_end

    # --- Summary ---
    print_section "Deployment Summary"
    echo -e "${BOLD_BLUE}│${RESET}"
    kv "Network" "$NETWORK" "$WHITE" "$BOLD_MAGENTA"
    kv "Deployer" "$ACCOUNT" "$WHITE" "$BOLD_GREEN"
    print_divider
    kv "Mock Verifier" "$mock_id" "$WHITE" "$BOLD_CYAN"
    kv "Selector" "$selector" "$WHITE" "$BOLD_YELLOW"
    echo -e "${BOLD_BLUE}│${RESET}"
    info "Use ${CYAN}schedule-add-verifier${RESET} to add it to the router via the timelock."
    echo -e "${BOLD_BLUE}│${RESET}"
    print_section_end
}
