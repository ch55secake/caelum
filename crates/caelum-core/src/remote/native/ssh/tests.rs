use super::*;
mod workspace;
use crate::remote::{Host, MachineId};
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use wiremock::{
    matchers::{body_json, method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn reauthorizes_the_exact_machine_and_replaces_its_host_pin_after_resume() {
    let temp = tempfile::tempdir().unwrap();
    let (path_to_key, public) = identity(&temp.path().join("identity")).unwrap();
    let (_, resumed) = identity(&temp.path().join("identity")).unwrap();
    assert_eq!(public, resumed);
    assert_eq!(
        fs::metadata(&path_to_key).unwrap().permissions().mode() & 0o077,
        0
    );
    let (_, second_host) = identity(&temp.path().join("host2")).unwrap();
    let server = MockServer::start().await;
    let boat = Boat {
        client: boat_sdk::Client::from_config(
            boat_sdk::Config::new("example-key")
                .unwrap()
                .with_base_url(&server.uri())
                .unwrap(),
        )
        .unwrap(),
        account: "account-1".into(),
    };
    let mut target = NativeTarget {
        machine: MachineId {
            host: Host::Boat,
            account: "another".into(),
            id: "vm-1".into(),
        },
        repository: "example/project".into(),
    };
    assert!(authorize(&boat, &target, public.clone()).await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
    target.machine.account.clone_from(&boat.account);
    for (address, port, host_key) in [
        (None, 22, &public),
        (Some("ssh.example.org:2222"), 2222, &second_host),
    ] {
        let mock = Mock::given(method("POST")).and(path("/sandboxes/vm-1/sshkey"))
            .and(body_json(json!({"key":public}))).respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "ok":true,"type":"sshkey","sshEndpoint":address,"machineIp":"192.0.2.3","hostKey":host_key,"sshUser":"user"
            }))).expect(1).mount_as_scoped(&server).await;
        let endpoint =
            Endpoint::new(authorize(&boat, &target, public.clone()).await.unwrap()).unwrap();
        assert_eq!(endpoint.port, port);
        assert_eq!(
            endpoint.host,
            if address.is_some() {
                "ssh.example.org"
            } else {
                "192.0.2.3"
            }
        );
        assert_eq!(
            endpoint.pin,
            format!(
                "caelum-boat ssh-ed25519 {}\n",
                host_key.split_whitespace().nth(1).unwrap()
            )
        );
        drop(mock);
    }
}

#[test]
fn an_incomplete_pin_or_unusable_forwarded_endpoint_never_falls_back() {
    for (addr, key) in [
        (None, None),
        (Some("-f:22"), Some("ssh-ed25519 AAAA")),
        (Some("host:0"), Some("ssh-ed25519 AAAA")),
        (Some("host:222"), Some("ssh-ed25519 AAAA\nextra")),
        (None, Some("ssh-ed25519 invalid!!!")),
    ] {
        assert!(Endpoint::new(serde_json::from_value(json!({"ok":true,"type":"sshkey","machineIp":"192.0.2.1","sshEndpoint":addr,"hostKey":key})).unwrap()).is_err());
    }
}
