//! Real router + PostgreSQL boundaries. Only run against a disposable test database.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use rapidmaker_auth::{AuthKey, hash_token, random_token};
use rapidmaker_storage::{OrderInput, Storage, StoredModel, account_owner};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    time::{SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;

#[derive(Clone)]
struct Actor {
    cookie: String,
    csrf: Option<String>,
}
impl Actor {
    fn session(token: &str, key: &AuthKey) -> Self {
        Self {
            cookie: format!("rapidmaker_session={token}"),
            csrf: Some(key.csrf(token)),
        }
    }
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    actor: Option<&Actor>,
    body: Option<Value>,
) -> (u16, Vec<u8>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost:18430")
        .header("origin", "http://localhost:18430");
    if let Some(actor) = actor {
        request = request.header("cookie", &actor.cookie);
        if let Some(csrf) = &actor.csrf {
            request = request.header("x-csrf-token", csrf);
        }
    }
    let body = if let Some(body) = body {
        request = request.header("content-type", "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    (
        response.status().as_u16(),
        to_bytes(response.into_body(), 2_000_000)
            .await
            .unwrap()
            .to_vec(),
    )
}
async fn session(
    pool: &PgPool,
    user: &str,
    role: &str,
    verified: bool,
    now: i64,
    key: &AuthKey,
) -> Actor {
    sqlx::query("INSERT INTO users(id,email,role,created_at) VALUES($1,$2,$3,$4) ON CONFLICT(id) DO NOTHING")
        .bind(user).bind(format!("{user}@example.invalid")).bind(role).bind(now).execute(pool).await.unwrap();
    let token = random_token().unwrap();
    let complete = role == "client" || verified;
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,last_seen_at,expires_at,admin_verified_at,stage,primary_verified_at,factor_verified_at) VALUES($1,$2,$3,$3,$4,$5,$6,$7,$7)")
        .bind(hash_token(&token).as_slice()).bind(user).bind(now).bind(now+3600)
        .bind(verified.then_some(now)).bind(if complete {"authenticated"} else {"second_factor"})
        .bind(complete.then_some(now)).execute(pool).await.unwrap();
    Actor::session(&token, key)
}
struct Fixture {
    order: String,
    model: String,
}
async fn fixture(
    db: &Storage,
    pool: &PgPool,
    user: Option<&str>,
    owner: &str,
    now: i64,
) -> Fixture {
    let model = uuid::Uuid::new_v4().to_string();
    let root =
        std::path::PathBuf::from(std::env::var_os("MODEL_STORAGE_DIR").unwrap()).join(&model);
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let source = b"private-model-canary";
    let preview = include_bytes!("../../geometry/tests/fixtures/simple.glb");
    fs::write(root.join("model.stl"), source).unwrap();
    fs::write(root.join("canonical.stl"), [0; 84]).unwrap();
    fs::write(root.join("preview.glb"), preview).unwrap();
    assert!(
        db.insert_model(&StoredModel {
            id: model.clone(),
            owner_hash: owner.into(),
            user_id: user.map(str::to_owned),
            filename: "private-filename-canary.stl".into(),
            extension: "stl".into(),
            source_bytes: source.len() as i64,
            mesh_bytes: 84,
            preview_bytes: preview.len() as i64,
            sha256: "a".repeat(64),
            dimensions_mm: [20.; 3],
            expires_at: now + 3600,
        })
        .await
        .unwrap()
    );
    let quote = uuid::Uuid::new_v4().to_string();
    db.save_quote(&quote,&json!({"owner_hash":owner,"user_id":user,"expires_at":now+600,"model_id":model,"model_sha256":"a".repeat(64),"options":{"quantity":1}}),&json!({})).await.unwrap();
    let order = uuid::Uuid::new_v4().to_string();
    assert!(
        db.hold_order_files(&order, owner, &[quote], now)
            .await
            .unwrap()
    );
    db.create_order(
        &OrderInput {
            id: &order,
            owner_hash: owner,
            user_id: user,
            contact: &json!({"email":"owner@example.invalid"}),
            lines: &json!([]),
            amount_grosze: 1000,
            provider_kind: "intent",
            provider_id: &format!("pi_{}", model.replace('-', "")),
            guest_updates: false,
        },
        now,
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO order_messages(id,order_id,operator,body,created_at) VALUES($1,$2,false,'private-chat-canary',$3)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&order).bind(now).execute(pool).await.unwrap();
    Fixture { order, model }
}

#[tokio::test]
#[ignore = "requires disposable ACCESS_TEST_DATABASE_URL and isolated AUTH/MODEL_STORAGE_DIR environment"]
async fn clients_guests_and_admins_cannot_cross_resource_boundaries() {
    let url = std::env::var("ACCESS_TEST_DATABASE_URL").unwrap();
    assert!(
        std::env::var("DATABASE_URL").unwrap() == url,
        "DATABASE_URL must match the disposable test database"
    );
    let root = std::path::PathBuf::from(std::env::var_os("MODEL_STORAGE_DIR").unwrap());
    assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("rapidmaker-access-")
    );
    let parsed = rapidmaker_auth::passkeys::Url::parse(&url).unwrap();
    assert!(parsed.path().starts_with("/rapidmaker_test_access_"));
    assert_eq!(std::env::var("AUTH_MODE").unwrap(), "localhost");
    assert_eq!(std::env::var("MAIL_MODE").unwrap(), "disabled");
    assert_eq!(std::env::var("SLICER_WORKER_MODE").unwrap(), "external");
    for name in [
        "STRIPE_SECRET_KEY",
        "FURGONETKA_SANDBOX_TOKEN",
        "METRICS_TOKEN",
        "MAIL_INBOUND_MODE",
    ] {
        assert!(
            std::env::var_os(name).is_none(),
            "unexpected provider environment"
        );
    }
    let key = AuthKey::from_hex(&std::env::var("AUTH_CODE_KEY").unwrap()).unwrap();
    let db = Storage::connect(&url).await.unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let users: Vec<_> = (0..3).map(|_| uuid::Uuid::new_v4().to_string()).collect();
    let a = session(&pool, &users[0], "client", false, now, &key).await;
    let b = session(&pool, &users[1], "client", false, now, &key).await;
    let pending = session(&pool, &users[2], "admin", false, now, &key).await;
    let admin = session(&pool, &users[2], "admin", true, now, &key).await;
    let fa = fixture(&db, &pool, Some(&users[0]), &account_owner(&users[0]), now).await;
    let fb = fixture(&db, &pool, Some(&users[1]), &account_owner(&users[1]), now).await;
    let fm = fixture(&db, &pool, Some(&users[2]), &account_owner(&users[2]), now).await;
    let guest_token = random_token().unwrap();
    let guest = Actor {
        cookie: format!("rapidmaker_guest={guest_token}"),
        csrf: None,
    };
    let guest_owner = hash_token(&guest_token)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let fg = fixture(&db, &pool, None, &guest_owner, now).await;
    let (app, workers) = rapidmaker_api::router().await.unwrap();
    for (owner, foreign, f) in [(&a, &b, &fa), (&b, &a, &fb)] {
        for path in [
            format!("/api/models/{}", f.model),
            format!("/api/models/{}/source", f.model),
            format!("/api/models/{}/preview.glb", f.model),
            format!("/api/account/orders/{}", f.order),
            format!("/api/account/orders/{}/messages", f.order),
            format!("/api/account/orders/{}/files", f.order),
            format!("/api/account/orders/{}/files/{}/source", f.order, f.model),
            format!(
                "/api/account/orders/{}/files/{}/preview.glb",
                f.order, f.model
            ),
        ] {
            assert_eq!(
                call(&app, "GET", &path, Some(owner), None).await.0,
                200,
                "owner {path}"
            );
            let (status, body) = call(&app, "GET", &path, Some(foreign), None).await;
            assert_eq!(status, 404, "foreign {path}");
            assert!(!String::from_utf8_lossy(&body).contains("canary"));
            assert_eq!(
                call(&app, "GET", &path, None, None).await.0,
                401,
                "anonymous {path}"
            );
        }
        for (path, body) in [
            (
                format!("/api/account/orders/{}/messages", f.order),
                json!({"body":"forbidden"}),
            ),
            (
                format!("/api/account/orders/{}/messages/read", f.order),
                json!({"sequence":0}),
            ),
        ] {
            assert_eq!(
                call(&app, "POST", &path, Some(foreign), Some(body)).await.0,
                404,
                "foreign mutation {path}"
            );
        }
    }
    for (actor, own, foreign) in [(&a, &fa, &fb), (&b, &fb, &fa)] {
        let (status, body) = call(&app, "GET", "/api/account/orders", Some(actor), None).await;
        assert_eq!(status, 200);
        let list: Value = serde_json::from_slice(&body).unwrap();
        let ids: Vec<_> = list["orders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&own.order.as_str()) && !ids.contains(&foreign.order.as_str()));
    }
    let mut invalid_session = Actor::session(&random_token().unwrap(), &key);
    invalid_session
        .cookie
        .push_str(&format!("; {}", guest.cookie));
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/models/{}", fg.model),
            Some(&invalid_session),
            None
        )
        .await
        .0,
        401
    );
    let wrong_guest = Actor {
        cookie: format!("rapidmaker_guest={}", random_token().unwrap()),
        csrf: None,
    };
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/models/{}", fg.model),
            Some(&wrong_guest),
            None
        )
        .await
        .0,
        404
    );
    let own_messages = format!("/api/account/orders/{}/messages", fa.order);
    let mut no_csrf = a.clone();
    no_csrf.csrf = None;
    assert_eq!(
        call(
            &app,
            "POST",
            &own_messages,
            Some(&no_csrf),
            Some(json!({"body":"blocked"}))
        )
        .await
        .0,
        403
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &own_messages,
            Some(&a),
            Some(json!({"body":"allowed"}))
        )
        .await
        .0,
        200
    );
    for path in [
        "/api/admin/orders".into(),
        "/api/admin/materials".into(),
        format!("/api/admin/orders/{}/messages", fa.order),
        format!("/api/admin/orders/{}/files", fa.order),
        format!("/api/admin/orders/{}/refunds", fa.order),
    ] {
        for actor in [&a, &b, &pending] {
            assert_eq!(
                call(&app, "GET", &path, Some(actor), None).await.0,
                if std::ptr::eq(actor, &pending) {
                    401
                } else {
                    403
                },
                "admin boundary {path}"
            );
        }
        assert_eq!(
            call(&app, "GET", &path, Some(&admin), None).await.0,
            200,
            "verified admin {path}"
        );
    }
    for (path, body) in [
        (
            format!("/api/admin/orders/{}/status", fa.order),
            json!({"status":"in_production"}),
        ),
        (
            format!("/api/admin/orders/{}/messages", fa.order),
            json!({"body":"blocked"}),
        ),
        (
            format!("/api/admin/orders/{}/refunds", fa.order),
            json!({"request_id":uuid::Uuid::new_v4().to_string(),"amount_grosze":100}),
        ),
    ] {
        for actor in [&a, &pending] {
            assert_eq!(
                call(&app, "POST", &path, Some(actor), Some(body.clone()))
                    .await
                    .0,
                if std::ptr::eq(actor, &pending) {
                    401
                } else {
                    403
                },
                "admin mutation {path}"
            );
        }
        let mut missing = admin.clone();
        missing.csrf = None;
        assert_eq!(
            call(&app, "POST", &path, Some(&missing), Some(body))
                .await
                .0,
            403,
            "admin CSRF {path}"
        );
    }
    let admin_message = format!("/api/admin/orders/{}/messages", fa.order);
    assert_eq!(
        call(
            &app,
            "POST",
            &admin_message,
            Some(&admin),
            Some(json!({"body":"admin-allowed"}))
        )
        .await
        .0,
        200
    );
    let blocked = Request::builder()
        .method("POST")
        .uri(&admin_message)
        .header("host", "localhost:18430")
        .header("origin", "http://attacker.invalid")
        .header("cookie", &admin.cookie)
        .header("x-csrf-token", admin.csrf.as_ref().unwrap())
        .header("content-type", "application/json")
        .body(Body::from(r#"{"body":"blocked"}"#))
        .unwrap();
    assert_eq!(
        app.clone()
            .oneshot(blocked)
            .await
            .unwrap()
            .status()
            .as_u16(),
        403
    );
    // Incomplete authentication is not a business session: it must finish the second factor.
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/account/orders/{}", fm.order),
            Some(&pending),
            None
        )
        .await
        .0,
        401
    );
    let mut pending_guest = pending.clone();
    pending_guest
        .cookie
        .push_str(&format!("; {}", guest.cookie));
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/auth/claim-guest",
            Some(&pending_guest),
            Some(json!({}))
        )
        .await
        .0,
        401
    );
    // Claim needs both authenticated account and the original guest cookie. Knowing IDs is insufficient.
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/auth/claim-guest",
            Some(&b),
            Some(json!({}))
        )
        .await
        .0,
        401
    );
    let guest_path = format!("/api/account/orders/{}", fg.order);
    assert_eq!(
        call(&app, "GET", &guest_path, Some(&guest), None).await.0,
        200
    );
    assert_eq!(call(&app, "GET", &guest_path, Some(&b), None).await.0, 404);
    let mut claiming = a.clone();
    claiming.cookie.push_str(&format!("; {}", guest.cookie));
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/auth/claim-guest",
            Some(&claiming),
            Some(json!({}))
        )
        .await
        .0,
        200
    );
    for suffix in ["", "/source", "/preview.glb"] {
        let path = format!("/api/models/{}{suffix}", fg.model);
        assert_eq!(call(&app, "GET", &path, Some(&a), None).await.0, 200);
        assert_eq!(call(&app, "GET", &path, Some(&guest), None).await.0, 404);
    }
    assert_eq!(
        call(&app, "GET", &guest_path, Some(&guest), None).await.0,
        404
    );
    assert_eq!(call(&app, "GET", &guest_path, Some(&a), None).await.0, 200);
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/account/orders/{}/files/{}/source", fa.order, fb.model),
            Some(&a),
            None
        )
        .await
        .0,
        410
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/account/orders/{}/files/{}/mesh", fa.order, fa.model),
            Some(&a),
            None
        )
        .await
        .0,
        404
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/admin/orders/{}/files/{}/mesh", fa.order, fa.model),
            Some(&admin),
            None
        )
        .await
        .0,
        200
    );
    let b_hash = hash_token(b.cookie.strip_prefix("rapidmaker_session=").unwrap());
    sqlx::query("UPDATE sessions SET expires_at=$2 WHERE token_hash=$1")
        .bind(b_hash.as_slice())
        .bind(now - 1)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/account/orders/{}", fb.order),
            Some(&b),
            None
        )
        .await
        .0,
        401
    );
    sqlx::query("UPDATE sessions SET expires_at=$2 WHERE token_hash=$1")
        .bind(b_hash.as_slice())
        .bind(now + 3600)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET disabled_at=$2 WHERE id=$1")
        .bind(&users[1])
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/api/account/orders/{}", fb.order),
            Some(&b),
            None
        )
        .await
        .0,
        401
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM payment_refunds")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    workers.shutdown().await;
    pool.close().await;
}
