use super::{Compatibility, Event, Job};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::{
    collections::BTreeMap,
    io::Read,
    os::fd::AsRawFd,
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
    pub discard: Arc<AtomicBool>,
    pub pid: Arc<Mutex<Option<u32>>>,
}
impl Running {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            cancel: Arc::new(AtomicBool::new(false)),
            discard: Arc::new(AtomicBool::new(false)),
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
            // Also stop the direct child if process-group signalling is unavailable.
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}
pub fn cancel(r: &Running) {
    r.discard.store(true, Ordering::SeqCst);
    stop(r);
}
pub fn new_job_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{:x}-{:x}-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
pub fn work_path(job: &Job) -> Result<PathBuf, String> {
    if job.id.is_empty()
        || job.id.len() > 80
        || !job.id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
    {
        return Err("Invalid download identifier".into());
    }
    Ok(Path::new(&job.settings.folder).join(format!(".hamster-work-{}", job.id)))
}
fn prepare_work(job: &Job) -> Result<PathBuf, String> {
    let path = work_path(job)?;
    match std::fs::create_dir(&path) {
        Ok(()) => {
            std::fs::write(path.join(".hamster-owned"), &job.id).map_err(|e| e.to_string())?
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => validate_work(job, &path)?,
        Err(e) => return Err(e.to_string()),
    }
    Ok(path)
}
fn validate_work(job: &Job, path: &Path) -> Result<(), String> {
    if path
        .symlink_metadata()
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
        || std::fs::read_to_string(path.join(".hamster-owned"))
            .ok()
            .as_deref()
            != Some(job.id.as_str())
    {
        return Err(
            "The temporary folder is not owned by this download; it was left untouched.".into(),
        );
    }
    Ok(())
}
pub fn cleanup(job: &Job) -> Result<(), String> {
    let path = work_path(job)?;
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    validate_work(job, &path)?;
    std::fs::remove_dir_all(path).map_err(|e| format!("Could not remove unfinished files: {e}"))
}
fn publish(source: &Path, folder: &Path, r: &Running) -> Result<PathBuf, String> {
    if r.cancel.load(Ordering::SeqCst) {
        return Err("Stopped".into());
    }
    let name = source.file_name().ok_or("Missing output filename")?;
    let mut dest = folder.join(name);
    // Never overwrite a file already in the user's destination.
    for suffix in 0..10000 {
        if suffix > 0 {
            dest = folder.join(format!(
                "{} ({suffix}).{}",
                source.file_stem().unwrap().to_string_lossy(),
                source.extension().unwrap_or_default().to_string_lossy()
            ));
        }
        match std::fs::hard_link(source, &dest) {
            Ok(()) => {
                std::fs::remove_file(source).map_err(|e| e.to_string())?;
                return Ok(dest);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Could not save completed file: {e}")),
        }
    }
    Err("Too many files with the same name".into())
}
fn read_lines<T: Read + AsRawFd>(
    mut pipe: T,
    finished: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    mut emit: impl FnMut(String),
) {
    // A subprocess may leave an inherited pipe open. Never let that prevent Cancel from finishing.
    let fd = pipe.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return;
        }
    }
    let mut buffer = [0_u8; 8192];
    let mut pending = Vec::new();
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                pending.extend_from_slice(&buffer[..n]);
                while let Some(end) = pending.iter().position(|b| *b == b'\n') {
                    let line = String::from_utf8_lossy(&pending[..end])
                        .trim_end_matches('\r')
                        .to_string();
                    pending.drain(..=end);
                    emit(line);
                }
                if pending.len() > 32 * 1024 * 1024 {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if finished.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    if !pending.is_empty() {
        emit(String::from_utf8_lossy(&pending).into());
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
    media_duration: Option<f64>,
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
    let finished = Arc::new(AtomicBool::new(false));
    let output_done = finished.clone();
    let output_cancel = r.cancel.clone();
    let send = tx.cloned();
    let out = thread::spawn(move || {
        let mut lines = Vec::new();
        let mut size = 0;
        let mut seconds = 0_f64;
        let mut speed = String::new();
        let began = Instant::now();
        read_lines(stdout, output_done, output_cancel, |line| {
            if let Some(t) = &send {
                if let Some(total) = media_duration {
                    if let Some(value) = line.strip_prefix("out_time_us=") {
                        seconds = value.parse::<f64>().unwrap_or(0.) / 1_000_000.;
                    }
                    if let Some(value) = line.strip_prefix("speed=") {
                        speed = value.trim().to_string();
                    }
                    if line.starts_with("progress=") {
                        let fraction = if total > 0. {
                            (seconds / total).clamp(0., 0.99) as f32
                        } else {
                            0.
                        };
                        let detail = format!(
                            "Converting · {:.0}s of video processed · {} · elapsed {}s",
                            seconds,
                            if speed.is_empty() { "starting" } else { &speed },
                            began.elapsed().as_secs()
                        );
                        let _ = t.send(Event::Encoding(index, fraction, detail));
                    }
                }
                let _ = t.send(Event::Line(index, line.clone()));
            }
            size += line.len();
            if size < 32 * 1024 * 1024 {
                lines.push(line);
            }
        });
        lines
    });
    let error_done = finished.clone();
    let error_cancel = r.cancel.clone();
    let send = tx.cloned();
    let err = thread::spawn(move || {
        let mut lines = std::collections::VecDeque::new();
        read_lines(stderr, error_done, error_cancel, |line| {
            if let Some(t) = &send {
                let _ = t.send(Event::Line(index, line.clone()));
            }
            if lines.len() >= 50 {
                lines.pop_front();
            }
            lines.push_back(line);
        });
        lines.into_iter().collect::<Vec<_>>().join("\n")
    });
    let start = Instant::now();
    let mut timed_out = false;
    let result = loop {
        if timeout.is_some_and(|d| start.elapsed() > d) {
            timed_out = true;
            stop(r);
        }
        if r.cancel.load(Ordering::SeqCst) {
            stop(r);
        }
        match child.try_wait() {
            Ok(Some(s)) => {
                if r.cancel.load(Ordering::SeqCst) {
                    stop(r);
                }
                break Ok(s.success());
            }
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(e) => {
                stop(r);
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    finished.store(true, Ordering::SeqCst);
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
        let result = run(
            &tool("yt-dlp"),
            &a,
            &r,
            None,
            Some(Duration::from_secs(90)),
            None,
        )
        .and_then(|o| {
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
        "-show_format".into(),
        "-of".into(),
        "json".into(),
        path.to_string_lossy().into(),
    ];
    let out = run(
        &tool("ffprobe"),
        &a,
        r,
        None,
        Some(Duration::from_secs(30)),
        None,
    )?;
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
fn positive_number(value: &Value) -> Option<f64> {
    let n = value.as_f64().or_else(|| value.as_str()?.parse().ok())?;
    (n.is_finite() && n > 0.).then_some(n)
}

// Video stream bitrate only: container bitrate includes audio, subtitles and overhead.
pub fn hevc_target_bitrate(video: &Value) -> Result<u64, String> {
    let source = positive_number(&video["bit_rate"])
        .ok_or("Cannot determine the source video bitrate. Try Original quality.")?;
    // Conservative starting policy, not a claim of equal perceptual quality.
    // VP9/AV1 are already efficient: do not apply H.264's reduction to them.
    let fraction = if video["codec_name"] == "h264" {
        0.80
    } else {
        1.0
    };
    Ok((source * fraction).round().max(1.) as u64)
}

pub fn measured_packet_bitrate(lines: &[String], duration: f64) -> Result<u64, String> {
    let mut count = 0_u64;
    let mut bytes = 0_u64;
    let mut expected = None;
    for line in lines {
        if let Some(value) = line.strip_prefix("packet|size=") {
            let size: u64 = value
                .split('|')
                .next()
                .unwrap_or("")
                .parse()
                .map_err(|_| "Invalid video packet size")?;
            bytes = bytes.checked_add(size).ok_or("Video size overflow")?;
            count += 1;
        } else if let Some(value) = line.strip_prefix("stream|nb_read_packets=") {
            expected = value.split('|').next().and_then(|s| s.parse::<u64>().ok());
        }
    }
    if expected != Some(count)
        || count == 0
        || bytes == 0
        || !duration.is_finite()
        || duration <= 0.
    {
        return Err("Could not measure the complete video bitrate. Try Original quality.".into());
    }
    Ok((bytes as f64 * 8. / duration).round().max(1.) as u64)
}

fn measure_source_bitrate(source: &Path, info: &mut Value, r: &Running) -> Result<(), String> {
    let video_index = info["streams"]
        .as_array()
        .and_then(|streams| streams.iter().position(|s| s["codec_type"] == "video"))
        .ok_or("No video stream")?;
    let video = &info["streams"][video_index];
    if video["codec_name"] == "hevc" || positive_number(&video["bit_rate"]).is_some() {
        return Ok(());
    }
    let duration = positive_number(&video["duration"])
        .or_else(|| positive_number(&info["format"]["duration"]))
        .ok_or("Cannot determine video duration. Try Original quality.")?;
    // MKV/WebM frequently omit per-stream bitrates. Count compressed video bytes,
    // never infer them from total file size (audio may dominate a small video).
    let args = [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_packets",
        "-count_packets",
        "-show_entries",
        "packet=size:stream=nb_read_packets",
        "-of",
        "compact=p=1:nk=0",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([source.to_string_lossy().into_owned()])
    .collect::<Vec<_>>();
    let out = run(&tool("ffprobe"), &args, r, None, None, None)?;
    if !out.success {
        return Err("Could not measure source video bitrate. Try Original quality.".into());
    }
    let rate = measured_packet_bitrate(&out.stdout, duration)?;
    info["streams"][video_index]["bit_rate"] = Value::String(rate.to_string());
    Ok(())
}

pub fn size_summary(before: u64, after: u64) -> String {
    let size = format!("{:.1} MB", after as f64 / 1_000_000.);
    if before == 0 {
        return size;
    }
    let change = (after as f64 / before as f64 - 1.) * 100.;
    if change.abs() < 1. {
        format!("{size} · about the same size as the source")
    } else if change < 0. {
        format!("{size} · {:.0}% smaller than the source", -change)
    } else {
        format!("{size} · {change:.0}% larger than the source; Original quality avoids re-encoding")
    }
}

const HEVC_SAMPLE_SIMILARITY: f64 = 0.97;

pub fn sample_positions(duration: f64) -> Vec<f64> {
    if duration <= 12. {
        vec![0.]
    } else {
        vec![0., (duration - 4.) / 2., duration - 4.]
    }
}

pub fn parse_similarity(stderr: &str) -> Option<f64> {
    stderr
        .lines()
        .rev()
        .filter(|line| line.contains("SSIM "))
        .find_map(|line| {
            line.split("All:")
                .nth(1)?
                .split_whitespace()
                .next()?
                .parse::<f64>()
                .ok()
        })
        .filter(|score| score.is_finite() && (0. ..=1.).contains(score))
}

fn set_bitrate(args: &mut [String], bitrate: u64) {
    if let Some(i) = args.iter().position(|s| s == "-b:v") {
        args[i + 1] = bitrate.to_string();
    }
}

fn calibrate_hevc(
    source: &Path,
    info: &Value,
    r: &Running,
    tx: &mpsc::Sender<Event>,
) -> Result<u64, String> {
    let video = info["streams"]
        .as_array()
        .and_then(|s| s.iter().find(|v| v["codec_type"] == "video"))
        .ok_or("No video stream")?;
    let duration = positive_number(&video["duration"])
        .or_else(|| positive_number(&info["format"]["duration"]))
        .ok_or("Cannot determine video duration for the HEVC quality check")?;
    let positions = sample_positions(duration);
    let seconds = duration.min(4.);
    let sample = source
        .parent()
        .ok_or("Missing download folder")?
        .join(".hamster-quality-check.mp4");
    let initial = hevc_target_bitrate(video)?;
    let mut rate = initial;
    for attempt in 0..5 {
        let mut lowest: f64 = 1.;
        for (i, offset) in positions.iter().enumerate() {
            if r.cancel.load(Ordering::SeqCst) {
                return Err("Stopped".into());
            }
            let _ = tx.send(Event::Stage(
                r.index,
                format!(
                    "Checking HEVC quality · pass {} of 5 · sample {} of {}",
                    attempt + 1,
                    i + 1,
                    positions.len()
                ),
            ));
            if sample.exists() {
                std::fs::remove_file(&sample).map_err(|e| e.to_string())?;
            }
            let mut args = conversion_args(source, &sample, info, Compatibility::Hevc)?;
            set_bitrate(&mut args, rate);
            let input = args.iter().position(|s| s == "-i").unwrap();
            args.splice(input..input, ["-ss".into(), offset.to_string()]);
            let end = args.len() - 1;
            args.splice(end..end, ["-t".into(), seconds.to_string(), "-an".into()]);
            let output = run(&tool("ffmpeg"), &args, r, None, None, None)?;
            if !output.success {
                return Err(format!(
                    "HEVC quality check could not encode a sample: {}",
                    output.stderr
                ));
            }
            // Align matching frame numbers, not rounded container timestamps.
            // Otherwise an MKV timebase can look like quality loss and inflate bitrate.
            let compare: Vec<String> = ["-hide_banner", "-nostdin", "-ss", &offset.to_string(), "-i",
                &source.to_string_lossy(), "-i", &sample.to_string_lossy(), "-filter_complex",
                "[0:v:0]setpts=N/TB[ref];[1:v:0]setpts=N/TB[encoded];[encoded][ref]ssim=shortest=1",
                "-an", "-sn", "-fps_mode", "passthrough", "-f", "null", "-"].iter().map(|s| s.to_string()).collect();
            let check = run(&tool("ffmpeg"), &compare, r, None, None, None)?;
            if !check.success {
                return Err(format!(
                    "Could not compare HEVC sample quality: {}",
                    check.stderr
                ));
            }
            let score = parse_similarity(&check.stderr)
                .ok_or("The HEVC sample quality check returned no result")?;
            lowest = lowest.min(score);
            let _ = tx.send(Event::Line(
                r.index,
                format!("HEVC sample: target {rate} bit/s, similarity {score:.4}"),
            ));
            std::fs::remove_file(&sample).map_err(|e| e.to_string())?;
            if score < HEVC_SAMPLE_SIMILARITY {
                break;
            }
        }
        if lowest >= HEVC_SAMPLE_SIMILARITY {
            return Ok(rate);
        }
        if attempt < 4 {
            rate = ((rate as f64 * 1.5).round() as u64).max(rate + 1);
        }
    }
    // Do not repeatedly spend minutes attempting to shrink an unsuitable source.
    // Keep the requested codec, favour quality, and show actual size after conversion.
    let _ = tx.send(Event::Line(r.index, "HEVC sample target was not reached within five passes; using the highest tested bitrate. Original quality avoids re-encoding loss.".into()));
    Ok(rate)
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
    let mut a: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-progress",
        "pipe:1",
        "-nostats",
        "-n",
        "-i",
    ]
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
            hevc_target_bitrate(video)?
        } else {
            rate * 1000
        };
        a.extend([
            "-allow_sw".into(),
            "1".into(),
            "-b:v".into(),
            rate.to_string(),
            "-pix_fmt".into(),
            if mode == Compatibility::Hevc
                && (video["pix_fmt"]
                    .as_str()
                    .is_some_and(|p| p.contains("10") || p.contains("12"))
                    || positive_number(&video["bits_per_raw_sample"]).is_some_and(|n| n > 8.))
            {
                "p010le".into()
            } else {
                "yuv420p".into()
            },
        ]);
        if mode == Compatibility::Hevc {
            a.extend([
                "-prio_speed".into(),
                "0".into(),
                "-realtime".into(),
                "0".into(),
            ]);
        }
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
        "-fps_mode".into(),
        "passthrough".into(),
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
    let mut info = media_info(source, r)?;
    let _ = tx.send(Event::Stage(
        r.index,
        format!("Preparing {} video…", job.settings.compatibility.short()),
    ));
    if job.settings.compatibility == Compatibility::Hevc {
        let _ = tx.send(Event::Stage(
            r.index,
            "Checking source video for efficient HEVC conversion…".into(),
        ));
        measure_source_bitrate(source, &mut info, r)?;
    }
    let mut a = conversion_args(source, &temp, &info, job.settings.compatibility)?;
    if job.settings.compatibility == Compatibility::Hevc && video_codec(&info) != Some("hevc") {
        let rate = calibrate_hevc(source, &info, r, tx)?;
        set_bitrate(&mut a, rate);
        let _ = tx.send(Event::Stage(r.index, "Converting HEVC video…".into()));
    }
    let duration = info["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.);
    let out = run(&tool("ffmpeg"), &a, r, Some(tx), None, Some(duration))?;
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
            std::fs::create_dir_all(&job.settings.folder).map_err(|e| e.to_string())?;
            let directory = prepare_work(&job)?;
            let mut working = job.clone();
            working.settings.folder = directory.to_string_lossy().into();
            let a = download_args(&working)?;
            let out = run(&tool("yt-dlp"), &a, &r, Some(&tx), None, None)?;
            if r.cancel.load(Ordering::SeqCst) {
                return Err("Stopped".into());
            }
            let paths: Vec<_> = out
                .stdout
                .iter()
                .filter_map(|s| s.strip_prefix("FILE|"))
                .map(PathBuf::from)
                .collect();
            for source in &paths {
                // Accept only output in this job's own temporary directory.
                if source.parent() != Some(directory.as_path()) {
                    return Err("Unexpected download output path".into());
                }
                let source_size = std::fs::metadata(source).map_err(|e| e.to_string())?.len();
                let ready = finish_file(source, &working, &r, &tx)?;
                let dest = publish(&ready, Path::new(&job.settings.folder), &r)?;
                let detail = if job.settings.compatibility == Compatibility::Hevc
                    && !job.settings.audio
                {
                    let output_size = std::fs::metadata(&dest).map_err(|e| e.to_string())?.len();
                    format!(
                        "{} · {}",
                        dest.display(),
                        size_summary(source_size, output_size)
                    )
                } else {
                    dest.display().to_string()
                };
                let _ = tx.send(Event::Line(index, format!("SAVED|{detail}")));
            }
            if !out.success {
                return Err(if out.stderr.is_empty() {
                    "Download failed; the selected language may be unavailable.".into()
                } else {
                    out.stderr
                });
            }
            if paths.is_empty() {
                return Err("The site did not return downloadable media".into());
            }
            Ok(())
        })();
        let stopped = r.cancel.load(Ordering::SeqCst);
        let discard = r.discard.load(Ordering::SeqCst);
        if discard || (result.is_ok() && !stopped) {
            if let Err(e) = cleanup(&job) {
                let _ = tx.send(Event::Line(index, format!("ERROR: {e}")));
                let _ = tx.send(Event::Done(index, false, false));
                return;
            }
        }
        if let Err(e) = &result {
            if !stopped {
                let _ = tx.send(Event::Line(index, format!("ERROR: {e}")));
            }
        }
        let _ = tx.send(Event::Done(index, result.is_ok(), stopped));
    });
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    fn job(folder: &Path) -> Job {
        Job {
            id: new_job_id(),
            title: String::new(),
            url: "https://example.com/video".into(),
            settings: super::super::Settings {
                folder: folder.to_string_lossy().into(),
                ..Default::default()
            },
            status: String::new(),
            progress: 0.,
            detail: String::new(),
        }
    }
    #[test]
    fn cancel_stops_process_and_only_removes_owned_partial_files() {
        let folder = std::env::temp_dir().join(format!("hamster-test-{}", new_job_id()));
        std::fs::create_dir(&folder).unwrap();
        let j = job(&folder);
        let stage = prepare_work(&j).unwrap();
        std::fs::write(stage.join("partial.mp4"), b"unfinished").unwrap();
        std::fs::write(folder.join("keep.mp4"), b"existing video").unwrap();
        let running = Running::new(0);
        let r = running.clone();
        let handle = thread::spawn(move || {
            run(
                Path::new("/bin/sh"),
                &["-c".into(), "sleep 60 & wait".into()],
                &r,
                None,
                None,
                None,
            )
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while running.pid.lock().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        let start = Instant::now();
        cancel(&running);
        let out = handle.join().unwrap().unwrap();
        assert!(!out.success);
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(running.discard.load(Ordering::SeqCst));
        cleanup(&j).unwrap();
        assert!(!stage.exists());
        assert_eq!(
            std::fs::read(folder.join("keep.mp4")).unwrap(),
            b"existing video"
        );
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn pause_keeps_resumable_files_and_unowned_cleanup_is_rejected() {
        let folder = std::env::temp_dir().join(format!("hamster-test-{}", new_job_id()));
        std::fs::create_dir(&folder).unwrap();
        let j = job(&folder);
        let stage = prepare_work(&j).unwrap();
        std::fs::write(stage.join("partial"), b"partial").unwrap();
        let r = Running::new(0);
        stop(&r);
        assert!(!r.discard.load(Ordering::SeqCst));
        assert!(stage.join("partial").exists());
        std::fs::write(stage.join(".hamster-owned"), "someone else").unwrap();
        assert!(cleanup(&j).is_err());
        assert!(stage.join("partial").exists());
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn completed_outputs_never_replace_existing_files() {
        let folder = std::env::temp_dir().join(format!("hamster-test-{}", new_job_id()));
        std::fs::create_dir(&folder).unwrap();
        let j = job(&folder);
        let stage = prepare_work(&j).unwrap();
        std::fs::write(folder.join("video.mp4"), b"existing").unwrap();
        std::fs::write(stage.join("video.mp4"), b"new").unwrap();
        let r = Running::new(0);
        let output = publish(&stage.join("video.mp4"), &folder, &r).unwrap();
        assert_eq!(output.file_name().unwrap(), "video (1).mp4");
        assert_eq!(
            std::fs::read(folder.join("video.mp4")).unwrap(),
            b"existing"
        );
        cleanup(&j).unwrap();
        assert_eq!(std::fs::read(output).unwrap(), b"new");
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn conversion_progress_emits_processed_fraction() {
        let (tx, rx) = mpsc::channel();
        let r = Running::new(3);
        let out = run(
            Path::new("/bin/sh"),
            &[
                "-c".into(),
                "printf 'out_time_us=2500000\\nspeed=1.5x\\nprogress=continue\\n'".into(),
            ],
            &r,
            Some(&tx),
            None,
            Some(10.),
        )
        .unwrap();
        assert!(out.success);
        assert!(rx
            .try_iter()
            .any(|e| matches!(e,Event::Encoding(3,p,_) if (p-0.25).abs()<0.001)));
    }
}
