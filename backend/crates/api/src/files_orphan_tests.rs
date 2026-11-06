use super::*;
use sqlx::PgPool;

fn old_directory(root: &Path, name: &str, timestamp: i64) -> PathBuf {
    let path = root.join(name);
    DirBuilder::new().mode(0o700).create(&path).unwrap();
    fs::write(path.join("sentinel"), b"owned test data").unwrap();
    File::open(&path)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(UNIX_EPOCH + Duration::from_secs((timestamp - 86401) as u64)),
        )
        .unwrap();
    path
}

async fn register(db: &Storage, id: &str, timestamp: i64) {
    assert!(
        db.insert_model(&StoredModel {
            id: id.into(),
            owner_hash: "a".repeat(64),
            user_id: None,
            filename: "test.stl".into(),
            extension: "stl".into(),
            source_bytes: 100,
            mesh_bytes: 100,
            preview_bytes: 0,
            sha256: "b".repeat(64),
            dimensions_mm: [10.; 3],
            expires_at: timestamp - 1,
        })
        .await
        .unwrap()
    );
}

#[tokio::test]
#[ignore = "requires isolated TEST_DATABASE_URL"]
async fn aged_orphans_are_fenced_bounded_and_fail_closed() {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let parsed = reqwest::Url::parse(&url).unwrap();
    let name = parsed.path().strip_prefix('/').unwrap();
    // Guard BEFORE Storage::connect (which runs migrations) or any mutation.
    assert!(
        matches!(parsed.scheme(), "postgres" | "postgresql")
            && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1"))
            && parsed.port() == Some(18434)
            && parsed.query().is_none()
            && parsed.fragment().is_none()
            && name.starts_with("rapidmaker_test_")
            && name.len() > "rapidmaker_test_".len()
            && name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    );
    let db = Storage::connect(&url).await.unwrap();
    let pool = PgPool::connect(&url).await.unwrap();
    let root = std::env::temp_dir().join(format!("rapidmaker-orphans-{}", Uuid::new_v4()));
    DirBuilder::new().mode(0o700).create(&root).unwrap();
    let _owned_root = PendingDirectory(Some(root.clone()));
    let outside =
        std::env::temp_dir().join(format!("rapidmaker-orphan-outside-{}", Uuid::new_v4()));
    DirBuilder::new().mode(0o700).create(&outside).unwrap();
    let _owned_outside = PendingDirectory(Some(outside.clone()));
    let timestamp = now().unwrap();
    let external = old_directory(&outside, "external", timestamp);
    let state = FileState {
        storage: Some(db.clone()),
        root: root.clone(),
        profile: ModelProfile::Standard,
    };
    let mut cursor = ModelCleanupCursor::default();
    let orphan = old_directory(&root, &Uuid::new_v4().to_string(), timestamp);
    let fresh = root.join(Uuid::new_v4().to_string());
    DirBuilder::new().mode(0o700).create(&fresh).unwrap();
    let future = old_directory(&root, &Uuid::new_v4().to_string(), timestamp);
    File::open(&future)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(UNIX_EPOCH + Duration::from_secs((timestamp + 86400) as u64)),
        )
        .unwrap();
    let junk = old_directory(&root, "not-a-uuid", timestamp);
    let symlink = root.join(Uuid::new_v4().to_string());
    std::os::unix::fs::symlink(&external, &symlink).unwrap();
    let registered_id = Uuid::new_v4().to_string();
    let registered = old_directory(&root, &registered_id, timestamp);
    register(&db, &registered_id, timestamp).await;
    let deleting_id = Uuid::new_v4().to_string();
    let deleting = old_directory(&root, &deleting_id, timestamp);
    register(&db, &deleting_id, timestamp).await;
    sqlx::query("UPDATE private_models SET deletion_started=true WHERE id=$1")
        .bind(&deleting_id)
        .execute(&pool)
        .await
        .unwrap();
    let job_id = Uuid::new_v4().to_string();
    let job = old_directory(&root, &job_id, timestamp);
    sqlx::query("INSERT INTO slicer_jobs(id,owner_hash,model_id,fingerprint,payload,available_at,created_at,expires_at) VALUES($1,$2,$1,'orphan-test','{}',$3,$3,$4)")
        .bind(&job_id).bind("a".repeat(64)).bind(timestamp).bind(timestamp+3600).execute(&pool).await.unwrap();
    let running_id = Uuid::new_v4().to_string();
    let running = old_directory(&root, &running_id, timestamp);
    sqlx::query("INSERT INTO slicer_jobs(id,owner_hash,model_id,fingerprint,payload,status,attempts,lease_token,lease_until,available_at,created_at,expires_at) VALUES($1,$2,$1,'orphan-running-test','{}','running',1,'test-lease',$3,$4,$4,$4)")
        .bind(&running_id).bind("a".repeat(64)).bind(timestamp+3600).bind(timestamp-1).execute(&pool).await.unwrap();
    let held_id = Uuid::new_v4().to_string();
    let held = old_directory(&root, &held_id, timestamp);
    sqlx::query("INSERT INTO quotes(id,request,result) VALUES($1,'{}','{}')")
        .bind(&held_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO order_file_holds(order_id,owner_hash,created_at) VALUES($1,$2,$3)")
        .bind(&held_id)
        .bind("a".repeat(64))
        .bind(timestamp)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO order_files(order_id,quote_id,model_id,filename) VALUES($1,$1,$1,'test.stl')",
    )
    .bind(&held_id)
    .execute(&pool)
    .await
    .unwrap();

    // The exact helper used by upload holds another fresh FD; contention must not advance.
    let upload_lock = lock_model_root(&root).unwrap().unwrap();
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(cursor.scan.is_none() && orphan.exists());
    assert!(lock_model_root(&root).unwrap().is_none());
    drop(upload_lock);
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(!orphan.exists());
    for path in [
        &fresh,
        &future,
        &junk,
        &registered,
        &deleting,
        &job,
        &running,
        &held,
        &external,
    ] {
        assert!(path.exists());
    }
    assert!(
        fs::symlink_metadata(&symlink)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let uncertain_id = Uuid::new_v4().to_string();
    let uncertain = old_directory(&root, &uncertain_id, timestamp);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(72617069646)")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO private_models(id,owner_hash,filename,extension,source_bytes,mesh_bytes,sha256,dimensions_mm,expires_at) VALUES($1,$2,'test.stl','stl',100,100,$2,ARRAY[10.,10.,10.],$3)")
        .bind(&uncertain_id).bind("a".repeat(64)).bind(timestamp-1).execute(&mut *tx).await.unwrap();
    cursor.scan = None;
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(uncertain.exists());
    tx.commit().await.unwrap();
    cursor.scan = None;
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(uncertain.exists());

    // Fail the presence query on our isolated schema; restore it before assertions.
    let failure_id = Uuid::new_v4().to_string();
    let failure = old_directory(&root, &failure_id, timestamp);
    let failure_db = Storage::connect(&url).await.unwrap();
    sqlx::query("ALTER TABLE private_models RENAME TO orphan_test_models")
        .execute(&pool)
        .await
        .unwrap();
    let failure_state = FileState {
        storage: Some(failure_db),
        ..state.clone()
    };
    cursor.scan = None;
    let result = cleanup_orphans(&failure_state, timestamp, &mut cursor).await;
    sqlx::query("ALTER TABLE orphan_test_models RENAME TO private_models")
        .execute(&pool)
        .await
        .unwrap();
    assert!(result.is_err() && failure.exists());
    cursor.scan = None;
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(!failure.exists());

    // One filesystem failure cannot prevent another eligible directory being removed.
    let failing = old_directory(&root, &Uuid::new_v4().to_string(), timestamp);
    fs::set_permissions(&failing, fs::Permissions::from_mode(0o500)).unwrap();
    let succeeding = old_directory(&root, &Uuid::new_v4().to_string(), timestamp);
    cursor.scan = None;
    let removal = cleanup_orphans(&state, timestamp, &mut cursor).await;
    // Restore our own permissions before assertions/RAII cleanup, even after failure.
    fs::set_permissions(&failing, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(removal.is_ok() && failing.exists() && !succeeding.exists());

    // More entries than one batch, regardless of filesystem enumeration order.
    for n in 0..150 {
        old_directory(&root, &format!("junk-{n}"), timestamp);
    }
    let mut later = Vec::new();
    for _ in 0..80 {
        later.push(old_directory(&root, &Uuid::new_v4().to_string(), timestamp));
    }
    cursor.scan = None;
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(cursor.scan.is_some());
    assert!(later.iter().any(|p| p.exists())); // At most 64 entries advanced.
    // Busy roots preserve the position of a partially consumed cursor.
    let upload_lock = lock_model_root(&root).unwrap().unwrap();
    let remaining = later.iter().filter(|p| p.exists()).count();
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert_eq!(remaining, later.iter().filter(|p| p.exists()).count());
    drop(upload_lock);
    for _ in 0..8 {
        cleanup_orphans(&state, timestamp, &mut cursor)
            .await
            .unwrap();
    }
    assert!(later.iter().all(|p| !p.exists()));

    // Retained ReadDir must not inspect a renamed old root after replacement.
    cursor.scan = None;
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(cursor.scan.is_some());
    let replaced = outside.join("replaced-root");
    fs::rename(&root, &replaced).unwrap();
    let old_only = old_directory(&replaced, &Uuid::new_v4().to_string(), timestamp);
    DirBuilder::new().mode(0o700).create(&root).unwrap();
    let replacement_orphan = old_directory(&root, &Uuid::new_v4().to_string(), timestamp);
    cleanup_orphans(&state, timestamp, &mut cursor)
        .await
        .unwrap();
    assert!(!replacement_orphan.exists() && old_only.exists());
    assert!(external.join("sentinel").exists());
}
