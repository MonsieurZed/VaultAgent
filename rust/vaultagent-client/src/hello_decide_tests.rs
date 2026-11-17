//! Tests de `decide-unlock` avec la vraie preuve Hello du poste (`SystemProofs`) et un faux système Hello qui signe
//! de vraies preuves WebAuthn : corps exact envoyé, preuve vérifiée par `vaultagent-core`, aucune invite.

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::Value;
    use vaultagent_core::{HelloProof, decision_message, verify_hello};

    use crate::approval_request::ApprovalRequest;
    use crate::approver_key::ApproverKey;
    use crate::config::ClientConfig;
    use crate::decide::Approver;
    use crate::fake_elevation::FakeElevation;
    use crate::fake_hello::FakeHello;
    use crate::fake_transport::FakeTransport;
    use crate::hello_fixture::{credential, signing_key, working_hello};
    use crate::proofs::SystemProofs;
    use crate::protector::ReversingProtector;
    use crate::server_address::ServerAddress;
    use crate::test_dir::{TestDir, sample_token};
    use crate::token::Token;

    const ANSWER: &str = "{\"vaultId\":\"default\",\"requestId\":\"r1\",\"state\":\"approved\"}";

    /// Configuration, clé d'approbateur et dossier de test.
    fn setup() -> (ClientConfig, ApproverKey, TestDir) {
        let dir = TestDir::new();
        let key = ApproverKey::load_or_create(&dir.path().join("k"), &ReversingProtector).unwrap();
        let config = ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        };
        (config, key, dir)
    }

    /// Demande qui exige Hello avec la clé de test enrôlée.
    fn hello_request() -> ApprovalRequest {
        ApprovalRequest {
            vault_id: None,
            request_id: "r1".to_owned(),
            challenge: "chal".to_owned(),
            hello_required: true,
            elevation_required: false,
            hello_credential: Some(serde_json::to_value(credential(&signing_key(9))).unwrap()),
        }
    }

    #[test]
    fn approving_sends_a_hello_proof_covering_the_decision_message() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let (system, hello) = (FakeElevation::elevated("S-1-5-21-1"), working_hello(&signing_key(9)));
        let proofs = SystemProofs::new(&system, &hello, Path::new("."));
        approver.decide_unlock(&hello_request(), true, true, &proofs).unwrap();
        let body: Value = serde_json::from_str(&fake.calls()[0].body).unwrap();
        let message = decision_message("https://vault.example.test", "default", "r1", "chal", true, true);
        let proof: HelloProof = serde_json::from_value(body["helloProof"].clone()).unwrap();
        assert!(verify_hello(Some(&credential(&signing_key(9))), Some(&proof), &message).is_ok());
        assert!(body.get("elevationSignature").is_none());
        assert_eq!(hello.challenges().len(), 1);
    }

    #[test]
    fn denial_never_prompts_hello() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let (system, hello) = (FakeElevation::elevated("S-1-5-21-1"), working_hello(&signing_key(9)));
        let proofs = SystemProofs::new(&system, &hello, Path::new("."));
        approver.decide_unlock(&hello_request(), false, false, &proofs).unwrap();
        let body: Value = serde_json::from_str(&fake.calls()[0].body).unwrap();
        assert!(body.get("helloProof").is_none());
        assert_eq!(hello.challenges().len(), 0);
    }

    #[test]
    fn unavailable_hello_sends_nothing() {
        let (config, key, _dir) = setup();
        let fake = FakeTransport::new().reply(200, ANSWER);
        let approver = Approver {
            transport: &fake,
            config: &config,
            key: &key,
        };
        let (system, hello) = (FakeElevation::elevated("S-1-5-21-1"), FakeHello::unavailable());
        let proofs = SystemProofs::new(&system, &hello, Path::new("."));
        let error = approver
            .decide_unlock(&hello_request(), true, false, &proofs)
            .unwrap_err();
        assert_eq!(error.code(), "hello-unavailable");
        assert_eq!(fake.calls().len(), 0);
    }
}
