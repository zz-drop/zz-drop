use std::io::Cursor;

use zz_drop_core::agent_proto::{
    AgentError, AgentRequest, AgentResponse, EntryKindFilter, FRAME_LIMIT, FramingError,
    KekPayload, PROTOCOL_VERSION, RemoteKind, RemoteListEntry, VersionedRequest, decode_request,
    decode_response, encode_request, encode_response, read_frame, write_frame,
};
use zz_drop_core::{
    Argon2idConfig, CollisionPolicy, NextcloudAuth, NextcloudProfile, PlainProfile,
    ProfileSet, ProfileSettings, ProviderProfile,
};

fn sample_profile() -> PlainProfile {
    PlainProfile {
        profile_version: 1,
        profile_id: "p-0001".into(),
        alias: "casa".into(),
        default_target: "nc".into(),
        providers: vec![ProviderProfile::Nextcloud(NextcloudProfile {
            server_url: "https://example.org".into(),
            username: "user".into(),
            auth: NextcloudAuth::AppPassword {
                secret: "topsecret".into(),
            },
            remote_root: "/zz".into(),
        })],
        collision_policy: CollisionPolicy::Rename,
        settings: ProfileSettings::default(),
        created_at: "2026-04-26T12:00:00Z".into(),
        updated_at: "2026-04-26T12:00:00Z".into(),
    }
}

fn sample_kek_payload() -> KekPayload {
    KekPayload {
        key: [0x42u8; 32],
        salt: [0x11u8; 16],
        kdf: Argon2idConfig::DEFAULT,
    }
}

#[test]
fn protocol_version_constant() {
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn frame_limit_constant() {
    assert_eq!(FRAME_LIMIT, 1 << 20);
}

/// Locks the postcard discriminant byte for every existing
/// `AgentRequest` and `AgentResponse` variant. Postcard encodes the
/// enum tag as a varint of the declaration index, so inserting a
/// new variant **between** existing ones silently shifts every
/// later index and breaks any older client/agent on the wire. The
/// rule documented in `agent-protocol.md` is "additive variants
/// land at the bottom"; this test fails at compile/run time if
/// someone forgets.
#[test]
fn variant_discriminants_are_stable() {
    // Frame layout: [4 bytes BE length][postcard payload].
    // The payload begins with `VersionedRequest::version: u16`,
    // which postcard varint-encodes to a single byte for v=1.
    // The enum discriminant of the inner request follows it,
    // so byte 5 of the frame is the variant tag.
    fn req_byte(v: &AgentRequest) -> u8 {
        encode_request(v).unwrap()[5]
    }
    fn resp_byte(v: &AgentResponse) -> u8 {
        encode_response(v).unwrap()[5]
    }
    let kek = sample_kek_payload();
    let p = sample_profile();
    let set = ProfileSet::with_profile(p.clone());

    // Original v1 surface — these eight indices are frozen and
    // any change is a wire break.
    assert_eq!(req_byte(&AgentRequest::Ping), 0);
    assert_eq!(
        req_byte(&AgentRequest::Unlock {
            profile_set: set.clone(),
            kek: kek.clone(),
            active_alias: "a".into(),
            ttl_secs: None,
        }),
        1
    );
    assert_eq!(req_byte(&AgentRequest::GetProfile), 2);
    assert_eq!(
        req_byte(&AgentRequest::UpdateProfile { profile: p.clone() }),
        3
    );
    assert_eq!(
        req_byte(&AgentRequest::UpdateProfileSet {
            profile_set: set.clone()
        }),
        4
    );
    assert_eq!(
        req_byte(&AgentRequest::SetActiveAlias { alias: "a".into() }),
        5
    );
    assert_eq!(req_byte(&AgentRequest::Lock), 6);
    assert_eq!(req_byte(&AgentRequest::Exit), 7);
    assert_eq!(req_byte(&AgentRequest::Status), 8);
    // Additive variants land after the original surface.
    assert_eq!(
        req_byte(&AgentRequest::ListRemote {
            prefix: None,
            kind_filter: EntryKindFilter::Both,
            max_results: 200,
        }),
        9
    );
    assert_eq!(
        req_byte(&AgentRequest::InvalidateRemote { prefix: None }),
        10
    );

    // Same lock for AgentResponse.
    assert_eq!(resp_byte(&AgentResponse::Pong), 0);
    assert_eq!(resp_byte(&AgentResponse::Unlocked), 1);
    assert_eq!(resp_byte(&AgentResponse::Profile(p.clone())), 2);
    assert_eq!(resp_byte(&AgentResponse::Updated), 3);
    assert_eq!(resp_byte(&AgentResponse::Locked), 4);
    assert_eq!(resp_byte(&AgentResponse::Exited), 5);
    assert_eq!(
        resp_byte(&AgentResponse::Status {
            unlocked: true,
            ttl_remaining_secs: None,
        }),
        6
    );
    assert_eq!(
        resp_byte(&AgentResponse::Error(AgentError::NotUnlocked)),
        7
    );
    // Additive RemoteList variant must land after Error.
    assert_eq!(
        resp_byte(&AgentResponse::RemoteList {
            entries: vec![],
            cached_at_secs: 0,
            truncated: false,
        }),
        8
    );
}

#[test]
fn round_trip_each_request_variant() {
    let cases = [
        AgentRequest::Ping,
        AgentRequest::Unlock {
            profile_set: ProfileSet::with_profile(sample_profile()),
            kek: sample_kek_payload(),
            active_alias: "casa".into(),
            ttl_secs: Some(600),
        },
        AgentRequest::Unlock {
            profile_set: ProfileSet::with_profile(sample_profile()),
            kek: sample_kek_payload(),
            active_alias: "casa".into(),
            ttl_secs: None,
        },
        AgentRequest::GetProfile,
        AgentRequest::UpdateProfile {
            profile: sample_profile(),
        },
        AgentRequest::UpdateProfileSet {
            profile_set: ProfileSet::with_profile(sample_profile()),
        },
        AgentRequest::SetActiveAlias {
            alias: "casa".into(),
        },
        AgentRequest::Lock,
        AgentRequest::Exit,
        AgentRequest::Status,
        AgentRequest::ListRemote {
            prefix: None,
            kind_filter: EntryKindFilter::Both,
            max_results: 200,
        },
        AgentRequest::ListRemote {
            prefix: Some("backup/snap".into()),
            kind_filter: EntryKindFilter::File,
            max_results: 50,
        },
        AgentRequest::ListRemote {
            prefix: Some("docs".into()),
            kind_filter: EntryKindFilter::Directory,
            max_results: 25,
        },
        AgentRequest::InvalidateRemote { prefix: None },
        AgentRequest::InvalidateRemote {
            prefix: Some("backup/snap".into()),
        },
    ];
    for req in cases {
        let frame = encode_request(&req).unwrap();
        let decoded = decode_request(&frame).unwrap();
        assert!(req == decoded, "round trip failed for {req:?}");
    }
}

#[test]
fn round_trip_each_response_variant() {
    let cases = [
        AgentResponse::Pong,
        AgentResponse::Unlocked,
        AgentResponse::Profile(sample_profile()),
        AgentResponse::Locked,
        AgentResponse::Exited,
        AgentResponse::Status {
            unlocked: true,
            ttl_remaining_secs: Some(420),
        },
        AgentResponse::Status {
            unlocked: false,
            ttl_remaining_secs: None,
        },
        AgentResponse::Error(AgentError::NotUnlocked),
        AgentResponse::Error(AgentError::ProtocolMismatch {
            got: 0,
            expected: 1,
        }),
        AgentResponse::RemoteList {
            entries: vec![],
            cached_at_secs: 0,
            truncated: false,
        },
        AgentResponse::RemoteList {
            entries: vec![
                RemoteListEntry {
                    name: "readme.md".into(),
                    size: Some(12_345),
                    kind: RemoteKind::File,
                    mtime_secs: Some(1_700_000_000),
                },
                RemoteListEntry {
                    name: "docs".into(),
                    size: None,
                    kind: RemoteKind::Directory,
                    mtime_secs: None,
                },
            ],
            cached_at_secs: 1_700_000_123,
            truncated: true,
        },
    ];
    for resp in cases {
        let frame = encode_response(&resp).unwrap();
        let decoded = decode_response(&frame).unwrap();
        assert!(resp == decoded, "round trip failed for {resp:?}");
    }
}

#[test]
fn encode_starts_with_4_byte_be_length() {
    let frame = encode_request(&AgentRequest::Ping).unwrap();
    assert!(frame.len() > 4);
    let len_bytes: [u8; 4] = frame[..4].try_into().unwrap();
    let declared_len = u32::from_be_bytes(len_bytes) as usize;
    assert_eq!(frame.len(), 4 + declared_len);
}

#[test]
fn encode_response_starts_with_4_byte_be_length() {
    let frame = encode_response(&AgentResponse::Pong).unwrap();
    assert!(frame.len() > 4);
    let len_bytes: [u8; 4] = frame[..4].try_into().unwrap();
    let declared_len = u32::from_be_bytes(len_bytes) as usize;
    assert_eq!(frame.len(), 4 + declared_len);
}

#[test]
fn write_frame_rejects_payload_above_limit() {
    let payload = vec![0u8; FRAME_LIMIT + 1];
    let mut sink: Vec<u8> = Vec::new();
    let res = write_frame(&mut sink, &payload);
    match res {
        Err(FramingError::FrameTooLarge { size, limit }) => {
            assert_eq!(size, FRAME_LIMIT + 1);
            assert_eq!(limit, FRAME_LIMIT);
        }
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

#[test]
fn decode_request_rejects_oversized_len_header_before_allocating() {
    // Hand-craft a header declaring more than the limit. We deliberately
    // do NOT provide the body — decode must reject on the header alone.
    let huge = (FRAME_LIMIT as u32 + 1).to_be_bytes();
    let frame: Vec<u8> = huge.to_vec();
    let res = decode_request(&frame);
    match res {
        Err(FramingError::FrameTooLarge { size, limit }) => {
            assert_eq!(size, FRAME_LIMIT + 1);
            assert_eq!(limit, FRAME_LIMIT);
        }
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

#[test]
fn decode_request_rejects_unsupported_version() {
    let envelope = VersionedRequest {
        version: 99,
        request: AgentRequest::Ping,
    };
    let payload = postcard::to_allocvec(&envelope).unwrap();
    let len = u32::try_from(payload.len()).unwrap().to_be_bytes();
    let mut frame = Vec::new();
    frame.extend_from_slice(&len);
    frame.extend_from_slice(&payload);

    let res = decode_request(&frame);
    match res {
        Err(FramingError::UnsupportedVersion { got, expected }) => {
            assert_eq!(got, 99);
            assert_eq!(expected, PROTOCOL_VERSION);
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn decode_request_corrupt_postcard_yields_decode_or_version_error() {
    let mut frame = encode_request(&AgentRequest::Ping).unwrap();
    *frame.last_mut().unwrap() ^= 0xFF;
    let res = decode_request(&frame);
    assert!(
        matches!(
            res,
            Err(FramingError::Decode(_)) | Err(FramingError::UnsupportedVersion { .. })
        ),
        "got {res:?}"
    );
}

/// Wire types that carry secret material (KEK bytes, passphrases,
/// access tokens) must redact through `Debug` so a stray `dbg!()` or
/// `tracing::error!("{:?}", req)` cannot leak them. Locks the
/// guarantee for the highest-value type — `KekPayload` carries the
/// raw 32-byte key that decrypts the container.
#[test]
fn kek_payload_debug_does_not_leak_key_bytes_or_salt() {
    let kek = KekPayload {
        key: [0xCA; 32],
        salt: [0xFE; 16],
        kdf: Argon2idConfig::DEFAULT,
    };
    let dbg = format!("{kek:?}");
    assert!(dbg.contains("redacted"), "expected `redacted`, got: {dbg}");
    // No raw byte rendering of the key or salt — neither hex nor
    // decimal representations should appear.
    let key_hex: String = kek.key.iter().map(|b| format!("{b:02x}")).collect();
    let salt_hex: String = kek.salt.iter().map(|b| format!("{b:02x}")).collect();
    assert!(!dbg.contains(&key_hex));
    assert!(!dbg.contains(&salt_hex));
    assert!(!dbg.contains("202"), "expected no decimal byte rendering"); // 0xCA == 202
    assert!(!dbg.contains("254"), "expected no decimal byte rendering"); // 0xFE == 254
}

#[test]
fn unlock_request_debug_redacts_passphrase_bearing_fields() {
    let req = AgentRequest::Unlock {
        profile_set: ProfileSet::with_profile(sample_profile()),
        kek: KekPayload {
            key: [0xCA; 32],
            salt: [0xFE; 16],
            kdf: Argon2idConfig::DEFAULT,
        },
        active_alias: "alias-canary".into(),
        ttl_secs: Some(60),
    };
    let dbg = format!("{req:?}");
    // Inner profile name + KEK + active alias must all be redacted.
    assert!(dbg.contains("redacted"));
    assert!(!dbg.contains("alias-canary"));
    assert!(!dbg.contains("topsecret"));
}

/// SACS list/invalidate requests carry remote prefixes that may
/// reflect user file layout. Treated as user data, not secrets,
/// but symmetric `Debug` redaction guards against an accidental
/// `tracing::error!("{:?}", req)` leaking the layout into a log
/// stream that is later shared.
#[test]
fn list_remote_request_debug_redacts_prefix() {
    let req = AgentRequest::ListRemote {
        prefix: Some("backup/private-prefix-canary".into()),
        kind_filter: EntryKindFilter::Both,
        max_results: 200,
    };
    let dbg = format!("{req:?}");
    assert!(dbg.contains("redacted"), "expected redaction, got: {dbg}");
    assert!(!dbg.contains("private-prefix-canary"));
    // Non-secret fields must still appear, so debugging stays
    // useful for protocol issues.
    assert!(dbg.contains("Both"));
    assert!(dbg.contains("200"));
}

#[test]
fn invalidate_remote_request_debug_redacts_prefix() {
    let req = AgentRequest::InvalidateRemote {
        prefix: Some("private-prefix-canary".into()),
    };
    let dbg = format!("{req:?}");
    assert!(dbg.contains("redacted"), "expected redaction, got: {dbg}");
    assert!(!dbg.contains("private-prefix-canary"));
}

/// `RemoteList` carries the file/dir names returned by the
/// provider — reused by the SACS dropdown. The wire form is
/// fine in plaintext; the `Debug` form redacts to keep stray
/// log statements safe.
#[test]
fn remote_list_response_debug_does_not_leak_entry_names() {
    let resp = AgentResponse::RemoteList {
        entries: vec![RemoteListEntry {
            name: "private-file-canary.md".into(),
            size: Some(123),
            kind: RemoteKind::File,
            mtime_secs: None,
        }],
        cached_at_secs: 1_700_000_000,
        truncated: false,
    };
    let dbg = format!("{resp:?}");
    assert!(!dbg.contains("private-file-canary.md"));
    assert!(dbg.contains("redacted"));
    assert!(dbg.contains("1700000000"));
}

#[test]
fn decode_request_empty_input_is_truncated() {
    let res = decode_request(&[]);
    assert!(matches!(res, Err(FramingError::Truncated)), "got {res:?}");
}

#[test]
fn decode_request_short_body_is_truncated() {
    // valid 4-byte length header claiming 100 bytes, but only 5 follow.
    let mut frame = 100u32.to_be_bytes().to_vec();
    frame.extend_from_slice(&[1, 2, 3, 4, 5]);
    let res = decode_request(&frame);
    assert!(matches!(res, Err(FramingError::Truncated)), "got {res:?}");
}

#[test]
fn write_then_read_frame_round_trip() {
    let payload = b"hello agent".to_vec();
    let mut buf: Vec<u8> = Vec::new();
    write_frame(&mut buf, &payload).unwrap();

    let mut cursor = Cursor::new(buf);
    let restored = read_frame(&mut cursor).unwrap();
    assert_eq!(restored, payload);
}

#[test]
fn read_frame_truncated_stream_is_truncated_error() {
    let buf: Vec<u8> = vec![0, 0, 0, 10];
    let mut cursor = Cursor::new(buf);
    let res = read_frame(&mut cursor);
    assert!(matches!(res, Err(FramingError::Truncated)), "got {res:?}");
}

#[test]
fn read_frame_rejects_oversize_len_header() {
    let huge = (FRAME_LIMIT as u32 + 100).to_be_bytes();
    let mut cursor = Cursor::new(huge.to_vec());
    let res = read_frame(&mut cursor);
    match res {
        Err(FramingError::FrameTooLarge { size, .. }) => {
            assert_eq!(size, FRAME_LIMIT + 100);
        }
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

#[test]
fn round_trip_via_streaming_io() {
    // Realistic CLI ↔ agent path: encode, write_frame body to a stream,
    // read_frame back, decode.
    let req = AgentRequest::Unlock {
        profile_set: ProfileSet::with_profile(sample_profile()),
        kek: sample_kek_payload(),
        active_alias: "casa".into(),
        ttl_secs: Some(600),
    };
    let frame = encode_request(&req).unwrap();

    // The frame already contains its length prefix, so we strip it and
    // pass only the body to write_frame, which re-prepends a length.
    let body = &frame[4..];
    let mut buf: Vec<u8> = Vec::new();
    write_frame(&mut buf, body).unwrap();

    let mut cursor = Cursor::new(buf);
    let body_back = read_frame(&mut cursor).unwrap();

    let mut full = u32::try_from(body_back.len()).unwrap().to_be_bytes().to_vec();
    full.extend_from_slice(&body_back);
    let decoded = decode_request(&full).unwrap();

    assert!(req == decoded);
}
