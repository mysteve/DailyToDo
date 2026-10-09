use crate::{db::validate_id, models::*};
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

pub fn validate_metadata(image: &NoteImage) -> Result<(), String> {
    validate_id(&image.id)?;
    if image.name.is_empty() || image.name.chars().count() > 200 {
        return Err("图片名称需为 1 至 200 字".into());
    }
    if !matches!(
        image.mime.as_str(),
        "image/png" | "image/jpeg" | "image/webp"
    ) || image.width == 0
        || image.height == 0
        || image.width > 2048
        || image.height > 2048
    {
        return Err("图片格式或尺寸不正确".into());
    }
    Ok(())
}

pub fn decode(image: &NoteImage, data: &str) -> Result<Vec<u8>, String> {
    validate_metadata(image)?;
    if data.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
        return Err("单张保存的图片不能超过 2 MB".into());
    }
    let bytes = STANDARD.decode(data).map_err(|_| "图片数据不正确")?;
    let valid = match image.mime.as_str() {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    };
    if !valid || bytes.len() > MAX_IMAGE_BYTES {
        return Err("图片数据不正确或超过 2 MB".into());
    }
    Ok(bytes)
}

pub fn store(connection: &Connection, image: &NoteImage, data: &str) -> Result<(), String> {
    let bytes = decode(image, data)?;
    let payload = serde_json::to_string(image).map_err(|e| e.to_string())?;
    let existing: Option<(String, Vec<u8>)> = connection
        .query_row(
            "SELECT payload, data FROM images WHERE id = ?1",
            params![image.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((old_payload, old_bytes)) = existing {
        let old_image: NoteImage =
            serde_json::from_str(&old_payload).map_err(|_| "图片数据异常")?;
        return if old_image == *image && old_bytes == bytes {
            Ok(())
        } else {
            Err("图片标识冲突，请重新添加图片".into())
        };
    }
    connection
        .execute(
            "INSERT INTO images(id, payload, data) VALUES (?1, ?2, ?3)",
            params![image.id, payload, bytes],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn check_references(connection: &Connection, images: &[NoteImage]) -> Result<(), String> {
    for image in images {
        let payload: String = connection
            .query_row(
                "SELECT payload FROM images WHERE id = ?1",
                params![image.id],
                |row| row.get(0),
            )
            .map_err(|_| "图片不存在，请重新添加")?;
        let existing: NoteImage = serde_json::from_str(&payload).map_err(|_| "图片数据异常")?;
        if existing != *image {
            return Err("图片信息不一致，请重新添加".into());
        }
    }
    Ok(())
}

pub fn read(connection: &Connection, id: &str) -> Result<String, String> {
    validate_id(id)?;
    let (payload, bytes): (String, Vec<u8>) = connection
        .query_row(
            "SELECT payload, data FROM images WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "图片不存在")?;
    let image: NoteImage = serde_json::from_str(&payload).map_err(|_| "图片数据异常")?;
    Ok(format!(
        "data:{};base64,{}",
        image.mime,
        STANDARD.encode(bytes)
    ))
}

pub fn prune(connection: &Connection) -> Result<(), String> {
    connection.execute_batch("DELETE FROM images WHERE id NOT IN (
        SELECT json_extract(image.value, '$.id') FROM notes, json_each(notes.payload, '$.images') AS image
        UNION
        SELECT json_extract(image.value, '$.id') FROM reports,
            json_each(reports.payload, '$.sources.notes') AS note,
            json_each(note.value, '$.images') AS image
    )").map_err(|e| e.to_string())
}

pub fn export(connection: &Connection, workspace: &Workspace) -> Result<Vec<BackupImage>, String> {
    let mut unique = BTreeMap::new();
    for note in workspace.notes.iter().chain(
        workspace
            .reports
            .iter()
            .flat_map(|report| &report.sources.notes),
    ) {
        for image in &note.images {
            unique.insert(&image.id, image);
        }
    }
    unique
        .into_values()
        .map(|image| {
            check_references(connection, std::slice::from_ref(image))?;
            let bytes: Vec<u8> = connection
                .query_row(
                    "SELECT data FROM images WHERE id = ?1",
                    params![image.id],
                    |row| row.get(0),
                )
                .map_err(|_| "无法读取备份图片")?;
            Ok(BackupImage {
                image: image.clone(),
                data: STANDARD.encode(bytes),
            })
        })
        .collect()
}
