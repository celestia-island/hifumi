//! ⑥ 非 JSON wire 通道。
//!
//! 帧结构 `b"HIFU" | fnv1a(type@version) LE | postcard(当前 schema 明文结构)`。
//! 版本哈希钉死 schema：错版本/错类型的帧显式报 VersionMismatch。
//!
//! 载荷不走版本化 serde 表示（`$version` 内部标签枚举需要自描述格式，
//! postcard 拒绝 deserialize_any）——这是 `WireCodec` 只编解码 final
//! 版本明文结构的原因，见 `hifumi::wire` 模块文档。

use anyhow::Result;
use hifumi::{version, wire};

#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + note: String,
})]
struct Doc {
    id: u32,
    note: String,
}

/// 帧 = 魔数 + LE 哈希 + postcard 载荷，且与 JSON 相比显著更小。
#[test]
fn frame_layout_and_roundtrip() -> Result<()> {
    let doc = Doc {
        id: 42,
        note: "hello".to_string(),
    };
    let frame = wire::to_wire(&doc, &Doc::__HIFUMI_META)?;

    assert_eq!(&frame[..4], b"HIFU", "magic");
    let hash = u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
    assert_eq!(
        hash,
        wire::version_hash("Doc", "0.2"),
        "header hash must pin type@current-version"
    );

    let back: Doc = wire::from_wire(&frame, &Doc::__HIFUMI_META)?;
    assert_eq!(back, doc);

    // postcard 紧凑性是本通道的存在理由：对同一负载做量级断言
    let json = serde_json::to_vec(&serde_json::json!({
        "$version": "0.2",
        "id": 42,
        "note": "hello",
    }))?;
    assert!(
        frame.len() < json.len(),
        "wire frame ({}) should beat JSON ({})",
        frame.len(),
        json.len()
    );

    Ok(())
}

/// 帧损坏（坏魔数/截断）→ BadFrame，不进入载荷解析。
#[test]
fn bad_frames_rejected() {
    let doc = Doc {
        id: 1,
        note: String::new(),
    };
    let frame = wire::to_wire(&doc, &Doc::__HIFUMI_META).unwrap();

    let mut bad_magic = frame.clone();
    bad_magic[0] = b'X';
    match wire::from_wire::<Doc>(&bad_magic, &Doc::__HIFUMI_META) {
        Err(wire::WireError::BadFrame) => {}
        other => panic!("expected BadFrame, got {other:?}"),
    }

    match wire::from_wire::<Doc>(&frame[..7], &Doc::__HIFUMI_META) {
        Err(wire::WireError::BadFrame) => {}
        other => panic!("short frame must be BadFrame, got {other:?}"),
    }
}

/// 版本哈希不一致 → VersionMismatch，错误文案指向 JSON 层迁移通道。
#[test]
fn version_mismatch_names_both_sides() {
    // 模拟「发送方还是旧 schema 0.1」的帧：手工拼 8 字节帧头
    // （旧版本哈希）+ 任意 postcard 载荷。哈希校验发生在载荷解析
    // 之前，因此载荷内容对 mismatch 路径无关紧要。
    let mut old_frame = b"HIFU".to_vec();
    old_frame.extend_from_slice(&wire::version_hash("Doc", "0.1").to_le_bytes());
    old_frame.push(7);

    match wire::from_wire::<Doc>(&old_frame, &Doc::__HIFUMI_META) {
        Err(wire::WireError::VersionMismatch {
            wire: got,
            expected,
            expected_key,
        }) => {
            assert_eq!(got, wire::version_hash("Doc", "0.1"));
            assert_eq!(expected, wire::version_hash("Doc", "0.2"));
            assert_eq!(expected_key, "Doc@0.2");
        }
        other => panic!("expected VersionMismatch, got {other:?}"),
    }

    // 错误文案必须指路 JSON 层迁移，而不是把字节解成垃圾
    let err = wire::from_wire::<Doc>(&old_frame, &Doc::__HIFUMI_META).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("version hash mismatch"), "{msg}");
    assert!(msg.contains("JSON layer"), "{msg}");
}

/// fnv1a 对拍公开测试向量（防手写常量翻车）。
#[test]
fn fnv1a_vectors() {
    assert_eq!(wire::fnv1a32(b""), 0x811c_9dc5);
    assert_eq!(wire::fnv1a32(b"a"), 0xe40c_292c);
    assert_eq!(wire::fnv1a32(b"foobar"), 0xbf9c_f968);
}
