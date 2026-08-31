#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod config;

#[cfg(windows)]
mod assets;
#[cfg(windows)]
mod editability;
#[cfg(windows)]
mod ime;
#[cfg(windows)]
mod outlook;
#[cfg(windows)]
mod win;

#[cfg(not(windows))]
fn main() {
    eprintln!("IME Caret is a Windows caret indicator. Build it on Windows.");
}

#[cfg(windows)]
fn main() {
    windows_app::run();
}

#[cfg(windows)]
mod windows_app {
    use crate::assets::*;
    use crate::config::{Config, IndicatorPosition, RgbaColor};
    use crate::editability::{
        CaretAnchor, EditabilityDetector, FocusedInputContext, FocusedInputHost,
    };
    use crate::ime::{is_modern_shell_overlay_window, ImeEngine, ImeSnapshot, Validity};
    use crate::win::*;
    use std::ffi::{c_void, OsStr};
    use std::iter::once;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    const APP_NAME: &str = "IME Caret";
    const APP_VERSION: &str = "2.4";
    const MAIN_CLASS: &str = "ImeCaret.MainWindow";
    const BADGE_CLASS: &str = "ImeCaret.BadgeWindow";
    const SETTINGS_CLASS: &str = "ImeCaret.SettingsWindow";
    const MUTEX_NAME: &str = "Local\\ImeCaret.Singleton.7740C7D2-5D89-4874-A4D5-1B344507A604";

    const TIMER_ID: usize = 1;
    const TRAY_HINT_TIMER_ID: usize = 2;
    const KEYBOARD_ACTIVITY_TIMER_ID: usize = 3;
    const TASK_MANAGER_IME_STATE_TIMER_ID: usize = 4;
    const TASK_MANAGER_IME_STATE_POLL_INTERVAL_MS: u32 = 50;
    const TRAY_ID: u32 = 1;
    const ACTIVE_IME_POLL_INTERVAL_MS: u32 = 100;
    const RAW_INPUT_ACTIVE_POLL_INTERVAL_MS: u32 = 500;
    const FALLBACK_REFRESH_INTERVAL_MS: u32 = 500;
    const IDLE_FALLBACK_INTERVAL_MS: u32 = 2_000;
    const DEEP_IDLE_FALLBACK_INTERVAL_MS: u32 = 5_000;
    const FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
    const RAW_INPUT_FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(1_000);
    const IDLE_FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(2_000);
    const DEEP_IDLE_FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(5_000);
    const EVENT_REFRESH_MIN_INTERVAL: Duration = Duration::from_millis(50);
    const KEYBOARD_ACTIVITY_DELAY_MS: u32 = 20;
    const FOCUS_ACTIVATION_RETRY_COUNT: u8 = 3;
    const DELAYED_FOCUS_SURFACE_RETRY_COUNT: u8 = 3;
    const RAW_INPUT_DUPLICATE_WINDOW: Duration = Duration::from_millis(2);
    const ACTIVITY_BACKOFF_DELAY: Duration = Duration::from_secs(3);
    const DEEP_IDLE_BACKOFF_DELAY: Duration = Duration::from_secs(15);
    const WIN_EVENT_HOOK_COUNT: usize = 4;
    const SHELL_FOCUSED_HOST_CACHE_DURATION: Duration = Duration::from_millis(250);
    const TRAY_HINT_DISPLAY_MS: u32 = 2_000;
    const TRAY_HINT_MIN_WIDTH: i32 = 245;
    const TRAY_HINT_MIN_HEIGHT: i32 = 28;
    const TRAY_HINT_HORIZONTAL_PADDING: i32 = 32;
    const TRAY_HINT_VERTICAL_PADDING: i32 = 12;
    const TRAY_HINT_GAP: i32 = 4;
    const UNKNOWN_CLEAR_DELAY: Duration = Duration::from_millis(300);
    const CARET_INDICATOR_WIDTH: i32 = 15;
    const CARET_INDICATOR_HEIGHT: i32 = 15;
    const CARET_INDICATOR_FONT_HEIGHT: i32 = 11;
    const CARET_INDICATOR_X_GAP: i32 = 4;
    const CARET_INDICATOR_VERTICAL_GAP: i32 = 2;
    const DEFAULT_DPI: u32 = 96;
    const SHELL_OVERLAY_EDGE_GAP: i32 = 0;

    const WIN_EVENT_FLAG_FOREGROUND: u32 = 1 << 0;
    const WIN_EVENT_FLAG_FOCUS: u32 = 1 << 1;
    const WIN_EVENT_FLAG_CARET: u32 = 1 << 2;
    const WIN_EVENT_FLAG_TEXT_SELECTION: u32 = 1 << 3;
    const WIN_EVENT_FLAG_WINDOW_LOCATION: u32 = 1 << 4;
    const WIN_EVENT_PRIORITY_FLAGS: u32 = WIN_EVENT_FLAG_FOREGROUND | WIN_EVENT_FLAG_FOCUS;

    static WIN_EVENT_TARGET_HWND: AtomicUsize = AtomicUsize::new(0);
    static WIN_EVENT_UPDATE_PENDING: AtomicBool = AtomicBool::new(false);
    static WIN_EVENT_PENDING_FLAGS: AtomicU32 = AtomicU32::new(0);
    static WIN_EVENT_ALLOWED_HOST_PROCESS_ID: AtomicU32 = AtomicU32::new(0);

    const SETTINGS_CLIENT_WIDTH: i32 = 414;
    const SETTINGS_CLIENT_HEIGHT: i32 = 402;
    const SETTINGS_WINDOW_STYLE: DWORD = WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
    const SETTINGS_WINDOW_EX_STYLE: DWORD = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT;
    const SETTINGS_HORIZONTAL_MARGIN: i32 = 18;
    const SETTINGS_TOP_MARGIN: i32 = 14;
    const SETTINGS_BOTTOM_MARGIN: i32 = 18;
    const SETTINGS_BUTTON_WIDTH: i32 = 78;
    const SETTINGS_BUTTON_HEIGHT: i32 = 28;
    const SETTINGS_BUTTON_GAP: i32 = 10;
    const SETTINGS_GROUP_BUTTON_GAP: i32 = 14;
    const SETTINGS_CONTENT_INSET: i32 = 16;
    const SETTINGS_CONTENT_VERTICAL_MARGIN: i32 = 30;
    const SETTINGS_GROUP_CAPTION_VISUAL_INSET: i32 = 9;
    const SETTINGS_POSITION_CONTROL_GAP: i32 = 8;
    const SETTINGS_POSITION_COMBO_WIDTH: i32 = 176;
    const SETTINGS_COLOR_EDIT_WIDTH: i32 = 176;

    const MENU_TOGGLE_SOUND: u16 = 1001;
    const MENU_SETTINGS: u16 = 1002;
    const MENU_ABOUT: u16 = 1003;
    const MENU_EXIT: u16 = 1004;

    const CTRL_PLAY_ALL: u16 = 2101;
    const CTRL_PLAY_ENGLISH: u16 = 2102;
    const CTRL_PLAY_JAPANESE: u16 = 2103;
    const CTRL_PLAY_KOREAN: u16 = 2104;
    const CTRL_INDICATOR_POSITION: u16 = 2105;
    const CTRL_INDICATOR_TEXT_COLOR: u16 = 2106;
    const CTRL_ENGLISH_BACKGROUND_COLOR: u16 = 2107;
    const CTRL_JAPANESE_BACKGROUND_COLOR: u16 = 2108;
    const CTRL_KOREAN_BACKGROUND_COLOR: u16 = 2109;
    const CTRL_OK: u16 = IDOK;
    const CTRL_CANCEL: u16 = IDCANCEL;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ImeKind {
        English,
        JapaneseHiragana,
        JapaneseKatakana,
        Korean,
        Unsupported,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ScreenEdge {
        Left,
        Top,
        Right,
        Bottom,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct RawKeyboardSignal {
        device: usize,
        make_code: WORD,
        flags: WORD,
        virtual_key: WORD,
    }

    #[derive(Clone, Copy)]
    struct LastRawKeyboardSignal {
        signal: RawKeyboardSignal,
        tick: Instant,
    }

    enum RawKeyboardActivity {
        KeyDown(RawKeyboardSignal),
        Ignore,
        Fallback,
    }

    #[derive(Clone, Copy)]
    struct TrayMenuPlacement {
        anchor: POINT,
        flags: UINT,
        exclude: Option<RECT>,
    }

    struct IconSet {
        default: HICON,
        owned: HICON,
    }

    impl IconSet {
        unsafe fn create() -> Self {
            let owned = create_icon_from_hex(ICON_DEFAULT_HEX);
            let default = if owned.is_null() {
                LoadIconW(null_mut(), make_int_resource(IDI_APPLICATION))
            } else {
                owned
            };
            Self { default, owned }
        }

        unsafe fn destroy(&mut self) {
            if !self.owned.is_null() {
                DestroyIcon(self.owned);
            }
            self.default = null_mut();
            self.owned = null_mut();
        }
    }

    struct AppState {
        hinstance: HINSTANCE,
        main_hwnd: HWND,
        badge_hwnd: HWND,
        tray_hint_hwnd: HWND,
        settings_hwnd: HWND,
        mutex_handle: HANDLE,
        taskbar_created_message: u32,

        exe_dir: PathBuf,
        config_path: PathBuf,
        config: Config,
        ime_engine: ImeEngine,
        editability_detector: EditabilityDetector,
        icons: IconSet,

        tray_added: bool,
        old_kind: Option<ImeKind>,
        old_caps: bool,
        invalid_since: Option<Instant>,
        cleared_unknown: bool,

        badge_visible: bool,
        badge_kind: Option<ImeKind>,
        badge_text: u16,
        badge_text_color: RgbaColor,
        badge_background_color: RgbaColor,
        badge_position: Option<(i32, i32)>,
        badge_size: Option<(i32, i32)>,
        badge_monitor: HMONITOR,
        badge_monitor_dpi: Option<u32>,
        // True only while the window's retained layered surface matches the
        // stored glyph, colors, size, and current display rendering settings.
        badge_surface_valid: bool,
        cleaning_up: bool,
        shell_overlay_hwnd: HWND,
        shell_overlay_active: bool,
        shell_focused_host: FocusedInputHost,
        shell_focused_host_tick: Option<Instant>,
        win_event_hooks: [HWINEVENTHOOK; WIN_EVENT_HOOK_COUNT],
        raw_keyboard_registered: bool,
        last_raw_keyboard_signal: Option<LastRawKeyboardSignal>,
        keyboard_activity_pending: bool,
        keyboard_activity_covered_by_caret_event: bool,
        delayed_focus_surface_pending: bool,
        delayed_focus_surface_retries_remaining: u8,
        poll_interval_ms: u32,
        last_activity: Instant,
        last_full_refresh: Option<Instant>,
        last_caret_refresh: Option<Instant>,
        full_refresh_pending: bool,
        caret_refresh_pending: bool,
        focus_activation_retries_remaining: u8,
        active_caret_anchor: Option<CaretAnchor>,
        active_caret_reassert_topmost: bool,
        active_shell_overlay: Option<RECT>,
        active_focused_host: FocusedInputHost,
        active_task_manager_search: bool,
        task_manager_ime_process_id: u32,
        task_manager_korean_open: Option<bool>,
    }

    impl AppState {
        fn new(
            hinstance: HINSTANCE,
            mutex_handle: HANDLE,
            taskbar_created_message: u32,
            exe_dir: PathBuf,
            config_path: PathBuf,
            config: Config,
            icons: IconSet,
        ) -> Self {
            Self {
                hinstance,
                main_hwnd: null_mut(),
                badge_hwnd: null_mut(),
                tray_hint_hwnd: null_mut(),
                settings_hwnd: null_mut(),
                mutex_handle,
                taskbar_created_message,
                exe_dir,
                config_path,
                config: config.clone(),
                ime_engine: ImeEngine::default(),
                editability_detector: EditabilityDetector::new(),
                icons,
                tray_added: false,
                old_kind: None,
                old_caps: false,
                invalid_since: None,
                cleared_unknown: false,
                badge_visible: false,
                badge_kind: None,
                badge_text: 'A' as u16,
                badge_text_color: config.indicator_text_color,
                badge_background_color: config.english_background_color,
                badge_position: None,
                badge_size: None,
                badge_monitor: null_mut(),
                badge_monitor_dpi: None,
                badge_surface_valid: false,
                cleaning_up: false,
                shell_overlay_hwnd: null_mut(),
                shell_overlay_active: false,
                shell_focused_host: FocusedInputHost::default(),
                shell_focused_host_tick: None,
                win_event_hooks: [null_mut(); WIN_EVENT_HOOK_COUNT],
                raw_keyboard_registered: false,
                last_raw_keyboard_signal: None,
                keyboard_activity_pending: false,
                keyboard_activity_covered_by_caret_event: false,
                delayed_focus_surface_pending: false,
                delayed_focus_surface_retries_remaining: 0,
                poll_interval_ms: 0,
                last_activity: Instant::now(),
                last_full_refresh: None,
                last_caret_refresh: None,
                full_refresh_pending: false,
                caret_refresh_pending: false,
                focus_activation_retries_remaining: 0,
                active_caret_anchor: None,
                active_caret_reassert_topmost: false,
                active_shell_overlay: None,
                active_focused_host: FocusedInputHost::default(),
                active_task_manager_search: false,
                task_manager_ime_process_id: 0,
                task_manager_korean_open: None,
            }
        }

        unsafe fn initialize_window(&mut self, hwnd: HWND) {
            self.main_hwnd = hwnd;
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL, self.icons.default as LPARAM);
            SendMessageW(hwnd, WM_SETICON, ICON_BIG, self.icons.default as LPARAM);

            self.badge_hwnd = create_badge_window(self);
            self.add_tray();
            self.raw_keyboard_registered = register_keyboard_activity(hwnd);
            self.install_win_event_hooks();
            self.refresh_from_active_caret();
            self.reschedule_poll_timer();
        }

        unsafe fn on_timer(&mut self) {
            if self.full_refresh_pending {
                self.refresh_from_active_caret();
                self.schedule_focus_activation_retry_if_needed();
                self.schedule_delayed_focus_surface_retry_if_needed();
                self.reschedule_poll_timer();
                return;
            }
            if self.caret_refresh_pending {
                self.refresh_caret_position_only();
                self.reschedule_poll_timer();
                return;
            }

            let activity_backoff_level = self.activity_backoff_level();
            let full_refresh_recent = self
                .last_full_refresh
                .and_then(|tick| Instant::now().checked_duration_since(tick))
                .is_some_and(|age| {
                    age < full_refresh_interval(
                        self.raw_keyboard_registered,
                        activity_backoff_level,
                    )
                });

            if !self.full_refresh_pending
                && self.badge_visible
                && self.active_caret_anchor.is_some()
                && full_refresh_recent
            {
                self.refresh_ime_state_only();
            } else {
                self.refresh_from_active_caret();
            }
            self.reschedule_poll_timer();
        }

        unsafe fn on_keyboard_activity(&mut self, signal: Option<RawKeyboardSignal>) {
            if self.cleaning_up || !self.raw_keyboard_registered || self.main_hwnd.is_null() {
                return;
            }

            let now = Instant::now();
            let mut delayed_focus_surface = false;
            let refresh_candidate = if let Some(signal) = signal {
                let duplicate =
                    raw_keyboard_signal_is_duplicate(self.last_raw_keyboard_signal, signal, now);
                self.last_raw_keyboard_signal = Some(LastRawKeyboardSignal { signal, tick: now });
                if duplicate {
                    return;
                }
                delayed_focus_surface = raw_keyboard_signal_opens_delayed_focus_surface(signal);
                raw_keyboard_signal_needs_refresh(signal)
            } else {
                // If the Raw Input payload couldn't be decoded, preserve the
                // original activity path and avoid deduplicating against it.
                self.last_raw_keyboard_signal = None;
                true
            };

            self.delayed_focus_surface_pending |= delayed_focus_surface;

            self.last_activity = now;
            if !refresh_candidate
                && self.badge_visible
                && self.active_caret_anchor.is_some()
                && !self.full_refresh_pending
            {
                self.reschedule_poll_timer();
                return;
            }

            self.keyboard_activity_covered_by_caret_event = false;
            self.keyboard_activity_pending = SetTimer(
                self.main_hwnd,
                KEYBOARD_ACTIVITY_TIMER_ID,
                KEYBOARD_ACTIVITY_DELAY_MS,
                None,
            ) != 0;
        }

        unsafe fn on_keyboard_activity_timer(&mut self) {
            if !self.main_hwnd.is_null() {
                KillTimer(self.main_hwnd, KEYBOARD_ACTIVITY_TIMER_ID);
            }
            let covered_by_caret_event = self.keyboard_activity_covered_by_caret_event;
            let delayed_focus_surface = self.delayed_focus_surface_pending;
            self.keyboard_activity_pending = false;
            self.keyboard_activity_covered_by_caret_event = false;
            self.delayed_focus_surface_pending = false;
            if self.cleaning_up {
                return;
            }

            let refresh_needed =
                keyboard_activity_refresh_needed(covered_by_caret_event, self.full_refresh_pending);
            if refresh_needed && self.full_refresh_pending {
                self.refresh_from_active_caret();
            } else if refresh_needed && self.badge_visible && self.active_caret_anchor.is_some() {
                self.refresh_ime_state_only();
            } else if refresh_needed {
                self.refresh_from_active_caret();
            }
            if delayed_focus_surface {
                // Excel Find/Replace and command/search overlays can create
                // their focused control after the shortcut's Raw Input
                // signal. Probe a small bounded window so the new surface is
                // caught after it has materialized without affecting ordinary
                // keyboard activity.
                self.editability_detector.invalidate_focus_cache();
                self.delayed_focus_surface_retries_remaining =
                    DELAYED_FOCUS_SURFACE_RETRY_COUNT;
                self.full_refresh_pending = true;
            }
            self.reschedule_poll_timer();
        }

        unsafe fn on_win_events(&mut self) {
            let event_flags = WIN_EVENT_PENDING_FLAGS.swap(0, Ordering::AcqRel);

            if !self.cleaning_up && event_flags != 0 {
                self.note_activity();
                let priority_event = event_flags & WIN_EVENT_PRIORITY_FLAGS != 0;
                let caret_event = event_flags
                    & (WIN_EVENT_FLAG_CARET | WIN_EVENT_FLAG_TEXT_SELECTION)
                    != 0;
                let geometry_event = caret_event
                    || event_flags & WIN_EVENT_FLAG_WINDOW_LOCATION != 0;
                let activation_event = priority_event
                    || (caret_event
                        && (!self.badge_visible || self.active_caret_anchor.is_none()));

                if activation_event {
                    self.focus_activation_retries_remaining = FOCUS_ACTIVATION_RETRY_COUNT;
                    self.editability_detector.invalidate_focus_cache();
                }
                if priority_event {
                    self.shell_overlay_hwnd = null_mut();
                    self.shell_overlay_active = false;
                    self.shell_focused_host = FocusedInputHost::default();
                    self.shell_focused_host_tick = None;
                    WIN_EVENT_ALLOWED_HOST_PROCESS_ID.store(0, Ordering::Release);
                }

                let event_refresh_due = !self
                    .last_caret_refresh
                    .and_then(|tick| Instant::now().checked_duration_since(tick))
                    .is_some_and(|age| age < EVENT_REFRESH_MIN_INTERVAL);
                let can_refresh_caret_only = !priority_event
                    && geometry_event
                    && !self.full_refresh_pending
                    && self.badge_visible
                    && self.active_caret_anchor.is_some()
                    && self.badge_kind.is_some();
                let mut refreshed_ime_state = false;

                if priority_event {
                    self.refresh_from_active_caret();
                    self.schedule_focus_activation_retry_if_needed();
                    refreshed_ime_state = true;
                } else if event_refresh_due && can_refresh_caret_only {
                    refreshed_ime_state = !self.refresh_caret_position_only();
                } else if event_refresh_due {
                    self.refresh_from_active_caret();
                    if activation_event {
                        self.schedule_focus_activation_retry_if_needed();
                    }
                    refreshed_ime_state = true;
                } else if can_refresh_caret_only {
                    self.caret_refresh_pending = true;
                } else {
                    self.full_refresh_pending = true;
                }

                if refreshed_ime_state && self.keyboard_activity_pending && caret_event {
                    self.keyboard_activity_covered_by_caret_event = true;
                }
                self.reschedule_poll_timer();
            }

            WIN_EVENT_UPDATE_PENDING.store(false, Ordering::Release);
            post_pending_win_event_message();
        }

        unsafe fn refresh_from_active_caret(&mut self) {
            self.full_refresh_pending = false;
            self.caret_refresh_pending = false;
            let now = Instant::now();
            self.last_full_refresh = Some(now);
            self.last_caret_refresh = Some(now);

            // Mouse position and mouse cursor state are deliberately ignored.
            // The indicator follows only a positively identified editable caret.
            let input_context = FocusedInputContext::capture();
            let task_manager_search = task_manager_search_is_active(input_context);
            if task_manager_search {
                if !self.active_task_manager_search
                    || self.task_manager_ime_process_id != input_context.process_id
                {
                    self.task_manager_ime_process_id = input_context.process_id;
                    self.task_manager_korean_open = match self.old_kind {
                        Some(ImeKind::Korean) => Some(true),
                        Some(ImeKind::English) => Some(false),
                        _ => None,
                    };
                }
            }
            self.set_task_manager_search_active(task_manager_search);
            let editability = self
                .editability_detector
                .focused_input_with_context(input_context);
            if !editability.accepts_text_input() {
                self.clear_active_caret();
                self.hide_badge();
                return;
            }

            let shell_overlay = self.active_shell_overlay_bounds(input_context.foreground);
            let mut focused_host = self.focused_input_host_for_shell(input_context);
            let mut snapshot = self
                .ime_engine
                .query_with_context(focused_host, input_context);
            if snapshot.validity == Validity::Invalid
                && focused_host == FocusedInputHost::default()
            {
                // Electron and WinUI can keep their visible editor in the
                // foreground frame while the live input context belongs to a
                // UIA-published native host. The shell path already supplies
                // that identity; for ordinary apps, pay for the extra UIA
                // ancestry walk only after the cheap HWND query has failed.
                focused_host = self
                    .editability_detector
                    .focused_input_host_with_context(input_context);
                if focused_host != FocusedInputHost::default() {
                    WIN_EVENT_ALLOWED_HOST_PROCESS_ID
                        .store(focused_host.process_id, Ordering::Release);
                    snapshot = self
                        .ime_engine
                        .query_with_context(focused_host, input_context);
                }
            }
            snapshot = self.apply_task_manager_ime_override(snapshot);
            if snapshot.validity == Validity::Invalid {
                self.clear_active_caret();
                self.show_ime(
                    snapshot,
                    CaretAnchor {
                        x: 0,
                        top: 0,
                        bottom: 1,
                    },
                    shell_overlay,
                );
                return;
            }

            let console_cell_span = match classify_ime(snapshot) {
                ImeKind::Korean | ImeKind::JapaneseHiragana | ImeKind::JapaneseKatakana => 2,
                ImeKind::English | ImeKind::Unsupported => 1,
            };
            let Some(anchor) = self
                .editability_detector
                .focused_caret_anchor_with_context(input_context, console_cell_span)
            else {
                self.clear_active_caret();
                self.hide_badge();
                return;
            };

            self.active_caret_anchor = Some(anchor);
            self.active_caret_reassert_topmost = self
                .editability_detector
                .caret_needs_topmost_reassert();
            self.active_shell_overlay = shell_overlay;
            self.active_focused_host = focused_host;
            self.show_ime(snapshot, anchor, shell_overlay);
            if self.badge_visible && !self.active_caret_reassert_topmost {
                self.focus_activation_retries_remaining = 0;
            }
        }

        unsafe fn schedule_focus_activation_retry_if_needed(&mut self) {
            if self.badge_visible
                && self.active_caret_reassert_topmost
                && self.focus_activation_retries_remaining > 0
            {
                if let Some((x, y)) = self.badge_position {
                    SetWindowPos(
                        self.badge_hwnd,
                        HWND_TOPMOST,
                        x,
                        y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
                self.focus_activation_retries_remaining -= 1;
                if self.focus_activation_retries_remaining > 0 {
                    self.editability_detector.invalidate_focus_cache();
                    self.full_refresh_pending = true;
                }
            } else if self.badge_visible {
                self.focus_activation_retries_remaining = 0;
            } else if self.focus_activation_retries_remaining > 0 {
                self.focus_activation_retries_remaining -= 1;
                // Office may publish its editable element or accessibility
                // caret shortly after the initial focus/caret notification.
                // Re-probe on each short activation retry instead of reusing
                // the transient Unknown result for the whole retry window.
                self.editability_detector.invalidate_focus_cache();
                self.full_refresh_pending = true;
            }
        }

        unsafe fn schedule_delayed_focus_surface_retry_if_needed(&mut self) {
            if self.delayed_focus_surface_retries_remaining == 0 {
                return;
            }

            // A modeless find dialog or command/search overlay can complete
            // its Z-order activation after the badge has already been shown.
            // Reassert topmost only during these bounded shortcut retries.
            if self.badge_visible {
                if let Some((x, y)) = self.badge_position {
                    SetWindowPos(
                        self.badge_hwnd,
                        HWND_TOPMOST,
                        x,
                        y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
            }

            self.delayed_focus_surface_retries_remaining -= 1;
            if self.delayed_focus_surface_retries_remaining > 0 {
                self.editability_detector.invalidate_focus_cache();
                self.full_refresh_pending = true;
            }
        }

        /// Repositions a badge for the same focused editor without repeating
        /// editability and IME-state queries. Returns true when the lightweight
        /// path succeeded, or false when it had to fall back to a full refresh.
        unsafe fn refresh_caret_position_only(&mut self) -> bool {
            self.caret_refresh_pending = false;
            let Some(kind) = self.badge_kind else {
                self.refresh_from_active_caret();
                return false;
            };
            if !self.badge_visible || self.active_caret_anchor.is_none() {
                self.refresh_from_active_caret();
                return false;
            }

            let console_cell_span = match kind {
                ImeKind::Korean | ImeKind::JapaneseHiragana | ImeKind::JapaneseKatakana => 2,
                ImeKind::English | ImeKind::Unsupported => 1,
            };
            let Some(anchor) = self
                .editability_detector
                .focused_caret_anchor(console_cell_span)
            else {
                self.refresh_from_active_caret();
                return false;
            };

            self.last_caret_refresh = Some(Instant::now());
            self.active_caret_anchor = Some(anchor);
            self.active_caret_reassert_topmost = self
                .editability_detector
                .caret_needs_topmost_reassert();
            self.update_active_caret_indicator(
                kind,
                self.old_caps,
                anchor,
                self.active_shell_overlay,
            );
            true
        }

        unsafe fn refresh_ime_state_only(&mut self) {
            let Some(anchor) = self.active_caret_anchor else {
                self.refresh_from_active_caret();
                return;
            };

            let snapshot = self.ime_engine.query(self.active_focused_host);
            let snapshot = self.apply_task_manager_ime_override(snapshot);
            if snapshot.validity == Validity::Invalid {
                self.refresh_from_active_caret();
                return;
            }
            self.show_ime(snapshot, anchor, self.active_shell_overlay);
        }

        fn apply_task_manager_ime_override(&self, mut snapshot: ImeSnapshot) -> ImeSnapshot {
            if self.active_task_manager_search
                && (snapshot.language_id & 0x03ff) == 0x12
            {
                if let Some(is_open) = self.task_manager_korean_open {
                    snapshot.is_open = is_open;
                    snapshot.conversion_mode = if is_open { 1 } else { 0 };
                }
            }
            snapshot
        }

        unsafe fn set_task_manager_search_active(&mut self, active: bool) {
            if self.active_task_manager_search == active {
                return;
            }

            self.active_task_manager_search = active;
            if active {
                self.refresh_task_manager_ime_from_system_indicator();
                SetTimer(
                    self.main_hwnd,
                    TASK_MANAGER_IME_STATE_TIMER_ID,
                    TASK_MANAGER_IME_STATE_POLL_INTERVAL_MS,
                    None,
                );
            } else {
                KillTimer(self.main_hwnd, TASK_MANAGER_IME_STATE_TIMER_ID);
            }
        }

        unsafe fn on_task_manager_ime_state_timer(&mut self) {
            if self.cleaning_up || !self.active_task_manager_search {
                return;
            }

            let input_context = FocusedInputContext::capture();
            if !task_manager_search_is_active(input_context) {
                self.set_task_manager_search_active(false);
                return;
            }

            if self.refresh_task_manager_ime_from_system_indicator() {
                self.last_activity = Instant::now();
                self.refresh_ime_state_only();
            }
        }

        fn refresh_task_manager_ime_from_system_indicator(&mut self) -> bool {
            let Some(korean_open) = self
                .editability_detector
                .taskbar_korean_input_mode()
            else {
                return false;
            };
            let changed = self.task_manager_korean_open != Some(korean_open);
            self.task_manager_korean_open = Some(korean_open);
            changed
        }

        fn clear_active_caret(&mut self) {
            self.active_caret_anchor = None;
            self.active_caret_reassert_topmost = false;
            self.active_shell_overlay = None;
            self.active_focused_host = FocusedInputHost::default();
        }

        unsafe fn install_win_event_hooks(&mut self) {
            WIN_EVENT_TARGET_HWND.store(self.main_hwnd as usize, Ordering::Release);
            let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
            let events = [
                EVENT_SYSTEM_FOREGROUND,
                EVENT_OBJECT_FOCUS,
                EVENT_OBJECT_LOCATIONCHANGE,
                EVENT_OBJECT_TEXTSELECTIONCHANGED,
            ];
            for (slot, event) in self.win_event_hooks.iter_mut().zip(events) {
                *slot =
                    SetWinEventHook(event, event, null_mut(), Some(win_event_proc), 0, 0, flags);
            }
        }

        unsafe fn remove_win_event_hooks(&mut self) {
            WIN_EVENT_TARGET_HWND.store(0, Ordering::Release);
            WIN_EVENT_UPDATE_PENDING.store(false, Ordering::Release);
            WIN_EVENT_PENDING_FLAGS.store(0, Ordering::Release);
            WIN_EVENT_ALLOWED_HOST_PROCESS_ID.store(0, Ordering::Release);
            for hook in &mut self.win_event_hooks {
                if !hook.is_null() {
                    UnhookWinEvent(*hook);
                    *hook = null_mut();
                }
            }
        }

        fn note_activity(&mut self) {
            self.last_activity = Instant::now();
        }

        fn all_activity_events_registered(&self) -> bool {
            self.win_event_hooks.iter().all(|hook| !hook.is_null())
        }

        fn activity_backoff_level(&self) -> u8 {
            let eligible = self.raw_keyboard_registered && self.all_activity_events_registered();
            let activity_age = Instant::now()
                .checked_duration_since(self.last_activity)
                .unwrap_or_default();
            activity_backoff_level(eligible, activity_age)
        }

        unsafe fn reschedule_poll_timer(&mut self) {
            if self.main_hwnd.is_null() {
                return;
            }
            let interval = poll_interval_for_state(
                self.badge_visible,
                self.full_refresh_pending || self.caret_refresh_pending,
                self.raw_keyboard_registered,
                self.activity_backoff_level(),
            );
            if self.poll_interval_ms == interval {
                return;
            }

            if self.poll_interval_ms != 0 {
                KillTimer(self.main_hwnd, TIMER_ID);
            }
            self.poll_interval_ms = if SetTimer(self.main_hwnd, TIMER_ID, interval, None) == 0 {
                0
            } else {
                interval
            };
        }

        unsafe fn show_ime(
            &mut self,
            snapshot: ImeSnapshot,
            anchor: CaretAnchor,
            shell_overlay: Option<RECT>,
        ) {
            let now = Instant::now();
            if snapshot.validity == Validity::Invalid {
                self.hide_badge();
                let invalid_since = self.invalid_since.get_or_insert(now);
                if !self.cleared_unknown
                    && now
                        .checked_duration_since(*invalid_since)
                        .is_some_and(|elapsed| elapsed > UNKNOWN_CLEAR_DELAY)
                {
                    self.cleared_unknown = true;
                    self.old_kind = None;
                }
                return;
            }

            self.invalid_since = None;
            self.cleared_unknown = false;

            let kind = classify_ime(snapshot);
            let caps = (GetKeyState(VK_CAPITAL) & 1) != 0;
            let state_changed = self.old_kind != Some(kind) || self.old_caps != caps;

            if state_changed {
                if self.config.play_sounds && self.old_kind != Some(kind) {
                    self.play_kind_sound(kind);
                }

                self.old_kind = Some(kind);
                self.old_caps = caps;
            }

            self.update_active_caret_indicator(kind, caps, anchor, shell_overlay);
        }

        unsafe fn play_kind_sound(&self, kind: ImeKind) {
            let (enabled, filename) = match kind {
                ImeKind::English => (self.config.play_english_sound, "IMEE.wav"),
                ImeKind::JapaneseHiragana | ImeKind::JapaneseKatakana => {
                    (self.config.play_japanese_sound, "IMEJ.wav")
                }
                ImeKind::Korean => (self.config.play_korean_sound, "IMEK.wav"),
                ImeKind::Unsupported => return,
            };
            if !enabled {
                return;
            }

            let path = self.exe_dir.join(filename);
            if !path.is_file() {
                return;
            }
            let sound = path_to_wide(&path);
            PlaySoundW(
                sound.as_ptr(),
                null_mut(),
                SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
            );
        }

        /// Shows a small, neutral IME state marker beside the active insertion
        /// caret. The mouse cursor is never queried, replaced, or decorated.
        unsafe fn update_active_caret_indicator(
            &mut self,
            kind: ImeKind,
            caps: bool,
            anchor: CaretAnchor,
            shell_overlay: Option<RECT>,
        ) {
            if self.badge_hwnd.is_null() {
                self.hide_badge();
                return;
            }

            let (text, background_color) = match kind {
                ImeKind::Korean => ('가' as u16, self.config.korean_background_color),
                ImeKind::JapaneseHiragana => {
                    ('ひ' as u16, self.config.japanese_background_color)
                }
                ImeKind::JapaneseKatakana => {
                    ('カ' as u16, self.config.japanese_background_color)
                }
                ImeKind::English if caps => ('A' as u16, self.config.english_background_color),
                ImeKind::English => ('a' as u16, self.config.english_background_color),
                ImeKind::Unsupported => {
                    self.hide_badge();
                    return;
                }
            };
            let text_color = self.config.indicator_text_color;

            let appearance_changed = self.badge_kind != Some(kind)
                || self.badge_text != text
                || self.badge_text_color != text_color
                || self.badge_background_color != background_color;
            if appearance_changed {
                self.badge_kind = Some(kind);
                self.badge_text = text;
                self.badge_text_color = text_color;
                self.badge_background_color = background_color;
                self.badge_surface_valid = false;
            }

            let dpi = self.badge_dpi_at_point(POINT {
                x: anchor.x,
                y: anchor.bottom,
            });
            let badge_width = scale_for_dpi(CARET_INDICATOR_WIDTH, dpi);
            let badge_height = scale_for_dpi(CARET_INDICATOR_HEIGHT, dpi);

            let (x, y) = caret_indicator_position_avoiding_rect(
                anchor,
                badge_width,
                badge_height,
                self.config.indicator_position,
                shell_overlay,
            );
            let size_changed = self.badge_size != Some((badge_width, badge_height));
            if size_changed {
                self.badge_surface_valid = false;
            }

            // ShowWindow(SW_HIDE) preserves a layered window's last surface.
            // Only the first paint, a glyph/color change, or a size change
            // invalidates it; a same-appearance re-show only needs Z-order and
            // position restoration.
            let needs_render = !self.badge_surface_valid;
            if needs_render {
                if !render_layered_badge(
                    self.badge_hwnd,
                    x,
                    y,
                    badge_width,
                    badge_height,
                    self.badge_text,
                    self.badge_text_color,
                    self.badge_background_color,
                ) {
                    self.badge_surface_valid = false;
                    self.hide_badge();
                    return;
                }
                self.badge_position = Some((x, y));
                self.badge_size = Some((badge_width, badge_height));
                self.badge_surface_valid = true;
            } else if !self.badge_visible || self.badge_position != Some((x, y)) {
                SetWindowPos(
                    self.badge_hwnd,
                    HWND_TOPMOST,
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                self.badge_position = Some((x, y));
            }

            // Excel's modeless Find/Replace window can reassert its Z-order
            // after the initial activation retries have finished. In that
            // dialog, refresh the badge's topmost order even when the caret
            // geometry is unchanged; otherwise the badge can remain hidden
            // until typing or moving the dialog changes its position.
            if self.active_caret_reassert_topmost || is_excel_find_replace_foreground() {
                SetWindowPos(
                    self.badge_hwnd,
                    HWND_TOPMOST,
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            self.badge_visible = true;
        }

        unsafe fn badge_dpi_at_point(&mut self, point: POINT) -> u32 {
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            if !monitor.is_null() {
                if monitor == self.badge_monitor {
                    if let Some(dpi) = self.badge_monitor_dpi {
                        return dpi;
                    }
                }

                if let Some(dpi) = monitor_dpi(monitor) {
                    self.badge_monitor = monitor;
                    self.badge_monitor_dpi = Some(dpi);
                    return dpi;
                }
            }

            self.badge_monitor = monitor;
            self.badge_monitor_dpi = None;
            window_dpi(GetForegroundWindow())
        }

        unsafe fn active_shell_overlay_bounds(&mut self, foreground: HWND) -> Option<RECT> {
            if foreground != self.shell_overlay_hwnd {
                self.shell_overlay_hwnd = foreground;
                self.shell_overlay_active =
                    !foreground.is_null() && is_modern_shell_overlay_window(foreground);
                self.shell_focused_host = FocusedInputHost::default();
                self.shell_focused_host_tick = None;
                WIN_EVENT_ALLOWED_HOST_PROCESS_ID.store(0, Ordering::Release);
            }
            if !self.shell_overlay_active {
                WIN_EVENT_ALLOWED_HOST_PROCESS_ID.store(0, Ordering::Release);
                return None;
            }

            visible_window_bounds(foreground)
        }

        unsafe fn focused_input_host_for_shell(
            &mut self,
            input_context: FocusedInputContext,
        ) -> FocusedInputHost {
            if !self.shell_overlay_active {
                return FocusedInputHost::default();
            }

            let now = Instant::now();
            if self
                .shell_focused_host_tick
                .and_then(|tick| now.checked_duration_since(tick))
                .is_some_and(|age| age <= SHELL_FOCUSED_HOST_CACHE_DURATION)
            {
                WIN_EVENT_ALLOWED_HOST_PROCESS_ID
                    .store(self.shell_focused_host.process_id, Ordering::Release);
                return self.shell_focused_host;
            }

            self.shell_focused_host = self
                .editability_detector
                .focused_input_host_with_context(input_context);
            self.shell_focused_host_tick = Some(now);
            WIN_EVENT_ALLOWED_HOST_PROCESS_ID
                .store(self.shell_focused_host.process_id, Ordering::Release);
            self.shell_focused_host
        }

        unsafe fn hide_badge(&mut self) {
            if self.badge_visible && !self.badge_hwnd.is_null() {
                ShowWindow(self.badge_hwnd, SW_HIDE);
            }
            self.badge_visible = false;
        }

        unsafe fn add_tray(&mut self) {
            if self.tray_added || self.main_hwnd.is_null() {
                return;
            }

            let mut data = self.notify_icon_data();
            if Shell_NotifyIconW(NIM_ADD, &mut data) == FALSE {
                return;
            }

            self.tray_added = true;
            let mut version_data: NOTIFYICONDATAW = zeroed();
            version_data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            version_data.hWnd = self.main_hwnd;
            version_data.uID = TRAY_ID;
            version_data.uTimeoutOrVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &mut version_data);
        }

        unsafe fn notify_icon_data(&self) -> NOTIFYICONDATAW {
            let mut data: NOTIFYICONDATAW = zeroed();
            data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = self.main_hwnd;
            data.uID = TRAY_ID;
            data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
            data.uCallbackMessage = WM_APP_TRAY;
            data.hIcon = self.icons.default;
            copy_wide_to_fixed(&wide_without_null(&tray_tooltip_text()), &mut data.szTip);
            data
        }

        unsafe fn delete_tray(&mut self) {
            self.hide_tray_click_hint();
            if !self.tray_added || self.main_hwnd.is_null() {
                return;
            }
            let mut data: NOTIFYICONDATAW = zeroed();
            data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = self.main_hwnd;
            data.uID = TRAY_ID;
            Shell_NotifyIconW(NIM_DELETE, &mut data);
            self.tray_added = false;
        }

        unsafe fn tray_set_focus(&self) {
            if !self.tray_added {
                return;
            }
            let mut data: NOTIFYICONDATAW = zeroed();
            data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = self.main_hwnd;
            data.uID = TRAY_ID;
            Shell_NotifyIconW(NIM_SETFOCUS, &mut data);
        }

        unsafe fn on_tray_message(&mut self, wparam: WPARAM, lparam: LPARAM) {
            let event = loword(lparam as usize) as u32;
            match event {
                // With NOTIFYICON_VERSION_4, mouse notification coordinates are
                // packed into wParam. WM_CONTEXTMENU can be keyboard-generated,
                // so its wParam is undefined and the icon rectangle is preferred.
                WM_RBUTTONUP => self.show_tray_menu(Some(point_from_message(wparam))),
                WM_CONTEXTMENU => self.show_tray_menu(None),
                WM_LBUTTONUP | NIN_SELECT => {
                    self.show_tray_click_hint(Some(point_from_message(wparam)))
                }
                NIN_KEYSELECT => self.show_tray_click_hint(None),
                WM_LBUTTONDBLCLK => self.toggle_sounds(),
                _ => {}
            }
        }

        unsafe fn show_tray_click_hint(&mut self, event_anchor: Option<POINT>) {
            if !self.tray_added || self.main_hwnd.is_null() {
                return;
            }

            let bounds = virtual_screen_bounds();
            let icon_rect = self
                .tray_icon_rect()
                .or_else(|| event_anchor.map(point_as_rect))
                .unwrap_or(RECT {
                    left: bounds.right.saturating_sub(24),
                    top: bounds.bottom.saturating_sub(24),
                    right: bounds.right,
                    bottom: bounds.bottom,
                });
            self.hide_tray_click_hint();

            let mut measured_text = wide_without_null("설정을 변경하려면 우클릭하세요.");
            let (hint_width, hint_height) = tray_hint_size(&mut measured_text);
            let (x, y) = tray_hint_position(icon_rect, hint_width, hint_height, bounds);
            let class = wide("STATIC");
            let title = wide("설정을 변경하려면 우클릭하세요.");
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_BORDER | SS_CENTER | SS_CENTERIMAGE,
                x,
                y,
                hint_width,
                hint_height,
                self.main_hwnd,
                null_mut(),
                self.hinstance,
                null(),
            );
            if hwnd.is_null() {
                return;
            }

            self.tray_hint_hwnd = hwnd;
            SendMessageW(
                hwnd,
                WM_SETFONT,
                GetStockObject(DEFAULT_GUI_FONT) as WPARAM,
                TRUE as LPARAM,
            );
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
            SetTimer(
                self.main_hwnd,
                TRAY_HINT_TIMER_ID,
                TRAY_HINT_DISPLAY_MS,
                None,
            );
        }

        unsafe fn hide_tray_click_hint(&mut self) {
            if !self.main_hwnd.is_null() {
                KillTimer(self.main_hwnd, TRAY_HINT_TIMER_ID);
            }
            if !self.tray_hint_hwnd.is_null() && IsWindow(self.tray_hint_hwnd) != FALSE {
                DestroyWindow(self.tray_hint_hwnd);
            }
            self.tray_hint_hwnd = null_mut();
        }

        unsafe fn tray_icon_rect(&self) -> Option<RECT> {
            if !self.tray_added || self.main_hwnd.is_null() {
                return None;
            }

            let identifier = NOTIFYICONIDENTIFIER {
                cbSize: size_of::<NOTIFYICONIDENTIFIER>() as DWORD,
                hWnd: self.main_hwnd,
                uID: TRAY_ID,
                guidItem: GUID::default(),
            };
            let mut rect = RECT::default();
            if Shell_NotifyIconGetRect(&identifier, &mut rect) == S_OK && rect_is_valid(rect) {
                Some(rect)
            } else {
                None
            }
        }

        unsafe fn tray_menu_placement(&self, event_anchor: Option<POINT>) -> TrayMenuPlacement {
            let icon_rect = self.tray_icon_rect();

            let fallback = event_anchor
                .or_else(|| icon_rect.map(rect_center))
                .unwrap_or_default();

            let reference = icon_rect.map(rect_center).unwrap_or(fallback);
            let monitor = MonitorFromPoint(reference, MONITOR_DEFAULTTONEAREST);
            if !monitor.is_null() {
                let mut info: MONITORINFO = zeroed();
                info.cbSize = size_of::<MONITORINFO>() as DWORD;
                if GetMonitorInfoW(monitor, &mut info) != FALSE && rect_is_valid(info.rcMonitor) {
                    return calculate_tray_menu_placement(
                        icon_rect,
                        fallback,
                        info.rcMonitor,
                        info.rcWork,
                    );
                }
            }

            // Monitor APIs are expected to succeed on supported Windows versions,
            // but the virtual desktop still gives a safe edge-aware fallback.
            let virtual_left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let virtual_top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let virtual_screen = RECT {
                left: virtual_left,
                top: virtual_top,
                right: virtual_left.saturating_add(GetSystemMetrics(SM_CXVIRTUALSCREEN)),
                bottom: virtual_top.saturating_add(GetSystemMetrics(SM_CYVIRTUALSCREEN)),
            };
            calculate_tray_menu_placement(icon_rect, fallback, virtual_screen, virtual_screen)
        }

        unsafe fn show_tray_menu(&mut self, event_anchor: Option<POINT>) {
            self.hide_tray_click_hint();
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return;
            }

            let sound_flags = MF_STRING
                | if self.config.play_sounds {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                };
            append_menu_text(menu, sound_flags, MENU_TOGGLE_SOUND as usize, "소리 재생");
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            append_menu_text(menu, MF_STRING, MENU_SETTINGS as usize, "설정");
            append_menu_text(menu, MF_STRING, MENU_ABOUT as usize, "정보");
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            append_menu_text(menu, MF_STRING, MENU_EXIT as usize, "종료");

            let placement = self.tray_menu_placement(event_anchor);
            let params = placement.exclude.map(|rc_exclude| TPMPARAMS {
                cbSize: size_of::<TPMPARAMS>() as UINT,
                rcExclude: rc_exclude,
            });
            let params_ptr = params
                .as_ref()
                .map_or(null(), |value| value as *const TPMPARAMS);

            // The taskbar is a topmost window. Temporarily placing the hidden
            // owner in the topmost band prevents the native popup menu from
            // being occluded after fullscreen, sleep, or monitor transitions.
            let zorder_flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
            SetWindowPos(self.main_hwnd, HWND_TOPMOST, 0, 0, 0, 0, zorder_flags);
            SetForegroundWindow(self.main_hwnd);

            let command = TrackPopupMenuEx(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY | placement.flags,
                placement.anchor.x,
                placement.anchor.y,
                self.main_hwnd,
                params_ptr,
            );

            SetWindowPos(self.main_hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, zorder_flags);
            DestroyMenu(menu);
            PostMessageW(self.main_hwnd, WM_NULL, 0, 0);
            self.tray_set_focus();

            if command > 0 {
                self.handle_command(command as u16);
            }
        }

        unsafe fn handle_command(&mut self, command: u16) {
            match command {
                MENU_TOGGLE_SOUND => self.toggle_sounds(),
                MENU_SETTINGS => self.show_settings(),
                MENU_ABOUT => self.show_about(),
                MENU_EXIT => {
                    if !self.main_hwnd.is_null() {
                        DestroyWindow(self.main_hwnd);
                    }
                }
                _ => {}
            }
        }

        unsafe fn toggle_sounds(&mut self) {
            self.config.play_sounds = !self.config.play_sounds;
            self.save_config();
        }

        unsafe fn show_settings(&mut self) {
            if !self.settings_hwnd.is_null() && IsWindow(self.settings_hwnd) != FALSE {
                ShowWindow(self.settings_hwnd, SW_SHOWNORMAL);
                SetForegroundWindow(self.settings_hwnd);
                return;
            }

            let class = wide(SETTINGS_CLASS);
            let title = wide(&format!("{APP_NAME} 설정"));
            let (window_width, window_height) = settings_window_size();
            let (x, y) = centered_window_position(
                window_width,
                window_height,
                RECT {
                    left: 0,
                    top: 0,
                    right: GetSystemMetrics(SM_CXSCREEN),
                    bottom: GetSystemMetrics(SM_CYSCREEN),
                },
            );
            let hwnd = CreateWindowExW(
                SETTINGS_WINDOW_EX_STYLE,
                class.as_ptr(),
                title.as_ptr(),
                SETTINGS_WINDOW_STYLE,
                x,
                y,
                window_width,
                window_height,
                self.main_hwnd,
                null_mut(),
                self.hinstance,
                self as *mut Self as *const c_void,
            );
            if hwnd.is_null() {
                return;
            }
            self.settings_hwnd = hwnd;
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
            SetForegroundWindow(hwnd);
        }

        unsafe fn apply_settings_from_window(&mut self, hwnd: HWND) -> bool {
            let color_fields = [
                (CTRL_INDICATOR_TEXT_COLOR, "상태 표시 글자색"),
                (CTRL_ENGLISH_BACKGROUND_COLOR, "영문 배경색"),
                (CTRL_JAPANESE_BACKGROUND_COLOR, "일본어 배경색"),
                (CTRL_KOREAN_BACKGROUND_COLOR, "한글 배경색"),
            ];
            let mut colors = [self.config.indicator_text_color; 4];
            for ((control_id, label), color) in color_fields.into_iter().zip(&mut colors) {
                let Some(value) = read_control_text(hwnd, control_id) else {
                    show_invalid_color_message(hwnd, label);
                    return false;
                };
                let Some(parsed) = RgbaColor::parse(&value) else {
                    show_invalid_color_message(hwnd, label);
                    return false;
                };
                *color = parsed;
            }

            self.config.play_sounds = read_checkbox(hwnd, CTRL_PLAY_ALL);
            self.config.play_english_sound = read_checkbox(hwnd, CTRL_PLAY_ENGLISH);
            self.config.play_japanese_sound = read_checkbox(hwnd, CTRL_PLAY_JAPANESE);
            self.config.play_korean_sound = read_checkbox(hwnd, CTRL_PLAY_KOREAN);
            if let Some(index) = read_combo_selection(hwnd, CTRL_INDICATOR_POSITION) {
                self.config.indicator_position = IndicatorPosition::from_combo_index(index);
            }
            self.config.indicator_text_color = colors[0];
            self.config.english_background_color = colors[1];
            self.config.japanese_background_color = colors[2];
            self.config.korean_background_color = colors[3];
            self.save_config();

            self.old_kind = None;
            true
        }

        unsafe fn save_config(&self) {
            if let Err(error) = self.config.save(&self.config_path) {
                let text = wide(&format!(
                    "설정 파일을 저장하지 못했습니다.\n\n{}\n\n{}",
                    self.config_path.display(),
                    error
                ));
                let title = wide(&format!("{APP_NAME} 설정 오류"));
                MessageBoxW(
                    self.main_hwnd,
                    text.as_ptr(),
                    title.as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
            }
        }

        unsafe fn show_about(&self) {
            let text = wide(&format!(
                "{APP_NAME} {APP_VERSION}\n\n설정 파일: {}",
                self.config_path.display()
            ));
            let title = wide(&format!("{APP_NAME} 정보"));
            MessageBoxW(
                self.main_hwnd,
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }

        unsafe fn cleanup(&mut self) {
            if self.cleaning_up {
                return;
            }
            self.cleaning_up = true;

            self.remove_win_event_hooks();
            if self.raw_keyboard_registered {
                unregister_keyboard_activity();
                self.raw_keyboard_registered = false;
            }
            if !self.main_hwnd.is_null() {
                KillTimer(self.main_hwnd, TIMER_ID);
                KillTimer(self.main_hwnd, KEYBOARD_ACTIVITY_TIMER_ID);
                KillTimer(self.main_hwnd, TASK_MANAGER_IME_STATE_TIMER_ID);
            }
            self.keyboard_activity_pending = false;
            self.keyboard_activity_covered_by_caret_event = false;
            self.delayed_focus_surface_pending = false;
            self.delayed_focus_surface_retries_remaining = 0;
            self.poll_interval_ms = 0;
            self.clear_active_caret();
            self.hide_badge();
            self.hide_tray_click_hint();
            self.delete_tray();

            if !self.settings_hwnd.is_null() && IsWindow(self.settings_hwnd) != FALSE {
                let hwnd = self.settings_hwnd;
                self.settings_hwnd = null_mut();
                DestroyWindow(hwnd);
            }
            if !self.badge_hwnd.is_null() && IsWindow(self.badge_hwnd) != FALSE {
                let hwnd = self.badge_hwnd;
                self.badge_hwnd = null_mut();
                DestroyWindow(hwnd);
            }

            self.icons.destroy();

            if !self.mutex_handle.is_null() {
                CloseHandle(self.mutex_handle);
                self.mutex_handle = null_mut();
            }
        }
    }

    fn poll_interval_for_state(
        badge_visible: bool,
        full_refresh_pending: bool,
        raw_keyboard_registered: bool,
        activity_backoff_level: u8,
    ) -> u32 {
        if full_refresh_pending {
            ACTIVE_IME_POLL_INTERVAL_MS
        } else if badge_visible && raw_keyboard_registered {
            if activity_backoff_level >= 2 {
                DEEP_IDLE_FALLBACK_INTERVAL_MS
            } else if activity_backoff_level >= 1 {
                IDLE_FALLBACK_INTERVAL_MS
            } else {
                RAW_INPUT_ACTIVE_POLL_INTERVAL_MS
            }
        } else if badge_visible {
            ACTIVE_IME_POLL_INTERVAL_MS
        } else if activity_backoff_level >= 2 {
            DEEP_IDLE_FALLBACK_INTERVAL_MS
        } else if activity_backoff_level >= 1 {
            IDLE_FALLBACK_INTERVAL_MS
        } else {
            FALLBACK_REFRESH_INTERVAL_MS
        }
    }

    fn activity_backoff_level(eligible: bool, activity_age: Duration) -> u8 {
        if !eligible {
            0
        } else if activity_age >= DEEP_IDLE_BACKOFF_DELAY {
            2
        } else if activity_age >= ACTIVITY_BACKOFF_DELAY {
            1
        } else {
            0
        }
    }

    fn full_refresh_interval(
        raw_keyboard_registered: bool,
        activity_backoff_level: u8,
    ) -> Duration {
        if activity_backoff_level >= 2 {
            DEEP_IDLE_FULL_REFRESH_INTERVAL
        } else if activity_backoff_level >= 1 {
            IDLE_FULL_REFRESH_INTERVAL
        } else if raw_keyboard_registered {
            RAW_INPUT_FULL_REFRESH_INTERVAL
        } else {
            FULL_REFRESH_INTERVAL
        }
    }

    unsafe fn register_keyboard_activity(hwnd: HWND) -> bool {
        if hwnd.is_null() {
            return false;
        }
        let device = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        };
        RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as UINT) != FALSE
    }

    unsafe fn unregister_keyboard_activity() {
        let device = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            dwFlags: RIDEV_REMOVE,
            hwndTarget: null_mut(),
        };
        RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as UINT);
    }

    unsafe fn raw_keyboard_activity(lparam: LPARAM) -> RawKeyboardActivity {
        if lparam == 0 {
            return RawKeyboardActivity::Fallback;
        }

        let mut input: RAWINPUTKEYBOARD = zeroed();
        let mut input_size = size_of::<RAWINPUTKEYBOARD>() as UINT;
        let bytes_read = GetRawInputData(
            lparam as usize as HRAWINPUT,
            RID_INPUT,
            &mut input as *mut RAWINPUTKEYBOARD as *mut c_void,
            &mut input_size,
            size_of::<RAWINPUTHEADER>() as UINT,
        );
        if bytes_read == UINT::MAX
            || bytes_read < size_of::<RAWINPUTKEYBOARD>() as UINT
            || input.header.dwSize < size_of::<RAWINPUTKEYBOARD>() as DWORD
        {
            return RawKeyboardActivity::Fallback;
        }
        if input.header.dwType != RIM_TYPEKEYBOARD {
            return RawKeyboardActivity::Ignore;
        }
        if input.keyboard.Flags & RI_KEY_BREAK != 0 || input.keyboard.VKey == 0x00ff {
            return RawKeyboardActivity::Ignore;
        }

        RawKeyboardActivity::KeyDown(RawKeyboardSignal {
            device: input.header.hDevice as usize,
            make_code: input.keyboard.MakeCode,
            flags: input.keyboard.Flags,
            virtual_key: input.keyboard.VKey,
        })
    }

    fn raw_keyboard_signal_is_duplicate(
        previous: Option<LastRawKeyboardSignal>,
        signal: RawKeyboardSignal,
        now: Instant,
    ) -> bool {
        previous.is_some_and(|previous| {
            previous.signal == signal
                && now
                    .checked_duration_since(previous.tick)
                    .is_some_and(|age| age <= RAW_INPUT_DUPLICATE_WINDOW)
        })
    }

    unsafe fn raw_keyboard_signal_needs_refresh(signal: RawKeyboardSignal) -> bool {
        let virtual_key = signal.virtual_key as i32;
        raw_keyboard_key_can_change_ime(virtual_key) || keyboard_modifier_is_down()
    }

    unsafe fn window_class_equals(window: HWND, expected: &str) -> bool {
        if window.is_null() {
            return false;
        }
        let mut class_name = [0u16; 128];
        let length = GetClassNameW(
            window,
            class_name.as_mut_ptr(),
            class_name.len() as i32,
        );
        length > 0
            && String::from_utf16_lossy(&class_name[..length as usize])
                .eq_ignore_ascii_case(expected)
    }

    unsafe fn task_manager_search_is_active(context: FocusedInputContext) -> bool {
        window_class_equals(context.foreground, "TaskManagerWindow")
            && window_class_equals(
                context.focus,
                "Windows.UI.Input.InputSite.WindowClass",
            )
    }

    unsafe fn raw_keyboard_signal_opens_delayed_focus_surface(signal: RawKeyboardSignal) -> bool {
        matches!(signal.virtual_key, 0x46 | 0x48 | 0x50) && GetAsyncKeyState(VK_CONTROL) < 0
    }

    unsafe fn is_excel_find_replace_foreground() -> bool {
        const EXCEL_DIALOG_PREFIX: &[u8] = b"bosa_sdm_xl";

        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return false;
        }
        let mut class_name = [0u16; 64];
        let length = GetClassNameW(
            foreground,
            class_name.as_mut_ptr(),
            class_name.len() as i32,
        );
        if length < EXCEL_DIALOG_PREFIX.len() as i32 {
            return false;
        }

        EXCEL_DIALOG_PREFIX
            .iter()
            .enumerate()
            .all(|(index, expected)| {
                let actual = class_name[index];
                actual == u16::from(*expected)
                    || actual == u16::from(expected.to_ascii_uppercase())
            })
    }

    fn raw_keyboard_key_can_change_ime(virtual_key: i32) -> bool {
        matches!(
            virtual_key,
            // Shift through Space includes modifiers, Caps Lock, the Korean
            // and Japanese IME keys, conversion keys, Escape, and Space used
            // by common language-switch shortcuts.
            0x10..=0x20
                | VK_LWIN
                | VK_RWIN
                | 0xa0..=0xa5
                | 0xe5 // VK_PROCESSKEY
        )
    }

    unsafe fn keyboard_modifier_is_down() -> bool {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(|virtual_key| GetAsyncKeyState(virtual_key) < 0)
    }

    fn keyboard_activity_refresh_needed(
        covered_by_caret_event: bool,
        full_refresh_pending: bool,
    ) -> bool {
        !covered_by_caret_event || full_refresh_pending
    }

    fn win_event_flag(event: DWORD, object_id: LONG) -> u32 {
        match event {
            EVENT_SYSTEM_FOREGROUND => WIN_EVENT_FLAG_FOREGROUND,
            EVENT_OBJECT_FOCUS => WIN_EVENT_FLAG_FOCUS,
            EVENT_OBJECT_LOCATIONCHANGE if object_id == OBJID_CARET => WIN_EVENT_FLAG_CARET,
            EVENT_OBJECT_LOCATIONCHANGE if object_id == OBJID_WINDOW => {
                WIN_EVENT_FLAG_WINDOW_LOCATION
            }
            EVENT_OBJECT_TEXTSELECTIONCHANGED => WIN_EVENT_FLAG_TEXT_SELECTION,
            _ => 0,
        }
    }

    #[cfg(test)]
    fn is_relevant_win_event(event: DWORD, object_id: LONG) -> bool {
        win_event_flag(event, object_id) != 0
    }

    unsafe fn win_event_source_is_relevant(
        event: DWORD,
        hwnd: HWND,
        object_id: LONG,
    ) -> bool {
        // A foreground transition must always be observed. Focus hooks are
        // global, however, and background applications can raise them without
        // changing the user's active editor, so filter focus like caret and
        // selection events below.
        if event == EVENT_SYSTEM_FOREGROUND {
            return true;
        }
        if hwnd.is_null() {
            return false;
        }

        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return true;
        }
        if event == EVENT_OBJECT_LOCATIONCHANGE && object_id == OBJID_WINDOW {
            // A top-level move changes the caret's screen coordinates without
            // changing its client-relative rectangle. Limit this high-volume
            // event to the actual foreground window; child layout changes are
            // handled by caret/selection events or the existing fallback poll.
            return hwnd == foreground;
        }
        if hwnd == foreground
            || GetAncestor(hwnd, GA_ROOT) == foreground
            || GetAncestor(hwnd, GA_ROOTOWNER) == foreground
        {
            return true;
        }

        let allowed_process_id = WIN_EVENT_ALLOWED_HOST_PROCESS_ID.load(Ordering::Acquire);
        if allowed_process_id == 0 {
            return false;
        }
        let mut source_process_id = 0;
        GetWindowThreadProcessId(hwnd, &mut source_process_id);
        source_process_id == allowed_process_id
    }

    unsafe fn post_pending_win_event_message() {
        if WIN_EVENT_PENDING_FLAGS.load(Ordering::Acquire) == 0 {
            return;
        }
        let target = WIN_EVENT_TARGET_HWND.load(Ordering::Acquire) as HWND;
        if target.is_null() || WIN_EVENT_UPDATE_PENDING.swap(true, Ordering::AcqRel) {
            return;
        }
        if PostMessageW(target, WM_APP_ACTIVITY, 0, 0) == FALSE {
            WIN_EVENT_UPDATE_PENDING.store(false, Ordering::Release);
        }
    }

    unsafe extern "system" fn win_event_proc(
        _hook: HWINEVENTHOOK,
        event: DWORD,
        hwnd: HWND,
        object_id: LONG,
        _child_id: LONG,
        _event_thread: DWORD,
        _event_time: DWORD,
    ) {
        let event_flag = win_event_flag(event, object_id);
        if event_flag == 0 || !win_event_source_is_relevant(event, hwnd, object_id) {
            return;
        }

        WIN_EVENT_PENDING_FLAGS.fetch_or(event_flag, Ordering::AcqRel);
        post_pending_win_event_message();
    }

    pub fn run() {
        unsafe {
            if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == FALSE {
                SetProcessDPIAware();
            }

            let mutex_name = wide(MUTEX_NAME);
            let mutex_handle = CreateMutexW(null(), TRUE, mutex_name.as_ptr());
            if mutex_handle.is_null() {
                show_fatal_error("프로그램 단일 실행 잠금을 만들 수 없습니다.");
                return;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let class = wide(MAIN_CLASS);
                let existing = FindWindowW(class.as_ptr(), null());
                if !existing.is_null() {
                    PostMessageW(existing, WM_APP_SHOW_SETTINGS, 0, 0);
                }
                CloseHandle(mutex_handle);
                return;
            }

            let hinstance = GetModuleHandleW(null());
            if hinstance.is_null() {
                CloseHandle(mutex_handle);
                show_fatal_error("Windows 모듈 핸들을 가져올 수 없습니다.");
                return;
            }

            let exe_path =
                std::env::current_exe().unwrap_or_else(|_| PathBuf::from("IME Caret.exe"));
            let exe_dir = exe_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let config_path = exe_dir.join("IMECaret.ini");
            let config = Config::load(&config_path);
            let mut icons = IconSet::create();

            if !register_window_classes(hinstance, icons.default) {
                icons.destroy();
                CloseHandle(mutex_handle);
                show_fatal_error("Windows 창 클래스를 등록할 수 없습니다.");
                return;
            }

            let taskbar_message = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
            let state = Box::new(AppState::new(
                hinstance,
                mutex_handle,
                taskbar_message,
                exe_dir,
                config_path,
                config,
                icons,
            ));
            let state_ptr = Box::into_raw(state);

            let class = wide(MAIN_CLASS);
            let title = wide("IME Caret Hidden Window");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                hinstance,
                state_ptr as *const c_void,
            );

            if hwnd.is_null() {
                let state = &mut *state_ptr;
                state.cleanup();
                drop(Box::from_raw(state_ptr));
                show_fatal_error("메인 창을 만들 수 없습니다.");
                return;
            }

            let mut message: MSG = zeroed();
            loop {
                let result = GetMessageW(&mut message, null_mut(), 0, 0);
                if result == -1 || result == 0 {
                    break;
                }
                let settings = (*state_ptr).settings_hwnd;
                if !settings.is_null()
                    && IsWindow(settings) != FALSE
                    && IsDialogMessageW(settings, &mut message) != FALSE
                {
                    continue;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }

            let state = &mut *state_ptr;
            state.cleanup();
            drop(Box::from_raw(state_ptr));
        }
    }

    unsafe fn register_window_classes(hinstance: HINSTANCE, app_icon: HICON) -> bool {
        let arrow = LoadCursorW(null_mut(), make_int_resource(IDC_ARROW));

        let main_name = wide(MAIN_CLASS);
        let main_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(main_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: app_icon,
            hCursor: arrow,
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: main_name.as_ptr(),
        };
        if RegisterClassW(&main_class) == 0 {
            return false;
        }

        let badge_name = wide(BADGE_CLASS);
        let badge_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(badge_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: badge_name.as_ptr(),
        };
        if RegisterClassW(&badge_class) == 0 {
            return false;
        }

        let settings_name = wide(SETTINGS_CLASS);
        let settings_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(settings_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: app_icon,
            hCursor: arrow,
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszMenuName: null(),
            lpszClassName: settings_name.as_ptr(),
        };
        RegisterClassW(&settings_class) != 0
    }

    unsafe extern "system" fn main_wnd_proc(
        hwnd: HWND,
        message: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            set_window_long_ptr(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            return TRUE as LRESULT;
        }

        let state_ptr = get_window_long_ptr(hwnd, GWLP_USERDATA) as *mut AppState;
        if state_ptr.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let state = &mut *state_ptr;

        if message == state.taskbar_created_message && message != 0 {
            state.tray_added = false;
            state.add_tray();
            return 0;
        }

        match message {
            WM_CREATE => {
                state.initialize_window(hwnd);
                0
            }
            WM_INPUT => {
                // Raw Input is used only as an activity signal. Passing the
                // message to DefWindowProc lets Windows release its data.
                match raw_keyboard_activity(lparam) {
                    RawKeyboardActivity::KeyDown(signal) => {
                        state.on_keyboard_activity(Some(signal));
                    }
                    RawKeyboardActivity::Ignore => {}
                    RawKeyboardActivity::Fallback => {
                        state.on_keyboard_activity(None);
                    }
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_TIMER if wparam == TIMER_ID => {
                state.on_timer();
                0
            }
            WM_TIMER if wparam == KEYBOARD_ACTIVITY_TIMER_ID => {
                state.on_keyboard_activity_timer();
                0
            }
            WM_TIMER if wparam == TASK_MANAGER_IME_STATE_TIMER_ID => {
                state.on_task_manager_ime_state_timer();
                0
            }
            WM_TIMER if wparam == TRAY_HINT_TIMER_ID => {
                state.hide_tray_click_hint();
                0
            }
            WM_APP_ACTIVITY => {
                state.on_win_events();
                0
            }
            WM_APP_TRAY => {
                state.on_tray_message(wparam, lparam);
                0
            }
            WM_APP_SHOW_SETTINGS => {
                state.show_settings();
                0
            }
            WM_COMMAND => {
                state.handle_command(loword(wparam));
                0
            }
            WM_SETTINGCHANGE | WM_DISPLAYCHANGE => {
                state.note_activity();
                state.old_kind = None;
                state.badge_monitor = null_mut();
                state.badge_monitor_dpi = None;
                state.badge_surface_valid = false;
                state.editability_detector.invalidate_focus_cache();
                state.refresh_from_active_caret();
                state.reschedule_poll_timer();
                0
            }
            WM_QUERYENDSESSION => TRUE as LRESULT,
            WM_ENDSESSION => {
                if wparam != 0 {
                    state.cleanup();
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                state.cleanup();
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe extern "system" fn badge_wnd_proc(
        hwnd: HWND,
        message: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            set_window_long_ptr(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            return TRUE as LRESULT;
        }

        match message {
            WM_NCHITTEST => HTTRANSPARENT,
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = zeroed();
                BeginPaint(hwnd, &mut paint);
                EndPaint(hwnd, &paint);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe extern "system" fn settings_wnd_proc(
        hwnd: HWND,
        message: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            set_window_long_ptr(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            return TRUE as LRESULT;
        }

        let state_ptr = get_window_long_ptr(hwnd, GWLP_USERDATA) as *mut AppState;
        match message {
            WM_CREATE => {
                if !state_ptr.is_null() {
                    create_settings_controls(hwnd, &mut *state_ptr);
                }
                0
            }
            WM_COMMAND => {
                if state_ptr.is_null() {
                    return 0;
                }
                let control_id = loword(wparam);
                let notification = hiword(wparam);
                if notification == BN_CLICKED {
                    match control_id {
                        CTRL_OK => {
                            if (&mut *state_ptr).apply_settings_from_window(hwnd) {
                                DestroyWindow(hwnd);
                            }
                        }
                        CTRL_CANCEL => {
                            DestroyWindow(hwnd);
                        }
                        _ => {}
                    }
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_NCDESTROY => {
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    if state.settings_hwnd == hwnd {
                        state.settings_hwnd = null_mut();
                    }
                }
                set_window_long_ptr(hwnd, GWLP_USERDATA, 0);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn create_badge_window(state: &mut AppState) -> HWND {
        let class = wide(BADGE_CLASS);
        let title = wide("IME Caret Badge");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            CARET_INDICATOR_WIDTH,
            CARET_INDICATOR_HEIGHT,
            null_mut(),
            null_mut(),
            state.hinstance,
            state as *mut AppState as *const c_void,
        );
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
        hwnd
    }

    unsafe fn render_layered_badge(
        hwnd: HWND,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        text: u16,
        text_color: RgbaColor,
        background_color: RgbaColor,
    ) -> bool {
        if hwnd.is_null() || width <= 0 || height <= 0 {
            return false;
        }

        let screen_dc = GetDC(null_mut());
        if screen_dc.is_null() {
            return false;
        }
        let memory_dc = CreateCompatibleDC(screen_dc);
        if memory_dc.is_null() {
            ReleaseDC(null_mut(), screen_dc);
            return false;
        }

        let mut bitmap_info = BITMAPINFO::default();
        bitmap_info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as DWORD;
        bitmap_info.bmiHeader.biWidth = width;
        bitmap_info.bmiHeader.biHeight = -height;
        bitmap_info.bmiHeader.biPlanes = 1;
        bitmap_info.bmiHeader.biBitCount = 32;
        bitmap_info.bmiHeader.biCompression = BI_RGB;

        let mut bits = null_mut();
        let bitmap = CreateDIBSection(
            screen_dc,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            null_mut(),
            0,
        );
        if bitmap.is_null() || bits.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap as HGDIOBJ);
            }
            DeleteDC(memory_dc);
            ReleaseDC(null_mut(), screen_dc);
            return false;
        }

        let old_bitmap = SelectObject(memory_dc, bitmap as HGDIOBJ);
        let pixel_count = (width as usize).saturating_mul(height as usize);
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, pixel_count);
        pixels.fill(0);

        SetBkMode(memory_dc, OPAQUE);
        SetBkColor(memory_dc, rgb(0, 0, 0));
        SetTextColor(memory_dc, rgb(0xff, 0xff, 0xff));
        let font_face = wide("Segoe UI");
        let font_height = proportional_indicator_metric(
            CARET_INDICATOR_FONT_HEIGHT,
            height,
            CARET_INDICATOR_HEIGHT,
        );
        let created_font = CreateFontW(
            -font_height,
            0,
            0,
            0,
            400,
            FALSE as DWORD,
            FALSE as DWORD,
            FALSE as DWORD,
            1,
            0,
            0,
            ANTIALIASED_QUALITY,
            0,
            font_face.as_ptr(),
        );
        let font = if created_font.is_null() {
            GetStockObject(DEFAULT_GUI_FONT)
        } else {
            created_font as HGDIOBJ
        };
        let old_font = if font.is_null() {
            null_mut()
        } else {
            SelectObject(memory_dc, font)
        };
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let mut text = [text];
        DrawTextW(
            memory_dc,
            text.as_mut_ptr(),
            text.len() as i32,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        if !old_font.is_null() {
            SelectObject(memory_dc, old_font);
        }
        if !created_font.is_null() {
            DeleteObject(created_font as HGDIOBJ);
        }

        for pixel in pixels {
            let blue = (*pixel & 0xff) as u16;
            let green = ((*pixel >> 8) & 0xff) as u16;
            let red = ((*pixel >> 16) & 0xff) as u16;
            let coverage = ((red + green + blue + 1) / 3) as u8;
            *pixel = compose_badge_pixel(text_color, background_color, coverage);
        }

        let destination = POINT { x, y };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER,
            BlendFlags: 0,
            SourceConstantAlpha: 0xff,
            AlphaFormat: AC_SRC_ALPHA,
        };
        let updated = UpdateLayeredWindow(
            hwnd,
            screen_dc,
            &destination,
            &size,
            memory_dc,
            &source,
            0,
            &blend,
            ULW_ALPHA,
        ) != FALSE;

        if !old_bitmap.is_null() {
            SelectObject(memory_dc, old_bitmap);
        }
        DeleteObject(bitmap as HGDIOBJ);
        DeleteDC(memory_dc);
        ReleaseDC(null_mut(), screen_dc);

        if updated {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        updated
    }

    fn compose_badge_pixel(
        text: RgbaColor,
        background: RgbaColor,
        text_coverage: u8,
    ) -> u32 {
        let alpha = interpolate_u8(background.alpha, text.alpha, text_coverage);
        let red = interpolate_u8(
            multiply_u8(background.red, background.alpha),
            multiply_u8(text.red, text.alpha),
            text_coverage,
        );
        let green = interpolate_u8(
            multiply_u8(background.green, background.alpha),
            multiply_u8(text.green, text.alpha),
            text_coverage,
        );
        let blue = interpolate_u8(
            multiply_u8(background.blue, background.alpha),
            multiply_u8(text.blue, text.alpha),
            text_coverage,
        );
        u32::from(blue)
            | (u32::from(green) << 8)
            | (u32::from(red) << 16)
            | (u32::from(alpha) << 24)
    }

    fn multiply_u8(left: u8, right: u8) -> u8 {
        ((u16::from(left) * u16::from(right) + 127) / 255) as u8
    }

    fn interpolate_u8(from: u8, to: u8, amount: u8) -> u8 {
        let inverse = 0xff - amount;
        ((u32::from(from) * u32::from(inverse)
            + u32::from(to) * u32::from(amount)
            + 127)
            / 255) as u8
    }

    #[derive(Clone, Copy)]
    struct SettingsLayout {
        group: RECT,
        content_left: i32,
        content_width: i32,
        ok_button: RECT,
        cancel_button: RECT,
    }

    #[derive(Clone, Copy)]
    struct SettingsFieldLayout {
        label: RECT,
        control: RECT,
    }

    unsafe fn settings_window_size() -> (i32, i32) {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: SETTINGS_CLIENT_WIDTH,
            bottom: SETTINGS_CLIENT_HEIGHT,
        };
        if AdjustWindowRectEx(
            &mut rect,
            SETTINGS_WINDOW_STYLE,
            FALSE,
            SETTINGS_WINDOW_EX_STYLE,
        ) != FALSE
        {
            (
                rect.right.saturating_sub(rect.left),
                rect.bottom.saturating_sub(rect.top),
            )
        } else {
            (430, 451)
        }
    }

    fn settings_layout(client_width: i32, client_height: i32) -> SettingsLayout {
        let button_bottom = (client_height - SETTINGS_BOTTOM_MARGIN).max(SETTINGS_BUTTON_HEIGHT);
        let button_top = button_bottom - SETTINGS_BUTTON_HEIGHT;
        let cancel_right = (client_width - SETTINGS_HORIZONTAL_MARGIN).max(SETTINGS_BUTTON_WIDTH);
        let cancel_left = cancel_right - SETTINGS_BUTTON_WIDTH;
        let ok_right = cancel_left - SETTINGS_BUTTON_GAP;
        let ok_left = ok_right - SETTINGS_BUTTON_WIDTH;
        let group_right =
            (client_width - SETTINGS_HORIZONTAL_MARGIN).max(SETTINGS_HORIZONTAL_MARGIN + 1);
        let group_bottom = (button_top - SETTINGS_GROUP_BUTTON_GAP).max(SETTINGS_TOP_MARGIN + 1);
        let content_left = SETTINGS_HORIZONTAL_MARGIN + SETTINGS_CONTENT_INSET;
        let content_width =
            (group_right - SETTINGS_HORIZONTAL_MARGIN - SETTINGS_CONTENT_INSET * 2).max(1);

        SettingsLayout {
            group: RECT {
                left: SETTINGS_HORIZONTAL_MARGIN,
                top: SETTINGS_TOP_MARGIN,
                right: group_right,
                bottom: group_bottom,
            },
            content_left,
            content_width,
            ok_button: RECT {
                left: ok_left,
                top: button_top,
                right: ok_right,
                bottom: button_bottom,
            },
            cancel_button: RECT {
                left: cancel_left,
                top: button_top,
                right: cancel_right,
                bottom: button_bottom,
            },
        }
    }

    fn settings_field_layout(
        layout: SettingsLayout,
        y: i32,
        control_width: i32,
    ) -> SettingsFieldLayout {
        let content_right = layout.content_left.saturating_add(layout.content_width);
        let control_width = control_width.min(
            layout
                .content_width
                .saturating_sub(SETTINGS_POSITION_CONTROL_GAP + 1),
        );
        let control_left = content_right.saturating_sub(control_width);
        SettingsFieldLayout {
            label: RECT {
                left: layout.content_left,
                top: y,
                right: control_left.saturating_sub(SETTINGS_POSITION_CONTROL_GAP),
                bottom: y.saturating_add(22),
            },
            control: RECT {
                left: control_left,
                top: y.saturating_sub(3),
                right: content_right,
                bottom: y.saturating_add(22),
            },
        }
    }

    unsafe fn create_settings_controls(hwnd: HWND, state: &mut AppState) {
        let font = GetStockObject(DEFAULT_GUI_FONT);
        let mut client_rect: RECT = zeroed();
        if GetClientRect(hwnd, &mut client_rect) == FALSE {
            client_rect.right = SETTINGS_CLIENT_WIDTH;
            client_rect.bottom = SETTINGS_CLIENT_HEIGHT;
        }
        let layout = settings_layout(
            client_rect.right.saturating_sub(client_rect.left),
            client_rect.bottom.saturating_sub(client_rect.top),
        );

        create_control(
            state,
            hwnd,
            "BUTTON",
            "설정",
            WS_CHILD | WS_VISIBLE | BS_GROUPBOX,
            layout.group.left,
            layout.group.top,
            layout.group.right - layout.group.left,
            layout.group.bottom - layout.group.top,
            0,
            font,
        );

        let checks = [
            (
                CTRL_PLAY_ALL,
                "상태 변경 소리 전체 사용",
                state.config.play_sounds,
            ),
            (
                CTRL_PLAY_ENGLISH,
                "영문 전환 소리 재생",
                state.config.play_english_sound,
            ),
            (
                CTRL_PLAY_JAPANESE,
                "일본어 전환 소리 재생",
                state.config.play_japanese_sound,
            ),
            (
                CTRL_PLAY_KOREAN,
                "한글 전환 소리 재생",
                state.config.play_korean_sound,
            ),
        ];

        let mut y = layout.group.top
            + SETTINGS_GROUP_CAPTION_VISUAL_INSET
            + SETTINGS_CONTENT_VERTICAL_MARGIN;
        for (id, text, checked) in checks {
            let control = create_control(
                state,
                hwnd,
                "BUTTON",
                text,
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX,
                layout.content_left,
                y,
                layout.content_width,
                22,
                id,
                font,
            );
            if !control.is_null() {
                SendMessageW(
                    control,
                    BM_SETCHECK,
                    if checked { BST_CHECKED } else { BST_UNCHECKED },
                    0,
                );
            }
            y += 29;
        }

        y += 5;
        let position_layout =
            settings_field_layout(layout, y, SETTINGS_POSITION_COMBO_WIDTH);
        create_control(
            state,
            hwnd,
            "STATIC",
            "상태 표시 위치",
            WS_CHILD | WS_VISIBLE | SS_LEFT,
            position_layout.label.left,
            position_layout.label.top,
            position_layout.label.right - position_layout.label.left,
            position_layout.label.bottom - position_layout.label.top,
            0,
            font,
        );

        let position_combo = create_control(
            state,
            hwnd,
            "COMBOBOX",
            "",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST | CBS_HASSTRINGS,
            position_layout.control.left,
            position_layout.control.top,
            position_layout.control.right - position_layout.control.left,
            120,
            CTRL_INDICATOR_POSITION,
            font,
        );
        if !position_combo.is_null() {
            for label in ["캐럿 오른쪽", "캐럿 위", "캐럿 아래"] {
                let label = wide(label);
                SendMessageW(position_combo, CB_ADDSTRING, 0, label.as_ptr() as LPARAM);
            }
            SendMessageW(
                position_combo,
                CB_SETCURSEL,
                state.config.indicator_position.combo_index(),
                0,
            );
        }

        y += 29;
        let color_fields = [
            (
                CTRL_INDICATOR_TEXT_COLOR,
                "상태 표시 글자색",
                state.config.indicator_text_color,
            ),
            (
                CTRL_ENGLISH_BACKGROUND_COLOR,
                "영문 배경색",
                state.config.english_background_color,
            ),
            (
                CTRL_JAPANESE_BACKGROUND_COLOR,
                "일본어 배경색",
                state.config.japanese_background_color,
            ),
            (
                CTRL_KOREAN_BACKGROUND_COLOR,
                "한글 배경색",
                state.config.korean_background_color,
            ),
        ];
        for (id, label, color) in color_fields {
            let field_layout = settings_field_layout(layout, y, SETTINGS_COLOR_EDIT_WIDTH);
            create_control(
                state,
                hwnd,
                "STATIC",
                label,
                WS_CHILD | WS_VISIBLE | SS_LEFT,
                field_layout.label.left,
                field_layout.label.top,
                field_layout.label.right - field_layout.label.left,
                field_layout.label.bottom - field_layout.label.top,
                0,
                font,
            );
            let edit = create_control(
                state,
                hwnd,
                "EDIT",
                &color.as_rrggbbaa(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | ES_UPPERCASE
                    | ES_AUTOHSCROLL,
                field_layout.control.left,
                field_layout.control.top,
                field_layout.control.right - field_layout.control.left,
                field_layout.control.bottom - field_layout.control.top,
                id,
                font,
            );
            if !edit.is_null() {
                SendMessageW(edit, EM_SETLIMITTEXT, 8, 0);
            }
            y += 29;
        }

        create_control(
            state,
            hwnd,
            "BUTTON",
            "확인",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON,
            layout.ok_button.left,
            layout.ok_button.top,
            layout.ok_button.right - layout.ok_button.left,
            layout.ok_button.bottom - layout.ok_button.top,
            CTRL_OK,
            font,
        );
        create_control(
            state,
            hwnd,
            "BUTTON",
            "취소",
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON,
            layout.cancel_button.left,
            layout.cancel_button.top,
            layout.cancel_button.right - layout.cancel_button.left,
            layout.cancel_button.bottom - layout.cancel_button.top,
            CTRL_CANCEL,
            font,
        );
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn create_control(
        state: &AppState,
        parent: HWND,
        class_name: &str,
        text: &str,
        style: DWORD,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        id: u16,
        font: HGDIOBJ,
    ) -> HWND {
        let class = wide(class_name);
        let label = wide(text);
        let control = CreateWindowExW(
            0,
            class.as_ptr(),
            label.as_ptr(),
            style,
            x,
            y,
            width,
            height,
            parent,
            control_id(id),
            state.hinstance,
            null(),
        );
        if !control.is_null() && !font.is_null() {
            SendMessageW(control, WM_SETFONT, font as WPARAM, TRUE as LPARAM);
        }
        control
    }

    unsafe fn read_checkbox(parent: HWND, id: u16) -> bool {
        let control = GetDlgItem(parent, id as i32);
        !control.is_null() && SendMessageW(control, BM_GETCHECK, 0, 0) == BST_CHECKED as LRESULT
    }

    unsafe fn read_combo_selection(parent: HWND, id: u16) -> Option<usize> {
        let control = GetDlgItem(parent, id as i32);
        if control.is_null() {
            return None;
        }
        let selected = SendMessageW(control, CB_GETCURSEL, 0, 0);
        if selected == CB_ERR {
            None
        } else {
            Some(selected as usize)
        }
    }

    unsafe fn read_control_text(parent: HWND, id: u16) -> Option<String> {
        let control = GetDlgItem(parent, id as i32);
        if control.is_null() {
            return None;
        }
        let mut buffer = [0u16; 9];
        let length = GetWindowTextW(control, buffer.as_mut_ptr(), buffer.len() as i32);
        (length >= 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
    }

    unsafe fn show_invalid_color_message(parent: HWND, label: &str) {
        let text = wide(&format!(
            "{label} 값은 RRGGBBAA 형식의 8자리 16진수여야 합니다.\n\n예: 626262A5"
        ));
        let title = wide(&format!("{APP_NAME} 설정 오류"));
        MessageBoxW(parent, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }

    fn centered_window_position(width: i32, height: i32, bounds: RECT) -> (i32, i32) {
        (
            bounds
                .left
                .saturating_add((bounds.right.saturating_sub(bounds.left) - width) / 2),
            bounds
                .top
                .saturating_add((bounds.bottom.saturating_sub(bounds.top) - height) / 2),
        )
    }

    fn tray_tooltip_text() -> String {
        format!("{APP_NAME} {APP_VERSION}")
    }

    unsafe fn virtual_screen_bounds() -> RECT {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        RECT {
            left,
            top,
            right: left.saturating_add(GetSystemMetrics(SM_CXVIRTUALSCREEN)),
            bottom: top.saturating_add(GetSystemMetrics(SM_CYVIRTUALSCREEN)),
        }
    }

    unsafe fn tray_hint_size(text: &mut [u16]) -> (i32, i32) {
        let dc = GetDC(null_mut());
        if dc.is_null() {
            return (TRAY_HINT_MIN_WIDTH, TRAY_HINT_MIN_HEIGHT);
        }

        let font = GetStockObject(DEFAULT_GUI_FONT);
        let old_font = if font.is_null() {
            null_mut()
        } else {
            SelectObject(dc, font)
        };
        let mut text_rect = RECT::default();
        let measured = DrawTextW(
            dc,
            text.as_mut_ptr(),
            text.len() as i32,
            &mut text_rect,
            DT_SINGLELINE | DT_CALCRECT,
        );
        if !old_font.is_null() {
            SelectObject(dc, old_font);
        }
        ReleaseDC(null_mut(), dc);

        if measured == 0 {
            return (TRAY_HINT_MIN_WIDTH, TRAY_HINT_MIN_HEIGHT);
        }

        (
            text_rect
                .right
                .saturating_sub(text_rect.left)
                .saturating_add(TRAY_HINT_HORIZONTAL_PADDING)
                .max(TRAY_HINT_MIN_WIDTH),
            text_rect
                .bottom
                .saturating_sub(text_rect.top)
                .saturating_add(TRAY_HINT_VERTICAL_PADDING)
                .max(TRAY_HINT_MIN_HEIGHT),
        )
    }

    fn tray_hint_position(icon: RECT, width: i32, height: i32, bounds: RECT) -> (i32, i32) {
        let max_x = bounds.right.saturating_sub(width);
        let max_y = bounds.bottom.saturating_sub(height);
        let icon_center = icon
            .left
            .saturating_add(icon.right.saturating_sub(icon.left) / 2);
        let x = icon_center.saturating_sub(width / 2);
        let above = icon
            .top
            .saturating_sub(TRAY_HINT_GAP)
            .saturating_sub(height);
        let y = if above >= bounds.top {
            above
        } else {
            icon.bottom.saturating_add(TRAY_HINT_GAP)
        };
        (
            clamp_coordinate(x, bounds.left, max_x),
            clamp_coordinate(y, bounds.top, max_y),
        )
    }

    fn point_as_rect(point: POINT) -> RECT {
        RECT {
            left: point.x,
            top: point.y,
            right: point.x.saturating_add(1),
            bottom: point.y.saturating_add(1),
        }
    }

    fn caret_indicator_position_avoiding_rect(
        anchor: CaretAnchor,
        width: i32,
        height: i32,
        position: IndicatorPosition,
        occluding_rect: Option<RECT>,
    ) -> (i32, i32) {
        let bounds = RECT {
            left: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
            top: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
            right: unsafe {
                GetSystemMetrics(SM_XVIRTUALSCREEN)
                    .saturating_add(GetSystemMetrics(SM_CXVIRTUALSCREEN))
            },
            bottom: unsafe {
                GetSystemMetrics(SM_YVIRTUALSCREEN)
                    .saturating_add(GetSystemMetrics(SM_CYVIRTUALSCREEN))
            },
        };
        caret_indicator_position_avoiding_rect_in_bounds(
            anchor,
            width,
            height,
            bounds,
            position,
            occluding_rect,
        )
    }

    fn caret_indicator_position_avoiding_rect_in_bounds(
        anchor: CaretAnchor,
        width: i32,
        height: i32,
        bounds: RECT,
        position: IndicatorPosition,
        occluding_rect: Option<RECT>,
    ) -> (i32, i32) {
        let natural = caret_indicator_position_in_bounds(anchor, width, height, bounds, position);
        let Some(occluder) = occluding_rect.filter(|rect| rect_is_valid(*rect)) else {
            return natural;
        };
        let badge = RECT {
            left: natural.0,
            top: natural.1,
            right: natural.0.saturating_add(width),
            bottom: natural.1.saturating_add(height),
        };
        if !rectangles_overlap(badge, occluder) {
            return natural;
        }

        let gap = SHELL_OVERLAY_EDGE_GAP;
        let candidates = [
            (
                natural.0,
                occluder.top.saturating_sub(gap).saturating_sub(height),
            ),
            (
                occluder.left.saturating_sub(gap).saturating_sub(width),
                natural.1,
            ),
            (occluder.right.saturating_add(gap), natural.1),
            (natural.0, occluder.bottom.saturating_add(gap)),
        ];
        candidates
            .into_iter()
            .filter(|(x, y)| {
                *x >= bounds.left
                    && *y >= bounds.top
                    && x.saturating_add(width) <= bounds.right
                    && y.saturating_add(height) <= bounds.bottom
            })
            .min_by_key(|(x, y)| {
                i64::from(x.saturating_sub(natural.0).abs())
                    + i64::from(y.saturating_sub(natural.1).abs())
            })
            .unwrap_or(natural)
    }

    fn rectangles_overlap(left: RECT, right: RECT) -> bool {
        left.left < right.right
            && left.right > right.left
            && left.top < right.bottom
            && left.bottom > right.top
    }

    unsafe fn visible_window_bounds(window: HWND) -> Option<RECT> {
        let mut rect = RECT::default();
        if DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut c_void,
            size_of::<RECT>() as DWORD,
        ) >= 0
            && rect_is_valid(rect)
        {
            return Some(rect);
        }

        (GetWindowRect(window, &mut rect) != FALSE && rect_is_valid(rect)).then_some(rect)
    }

    fn caret_indicator_position_in_bounds(
        anchor: CaretAnchor,
        width: i32,
        height: i32,
        bounds: RECT,
        position: IndicatorPosition,
    ) -> (i32, i32) {
        let max_x = bounds.right.saturating_sub(width);
        let max_y = bounds.bottom.saturating_sub(height);
        let x_gap = proportional_indicator_metric(
            CARET_INDICATOR_X_GAP,
            width,
            CARET_INDICATOR_WIDTH,
        );
        let vertical_gap = proportional_indicator_metric(
            CARET_INDICATOR_VERTICAL_GAP,
            height,
            CARET_INDICATOR_HEIGHT,
        );

        let (mut x, mut y) = match position {
            IndicatorPosition::Right => (
                anchor.x.saturating_add(x_gap),
                anchor.bottom.saturating_sub(height),
            ),
            IndicatorPosition::Above => (
                anchor.x.saturating_add(x_gap),
                anchor
                    .top
                    .saturating_sub(vertical_gap)
                    .saturating_sub(height),
            ),
            IndicatorPosition::Below => (
                anchor.x.saturating_add(x_gap),
                anchor.bottom.saturating_add(vertical_gap),
            ),
        };

        match position {
            _ if x.saturating_add(width) > bounds.right => {
                x = anchor
                    .x
                    .saturating_sub(x_gap)
                    .saturating_sub(width);
            }
            IndicatorPosition::Above if y < bounds.top => {
                y = anchor.bottom.saturating_add(vertical_gap);
            }
            IndicatorPosition::Below if y.saturating_add(height) > bounds.bottom => {
                y = anchor
                    .top
                    .saturating_sub(vertical_gap)
                    .saturating_sub(height);
            }
            _ => {}
        }

        (
            clamp_coordinate(x, bounds.left, max_x),
            clamp_coordinate(y, bounds.top, max_y),
        )
    }

    fn proportional_indicator_metric(base: i32, extent: i32, base_extent: i32) -> i32 {
        base.saturating_mul(extent)
            .saturating_add(base_extent / 2)
            / base_extent.max(1)
    }

    fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
        value
            .saturating_mul(dpi.max(1) as i32)
            .saturating_add(DEFAULT_DPI as i32 / 2)
            / DEFAULT_DPI as i32
    }

    unsafe fn window_dpi(window: HWND) -> u32 {
        if window.is_null() {
            return DEFAULT_DPI;
        }
        let dpi = GetDpiForWindow(window);
        if dpi == 0 { DEFAULT_DPI } else { dpi }
    }

    unsafe fn monitor_dpi(monitor: HMONITOR) -> Option<u32> {
        if monitor.is_null() {
            return None;
        }

        let mut dpi_x = 0;
        let mut dpi_y = 0;
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == S_OK
            && dpi_x != 0
        {
            Some(dpi_x)
        } else {
            None
        }
    }

    fn classify_ime(snapshot: ImeSnapshot) -> ImeKind {
        let primary_language = snapshot.language_id & 0x03ff;
        if !snapshot.is_open {
            return ImeKind::English;
        }

        match primary_language {
            0x12 => {
                if snapshot.conversion_mode & 1 != 0 {
                    ImeKind::Korean
                } else {
                    ImeKind::English
                }
            }
            0x11 => match snapshot.conversion_mode {
                9 | 25 => ImeKind::JapaneseHiragana,
                3 | 11 | 19 | 27 => ImeKind::JapaneseKatakana,
                _ => ImeKind::English,
            },
            0x04 => ImeKind::Unsupported,
            _ => match snapshot.conversion_mode {
                9 | 25 => ImeKind::JapaneseHiragana,
                3 | 11 | 19 | 27 => ImeKind::JapaneseKatakana,
                mode if mode & 1 != 0 => ImeKind::Korean,
                _ => ImeKind::English,
            },
        }
    }

    unsafe fn create_icon_from_hex(hex: &str) -> HICON {
        let Some(bytes) = decode_hex(hex) else {
            return null_mut();
        };
        CreateIconFromResourceEx(
            bytes.as_ptr(),
            bytes.len() as u32,
            TRUE,
            0x0003_0000,
            16,
            16,
            0,
        )
    }

    fn decode_hex(hex: &str) -> Option<Vec<u8>> {
        if hex.len() % 2 != 0 {
            return None;
        }
        let bytes = hex.as_bytes();
        let mut output = Vec::with_capacity(bytes.len() / 2);
        for pair in bytes.chunks_exact(2) {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            output.push((high << 4) | low);
        }
        Some(output)
    }

    fn hex_nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(once(0)).collect()
    }

    fn wide_without_null(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().collect()
    }

    fn path_to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(once(0)).collect()
    }

    fn copy_wide_to_fixed<const N: usize>(source: &[u16], destination: &mut [u16; N]) {
        destination.fill(0);
        let count = source.len().min(N.saturating_sub(1));
        destination[..count].copy_from_slice(&source[..count]);
    }

    fn point_from_message(value: WPARAM) -> POINT {
        POINT {
            x: loword(value) as i16 as i32,
            y: hiword(value) as i16 as i32,
        }
    }

    fn rect_is_valid(rect: RECT) -> bool {
        rect.right > rect.left && rect.bottom > rect.top
    }

    fn rect_center(rect: RECT) -> POINT {
        POINT {
            x: rect.left + (rect.right - rect.left) / 2,
            y: rect.top + (rect.bottom - rect.top) / 2,
        }
    }

    fn clamp_coordinate(value: i32, minimum: i32, maximum: i32) -> i32 {
        if maximum < minimum {
            minimum
        } else {
            value.max(minimum).min(maximum)
        }
    }

    fn nearest_screen_edge(point: POINT, monitor: RECT) -> ScreenEdge {
        let left = (point.x as i64 - monitor.left as i64).abs();
        let top = (point.y as i64 - monitor.top as i64).abs();
        let right = (monitor.right as i64 - point.x as i64).abs();
        let bottom = (monitor.bottom as i64 - point.y as i64).abs();

        // Bottom wins ties because the Windows taskbar defaults to that edge.
        let mut best = (ScreenEdge::Bottom, bottom);
        for candidate in [
            (ScreenEdge::Top, top),
            (ScreenEdge::Left, left),
            (ScreenEdge::Right, right),
        ] {
            if candidate.1 < best.1 {
                best = candidate;
            }
        }
        best.0
    }

    fn tray_screen_edge(reference: POINT, monitor: RECT, work: RECT) -> ScreenEdge {
        if work.bottom < monitor.bottom && reference.y >= work.bottom {
            ScreenEdge::Bottom
        } else if work.top > monitor.top && reference.y < work.top {
            ScreenEdge::Top
        } else if work.left > monitor.left && reference.x < work.left {
            ScreenEdge::Left
        } else if work.right < monitor.right && reference.x >= work.right {
            ScreenEdge::Right
        } else {
            nearest_screen_edge(reference, monitor)
        }
    }

    fn taskbar_or_icon_exclusion(edge: ScreenEdge, icon: RECT, monitor: RECT, work: RECT) -> RECT {
        let center = rect_center(icon);
        match edge {
            ScreenEdge::Bottom if work.bottom < monitor.bottom && center.y >= work.bottom => RECT {
                left: monitor.left,
                top: work.bottom,
                right: monitor.right,
                bottom: monitor.bottom,
            },
            ScreenEdge::Top if work.top > monitor.top && center.y < work.top => RECT {
                left: monitor.left,
                top: monitor.top,
                right: monitor.right,
                bottom: work.top,
            },
            ScreenEdge::Left if work.left > monitor.left && center.x < work.left => RECT {
                left: monitor.left,
                top: monitor.top,
                right: work.left,
                bottom: monitor.bottom,
            },
            ScreenEdge::Right if work.right < monitor.right && center.x >= work.right => RECT {
                left: work.right,
                top: monitor.top,
                right: monitor.right,
                bottom: monitor.bottom,
            },
            _ => icon,
        }
    }

    fn calculate_tray_menu_placement(
        icon_rect: Option<RECT>,
        fallback: POINT,
        monitor: RECT,
        work: RECT,
    ) -> TrayMenuPlacement {
        let work = if rect_is_valid(work) { work } else { monitor };
        let reference = icon_rect.map(rect_center).unwrap_or(fallback);
        let edge = tray_screen_edge(reference, monitor, work);
        let middle_x = work.left + (work.right - work.left) / 2;
        let middle_y = work.top + (work.bottom - work.top) / 2;

        let (anchor, flags) = match edge {
            ScreenEdge::Bottom => {
                let align_right = reference.x >= middle_x;
                let x = if let Some(icon) = icon_rect {
                    if align_right {
                        clamp_coordinate(icon.right, work.left, work.right)
                    } else {
                        clamp_coordinate(icon.left, work.left, work.right)
                    }
                } else {
                    clamp_coordinate(reference.x, work.left, work.right)
                };
                let y = icon_rect
                    .map(|icon| icon.top.min(work.bottom))
                    .unwrap_or(work.bottom);
                (
                    POINT {
                        x,
                        y: clamp_coordinate(y, work.top, work.bottom),
                    },
                    (if align_right {
                        TPM_RIGHTALIGN
                    } else {
                        TPM_LEFTALIGN
                    }) | TPM_BOTTOMALIGN,
                )
            }
            ScreenEdge::Top => {
                let align_right = reference.x >= middle_x;
                let x = if let Some(icon) = icon_rect {
                    if align_right {
                        clamp_coordinate(icon.right, work.left, work.right)
                    } else {
                        clamp_coordinate(icon.left, work.left, work.right)
                    }
                } else {
                    clamp_coordinate(reference.x, work.left, work.right)
                };
                let y = icon_rect
                    .map(|icon| icon.bottom.max(work.top))
                    .unwrap_or(work.top);
                (
                    POINT {
                        x,
                        y: clamp_coordinate(y, work.top, work.bottom),
                    },
                    if align_right {
                        TPM_RIGHTALIGN | TPM_TOPALIGN
                    } else {
                        TPM_LEFTALIGN | TPM_TOPALIGN
                    },
                )
            }
            ScreenEdge::Right => {
                let align_bottom = reference.y >= middle_y;
                let x = icon_rect
                    .map(|icon| icon.left.min(work.right))
                    .unwrap_or(work.right);
                let y = if let Some(icon) = icon_rect {
                    if align_bottom {
                        clamp_coordinate(icon.bottom, work.top, work.bottom)
                    } else {
                        clamp_coordinate(icon.top, work.top, work.bottom)
                    }
                } else {
                    clamp_coordinate(reference.y, work.top, work.bottom)
                };
                (
                    POINT {
                        x: clamp_coordinate(x, work.left, work.right),
                        y,
                    },
                    TPM_RIGHTALIGN
                        | (if align_bottom {
                            TPM_BOTTOMALIGN
                        } else {
                            TPM_TOPALIGN
                        }),
                )
            }
            ScreenEdge::Left => {
                let align_bottom = reference.y >= middle_y;
                let x = icon_rect
                    .map(|icon| icon.right.max(work.left))
                    .unwrap_or(work.left);
                let y = if let Some(icon) = icon_rect {
                    if align_bottom {
                        clamp_coordinate(icon.bottom, work.top, work.bottom)
                    } else {
                        clamp_coordinate(icon.top, work.top, work.bottom)
                    }
                } else {
                    clamp_coordinate(reference.y, work.top, work.bottom)
                };
                (
                    POINT {
                        x: clamp_coordinate(x, work.left, work.right),
                        y,
                    },
                    TPM_LEFTALIGN
                        | (if align_bottom {
                            TPM_BOTTOMALIGN
                        } else {
                            TPM_TOPALIGN
                        }),
                )
            }
        };

        TrayMenuPlacement {
            anchor,
            flags,
            exclude: icon_rect.map(|icon| taskbar_or_icon_exclusion(edge, icon, monitor, work)),
        }
    }

    unsafe fn append_menu_text(menu: HMENU, flags: UINT, id: usize, text: &str) {
        let label = wide(text);
        AppendMenuW(menu, flags, id, label.as_ptr());
    }

    unsafe fn show_fatal_error(message: &str) {
        let text = wide(message);
        let title = wide("IME Caret 오류");
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn caret_indicator_is_small_bottom_aligned_and_offset() {
            let anchor = CaretAnchor {
                x: 120,
                top: 200,
                bottom: 222,
            };
            let bounds = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            };
            assert_eq!(
                caret_indicator_position_in_bounds(
                    anchor,
                    CARET_INDICATOR_WIDTH,
                    CARET_INDICATOR_HEIGHT,
                    bounds,
                    IndicatorPosition::Right,
                ),
                (124, 207)
            );
        }

        #[test]
        fn caret_indicator_moves_left_at_the_screen_edge() {
            let anchor = CaretAnchor {
                x: 1918,
                top: 200,
                bottom: 222,
            };
            let bounds = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            };
            assert_eq!(
                caret_indicator_position_in_bounds(
                    anchor,
                    CARET_INDICATOR_WIDTH,
                    CARET_INDICATOR_HEIGHT,
                    bounds,
                    IndicatorPosition::Right,
                ),
                (1899, 207)
            );
        }

        #[test]
        fn caret_indicator_can_be_placed_above_or_below_the_caret() {
            let anchor = CaretAnchor {
                x: 120,
                top: 200,
                bottom: 222,
            };
            let bounds = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            };

            assert_eq!(
                caret_indicator_position_in_bounds(
                    anchor,
                    CARET_INDICATOR_WIDTH,
                    CARET_INDICATOR_HEIGHT,
                    bounds,
                    IndicatorPosition::Above,
                ),
                (124, 183)
            );
            assert_eq!(
                caret_indicator_position_in_bounds(
                    anchor,
                    CARET_INDICATOR_WIDTH,
                    CARET_INDICATOR_HEIGHT,
                    bounds,
                    IndicatorPosition::Below,
                ),
                (124, 224)
            );
        }

        #[test]
        fn settings_window_is_centered_in_screen_bounds() {
            assert_eq!(
                centered_window_position(
                    430,
                    326,
                    RECT {
                        left: 0,
                        top: 0,
                        right: 1920,
                        bottom: 1080,
                    },
                ),
                (745, 377)
            );
        }

        #[test]
        fn shell_overlay_moves_indicator_to_nearest_visible_edge() {
            let anchor = CaretAnchor {
                x: 96,
                top: 537,
                bottom: 556,
            };
            let bounds = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            };
            let shell = RECT {
                left: 0,
                top: 502,
                right: 858,
                bottom: 1080,
            };
            assert_eq!(
                caret_indicator_position_avoiding_rect_in_bounds(
                    anchor,
                    CARET_INDICATOR_WIDTH,
                    CARET_INDICATOR_HEIGHT,
                    bounds,
                    IndicatorPosition::Right,
                    Some(shell),
                ),
                (100, 487)
            );
        }

        #[test]
        fn settings_layout_keeps_equal_outer_and_group_content_margins() {
            let layout = settings_layout(SETTINGS_CLIENT_WIDTH, SETTINGS_CLIENT_HEIGHT);
            let visual_group_top = layout.group.top + SETTINGS_GROUP_CAPTION_VISUAL_INSET;
            let first_control_top = visual_group_top + SETTINGS_CONTENT_VERTICAL_MARGIN;
            let position_y = first_control_top + 4 * 29 + 5;
            let position =
                settings_field_layout(layout, position_y, SETTINGS_POSITION_COMBO_WIDTH);
            let last_color = settings_field_layout(
                layout,
                position_y + 4 * 29,
                SETTINGS_COLOR_EDIT_WIDTH,
            );

            assert_eq!(layout.group.left, SETTINGS_HORIZONTAL_MARGIN);
            assert_eq!(
                SETTINGS_CLIENT_WIDTH - layout.group.right,
                SETTINGS_HORIZONTAL_MARGIN
            );
            assert_eq!(
                SETTINGS_CLIENT_HEIGHT - layout.cancel_button.bottom,
                SETTINGS_BOTTOM_MARGIN
            );
            assert_eq!(
                layout.cancel_button.left - layout.ok_button.right,
                SETTINGS_BUTTON_GAP
            );
            assert_eq!(
                layout.ok_button.top - layout.group.bottom,
                SETTINGS_GROUP_BUTTON_GAP
            );
            assert_eq!(
                position.label.left - layout.group.left,
                SETTINGS_CONTENT_INSET
            );
            assert_eq!(
                layout.group.right - position.control.right,
                SETTINGS_CONTENT_INSET
            );
            assert_eq!(
                position.control.left - position.label.right,
                SETTINGS_POSITION_CONTROL_GAP
            );
            assert_eq!(
                first_control_top - visual_group_top,
                layout.group.bottom - last_color.label.bottom
            );
            assert_eq!(position.control.right, last_color.control.right);
        }

        #[test]
        fn tray_tooltip_contains_program_name_and_version() {
            assert_eq!(tray_tooltip_text(), "IME Caret 2.4");
        }

        #[test]
        fn activity_backoff_preserves_fast_paths_and_slows_idle_fallbacks() {
            assert_eq!(poll_interval_for_state(true, false, false, 0), 100);
            assert_eq!(poll_interval_for_state(true, false, true, 0), 500);
            assert_eq!(poll_interval_for_state(true, false, true, 1), 2_000);
            assert_eq!(poll_interval_for_state(true, false, true, 2), 5_000);
            assert_eq!(poll_interval_for_state(false, true, true, 2), 100);
            assert_eq!(poll_interval_for_state(false, false, true, 0), 500);
            assert_eq!(poll_interval_for_state(false, false, true, 1), 2_000);
            assert_eq!(poll_interval_for_state(false, false, true, 2), 5_000);
            assert_eq!(full_refresh_interval(false, 0), Duration::from_millis(500));
            assert_eq!(full_refresh_interval(true, 0), Duration::from_millis(1_000));
            assert_eq!(full_refresh_interval(true, 1), Duration::from_millis(2_000));
            assert_eq!(full_refresh_interval(true, 2), Duration::from_millis(5_000));
            assert_eq!(activity_backoff_level(false, Duration::from_secs(30)), 0);
            assert_eq!(activity_backoff_level(true, Duration::from_secs(2)), 0);
            assert_eq!(activity_backoff_level(true, Duration::from_secs(3)), 1);
            assert_eq!(activity_backoff_level(true, Duration::from_secs(15)), 2);
        }

        #[test]
        fn raw_keyboard_duplicate_filter_is_device_and_time_scoped() {
            let now = Instant::now();
            let signal = RawKeyboardSignal {
                device: 1,
                make_code: 30,
                flags: 0,
                virtual_key: 0x41,
            };
            let previous = Some(LastRawKeyboardSignal { signal, tick: now });

            assert!(raw_keyboard_signal_is_duplicate(previous, signal, now));
            assert!(!raw_keyboard_signal_is_duplicate(
                previous,
                RawKeyboardSignal {
                    device: 2,
                    ..signal
                },
                now
            ));
            assert!(!raw_keyboard_signal_is_duplicate(
                previous,
                signal,
                now + Duration::from_millis(3)
            ));
        }

        #[test]
        fn raw_keyboard_refresh_filter_keeps_ime_switch_keys() {
            assert!(raw_keyboard_key_can_change_ime(VK_CAPITAL));
            assert!(raw_keyboard_key_can_change_ime(0x15));
            assert!(raw_keyboard_key_can_change_ime(0x20));
            assert!(raw_keyboard_key_can_change_ime(VK_LWIN));
            assert!(raw_keyboard_key_can_change_ime(0xe5));
            assert!(!raw_keyboard_key_can_change_ime(0x41));
            assert!(!raw_keyboard_key_can_change_ime(0x31));
        }

        #[test]
        fn caret_event_coalescing_preserves_pending_full_refreshes() {
            assert!(keyboard_activity_refresh_needed(false, false));
            assert!(!keyboard_activity_refresh_needed(true, false));
            assert!(keyboard_activity_refresh_needed(true, true));
        }

        #[test]
        fn win_event_filter_accepts_focus_and_caret_activity_only() {
            assert!(is_relevant_win_event(EVENT_SYSTEM_FOREGROUND, 0));
            assert!(is_relevant_win_event(EVENT_OBJECT_FOCUS, 0));
            assert!(is_relevant_win_event(
                EVENT_OBJECT_LOCATIONCHANGE,
                OBJID_CARET
            ));
            assert!(is_relevant_win_event(EVENT_OBJECT_TEXTSELECTIONCHANGED, 0));
            assert!(is_relevant_win_event(
                EVENT_OBJECT_LOCATIONCHANGE,
                OBJID_WINDOW
            ));
            assert!(!is_relevant_win_event(0, 0));
            assert_eq!(
                win_event_flag(EVENT_SYSTEM_FOREGROUND, 0) | win_event_flag(EVENT_OBJECT_FOCUS, 0),
                WIN_EVENT_PRIORITY_FLAGS
            );
        }

        #[test]
        fn tray_click_hint_is_placed_above_the_taskbar_icon() {
            assert_eq!(
                tray_hint_position(
                    RECT {
                        left: 1850,
                        top: 1048,
                        right: 1874,
                        bottom: 1072,
                    },
                    TRAY_HINT_MIN_WIDTH,
                    TRAY_HINT_MIN_HEIGHT,
                    RECT {
                        left: 0,
                        top: 0,
                        right: 1920,
                        bottom: 1080,
                    },
                ),
                (1675, 1016)
            );
            assert_eq!(TRAY_HINT_DISPLAY_MS, 2_000);
            assert_eq!(TRAY_HINT_HORIZONTAL_PADDING, 32);
        }

        #[test]
        fn embedded_tray_icon_has_expected_size() {
            assert_eq!(decode_hex(ICON_DEFAULT_HEX).map(|v| v.len()), Some(296));
        }

        #[test]
        fn tray_menu_uses_work_area_above_bottom_taskbar() {
            let monitor = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            };
            let work = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            };
            let icon = RECT {
                left: 1850,
                top: 1048,
                right: 1874,
                bottom: 1072,
            };
            let placement =
                calculate_tray_menu_placement(Some(icon), POINT::default(), monitor, work);

            assert_eq!(placement.anchor.y, work.bottom);
            assert_ne!(placement.flags & TPM_BOTTOMALIGN, 0);
            assert_ne!(placement.flags & TPM_RIGHTALIGN, 0);
            let exclude = placement.exclude.expect("taskbar exclusion");
            assert_eq!(exclude.top, work.bottom);
            assert_eq!(exclude.bottom, monitor.bottom);
        }

        #[test]
        fn tray_menu_supports_top_and_side_taskbars() {
            let monitor = RECT {
                left: -1600,
                top: 0,
                right: 0,
                bottom: 900,
            };

            let top_work = RECT {
                left: -1600,
                top: 48,
                right: 0,
                bottom: 900,
            };
            let top_icon = RECT {
                left: -80,
                top: 10,
                right: -56,
                bottom: 34,
            };
            let top =
                calculate_tray_menu_placement(Some(top_icon), POINT::default(), monitor, top_work);
            assert_eq!(top.anchor.y, top_work.top);
            assert_eq!(top.flags & TPM_BOTTOMALIGN, 0);

            let right_work = RECT {
                left: -1600,
                top: 0,
                right: -52,
                bottom: 900,
            };
            let right_icon = RECT {
                left: -42,
                top: 830,
                right: -18,
                bottom: 854,
            };
            let right = calculate_tray_menu_placement(
                Some(right_icon),
                POINT::default(),
                monitor,
                right_work,
            );
            assert_eq!(right.anchor.x, right_work.right);
            assert_ne!(right.flags & TPM_RIGHTALIGN, 0);
            assert_ne!(right.flags & TPM_BOTTOMALIGN, 0);
        }

        #[test]
        fn notification_coordinates_keep_negative_monitor_values() {
            let packed = ((-120i16 as u16 as usize) << 16) | (-640i16 as u16 as usize);
            let point = point_from_message(packed);
            assert_eq!(point.x, -640);
            assert_eq!(point.y, -120);
        }
    }
}
