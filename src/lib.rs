#![forbid(unsafe_code)]

//! ASC X12: an interchange as its segments, one part each, named by the
//! tag — `ISA`, `GS`, `ST`, `BEG`, `SE`, `GE`, `IEA` — in the order they
//! lie. The delimiters are the ones the fixed-width `ISA` envelope carries:
//! the element separator is its fourth character, the component separator
//! ISA16 at position 104 and the segment terminator the character after it,
//! at 105. The announced type is ST01 of the first `ST`, the transaction set
//! identifier — `850`, `810`, `856` — when there is an `ST` (ADR-0047).
//!
//! The walk is the Foundation's `message::segment`: this shape reads the
//! envelope and brings the delimiters. A contract checks everything else.

use message::segment::{self, Delimiters};
use message::{Shape, ShapeError, Shaped};
use stream::Stream;

/// The ASC X12 shape.
#[derive(Clone, Copy, Debug, Default)]
pub struct X12;

/// The length of an `ISA` with its terminator: every element fixed width.
pub const ISA_LENGTH: usize = 106;

/// The delimiters the `ISA` envelope carries.
///
/// # Errors
/// No `ISA` at the head, or one shorter than its 106 characters.
pub fn delimiters(bytes: &[u8]) -> Result<Delimiters, ShapeError> {
    if !bytes.starts_with(b"ISA") {
        return Err(ShapeError::new("edi-x12", "no ISA at the head").at(0));
    }
    let Some(isa) = bytes.get(..ISA_LENGTH) else {
        return Err(
            ShapeError::new("edi-x12", "an ISA shorter than its 106 characters").at(bytes.len()),
        );
    };
    Ok(Delimiters::new(isa[105], isa[3], isa[104]))
}

/// Whether the head is an `ISA` whose fixed-width envelope holds: the
/// element separator after the tag recurs where ISA01, ISA02 and ISA15 end.
fn envelope_holds(bytes: &[u8]) -> bool {
    bytes.starts_with(b"ISA")
        && bytes.len() >= ISA_LENGTH
        && [6, 17, 103].iter().all(|&at| bytes[at] == bytes[3])
}

impl Shape for X12 {
    fn technology(&self) -> &'static str {
        "edi-x12"
    }

    fn media_types(&self) -> &'static [&'static str] {
        &["application/edi-x12"]
    }

    fn recognises(&self, bytes: &[u8]) -> bool {
        envelope_holds(bytes)
    }

    fn shape(&self, stream: &Stream) -> Result<Shaped, ShapeError> {
        let delimiters = delimiters(stream.bytes())?;
        let segments = segment::segments(stream.bytes(), &delimiters)
            .map_err(|stop| ShapeError::refused("edi-x12", stop))?;
        let message_type = segment::first(&segments, "ST")
            .and_then(|st| st.element(0, &delimiters))
            .filter(|kind| !kind.is_empty())
            .map(|kind| String::from_utf8_lossy(kind).into_owned());
        let media = stream.media_type().unwrap_or("application/edi-x12");
        Ok(Shaped {
            parts: segment::parts(&segments, media),
            message_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcore::StreamId;

    const ISA: &[u8] = b"ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       \
*240101*1200*U*00401*000000001*0*P*:~";

    fn order() -> Vec<u8> {
        [
            ISA,
            b"\nGS*PO*SENDER*RECEIVER*20240101*1200*1*X*004010~\nST*850*0001~\n",
            b"BEG*00*SA*PO-1**20240101~\nSE*3*0001~\nGE*1*1~\nIEA*1*000000001~\n",
        ]
        .concat()
    }

    fn stream(bytes: &[u8], media: Option<&str>) -> Stream {
        Stream::new(StreamId::new(1), bytes.to_vec(), media.map(str::to_string))
    }

    #[test]
    fn an_interchange_is_its_segments_named_by_tag_and_st01_is_the_announced_type() {
        let shaped = X12.shape(&stream(&order(), None)).expect("well-formed");
        let names: Vec<&str> = shaped
            .parts
            .iter()
            .filter_map(|p| p.name.as_deref())
            .collect();
        assert_eq!(names, ["ISA", "GS", "ST", "BEG", "SE", "GE", "IEA"]);
        assert_eq!(shaped.parts[0].bytes, &ISA[..105]);
        assert_eq!(shaped.parts[2].bytes, b"ST*850*0001");
        assert_eq!(
            shaped.parts[3].media_type.as_deref(),
            Some("application/edi-x12")
        );
        assert_eq!(shaped.message_type.as_deref(), Some("850"));

        let typed = X12
            .shape(&stream(
                &order(),
                Some("application/edi-x12; charset=us-ascii"),
            ))
            .expect("well-formed");
        assert_eq!(
            typed.parts[0].media_type.as_deref(),
            Some("application/edi-x12; charset=us-ascii")
        );
    }

    #[test]
    fn the_envelope_announces_other_separators_and_without_st_nothing_is_announced() {
        let mut other = order();
        for byte in &mut other {
            *byte = match *byte {
                b'*' => b'|',
                b'~' => b'!',
                b':' => b'>',
                b => b,
            };
        }
        let shaped = X12.shape(&stream(&other, None)).expect("well-formed");
        assert_eq!(shaped.parts.len(), 7);
        assert_eq!(shaped.parts[2].bytes, b"ST|850|0001");
        assert_eq!(shaped.message_type.as_deref(), Some("850"));
        assert_eq!(delimiters(&other), Ok(Delimiters::new(b'!', b'|', b'>')));

        let envelope_only = [ISA, b"IEA*0*000000001~"].concat();
        let shaped = X12
            .shape(&stream(&envelope_only, None))
            .expect("well-formed");
        assert_eq!(shaped.parts.len(), 2);
        assert_eq!(shaped.message_type, None);
    }

    #[test]
    fn a_missing_or_short_isa_or_a_cut_segment_is_refused_where_it_fails() {
        let none = X12
            .shape(&stream(b"UNB+UNOA:2'", None))
            .expect_err("no ISA");
        assert_eq!(none.offset, Some(0));
        assert_eq!(none.to_string(), "edi-x12: no ISA at the head at byte 0");

        let short = X12.shape(&stream(&ISA[..80], None)).expect_err("short");
        assert_eq!(short.offset, Some(80));
        assert_eq!(short.reason, "an ISA shorter than its 106 characters");

        let cut = [ISA, b"GS*PO*S*R"].concat();
        let cut = X12.shape(&stream(&cut, None)).expect_err("no terminator");
        assert_eq!(cut.offset, Some(106));
        assert_eq!(cut.reason, "a segment without its terminator");
    }

    #[test]
    fn the_shape_claims_x12_and_recognises_an_isa_whose_envelope_holds() {
        assert_eq!(X12.technology(), "edi-x12");
        assert_eq!(X12.media_types(), &["application/edi-x12"]);
        assert!(X12.recognises(ISA));
        assert!(!X12.recognises(&ISA[..100]));
        assert!(!X12.recognises(
            b"ISA is a word that opens this long enough text to pass the length \
check but not the envelope check, which wants separators in place."
        ));
        assert!(!X12.recognises(b"UNB+"));

        let shapes: [&dyn Shape; 1] = [&X12];
        let by_media = message::choose(&shapes, &stream(b"x", Some("application/EDI-X12")));
        assert_eq!(by_media.map(Shape::technology), Some("edi-x12"));
        let by_look = message::choose(&shapes, &stream(&order(), None));
        assert_eq!(by_look.map(Shape::technology), Some("edi-x12"));
        assert!(message::choose(&shapes, &stream(b"STX=", None)).is_none());
    }
}
