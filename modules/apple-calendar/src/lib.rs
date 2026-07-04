//! Apple Calendar gateway backed by EventKit.
//!
//! This crate is intentionally Apple-specific. Non-Apple production builds fail
//! clearly unless `test-support` is enabled for mock-only tests.

#[cfg(all(
    not(any(target_os = "macos", target_os = "ios")),
    not(feature = "test-support")
))]
compile_error!(
    "apple-calendar requires an Apple target (macOS or iOS). Do not enable mod-apple-calendar on non-Apple builds."
);

use std::sync::Arc;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppleCalendarError>;

/// Milliseconds since the Unix epoch, UTC.
pub type UnixMillis = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessStatus {
    NotDetermined,
    Restricted,
    Denied,
    FullAccess,
    WriteOnly,
    Unknown,
}

impl AccessStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            AccessStatus::NotDetermined => "not_determined",
            AccessStatus::Restricted => "restricted",
            AccessStatus::Denied => "denied",
            AccessStatus::FullAccess => "full_access",
            AccessStatus::WriteOnly => "write_only",
            AccessStatus::Unknown => "unknown",
        }
    }

    pub fn has_full_access(self) -> bool {
        matches!(self, AccessStatus::FullAccess)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarStatus {
    pub access: AccessStatus,
    pub full_access: bool,
    pub supported: bool,
    pub platform: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarAccessResponse {
    pub access: AccessStatus,
    pub granted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarInfo {
    pub id: String,
    pub title: String,
    pub source: String,
    pub color: String,
    pub allows_modifications: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInfo {
    pub id: String,
    pub calendar_id: String,
    pub title: String,
    pub start_time: UnixMillis,
    pub end_time: UnixMillis,
    pub all_day: bool,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventQuery {
    pub start_time: UnixMillis,
    pub end_time: UnixMillis,
    pub calendar_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEventRequest {
    pub title: String,
    pub start_time: UnixMillis,
    pub end_time: UnixMillis,
    pub calendar_id: Option<String>,
    pub notes: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub all_day: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateEventRequest {
    pub event_id: String,
    pub title: Option<String>,
    pub start_time: Option<UnixMillis>,
    pub end_time: Option<UnixMillis>,
    pub calendar_id: Option<String>,
    pub notes: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub all_day: Option<bool>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppleCalendarError {
    #[error("calendar: full access required")]
    FullAccessRequired,

    #[error("calendar: access denied")]
    AccessDenied,

    #[error("calendar: access restricted")]
    AccessRestricted,

    #[error("calendar: access not determined; call calendar.request_access(\"full\") first")]
    AccessNotDetermined,

    #[error("calendar: write-only access is insufficient; full access required")]
    WriteOnlyAccess,

    #[error("calendar: unsupported OS; requires iOS 17 or macOS 14 or newer")]
    UnsupportedOs,

    #[error("calendar: not found: {0}")]
    NotFound(String),

    #[error("calendar: default calendar unavailable")]
    DefaultCalendarUnavailable,

    #[error("calendar: calendar is read-only")]
    ReadOnlyCalendar,

    #[error("calendar: recurring event mutations are not supported in V1")]
    RecurrenceMutationUnsupported,

    #[error("calendar: invalid input: {0}")]
    InvalidInput(String),

    #[error("calendar: EventKit error: {0}")]
    EventKit(String),

    #[error("calendar: backend unavailable: {0}")]
    BackendUnavailable(String),
}

pub trait AppleCalendarBackend: Send + Sync {
    fn status(&self) -> Result<CalendarStatus>;
    fn request_full_access(&self) -> Result<CalendarAccessResponse>;
    fn calendars(&self) -> Result<Vec<CalendarInfo>>;
    fn default_calendar(&self) -> Result<CalendarInfo>;
    fn events(&self, query: EventQuery) -> Result<Vec<EventInfo>>;
    fn get(&self, event_id: &str) -> Result<EventInfo>;
    fn create(&self, request: CreateEventRequest) -> Result<EventInfo>;
    fn update(&self, request: UpdateEventRequest) -> Result<EventInfo>;
    fn delete(&self, event_id: &str) -> Result<bool>;
}

/// Thin gateway wrapper so hosts can inject a mock backend or use EventKit.
pub struct AppleCalendarGateway {
    backend: Box<dyn AppleCalendarBackend>,
}

impl AppleCalendarGateway {
    pub fn new(backend: Box<dyn AppleCalendarBackend>) -> Self {
        Self { backend }
    }

    pub fn platform_default() -> Self {
        Self::new(platform_default_backend())
    }

    pub fn shared_platform_default() -> Arc<Self> {
        Arc::new(Self::platform_default())
    }

    pub fn status(&self) -> Result<CalendarStatus> {
        self.backend.status()
    }

    pub fn request_full_access(&self) -> Result<CalendarAccessResponse> {
        self.backend.request_full_access()
    }

    pub fn calendars(&self) -> Result<Vec<CalendarInfo>> {
        self.backend.calendars()
    }

    pub fn default_calendar(&self) -> Result<CalendarInfo> {
        self.backend.default_calendar()
    }

    pub fn events(&self, query: EventQuery) -> Result<Vec<EventInfo>> {
        self.backend.events(query)
    }

    pub fn get(&self, event_id: &str) -> Result<EventInfo> {
        self.backend.get(event_id)
    }

    pub fn create(&self, request: CreateEventRequest) -> Result<EventInfo> {
        self.backend.create(request)
    }

    pub fn update(&self, request: UpdateEventRequest) -> Result<EventInfo> {
        self.backend.update(request)
    }

    pub fn delete(&self, event_id: &str) -> Result<bool> {
        self.backend.delete(event_id)
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn platform_default_backend() -> Box<dyn AppleCalendarBackend> {
    Box::new(eventkit::EventKitBackend::new())
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn platform_default_backend() -> Box<dyn AppleCalendarBackend> {
    Box::new(UnsupportedBackend)
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
struct UnsupportedBackend;

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
impl AppleCalendarBackend for UnsupportedBackend {
    fn status(&self) -> Result<CalendarStatus> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn request_full_access(&self) -> Result<CalendarAccessResponse> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn calendars(&self) -> Result<Vec<CalendarInfo>> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn default_calendar(&self) -> Result<CalendarInfo> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn events(&self, _query: EventQuery) -> Result<Vec<EventInfo>> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn get(&self, _event_id: &str) -> Result<EventInfo> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn create(&self, _request: CreateEventRequest) -> Result<EventInfo> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn update(&self, _request: UpdateEventRequest) -> Result<EventInfo> {
        Err(AppleCalendarError::UnsupportedOs)
    }

    fn delete(&self, _event_id: &str) -> Result<bool> {
        Err(AppleCalendarError::UnsupportedOs)
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn ensure_full_access(status: AccessStatus) -> Result<()> {
    match status {
        AccessStatus::FullAccess => Ok(()),
        AccessStatus::NotDetermined => Err(AppleCalendarError::AccessNotDetermined),
        AccessStatus::Restricted => Err(AppleCalendarError::AccessRestricted),
        AccessStatus::Denied => Err(AppleCalendarError::AccessDenied),
        AccessStatus::WriteOnly => Err(AppleCalendarError::WriteOnlyAccess),
        AccessStatus::Unknown => Err(AppleCalendarError::FullAccessRequired),
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod eventkit {
    use super::*;
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::Bool;
    use objc2::AnyThread;
    use objc2_core_graphics::CGColor;
    use objc2_event_kit::{
        EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore, EKSpan,
    };
    use objc2_foundation::{
        NSArray, NSDate, NSError, NSOperatingSystemVersion, NSProcessInfo, NSString, NSURL,
    };
    use std::sync::mpsc;

    pub struct EventKitBackend {
        tx: mpsc::Sender<Command>,
    }

    impl EventKitBackend {
        pub fn new() -> Self {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("cpsl-apple-calendar".to_string())
                .spawn(move || worker_loop(rx))
                .expect("failed to spawn EventKit worker");
            Self { tx }
        }

        fn call<T>(&self, make: impl FnOnce(mpsc::Sender<Result<T>>) -> Command) -> Result<T> {
            let (reply_tx, reply_rx) = mpsc::channel();
            self.tx
                .send(make(reply_tx))
                .map_err(|_| AppleCalendarError::BackendUnavailable("worker stopped".into()))?;
            reply_rx
                .recv()
                .map_err(|_| AppleCalendarError::BackendUnavailable("worker stopped".into()))?
        }
    }

    impl Default for EventKitBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    impl AppleCalendarBackend for EventKitBackend {
        fn status(&self) -> Result<CalendarStatus> {
            self.call(Command::Status)
        }

        fn request_full_access(&self) -> Result<CalendarAccessResponse> {
            self.call(Command::RequestFullAccess)
        }

        fn calendars(&self) -> Result<Vec<CalendarInfo>> {
            self.call(Command::Calendars)
        }

        fn default_calendar(&self) -> Result<CalendarInfo> {
            self.call(Command::DefaultCalendar)
        }

        fn events(&self, query: EventQuery) -> Result<Vec<EventInfo>> {
            self.call(|tx| Command::Events(query, tx))
        }

        fn get(&self, event_id: &str) -> Result<EventInfo> {
            self.call(|tx| Command::Get(event_id.to_string(), tx))
        }

        fn create(&self, request: CreateEventRequest) -> Result<EventInfo> {
            self.call(|tx| Command::Create(request, tx))
        }

        fn update(&self, request: UpdateEventRequest) -> Result<EventInfo> {
            self.call(|tx| Command::Update(request, tx))
        }

        fn delete(&self, event_id: &str) -> Result<bool> {
            self.call(|tx| Command::Delete(event_id.to_string(), tx))
        }
    }

    enum Command {
        Status(mpsc::Sender<Result<CalendarStatus>>),
        RequestFullAccess(mpsc::Sender<Result<CalendarAccessResponse>>),
        Calendars(mpsc::Sender<Result<Vec<CalendarInfo>>>),
        DefaultCalendar(mpsc::Sender<Result<CalendarInfo>>),
        Events(EventQuery, mpsc::Sender<Result<Vec<EventInfo>>>),
        Get(String, mpsc::Sender<Result<EventInfo>>),
        Create(CreateEventRequest, mpsc::Sender<Result<EventInfo>>),
        Update(UpdateEventRequest, mpsc::Sender<Result<EventInfo>>),
        Delete(String, mpsc::Sender<Result<bool>>),
    }

    fn worker_loop(rx: mpsc::Receiver<Command>) {
        let store = unsafe { EKEventStore::init(EKEventStore::alloc()) };
        let worker = Worker { store };
        while let Ok(command) = rx.recv() {
            autoreleasepool(|_| {
                match command {
                    Command::Status(tx) => {
                        let _ = tx.send(worker.status());
                    }
                    Command::RequestFullAccess(tx) => {
                        let _ = tx.send(worker.request_full_access());
                    }
                    Command::Calendars(tx) => {
                        let _ = tx.send(worker.calendars());
                    }
                    Command::DefaultCalendar(tx) => {
                        let _ = tx.send(worker.default_calendar());
                    }
                    Command::Events(query, tx) => {
                        let _ = tx.send(worker.events(query));
                    }
                    Command::Get(event_id, tx) => {
                        let _ = tx.send(worker.get(&event_id));
                    }
                    Command::Create(request, tx) => {
                        let _ = tx.send(worker.create(request));
                    }
                    Command::Update(request, tx) => {
                        let _ = tx.send(worker.update(request));
                    }
                    Command::Delete(event_id, tx) => {
                        let _ = tx.send(worker.delete(&event_id));
                    }
                }
            });
        }
    }

    struct Worker {
        store: Retained<EKEventStore>,
    }

    impl Worker {
        fn status(&self) -> Result<CalendarStatus> {
            Ok(CalendarStatus {
                access: access_status(),
                full_access: access_status().has_full_access(),
                supported: os_supported(),
                platform: platform_name().to_string(),
            })
        }

        fn request_full_access(&self) -> Result<CalendarAccessResponse> {
            if !os_supported() {
                return Err(AppleCalendarError::UnsupportedOs);
            }

            let (tx, rx) = mpsc::channel();
            let block = block2::RcBlock::new(move |granted: Bool, error: *mut NSError| {
                let result = if !error.is_null() {
                    let err = unsafe { &*error };
                    Err(AppleCalendarError::EventKit(
                        err.localizedDescription().to_string(),
                    ))
                } else {
                    Ok(CalendarAccessResponse {
                        access: access_status(),
                        granted: granted.as_bool(),
                    })
                };
                let _ = tx.send(result);
            });

            unsafe {
                self.store
                    .requestFullAccessToEventsWithCompletion(block2::RcBlock::as_ptr(&block));
            }

            rx.recv()
                .map_err(|_| AppleCalendarError::BackendUnavailable("access callback dropped".into()))?
        }

        fn calendars(&self) -> Result<Vec<CalendarInfo>> {
            self.require_full_access()?;
            let calendars =
                unsafe { self.store.calendarsForEntityType(EKEntityType::Event) };
            Ok(calendars
                .to_vec()
                .iter()
                .map(|calendar| calendar_snapshot(calendar))
                .collect())
        }

        fn default_calendar(&self) -> Result<CalendarInfo> {
            self.require_full_access()?;
            let calendar = unsafe { self.store.defaultCalendarForNewEvents() }
                .ok_or(AppleCalendarError::DefaultCalendarUnavailable)?;
            Ok(calendar_snapshot(&calendar))
        }

        fn events(&self, query: EventQuery) -> Result<Vec<EventInfo>> {
            self.require_full_access()?;
            if query.end_time <= query.start_time {
                return Err(AppleCalendarError::InvalidInput(
                    "end_time must be after start_time".into(),
                ));
            }

            let start = nsdate_from_unix_millis(query.start_time);
            let end = nsdate_from_unix_millis(query.end_time);
            let calendar_array = self.calendar_filter(query.calendar_id.as_deref())?;
            let predicate = unsafe {
                self.store.predicateForEventsWithStartDate_endDate_calendars(
                    &start,
                    &end,
                    calendar_array.as_deref(),
                )
            };
            let events = unsafe { self.store.eventsMatchingPredicate(&predicate) };
            let mut snapshots = Vec::new();
            if query.limit == Some(0) {
                return Ok(snapshots);
            }
            for event in events.to_vec() {
                snapshots.push(event_snapshot(&event)?);
                if query.limit.is_some_and(|limit| snapshots.len() >= limit) {
                    break;
                }
            }
            Ok(snapshots)
        }

        fn get(&self, event_id: &str) -> Result<EventInfo> {
            self.require_full_access()?;
            let event = self.find_event(event_id)?;
            event_snapshot(&event)
        }

        fn create(&self, request: CreateEventRequest) -> Result<EventInfo> {
            self.require_full_access()?;
            if request.end_time <= request.start_time {
                return Err(AppleCalendarError::InvalidInput(
                    "end_time must be after start_time".into(),
                ));
            }

            let event = unsafe { EKEvent::eventWithEventStore(&self.store) };
            unsafe {
                event.setTitle(Some(&NSString::from_str(&request.title)));
                event.setStartDate(Some(&nsdate_from_unix_millis(request.start_time)));
                event.setEndDate(Some(&nsdate_from_unix_millis(request.end_time)));
                event.setAllDay(request.all_day);
                if let Some(notes) = request.notes.as_deref() {
                    event.setNotes(Some(&NSString::from_str(notes)));
                }
                if let Some(location) = request.location.as_deref() {
                    event.setLocation(Some(&NSString::from_str(location)));
                }
                if let Some(url) = request.url.as_deref() {
                    let ns_url = parse_url(url)?;
                    event.setURL(Some(&ns_url));
                }
                let calendar = self.target_calendar(request.calendar_id.as_deref())?;
                if !calendar.allowsContentModifications() {
                    return Err(AppleCalendarError::ReadOnlyCalendar);
                }
                event.setCalendar(Some(&calendar));
                self.store
                    .saveEvent_span_commit_error(&event, EKSpan::ThisEvent, true)
                    .map_err(nserror)?;
            }

            event_snapshot(&event)
        }

        fn update(&self, request: UpdateEventRequest) -> Result<EventInfo> {
            self.require_full_access()?;
            let event = self.find_event(&request.event_id)?;
            if unsafe { event.hasRecurrenceRules() } {
                return Err(AppleCalendarError::RecurrenceMutationUnsupported);
            }

            let effective_start = match request.start_time {
                Some(start) => start,
                None => {
                    let start = unsafe { event.startDate() };
                    unix_millis_from_nsdate(&start)
                }
            };
            let effective_end = match request.end_time {
                Some(end) => end,
                None => {
                    let end = unsafe { event.endDate() };
                    unix_millis_from_nsdate(&end)
                }
            };
            if effective_end <= effective_start {
                return Err(AppleCalendarError::InvalidInput(
                    "end_time must be after start_time".into(),
                ));
            }

            unsafe {
                if let Some(title) = request.title.as_deref() {
                    event.setTitle(Some(&NSString::from_str(title)));
                }
                if let Some(start) = request.start_time {
                    event.setStartDate(Some(&nsdate_from_unix_millis(start)));
                }
                if let Some(end) = request.end_time {
                    event.setEndDate(Some(&nsdate_from_unix_millis(end)));
                }
                if let Some(all_day) = request.all_day {
                    event.setAllDay(all_day);
                }
                if let Some(notes) = request.notes.as_deref() {
                    event.setNotes(Some(&NSString::from_str(notes)));
                }
                if let Some(location) = request.location.as_deref() {
                    event.setLocation(Some(&NSString::from_str(location)));
                }
                if let Some(url) = request.url.as_deref() {
                    let ns_url = parse_url(url)?;
                    event.setURL(Some(&ns_url));
                }
                if let Some(calendar_id) = request.calendar_id.as_deref() {
                    let calendar = self.target_calendar(Some(calendar_id))?;
                    if !calendar.allowsContentModifications() {
                        return Err(AppleCalendarError::ReadOnlyCalendar);
                    }
                    event.setCalendar(Some(&calendar));
                }
                self.store
                    .saveEvent_span_commit_error(&event, EKSpan::ThisEvent, true)
                    .map_err(nserror)?;
            }

            event_snapshot(&event)
        }

        fn delete(&self, event_id: &str) -> Result<bool> {
            self.require_full_access()?;
            let event = self.find_event(event_id)?;
            if unsafe { event.hasRecurrenceRules() } {
                return Err(AppleCalendarError::RecurrenceMutationUnsupported);
            }
            unsafe {
                self.store
                    .removeEvent_span_commit_error(&event, EKSpan::ThisEvent, true)
                    .map_err(nserror)?;
            }
            Ok(true)
        }

        fn require_full_access(&self) -> Result<()> {
            if !os_supported() {
                return Err(AppleCalendarError::UnsupportedOs);
            }
            ensure_full_access(access_status())
        }

        fn find_event(&self, event_id: &str) -> Result<Retained<EKEvent>> {
            let id = NSString::from_str(event_id);
            unsafe { self.store.eventWithIdentifier(&id) }
                .ok_or_else(|| AppleCalendarError::NotFound(event_id.to_string()))
        }

        fn target_calendar(&self, calendar_id: Option<&str>) -> Result<Retained<EKCalendar>> {
            match calendar_id {
                Some(id) => unsafe { self.store.calendarWithIdentifier(&NSString::from_str(id)) }
                    .ok_or_else(|| AppleCalendarError::NotFound(id.to_string())),
                None => unsafe { self.store.defaultCalendarForNewEvents() }
                    .ok_or(AppleCalendarError::DefaultCalendarUnavailable),
            }
        }

        fn calendar_filter(
            &self,
            calendar_id: Option<&str>,
        ) -> Result<Option<Retained<NSArray<EKCalendar>>>> {
            let Some(id) = calendar_id else {
                return Ok(None);
            };
            let calendar = self.target_calendar(Some(id))?;
            Ok(Some(NSArray::from_retained_slice(&[calendar])))
        }
    }

    fn access_status() -> AccessStatus {
        let status =
            unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) };
        if status == EKAuthorizationStatus::NotDetermined {
            AccessStatus::NotDetermined
        } else if status == EKAuthorizationStatus::Restricted {
            AccessStatus::Restricted
        } else if status == EKAuthorizationStatus::Denied {
            AccessStatus::Denied
        } else if status == EKAuthorizationStatus::FullAccess {
            AccessStatus::FullAccess
        } else if status == EKAuthorizationStatus::WriteOnly {
            AccessStatus::WriteOnly
        } else {
            AccessStatus::Unknown
        }
    }

    fn os_supported() -> bool {
        let min = NSOperatingSystemVersion {
            majorVersion: if cfg!(target_os = "macos") { 14 } else { 17 },
            minorVersion: 0,
            patchVersion: 0,
        };
        NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(min)
    }

    fn platform_name() -> &'static str {
        if cfg!(target_os = "macos") {
            "macos"
        } else {
            "ios"
        }
    }

    fn calendar_snapshot(calendar: &EKCalendar) -> CalendarInfo {
        let source = unsafe { calendar.source() }
            .map(|source| unsafe { source.title().to_string() })
            .unwrap_or_default();
        CalendarInfo {
            id: unsafe { calendar.calendarIdentifier().to_string() },
            title: unsafe { calendar.title().to_string() },
            source,
            color: calendar_color(calendar),
            allows_modifications: unsafe { calendar.allowsContentModifications() },
        }
    }

    fn calendar_color(calendar: &EKCalendar) -> String {
        unsafe { calendar.CGColor() }
            .as_deref()
            .and_then(hex_color)
            .unwrap_or_default()
    }

    fn hex_color(color: &CGColor) -> Option<String> {
        let count = CGColor::number_of_components(Some(color));
        let components = CGColor::components(Some(color));
        if components.is_null() {
            return None;
        }

        let parts = unsafe { std::slice::from_raw_parts(components, count) };
        let (r, g, b) = match parts {
            [gray, _alpha] => (*gray, *gray, *gray),
            [r, g, b, _alpha] => (*r, *g, *b),
            _ => return None,
        };
        Some(format!(
            "#{:02x}{:02x}{:02x}",
            color_component(r),
            color_component(g),
            color_component(b)
        ))
    }

    fn color_component(value: f64) -> u8 {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    fn event_snapshot(event: &EKEvent) -> Result<EventInfo> {
        let id = unsafe { event.eventIdentifier() }
            .ok_or_else(|| AppleCalendarError::EventKit("event has no identifier".into()))?
            .to_string();
        let calendar = unsafe { event.calendar() }
            .ok_or_else(|| AppleCalendarError::EventKit("event has no calendar".into()))?;
        Ok(EventInfo {
            id,
            calendar_id: unsafe { calendar.calendarIdentifier().to_string() },
            title: unsafe { event.title().to_string() },
            start_time: {
                let start = unsafe { event.startDate() };
                unix_millis_from_nsdate(&start)
            },
            end_time: {
                let end = unsafe { event.endDate() };
                unix_millis_from_nsdate(&end)
            },
            all_day: unsafe { event.isAllDay() },
            location: unsafe { event.location() }.map(|s| s.to_string()),
            notes: unsafe { event.notes() }.map(|s| s.to_string()),
            url: unsafe { event.URL() }
                .and_then(|url| url.absoluteString())
                .map(|s| s.to_string()),
        })
    }

    fn nsdate_from_unix_millis(ms: UnixMillis) -> Retained<NSDate> {
        NSDate::dateWithTimeIntervalSince1970(ms as f64 / 1000.0)
    }

    fn unix_millis_from_nsdate(date: &NSDate) -> UnixMillis {
        (date.timeIntervalSince1970() * 1000.0).round() as UnixMillis
    }

    fn parse_url(url: &str) -> Result<Retained<NSURL>> {
        NSURL::URLWithString(&NSString::from_str(url))
            .ok_or_else(|| AppleCalendarError::InvalidInput(format!("invalid url '{}'", url)))
    }

    fn nserror(error: Retained<NSError>) -> AppleCalendarError {
        AppleCalendarError::EventKit(error.localizedDescription().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct RecordingBackend {
        calls: Mutex<Vec<&'static str>>,
    }

    impl RecordingBackend {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }

        fn record(&self, call: &'static str) {
            self.calls.lock().unwrap().push(call);
        }
    }

    impl AppleCalendarBackend for Arc<RecordingBackend> {
        fn status(&self) -> Result<CalendarStatus> {
            self.record("status");
            Ok(CalendarStatus {
                access: AccessStatus::FullAccess,
                full_access: true,
                supported: true,
                platform: "test".into(),
            })
        }

        fn request_full_access(&self) -> Result<CalendarAccessResponse> {
            self.record("request_full_access");
            Ok(CalendarAccessResponse {
                access: AccessStatus::FullAccess,
                granted: true,
            })
        }

        fn calendars(&self) -> Result<Vec<CalendarInfo>> {
            self.record("calendars");
            Ok(vec![CalendarInfo {
                id: "cal-1".into(),
                title: "Work".into(),
                source: "iCloud".into(),
                color: String::new(),
                allows_modifications: true,
            }])
        }

        fn default_calendar(&self) -> Result<CalendarInfo> {
            self.record("default_calendar");
            Ok(CalendarInfo {
                id: "cal-1".into(),
                title: "Work".into(),
                source: "iCloud".into(),
                color: String::new(),
                allows_modifications: true,
            })
        }

        fn events(&self, query: EventQuery) -> Result<Vec<EventInfo>> {
            self.record("events");
            assert_eq!(query.limit, Some(2));
            Ok(Vec::new())
        }

        fn get(&self, event_id: &str) -> Result<EventInfo> {
            self.record("get");
            event(event_id)
        }

        fn create(&self, request: CreateEventRequest) -> Result<EventInfo> {
            self.record("create");
            event(&request.title)
        }

        fn update(&self, request: UpdateEventRequest) -> Result<EventInfo> {
            self.record("update");
            event(&request.event_id)
        }

        fn delete(&self, _event_id: &str) -> Result<bool> {
            self.record("delete");
            Ok(true)
        }
    }

    fn event(id: &str) -> Result<EventInfo> {
        Ok(EventInfo {
            id: id.into(),
            calendar_id: "cal-1".into(),
            title: "Title".into(),
            start_time: 1_783_056_000_000,
            end_time: 1_783_059_600_000,
            all_day: false,
            location: None,
            notes: None,
            url: None,
        })
    }

    #[test]
    fn gateway_forwards_to_backend() {
        let backend = Arc::new(RecordingBackend::new());
        let gateway = AppleCalendarGateway::new(Box::new(backend.clone()));

        gateway.status().unwrap();
        gateway.request_full_access().unwrap();
        gateway.calendars().unwrap();
        gateway.default_calendar().unwrap();
        gateway
            .events(EventQuery {
                start_time: 1,
                end_time: 2,
                calendar_id: None,
                limit: Some(2),
            })
            .unwrap();
        gateway.get("event-1").unwrap();
        gateway
            .create(CreateEventRequest {
                title: "created".into(),
                start_time: 1,
                end_time: 2,
                calendar_id: None,
                notes: None,
                location: None,
                url: None,
                all_day: false,
            })
            .unwrap();
        gateway
            .update(UpdateEventRequest {
                event_id: "event-1".into(),
                title: Some("updated".into()),
                start_time: None,
                end_time: None,
                calendar_id: None,
                notes: None,
                location: None,
                url: None,
                all_day: None,
            })
            .unwrap();
        assert!(gateway.delete("event-1").unwrap());

        assert_eq!(
            backend.calls(),
            vec![
                "status",
                "request_full_access",
                "calendars",
                "default_calendar",
                "events",
                "get",
                "create",
                "update",
                "delete",
            ]
        );
    }

    #[test]
    fn access_status_strings_are_stable() {
        assert_eq!(AccessStatus::NotDetermined.as_str(), "not_determined");
        assert_eq!(AccessStatus::Restricted.as_str(), "restricted");
        assert_eq!(AccessStatus::Denied.as_str(), "denied");
        assert_eq!(AccessStatus::FullAccess.as_str(), "full_access");
        assert_eq!(AccessStatus::WriteOnly.as_str(), "write_only");
    }

    #[test]
    fn errors_are_lua_friendly() {
        assert_eq!(
            AppleCalendarError::FullAccessRequired.to_string(),
            "calendar: full access required"
        );
        assert_eq!(
            AppleCalendarError::RecurrenceMutationUnsupported.to_string(),
            "calendar: recurring event mutations are not supported in V1"
        );
    }
}
