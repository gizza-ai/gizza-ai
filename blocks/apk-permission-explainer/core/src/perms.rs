//! Plain-English descriptions and risk categories for Android permissions.
//!
//! Categories follow the platform's protection levels, split so the two things
//! a reader actually wants — "does this prompt me?" and "is this quietly
//! privacy-relevant?" — are separate:
//!
//! * `Dangerous` — runtime permissions; Android shows a consent dialog.
//! * `PrivacySensitive` — granted at install or via a Settings toggle with no
//!   runtime prompt, yet with real reach (overlays, sideloading, app inventory,
//!   the advertising id, all-files access).
//! * `Signature` — only granted to system apps or apps signed with the same key
//!   as the declaring app; harmless in a normal store APK, a red flag otherwise.
//! * `Normal` — auto-granted, low reach.
//! * `Unknown` — app-, SDK- or vendor-defined; not part of the platform set.

/// Risk bucket a permission falls into. Ordering is the report's section order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Dangerous,
    PrivacySensitive,
    Signature,
    Normal,
    Unknown,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::Dangerous => "Dangerous",
            Risk::PrivacySensitive => "Privacy-sensitive",
            Risk::Signature => "Signature / system",
            Risk::Normal => "Normal",
            Risk::Unknown => "Unknown / app-defined",
        }
    }

    /// Machine-friendly slug — used by `mode=csv`/`mode=json` and the `risk` filter.
    pub fn slug(self) -> &'static str {
        match self {
            Risk::Dangerous => "dangerous",
            Risk::PrivacySensitive => "privacy-sensitive",
            Risk::Signature => "signature",
            Risk::Normal => "normal",
            Risk::Unknown => "unknown",
        }
    }

    /// One line explaining what the bucket means, printed under the summary.
    pub fn meaning(self) -> &'static str {
        match self {
            Risk::Dangerous => {
                "runtime permission — Android asks the user before the app may use it"
            }
            Risk::PrivacySensitive => {
                "no runtime prompt, but wide reach over privacy or device control"
            }
            Risk::Signature => {
                "only granted to system apps or apps signed with the declaring app's key"
            }
            Risk::Normal => "granted automatically at install; limited reach",
            Risk::Unknown => "defined by the app, an SDK or the device vendor, not by Android",
        }
    }
}

/// Table of the permissions a consumer APK realistically requests. Entries are
/// stored without the `android.permission.` prefix where they carry it;
/// vendor/Google permissions are stored fully qualified.
const TABLE: &[(&str, Risk, &str)] = &[
    // ---- Camera & microphone -------------------------------------------------
    ("CAMERA", Risk::Dangerous, "Take photos and record video with the cameras at any time while the app is in use."),
    ("RECORD_AUDIO", Risk::Dangerous, "Record audio from the microphone."),
    ("CAPTURE_AUDIO_OUTPUT", Risk::Signature, "Capture audio that other apps are playing."),
    ("HIGH_SAMPLING_RATE_SENSORS", Risk::Normal, "Read motion sensors faster than 200 Hz, which can reveal fine-grained gestures."),
    // ---- Location ------------------------------------------------------------
    ("ACCESS_FINE_LOCATION", Risk::Dangerous, "Read your precise location from GPS and network sources."),
    ("ACCESS_COARSE_LOCATION", Risk::Dangerous, "Read your approximate location, typically to within a city block."),
    ("ACCESS_BACKGROUND_LOCATION", Risk::Dangerous, "Keep reading your location while the app is closed or in the background."),
    ("ACCESS_MEDIA_LOCATION", Risk::Dangerous, "Read the GPS coordinates embedded in your photos and videos."),
    ("ACCESS_LOCATION_EXTRA_COMMANDS", Risk::Normal, "Send low-level commands to the GPS provider; does not by itself reveal location."),
    ("LOCATION_HARDWARE", Risk::Signature, "Use location hardware directly, bypassing the normal location providers."),
    // ---- Contacts & accounts -------------------------------------------------
    ("READ_CONTACTS", Risk::Dangerous, "Read your entire contact list, including names, numbers and email addresses."),
    ("WRITE_CONTACTS", Risk::Dangerous, "Add, change or delete entries in your contact list."),
    ("GET_ACCOUNTS", Risk::Dangerous, "List the accounts signed in on the device, which usually exposes your email addresses."),
    ("AUTHENTICATE_ACCOUNTS", Risk::Normal, "Act as an account authenticator for its own account type (legacy, API 22 and below)."),
    ("MANAGE_ACCOUNTS", Risk::Normal, "Add or remove accounts of its own type (legacy, API 22 and below)."),
    ("USE_CREDENTIALS", Risk::Normal, "Request authentication tokens for accounts on the device (legacy, API 22 and below)."),
    // ---- Calendar ------------------------------------------------------------
    ("READ_CALENDAR", Risk::Dangerous, "Read every event in your calendars, including titles, guests and locations."),
    ("WRITE_CALENDAR", Risk::Dangerous, "Create, edit or delete calendar events, and email guests on your behalf."),
    // ---- Phone ---------------------------------------------------------------
    ("READ_PHONE_STATE", Risk::Dangerous, "Read phone status and identity, including whether a call is active and the carrier/SIM details."),
    ("READ_PHONE_NUMBERS", Risk::Dangerous, "Read the phone numbers of the SIMs in the device."),
    ("CALL_PHONE", Risk::Dangerous, "Place phone calls without going through the dialler, so without your confirmation."),
    ("ANSWER_PHONE_CALLS", Risk::Dangerous, "Answer incoming calls programmatically."),
    ("READ_CALL_LOG", Risk::Dangerous, "Read your call history: who called, when and for how long."),
    ("WRITE_CALL_LOG", Risk::Dangerous, "Add to or delete entries from your call history."),
    ("PROCESS_OUTGOING_CALLS", Risk::Dangerous, "See the number of every outgoing call, and redirect or abort it (deprecated in API 29)."),
    ("ADD_VOICEMAIL", Risk::Dangerous, "Add voicemail messages to the system voicemail inbox."),
    ("USE_SIP", Risk::Dangerous, "Make and receive internet (SIP) calls."),
    ("READ_PRIVILEGED_PHONE_STATE", Risk::Signature, "Read privileged identifiers such as the IMEI; reserved for system apps."),
    ("CALL_PRIVILEGED", Risk::Signature, "Call any number, including emergency numbers, bypassing the dialler."),
    ("MODIFY_PHONE_STATE", Risk::Signature, "Change telephony state, such as enabling or disabling radios."),
    // ---- SMS / messaging -----------------------------------------------------
    ("SEND_SMS", Risk::Dangerous, "Send text messages, which can cost money. A common abuse route for premium-rate fraud."),
    ("RECEIVE_SMS", Risk::Dangerous, "Receive and read incoming text messages, including one-time login codes."),
    ("READ_SMS", Risk::Dangerous, "Read text messages already stored on the device."),
    ("RECEIVE_MMS", Risk::Dangerous, "Receive and read incoming multimedia messages."),
    ("RECEIVE_WAP_PUSH", Risk::Dangerous, "Receive WAP push messages, a legacy carrier message channel."),
    ("BROADCAST_SMS", Risk::Signature, "Broadcast an SMS-received notification; reserved for the system."),
    // ---- Body & activity sensors --------------------------------------------
    ("BODY_SENSORS", Risk::Dangerous, "Read body sensors such as heart rate."),
    ("BODY_SENSORS_BACKGROUND", Risk::Dangerous, "Keep reading body sensors while the app is in the background (Android 13+)."),
    ("ACTIVITY_RECOGNITION", Risk::Dangerous, "Detect your physical activity — walking, cycling, driving, step count."),
    // ---- Storage & media -----------------------------------------------------
    ("READ_EXTERNAL_STORAGE", Risk::Dangerous, "Read files in shared storage: photos, downloads and documents (replaced by the READ_MEDIA_* permissions on Android 13+)."),
    ("WRITE_EXTERNAL_STORAGE", Risk::Dangerous, "Write to shared storage. Has no effect on Android 11+ outside legacy mode."),
    ("READ_MEDIA_IMAGES", Risk::Dangerous, "Read the photos in your shared media library (Android 13+)."),
    ("READ_MEDIA_VIDEO", Risk::Dangerous, "Read the videos in your shared media library (Android 13+)."),
    ("READ_MEDIA_AUDIO", Risk::Dangerous, "Read the music and audio files in your shared media library (Android 13+)."),
    ("READ_MEDIA_VISUAL_USER_SELECTED", Risk::Dangerous, "Read only the photos and videos you pick in the system photo picker (Android 14+). A narrower alternative to full media access."),
    ("MANAGE_EXTERNAL_STORAGE", Risk::PrivacySensitive, "Read and write nearly every file in shared storage. Granted from a separate Settings screen, not a prompt, and restricted on Google Play."),
    ("MANAGE_MEDIA", Risk::PrivacySensitive, "Modify or delete media files without a confirmation prompt each time."),
    ("MOUNT_UNMOUNT_FILESYSTEMS", Risk::Signature, "Mount and unmount storage volumes; reserved for the system."),
    // ---- Notifications & alarms ---------------------------------------------
    ("POST_NOTIFICATIONS", Risk::Dangerous, "Show notifications. Prompted since Android 13; auto-granted on older versions."),
    ("ACCESS_NOTIFICATION_POLICY", Risk::Normal, "Change Do Not Disturb settings."),
    ("BIND_NOTIFICATION_LISTENER_SERVICE", Risk::Signature, "Read the content of every notification on the device once the user enables it in Settings. Highly privacy-relevant."),
    ("SCHEDULE_EXACT_ALARM", Risk::Normal, "Schedule alarms that fire at an exact time, waking the device. Revocable in Settings on Android 13+."),
    ("USE_EXACT_ALARM", Risk::Normal, "Schedule exact alarms without a user toggle; Google Play restricts it to alarm and calendar apps."),
    ("SET_ALARM", Risk::Normal, "Hand an alarm to the installed clock app."),
    ("com.android.alarm.permission.SET_ALARM", Risk::Normal, "Hand an alarm to the installed clock app."),
    ("USE_FULL_SCREEN_INTENT", Risk::Normal, "Show full-screen notifications, as a call or alarm app does."),
    // ---- Network -------------------------------------------------------------
    ("INTERNET", Risk::Normal, "Open network connections. Nearly every app requests this; it is what lets any other collected data leave the device."),
    ("ACCESS_NETWORK_STATE", Risk::Normal, "See whether the device is online and on what kind of connection."),
    ("ACCESS_WIFI_STATE", Risk::Normal, "Read Wi-Fi state, including the list of configured and nearby networks, which can hint at location."),
    ("CHANGE_WIFI_STATE", Risk::Normal, "Turn Wi-Fi on or off and connect to networks."),
    ("CHANGE_NETWORK_STATE", Risk::Normal, "Change network connectivity state."),
    ("CHANGE_WIFI_MULTICAST_STATE", Risk::Normal, "Receive multicast Wi-Fi packets, used for local device discovery."),
    ("NEARBY_WIFI_DEVICES", Risk::Dangerous, "Find and connect to nearby Wi-Fi devices without needing location access (Android 13+)."),
    ("BIND_VPN_SERVICE", Risk::Signature, "Act as a VPN, which routes all device traffic through the app once the user approves it."),
    // ---- Bluetooth & nearby --------------------------------------------------
    ("BLUETOOTH", Risk::Normal, "Connect to already-paired Bluetooth devices (Android 11 and below)."),
    ("BLUETOOTH_ADMIN", Risk::Normal, "Discover and pair Bluetooth devices (Android 11 and below)."),
    ("BLUETOOTH_PRIVILEGED", Risk::Signature, "Pair Bluetooth devices without user confirmation; reserved for the system."),
    ("BLUETOOTH_SCAN", Risk::Dangerous, "Scan for nearby Bluetooth devices (Android 12+). Bluetooth beacons are a well-known location-tracking signal."),
    ("BLUETOOTH_CONNECT", Risk::Dangerous, "Connect to paired Bluetooth devices and read their names (Android 12+)."),
    ("BLUETOOTH_ADVERTISE", Risk::Dangerous, "Make the device discoverable to nearby Bluetooth devices (Android 12+)."),
    ("UWB_RANGING", Risk::Dangerous, "Measure precise distance to nearby ultra-wideband devices."),
    ("NFC", Risk::Normal, "Use near-field communication, for tags and contactless payments."),
    // ---- Identity & app inventory -------------------------------------------
    ("com.google.android.gms.permission.AD_ID", Risk::PrivacySensitive, "Read the Google Advertising ID, a resettable device identifier used to profile you across apps for advertising."),
    ("QUERY_ALL_PACKAGES", Risk::PrivacySensitive, "See the full list of apps installed on the device (Android 11+). An app inventory is a strong fingerprinting and profiling signal; Google Play restricts this."),
    ("PACKAGE_USAGE_STATS", Risk::PrivacySensitive, "See which apps you use and for how long, once you enable it in Settings."),
    ("GET_TASKS", Risk::PrivacySensitive, "See which apps are running (deprecated since Android 5; returns only the caller's own tasks)."),
    ("REQUEST_INSTALL_PACKAGES", Risk::PrivacySensitive, "Prompt you to install other APKs. Expected in app stores and updaters; unexpected elsewhere, and a common sideloaded-malware route."),
    ("REQUEST_DELETE_PACKAGES", Risk::Normal, "Ask you to uninstall an app; you still confirm each removal."),
    ("INSTALL_PACKAGES", Risk::Signature, "Install apps silently, with no prompt. Reserved for system apps."),
    ("DELETE_PACKAGES", Risk::Signature, "Uninstall apps silently. Reserved for system apps."),
    ("GET_PACKAGE_SIZE", Risk::Normal, "Read how much storage other apps occupy."),
    // ---- Display, input & device control ------------------------------------
    ("SYSTEM_ALERT_WINDOW", Risk::PrivacySensitive, "Draw windows on top of every other app. Granted from a Settings screen, and the mechanism behind overlay/tap-jacking attacks and fake login screens."),
    ("HIDE_OVERLAY_WINDOWS", Risk::Normal, "Ask the system to hide other apps' overlays over its own windows — a defensive permission."),
    ("WRITE_SETTINGS", Risk::PrivacySensitive, "Change system settings such as brightness and ringtone, after you allow it in Settings."),
    ("WRITE_SECURE_SETTINGS", Risk::Signature, "Change protected system settings. Reserved for system apps."),
    ("BIND_ACCESSIBILITY_SERVICE", Risk::Signature, "Run as an accessibility service: once the user enables it, the app can read everything on screen and act on your behalf. Legitimate for assistive apps and heavily abused by malware."),
    ("BIND_INPUT_METHOD", Risk::Signature, "Act as a keyboard, which means seeing everything you type once selected."),
    ("BIND_DEVICE_ADMIN", Risk::Signature, "Act as a device administrator: enforce lock-screen policy, wipe the device, and resist uninstallation."),
    ("BIND_ACCESSIBILITY_SERVICE_ALIAS", Risk::Signature, "Accessibility-service binding alias."),
    ("MEDIA_CONTENT_CONTROL", Risk::Signature, "See and control what media any app is playing. Reserved for the system."),
    ("DISABLE_KEYGUARD", Risk::Normal, "Dismiss the lock screen when it has no password set."),
    ("TURN_SCREEN_ON", Risk::Signature, "Turn the display on programmatically."),
    ("WAKE_LOCK", Risk::Normal, "Keep the device awake, which can drain the battery."),
    ("VIBRATE", Risk::Normal, "Use the vibrator."),
    ("FLASHLIGHT", Risk::Normal, "Control the camera flash (legacy permission)."),
    ("SET_WALLPAPER", Risk::Normal, "Change the wallpaper."),
    ("SET_WALLPAPER_HINTS", Risk::Normal, "Set wallpaper size hints."),
    ("EXPAND_STATUS_BAR", Risk::Normal, "Expand or collapse the status bar."),
    ("REORDER_TASKS", Risk::Normal, "Move its own tasks to the foreground or background."),
    ("KILL_BACKGROUND_PROCESSES", Risk::Normal, "Ask the system to end other apps' background processes."),
    ("MODIFY_AUDIO_SETTINGS", Risk::Normal, "Change global audio settings such as volume and routing."),
    ("DETECT_SCREEN_CAPTURE", Risk::Normal, "Be told when the user screenshots the app."),
    ("BATTERY_STATS", Risk::Signature, "Read detailed per-app battery statistics."),
    ("REQUEST_IGNORE_BATTERY_OPTIMIZATIONS", Risk::Normal, "Ask to be exempted from battery optimisation so it can run freely in the background."),
    ("DUMP", Risk::Signature, "Dump internal system state. Reserved for the system."),
    ("READ_LOGS", Risk::Signature, "Read the system log, which can contain other apps' data. Reserved for the system."),
    // ---- Biometrics ----------------------------------------------------------
    ("USE_BIOMETRIC", Risk::Normal, "Prompt for fingerprint or face unlock to authenticate you in-app."),
    ("USE_FINGERPRINT", Risk::Normal, "Prompt for fingerprint authentication (deprecated; replaced by USE_BIOMETRIC)."),
    // ---- Background execution & sync ----------------------------------------
    ("RECEIVE_BOOT_COMPLETED", Risk::Normal, "Start automatically when the device finishes booting."),
    ("FOREGROUND_SERVICE", Risk::Normal, "Run a foreground service, which shows a persistent notification."),
    ("FOREGROUND_SERVICE_CAMERA", Risk::Normal, "Run a foreground service that uses the camera (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_MICROPHONE", Risk::Normal, "Run a foreground service that uses the microphone (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_LOCATION", Risk::Normal, "Run a foreground service that uses location (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_DATA_SYNC", Risk::Normal, "Run a foreground service for data sync (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_MEDIA_PLAYBACK", Risk::Normal, "Run a foreground service for media playback (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_MEDIA_PROJECTION", Risk::Normal, "Run a foreground service that records the screen (Android 14+ type declaration); screen capture still needs your per-session consent."),
    ("FOREGROUND_SERVICE_CONNECTED_DEVICE", Risk::Normal, "Run a foreground service for a connected device (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_HEALTH", Risk::Normal, "Run a foreground service for health and fitness tracking (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_PHONE_CALL", Risk::Normal, "Run a foreground service for an ongoing call (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_REMOTE_MESSAGING", Risk::Normal, "Run a foreground service for remote messaging (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_SHORT_SERVICE", Risk::Normal, "Run a short foreground service (Android 14+ type declaration)."),
    ("FOREGROUND_SERVICE_SPECIAL_USE", Risk::Normal, "Run a foreground service with a custom purpose (Android 14+ type declaration); Google Play asks for a justification."),
    ("FOREGROUND_SERVICE_SYSTEM_EXEMPTED", Risk::Normal, "Run a system-exempted foreground service (Android 14+ type declaration)."),
    ("READ_SYNC_SETTINGS", Risk::Normal, "Read account sync settings."),
    ("WRITE_SYNC_SETTINGS", Risk::Normal, "Turn account sync on or off."),
    ("READ_SYNC_STATS", Risk::Normal, "Read sync history for accounts."),
    ("BROADCAST_STICKY", Risk::Normal, "Send sticky broadcasts (deprecated)."),
    ("CHANGE_CONFIGURATION", Risk::Signature, "Change the device configuration, such as locale."),
    // ---- Store, push and shortcut permissions --------------------------------
    ("com.android.vending.BILLING", Risk::Normal, "Sell in-app purchases and subscriptions through Google Play Billing."),
    ("com.android.vending.CHECK_LICENSE", Risk::Normal, "Check the Google Play licence for a paid app."),
    ("com.google.android.c2dm.permission.RECEIVE", Risk::Normal, "Receive push messages through Firebase Cloud Messaging."),
    ("com.google.android.finsky.permission.BIND_GET_INSTALL_REFERRER_SERVICE", Risk::Normal, "Read the Play Store install referrer, which tells the developer which campaign led to the install."),
    ("com.google.android.providers.gsf.permission.READ_GSERVICES", Risk::Normal, "Read Google services framework settings, used by older Google Play services SDKs."),
    ("com.android.launcher.permission.INSTALL_SHORTCUT", Risk::PrivacySensitive, "Add home-screen shortcuts without asking. Launchers ignore it on modern Android."),
    ("com.android.launcher.permission.UNINSTALL_SHORTCUT", Risk::Normal, "Remove home-screen shortcuts it created."),
    ("INSTALL_SHORTCUT", Risk::PrivacySensitive, "Add home-screen shortcuts without asking."),
    ("UNINSTALL_SHORTCUT", Risk::Normal, "Remove home-screen shortcuts."),
];

/// What a permission does and how risky it is.
pub struct Info {
    pub risk: Risk,
    pub description: String,
}

/// Look a permission up by its fully-qualified name, falling back to
/// prefix-based heuristics so unrecognised names still get a useful verdict.
pub fn lookup(full: &str) -> Info {
    let short = full.strip_prefix("android.permission.").unwrap_or(full);
    if let Some((_, risk, desc)) = TABLE.iter().find(|(k, _, _)| *k == short || *k == full) {
        return Info {
            risk: *risk,
            description: (*desc).to_string(),
        };
    }

    // Heuristics, in order of how much they actually tell the reader.
    if short.starts_with("BIND_") {
        return Info {
            risk: Risk::Signature,
            description: "Lets a system component bind to one of the app's services. Granted only to the matching system role.".into(),
        };
    }
    if full.ends_with(".permission.C2D_MESSAGE")
        || full.ends_with(".permission.RECEIVE_ADM_MESSAGE")
    {
        return Info {
            risk: Risk::Normal,
            description: "App-specific push-messaging delivery permission, declared so only the push service can wake this app.".into(),
        };
    }
    if full.ends_with(".DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION") {
        return Info {
            risk: Risk::Normal,
            description: "Generated by AndroidX to keep the app's own runtime-registered broadcast receivers private.".into(),
        };
    }
    if full.starts_with("android.permission.") {
        return Info {
            risk: Risk::Unknown,
            description: "An Android platform permission this tool does not have a description for. Check the official permission reference for its protection level.".into(),
        };
    }
    Info {
        risk: Risk::Unknown,
        description: "Defined by the app itself, a bundled SDK or the device vendor rather than by Android. Its meaning depends on whoever declared it.".into(),
    }
}
