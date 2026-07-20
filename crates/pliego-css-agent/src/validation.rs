use super::RepairContractError;

pub(crate) fn validate_sha256(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RepairContractError::new(format!(
            "{field} must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

pub(crate) fn validate_version(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.is_empty()
        || value.len() > 64
        || !value.as_bytes()[0].is_ascii_digit()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err(RepairContractError::new(format!(
            "{field} must be a bounded ASCII version beginning with a digit"
        )));
    }
    Ok(())
}

pub(crate) fn validate_fingerprint(field: &str, value: &str) -> Result<(), RepairContractError> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| RepairContractError::new(format!("{field} must start with `sha256:`")))?;
    validate_sha256(field, digest)
}

pub(crate) fn validate_finding_code(value: &str) -> Result<(), RepairContractError> {
    if !(3..=64).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_uppercase()
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(RepairContractError::new(format!(
            "finding code `{value}` is not canonical"
        )));
    }
    Ok(())
}

pub(crate) fn validate_slug(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_lowercase()
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(RepairContractError::new(format!(
            "{field} `{value}` must be lowercase kebab-case ASCII"
        )));
    }
    Ok(())
}

pub(crate) fn validate_dotted_id(field: &str, value: &str) -> Result<(), RepairContractError> {
    if value.is_empty() || value.len() > 256 {
        return Err(RepairContractError::new(format!(
            "{field} must be 1-256 bytes"
        )));
    }
    for segment in value.split('.') {
        validate_slug(field, segment)?;
    }
    Ok(())
}

pub(crate) fn validate_text(
    field: &str,
    value: &str,
    max: usize,
) -> Result<(), RepairContractError> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(RepairContractError::new(format!(
            "{field} must be non-empty, at most {max} bytes, and contain no control characters"
        )));
    }
    Ok(())
}

pub(crate) fn validate_logical_path(field: &str, value: &str) -> Result<(), RepairContractError> {
    validate_text(field, value, 4_096)?;
    if value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(RepairContractError::new(format!(
            "{field} `{value}` must be a portable relative logical path"
        )));
    }
    Ok(())
}
