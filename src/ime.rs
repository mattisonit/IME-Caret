use crate::editability::FocusedInputHost;
use crate::win::*;
use std::collections::{HashMap, HashSet};
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;
use std::time::{Duration, Instant};

const CACHE_BRIDGE: Duration = Duration::from_millis(400);
const CACHE_RETENTION: Duration = Duration::from_secs(10);
const SHELL_INPUT_WINDOW_CACHE: Duration = Duration::from_secs(1);
const MAX_ENUMERATED_INPUT_WINDOWS: usize = 128;

const LANG_CHINESE: u16 = 0x04;
const LANG_JAPANESE: u16 = 0x11;
const LANG_KOREAN: u16 = 0x12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Validity {
    Invalid,
    Live,
    Cached,
}

#[derive(Clone, Copy, Debug)]
pub struct ImeSnapshot {
    pub validity: Validity,
    pub conversion_mode: u32,
    pub language_id: u16,
    pub is_open: bool,
}

impl Default for ImeSnapshot {
    fn default() -> Self {
        Self {
            validity: Validity::Invalid,
            conversion_mode: 0,
            language_id: 0,
            is_open: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum CacheKey {
    Thread(u32),
    Window(usize),
}

#[derive(Clone, Copy, Debug)]
struct CachedIme {
    mode: u32,
    is_open: bool,
    language_id: u16,
    tick: Instant,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct ForegroundTargets {
    active: HWND,
    focus: HWND,
    caret: HWND,
    foreground: HWND,
}

#[derive(Clone, Copy)]
enum CachedLiveSource {
    ImeWindow {
        window: HWND,
        ime_window: HWND,
    },
    InputContext {
        window: HWND,
    },
}

#[derive(Clone, Copy)]
struct CachedLiveTarget {
    foreground: ForegroundTargets,
    focused_host: FocusedInputHost,
    source: CachedLiveSource,
}

#[derive(Clone)]
struct CachedShellInputWindows {
    foreground_process_id: u32,
    focused_process_id: u32,
    windows: Vec<HWND>,
    tick: Instant,
}

#[derive(Default)]
pub struct ImeEngine {
    cache: HashMap<CacheKey, CachedIme>,
    live_target: Option<CachedLiveTarget>,
    shell_input_windows: Option<CachedShellInputWindows>,
}

impl ImeEngine {
    pub fn query(&mut self, focused_host: FocusedInputHost) -> ImeSnapshot {
        unsafe { self.query_unsafe(focused_host) }
    }

    unsafe fn query_unsafe(&mut self, focused_host: FocusedInputHost) -> ImeSnapshot {
        let now = Instant::now();
        if self.cache.len() > 128 {
            self.cache.retain(|_, value| {
                now.checked_duration_since(value.tick)
                    .is_some_and(|age| age <= CACHE_RETENTION)
            });
        }

        let foreground = foreground_targets();
        if let Some(snapshot) =
            self.query_cached_live_target(foreground, focused_host, now)
        {
            return snapshot;
        }

        let mut candidates = Vec::<HWND>::with_capacity(24);
        let mut seen = HashSet::<usize>::with_capacity(24);
        let mut target = first_non_null(&[
            focused_host.native_window,
            foreground.focus,
            foreground.caret,
            foreground.active,
            foreground.foreground,
        ]);
        add_window_chain(&mut candidates, &mut seen, foreground.focus);
        add_window_chain(&mut candidates, &mut seen, foreground.caret);
        add_window_chain(&mut candidates, &mut seen, foreground.active);
        add_window_chain(&mut candidates, &mut seen, foreground.foreground);
        add_window_chain(
            &mut candidates,
            &mut seen,
            focused_host.native_window,
        );

        let foreground_process_id = window_process_id(foreground.foreground);
        if focused_host.process_id != 0
            && focused_host.process_id != foreground_process_id
        {
            add_process_windows(
                &mut candidates,
                &mut seen,
                focused_host.process_id,
            );
        }

        if target.is_null() {
            target = candidates.first().copied().unwrap_or(null_mut());
        }
        if target.is_null() {
            return ImeSnapshot::default();
        }

        if let Some(snapshot) =
            self.query_live_candidates(&candidates, foreground, focused_host, now)
        {
            return snapshot;
        }

        // Windows Search, Start, and other modern shell surfaces can host the
        // visible UI Automation Edit element separately from the HWND that
        // owns its IMM context. Only after the ordinary foreground candidates
        // fail, enumerate the tightly allow-listed input hosts in this user's
        // session. Cache their HWNDs briefly to avoid a process/window scan on
        // every polling tick.
        let initial_candidate_count = candidates.len();
        for window in self.cached_shell_input_windows(
            foreground_process_id,
            focused_host.process_id,
            now,
        ) {
            add_window_candidate(&mut candidates, &mut seen, window);
        }
        if candidates.len() > initial_candidate_count {
            if let Some(snapshot) =
                self.query_live_candidates(
                    &candidates[initial_candidate_count..],
                    foreground,
                    focused_host,
                    now,
                )
            {
                return snapshot;
            }
        }

        let first_thread = candidates
            .iter()
            .map(|hwnd| window_thread(*hwnd))
            .find(|thread_id| *thread_id != 0)
            .unwrap_or(0);
        let target_thread = match window_thread(target) {
            0 => first_thread,
            thread_id => thread_id,
        };
        let mut language_id = thread_language(target_thread);

        // Briefly reuse a state from the same UI thread while focus is moving between
        // controls. The short lifetime prevents a stale Korean/Japanese state from being
        // displayed indefinitely after the target application disappears.
        for &hwnd in &candidates {
            let thread_id = window_thread(hwnd);
            let key = cache_key(thread_id, hwnd);
            let Some(cached) = self.cache.get(&key).copied() else {
                continue;
            };
            let Some(age) = now.checked_duration_since(cached.tick) else {
                continue;
            };
            if age <= CACHE_BRIDGE {
                language_id = cached.language_id;
                return ImeSnapshot {
                    validity: Validity::Cached,
                    conversion_mode: cached.mode,
                    language_id,
                    is_open: cached.is_open,
                };
            }
        }

        let primary_language = language_id & 0x03ff;
        if language_id != 0
            && primary_language != LANG_CHINESE
            && primary_language != LANG_JAPANESE
            && primary_language != LANG_KOREAN
        {
            return ImeSnapshot {
                validity: Validity::Live,
                conversion_mode: 0,
                language_id,
                is_open: false,
            };
        }

        ImeSnapshot {
            validity: Validity::Invalid,
            conversion_mode: 0,
            language_id,
            is_open: false,
        }
    }

    unsafe fn query_live_candidates(
        &mut self,
        candidates: &[HWND],
        foreground: ForegroundTargets,
        focused_host: FocusedInputHost,
        now: Instant,
    ) -> Option<ImeSnapshot> {
        let mut queried_ime_windows = HashSet::<usize>::with_capacity(candidates.len());

        for &hwnd in candidates {
            let thread_id = window_thread(hwnd);
            let ime_window = ImmGetDefaultIMEWnd(hwnd);
            if !ime_window.is_null() && queried_ime_windows.insert(ime_window as usize) {
                if let Some(query) = query_ime_window(ime_window) {
                    self.live_target = Some(CachedLiveTarget {
                        foreground,
                        focused_host,
                        source: CachedLiveSource::ImeWindow {
                            window: hwnd,
                            ime_window,
                        },
                    });
                    return Some(self.record_live_snapshot(thread_id, hwnd, query, now));
                }
            }

            // Use the direct input context only as a fallback. Chromium input
            // fields and console hosts can expose a valid HIMC even when their
            // default IME helper window is absent or unresponsive.
            if let Some(query) = query_ime_context(hwnd) {
                self.live_target = Some(CachedLiveTarget {
                    foreground,
                    focused_host,
                    source: CachedLiveSource::InputContext { window: hwnd },
                });
                return Some(self.record_live_snapshot(thread_id, hwnd, query, now));
            }
        }
        None
    }

    unsafe fn query_cached_live_target(
        &mut self,
        foreground: ForegroundTargets,
        focused_host: FocusedInputHost,
        now: Instant,
    ) -> Option<ImeSnapshot> {
        let cached = self.live_target?;
        if cached.foreground != foreground || cached.focused_host != focused_host {
            self.live_target = None;
            return None;
        }

        let (window, query) = match cached.source {
            CachedLiveSource::ImeWindow { window, ime_window }
                if IsWindow(window) != FALSE
                    && IsWindow(ime_window) != FALSE
                    && ImmGetDefaultIMEWnd(window) == ime_window =>
            {
                (window, query_ime_window(ime_window))
            }
            CachedLiveSource::InputContext { window } if IsWindow(window) != FALSE => {
                (window, query_ime_context(window))
            }
            _ => {
                self.live_target = None;
                return None;
            }
        };

        let Some(query) = query else {
            self.live_target = None;
            return None;
        };
        Some(self.record_live_snapshot(window_thread(window), window, query, now))
    }

    fn record_live_snapshot(
        &mut self,
        thread_id: u32,
        window: HWND,
        query: ImeWindowQuery,
        now: Instant,
    ) -> ImeSnapshot {
        let language_id = unsafe { thread_language(thread_id) };
        let mode = normalized_conversion_mode(query, language_id);
        self.cache.insert(
            cache_key(thread_id, window),
            CachedIme {
                mode,
                is_open: query.is_open,
                language_id,
                tick: now,
            },
        );
        ImeSnapshot {
            validity: Validity::Live,
            conversion_mode: mode,
            language_id,
            is_open: query.is_open,
        }
    }

    unsafe fn cached_shell_input_windows(
        &mut self,
        foreground_process_id: u32,
        focused_process_id: u32,
        now: Instant,
    ) -> Vec<HWND> {
        if let Some(cached) = &self.shell_input_windows {
            if cached.foreground_process_id == foreground_process_id
                && cached.focused_process_id == focused_process_id
                && now
                    .checked_duration_since(cached.tick)
                    .is_some_and(|age| age <= SHELL_INPUT_WINDOW_CACHE)
            {
                return cached.windows.clone();
            }
        }

        let mut windows = Vec::new();
        let mut seen = HashSet::new();
        for process_id in
            modern_shell_input_processes(foreground_process_id, focused_process_id)
        {
            add_process_windows(&mut windows, &mut seen, process_id);
            if windows.len() >= MAX_ENUMERATED_INPUT_WINDOWS {
                break;
            }
        }
        self.shell_input_windows = Some(CachedShellInputWindows {
            foreground_process_id,
            focused_process_id,
            windows: windows.clone(),
            tick: now,
        });
        windows
    }
}

fn normalized_conversion_mode(query: ImeWindowQuery, language_id: u16) -> u32 {
    if !query.is_open || query.conversion_valid {
        return query.conversion_mode;
    }

    // Some modern IMEs report only the open state. Preserve the native-mode
    // distinction instead of treating a missing conversion value as English.
    match language_id & 0x03ff {
        LANG_JAPANESE => 9,
        LANG_KOREAN => 1,
        _ => 0,
    }
}

unsafe fn query_ime_context(window: HWND) -> Option<ImeWindowQuery> {
    if window.is_null() || IsWindow(window) == FALSE {
        return None;
    }

    let context = ImmGetContext(window);
    if context.is_null() {
        return None;
    }

    let is_open = ImmGetOpenStatus(context) != FALSE;
    let mut conversion_mode = 0u32;
    let mut sentence_mode = 0u32;
    let conversion_valid = is_open
        && ImmGetConversionStatus(
            context,
            &mut conversion_mode,
            &mut sentence_mode,
        ) != FALSE;
    ImmReleaseContext(window, context);

    Some(ImeWindowQuery {
        is_open,
        conversion_mode,
        conversion_valid,
    })
}

#[derive(Clone, Copy, Debug)]
struct ImeWindowQuery {
    is_open: bool,
    conversion_mode: u32,
    conversion_valid: bool,
}

unsafe fn query_ime_window(ime_window: HWND) -> Option<ImeWindowQuery> {
    if ime_window.is_null() || IsWindow(ime_window) == FALSE {
        return None;
    }

    let open_result = send_ime_control(ime_window, IMC_GETOPENSTATUS)?;
    let is_open = open_result != 0;
    if !is_open {
        return Some(ImeWindowQuery {
            is_open: false,
            conversion_mode: 0,
            conversion_valid: false,
        });
    }

    match send_ime_control(ime_window, IMC_GETCONVERSIONMODE) {
        Some(mode) => Some(ImeWindowQuery {
            is_open: true,
            conversion_mode: mode as u32,
            conversion_valid: true,
        }),
        None => Some(ImeWindowQuery {
            is_open: true,
            conversion_mode: 0,
            conversion_valid: false,
        }),
    }
}

unsafe fn send_ime_control(ime_window: HWND, command: WPARAM) -> Option<usize> {
    let mut result = 0usize;
    let ok = SendMessageTimeoutW(
        ime_window,
        WM_IME_CONTROL,
        command,
        0,
        SMTO_BLOCK | SMTO_ABORTIFHUNG,
        50,
        &mut result,
    );
    (ok != 0).then_some(result)
}

unsafe fn foreground_targets() -> ForegroundTargets {
    let foreground = GetForegroundWindow();
    let thread_id = window_thread(foreground);
    let mut result = ForegroundTargets {
        active: foreground,
        focus: null_mut(),
        caret: null_mut(),
        foreground,
    };

    if thread_id == 0 {
        return result;
    }

    let mut info: GUITHREADINFO = zeroed();
    info.cbSize = size_of::<GUITHREADINFO>() as u32;
    if GetGUIThreadInfo(thread_id, &mut info) != FALSE {
        result.active = if info.hwndActive.is_null() {
            foreground
        } else {
            info.hwndActive
        };
        result.focus = info.hwndFocus;
        result.caret = info.hwndCaret;
    }
    result
}


unsafe fn add_window_chain(list: &mut Vec<HWND>, seen: &mut HashSet<usize>, hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }

    let original = hwnd;
    let mut current = hwnd;
    for _ in 0..8 {
        add_window_candidate(list, seen, current);
        let parent = GetParent(current);
        if parent.is_null() || parent == current {
            break;
        }
        current = parent;
    }

    add_window_candidate(list, seen, GetAncestor(original, GA_ROOT));
    add_window_candidate(list, seen, GetAncestor(original, GA_ROOTOWNER));
}

unsafe fn add_window_candidate(list: &mut Vec<HWND>, seen: &mut HashSet<usize>, hwnd: HWND) {
    if hwnd.is_null() || IsWindow(hwnd) == FALSE {
        return;
    }
    if seen.insert(hwnd as usize) {
        list.push(hwnd);
    }
}

struct ProcessWindowEnumeration {
    process_id: u32,
    windows: Vec<HWND>,
}

unsafe extern "system" fn enum_process_window(hwnd: HWND, parameter: LPARAM) -> BOOL {
    if parameter == 0 {
        return FALSE;
    }
    let enumeration = &mut *(parameter as *mut ProcessWindowEnumeration);
    if enumeration.windows.len() >= MAX_ENUMERATED_INPUT_WINDOWS {
        return FALSE;
    }

    let mut process_id = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id);
    if process_id == enumeration.process_id {
        enumeration.windows.push(hwnd);
    }
    TRUE
}

unsafe fn add_process_windows(
    list: &mut Vec<HWND>,
    seen: &mut HashSet<usize>,
    process_id: u32,
) {
    if process_id == 0 || list.len() >= MAX_ENUMERATED_INPUT_WINDOWS {
        return;
    }

    let mut top_level = ProcessWindowEnumeration {
        process_id,
        windows: Vec::new(),
    };
    EnumWindows(
        Some(enum_process_window),
        &mut top_level as *mut ProcessWindowEnumeration as LPARAM,
    );

    for window in top_level.windows {
        add_window_candidate(list, seen, window);
        if list.len() >= MAX_ENUMERATED_INPUT_WINDOWS {
            break;
        }

        let mut children = ProcessWindowEnumeration {
            process_id,
            windows: Vec::new(),
        };
        EnumChildWindows(
            window,
            Some(enum_process_window),
            &mut children as *mut ProcessWindowEnumeration as LPARAM,
        );
        for child in children.windows {
            add_window_candidate(list, seen, child);
            if list.len() >= MAX_ENUMERATED_INPUT_WINDOWS {
                break;
            }
        }
    }
}

unsafe fn modern_shell_input_processes(
    foreground_process_id: u32,
    focused_process_id: u32,
) -> Vec<u32> {
    let reference_process_id = if focused_process_id != 0 {
        focused_process_id
    } else {
        foreground_process_id
    };
    if reference_process_id == 0 {
        return Vec::new();
    }

    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return Vec::new();
    }

    let mut reference_session = 0;
    let filter_session =
        ProcessIdToSessionId(reference_process_id, &mut reference_session) != FALSE;
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut processes = Vec::<(u32, String)>::new();

    if Process32FirstW(snapshot, &mut entry) != FALSE {
        loop {
            let process_id = entry.th32ProcessID;
            let mut session_id = 0;
            let same_session = !filter_session
                || (ProcessIdToSessionId(process_id, &mut session_id) != FALSE
                    && session_id == reference_session);
            if process_id != 0 && same_session {
                processes.push((process_id, executable_name(&entry.szExeFile)));
            }

            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            if Process32NextW(snapshot, &mut entry) == FALSE {
                break;
            }
        }
    }
    CloseHandle(snapshot);

    let shell_has_focus = processes.iter().any(|(process_id, name)| {
        (*process_id == foreground_process_id || *process_id == focused_process_id)
            && is_modern_shell_focus_process(name)
    });
    if !shell_has_focus {
        return Vec::new();
    }

    let mut candidates = processes
        .into_iter()
        .filter_map(|(process_id, name)| {
            modern_input_host_priority(&name).map(|priority| {
                let focus_priority = if process_id == focused_process_id {
                    0
                } else if process_id == foreground_process_id {
                    1
                } else {
                    2
                };
                (focus_priority, priority, process_id)
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.dedup_by_key(|candidate| candidate.2);
    candidates
        .into_iter()
        .map(|candidate| candidate.2)
        .collect()
}

fn executable_name(value: &[u16; 260]) -> String {
    let length = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..length]).to_ascii_lowercase()
}

fn is_modern_shell_focus_process(name: &str) -> bool {
    matches!(
        name,
        "searchhost.exe"
            | "searchapp.exe"
            | "startmenuexperiencehost.exe"
            | "shellexperiencehost.exe"
            | "textinputhost.exe"
            | "explorer.exe"
    )
}

pub fn is_modern_shell_overlay_window(window: HWND) -> bool {
    unsafe {
        let process_id = window_process_id(window);
        process_executable_name(process_id)
            .as_deref()
            .is_some_and(is_modern_shell_overlay_process)
    }
}

unsafe fn process_executable_name(process_id: u32) -> Option<String> {
    if process_id == 0 {
        return None;
    }

    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut result = None;
    if Process32FirstW(snapshot, &mut entry) != FALSE {
        loop {
            if entry.th32ProcessID == process_id {
                result = Some(executable_name(&entry.szExeFile));
                break;
            }
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            if Process32NextW(snapshot, &mut entry) == FALSE {
                break;
            }
        }
    }
    CloseHandle(snapshot);
    result
}

fn is_modern_shell_overlay_process(name: &str) -> bool {
    matches!(
        name,
        "searchhost.exe"
            | "searchapp.exe"
            | "startmenuexperiencehost.exe"
            | "shellexperiencehost.exe"
    )
}

fn modern_input_host_priority(name: &str) -> Option<u8> {
    match name {
        "textinputhost.exe" => Some(0),
        "searchhost.exe" | "searchapp.exe" => Some(1),
        "startmenuexperiencehost.exe" => Some(2),
        "shellexperiencehost.exe" => Some(3),
        "explorer.exe" => Some(4),
        _ => None,
    }
}

unsafe fn window_thread(hwnd: HWND) -> u32 {
    if hwnd.is_null() {
        0
    } else {
        GetWindowThreadProcessId(hwnd, null_mut())
    }
}

unsafe fn window_process_id(hwnd: HWND) -> u32 {
    if hwnd.is_null() {
        return 0;
    }
    let mut process_id = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id);
    process_id
}

unsafe fn thread_language(thread_id: u32) -> u16 {
    if thread_id == 0 {
        return 0;
    }
    let layout = GetKeyboardLayout(thread_id);
    if layout.is_null() {
        0
    } else {
        (layout as usize & 0xffff) as u16
    }
}

fn cache_key(thread_id: u32, hwnd: HWND) -> CacheKey {
    if thread_id != 0 {
        CacheKey::Thread(thread_id)
    } else {
        CacheKey::Window(hwnd as usize)
    }
}

fn first_non_null(values: &[HWND]) -> HWND {
    values
        .iter()
        .copied()
        .find(|hwnd| !hwnd.is_null())
        .unwrap_or(null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executable_name_buffer(value: &str) -> [u16; 260] {
        let mut result = [0u16; 260];
        for (index, character) in value.encode_utf16().take(259).enumerate() {
            result[index] = character;
        }
        result
    }

    #[test]
    fn language_masks_use_primary_language_bits() {
        assert_eq!(0x0412u16 & 0x03ff, LANG_KOREAN);
        assert_eq!(0x0411u16 & 0x03ff, LANG_JAPANESE);
    }

    #[test]
    fn modern_windows_input_hosts_are_narrowly_identified() {
        assert!(is_modern_shell_focus_process("searchhost.exe"));
        assert!(is_modern_shell_focus_process(
            "startmenuexperiencehost.exe"
        ));
        assert_eq!(modern_input_host_priority("textinputhost.exe"), Some(0));
        assert_eq!(modern_input_host_priority("notepad.exe"), None);
        assert!(!is_modern_shell_focus_process("notepad.exe"));
        assert!(is_modern_shell_overlay_process("searchhost.exe"));
        assert!(!is_modern_shell_overlay_process("explorer.exe"));
    }

    #[test]
    fn process_snapshot_names_are_normalized() {
        assert_eq!(
            executable_name(&executable_name_buffer("SearchHost.EXE")),
            "searchhost.exe"
        );
    }
}
