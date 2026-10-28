use nomination_lab::app::Config;
use nomination_lab::inventory::read_inventory;
#[test]
fn inventory_is_public_metadata_and_never_returns_callback_or_key_passwords() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("gateway");
    std::fs::create_dir_all(home.join("repository/pmodes")).unwrap();
    let xml = r#"<PMode><id>mode</id><Initiator><PartyId>A</PartyId><SecurityConfiguration><Signing><KeystoreAlias password="VERY_SECRET_KEY">alias</KeystoreAlias><Algorithm>rsa-sha256</Algorithm></Signing></SecurityConfiguration></Initiator><Responder><PartyId>B</PartyId></Responder><Leg><Protocol><Address>https://as4.example.com/as4</Address></Protocol><DefaultDelivery><Parameter><name>URL</name><value>http://localhost/integrations/VERY_SECRET_CALLBACK</value></Parameter></DefaultDelivery><UserMessageFlow><BusinessInfo><Service><name>A06</name></Service><Action>submit</Action></BusinessInfo></UserMessageFlow></Leg></PMode>"#;
    std::fs::write(home.join("repository/pmodes/route.xml"), xml).unwrap();
    let cfg = Config::local("brp", dir.path().join("app"));
    let result = read_inventory(&cfg).unwrap();
    let text = serde_json::to_string(&result).unwrap();
    assert!(!text.contains("VERY_SECRET"));
    assert_eq!(result["pmodes"][0]["id"], "mode");
    assert_eq!(result["pmodes"][0]["encryption"], "Not configured");
    assert_eq!(result["pmodes"][0]["service"], "A06");
    assert_eq!(
        result["loaded_status"],
        "Configuration on disk; gateway load status not verified"
    );
}
