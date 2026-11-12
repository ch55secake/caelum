use super::*;
use caelum_core::application::{Provider, Scope};

#[test]
fn native_content_preserves_scoped_unicode_targets_and_literal_text() {
    let notification = Notification {
        title: "Caelum · Question".into(),
        body: "A literal \"quote\", $variable and café ☁️".into(),
        target: Some(SessionKey {
            scope: Scope {
                project: "/projects/café #1 & 2".into(),
                provider: Provider::OpenCode,
            },
            id: "session-1".into(),
        }),
    };
    let content = content(&notification);
    assert_eq!(content.title().to_string(), notification.title);
    assert_eq!(content.body().to_string(), notification.body);
    assert_eq!(target(&content), notification.target);
    let invalid =
        NSDictionary::from_slices(&[ns_string!("caelum.session")], &[ns_string!("not JSON")]);
    unsafe { content.setUserInfo(invalid.cast_unchecked()) };
    assert!(target(&content).is_none());
    assert!(target(&UNMutableNotificationContent::new()).is_none());
}
