use super::{Compatibility, Event, Job};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Running {
    pub index: usize,
    pub cancel: Arc<AtomicBool>,
    pub pid: Arc<Mutex<Option<u32>>>,
}
impl Running {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            cancel: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(Mutex::new(None)),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioTrack {
    pub code: String,
    pub label: String,
    pub original: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaInfo {
    pub title: String,
    pub tracks: Vec<AudioTrack>,
}

pub fn tool(name: &str) -> PathBuf {
    if let Ok(p) = std::env::var("HAMSTER_TOOLS") {
        return PathBuf::from(p).join(name);
    }
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(Path::new("."))
        .join("../Resources/tools")
        .join(name)
}
pub fn valid_url(s: &str) -> bool {
    url::Url::parse(s)
        .map(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
        .unwrap_or(false)
}
pub fn valid_language(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 40
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
pub fn language_name(code: &str) -> String {
    let base = code.split(['-', '_']).next().unwrap_or(code);
    let name = match base {
        "en" => "English",
        "de" => "German",
        "es" => "Spanish",
        "nl" => "Dutch",
        "fr" => "French",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "zh" => "Chinese",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "ru" => "Russian",
        "pl" => "Polish",
        "tr" => "Turkish",
        "uk" => "Ukrainian",
        "id" => "Indonesian",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "sv" => "Swedish",
        "da" => "Danish",
        "no" => "Norwegian",
        "fi" => "Finnish",
        "cs" => "Czech",
        "ro" => "Romanian",
        "hu" => "Hungarian",
        "el" => "Greek",
        "he" => "Hebrew",
        "ta" => "Tamil",
        "bn" => "Bengali",
        _ => code,
    };
    format!("{name} ({code})")
}
pub fn parse_info(value: &Value) -> Result<MediaInfo, String> {
    let value = if let Some(entries) = value.get("entries").and_then(Value::as_array) {
        entries
            .iter()
            .find(|v| v.get("formats").is_some())
            .ok_or("No video information in this playlist")?
    } else {
        value
    };
    let mut tracks: BTreeMap<String, AudioTrack> = BTreeMap::new();
    for f in value
        .get("formats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if f.get("acodec")
            .and_then(Value::as_str)
            .is_none_or(|s| s == "none")
        {
            continue;
        }
        let Some(code) = f
            .get("language")
            .and_then(Value::as_str)
            .filter(|s| valid_language(s))
        else {
            continue;
        };
        let note = f
            .get("format_note")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        let original = note.contains("original") || code.ends_with("-orig");
        let track = tracks.entry(code.into()).or_insert_with(|| AudioTrack {
            code: code.into(),
            label: language_name(code),
            original: false,
        });
        track.original |= original;
    }
    let mut tracks: Vec<_> = tracks.into_values().collect();
    tracks.sort_by(|a, b| b.original.cmp(&a.original).then(a.label.cmp(&b.label)));
    Ok(MediaInfo {
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Video")
            .into(),
        tracks,
    })
}
fn base_args() -> Vec<String> {
    [
        "--ignore-config",
        "--no-colors",
        "--socket-timeout",
        "20",
        "--retries",
        "2",
        "--js-runtimes",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([format!("deno:{}", tool("deno").display())])
    .collect()
}
pub fn download_args(job: &Job) -> Result<Vec<String>, String> {
    let s = &job.settings;
    if let Some(lang) = &s.audio_language {
        if !valid_language(lang) {
            return Err("Invalid audio language selection".into());
        }
    }
    let filter = s
        .audio_language
        .as_ref()
        .map(|l| format!("[language={l}]"))
        .unwrap_or_default();
    let audio = format!("ba{filter}");
    let combined = format!("b[height<=?{}]{filter}", s.quality);
    let format = if s.audio {
        format!("{audio}/b{filter}")
    } else {
        format!("bv[height<=?{}]+{audio}/{combined}", s.quality)
    };
    let mut a = base_args();
    a.extend(["--embed-metadata","--newline","--continue","--no-overwrites","--progress","--progress-template","download:PROGRESS|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s","--print","before_dl:TITLE|%(title)s","--print","after_move:FILE|%(filepath)s","--no-simulate","--ffmpeg-location"].iter().map(|s|s.to_string()));
    a.push(tool("ffmpeg").parent().unwrap().to_string_lossy().into());
    let language = s.audio_language.as_deref().unwrap_or("default");
    let mode = if s.audio {
        "audio"
    } else {
        s.compatibility.slug()
    };
    let marker = if !s.audio && s.compatibility != Compatibility::Original {
        ".source"
    } else {
        ""
    };
    let template = format!(
        "%(title).140B [%(id)s] [{mode}-{}p-{language}]{marker}.%(ext)s",
        s.quality
    );
    a.extend([
        "-P".into(),
        s.folder.clone(),
        "-o".into(),
        template,
        "-f".into(),
        format,
    ]);
    a.push(
        if s.playlist {
            "--yes-playlist"
        } else {
            "--no-playlist"
        }
        .into(),
    );
    if s.audio {
        a.extend(
            ["-x", "--audio-format", "mp3", "--audio-quality", "0"]
                .iter()
                .map(|s| s.to_string()),
        );
    } else {
        a.extend(["--merge-output-format".into(), "mkv".into()]);
        // Prefer compatible sources at the requested resolution; conversion still verifies actual codecs.
        if s.compatibility != Compatibility::Original {
            a.extend([
                "-S".into(),
                format!(
                    "res:{},vcodec:{},acodec:aac",
                    s.quality,
                    if s.compatibility == Compatibility::Apple {
                        "h264"
                    } else {
                        "h265"
                    }
                ),
            ]);
        }
    }
    a.extend(["--".into(), job.url.clone()]);
    Ok(a)
}
pub fn progress(line: &str) -> Option<(f32, String)> {
    let mut p = line.strip_prefix("PROGRESS|")?.split('|');
    let n = p.next()?.trim().trim_end_matches('%').parse::<f32>().ok()?;
    Some((
        (n / 100.).clamp(0., 1.),
        format!(
            "{} · ETA {}",
            p.next().unwrap_or("—").trim(),
            p.next().unwrap_or("—").trim()
        ),
    ))
}
pub fn stop(r: &Running) {
    r.cancel.store(true, Ordering::SeqCst);
    if let Some(pid) = *r.pid.lock().unwrap() {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}
struct Output {
    success: bool,
    stdout: Vec<String>,
    stderr: String,
}
fn run(
    program: &Path,
    args: &[String],
    r: &Running,
    tx: Option<&mpsc::Sender<Event>>,
    timeout: Option<Duration>,
) -> Result<Output, String> {
    if r.cancel.load(Ordering::SeqCst) {
        return Err("Paused".into());
    }
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| {
            format!(
                "Could not start {}: {e}",
                program.file_name().unwrap_or_default().to_string_lossy()
            )
        })?;
    *r.pid.lock().unwrap() = Some(child.id());
    if r.cancel.load(Ordering::SeqCst) {
        stop(r);
    }
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let index = r.index;
    let send = tx.cloned();
    let out = thread::spawn(move || {
        let mut lines = Vec::new();
        let mut size = 0;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(t) = &send {
                let _ = t.send(Event::Line(index, line.clone()));
            }
            size += line.len();
            if size < 32 * 1024 * 1024 {
                lines.push(line);
            }
        }
        lines
    });
    let send = tx.cloned();
    let err = thread::spawn(move || {
        let mut lines = std::collections::VecDeque::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(t) = &send {
                let _ = t.send(Event::Line(index, line.clone()));
            }
            if lines.len() >= 50 {
                lines.pop_front();
            }
            lines.push_back(line);
        }
        lines.into_iter().collect::<Vec<_>>().join("\n")
    });
    let start = Instant::now();
    let mut timed_out = false;
    let result = loop {
        if timeout.is_some_and(|d| start.elapsed() > d) {
            timed_out = true;
            stop(r);
        }
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s.success()),
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(e) => {
                stop(r);
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    *r.pid.lock().unwrap() = None;
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if timed_out {
        return Err("The site took too long to respond. Try again.".into());
    }
    Ok(Output {
        success: result?,
        stdout,
        stderr,
    })
}
pub fn probe(url: String, r: Running, tx: mpsc::Sender<Event>) {
    thread::spawn(move || {
        let mut a = base_args();
        a.extend(
            [
                "--no-playlist",
                "--playlist-items",
                "1",
                "--skip-download",
                "--dump-single-json",
                "--no-warnings",
                "--",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
        a.push(url.clone());
        let result =
            run(&tool("yt-dlp"), &a, &r, None, Some(Duration::from_secs(90))).and_then(|o| {
                if !o.success {
                    return Err(o.stderr);
                }
                let value: Value = serde_json::from_str(&o.stdout.join("\n"))
                    .map_err(|e| format!("Could not read audio tracks: {e}"))?;
                parse_info(&value)
            });
        let _ = tx.send(Event::Metadata(r.index, url, result));
    });
}
fn media_info(path: &Path, r: &Running) -> Result<Value, String> {
    let a = vec![
        "-v".into(),
        "error".into(),
        "-show_streams".into(),
        "-of".into(),
        "json".into(),
        path.to_string_lossy().into(),
    ];
    let out = run(&tool("ffprobe"), &a, r, None, Some(Duration::from_secs(30)))?;
    if !out.success {
        return Err(format!("Cannot inspect downloaded media: {}", out.stderr));
    }
    serde_json::from_str(&out.stdout.join("\n")).map_err(|e| e.to_string())
}
fn video_codec(info: &Value) -> Option<&str> {
    info.get("streams")?
        .as_array()?
        .iter()
        .find(|s| s["codec_type"] == "video")?["codec_name"]
        .as_str()
}
pub fn conversion_args(
    source: &Path,
    target: &Path,
    info: &Value,
    mode: Compatibility,
) -> Result<Vec<String>, String> {
    let streams = info
        .get("streams")
        .and_then(Value::as_array)
        .ok_or("Missing media streams")?;
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .ok_or("No video stream was found")?;
    let codec = video["codec_name"].as_str().unwrap_or("");
    let wanted = if mode == Compatibility::Apple {
        "h264"
    } else {
        "hevc"
    };
    let copy = codec == wanted
        && (mode != Compatibility::Apple
            || matches!(
                video["pix_fmt"].as_str(),
                Some("yuv420p") | Some("yuvj420p")
            ));
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-i"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    a.push(source.to_string_lossy().into());
    a.extend(
        [
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-map_metadata",
            "0",
            "-sn",
            "-c:v",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    a.push(if copy {
        "copy".into()
    } else {
        format!("{wanted}_videotoolbox")
    });
    if !copy {
        let h = video["height"].as_u64().unwrap_or(1080);
        let rate = if h > 1440 {
            25000
        } else if h > 1080 {
            14000
        } else if h > 720 {
            8000
        } else {
            4000
        };
        let rate = if mode == Compatibility::Hevc {
            rate * 3 / 5
        } else {
            rate
        };
        a.extend([
            "-allow_sw".into(),
            "1".into(),
            "-b:v".into(),
            format!("{rate}k"),
            "-pix_fmt".into(),
            "yuv420p".into(),
        ]);
    }
    a.extend([
        "-tag:v".into(),
        if mode == Compatibility::Hevc {
            "hvc1"
        } else {
            "avc1"
        }
        .into(),
        "-c:a".into(),
        if audio.is_none_or(|s| s["codec_name"] == "aac") {
            "copy"
        } else {
            "aac"
        }
        .into(),
        "-movflags".into(),
        "+faststart".into(),
        "-f".into(),
        "mp4".into(),
        target.to_string_lossy().into(),
    ]);
    Ok(a)
}
fn finish_file(
    source: &Path,
    job: &Job,
    r: &Running,
    tx: &mpsc::Sender<Event>,
) -> Result<PathBuf, String> {
    if job.settings.audio || job.settings.compatibility == Compatibility::Original {
        return Ok(source.into());
    }
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid downloaded filename")?;
    let final_stem = stem
        .strip_suffix(".source")
        .ok_or("Unrecognized intermediate filename")?;
    let dest = source.with_file_name(format!("{final_stem}.mp4"));
    let expected = if job.settings.compatibility == Compatibility::Apple {
        "h264"
    } else {
        "hevc"
    };
    if dest.exists() {
        let info = media_info(&dest, r)?;
        if video_codec(&info) == Some(expected) {
            return Ok(dest);
        }
        return Err(
            "A different file already exists at the destination. Choose another folder.".into(),
        );
    }
    let temp = dest.with_file_name(format!("{final_stem}.converting.mp4"));
    if temp.exists() {
        std::fs::remove_file(&temp).map_err(|e| e.to_string())?;
    }
    let info = media_info(source, r)?;
    let _ = tx.send(Event::Stage(
        r.index,
        format!("Preparing {} video…", job.settings.compatibility.short()),
    ));
    let a = conversion_args(source, &temp, &info, job.settings.compatibility)?;
    let out = run(&tool("ffmpeg"), &a, r, Some(tx), None)?;
    if !out.success {
        let _ = std::fs::remove_file(&temp);
        return Err(format!(
            "Video conversion failed. Your downloaded source is kept. Try Original quality{} . {}",
            if job.settings.compatibility == Compatibility::Hevc {
                " or Apple compatible if this Mac cannot encode HEVC"
            } else {
                ""
            },
            out.stderr
        ));
    }
    let check = media_info(&temp, r)?;
    if video_codec(&check) != Some(expected) {
        return Err("The converted file does not have the requested codec".into());
    }
    if r.cancel.load(Ordering::SeqCst) {
        return Err("Paused".into());
    }
    std::fs::rename(&temp, &dest).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(source);
    Ok(dest)
}
pub fn worker(index: usize, job: Job, tx: mpsc::Sender<Event>, r: Running) {
    thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            std::fs::create_dir_all(&job.settings.folder)
                .map_err(|e| format!("Cannot create destination: {e}"))?;
            let a = download_args(&job)?;
            let out = run(&tool("yt-dlp"), &a, &r, Some(&tx), None)?;
            let paths: Vec<_> = out
                .stdout
                .iter()
                .filter_map(|s| s.strip_prefix("FILE|"))
                .map(PathBuf::from)
                .collect();
            // Successfully fetched playlist items are finalized even if a later item failed.
            for source in &paths {
                let dest = finish_file(source, &job, &r, &tx)?;
                let _ = tx.send(Event::Line(index, format!("SAVED|{}", dest.display())));
            }
            if !out.success {
                return Err(if out.stderr.is_empty() {
                    "Download failed. The selected audio language may be unavailable.".into()
                } else {
                    out.stderr
                });
            }
            if paths.is_empty() {
                return Err("The site did not return a downloadable media file".into());
            }
            Ok(())
        })();
        let cancelled = r.cancel.load(Ordering::SeqCst);
        if let Err(e) = &result {
            if !cancelled {
                let _ = tx.send(Event::Line(index, format!("ERROR: {e}")));
            }
        }
        let _ = tx.send(Event::Done(index, result.is_ok(), cancelled));
    });
}
