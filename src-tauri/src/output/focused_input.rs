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
    confirmation: InsertionConfirmation,
) -> DestinationDecision {
    if status == crate::output::InsertStatus::Inserted
        && confirmation != InsertionConfirmation::NotInserted
    {
        DestinationDecision::Insert
    } else {
        DestinationDecision::Popup("output_failed")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertionConfirmation {
    Confirmed,
    NotInserted,
    // Unsupported AX attributes, focus changes and read errors are not proof of failure.
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct InsertionProbe {
    process_id: u32,
    element_id: u64,
    selection: Option<(usize, usize)>,
    before_value: Option<String>,
    before_range: Option<String>,
}

fn normalized_lines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

// AX selection offsets count UTF-16 units, not bytes or Unicode scalar values.
fn expected_value(before: &str, selection: (usize, usize), text: &str) -> Option<String> {
    let units: Vec<u16> = before.encode_utf16().collect();
    let (start, length) = selection;
    let end = start.checked_add(length)?;
    let prefix = String::from_utf16(units.get(..start)?).ok()?;
    let suffix = String::from_utf16(units.get(end..)?).ok()?;
    Some(format!("{prefix}{text}{suffix}"))
}

fn assess_readback(
    probe: &InsertionProbe,
    text: &str,
    after_value: Option<&str>,
    after_range: Option<&str>,
) -> InsertionConfirmation {
    use InsertionConfirmation::*;
    let expected = probe
        .before_value
        .as_deref()
        .zip(probe.selection)
        .and_then(|(before, selection)| expected_value(before, selection, text));
    if let (Some(expected), Some(after)) = (expected.as_deref(), after_value) {
        if normalized_lines(expected) == normalized_lines(after) {
            return Confirmed;
        }
    }
    // A matching range can be pre-existing text. An unchanged, readable field
    // must not hide a failed insertion, unless the requested replacement is identical.
    if let (Some(before), Some(after)) = (probe.before_value.as_deref(), after_value) {
        if normalized_lines(before) == normalized_lines(after) {
            return NotInserted;
        }
    }
    if let Some(after) = after_range {
        if normalized_lines(after) == normalized_lines(text) {
            return if probe
                .before_range
                .as_deref()
                .is_some_and(|before| normalized_lines(before) == normalized_lines(after))
            {
                Unavailable
            } else {
                Confirmed
            };
        }
        return NotInserted;
    }
    if expected.is_some() && after_value.is_some() {
        NotInserted
    } else {
        Unavailable
    }
}

// Read again while slow editors process queued input. Never dispatch the text again.
pub async fn wait_for_insertion_confirmation(
    mut read: impl FnMut() -> InsertionConfirmation,
) -> InsertionConfirmation {
    let mut latest = InsertionConfirmation::Unavailable;
    for delay_ms in [0, 40, 80, 160, 320, 400] {
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        latest = read();
        if latest == InsertionConfirmation::Confirmed {
            break;
        }
    }
    // Use the final observation: an earlier stale value cannot prove failure
    // if the editor subsequently replaced its accessibility element.
    latest
}

pub fn begin_insertion_probe(focus: FocusedInput, text: &str) -> Option<InsertionProbe> {
    #[cfg(target_os = "macos")]
    {
        macos::begin_probe(focus, text)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (focus, text);
        None
    }
}

pub fn confirm_inserted_text(probe: &InsertionProbe, text: &str) -> InsertionConfirmation {
    #[cfg(target_os = "macos")]
    {
        macos::confirm(probe, text)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (probe, text);
        InsertionConfirmation::Unavailable
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
    use super::{
        assess_readback, classify_focused_element, FocusKind, FocusedInput, InsertionConfirmation,
        InsertionProbe,
    };
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
        fn CFStringGetLength(value: CFRef) -> isize;
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
        let length = unsafe { CFStringGetLength(value) };
        if !(0..=1_048_576).contains(&length) {
            return None;
        }
        let mut bytes = vec![0_i8; length as usize * 4 + 1];
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

    pub(super) fn begin_probe(focus: FocusedInput, text: &str) -> Option<InsertionProbe> {
        if focus.kind != FocusKind::Editable {
            return None;
        }
        let pid = focus.process_id?;
        let (_, element) = focused_element(pid)?;
        let id = unsafe { CFHash(element.0) } as u64;
        if Some(id) != focus.element_id {
            return None;
        }
        let selection =
            selected_range(element.0).map(|range| (range.location as usize, range.length as usize));
        let before_value = attribute(element.0, b"AXValue\0")
            .ok()
            .and_then(|value| string(value.0));
        let before_range = selection.and_then(|(location, _)| {
            range_string(element.0, location, text.encode_utf16().count())
        });
        // Without a readable selection the destination is still trusted; only
        // post-insertion verification is less capable.
        Some(InsertionProbe {
            process_id: pid,
            element_id: id,
            selection,
            before_value,
            before_range,
        })
    }

    fn range_string(element: CFRef, location: usize, length: usize) -> Option<String> {
        if length == 0 || length > 1_048_576 {
            return None;
        }
        let range = CFRange {
            location: isize::try_from(location).ok()?,
            length: length as isize,
        };
        let parameter = unsafe { AXValueCreate(4, (&range as *const CFRange).cast()) };
        if parameter.is_null() {
            return None;
        }
        let parameter = OwnedCF(parameter);
        let name = attribute_name(b"AXStringForRange\0")?;
        let mut value = std::ptr::null();
        let status = unsafe {
            AXUIElementCopyParameterizedAttributeValue(element, name.0, parameter.0, &mut value)
        };
        if status != 0 || value.is_null() {
            if !value.is_null() {
                unsafe { CFRelease(value) };
            }
            return None;
        }
        let value = OwnedCF(value);
        string(value.0)
    }

    pub(super) fn confirm(probe: &InsertionProbe, text: &str) -> InsertionConfirmation {
        let Some((_, element)) = focused_element(probe.process_id) else {
            return InsertionConfirmation::Unavailable;
        };
        if unsafe { CFHash(element.0) } as u64 != probe.element_id {
            return InsertionConfirmation::Unavailable;
        }
        let after_value = attribute(element.0, b"AXValue\0")
            .ok()
            .and_then(|value| string(value.0));
        let after_range = probe.selection.and_then(|(location, _)| {
            range_string(element.0, location, text.encode_utf16().count())
        });
        assess_readback(probe, text, after_value.as_deref(), after_range.as_deref())
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

    fn probe(before: Option<&str>, selection: Option<(usize, usize)>) -> InsertionProbe {
        InsertionProbe {
            process_id: 42,
            element_id: 7,
            selection,
            before_value: before.map(str::to_string),
            before_range: None,
        }
    }

    #[test]
    fn full_value_confirms_when_parameterized_range_is_unsupported() {
        let probe = probe(Some("前后"), Some((1, 0)));
        assert_eq!(
            assess_readback(&probe, "中", Some("前中后"), None),
            InsertionConfirmation::Confirmed
        );
        assert_eq!(
            decide_after_output(
                crate::output::InsertStatus::Inserted,
                assess_readback(&probe, "中", Some("前中后"), None)
            ),
            DestinationDecision::Insert
        );
    }

    #[test]
    fn replacement_uses_utf16_offsets_and_preserves_multiline_text() {
        let probe = probe(Some("前🎙️旧文后"), Some((4, 2)));
        assert_eq!(
            assess_readback(&probe, "新\n文", Some("前🎙️新\r\n文后"), None),
            InsertionConfirmation::Confirmed
        );
        assert_eq!(expected_value("😀后", (1, 0), "x"), None);
        assert_eq!(expected_value("短", (20, 0), "x"), None);
        assert_eq!(expected_value("短", (1, usize::MAX), "x"), None);
    }

    #[test]
    fn unavailable_attributes_do_not_prevent_successful_dispatch() {
        for probe in [
            probe(None, None),
            probe(None, Some((0, 0))),
            probe(Some(""), Some((0, 0))),
        ] {
            let confirmation = assess_readback(&probe, "完整文字", None, None);
            assert_eq!(confirmation, InsertionConfirmation::Unavailable);
            assert_eq!(
                decide_after_output(crate::output::InsertStatus::Inserted, confirmation),
                DestinationDecision::Insert
            );
        }
    }

    #[test]
    fn readable_unchanged_partial_and_incorrect_output_remain_recoverable() {
        let probe = probe(Some(""), Some((0, 0)));
        for after in ["", "完整", "错误文字", " 完整文字"] {
            assert_eq!(
                assess_readback(&probe, "完整文字", Some(after), None),
                InsertionConfirmation::NotInserted
            );
        }
        assert_eq!(
            assess_readback(&probe, "完整文字", None, Some("完整")),
            InsertionConfirmation::NotInserted
        );
    }

    #[test]
    fn range_readback_supports_editors_without_a_full_value() {
        let probe = probe(None, Some((8, 0)));
        assert_eq!(
            assess_readback(&probe, "两\n行", None, Some("两\r行")),
            InsertionConfirmation::Confirmed
        );
    }

    #[test]
    fn preexisting_matching_text_does_not_prove_a_new_insertion() {
        let mut probe = probe(Some("重复文字"), Some((0, 0)));
        probe.before_range = Some("重复文字".into());
        assert_eq!(
            assess_readback(&probe, "重复文字", Some("重复文字"), Some("重复文字")),
            InsertionConfirmation::NotInserted
        );
        probe.before_value = None;
        assert_eq!(
            assess_readback(&probe, "重复文字", None, Some("重复文字")),
            InsertionConfirmation::Unavailable
        );
    }

    #[test]
    fn identical_replacement_is_already_the_intended_result() {
        let probe = probe(Some("前文字后"), Some((1, 2)));
        assert_eq!(
            assess_readback(&probe, "文字", Some("前文字后"), Some("文字")),
            InsertionConfirmation::Confirmed
        );
    }

    #[test]
    fn long_output_can_be_confirmed_without_the_old_16k_limit() {
        let text = "中文😀\n".repeat(5000);
        assert_eq!(
            assess_readback(&probe(Some(""), Some((0, 0))), &text, Some(&text), None),
            InsertionConfirmation::Confirmed
        );
    }

    #[tokio::test(start_paused = true)]
    async fn delayed_editor_success_is_rechecked_without_reinsertion() {
        use InsertionConfirmation::*;
        let mut reads = [NotInserted, NotInserted, Unavailable, Confirmed].into_iter();
        let start = tokio::time::Instant::now();
        let confirmation =
            wait_for_insertion_confirmation(|| reads.next().expect("stop on success")).await;
        assert_eq!(confirmation, Confirmed);
        assert_eq!(start.elapsed(), std::time::Duration::from_millis(280));
        assert_eq!(
            decide_after_output(crate::output::InsertStatus::Inserted, confirmation),
            DestinationDecision::Insert
        );
    }

    #[tokio::test(start_paused = true)]
    async fn only_a_final_readable_failure_triggers_recovery() {
        use InsertionConfirmation::*;
        assert_eq!(
            wait_for_insertion_confirmation(|| NotInserted).await,
            NotInserted
        );
        assert_eq!(
            wait_for_insertion_confirmation(|| Unavailable).await,
            Unavailable
        );
        let mut reads = [
            NotInserted,
            NotInserted,
            Unavailable,
            Unavailable,
            Unavailable,
            Unavailable,
        ]
        .into_iter();
        assert_eq!(
            wait_for_insertion_confirmation(|| reads.next().unwrap()).await,
            Unavailable
        );
    }

    #[test]
    fn unreadable_text_after_successful_dispatch_is_not_a_failed_insertion() {
        assert_eq!(
            decide_after_output(
                crate::output::InsertStatus::Inserted,
                InsertionConfirmation::Unavailable
            ),
            DestinationDecision::Insert
        );
    }

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
    fn observed_failure_and_transport_failure_preserve_full_result() {
        use crate::output::InsertStatus;
        assert_eq!(
            decide_after_output(InsertStatus::Inserted, InsertionConfirmation::NotInserted),
            DestinationDecision::Popup("output_failed")
        );
        assert_eq!(
            decide_after_output(
                InsertStatus::CopiedFallback,
                InsertionConfirmation::Confirmed
            ),
            DestinationDecision::Popup("output_failed")
        );
        assert_eq!(
            decide_after_output(InsertStatus::Failed, InsertionConfirmation::Unavailable),
            DestinationDecision::Popup("output_failed")
        );
        assert_eq!(
            decide_after_output(InsertStatus::Inserted, InsertionConfirmation::Confirmed),
            DestinationDecision::Insert
        );
    }
}
