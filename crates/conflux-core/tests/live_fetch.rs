//! Live subscription fetch test.
//!
//! Run manually with a private subscription URL (never commit real tokens):
//!   set CONFLUX_TEST_SUBSCRIPTION_URL=https://example.com/your-token
//!   cargo test -p conflux-core --test live_fetch -- --ignored --nocapture

use conflux_core::fetch_and_normalize;

#[tokio::test]
#[ignore = "requires CONFLUX_TEST_SUBSCRIPTION_URL environment variable"]
async fn live_fetch_parses_nodes() {
    let url = std::env::var("CONFLUX_TEST_SUBSCRIPTION_URL")
        .expect("set CONFLUX_TEST_SUBSCRIPTION_URL to a HTTPS subscription URL");

    let profile = fetch_and_normalize(&url)
        .await
        .expect("fetch and normalize subscription");

    assert!(
        !profile.nodes.is_empty(),
        "expected at least one node, got title={:?}",
        profile.title
    );

    let protocols: std::collections::BTreeMap<String, usize> =
        profile
            .nodes
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut acc, node| {
                *acc.entry(format!("{:?}", node.protocol)).or_insert(0) += 1;
                acc
            });

    eprintln!("title: {}", profile.title);
    eprintln!("nodes: {}", profile.nodes.len());
    eprintln!("protocols: {protocols:?}");

    if let Some(info) = &profile.user_info {
        eprintln!(
            "quota: up={} down={} total={} expire={:?}",
            info.upload_bytes, info.download_bytes, info.total_bytes, info.expire_unix
        );
    }
}
