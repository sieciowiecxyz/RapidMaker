// Included in files::tests so all global-download permit checks share one lock.
fn transfer_fixture() -> (ModelHandle, PendingDirectory) {
    let root = std::env::temp_dir().join(format!("rapidmaker-transfer-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = b"private source";
    let mesh = b"production mesh";
    fs::write(root.join("model.stl"), source).unwrap();
    fs::write(root.join("canonical.stl"), mesh).unwrap();
    let glb = gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: 0,
        },
        json: std::borrow::Cow::Borrowed(b"{}  "),
        bin: Some(std::borrow::Cow::Borrowed(b"1234")),
    }
    .to_vec()
    .unwrap();
    fs::write(root.join("preview.glb"), &glb).unwrap();
    (
        ModelHandle {
            source: root.join("model.stl"),
            mesh: root.join("canonical.stl"),
            record: StoredModel {
                id: Uuid::new_v4().to_string(),
                owner_hash: "a".repeat(64),
                user_id: None,
                filename: "część.stl".into(),
                extension: "stl".into(),
                source_bytes: source.len() as i64,
                mesh_bytes: mesh.len() as i64,
                preview_bytes: glb.len() as i64,
                sha256: "a".repeat(64),
                dimensions_mm: [20.; 3],
                expires_at: 2_000_000_000,
            },
        },
        PendingDirectory(Some(root)),
    )
}

#[tokio::test]
async fn private_transfers_preserve_bytes_headers_and_reject_invalid_files() {
    let _serial = DOWNLOAD_TEST.lock().await;
    let (handle, _cleanup) = transfer_fixture();
    for (kind, path, content_type, disposition) in [
        (
            "source",
            handle.source.clone(),
            "application/octet-stream",
            "attachment; filename=\"model.stl\"; filename*=UTF-8''cz%C4%99%C5%9B%C4%87%2Estl",
        ),
        (
            "preview.glb",
            handle.mesh.with_file_name("preview.glb"),
            "model/gltf-binary",
            "inline; filename=preview.glb",
        ),
        (
            "mesh",
            handle.mesh.clone(),
            "application/octet-stream",
            "attachment; filename=production.stl",
        ),
    ] {
        let response = artifact_response(&handle, kind, true).await.unwrap();
        assert_eq!(response.headers()[header::CONTENT_TYPE], content_type);
        assert_eq!(response.headers()[header::CONTENT_DISPOSITION], disposition);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        let expected = fs::read(path).unwrap();
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            expected.len().to_string()
        );
        assert_eq!(
            &axum::body::to_bytes(response.into_body(), 1000)
                .await
                .unwrap()[..],
            expected
        );
    }
    assert_eq!(DOWNLOADS.available_permits(), 4);
    assert_eq!(
        artifact_response(&handle, "mesh", false)
            .await
            .unwrap_err()
            .0,
        StatusCode::NOT_FOUND
    );
    fs::write(&handle.source, b"short").unwrap();
    assert_eq!(
        source_response(&handle).await.unwrap_err().0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    fs::remove_file(&handle.source).unwrap();
    assert_eq!(
        source_response(&handle).await.unwrap_err().0,
        StatusCode::GONE
    );
    std::os::unix::fs::symlink(&handle.mesh, &handle.source).unwrap();
    assert_eq!(
        source_response(&handle).await.unwrap_err().0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    fs::write(
        handle.mesh.with_file_name("preview.glb"),
        vec![0; handle.record.preview_bytes as usize],
    )
    .unwrap();
    assert_eq!(
        preview_response(&handle).await.unwrap_err().0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(DOWNLOADS.available_permits(), 4);
}

#[tokio::test]
async fn private_transfer_permits_follow_body_drop_eof_and_read_errors() {
    let _serial = DOWNLOAD_TEST.lock().await;
    let (mut handle, _cleanup) = transfer_fixture();
    let original = fs::read(&handle.source).unwrap();
    OpenOptions::new()
        .write(true)
        .open(&handle.source)
        .unwrap()
        .set_len(256 * 1024)
        .unwrap();
    handle.record.source_bytes = 256 * 1024;
    let mut bodies = Vec::new();
    for _ in 0..4 {
        bodies.push(source_response(&handle).await.unwrap().into_body());
    }
    assert_eq!(
        source_response(&handle).await.unwrap_err().0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    drop(bodies.pop());
    wait_download_permits(1).await;
    let body = source_response(&handle).await.unwrap().into_body();
    axum::body::to_bytes(body, 256 * 1024).await.unwrap();
    assert_eq!(DOWNLOADS.available_permits(), 1);
    drop(bodies);
    wait_download_permits(4).await;
    fs::write(&handle.source, &original).unwrap();
    handle.record.source_bytes = original.len() as i64;
    let body = source_response(&handle).await.unwrap().into_body();
    fs::write(&handle.source, b"").unwrap();
    let failure = axum::body::to_bytes(body, 1000).await.unwrap_err();
    assert!(
        !failure
            .to_string()
            .contains(handle.source.to_str().unwrap())
    );
    assert_eq!(DOWNLOADS.available_permits(), 4);
    fs::write(&handle.source, b"private source").unwrap();
    let body = source_response(&handle).await.unwrap().into_body();
    fs::remove_file(&handle.source).unwrap();
    assert_eq!(
        &axum::body::to_bytes(body, 1000).await.unwrap()[..],
        b"private source"
    );
}

#[tokio::test]
async fn bounded_glb_validation_matches_gltf_binary_parser() {
    let _serial = DOWNLOAD_TEST.lock().await;
    let (handle, _cleanup) = transfer_fixture();
    let path = handle.mesh.with_file_name("preview.glb");
    let valid = fs::read(&path).unwrap();
    let mut cases = vec![valid.clone()];
    for length in 0..valid.len() {
        cases.push(valid[..length].to_vec());
    }
    // Cover each envelope field and payload bytes, including accepted trailing data.
    for position in 0..valid.len() {
        let mut bytes = valid.clone();
        bytes[position] ^= 255;
        cases.push(bytes);
    }
    let mut trailing = valid.clone();
    trailing.extend_from_slice(b"extra");
    cases.push(trailing);
    let mut json_only = valid[..24].to_vec();
    json_only[8..12].copy_from_slice(&24u32.to_le_bytes());
    cases.push(json_only);
    let mut short_declared = valid.clone();
    short_declared[8..12].copy_from_slice(&12u32.to_le_bytes());
    cases.push(short_declared);
    for bytes in cases {
        // gltf subtracts 12 without checking and panics for a shorter declared length.
        let declared = bytes
            .get(8..12)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let expected =
            declared.is_some_and(|n| n >= 12) && gltf::binary::Glb::from_slice(&bytes).is_ok();
        fs::write(&path, &bytes).unwrap();
        let mut file = tokio::fs::File::open(&path).await.unwrap();
        assert_eq!(
            validate_glb(&mut file, bytes.len() as u64).await.is_ok(),
            expected,
            "{bytes:?}"
        );
        if expected {
            let mut returned = Vec::new();
            file.read_to_end(&mut returned).await.unwrap();
            assert_eq!(returned, bytes);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn stalled_upload_deadline_releases_admission_and_allows_recovery() {
    let gate = Semaphore::new(1);
    let pending =
        Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
    let future = async {
        let _permit = gate.try_acquire().unwrap();
        upload_body(Request::new(pending)).await
    };
    tokio::pin!(future);
    tokio::select! { _ = &mut future => panic!("deadline fired early"), _ = tokio::task::yield_now() => {} }
    assert!(gate.try_acquire().is_err());
    tokio::time::advance(Duration::from_secs(59)).await;
    tokio::select! { _ = &mut future => panic!("deadline fired early"), _ = tokio::task::yield_now() => {} }
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(
        future.await.unwrap_err().status(),
        StatusCode::REQUEST_TIMEOUT
    );
    let _permit = gate.try_acquire().unwrap();
    assert_eq!(
        upload_body(Request::new(Body::from("recovered")))
            .await
            .unwrap(),
        "recovered"
    );
}

#[tokio::test]
async fn four_concurrent_sparse_200mib_transfers_have_bounded_chunks() {
    let _serial = DOWNLOAD_TEST.lock().await;
    let (handle, _cleanup) = transfer_fixture();
    let length = ModelProfile::Large.stl_bytes();
    OpenOptions::new()
        .write(true)
        .open(&handle.mesh)
        .unwrap()
        .set_len(length as u64)
        .unwrap();
    let rss = || {
        fs::read_to_string("/proc/self/status")
            .unwrap()
            .lines()
            .find_map(|line| {
                line.strip_prefix("VmRSS:")
                    .map(|n| n.split_whitespace().next().unwrap().parse::<u64>().unwrap())
            })
            .unwrap()
    };
    let initial = rss();
    let mut peak = initial;
    let mut streams = Vec::new();
    for _ in 0..4 {
        streams.push(
            private_file(&handle.mesh, length as i64, length)
                .await
                .unwrap()
                .body()
                .1
                .into_data_stream(),
        );
        peak = peak.max(rss());
    }
    let mut totals = [0; 4];
    loop {
        let mut active = false;
        for (index, stream) in streams.iter_mut().enumerate() {
            if let Some(bytes) = stream.next().await {
                let bytes = bytes.unwrap();
                assert!(bytes.len() <= DOWNLOAD_CHUNK_BYTES);
                totals[index] += bytes.len();
                active = true;
            }
        }
        if !active {
            break;
        }
        if totals[0] % (8 * 1024 * 1024) == 0 {
            peak = peak.max(rss());
        }
    }
    assert_eq!(totals, [length; 4]);
    assert_eq!(DOWNLOADS.available_permits(), 4);
    eprintln!(
        "4 x {length} bytes; 64KiB max chunks; VmRSS initial={initial} KiB peak={peak} KiB growth={} KiB",
        peak.saturating_sub(initial)
    );
    // Isolated focused run: this leaves ample allocator/runtime slack, but catches
    // the old whole-file buffering (800MiB) without relying on a tight RSS value.
    assert!(peak.saturating_sub(initial) < 64 * 1024);
}

#[tokio::test]
async fn upload_body_keeps_configured_limit_and_maps_broken_bodies() {
    use tower::ServiceExt;
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::post(|request: Request| async {
                match upload_body(request).await {
                    Ok(bytes) => {
                        let address = bytes.as_ptr() as usize;
                        let extracted = Bytes::from_request(Request::new(Body::from(bytes)), &())
                            .await
                            .unwrap();
                        assert_eq!(
                            extracted.as_ptr() as usize,
                            address,
                            "reconstructed body must share Bytes allocation"
                        );
                        extracted.into_response()
                    }
                    Err(response) => response,
                }
            }),
        )
        .layer(DefaultBodyLimit::max(8));
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .body(Body::from("123456789"))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .body(Body::from("12345678"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        &axum::body::to_bytes(response.into_body(), 8).await.unwrap()[..],
        b"12345678"
    );
    let broken = Body::from_stream(futures_util::stream::once(async {
        Err::<Bytes, _>(std::io::Error::other("private path must not escape"))
    }));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .body(broken)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        !String::from_utf8(
            axum::body::to_bytes(response.into_body(), 1000)
                .await
                .unwrap()
                .to_vec()
        )
        .unwrap()
        .contains("private path")
    );
}

async fn wait_download_permits(expected: usize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while DOWNLOADS.available_permits() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn private_transfer_deadline_releases_unpolled_bodies_and_reports_partial_error() {
    let _serial = DOWNLOAD_TEST.lock().await;
    let (handle, _cleanup) = transfer_fixture();
    let length = 256 * 1024;
    OpenOptions::new()
        .write(true)
        .open(&handle.mesh)
        .unwrap()
        .set_len(length as u64)
        .unwrap();
    let mut files = Vec::new();
    for _ in 0..4 {
        files.push(
            private_file(&handle.mesh, length as i64, length)
                .await
                .unwrap(),
        );
    }
    tokio::time::pause();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    let mut bodies = files
        .into_iter()
        .map(|file| file.body_at(deadline).1)
        .collect::<Vec<_>>();
    assert_eq!(DOWNLOADS.available_permits(), 0);
    tokio::time::advance(Duration::from_secs(301)).await;
    wait_download_permits(4).await;
    assert_eq!(
        bodies.len(),
        4,
        "deadline releases permits without body drop"
    );
    let failure = axum::body::to_bytes(bodies.pop().unwrap(), length)
        .await
        .unwrap_err();
    assert!(failure.to_string().contains("Private model read failed"));
    assert!(!failure.to_string().contains(handle.mesh.to_str().unwrap()));
    drop(bodies);
    let file = private_file(&handle.mesh, length as i64, length)
        .await
        .unwrap();
    drop(file);
    assert_eq!(DOWNLOADS.available_permits(), 4);
    tokio::time::resume();
    let file = private_file(&handle.mesh, length as i64, length)
        .await
        .unwrap();
    let mut stream = file.body().1.into_data_stream();
    assert!(!stream.next().await.unwrap().unwrap().is_empty());
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(301)).await;
    wait_download_permits(4).await;
    let mut failed = false;
    while let Some(chunk) = stream.next().await {
        if let Err(error) = chunk {
            assert!(error.to_string().contains("Private model read failed"));
            failed = true;
            break;
        }
    }
    assert!(
        failed,
        "a partially sent timed-out body must not end successfully"
    );
}

#[tokio::test]
async fn private_download_socket_deadline_recovers_with_receivers_still_open() {
    use std::os::fd::AsRawFd;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let _serial = DOWNLOAD_TEST.lock().await;
    assert_eq!(DOWNLOADS.available_permits(), 4);
    let directory =
        std::env::temp_dir().join(format!("rapidmaker-socket-probe-{}", Uuid::new_v4()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let _cleanup = PendingDirectory(Some(directory.clone()));
    let path = directory.join("benign-sparse.stl");
    let length = 32 * 1024 * 1024;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap()
        .set_len(length as u64)
        .unwrap();
    let app = axum::Router::new().route(
        "/fixture",
        axum::routing::get(move || {
            let path = path.clone();
            async move {
                match private_file(&path, length as i64, length).await {
                    Ok(file) => file
                        .body_at(tokio::time::Instant::now() + Duration::from_secs(2))
                        .into_response(),
                    Err(error) => error.into_response(),
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    struct AbortServer(tokio::task::JoinHandle<()>);
    impl Drop for AbortServer {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _server = AbortServer(server);
    async fn request(address: std::net::SocketAddr) -> (tokio::net::TcpStream, String) {
        let socket = tokio::net::TcpStream::connect(address).await.unwrap();
        let value: libc::c_int = 1024;
        unsafe {
            assert_eq!(
                libc::setsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_RCVBUF,
                    &value as *const _ as *const libc::c_void,
                    std::mem::size_of_val(&value) as libc::socklen_t
                ),
                0
            );
        }
        let mut socket = socket;
        socket
            .write_all(b"GET /fixture HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            socket.read_exact(&mut byte).await.unwrap();
            headers.push(byte[0]);
            assert!(headers.len() < 4096);
        }
        (socket, String::from_utf8(headers).unwrap())
    }
    let result = tokio::time::timeout(Duration::from_secs(18), async {
        let mut receivers = Vec::new();
        for index in 0..4 {
            let (socket, headers) = request(address).await;
            assert!(headers.starts_with("HTTP/1.1 200"), "{headers}");
            receivers.push(socket);
            eprintln!("receiver {}: 200 headers read, body unconsumed", index + 1);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(DOWNLOADS.available_permits(), 0);
        let (fifth, headers) = request(address).await;
        eprintln!(
            "fifth while sockets stalled: {}",
            headers.lines().next().unwrap()
        );
        assert!(headers.starts_with("HTTP/1.1 503"), "{headers}");
        drop(fifth);
        let recovery = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let (socket, headers) = request(address).await;
                drop(socket);
                if headers.starts_with("HTTP/1.1 200") {
                    eprintln!(
                        "after deadline with original four sockets still open: 200 recovered"
                    );
                    break;
                }
                assert!(headers.starts_with("HTTP/1.1 503"), "{headers}");
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
        })
        .await;
        assert!(recovery.is_ok(), "download capacity did not recover");
        assert_eq!(receivers.len(), 4);
        drop(receivers);
        wait_download_permits(4).await;
    })
    .await;
    assert!(result.is_ok(), "local transport probe exceeded 18 seconds");
}
