use sql_rock::database::Database;
use sql_rock::model::Statement;
use sql_rock::parser::parse_statement;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT_LONG_PASSWORD: u32 = 0x0000_0001;
const CLIENT_LONG_FLAG: u32 = 0x0000_0004;
const CLIENT_CONNECT_WITH_DB: u32 = 0x0000_0008;
const CLIENT_PROTOCOL_41: u32 = 0x0000_0200;
const CLIENT_TRANSACTIONS: u32 = 0x0000_2000;
const CLIENT_SECURE_CONNECTION: u32 = 0x0000_8000;
const CLIENT_PLUGIN_AUTH: u32 = 0x0008_0000;

const COM_QUIT: u8 = 0x01;
const COM_INIT_DB: u8 = 0x02;
const COM_QUERY: u8 = 0x03;
const COM_PING: u8 = 0x0e;

const SERVER_CAPABILITIES: u32 = CLIENT_LONG_PASSWORD
    | CLIENT_LONG_FLAG
    | CLIENT_CONNECT_WITH_DB
    | CLIENT_PROTOCOL_41
    | CLIENT_TRANSACTIONS
    | CLIENT_SECURE_CONNECTION
    | CLIENT_PLUGIN_AUTH;

pub fn serve_connection(
    mut stream: TcpStream,
    database: &Arc<Mutex<Database>>,
    expected_user: &str,
    password: &str,
) -> Result<(), String> {
    stream
        .set_nodelay(true)
        .map_err(|error| error.to_string())?;
    let scramble = generate_scramble();
    write_packet(&mut stream, 0, &handshake_packet(&scramble))?;

    let (sequence, response) = read_packet(&mut stream)?;
    let login = parse_handshake_response(&response)?;
    if login.username != expected_user
        || !verify_mysql_native_password(password.as_bytes(), &scramble, &login.auth_response)
    {
        write_error(
            &mut stream,
            sequence.wrapping_add(1),
            1045,
            "28000",
            &format!("Access denied for user '{}'", login.username),
        )?;
        return Ok(());
    }
    write_ok(&mut stream, sequence.wrapping_add(1), 0, 0)?;
    let mut prepared_statements = HashMap::new();

    loop {
        let (_, packet) = match read_packet(&mut stream) {
            Ok(packet) => packet,
            Err(error) if error.contains("failed to fill whole buffer") => return Ok(()),
            Err(error) => return Err(error),
        };
        let Some(command) = packet.first().copied() else {
            write_error(&mut stream, 1, 1064, "42000", "Empty command")?;
            continue;
        };
        match command {
            COM_QUIT => return Ok(()),
            COM_PING | COM_INIT_DB => write_ok(&mut stream, 1, 0, 0)?,
            COM_QUERY => {
                let sql = String::from_utf8_lossy(&packet[1..]);
                execute_query(&mut stream, database, sql.trim(), &mut prepared_statements)?;
            }
            _ => write_error(
                &mut stream,
                1,
                1047,
                "08S01",
                &format!("Unsupported command {command}"),
            )?,
        }
    }
}

struct Login {
    username: String,
    auth_response: Vec<u8>,
}

fn parse_handshake_response(packet: &[u8]) -> Result<Login, String> {
    let mut position = 0;
    let capabilities = read_u32_le(packet, &mut position)?;
    read_bytes(packet, &mut position, 4)?;
    read_bytes(packet, &mut position, 1)?;
    read_bytes(packet, &mut position, 23)?;
    let username = String::from_utf8_lossy(read_null_terminated(packet, &mut position)?).into();
    let auth_response = if capabilities & CLIENT_SECURE_CONNECTION != 0 {
        let length = read_u8(packet, &mut position)? as usize;
        read_bytes(packet, &mut position, length)?.to_vec()
    } else {
        read_null_terminated(packet, &mut position)?.to_vec()
    };
    Ok(Login {
        username,
        auth_response,
    })
}

fn execute_query(
    stream: &mut TcpStream,
    database: &Arc<Mutex<Database>>,
    sql: &str,
    prepared_statements: &mut HashMap<String, String>,
) -> Result<(), String> {
    if starts_with_keyword(sql, "prepare") {
        let (name, statement) = match parse_prepare(sql) {
            Ok(value) => value,
            Err(error) => return write_error(stream, 1, 1064, "42000", &error),
        };
        prepared_statements.insert(name.to_ascii_lowercase(), statement);
        return write_ok(stream, 1, 0, 0);
    }
    if starts_with_keyword(sql, "execute") {
        let (name, parameters) = match parse_execute(sql) {
            Ok(value) => value,
            Err(error) => return write_error(stream, 1, 1210, "HY000", &error),
        };
        let statement = match prepared_statements.get(&name.to_ascii_lowercase()) {
            Some(statement) => statement,
            None => {
                return write_error(
                    stream,
                    1,
                    1243,
                    "HY000",
                    &format!("Unknown prepared statement handler ({name})"),
                );
            }
        };
        let bound_sql = match bind_parameters(statement, &parameters) {
            Ok(sql) => sql,
            Err(error) => return write_error(stream, 1, 1210, "HY000", &error),
        };
        return execute_regular_query(stream, database, &bound_sql);
    }
    if starts_with_keyword(sql, "deallocate prepare") {
        let name = match parse_deallocate(sql) {
            Ok(name) => name,
            Err(error) => return write_error(stream, 1, 1064, "42000", &error),
        };
        if prepared_statements
            .remove(&name.to_ascii_lowercase())
            .is_none()
        {
            return write_error(
                stream,
                1,
                1243,
                "HY000",
                &format!("Unknown prepared statement handler ({name})"),
            );
        }
        return write_ok(stream, 1, 0, 0);
    }

    execute_regular_query(stream, database, sql)
}

fn execute_regular_query(
    stream: &mut TcpStream,
    database: &Arc<Mutex<Database>>,
    sql: &str,
) -> Result<(), String> {
    let statement = match parse_statement(sql) {
        Ok(statement) => statement,
        Err(error) => {
            return write_error(stream, 1, 1064, "42000", &error.to_string());
        }
    };
    let returns_rows = statement_returns_rows(&statement);
    let output = match database
        .lock()
        .map_err(|_| "database lock is poisoned".to_string())?
        .execute(statement)
    {
        Ok(output) => output,
        Err(error) => return write_error(stream, 1, 1105, "HY000", &error.to_string()),
    };

    if returns_rows {
        write_result_set(stream, &output)
    } else {
        write_ok(stream, 1, affected_rows(&output), 0)
    }
}

fn parse_prepare(sql: &str) -> Result<(String, String), String> {
    let rest = strip_keyword(sql, "prepare")?.trim_start();
    let (name, rest) = take_identifier(rest)?;
    let rest = strip_keyword(rest.trim_start(), "from")?.trim_start();
    let (statement, trailing) = take_quoted_string(rest)?;
    if !trailing.trim().trim_end_matches(';').trim().is_empty() {
        return Err("unexpected trailing PREPARE syntax".to_string());
    }
    Ok((name, statement))
}

fn parse_execute(sql: &str) -> Result<(String, Vec<String>), String> {
    let rest = strip_keyword(sql, "execute")?.trim_start();
    let (name, rest) = take_identifier(rest)?;
    let rest = rest.trim().trim_end_matches(';').trim();
    if rest.is_empty() {
        return Ok((name, Vec::new()));
    }
    let values = strip_keyword(rest, "using")?.trim_start();
    Ok((name, split_parameters(values)?))
}

fn parse_deallocate(sql: &str) -> Result<String, String> {
    let rest = strip_keyword(sql, "deallocate prepare")?.trim_start();
    let (name, trailing) = take_identifier(rest)?;
    if !trailing.trim().trim_end_matches(';').trim().is_empty() {
        return Err("unexpected trailing DEALLOCATE PREPARE syntax".to_string());
    }
    Ok(name)
}

fn bind_parameters(statement: &str, parameters: &[String]) -> Result<String, String> {
    let mut output = String::new();
    let mut index = 0;
    let mut in_string = false;
    let mut chars = statement.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            output.push(ch);
            if in_string && chars.peek() == Some(&'\'') {
                output.push(chars.next().expect("peeked character exists"));
            } else {
                in_string = !in_string;
            }
        } else if ch == '?' && !in_string {
            let value = parameters
                .get(index)
                .ok_or_else(|| "not enough EXECUTE parameters".to_string())?;
            output.push_str(value);
            index += 1;
        } else {
            output.push(ch);
        }
    }
    if index != parameters.len() {
        return Err(format!(
            "prepared statement requires {index} parameter(s), but {} were supplied",
            parameters.len()
        ));
    }
    Ok(output)
}

fn split_parameters(input: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            current.push(ch);
            if in_string && chars.peek() == Some(&'\'') {
                current.push(chars.next().expect("peeked character exists"));
            } else {
                in_string = !in_string;
            }
        } else if ch == ',' && !in_string {
            push_parameter(&mut values, &mut current)?;
        } else {
            current.push(ch);
        }
    }
    if in_string {
        return Err("unterminated EXECUTE parameter".to_string());
    }
    push_parameter(&mut values, &mut current)?;
    Ok(values)
}

fn push_parameter(values: &mut Vec<String>, current: &mut String) -> Result<(), String> {
    let value = current.trim();
    if value.is_empty() {
        return Err("empty EXECUTE parameter".to_string());
    }
    if !is_parameter_literal(value) {
        return Err(format!("unsupported EXECUTE parameter `{value}`"));
    }
    values.push(value.to_string());
    current.clear();
    Ok(())
}

fn is_parameter_literal(value: &str) -> bool {
    value.parse::<f64>().is_ok()
        || ["null", "now()", "today()"]
            .iter()
            .any(|literal| value.eq_ignore_ascii_case(literal))
        || value.starts_with('\'')
            && take_quoted_string(value).is_ok_and(|(_, trailing)| trailing.trim().is_empty())
}

fn take_identifier(input: &str) -> Result<(String, &str), String> {
    let input = input.trim_start();
    let end = input
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    if end == 0 {
        return Err("expected prepared statement name".to_string());
    }
    Ok((input[..end].to_string(), &input[end..]))
}

fn take_quoted_string(input: &str) -> Result<(String, &str), String> {
    let rest = input
        .strip_prefix('\'')
        .ok_or_else(|| "PREPARE FROM requires quoted SQL".to_string())?;
    let mut value = String::new();
    let mut chars = rest.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '\'' {
            if chars.peek().is_some_and(|(_, next)| *next == '\'') {
                chars.next();
                value.push('\'');
            } else {
                return Ok((value, &rest[index + 1..]));
            }
        } else {
            value.push(ch);
        }
    }
    Err("unterminated PREPARE SQL string".to_string())
}

fn starts_with_keyword(input: &str, keyword: &str) -> bool {
    input
        .get(..keyword.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
        && input
            .get(keyword.len()..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|ch| ch.is_whitespace())
}

fn strip_keyword<'a>(input: &'a str, keyword: &str) -> Result<&'a str, String> {
    if starts_with_keyword(input, keyword) {
        Ok(&input[keyword.len()..])
    } else {
        Err(format!("expected keyword `{keyword}`"))
    }
}

fn statement_returns_rows(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::SelectAll { .. }
            | Statement::SelectCount { .. }
            | Statement::SelectQuery(_)
            | Statement::DescribeTable { .. }
            | Statement::ShowTables
            | Statement::ShowCreateTable { .. }
    )
}

fn affected_rows(output: &str) -> u64 {
    output
        .split_whitespace()
        .find_map(|part| part.parse().ok())
        .unwrap_or(0)
}

fn write_result_set(stream: &mut TcpStream, output: &str) -> Result<(), String> {
    let mut lines = output.lines();
    let headers = lines
        .next()
        .unwrap_or_default()
        .split('\t')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut sequence = 1;
    write_packet(stream, sequence, &lenenc_int(headers.len() as u64))?;
    sequence = sequence.wrapping_add(1);

    for header in &headers {
        write_packet(stream, sequence, &column_definition(header))?;
        sequence = sequence.wrapping_add(1);
    }
    write_packet(stream, sequence, &eof_packet())?;
    sequence = sequence.wrapping_add(1);

    for line in lines {
        let values = line.split('\t').collect::<Vec<_>>();
        let mut row = Vec::new();
        for value in values {
            write_lenenc_bytes(&mut row, value.as_bytes());
        }
        write_packet(stream, sequence, &row)?;
        sequence = sequence.wrapping_add(1);
    }
    write_packet(stream, sequence, &eof_packet())
}

fn handshake_packet(scramble: &[u8; 20]) -> Vec<u8> {
    let mut packet = Vec::new();
    packet.push(10);
    packet.extend_from_slice(b"8.0.0-sql-rock\0");
    packet.extend_from_slice(&1_u32.to_le_bytes());
    packet.extend_from_slice(&scramble[..8]);
    packet.push(0);
    packet.extend_from_slice(&(SERVER_CAPABILITIES as u16).to_le_bytes());
    packet.push(45);
    packet.extend_from_slice(&2_u16.to_le_bytes());
    packet.extend_from_slice(&((SERVER_CAPABILITIES >> 16) as u16).to_le_bytes());
    packet.push(21);
    packet.extend_from_slice(&[0; 10]);
    packet.extend_from_slice(&scramble[8..]);
    packet.push(0);
    packet.extend_from_slice(b"mysql_native_password\0");
    packet
}

fn column_definition(name: &str) -> Vec<u8> {
    let mut packet = Vec::new();
    for value in ["def", "", "", "", name, name] {
        write_lenenc_bytes(&mut packet, value.as_bytes());
    }
    packet.push(0x0c);
    packet.extend_from_slice(&45_u16.to_le_bytes());
    packet.extend_from_slice(&1024_u32.to_le_bytes());
    packet.push(0xfd);
    packet.extend_from_slice(&0_u16.to_le_bytes());
    packet.push(0);
    packet.extend_from_slice(&[0, 0]);
    packet
}

fn write_ok(
    stream: &mut TcpStream,
    sequence: u8,
    affected_rows: u64,
    last_insert_id: u64,
) -> Result<(), String> {
    let mut packet = vec![0x00];
    packet.extend_from_slice(&lenenc_int(affected_rows));
    packet.extend_from_slice(&lenenc_int(last_insert_id));
    packet.extend_from_slice(&2_u16.to_le_bytes());
    packet.extend_from_slice(&0_u16.to_le_bytes());
    write_packet(stream, sequence, &packet)
}

fn write_error(
    stream: &mut TcpStream,
    sequence: u8,
    code: u16,
    state: &str,
    message: &str,
) -> Result<(), String> {
    let mut packet = vec![0xff];
    packet.extend_from_slice(&code.to_le_bytes());
    packet.push(b'#');
    packet.extend_from_slice(state.as_bytes());
    packet.extend_from_slice(message.as_bytes());
    write_packet(stream, sequence, &packet)
}

fn eof_packet() -> Vec<u8> {
    let mut packet = vec![0xfe];
    packet.extend_from_slice(&0_u16.to_le_bytes());
    packet.extend_from_slice(&2_u16.to_le_bytes());
    packet
}

fn verify_mysql_native_password(password: &[u8], scramble: &[u8], response: &[u8]) -> bool {
    if password.is_empty() {
        return response.is_empty();
    }
    let stage1 = sha1(password);
    let stage2 = sha1(&stage1);
    let mut input = scramble.to_vec();
    input.extend_from_slice(&stage2);
    let expected = xor(&stage1, &sha1(&input));
    constant_time_equal(&expected, response)
}

fn generate_scramble() -> [u8; 20] {
    let mut state = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut scramble = [0_u8; 20];
    for byte in &mut scramble {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = ((state % 94) + 33) as u8;
    }
    scramble
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn lenenc_int(value: u64) -> Vec<u8> {
    match value {
        0..=250 => vec![value as u8],
        251..=65_535 => {
            let mut bytes = vec![0xfc];
            bytes.extend_from_slice(&(value as u16).to_le_bytes());
            bytes
        }
        65_536..=16_777_215 => vec![0xfd, value as u8, (value >> 8) as u8, (value >> 16) as u8],
        _ => {
            let mut bytes = vec![0xfe];
            bytes.extend_from_slice(&value.to_le_bytes());
            bytes
        }
    }
}

fn write_lenenc_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&lenenc_int(value.len() as u64));
    output.extend_from_slice(value);
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

fn read_u8(packet: &[u8], position: &mut usize) -> Result<u8, String> {
    let value = *packet
        .get(*position)
        .ok_or_else(|| "unexpected end of MySQL packet".to_string())?;
    *position += 1;
    Ok(value)
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
        .ok_or_else(|| "invalid packet length".to_string())?;
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
    let mut data = input.to_vec();
    let bit_length = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = [
        0x67452301_u32,
        0xefcdab89,
        0x98badcfe,
        0x10325476,
        0xc3d2e1f0,
    ];
    for chunk in data.chunks_exact(64) {
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
    let mut output = [0_u8; 20];
    for (index, word) in state.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        affected_rows, bind_parameters, constant_time_equal, handshake_packet, lenenc_int,
        parse_deallocate, parse_execute, parse_prepare, sha1, statement_returns_rows,
        verify_mysql_native_password, xor,
    };
    use sql_rock::parser::parse_statement;

    #[test]
    fn creates_protocol_values() {
        assert_eq!(handshake_packet(&[b'a'; 20])[0], 10);
        assert_eq!(lenenc_int(250), vec![250]);
        assert_eq!(lenenc_int(251), vec![0xfc, 251, 0]);
    }

    #[test]
    fn recognizes_result_statements_and_affected_rows() {
        assert!(statement_returns_rows(
            &parse_statement("SELECT * FROM users").unwrap()
        ));
        assert!(!statement_returns_rows(
            &parse_statement("INSERT INTO users (id) VALUES (1)").unwrap()
        ));
        assert_eq!(affected_rows("updated 3 row(s) in `users`"), 3);
    }

    #[test]
    fn verifies_native_password_response() {
        let scramble = *b"12345678901234567890";
        let password = b"sqlrock";
        let stage1 = sha1(password);
        let stage2 = sha1(&stage1);
        let mut input = scramble.to_vec();
        input.extend_from_slice(&stage2);
        let response = xor(&stage1, &sha1(&input));
        assert!(verify_mysql_native_password(password, &scramble, &response));
        assert!(!verify_mysql_native_password(
            b"wrong", &scramble, &response
        ));
        assert!(constant_time_equal(b"same", b"same"));
    }

    #[test]
    fn parses_and_binds_prepared_statements() {
        assert_eq!(
            parse_prepare("PREPARE stmt FROM 'SELECT * FROM users WHERE id = ?'").unwrap(),
            (
                "stmt".to_string(),
                "SELECT * FROM users WHERE id = ?".to_string()
            )
        );
        let (name, parameters) = parse_execute("EXECUTE stmt USING 123").unwrap();
        assert_eq!(name, "stmt");
        assert_eq!(
            bind_parameters("SELECT * FROM users WHERE id = ?", &parameters).unwrap(),
            "SELECT * FROM users WHERE id = 123"
        );
        assert_eq!(parse_deallocate("DEALLOCATE PREPARE stmt").unwrap(), "stmt");
    }
}
