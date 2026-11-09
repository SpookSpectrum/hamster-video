use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};
use std::os::unix::process::CommandExt;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

const ACCENT: Color32 = Color32::from_rgb(69, 215, 183);
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
enum Appearance {
    Dark,
    Light,
    #[default]
    Automatic,
}
impl Appearance {
    fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Automatic => "Automatic",
        }
    }
    fn preference(self) -> egui::ThemePreference {
        match self {
            Self::Dark => egui::ThemePreference::Dark,
            Self::Light => egui::ThemePreference::Light,
            Self::Automatic => egui::ThemePreference::System,
        }
    }
}
fn configure_styles(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            style.spacing.item_spacing = egui::vec2(12., 12.);
            style.spacing.button_padding = egui::vec2(16., 10.);
            if theme == egui::Theme::Dark {
                style.visuals.panel_fill = Color32::from_rgb(15, 23, 32);
                style.visuals.window_fill = Color32::from_rgb(23, 33, 44);
                style.visuals.selection.bg_fill = Color32::from_rgb(25, 104, 94);
            } else {
                style.visuals.panel_fill = Color32::from_rgb(246, 248, 249);
                style.visuals.window_fill = Color32::WHITE;
                style.visuals.selection.bg_fill = Color32::from_rgb(176, 232, 221);
            }
        });
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    appearance: Appearance,
    mp4: bool,
    folder: String,
    quality: u32,
    audio: bool,
    playlist: bool,
    subtitles: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: Appearance::Automatic,
            mp4: true,
            folder: format!(
                "{}/Downloads/Hamster Video",
                std::env::var("HOME").unwrap_or_default()
            ),
            quality: 2160,
            audio: false,
            playlist: false,
            subtitles: false,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Job {
    #[serde(default)]
    title: String,
    url: String,
    settings: Settings,
    status: String,
    progress: f32,
    detail: String,
}
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    settings: Settings,
    jobs: Vec<Job>,
}
enum Event {
    Line(usize, String),
    Done(usize, bool, bool),
}
struct Running {
    index: usize,
    cancel: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
}
struct Hamster {
    icon: egui::TextureHandle,
    saved: Saved,
    input: String,
    message: String,
    queue: VecDeque<usize>,
    running: Option<Running>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
    log: VecDeque<String>,
    show_log: bool,
}
fn state_path() -> PathBuf {
    if let Ok(p) = std::env::var("HAMSTER_STATE") {
        return p.into();
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/Hamster Video/state.json")
}
fn tool(name: &str) -> PathBuf {
    if let Ok(p) = std::env::var("HAMSTER_TOOLS") {
        return PathBuf::from(p).join(name);
    }
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("../Resources/tools")
        .join(name)
}
fn valid_url(s: &str) -> bool {
    url::Url::parse(s)
        .map(|u| {
            matches!(u.scheme(), "https" | "http")
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
        .unwrap_or(false)
}
fn args(job: &Job) -> Vec<String> {
    let mut a: Vec<String> = [
        "--ignore-config",
        "--no-colors",
        "--newline",
        "--continue",
        "--no-overwrites",
        "--socket-timeout",
        "30",
        "--retries",
        "3",
        "--progress",
        "--progress-template",
        "download:PROGRESS|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s",
        "--print",
        "before_dl:TITLE|%(title)s",
        "--print",
        "after_move:FILE|%(filepath)s",
        "--no-simulate",
        "--ffmpeg-location",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push(
        tool("ffmpeg")
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string(),
    );
    a.extend([
        "--js-runtimes".into(),
        format!("deno:{}", tool("deno").display()),
        "-P".into(),
        job.settings.folder.clone(),
        "-o".into(),
        "%(title).180B [%(id)s].%(ext)s".into(),
    ]);
    a.push(
        if job.settings.playlist {
            "--yes-playlist"
        } else {
            "--no-playlist"
        }
        .into(),
    );
    if job.settings.audio {
        a.extend(
            [
                "-f",
                "bestaudio/best",
                "-x",
                "--audio-format",
                "mp3",
                "--audio-quality",
                "0",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
    } else if job.settings.mp4 {
        a.extend([
            "-f".into(),
            format!(
                "bv*[ext=mp4][height<=?{q}]+ba[ext=m4a]/b[ext=mp4][height<=?{q}]",
                q = job.settings.quality
            ),
            "--merge-output-format".into(),
            "mp4".into(),
        ]);
    } else {
        a.extend([
            "-f".into(),
            format!(
                "bv*[height<=?{q}]+ba/b[height<=?{q}]",
                q = job.settings.quality
            ),
            "--merge-output-format".into(),
            "mkv".into(),
        ]);
    }
    if job.settings.subtitles {
        a.extend(
            [
                "--write-subs",
                "--sub-langs",
                "en.*,nl.*",
                "--convert-subs",
                "srt",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
    }
    a.extend(["--".into(), job.url.clone()]);
    a
}
fn progress(line: &str) -> Option<(f32, String)> {
    let mut p = line.strip_prefix("PROGRESS|")?.split('|');
    let n = p.next()?.trim().trim_end_matches('%').parse::<f32>().ok()?;
    Some((
        (n / 100.0).clamp(0.0, 1.0),
        format!(
            "{}  ·  ETA {}",
            p.next().unwrap_or("—").trim(),
            p.next().unwrap_or("—").trim()
        ),
    ))
}
fn stop(r: &Running) {
    r.cancel.store(true, Ordering::SeqCst);
    if let Some(pid) = *r.pid.lock().unwrap() {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}
fn worker(
    index: usize,
    job: Job,
    tx: mpsc::Sender<Event>,
    cancel: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
) {
    thread::spawn(move || {
        if let Err(e) = std::fs::create_dir_all(&job.settings.folder) {
            let _ = tx.send(Event::Line(
                index,
                format!("ERROR: Cannot create destination: {e}"),
            ));
            let _ = tx.send(Event::Done(index, false, false));
            return;
        }
        let mut command = Command::new(tool("yt-dlp"));
        command
            .args(args(&job))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        match command.spawn() {
            Ok(mut child) => {
                *pid.lock().unwrap() = Some(child.id());
                if cancel.load(Ordering::SeqCst) {
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                    }
                }
                let out = child.stdout.take().unwrap();
                let err = child.stderr.take().unwrap();
                let t = tx.clone();
                let reader = thread::spawn(move || {
                    for line in BufReader::new(out).lines().map_while(Result::ok) {
                        let _ = t.send(Event::Line(index, line));
                    }
                });
                let t = tx.clone();
                let errors = thread::spawn(move || {
                    for line in BufReader::new(err).lines().map_while(Result::ok) {
                        let _ = t.send(Event::Line(index, line));
                    }
                });
                let ok = child.wait().map(|s| s.success()).unwrap_or(false);
                *pid.lock().unwrap() = None;
                let _ = reader.join();
                let _ = errors.join();
                let _ = tx.send(Event::Done(index, ok, cancel.load(Ordering::SeqCst)));
            }
            Err(e) => {
                let _ = tx.send(Event::Line(
                    index,
                    format!("ERROR: Download engine could not start: {e}"),
                ));
                let _ = tx.send(Event::Done(index, false, false));
            }
        }
    });
}
impl Hamster {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_styles(&cc.egui_ctx);
        let data = eframe::icon_data::from_png_bytes(include_bytes!("../assets/hamster-icon.png"))
            .expect("bundled icon");
        let icon = cc.egui_ctx.load_texture(
            "hamster",
            egui::ColorImage::from_rgba_unmultiplied(
                [data.width as usize, data.height as usize],
                &data.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        let mut saved: Saved = std::fs::read(state_path())
            .or_else(|_| {
                std::fs::read(
                    PathBuf::from(std::env::var("HOME").unwrap_or_default())
                        .join("Library/Application Support/Harbor Video/state.json"),
                )
            })
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        for j in &mut saved.jobs {
            if j.status == "Downloading" || j.status == "Queued" {
                j.status = "Paused".into();
                j.detail = "Interrupted last session. Resume to continue partial downloads.".into();
            }
        }
        cc.egui_ctx
            .set_theme(saved.settings.appearance.preference());
        let (tx, rx) = mpsc::channel();
        Self {
            icon,
            saved,
            input: String::new(),
            message: String::new(),
            queue: VecDeque::new(),
            running: None,
            tx,
            rx,
            log: VecDeque::new(),
            show_log: false,
        }
    }
    fn persist(&self) {
        let p = state_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_vec_pretty(&self.saved) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }
    fn add(&mut self) {
        let links: Vec<_> = self
            .input
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if links.is_empty() {
            self.message = "Paste a video link first.".into();
            return;
        }
        if links.iter().any(|s| !valid_url(s)) {
            self.message = "Use a complete http:// or https:// link, one per line.".into();
            return;
        }
        if self.saved.settings.folder.trim().is_empty()
            || !PathBuf::from(&self.saved.settings.folder).is_absolute()
        {
            self.message = "Choose an absolute destination folder.".into();
            return;
        }
        for u in links {
            let i = self.saved.jobs.len();
            self.saved.jobs.push(Job {
                title: String::new(),
                url: u,
                settings: self.saved.settings.clone(),
                status: "Queued".into(),
                progress: 0.,
                detail: "Waiting to download".into(),
            });
            self.queue.push_back(i);
        }
        self.input.clear();
        self.message.clear();
        self.persist();
    }
    fn tick(&mut self) {
        while let Ok(e) = self.rx.try_recv() {
            match e {
                Event::Line(i, line) => {
                    if self.log.len() > 300 {
                        self.log.pop_front();
                    }
                    self.log.push_back(line.clone());
                    if let Some(j) = self.saved.jobs.get_mut(i) {
                        if let Some((p, d)) = progress(&line) {
                            j.progress = p;
                            j.detail = d;
                        } else if let Some(title) = line.strip_prefix("TITLE|") {
                            j.title = title.into();
                        } else if let Some(file) = line.strip_prefix("FILE|") {
                            j.detail = file.into();
                        } else if line.contains("ERROR:") {
                            j.detail = line;
                        }
                    }
                }
                Event::Done(i, ok, cancelled) => {
                    if let Some(j) = self.saved.jobs.get_mut(i) {
                        j.status = if cancelled {
                            "Paused"
                        } else if ok {
                            "Complete"
                        } else {
                            "Failed"
                        }
                        .into();
                        if ok {
                            j.progress = 1.;
                        }
                        if cancelled {
                            j.detail = "Paused. Resume to continue available partial files.".into();
                        }
                    }
                    self.running = None;
                    self.persist();
                }
            }
        }
        if self.running.is_none() {
            if let Some(i) = self.queue.pop_front() {
                self.saved.jobs[i].status = "Downloading".into();
                let cancel = Arc::new(AtomicBool::new(false));
                let pid = Arc::new(Mutex::new(None));
                worker(
                    i,
                    self.saved.jobs[i].clone(),
                    self.tx.clone(),
                    cancel.clone(),
                    pid.clone(),
                );
                self.running = Some(Running {
                    index: i,
                    cancel,
                    pid,
                });
                self.persist();
            }
        }
    }
}
impl Drop for Hamster {
    fn drop(&mut self) {
        if let Some(r) = &self.running {
            stop(r);
        }
        self.persist();
    }
}
impl eframe::App for Hamster {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.tick();
        ctx.set_theme(self.saved.settings.appearance.preference());
        let accent = if ctx.theme() == egui::Theme::Dark {
            ACCENT
        } else {
            Color32::from_rgb(0, 108, 88)
        };
        let muted = if ctx.theme() == egui::Theme::Dark {
            Color32::from_rgb(153, 171, 188)
        } else {
            Color32::from_rgb(86, 102, 116)
        };
        ctx.request_repaint_after(Duration::from_millis(120));
        egui::SidePanel::left("sidebar")
            .exact_width(190.)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(20.);
                ui.image((self.icon.id(), egui::vec2(82., 82.)));
                ui.label(RichText::new("HAMSTER").size(27.).strong().color(accent));
                ui.label(RichText::new("V I D E O").size(13.).color(muted));
                ui.add_space(36.);
                ui.label(RichText::new("A home for your videos.").size(16.));
                ui.add_space(20.);
                ui.separator();
                ui.label(format!("{} downloads", self.saved.jobs.len()));
                ui.label(format!(
                    "{} complete",
                    self.saved
                        .jobs
                        .iter()
                        .filter(|j| j.status == "Complete")
                        .count()
                ));
                ui.label(format!("{} waiting", self.queue.len()));
                ui.add_space(24.);
                if ui.button("Open destination").clicked() {
                    let _ = std::fs::create_dir_all(&self.saved.settings.folder);
                    let _ = Command::new("/usr/bin/open")
                        .arg(&self.saved.settings.folder)
                        .spawn();
                }
                ui.checkbox(&mut self.show_log, "Engine log");
                ui.add_space(18.);
                ui.label("Appearance");
                let before = self.saved.settings.appearance;
                egui::ComboBox::from_id_salt("appearance")
                    .width(145.)
                    .selected_text(before.label())
                    .show_ui(ui, |ui| {
                        for mode in [Appearance::Automatic, Appearance::Light, Appearance::Dark] {
                            ui.selectable_value(
                                &mut self.saved.settings.appearance,
                                mode,
                                mode.label(),
                            );
                        }
                    });
                if before != self.saved.settings.appearance {
                    ctx.set_theme(self.saved.settings.appearance.preference());
                    self.persist();
                }
                if self.saved.settings.appearance == Appearance::Automatic {
                    ui.label(RichText::new("Follows your Mac").small().color(muted));
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(18.);
                    ui.label(
                        RichText::new("Hamster Video 0.1\nIndependent Rust app")
                            .small()
                            .color(muted),
                    );
                });
            });
        egui::CentralPanel::default().show(ctx,|ui|{
   ui.add_space(20.);ui.heading(RichText::new("Bring your videos home.").size(30.));ui.label(RichText::new("Save a link. Choose your quality. Keep watching offline.").color(muted));ui.add_space(16.);
   egui::Frame::group(ui.style()).inner_margin(18.).show(ui,|ui|{
    ui.horizontal(|ui|{ui.label(RichText::new("NEW DOWNLOAD").strong().color(accent));if ui.small_button("Paste clipboard").clicked(){if let Ok(o)=Command::new("/usr/bin/pbpaste").output(){self.input=String::from_utf8_lossy(&o.stdout).trim().into();}}});
    ui.add(egui::TextEdit::multiline(&mut self.input).hint_text("https://…\nOne video or playlist link per line").desired_rows(2).desired_width(f32::INFINITY));
    ui.horizontal_wrapped(|ui|{
     ui.label("Save as");egui::ComboBox::from_id_salt("format").selected_text(if self.saved.settings.audio{"MP3 audio"}else{"Video"}).show_ui(ui,|ui|{ui.selectable_value(&mut self.saved.settings.audio,false,"Video");ui.selectable_value(&mut self.saved.settings.audio,true,"MP3 audio");});
     ui.add_enabled_ui(!self.saved.settings.audio,|ui|{ui.checkbox(&mut self.saved.settings.mp4,"MP4").on_hover_text("Uncheck for MKV and broader codec availability. Files without reported resolution retain their original quality.");egui::ComboBox::from_id_salt("quality").selected_text(format!("Up to {}p",self.saved.settings.quality)).show_ui(ui,|ui|{for (n,s) in [(2160,"4K · 2160p"),(1440,"1440p"),(1080,"1080p"),(720,"720p"),(480,"480p")]{ui.selectable_value(&mut self.saved.settings.quality,n,s);}});});
     ui.checkbox(&mut self.saved.settings.playlist,"Whole playlist");ui.checkbox(&mut self.saved.settings.subtitles,"Subtitles (EN/NL)");
    });
    ui.horizontal(|ui|{ui.label("Folder");ui.add(egui::TextEdit::singleline(&mut self.saved.settings.folder).desired_width((ui.available_width()-100.).max(100.)));if ui.button("Choose…").clicked(){if let Ok(o)=Command::new("/usr/bin/osascript").args(["-e","POSIX path of (choose folder with prompt \"Save Hamster downloads to\")"]).output(){if o.status.success(){self.saved.settings.folder=String::from_utf8_lossy(&o.stdout).trim().into();self.persist();}}}});
    ui.horizontal(|ui|{if ui.add(egui::Button::new(RichText::new("Add to downloads").strong().color(Color32::from_rgb(10,30,26))).fill(ACCENT)).clicked(){self.add();}ui.label(RichText::new("Quality depends on the source.").small().color(muted));});
    if !self.message.is_empty(){ui.colored_label(Color32::from_rgb(255,180,110),&self.message);}
   });
   ui.add_space(12.);ui.horizontal(|ui|{ui.heading("Downloads");if ui.small_button("Clear finished").clicked() && self.running.is_none() && self.queue.is_empty(){self.saved.jobs.retain(|j|j.status!="Complete");self.persist();}});
   if self.show_log {egui::CollapsingHeader::new("Download engine output").default_open(true).show(ui,|ui|{egui::ScrollArea::vertical().id_salt("logs").max_height(130.).stick_to_bottom(true).show(ui,|ui|{for l in &self.log{ui.monospace(l);}});});}
   let mut resume=None;let mut pause=None;
   egui::ScrollArea::vertical().id_salt("jobs").show(ui,|ui|{
    if self.saved.jobs.is_empty(){ui.add_space(35.);ui.vertical_centered(|ui|{ui.label(RichText::new("Your hamster is ready.").size(23.).color(muted));ui.label("Paste your first link above to begin.");});}
    for (i,j) in self.saved.jobs.iter().enumerate().rev(){egui::Frame::group(ui.style()).inner_margin(14.).show(ui,|ui|{
     ui.horizontal(|ui|{ui.label(RichText::new(&j.status).strong().color(if j.status=="Failed"{Color32::from_rgb(255,145,130)}else{ACCENT}));ui.label(if j.settings.audio{"MP3".into()}else{format!("≤ {}p",j.settings.quality)});ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ["Paused","Failed"].contains(&j.status.as_str()){if ui.small_button("Resume / retry").clicked(){resume=Some(i);}}if ["Downloading","Queued"].contains(&j.status.as_str()) && ui.small_button("Pause").clicked(){pause=Some(i);}if j.status=="Complete" && ui.small_button("Show folder").clicked(){let _=Command::new("/usr/bin/open").arg(&j.settings.folder).spawn();}});});
     if !j.title.is_empty(){ui.label(RichText::new(&j.title).size(16.).strong());}ui.add(egui::Label::new(RichText::new(&j.url).size(14.)).truncate());ui.add(egui::ProgressBar::new(j.progress).fill(ACCENT).show_percentage().animate(j.status=="Downloading"));ui.add(egui::Label::new(RichText::new(&j.detail).small().color(muted)).wrap());
    });}
   });
   if let Some(i)=resume{self.saved.jobs[i].status="Queued".into();self.queue.push_back(i);self.persist();}
   if let Some(i)=pause{if let Some(r)=&self.running {if r.index==i{stop(r);}}if self.queue.contains(&i){self.queue.retain(|n|*n!=i);self.saved.jobs[i].status="Paused".into();self.persist();}}
  });
    }
}
fn main() -> eframe::Result {
    let cli: Vec<String> = std::env::args().collect();
    if cli.get(1).map(String::as_str) == Some("--check-download") {
        if cli.len() < 4 || !valid_url(&cli[2]) {
            eprintln!("Usage: hamster-video --check-download URL ABSOLUTE_FOLDER [--audio]");
            std::process::exit(2);
        }
        let mut settings = Settings::default();
        settings.folder = cli[3].clone();
        settings.audio = cli.iter().any(|s| s == "--audio");
        if let Some(q) = cli.iter().find_map(|s| s.strip_prefix("--quality=")) {
            settings.quality = q.parse().unwrap_or(2160);
        }
        settings.mp4 = !cli.iter().any(|s| s == "--mkv");
        let job = Job {
            title: String::new(),
            url: cli[2].clone(),
            settings,
            status: String::new(),
            progress: 0.,
            detail: String::new(),
        };
        let (tx, rx) = mpsc::channel();
        worker(
            0,
            job,
            tx,
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(None)),
        );
        loop {
            match rx.recv_timeout(Duration::from_secs(180)) {
                Ok(Event::Line(_, s)) => println!("{s}"),
                Ok(Event::Done(_, ok, _)) => std::process::exit(if ok { 0 } else { 1 }),
                Err(_) => std::process::exit(3),
            }
        }
    }
    eframe::run_native(
        "Hamster Video",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_icon(
                    eframe::icon_data::from_png_bytes(include_bytes!("../assets/hamster-icon.png"))
                        .expect("bundled icon"),
                )
                .with_inner_size([1080., 760.])
                .with_min_inner_size([860., 650.]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(Hamster::new(cc)))),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_round_trips_and_system_tracks_os() {
        let ctx = egui::Context::default();
        configure_styles(&ctx);
        for mode in [Appearance::Dark, Appearance::Light, Appearance::Automatic] {
            let mut settings = Settings::default();
            settings.appearance = mode;
            let saved: Settings =
                serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(saved.appearance, mode);
            ctx.set_theme(mode.preference());
            for system in [egui::Theme::Light, egui::Theme::Dark] {
                let _ = ctx.run(
                    egui::RawInput {
                        system_theme: Some(system),
                        ..Default::default()
                    },
                    |_| {},
                );
                let expected = match mode {
                    Appearance::Dark => egui::Theme::Dark,
                    Appearance::Light => egui::Theme::Light,
                    Appearance::Automatic => system,
                };
                assert_eq!(ctx.theme(), expected);
            }
        }
        let old: Settings = serde_json::from_str(r#"{"folder":"/tmp/existing"}"#).unwrap();
        assert_eq!(old.appearance, Appearance::Automatic);
        assert_eq!(old.folder, "/tmp/existing");
    }
    #[test]
    fn cancellation_terminates_own_process_group() {
        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let r = Running {
            index: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(Mutex::new(Some(child.id()))),
        };
        stop(&r);
        assert!(!child.wait().unwrap().success());
        assert!(r.cancel.load(Ordering::SeqCst));
    }
    #[test]
    fn urls_reject_local_files_and_options() {
        for s in [
            "file:///etc/passwd",
            "--exec=touch /tmp/x",
            "javascript:alert(1)",
            "https://user:pass@example.com/a",
            "",
        ] {
            assert!(!valid_url(s));
        }
        assert!(valid_url("https://example.com/watch?v=1&x=2"));
    }
    #[test]
    fn progress_handles_noise() {
        assert_eq!(progress("PROGRESS| 42.5%|2MiB/s|00:03").unwrap().0, 0.425);
        assert!(progress("PROGRESS|N/A|N/A|N/A").is_none());
        assert!(progress("WARNING: unavailable").is_none());
    }
    #[test]
    fn command_preserves_url_as_single_final_argument() {
        let j = Job {
            title: String::new(),
            url: "https://example.com/v?a=1&b=$(touch nope)".into(),
            settings: Settings::default(),
            status: String::new(),
            progress: 0.,
            detail: String::new(),
        };
        let a = args(&j);
        assert_eq!(a[a.len() - 2], "--");
        assert_eq!(a.last().unwrap(), &j.url);
        assert!(a.contains(&"--no-playlist".into()));
        assert!(
            a.contains(&"bv*[ext=mp4][height<=?2160]+ba[ext=m4a]/b[ext=mp4][height<=?2160]".into())
        );
    }
    #[test]
    fn audio_and_playlist_flags() {
        let mut j = Job {
            title: String::new(),
            url: "https://example.com".into(),
            settings: Settings::default(),
            status: String::new(),
            progress: 0.,
            detail: String::new(),
        };
        j.settings.audio = true;
        j.settings.playlist = true;
        let a = args(&j);
        assert!(a.contains(&"--yes-playlist".into()));
        assert!(a.contains(&"mp3".into()));
        assert!(!a.contains(&"--merge-output-format".into()));
    }
}
