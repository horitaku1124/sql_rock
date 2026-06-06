use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const CLIENT_LONG_PASSWORD: u32 = 0x0000_0001;
const CLIENT_CONNECT_WITH_DB: u32 = 0x0000_0008;
const CLIENT_LONG_FLAG: u32 = 0x0000_0004;
const CLIENT_PROTOCOL_41: u32 = 0x0000_0200;
const CLIENT_TRANSACTIONS: u32 = 0x0000_2000;
const CLIENT_SECURE_CONNECTION: u32 = 0x0000_8000;
const CLIENT_PLUGIN_AUTH: u32 = 0x0008_0000;

const COM_QUIT: u8 = 0x01;
const COM_QUERY: u8 = 0x03;

pub enum QueryResult {
    Command {
        affected_rows: u64,
        last_insert_id: u64,
    },
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
    },
}

pub struct MySqlClient {
    stream: TcpStream,
}

impl MySqlClient {
    pub fn connect(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        database: Option<&str>,
    ) -> Result<Self, String> {
        let address = (host, port)
            .to_socket_addrs()
            .map_err(|error| format!("cannot resolve {host}:{port}: {error}"))?
            .next()
            .ok_or_else(|| format!("cannot resolve {host}:{port}"))?;
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(10))
            .map_err(|error| format!("cannot connect to {host}:{port}: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|error| error.to_string())?;

        let (_, handshake_packet) = read_packet(&mut stream)?;
        let handshake = Handshake::parse(&handshake_packet)?;
        let capabilities = client_capabilities(handshake.capabilities, database.is_some());
        let auth_response =
            authentication_response(&handshake.plugin, password.as_bytes(), &handshake.scramble)?;
        let response = handshake_response(
            capabilities,
            user,
            &auth_response,
            database,
            &handshake.plugin,
        )?;
        write_packet(&mut stream, 1, &response)?;
        finish_authentication(&mut stream, password.as_bytes(), handshake.plugin)?;

        Ok(Self { stream })
    }

    pub fn query(&mut self, sql: &str) -> Result<QueryResult, String> {
        let mut payload = Vec::with_capacity(sql.len() + 1);
        payload.push(COM_QUERY);
        payload.extend_from_slice(sql.as_bytes());
        write_packet(&mut self.stream, 0, &payload)?;

        let (_, first) = read_packet(&mut self.stream)?;
        if first.is_empty() {
            return Err("empty query response".to_string());
        }
        match first[0] {
            0x00 => parse_ok_packet(&first),
            0xff => Err(parse_error_packet(&first)),
            _ => self.read_result_set(&first),
        }
    }

    fn read_result_set(&mut self, first: &[u8]) -> Result<QueryResult, String> {
        let mut position = 0;
        let column_count = read_lenenc_int(first, &mut position)? as usize;
        let mut columns = Vec::with_capacity(column_count);
        for _ in 0..column_count {
            let (_, packet) = read_packet(&mut self.stream)?;
            if packet.first() == Some(&0xff) {
                return Err(parse_error_packet(&packet));
            }
            columns.push(parse_column_name(&packet)?);
        }

        let (_, terminator) = read_packet(&mut self.stream)?;
        if !is_eof_packet(&terminator) {
            return Err("expected column-definition terminator".to_string());
        }

        let mut rows = Vec::new();
        loop {
            let (_, packet) = read_packet(&mut self.stream)?;
            if packet.first() == Some(&0xff) {
                return Err(parse_error_packet(&packet));
            }
            if is_eof_packet(&packet) {
                break;
            }
            rows.push(parse_text_row(&packet, column_count)?);
        }
        Ok(QueryResult::Rows { columns, rows })
    }
}

impl Drop for MySqlClient {
    fn drop(&mut self) {
        let _ = write_packet(&mut self.stream, 0, &[COM_QUIT]);
    }
}

struct Handshake {
    capabilities: u32,
    scramble: Vec<u8>,
    plugin: String,
}

impl Handshake {
    fn parse(packet: &[u8]) -> Result<Self, String> {
        let mut position = 0;
        if read_u8(packet, &mut position)? != 10 {
            return Err("unsupported MySQL handshake protocol".to_string());
        }
        read_null_terminated(packet, &mut position)?;
        read_u32_le(packet, &mut position)?;
        let mut scramble = read_bytes(packet, &mut position, 8)?.to_vec();
        position += 1;
        let lower = read_u16_le(packet, &mut position)? as u32;
        if position >= packet.len() {
            return Ok(Self {
                capabilities: lower,
                scramble,
                plugin: "mysql_native_password".to_string(),
            });
        }

        position += 1;
        position += 2;
        let upper = read_u16_le(packet, &mut position)? as u32;
        let capabilities = lower | (upper << 16);
        let auth_length = read_u8(packet, &mut position)? as usize;
        position += 10;
        let second_length = auth_length.saturating_sub(8).max(13);
        let available = packet.len().saturating_sub(position);
        let second = read_bytes(packet, &mut position, second_length.min(available))?;
        scramble.extend_from_slice(second);
        while scramble.last() == Some(&0) {
            scramble.pop();
        }

        let plugin = if capabilities & CLIENT_PLUGIN_AUTH != 0 && position < packet.len() {
            String::from_utf8_lossy(read_null_terminated(packet, &mut position)?).into_owned()
        } else {
            "mysql_native_password".to_string()
        };
        Ok(Self {
            capabilities,
            scramble,
            plugin,
        })
    }
}

fn client_capabilities(server: u32, with_database: bool) -> u32 {
    let mut capabilities = CLIENT_LONG_PASSWORD
        | CLIENT_LONG_FLAG
        | CLIENT_PROTOCOL_41
        | CLIENT_TRANSACTIONS
        | CLIENT_SECURE_CONNECTION
        | CLIENT_PLUGIN_AUTH;
    if with_database {
        capabilities |= CLIENT_CONNECT_WITH_DB;
    }
    capabilities & server
}

fn handshake_response(
    capabilities: u32,
    user: &str,
    auth_response: &[u8],
    database: Option<&str>,
    plugin: &str,
) -> Result<Vec<u8>, String> {
    if auth_response.len() > u8::MAX as usize {
        return Err("authentication response is too long".to_string());
    }
    let mut response = Vec::new();
    response.extend_from_slice(&capabilities.to_le_bytes());
    response.extend_from_slice(&(16 * 1024 * 1024_u32).to_le_bytes());
    response.push(45);
    response.extend_from_slice(&[0; 23]);
    response.extend_from_slice(user.as_bytes());
    response.push(0);
    response.push(auth_response.len() as u8);
    response.extend_from_slice(auth_response);
    if capabilities & CLIENT_CONNECT_WITH_DB != 0 {
        response.extend_from_slice(database.unwrap_or_default().as_bytes());
        response.push(0);
    }
    if capabilities & CLIENT_PLUGIN_AUTH != 0 {
        response.extend_from_slice(plugin.as_bytes());
        response.push(0);
    }
    Ok(response)
}

fn finish_authentication(
    stream: &mut TcpStream,
    password: &[u8],
    mut plugin: String,
) -> Result<(), String> {
    loop {
        let (sequence, packet) = read_packet(stream)?;
        if packet.is_empty() {
            return Err("empty authentication response".to_string());
        }
        match packet[0] {
            0x00 => return Ok(()),
            0xff => return Err(parse_error_packet(&packet)),
            0xfe => {
                let mut position = 1;
                plugin =
                    String::from_utf8_lossy(read_null_terminated(&packet, &mut position)?).into();
                let mut scramble = packet[position..].to_vec();
                while scramble.last() == Some(&0) {
                    scramble.pop();
                }
                let response = authentication_response(&plugin, password, &scramble)?;
                write_packet(stream, sequence.wrapping_add(1), &response)?;
            }
            0x01 if plugin == "caching_sha2_password" => match packet.get(1) {
                Some(0x03) => continue,
                Some(0x04) => {
                    return Err(
                        "caching_sha2_password requested full authentication; TLS/RSA authentication is not available in the standard-library-only client"
                            .to_string(),
                    );
                }
                _ => return Err("unsupported caching_sha2_password response".to_string()),
            },
            _ => {
                return Err(format!(
                    "unsupported authentication packet: 0x{:02x}",
                    packet[0]
                ));
            }
        }
    }
}

fn authentication_response(
    plugin: &str,
    password: &[u8],
    scramble: &[u8],
) -> Result<Vec<u8>, String> {
    if password.is_empty() {
        return Ok(Vec::new());
    }
    match plugin {
        "mysql_native_password" => {
            let stage1 = sha1(password);
            let stage2 = sha1(&stage1);
            let mut input = scramble.to_vec();
            input.extend_from_slice(&stage2);
            Ok(xor(&stage1, &sha1(&input)))
        }
        "caching_sha2_password" => {
            let stage1 = sha256(password);
            let stage2 = sha256(&stage1);
            let mut input = stage2.to_vec();
            input.extend_from_slice(scramble);
            Ok(xor(&stage1, &sha256(&input)))
        }
        other => Err(format!("unsupported authentication plugin `{other}`")),
    }
}

fn parse_ok_packet(packet: &[u8]) -> Result<QueryResult, String> {
    let mut position = 1;
    Ok(QueryResult::Command {
        affected_rows: read_lenenc_int(packet, &mut position)?,
        last_insert_id: read_lenenc_int(packet, &mut position)?,
    })
}

fn parse_error_packet(packet: &[u8]) -> String {
    if packet.len() < 3 {
        return "malformed MySQL error packet".to_string();
    }
    let code = u16::from_le_bytes([packet[1], packet[2]]);
    let message_position = if packet.get(3) == Some(&b'#') && packet.len() >= 9 {
        9
    } else {
        3
    };
    format!(
        "MySQL error {code}: {}",
        String::from_utf8_lossy(&packet[message_position..])
    )
}

fn parse_column_name(packet: &[u8]) -> Result<String, String> {
    let mut position = 0;
    for _ in 0..4 {
        read_lenenc_bytes(packet, &mut position)?;
    }
    Ok(String::from_utf8_lossy(read_lenenc_bytes(packet, &mut position)?).into_owned())
}

fn parse_text_row(packet: &[u8], column_count: usize) -> Result<Vec<Option<String>>, String> {
    let mut position = 0;
    let mut row = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        if packet.get(position) == Some(&0xfb) {
            position += 1;
            row.push(None);
        } else {
            row.push(Some(
                String::from_utf8_lossy(read_lenenc_bytes(packet, &mut position)?).into_owned(),
            ));
        }
    }
    Ok(row)
}

fn is_eof_packet(packet: &[u8]) -> bool {
    packet.first() == Some(&0xfe) && packet.len() < 9
}

fn read_packet(stream: &mut TcpStream) -> Result<(u8, Vec<u8>), String> {
    let mut header = [0_u8; 4];
    stream
        .read_exact(&mut header)
        .map_err(|error| format!("cannot read MySQL packet header: {error}"))?;
    let length = header[0] as usize | ((header[1] as usize) << 8) | ((header[2] as usize) << 16);
    let mut payload = vec![0; length];
    stream
        .read_exact(&mut payload)
        .map_err(|error| format!("cannot read MySQL packet: {error}"))?;
    Ok((header[3], payload))
}

fn write_packet(stream: &mut TcpStream, sequence: u8, payload: &[u8]) -> Result<(), String> {
    if payload.len() > 0x00ff_ffff {
        return Err("MySQL packet is too large".to_string());
    }
    let length = payload.len();
    let header = [
        length as u8,
        (length >> 8) as u8,
        (length >> 16) as u8,
        sequence,
    ];
    stream
        .write_all(&header)
        .and_then(|_| stream.write_all(payload))
        .map_err(|error| format!("cannot write MySQL packet: {error}"))
}

fn read_lenenc_int(packet: &[u8], position: &mut usize) -> Result<u64, String> {
    match read_u8(packet, position)? {
        value @ 0x00..=0xfa => Ok(value as u64),
        0xfc => Ok(read_u16_le(packet, position)? as u64),
        0xfd => {
            let bytes = read_bytes(packet, position, 3)?;
            Ok(bytes[0] as u64 | ((bytes[1] as u64) << 8) | ((bytes[2] as u64) << 16))
        }
        0xfe => {
            let bytes = read_bytes(packet, position, 8)?;
            Ok(u64::from_le_bytes(
                bytes.try_into().expect("checked length"),
            ))
        }
        _ => Err("invalid length-encoded integer".to_string()),
    }
}

fn read_lenenc_bytes<'a>(packet: &'a [u8], position: &mut usize) -> Result<&'a [u8], String> {
    let length = read_lenenc_int(packet, position)? as usize;
    read_bytes(packet, position, length)
}

fn read_u8(packet: &[u8], position: &mut usize) -> Result<u8, String> {
    let value = *packet
        .get(*position)
        .ok_or_else(|| "unexpected end of MySQL packet".to_string())?;
    *position += 1;
    Ok(value)
}

fn read_u16_le(packet: &[u8], position: &mut usize) -> Result<u16, String> {
    let bytes = read_bytes(packet, position, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_le(packet: &[u8], position: &mut usize) -> Result<u32, String> {
    let bytes = read_bytes(packet, position, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_bytes<'a>(
    packet: &'a [u8],
    position: &mut usize,
    length: usize,
) -> Result<&'a [u8], String> {
    let end = position
        .checked_add(length)
        .ok_or_else(|| "invalid MySQL packet length".to_string())?;
    let bytes = packet
        .get(*position..end)
        .ok_or_else(|| "unexpected end of MySQL packet".to_string())?;
    *position = end;
    Ok(bytes)
}

fn read_null_terminated<'a>(packet: &'a [u8], position: &mut usize) -> Result<&'a [u8], String> {
    let rest = packet
        .get(*position..)
        .ok_or_else(|| "unexpected end of MySQL packet".to_string())?;
    let length = rest
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| "unterminated MySQL string".to_string())?;
    let value = &rest[..length];
    *position += length + 1;
    Ok(value)
}

fn xor(left: &[u8], right: &[u8]) -> Vec<u8> {
    left.iter()
        .zip(right)
        .map(|(left, right)| left ^ right)
        .collect()
}

fn sha1(input: &[u8]) -> [u8; 20] {
    let mut data = pad_message(input);
    let mut state = [
        0x67452301_u32,
        0xefcdab89,
        0x98badcfe,
        0x10325476,
        0xc3d2e1f0,
    ];
    for chunk in data.chunks_exact_mut(64) {
        let mut words = [0_u32; 80];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let next = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = next;
        }
        for (value, addition) in state.iter_mut().zip([a, b, c, d, e]) {
            *value = value.wrapping_add(addition);
        }
    }
    words_to_bytes(&state)
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = pad_message(input);
    let mut state = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in data.chunks_exact_mut(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (value, addition) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *value = value.wrapping_add(addition);
        }
    }
    words_to_bytes(&state)
}

fn pad_message(input: &[u8]) -> Vec<u8> {
    let mut data = input.to_vec();
    let bit_length = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_length.to_be_bytes());
    data
}

fn words_to_bytes<const WORDS: usize, const BYTES: usize>(words: &[u32; WORDS]) -> [u8; BYTES] {
    let mut output = [0_u8; BYTES];
    for (index, word) in words.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        QueryResult, authentication_response, parse_column_name, parse_ok_packet, parse_text_row,
        sha1, sha256,
    };

    #[test]
    fn hashes_known_vectors() {
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn creates_authentication_responses() {
        assert_eq!(
            authentication_response("mysql_native_password", b"", b"scramble").unwrap(),
            Vec::<u8>::new()
        );
        assert_eq!(
            authentication_response("mysql_native_password", b"secret", b"12345678901234567890")
                .unwrap()
                .len(),
            20
        );
        assert_eq!(
            authentication_response("caching_sha2_password", b"secret", b"12345678901234567890")
                .unwrap()
                .len(),
            32
        );
    }

    #[test]
    fn parses_text_protocol_packets() {
        let column = lenenc_fields(&["def", "test", "users", "users", "name", "name"]);
        assert_eq!(parse_column_name(&column).unwrap(), "name");

        let row = lenenc_fields(&["1", "Alice"]);
        assert_eq!(
            parse_text_row(&row, 2).unwrap(),
            vec![Some("1".to_string()), Some("Alice".to_string())]
        );

        let null_row = [1, b'1', 0xfb];
        assert_eq!(
            parse_text_row(&null_row, 2).unwrap(),
            vec![Some("1".to_string()), None]
        );
    }

    #[test]
    fn parses_ok_packet() {
        let result = parse_ok_packet(&[0x00, 0x02, 0x05]).unwrap();
        match result {
            QueryResult::Command {
                affected_rows,
                last_insert_id,
            } => {
                assert_eq!(affected_rows, 2);
                assert_eq!(last_insert_id, 5);
            }
            QueryResult::Rows { .. } => panic!("expected command result"),
        }
    }

    fn lenenc_fields(fields: &[&str]) -> Vec<u8> {
        let mut packet = Vec::new();
        for field in fields {
            packet.push(field.len() as u8);
            packet.extend_from_slice(field.as_bytes());
        }
        packet
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
