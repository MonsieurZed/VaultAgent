//! Tests de `hello-status` et `hello-enroll` : corps exacts envoyés au faux serveur, preuve vérifiée par
//! `vaultagent-core`, une seule création de clé, aucune invite Windows Hello.

#[cfg(test)]
mod tests {
    use crate::approver_key::ApproverKey;
    use crate::config::ClientConfig;
    use crate::decide::Approver;
    use crate::error::ClientError;
    use crate::fake_hello::FakeHello;
    use crate::fake_transport::FakeTransport;
    use crate::hello_enroll::hello_status;
    use crate::hello_fixture::{credential, signing_key, working_hello};
    use crate::protector::ReversingProtector;
    use crate::server_address::ServerAddress;
    use crate::test_dir::{TestDir, sample_token};
    use crate::token::Token;
    use serde_json::Value;
    use vaultagent_core::{ErrorCode, decision_message, verify_hello, verify_pss};

    const VAULTS: &str = "[{\"id\":\"a\"},{\"id\":\"b\"},{\"id\":\"c\"}]";
    const READY: &str = "{\"approvalSecurity\":{\"registered\":true,\"helloEnabled\":false}}";
    const BEGIN: &str =
        "{\"vaultId\":\"a\",\"requestId\":\"enroll-hello\",\"challenge\":\"chal\",\"helloCredential\":null}";

    /// Exécute l'enrôlement et rend le résultat avec la clé d'approbateur.
    fn run(fake: &FakeTransport, hello: &FakeHello, only: Option<&str>) -> (Result<String, ClientError>, ApproverKey) {
        let dir = TestDir::new();
        let key = ApproverKey::load_or_create(&dir.path().join("k"), &ReversingProtector).unwrap();
        let config = ClientConfig {
            server: ServerAddress::parse("https://vault.example.test").unwrap(),
            token: Token::parse(&sample_token()).unwrap(),
        };
        let result = Approver {
            transport: fake,
            config: &config,
            key: &key,
        }
        .enroll_hello(hello, only);
        (result, key)
    }

    /// Chemins appelés, sans le préfixe de l'API.
    fn paths(fake: &FakeTransport) -> Vec<String> {
        fake.calls()
            .iter()
            .map(|call| call.path.trim_start_matches("/api/agent/").to_owned())
            .collect()
    }

    #[test]
    fn eligible_vault_creates_the_key_once_and_enrolls_with_exact_bodies() {
        let key = signing_key(9);
        let hello = working_hello(&key);
        let fake = FakeTransport::new()
            .reply(200, VAULTS)
            .reply(200, READY)
            .reply(
                200,
                "{\"approvalSecurity\":{\"registered\":true,\"helloEnabled\":true,\"helloRequiredForAll\":true}}",
            )
            .reply(200, "{\"approvalSecurity\":{\"registered\":false}}")
            .reply(200, BEGIN)
            .reply(200, BEGIN)
            .reply(200, "{\"helloEnabled\":true,\"helloRequired\":true,\"vaultId\":\"a\"}");
        let (result, approver_key) = run(&fake, &hello, None);
        assert_eq!(result.unwrap(), "{\"enrolled\":1}");
        assert_eq!(
            paths(&fake),
            [
                "vaults",
                "status",
                "status",
                "status",
                "hello-begin",
                "hello-begin",
                "hello-enroll"
            ]
        );
        assert_eq!(hello.registers(), 1);
        let body: Value = serde_json::from_str(&fake.calls()[6].body).unwrap();
        assert_eq!(body["vaultId"], "a");
        assert_eq!(body["credential"], serde_json::to_value(credential(&key)).unwrap());
        let message = decision_message("https://vault.example.test", "a", "enroll-hello", "chal", true, false);
        verify_pss(
            &approver_key.public_key().unwrap(),
            &message,
            body["signature"].as_str().unwrap(),
            ErrorCode::InvalidKey,
        )
        .unwrap();
        let proof: vaultagent_core::HelloProof = serde_json::from_value(body["helloProof"].clone()).unwrap();
        assert!(verify_hello(Some(&credential(&key)), Some(&proof), &message).is_ok());
        let mut names: Vec<&str> = body.as_object().unwrap().keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["credential", "helloProof", "signature", "vaultId"]);
    }

    #[test]
    fn existing_server_key_is_reused_without_creating_one() {
        let key = signing_key(9);
        let with_key = format!(
            "{{\"vaultId\":\"a\",\"requestId\":\"enroll-hello\",\"challenge\":\"chal\",\"helloCredential\":{}}}",
            serde_json::to_string(&credential(&key)).unwrap()
        );
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"}]")
            .reply(200, READY)
            .reply(200, &with_key)
            .reply(200, "{}");
        let hello = working_hello(&key);
        assert_eq!(run(&fake, &hello, Some("a")).0.unwrap(), "{\"enrolled\":1}");
        assert_eq!(hello.registers(), 0);
        assert_eq!(paths(&fake), ["vaults", "status", "hello-begin", "hello-enroll"]);
    }

    #[test]
    fn nothing_to_enroll_prints_zero_without_prompt() {
        let fake = FakeTransport::new().reply(200, "[]");
        let hello = working_hello(&signing_key(9));
        assert_eq!(run(&fake, &hello, None).0.unwrap(), "{\"enrolled\":0}");
        assert_eq!(hello.registers() + hello.challenges().len(), 0);
    }

    #[test]
    fn unavailable_hello_refuses_before_any_request() {
        let fake = FakeTransport::new();
        let error = run(&fake, &FakeHello::unavailable(), None).0.unwrap_err();
        assert_eq!(error.code(), "hello-unavailable");
        assert_eq!(fake.calls().len(), 0);
    }

    #[test]
    fn cancelled_prompt_stops_before_enrolling() {
        let hello = FakeHello::new(
            Err(ClientError::HelloCancelled),
            Box::new(|_, _| Err(ClientError::HelloCancelled)),
        );
        let fake = FakeTransport::new()
            .reply(200, "[{\"id\":\"a\"}]")
            .reply(200, READY)
            .reply(200, BEGIN);
        assert_eq!(run(&fake, &hello, None).0.unwrap_err().code(), "hello-cancelled");
        assert_eq!(paths(&fake), ["vaults", "status", "hello-begin"]);
    }

    #[test]
    fn status_line_has_the_dotnet_field_names() {
        assert_eq!(
            hello_status(&working_hello(&signing_key(9))).unwrap(),
            "{\"available\":true,\"apiVersion\":7}"
        );
        assert_eq!(
            hello_status(&FakeHello::unavailable()).unwrap(),
            "{\"available\":false,\"apiVersion\":0}"
        );
    }
}
