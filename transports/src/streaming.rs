//! Connect stream termination and framing, independent of the HTTP client/server.
use crate::framing::{self, Envelope};
use arut_rpc::{Code, RpcStream, Status};
use bytes::BytesMut;
use futures_util::{Stream, StreamExt, stream};
use std::convert::Infallible;
use tokio_util::codec::Decoder;

pub(crate) fn decode<B: AsRef<[u8]> + Send + 'static>(
    input: impl Stream<Item = Result<B, Status>> + Send + 'static,
) -> RpcStream<Vec<u8>> {
    Box::pin(stream::unfold(
        Some((Box::pin(input), BytesMut::new())),
        |state| async move {
            let (mut input, mut buffer) = state?;
            loop {
                match Envelope.decode(&mut buffer) {
                    Ok(Some((0, body))) => return Some((Ok(body), Some((input, buffer)))),
                    Ok(Some((_, body))) => {
                        let end: serde_json::Value = match serde_json::from_slice(&body) {
                            Ok(end) => end,
                            Err(_) => {
                                return Some((
                                    Err(Status::new(Code::Internal, "invalid end envelope")),
                                    None,
                                ));
                            }
                        };
                        return end
                            .get("error")
                            .map(|error| (Err(framing::parse_error(error)), None));
                    }
                    Err(error) => return Some((Err(error), None)),
                    Ok(None) => {}
                }
                match input.next().await {
                    Some(Ok(bytes)) => buffer.extend_from_slice(bytes.as_ref()),
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => {
                        return Some((
                            Err(Status::new(
                                Code::Internal,
                                "stream ended without end envelope",
                            )),
                            None,
                        ));
                    }
                }
            }
        },
    ))
}

pub(crate) fn encode(input: RpcStream<Vec<u8>>) -> impl Stream<Item = Result<Vec<u8>, Infallible>> {
    stream::unfold(Some(input), |state| async move {
        let mut input = state?;
        let error = match input.next().await {
            Some(Ok(body)) => match framing::envelope(0, &body) {
                Ok(envelope) => return Some((Ok(envelope), Some(input))),
                Err(error) => Some(error),
            },
            Some(Err(error)) => Some(error),
            None => None,
        };
        Some((Ok(framing::end_envelope(error)), None))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;

    fn collect<T>(input: impl Stream<Item = T>) -> Vec<T> {
        input
            .collect()
            .now_or_never()
            .expect("fixture stream is ready")
    }

    #[test]
    fn fragmented_and_coalesced_envelopes_have_the_same_messages() {
        let input = Box::pin(stream::iter([
            Ok(b"first".to_vec()),
            Ok(b"second".to_vec()),
        ]));
        let wire: Vec<u8> = collect(encode(input))
            .into_iter()
            .flat_map(Result::unwrap)
            .collect();
        for size in [1, 3, wire.len()] {
            let chunks: Vec<_> = wire.chunks(size).map(|chunk| Ok(chunk.to_vec())).collect();
            assert_eq!(
                collect(decode(stream::iter(chunks))),
                [Ok(b"first".to_vec()), Ok(b"second".to_vec())]
            );
        }
    }

    #[test]
    fn terminal_error_preserves_details_and_stops_before_later_messages() {
        let mut error = Status::new(Code::Aborted, "retry");
        error.details = vec![1, 2];
        let input = Box::pin(stream::iter([Err(error.clone()), Ok(b"ignored".to_vec())]));
        let envelopes = collect(encode(input));
        assert_eq!(envelopes.len(), 1);
        let wire = envelopes.into_iter().map(|envelope| Ok(envelope.unwrap()));
        assert_eq!(collect(decode(stream::iter(wire))), [Err(error)]);
    }

    #[test]
    fn missing_malformed_and_oversized_envelopes_end_with_one_error() {
        let oversized = [
            vec![0],
            ((framing::MAX_MESSAGE + 1) as u32).to_be_bytes().to_vec(),
        ]
        .concat();
        for (wire, code) in [
            (vec![], Code::Internal),
            (vec![0, 0, 0], Code::Internal),
            (framing::envelope(2, b"{").unwrap(), Code::Internal),
            (oversized, Code::ResourceExhausted),
        ] {
            let results = collect(decode(stream::iter([Ok(wire)])));
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].as_ref().unwrap_err().code, code);
        }
    }

    #[test]
    fn oversized_output_becomes_a_terminal_error() {
        let input = Box::pin(stream::iter([Ok(vec![0; framing::MAX_MESSAGE + 1])]));
        let wire = collect(encode(input))
            .into_iter()
            .map(|item| Ok(item.unwrap()));
        let results = collect(decode(stream::iter(wire)));
        assert_eq!(results, [Err(framing::message_limit())]);
    }
}
