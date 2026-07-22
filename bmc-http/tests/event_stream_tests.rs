// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod common;

#[cfg(feature = "reqwest")]
mod tests {
    use crate::common::test_utils::*;
    use futures_util::StreamExt;
    use nv_redfish_bmc_http::reqwest::BmcError;
    use nv_redfish_core::Bmc;
    use serde::Deserialize;
    use serde_json::Value as JsonValue;
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    const SSE_URI: &str = "/redfish/v1/EventService/SSE";

    #[derive(Debug, Deserialize, PartialEq)]
    struct StreamPayload {
        event_id: String,
        severity: String,
    }

    #[tokio::test]
    async fn test_event_stream_reads_typed_json() {
        let mock_server = MockServer::start().await;
        let sse_body = concat!(
            "event: Alert\n",
            "data: {\"event_id\":\"1\",\"severity\":\"Critical\"}\n\n",
            "event: StatusChange\n",
            "data: {\"event_id\":\"2\",\"severity\":\"OK\"}\n\n"
        );

        Mock::given(method("GET"))
            .and(path(SSE_URI))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("accept", "text/event-stream"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_body),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);
        let mut stream = bmc
            .stream::<JsonValue>(SSE_URI)
            .await
            .expect("must open stream");

        let first = stream
            .next()
            .await
            .expect("first event expected")
            .expect("first event parse");
        assert_eq!(
            first,
            serde_json::json!({
                "event_id": "1",
                "severity": "Critical"
            })
        );

        let second = stream
            .next()
            .await
            .expect("second event expected")
            .expect("second event parse");
        assert_eq!(
            second,
            serde_json::json!({
                "event_id": "2",
                "severity": "OK"
            })
        );

        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_event_stream_json_decodes_payload() {
        let mock_server = MockServer::start().await;
        let sse_body = concat!(
            "event: Alert\n",
            "data: {\"event_id\":\"10\",\"severity\":\"Warning\"}\n\n",
            "event: Alert\n",
            "data: {\"event_id\":\"11\",\"severity\":\"Critical\"}\n\n"
        );

        Mock::given(method("GET"))
            .and(path(SSE_URI))
            .and(header("authorization", "Basic cm9vdDpwYXNzd29yZA=="))
            .and(header("accept", "text/event-stream"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_body),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let bmc = create_test_bmc(&mock_server);
        let mut stream = bmc
            .stream::<StreamPayload>(SSE_URI)
            .await
            .expect("must open stream");

        let first = stream
            .next()
            .await
            .expect("first event expected")
            .expect("first event parse");
        assert_eq!(
            first,
            StreamPayload {
                event_id: "10".to_string(),
                severity: "Warning".to_string(),
            }
        );

        let second = stream
            .next()
            .await
            .expect("second event expected")
            .expect("second event parse");
        assert_eq!(
            second,
            StreamPayload {
                event_id: "11".to_string(),
                severity: "Critical".to_string(),
            }
        );

        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn sse_aborts_when_event_exceeds_byte_limit() {
        use nv_redfish_bmc_http::reqwest::{Client, ClientParams};
        use nv_redfish_bmc_http::{CacheSettings, HttpBmc};
        use url::Url;

        let mock_server = MockServer::start().await;

        // 6-byte prefix ("data: ") + 20 bytes of data + newline = 27 bytes total,
        // well over the 16-byte limit. No event terminator (\n\n) so the decoder
        // never emits a complete event — the counter fires first.
        let sse_body = format!("data: {}\n", "x".repeat(20));

        Mock::given(method("GET"))
            .and(path(SSE_URI))
            .and(header("accept", "text/event-stream"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse_body),
            )
            .mount(&mock_server)
            .await;

        let client = Client::with_params(ClientParams::new().sse_max_event_bytes(16)).unwrap();
        let bmc = HttpBmc::new(
            client,
            Url::parse(&mock_server.uri()).unwrap(),
            create_test_credentials(),
            CacheSettings::default(),
        );

        let mut stream = bmc
            .stream::<JsonValue>(SSE_URI)
            .await
            .expect("stream must open");

        let result = stream.next().await.expect("expected an error item");
        assert!(
            matches!(result, Err(BmcError::SseEventTooLarge { limit: 16 })),
            "expected SseEventTooLarge, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn sse_aborts_on_idle_timeout() {
        use nv_redfish_bmc_http::reqwest::{Client, ClientParams};
        use nv_redfish_bmc_http::{CacheSettings, HttpBmc};
        use std::time::Duration;
        use tokio::io::AsyncWriteExt as _;
        use tokio::net::TcpListener;
        use url::Url;

        // Bind to an OS-assigned port so we never collide with other tests.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn a minimal HTTP server that sends one SSE event then stalls,
        // keeping the connection open so the client has nothing to time out on.
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\n\
                      Content-Type: text/event-stream\r\n\
                      Connection: keep-alive\r\n\
                      \r\n\
                      data: {}\n\n",
                )
                .await
                .unwrap();
            socket.flush().await.unwrap();
            // Hold the connection open so the idle timeout is what fires, not EOF.
            tokio::time::sleep(Duration::from_secs(30)).await;
        });

        let client =
            Client::with_params(ClientParams::new().sse_idle_timeout(Duration::from_millis(100)))
                .unwrap();
        let bmc = HttpBmc::new(
            client,
            Url::parse(&format!("http://{addr}")).unwrap(),
            create_test_credentials(),
            CacheSettings::default(),
        );

        let mut stream = bmc
            .stream::<JsonValue>(SSE_URI)
            .await
            .expect("stream must open");

        // First poll delivers the one event the server sent.
        let first = stream
            .next()
            .await
            .expect("first event expected")
            .expect("first event must be Ok");
        assert_eq!(first, serde_json::json!({}));

        // Second poll blocks until the idle timeout fires.
        let result = stream.next().await.expect("expected an error item");
        assert!(
            matches!(result, Err(BmcError::SseIdleTimeout { .. })),
            "expected SseIdleTimeout, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_event_stream_rejects_cross_origin_uri() {
        let mock_server = MockServer::start().await;
        let bmc = create_test_bmc(&mock_server);

        let result = bmc
            .stream::<JsonValue>("https://bmc.example.evil/redfish/v1/EventService/SSE")
            .await;

        assert!(matches!(result, Err(BmcError::InvalidRequest(_))));
    }
}
