use coincync::config::NetworkType;
use coincync::consensus::v1_0_12_rules_active;
use coincync::constants::HARD_FORK_V1_0_12_HEIGHT;

#[test]
fn mainnet_activates_v1_0_12_rules_from_genesis() {
    assert!(v1_0_12_rules_active(NetworkType::Mainnet, 0));
    assert!(v1_0_12_rules_active(NetworkType::Mainnet, 1));
    assert!(v1_0_12_rules_active(
        NetworkType::Mainnet,
        HARD_FORK_V1_0_12_HEIGHT.saturating_sub(1),
    ));
}

#[test]
fn testnet_preserves_the_height_13_000_flag_day() {
    assert!(HARD_FORK_V1_0_12_HEIGHT > 0);
    assert!(!v1_0_12_rules_active(NetworkType::Testnet, 0));
    assert!(!v1_0_12_rules_active(
        NetworkType::Testnet,
        HARD_FORK_V1_0_12_HEIGHT - 1,
    ));
    assert!(v1_0_12_rules_active(
        NetworkType::Testnet,
        HARD_FORK_V1_0_12_HEIGHT,
    ));
}

#[test]
fn regtest_uses_hardened_rules_from_genesis() {
    assert!(v1_0_12_rules_active(NetworkType::Regtest, 0));
}
