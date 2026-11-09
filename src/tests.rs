use super::*;
use serde_json::json;
use std::os::unix::process::CommandExt;
use std::sync::atomic::Ordering;
fn job() -> Job {
    Job {
        title: String::new(),
        url: "https://example.com/video?a=1&b=$(echo nope)".into(),
        settings: Settings::default(),
        status: String::new(),
        progress: 0.,
        detail: String::new(),
    }
}
#[test]
fn appearance_cycle_and_system_following() {
    assert_eq!(Appearance::Automatic.next(), Appearance::Light);
    assert_eq!(Appearance::Light.next(), Appearance::Dark);
    assert_eq!(Appearance::Dark.next(), Appearance::Automatic);
    let ctx = egui::Context::default();
    configure_styles(&ctx);
    for mode in [Appearance::Automatic, Appearance::Light, Appearance::Dark] {
        let settings = Settings {
            appearance: mode,
            ..Default::default()
        };
        let saved: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(saved.appearance, mode);
        ctx.set_theme(mode.preference());
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let _ = ctx.run(
                egui::RawInput {
                    system_theme: Some(theme),
                    ..Default::default()
                },
                |_| {},
            );
            assert_eq!(
                ctx.theme(),
                match mode {
                    Appearance::Automatic => theme,
                    Appearance::Dark => egui::Theme::Dark,
                    Appearance::Light => egui::Theme::Light,
                }
            );
        }
    }
}
#[test]
fn old_preferences_load_and_removed_subtitles_are_ignored() {
    let s: Settings =
        serde_json::from_str(r#"{"folder":"/tmp/existing","mp4":true,"subtitles":true}"#).unwrap();
    assert_eq!(s.folder, "/tmp/existing");
    assert_eq!(s.compatibility, Compatibility::Apple);
    assert!(s.audio_language.is_none());
    assert!(!serde_json::to_string(&s).unwrap().contains("subtitles"));
}
#[test]
fn selected_language_never_falls_back_to_another_language() {
    let mut j = job();
    j.settings.audio_language = Some("es".into());
    let a = engine::download_args(&j).unwrap();
    let format = &a[a.iter().position(|s| s == "-f").unwrap() + 1];
    assert_eq!(
        format,
        "bv[height<=?2160]+ba[language=es]/b[height<=?2160][language=es]"
    );
    j.settings.audio = true;
    let a = engine::download_args(&j).unwrap();
    assert!(a.contains(&"ba[language=es]/b[language=es]".into()));
    j.settings.audio_language = Some("es]/ba".into());
    assert!(engine::download_args(&j).is_err());
}
#[test]
fn tracks_deduplicate_formats_and_identify_original() {
    let info = engine::parse_info(&json!({"title":"Demo","formats":[
        {"acodec":"aac","language":"es"},{"acodec":"opus","language":"es"},
        {"acodec":"opus","language":"de","format_note":"German original"},
        {"acodec":"none","language":"fr"},{"acodec":"aac","language":"bad]/x"}
    ]}))
    .unwrap();
    assert_eq!(info.tracks.len(), 2);
    assert_eq!(info.tracks[0].code, "de");
    assert!(info.tracks[0].original);
    assert_eq!(info.tracks[1].label, "Spanish (es)");
}
#[test]
fn codec_modes_copy_or_convert_and_use_apple_tags() {
    let source = std::path::Path::new("input.mkv");
    let target = std::path::Path::new("out.mp4");
    let compatible = json!({"streams":[{"codec_type":"video","codec_name":"h264","pix_fmt":"yuv420p","height":1080},{"codec_type":"audio","codec_name":"aac"}]});
    let a = engine::conversion_args(source, target, &compatible, Compatibility::Apple).unwrap();
    assert!(a.windows(2).any(|p| p == ["-c:v", "copy"]));
    assert!(a.windows(2).any(|p| p == ["-tag:v", "avc1"]));
    let a = engine::conversion_args(source, target, &compatible, Compatibility::Hevc).unwrap();
    assert!(a.contains(&"hevc_videotoolbox".into()));
    assert!(a.contains(&"hvc1".into()));
    let incompatible = json!({"streams":[{"codec_type":"video","codec_name":"vp9","height":2160},{"codec_type":"audio","codec_name":"opus"}]});
    let a = engine::conversion_args(source, target, &incompatible, Compatibility::Apple).unwrap();
    assert!(a.contains(&"h264_videotoolbox".into()));
    assert!(a.windows(2).any(|p| p == ["-c:a", "aac"]));
}
#[test]
fn original_quality_does_not_request_recoding_and_url_is_literal() {
    let mut j = job();
    j.settings.compatibility = Compatibility::Original;
    let a = engine::download_args(&j).unwrap();
    assert_eq!(a[a.len() - 2], "--");
    assert_eq!(a.last().unwrap(), &j.url);
    assert!(!a.contains(&"-S".into()));
    assert!(!a
        .iter()
        .any(|s| s.contains(".source") || s.contains("subs") || s.contains("recode")));
}
#[test]
fn cancellation_terminates_process_group() {
    let mut child = Command::new("/bin/sleep")
        .arg("30")
        .process_group(0)
        .spawn()
        .unwrap();
    let r = Running::new(0);
    *r.pid.lock().unwrap() = Some(child.id());
    stop(&r);
    assert!(!child.wait().unwrap().success());
    assert!(r.cancel.load(Ordering::SeqCst));
}
#[test]
fn urls_and_progress_handle_noise() {
    for s in [
        "file:///etc/passwd",
        "--exec=touch /tmp/x",
        "https://user:pass@example.com/a",
        "",
    ] {
        assert!(!valid_url(s));
    }
    assert!(valid_url("https://example.com/watch?v=1&x=2"));
    assert_eq!(progress("PROGRESS|42.5%|2MiB/s|00:03").unwrap().0, 0.425);
    assert!(progress("PROGRESS|N/A|N/A|N/A").is_none());
}
