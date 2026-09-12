#[cfg(not(target_os = "android"))]
use arboard::ClipboardData;
use hbb_common::{
    compress::{compress as compress_func, decompress},
};
use base::message_proto::{Clipboard, ClipboardFormat, Message, MultiClipboards};

fn plain_to_proto(s: String, format: ClipboardFormat) -> Clipboard {
    let compressed = compress_func(s.as_bytes());
    let compress = compressed.len() < s.as_bytes().len();
    let content = if compress {
        compressed
    } else {
        s.bytes().collect::<Vec<u8>>()
    };
    Clipboard {
        compress,
        content: content.into(),
        format: format.into(),
        ..Default::default()
    }
}

#[cfg(not(target_os = "android"))]
fn image_to_proto(a: arboard::ImageData) -> Clipboard {
    match &a {
        arboard::ImageData::Rgba(rgba) => {
            let compressed = compress_func(&a.bytes());
            let compress = compressed.len() < a.bytes().len();
            let content = if compress {
                compressed
            } else {
                a.bytes().to_vec()
            };
            Clipboard {
                compress,
                content: content.into(),
                width: rgba.width as _,
                height: rgba.height as _,
                format: ClipboardFormat::ImageRgba.into(),
                ..Default::default()
            }
        }
        arboard::ImageData::Png(png) => Clipboard {
            compress: false,
            content: png.to_owned().to_vec().into(),
            format: ClipboardFormat::ImagePng.into(),
            ..Default::default()
        },
        arboard::ImageData::Svg(_) => {
            let compressed = compress_func(&a.bytes());
            let compress = compressed.len() < a.bytes().len();
            let content = if compress {
                compressed
            } else {
                a.bytes().to_vec()
            };
            Clipboard {
                compress,
                content: content.into(),
                format: ClipboardFormat::ImageSvg.into(),
                ..Default::default()
            }
        }
    }
}

fn special_to_proto(d: Vec<u8>, s: String) -> Clipboard {
    let compressed = compress_func(&d);
    let compress = compressed.len() < d.len();
    let content = if compress {
        compressed
    } else {
        d
    };
    Clipboard {
        compress,
        content: content.into(),
        format: ClipboardFormat::Special.into(),
        special_name: s,
        ..Default::default()
    }
}

#[cfg(not(target_os = "android"))]
fn clipboard_data_to_proto(data: ClipboardData) -> Option<Clipboard> {
    let d = match data {
        ClipboardData::Text(s) => plain_to_proto(s, ClipboardFormat::Text),
        ClipboardData::Rtf(s) => plain_to_proto(s, ClipboardFormat::Rtf),
        ClipboardData::Html(s) => plain_to_proto(s, ClipboardFormat::Html),
        ClipboardData::Image(a) => image_to_proto(a),
        ClipboardData::Special((s, d)) => special_to_proto(d, s),
        _ => return None,
    };
    Some(d)
}

#[cfg(not(target_os = "android"))]
pub fn create_multi_clipboards(vec_data: Vec<ClipboardData>) -> MultiClipboards {
    MultiClipboards {
        clipboards: vec_data
            .into_iter()
            .filter_map(clipboard_data_to_proto)
            .collect(),
        ..Default::default()
    }
}

#[cfg(not(target_os = "android"))]
fn from_clipboard(clipboard: Clipboard) -> Option<ClipboardData> {
    let data = if clipboard.compress {
        decompress(&clipboard.content)
    } else {
        clipboard.content.into()
    };
    match clipboard.format.enum_value() {
        Ok(ClipboardFormat::Text) => String::from_utf8(data).ok().map(ClipboardData::Text),
        Ok(ClipboardFormat::Rtf) => String::from_utf8(data).ok().map(ClipboardData::Rtf),
        Ok(ClipboardFormat::Html) => String::from_utf8(data).ok().map(ClipboardData::Html),
        Ok(ClipboardFormat::ImageRgba) => {
            let (width, height) =
                super::valid_rgba_dimensions(clipboard.width, clipboard.height, data.len())?;
            Some(ClipboardData::Image(arboard::ImageData::rgba(
                width,
                height,
                data.into(),
            )))
        }
        Ok(ClipboardFormat::ImagePng) => {
            Some(ClipboardData::Image(arboard::ImageData::png(data.into())))
        }
        Ok(ClipboardFormat::ImageSvg) => Some(ClipboardData::Image(arboard::ImageData::svg(
            std::str::from_utf8(&data).unwrap_or_default(),
        ))),
        Ok(ClipboardFormat::Special) => {
            Some(ClipboardData::Special((clipboard.special_name, data)))
        }
        _ => None,
    }
}

#[cfg(not(target_os = "android"))]
pub fn from_multi_clipboards(multi_clipboards: Vec<Clipboard>) -> Vec<ClipboardData> {
    multi_clipboards
        .into_iter()
        .filter_map(from_clipboard)
        .collect()
}

pub fn get_msg_if_not_support_multi_clip(
    version: &str,
    platform: &str,
    multi_clipboards: &MultiClipboards,
) -> Option<Message> {
    if crate::clipboard::is_support_multi_clipboard(version, platform) {
        return None;
    }

    // Find the first text clipboard and send it.
    multi_clipboards
        .clipboards
        .iter()
        .find(|c| c.format.enum_value() == Ok(ClipboardFormat::Text))
        .map(|c| {
            let mut msg = Message::new();
            msg.set_clipboard(c.clone());
            msg
        })
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::{from_clipboard, special_to_proto};
    use arboard::ClipboardData;

    #[test]
    fn preserves_uncompressed_special_clipboard_data() {
        let data = vec![0x01, 0x02, 0x03];
        let name = "custom-format".to_owned();

        let clipboard = special_to_proto(data.clone(), name.clone());

        assert!(!clipboard.compress);
        assert_eq!(clipboard.content.as_ref(), data.as_slice());
        assert_eq!(clipboard.special_name, name);
        assert!(matches!(
            from_clipboard(clipboard),
            Some(ClipboardData::Special((restored_name, restored_data)))
                if restored_name == name && restored_data == data
        ));
    }
}
