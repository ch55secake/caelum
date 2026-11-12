use block2::{DynBlock, RcBlock};
use caelum_core::application::{Event, Notification, Reply, SessionKey};
use objc2::{
    AnyThread, DefinedClass, define_class, msg_send,
    rc::Retained,
    runtime::{Bool, ProtocolObject},
};
use objc2_foundation::{
    NSBundle, NSDictionary, NSError, NSObject, NSObjectProtocol, NSString, NSUUID, ns_string,
};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationDefaultActionIdentifier, UNNotificationPresentationOptions,
    UNNotificationRequest, UNNotificationResponse, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};
use rune::AppHandle;

use crate::Workspace;

define_class!(
    // NSObject has no subclassing requirements. The ivar is a thread-safe Rune
    // handle: native callbacks queue work rather than touching UI state.
    #[unsafe(super = NSObject)]
    #[name = "CaelumNotificationDelegate"]
    #[ivars = AppHandle<Workspace>]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn present(&self, _: &UNUserNotificationCenter, _: &UNNotification, completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>) {
            // Core has already applied the foreground/background preference.
            // Sound is the existing volume-controlled effect, not a second alert.
            completion.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn response(&self, _: &UNUserNotificationCenter, response: &UNNotificationResponse, completion: &DynBlock<dyn Fn()>) {
            // Dismissing a banner must not activate the app.
            if *response.actionIdentifier() == *unsafe { UNNotificationDefaultActionIdentifier } {
                let target = target(&response.notification().request().content());
                self.ivars().update(move |s, _| super::clicked(s, target));
            }
            completion.call(());
        }
    }
);

pub(super) struct Client {
    center: Retained<UNUserNotificationCenter>,
    // UNUserNotificationCenter retains its delegate weakly.
    _delegate: Retained<Delegate>,
    handle: AppHandle<Workspace>,
}

impl Client {
    pub(super) fn new(handle: AppHandle<Workspace>) -> Result<Self, String> {
        let bundle = NSBundle::mainBundle();
        if bundle.bundleIdentifier().is_none() || !bundle.bundlePath().to_string().ends_with(".app")
        {
            return Err("Native notifications require an app bundle; run make dev".into());
        }
        // Register before NSApplication finishes launching so clicks can also
        // reach a cold app. Authorization is requested only on first delivery.
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let delegate: Retained<Delegate> =
            unsafe { msg_send![super(Delegate::alloc().set_ivars(handle.clone())), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Ok(Self {
            center,
            _delegate: delegate,
            handle,
        })
    }

    pub(super) fn send(&self, id: u64, notification: Notification) {
        let content = content(&notification);
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSUUID::new().UUIDString(),
            &content,
            None,
        );
        let center = self.center.clone();
        let handle = self.handle.clone();
        self.center
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert,
                &RcBlock::new(move |granted: Bool, error: *mut NSError| {
                    if let Some(error) = error_message(error) {
                        complete(&handle, id, Err(error));
                    } else if !granted.as_bool() {
                        // Denial is a system preference, not a failed provider operation.
                        complete(&handle, id, Ok(Reply::Done));
                    } else {
                        let handle = handle.clone();
                        center.addNotificationRequest_withCompletionHandler(
                            &request,
                            Some(&RcBlock::new(move |error: *mut NSError| {
                                complete(
                                    &handle,
                                    id,
                                    error_message(error).map_or(Ok(Reply::Done), Err),
                                );
                            })),
                        );
                    }
                }),
            );
    }
}

fn content(notification: &Notification) -> Retained<UNMutableNotificationContent> {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(&notification.title));
    content.setBody(&NSString::from_str(&notification.body));
    if let Some(target) = &notification.target {
        let json =
            NSString::from_str(&serde_json::to_string(target).expect("serializable session key"));
        let info = NSDictionary::from_slices(&[ns_string!("caelum.session")], &[&*json]);
        // Both keys and values are immutable, property-list-compatible strings.
        unsafe { content.setUserInfo(info.cast_unchecked()) };
    }
    content
}

fn target(content: &objc2_user_notifications::UNNotificationContent) -> Option<SessionKey> {
    let value = content
        .userInfo()
        .objectForKey(ns_string!("caelum.session"))?;
    serde_json::from_str(&value.downcast_ref::<NSString>()?.to_string()).ok()
}

fn error_message(error: *mut NSError) -> Option<String> {
    // The system owns a nullable NSError for the duration of each callback.
    unsafe { error.as_ref() }.map(|error| error.localizedDescription().to_string())
}

fn complete(handle: &AppHandle<Workspace>, id: u64, result: Result<Reply, String>) {
    if let Err(error) = &result {
        eprintln!("Notification: {error}");
    }
    handle.update(move |s, _| {
        if let Some(live) = &mut s.live {
            live.apply(Event::Completed(id, result));
        }
    });
}

#[cfg(test)]
mod tests;
