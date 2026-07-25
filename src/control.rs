//! Canonical Aegis command grammar.
//!
//! This is intentionally only the authority-neutral parse boundary. Applying
//! a parsed request requires an authenticated control channel to the live
//! PID-1 supervisor; no parser result is itself permission to change service
//! state.

pub const MAXIMUM_SERVICE_NAME_BYTES: usize = 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AegisVerb {
    Status,
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
    Logs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AegisCommand<'a> {
    pub verb: AegisVerb,
    pub service: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AegisCommandError {
    Usage,
    UnknownVerb,
    InvalidService,
}

/// Parses `aegis <verb> <service>` without shell syntax or path resolution.
pub fn parse<'a>(arguments: &[&'a [u8]]) -> Result<AegisCommand<'a>, AegisCommandError> {
    if arguments.len() != 3 || arguments[0] != b"aegis" {
        return Err(AegisCommandError::Usage);
    }
    let verb = match arguments[1] {
        b"status" => AegisVerb::Status,
        b"start" => AegisVerb::Start,
        b"stop" => AegisVerb::Stop,
        b"restart" => AegisVerb::Restart,
        b"enable" => AegisVerb::Enable,
        b"disable" => AegisVerb::Disable,
        b"logs" => AegisVerb::Logs,
        _ => return Err(AegisCommandError::UnknownVerb),
    };
    let service = arguments[2];
    if !valid_service_name(service) {
        return Err(AegisCommandError::InvalidService);
    }
    Ok(AegisCommand { verb, service })
}

fn valid_service_name(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= MAXIMUM_SERVICE_NAME_BYTES
        && name.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_complete_crest_command_surface() {
        for (word, verb) in [
            (b"status" as &[u8], AegisVerb::Status),
            (b"start", AegisVerb::Start),
            (b"stop", AegisVerb::Stop),
            (b"restart", AegisVerb::Restart),
            (b"enable", AegisVerb::Enable),
            (b"disable", AegisVerb::Disable),
            (b"logs", AegisVerb::Logs),
        ] {
            assert_eq!(
                parse(&[b"aegis", word, b"crest"]),
                Ok(AegisCommand {
                    verb,
                    service: b"crest"
                })
            );
        }
    }

    #[test]
    fn rejects_paths_and_unknown_verbs() {
        assert_eq!(
            parse(&[b"aegis", b"start", b"../crest"]),
            Err(AegisCommandError::InvalidService)
        );
        assert_eq!(
            parse(&[b"aegis", b"tail", b"crest"]),
            Err(AegisCommandError::UnknownVerb)
        );
    }
}
