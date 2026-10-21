//! Probe sementara (hanya build debug): apakah aplikasi unpackaged boleh memakai UserNotificationListener.

use windows::core::Result;
use windows::UI::Notifications::Management::UserNotificationListener;

pub fn probe() -> Result<()> {
    let listener = UserNotificationListener::Current()?;
    let status = listener.RequestAccessAsync()?.join()?;
    eprintln!("[notif] status akses: {status:?}");
    Ok(())
}