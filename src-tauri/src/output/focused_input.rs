#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusKind {
    Editable,
    NotEditable,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusedInput {
    pub kind: FocusKind,
    pub process_id: Option<u32>,
    pub element_id: Option<u64>,
}

impl Default for FocusedInput {
    fn default() -> Self {
        Self {
            kind: FocusKind::Unknown,
            process_id: None,
            element_id: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationDecision {
    Insert,
    Popup(&'static str),
}

pub fn classify_focused_element(
    role: Option<&str>,
    subrole: Option<&str>,
    enabled: Option<bool>,
    settable: Option<bool>,
) -> FocusKind {
    if subrole.is_some_and(|value| value.contains("Secure")) || enabled == Some(false) {
        return FocusKind::NotEditable;
    }
    match role {
        Some("AXTextField" | "AXTextArea" | "AXComboBox" | "AXTextView" | "AXTextEntryArea") => {
            match (enabled, settable) {
                (Some(true), Some(true)) => FocusKind::Editable,
                (_, Some(false)) => FocusKind::NotEditable,
                _ => FocusKind::Unknown,
            }
        }
        Some(_) => FocusKind::NotEditable,
        None => FocusKind::Unknown,
    }
}

pub fn decide_destination(
    start: FocusedInput,
    current: FocusedInput,
    same_app: bool,
) -> DestinationDecision {
    match start.kind {
        FocusKind::NotEditable => return DestinationDecision::Popup("no_target"),
        FocusKind::Unknown => return DestinationDecision::Popup("unknown_target"),
        FocusKind::Editable => {}
    }
    if !same_app || start.process_id != current.process_id {
        return DestinationDecision::Popup("target_changed");
    }
    match current.kind {
        FocusKind::Unknown => DestinationDecision::Popup("unknown_target"),
        FocusKind::NotEditable => DestinationDecision::Popup("target_changed"),
        FocusKind::Editable => match (start.element_id, current.element_id) {
            (Some(expected), Some(actual)) if expected == actual => DestinationDecision::Insert,
            (Some(_), Some(_)) => DestinationDecision::Popup("target_changed"),
            _ => DestinationDecision::Popup("unknown_target"),
        },
    }
}

pub fn decide_after_output(
    status: crate::output::InsertStatus,
    confirmed_text: bool,
) -> DestinationDecision {
    if status == crate::output::InsertStatus::Inserted && confirmed_text {
        DestinationDecision::Insert
    } else {
        DestinationDecision::Popup("output_failed")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InsertionProbe {
    process_id: u32,
    element_id: u64,
    location: isize,
}

pub fn begin_insertion_probe(focus: FocusedInput) -> Option<InsertionProbe> {
    #[cfg(target_os = "macos")]
    {
        macos::begin_probe(focus)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = focus;
        None
    }
}

pub fn confirm_inserted_text(probe: InsertionProbe, text: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::confirm(probe, text)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (probe, text);
        false
    }
}

pub fn capture_focused_input(process_id: Option<u32>) -> FocusedInput {
    #[cfg(target_os = "macos")]
    {
        macos::capture(process_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = process_id;
        FocusedInput::default()
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{classify_focused_element, FocusKind, FocusedInput, InsertionProbe};
    use std::ffi::c_void;

    type CFRef = *const c_void;
    const UTF8: u32 = 0x0800_0100;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
        fn AXUIElementCreateApplication(pid: i32) -> CFRef;
        fn AXUIElementSetMessagingTimeout(element: CFRef, timeout: f32) -> i32;
        fn AXUIElementCopyAttributeValue(
            element: CFRef,
            attribute: CFRef,
            value: *mut CFRef,
        ) -> i32;
        fn AXUIElementIsAttributeSettable(
            element: CFRef,
            attribute: CFRef,
            settable: *mut u8,
        ) -> i32;
        fn AXUIElementCopyParameterizedAttributeValue(
            element: CFRef,
            attribute: CFRef,
            parameter: CFRef,
            value: *mut CFRef,
        ) -> i32;
        fn AXValueCreate(value_type: u32, value: *const c_void) -> CFRef;
        fn AXValueGetType(value: CFRef) -> u32;
        fn AXValueGetValue(value: CFRef, value_type: u32, result: *mut c_void) -> u8;
        fn CFStringCreateWithCString(allocator: CFRef, text: *const i8, encoding: u32) -> CFRef;
        fn CFStringGetCString(value: CFRef, buffer: *mut i8, size: isize, encoding: u32) -> u8;
        fn CFStringGetTypeID() -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFBooleanGetValue(value: CFRef) -> u8;
        fn CFGetTypeID(value: CFRef) -> usize;
        fn CFHash(value: CFRef) -> usize;
        fn CFRelease(value: CFRef);
    }

    struct OwnedCF(CFRef);
    impl Drop for OwnedCF {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }

    fn attribute_name(name: &'static [u8]) -> Option<OwnedCF> {
        let value =
            unsafe { CFStringCreateWithCString(std::ptr::null(), name.as_ptr().cast(), UTF8) };
        (!value.is_null()).then_some(OwnedCF(value))
    }

    fn attribute(element: CFRef, name: &'static [u8]) -> Result<OwnedCF, i32> {
        let name = attribute_name(name).ok_or(-1)?;
        let mut value = std::ptr::null();
        let status = unsafe { AXUIElementCopyAttributeValue(element, name.0, &mut value) };
        if status == 0 && !value.is_null() {
            Ok(OwnedCF(value))
        } else {
            if !value.is_null() {
                unsafe { CFRelease(value) };
            }
            Err(status)
        }
    }

    fn string(value: CFRef) -> Option<String> {
        if unsafe { CFGetTypeID(value) } != unsafe { CFStringGetTypeID() } {
            return None;
        }
        let mut bytes = [0_i8; 256];
        let ok =
            unsafe { CFStringGetCString(value, bytes.as_mut_ptr(), bytes.len() as isize, UTF8) };
        if ok == 0 {
            return None;
        }
        Some(
            unsafe { std::ffi::CStr::from_ptr(bytes.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    }

    fn bool_value(value: CFRef) -> Option<bool> {
        (unsafe { CFGetTypeID(value) } == unsafe { CFBooleanGetTypeID() })
            .then(|| unsafe { CFBooleanGetValue(value) != 0 })
    }

    fn is_settable(element: CFRef, name: &'static [u8]) -> Option<bool> {
        let name = attribute_name(name)?;
        let mut settable = 0_u8;
        (unsafe { AXUIElementIsAttributeSettable(element, name.0, &mut settable) } == 0)
            .then_some(settable != 0)
    }

    #[repr(C)]
    #[derive(Default)]
    struct CFRange {
        location: isize,
        length: isize,
    }

    fn focused_element(pid: u32) -> Option<(OwnedCF, OwnedCF)> {
        if unsafe { AXIsProcessTrusted() } == 0 {
            return None;
        }
        let application = unsafe { AXUIElementCreateApplication(pid as i32) };
        if application.is_null() {
            return None;
        }
        let application = OwnedCF(application);
        unsafe { AXUIElementSetMessagingTimeout(application.0, 0.25) };
        let element = attribute(application.0, b"AXFocusedUIElement\0").ok()?;
        unsafe { AXUIElementSetMessagingTimeout(element.0, 0.25) };
        Some((application, element))
    }

    fn selected_range(element: CFRef) -> Option<CFRange> {
        let value = attribute(element, b"AXSelectedTextRange\0").ok()?;
        if unsafe { AXValueGetType(value.0) } != 4 {
            return None;
        }
        let mut range = CFRange::default();
        let ok = unsafe { AXValueGetValue(value.0, 4, (&mut range as *mut CFRange).cast()) };
        (ok != 0 && range.location >= 0 && range.length >= 0).then_some(range)
    }

    pub(super) fn begin_probe(focus: FocusedInput) -> Option<InsertionProbe> {
        if focus.kind != FocusKind::Editable {
            return None;
        }
        let pid = focus.process_id?;
        let (_, element) = focused_element(pid)?;
        let id = unsafe { CFHash(element.0) } as u64;
        if Some(id) != focus.element_id {
            return None;
        }
        let range = selected_range(element.0)?;
        Some(InsertionProbe {
            process_id: pid,
            element_id: id,
            location: range.location,
        })
    }

    pub(super) fn confirm(probe: InsertionProbe, text: &str) -> bool {
        let utf16_len = text.encode_utf16().count();
        if utf16_len == 0 || utf16_len > 16_384 {
            return false;
        }
        let Some((_, element)) = focused_element(probe.process_id) else {
            return false;
        };
        if unsafe { CFHash(element.0) } as u64 != probe.element_id {
            return false;
        }
        let range = CFRange {
            location: probe.location,
            length: utf16_len as isize,
        };
        let parameter = unsafe { AXValueCreate(4, (&range as *const CFRange).cast()) };
        if parameter.is_null() {
            return false;
        }
        let parameter = OwnedCF(parameter);
        let Some(name) = attribute_name(b"AXStringForRange\0") else {
            return false;
        };
        let mut value = std::ptr::null();
        let status = unsafe {
            AXUIElementCopyParameterizedAttributeValue(element.0, name.0, parameter.0, &mut value)
        };
        if status != 0 || value.is_null() {
            if !value.is_null() {
                unsafe { CFRelease(value) };
            }
            return false;
        }
        let value = OwnedCF(value);
        if unsafe { CFGetTypeID(value.0) } != unsafe { CFStringGetTypeID() } {
            return false;
        }
        let mut bytes = vec![0_i8; utf16_len.saturating_mul(4).saturating_add(1)];
        let ok =
            unsafe { CFStringGetCString(value.0, bytes.as_mut_ptr(), bytes.len() as isize, UTF8) };
        ok != 0 && unsafe { std::ffi::CStr::from_ptr(bytes.as_ptr()) }.to_bytes() == text.as_bytes()
    }

    pub(super) fn capture(process_id: Option<u32>) -> FocusedInput {
        let mut snapshot = FocusedInput {
            process_id,
            ..FocusedInput::default()
        };
        let Some(pid) = process_id else {
            return snapshot;
        };
        if pid == std::process::id() {
            snapshot.kind = FocusKind::NotEditable;
            return snapshot;
        }
        let Some((_, element)) = focused_element(pid) else {
            return snapshot;
        };
        let role = attribute(element.0, b"AXRole\0")
            .ok()
            .and_then(|value| string(value.0));
        let subrole = attribute(element.0, b"AXSubrole\0")
            .ok()
            .and_then(|value| string(value.0));
        let enabled = attribute(element.0, b"AXEnabled\0")
            .ok()
            .and_then(|value| bool_value(value.0));
        let settable = is_settable(element.0, b"AXValue\0");
        snapshot.kind =
            classify_focused_element(role.as_deref(), subrole.as_deref(), enabled, settable);
        snapshot.element_id = Some(unsafe { CFHash(element.0) } as u64);
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focus(kind: FocusKind, id: Option<u64>) -> FocusedInput {
        FocusedInput {
            kind,
            process_id: Some(42),
            element_id: id,
        }
    }

    #[test]
    fn writable_field_in_same_app_and_element_allows_insertion() {
        assert_eq!(
            decide_destination(
                focus(FocusKind::Editable, Some(7)),
                focus(FocusKind::Editable, Some(7)),
                true
            ),
            DestinationDecision::Insert
        );
    }

    #[test]
    fn no_field_and_unknown_permission_go_to_popup() {
        assert_eq!(
            decide_destination(
                focus(FocusKind::NotEditable, None),
                focus(FocusKind::NotEditable, None),
                true
            ),
            DestinationDecision::Popup("no_target")
        );
        assert_eq!(
            decide_destination(
                focus(FocusKind::Unknown, None),
                focus(FocusKind::Unknown, None),
                true
            ),
            DestinationDecision::Popup("unknown_target")
        );
    }

    #[test]
    fn read_only_and_secure_elements_are_not_editable() {
        assert_eq!(
            classify_focused_element(Some("AXTextField"), None, Some(true), Some(false)),
            FocusKind::NotEditable
        );
        assert_eq!(
            classify_focused_element(
                Some("AXTextField"),
                Some("AXSecureTextField"),
                Some(true),
                Some(true)
            ),
            FocusKind::NotEditable
        );
    }

    #[test]
    fn changed_input_within_same_app_goes_to_popup() {
        assert_eq!(
            decide_destination(
                focus(FocusKind::Editable, Some(7)),
                focus(FocusKind::Editable, Some(8)),
                true
            ),
            DestinationDecision::Popup("target_changed")
        );
        assert_eq!(
            decide_destination(
                focus(FocusKind::Editable, Some(7)),
                focus(FocusKind::Editable, Some(7)),
                false
            ),
            DestinationDecision::Popup("target_changed")
        );
    }

    #[test]
    fn dispatch_without_confirmed_text_and_failed_insertion_preserve_full_result() {
        use crate::output::InsertStatus;
        assert_eq!(
            decide_after_output(InsertStatus::Inserted, false),
            DestinationDecision::Popup("output_failed")
        );
        assert_eq!(
            decide_after_output(InsertStatus::CopiedFallback, true),
            DestinationDecision::Popup("output_failed")
        );
        assert_eq!(
            decide_after_output(InsertStatus::Failed, false),
            DestinationDecision::Popup("output_failed")
        );
        assert_eq!(
            decide_after_output(InsertStatus::Inserted, true),
            DestinationDecision::Insert
        );
    }
}
