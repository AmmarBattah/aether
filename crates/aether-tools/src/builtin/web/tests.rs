use super::*;

#[test]
fn html_becomes_readable_text_without_script_bodies() {
    let html = "<html><head><style>b{x:1}</style><script>alert('no')</script></head>\
                <body><h1>Title</h1><p>Some &amp; text</p></body></html>";
    let text = html_to_text(html);
    assert!(text.contains("Title"));
    assert!(text.contains("Some & text"));
    assert!(!text.contains("alert"), "{text}");
    assert!(!text.contains("x:1"), "{text}");
}

#[test]
fn an_element_whose_name_merely_starts_with_script_is_kept() {
    let text = html_to_text("<p>before</p><scriptish>kept</scriptish><p>after</p>");
    assert!(text.contains("kept"), "{text}");
    assert!(text.contains("before") && text.contains("after"), "{text}");
    // The real thing still goes.
    assert!(!html_to_text("<script >gone</script>").contains("gone"));
    assert!(!html_to_text("<script>gone</script>").contains("gone"));
    assert!(!html_to_text("<script type=\"x\">gone</script>").contains("gone"));
}

#[tokio::test]
async fn only_public_http_targets_are_fetched() {
    for bad in [
        "file:///etc/passwd",
        "ftp://example.com/x",
        "http://127.0.0.1:8080/",
        "http://localhost/",
        "http://169.254.169.254/latest/meta-data",
        "http://10.0.0.1/",
        "http://[::1]/",
    ] {
        assert!(safe_url(bad).await.is_err(), "{bad} must not be fetchable");
    }
}

#[tokio::test]
async fn an_ipv6_literal_reaches_the_address_check_rather_than_the_resolver() {
    // `host_str` brackets these, and no resolver accepts `[::1]`, so
    // before this they were refused as unresolvable - right answer,
    // wrong reason, and it proved nothing about `is_public`.
    for bad in [
        "http://[::1]/",
        "http://[::ffff:127.0.0.1]/",
        "http://[fd00::1]/",
    ] {
        let err = safe_url(bad).await.expect_err(bad);
        assert!(
            matches!(err, ToolError::Denied(_)),
            "{bad} must be refused as a private address, not as unresolvable: {err:?}"
        );
    }
}
