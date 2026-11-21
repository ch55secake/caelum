use super::*;
use crate::remote::Host;

#[test]
fn retiring_one_machine_removes_its_repositories_and_rejects_late_writes() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("workspace.json");
    fs::write(&parent, "laptop").unwrap();
    let target = |account: &str, id: &str, repository: &str| NativeTarget {
        machine: MachineId {
            host: Host::Boat,
            account: account.into(),
            id: id.into(),
        },
        repository: repository.into(),
        checkout_root: "/workspace/checkout".into(),
    };
    let first = target("one", "vm-a", "example/project");
    let second = target("one", "vm-a", "example/other");
    let neighbors = [
        target("two", "vm-a", "example/project"),
        target("one", "vm-b", "example/project"),
    ];
    for t in [&first, &second].into_iter().chain(&neighbors) {
        t.with_live_profile(&parent, || {
            let path = t.store_path(&parent);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "independent draft").unwrap();
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(
        first.store_path(&parent),
        target("one", "vm-a", "Example/Project").store_path(&parent)
    );
    retire(&parent, &first.machine).unwrap();
    for t in [first, second] {
        assert!(!t.store_path(&parent).exists());
        assert!(t
            .with_live_profile(&parent, || -> Result<(), String> {
                panic!("retired writer")
            })
            .is_err());
    }
    for t in neighbors {
        assert_eq!(
            fs::read_to_string(t.store_path(&parent)).unwrap(),
            "independent draft"
        );
    }
    assert_eq!(fs::read_to_string(parent).unwrap(), "laptop");
}
