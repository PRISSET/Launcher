use iced::{window, widget::image};
use std::time::Duration;
use crate::app::state::{
    ServerStatus, UpdateResult, CURRENT_VERSION, GITHUB_RELEASES_API, INSTALLER_NAME
};

pub fn load_gif_frames() -> Vec<image::Handle> {
    use ::image::codecs::gif::GifDecoder;
    use ::image::AnimationDecoder;
    
    let gif_data = include_bytes!("../background.gif");
    let cursor = std::io::Cursor::new(gif_data.as_slice());
    
    if let Ok(decoder) = GifDecoder::new(cursor) {
        decoder.into_frames()
            .filter_map(|f| f.ok())
            .map(|frame| {
                let rgba = frame.into_buffer();
                let (w, h) = rgba.dimensions();
                image::Handle::from_rgba(w, h, rgba.into_raw())
            })
            .collect()
    } else {
        vec![image::Handle::from_bytes(include_bytes!("../../background.png").to_vec())]
    }
}

pub fn load_avatar_frames() -> Vec<image::Handle> {
    use ::image::codecs::gif::GifDecoder;
    use ::image::AnimationDecoder;
    
    let gif_data = include_bytes!("../avatar.gif");
    let cursor = std::io::Cursor::new(gif_data.as_slice());
    
    if let Ok(decoder) = GifDecoder::new(cursor) {
        decoder.into_frames()
            .filter_map(|f| f.ok())
            .map(|frame| {
                let rgba = frame.into_buffer();
                let (w, h) = rgba.dimensions();
                image::Handle::from_rgba(w, h, rgba.into_raw())
            })
            .collect()
    } else {
        vec![image::Handle::from_bytes(include_bytes!("../icon.png").to_vec())]
    }
}

pub fn load_icon() -> Option<window::Icon> {
    let icon_data = include_bytes!("../icon.png");
    let img = ::image::load_from_memory(icon_data).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    window::icon::from_rgba(img.into_raw(), width, height).ok()
}


pub async fn check_for_updates() -> UpdateResult {
    let client = reqwest::Client::new();
    
    let response = match client
        .get(GITHUB_RELEASES_API)
        .header("User-Agent", "ByStep-Launcher")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    if !response.status().is_success() {
        return UpdateResult::NoUpdate;
    }
    
    let release: serde_json::Value = match response.json().await {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    let latest_version = release.get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v');
    
    if latest_version.is_empty() || latest_version == CURRENT_VERSION {
        return UpdateResult::NoUpdate;
    }
    
    if let Some(assets) = release.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name == INSTALLER_NAME {
                if let Some(url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                    return UpdateResult::UpdateAvailable(
                        latest_version.to_string(),
                        url.to_string()
                    );
                }
            }
        }
    }
    
    UpdateResult::NoUpdate
}

pub async fn download_and_run_update(url: String) -> UpdateResult {
    let client = reqwest::Client::new();
    
    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    if !response.status().is_success() {
        return UpdateResult::Error("Не удалось скачать обновление".to_string());
    }
    
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => return UpdateResult::Error(e.to_string()),
    };
    
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join(INSTALLER_NAME);
    
    if let Err(e) = std::fs::write(&installer_path, &bytes) {
        return UpdateResult::Error(e.to_string());
    }
    
    UpdateResult::Downloaded(installer_path)
}

pub async fn fetch_server_status() -> ServerStatus {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    
    let mut status = ServerStatus::default();
    
    let stream = match TcpStream::connect_timeout(
        &"144.31.169.7:25565".parse().unwrap(),
        Duration::from_secs(5)
    ) {
        Ok(s) => s,
        Err(_) => return status,
    };
    
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    
    let mut stream = stream;
    
    let mut handshake = Vec::new();
    handshake.push(0x00);
    write_varint(&mut handshake, 767);
    write_string(&mut handshake, "144.31.169.7");
    handshake.extend_from_slice(&25565u16.to_be_bytes());
    write_varint(&mut handshake, 1);
    
    let mut packet = Vec::new();
    write_varint(&mut packet, handshake.len() as i32);
    packet.extend(handshake);
    
    if stream.write_all(&packet).is_err() {
        return status;
    }
    
    let status_request = vec![0x01, 0x00];
    if stream.write_all(&status_request).is_err() {
        return status;
    }
    
    let mut length_buf = [0u8; 5];
    let mut length_bytes = 0;
    for i in 0..5 {
        if stream.read_exact(&mut length_buf[i..i+1]).is_err() {
            return status;
        }
        length_bytes += 1;
        if length_buf[i] & 0x80 == 0 {
            break;
        }
    }
    
    let (packet_length, _) = read_varint(&length_buf[..length_bytes]);
    if packet_length <= 0 || packet_length > 65535 {
        return status;
    }
    
    let mut response_data = vec![0u8; packet_length as usize];
    if stream.read_exact(&mut response_data).is_err() {
        return status;
    }
    
    let (_, id_len) = read_varint(&response_data);
    let (json_len, json_len_size) = read_varint(&response_data[id_len..]);
    let json_start = id_len + json_len_size;
    let json_end = json_start + json_len as usize;
    
    if json_end > response_data.len() {
        return status;
    }
    
    let json_str = match std::str::from_utf8(&response_data[json_start..json_end]) {
        Ok(s) => s,
        Err(_) => return status,
    };
    
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
        status.online = true;
        
        if let Some(players) = json.get("players") {
            status.players_online = players.get("online").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            status.players_max = players.get("max").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            
            if let Some(sample) = players.get("sample").and_then(|v| v.as_array()) {
                status.player_names = sample.iter()
                    .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect();
            }
        }
    }
    
    status
}

fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_varint(buf, s.len() as i32);
    buf.extend_from_slice(s.as_bytes());
}

fn read_varint(data: &[u8]) -> (i32, usize) {
    let mut result = 0i32;
    let mut shift = 0;
    let mut bytes_read = 0;
    
    for &byte in data {
        bytes_read += 1;
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    
    (result, bytes_read)
}
