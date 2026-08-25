#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]

use std::ffi::c_void;

pub type BOOL = i32;
pub type ATOM = u16;
pub type WORD = u16;
pub type DWORD = u32;
pub type UINT = u32;
pub type LONG = i32;
pub type ULONG = u32;
pub type HRESULT = i32;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type COLORREF = u32;
pub type HANDLE = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HMONITOR = *mut c_void;
pub type HWND = *mut c_void;
pub type HWINEVENTHOOK = *mut c_void;
pub type HMENU = *mut c_void;
pub type HICON = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HBITMAP = *mut c_void;
pub type HDC = *mut c_void;
pub type HGDIOBJ = *mut c_void;
pub type HFONT = *mut c_void;
pub type HKL = *mut c_void;
pub type HRAWINPUT = *mut c_void;
pub type DPI_AWARENESS_CONTEXT = HANDLE;
pub type PCWSTR = *const u16;
pub type PWSTR = *mut u16;
pub type SAFEARRAY = c_void;

pub const FALSE: BOOL = 0;
pub const TRUE: BOOL = 1;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: DPI_AWARENESS_CONTEXT =
    -4isize as DPI_AWARENESS_CONTEXT;

pub type WNDPROC = Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>;
pub type WNDENUMPROC = Option<unsafe extern "system" fn(HWND, LPARAM) -> BOOL>;
pub type TIMERPROC = Option<unsafe extern "system" fn(HWND, UINT, usize, DWORD)>;
pub type WINEVENTPROC =
    Option<unsafe extern "system" fn(HWINEVENTHOOK, DWORD, HWND, LONG, LONG, DWORD, DWORD)>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct COORD {
    pub X: i16,
    pub Y: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SMALL_RECT {
    pub Left: i16,
    pub Top: i16,
    pub Right: i16,
    pub Bottom: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CONSOLE_SCREEN_BUFFER_INFO {
    pub dwSize: COORD,
    pub dwCursorPosition: COORD,
    pub wAttributes: WORD,
    pub srWindow: SMALL_RECT,
    pub dwMaximumWindowSize: COORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CONSOLE_FONT_INFOEX {
    pub cbSize: ULONG,
    pub nFont: DWORD,
    pub dwFontSize: COORD,
    pub FontFamily: UINT,
    pub FontWeight: UINT,
    pub FaceName: [u16; 32],
}

impl Default for CONSOLE_FONT_INFOEX {
    fn default() -> Self {
        Self {
            cbSize: 0,
            nFont: 0,
            dwFontSize: COORD::default(),
            FontFamily: 0,
            FontWeight: 0,
            FaceName: [0; 32],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PROCESSENTRY32W {
    pub dwSize: DWORD,
    pub cntUsage: DWORD,
    pub th32ProcessID: DWORD,
    pub th32DefaultHeapID: usize,
    pub th32ModuleID: DWORD,
    pub cntThreads: DWORD,
    pub th32ParentProcessID: DWORD,
    pub pcPriClassBase: LONG,
    pub dwFlags: DWORD,
    pub szExeFile: [u16; 260],
}

impl Default for PROCESSENTRY32W {
    fn default() -> Self {
        Self {
            dwSize: 0,
            cntUsage: 0,
            th32ProcessID: 0,
            th32DefaultHeapID: 0,
            th32ModuleID: 0,
            cntThreads: 0,
            th32ParentProcessID: 0,
            pcPriClassBase: 0,
            dwFlags: 0,
            szExeFile: [0; 260],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RAWINPUTDEVICE {
    pub usUsagePage: WORD,
    pub usUsage: WORD,
    pub dwFlags: DWORD,
    pub hwndTarget: HWND,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RAWINPUTHEADER {
    pub dwType: DWORD,
    pub dwSize: DWORD,
    pub hDevice: HANDLE,
    pub wParam: WPARAM,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RAWKEYBOARD {
    pub MakeCode: WORD,
    pub Flags: WORD,
    pub Reserved: WORD,
    pub VKey: WORD,
    pub Message: UINT,
    pub ExtraInformation: ULONG,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RAWINPUTKEYBOARD {
    pub header: RAWINPUTHEADER,
    pub keyboard: RAWKEYBOARD,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
    pub lPrivate: DWORD,
}

#[repr(C)]
pub struct WNDCLASSW {
    pub style: UINT,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: PCWSTR,
    pub lpszClassName: PCWSTR,
}

#[repr(C)]
pub struct CREATESTRUCTW {
    pub lpCreateParams: *mut c_void,
    pub hInstance: HINSTANCE,
    pub hMenu: HMENU,
    pub hwndParent: HWND,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: LONG,
    pub lpszName: PCWSTR,
    pub lpszClass: PCWSTR,
    pub dwExStyle: DWORD,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RGBQUAD {
    pub rgbBlue: u8,
    pub rgbGreen: u8,
    pub rgbRed: u8,
    pub rgbReserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BITMAPINFOHEADER {
    pub biSize: DWORD,
    pub biWidth: LONG,
    pub biHeight: LONG,
    pub biPlanes: WORD,
    pub biBitCount: WORD,
    pub biCompression: DWORD,
    pub biSizeImage: DWORD,
    pub biXPelsPerMeter: LONG,
    pub biYPelsPerMeter: LONG,
    pub biClrUsed: DWORD,
    pub biClrImportant: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [RGBQUAD; 1],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BLENDFUNCTION {
    pub BlendOp: u8,
    pub BlendFlags: u8,
    pub SourceConstantAlpha: u8,
    pub AlphaFormat: u8,
}

#[repr(C)]
pub struct GUITHREADINFO {
    pub cbSize: DWORD,
    pub flags: DWORD,
    pub hwndActive: HWND,
    pub hwndFocus: HWND,
    pub hwndCapture: HWND,
    pub hwndMenuOwner: HWND,
    pub hwndMoveSize: HWND,
    pub hwndCaret: HWND,
    pub rcCaret: RECT,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VARIANT_RECORD {
    pub pvRecord: *mut c_void,
    pub pRecInfo: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union VARIANT_DATA {
    pub ll_val: i64,
    pub l_val: i32,
    pub bool_val: i16,
    pub pointer: *mut c_void,
    pub record: VARIANT_RECORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VARIANT {
    pub vt: u16,
    pub reserved1: u16,
    pub reserved2: u16,
    pub reserved3: u16,
    pub data: VARIANT_DATA,
}

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: DWORD,
    pub hWnd: HWND,
    pub uID: UINT,
    pub uFlags: UINT,
    pub uCallbackMessage: UINT,
    pub hIcon: HICON,
    pub szTip: [u16; 128],
    pub dwState: DWORD,
    pub dwStateMask: DWORD,
    pub szInfo: [u16; 256],
    pub uTimeoutOrVersion: UINT,
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: DWORD,
    pub guidItem: GUID,
    pub hBalloonIcon: HICON,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NOTIFYICONIDENTIFIER {
    pub cbSize: DWORD,
    pub hWnd: HWND,
    pub uID: UINT,
    pub guidItem: GUID,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MONITORINFO {
    pub cbSize: DWORD,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TPMPARAMS {
    pub cbSize: UINT,
    pub rcExclude: RECT,
}

// Window messages.
pub const WM_NULL: UINT = 0x0000;
pub const WM_CREATE: UINT = 0x0001;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_QUERYENDSESSION: UINT = 0x0011;
pub const WM_ENDSESSION: UINT = 0x0016;
pub const WM_SETTINGCHANGE: UINT = 0x001A;
pub const WM_PAINT: UINT = 0x000F;
pub const WM_SETFONT: UINT = 0x0030;
pub const WM_GETOBJECT: UINT = 0x003D;
pub const WM_CONTEXTMENU: UINT = 0x007B;
pub const WM_DISPLAYCHANGE: UINT = 0x007E;
pub const WM_SETICON: UINT = 0x0080;
pub const WM_NCCREATE: UINT = 0x0081;
pub const WM_NCDESTROY: UINT = 0x0082;
pub const WM_NCHITTEST: UINT = 0x0084;
pub const WM_INPUT: UINT = 0x00FF;
pub const WM_COMMAND: UINT = 0x0111;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_LBUTTONUP: UINT = 0x0202;
pub const WM_LBUTTONDBLCLK: UINT = 0x0203;
pub const WM_RBUTTONUP: UINT = 0x0205;
pub const WM_USER: UINT = 0x0400;
pub const WM_APP: UINT = 0x8000;
pub const WM_IME_CONTROL: UINT = 0x0283;

pub const WM_APP_TRAY: UINT = WM_APP + 1;
pub const WM_APP_SHOW_SETTINGS: UINT = WM_APP + 2;
pub const WM_APP_ACTIVITY: UINT = WM_APP + 3;

// WinEvent accessibility notifications.
pub const EVENT_SYSTEM_FOREGROUND: DWORD = 0x0003;
pub const EVENT_OBJECT_FOCUS: DWORD = 0x8005;
pub const EVENT_OBJECT_LOCATIONCHANGE: DWORD = 0x800B;
pub const EVENT_OBJECT_TEXTSELECTIONCHANGED: DWORD = 0x8014;
pub const WINEVENT_OUTOFCONTEXT: DWORD = 0x0000;
pub const WINEVENT_SKIPOWNPROCESS: DWORD = 0x0002;
pub const OBJID_CARET: LONG = -8;

// Raw Input keyboard activity keeps normal legacy keyboard messages enabled.
pub const HID_USAGE_PAGE_GENERIC: WORD = 0x01;
pub const HID_USAGE_GENERIC_KEYBOARD: WORD = 0x06;
pub const RIDEV_REMOVE: DWORD = 0x0000_0001;
pub const RIDEV_INPUTSINK: DWORD = 0x0000_0100;
pub const RID_INPUT: UINT = 0x1000_0003;
pub const RIM_TYPEKEYBOARD: DWORD = 1;
pub const RI_KEY_BREAK: WORD = 0x0001;

// Window styles.
pub const WS_OVERLAPPED: DWORD = 0x0000_0000;
pub const WS_POPUP: DWORD = 0x8000_0000;
pub const WS_CHILD: DWORD = 0x4000_0000;
pub const WS_VISIBLE: DWORD = 0x1000_0000;
pub const WS_CAPTION: DWORD = 0x00C0_0000;
pub const WS_SYSMENU: DWORD = 0x0008_0000;
pub const WS_THICKFRAME: DWORD = 0x0004_0000;
pub const WS_MINIMIZEBOX: DWORD = 0x0002_0000;
pub const WS_MAXIMIZEBOX: DWORD = 0x0001_0000;
pub const WS_TABSTOP: DWORD = 0x0001_0000;
pub const WS_BORDER: DWORD = 0x0080_0000;
pub const SS_CENTER: DWORD = 0x0000_0001;
pub const SS_CENTERIMAGE: DWORD = 0x0000_0200;
pub const WS_CLIPSIBLINGS: DWORD = 0x0400_0000;
pub const WS_CLIPCHILDREN: DWORD = 0x0200_0000;
pub const WS_OVERLAPPEDWINDOW: DWORD =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;

pub const WS_EX_DLGMODALFRAME: DWORD = 0x0000_0001;
pub const WS_EX_CONTROLPARENT: DWORD = 0x0001_0000;
pub const WS_EX_TOPMOST: DWORD = 0x0000_0008;
pub const WS_EX_TRANSPARENT: DWORD = 0x0000_0020;
pub const WS_EX_TOOLWINDOW: DWORD = 0x0000_0080;
pub const WS_EX_LAYERED: DWORD = 0x0008_0000;
pub const WS_EX_NOACTIVATE: DWORD = 0x0800_0000;

// Control styles and messages.
pub const BS_PUSHBUTTON: DWORD = 0x0000_0000;
pub const BS_DEFPUSHBUTTON: DWORD = 0x0000_0001;
pub const BS_AUTOCHECKBOX: DWORD = 0x0000_0003;
pub const BS_GROUPBOX: DWORD = 0x0000_0007;
pub const SS_LEFT: DWORD = 0x0000_0000;
pub const CBS_DROPDOWNLIST: DWORD = 0x0000_0003;
pub const CBS_HASSTRINGS: DWORD = 0x0000_0200;
pub const BM_GETCHECK: UINT = 0x00F0;
pub const BM_SETCHECK: UINT = 0x00F1;
pub const BST_UNCHECKED: WPARAM = 0;
pub const BST_CHECKED: WPARAM = 1;
pub const CB_ADDSTRING: UINT = 0x0143;
pub const CB_GETCURSEL: UINT = 0x0147;
pub const CB_SETCURSEL: UINT = 0x014E;
pub const CB_ERR: LRESULT = -1;
pub const BN_CLICKED: u16 = 0;
pub const IDOK: u16 = 1;
pub const IDCANCEL: u16 = 2;
pub const ES_READONLY: DWORD = 0x0000_0800;
pub const ES_UPPERCASE: DWORD = 0x0000_0008;
pub const ES_AUTOHSCROLL: DWORD = 0x0000_0080;
pub const EM_SETLIMITTEXT: UINT = 0x00C5;
pub const SCI_GETREADONLY: UINT = 2140;

// Get/SetWindowLongPtr.
pub const GWL_STYLE: i32 = -16;
pub const GWLP_USERDATA: i32 = -21;

// ShowWindow and SetWindowPos.
pub const SW_HIDE: i32 = 0;
pub const SW_SHOW: i32 = 5;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SW_SHOWNORMAL: i32 = 1;
pub const SWP_NOSIZE: UINT = 0x0001;
pub const SWP_NOMOVE: UINT = 0x0002;
pub const SWP_NOZORDER: UINT = 0x0004;
pub const SWP_NOACTIVATE: UINT = 0x0010;
pub const SWP_SHOWWINDOW: UINT = 0x0040;
pub const HWND_TOPMOST: HWND = -1isize as HWND;
pub const HWND_NOTOPMOST: HWND = -2isize as HWND;
pub const CW_USEDEFAULT: i32 = i32::MIN;

// Window hierarchy and hit testing.
pub const GA_ROOT: UINT = 2;
pub const GA_ROOTOWNER: UINT = 3;
pub const HTTRANSPARENT: LRESULT = -1;

// IME commands and SendMessageTimeout flags.
pub const IMC_GETCONVERSIONMODE: WPARAM = 0x0001;
pub const IMC_GETOPENSTATUS: WPARAM = 0x0005;
pub const SMTO_BLOCK: UINT = 0x0001;
pub const SMTO_ABORTIFHUNG: UINT = 0x0002;

// Keyboard state.
pub const VK_SHIFT: i32 = 0x10;
pub const VK_CONTROL: i32 = 0x11;
pub const VK_MENU: i32 = 0x12;
pub const VK_CAPITAL: i32 = 0x14;
pub const VK_LWIN: i32 = 0x5b;
pub const VK_RWIN: i32 = 0x5c;

// Cursor used only by this application's own windows.
pub const IDC_ARROW: usize = 32512;

// Tray icon constants.
pub const NIM_ADD: DWORD = 0x0000_0000;
pub const NIM_DELETE: DWORD = 0x0000_0002;
pub const NIM_SETFOCUS: DWORD = 0x0000_0003;
pub const NIM_SETVERSION: DWORD = 0x0000_0004;
pub const NIF_MESSAGE: UINT = 0x0000_0001;
pub const NIF_ICON: UINT = 0x0000_0002;
pub const NIF_TIP: UINT = 0x0000_0004;
pub const NIF_SHOWTIP: UINT = 0x0000_0080;
pub const NOTIFYICON_VERSION_4: UINT = 4;
pub const NIN_SELECT: UINT = WM_USER;
pub const NIN_KEYSELECT: UINT = WM_USER + 1;

// Menu constants.
pub const MF_STRING: UINT = 0x0000_0000;
pub const MF_CHECKED: UINT = 0x0000_0008;
pub const MF_UNCHECKED: UINT = 0x0000_0000;
pub const MF_SEPARATOR: UINT = 0x0000_0800;
pub const TPM_LEFTALIGN: UINT = 0x0000;
pub const TPM_RIGHTALIGN: UINT = 0x0008;
pub const TPM_TOPALIGN: UINT = 0x0000;
pub const TPM_BOTTOMALIGN: UINT = 0x0020;
pub const TPM_VERTICAL: UINT = 0x0040;
pub const TPM_RIGHTBUTTON: UINT = 0x0002;
pub const TPM_RETURNCMD: UINT = 0x0100;
pub const TPM_NONOTIFY: UINT = 0x0080;

// Painting and layered window constants.
pub const COLOR_WINDOW: usize = 5;
pub const DEFAULT_GUI_FONT: i32 = 17;
pub const TRANSPARENT: i32 = 1;
pub const OPAQUE: i32 = 2;
pub const DT_CENTER: UINT = 0x0000_0001;
pub const DT_VCENTER: UINT = 0x0000_0004;
pub const DT_SINGLELINE: UINT = 0x0000_0020;
pub const DT_CALCRECT: UINT = 0x0000_0400;
pub const LWA_ALPHA: DWORD = 0x0000_0002;
pub const ULW_ALPHA: DWORD = 0x0000_0002;
pub const AC_SRC_OVER: u8 = 0x00;
pub const AC_SRC_ALPHA: u8 = 0x01;
pub const BI_RGB: DWORD = 0;
pub const DIB_RGB_COLORS: UINT = 0;
pub const ANTIALIASED_QUALITY: DWORD = 4;

// System metrics.
pub const SM_XVIRTUALSCREEN: i32 = 76;
pub const SM_YVIRTUALSCREEN: i32 = 77;
pub const SM_CXVIRTUALSCREEN: i32 = 78;
pub const SM_CYVIRTUALSCREEN: i32 = 79;
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

// Monitor constants.
pub const MONITOR_DEFAULTTONEAREST: DWORD = 0x0000_0002;
pub const MDT_EFFECTIVE_DPI: i32 = 0;
pub const DWMWA_EXTENDED_FRAME_BOUNDS: DWORD = 9;

// HRESULT values.
pub const S_OK: HRESULT = 0;
pub const S_FALSE: HRESULT = 1;
pub const COINIT_APARTMENTTHREADED: DWORD = 0x0000_0002;
pub const CLSCTX_INPROC_SERVER: DWORD = 0x0000_0001;
pub const VT_EMPTY: u16 = 0;
pub const VT_I4: u16 = 3;
pub const VT_BSTR: u16 = 8;
pub const VT_DISPATCH: u16 = 9;
pub const VT_UNKNOWN: u16 = 13;
pub const GCS_COMPSTR: DWORD = 0x0008;
pub const GCS_CURSORPOS: DWORD = 0x0080;
pub const VT_BOOL: u16 = 11;
pub const VARIANT_FALSE: i16 = 0;

// Console constants.
pub const GENERIC_READ: DWORD = 0x8000_0000;
pub const FILE_SHARE_READ: DWORD = 0x0000_0001;
pub const FILE_SHARE_WRITE: DWORD = 0x0000_0002;
pub const OPEN_EXISTING: DWORD = 3;
pub const INVALID_HANDLE_VALUE: HANDLE = (-1isize) as HANDLE;
pub const TH32CS_SNAPPROCESS: DWORD = 0x0000_0002;

// Sound flags.
pub const SND_ASYNC: DWORD = 0x0001;
pub const SND_NODEFAULT: DWORD = 0x0002;
pub const SND_FILENAME: DWORD = 0x0002_0000;

// MessageBox and icon constants.
pub const MB_OK: UINT = 0x0000_0000;
pub const MB_ICONERROR: UINT = 0x0000_0010;
pub const MB_ICONINFORMATION: UINT = 0x0000_0040;
pub const ICON_SMALL: WPARAM = 0;
pub const ICON_BIG: WPARAM = 1;
pub const IDI_APPLICATION: usize = 32512;

pub const ERROR_ALREADY_EXISTS: DWORD = 183;

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(lpModuleName: PCWSTR) -> HMODULE;
    pub fn CreateMutexW(
        lpMutexAttributes: *const c_void,
        bInitialOwner: BOOL,
        lpName: PCWSTR,
    ) -> HANDLE;
    pub fn GetLastError() -> DWORD;
    pub fn GetCurrentProcessId() -> DWORD;
    pub fn ProcessIdToSessionId(dwProcessId: DWORD, pSessionId: *mut DWORD) -> BOOL;
    pub fn CreateToolhelp32Snapshot(dwFlags: DWORD, th32ProcessID: DWORD) -> HANDLE;
    pub fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    pub fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn AttachConsole(dwProcessId: DWORD) -> BOOL;
    pub fn FreeConsole() -> BOOL;
    pub fn GetConsoleWindow() -> HWND;
    pub fn CreateFileW(
        lpFileName: PCWSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: *const c_void,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn GetConsoleScreenBufferInfo(
        hConsoleOutput: HANDLE,
        lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
    ) -> BOOL;
    pub fn GetCurrentConsoleFontEx(
        hConsoleOutput: HANDLE,
        bMaximumWindow: BOOL,
        lpConsoleCurrentFontEx: *mut CONSOLE_FONT_INFOEX,
    ) -> BOOL;
}

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> ATOM;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: PCWSTR,
        lpWindowName: PCWSTR,
        dwStyle: DWORD,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *const c_void,
    ) -> HWND;
    pub fn AdjustWindowRectEx(
        lpRect: *mut RECT,
        dwStyle: DWORD,
        bMenu: BOOL,
        dwExStyle: DWORD,
    ) -> BOOL;
    pub fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;
    pub fn FindWindowW(lpClassName: PCWSTR, lpWindowName: PCWSTR) -> HWND;
    pub fn EnumWindows(lpEnumFunc: WNDENUMPROC, lParam: LPARAM) -> BOOL;
    pub fn EnumChildWindows(hWndParent: HWND, lpEnumFunc: WNDENUMPROC, lParam: LPARAM) -> BOOL;
    pub fn MessageBoxW(hWnd: HWND, lpText: PCWSTR, lpCaption: PCWSTR, uType: UINT) -> i32;

    pub fn GetMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
    ) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn IsDialogMessageW(hDlg: HWND, lpMsg: *mut MSG) -> BOOL;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn SendMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn SendMessageTimeoutW(
        hWnd: HWND,
        Msg: UINT,
        wParam: WPARAM,
        lParam: LPARAM,
        fuFlags: UINT,
        uTimeout: UINT,
        lpdwResult: *mut usize,
    ) -> LRESULT;

    pub fn SetTimer(hWnd: HWND, nIDEvent: usize, uElapse: UINT, lpTimerFunc: TIMERPROC) -> usize;
    pub fn KillTimer(hWnd: HWND, uIDEvent: usize) -> BOOL;
    pub fn SetWinEventHook(
        eventMin: DWORD,
        eventMax: DWORD,
        hmodWinEventProc: HMODULE,
        pfnWinEventProc: WINEVENTPROC,
        idProcess: DWORD,
        idThread: DWORD,
        dwFlags: DWORD,
    ) -> HWINEVENTHOOK;
    pub fn UnhookWinEvent(hWinEventHook: HWINEVENTHOOK) -> BOOL;
    pub fn RegisterRawInputDevices(
        pRawInputDevices: *const RAWINPUTDEVICE,
        uiNumDevices: UINT,
        cbSize: UINT,
    ) -> BOOL;
    pub fn GetRawInputData(
        hRawInput: HRAWINPUT,
        uiCommand: UINT,
        pData: *mut c_void,
        pcbSize: *mut UINT,
        cbSizeHeader: UINT,
    ) -> UINT;

    #[cfg(target_pointer_width = "64")]
    pub fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> isize;
    #[cfg(target_pointer_width = "64")]
    pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
    #[cfg(target_pointer_width = "32")]
    pub fn GetWindowLongW(hWnd: HWND, nIndex: i32) -> LONG;
    #[cfg(target_pointer_width = "32")]
    pub fn SetWindowLongW(hWnd: HWND, nIndex: i32, dwNewLong: LONG) -> LONG;

    pub fn GetForegroundWindow() -> HWND;
    pub fn GetGUIThreadInfo(idThread: DWORD, pgui: *mut GUITHREADINFO) -> BOOL;
    pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut DWORD) -> DWORD;
    pub fn GetParent(hWnd: HWND) -> HWND;
    pub fn GetAncestor(hWnd: HWND, gaFlags: UINT) -> HWND;
    pub fn IsWindow(hWnd: HWND) -> BOOL;
    pub fn IsWindowEnabled(hWnd: HWND) -> BOOL;
    pub fn GetClassNameW(hWnd: HWND, lpClassName: PWSTR, nMaxCount: i32) -> i32;
    pub fn GetWindowTextW(hWnd: HWND, lpString: PWSTR, nMaxCount: i32) -> i32;

    pub fn MonitorFromPoint(pt: POINT, dwFlags: DWORD) -> HMONITOR;
    pub fn MonitorFromWindow(hwnd: HWND, dwFlags: DWORD) -> HMONITOR;
    pub fn GetMonitorInfoW(hMonitor: HMONITOR, lpmi: *mut MONITORINFO) -> BOOL;
    pub fn ClientToScreen(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: PCWSTR) -> HCURSOR;
    pub fn LoadIconW(hInstance: HINSTANCE, lpIconName: PCWSTR) -> HICON;

    pub fn GetKeyboardLayout(idThread: DWORD) -> HKL;
    pub fn GetKeyState(nVirtKey: i32) -> i16;
    pub fn GetAsyncKeyState(vKey: i32) -> i16;

    pub fn RegisterWindowMessageW(lpString: PCWSTR) -> UINT;
    pub fn CreateIconFromResourceEx(
        presbits: *const u8,
        dwResSize: DWORD,
        fIcon: BOOL,
        dwVer: DWORD,
        cxDesired: i32,
        cyDesired: i32,
        uFlags: UINT,
    ) -> HICON;
    pub fn DestroyIcon(hIcon: HICON) -> BOOL;

    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(hMenu: HMENU, uFlags: UINT, uIDNewItem: usize, lpNewItem: PCWSTR) -> BOOL;
    pub fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: UINT,
        x: i32,
        y: i32,
        nReserved: i32,
        hWnd: HWND,
        prcRect: *const RECT,
    ) -> BOOL;
    pub fn TrackPopupMenuEx(
        hMenu: HMENU,
        uFlags: UINT,
        x: i32,
        y: i32,
        hWnd: HWND,
        lptpm: *const TPMPARAMS,
    ) -> BOOL;
    pub fn DestroyMenu(hMenu: HMENU) -> BOOL;

    pub fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: UINT,
    ) -> BOOL;
    pub fn SetLayeredWindowAttributes(
        hwnd: HWND,
        crKey: COLORREF,
        bAlpha: u8,
        dwFlags: DWORD,
    ) -> BOOL;
    pub fn UpdateLayeredWindow(
        hwnd: HWND,
        hdcDst: HDC,
        pptDst: *const POINT,
        psize: *const SIZE,
        hdcSrc: HDC,
        pptSrc: *const POINT,
        crKey: COLORREF,
        pblend: *const BLENDFUNCTION,
        dwFlags: DWORD,
    ) -> BOOL;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetDC(hWnd: HWND) -> HDC;
    pub fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
    pub fn FillRect(hDC: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn DrawTextW(hdc: HDC, lpchText: PWSTR, cchText: i32, lprc: *mut RECT, format: UINT)
        -> i32;
    pub fn GetSystemMetrics(nIndex: i32) -> i32;
    pub fn GetDlgItem(hDlg: HWND, nIDDlgItem: i32) -> HWND;
    pub fn SetWindowTextW(hWnd: HWND, lpString: PCWSTR) -> BOOL;
    pub fn GetDpiForWindow(hwnd: HWND) -> UINT;
    pub fn SetProcessDpiAwarenessContext(value: DPI_AWARENESS_CONTEXT) -> BOOL;
    pub fn SetProcessDPIAware() -> BOOL;
}

#[link(name = "shcore")]
extern "system" {
    pub fn GetDpiForMonitor(
        hmonitor: HMONITOR,
        dpi_type: i32,
        dpi_x: *mut UINT,
        dpi_y: *mut UINT,
    ) -> HRESULT;
}

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmGetWindowAttribute(
        hwnd: HWND,
        dwAttribute: DWORD,
        pvAttribute: *mut c_void,
        cbAttribute: DWORD,
    ) -> HRESULT;
}

#[link(name = "oleacc")]
extern "system" {
    pub fn AccessibleObjectFromWindow(
        hwnd: HWND,
        dwId: DWORD,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT;
}

#[link(name = "ole32")]
extern "system" {
    pub fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: DWORD) -> HRESULT;
    pub fn CoUninitialize();
    pub fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut c_void,
        dwClsContext: DWORD,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT;
}

#[link(name = "oleaut32")]
extern "system" {
    pub fn VariantClear(pvarg: *mut VARIANT) -> HRESULT;
    pub fn SysFreeString(bstrString: *mut u16);
    pub fn SysStringLen(pbstr: *const u16) -> UINT;
    pub fn SafeArrayGetDim(psa: *mut SAFEARRAY) -> UINT;
    pub fn SafeArrayGetLBound(psa: *mut SAFEARRAY, nDim: UINT, plLbound: *mut LONG) -> HRESULT;
    pub fn SafeArrayGetUBound(psa: *mut SAFEARRAY, nDim: UINT, plUbound: *mut LONG) -> HRESULT;
    pub fn SafeArrayAccessData(psa: *mut SAFEARRAY, ppvData: *mut *mut c_void) -> HRESULT;
    pub fn SafeArrayUnaccessData(psa: *mut SAFEARRAY) -> HRESULT;
    pub fn SafeArrayDestroy(psa: *mut SAFEARRAY) -> HRESULT;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn GetStockObject(i: i32) -> HGDIOBJ;
    pub fn CreateFontW(
        cHeight: i32,
        cWidth: i32,
        cEscapement: i32,
        cOrientation: i32,
        cWeight: i32,
        bItalic: DWORD,
        bUnderline: DWORD,
        bStrikeOut: DWORD,
        iCharSet: DWORD,
        iOutPrecision: DWORD,
        iClipPrecision: DWORD,
        iQuality: DWORD,
        iPitchAndFamily: DWORD,
        pszFaceName: PCWSTR,
    ) -> HFONT;
    pub fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn CreateDIBSection(
        hdc: HDC,
        pbmi: *const BITMAPINFO,
        usage: UINT,
        bits: *mut *mut c_void,
        section: HANDLE,
        offset: DWORD,
    ) -> HBITMAP;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn SetBkColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
}

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(dwMessage: DWORD, lpData: *mut NOTIFYICONDATAW) -> BOOL;
    pub fn Shell_NotifyIconGetRect(
        identifier: *const NOTIFYICONIDENTIFIER,
        iconLocation: *mut RECT,
    ) -> HRESULT;
}

#[link(name = "imm32")]
extern "system" {
    pub fn ImmGetDefaultIMEWnd(hWnd: HWND) -> HWND;
    pub fn ImmGetContext(hWnd: HWND) -> HANDLE;
    pub fn ImmReleaseContext(hWnd: HWND, hIMC: HANDLE) -> BOOL;
    pub fn ImmGetOpenStatus(hIMC: HANDLE) -> BOOL;
    pub fn ImmGetConversionStatus(
        hIMC: HANDLE,
        lpfdwConversion: *mut DWORD,
        lpfdwSentence: *mut DWORD,
    ) -> BOOL;
    pub fn ImmGetCompositionStringW(
        hIMC: HANDLE,
        dwIndex: DWORD,
        lpBuf: *mut c_void,
        dwBufLen: DWORD,
    ) -> LONG;
}

#[link(name = "winmm")]
extern "system" {
    pub fn PlaySoundW(pszSound: PCWSTR, hmod: HMODULE, fdwSound: DWORD) -> BOOL;
}

#[inline]
pub unsafe fn get_window_long_ptr(hwnd: HWND, index: i32) -> isize {
    #[cfg(target_pointer_width = "64")]
    {
        GetWindowLongPtrW(hwnd, index)
    }
    #[cfg(target_pointer_width = "32")]
    {
        GetWindowLongW(hwnd, index) as isize
    }
}

#[inline]
pub unsafe fn set_window_long_ptr(hwnd: HWND, index: i32, value: isize) -> isize {
    #[cfg(target_pointer_width = "64")]
    {
        SetWindowLongPtrW(hwnd, index, value)
    }
    #[cfg(target_pointer_width = "32")]
    {
        SetWindowLongW(hwnd, index, value as LONG) as isize
    }
}

#[inline]
pub const fn make_int_resource(id: usize) -> PCWSTR {
    id as PCWSTR
}

#[inline]
pub const fn loword(value: usize) -> u16 {
    (value & 0xffff) as u16
}

#[inline]
pub const fn hiword(value: usize) -> u16 {
    ((value >> 16) & 0xffff) as u16
}

#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

#[inline]
pub const fn control_id(id: u16) -> HMENU {
    id as usize as HMENU
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn win32_layouts_match_windows_abi() {
        assert_eq!(CB_GETCURSEL, 0x0147);
        assert_eq!(NIF_SHOWTIP, 0x80);
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(size_of::<GUITHREADINFO>(), 72);
            assert_eq!(size_of::<NOTIFYICONDATAW>(), 976);
            assert_eq!(size_of::<NOTIFYICONIDENTIFIER>(), 40);
            assert_eq!(size_of::<MONITORINFO>(), 40);
            assert_eq!(size_of::<TPMPARAMS>(), 20);
            assert_eq!(size_of::<RAWINPUTDEVICE>(), 16);
            assert_eq!(size_of::<RAWINPUTHEADER>(), 24);
            assert_eq!(size_of::<RAWKEYBOARD>(), 16);
            assert_eq!(size_of::<RAWINPUTKEYBOARD>(), 40);
            assert_eq!(size_of::<WNDCLASSW>(), 72);
            assert_eq!(size_of::<MSG>(), 48);
            assert_eq!(size_of::<VARIANT>(), 24);
            assert_eq!(size_of::<CONSOLE_SCREEN_BUFFER_INFO>(), 22);
            assert_eq!(size_of::<CONSOLE_FONT_INFOEX>(), 84);
            assert_eq!(size_of::<PROCESSENTRY32W>(), 568);
        }
        #[cfg(target_pointer_width = "32")]
        {
            assert_eq!(size_of::<GUITHREADINFO>(), 48);
            assert_eq!(size_of::<NOTIFYICONDATAW>(), 956);
            assert_eq!(size_of::<NOTIFYICONIDENTIFIER>(), 28);
            assert_eq!(size_of::<MONITORINFO>(), 40);
            assert_eq!(size_of::<TPMPARAMS>(), 20);
            assert_eq!(size_of::<RAWINPUTDEVICE>(), 12);
            assert_eq!(size_of::<RAWINPUTHEADER>(), 16);
            assert_eq!(size_of::<RAWKEYBOARD>(), 16);
            assert_eq!(size_of::<RAWINPUTKEYBOARD>(), 32);
            assert_eq!(size_of::<WNDCLASSW>(), 40);
            assert_eq!(size_of::<MSG>(), 32);
            assert_eq!(size_of::<VARIANT>(), 16);
            assert_eq!(size_of::<CONSOLE_SCREEN_BUFFER_INFO>(), 22);
            assert_eq!(size_of::<CONSOLE_FONT_INFOEX>(), 84);
            assert_eq!(size_of::<PROCESSENTRY32W>(), 556);
        }
    }
}
