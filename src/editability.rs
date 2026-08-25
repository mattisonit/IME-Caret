use crate::win::*;
use crate::outlook::{self, OutlookEditorState};
use std::ffi::c_void;
use std::mem::{size_of, transmute, zeroed};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

const FOCUS_PROBE_CACHE_DURATION: Duration = Duration::from_millis(250);
const UIA_FOCUSED_ELEMENT_CACHE_DURATION: Duration = Duration::from_millis(100);
const OUTLOOK_CARET_CACHE_DURATION: Duration = Duration::from_millis(120);
const MAX_UIA_PARENT_DEPTH: usize = 12;
const MAX_UIA_CARET_PARENT_DEPTH: usize = 12;
const MAX_UIA_CARET_DESCENDANT_DEPTH: usize = 4;
const MAX_UIA_CARET_DESCENDANT_NODES: usize = 64;
const CLASS_NAME_CAPACITY: usize = 128;
const CONTROL_MESSAGE_TIMEOUT_MS: u32 = 25;

const UIA_BOUNDING_RECTANGLE_PROPERTY_ID: i32 = 30001;
const UIA_PROCESS_ID_PROPERTY_ID: i32 = 30002;
const UIA_CONTROL_TYPE_PROPERTY_ID: i32 = 30003;
const UIA_NAME_PROPERTY_ID: i32 = 30005;
const UIA_CLASS_NAME_PROPERTY_ID: i32 = 30012;
const UIA_HAS_KEYBOARD_FOCUS_PROPERTY_ID: i32 = 30008;
const UIA_IS_KEYBOARD_FOCUSABLE_PROPERTY_ID: i32 = 30009;
const UIA_IS_ENABLED_PROPERTY_ID: i32 = 30010;
const UIA_IS_VALUE_PATTERN_AVAILABLE_PROPERTY_ID: i32 = 30043;
const UIA_VALUE_VALUE_PROPERTY_ID: i32 = 30045;
const UIA_VALUE_IS_READ_ONLY_PROPERTY_ID: i32 = 30046;
const UIA_LEGACY_IACCESSIBLE_STATE_PROPERTY_ID: i32 = 30096;
const UIA_ARIA_ROLE_PROPERTY_ID: i32 = 30101;
const UIA_IS_TEXT_EDIT_PATTERN_AVAILABLE_PROPERTY_ID: i32 = 30149;
const UIA_NATIVE_WINDOW_HANDLE_PROPERTY_ID: i32 = 30020;

const UIA_COMBO_BOX_CONTROL_TYPE_ID: i32 = 50003;
const UIA_EDIT_CONTROL_TYPE_ID: i32 = 50004;
const UIA_TEXT_CONTROL_TYPE_ID: i32 = 50020;
const UIA_DOCUMENT_CONTROL_TYPE_ID: i32 = 50030;

const STATE_SYSTEM_UNAVAILABLE: i32 = 0x0000_0001;
const STATE_SYSTEM_READONLY: i32 = 0x0000_0040;

const CLSID_CUIAUTOMATION: GUID = GUID {
    Data1: 0xff48dba4,
    Data2: 0x60ef,
    Data3: 0x4201,
    Data4: [0xaa, 0x87, 0x54, 0x10, 0x3e, 0xef, 0x59, 0x4e],
};

const IID_IUIAUTOMATION: GUID = GUID {
    Data1: 0x30cbe57d,
    Data2: 0xd9d0,
    Data3: 0x452a,
    Data4: [0xab, 0x13, 0x7a, 0xc5, 0xac, 0x48, 0x25, 0xee],
};

const IID_IUIAUTOMATION_TEXT_PATTERN: GUID = GUID {
    Data1: 0x32eba289,
    Data2: 0x3583,
    Data3: 0x42c9,
    Data4: [0x9c, 0x59, 0x3b, 0x6d, 0x9a, 0x1e, 0x9b, 0x6a],
};

const IID_IUIAUTOMATION_TEXT_PATTERN2: GUID = GUID {
    Data1: 0x506a921a,
    Data2: 0xfcc9,
    Data3: 0x409f,
    Data4: [0xb2, 0x3b, 0x37, 0xeb, 0x74, 0x10, 0x68, 0x72],
};

const IID_IUIAUTOMATION_TEXT_EDIT_PATTERN: GUID = GUID {
    Data1: 0x17e21576,
    Data2: 0x996c,
    Data3: 0x4870,
    Data4: [0x99, 0xd9, 0xbf, 0xf3, 0x23, 0x38, 0x0c, 0x06],
};

const IID_IACCESSIBLE: GUID = GUID {
    Data1: 0x618736e0,
    Data2: 0x3c3d,
    Data3: 0x11cf,
    Data4: [0x81, 0x0c, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
};

const UIA_TEXT_PATTERN_ID: i32 = 10014;
const UIA_TEXT_PATTERN2_ID: i32 = 10024;
const UIA_TEXT_EDIT_PATTERN_ID: i32 = 10032;
const OBJID_CARET: DWORD = 0xffff_fff8;
const CHILDID_SELF: i32 = 0;
// Outlook 2016 uses the same _WwG class for readable and editable message
// bodies. A standalone viewer marks its containing _WwB window with these
// style bits, while the main-window Reading Pane embeds its first
// rctrl_renwnd32 host as a child window.
const MAX_OFFICE_WORD_HOST_PARENT_DEPTH: usize = 8;
const MAX_ACCESSIBLE_CARET_WIDTH: i32 = 8;
const MAX_ACCESSIBLE_CARET_HEIGHT: i32 = 256;
const TEXT_PATTERN_RANGE_ENDPOINT_START: i32 = 0;
const TEXT_PATTERN_RANGE_ENDPOINT_END: i32 = 1;
const TEXT_UNIT_CHARACTER: i32 = 0;
const MAX_DIRECT_CARET_RECT_WIDTH: f64 = 4.0;
const MAX_CHARACTER_RECT_WIDTH: f64 = 64.0;
const ELEMENT_ANCHOR_TOLERANCE: f64 = 6.0;
const VT_ARRAY_R8: u16 = 0x2000 | 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Editability {
    Editable,
    ReadOnly,
    Unknown,
}

impl Editability {
    /// Returns true only when the focused element exposes positive evidence
    /// that it accepts text input. Ambiguous selectable text stays excluded.
    pub fn accepts_text_input(self) -> bool {
        matches!(self, Self::Editable)
    }
}

/// Screen-space geometry for the active insertion caret. `x` is the edge
/// immediately to the right of the caret; `top` and `bottom` preserve the
/// caret height so the IME indicator can align to its lower edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaretAnchor {
    pub x: i32,
    pub top: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusedInputHost {
    pub native_window: HWND,
    pub process_id: u32,
}

impl Default for FocusedInputHost {
    fn default() -> Self {
        Self {
            native_window: null_mut(),
            process_id: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct CachedFocusProbe {
    foreground: HWND,
    focus: HWND,
    caret: HWND,
    result: Editability,
    tick: Instant,
}

struct CachedFocusedElement {
    foreground: HWND,
    focus: HWND,
    caret: HWND,
    element: *mut c_void,
    tick: Instant,
}

#[derive(Clone, Copy)]
struct CachedOutlookEditability {
    foreground: HWND,
    focus: HWND,
    caret: HWND,
    result: Editability,
}

#[derive(Clone, Copy)]
struct CachedOutlookCaretAnchor {
    foreground: HWND,
    focus: HWND,
    caret: HWND,
    window: HWND,
    anchor: CaretAnchor,
    tick: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NodeEvidence {
    EditableField,
    EditableDocument,
    ReadOnly,
    SelectableText,
    CodeLikeText,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaretProbeResult {
    Found(CaretAnchor),
    Suppress,
    Missing,
}

fn evidence_accepts_caret_probe(
    evidence: NodeEvidence,
    console_like: bool,
    exact_geometry_only: bool,
) -> bool {
    console_like
        || matches!(evidence, NodeEvidence::EditableField)
        || (!exact_geometry_only && matches!(evidence, NodeEvidence::EditableDocument))
}

/// Determines whether the foreground focus/caret belongs to an editable text
/// surface. Mouse position and mouse cursor state are intentionally ignored.
pub struct EditabilityDetector {
    automation: *mut c_void,
    raw_view_walker: *mut c_void,
    co_initialized: bool,
    last_focus_probe: Option<CachedFocusProbe>,
    focused_element_cache: Option<CachedFocusedElement>,
    outlook_editability_cache: Option<CachedOutlookEditability>,
    outlook_caret_anchor_cache: Option<CachedOutlookCaretAnchor>,
    attached_console_pid: u32,
}

impl EditabilityDetector {
    /// Creates the UI Automation client used as a fallback for non-standard
    /// controls such as browsers, Electron, WPF, and WinUI.
    pub fn new() -> Self {
        unsafe { Self::new_unsafe() }
    }

    unsafe fn new_unsafe() -> Self {
        let init_result = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED);
        let co_initialized = init_result == S_OK || init_result == S_FALSE;

        let mut automation = null_mut();
        let create_result = CoCreateInstance(
            &CLSID_CUIAUTOMATION,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IUIAUTOMATION,
            &mut automation,
        );
        if create_result < 0 {
            automation = null_mut();
        }

        let raw_view_walker = if automation.is_null() {
            null_mut()
        } else {
            get_raw_view_walker(automation).unwrap_or(null_mut())
        };

        Self {
            automation,
            raw_view_walker,
            co_initialized,
            last_focus_probe: None,
            focused_element_cache: None,
            outlook_editability_cache: None,
            outlook_caret_anchor_cache: None,
            attached_console_pid: 0,
        }
    }

    /// Returns whether the foreground application currently owns an editable
    /// focused control or caret. This probe is independent of the mouse
    /// position and is used to keep the IME state indicator attached to the
    /// blinking text caret while the pointer is parked elsewhere.
    pub fn focused_input(&mut self) -> Editability {
        unsafe { self.focused_input_unsafe() }
    }

    /// Invalidates only the focus classification cache. Foreground and focus
    /// events call this before an immediate refresh so a newly focused control
    /// is never classified using the prior result.
    pub fn invalidate_focus_cache(&mut self) {
        self.last_focus_probe = None;
        self.outlook_editability_cache = None;
        self.outlook_caret_anchor_cache = None;
        unsafe {
            self.clear_focused_element_cache();
        }
    }

    unsafe fn focused_element_for_targets(
        &mut self,
        targets: ForegroundFocusTargets,
    ) -> Option<*mut c_void> {
        if self.automation.is_null() || targets.foreground.is_null() {
            self.clear_focused_element_cache();
            return None;
        }

        let now = Instant::now();
        let cached_element = self
            .focused_element_cache
            .as_ref()
            .filter(|cached| {
                cached.foreground == targets.foreground
                    && cached.focus == targets.focus
                    && cached.caret == targets.caret
                    && now
                        .checked_duration_since(cached.tick)
                        .is_some_and(|age| age <= UIA_FOCUSED_ELEMENT_CACHE_DURATION)
            })
            .map(|cached| cached.element);
        if let Some(element) = cached_element {
            if add_ref_com(element) {
                return Some(element);
            }
        }

        self.clear_focused_element_cache();
        let element = get_focused_element(self.automation)?;

        if add_ref_com(element) {
            self.focused_element_cache = Some(CachedFocusedElement {
                foreground: targets.foreground,
                focus: targets.focus,
                caret: targets.caret,
                element,
                tick: now,
            });
        }
        Some(element)
    }

    unsafe fn clear_focused_element_cache(&mut self) {
        if let Some(cached) = self.focused_element_cache.take() {
            if !cached.element.is_null() {
                release_com(cached.element);
            }
        }
    }

    /// Returns the native window and process published by the focused UI
    /// Automation element. Modern Windows shell surfaces can render the
    /// visible search box in one window while hosting its IME context in
    /// another process, so the IME engine uses this identity as an additional
    /// candidate instead of relying only on GetForegroundWindow.
    pub fn focused_input_host(&mut self) -> FocusedInputHost {
        unsafe { self.focused_input_host_unsafe() }
    }

    /// Returns a screen-space anchor immediately beside the active text caret.
    /// Classic consoles use their screen-buffer cursor before the generic
    /// Win32 GUI-thread caret so IME composition does not leave the indicator
    /// at a stale composition start. Other controls use
    /// TextPattern2 and the older TextPattern selection as fallbacks.
    pub fn focused_caret_anchor(&mut self, console_cell_span: i32) -> Option<CaretAnchor> {
        unsafe { self.focused_caret_anchor_unsafe(console_cell_span.clamp(1, 2)) }
    }

    unsafe fn focused_caret_anchor_unsafe(
        &mut self,
        console_cell_span: i32,
    ) -> Option<CaretAnchor> {
        let targets = foreground_focus_targets();
        if targets.foreground.is_null() {
            self.detach_console();
            return None;
        }

        let console_like = is_console_like_window(targets.foreground);
        let classic_console = is_classic_console_window(targets.foreground);
        let foreground_class = window_class_name(targets.foreground).unwrap_or_default();
        let uia_preferred = is_uia_preferred_caret_class(&foreground_class);
        let office_uia_preferred = is_office_uia_preferred_caret_class(&foreground_class);
        let force_exact_uia_geometry = uia_preferred && is_chromium_widget_class(&foreground_class);
        if !classic_console {
            self.detach_console();
        }

        // Classic cmd.exe reports the screen-buffer cursor at the first cell
        // occupied by an active IME composition. Use the current input mode as
        // a one- or two-cell visual span, and replace it with the measured
        // composition width when IMM exposes one.
        if classic_console {
            if let Some(anchor) = self.classic_console_caret_anchor(
                targets.foreground,
                targets.process_id,
                [targets.focus, targets.caret, targets.foreground],
                console_cell_span,
            ) {
                return Some(anchor);
            }
        }

        // Chromium/Electron create a compatibility Win32 caret whose position
        // can remain at the beginning of the address bar. UI Automation is the
        // authoritative source in these hosts. Suppressed selections and
        // missing exact geometry must not fall through to that stale caret.
        if uia_preferred {
            // Chromium Views publishes a dedicated accessibility system-caret
            // object whose accLocation tracks the native textfield cursor more
            // reliably than the compatibility Win32 caret or a stale
            // TextPattern range. Accept it only when it lies inside the
            // focused editable UIA element.
            if let Some(anchor) = self.accessible_focused_caret_anchor(
                targets,
                targets.foreground,
                force_exact_uia_geometry,
            ) {
                return Some(anchor);
            }
            return match self.uia_focused_caret_anchor(
                targets,
                console_like,
                force_exact_uia_geometry,
            ) {
                CaretProbeResult::Found(anchor) => Some(anchor),
                CaretProbeResult::Suppress | CaretProbeResult::Missing => None,
            };
        }

        // Console hosts can expose a stale compatibility Win32 caret at the
        // beginning of an IME composition. Classic conhost has already used
        // the screen-buffer path above; Windows Terminal and any failed
        // classic attachment must use their text provider instead of falling
        // through to that stale GUI-thread caret.
        if console_like {
            return match self.uia_focused_caret_anchor(targets, true, false) {
                CaretProbeResult::Found(anchor) => Some(anchor),
                CaretProbeResult::Suppress | CaretProbeResult::Missing => None,
            };
        }

        // Excel uses private editor windows instead of a standard Edit
        // control. EXCEL6 is the in-cell editor (including current Microsoft
        // 365 builds), EXCEL< is the formula-bar editor, and EDTBX is used by
        // classic Find/Replace dialogs. Prefer Excel's GUI-thread caret and
        // fall back to its Active Accessibility caret when Office doesn't
        // publish a UI Automation text range.
        if let Some(window) = excel_editor_window(targets) {
            if let Some(anchor) = win32_caret_anchor(targets) {
                return Some(anchor);
            }
            if let Some(anchor) = accessible_system_caret_anchor(window)
                .filter(|anchor| anchor_matches_window(*anchor, window))
            {
                return Some(anchor);
            }
            if let CaretProbeResult::Found(anchor) =
                self.uia_focused_caret_anchor(targets, false, false)
            {
                return Some(anchor);
            }
            if let Some(anchor) = self.excel_dialog_focused_element_anchor(targets) {
                return Some(anchor);
            }
            return excel_dialog_editor_bounds_anchor(targets, window);
        }
        if let Some(anchor) = self.excel_dialog_focused_element_anchor(targets) {
            return Some(anchor);
        }
        if let Some(anchor) = excel_dialog_caret_anchor(targets) {
            return Some(anchor);
        }

        // Outlook's classic mail composer hosts its message body in Word's
        // innermost _WwG document window. Outlook 2016 exposes a GUI-thread
        // caret for read-only message viewers, while the editable composer
        // publishes only an Active Accessibility caret. Office publishes that
        // accessibility caret as a short-lived object, so acquire it for each
        // position query but never probe the same window twice per refresh.
        let mut saw_outlook_word_editor = false;
        let mut last_office_window = null_mut();
        for window in [targets.caret, targets.focus] {
            if !window.is_null() && is_office_word_editor_window(window) {
                if window == last_office_window {
                    continue;
                }
                last_office_window = window;

                if outlook_word_host(window).is_some() {
                    saw_outlook_word_editor = true;
                    if matches!(
                        self.classify_office_word_editor_target(targets, window),
                        Some(Editability::Editable)
                    ) {
                        if let Some(anchor) = self.outlook_caret_anchor(targets, window) {
                            return Some(anchor);
                        }
                    }
                } else if let Some(anchor) = accessible_system_caret_anchor(window)
                    .filter(|anchor| anchor_matches_window(*anchor, window))
                {
                    // Word uses the same _WwG document class as Outlook's
                    // embedded Word editor. Only the rctrl_renwnd32 ancestry
                    // identifies Outlook; a normal Word document must retain
                    // its own accessibility caret and generic fallbacks.
                    return Some(anchor);
                }
            }
        }

        if saw_outlook_word_editor {
            return None;
        }

        // PowerPoint can publish a compatibility Win32 caret that
        // is absent or detached from the visible insertion point. Prefer the
        // focused UI Automation text range, then the Office accessibility
        // caret, and do not fall through to stale Win32 geometry.
        if office_uia_preferred {
            match self.uia_focused_caret_anchor(targets, false, false) {
                CaretProbeResult::Found(anchor) => return Some(anchor),
                CaretProbeResult::Suppress => return None,
                CaretProbeResult::Missing => {}
            }
            return office_accessible_caret_anchor(targets);
        }

        if let Some(anchor) = win32_caret_anchor(targets) {
            return Some(anchor);
        }

        match self.uia_focused_caret_anchor(targets, false, false) {
            CaretProbeResult::Found(anchor) => Some(anchor),
            CaretProbeResult::Suppress | CaretProbeResult::Missing => None,
        }
    }

    unsafe fn outlook_caret_anchor(
        &mut self,
        targets: ForegroundFocusTargets,
        window: HWND,
    ) -> Option<CaretAnchor> {
        let now = Instant::now();
        if let Some(cached) = self.outlook_caret_anchor_cache {
            if cached.foreground == targets.foreground
                && cached.focus == targets.focus
                && cached.caret == targets.caret
                && cached.window == window
                && now
                    .checked_duration_since(cached.tick)
                    .is_some_and(|age| age <= OUTLOOK_CARET_CACHE_DURATION)
            {
                return Some(cached.anchor);
            }
        }

        let anchor = accessible_system_caret_anchor(window)
            .filter(|anchor| anchor_matches_window(*anchor, window));
        self.outlook_caret_anchor_cache = anchor.map(|anchor| CachedOutlookCaretAnchor {
            foreground: targets.foreground,
            focus: targets.focus,
            caret: targets.caret,
            window,
            anchor,
            tick: now,
        });
        anchor
    }

    unsafe fn accessible_focused_caret_anchor(
        &mut self,
        targets: ForegroundFocusTargets,
        window: HWND,
        exact_geometry_only: bool,
    ) -> Option<CaretAnchor> {
        let anchor = accessible_system_caret_anchor(window)?;
        if !anchor_matches_window(anchor, window) {
            return None;
        }

        if self.automation.is_null() {
            return (!exact_geometry_only).then_some(anchor);
        }

        let Some(mut element) = self.focused_element_for_targets(targets) else {
            return (!exact_geometry_only).then_some(anchor);
        };

        // Browser chrome can put focus on a wrapper while the real Edit node
        // is a nearby raw-view descendant. Validate the accessibility caret
        // against that bounded subtree before walking up the parent chain.
        if !self.raw_view_walker.is_null() {
            let mut remaining = MAX_UIA_CARET_DESCENDANT_NODES;
            if descendant_contains_matching_anchor(
                self.raw_view_walker,
                element,
                MAX_UIA_CARET_DESCENDANT_DEPTH,
                &mut remaining,
                anchor,
                exact_geometry_only,
            ) {
                release_com(element);
                return Some(anchor);
            }
        }

        for _ in 0..=MAX_UIA_CARET_PARENT_DEPTH {
            let evidence = inspect_element(element);
            if evidence_accepts_caret_probe(evidence, false, exact_geometry_only)
                && anchor_matches_element(anchor, element)
            {
                release_com(element);
                return Some(anchor);
            }

            if self.raw_view_walker.is_null() {
                break;
            }
            let Some(parent) = tree_walker_parent(self.raw_view_walker, element) else {
                break;
            };
            release_com(element);
            element = parent;
        }
        release_com(element);
        None
    }

    unsafe fn focused_input_unsafe(&mut self) -> Editability {
        let targets = foreground_focus_targets();
        if targets.foreground.is_null() {
            self.clear_focused_element_cache();
            return Editability::Unknown;
        }

        let now = Instant::now();
        if !is_classic_console_window(targets.foreground) {
            self.detach_console();
        }
        if is_console_like_window(targets.foreground) {
            let result = Editability::Editable;
            self.last_focus_probe = Some(CachedFocusProbe {
                foreground: targets.foreground,
                focus: targets.focus,
                caret: targets.caret,
                result,
                tick: now,
            });
            return result;
        }

        let foreground_class = window_class_name(targets.foreground).unwrap_or_default();
        if excel_editor_window(targets).is_some()
            || self
                .excel_dialog_focused_element_anchor(targets)
                .is_some()
            || excel_dialog_caret_anchor(targets).is_some()
        {
            let result = Editability::Editable;
            self.last_focus_probe = Some(CachedFocusProbe {
                foreground: targets.foreground,
                focus: targets.focus,
                caret: targets.caret,
                result,
                tick: now,
            });
            return result;
        }

        if foreground_class.eq_ignore_ascii_case("OpusApp")
            && [targets.caret, targets.focus].into_iter().any(|window| {
                !window.is_null()
                    && IsWindowEnabled(window) != FALSE
                    && is_office_word_editor_window(window)
            })
        {
            // A focused _WwG hosted by Word itself is the document editor.
            // Recognize it directly instead of waiting for Word's UIA tree to
            // publish its editable state. Outlook is excluded by its distinct
            // foreground frame and continues through the guarded mail path.
            let result = Editability::Editable;
            self.last_focus_probe = Some(CachedFocusProbe {
                foreground: targets.foreground,
                focus: targets.focus,
                caret: targets.caret,
                result,
                tick: now,
            });
            return result;
        }

        if let Some(cached) = self.last_focus_probe {
            if cached.foreground == targets.foreground
                && cached.focus == targets.focus
                && cached.caret == targets.caret
                && now
                    .checked_duration_since(cached.tick)
                    .is_some_and(|age| age <= FOCUS_PROBE_CACHE_DURATION)
            {
                return cached.result;
            }
        }

        let mut saw_read_only = false;
        let mut office_word_editability = None;
        for window in [targets.caret, targets.focus] {
            if window.is_null() {
                continue;
            }
            if is_office_word_editor_window(window) {
                if outlook_word_host(window).is_some() && office_word_editability.is_none() {
                    office_word_editability =
                        self.classify_office_word_editor_target(targets, window);
                }
                continue;
            }
            match classify_standard_window(window) {
                Some(Editability::Editable) => {
                    let result = Editability::Editable;
                    self.last_focus_probe = Some(CachedFocusProbe {
                        foreground: targets.foreground,
                        focus: targets.focus,
                        caret: targets.caret,
                        result,
                        tick: now,
                    });
                    return result;
                }
                Some(Editability::ReadOnly) => saw_read_only = true,
                _ => {}
            }
        }

        if is_office_uia_preferred_caret_class(&foreground_class)
            && office_accessible_caret_anchor(targets).is_some()
        {
            let result = Editability::Editable;
            self.last_focus_probe = Some(CachedFocusProbe {
                foreground: targets.foreground,
                focus: targets.focus,
                caret: targets.caret,
                result,
                tick: now,
            });
            return result;
        }

        let result = match office_word_editability {
            Some(result) => result,
            None => {
                let uia_result = self.classify_focused_with_uia(targets);
                match uia_result {
                    Editability::Editable => Editability::Editable,
                    Editability::ReadOnly => Editability::ReadOnly,
                    Editability::Unknown if saw_read_only => Editability::ReadOnly,
                    Editability::Unknown => Editability::Unknown,
                }
            }
        };
        self.last_focus_probe = Some(CachedFocusProbe {
            foreground: targets.foreground,
            focus: targets.focus,
            caret: targets.caret,
            result,
            tick: now,
        });
        result
    }

    unsafe fn excel_dialog_focused_element_anchor(
        &mut self,
        targets: ForegroundFocusTargets,
    ) -> Option<CaretAnchor> {
        let dialog = excel_dialog_root_for_targets(targets)?;
        let mut element = self.focused_element_for_targets(targets)?;
        let mut anchor = None;

        // Excel 2016's Find/Replace dialog can focus its first field without
        // creating either a Win32 or Active Accessibility caret. UI
        // Automation still exposes the focused Edit element, so use the
        // field's left inset as a temporary anchor until typing creates the
        // real caret. Keep this fallback strictly inside bosa_sdm_XL dialogs.
        for _ in 0..=MAX_UIA_PARENT_DEPTH {
            let class_name = property_string(element, UIA_CLASS_NAME_PROPERTY_ID)
                .unwrap_or_default();
            let control_type = property_i32(element, UIA_CONTROL_TYPE_PROPERTY_ID);
            let is_editor = property_bool(element, UIA_IS_ENABLED_PROPERTY_ID) != Some(false)
                && (class_name.eq_ignore_ascii_case("EDTBX")
                    || class_name.eq_ignore_ascii_case("Edit")
                    || control_type == Some(UIA_EDIT_CONTROL_TYPE_ID)
                    || matches!(inspect_element(element), NodeEvidence::EditableField));

            if is_editor {
                anchor = property_bounding_rect(element)
                    .and_then(editable_element_caret_anchor)
                    .filter(|candidate| anchor_matches_window(*candidate, dialog));
                break;
            }

            if self.raw_view_walker.is_null() {
                break;
            }
            let Some(parent) = tree_walker_parent(self.raw_view_walker, element) else {
                break;
            };
            release_com(element);
            element = parent;
        }
        release_com(element);
        anchor
    }

    unsafe fn classify_office_word_editor_target(
        &mut self,
        targets: ForegroundFocusTargets,
        window: HWND,
    ) -> Option<Editability> {
        if !is_office_word_editor_window(window) {
            return None;
        }

        if outlook_word_host(window).is_some() {
            if let Some(cached) = self.outlook_editability_cache {
                if cached.foreground == targets.foreground
                    && cached.focus == targets.focus
                    && cached.caret == targets.caret
                {
                    return Some(cached.result);
                }
            }

            let result = match outlook::editor_state(targets.foreground) {
                OutlookEditorState::Editable => Editability::Editable,
                OutlookEditorState::ReadOnly => Editability::ReadOnly,
                // Fail closed for an Outlook document. A transient Automation
                // failure may briefly hide the badge, but can never expose it
                // in a read-only mail body.
                OutlookEditorState::Unknown => Editability::ReadOnly,
            };
            self.outlook_editability_cache = Some(CachedOutlookEditability {
                foreground: targets.foreground,
                focus: targets.focus,
                caret: targets.caret,
                result,
            });
            return Some(result);
        }

        None
    }

    unsafe fn focused_input_host_unsafe(&mut self) -> FocusedInputHost {
        if self.automation.is_null() {
            return FocusedInputHost::default();
        }

        let targets = foreground_focus_targets();
        let Some(mut element) = self.focused_element_for_targets(targets) else {
            return FocusedInputHost::default();
        };
        let mut host = FocusedInputHost::default();

        for _ in 0..=MAX_UIA_PARENT_DEPTH {
            if host.process_id == 0 {
                host.process_id = property_i32(element, UIA_PROCESS_ID_PROPERTY_ID)
                    .filter(|value| *value > 0)
                    .map(|value| value as u32)
                    .unwrap_or(0);
            }
            if host.native_window.is_null() {
                host.native_window = property_i32(element, UIA_NATIVE_WINDOW_HANDLE_PROPERTY_ID)
                    .filter(|value| *value != 0)
                    .map(|value| value as u32 as usize as HWND)
                    .unwrap_or(null_mut());
            }
            if host.process_id != 0 && !host.native_window.is_null() {
                break;
            }

            if self.raw_view_walker.is_null() {
                break;
            }
            let Some(parent) = tree_walker_parent(self.raw_view_walker, element) else {
                break;
            };
            release_com(element);
            element = parent;
        }
        release_com(element);

        if host.process_id == 0 && !host.native_window.is_null() {
            GetWindowThreadProcessId(host.native_window, &mut host.process_id);
        }
        host
    }

    unsafe fn classify_focused_with_uia(&mut self, targets: ForegroundFocusTargets) -> Editability {
        if self.automation.is_null() {
            return Editability::Unknown;
        }

        let Some(element) = self.focused_element_for_targets(targets) else {
            return Editability::Unknown;
        };

        // Chromium can report focus on an anonymous wrapper while the actual
        // search/edit element is a nearby descendant. Probe only a small raw
        // subtree so YouTube-style search boxes are recognized without
        // turning an entire browser document into an editable surface.
        if !self.raw_view_walker.is_null() {
            let mut remaining = MAX_UIA_CARET_DESCENDANT_NODES;
            if descendant_contains_editable(
                self.raw_view_walker,
                element,
                MAX_UIA_CARET_DESCENDANT_DEPTH,
                &mut remaining,
            ) {
                release_com(element);
                return Editability::Editable;
            }
        }

        self.classify_element_chain(element)
    }

    unsafe fn uia_focused_caret_anchor(
        &mut self,
        targets: ForegroundFocusTargets,
        console_like: bool,
        exact_geometry_only: bool,
    ) -> CaretProbeResult {
        if self.automation.is_null() {
            return CaretProbeResult::Missing;
        }

        // Chromium and Electron frequently put keyboard focus on a descendant
        // text node while TextPattern/TextPattern2 lives on the enclosing Edit
        // or Document element. Probe the focused node and walk upward instead
        // of requiring the exact focused element to expose the pattern.
        let Some(mut element) = self.focused_element_for_targets(targets) else {
            return CaretProbeResult::Missing;
        };
        let mut editable_bounds_fallback = None;

        let initial_evidence = inspect_element(element);
        if evidence_accepts_caret_probe(initial_evidence, console_like, exact_geometry_only) {
            match probe_caret_anchor_from_uia_element(element, exact_geometry_only) {
                CaretProbeResult::Found(point) => {
                    release_com(element);
                    return CaretProbeResult::Found(point);
                }
                CaretProbeResult::Suppress => {
                    release_com(element);
                    return CaretProbeResult::Suppress;
                }
                CaretProbeResult::Missing => {}
            }
        }

        if !self.raw_view_walker.is_null() {
            let mut remaining = MAX_UIA_CARET_DESCENDANT_NODES;
            let descendant = if console_like {
                descendant_caret_anchor(
                    self.raw_view_walker,
                    element,
                    MAX_UIA_CARET_DESCENDANT_DEPTH,
                    &mut remaining,
                    exact_geometry_only,
                )
            } else {
                descendant_editable_caret_anchor(
                    self.raw_view_walker,
                    element,
                    MAX_UIA_CARET_DESCENDANT_DEPTH,
                    &mut remaining,
                    exact_geometry_only,
                )
            };
            match descendant {
                CaretProbeResult::Found(point) => {
                    release_com(element);
                    return CaretProbeResult::Found(point);
                }
                CaretProbeResult::Suppress => {
                    release_com(element);
                    return CaretProbeResult::Suppress;
                }
                CaretProbeResult::Missing => {}
            }
        }

        for _ in 0..MAX_UIA_CARET_PARENT_DEPTH {
            let evidence = inspect_element(element);
            if evidence_accepts_caret_probe(evidence, console_like, exact_geometry_only) {
                match probe_caret_anchor_from_uia_element(element, exact_geometry_only) {
                    CaretProbeResult::Found(point) => {
                        release_com(element);
                        return CaretProbeResult::Found(point);
                    }
                    CaretProbeResult::Suppress => {
                        release_com(element);
                        return CaretProbeResult::Suppress;
                    }
                    CaretProbeResult::Missing => {}
                }
            }

            if evidence_accepts_caret_probe(evidence, false, exact_geometry_only) {
                // Chromium may put the TextPattern provider on a child node of
                // the focused search box rather than on the focused element or
                // one of its ancestors. YouTube's search field is a common
                // example. Search a small, bounded descendant subtree before
                // using the element rectangle only for a confirmed empty field.
                if !self.raw_view_walker.is_null() {
                    let mut remaining = MAX_UIA_CARET_DESCENDANT_NODES;
                    let descendant = if exact_geometry_only {
                        descendant_editable_caret_anchor(
                            self.raw_view_walker,
                            element,
                            MAX_UIA_CARET_DESCENDANT_DEPTH,
                            &mut remaining,
                            true,
                        )
                    } else {
                        descendant_caret_anchor(
                            self.raw_view_walker,
                            element,
                            MAX_UIA_CARET_DESCENDANT_DEPTH,
                            &mut remaining,
                            false,
                        )
                    };
                    match descendant {
                        CaretProbeResult::Found(point) => {
                            release_com(element);
                            return CaretProbeResult::Found(point);
                        }
                        CaretProbeResult::Suppress => {
                            release_com(element);
                            return CaretProbeResult::Suppress;
                        }
                        CaretProbeResult::Missing => {}
                    }
                }

                if editable_bounds_fallback.is_none() {
                    editable_bounds_fallback = empty_editable_element_caret_anchor(element);
                }
            }

            if self.raw_view_walker.is_null() {
                break;
            }
            let Some(parent) = tree_walker_parent(self.raw_view_walker, element) else {
                break;
            };
            release_com(element);
            element = parent;
        }

        release_com(element);
        editable_bounds_fallback
            .map(CaretProbeResult::Found)
            .unwrap_or(CaretProbeResult::Missing)
    }

    unsafe fn classify_element_chain(&self, mut element: *mut c_void) -> Editability {
        let mut saw_read_only = false;
        let mut saw_selectable_text = false;
        let mut saw_code_like_text = false;
        for _ in 0..MAX_UIA_PARENT_DEPTH {
            match inspect_element(element) {
                NodeEvidence::EditableField if !saw_code_like_text => {
                    release_com(element);
                    return Editability::Editable;
                }
                NodeEvidence::EditableField => {}
                NodeEvidence::EditableDocument if !saw_code_like_text => {
                    release_com(element);
                    return Editability::Editable;
                }
                NodeEvidence::EditableDocument => {}
                NodeEvidence::ReadOnly => saw_read_only = true,
                NodeEvidence::CodeLikeText => {
                    saw_selectable_text = true;
                    saw_code_like_text = true;
                }
                NodeEvidence::SelectableText => saw_selectable_text = true,
                NodeEvidence::Unknown => {}
            }

            if self.raw_view_walker.is_null() {
                break;
            }
            let Some(parent) = tree_walker_parent(self.raw_view_walker, element) else {
                break;
            };
            release_com(element);
            element = parent;
        }

        release_com(element);
        if saw_read_only || saw_selectable_text {
            Editability::ReadOnly
        } else {
            Editability::Unknown
        }
    }

    unsafe fn detach_console(&mut self) {
        if self.attached_console_pid != 0 {
            FreeConsole();
            self.attached_console_pid = 0;
        }
    }

    unsafe fn ensure_console_attachment(&mut self, process_id: u32, window: HWND) -> bool {
        if process_id == 0 || window.is_null() {
            return false;
        }

        if self.attached_console_pid != 0 && GetConsoleWindow() == window {
            return true;
        }

        self.detach_console();
        if self.try_attach_console_process(process_id, window) {
            return true;
        }

        // ConsoleWindowClass is owned by conhost/OpenConsole rather than by
        // the cmd.exe process whose screen buffer we need. Enumerate console
        // clients in the same session and keep the one whose console HWND is
        // the foreground window. The successful PID is cached, so this scan is
        // needed only when the active classic console changes.
        for candidate in console_client_candidates(process_id) {
            if candidate != process_id && self.try_attach_console_process(candidate, window) {
                return true;
            }
        }
        false
    }

    unsafe fn try_attach_console_process(&mut self, process_id: u32, window: HWND) -> bool {
        if process_id == 0 || AttachConsole(process_id) == FALSE {
            return false;
        }

        if GetConsoleWindow() == window {
            self.attached_console_pid = process_id;
            return true;
        }

        FreeConsole();
        false
    }

    unsafe fn classic_console_caret_anchor(
        &mut self,
        window: HWND,
        process_id: u32,
        composition_windows: [HWND; 3],
        default_cell_span: i32,
    ) -> Option<CaretAnchor> {
        if !self.ensure_console_attachment(process_id, window) {
            return None;
        }

        const CONOUT_DEVICE: [u16; 8] = [
            b'C' as u16,
            b'O' as u16,
            b'N' as u16,
            b'O' as u16,
            b'U' as u16,
            b'T' as u16,
            b'$' as u16,
            0,
        ];
        let output = CreateFileW(
            CONOUT_DEVICE.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        );
        if output.is_null() || output == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut info: CONSOLE_SCREEN_BUFFER_INFO = zeroed();
        if GetConsoleScreenBufferInfo(output, &mut info) == FALSE {
            CloseHandle(output);
            return None;
        }

        // During an active IME composition conhost can hide its ordinary
        // screen-buffer cursor while keeping dwCursorPosition at the base
        // insertion cell. The position remains usable; rejecting a hidden
        // cursor would fall through to the stale GUI-thread caret at the left
        // edge of the composition string.

        let column = i32::from(info.dwCursorPosition.X) - i32::from(info.srWindow.Left);
        let row = i32::from(info.dwCursorPosition.Y) - i32::from(info.srWindow.Top);
        let columns = i32::from(info.srWindow.Right) - i32::from(info.srWindow.Left) + 1;
        let rows = i32::from(info.srWindow.Bottom) - i32::from(info.srWindow.Top) + 1;
        if column < 0 || row < 0 || column >= columns || row >= rows {
            CloseHandle(output);
            return None;
        }

        let mut client_rect = RECT::default();
        let mut origin = POINT::default();
        if GetClientRect(window, &mut client_rect) == FALSE
            || ClientToScreen(window, &mut origin) == FALSE
        {
            CloseHandle(output);
            return None;
        }

        let mut font: CONSOLE_FONT_INFOEX = zeroed();
        font.cbSize = size_of::<CONSOLE_FONT_INFOEX>() as u32;
        let font_ok = GetCurrentConsoleFontEx(output, FALSE, &mut font) != FALSE;
        CloseHandle(output);

        let client_width = client_rect.right.saturating_sub(client_rect.left).max(1);
        let client_height = client_rect.bottom.saturating_sub(client_rect.top).max(1);
        let fallback_width = (client_width / columns.max(1)).max(1);
        let fallback_height = (client_height / rows.max(1)).max(1);
        let cell_width = if font_ok {
            i32::from(font.dwFontSize.X).max(1)
        } else {
            fallback_width
        };
        let cell_height = if font_ok {
            i32::from(font.dwFontSize.Y).max(1)
        } else {
            fallback_height
        };

        let measured_span = ime_composition_display_columns(&composition_windows);
        let visual_cell_span = measured_span
            .filter(|columns| *columns > 0)
            .unwrap_or(default_cell_span)
            .clamp(1, 8);
        Some(console_cell_anchor(
            origin,
            column,
            row,
            cell_width,
            cell_height,
            visual_cell_span,
        ))
    }
}

unsafe fn console_client_candidates(preferred_process_id: u32) -> Vec<u32> {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return Vec::new();
    }

    let mut preferred_session = 0;
    let filter_session =
        ProcessIdToSessionId(preferred_process_id, &mut preferred_session) != FALSE;
    let current_process_id = GetCurrentProcessId();
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut candidates = Vec::<(u8, u32)>::new();

    if Process32FirstW(snapshot, &mut entry) != FALSE {
        loop {
            let process_id = entry.th32ProcessID;
            if process_id != 0
                && process_id != current_process_id
                && process_id != preferred_process_id
            {
                let mut session_id = 0;
                let same_session = !filter_session
                    || (ProcessIdToSessionId(process_id, &mut session_id) != FALSE
                        && session_id == preferred_session);
                if same_session {
                    candidates.push((console_process_priority(&entry.szExeFile), process_id));
                }
            }

            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            if Process32NextW(snapshot, &mut entry) == FALSE {
                break;
            }
        }
    }
    CloseHandle(snapshot);

    candidates.sort_unstable_by_key(|candidate| *candidate);
    candidates.dedup_by_key(|candidate| candidate.1);
    candidates
        .into_iter()
        .map(|candidate| candidate.1)
        .collect()
}

fn console_process_priority(executable_name: &[u16; 260]) -> u8 {
    let length = executable_name
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(executable_name.len());
    let name = String::from_utf16_lossy(&executable_name[..length]).to_ascii_lowercase();
    match name.as_str() {
        "cmd.exe" => 0,
        "powershell.exe" | "pwsh.exe" | "wsl.exe" | "bash.exe" => 1,
        "conhost.exe" | "openconsole.exe" => 3,
        _ => 2,
    }
}

impl Drop for EditabilityDetector {
    fn drop(&mut self) {
        unsafe {
            self.detach_console();
            self.clear_focused_element_cache();
            if !self.raw_view_walker.is_null() {
                release_com(self.raw_view_walker);
                self.raw_view_walker = null_mut();
            }
            if !self.automation.is_null() {
                release_com(self.automation);
                self.automation = null_mut();
            }
            if self.co_initialized {
                CoUninitialize();
                self.co_initialized = false;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ForegroundFocusTargets {
    foreground: HWND,
    focus: HWND,
    caret: HWND,
    caret_rect: RECT,
    process_id: u32,
}

unsafe fn accessible_system_caret_anchor(window: HWND) -> Option<CaretAnchor> {
    let accessible = accessible_system_caret_object(window)?;
    let anchor = accessible_caret_anchor_from_object(accessible);
    release_com(accessible);
    anchor
}

unsafe fn accessible_system_caret_object(window: HWND) -> Option<*mut c_void> {
    if window.is_null() || IsWindow(window) == FALSE {
        return None;
    }

    let mut accessible = null_mut();
    if AccessibleObjectFromWindow(window, OBJID_CARET, &IID_IACCESSIBLE, &mut accessible) < 0
        || accessible.is_null()
    {
        return None;
    }

    Some(accessible)
}

unsafe fn accessible_caret_anchor_from_object(accessible: *mut c_void) -> Option<CaretAnchor> {
    let mut child: VARIANT = zeroed();
    child.vt = VT_I4;
    child.data.l_val = CHILDID_SELF;
    accessible_caret_anchor_from_object_child(accessible, child)
}

unsafe fn accessible_caret_anchor_from_object_child(
    accessible: *mut c_void,
    child: VARIANT,
) -> Option<CaretAnchor> {
    if accessible.is_null() {
        return None;
    }
    type AccLocation = unsafe extern "system" fn(
        *mut c_void,
        *mut LONG,
        *mut LONG,
        *mut LONG,
        *mut LONG,
        VARIANT,
    ) -> HRESULT;
    let Some(address) = com_method_address(accessible, 22) else {
        return None;
    };
    let method: AccLocation = transmute(address);

    let mut left = 0;
    let mut top = 0;
    let mut width = 0;
    let mut height = 0;
    let result = method(
        accessible,
        &mut left,
        &mut top,
        &mut width,
        &mut height,
        child,
    );
    if result < 0 {
        return None;
    }

    accessible_caret_rect_anchor(left, top, width, height)
}

fn accessible_caret_rect_anchor(
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) -> Option<CaretAnchor> {
    if !(0..=MAX_ACCESSIBLE_CARET_WIDTH).contains(&width)
        || !(1..=MAX_ACCESSIBLE_CARET_HEIGHT).contains(&height)
    {
        return None;
    }
    Some(CaretAnchor {
        x: left.saturating_add(width.max(1)),
        top,
        bottom: top.saturating_add(height).max(top.saturating_add(1)),
    })
}

unsafe fn anchor_matches_window(anchor: CaretAnchor, window: HWND) -> bool {
    let mut rect = RECT::default();
    if GetWindowRect(window, &mut rect) == FALSE {
        return false;
    }
    const TOLERANCE: i32 = 16;
    anchor.x >= rect.left.saturating_sub(TOLERANCE)
        && anchor.x <= rect.right.saturating_add(TOLERANCE)
        && anchor.bottom >= rect.top.saturating_sub(TOLERANCE)
        && anchor.top <= rect.bottom.saturating_add(TOLERANCE)
}

unsafe fn foreground_focus_targets() -> ForegroundFocusTargets {
    let foreground = GetForegroundWindow();
    if foreground.is_null() {
        return ForegroundFocusTargets {
            foreground: null_mut(),
            focus: null_mut(),
            caret: null_mut(),
            caret_rect: RECT::default(),
            process_id: 0,
        };
    }

    let mut process_id = 0u32;
    let thread_id = GetWindowThreadProcessId(foreground, &mut process_id);
    if thread_id == 0 {
        return ForegroundFocusTargets {
            foreground,
            focus: null_mut(),
            caret: null_mut(),
            caret_rect: RECT::default(),
            process_id,
        };
    }

    let mut info: GUITHREADINFO = zeroed();
    info.cbSize = size_of::<GUITHREADINFO>() as u32;
    if GetGUIThreadInfo(thread_id, &mut info) == FALSE {
        return ForegroundFocusTargets {
            foreground,
            focus: null_mut(),
            caret: null_mut(),
            caret_rect: RECT::default(),
            process_id,
        };
    }

    ForegroundFocusTargets {
        foreground,
        focus: info.hwndFocus,
        caret: info.hwndCaret,
        caret_rect: info.rcCaret,
        process_id,
    }
}

fn console_cell_anchor(
    origin: POINT,
    column: i32,
    row: i32,
    cell_width: i32,
    cell_height: i32,
    visual_cell_span: i32,
) -> CaretAnchor {
    let top = origin.y.saturating_add(row.saturating_mul(cell_height));
    CaretAnchor {
        x: origin
            .x
            .saturating_add((column + visual_cell_span.clamp(1, 8)).saturating_mul(cell_width)),
        top,
        bottom: top.saturating_add(cell_height),
    }
}

unsafe fn ime_composition_display_columns(windows: &[HWND]) -> Option<i32> {
    let mut seen = Vec::<usize>::with_capacity(windows.len() * 2);
    let mut best = 0;

    for &window in windows {
        if window.is_null() || IsWindow(window) == FALSE {
            continue;
        }
        for candidate in [window, ImmGetDefaultIMEWnd(window)] {
            if candidate.is_null() || IsWindow(candidate) == FALSE {
                continue;
            }
            let key = candidate as usize;
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            if let Some(columns) = ime_composition_columns_for_window(candidate) {
                best = best.max(columns);
            }
        }
    }

    (best > 0).then_some(best)
}

unsafe fn ime_composition_columns_for_window(window: HWND) -> Option<i32> {
    let context = ImmGetContext(window);
    if context.is_null() {
        return None;
    }

    let byte_length = ImmGetCompositionStringW(context, GCS_COMPSTR, null_mut(), 0);
    if byte_length <= 0 {
        ImmReleaseContext(window, context);
        return None;
    }

    let mut text = vec![0u16; ((byte_length as usize) + 1) / 2];
    let copied = ImmGetCompositionStringW(
        context,
        GCS_COMPSTR,
        text.as_mut_ptr() as *mut c_void,
        (text.len() * size_of::<u16>()) as DWORD,
    );
    let cursor_position = ImmGetCompositionStringW(context, GCS_CURSORPOS, null_mut(), 0);
    ImmReleaseContext(window, context);

    if copied <= 0 {
        return None;
    }
    let copied_units = ((copied as usize) / size_of::<u16>()).min(text.len());
    text.truncate(copied_units);
    if text.is_empty() {
        return None;
    }

    // Some Korean IMEs report cursor position zero while the current syllable
    // is still being composed. In that case the visible composition itself is
    // the best available distance from the console buffer cursor.
    let units_before_cursor = if cursor_position > 0 {
        (cursor_position as usize).min(text.len())
    } else {
        text.len()
    };
    Some(utf16_console_columns(&text[..units_before_cursor]))
}

fn utf16_console_columns(text: &[u16]) -> i32 {
    std::char::decode_utf16(text.iter().copied())
        .map(|value| value.unwrap_or('\u{fffd}'))
        .map(console_character_columns)
        .sum()
}

fn console_character_columns(character: char) -> i32 {
    let value = character as u32;
    if matches!(
        value,
        0x1100..=0x11ff
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7af
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe6f
            | 0xff01..=0xff60
            | 0xffe0..=0xffe6
            | 0x1f300..=0x1faff
    ) {
        2
    } else {
        1
    }
}

unsafe fn win32_caret_anchor(targets: ForegroundFocusTargets) -> Option<CaretAnchor> {
    if targets.caret.is_null() {
        return None;
    }

    let rect = targets.caret_rect;
    let caret_width = rect.right.saturating_sub(rect.left).clamp(1, 8);
    let mut client_origin = POINT { x: 0, y: 0 };
    if ClientToScreen(targets.caret, &mut client_origin) == FALSE {
        return None;
    }

    // GetGUIThreadInfo returns rcCaret in the caret window's own logical
    // coordinate space. ClientToScreen is evaluated in this process's PMv2
    // coordinate space, so passing rcCaret to it directly makes the horizontal
    // error grow with every character when a system-DPI-aware window moves to
    // a monitor with a different scale. Convert client offsets explicitly.
    let reported_dpi = GetDpiForWindow(targets.caret);
    let logical_dpi = if reported_dpi == 0 { 96 } else { reported_dpi };
    let physical_dpi = monitor_dpi_for_window(targets.caret).unwrap_or(logical_dpi);
    let right = rect.left.saturating_add(caret_width);
    let bottom = rect.bottom.max(rect.top.saturating_add(1));
    let top_right = POINT {
        x: client_origin
            .x
            .saturating_add(scale_between_dpi(right, logical_dpi, physical_dpi)),
        y: client_origin
            .y
            .saturating_add(scale_between_dpi(rect.top, logical_dpi, physical_dpi)),
    };
    let bottom_right = POINT {
        x: top_right.x,
        y: client_origin
            .y
            .saturating_add(scale_between_dpi(bottom, logical_dpi, physical_dpi)),
    };
    Some(CaretAnchor {
        x: top_right.x,
        top: top_right.y,
        bottom: bottom_right.y.max(top_right.y.saturating_add(1)),
    })
}

unsafe fn monitor_dpi_for_window(window: HWND) -> Option<u32> {
    let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
    if monitor.is_null() {
        return None;
    }

    let mut dpi_x = 0;
    let mut dpi_y = 0;
    if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == S_OK && dpi_x != 0 {
        Some(dpi_x)
    } else {
        None
    }
}

fn scale_between_dpi(value: i32, from_dpi: u32, to_dpi: u32) -> i32 {
    if from_dpi == 0 || from_dpi == to_dpi {
        return value;
    }

    let numerator = i64::from(value) * i64::from(to_dpi);
    let half = i64::from(from_dpi) / 2;
    let rounded = if numerator >= 0 {
        numerator.saturating_add(half)
    } else {
        numerator.saturating_sub(half)
    };
    (rounded / i64::from(from_dpi)).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

unsafe fn is_console_like_window(window: HWND) -> bool {
    let Some(class_name) = window_class_name(window) else {
        return false;
    };
    is_console_like_class(&class_name)
}

unsafe fn is_classic_console_window(window: HWND) -> bool {
    window_class_name(window)
        .is_some_and(|class_name| class_name.eq_ignore_ascii_case("ConsoleWindowClass"))
}

fn is_console_like_class(class_name: &str) -> bool {
    matches!(
        class_name.to_ascii_lowercase().as_str(),
        "consolewindowclass" | "cascadia_hosting_window_class" | "virtualconsoleclass"
    )
}

fn is_uia_preferred_caret_class(class_name: &str) -> bool {
    let normalized = class_name.to_ascii_lowercase();
    normalized.starts_with("chrome_widgetwin_") || normalized == "mozillawindowclass"
}

fn is_chromium_widget_class(class_name: &str) -> bool {
    class_name
        .to_ascii_lowercase()
        .starts_with("chrome_widgetwin_")
}

fn is_office_uia_preferred_caret_class(class_name: &str) -> bool {
    class_name.eq_ignore_ascii_case("PPTFrameClass")
}

unsafe fn excel_editor_window(targets: ForegroundFocusTargets) -> Option<HWND> {
    if !is_excel_host_window(targets.foreground) {
        return None;
    }

    for start in [targets.caret, targets.focus] {
        let mut window = start;
        for _ in 0..=4 {
            if window.is_null() {
                break;
            }
            if IsWindowEnabled(window) != FALSE
                && window_class_name(window)
                    .is_some_and(|class_name| is_excel_editor_class(&class_name))
            {
                return Some(window);
            }
            let parent = GetParent(window);
            if parent.is_null() || parent == window {
                break;
            }
            window = parent;
        }
    }
    None
}

unsafe fn is_excel_host_window(window: HWND) -> bool {
    if window.is_null() {
        return false;
    }

    let root = GetAncestor(window, GA_ROOT);
    let root_owner = GetAncestor(window, GA_ROOTOWNER);
    [window, root, root_owner].into_iter().any(|candidate| {
        !candidate.is_null()
            && window_class_name(candidate)
                .is_some_and(|class_name| is_excel_host_class(&class_name))
    })
}

fn is_excel_host_class(class_name: &str) -> bool {
    let normalized = class_name.to_ascii_lowercase();
    normalized == "xlmain" || normalized.starts_with("bosa_sdm_xl")
}

fn is_excel_editor_class(class_name: &str) -> bool {
    matches!(
        class_name.to_ascii_lowercase().as_str(),
        "excel6" | "excel<" | "edtbx"
    )
}

#[derive(Default)]
struct ExcelDialogCaretSearch {
    anchor: Option<CaretAnchor>,
    fallback: Option<CaretAnchor>,
}

unsafe fn excel_dialog_caret_anchor(targets: ForegroundFocusTargets) -> Option<CaretAnchor> {
    let dialog = excel_dialog_root_for_targets(targets)?;
    let mut search = ExcelDialogCaretSearch::default();
    EnumChildWindows(
        dialog,
        Some(enum_excel_dialog_caret),
        &mut search as *mut ExcelDialogCaretSearch as LPARAM,
    );
    search.anchor.or(search.fallback)
}

unsafe fn excel_dialog_root_for_targets(targets: ForegroundFocusTargets) -> Option<HWND> {
    [targets.focus, targets.caret, targets.foreground]
        .into_iter()
        .find_map(|window| excel_dialog_root(window))
        .or_else(|| excel_dialog_for_process(targets.process_id))
}

unsafe fn excel_dialog_root(window: HWND) -> Option<HWND> {
    if window.is_null() {
        return None;
    }
    let root = GetAncestor(window, GA_ROOT);
    let root_owner = GetAncestor(window, GA_ROOTOWNER);
    [window, root, root_owner].into_iter().find(|candidate| {
        !candidate.is_null()
            && window_class_name(*candidate)
                .is_some_and(|class_name| is_excel_dialog_class(&class_name))
    })
}

fn is_excel_dialog_class(class_name: &str) -> bool {
    class_name
        .to_ascii_lowercase()
        .starts_with("bosa_sdm_xl")
}

#[derive(Default)]
struct ExcelDialogWindowSearch {
    process_id: u32,
    dialog: HWND,
}

unsafe fn excel_dialog_for_process(process_id: u32) -> Option<HWND> {
    if process_id == 0 {
        return None;
    }
    let mut search = ExcelDialogWindowSearch {
        process_id,
        dialog: null_mut(),
    };
    EnumWindows(
        Some(enum_excel_dialog_window),
        &mut search as *mut ExcelDialogWindowSearch as LPARAM,
    );
    (!search.dialog.is_null()).then_some(search.dialog)
}

unsafe extern "system" fn enum_excel_dialog_window(window: HWND, parameter: LPARAM) -> BOOL {
    if parameter == 0
        || IsWindowEnabled(window) == FALSE
        || IsWindowVisible(window) == FALSE
        || !window_class_name(window)
            .is_some_and(|class_name| is_excel_dialog_class(&class_name))
    {
        return TRUE;
    }

    let search = &mut *(parameter as *mut ExcelDialogWindowSearch);
    let mut process_id = 0;
    GetWindowThreadProcessId(window, &mut process_id);
    if process_id != search.process_id {
        return TRUE;
    }

    search.dialog = window;
    FALSE
}

unsafe fn excel_dialog_editor_bounds_anchor(
    targets: ForegroundFocusTargets,
    editor: HWND,
) -> Option<CaretAnchor> {
    excel_dialog_root_for_targets(targets)?;
    if !window_class_name(editor).is_some_and(|class_name| {
        class_name.eq_ignore_ascii_case("EDTBX") || class_name.eq_ignore_ascii_case("Edit")
    }) {
        return None;
    }

    excel_editor_bounds_anchor(editor)
}

unsafe fn excel_editor_bounds_anchor(editor: HWND) -> Option<CaretAnchor> {
    let mut rect = RECT::default();
    if GetWindowRect(editor, &mut rect) == FALSE
        || rect.right <= rect.left
        || rect.bottom <= rect.top
    {
        return None;
    }
    let vertical_inset = ((rect.bottom - rect.top) / 6).clamp(2, 6);
    Some(CaretAnchor {
        x: rect.left.saturating_add(4),
        top: rect.top.saturating_add(vertical_inset),
        bottom: rect.bottom.saturating_sub(vertical_inset),
    })
}

unsafe extern "system" fn enum_excel_dialog_caret(window: HWND, parameter: LPARAM) -> BOOL {
    if parameter == 0 || IsWindowEnabled(window) == FALSE {
        return TRUE;
    }
    let Some(class_name) = window_class_name(window) else {
        return TRUE;
    };
    if !class_name.eq_ignore_ascii_case("EDTBX")
        && !class_name.eq_ignore_ascii_case("Edit")
    {
        return TRUE;
    }

    let search = &mut *(parameter as *mut ExcelDialogCaretSearch);
    if search.fallback.is_none() {
        search.fallback = excel_editor_bounds_anchor(window);
    }

    if let Some(anchor) = accessible_system_caret_anchor(window)
        .filter(|anchor| anchor_matches_window(*anchor, window))
    {
        search.anchor = Some(anchor);
        FALSE
    } else {
        TRUE
    }
}

unsafe fn office_accessible_caret_anchor(
    targets: ForegroundFocusTargets,
) -> Option<CaretAnchor> {
    let mut last_window = null_mut();
    for window in [targets.focus, targets.caret] {
        if window.is_null() || window == last_window {
            continue;
        }
        last_window = window;
        if let Some(anchor) = accessible_system_caret_anchor(window)
            .filter(|anchor| anchor_matches_window(*anchor, window))
        {
            return Some(anchor);
        }
    }
    None
}

unsafe fn is_office_word_editor_window(window: HWND) -> bool {
    window_class_name(window).is_some_and(|class_name| is_office_word_editor_class(&class_name))
}

unsafe fn outlook_word_host(window: HWND) -> Option<HWND> {
    if !is_office_word_editor_window(window) {
        return None;
    }
    let document = GetParent(window);
    if document.is_null()
        || !window_class_name(document)
            .is_some_and(|class_name| class_name.eq_ignore_ascii_case("_WwB"))
    {
        return None;
    }

    let mut host = GetParent(document);
    for _ in 0..MAX_OFFICE_WORD_HOST_PARENT_DEPTH {
        if host.is_null() {
            return None;
        }
        if window_class_name(host)
            .is_some_and(|class_name| class_name.eq_ignore_ascii_case("rctrl_renwnd32"))
        {
            return Some(host);
        }

        let parent = GetParent(host);
        if parent.is_null() || parent == host {
            return None;
        }
        host = parent;
    }
    None
}

fn is_office_word_editor_class(class_name: &str) -> bool {
    class_name.eq_ignore_ascii_case("_WwG")
}

unsafe fn classify_standard_window(window: HWND) -> Option<Editability> {
    if IsWindowEnabled(window) == FALSE {
        return Some(Editability::ReadOnly);
    }

    let class_name = window_class_name(window)?;
    let normalized = class_name.to_ascii_lowercase();

    if normalized == "edit" || normalized.starts_with("richedit") {
        let style = get_window_long_ptr(window, GWL_STYLE) as u32;
        return Some(if style & ES_READONLY != 0 {
            Editability::ReadOnly
        } else {
            Editability::Editable
        });
    }

    if normalized == "scintilla" {
        let mut read_only = 0usize;
        let sent = SendMessageTimeoutW(
            window,
            SCI_GETREADONLY,
            0,
            0,
            SMTO_BLOCK | SMTO_ABORTIFHUNG,
            CONTROL_MESSAGE_TIMEOUT_MS,
            &mut read_only,
        );
        if sent != 0 {
            return Some(if read_only != 0 {
                Editability::ReadOnly
            } else {
                Editability::Editable
            });
        }
    }

    if normalized == "static" {
        return Some(Editability::ReadOnly);
    }

    None
}

unsafe fn window_class_name(window: HWND) -> Option<String> {
    let mut buffer = [0u16; CLASS_NAME_CAPACITY];
    let length = GetClassNameW(window, buffer.as_mut_ptr(), buffer.len() as i32);
    if length <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

unsafe fn inspect_element(element: *mut c_void) -> NodeEvidence {
    if property_bool(element, UIA_IS_ENABLED_PROPERTY_ID) == Some(false) {
        return NodeEvidence::ReadOnly;
    }

    let control_type = property_i32(element, UIA_CONTROL_TYPE_PROPERTY_ID);
    let has_keyboard_focus = property_bool(element, UIA_HAS_KEYBOARD_FOCUS_PROPERTY_ID);
    let keyboard_focusable = property_bool(element, UIA_IS_KEYBOARD_FOCUSABLE_PROPERTY_ID);
    let legacy_state = property_i32(element, UIA_LEGACY_IACCESSIBLE_STATE_PROPERTY_ID).unwrap_or(0);
    let aria_role = property_string(element, UIA_ARIA_ROLE_PROPERTY_ID)
        .map(|role| role.trim().to_ascii_lowercase());

    if legacy_state & (STATE_SYSTEM_UNAVAILABLE | STATE_SYSTEM_READONLY) != 0 {
        return NodeEvidence::ReadOnly;
    }

    let value_pattern =
        property_bool(element, UIA_IS_VALUE_PATTERN_AVAILABLE_PROPERTY_ID).unwrap_or(false);
    let value_is_read_only = value_pattern
        .then(|| property_bool(element, UIA_VALUE_IS_READ_ONLY_PROPERTY_ID))
        .flatten();
    if value_is_read_only == Some(true) {
        return NodeEvidence::ReadOnly;
    }

    let text_edit_pattern =
        property_bool(element, UIA_IS_TEXT_EDIT_PATTERN_AVAILABLE_PROPERTY_ID) == Some(true);

    // Chromium exposes inline <code> and similar selectable fragments with
    // inconsistent Value/TextEdit pattern metadata. ARIA role "code" is
    // explicit evidence that the element is display text, not an input field.
    if aria_role.as_deref().is_some_and(is_code_like_aria_role) {
        return NodeEvidence::CodeLikeText;
    }

    // Custom browser controls are accepted only when their semantic role says
    // that they are text-entry controls and they can receive keyboard focus.
    if aria_role.as_deref().is_some_and(is_text_entry_aria_role) {
        return if text_entry_role_accepts_focus(has_keyboard_focus, keyboard_focusable) {
            NodeEvidence::EditableField
        } else {
            NodeEvidence::ReadOnly
        };
    }

    match control_type {
        Some(UIA_EDIT_CONTROL_TYPE_ID) => {
            // Chromium sometimes omits IsKeyboardFocusable on focused HTML
            // inputs. Value.IsReadOnly=false or TextEditPattern is therefore
            // accepted as additional positive evidence for a real Edit node.
            if has_keyboard_focus == Some(true)
                || keyboard_focusable == Some(true)
                || value_is_read_only == Some(false)
                || text_edit_pattern
            {
                NodeEvidence::EditableField
            } else if keyboard_focusable == Some(false) {
                NodeEvidence::ReadOnly
            } else {
                NodeEvidence::Unknown
            }
        }
        Some(UIA_COMBO_BOX_CONTROL_TYPE_ID) => {
            // Search fields may be exposed as editable combo boxes when
            // autocomplete suggestions are active. Non-editable drop-downs do
            // not expose a writable Value or TextEdit pattern.
            if (has_keyboard_focus == Some(true) || keyboard_focusable == Some(true))
                && (value_is_read_only == Some(false) || text_edit_pattern)
            {
                NodeEvidence::EditableField
            } else {
                NodeEvidence::Unknown
            }
        }
        Some(UIA_DOCUMENT_CONTROL_TYPE_ID) => {
            // A normal browser/mail document is selectable text. Only a
            // focused or focusable Document with TextEditPattern is considered
            // a contenteditable surface.
            if text_edit_pattern
                && (has_keyboard_focus == Some(true) || keyboard_focusable == Some(true))
            {
                NodeEvidence::EditableDocument
            } else {
                NodeEvidence::SelectableText
            }
        }
        Some(UIA_TEXT_CONTROL_TYPE_ID) => NodeEvidence::SelectableText,
        _ => {
            // ValuePattern or TextEditPattern alone is weak evidence because
            // Chromium may expose those patterns on non-editable inline
            // content. Unknown custom controls therefore remain blocked unless
            // their ARIA role explicitly identifies a text-entry surface.
            NodeEvidence::Unknown
        }
    }
}

fn is_code_like_aria_role(role: &str) -> bool {
    role.split_ascii_whitespace()
        .any(|token| matches!(token, "code" | "doc-code" | "pre" | "presentation"))
}

fn is_text_entry_aria_role(role: &str) -> bool {
    role.split_ascii_whitespace()
        .any(|token| matches!(token, "textbox" | "searchbox" | "spinbutton" | "combobox"))
}

fn text_entry_role_accepts_focus(
    has_keyboard_focus: Option<bool>,
    keyboard_focusable: Option<bool>,
) -> bool {
    has_keyboard_focus == Some(true) || keyboard_focusable != Some(false)
}

unsafe fn get_focused_element(automation: *mut c_void) -> Option<*mut c_void> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
    let method: Method = transmute(com_method_address(automation, 8)?);
    let mut element = null_mut();
    let result = method(automation, &mut element);
    if result >= 0 && !element.is_null() {
        Some(element)
    } else {
        None
    }
}

unsafe fn get_raw_view_walker(automation: *mut c_void) -> Option<*mut c_void> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
    let method: Method = transmute(com_method_address(automation, 16)?);
    let mut walker = null_mut();
    let result = method(automation, &mut walker);
    if result >= 0 && !walker.is_null() {
        Some(walker)
    } else {
        None
    }
}

unsafe fn tree_walker_parent(walker: *mut c_void, element: *mut c_void) -> Option<*mut c_void> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT;
    let method: Method = transmute(com_method_address(walker, 3)?);
    let mut parent = null_mut();
    let result = method(walker, element, &mut parent);
    if result >= 0 && !parent.is_null() {
        Some(parent)
    } else {
        None
    }
}

unsafe fn tree_walker_first_child(
    walker: *mut c_void,
    element: *mut c_void,
) -> Option<*mut c_void> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT;
    let method: Method = transmute(com_method_address(walker, 4)?);
    let mut child = null_mut();
    let result = method(walker, element, &mut child);
    if result >= 0 && !child.is_null() {
        Some(child)
    } else {
        None
    }
}

unsafe fn tree_walker_next_sibling(
    walker: *mut c_void,
    element: *mut c_void,
) -> Option<*mut c_void> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT;
    let method: Method = transmute(com_method_address(walker, 6)?);
    let mut sibling = null_mut();
    let result = method(walker, element, &mut sibling);
    if result >= 0 && !sibling.is_null() {
        Some(sibling)
    } else {
        None
    }
}

unsafe fn current_pattern_as(
    element: *mut c_void,
    pattern_id: i32,
    interface_id: *const GUID,
) -> Option<*mut c_void> {
    type Method =
        unsafe extern "system" fn(*mut c_void, i32, *const GUID, *mut *mut c_void) -> HRESULT;
    let address = com_method_address(element, 14)?;
    let method: Method = transmute(address);
    let mut pattern = null_mut();
    let result = method(element, pattern_id, interface_id, &mut pattern);
    (result >= 0 && !pattern.is_null()).then_some(pattern)
}

unsafe fn probe_caret_anchor_from_uia_element(
    element: *mut c_void,
    force_exact_geometry: bool,
) -> CaretProbeResult {
    let exact_geometry_only =
        force_exact_geometry || element_requires_exact_caret_geometry(element);

    // TextEditPattern is the editing-specific provider. During an IME
    // composition its active composition range is more current than the
    // compatibility caret exposed by browser chrome. Collapse that range at
    // its trailing endpoint and derive the visual insertion edge from the
    // adjacent character.
    let mut selection_range = None;
    if let Some(pattern) = current_pattern_as(
        element,
        UIA_TEXT_EDIT_PATTERN_ID,
        &IID_IUIAUTOMATION_TEXT_EDIT_PATTERN,
    ) {
        if let Some(range) = text_edit_active_composition_range(pattern) {
            let point = caret_anchor_from_range_end(range);
            release_com(range);
            if let Some(point) = point.filter(|point| anchor_matches_element(*point, element)) {
                release_com(pattern);
                return CaretProbeResult::Found(point);
            }
        }
        selection_range = text_pattern_selection_range(pattern);
        release_com(pattern);
    }

    // Chromium can expose only TextPattern2 on browser chrome. Because
    // TextPattern2 inherits TextPattern, query GetSelection from that same
    // provider before requesting a separate TextPattern object. A separately
    // marshalled compatibility object can remain fixed at offset zero.
    let pattern2 = current_pattern_as(
        element,
        UIA_TEXT_PATTERN2_ID,
        &IID_IUIAUTOMATION_TEXT_PATTERN2,
    );
    if selection_range.is_none() {
        selection_range = pattern2.and_then(|pattern| text_pattern_selection_range(pattern));
    }
    if selection_range.is_none() {
        selection_range = current_pattern_as(
            element,
            UIA_TEXT_PATTERN_ID,
            &IID_IUIAUTOMATION_TEXT_PATTERN,
        )
        .and_then(|pattern| {
            let range = text_pattern_selection_range(pattern);
            release_com(pattern);
            range
        });
    }

    // A non-collapsed selection has no single insertion point. Browser address
    // bars commonly select the entire URL on focus, so suppress the marker
    // until typing or an explicit click collapses that selection.
    if let Some(range) = selection_range {
        if selection_is_noncollapsed(range) {
            release_com(range);
            if let Some(pattern) = pattern2 {
                release_com(pattern);
            }
            return CaretProbeResult::Suppress;
        }
    }

    if exact_geometry_only {
        // For browser chrome, the collapsed selection is the authoritative
        // insertion range. Its own rectangle may be empty, so derive the
        // position from the immediately adjacent character before considering
        // any compatibility caret returned by GetCaretRange.
        if let Some(range) = selection_range.take() {
            let point = caret_anchor_from_selection_range(range, true);
            release_com(range);
            if let Some(pattern) = pattern2 {
                release_com(pattern);
            }
            return point
                .filter(|point| anchor_matches_element(*point, element))
                .map(CaretProbeResult::Found)
                .unwrap_or(CaretProbeResult::Missing);
        }

        // With an existing value, a browser caret without a matching selection
        // range is not trustworthy: Chromium may expose a stale zero-offset
        // compatibility caret. Hiding is safer than placing the marker at the
        // beginning of the address bar.
        if property_string(element, UIA_VALUE_VALUE_PROPERTY_ID)
            .is_some_and(|value| !value.is_empty())
        {
            if let Some(pattern) = pattern2 {
                release_com(pattern);
            }
            return CaretProbeResult::Missing;
        }
    }

    // TextPattern2 remains the primary source for ordinary controls and empty
    // browser fields where there is no preceding character to measure.
    if let Some(pattern) = pattern2 {
        let range = text_pattern2_caret_range(pattern);
        release_com(pattern);
        if let Some(range) = range {
            let point = caret_anchor_from_text_range(range, exact_geometry_only);
            release_com(range);
            if let Some(point) = point.filter(|point| anchor_matches_element(*point, element)) {
                if let Some(selection_range) = selection_range {
                    release_com(selection_range);
                }
                return CaretProbeResult::Found(point);
            }
        }
    }

    // Providers that expose only TextPattern still provide a collapsed
    // selection after actual text entry begins.
    if let Some(range) = selection_range {
        let point = caret_anchor_from_selection_range(range, exact_geometry_only);
        release_com(range);
        return point
            .filter(|point| anchor_matches_element(*point, element))
            .map(CaretProbeResult::Found)
            .unwrap_or(CaretProbeResult::Missing);
    }

    CaretProbeResult::Missing
}

unsafe fn descendant_contains_matching_anchor(
    walker: *mut c_void,
    parent: *mut c_void,
    depth: usize,
    remaining: &mut usize,
    anchor: CaretAnchor,
    exact_geometry_only: bool,
) -> bool {
    if depth == 0 || *remaining == 0 {
        return false;
    }

    let Some(mut child) = tree_walker_first_child(walker, parent) else {
        return false;
    };
    loop {
        *remaining = (*remaining).saturating_sub(1);
        let evidence = inspect_element(child);
        if evidence_accepts_caret_probe(evidence, false, exact_geometry_only)
            && anchor_matches_element(anchor, child)
        {
            release_com(child);
            return true;
        }
        if descendant_contains_matching_anchor(
            walker,
            child,
            depth - 1,
            remaining,
            anchor,
            exact_geometry_only,
        ) {
            release_com(child);
            return true;
        }

        let next = tree_walker_next_sibling(walker, child);
        release_com(child);
        let Some(next) = next else {
            break;
        };
        child = next;
        if *remaining == 0 {
            release_com(child);
            break;
        }
    }
    false
}

unsafe fn descendant_contains_editable(
    walker: *mut c_void,
    parent: *mut c_void,
    depth: usize,
    remaining: &mut usize,
) -> bool {
    if depth == 0 || *remaining == 0 {
        return false;
    }

    let Some(mut child) = tree_walker_first_child(walker, parent) else {
        return false;
    };
    loop {
        *remaining = (*remaining).saturating_sub(1);
        if matches!(
            inspect_element(child),
            NodeEvidence::EditableField | NodeEvidence::EditableDocument
        ) {
            release_com(child);
            return true;
        }

        if *remaining > 0 && descendant_contains_editable(walker, child, depth - 1, remaining) {
            release_com(child);
            return true;
        }

        let next = tree_walker_next_sibling(walker, child);
        release_com(child);
        let Some(next) = next else {
            break;
        };
        child = next;
        if *remaining == 0 {
            release_com(child);
            break;
        }
    }
    false
}

unsafe fn descendant_editable_caret_anchor(
    walker: *mut c_void,
    parent: *mut c_void,
    depth: usize,
    remaining: &mut usize,
    exact_geometry_only: bool,
) -> CaretProbeResult {
    if depth == 0 || *remaining == 0 {
        return CaretProbeResult::Missing;
    }

    let Some(mut child) = tree_walker_first_child(walker, parent) else {
        return CaretProbeResult::Missing;
    };
    let mut fallback = None;
    loop {
        *remaining = (*remaining).saturating_sub(1);
        let editable =
            evidence_accepts_caret_probe(inspect_element(child), false, exact_geometry_only);

        if editable {
            match probe_caret_anchor_from_uia_element(child, exact_geometry_only) {
                CaretProbeResult::Found(point) => {
                    release_com(child);
                    return CaretProbeResult::Found(point);
                }
                CaretProbeResult::Suppress => {
                    release_com(child);
                    return CaretProbeResult::Suppress;
                }
                CaretProbeResult::Missing => {}
            }
            if *remaining > 0 {
                match descendant_caret_anchor(
                    walker,
                    child,
                    depth - 1,
                    remaining,
                    exact_geometry_only,
                ) {
                    CaretProbeResult::Found(point) => {
                        release_com(child);
                        return CaretProbeResult::Found(point);
                    }
                    CaretProbeResult::Suppress => {
                        release_com(child);
                        return CaretProbeResult::Suppress;
                    }
                    CaretProbeResult::Missing => {}
                }
            }
            if fallback.is_none() {
                fallback = empty_editable_element_caret_anchor(child);
            }
        } else if *remaining > 0 {
            match descendant_editable_caret_anchor(
                walker,
                child,
                depth - 1,
                remaining,
                exact_geometry_only,
            ) {
                CaretProbeResult::Found(point) => {
                    release_com(child);
                    return CaretProbeResult::Found(point);
                }
                CaretProbeResult::Suppress => {
                    release_com(child);
                    return CaretProbeResult::Suppress;
                }
                CaretProbeResult::Missing => {}
            }
        }

        let next = tree_walker_next_sibling(walker, child);
        release_com(child);
        let Some(next) = next else {
            break;
        };
        child = next;
        if *remaining == 0 {
            release_com(child);
            break;
        }
    }

    fallback
        .map(CaretProbeResult::Found)
        .unwrap_or(CaretProbeResult::Missing)
}

unsafe fn descendant_caret_anchor(
    walker: *mut c_void,
    parent: *mut c_void,
    depth: usize,
    remaining: &mut usize,
    exact_geometry_only: bool,
) -> CaretProbeResult {
    if depth == 0 || *remaining == 0 {
        return CaretProbeResult::Missing;
    }

    let Some(mut child) = tree_walker_first_child(walker, parent) else {
        return CaretProbeResult::Missing;
    };
    loop {
        *remaining = (*remaining).saturating_sub(1);

        match probe_caret_anchor_from_uia_element(child, exact_geometry_only) {
            CaretProbeResult::Found(point) => {
                release_com(child);
                return CaretProbeResult::Found(point);
            }
            CaretProbeResult::Suppress => {
                release_com(child);
                return CaretProbeResult::Suppress;
            }
            CaretProbeResult::Missing => {}
        }

        if *remaining > 0 {
            match descendant_caret_anchor(walker, child, depth - 1, remaining, exact_geometry_only)
            {
                CaretProbeResult::Found(point) => {
                    release_com(child);
                    return CaretProbeResult::Found(point);
                }
                CaretProbeResult::Suppress => {
                    release_com(child);
                    return CaretProbeResult::Suppress;
                }
                CaretProbeResult::Missing => {}
            }
        }

        let next = tree_walker_next_sibling(walker, child);
        release_com(child);
        let Some(next) = next else {
            break;
        };
        child = next;
        if *remaining == 0 {
            release_com(child);
            break;
        }
    }

    CaretProbeResult::Missing
}

unsafe fn text_edit_active_composition_range(pattern: *mut c_void) -> Option<*mut c_void> {
    // IUIAutomationTextEditPattern inherits all TextPattern methods. Its first
    // added method, GetActiveComposition, is vtable slot 9.
    type Method = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
    let address = com_method_address(pattern, 9)?;
    let method: Method = transmute(address);
    let mut range = null_mut();
    let result = method(pattern, &mut range);
    if result >= 0 && !range.is_null() {
        Some(range)
    } else {
        if !range.is_null() {
            release_com(range);
        }
        None
    }
}

unsafe fn text_pattern2_caret_range(pattern: *mut c_void) -> Option<*mut c_void> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut BOOL, *mut *mut c_void) -> HRESULT;
    let address = com_method_address(pattern, 10)?;
    let method: Method = transmute(address);
    let mut active = FALSE;
    let mut range = null_mut();
    let result = method(pattern, &mut active, &mut range);
    if result >= 0 && active != FALSE && !range.is_null() {
        Some(range)
    } else {
        if !range.is_null() {
            release_com(range);
        }
        None
    }
}

unsafe fn text_pattern_selection_range(pattern: *mut c_void) -> Option<*mut c_void> {
    // IUIAutomationTextPattern::GetSelection is vtable slot 5 after IUnknown,
    // RangeFromPoint and RangeFromChild.
    type Method = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
    let address = com_method_address(pattern, 5)?;
    let method: Method = transmute(address);
    let mut ranges = null_mut();
    if method(pattern, &mut ranges) < 0 || ranges.is_null() {
        return None;
    }

    let range = text_range_array_first(ranges);
    release_com(ranges);
    range
}

unsafe fn text_range_array_first(ranges: *mut c_void) -> Option<*mut c_void> {
    type LengthMethod = unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT;
    type ElementMethod = unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> HRESULT;

    let length_method: LengthMethod = transmute(com_method_address(ranges, 3)?);
    let mut length = 0;
    if length_method(ranges, &mut length) < 0 || length <= 0 {
        return None;
    }

    let element_method: ElementMethod = transmute(com_method_address(ranges, 4)?);
    let mut range = null_mut();
    let result = element_method(ranges, 0, &mut range);
    (result >= 0 && !range.is_null()).then_some(range)
}

unsafe fn selection_is_noncollapsed(range: *mut c_void) -> bool {
    text_range_compare_endpoints(
        range,
        TEXT_PATTERN_RANGE_ENDPOINT_START,
        range,
        TEXT_PATTERN_RANGE_ENDPOINT_END,
    )
    .is_some_and(|order| order != 0)
}

unsafe fn caret_anchor_from_range_end(range: *mut c_void) -> Option<CaretAnchor> {
    let caret = text_range_clone(range)?;
    let collapsed = text_range_move_endpoint_by_range(
        caret,
        TEXT_PATTERN_RANGE_ENDPOINT_START,
        range,
        TEXT_PATTERN_RANGE_ENDPOINT_END,
    );
    let point = if collapsed {
        caret_anchor_from_adjacent_character(caret)
            .or_else(|| caret_anchor_from_text_range(caret, true))
    } else {
        None
    };
    release_com(caret);
    point
}

unsafe fn caret_anchor_from_selection_range(
    range: *mut c_void,
    exact_geometry_only: bool,
) -> Option<CaretAnchor> {
    if selection_is_noncollapsed(range) {
        return None;
    }

    // In browser chrome the provider can attach a stale narrow rectangle to a
    // valid collapsed selection. Character-boundary geometry is tied to the
    // selection endpoint itself and therefore takes precedence.
    if exact_geometry_only {
        if let Some(point) = caret_anchor_from_adjacent_character(range) {
            return Some(point);
        }
    }
    caret_anchor_from_text_range(range, exact_geometry_only)
}

unsafe fn caret_anchor_from_text_range(
    range: *mut c_void,
    exact_geometry_only: bool,
) -> Option<CaretAnchor> {
    let direct = text_range_rectangles(range);
    if let Some(rect) = direct.first().copied() {
        // GetCaretRange is defined as a zero-length range. Accept only a
        // genuinely narrow caret rectangle. Chromium can occasionally return
        // the whole edit field or selected text span here; those rectangles
        // must be ignored so the adjacent-character path can recover the real
        // insertion point.
        if let Some(point) = direct_text_range_anchor(rect) {
            return Some(point);
        }
    }

    // Degenerate ranges commonly have no rectangle. Derive the insertion
    // point from the immediately preceding character first; its right edge is
    // exactly the caret location. At offset zero, use the following
    // character's left edge instead. This avoids guessing from the full edit
    // control bounds.
    if let Some(point) = caret_anchor_from_adjacent_character(range) {
        return Some(point);
    }

    if exact_geometry_only {
        return None;
    }

    let expanded = text_range_clone(range)?;
    let expanded_ok = text_range_expand_to_character(expanded);
    if !expanded_ok {
        release_com(expanded);
        return None;
    }

    let start_comparison = text_range_compare_endpoints(
        range,
        TEXT_PATTERN_RANGE_ENDPOINT_START,
        expanded,
        TEXT_PATTERN_RANGE_ENDPOINT_START,
    );
    let end_comparison = text_range_compare_endpoints(
        range,
        TEXT_PATTERN_RANGE_ENDPOINT_START,
        expanded,
        TEXT_PATTERN_RANGE_ENDPOINT_END,
    );
    let rects = text_range_rectangles(expanded);
    release_com(expanded);
    let rect = rects.first().copied()?;
    let use_right_edge =
        start_comparison.is_some_and(|value| value > 0) || end_comparison == Some(0);
    character_rect_anchor(rect, use_right_edge)
}

unsafe fn caret_anchor_from_adjacent_character(range: *mut c_void) -> Option<CaretAnchor> {
    if let Some(previous) = text_range_clone(range) {
        let moved = text_range_move_endpoint_by_unit(
            previous,
            TEXT_PATTERN_RANGE_ENDPOINT_START,
            TEXT_UNIT_CHARACTER,
            -1,
        );
        if moved.is_some_and(|count| count < 0) {
            let rects = text_range_rectangles(previous);
            release_com(previous);
            if let Some(rect) = rects.last().copied() {
                return character_rect_anchor(rect, true);
            }
        } else {
            release_com(previous);
        }
    }

    let next = text_range_clone(range)?;
    let moved = text_range_move_endpoint_by_unit(
        next,
        TEXT_PATTERN_RANGE_ENDPOINT_END,
        TEXT_UNIT_CHARACTER,
        1,
    );
    let rects = if moved.is_some_and(|count| count > 0) {
        text_range_rectangles(next)
    } else {
        Vec::new()
    };
    release_com(next);
    let rect = rects.first().copied()?;
    character_rect_anchor(rect, false)
}

unsafe fn text_range_move_endpoint_by_unit(
    range: *mut c_void,
    endpoint: i32,
    unit: i32,
    count: i32,
) -> Option<i32> {
    type Method = unsafe extern "system" fn(*mut c_void, i32, i32, i32, *mut i32) -> HRESULT;
    let address = com_method_address(range, 14)?;
    let method: Method = transmute(address);
    let mut moved = 0;
    let result = method(range, endpoint, unit, count, &mut moved);
    (result >= 0).then_some(moved)
}

unsafe fn text_range_move_endpoint_by_range(
    range: *mut c_void,
    endpoint: i32,
    target_range: *mut c_void,
    target_endpoint: i32,
) -> bool {
    type Method = unsafe extern "system" fn(*mut c_void, i32, *mut c_void, i32) -> HRESULT;
    let Some(address) = com_method_address(range, 15) else {
        return false;
    };
    let method: Method = transmute(address);
    method(range, endpoint, target_range, target_endpoint) >= 0
}

unsafe fn text_range_clone(range: *mut c_void) -> Option<*mut c_void> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
    let address = com_method_address(range, 3)?;
    let method: Method = transmute(address);
    let mut clone = null_mut();
    let result = method(range, &mut clone);
    (result >= 0 && !clone.is_null()).then_some(clone)
}

unsafe fn text_range_expand_to_character(range: *mut c_void) -> bool {
    type Method = unsafe extern "system" fn(*mut c_void, i32) -> HRESULT;
    let Some(address) = com_method_address(range, 6) else {
        return false;
    };
    let method: Method = transmute(address);
    method(range, TEXT_UNIT_CHARACTER) >= 0
}

unsafe fn text_range_compare_endpoints(
    source_range: *mut c_void,
    source_endpoint: i32,
    target_range: *mut c_void,
    target_endpoint: i32,
) -> Option<i32> {
    type Method =
        unsafe extern "system" fn(*mut c_void, i32, *mut c_void, i32, *mut i32) -> HRESULT;
    let address = com_method_address(source_range, 5)?;
    let method: Method = transmute(address);
    let mut comparison = 0;
    let result = method(
        source_range,
        source_endpoint,
        target_range,
        target_endpoint,
        &mut comparison,
    );
    (result >= 0).then_some(comparison)
}

unsafe fn text_range_rectangles(range: *mut c_void) -> Vec<[f64; 4]> {
    type Method = unsafe extern "system" fn(*mut c_void, *mut *mut SAFEARRAY) -> HRESULT;
    let Some(address) = com_method_address(range, 10) else {
        return Vec::new();
    };
    let method: Method = transmute(address);
    let mut array = null_mut();
    if method(range, &mut array) < 0 || array.is_null() {
        return Vec::new();
    }

    let values = safe_array_f64_values(array);
    values
        .chunks_exact(4)
        .filter_map(|chunk| {
            let rect = [chunk[0], chunk[1], chunk[2], chunk[3]];
            (rect.iter().all(|value| value.is_finite()) && rect[2] >= 0.0 && rect[3] >= 0.0)
                .then_some(rect)
        })
        .collect()
}

unsafe fn safe_array_f64_values(array: *mut SAFEARRAY) -> Vec<f64> {
    let values = safe_array_f64_values_borrowed(array);
    SafeArrayDestroy(array);
    values
}

unsafe fn safe_array_f64_values_borrowed(array: *mut SAFEARRAY) -> Vec<f64> {
    let mut values = Vec::new();
    if array.is_null() || SafeArrayGetDim(array) != 1 {
        return values;
    }

    let mut lower = 0;
    let mut upper = -1;
    if SafeArrayGetLBound(array, 1, &mut lower) < 0
        || SafeArrayGetUBound(array, 1, &mut upper) < 0
        || upper < lower
    {
        return values;
    }

    let count = (upper - lower + 1) as usize;
    let mut data = null_mut();
    if SafeArrayAccessData(array, &mut data) >= 0 {
        if !data.is_null() {
            values.extend_from_slice(std::slice::from_raw_parts(data as *const f64, count));
        }
        SafeArrayUnaccessData(array);
    }
    values
}

unsafe fn element_requires_exact_caret_geometry(element: *mut c_void) -> bool {
    let class_name = property_string(element, UIA_CLASS_NAME_PROPERTY_ID)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = property_string(element, UIA_NAME_PROPERTY_ID)
        .unwrap_or_default()
        .to_ascii_lowercase();
    class_name.contains("omnibox")
        || class_name.contains("addressbar")
        || name.contains("address and search")
        || name.contains("주소 및 검색")
}

unsafe fn anchor_matches_element(anchor: CaretAnchor, element: *mut c_void) -> bool {
    let Some(rect) = property_bounding_rect(element) else {
        return true;
    };
    if !rect.iter().all(|value| value.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
        return false;
    }
    let left = rect[0] - ELEMENT_ANCHOR_TOLERANCE;
    let right = rect[0] + rect[2] + ELEMENT_ANCHOR_TOLERANCE;
    let top = rect[1] - ELEMENT_ANCHOR_TOLERANCE;
    let bottom = rect[1] + rect[3] + ELEMENT_ANCHOR_TOLERANCE;
    let x = f64::from(anchor.x);
    let anchor_top = f64::from(anchor.top);
    let anchor_bottom = f64::from(anchor.bottom);
    x >= left && x <= right && anchor_bottom >= top && anchor_top <= bottom
}

fn character_rect_anchor(rect: [f64; 4], use_right_edge: bool) -> Option<CaretAnchor> {
    if !rect[2].is_finite() || rect[2] <= 0.0 || rect[2] > MAX_CHARACTER_RECT_WIDTH {
        return None;
    }
    rect_anchor(rect, use_right_edge)
}

fn direct_text_range_anchor(rect: [f64; 4]) -> Option<CaretAnchor> {
    if !rect[2].is_finite() || rect[2] > MAX_DIRECT_CARET_RECT_WIDTH {
        return None;
    }
    rect_anchor(rect, false)
}

fn rect_anchor(rect: [f64; 4], use_right_edge: bool) -> Option<CaretAnchor> {
    let x = if use_right_edge {
        rect[0] + rect[2]
    } else {
        rect[0]
    };
    let top = rect[1];
    let bottom = rect[1] + rect[3].max(1.0);
    if !x.is_finite() || !top.is_finite() || !bottom.is_finite() {
        return None;
    }
    let top = top.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    let bottom = bottom.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    Some(CaretAnchor {
        x: x.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        top,
        bottom: bottom.max(top.saturating_add(1)),
    })
}

unsafe fn empty_editable_element_caret_anchor(element: *mut c_void) -> Option<CaretAnchor> {
    let value = property_string(element, UIA_VALUE_VALUE_PROPERTY_ID)?;
    if !value.is_empty() {
        return None;
    }
    property_bounding_rect(element).and_then(editable_element_caret_anchor)
}

fn editable_element_caret_anchor(rect: [f64; 4]) -> Option<CaretAnchor> {
    if !rect.iter().all(|value| value.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
        return None;
    }

    let top = (rect[1] + 2.0)
        .round()
        .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    let bottom = (rect[1] + rect[3] - 2.0)
        .round()
        .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    Some(CaretAnchor {
        x: (rect[0] + 4.0)
            .round()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        top,
        bottom: bottom.max(top.saturating_add(1)),
    })
}

unsafe fn property_bounding_rect(element: *mut c_void) -> Option<[f64; 4]> {
    let mut value: VARIANT = zeroed();
    if !get_property(element, UIA_BOUNDING_RECTANGLE_PROPERTY_ID, &mut value) {
        VariantClear(&mut value);
        return None;
    }

    let result = if value.vt == VT_ARRAY_R8 {
        let array = value.data.pointer as *mut SAFEARRAY;
        let values = safe_array_f64_values_borrowed(array);
        (values.len() >= 4).then(|| [values[0], values[1], values[2], values[3]])
    } else {
        None
    };
    VariantClear(&mut value);
    result
}

unsafe fn property_bool(element: *mut c_void, property_id: i32) -> Option<bool> {
    let mut value: VARIANT = zeroed();
    if !get_property(element, property_id, &mut value) {
        VariantClear(&mut value);
        return None;
    }
    let result = if value.vt == VT_BOOL {
        Some(value.data.bool_val != VARIANT_FALSE)
    } else {
        None
    };
    VariantClear(&mut value);
    result
}

unsafe fn property_i32(element: *mut c_void, property_id: i32) -> Option<i32> {
    let mut value: VARIANT = zeroed();
    if !get_property(element, property_id, &mut value) {
        VariantClear(&mut value);
        return None;
    }
    let result = if value.vt == VT_I4 {
        Some(value.data.l_val)
    } else {
        None
    };
    VariantClear(&mut value);
    result
}

unsafe fn property_string(element: *mut c_void, property_id: i32) -> Option<String> {
    let mut value: VARIANT = zeroed();
    if !get_property(element, property_id, &mut value) {
        VariantClear(&mut value);
        return None;
    }

    let result = if value.vt == VT_BSTR {
        let pointer = value.data.pointer as *const u16;
        if pointer.is_null() {
            Some(String::new())
        } else {
            let length = SysStringLen(pointer) as usize;
            let text = std::slice::from_raw_parts(pointer, length);
            Some(String::from_utf16_lossy(text))
        }
    } else {
        None
    };
    VariantClear(&mut value);
    result
}

unsafe fn get_property(element: *mut c_void, property_id: i32, value: *mut VARIANT) -> bool {
    type Method = unsafe extern "system" fn(*mut c_void, i32, BOOL, *mut VARIANT) -> HRESULT;
    let Some(address) = com_method_address(element, 11) else {
        return false;
    };
    let method: Method = transmute(address);
    // Ignore UI Automation's default values so an unsupported IsEnabled or
    // IsReadOnly property is not mistaken for real read-only evidence.
    method(element, property_id, TRUE, value) >= 0
}

unsafe fn add_ref_com(object: *mut c_void) -> bool {
    if object.is_null() {
        return false;
    }
    type Method = unsafe extern "system" fn(*mut c_void) -> u32;
    let Some(address) = com_method_address(object, 1) else {
        return false;
    };
    let method: Method = transmute(address);
    method(object);
    true
}

unsafe fn release_com(object: *mut c_void) {
    if object.is_null() {
        return;
    }
    type Method = unsafe extern "system" fn(*mut c_void) -> u32;
    if let Some(address) = com_method_address(object, 2) {
        let method: Method = transmute(address);
        method(object);
    }
}

unsafe fn com_method_address(object: *mut c_void, index: usize) -> Option<usize> {
    if object.is_null() {
        return None;
    }
    let vtable = *(object as *mut *mut usize);
    if vtable.is_null() {
        return None;
    }
    let address = *vtable.add(index);
    (address != 0).then_some(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_and_unavailable_states_are_distinct_bits() {
        assert_eq!(STATE_SYSTEM_READONLY, 0x40);
        assert_eq!(STATE_SYSTEM_UNAVAILABLE, 0x01);
    }

    #[test]
    fn only_positive_editable_evidence_accepts_text_input() {
        assert!(Editability::Editable.accepts_text_input());
        assert!(!Editability::ReadOnly.accepts_text_input());
        assert!(!Editability::Unknown.accepts_text_input());
    }

    #[test]
    fn inline_code_roles_are_never_text_entry_evidence() {
        assert!(is_code_like_aria_role("code"));
        assert!(is_code_like_aria_role("doc-code"));
        assert!(!is_text_entry_aria_role("code"));
    }

    #[test]
    fn only_explicit_text_entry_roles_are_accepted() {
        assert!(is_text_entry_aria_role("textbox"));
        assert!(is_text_entry_aria_role("searchbox"));
        assert!(!is_text_entry_aria_role("document"));
        assert!(!is_text_entry_aria_role("generic"));
    }

    #[test]
    fn chromium_searchbox_accepts_missing_focusable_property() {
        assert!(text_entry_role_accepts_focus(None, None));
        assert!(text_entry_role_accepts_focus(Some(true), Some(false)));
        assert!(!text_entry_role_accepts_focus(Some(false), Some(false)));
    }

    #[test]
    fn caret_rectangle_anchor_uses_requested_edge() {
        assert_eq!(
            rect_anchor([100.0, 200.0, 12.0, 18.0], false).unwrap().x,
            100
        );
        assert_eq!(
            rect_anchor([100.0, 200.0, 12.0, 18.0], true).unwrap().x,
            112
        );
        let anchor = rect_anchor([100.0, 200.0, 12.0, 18.0], true).unwrap();
        assert_eq!(anchor.top, 200);
        assert_eq!(anchor.bottom, 218);
    }

    #[test]
    fn direct_text_range_never_jumps_to_a_wide_rectangle_edge() {
        assert_eq!(
            direct_text_range_anchor([100.0, 200.0, 1.0, 18.0])
                .unwrap()
                .x,
            100
        );
        assert!(direct_text_range_anchor([100.0, 200.0, 240.0, 18.0]).is_none());
    }

    #[test]
    fn editable_element_fallback_stays_inside_element() {
        let point = editable_element_caret_anchor([100.0, 200.0, 240.0, 32.0]).unwrap();
        assert_eq!(point.x, 104);
        assert_eq!(point.top, 202);
        assert_eq!(point.bottom, 230);
        assert!(editable_element_caret_anchor([0.0, 0.0, 0.0, 20.0]).is_none());
    }

    #[test]
    fn caret_descendant_search_is_bounded() {
        assert_eq!(MAX_UIA_CARET_DESCENDANT_DEPTH, 4);
        assert_eq!(MAX_UIA_CARET_DESCENDANT_NODES, 64);
    }

    #[test]
    fn browser_exact_mode_rejects_editable_documents() {
        assert!(evidence_accepts_caret_probe(
            NodeEvidence::EditableField,
            false,
            true,
        ));
        assert!(!evidence_accepts_caret_probe(
            NodeEvidence::EditableDocument,
            false,
            true,
        ));
        assert!(evidence_accepts_caret_probe(
            NodeEvidence::EditableDocument,
            false,
            false,
        ));
    }

    #[test]
    fn console_cell_coordinates_are_converted_to_screen_pixels() {
        assert_eq!(
            console_cell_anchor(POINT { x: 100, y: 200 }, 3, 2, 8, 16, 1),
            CaretAnchor {
                x: 132,
                top: 232,
                bottom: 248,
            }
        );
        assert_eq!(
            console_cell_anchor(POINT { x: 100, y: 200 }, 3, 2, 8, 16, 2),
            CaretAnchor {
                x: 140,
                top: 232,
                bottom: 248,
            }
        );
    }

    #[test]
    fn korean_composition_uses_double_width_console_columns() {
        assert_eq!(
            utf16_console_columns(&"가".encode_utf16().collect::<Vec<_>>()),
            2
        );
        assert_eq!(
            utf16_console_columns(&"ab".encode_utf16().collect::<Vec<_>>()),
            2
        );
        assert_eq!(
            utf16_console_columns(&"가a".encode_utf16().collect::<Vec<_>>()),
            3
        );
    }

    #[test]
    fn console_window_classes_are_recognized() {
        assert!(is_console_like_class("ConsoleWindowClass"));
        assert!(is_console_like_class("CASCADIA_HOSTING_WINDOW_CLASS"));
        assert!(!is_console_like_class("Chrome_WidgetWin_1"));
    }

    #[test]
    fn chromium_and_firefox_use_uia_as_the_authoritative_caret_source() {
        assert!(is_uia_preferred_caret_class("Chrome_WidgetWin_1"));
        assert!(is_uia_preferred_caret_class("MozillaWindowClass"));
        assert!(!is_uia_preferred_caret_class("Notepad"));
    }

    #[test]
    fn office_word_document_window_is_recognized() {
        assert!(is_office_word_editor_class("_WwG"));
        assert!(is_office_word_editor_class("_wwg"));
        assert!(!is_office_word_editor_class("_WwB"));
        assert!(!is_office_word_editor_class("rctrl_renwnd32"));
    }

    #[test]
    fn adjacent_character_rectangles_must_be_character_sized() {
        assert!(character_rect_anchor([100.0, 200.0, 12.0, 18.0], true).is_some());
        assert!(character_rect_anchor([100.0, 200.0, 320.0, 18.0], true).is_none());
    }

    #[test]
    fn editable_combobox_role_is_supported() {
        assert!(is_text_entry_aria_role("combobox"));
    }

    #[test]
    fn text_pattern_identifiers_match_windows_headers() {
        assert_eq!(UIA_TEXT_PATTERN_ID, 10014);
        assert_eq!(IID_IUIAUTOMATION_TEXT_PATTERN.Data1, 0x32eba289);
        assert_eq!(UIA_TEXT_PATTERN2_ID, 10024);
        assert_eq!(IID_IUIAUTOMATION_TEXT_PATTERN2.Data1, 0x506a921a);
        assert_eq!(UIA_TEXT_EDIT_PATTERN_ID, 10032);
        assert_eq!(IID_IUIAUTOMATION_TEXT_EDIT_PATTERN.Data1, 0x17e21576);
        assert_eq!(IID_IACCESSIBLE.Data1, 0x618736e0);
        assert_eq!(OBJID_CARET, 0xffff_fff8);
    }

    #[test]
    fn accessibility_caret_location_uses_the_narrow_caret_right_edge() {
        assert_eq!(
            accessible_caret_rect_anchor(100, 20, 1, 18),
            Some(CaretAnchor {
                x: 101,
                top: 20,
                bottom: 38,
            })
        );
        assert_eq!(
            accessible_caret_rect_anchor(100, 20, 0, 18),
            Some(CaretAnchor {
                x: 101,
                top: 20,
                bottom: 38,
            })
        );
        assert_eq!(accessible_caret_rect_anchor(100, 20, 40, 18), None);
    }

    #[test]
    fn cmd_is_prioritized_when_resolving_a_console_client() {
        fn executable_name(value: &str) -> [u16; 260] {
            let mut output = [0u16; 260];
            for (index, character) in value.encode_utf16().enumerate() {
                output[index] = character;
            }
            output
        }

        assert_eq!(console_process_priority(&executable_name("cmd.exe")), 0);
        assert_eq!(console_process_priority(&executable_name("pwsh.exe")), 1);
        assert_eq!(console_process_priority(&executable_name("python.exe")), 2);
        assert_eq!(console_process_priority(&executable_name("conhost.exe")), 3);
    }

    #[test]
    fn variant_layout_matches_automation_abi() {
        #[cfg(target_pointer_width = "64")]
        assert_eq!(size_of::<VARIANT>(), 24);
        #[cfg(target_pointer_width = "32")]
        assert_eq!(size_of::<VARIANT>(), 16);
    }
}
