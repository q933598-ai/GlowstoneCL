//! Local Minecraft instance content helpers: Java Edition server list management.
use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};

use crate::store;

const MAX_SERVER_LIST_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerEntry {
    pub name: String,
    pub ip: String,
    #[serde(default, rename = "acceptTextures")]
    pub accept_textures: i8,
    #[serde(default)]
    pub hidden: i8,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ServersDat {
    #[serde(default)]
    servers: Vec<ServerEntry>,
}

pub fn list_servers(instance_id: &str) -> Result<Vec<ServerEntry>, String> {
    let path = servers_path(instance_id)?;
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let compressed = std::fs::read(&path)
        .map_err(|error| format!("Не удалось прочитать server list: {error}"))?;
    if compressed.len() > MAX_SERVER_LIST_BYTES {
        return Err("Файл server list слишком большой.".to_owned());
    }

    let bytes = if compressed.starts_with(&[0x1f, 0x8b]) {
        let decoder = GzDecoder::new(compressed.as_slice());
        let mut output = Vec::new();
        decoder.take((MAX_SERVER_LIST_BYTES + 1) as u64)
            .read_to_end(&mut output)
            .map_err(|error| format!("Не удалось распаковать servers.dat: {error}"))?;
        if output.len() > MAX_SERVER_LIST_BYTES {
            return Err("Распакованный servers.dat превышает допустимый размер.".to_owned());
        }
        output
    } else {
        compressed
    };

    let mut servers: ServersDat = fastnbt::from_bytes(&bytes)
        .map_err(|error| format!("Не удалось прочитать NBT в servers.dat: {error}"))?;
    servers.servers.retain(|server| !server.ip.trim().is_empty());
    Ok(servers.servers)
}

pub fn add_server(instance_id: &str, name: &str, address: &str) -> Result<Vec<ServerEntry>, String> {
    let name = clean_server_name(name)?;
    let address = clean_server_address(address)?;
    let mut servers = list_servers(instance_id)?;
    if servers.iter().any(|server| server.ip.eq_ignore_ascii_case(&address)) {
        return Err("Этот сервер уже есть в списке.".to_owned());
    }
    servers.push(ServerEntry {
        name,
        ip: address,
        accept_textures: 1,
        hidden: 0,
    });
    save_servers(instance_id, &servers)?;
    Ok(servers)
}

pub fn remove_server(instance_id: &str, index: usize) -> Result<Vec<ServerEntry>, String> {
    let mut servers = list_servers(instance_id)?;
    if index >= servers.len() {
        return Err("Сервер уже отсутствует в списке.".to_owned());
    }
    servers.remove(index);
    save_servers(instance_id, &servers)?;
    Ok(servers)
}

fn save_servers(instance_id: &str, servers: &[ServerEntry]) -> Result<(), String> {
    if servers.len() > 512 {
        return Err("Список не может содержать больше 512 серверов.".to_owned());
    }
    let path = servers_path(instance_id)?;
    let parent = path.parent().ok_or_else(|| "Не найден каталог сборки.".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Не удалось создать каталог сборки: {error}"))?;

    let data = fastnbt::to_bytes(&ServersDat { servers: servers.to_vec() })
        .map_err(|error| format!("Не удалось создать NBT servers.dat: {error}"))?;
    if data.len() > MAX_SERVER_LIST_BYTES {
        return Err("Список серверов слишком большой.".to_owned());
    }
    let temporary = path.with_extension("dat.tmp");
    let file = std::fs::File::create(&temporary)
        .map_err(|error| format!("Не удалось создать временный servers.dat: {error}"))?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(&data)
        .map_err(|error| format!("Не удалось сжать servers.dat: {error}"))?;
    encoder.finish()
        .map_err(|error| format!("Не удалось завершить servers.dat: {error}"))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("Не удалось сохранить servers.dat: {error}"))?;
    Ok(())
}

fn servers_path(instance_id: &str) -> Result<std::path::PathBuf, String> {
    Ok(store::instance_directory(instance_id)?.join("servers.dat"))
}

fn clean_server_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 64 || value.chars().any(char::is_control) {
        return Err("Название сервера должно содержать от 1 до 64 обычных символов.".to_owned());
    }
    Ok(value.to_owned())
}

fn clean_server_address(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 255
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
        || value.contains('/') || value.contains('\\')
    {
        return Err("Укажи адрес вида play.example.net или play.example.net:25565.".to_owned());
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_name_and_address_are_validated() {
        assert_eq!(clean_server_name("  Survival  ").unwrap(), "Survival");
        assert_eq!(clean_server_address("play.example.net:25565").unwrap(), "play.example.net:25565");
        assert!(clean_server_name("  ").is_err());
        assert!(clean_server_address("bad address").is_err());
        assert!(clean_server_address("../server").is_err());
    }

    #[test]
    fn nbt_server_list_round_trips() {
        let input = ServersDat {
            servers: vec![ServerEntry {
                name: "Test".to_owned(),
                ip: "localhost:25565".to_owned(),
                accept_textures: 1,
                hidden: 0,
            }],
        };
        let bytes = fastnbt::to_bytes(&input).unwrap();
        let decoded: ServersDat = fastnbt::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.servers, input.servers);
    }
}
