module porto_london::porto_london {
    use aptos_framework::account;
    use aptos_framework::fungible_asset::{Self, Metadata};
    use aptos_framework::object::{Self, Object};
    use aptos_framework::primary_fungible_store;
    use aptos_std::table::{Self, Table};
    use std::signer;
    use std::option;
    use std::string;
    use std::vector;

    const E_ALREADY_INITIALIZED: u64 = 1;
    const E_NOT_ADMINISTRATOR: u64 = 2;
    const E_NOT_COORDINATOR: u64 = 3;
    const E_PAUSED: u64 = 4;
    const E_DUPLICATE_PERIOD: u64 = 5;
    const E_INVALID_AMOUNT: u64 = 6;
    const E_INVALID_LIMIT: u64 = 7;
    const E_PERIOD_NOT_FOUND: u64 = 8;
    const E_PERIOD_CLOSED: u64 = 9;
    const E_DUPLICATE_RIGHTS: u64 = 10;
    const E_INVALID_RIGHTS: u64 = 11;
    const E_BATCH_TOO_LARGE: u64 = 12;
    const E_INVALID_RANGE: u64 = 13;
    const E_CONFLICTING_RETRY: u64 = 14;
    const E_RANGE_OVERLAP: u64 = 15;
    const E_DUPLICATE_OBLIGATION: u64 = 16;
    const E_OBLIGATION_NOT_FOUND: u64 = 17;
    const E_OBLIGATION_PAID: u64 = 18;

    /// `settlement_asset` is the testnet fungible-asset Metadata object address.
    /// The address binding is immutable in this baseline; transfer integration is not included.
    struct ProtocolConfig has key {
        package_version: vector<u8>,
        settlement_asset: address,
        administrator: address,
        coordinator: address,
        custody_address: address,
        custody_capability: account::SignerCapability,
        paused: bool,
        max_usage_units_per_batch: u64,
        max_ranges_per_lane: u64,
        periods: Table<vector<u8>, FundingPeriod>,
        rights: Table<vector<u8>, RightsSnapshot>,
        accepted_batches: Table<vector<u8>, UsageBatch>,
        lane_ranges: Table<vector<u8>, vector<UsageRange>>,
        obligations: Table<vector<u8>, Obligation>,
    }

    struct FundingPeriod has store {
        funded_amount: u64,
        policy_hash: vector<u8>,
        open: bool,
    }

    struct RightsSnapshot has store {
        beneficiaries: vector<address>,
        basis_points: vector<u64>,
    }

    struct UsageBatch has store {
        sequence_start: u64,
        sequence_end: u64,
        payload_digest: vector<u8>,
        aggregate_usage: u64,
    }

    struct UsageRange has copy, drop, store {
        sequence_start: u64,
        sequence_end: u64,
    }

    struct Obligation has store {
        period_id: vector<u8>,
        recipient: address,
        amount: u64,
        paid: bool,
    }

    public entry fun initialize(
        publisher: &signer,
        administrator: address,
        coordinator: address,
        settlement_asset: address,
        max_usage_units_per_batch: u64,
        max_ranges_per_lane: u64,
    ) {
        let publisher_address = signer::address_of(publisher);
        assert!(!exists<ProtocolConfig>(publisher_address), E_ALREADY_INITIALIZED);
        assert!(max_usage_units_per_batch > 0 && max_ranges_per_lane > 0, E_INVALID_LIMIT);
        let metadata: Object<Metadata> = object::address_to_object<Metadata>(settlement_asset);
        let _metadata_address = metadata.object_address();
        let (custody_signer, custody_capability) = account::create_resource_account(publisher, b"porto-london-custody");
        let custody_address = signer::address_of(&custody_signer);
        move_to(publisher, ProtocolConfig {
            package_version: b"london.v1",
            settlement_asset,
            administrator,
            coordinator,
            custody_address,
            custody_capability,
            paused: false,
            max_usage_units_per_batch,
            max_ranges_per_lane,
            periods: table::new<vector<u8>, FundingPeriod>(),
            rights: table::new<vector<u8>, RightsSnapshot>(),
            accepted_batches: table::new<vector<u8>, UsageBatch>(),
            lane_ranges: table::new<vector<u8>, vector<UsageRange>>(),
            obligations: table::new<vector<u8>, Obligation>(),
        });
    }

    public entry fun open_funding_period(
        administrator: &signer,
        period_id: vector<u8>,
        funded_amount: u64,
        policy_hash: vector<u8>,
    ) acquires ProtocolConfig {
        let config = borrow_global_mut<ProtocolConfig>(@porto_london);
        assert_administrator(config, administrator);
        assert!(!config.paused, E_PAUSED);
        assert!(funded_amount > 0, E_INVALID_AMOUNT);
        assert!(!table::contains(&config.periods, period_id), E_DUPLICATE_PERIOD);
        let metadata: Object<Metadata> = object::address_to_object<Metadata>(config.settlement_asset);
        primary_fungible_store::transfer(administrator, metadata, config.custody_address, funded_amount);
        table::add(&mut config.periods, period_id, FundingPeriod { funded_amount, policy_hash, open: true });
    }

    public entry fun close_funding_period(administrator: &signer, period_id: vector<u8>) acquires ProtocolConfig {
        let config = borrow_global_mut<ProtocolConfig>(@porto_london);
        assert_administrator(config, administrator);
        let period = borrow_period_mut(config, period_id);
        period.open = false;
    }

    public entry fun register_rights_snapshot(
        administrator: &signer,
        work_id: vector<u8>,
        version: u64,
        beneficiaries: vector<address>,
        basis_points: vector<u64>,
    ) acquires ProtocolConfig {
        let config = borrow_global_mut<ProtocolConfig>(@porto_london);
        assert_administrator(config, administrator);
        assert!(!config.paused, E_PAUSED);
        assert!(vector::length(&beneficiaries) > 0 && vector::length(&beneficiaries) == vector::length(&basis_points), E_INVALID_RIGHTS);
        assert!(sum_basis_points(&basis_points) == 10000, E_INVALID_RIGHTS);
        let key = rights_key(work_id, version);
        assert!(!table::contains(&config.rights, key), E_DUPLICATE_RIGHTS);
        table::add(&mut config.rights, key, RightsSnapshot { beneficiaries, basis_points });
    }

    public entry fun submit_usage_batch(
        coordinator: &signer,
        period_id: vector<u8>,
        lane_id: vector<u8>,
        sequence_start: u64,
        sequence_end: u64,
        bucket_id: vector<u8>,
        payload_digest: vector<u8>,
        aggregate_usage: u64,
    ) acquires ProtocolConfig {
        let config = borrow_global_mut<ProtocolConfig>(@porto_london);
        assert_coordinator(config, coordinator);
        assert!(!config.paused, E_PAUSED);
        assert!(sequence_start <= sequence_end, E_INVALID_RANGE);
        assert!(aggregate_usage <= config.max_usage_units_per_batch, E_BATCH_TOO_LARGE);
        let period = borrow_period(config, period_id);
        assert!(period.open, E_PERIOD_CLOSED);
        let batch_key = batch_key(period_id, bucket_id);
        if (table::contains(&config.accepted_batches, batch_key)) {
            let existing = table::borrow(&config.accepted_batches, batch_key);
            assert!(existing.sequence_start == sequence_start && existing.sequence_end == sequence_end && existing.payload_digest == payload_digest && existing.aggregate_usage == aggregate_usage, E_CONFLICTING_RETRY);
            return
        };
        let lane_key = lane_key(period_id, lane_id);
        if (!table::contains(&config.lane_ranges, lane_key)) {
            table::add(&mut config.lane_ranges, lane_key, vector::empty<UsageRange>());
        };
        let ranges = table::borrow_mut(&mut config.lane_ranges, lane_key);
        assert!(vector::length(ranges) < config.max_ranges_per_lane, E_BATCH_TOO_LARGE);
        assert_no_overlap(ranges, sequence_start, sequence_end);
        vector::push_back(ranges, UsageRange { sequence_start, sequence_end });
        table::add(&mut config.accepted_batches, batch_key, UsageBatch { sequence_start, sequence_end, payload_digest, aggregate_usage });
    }

    public entry fun pause(administrator: &signer, paused: bool) acquires ProtocolConfig {
        let config = borrow_global_mut<ProtocolConfig>(@porto_london);
        assert_administrator(config, administrator);
        config.paused = paused;
    }

    /// Internal settlement output. This is deliberately coordinator-only, and
    /// payment still requires Move custody to transfer the configured asset.
    public entry fun create_unpaid_obligation(
        coordinator: &signer,
        obligation_id: vector<u8>,
        period_id: vector<u8>,
        recipient: address,
        amount: u64,
    ) acquires ProtocolConfig {
        let config = borrow_global_mut<ProtocolConfig>(@porto_london);
        assert_coordinator(config, coordinator);
        assert!(!config.paused && amount > 0, E_INVALID_AMOUNT);
        assert!(table::contains(&config.periods, period_id), E_PERIOD_NOT_FOUND);
        assert!(!table::contains(&config.obligations, obligation_id), E_DUPLICATE_OBLIGATION);
        table::add(&mut config.obligations, obligation_id, Obligation { period_id, recipient, amount, paid: false });
    }

    /// Transfer and paid marker are in the same transaction. A transfer abort
    /// rolls back the marker, so an obligation remains unpaid on failure.
    public entry fun pay_obligation(payer: &signer, obligation_id: vector<u8>) acquires ProtocolConfig {
        let config = borrow_global_mut<ProtocolConfig>(@porto_london);
        assert!(table::contains(&config.obligations, obligation_id), E_OBLIGATION_NOT_FOUND);
        let obligation = table::borrow(&config.obligations, obligation_id);
        assert!(!obligation.paid, E_OBLIGATION_PAID);
        let recipient = obligation.recipient;
        let amount = obligation.amount;
        let asset_address = config.settlement_asset;
        let custody = account::create_signer_with_capability(&config.custody_capability);
        let metadata: Object<Metadata> = object::address_to_object<Metadata>(asset_address);
        primary_fungible_store::transfer(&custody, metadata, recipient, amount);
        table::borrow_mut(&mut config.obligations, obligation_id).paid = true;
        let _payer_address = signer::address_of(payer);
    }

    #[view]
    public fun settlement_asset(): address acquires ProtocolConfig {
        borrow_global<ProtocolConfig>(@porto_london).settlement_asset
    }

    #[view]
    public fun is_period_open(period_id: vector<u8>): bool acquires ProtocolConfig {
        let config = borrow_global<ProtocolConfig>(@porto_london);
        table::contains(&config.periods, period_id) && table::borrow(&config.periods, period_id).open
    }

    #[view]
    public fun is_obligation_paid(obligation_id: vector<u8>): bool acquires ProtocolConfig {
        let config = borrow_global<ProtocolConfig>(@porto_london);
        table::contains(&config.obligations, obligation_id) && table::borrow(&config.obligations, obligation_id).paid
    }

    #[view]
    public fun custody_balance(): u64 acquires ProtocolConfig {
        let config = borrow_global<ProtocolConfig>(@porto_london);
        let metadata: Object<Metadata> = object::address_to_object<Metadata>(config.settlement_asset);
        primary_fungible_store::balance(config.custody_address, metadata)
    }

    fun assert_administrator(config: &ProtocolConfig, administrator: &signer) {
        assert!(signer::address_of(administrator) == config.administrator, E_NOT_ADMINISTRATOR);
    }

    fun assert_coordinator(config: &ProtocolConfig, coordinator: &signer) {
        assert!(signer::address_of(coordinator) == config.coordinator, E_NOT_COORDINATOR);
    }

    fun borrow_period(config: &ProtocolConfig, period_id: vector<u8>): &FundingPeriod {
        assert!(table::contains(&config.periods, period_id), E_PERIOD_NOT_FOUND);
        table::borrow(&config.periods, period_id)
    }

    fun borrow_period_mut(config: &mut ProtocolConfig, period_id: vector<u8>): &mut FundingPeriod {
        assert!(table::contains(&config.periods, period_id), E_PERIOD_NOT_FOUND);
        table::borrow_mut(&mut config.periods, period_id)
    }

    fun sum_basis_points(basis_points: &vector<u64>): u64 {
        let total = 0;
        let index = 0;
        while (index < vector::length(basis_points)) {
            total = total + *vector::borrow(basis_points, index);
            index = index + 1;
        };
        total
    }

    fun assert_no_overlap(ranges: &vector<UsageRange>, sequence_start: u64, sequence_end: u64) {
        let index = 0;
        while (index < vector::length(ranges)) {
            let range = vector::borrow(ranges, index);
            assert!(sequence_end < range.sequence_start || sequence_start > range.sequence_end, E_RANGE_OVERLAP);
            index = index + 1;
        };
    }

    fun rights_key(work_id: vector<u8>, version: u64): vector<u8> {
        let key = work_id;
        vector::append(&mut key, b":");
        vector::append(&mut key, u64_to_bytes(version));
        key
    }

    fun batch_key(period_id: vector<u8>, bucket_id: vector<u8>): vector<u8> {
        let key = period_id;
        vector::append(&mut key, b":");
        vector::append(&mut key, bucket_id);
        key
    }

    fun lane_key(period_id: vector<u8>, lane_id: vector<u8>): vector<u8> {
        let key = period_id;
        vector::append(&mut key, b":");
        vector::append(&mut key, lane_id);
        key
    }

    fun u64_to_bytes(value: u64): vector<u8> {
        let bytes = vector::empty<u8>();
        vector::push_back(&mut bytes, (value & 255) as u8);
        vector::push_back(&mut bytes, ((value >> 8) & 255) as u8);
        vector::push_back(&mut bytes, ((value >> 16) & 255) as u8);
        vector::push_back(&mut bytes, ((value >> 24) & 255) as u8);
        vector::push_back(&mut bytes, ((value >> 32) & 255) as u8);
        vector::push_back(&mut bytes, ((value >> 40) & 255) as u8);
        vector::push_back(&mut bytes, ((value >> 48) & 255) as u8);
        vector::push_back(&mut bytes, ((value >> 56) & 255) as u8);
        bytes
    }

    #[test_only]
    fun test_asset(asset_creator: &signer, administrator: &signer): address {
        let constructor = &object::create_sticky_object(signer::address_of(asset_creator));
        primary_fungible_store::create_primary_store_enabled_fungible_asset(
            constructor,
            option::none(),
            string::utf8(b"Test asset"),
            string::utf8(b"TEST"),
            6,
            string::utf8(b"https://example.invalid"),
            string::utf8(b"https://example.invalid"),
        );
        let mint_ref = fungible_asset::generate_mint_ref(constructor);
        let metadata = object::object_from_constructor_ref<Metadata>(constructor);
        let store = primary_fungible_store::ensure_primary_store_exists(
            signer::address_of(administrator), metadata
        );
        mint_ref.mint_to(store, 1_000_000);
        metadata.object_address()
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, asset_creator = @0xd)]
    fun initializes_with_the_test_asset_binding(publisher: signer, administrator: signer, coordinator: signer, asset_creator: signer) {
        let asset = test_asset(&asset_creator, &administrator);
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), asset, 100, 4);
        assert!(settlement_asset() == asset, 100);
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, stranger = @0xc, asset_creator = @0xd)]
    #[expected_failure(abort_code = E_NOT_ADMINISTRATOR)]
    fun rejects_unapproved_period_administrator(publisher: signer, administrator: signer, coordinator: signer, stranger: signer, asset_creator: signer) {
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), test_asset(&asset_creator, &administrator), 100, 4);
        open_funding_period(&stranger, b"period-a", 10, b"policy");
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, stranger = @0xc, asset_creator = @0xd)]
    #[expected_failure(abort_code = E_NOT_COORDINATOR)]
    fun rejects_unapproved_usage_coordinator(publisher: signer, administrator: signer, coordinator: signer, stranger: signer, asset_creator: signer) {
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), test_asset(&asset_creator, &administrator), 100, 4);
        open_funding_period(&administrator, b"period-a", 10, b"policy");
        submit_usage_batch(&stranger, b"period-a", b"lane-a", 1, 2, b"bucket-a", b"digest-a", 42);
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, asset_creator = @0xd)]
    fun accepts_an_exact_usage_retry(publisher: signer, administrator: signer, coordinator: signer, asset_creator: signer) {
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), test_asset(&asset_creator, &administrator), 100, 4);
        open_funding_period(&administrator, b"period-a", 10, b"policy");
        submit_usage_batch(&coordinator, b"period-a", b"lane-a", 1, 2, b"bucket-a", b"digest-a", 42);
        submit_usage_batch(&coordinator, b"period-a", b"lane-a", 1, 2, b"bucket-a", b"digest-a", 42);
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, asset_creator = @0xd)]
    #[expected_failure(abort_code = E_CONFLICTING_RETRY)]
    fun rejects_a_conflicting_usage_retry(publisher: signer, administrator: signer, coordinator: signer, asset_creator: signer) {
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), test_asset(&asset_creator, &administrator), 100, 4);
        open_funding_period(&administrator, b"period-a", 10, b"policy");
        submit_usage_batch(&coordinator, b"period-a", b"lane-a", 1, 2, b"bucket-a", b"digest-a", 42);
        submit_usage_batch(&coordinator, b"period-a", b"lane-a", 1, 2, b"bucket-a", b"digest-b", 42);
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, asset_creator = @0xd)]
    #[expected_failure(abort_code = E_RANGE_OVERLAP)]
    fun rejects_an_overlapping_lane_range(publisher: signer, administrator: signer, coordinator: signer, asset_creator: signer) {
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), test_asset(&asset_creator, &administrator), 100, 4);
        open_funding_period(&administrator, b"period-a", 10, b"policy");
        submit_usage_batch(&coordinator, b"period-a", b"lane-a", 1, 2, b"bucket-a", b"digest-a", 42);
        submit_usage_batch(&coordinator, b"period-a", b"lane-a", 2, 3, b"bucket-b", b"digest-b", 42);
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, asset_creator = @0xd)]
    #[expected_failure(abort_code = E_PERIOD_CLOSED)]
    fun rejects_usage_for_a_closed_period(publisher: signer, administrator: signer, coordinator: signer, asset_creator: signer) {
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), test_asset(&asset_creator, &administrator), 100, 4);
        open_funding_period(&administrator, b"period-a", 10, b"policy");
        close_funding_period(&administrator, b"period-a");
        submit_usage_batch(&coordinator, b"period-a", b"lane-a", 1, 2, b"bucket-a", b"digest-a", 42);
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, recipient = @0xc, asset_creator = @0xd)]
    fun funds_custody_and_pays_an_obligation(
        publisher: signer,
        administrator: signer,
        coordinator: signer,
        recipient: signer,
        asset_creator: signer,
    ) {
        let asset = test_asset(&asset_creator, &administrator);
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), asset, 100, 4);
        open_funding_period(&administrator, b"period-a", 25, b"policy-a");
        assert!(custody_balance() == 25, 101);
        create_unpaid_obligation(&coordinator, b"obligation-a", b"period-a", signer::address_of(&recipient), 25);
        pay_obligation(&administrator, b"obligation-a");
        assert!(is_obligation_paid(b"obligation-a"), 102);
        assert!(custody_balance() == 0, 103);
    }

    #[test(publisher = @porto_london, administrator = @0xa, coordinator = @0xb, recipient = @0xc, asset_creator = @0xd)]
    #[expected_failure(abort_code = E_OBLIGATION_PAID)]
    fun rejects_duplicate_payout(
        publisher: signer,
        administrator: signer,
        coordinator: signer,
        recipient: signer,
        asset_creator: signer,
    ) {
        let asset = test_asset(&asset_creator, &administrator);
        initialize(&publisher, signer::address_of(&administrator), signer::address_of(&coordinator), asset, 100, 4);
        open_funding_period(&administrator, b"period-a", 25, b"policy-a");
        create_unpaid_obligation(&coordinator, b"obligation-a", b"period-a", signer::address_of(&recipient), 25);
        pay_obligation(&administrator, b"obligation-a");
        pay_obligation(&administrator, b"obligation-a");
    }
}
