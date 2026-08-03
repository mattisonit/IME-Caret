use crate::win::*;
use std::ffi::c_void;
use std::mem::{transmute, zeroed};
use std::ptr::null_mut;

const IID_NULL: GUID = GUID {
    Data1: 0,
    Data2: 0,
    Data3: 0,
    Data4: [0; 8],
};
const IID_IDISPATCH: GUID = GUID {
    Data1: 0x0002_0400,
    Data2: 0,
    Data3: 0,
    Data4: [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const DISPATCH_METHOD: u16 = 0x1;
const DISPATCH_PROPERTYGET: u16 = 0x2;
const OUTLOOK_MAIL_ITEM_CLASS: i32 = 43;
const WINDOW_TITLE_CAPACITY: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutlookEditorState {
    Editable,
    ReadOnly,
    Unknown,
}

#[repr(C)]
struct DispatchParams {
    arguments: *mut VARIANT,
    named_arguments: *mut LONG,
    argument_count: UINT,
    named_argument_count: UINT,
}

struct ComPointer(*mut c_void);

impl ComPointer {
    fn as_ptr(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for ComPointer {
    fn drop(&mut self) {
        unsafe {
            release_com(self.0);
        }
    }
}

struct OwnedVariant(VARIANT);

impl OwnedVariant {
    unsafe fn dispatch(&self) -> Option<*mut c_void> {
        if matches!(self.0.vt, VT_DISPATCH | VT_UNKNOWN) {
            let pointer = self.0.data.pointer;
            (!pointer.is_null()).then_some(pointer)
        } else {
            None
        }
    }

    unsafe fn boolean(&self) -> Option<bool> {
        (self.0.vt == VT_BOOL).then(|| self.0.data.bool_val != 0)
    }

    unsafe fn integer(&self) -> Option<i32> {
        (self.0.vt == VT_I4).then(|| self.0.data.l_val)
    }

    unsafe fn string(&self) -> Option<String> {
        if self.0.vt != VT_BSTR {
            return None;
        }
        let bstr = self.0.data.pointer as *const u16;
        if bstr.is_null() {
            return Some(String::new());
        }
        let length = SysStringLen(bstr) as usize;
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(
            bstr, length,
        )))
    }
}

impl Drop for OwnedVariant {
    fn drop(&mut self) {
        unsafe {
            VariantClear(&mut self.0);
        }
    }
}

/// Uses Outlook's own item state instead of transient Word-editor caret
/// details. Sent mail is a viewer, unsent mail is a composer, and an Explorer
/// is editable only while it owns an active inline response.
pub unsafe fn editor_state(foreground: HWND) -> OutlookEditorState {
    if foreground.is_null() {
        return OutlookEditorState::Unknown;
    }
    let Some(foreground_title) = window_title(foreground) else {
        return OutlookEditorState::Unknown;
    };
    let Some(application) = outlook_application_dispatch() else {
        return OutlookEditorState::Unknown;
    };

    if let Some(inspector_value) = dispatch_invoke(
        application.as_ptr(),
        "ActiveInspector",
        DISPATCH_METHOD | DISPATCH_PROPERTYGET,
    ) {
        if let Some(inspector) = inspector_value.dispatch() {
            if dispatch_caption_matches(inspector, &foreground_title) {
                return inspector_editor_state(inspector);
            }
        }
    }

    if let Some(explorer_value) = dispatch_invoke(
        application.as_ptr(),
        "ActiveExplorer",
        DISPATCH_METHOD | DISPATCH_PROPERTYGET,
    ) {
        if let Some(explorer) = explorer_value.dispatch() {
            if dispatch_caption_matches(explorer, &foreground_title) {
                let Some(inline_response) = dispatch_invoke(
                    explorer,
                    "ActiveInlineResponse",
                    DISPATCH_PROPERTYGET,
                ) else {
                    return OutlookEditorState::Unknown;
                };
                return if inline_response.dispatch().is_some() {
                    OutlookEditorState::Editable
                } else {
                    OutlookEditorState::ReadOnly
                };
            }
        }
    }

    OutlookEditorState::Unknown
}

unsafe fn inspector_editor_state(inspector: *mut c_void) -> OutlookEditorState {
    let Some(item_value) = dispatch_invoke(inspector, "CurrentItem", DISPATCH_PROPERTYGET) else {
        return OutlookEditorState::Unknown;
    };
    let Some(item) = item_value.dispatch() else {
        return OutlookEditorState::Unknown;
    };
    let item_class = dispatch_invoke(item, "Class", DISPATCH_PROPERTYGET)
        .and_then(|value| value.integer());
    if item_class != Some(OUTLOOK_MAIL_ITEM_CLASS) {
        return OutlookEditorState::Unknown;
    }
    match dispatch_invoke(item, "Sent", DISPATCH_PROPERTYGET).and_then(|value| value.boolean()) {
        Some(false) => OutlookEditorState::Editable,
        Some(true) => OutlookEditorState::ReadOnly,
        None => OutlookEditorState::Unknown,
    }
}

unsafe fn dispatch_caption_matches(dispatch: *mut c_void, foreground_title: &str) -> bool {
    dispatch_invoke(dispatch, "Caption", DISPATCH_PROPERTYGET)
        .and_then(|value| value.string())
        .is_some_and(|caption| caption.trim() == foreground_title.trim())
}

unsafe fn outlook_application_dispatch() -> Option<ComPointer> {
    let program_id: Vec<u16> = "Outlook.Application"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut class_id = GUID::default();
    if CLSIDFromProgID(program_id.as_ptr(), &mut class_id) < 0 {
        return None;
    }

    let mut unknown = null_mut();
    if GetActiveObject(&class_id, null_mut(), &mut unknown) < 0 || unknown.is_null() {
        return None;
    }
    let mut dispatch = null_mut();
    let query_result = query_interface(unknown, &IID_IDISPATCH, &mut dispatch);
    release_com(unknown);
    if query_result < 0 || dispatch.is_null() {
        return None;
    }
    Some(ComPointer(dispatch))
}

unsafe fn dispatch_invoke(
    dispatch: *mut c_void,
    name: &str,
    flags: u16,
) -> Option<OwnedVariant> {
    if dispatch.is_null() {
        return None;
    }
    type GetIdsOfNames = unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        *mut PWSTR,
        UINT,
        DWORD,
        *mut LONG,
    ) -> HRESULT;
    type Invoke = unsafe extern "system" fn(
        *mut c_void,
        LONG,
        *const GUID,
        DWORD,
        u16,
        *mut DispatchParams,
        *mut VARIANT,
        *mut c_void,
        *mut UINT,
    ) -> HRESULT;

    let get_ids: GetIdsOfNames = transmute(com_method_address(dispatch, 5)?);
    let invoke: Invoke = transmute(com_method_address(dispatch, 6)?);
    let mut wide_name: Vec<u16> = name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut name_pointer = wide_name.as_mut_ptr();
    let mut dispatch_id = 0;
    if get_ids(
        dispatch,
        &IID_NULL,
        &mut name_pointer,
        1,
        0,
        &mut dispatch_id,
    ) < 0
    {
        return None;
    }

    let mut params = DispatchParams {
        arguments: null_mut(),
        named_arguments: null_mut(),
        argument_count: 0,
        named_argument_count: 0,
    };
    let mut result: VARIANT = zeroed();
    if invoke(
        dispatch,
        dispatch_id,
        &IID_NULL,
        0,
        flags,
        &mut params,
        &mut result,
        null_mut(),
        null_mut(),
    ) < 0
    {
        VariantClear(&mut result);
        return None;
    }
    Some(OwnedVariant(result))
}

unsafe fn window_title(window: HWND) -> Option<String> {
    let mut buffer = [0u16; WINDOW_TITLE_CAPACITY];
    let length = GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32);
    (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
}

unsafe fn query_interface(
    object: *mut c_void,
    interface_id: *const GUID,
    result: *mut *mut c_void,
) -> HRESULT {
    type QueryInterface = unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT;
    let Some(address) = com_method_address(object, 0) else {
        return -1;
    };
    let method: QueryInterface = transmute(address);
    method(object, interface_id, result)
}

unsafe fn release_com(object: *mut c_void) {
    if object.is_null() {
        return;
    }
    type Release = unsafe extern "system" fn(*mut c_void) -> ULONG;
    if let Some(address) = com_method_address(object, 2) {
        let method: Release = transmute(address);
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

#[link(name = "ole32")]
extern "system" {
    fn CLSIDFromProgID(program_id: PCWSTR, class_id: *mut GUID) -> HRESULT;
}

#[link(name = "oleaut32")]
extern "system" {
    fn GetActiveObject(
        class_id: *const GUID,
        reserved: *mut c_void,
        unknown: *mut *mut c_void,
    ) -> HRESULT;
}
