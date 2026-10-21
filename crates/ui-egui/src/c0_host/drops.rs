//! Bound browser drop reads before allocation; keep the upstream path at standalone defaults.

use crate::{Inbox, InboxEvent};
use std::sync::atomic::{AtomicUsize, Ordering};

static RESERVED: AtomicUsize = AtomicUsize::new(0);

pub struct FileReadBudget {
    bytes: usize,
}
impl FileReadBudget {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}
impl Drop for FileReadBudget {
    fn drop(&mut self) {
        RESERVED.fetch_sub(self.bytes, Ordering::SeqCst);
    }
}

pub fn reserve_file_bytes(bytes: usize, held: usize) -> Result<FileReadBudget, String> {
    let mut current = RESERVED.load(Ordering::SeqCst);
    loop {
        let next = current
            .checked_add(bytes)
            .filter(|n| n.checked_add(held).is_some_and(|total| total <= super::MAX_FILE_BYTES))
            .ok_or("files exceed the host inbox limit")?;
        match RESERVED.compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return Ok(FileReadBudget { bytes }),
            Err(observed) => current = observed,
        }
    }
}

pub async fn read_drop(file: &egui::DroppedFileHandle, inbox: &Inbox, bounded: bool) -> Result<Vec<u8>, String> {
    if !bounded {
        return file.bytes_async().await;
    }
    let browser_file = file.web_file().ok_or("dropped file has no browser size metadata")?;
    super::validate_name(&browser_file.name())?;
    let size = browser_file.size();
    if !size.is_finite() || size < 0.0 || size.fract() != 0.0 || size > super::MAX_FILE_BYTES as f64 {
        return Err("dropped file exceeds the host file limit".into());
    }
    // Size is now a finite, nonnegative integer below 72 MiB, fitting wasm32's usize.
    let bytes = size as usize;
    let held = inbox
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|event| match event {
            InboxEvent::File(file) => file.bytes.len(),
            _ => 0,
        })
        .fold(0, usize::saturating_add);
    let reservation = reserve_file_bytes(bytes, held)?;
    let data = file.bytes_async().await?;
    if data.len() > reservation.bytes {
        return Err("dropped file changed size while reading".into());
    }
    Ok(data)
}
