#!/usr/bin/env python3
"""Dependency-free static checks for IME Caret 2.0."""

from __future__ import annotations

import hashlib
import re
import tomllib
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"


def check_delimiters(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    stack: list[tuple[str, int]] = []
    pairs = {")": "(", "]": "[", "}": "{"}
    state = "code"
    block_depth = 0
    line = 1
    index = 0

    def looks_like_char_literal(position: int) -> bool:
        if position + 2 < len(text) and text[position + 2] == "'":
            return True
        return (
            position + 3 < len(text)
            and text[position + 1] == "\\"
            and text[position + 3] == "'"
        )

    while index < len(text):
        char = text[index]
        next_char = text[index + 1] if index + 1 < len(text) else ""

        if state == "code":
            if char == "/" and next_char == "/":
                state = "line-comment"
                index += 2
                continue
            if char == "/" and next_char == "*":
                state = "block-comment"
                block_depth = 1
                index += 2
                continue
            if char == "r" and re.match(r'r#*"', text[index:]):
                raise AssertionError(f"{path}: raw string scanner update required at line {line}")
            if char == '"':
                state = "string"
                index += 1
                continue
            if char == "'" and looks_like_char_literal(index):
                state = "char"
                index += 1
                continue
            if char in "([{":
                stack.append((char, line))
            elif char in ")]}":
                if not stack or stack[-1][0] != pairs[char]:
                    raise AssertionError(f"{path}: mismatched {char!r} at line {line}")
                stack.pop()
            if char == "\n":
                line += 1
            index += 1
            continue

        if state == "line-comment":
            if char == "\n":
                state = "code"
                line += 1
            index += 1
            continue

        if state == "block-comment":
            if char == "/" and next_char == "*":
                block_depth += 1
                index += 2
                continue
            if char == "*" and next_char == "/":
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
                continue
            if char == "\n":
                line += 1
            index += 1
            continue

        if state in {"string", "char"}:
            if char == "\\":
                index += 2
                continue
            terminator = '"' if state == "string" else "'"
            if char == terminator:
                state = "code"
            if char == "\n":
                line += 1
            index += 1

    assert not stack, f"{path}: unclosed delimiters: {stack[-5:]}"
    assert state in {"code", "line-comment"}, f"{path}: source ended in state {state}"


def extract_asset(name: str, source: str) -> bytes:
    match = re.search(
        rf"pub const {re.escape(name)}: &str = concat!\((.*?)\);",
        source,
        re.DOTALL,
    )
    assert match is not None, f"asset not found: {name}"
    hex_text = "".join(re.findall(r'"([0-9A-Fa-f]+)"', match.group(1)))
    return bytes.fromhex(hex_text)


def main() -> None:
    for path in sorted(SRC.glob("*.rs")):
        check_delimiters(path)

    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    assert cargo["package"]["name"] == "ime-caret"
    assert cargo["package"]["version"] == "2.0.0"
    assert 'name = "ime-caret"' in (ROOT / "Cargo.lock").read_text(encoding="utf-8")
    assert 'version = "2.0.0"' in (ROOT / "Cargo.lock").read_text(encoding="utf-8")
    assert "active editable text caret" in cargo["package"]["description"]

    main_rs = (SRC / "main.rs").read_text(encoding="utf-8")
    ime_rs = (SRC / "ime.rs").read_text(encoding="utf-8")
    editability_rs = (SRC / "editability.rs").read_text(encoding="utf-8")
    outlook_rs = (SRC / "outlook.rs").read_text(encoding="utf-8")
    config_rs = (SRC / "config.rs").read_text(encoding="utf-8")
    win_rs = (SRC / "win.rs").read_text(encoding="utf-8")
    assets_rs = (SRC / "assets.rs").read_text(encoding="utf-8")
    all_source = "\n".join([main_rs, ime_rs, editability_rs, outlook_rs, config_rs, win_rs, assets_rs])

    assert 'const APP_NAME: &str = "IME Caret";' in main_rs
    assert 'const APP_VERSION: &str = "2.0";' in main_rs
    assert "tray_tooltip_text()" in main_rs
    assert 'PathBuf::from("IME Caret.exe")' in main_rs
    assert 'exe_dir.join("IMECaret.ini")' in main_rs
    assert '"IME Cursor Rust' not in main_rs
    assert '"IME Cursor ' not in main_rs
    build_ps1 = (ROOT / "build.ps1").read_text(encoding="ascii")
    assert 'target\\release\\ime-caret.exe' in build_ps1
    assert '"IMECaret.exe"' in build_ps1
    assert '"IMECaret.ini"' in build_ps1

    forbidden = [
        "ImeTargetMode",
        "MouseControl",
        "window_at_cursor",
        "at_cursor(",
        "GetCursorInfo",
        "CURSORINFO",
        "SetSystemCursor",
        "SystemParametersInfoW",
        "CreateCursor",
        "DestroyCursor",
        "OCR_IBEAM",
        "SPI_SETCURSORS",
        "IDC_IBEAM",
        "CURSOR_DEFAULT_HEX",
        "CURSOR_EL_HEX",
        "CURSOR_EU_HEX",
        "CURSOR_JH_HEX",
        "CURSOR_JK_HEX",
        "CURSOR_K_HEX",
        "show_fallback_badge",
        "show_english_ibeam",
        "show_japanese_ibeam",
        "show_korean_ibeam",
    ]
    for token in forbidden:
        assert token not in all_source, token

    # No code path depends on the mouse position or mouse cursor state.
    assert "GetCursorPos" not in all_source

    refresh_start = main_rs.index("unsafe fn refresh_from_active_caret")
    refresh_end = main_rs.index("unsafe fn refresh_ime_state_only", refresh_start)
    refresh_body = main_rs[refresh_start:refresh_end]
    assert refresh_body.index("focused_input()") < refresh_body.index("ime_engine.query(")
    assert refresh_body.index("ime_engine.query(") < refresh_body.index(
        "focused_caret_anchor(console_cell_span)"
    )
    assert "focused_input_host_for_shell()" in refresh_body
    assert "GetCursorPos" not in refresh_body

    assert "const CARET_INDICATOR_WIDTH: i32 = 15;" in main_rs
    assert "const CARET_INDICATOR_HEIGHT: i32 = 15;" in main_rs
    assert "const CARET_INDICATOR_X_GAP: i32 = 4;" in main_rs
    assert "render_layered_badge" in main_rs
    assert "UpdateLayeredWindow" in all_source
    assert "AC_SRC_ALPHA" in all_source
    assert "const ACTIVE_IME_POLL_INTERVAL_MS: u32 = 100;" in main_rs
    assert "const RAW_INPUT_ACTIVE_POLL_INTERVAL_MS: u32 = 500;" in main_rs
    assert "const FALLBACK_REFRESH_INTERVAL_MS: u32 = 500;" in main_rs
    assert "const IDLE_FALLBACK_INTERVAL_MS: u32 = 2_000;" in main_rs
    assert "const DEEP_IDLE_FALLBACK_INTERVAL_MS: u32 = 5_000;" in main_rs
    assert "const FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(500);" in main_rs
    assert "const RAW_INPUT_FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(1_000);" in main_rs
    assert "const IDLE_FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(2_000);" in main_rs
    assert "const DEEP_IDLE_FULL_REFRESH_INTERVAL: Duration = Duration::from_millis(5_000);" in main_rs
    assert "const EVENT_REFRESH_MIN_INTERVAL: Duration = Duration::from_millis(50);" in main_rs
    assert "const KEYBOARD_ACTIVITY_DELAY_MS: u32 = 20;" in main_rs
    assert "const ACTIVITY_BACKOFF_DELAY: Duration = Duration::from_secs(3);" in main_rs
    assert "const DEEP_IDLE_BACKOFF_DELAY: Duration = Duration::from_secs(15);" in main_rs
    assert "badge_position: Option<(i32, i32)>" in main_rs
    assert "self.badge_position != Some((x, y))" in main_rs
    assert "SetWinEventHook(" in main_rs
    assert "UnhookWinEvent(" in main_rs
    assert "WIN_EVENT_PENDING_FLAGS.fetch_or" in main_rs
    assert "WIN_EVENT_ALLOWED_HOST_PROCESS_ID" in main_rs
    assert "win_event_source_is_relevant" in main_rs
    assert "RegisterRawInputDevices(" in main_rs
    assert "RIDEV_INPUTSINK" in main_rs
    assert "RIDEV_NOLEGACY" not in all_source
    assert "GetRawInputData(" in main_rs
    assert "RI_KEY_BREAK" in main_rs
    assert "const RAW_INPUT_DUPLICATE_WINDOW: Duration = Duration::from_millis(2);" in main_rs
    assert "raw_keyboard_signal_is_duplicate" in main_rs
    assert "raw_keyboard_signal_needs_refresh" in main_rs
    assert "raw_keyboard_key_can_change_ime" in main_rs
    assert "GetAsyncKeyState" in all_source
    assert "keyboard_activity_covered_by_caret_event" in main_rs
    assert "keyboard_activity_refresh_needed" in main_rs
    assert "WM_INPUT" in main_rs
    assert "const UIA_FOCUSED_ELEMENT_CACHE_DURATION: Duration = Duration::from_millis(100);" in editability_rs
    assert "focused_element_for_targets" in editability_rs
    assert "clear_focused_element_cache" in editability_rs
    assert "add_ref_com" in editability_rs
    assert "is_office_word_editor_class" in editability_rs
    assert 'class_name.eq_ignore_ascii_case("_WwG")' in editability_rs
    assert "classify_office_word_editor_window" in editability_rs
    assert "None => {\n                let uia_result = self.classify_focused_with_uia(targets);" in editability_rs
    assert "OFFICE_WORD_READER_PARENT_STYLE" in editability_rs
    assert "MAX_OFFICE_WORD_HOST_PARENT_DEPTH" in editability_rs
    assert 'class_name.eq_ignore_ascii_case("_WwB")' in editability_rs
    assert 'class_name.eq_ignore_ascii_case("rctrl_renwnd32")' in editability_rs
    assert "host_style & WS_CHILD" in editability_rs
    assert "classify_office_word_editor_target" in editability_rs
    assert "outlook::editor_state" in editability_rs
    assert "cached.caret == targets.caret" in editability_rs
    assert '"ActiveInspector"' in outlook_rs
    assert '"CurrentItem"' in outlook_rs
    assert '"Sent"' in outlook_rs
    assert '"ActiveInlineResponse"' in outlook_rs
    assert "OutlookEditorState::Unknown => Editability::ReadOnly" in editability_rs
    assert "accessible_caret_anchor_from_object" in editability_rs
    assert "saw_office_word_editor" in editability_rs
    assert "if window == last_office_window" in editability_rs
    assert "if saw_office_word_editor {\n            return None;" in editability_rs
    assert "accessible_system_caret_anchor(window)" in editability_rs
    assert "WM_APP_ACTIVITY" in main_rs
    assert "WIN_EVENT_UPDATE_PENDING" in main_rs
    assert "refresh_caret_position_only" in main_rs
    assert "caret_refresh_pending" in main_rs
    assert "FOCUS_ACTIVATION_RETRY_COUNT" in main_rs
    assert "schedule_focus_activation_retry_if_needed" in main_rs
    assert "if event == EVENT_SYSTEM_FOREGROUND" in main_rs
    assert "const FOCUS_PROBE_CACHE_DURATION: Duration = Duration::from_millis(250);" in editability_rs
    assert "caret_indicator_position_in_bounds" in main_rs
    assert "const SETTINGS_CLIENT_HEIGHT: i32 = 402;" in main_rs
    assert "const SETTINGS_HORIZONTAL_MARGIN: i32 = 18;" in main_rs
    assert "const SETTINGS_BOTTOM_MARGIN: i32 = 18;" in main_rs
    assert "const SETTINGS_GROUP_BUTTON_GAP: i32 = 14;" in main_rs
    assert "IME 상태는 활성 입력 캐럿 옆에" not in main_rs
    assert "SS_NOPREFIX" not in all_source
    assert "SETTINGS_GROUP_CAPTION_VISUAL_INSET" in main_rs
    assert "한글/영문/일본어 IME 상태를" not in main_rs
    assert "마우스 커서는 변경하지 않습니다." not in main_rs
    assert "fn settings_layout(" in main_rs
    assert "GetClientRect(hwnd, &mut client_rect)" in main_rs
    assert "settings_layout_keeps_equal_outer_and_group_content_margins" in main_rs
    assert 'wide("설정을 변경하려면 우클릭하세요.")' in main_rs
    assert "NIF_SHOWTIP" in main_rs
    assert "const TRAY_HINT_DISPLAY_MS: u32 = 2_000;" in main_rs
    assert '"IndicatorTextColor={}\\r\\n"' in config_rs
    assert '"EnglishBackgroundColor={}\\r\\n"' in config_rs
    assert '"JapaneseBackgroundColor={}\\r\\n"' in config_rs
    assert '"KoreanBackgroundColor={}\\r\\n"' in config_rs
    assert '"상태 표시 글자색"' in main_rs
    assert '"영문 배경색"' in main_rs
    assert '"일본어 배경색"' in main_rs
    assert '"한글 배경색"' in main_rs
    assert '"상태 표시 위치"' in main_rs
    assert '"한/영 표시 위치"' not in main_rs
    assert "play_sounds: false" in config_rs
    assert "indicator_position: IndicatorPosition::Below" in config_rs
    assert '"캐럿 오른쪽", "캐럿 위", "캐럿 아래"' in main_rs
    assert '"캐럿 오른쪽 (기본)"' not in main_rs
    assert "RgbaColor::new(0xff, 0x62, 0x62, 0xa5)" in config_rs
    assert "RgbaColor::new(0x62, 0xff, 0x62, 0xa5)" in config_rs
    assert "RgbaColor::new(0x62, 0x62, 0xff, 0xa5)" in config_rs
    assert "excel_editor_window" in editability_rs
    assert '"excel6" | "excel<" | "edtbx"' in editability_rs
    assert 'normalized == "xlmain" || normalized.starts_with("bosa_sdm_xl")' in editability_rs
    assert "excel_dialog_caret_anchor" in editability_rs
    assert "EnumChildWindows(" in editability_rs
    assert 'class_name.eq_ignore_ascii_case("EDTBX")' in editability_rs
assert "excel_dialog_editor_bounds_anchor" in editability_rs
assert "for _ in 0..=4" in editability_rs
assert "excel_dialog_focused_element_anchor" in editability_rs
assert "UI Automation still exposes the focused Edit element" in editability_rs
assert "excel_dialog_root_for_targets" in editability_rs
assert "excel_dialog_for_process" in editability_rs
assert "search.anchor.or(search.fallback)" in editability_rs
assert "delayed_focus_surface_pending" in main_rs
assert "raw_keyboard_signal_opens_delayed_focus_surface" in main_rs
assert "DELAYED_FOCUS_SURFACE_RETRY_COUNT" in main_rs
assert "schedule_delayed_focus_surface_retry_if_needed" in main_rs
assert "Reassert topmost only" in main_rs
assert "is_excel_find_replace_foreground" in main_rs
assert "refresh the badge's topmost order" in main_rs
    assert "accepts_text_input" in editability_rs
    assert "UIA_HAS_KEYBOARD_FOCUS_PROPERTY_ID" in editability_rs
    assert "descendant_caret_anchor" in editability_rs
    assert "MAX_UIA_CARET_DESCENDANT_NODES: usize = 64" in editability_rs
    assert "editable_element_caret_anchor" in editability_rs
    assert "descendant_contains_editable" in editability_rs
    assert "descendant_editable_caret_anchor" in editability_rs
    assert "UIA_COMBO_BOX_CONTROL_TYPE_ID" in editability_rs
    assert '"combobox"' in editability_rs
    assert "is_console_like_class" in editability_rs
    assert "classic_console_caret_anchor" in editability_rs
    assert "console_cell_anchor" in editability_rs
    assert "direct_text_range_anchor" in editability_rs
    assert "rect_anchor(rect, false)" in editability_rs
    assert "caret_anchor_from_adjacent_character" in editability_rs
    assert "text_range_move_endpoint_by_unit" in editability_rs
    assert "UIA_VALUE_VALUE_PROPERTY_ID" in editability_rs
    assert "empty_editable_element_caret_anchor" in editability_rs
    caret_probe_start = editability_rs.index("unsafe fn focused_caret_anchor_unsafe")
    caret_probe_end = editability_rs.index("unsafe fn focused_input_unsafe", caret_probe_start)
    caret_probe = editability_rs[caret_probe_start:caret_probe_end]
    assert caret_probe.index("uia_preferred") < caret_probe.index("classic_console_caret_anchor")
    assert "is_uia_preferred_caret_window" in caret_probe
    assert "CaretProbeResult::Suppress | CaretProbeResult::Missing => None" in caret_probe
    assert caret_probe.rindex("win32_caret_anchor") < caret_probe.rindex("uia_focused_caret_anchor")
    assert "selection_is_noncollapsed" in editability_rs
    assert "CaretProbeResult::Suppress" in editability_rs
    assert "probe_caret_anchor_from_uia_element" in editability_rs
    assert "exact_caret_anchor_from_uia_element" not in editability_rs
    assert "suppress the marker" in editability_rs
    assert "noncollapsed_selection_trailing_anchor" not in editability_rs
    assert "editable_element_trailing_anchor" not in editability_rs
    assert "direct_text_range_never_jumps_to_a_wide_rectangle_edge" in editability_rs
    assert "AttachConsole" in all_source
    assert "GetConsoleScreenBufferInfo" in all_source
    assert "cursor.bVisible" not in editability_rs
    assert "GetConsoleCursorInfo" not in all_source
    assert "screen-buffer cursor while keeping dwCursorPosition" in editability_rs
    assert "console_client_candidates" in editability_rs
    assert "try_attach_console_process" in editability_rs
    assert "AccessibleObjectFromWindow" in all_source
    assert '#[link(name = "oleacc")]' in win_rs
    assert "OBJID_CARET: DWORD = 0xffff_fff8" in editability_rs
    assert "IID_IACCESSIBLE" in editability_rs
    assert "accessible_focused_caret_anchor" in editability_rs
    assert "accessible_system_caret_anchor" in editability_rs
    assert "descendant_contains_matching_anchor" in editability_rs
    assert "accessible_caret_rect_anchor" in editability_rs
    assert "CreateToolhelp32Snapshot" in all_source
    assert "Process32FirstW" in all_source
    assert "Process32NextW" in all_source
    assert "ProcessIdToSessionId" in all_source
    assert "PROCESSENTRY32W" in win_rs
    assert "GetCurrentConsoleFontEx" in all_source
    assert "ImmGetCompositionStringW" in all_source
    assert "GCS_COMPSTR" in all_source
    assert "GCS_CURSORPOS" in all_source
    assert "ime_composition_display_columns" in editability_rs
    assert "console_cell_span" in main_rs
    assert "visual_cell_span.clamp(1, 8)" in editability_rs
    assert "korean_composition_uses_double_width_console_columns" in editability_rs
    assert "is_uia_preferred_caret_window" in editability_rs
    assert "exact_geometry_only" in editability_rs
    assert "anchor_matches_element" in editability_rs
    assert "MAX_CHARACTER_RECT_WIDTH" in editability_rs
    assert "chromium_and_firefox_use_uia_as_the_authoritative_caret_source" in editability_rs
    assert "column + visual_cell_span.clamp(1, 8)" in editability_rs
    assert "column + composition_columns.max(0) + 1" not in editability_rs
    assert "self.uia_focused_caret_anchor(console_like, true)" in caret_probe
    browser_branch_start = caret_probe.index("if uia_preferred")
    browser_branch_end = caret_probe.index("if console_like", browser_branch_start)
    browser_branch = caret_probe[browser_branch_start:browser_branch_end]
    assert browser_branch.index("accessible_focused_caret_anchor") < browser_branch.index("uia_focused_caret_anchor")
    console_branch_start = caret_probe.index("if console_like", browser_branch_end)
    console_branch_end = caret_probe.index("if let Some(anchor) = win32_caret_anchor", console_branch_start)
    console_branch = caret_probe[console_branch_start:console_branch_end]
    assert "self.uia_focused_caret_anchor(true, false)" in console_branch
    assert caret_probe.index("if console_like", browser_branch_end) < caret_probe.index("win32_caret_anchor")
    assert "selection_range.is_none()" in editability_rs
    assert "pattern2.and_then(|pattern| text_pattern_selection_range(pattern))" in editability_rs
    assert "UIA_TEXT_EDIT_PATTERN_ID: i32 = 10032" in editability_rs
    assert "IID_IUIAUTOMATION_TEXT_EDIT_PATTERN" in editability_rs
    assert "text_edit_active_composition_range" in editability_rs
    assert "caret_anchor_from_range_end" in editability_rs
    assert "text_range_move_endpoint_by_range" in editability_rs
    uia_probe_start = editability_rs.index("unsafe fn probe_caret_anchor_from_uia_element")
    uia_probe_end = editability_rs.index("unsafe fn descendant_contains_editable", uia_probe_start)
    uia_probe = editability_rs[uia_probe_start:uia_probe_end]
    assert uia_probe.index("UIA_TEXT_EDIT_PATTERN_ID") < uia_probe.index("selection_is_noncollapsed")
    assert uia_probe.index("text_edit_active_composition_range") < uia_probe.index("text_pattern_selection_range")
    assert uia_probe.index("selection_range.take()") < uia_probe.index("text_pattern2_caret_range")
    selection_anchor_start = editability_rs.index("unsafe fn caret_anchor_from_selection_range")
    selection_anchor_end = editability_rs.index("unsafe fn caret_anchor_from_text_range", selection_anchor_start)
    selection_anchor = editability_rs[selection_anchor_start:selection_anchor_end]
    assert selection_anchor.index("caret_anchor_from_adjacent_character") < selection_anchor.index("caret_anchor_from_text_range")
    text_anchor_start = editability_rs.index("unsafe fn caret_anchor_from_text_range")
    text_anchor_end = editability_rs.index("unsafe fn caret_anchor_from_adjacent_character", text_anchor_start)
    text_anchor = editability_rs[text_anchor_start:text_anchor_end]
    assert text_anchor.index("caret_anchor_from_adjacent_character") < text_anchor.index("if exact_geometry_only")
    assert "browser_exact_mode_rejects_editable_documents" in editability_rs
    assert "CONSOLE_SCREEN_BUFFER_INFO" in win_rs
    assert "CONSOLE_FONT_INFOEX" in win_rs
    assert "query_ime_context" in ime_rs
    assert "ImmGetContext" in all_source
    assert "pub target:" not in ime_rs
    assert "pub target_thread:" not in ime_rs

    assert "GetIMEStatus" not in (ROOT / "IMECaret.ini").read_text(encoding="utf-8")
    assert "ShowEnglishIBeam" not in config_rs

    assert len(extract_asset("ICON_DEFAULT_HEX", assets_rs)) == 296
    for token in [
        "ICON_E_HEX",
        "ICON_J_HEX",
        "ICON_K_HEX",
        "TrayDisplay",
        "show_ime_tray_icon",
        "ShowIMETrayIcon",
        "set_tray_display",
        "display_for_current_state",
        "tray_display",
    ]:
        assert token not in all_source + config_rs + assets_rs + (ROOT / "IMECaret.ini").read_text(encoding="utf-8"), token

    assert not (ROOT / "restore-cursor.cmd").exists()
    assert not (ROOT / "restore-cursor.ps1").exists()

    for path in [ROOT / "build.cmd", ROOT / "build.ps1"]:
        path.read_bytes().decode("ascii")

    expected_wav_hashes = {
        "IMEE.wav": "c1772ab89e863b09f0e314342210f39c25f46fa9fd6e20107202f1e868397913",
        "IMEJ.wav": "0f1140733387026580f2f8ac99e6375c451f548fd5a76de650826134b00d1616",
        "IMEK.wav": "79237c1a68b984163c9d62be59aca983df42ffb7bf7722621a583c7b5b4bd2d1",
    }
    for name, expected_hash in expected_wav_hashes.items():
        path = ROOT / "assets" / name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected_hash
        with wave.open(str(path), "rb") as audio:
            assert audio.getnchannels() == 1
            assert audio.getsampwidth() == 2
            assert audio.getframerate() == 22050
            assert audio.getnframes() > 0

    print("IME Caret 2.0 static checks passed")


if __name__ == "__main__":
    main()
