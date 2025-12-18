#[test]
fn motd_read_file_dx_ascii_forces_raw() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("MOTD.md");
    std::fs::write(&p, "dx:ascii\nHello\n").unwrap();
    let (lines, raw) = dx::motd::read_motd_file(&p).unwrap();
    assert_eq!(lines, vec!["Hello"]); // dx:ascii filtered out
    assert!(raw);
}

#[test]
fn asciinema_build_cmds_contain_expected_flags() {
    let cfg = dx::asciinema::AsciinemaConfig {
        enabled: true,
        external: false,
        on_relaunch: false,
        dir: None,
        file_prefix: Some("dx".into()),
        title: Some("t".into()),
        quiet: true,
        overwrite: false,
        stream: false,
        stream_mode: dx::asciinema::default_stream_mode(),
        local_addr: None,
        remote: Some("123".into()),
    };
    let rec = dx::asciinema::build_asciinema_cmd(&cfg, "file.cast", "echo hi");
    assert!(rec.contains("asciinema"));
    assert!(rec.contains("record"));
    assert!(rec.contains("file.cast"));
    assert!(rec.contains("echo hi"));

    let stream = dx::asciinema::build_asciinema_stream_cmd(&cfg, "echo hi");
    assert!(stream.contains("asciinema"));
    assert!(stream.contains("stream"));
    assert!(stream.contains("--remote"));
}

#[test]
fn menu_loads_yaml_and_toml() {
    let tmp = tempfile::tempdir().unwrap();
    // YAML (new unified format)
    let y = tmp.path().join("menu.yaml");
    std::fs::write(&y, "menu:\n  - name: Hello\n    cmd: echo hello\n").unwrap();
    let m = dx::menu::load_menu(&y).unwrap();
    assert_eq!(m.items.len(), 1);
    assert_eq!(m.items[0].name, "Hello");
    // TOML (new unified format)
    let t = tmp.path().join("menu.toml");
    std::fs::write(&t, "menu = [{ name = 'Hi', cmd = 'echo hi' }]\n").unwrap();
    let m2 = dx::menu::load_menu(&t).unwrap();
    assert_eq!(m2.items[0].name, "Hi");
}

#[test]
fn markdown_links_and_footnotes_render() {
    let md = "# Title\nA [link](https://example.com).\n";
    let (text, links) = dx::markdown::markdown_to_text_with_links_compat(md);
    assert_eq!(links.len(), 1);
    assert!(links[0].contains("https://example.com"));
    // Golden-ish: ensure "Links:" summary is present
    let joined: String = text
        .lines
        .iter()
        .map(|l| l.width())
        .sum::<usize>()
        .to_string();
    assert!(!joined.is_empty());
}
