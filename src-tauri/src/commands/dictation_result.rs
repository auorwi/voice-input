use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDictationResult {
    pub session_id: String,
    pub text: String,
    pub reason: String,
}

#[derive(Default)]
pub struct PendingDictationResults(Mutex<Vec<PendingDictationResult>>);

impl PendingDictationResults {
    pub fn publish(&self, result: PendingDictationResult) {
        let mut results = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(existing) = results
            .iter_mut()
            .find(|item| item.session_id == result.session_id)
        {
            *existing = result;
        } else {
            results.push(result);
        }
    }

    pub fn list(&self) -> Vec<PendingDictationResult> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn dismiss(&self, session_id: &str) -> usize {
        let mut results = self.0.lock().unwrap_or_else(|error| error.into_inner());
        results.retain(|item| item.session_id != session_id);
        results.len()
    }

    pub fn dismiss_and_hide_if_empty(
        &self,
        session_id: &str,
        hide: impl FnOnce() -> Result<(), String>,
    ) -> Result<usize, String> {
        let mut results = self.0.lock().unwrap_or_else(|error| error.into_inner());
        results.retain(|item| item.session_id != session_id);
        if results.is_empty() {
            hide()?;
        }
        Ok(results.len())
    }

    pub fn copy_with(
        &self,
        session_id: &str,
        copy: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        let text = self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .find(|item| item.session_id == session_id)
            .map(|item| item.text.clone())
            .ok_or_else(|| "Result is no longer available".to_string())?;
        copy(&text)
    }
}

#[tauri::command]
pub fn list_pending_dictation_results(
    state: tauri::State<'_, PendingDictationResults>,
) -> Vec<PendingDictationResult> {
    state.list()
}

#[tauri::command]
pub fn copy_pending_dictation_result(
    state: tauri::State<'_, PendingDictationResults>,
    session_id: String,
) -> Result<(), String> {
    state.copy_with(&session_id, |text| {
        let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
        clipboard.set_text(text).map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn dismiss_pending_dictation_result(
    app: tauri::AppHandle,
    state: tauri::State<'_, PendingDictationResults>,
    session_id: String,
) -> Result<usize, String> {
    let window = app.get_webview_window("dictation-result");
    let remaining = state.dismiss_and_hide_if_empty(&session_id, || {
        if let Some(window) = &window {
            window.hide().map_err(|error| error.to_string())?;
        }
        Ok(())
    })?;
    let _ = app.emit("dictation-result:changed", ());
    Ok(remaining)
}

pub fn publish_result(app: &tauri::AppHandle, text: &str, reason: &str) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    app.state::<PendingDictationResults>()
        .publish(PendingDictationResult {
            session_id: session_id.clone(),
            text: text.to_string(),
            reason: reason.to_string(),
        });

    let window = match app.get_webview_window("dictation-result") {
        Some(window) => window,
        None => {
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|item| item.label == "dictation-result");
            match config {
                Some(config) => tauri::WebviewWindowBuilder::from_config(app, config)
                    .map_err(|error| error.to_string())?
                    .build()
                    .map_err(|error| error.to_string())?,
                None => return Err("Dictation result window is not configured".to_string()),
            }
        }
    };
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    let _ = app.emit("dictation-result:changed", ());
    Ok(session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(session_id: &str, text: &str) -> PendingDictationResult {
        PendingDictationResult {
            session_id: session_id.to_string(),
            text: text.to_string(),
            reason: "no_target".to_string(),
        }
    }

    #[test]
    fn late_reader_sees_all_recordings_in_order() {
        let store = PendingDictationResults::default();
        store.publish(result("one", "第一段"));
        store.publish(result("two", "第二段"));
        assert_eq!(
            store.list(),
            vec![result("one", "第一段"), result("two", "第二段")]
        );
    }

    #[test]
    fn stale_dismiss_cannot_remove_a_new_session() {
        let store = PendingDictationResults::default();
        store.publish(result("new", "保留我"));
        assert_eq!(store.dismiss("old"), 1);
        assert_eq!(store.list(), vec![result("new", "保留我")]);
    }

    #[test]
    fn failed_copy_keeps_complete_text_and_success_does_not_dismiss() {
        let store = PendingDictationResults::default();
        store.publish(result("one", "完整文字"));
        assert_eq!(
            store.copy_with("one", |_| Err("clipboard unavailable".to_string())),
            Err("clipboard unavailable".to_string())
        );
        assert_eq!(store.list(), vec![result("one", "完整文字")]);
        let mut copied = String::new();
        assert_eq!(
            store.copy_with("one", |text| {
                copied = text.to_string();
                Ok(())
            }),
            Ok(())
        );
        assert_eq!(copied, "完整文字");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn close_hides_only_when_the_authoritative_queue_is_empty() {
        let store = PendingDictationResults::default();
        store.publish(result("old", "第一段"));
        store.publish(result("new", "第二段"));
        let mut hides = 0;
        assert_eq!(
            store.dismiss_and_hide_if_empty("old", || {
                hides += 1;
                Ok(())
            }),
            Ok(1)
        );
        assert_eq!(hides, 0);
        assert_eq!(store.list(), vec![result("new", "第二段")]);
        assert_eq!(
            store.dismiss_and_hide_if_empty("new", || {
                hides += 1;
                Ok(())
            }),
            Ok(0)
        );
        assert_eq!(hides, 1);
    }
}
