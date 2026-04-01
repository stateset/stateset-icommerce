use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::json;
use stateset_crypto::hash::PayloadAadParams;
use stateset_crypto::pqc::{
    encrypt_payload_hybrid, encrypt_payload_strict, generate_hybrid_recipient_keypair,
    generate_hybrid_signing_keypair, generate_hybrid_signing_pop, generate_strict_recipient_keypair,
    generate_strict_signing_keypair, generate_strict_signing_pop, hybrid_sign_event_hash,
    hybrid_verify_event_signature, strict_sign_event_hash, strict_verify_event_signature,
    unwrap_dek_hybrid, unwrap_dek_strict, wrap_dek_hybrid, wrap_dek_strict,
    PreparedHybridSigner, PreparedHybridVerifier, PreparedStrictSigner,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_hash() -> [u8; 32] {
    [0xAB; 32]
}

fn dummy_aad_params(plain_hash: &[u8; 32]) -> PayloadAadParams<'_> {
    PayloadAadParams {
        ves_version: 1,
        tenant_id: "550e8400-e29b-41d4-a716-446655440000",
        store_id: "660e8400-e29b-41d4-a716-446655440001",
        event_id: "770e8400-e29b-41d4-a716-446655440002",
        source_agent_id: "880e8400-e29b-41d4-a716-446655440003",
        agent_key_id: 1,
        entity_type: "order",
        entity_id: "990e8400-e29b-41d4-a716-446655440004",
        event_type: "order.created",
        created_at: "2026-01-01T00:00:00Z",
        payload_plain_hash: plain_hash,
    }
}

fn small_payload() -> serde_json::Value {
    json!({
        "order_id": "ord_12345",
        "customer": "cust_67890",
        "total": 199.99,
        "currency": "USD",
        "items": [
            { "sku": "WIDGET-001", "qty": 2, "price": 49.99 },
            { "sku": "GADGET-002", "qty": 1, "price": 100.01 }
        ]
    })
}

// ---------------------------------------------------------------------------
// pqc_signing — keygen, sign, verify for hybrid and strict
// ---------------------------------------------------------------------------

fn bench_pqc_signing(c: &mut Criterion) {
    let mut group = c.benchmark_group("pqc_signing");

    // -- Keygen --
    group.bench_function("hybrid_keygen", |b| {
        b.iter(|| generate_hybrid_signing_keypair().expect("hybrid keygen"));
    });

    group.bench_function("strict_keygen", |b| {
        b.iter(|| generate_strict_signing_keypair().expect("strict keygen"));
    });

    // -- Sign --
    let hybrid_kp = generate_hybrid_signing_keypair().expect("hybrid keygen setup");
    let strict_kp = generate_strict_signing_keypair().expect("strict keygen setup");
    let hash = test_hash();

    group.bench_function("hybrid_sign", |b| {
        b.iter(|| {
            hybrid_sign_event_hash(black_box(&hash), black_box(&hybrid_kp.private))
                .expect("hybrid sign")
        });
    });

    group.bench_function("strict_sign", |b| {
        b.iter(|| {
            strict_sign_event_hash(black_box(&hash), black_box(&strict_kp.private))
                .expect("strict sign")
        });
    });

    // -- Verify --
    let hybrid_sig =
        hybrid_sign_event_hash(&hash, &hybrid_kp.private).expect("hybrid sign setup");
    let strict_sig =
        strict_sign_event_hash(&hash, &strict_kp.private).expect("strict sign setup");

    group.bench_function("hybrid_verify", |b| {
        b.iter(|| {
            assert!(hybrid_verify_event_signature(
                black_box(&hash),
                black_box(&hybrid_sig),
                black_box(&hybrid_kp.public),
            ));
        });
    });

    group.bench_function("strict_verify", |b| {
        b.iter(|| {
            assert!(strict_verify_event_signature(
                black_box(&hash),
                black_box(&strict_sig),
                black_box(&strict_kp.public),
            ));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// pqc_kem — keygen, wrap, unwrap for hybrid and strict
// ---------------------------------------------------------------------------

fn bench_pqc_kem(c: &mut Criterion) {
    let mut group = c.benchmark_group("pqc_kem");

    // -- Keygen --
    group.bench_function("hybrid_recipient_keygen", |b| {
        b.iter(|| generate_hybrid_recipient_keypair(black_box(1)).expect("hybrid kem keygen"));
    });

    group.bench_function("strict_recipient_keygen", |b| {
        b.iter(|| generate_strict_recipient_keypair(black_box(1)).expect("strict kem keygen"));
    });

    // -- Wrap --
    let hybrid_rk = generate_hybrid_recipient_keypair(1).expect("hybrid rk setup");
    let strict_rk = generate_strict_recipient_keypair(2).expect("strict rk setup");
    let dek = [0x42u8; 32];
    let info = b"bench-context";

    group.bench_function("hybrid_wrap", |b| {
        b.iter(|| {
            wrap_dek_hybrid(
                black_box(&dek),
                black_box(&hybrid_rk.public),
                black_box(info.as_slice()),
            )
            .expect("hybrid wrap")
        });
    });

    group.bench_function("strict_wrap", |b| {
        b.iter(|| {
            wrap_dek_strict(
                black_box(&dek),
                black_box(&strict_rk.public),
                black_box(info.as_slice()),
            )
            .expect("strict wrap")
        });
    });

    // -- Unwrap --
    let hybrid_wrapped =
        wrap_dek_hybrid(&dek, &hybrid_rk.public, info).expect("hybrid wrap setup");
    let strict_wrapped =
        wrap_dek_strict(&dek, &strict_rk.public, info).expect("strict wrap setup");

    group.bench_function("hybrid_unwrap", |b| {
        b.iter(|| {
            unwrap_dek_hybrid(
                black_box(&hybrid_wrapped),
                black_box(&hybrid_rk.private),
                black_box(info.as_slice()),
            )
            .expect("hybrid unwrap")
        });
    });

    group.bench_function("strict_unwrap", |b| {
        b.iter(|| {
            unwrap_dek_strict(
                black_box(&strict_wrapped),
                black_box(&strict_rk.private),
                black_box(info.as_slice()),
            )
            .expect("strict unwrap")
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// pqc_payload — encrypt for hybrid and strict
// ---------------------------------------------------------------------------

fn bench_pqc_payload(c: &mut Criterion) {
    let mut group = c.benchmark_group("pqc_payload");

    let payload = small_payload();
    let zero_hash = [0u8; 32];
    let aad = dummy_aad_params(&zero_hash);

    let hybrid_rk = generate_hybrid_recipient_keypair(1).expect("hybrid rk setup");
    let strict_rk = generate_strict_recipient_keypair(2).expect("strict rk setup");

    let hybrid_recipients = [hybrid_rk.public.clone()];
    let strict_recipients = [strict_rk.public.clone()];

    group.bench_function("hybrid_encrypt_payload", |b| {
        b.iter(|| {
            encrypt_payload_hybrid(
                black_box(&payload),
                black_box(&aad),
                black_box(&hybrid_recipients),
            )
            .expect("hybrid encrypt")
        });
    });

    group.bench_function("strict_encrypt_payload", |b| {
        b.iter(|| {
            encrypt_payload_strict(
                black_box(&payload),
                black_box(&aad),
                black_box(&strict_recipients),
            )
            .expect("strict encrypt")
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// pqc_pop — proof-of-possession generation for hybrid and strict
// ---------------------------------------------------------------------------

fn bench_pqc_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("pqc_pop");

    let hybrid_kp = generate_hybrid_signing_keypair().expect("hybrid keygen setup");
    let strict_kp = generate_strict_signing_keypair().expect("strict keygen setup");

    group.bench_function("hybrid_pop_generate", |b| {
        b.iter(|| {
            generate_hybrid_signing_pop(black_box(&hybrid_kp)).expect("hybrid pop")
        });
    });

    group.bench_function("strict_pop_generate", |b| {
        b.iter(|| {
            generate_strict_signing_pop(black_box(&strict_kp)).expect("strict pop")
        });
    });

    group.finish();
}

fn bench_prepared_signers(c: &mut Criterion) {
    let mut group = c.benchmark_group("pqc_prepared");

    let hybrid_kp = generate_hybrid_signing_keypair().unwrap();
    let strict_kp = generate_strict_signing_keypair().unwrap();
    let hash = [0x42u8; 32];

    // Prepared hybrid sign (amortized key expansion)
    let hybrid_signer = PreparedHybridSigner::new(&hybrid_kp.private);
    group.bench_function("prepared_hybrid_sign", |b| {
        b.iter(|| hybrid_signer.sign(black_box(&hash)).unwrap());
    });

    // Compare: unprepared hybrid sign (key expansion every call)
    group.bench_function("unprepared_hybrid_sign", |b| {
        b.iter(|| hybrid_sign_event_hash(black_box(&hash), &hybrid_kp.private).unwrap());
    });

    // Prepared strict sign
    let strict_signer = PreparedStrictSigner::new(&strict_kp.private);
    group.bench_function("prepared_strict_sign", |b| {
        b.iter(|| strict_signer.sign(black_box(&hash)).unwrap());
    });

    // Compare: unprepared strict sign
    group.bench_function("unprepared_strict_sign", |b| {
        b.iter(|| strict_sign_event_hash(black_box(&hash), &strict_kp.private).unwrap());
    });

    // Prepared hybrid verify
    let hybrid_sig = hybrid_sign_event_hash(&hash, &hybrid_kp.private).unwrap();
    let hybrid_verifier = PreparedHybridVerifier::new(&hybrid_kp.public).unwrap();
    group.bench_function("prepared_hybrid_verify", |b| {
        b.iter(|| hybrid_verifier.verify(black_box(&hash), &hybrid_sig));
    });

    // Compare: unprepared hybrid verify
    group.bench_function("unprepared_hybrid_verify", |b| {
        b.iter(|| {
            hybrid_verify_event_signature(black_box(&hash), &hybrid_sig, &hybrid_kp.public)
        });
    });

    group.finish();
}

criterion_group!(
    pqc_signing,
    bench_pqc_signing
);
criterion_group!(
    pqc_kem,
    bench_pqc_kem
);
criterion_group!(
    pqc_payload,
    bench_pqc_payload
);
criterion_group!(
    pqc_pop,
    bench_pqc_pop
);
criterion_group!(
    pqc_prepared,
    bench_prepared_signers
);
criterion_main!(pqc_signing, pqc_kem, pqc_payload, pqc_pop, pqc_prepared);
