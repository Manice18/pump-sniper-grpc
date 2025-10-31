pub fn parse_create_instruction(
    data: &[u8],
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    if data.len() < 8 {
        return Err("Data too short".into());
    }
    let mut offset = 8;

    if offset + 4 > data.len() {
        return Err("Cannot read name length".into());
    }
    let name_len = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;
    if offset + name_len > data.len() {
        return Err("Name data out of bounds".into());
    }
    let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
    offset += name_len;

    if offset + 4 > data.len() {
        return Err("Cannot read symbol length".into());
    }
    let symbol_len = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;
    if offset + symbol_len > data.len() {
        return Err("Symbol data out of bounds".into());
    }
    let symbol = String::from_utf8_lossy(&data[offset..offset + symbol_len]).to_string();

    Ok((name, symbol))
}
