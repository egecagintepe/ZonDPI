//! Profile loader, JSON schema validator, and preset resolver.

use zondpi_packet_engine::ProfileDefinition;

pub struct ProfileEngine;

impl ProfileEngine {
    pub fn load_profile_from_json(json_str: &str) -> Result<ProfileDefinition, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_profile_from_json() {
        let json = r#"{
            "id": "test-profile",
            "name": "Test Profile",
            "description": "A test profile",
            "target_engine": "GoodbyeDPI",
            "arguments": ["-5", "--auto-ttl"]
        }"#;
        let profile = ProfileEngine::load_profile_from_json(json).expect("profile parse");
        assert_eq!(profile.id, "test-profile");
        assert_eq!(profile.arguments.len(), 2);
    }
}
