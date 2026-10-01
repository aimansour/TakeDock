//! Development-only verifier. Never installs an artifact or creates an app window.
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tauri_plugin_updater::UpdaterExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = std::fs::read(std::env::args().nth(1).ok_or("artifact path required")?)?;
    let signature =
        std::fs::read_to_string(std::env::args().nth(2).ok_or("signature path required")?)?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let base = format!("http://{}", listener.local_addr()?);
    let endpoint = format!("{base}/latest.json");
    let mode = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let server_mode = mode.clone();
    let server_stop = stop.clone();
    let expected = artifact.clone();
    let server = std::thread::spawn(move || {
        while !server_stop.load(Ordering::Acquire) {
            let Ok((mut socket, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0u8; 4096];
            let length = socket.read(&mut request).unwrap();
            let current = server_mode.load(Ordering::Acquire);
            let body = if String::from_utf8_lossy(&request[..length])
                .starts_with("GET /latest.json")
            {
                let version = match current {
                    2 => "1000.0.0",
                    3 => "0.1.0",
                    4 => "invalid",
                    _ => "999.0.0",
                };
                serde_json::to_vec(&serde_json::json!({"version":version,"notes":"Signature verification fixture","platforms":{"windows-x86_64":{"url":format!("{base}/artifact.exe"),"signature":signature.trim()}}})).unwrap()
            } else {
                let mut bytes = artifact.clone();
                if current == 1 {
                    bytes[0] ^= 1;
                }
                bytes
            };
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(header.as_bytes());
            let _ = socket.write_all(&body);
        }
    });
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    // Loopback HTTP is confined to this unbundled example, never the application config.
    context.config_mut().plugins.0.get_mut("updater").unwrap()["dangerousInsecureTransportProtocol"] =
        serde_json::json!(true);
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)?;
    let result = tauri::async_runtime::block_on(async {
        let updater = app
            .updater_builder()
            .endpoints(vec![endpoint.parse()?])?
            .timeout(Duration::from_secs(5))
            .build()?;
        let update = updater.check().await?.ok_or("expected update")?;
        assert_eq!(update.download(|_, _| {}, || {}).await?, expected);
        mode.store(1, Ordering::Release);
        assert!(
            update.download(|_, _| {}, || {}).await.is_err(),
            "corrupt artifact accepted"
        );
        mode.store(2, Ordering::Release);
        assert!(
            updater
                .check()
                .await?
                .unwrap()
                .download(|_, _| {}, || {})
                .await
                .is_err(),
            "signed version mismatch accepted"
        );
        mode.store(3, Ordering::Release);
        assert!(updater.check().await?.is_none(), "downgrade offered");
        mode.store(4, Ordering::Release);
        assert!(updater.check().await.is_err(), "invalid version accepted");
        Ok::<(), Box<dyn std::error::Error>>(())
    });
    stop.store(true, Ordering::Release);
    server.join().unwrap();
    result?;
    println!(
        "PASS: signed download, corrupt rejection, signed-version binding, downgrade and malformed-manifest rejection; no installation invoked."
    );
    Ok(())
}
